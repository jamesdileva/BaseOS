//! S01 UEFI guest: emits the serial-marker contract on COM1, fills a
//! [`baseos_boot_info::BootInfo`] from UEFI `GetMemoryMap` plus the GOP
//! framebuffer, prints the [`baseos_boot_info::write_boot_summary`] debug
//! summary, crosses `ExitBootServices`, then transfers the handoff to
//! [`baseos_kernel::kernel_entry`] and the S02 GDT/IDT/`#DE` bring-up.
//!
//! Minimal `no_std` UEFI application (`efi_main`): it parses the raw
//! `EFI_SYSTEM_TABLE`/`EFI_BOOT_SERVICES` tables with fixed byte offsets
//! (no `uefi` crate — dependency-free slice), calls `GetMemoryMap` into a
//! static 16 KiB buffer (retrying through an `AllocatePool` overflow buffer
//! on `EFI_BUFFER_TOO_SMALL`), locates the Graphics Output Protocol via
//! `LocateProtocol`, fills `BootInfo` via
//! [`baseos_boot_info::BootInfo::fill_from_raw_descriptors`] /
//! [`baseos_boot_info::BootInfo::fill_from_gop_mode`], reports the result
//! on COM1, exits boot services (map-key retry on `EFI_INVALID_PARAMETER`),
//! and enters the kernel. `tools/qemu/boot-uefi.ps1` builds this for
//! `x86_64-unknown-uefi`, boots OVMF, and asserts the marker plus the
//! `BOOTINFO valid=1`, `MEMMAP ok=1`, `FB present=1`, `GOP ok=1`,
//! `EBS ok=1`, `KERNEL ok=1`, and `EXC ok=1 vector=0` summary lines in the
//! serial capture.

#![cfg_attr(not(test), no_main)]
#![cfg_attr(not(test), no_std)]
// SAFETY (unsafe boundary, per `docs/coding-conventions.md`): port I/O
// (`in`/`out`), `hlt`, and raw UEFI table/service-pointer dereference have
// no safe equivalent in a `no_std` guest, so they live in this smallest
// possible hardware boundary behind the safe wrappers `uart_init` /
// `com1_write_byte` / `fill_boot_info` below. Tested end-to-end by
// `tools/qemu/boot-uefi.ps1` (OVMF boot asserts the marker and the BOOTINFO
// summary in the QEMU serial capture).
#![allow(unsafe_code)]

use baseos_boot_info::{write_boot_summary, write_str, write_u64_dec, write_u64_hex, BootInfo};
use baseos_kernel::kernel_entry;
use baseos_serial_marker::write_marker;
#[cfg(not(test))]
use core::panic::PanicInfo;
use core::{arch::asm, convert::Infallible};

/// COM1 base port (QEMU `-serial file:` backend captures these bytes).
const COM1_PORT: u16 = 0x3F8;
/// Line-status-register offset; bit 5 (THR empty) gates each write.
const COM1_LSR: u16 = COM1_PORT + 5;
/// Bit 5 of the LSR: transmitter holding register empty.
const LSR_THR_EMPTY: u8 = 0x20;

/// Byte offset of `BootServices` in `EFI_SYSTEM_TABLE` on x86-64
/// (header 24 + vendor 8 + revision 4 + pad 4 + six handles/pointers 48 +
/// `RuntimeServices` 8 = 96).
const SYSTEM_TABLE_BOOT_SERVICES_OFF: usize = 96;
/// Expected `EFI_TABLE_HEADER.Signature` for the system table (`"IBI SYST"`
/// little-endian: 0x5453595320494249). Checked before any offset deref so a
/// stale/non-UEFI pointer fails closed instead of mis-parsing (review #29
/// risk 3).
const SYSTEM_TABLE_SIGNATURE: u64 = 0x5453_5953_2049_4249;
/// Byte offset of `GetMemoryMap` in `EFI_BOOT_SERVICES`
/// (header 24 + `RaiseTPL`/`RestoreTPL`/`AllocatePages`/`FreePages` 32 = 56).
const BOOT_SERVICES_GET_MEMORY_MAP_OFF: usize = 56;
/// Byte offset of `AllocatePool` in `EFI_BOOT_SERVICES` (header 24 +
/// `RaiseTPL`/`RestoreTPL`/`AllocatePages`/`FreePages`/`GetMemoryMap`
/// 40 = 64). Used only by the `EFI_BUFFER_TOO_SMALL` map-retry dance.
const BOOT_SERVICES_ALLOCATE_POOL_OFF: usize = 64;
/// Byte offset of `LocateProtocol` in `EFI_BOOT_SERVICES` (header 24 + the
/// 37 preceding function pointers/reserved slots through
/// `LocateHandleBuffer`, 8 bytes each = 320).
const BOOT_SERVICES_LOCATE_PROTOCOL_OFF: usize = 320;
/// Byte offset of `ExitBootServices` in `EFI_BOOT_SERVICES` (header 24 +
/// the 25 preceding function pointers/reserved slots through
/// `UnloadImage`, 8 bytes each = 232). Verified against the UEFI spec table
/// layout (RaiseTPL at 24 … ExitBootServices after UnloadImage).
const BOOT_SERVICES_EXIT_BOOT_SERVICES_OFF: usize = 232;
/// `EFI_INVALID_PARAMETER` (bit 63 set + code 2): the map key from the last
/// `GetMemoryMap` is stale — the UEFI-mandated signal to refresh the key
/// and retry `ExitBootServices` once before failing closed.
const EFI_INVALID_PARAMETER: usize = 0x8000_0000_0000_0002;
/// `EFI_BUFFER_TOO_SMALL` (bit 63 set + code 5): `GetMemoryMap`'s signal
/// that the caller buffer is undersized — `MemoryMapSize` then holds the
/// required size (the input to the pool-retry dance).
const EFI_BUFFER_TOO_SMALL: usize = 0x8000_0000_0000_0005;
/// `EfiLoaderData` (type 2): pool type for the map overflow buffer —
/// reclaimable after `ExitBootServices`, never freed before it (FreePool
/// would invalidate the map key).
const EFI_LOADER_DATA: u32 = 2;
/// Bounded `GetMemoryMap` attempts in the buffer-too-small dance (initial
/// static call + pool retries). Each `AllocatePool` perturbs the map, so the
/// next call may report `EFI_BUFFER_TOO_SMALL` again — give up with the
/// raw status after this many calls instead of looping forever.
const GET_MAP_MAX_ATTEMPTS: usize = 4;
/// Slack appended to the `EFI_BUFFER_TOO_SMALL` required size when sizing
/// the `AllocatePool` overflow buffer: the allocation itself perturbs the
/// map (a free region may split and add a descriptor), so a buffer sized at
/// exactly `MemoryMapSize` can undershoot on the very next call. 4 KiB
/// covers ~85 extra 48-byte descriptors of headroom; the bounded
/// [`GET_MAP_MAX_ATTEMPTS`] loop still fails closed if firmware outgrows it.
const GET_MAP_POOL_SLACK: usize = 4096;
/// Synthetic failure `status` for structural/internal errors that have no
/// raw `EFI_STATUS` (slot load failed, GOP located but `Mode`/`Info` unusable,
/// `fill_from_gop_mode` rejected, descriptor layout invalid). Printed as
/// `0xffffffffffffffff` so serial never shows the contradictory
/// `ok=0 status=0x0` pair (agent-b #5 nit): any other status is a genuine
/// firmware `EFI_STATUS` and `0x0` only ever accompanies `ok=1`.
const STRUCTURAL_FAIL: usize = usize::MAX;
/// `EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID` (`9042a9de-23dc-4a38-96fb-7aded080516a`)
/// as a raw `EFI_GUID` (mixed-endian: `Data1`/`Data2`/`Data3` little-endian,
/// `Data4` byte array).
const GOP_GUID: [u8; 16] = [
    0xDE, 0xA9, 0x42, 0x90, 0xDC, 0x23, 0x38, 0x4A, 0x96, 0xFB, 0x7A, 0xDE, 0xD0, 0x80, 0x51, 0x6A,
];
/// Byte offset of `Mode` in `EFI_GRAPHICS_OUTPUT_PROTOCOL`
/// (`QueryMode`/`SetMode`/`Blt` 24 = 24).
const GOP_MODE_OFF: usize = 24;
/// Byte offset of `Info` in `EFI_GRAPHICS_OUTPUT_PROTOCOL_MODE`
/// (`MaxMode`/`Mode` 8).
const GOP_MODE_INFO_OFF: usize = 8;
/// Byte offset of `SizeOfInfo` in `EFI_GRAPHICS_OUTPUT_PROTOCOL_MODE`
/// (after `Info` pointer, 8-aligned).
const GOP_MODE_SIZE_OF_INFO_OFF: usize = 16;
/// Byte offset of `FrameBufferBase` in `EFI_GRAPHICS_OUTPUT_PROTOCOL_MODE`
/// (after `SizeOfInfo`).
const GOP_MODE_FB_BASE_OFF: usize = 24;
/// Byte offset of `FrameBufferSize` in `EFI_GRAPHICS_OUTPUT_PROTOCOL_MODE`
/// (after `FrameBufferBase`).
const GOP_MODE_FB_SIZE_OFF: usize = 32;
/// Minimum `EFI_GRAPHICS_OUTPUT_MODE_INFORMATION` size we parse
/// (through `PixelsPerScanLine` at +32, 4 bytes = 36).
const GOP_INFO_MIN: usize = 36;

/// Capacity of [`MEMMAP_BUF`].
const MEMMAP_BUF_LEN: usize = 16384;

/// Raw `GetMemoryMap` scratch: 16 KiB holds ~340 descriptors at the usual
/// 48-byte stride; QEMU/OVMF reports far fewer. Static (not stack) so the
/// boot stack stays small; local-only to the single-threaded boot path.
/// On `EFI_BUFFER_TOO_SMALL` the dance retries into an `AllocatePool`
/// overflow buffer instead of growing this static.
static mut MEMMAP_BUF: [u8; MEMMAP_BUF_LEN] = [0; MEMMAP_BUF_LEN];

/// Write one byte to an ISA I/O port.
///
/// # Safety
/// Caller must pass a valid, writable port for this platform (here: COM1
/// UART registers, always present on the QEMU `pc` target).
#[inline(always)]
unsafe fn outb(port: u16, value: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags),
    );
}

/// Read one byte from an ISA I/O port.
///
/// # Safety
/// Same contract as [`outb`]: caller passes a readable port (COM1 LSR).
#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!(
        "in al, dx",
        out("al") value,
        in("dx") port,
        options(nomem, nostack, preserves_flags),
    );
    value
}

/// Initialize COM1 (16550A, 115200 8N1, FIFOs on) so the marker reaches
/// QEMU's serial backend even if firmware left the UART in an odd state.
///
/// # Safety
/// Writes COM1 UART registers only; call once before any byte is written.
unsafe fn uart_init() {
    outb(COM1_PORT + 1, 0x00); // Disable interrupts.
    outb(COM1_PORT + 3, 0x80); // Enable DLAB (set baud-rate divisor).
    outb(COM1_PORT, 0x01); // Divisor low byte: 115200 baud.
    outb(COM1_PORT + 1, 0x00); // Divisor high byte.
    outb(COM1_PORT + 3, 0x03); // 8 data bits, no parity, one stop bit.
    outb(COM1_PORT + 2, 0xC7); // Enable FIFOs, clear them, 14-byte threshold.
    outb(COM1_PORT + 4, 0x0B); // DTR + RTS set, IRQs off.
}

/// Write one byte to COM1, polling for THR-empty first.
///
/// # Safety
/// COM1 must have been initialized via [`uart_init`]; polls a read-only
/// status bit, then writes the transmit register.
unsafe fn com1_write_byte(byte: u8) {
    while inb(COM1_LSR) & LSR_THR_EMPTY == 0 {}
    outb(COM1_PORT, byte);
}

/// COM1 byte sink matching the `write_marker` / `write_boot_summary`
/// callback style (infallible: UART polling cannot fail).
fn com1_emit(byte: u8) -> Result<(), Infallible> {
    // SAFETY: single-threaded boot environment, COM1 is ours exclusively;
    // `uart_init` ran once in `efi_main` before any emission.
    unsafe {
        com1_write_byte(byte);
    }
    Ok(())
}

/// Raw `ExitBootServices` signature (x86-64 UEFI = `efiapi` calling
/// convention): `(ImageHandle, MapKey) -> EFI_STATUS`.
type ExitBootServicesFn = unsafe extern "efiapi" fn(*const (), usize) -> usize;

/// Load the `ExitBootServices` entry point from the raw system table.
///
/// # Safety
/// Same contract as [`load_get_memory_map`]: live `EFI_SYSTEM_TABLE`,
/// signature-gated via [`load_boot_services`], null-slot fails closed.
unsafe fn load_exit_boot_services(system_table: *const u8) -> Option<ExitBootServicesFn> {
    let boot_services = load_boot_services(system_table)?;
    let slot = (boot_services.add(BOOT_SERVICES_EXIT_BOOT_SERVICES_OFF) as *const *const ())
        .read_unaligned();
    if slot.is_null() {
        return None;
    }
    Some(core::mem::transmute::<*const (), ExitBootServicesFn>(slot))
}

/// Raw `GetMemoryMap` signature (x86-64 UEFI = `efiapi` calling convention).
type GetMemoryMapFn = unsafe extern "efiapi" fn(
    *mut usize, // MemoryMapSize (in/out bytes)
    *mut u8,    // Buffer
    *mut usize, // MapKey (out)
    *mut usize, // DescriptorSize (out)
    *mut u32,   // DescriptorVersion (out)
) -> usize;

/// Load the `EFI_BOOT_SERVICES` pointer from the raw system table.
///
/// # Safety
/// `system_table` must point to a live `EFI_SYSTEM_TABLE` or be null;
/// validates the header signature before reading `BootServices` at the
/// fixed x86-64 offset. Returns `None` on null input, a signature
/// mismatch, or a null `BootServices` pointer instead of dereferencing
/// them (shared fail-loud gate for `GetMemoryMap` and `LocateProtocol`).
unsafe fn load_boot_services(system_table: *const u8) -> Option<*const u8> {
    if system_table.is_null() {
        return None;
    }
    let sig = (system_table as *const u64).read_unaligned();
    if sig != SYSTEM_TABLE_SIGNATURE {
        return None;
    }
    let boot_services = (system_table.add(SYSTEM_TABLE_BOOT_SERVICES_OFF) as *const *const u8)
        .read_unaligned()
        .cast::<u8>();
    if boot_services.is_null() {
        return None;
    }
    Some(boot_services)
}

/// Load the `GetMemoryMap` entry point from the raw system table.
///
/// # Safety
/// `system_table` must point to a live `EFI_SYSTEM_TABLE`; validates the
/// header signature (via [`load_boot_services`]), then reads the
/// `GetMemoryMap` slot at its fixed offset. Returns `None` on null links
/// or a signature mismatch instead of dereferencing them.
unsafe fn load_get_memory_map(system_table: *const u8) -> Option<GetMemoryMapFn> {
    let boot_services = load_boot_services(system_table)?;
    let slot =
        (boot_services.add(BOOT_SERVICES_GET_MEMORY_MAP_OFF) as *const *const ()).read_unaligned();
    if slot.is_null() {
        return None;
    }
    Some(core::mem::transmute::<*const (), GetMemoryMapFn>(slot))
}

/// Raw `AllocatePool` signature (x86-64 UEFI = `efiapi` calling convention):
/// `(PoolType, Size, Buffer) -> EFI_STATUS`.
type AllocatePoolFn = unsafe extern "efiapi" fn(u32, usize, *mut *mut u8) -> usize;

/// Load the `AllocatePool` entry point from the raw system table.
///
/// # Safety
/// Same contract as [`load_get_memory_map`]: live `EFI_SYSTEM_TABLE`,
/// signature-gated, null-slot fails closed. Only loaded once the
/// `EFI_BUFFER_TOO_SMALL` dance needs it, so boot-services fakes sized for
/// `GetMemoryMap` alone (all pre-dance host tests) never reach this slot.
unsafe fn load_allocate_pool(system_table: *const u8) -> Option<AllocatePoolFn> {
    let boot_services = load_boot_services(system_table)?;
    let slot =
        (boot_services.add(BOOT_SERVICES_ALLOCATE_POOL_OFF) as *const *const ()).read_unaligned();
    if slot.is_null() {
        return None;
    }
    Some(core::mem::transmute::<*const (), AllocatePoolFn>(slot))
}

/// Raw `LocateProtocol` signature (x86-64 UEFI = `efiapi` calling convention).
type LocateProtocolFn = unsafe extern "efiapi" fn(
    *const u8,    // Protocol (EFI_GUID*)
    *const u8,    // Registration (VOID*, always null here)
    *mut *mut u8, // Interface (VOID**)
) -> usize;

/// Load the `LocateProtocol` entry point from the raw system table.
///
/// # Safety
/// Same contract as [`load_get_memory_map`]: live `EFI_SYSTEM_TABLE`,
/// signature-gated, null-slot fails closed.
unsafe fn load_locate_protocol(system_table: *const u8) -> Option<LocateProtocolFn> {
    let boot_services = load_boot_services(system_table)?;
    let slot =
        (boot_services.add(BOOT_SERVICES_LOCATE_PROTOCOL_OFF) as *const *const ()).read_unaligned();
    if slot.is_null() {
        return None;
    }
    Some(core::mem::transmute::<*const (), LocateProtocolFn>(slot))
}

/// Call `GetMemoryMap` and fill `info`, escalating through an
/// `AllocatePool` overflow buffer on `EFI_BUFFER_TOO_SMALL`.
///
/// Returns `(ok, status, pushed, total, map_key)`: `ok` is true only when
/// the firmware call succeeded; `status` is the raw `EFI_STATUS`; `pushed` is
/// the number of regions stored (0 on failure); `total` is the firmware
/// descriptor count (`map_size / desc_size`, 0 on failure); `map_key` is the
/// opaque key [`ExitBootServices`] will validate (0 on failure — never pass
/// a zero key to EBS). `pushed < total` means the 128-entry inline map
/// truncated the firmware map — reported on serial as `truncated=1` so e2e
/// fails loudly (review #29 risk 1). Never panics; a failure still leaves
/// `info` valid-but-empty so the serial summary stays diagnosable.
/// Structural failures (slot load, `desc_size < 40`, oversized map) report
/// `status = `[`STRUCTURAL_FAIL`] instead of a misleading `0x0` (agent-b #5
/// nit: `ok=0 status=0x0` reads as "status succeeded, result failed").
///
/// Buffer-too-small dance (review #29 risk 2, closed this slice): the first
/// call targets the static 16 KiB; on [`EFI_BUFFER_TOO_SMALL`] the firmware
/// reports the required size and the loop escalates to an `AllocatePool`
/// overflow buffer of `required + GET_MAP_POOL_SLACK`, bounded by
/// [`GET_MAP_MAX_ATTEMPTS`] (each allocation perturbs the map, so the next
/// call may overflow again — exhaustion returns the last raw status).
/// Agent-b #51 invariants: (1) every `AllocatePool` happens BEFORE the final
/// successful `GetMemoryMap`, and `efi_main` runs GOP `LocateProtocol`
/// before this function, so nothing allocates between the call that produced
/// `map_key` and `ExitBootServices`; (2) the retained overflow buffer is
/// never `FreePool`'d pre-EBS (freeing would invalidate the key) and
/// `fill_from_raw_descriptors` reads exactly the buffer that produced it —
/// one pointer threads through the whole dance; (3) both halves are host
/// tested through scripted boot-services stands (BTS->SUCCESS sequence;
/// refresh-failure fail-closed asserts live on `exit_boot_services`).
///
/// # Safety
/// Must run pre-`ExitBootServices` on the boot thread with `system_table`
/// live. Exclusively owns the static buffer; any pool buffer obtained here
/// is retained (not aliased, not freed) until EBS.
unsafe fn fill_boot_info(
    system_table: *const u8,
    info: &mut BootInfo,
) -> (bool, usize, usize, usize, usize) {
    let Some(get_map) = load_get_memory_map(system_table) else {
        return (false, STRUCTURAL_FAIL, 0, 0, 0);
    };
    // Single pointer threaded through the dance: the final successful call
    // and the fill below always refer to the same buffer (invariant 2).
    let mut buf = core::ptr::addr_of_mut!(MEMMAP_BUF).cast::<u8>();
    let mut cap = MEMMAP_BUF_LEN;
    let mut map_size;
    let mut map_key: usize = 0;
    let mut desc_size: usize = 0;
    let mut desc_version: u32 = 0;
    let mut status;
    let mut attempts_left = GET_MAP_MAX_ATTEMPTS;
    loop {
        attempts_left -= 1;
        map_size = cap;
        status = get_map(
            &mut map_size,
            buf,
            &mut map_key,
            &mut desc_size,
            &mut desc_version,
        );
        if status != EFI_BUFFER_TOO_SMALL || attempts_left == 0 {
            break;
        }
        let Some(allocate_pool) = load_allocate_pool(system_table) else {
            return (false, STRUCTURAL_FAIL, 0, 0, 0);
        };
        let required = map_size.saturating_add(GET_MAP_POOL_SLACK);
        let mut pool_buf = core::ptr::null_mut::<u8>();
        let alloc_status = allocate_pool(EFI_LOADER_DATA, required, &mut pool_buf);
        if alloc_status != 0 {
            return (false, alloc_status, 0, 0, 0);
        }
        if pool_buf.is_null() {
            return (false, STRUCTURAL_FAIL, 0, 0, 0);
        }
        // Pre-final-success allocation (invariant 1); retained through EBS.
        buf = pool_buf;
        cap = required;
    }
    if status != 0 {
        return (false, status, 0, 0, 0);
    }
    if desc_size < 40 || map_size > cap {
        return (false, STRUCTURAL_FAIL, 0, 0, 0);
    }
    let total = map_size / desc_size;
    // Invariant 2: read the exact buffer whose call produced `map_key`.
    let pushed =
        info.fill_from_raw_descriptors(core::slice::from_raw_parts(buf, map_size), desc_size);
    (true, status, pushed, total, map_key)
}

/// Refresh only the map key (ignore descriptor contents) for an EBS retry.
///
/// Returns `(ok, map_key)`: `ok` is true only when `GetMemoryMap` succeeded
/// and returned a non-zero key; the caller must not use `info` from this
/// call (this is key-refresh only after a stale-key `EFI_INVALID_PARAMETER`).
/// Single-shot into the static buffer (no overflow dance): if the map has
/// outgrown 16 KiB the refresh fails closed and `exit_boot_services` reports
/// the original stale-key status — fail-loud over silent key fabrication.
///
/// # Safety
/// Pre-`ExitBootServices` only; same static-buffer exclusivity contract as
/// [`fill_boot_info`].
unsafe fn refresh_map_key(system_table: *const u8) -> (bool, usize) {
    let Some(get_map) = load_get_memory_map(system_table) else {
        return (false, 0);
    };
    let buf = &mut *core::ptr::addr_of_mut!(MEMMAP_BUF);
    let mut map_size = buf.len();
    let mut map_key: usize = 0;
    let mut desc_size: usize = 0;
    let mut desc_version: u32 = 0;
    let status = get_map(
        &mut map_size,
        buf.as_mut_ptr(),
        &mut map_key,
        &mut desc_size,
        &mut desc_version,
    );
    if status != 0 || map_key == 0 {
        return (false, 0);
    }
    (true, map_key)
}

/// Cross `ExitBootServices`: validate the map key and hand the CPU to the
/// kernel world (no more boot-service calls after success).
///
/// Returns `(ok, status)`: `ok` is true only when EBS returned 0; `status`
/// is the raw `EFI_STATUS` of the *last* attempt. On
/// `EFI_INVALID_PARAMETER` (stale key — GOP/summary activity may have
/// allocated) the map key is refreshed once and EBS retried, per the UEFI
/// spec; a second failure fails closed (no infinite retry, no silent
/// success). Never panics.
///
/// # Safety
/// Must run on the boot thread with `system_table` live and `image_handle`
/// from `efi_main`. On success the caller must not invoke any boot-service
/// function again (the firmware tables are invalid); on failure the caller
/// still owns boot services and may park.
unsafe fn exit_boot_services(
    image_handle: *const (),
    system_table: *const u8,
    map_key: usize,
) -> (bool, usize) {
    let Some(ebs) = load_exit_boot_services(system_table) else {
        return (false, STRUCTURAL_FAIL);
    };
    if map_key == 0 {
        return (false, STRUCTURAL_FAIL);
    }
    let status = ebs(image_handle, map_key);
    if status == 0 {
        return (true, 0);
    }
    if status != EFI_INVALID_PARAMETER {
        return (false, status);
    }
    let (refreshed, new_key) = refresh_map_key(system_table);
    if !refreshed {
        return (false, status);
    }
    let status2 = ebs(image_handle, new_key);
    (status2 == 0, status2)
}

/// Emit the one-line `MEMMAP` diagnostic after the two-line boot summary:
/// `MEMMAP ok=<0|1> status=<0x...> entries=<pushed>/<total> truncated=<0|1>`.
/// `status` is the raw `GetMemoryMap` `EFI_STATUS`, or `0xffffffffffffffff`
/// ([`STRUCTURAL_FAIL`]) for structural failures (never `ok=0 status=0x0`).
fn write_memmap_line(ok: bool, status: usize, pushed: usize, total: usize) {
    let mut emit = com1_emit;
    let _ = write_str(&mut emit, "MEMMAP ok=");
    let _ = write_u64_dec(&mut emit, u64::from(ok));
    let _ = write_str(&mut emit, " status=");
    let _ = write_u64_hex(&mut emit, status as u64);
    let _ = write_str(&mut emit, " entries=");
    let _ = write_u64_dec(&mut emit, pushed as u64);
    let _ = write_str(&mut emit, "/");
    let _ = write_u64_dec(&mut emit, total as u64);
    let _ = write_str(&mut emit, " truncated=");
    let _ = write_u64_dec(&mut emit, u64::from(pushed < total));
    let _ = write_str(&mut emit, "\n");
}

/// Locate the GOP instance and fill `info.framebuffer` from its active mode.
///
/// Returns `(ok, status)`: `ok` is true only when `LocateProtocol` succeeded
/// and [`BootInfo::fill_from_gop_mode`] accepted the mode info; `status` is
/// the raw `EFI_STATUS` of `LocateProtocol` on firmware failures, or
/// [`STRUCTURAL_FAIL`] when a later structural step fails (null protocol /
/// `Mode` / `Info`, undersized info, rejected mode info) so serial never
/// prints `GOP ok=0 status=0x0` (agent-b #5 nit). Never panics; a failure
/// leaves the framebuffer unset so `framebuffer_opt()` reports
/// `FB present=0` and the e2e script fails loudly (review #33/#35 fail-loud
/// gate requirement).
///
/// # Safety
/// Must run pre-`ExitBootServices` on the boot thread with `system_table`
/// live; dereferences firmware-owned GOP `Mode`/`Info` pointers for the
/// duration of the call only.
unsafe fn fill_framebuffer(system_table: *const u8, info: &mut BootInfo) -> (bool, usize) {
    let Some(locate) = load_locate_protocol(system_table) else {
        return (false, STRUCTURAL_FAIL);
    };
    let mut gop: *mut u8 = core::ptr::null_mut();
    let status = locate(GOP_GUID.as_ptr(), core::ptr::null(), &mut gop);
    if status != 0 {
        return (false, status);
    }
    if gop.is_null() {
        return (false, STRUCTURAL_FAIL);
    }
    let mode = (gop.add(GOP_MODE_OFF) as *const *const u8).read_unaligned();
    if mode.is_null() {
        return (false, STRUCTURAL_FAIL);
    }
    let info_ptr = (mode.add(GOP_MODE_INFO_OFF) as *const *const u8).read_unaligned();
    let size_of_info = (mode.add(GOP_MODE_SIZE_OF_INFO_OFF) as *const usize).read_unaligned();
    let fb_base = (mode.add(GOP_MODE_FB_BASE_OFF) as *const u64).read_unaligned();
    let fb_size = (mode.add(GOP_MODE_FB_SIZE_OFF) as *const usize).read_unaligned();
    if info_ptr.is_null() || size_of_info < GOP_INFO_MIN {
        return (false, STRUCTURAL_FAIL);
    }
    let mode_info = core::slice::from_raw_parts(info_ptr, GOP_INFO_MIN);
    if !info.fill_from_gop_mode(fb_base, fb_size as u64, mode_info) {
        return (false, STRUCTURAL_FAIL);
    }
    (true, status)
}

/// Emit the one-line `GOP` diagnostic after the `MEMMAP` line:
/// `GOP ok=<0|1> status=<0x...>` — `ok` covers locate + mode-info parse;
/// `status` is the raw `LocateProtocol` `EFI_STATUS`, or
/// `0xffffffffffffffff` ([`STRUCTURAL_FAIL`]) for structural failures.
fn write_gop_line(ok: bool, status: usize) {
    let mut emit = com1_emit;
    let _ = write_str(&mut emit, "GOP ok=");
    let _ = write_u64_dec(&mut emit, u64::from(ok));
    let _ = write_str(&mut emit, " status=");
    let _ = write_u64_hex(&mut emit, status as u64);
    let _ = write_str(&mut emit, "\n");
}

/// Emit the one-line `EBS` diagnostic after the GOP line:
/// `EBS ok=<0|1> status=<0x...>` — `ok` is 1 only when `ExitBootServices`
/// returned success (hard-require in `boot-uefi.ps1`); `status` is the raw
/// `EFI_STATUS` of the last attempt (includes any stale-key retry).
fn write_ebs_line(ok: bool, status: usize) {
    let mut emit = com1_emit;
    let _ = write_str(&mut emit, "EBS ok=");
    let _ = write_u64_dec(&mut emit, u64::from(ok));
    let _ = write_str(&mut emit, " status=");
    let _ = write_u64_hex(&mut emit, status as u64);
    let _ = write_str(&mut emit, "\n");
}

/// UEFI entry point: marker, memory-map + framebuffer fill, summary,
/// `ExitBootServices`, kernel handoff + CPU bring-up, park.
///
/// The firmware passes `(image_handle, system_table)` per the UEFI spec;
/// the handle feeds `ExitBootServices`, the table feeds `GetMemoryMap`,
/// GOP `LocateProtocol`, and the EBS slot load.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "efiapi" fn efi_main(image_handle: *const (), system_table: *const u8) -> usize {
    // SAFETY: single-threaded boot environment, COM1 is ours exclusively;
    // init-then-write order upheld by construction here. Sinks are
    // infallible, so `unwrap` cannot panic.
    unsafe {
        uart_init();
        write_marker::<Infallible>(|b| {
            com1_write_byte(b);
            Ok(())
        })
        .unwrap();
        let mut info = BootInfo::new();
        // Agent-b #51 key-stability invariant: GOP LocateProtocol may
        // allocate internally, so it runs BEFORE fill_boot_info — once the
        // final GetMemoryMap succeeds, nothing allocates until EBS (the
        // lines below are pure port I/O; the dance's AllocatePool calls
        // precede its final successful get_map). Print order below stays
        // summary/MEMMAP/GOP so the e2e asserts are untouched.
        let (gop_ok, gop_status) = fill_framebuffer(system_table, &mut info);
        let (ok, status, pushed, total, map_key) = fill_boot_info(system_table, &mut info);
        let mut emit = com1_emit;
        write_boot_summary::<Infallible>(&info, &mut emit).unwrap();
        write_memmap_line(ok, status, pushed, total);
        write_gop_line(gop_ok, gop_status);
        // S02 slice 2: leave the firmware world. After a successful EBS no
        // boot-service call is legal — kernel_entry + GDT/IDT/`#DE` run on
        // bare CPU + our COM1 only (agent-b #38 caveat: KERNEL ok=1 must be
        // post-EBS to honestly claim the S01 end goal).
        let (ebs_ok, ebs_status) = exit_boot_services(image_handle, system_table, map_key);
        write_ebs_line(ebs_ok, ebs_status);
        if ebs_ok {
            kernel_entry::<Infallible>(&info, &mut emit).unwrap();
            baseos_kernel::bring_up_cpu_and_prove_exception();
        }
        // EBS failed: still own boot services — park fail-loud (e2e sees
        // EBS ok=0 / missing KERNEL+EXC lines and exits non-zero).
        loop {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

/// Park the CPU on panic: a panicking boot guest must not return into
/// firmware with a bogus status, so halt forever (the boot script kills
/// QEMU after its smoke window).
///
/// Compiled out for host `cargo test` builds, where the test harness links
/// `std` (which owns the panic handler); the guest handler only exists in
/// the real `no_std` UEFI binary.
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        // SAFETY: `hlt` is a pure CPU wait; no memory or device is touched.
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "efiapi" fn dummy_get_map(
        _a: *mut usize,
        _b: *mut u8,
        _c: *mut usize,
        _d: *mut usize,
        _e: *mut u32,
    ) -> usize {
        0
    }

    unsafe extern "efiapi" fn dummy_locate(
        _a: *const u8,
        _b: *const u8,
        _c: *mut *mut u8,
    ) -> usize {
        0
    }

    unsafe extern "efiapi" fn dummy_ebs(_a: *const (), _b: usize) -> usize {
        0
    }

    fn fake_table(sig: u64, boot_services: *const u8) -> Vec<u8> {
        let mut st = vec![0u8; 112];
        st[0..8].copy_from_slice(&sig.to_le_bytes());
        st[SYSTEM_TABLE_BOOT_SERVICES_OFF..SYSTEM_TABLE_BOOT_SERVICES_OFF + 8]
            .copy_from_slice(&(boot_services as usize).to_le_bytes());
        st
    }

    /// Boot-services stand with one populated slot (rest zero); sized to
    /// cover `LocateProtocol` at +320.
    fn fake_bs_with_slot(off: usize, f: *const ()) -> Vec<u8> {
        let mut bs = vec![0u8; off + 8];
        bs[off..off + 8].copy_from_slice(&(f as usize).to_le_bytes());
        bs
    }

    #[test]
    fn system_table_signature_gates_get_memory_map() {
        // Review #29 risk 3: stale/non-UEFI pointer must fail closed.
        let bs = fake_bs_with_slot(BOOT_SERVICES_GET_MEMORY_MAP_OFF, dummy_get_map as *const ());
        // Null table -> None.
        assert!(unsafe { load_get_memory_map(core::ptr::null()) }.is_none());
        // Bad signature with otherwise-valid links -> None.
        let bad = fake_table(0xDEAD_BEEF_DEAD_BEEF, bs.as_ptr());
        assert!(unsafe { load_get_memory_map(bad.as_ptr()) }.is_none());
        // Good signature but null BootServices -> None.
        let null_bs = fake_table(SYSTEM_TABLE_SIGNATURE, core::ptr::null());
        assert!(unsafe { load_get_memory_map(null_bs.as_ptr()) }.is_none());
        // Good signature + valid links -> Some.
        let good = fake_table(SYSTEM_TABLE_SIGNATURE, bs.as_ptr());
        assert!(unsafe { load_get_memory_map(good.as_ptr()) }.is_some());
        // Good signature + null GetMemoryMap slot -> None.
        let bs_empty = [0u8; BOOT_SERVICES_GET_MEMORY_MAP_OFF + 8];
        let no_slot = fake_table(SYSTEM_TABLE_SIGNATURE, bs_empty.as_ptr());
        assert!(unsafe { load_get_memory_map(no_slot.as_ptr()) }.is_none());
    }

    #[test]
    fn system_table_signature_gates_locate_protocol() {
        // GOP slice: LocateProtocol reuses the review #29 risk-3 gate.
        let bs = fake_bs_with_slot(BOOT_SERVICES_LOCATE_PROTOCOL_OFF, dummy_locate as *const ());
        assert!(unsafe { load_locate_protocol(core::ptr::null()) }.is_none());
        let bad = fake_table(0xDEAD_BEEF_DEAD_BEEF, bs.as_ptr());
        assert!(unsafe { load_locate_protocol(bad.as_ptr()) }.is_none());
        let null_bs = fake_table(SYSTEM_TABLE_SIGNATURE, core::ptr::null());
        assert!(unsafe { load_locate_protocol(null_bs.as_ptr()) }.is_none());
        let good = fake_table(SYSTEM_TABLE_SIGNATURE, bs.as_ptr());
        assert!(unsafe { load_locate_protocol(good.as_ptr()) }.is_some());
        let bs_empty = [0u8; BOOT_SERVICES_LOCATE_PROTOCOL_OFF + 8];
        let no_slot = fake_table(SYSTEM_TABLE_SIGNATURE, bs_empty.as_ptr());
        assert!(unsafe { load_locate_protocol(no_slot.as_ptr()) }.is_none());
    }

    #[test]
    fn system_table_signature_gates_exit_boot_services() {
        // S02 slice 2: EBS reuses the review #29 risk-3 fail-closed gate.
        let bs = fake_bs_with_slot(BOOT_SERVICES_EXIT_BOOT_SERVICES_OFF, dummy_ebs as *const ());
        assert!(unsafe { load_exit_boot_services(core::ptr::null()) }.is_none());
        let bad = fake_table(0xDEAD_BEEF_DEAD_BEEF, bs.as_ptr());
        assert!(unsafe { load_exit_boot_services(bad.as_ptr()) }.is_none());
        let null_bs = fake_table(SYSTEM_TABLE_SIGNATURE, core::ptr::null());
        assert!(unsafe { load_exit_boot_services(null_bs.as_ptr()) }.is_none());
        let good = fake_table(SYSTEM_TABLE_SIGNATURE, bs.as_ptr());
        assert!(unsafe { load_exit_boot_services(good.as_ptr()) }.is_some());
        let bs_empty = [0u8; BOOT_SERVICES_EXIT_BOOT_SERVICES_OFF + 8];
        let no_slot = fake_table(SYSTEM_TABLE_SIGNATURE, bs_empty.as_ptr());
        assert!(unsafe { load_exit_boot_services(no_slot.as_ptr()) }.is_none());
        // Offset sanity: +232 must not collide with GetMemoryMap (+56) or
        // LocateProtocol (+320) — a wrong constant would load the wrong slot.
        assert_ne!(
            BOOT_SERVICES_EXIT_BOOT_SERVICES_OFF,
            BOOT_SERVICES_GET_MEMORY_MAP_OFF
        );
        assert_ne!(
            BOOT_SERVICES_EXIT_BOOT_SERVICES_OFF,
            BOOT_SERVICES_LOCATE_PROTOCOL_OFF
        );
        assert_eq!(BOOT_SERVICES_EXIT_BOOT_SERVICES_OFF, 232);
    }

    /// Serializes host tests that touch the static `MEMMAP_BUF` fill path
    /// (the Rust test harness runs tests in parallel threads).
    static MEMMAP_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// `GetMemoryMap` stand: firmware `EFI_STATUS` 0 but an implausible
    /// descriptor stride, i.e. the structural-failure branch of
    /// [`fill_boot_info`] with no raw error to report.
    unsafe extern "efiapi" fn get_map_structural(
        _map_size: *mut usize,
        _buf: *mut u8,
        _key: *mut usize,
        desc_size: *mut usize,
        _desc_version: *mut u32,
    ) -> usize {
        *desc_size = 8;
        0
    }

    /// `GetMemoryMap` stand: raw firmware error (`EFI_NOT_FOUND` encoding)
    /// that must pass through verbatim, not be masked by a sentinel.
    unsafe extern "efiapi" fn get_map_raw_error(
        _map_size: *mut usize,
        _buf: *mut u8,
        _key: *mut usize,
        _desc_size: *mut usize,
        _desc_version: *mut u32,
    ) -> usize {
        0x8000_0000_0000_000E
    }

    /// agent-b #5 nit: a failed `MEMMAP` line must never read
    /// `ok=0 status=0x0` — structural failures report [`STRUCTURAL_FAIL`],
    /// raw firmware errors keep their exact `EFI_STATUS`.
    #[test]
    fn fill_boot_info_failures_are_diagnosable() {
        let _guard = MEMMAP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Structural: locate slot present, status 0, bogus desc_size.
        let bs = fake_bs_with_slot(
            BOOT_SERVICES_GET_MEMORY_MAP_OFF,
            get_map_structural as *const (),
        );
        let st = fake_table(SYSTEM_TABLE_SIGNATURE, bs.as_ptr());
        let mut info = BootInfo::new();
        let (ok, status, pushed, total, key) = unsafe { fill_boot_info(st.as_ptr(), &mut info) };
        assert!(!ok);
        assert_eq!(status, STRUCTURAL_FAIL);
        assert_eq!((pushed, total, key), (0, 0, 0));
        // Raw firmware error passes through unchanged.
        let bs = fake_bs_with_slot(
            BOOT_SERVICES_GET_MEMORY_MAP_OFF,
            get_map_raw_error as *const (),
        );
        let st = fake_table(SYSTEM_TABLE_SIGNATURE, bs.as_ptr());
        let (ok, status, pushed, total, key) = unsafe { fill_boot_info(st.as_ptr(), &mut info) };
        assert!(!ok);
        assert_eq!(status, 0x8000_0000_0000_000E);
        assert_eq!((pushed, total, key), (0, 0, 0));
        // Missing/stale system table (slot load fails) is structural too.
        let (ok, status, pushed, total, key) =
            unsafe { fill_boot_info(core::ptr::null(), &mut info) };
        assert!(!ok);
        assert_eq!(status, STRUCTURAL_FAIL);
        assert_eq!((pushed, total, key), (0, 0, 0));
    }

    /// agent-b #5 nit, GOP half: structural GOP failures (null protocol,
    /// null `Mode`/`Info`, rejected mode info) report [`STRUCTURAL_FAIL`],
    /// raw `LocateProtocol` errors pass through, and the accepted path still
    /// returns `(true, 0)`.
    #[test]
    fn fill_framebuffer_failures_never_report_zero_status() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        static GOP_PTR: AtomicUsize = AtomicUsize::new(0);
        static LOCATE_STATUS: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "efiapi" fn locate_probe(
            _protocol: *const u8,
            _registration: *const u8,
            out: *mut *mut u8,
        ) -> usize {
            let status = LOCATE_STATUS.load(Ordering::Relaxed);
            if status == 0 {
                *out = GOP_PTR.load(Ordering::Relaxed) as *mut u8;
            }
            status
        }

        let bs = fake_bs_with_slot(BOOT_SERVICES_LOCATE_PROTOCOL_OFF, locate_probe as *const ());
        let st = fake_table(SYSTEM_TABLE_SIGNATURE, bs.as_ptr());
        let mut info = BootInfo::new();
        let run = |info: &mut BootInfo| unsafe { fill_framebuffer(st.as_ptr(), info) };

        // Locate ok, protocol pointer left null -> structural.
        GOP_PTR.store(0, Ordering::Relaxed);
        LOCATE_STATUS.store(0, Ordering::Relaxed);
        let (ok, status) = run(&mut info);
        assert!(!ok);
        assert_eq!(status, STRUCTURAL_FAIL);

        // Raw firmware error preserved verbatim (no sentinel overwrite).
        LOCATE_STATUS.store(0x8000_0000_0000_000E, Ordering::Relaxed);
        let (ok, status) = run(&mut info);
        assert!(!ok);
        assert_eq!(status, 0x8000_0000_0000_000E);
        LOCATE_STATUS.store(0, Ordering::Relaxed);

        // Locate ok, GOP present but Mode pointer null -> structural.
        let gop_null_mode = [0u8; 40];
        GOP_PTR.store(gop_null_mode.as_ptr() as usize, Ordering::Relaxed);
        let (ok, status) = run(&mut info);
        assert!(!ok);
        assert_eq!(status, STRUCTURAL_FAIL);

        // Mode present, Info pointer null -> structural.
        let mode = [0u8; 40];
        let mut gop = [0u8; 32];
        gop[GOP_MODE_OFF..GOP_MODE_OFF + 8]
            .copy_from_slice(&(mode.as_ptr() as usize).to_le_bytes());
        GOP_PTR.store(gop.as_ptr() as usize, Ordering::Relaxed);
        let (ok, status) = run(&mut info);
        assert!(!ok);
        assert_eq!(status, STRUCTURAL_FAIL);

        // Full parse, PixelBltOnly (format 3) fails closed in
        // fill_from_gop_mode -> structural, framebuffer stays unset.
        let mut mode_info = [0u8; 36];
        mode_info[4..8].copy_from_slice(&1024u32.to_le_bytes());
        mode_info[8..12].copy_from_slice(&768u32.to_le_bytes());
        mode_info[12..16].copy_from_slice(&3u32.to_le_bytes());
        mode_info[32..36].copy_from_slice(&1024u32.to_le_bytes());
        let mut mode = [0u8; 40];
        mode[GOP_MODE_INFO_OFF..GOP_MODE_INFO_OFF + 8]
            .copy_from_slice(&(mode_info.as_ptr() as usize).to_le_bytes());
        mode[GOP_MODE_SIZE_OF_INFO_OFF..GOP_MODE_SIZE_OF_INFO_OFF + 8]
            .copy_from_slice(&36usize.to_le_bytes());
        mode[GOP_MODE_FB_BASE_OFF..GOP_MODE_FB_BASE_OFF + 8]
            .copy_from_slice(&0x8000_0000u64.to_le_bytes());
        mode[GOP_MODE_FB_SIZE_OFF..GOP_MODE_FB_SIZE_OFF + 8]
            .copy_from_slice(&(8 * 1024 * 1024usize).to_le_bytes());
        let mut gop = [0u8; 32];
        gop[GOP_MODE_OFF..GOP_MODE_OFF + 8]
            .copy_from_slice(&(mode.as_ptr() as usize).to_le_bytes());
        GOP_PTR.store(gop.as_ptr() as usize, Ordering::Relaxed);
        let (ok, status) = run(&mut info);
        assert!(!ok);
        assert_eq!(status, STRUCTURAL_FAIL);
        assert_eq!(info.has_framebuffer, 0);

        // Accepted path unchanged: format 0 -> (true, raw status 0).
        mode_info[12..16].copy_from_slice(&0u32.to_le_bytes());
        let (ok, status) = run(&mut info);
        assert!(ok);
        assert_eq!(status, 0);
        assert_eq!(info.has_framebuffer, 1);

        GOP_PTR.store(0, Ordering::Relaxed);
    }

    /// S03 #5: stale-map-key retry of [`exit_boot_services`] — fake BS with
    /// BOTH GetMemoryMap (+56) and EBS (+232) slots: zero input key fails
    /// closed without calling EBS; `EFI_INVALID_PARAMETER` once refreshes
    /// the key and succeeds on the second call; a second stale failure
    /// fails closed (no infinite retry). Takes `MEMMAP_TEST_LOCK`
    /// (`refresh_map_key` writes the shared static `MEMMAP_BUF`).
    #[test]
    fn exit_boot_services_retries_stale_map_key_once() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        static EBS_CALLS: AtomicUsize = AtomicUsize::new(0);
        static EBS_ALWAYS_STALE: AtomicUsize = AtomicUsize::new(0);
        static REFRESH_CALLS: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "efiapi" fn get_map_refresh_key(
            _map_size: *mut usize,
            _buf: *mut u8,
            key: *mut usize,
            desc_size: *mut usize,
            _desc_version: *mut u32,
        ) -> usize {
            REFRESH_CALLS.fetch_add(1, Ordering::Relaxed);
            *key = 0xABCD;
            *desc_size = 48;
            0
        }

        unsafe extern "efiapi" fn ebs_scripted(_ih: *const (), key: usize) -> usize {
            assert_ne!(key, 0);
            let call = EBS_CALLS.fetch_add(1, Ordering::Relaxed);
            if EBS_ALWAYS_STALE.load(Ordering::Relaxed) != 0 {
                return EFI_INVALID_PARAMETER;
            }
            if call == 0 {
                EFI_INVALID_PARAMETER
            } else {
                0
            }
        }

        let _guard = MEMMAP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut bs = fake_bs_with_slot(
            BOOT_SERVICES_EXIT_BOOT_SERVICES_OFF,
            ebs_scripted as *const (),
        );
        bs[BOOT_SERVICES_GET_MEMORY_MAP_OFF..BOOT_SERVICES_GET_MEMORY_MAP_OFF + 8]
            .copy_from_slice(&(get_map_refresh_key as *const () as usize).to_le_bytes());
        let st = fake_table(SYSTEM_TABLE_SIGNATURE, bs.as_ptr());
        EBS_CALLS.store(0, Ordering::Relaxed);
        EBS_ALWAYS_STALE.store(0, Ordering::Relaxed);
        REFRESH_CALLS.store(0, Ordering::Relaxed);

        // Zero input key: structural fail, EBS never invoked.
        let (ok, status) = unsafe { exit_boot_services(core::ptr::null(), st.as_ptr(), 0) };
        assert!(!ok);
        assert_eq!(status, STRUCTURAL_FAIL);
        assert_eq!(EBS_CALLS.load(Ordering::Relaxed), 0);

        // Stale first attempt -> refresh key once -> succeed on retry.
        let (ok, status) = unsafe { exit_boot_services(core::ptr::null(), st.as_ptr(), 0x1234) };
        assert!(ok);
        assert_eq!(status, 0);
        assert_eq!(EBS_CALLS.load(Ordering::Relaxed), 2);
        assert_eq!(REFRESH_CALLS.load(Ordering::Relaxed), 1);

        // Second stale failure: fail closed with the raw status, one refresh.
        EBS_CALLS.store(0, Ordering::Relaxed);
        EBS_ALWAYS_STALE.store(1, Ordering::Relaxed);
        let (ok, status) = unsafe { exit_boot_services(core::ptr::null(), st.as_ptr(), 0x1234) };
        assert!(!ok);
        assert_eq!(status, EFI_INVALID_PARAMETER);
        assert_eq!(EBS_CALLS.load(Ordering::Relaxed), 2);
        assert_eq!(REFRESH_CALLS.load(Ordering::Relaxed), 2);

        EBS_ALWAYS_STALE.store(0, Ordering::Relaxed);
    }

    /// Agent-b #51 invariant 3 (dance half): inject the scripted status
    /// sequence `[BUFFER_TOO_SMALL(required=N), SUCCESS]` through a fake BS
    /// with GetMemoryMap (+56) AND AllocatePool (+64) slots. Asserts the
    /// escalation asks AllocatePool for `required + GET_MAP_POOL_SLACK`,
    /// the retry runs against the pool buffer (not the stale static), and
    /// `fill_from_raw_descriptors` reads the buffer that produced the key
    /// (invariant 2). Takes `MEMMAP_TEST_LOCK` (fill path owns MEMMAP_BUF).
    #[test]
    fn fill_boot_info_dances_through_pool_on_buffer_too_small() {
        use baseos_boot_info::MemoryKind;
        use core::sync::atomic::{AtomicUsize, Ordering};
        static GET_MAP_CALLS: AtomicUsize = AtomicUsize::new(0);
        static ALLOC_SIZE: AtomicUsize = AtomicUsize::new(0);
        static POOL_PTR: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "efiapi" fn allocate_pool_scripted(
            pool_type: u32,
            size: usize,
            out: *mut *mut u8,
        ) -> usize {
            assert_eq!(pool_type, EFI_LOADER_DATA);
            ALLOC_SIZE.store(size, Ordering::Relaxed);
            *out = POOL_PTR.load(Ordering::Relaxed) as *mut u8;
            0
        }

        unsafe extern "efiapi" fn get_map_buffer_too_small(
            map_size: *mut usize,
            buf: *mut u8,
            key: *mut usize,
            desc_size: *mut usize,
            desc_version: *mut u32,
        ) -> usize {
            let call = GET_MAP_CALLS.fetch_add(1, Ordering::Relaxed);
            if call == 0 {
                // Attempt 1 targets the static buffer with its full capacity
                // and overflows with a required size the static cannot hold.
                assert_eq!(buf, core::ptr::addr_of_mut!(MEMMAP_BUF).cast::<u8>());
                assert_eq!(*map_size, MEMMAP_BUF_LEN);
                *map_size = MEMMAP_BUF_LEN + 4096;
                return EFI_BUFFER_TOO_SMALL;
            }
            // Attempt 2 input capacity is the pool allocation (required+slack)
            // and the buffer pointer is the overflow block, not the static.
            assert_eq!(*map_size, MEMMAP_BUF_LEN + 4096 + GET_MAP_POOL_SLACK);
            assert_eq!(buf as usize, POOL_PTR.load(Ordering::Relaxed));
            let stride = 48usize;
            *map_size = stride;
            *key = 0x7777;
            *desc_size = stride;
            *desc_version = 1;
            // One EfiConventionalMemory descriptor at 1 MiB, 4 pages.
            let d = core::slice::from_raw_parts_mut(buf, stride);
            d[0..4].copy_from_slice(&7u32.to_le_bytes());
            d[8..16].copy_from_slice(&0x0010_0000u64.to_le_bytes());
            d[24..32].copy_from_slice(&4u64.to_le_bytes());
            0
        }

        let _guard = MEMMAP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut pool_scratch = [0u8; 32768];
        POOL_PTR.store(pool_scratch.as_mut_ptr() as usize, Ordering::Relaxed);
        let mut bs = fake_bs_with_slot(
            BOOT_SERVICES_ALLOCATE_POOL_OFF,
            allocate_pool_scripted as *const (),
        );
        bs[BOOT_SERVICES_GET_MEMORY_MAP_OFF..BOOT_SERVICES_GET_MEMORY_MAP_OFF + 8]
            .copy_from_slice(&(get_map_buffer_too_small as *const () as usize).to_le_bytes());
        let st = fake_table(SYSTEM_TABLE_SIGNATURE, bs.as_ptr());
        GET_MAP_CALLS.store(0, Ordering::Relaxed);
        ALLOC_SIZE.store(0, Ordering::Relaxed);

        let mut info = BootInfo::new();
        let (ok, status, pushed, total, key) = unsafe { fill_boot_info(st.as_ptr(), &mut info) };
        assert!(ok, "dance must succeed through the pool buffer");
        assert_eq!(status, 0);
        assert_eq!(GET_MAP_CALLS.load(Ordering::Relaxed), 2);
        assert_eq!(
            ALLOC_SIZE.load(Ordering::Relaxed),
            MEMMAP_BUF_LEN + 4096 + GET_MAP_POOL_SLACK
        );
        // Invariant 2: fill parsed the pool buffer that produced the key.
        assert_eq!((pushed, total, key), (1, 1, 0x7777));
        assert_eq!(info.region_count, 1);
        assert_eq!(info.regions[0].start, 0x0010_0000);
        assert_eq!(info.regions[0].page_count, 4);
        assert_eq!(info.regions[0].kind, MemoryKind::Usable);

        POOL_PTR.store(0, Ordering::Relaxed);
    }

    /// Agent-b #51 invariant 3 (refresh half): a stale-key EBS whose
    /// `refresh_map_key` fails — raw `GetMemoryMap` error OR success with a
    /// zero key — must fail closed with the original `EFI_INVALID_PARAMETER`,
    /// with exactly one EBS attempt (no second call on a bad key) and one
    /// refresh attempt, each mode asserted separately.
    #[test]
    fn exit_boot_services_refresh_failure_fails_closed() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        static EBS_CALLS: AtomicUsize = AtomicUsize::new(0);
        static REFRESH_CALLS: AtomicUsize = AtomicUsize::new(0);
        static REFRESH_MODE: AtomicUsize = AtomicUsize::new(0); // 0 raw error, 1 zero key

        unsafe extern "efiapi" fn get_map_refresh_fail(
            _map_size: *mut usize,
            _buf: *mut u8,
            key: *mut usize,
            desc_size: *mut usize,
            _desc_version: *mut u32,
        ) -> usize {
            REFRESH_CALLS.fetch_add(1, Ordering::Relaxed);
            *desc_size = 48;
            if REFRESH_MODE.load(Ordering::Relaxed) == 0 {
                return 0x8000_0000_0000_000E; // raw firmware error
            }
            *key = 0; // status 0 but unusable key
            0
        }

        unsafe extern "efiapi" fn ebs_always_stale(_ih: *const (), _key: usize) -> usize {
            EBS_CALLS.fetch_add(1, Ordering::Relaxed);
            EFI_INVALID_PARAMETER
        }

        let _guard = MEMMAP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut bs = fake_bs_with_slot(
            BOOT_SERVICES_EXIT_BOOT_SERVICES_OFF,
            ebs_always_stale as *const (),
        );
        bs[BOOT_SERVICES_GET_MEMORY_MAP_OFF..BOOT_SERVICES_GET_MEMORY_MAP_OFF + 8]
            .copy_from_slice(&(get_map_refresh_fail as *const () as usize).to_le_bytes());
        let st = fake_table(SYSTEM_TABLE_SIGNATURE, bs.as_ptr());
        EBS_CALLS.store(0, Ordering::Relaxed);
        REFRESH_CALLS.store(0, Ordering::Relaxed);
        REFRESH_MODE.store(0, Ordering::Relaxed);

        // Mode 0: refresh GetMemoryMap returns a raw error.
        let (ok, status) = unsafe { exit_boot_services(core::ptr::null(), st.as_ptr(), 0x1234) };
        assert!(!ok);
        assert_eq!(status, EFI_INVALID_PARAMETER);
        assert_eq!(EBS_CALLS.load(Ordering::Relaxed), 1);
        assert_eq!(REFRESH_CALLS.load(Ordering::Relaxed), 1);

        // Mode 1: refresh succeeds but yields a zero key.
        EBS_CALLS.store(0, Ordering::Relaxed);
        REFRESH_CALLS.store(0, Ordering::Relaxed);
        REFRESH_MODE.store(1, Ordering::Relaxed);
        let (ok, status) = unsafe { exit_boot_services(core::ptr::null(), st.as_ptr(), 0x1234) };
        assert!(!ok);
        assert_eq!(status, EFI_INVALID_PARAMETER);
        assert_eq!(EBS_CALLS.load(Ordering::Relaxed), 1);
        assert_eq!(REFRESH_CALLS.load(Ordering::Relaxed), 1);

        REFRESH_MODE.store(0, Ordering::Relaxed);
    }
}
