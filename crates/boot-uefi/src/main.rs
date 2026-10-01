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
//! `EBS ok=1`, `KERNEL ok=1`, `FRAMEALLOC ok=1`, `PAGING ok=1`, `PERM ok=1`,
//! `PERMAUDIT ok=1`, `PERMAUDIT2 ok=1`, `RMBZ ok=1`,
//! summary lines in the serial capture. The `MEMMAP` line also carries
//! `map_size`/`desc_size` for the 16 KiB + 4 KiB budget baseline (#59a).

#![cfg_attr(not(test), no_main)]
#![cfg_attr(not(test), no_std)]
// SAFETY (unsafe boundary, per `docs/coding-conventions.md`): `hlt` and raw
// UEFI table/service-pointer dereference have no safe equivalent in a
// `no_std` guest, so they live in this smallest possible hardware boundary
// behind the safe wrappers below (`fill_boot_info`, `com1_emit`). Port I/O
// (`in`/`out`) lives only in `kernel::uart` (S05 slice 8 unify): the guest
// calls `kernel::uart::{uart_init, com1_write_byte}` (safe wrappers over
// private `outb`/`inb` bodies). Tested end-to-end by
// `tools/qemu/boot-uefi.ps1` (OVMF boot asserts the marker and the BOOTINFO
// summary in the QEMU serial capture).
#![allow(unsafe_code)]

use baseos_boot_info::{write_boot_summary, write_str, write_u64_dec, write_u64_hex, BootInfo};
use baseos_kernel::cpu::{cr3_pml4_base, efer_nxe_enabled, read_cr3, read_efer};
use baseos_kernel::kernel_entry;
use baseos_kernel::page_table::{
    guard_selftest, perm_selftest, rmbz_audit_ok, rmbz_count_violations, table_base_in_boot_map,
    walk_full, walk_full_leaf_flags, walk_full_leaf_raw,
};
use baseos_kernel::thread::{run_on_thread_stack, swap_target, Thread};
use baseos_kernel::uart::{com1_write_byte, uart_init};
use baseos_serial_marker::write_marker;
#[cfg(not(test))]
use core::panic::PanicInfo;
use core::{arch::asm, convert::Infallible};

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

/// COM1 byte sink matching the `write_marker` / `write_boot_summary`
/// callback style (infallible: UART polling cannot fail).
///
/// S05 slice 8 UART unify: safe delegation to `kernel::uart::com1_write_byte`
/// (raw `in`/`out` lives only in `kernel::uart::outb`/`inb` now; the guest
/// holds no port-I/O `unsafe`). COM1 was initialized via `kernel::uart::uart_init`
/// once in `efi_main` before any emission; single-threaded boot owns the UART.
fn com1_emit(byte: u8) -> Result<(), Infallible> {
    com1_write_byte(byte);
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
/// Returns `(ok, status, pushed, total, map_key, map_size, desc_size)`:
/// `ok` is true only when the firmware call succeeded; `status` is the raw
/// `EFI_STATUS`; `pushed` is the number of regions stored (0 on failure);
/// `total` is the firmware descriptor count (`map_size / desc_size`, 0 on
/// failure); `map_key` is the opaque key [`ExitBootServices`] will validate
/// (0 on failure — never pass a zero key to EBS); `map_size`/`desc_size` are
/// the raw byte size and descriptor stride from the last `GetMemoryMap`
/// call (0/0 when no call ran) so serial can baseline them against the
/// 16 KiB + 4 KiB budget (#59a/#7). `pushed < total` means the 128-entry
/// inline map truncated the firmware map — reported on serial as
/// `truncated=1` so e2e fails loudly (review #29 risk 1). Never panics; a
/// failure still leaves `info` valid-but-empty so the serial summary stays
/// diagnosable. Structural failures (slot load, `desc_size < 40`, oversized
/// map) report `status = `[`STRUCTURAL_FAIL`] instead of a misleading `0x0`
/// (agent-b #5 nit: `ok=0 status=0x0` reads as "status succeeded, result
/// failed").
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
) -> (bool, usize, usize, usize, usize, usize, usize) {
    let mut map_size = 0usize;
    let mut map_key: usize = 0;
    let mut desc_size: usize = 0;
    let mut desc_version: u32 = 0;
    let Some(get_map) = load_get_memory_map(system_table) else {
        return (false, STRUCTURAL_FAIL, 0, 0, 0, map_size, desc_size);
    };
    // Single pointer threaded through the dance: the final successful call
    // and the fill below always refer to the same buffer (invariant 2).
    let mut buf = core::ptr::addr_of_mut!(MEMMAP_BUF).cast::<u8>();
    let mut cap = MEMMAP_BUF_LEN;
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
            return (false, STRUCTURAL_FAIL, 0, 0, 0, map_size, desc_size);
        };
        let required = map_size.saturating_add(GET_MAP_POOL_SLACK);
        let mut pool_buf = core::ptr::null_mut::<u8>();
        let alloc_status = allocate_pool(EFI_LOADER_DATA, required, &mut pool_buf);
        if alloc_status != 0 {
            return (false, alloc_status, 0, 0, 0, map_size, desc_size);
        }
        if pool_buf.is_null() {
            return (false, STRUCTURAL_FAIL, 0, 0, 0, map_size, desc_size);
        }
        // Pre-final-success allocation (invariant 1); retained through EBS.
        buf = pool_buf;
        cap = required;
    }
    if status != 0 {
        return (false, status, 0, 0, 0, map_size, desc_size);
    }
    if desc_size < 40 || map_size > cap {
        return (false, STRUCTURAL_FAIL, 0, 0, 0, map_size, desc_size);
    }
    let total = map_size / desc_size;
    // Invariant 2: read the exact buffer whose call produced `map_key`.
    let pushed =
        info.fill_from_raw_descriptors(core::slice::from_raw_parts(buf, map_size), desc_size);
    (true, status, pushed, total, map_key, map_size, desc_size)
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
/// `MEMMAP ok=<0|1> status=<0x...> entries=<pushed>/<total> truncated=<0|1>
/// map_size=<dec> desc_size=<dec>`.
/// `status` is the raw `GetMemoryMap` `EFI_STATUS`, or `0xffffffffffffffff`
/// ([`STRUCTURAL_FAIL`]) for structural failures (never `ok=0 status=0x0`).
/// `map_size`/`desc_size` baseline the firmware map against the 16 KiB
/// static + 4 KiB pool-slack budget (#59a/#7; zero-risk serial fields —
/// `MEMMAP ok=1` / `truncated=0` e2e substrings stay byte-identical).
fn write_memmap_line<E>(
    ok: bool,
    status: usize,
    pushed: usize,
    total: usize,
    map_size: usize,
    desc_size: usize,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "MEMMAP ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " status=")?;
    write_u64_hex(emit, status as u64)?;
    write_str(emit, " entries=")?;
    write_u64_dec(emit, pushed as u64)?;
    write_str(emit, "/")?;
    write_u64_dec(emit, total as u64)?;
    write_str(emit, " truncated=")?;
    write_u64_dec(emit, u64::from(pushed < total))?;
    write_str(emit, " map_size=")?;
    write_u64_dec(emit, map_size as u64)?;
    write_str(emit, " desc_size=")?;
    write_u64_dec(emit, desc_size as u64)?;
    write_str(emit, "\n")
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

/// Map a physical table base to the next `&[u64; 512]` via the OVMF
/// identity map (firmware identity-maps RAM, so phys == virt here).
///
/// Fail-closed (`None` -> `walk_full` [`MissingTable`](baseos_kernel::page_table::WalkerError::MissingTable)):
/// misaligned / zero / outside every [`BootInfo`] region (OOM) via
/// [`table_base_in_boot_map`]. Callers must only pass bases produced by
/// the walker, never attacker-controlled values.
///
/// # Safety
/// `phys` must be a live firmware page-table frame (bounds-checked above);
/// the returned reference borrows firmware-owned memory for the call only.
unsafe fn phys_to_table_ptr(info: &BootInfo, phys: u64) -> Option<&[u64; 512]> {
    if !table_base_in_boot_map(info, phys) {
        return None;
    }
    Some(&*(phys as *const [u64; 512]))
}

/// Emit the one-line `PAGING` diagnostic after the frame-allocator proof:
/// `PAGING ok=<0|1> cr3=0x... phys=0x...` — `ok` is 1 only when a full
/// [`walk_full`] from the live `CR3` resolved the probe address; `phys` is
/// the resolved physical address, or `0xffffffffffffffff`
/// ([`STRUCTURAL_FAIL`]) when the walk failed so serial never shows a
/// bare `phys=0x0` success/failure ambiguity.
fn write_paging_line<E>(
    ok: bool,
    cr3: u64,
    phys: u64,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "PAGING ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " cr3=")?;
    write_u64_hex(emit, cr3)?;
    write_str(emit, " phys=")?;
    write_u64_hex(emit, phys)?;
    write_str(emit, "\n")
}

/// Probe live paging post-`ExitBootServices`: read `CR3`, resolve one
/// known-mapped address (framebuffer base when GOP filled it, else low
/// `0x1000`) through [`walk_full`] with the identity-map lookup.
///
/// Returns `(ok, cr3, phys_or_sentinel)`: `phys` is the walked address on
/// success, [`STRUCTURAL_FAIL`] as `u64` on failure. Never panics.
///
/// # Safety
/// Post-EBS only, on the boot thread: reads `CR3` and dereferences
/// firmware page tables through [`phys_to_table_ptr`] (identity-mapped).
unsafe fn probe_paging(info: &BootInfo) -> (bool, u64, u64) {
    let cr3 = read_cr3();
    let probe_virt = info.framebuffer_opt().map(|fb| fb.base).unwrap_or(0x1000);
    let ok_phys = walk_full(cr3_pml4_base(cr3), probe_virt, |base| {
        phys_to_table_ptr(info, base)
    });
    match ok_phys {
        Ok(phys) => (true, cr3, phys),
        Err(_) => (false, cr3, STRUCTURAL_FAIL as u64),
    }
}

/// Emit the one-line `PERM` diagnostic after the `PAGING` line:
/// `PERM ok=<0|1> passed=<n>/<total>` — `ok` is 1 only when the synthetic
/// [`perm_selftest`] permission matrix (kernel RO/RW, user RW/NX, guard,
/// unmapped, non-canonical) passes all 9 probes. Pure software model, no
/// live-table mutation, so it runs deterministically post-EBS on any
/// firmware map; the live-table USER/NX-bit audit rides with a later
/// guest-hardening slice (reserved/MBZ open, #139 note 1).
fn write_perm_line<E>(
    ok: bool,
    passed: u32,
    total: u32,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "PERM ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " passed=")?;
    write_u64_dec(emit, u64::from(passed))?;
    write_str(emit, "/")?;
    write_u64_dec(emit, u64::from(total))?;
    write_str(emit, "\n")
}

/// Emit the one-line `GUARD` diagnostic after the `PERM` line:
/// `GUARD ok=<0|1> passed=<n>/<total>` — `ok` is 1 only when the synthetic
/// [`guard_selftest`] guard contract passes all 7 probes (adjacent mapped
/// pages pass, every guard touch faults `Guard`, next unmapped page is
/// `NotPresent`). Pure software model, no live-table mutation, so it runs
/// deterministically post-EBS on any firmware map. Hard-required by
/// `boot-uefi.ps1` (`GUARD ok=1`).
fn write_guard_line<E>(
    ok: bool,
    passed: u32,
    total: u32,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "GUARD ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " passed=")?;
    write_u64_dec(emit, u64::from(passed))?;
    write_str(emit, "/")?;
    write_u64_dec(emit, u64::from(total))?;
    write_str(emit, "\n")
}

/// Emit the one-line `PERMAUDIT` diagnostic after the `PERM` line:
/// `PERMAUDIT ok=<0|1> phys=<0x...> user=<0|1> nx=<0|1> w=<0|1>` — live-table
/// counterpart to the synthetic `PERM` matrix: `ok` is 1 only when
/// [`walk_full_leaf_flags`] resolved the same probe address the `PAGING`
/// line used; `phys` is the walked address (or `STRUCTURAL_FAIL`); `user`
/// / `nx` / `w` are the decoded leaf `USER` / `NO_EXEC` / `WRITABLE` bits
/// (0 on failure). Reporting-only this slice (no policy gate): the first
/// job is to baseline firmware USER/NX/W values on OVMF before any
/// enforcement slice decides what must fail. Hard-required by
/// `boot-uefi.ps1` (`PERMAUDIT ok=1` substring plus same-line ` w=1`
/// regex per REVIEW #153 gap 2).
///
/// S04 slice 20 adds a second probe line `PERMAUDIT2` (low-RAM `0x1000`)
/// via [`write_permaudit2_line`] so single-probe coverage (REVIEW #147
/// risk 1) no longer rests on the FB leaf alone.
fn write_permaudit_line<E>(
    ok: bool,
    phys: u64,
    user: bool,
    nx: bool,
    w: bool,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_tagged_permaudit_line("PERMAUDIT", ok, phys, user, nx, w, emit)
}

/// Emit the second-probe `PERMAUDIT2` line (slice 20, REVIEW #147 risk 1):
/// `PERMAUDIT2 ok=<0|1> phys=<0x...> user=<0|1> nx=<0|1> w=<0|1>` — same
/// shape as [`write_permaudit_line`] but for the low-RAM `0x1000` probe.
/// Hard-required (`PERMAUDIT2 ok=1`) by `boot-uefi.ps1`; `w` is
/// reporting-only until the baseline is measured.
fn write_permaudit2_line<E>(
    ok: bool,
    phys: u64,
    user: bool,
    nx: bool,
    w: bool,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_tagged_permaudit_line("PERMAUDIT2", ok, phys, user, nx, w, emit)
}

fn write_tagged_permaudit_line<E>(
    tag: &str,
    ok: bool,
    phys: u64,
    user: bool,
    nx: bool,
    w: bool,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, tag)?;
    write_str(emit, " ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " phys=")?;
    write_u64_hex(emit, phys)?;
    write_str(emit, " user=")?;
    write_u64_dec(emit, u64::from(user))?;
    write_str(emit, " nx=")?;
    write_u64_dec(emit, u64::from(nx))?;
    write_str(emit, " w=")?;
    write_u64_dec(emit, u64::from(w))?;
    write_str(emit, "\n")
}

/// Probe live leaf permission bits post-`ExitBootServices`: resolve the same
/// probe address [`probe_paging`] used (framebuffer base when GOP filled it,
/// else low `0x1000`) through [`walk_full_leaf_flags`] with the identity-map
/// lookup.
///
/// Returns `(ok, phys_or_sentinel, user, nx, w)`: `user` / `nx` / `w` are
/// the leaf `USER` / `NO_EXEC` / `WRITABLE` bits, false when the walk
/// failed. Never panics.
///
/// S04 slice 20: thin wrapper over [`probe_live_flags_for`] (REVIEW #147
/// risk 1 multi-probe: the low-RAM probe uses the same helper directly).
///
/// # Safety
/// Post-EBS only, on the boot thread: reads firmware page tables through
/// [`phys_to_table_ptr`] (identity-mapped).
unsafe fn probe_live_flags(info: &BootInfo) -> (bool, u64, bool, bool, bool) {
    let probe_virt = info.framebuffer_opt().map(|fb| fb.base).unwrap_or(0x1000);
    probe_live_flags_for(info, probe_virt)
}

/// Probe one virtual address through the live firmware tables (slice 20
/// helper for the multi-probe audit): same contract as
/// [`probe_live_flags`] but for an explicit `probe_virt` instead of the
/// FB-or-fallback default. Never panics.
///
/// # Safety
/// Post-EBS only, on the boot thread: reads firmware page tables through
/// [`phys_to_table_ptr`] (identity-mapped).
unsafe fn probe_live_flags_for(info: &BootInfo, probe_virt: u64) -> (bool, u64, bool, bool, bool) {
    let cr3 = read_cr3();
    match walk_full_leaf_flags(cr3_pml4_base(cr3), probe_virt, |base| {
        phys_to_table_ptr(info, base)
    }) {
        Ok((phys, flags)) => (
            true,
            phys,
            flags.contains(baseos_kernel::page_table::PageFlags::USER),
            flags.contains(baseos_kernel::page_table::PageFlags::NO_EXEC),
            flags.contains(baseos_kernel::page_table::PageFlags::WRITABLE),
        ),
        Err(_) => (false, STRUCTURAL_FAIL as u64, false, false, false),
    }
}

/// Emit the one-line `RMBZ` diagnostic after the `PERMAUDIT2` line:
/// `RMBZ ok=<0|1> nxen=<0|1> walks=<ok>/<total> viol=<n>` — live
/// reserved/MBZ audit (#139 note 1): `ok` is 1 only when every probe walk
/// succeeded (walk failures are reported via `walks`, never a silent
/// zero); `nxen` is the live `EFER.NXE` pin; `viol` counts leaves where
/// [`pte_reserved_violation`](baseos_kernel::page_table::pte_reserved_violation)
/// fires (PS at level 3/0, NX-bit while `NXE=0`).
/// Reporting-only this slice (no policy gate): the first job is to
/// Fail-closed since slice 36 (viol-enforcement policy): `ok` is driven
/// by [`rmbz_audit_ok`](baseos_kernel::page_table::rmbz_audit_ok), so any
/// `viol > 0` forces `ok=0`. Hard-required by `boot-uefi.ps1`
/// (`RMBZ ok=1` with same-line `viol=0`, mirroring the GUARD gate).
fn write_rmbz_line<E>(
    ok: bool,
    nx_enabled: bool,
    walks_ok: usize,
    walks_total: usize,
    violations: usize,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "RMBZ ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " nxen=")?;
    write_u64_dec(emit, u64::from(nx_enabled))?;
    write_str(emit, " walks=")?;
    write_u64_dec(emit, walks_ok as u64)?;
    write_str(emit, "/")?;
    write_u64_dec(emit, walks_total as u64)?;
    write_str(emit, " viol=")?;
    write_u64_dec(emit, violations as u64)?;
    write_str(emit, "\n")
}

/// Probe live reserved/MBZ violations post-`ExitBootServices`: read the
/// live `EFER.NXE` pin, walk both audit probes (framebuffer base when GOP
/// filled it, else low `0x1000`, plus low-RAM `0x1000` — deduplicated when
/// equal) through [`walk_full_leaf_raw`] with the identity-map lookup,
/// and count [`rmbz_count_violations`](baseos_kernel::page_table::rmbz_count_violations)
/// over the successfully walked leaves.
///
/// Returns `(ok, nx_enabled, walks_ok, walks_total, violations)`: `ok` is
/// [`rmbz_audit_ok`] (walks complete AND `viol == 0`); walk failures
/// contribute no leaf (the counter rule), so a failed walk lowers `ok`
/// via `walks` instead of masking `viol` to 0, and any violation lowers
/// `ok` fail-closed. Never panics.
///
/// # Safety
/// Post-EBS only, on the boot thread: reads `CR3`/`EFER` and dereferences
/// firmware page tables through [`phys_to_table_ptr`] (identity-mapped).
unsafe fn probe_rmbz(info: &BootInfo) -> (bool, bool, usize, usize, usize) {
    let nx_enabled = efer_nxe_enabled(read_efer());
    let fb_probe = info.framebuffer_opt().map(|fb| fb.base).unwrap_or(0x1000);
    let probes: [u64; 2] = [fb_probe, 0x1000];
    let total = if fb_probe == 0x1000 { 1 } else { 2 };
    let cr3 = cr3_pml4_base(read_cr3());
    let mut leaves: [(u64, u8); 2] = [(0, 0); 2];
    let mut ok_count = 0usize;
    let mut leaf_count = 0usize;
    let mut i = 0usize;
    while i < total {
        if let Ok((_phys, raw, level)) =
            walk_full_leaf_raw(cr3, probes[i], |base| phys_to_table_ptr(info, base))
        {
            leaves[leaf_count] = (raw, level);
            leaf_count += 1;
            ok_count += 1;
        }
        i += 1;
    }
    let violations = rmbz_count_violations(&leaves[..leaf_count], nx_enabled);
    (
        rmbz_audit_ok(ok_count, total, violations),
        nx_enabled,
        ok_count,
        total,
        violations,
    )
}

/// S06 slice 7 live-RSP probe: two 8 KiB stacks, 16-byte aligned so
/// `Thread::new` accepts them. Static (not stack) so the boot stack stays
/// small; single-threaded boot owns them exclusively post-EBS.
#[repr(align(16))]
#[allow(dead_code)]
struct AlignedStack([u8; 8192]);

static mut THREAD_STACK_A: AlignedStack = AlignedStack([0; 8192]);
static mut THREAD_STACK_B: AlignedStack = AlignedStack([0; 8192]);

/// Second-thread entry: runs ON the swapped-to stack (the `call` in
/// `run_on_thread_stack` pushes its return address there, and this body
/// spills its own frame there). Emits the proof line, then returns so the
/// probe restores the boot stack. If the new `RSP` were unmapped or
/// misaligned this faults and e2e fails on the missing lines.
extern "C" fn thread_b_entry() {
    let mut emit = com1_emit;
    let _ = write_str(&mut emit, "THREAD-RUN id=2\n");
}

// S06 slice 9: naked hostile callee — trashes the callee-saved set
// (`rbx`, `rbp`, `r12`-`r14`) then returns. `global_asm` (no compiler
// prologue/epilogue), so the trash is real as seen by the caller:
// `run_on_thread_stack`'s caller-stack push/pop must undo it, or the
// boot thread corrupts within a few stress iterations (fail-loud via
// missing `THREAD-CS` line, never silent).
#[cfg(not(test))]
core::arch::global_asm!(
    r#"
    .global thread_clobber_asm
    thread_clobber_asm:
        movabs rbx, 0xDEADDEADDEADDEAD
        movabs rbp, 0xBEEFBEEFBEEFBEEF
        movabs r12, 0x1212121212121212
        movabs r13, 0x1313131313131313
        movabs r14, 0x1414141414141414
        ret
"#
);

#[cfg(not(test))]
extern "C" {
    fn thread_clobber_asm();
}

/// Emit the one-line `THREAD-CS` diagnostic: `THREAD-CS ok=<0|1> iter=<n>`
/// — `ok` is 1 only when the 16-iteration clobber stress loop ran on the
/// swap-target stack and returned every time.
#[cfg(not(test))]
fn write_thread_cs_line<E>(
    ok: bool,
    iters: u64,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "THREAD-CS ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " iter=")?;
    write_u64_dec(emit, iters)?;
    write_str(emit, "\n")
}

/// Emit the one-line `THREAD` diagnostic after the `RMBZ` line:
/// `THREAD ok=<0|1> id=<n> top=<0x...> entry=<0x...>` — `ok` is 1 only
/// when two `Thread::new` descriptors validated, `swap_target` resolved,
/// and `run_on_thread_stack` ran `thread_b_entry` on the new `RSP` and
/// returned (the `THREAD-RUN` line above proves the new stack executed).
/// Failure prints `ok=0` with the sentinel `top`/`entry` zeroed so serial
/// never shows a plausible-but-unproven `RSP`/`RIP`.
fn write_thread_line<E>(
    ok: bool,
    id: u64,
    top: u64,
    entry: u64,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "THREAD ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " id=")?;
    write_u64_dec(emit, id)?;
    write_str(emit, " top=")?;
    write_u64_hex(emit, top)?;
    write_str(emit, " entry=")?;
    write_u64_hex(emit, entry)?;
    write_str(emit, "\n")
}

/// Live thread-stack swap post-`ExitBootServices`: build two descriptors
/// from the static stacks, resolve the round-robin target through
/// `swap_target` (the same gate future `swap` asm must use), run the
/// returned `entry` on the target stack, and emit the `THREAD` line.
///
/// REVIEW #287 risk 2: the call goes through the `entry` half of the
/// `swap_target` triple (transmuted to `extern "C" fn()`), not a direct
/// `thread_b_entry` reference — the emitted `entry=` hex is the address
/// actually called, so the triple is proven callable end-to-end. A null
/// or garbage `entry` faults loudly (missing lines fail e2e).
///
/// Returns the `(ok, id, top, entry)` the line reports. Never panics.
///
/// # Safety
/// Post-EBS only, on the boot thread: owns the static stacks, `cli` in
/// effect (caller upholds it — same contract as `bring_up_cpu...`), COM1
/// exclusive. A bad stack faults loudly (missing lines fail e2e).
unsafe fn probe_thread_swap<E>(
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(bool, u64, u64, u64), E> {
    let base_a = core::ptr::addr_of!(THREAD_STACK_A) as u64;
    let base_b = core::ptr::addr_of!(THREAD_STACK_B) as u64;
    let entry_b = thread_b_entry as *const () as u64;
    let (a, b) = match (
        Thread::new(1, base_a, 8192, 0x1000),
        Thread::new(2, base_b, 8192, entry_b),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        _ => {
            write_thread_line::<E>(false, 0, 0, 0, emit)?;
            return Ok((false, 0, 0, 0));
        }
    };
    let (id, top, entry) = match swap_target(&[a, b], 1) {
        Ok(t) => t,
        Err(_) => {
            write_thread_line::<E>(false, 0, 0, 0, emit)?;
            return Ok((false, 0, 0, 0));
        }
    };
    if entry == 0 {
        write_thread_line::<E>(false, 0, 0, 0, emit)?;
        return Ok((false, 0, 0, 0));
    }
    let func: extern "C" fn() = core::mem::transmute(entry as *const ());
    run_on_thread_stack(top, func);
    write_thread_line::<E>(true, id, top, entry, emit)?;
    Ok((true, id, top, entry))
}

/// S06 slice 9 callee-saved stress: resolve the same `swap_target` triple,
/// then run the naked clobber stub on the target stack 16 times through
/// `run_on_thread_stack` and emit the `THREAD-CS` line. Any push/pop
/// imbalance (or missing callee-saved spill) corrupts the caller stack
/// within a few iterations and faults loudly (missing lines fail e2e).
/// Returns `Ok(true)` only on the full 16-iteration run. Never panics.
///
/// # Safety
/// Post-EBS only, same contract as [`probe_thread_swap`]: owns the static
/// stacks, `cli` in effect, COM1 exclusive.
#[allow(unused_variables)]
unsafe fn probe_thread_callee_saved<E>(
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<bool, E> {
    #[cfg(not(test))]
    {
        let base_a = core::ptr::addr_of!(THREAD_STACK_A) as u64;
        let base_b = core::ptr::addr_of!(THREAD_STACK_B) as u64;
        let entry_b = thread_b_entry as *const () as u64;
        let (a, b) = match (
            Thread::new(1, base_a, 8192, 0x1000),
            Thread::new(2, base_b, 8192, entry_b),
        ) {
            (Ok(a), Ok(b)) => (a, b),
            _ => {
                write_thread_cs_line::<E>(false, 0, emit)?;
                return Ok(false);
            }
        };
        let (_, top, _) = match swap_target(&[a, b], 1) {
            Ok(t) => t,
            Err(_) => {
                write_thread_cs_line::<E>(false, 0, emit)?;
                return Ok(false);
            }
        };
        // `thread_clobber_asm` is a naked asm stub (unsafe foreign fn
        // under this toolchain); it only writes the callee-saved regs
        // and returns, and `run_on_thread_stack` preserves exactly that
        // set on the caller stack — so calling it as a safe fn pointer
        // is sound here. Same transmute idiom as `probe_thread_swap`.
        let clobber_addr = thread_clobber_asm as *const () as usize as u64;
        if clobber_addr == 0 {
            write_thread_cs_line::<E>(false, 0, emit)?;
            return Ok(false);
        }
        let clobber: extern "C" fn() = unsafe { core::mem::transmute(clobber_addr as *const ()) };
        let mut k = 0;
        while k < 16 {
            run_on_thread_stack(top, clobber);
            k += 1;
        }
        write_thread_cs_line::<E>(true, 16, emit)?;
        Ok(true)
    }
    #[cfg(test)]
    {
        Ok(true)
    }
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
        let (ok, status, pushed, total, map_key, map_size, desc_size) =
            fill_boot_info(system_table, &mut info);
        let mut emit = com1_emit;
        write_boot_summary::<Infallible>(&info, &mut emit).unwrap();
        write_memmap_line::<Infallible>(ok, status, pushed, total, map_size, desc_size, &mut emit)
            .unwrap();
        write_gop_line(gop_ok, gop_status);
        // S02 slice 2: leave the firmware world. After a successful EBS no
        // boot-service call is legal — kernel_entry + GDT/IDT/`#DE` run on
        // bare CPU + our COM1 only (agent-b #38 caveat: KERNEL ok=1 must be
        // post-EBS to honestly claim the S01 end goal).
        let (ebs_ok, ebs_status) = exit_boot_services(image_handle, system_table, map_key);
        write_ebs_line(ebs_ok, ebs_status);
        if ebs_ok {
            // REVIEW #287 risk 3: the post-EBS `cli in effect` contract was
            // doc-only (zero `cli` in the tree). Mask interrupts here, once,
            // before any post-EBS probe — the thread swap, paging audits,
            // and GDT/IDT bring-up below all run with IRQs masked.
            // Agent-b REVIEW #289 nit: `cli` clears IF so `preserves_flags`
            // is false — `options(nomem, nostack)` only.
            core::arch::asm!("cli", options(nomem, nostack));
            kernel_entry::<Infallible>(&info, &mut emit).unwrap();
            // S03 #5: post-EBS, COM1-only frame-allocator stress proof
            // (`FRAMEALLOC ok=1` hard-required by boot-uefi.ps1) before
            // the GDT/IDT/#DE bring-up parks the guest.
            baseos_kernel::frame_alloc::frame_alloc_stress::<Infallible>(&info, &mut emit).unwrap();
            // S04 slice 13: post-EBS live-paging proof — read CR3, walk the
            // framebuffer (or low RAM) through the identity map, emit the
            // PAGING line (hard-required by boot-uefi.ps1) before bring-up.
            let (paging_ok, paging_cr3, paging_phys) = probe_paging(&info);
            write_paging_line::<Infallible>(paging_ok, paging_cr3, paging_phys, &mut emit).unwrap();
            // S04 slice 14: synthetic permission-fault matrix (kernel/user,
            // guard, NX, canonical) — deterministic post-EBS proof with a
            // hard-required PERM line before bring-up.
            let (perm_ok, perm_passed, perm_total) = perm_selftest();
            write_perm_line::<Infallible>(perm_ok, perm_passed, perm_total, &mut emit).unwrap();
            // S04 slice 24: synthetic guard-page contract (adjacent mapped
            // pages pass, every guard touch faults Guard) — deterministic
            // post-EBS proof with a hard-required GUARD line before bring-up.
            let (guard_ok, guard_passed, guard_total) = guard_selftest();
            write_guard_line::<Infallible>(guard_ok, guard_passed, guard_total, &mut emit).unwrap();
            // S04 slice 15: live leaf USER/NX audit of the PAGING probe
            // address — reporting-only baseline before any enforcement.
            // S04 slice 20 (REVIEW #147 risk 1): second probe of low-RAM
            // 0x1000 on its own PERMAUDIT2 line, so one unmapped/odd leaf
            // cannot hide behind the FB probe.
            let (audit_ok, audit_phys, audit_user, audit_nx, audit_w) = probe_live_flags(&info);
            write_permaudit_line::<Infallible>(
                audit_ok, audit_phys, audit_user, audit_nx, audit_w, &mut emit,
            )
            .unwrap();
            let (audit2_ok, audit2_phys, audit2_user, audit2_nx, audit2_w) =
                probe_live_flags_for(&info, 0x1000);
            write_permaudit2_line::<Infallible>(
                audit2_ok,
                audit2_phys,
                audit2_user,
                audit2_nx,
                audit2_w,
                &mut emit,
            )
            .unwrap();
            // S04 slice 34 (#139 note 1): live reserved/MBZ audit over the
            // same two probes via the raw-leaf walker with the live
            // EFER.NXE pin — fail-closed since slice 36 (viol > 0 => ok=0).
            let (rmbz_ok, rmbz_nxen, rmbz_walks_ok, rmbz_walks_total, rmbz_viol) =
                probe_rmbz(&info);
            write_rmbz_line::<Infallible>(
                rmbz_ok,
                rmbz_nxen,
                rmbz_walks_ok,
                rmbz_walks_total,
                rmbz_viol,
                &mut emit,
            )
            .unwrap();
            // S06 slice 7: live-RSP probe — swap to a thread stack, run
            // `thread_b_entry` there (THREAD-RUN line), restore, report
            // THREAD ok=1. Faults loudly (missing lines) on a bad RSP.
            let _ = probe_thread_swap::<Infallible>(&mut emit);
            // S06 slice 9: callee-saved stress — 16 clobber swaps on the
            // same target stack, THREAD-CS ok=1 iter=16 (fail-loud).
            let _ = probe_thread_callee_saved::<Infallible>(&mut emit);
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
        let (ok, status, pushed, total, key, map_size, desc_size) =
            unsafe { fill_boot_info(st.as_ptr(), &mut info) };
        assert!(!ok);
        assert_eq!(status, STRUCTURAL_FAIL);
        assert_eq!((pushed, total, key), (0, 0, 0));
        // #59a baseline: last-call sizes still surface on structural fail
        // (map_size left at input cap; desc_size=8 from the stand).
        assert_eq!(map_size, MEMMAP_BUF_LEN);
        assert_eq!(desc_size, 8);
        // Raw firmware error passes through unchanged.
        let bs = fake_bs_with_slot(
            BOOT_SERVICES_GET_MEMORY_MAP_OFF,
            get_map_raw_error as *const (),
        );
        let st = fake_table(SYSTEM_TABLE_SIGNATURE, bs.as_ptr());
        let (ok, status, pushed, total, key, map_size, desc_size) =
            unsafe { fill_boot_info(st.as_ptr(), &mut info) };
        assert!(!ok);
        assert_eq!(status, 0x8000_0000_0000_000E);
        assert_eq!((pushed, total, key), (0, 0, 0));
        assert_eq!((map_size, desc_size), (MEMMAP_BUF_LEN, 0));
        // Missing/stale system table (slot load fails) is structural too —
        // no GetMemoryMap call ran, so both sizes stay 0.
        let (ok, status, pushed, total, key, map_size, desc_size) =
            unsafe { fill_boot_info(core::ptr::null(), &mut info) };
        assert!(!ok);
        assert_eq!(status, STRUCTURAL_FAIL);
        assert_eq!((pushed, total, key), (0, 0, 0));
        assert_eq!((map_size, desc_size), (0, 0));
    }

    /// #59a/#7: `MEMMAP` line embeds `map_size`/`desc_size` after
    /// `truncated` while the e2e hard-require substrings
    /// (`MEMMAP ok=1`, `truncated=0`) stay byte-identical.
    #[test]
    fn write_memmap_line_embeds_budget_fields() {
        let mut text = Vec::new();
        write_memmap_line::<core::convert::Infallible>(true, 0, 98, 98, 4704, 48, &mut |b| {
            text.push(b);
            Ok(())
        })
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(
            line,
            "MEMMAP ok=1 status=0x0 entries=98/98 truncated=0 map_size=4704 desc_size=48\n"
        );
        assert!(line.contains("MEMMAP ok=1"));
        assert!(line.contains("truncated=0"));

        let mut text = Vec::new();
        write_memmap_line::<core::convert::Infallible>(
            false,
            STRUCTURAL_FAIL,
            0,
            0,
            MEMMAP_BUF_LEN,
            8,
            &mut |b| {
                text.push(b);
                Ok(())
            },
        )
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(
            line,
            "MEMMAP ok=0 status=0xffffffffffffffff entries=0/0 truncated=0 map_size=16384 desc_size=8\n"
        );
        assert!(line.contains("MEMMAP ok=0 status=0xffffffffffffffff"));
        // Failure with pushed < total still reports truncated=1.
        let mut text = Vec::new();
        write_memmap_line::<core::convert::Infallible>(true, 0, 2, 3, 144, 48, &mut |b| {
            text.push(b);
            Ok(())
        })
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(
            line,
            "MEMMAP ok=1 status=0x0 entries=2/3 truncated=1 map_size=144 desc_size=48\n"
        );
    }

    /// S04 slice 13: `PAGING` line shape — ok carries the walked phys,
    /// failure carries the sentinel (never a bare `phys=0x0` ambiguity),
    /// and the e2e hard-require substring (`PAGING ok=1`) is byte-identical.
    #[test]
    fn write_paging_line_embeds_cr3_and_phys() {
        let mut text = Vec::new();
        write_paging_line::<core::convert::Infallible>(true, 0x1000, 0x8000_0000, &mut |b| {
            text.push(b);
            Ok(())
        })
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(line, "PAGING ok=1 cr3=0x1000 phys=0x80000000\n");
        assert!(line.contains("PAGING ok=1"));

        let mut text = Vec::new();
        write_paging_line::<core::convert::Infallible>(
            false,
            0x1000,
            STRUCTURAL_FAIL as u64,
            &mut |b| {
                text.push(b);
                Ok(())
            },
        )
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(line, "PAGING ok=0 cr3=0x1000 phys=0xffffffffffffffff\n");
    }

    /// S04 slice 14: `PERM` line shape — ok carries passed/total, and the
    /// e2e hard-require substring (`PERM ok=1`) is byte-identical.
    #[test]
    fn write_perm_line_embeds_passed_total() {
        let mut text = Vec::new();
        write_perm_line::<core::convert::Infallible>(true, 9, 9, &mut |b| {
            text.push(b);
            Ok(())
        })
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(line, "PERM ok=1 passed=9/9\n");
        assert!(line.contains("PERM ok=1"));

        let mut text = Vec::new();
        write_perm_line::<core::convert::Infallible>(false, 8, 9, &mut |b| {
            text.push(b);
            Ok(())
        })
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(line, "PERM ok=0 passed=8/9\n");
    }

    /// S04 slice 24: `GUARD` line shape — ok carries passed/total, and the
    /// e2e hard-require substring (`GUARD ok=1`) is byte-identical.
    #[test]
    fn write_guard_line_embeds_passed_total() {
        let mut text = Vec::new();
        write_guard_line::<core::convert::Infallible>(true, 7, 7, &mut |b| {
            text.push(b);
            Ok(())
        })
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(line, "GUARD ok=1 passed=7/7\n");
        assert!(line.contains("GUARD ok=1"));

        let mut text = Vec::new();
        write_guard_line::<core::convert::Infallible>(false, 6, 7, &mut |b| {
            text.push(b);
            Ok(())
        })
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(line, "GUARD ok=0 passed=6/7\n");
    }

    /// S04 slice 15/18/19: `PERMAUDIT` line shape — live leaf flags;
    /// e2e hard-requires `PERMAUDIT ok=1` AND ` w=1` on the same line
    /// (slice 19 gate tightening per REVIEW #153 gap 2: `ok` is
    /// walk-success, independent of `w`).
    #[test]
    fn write_permaudit_line_embeds_phys_user_nx_w() {
        let mut text = Vec::new();
        write_permaudit_line::<core::convert::Infallible>(
            true,
            0x8000_0000,
            false,
            true,
            true,
            &mut |b| {
                text.push(b);
                Ok(())
            },
        )
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(line, "PERMAUDIT ok=1 phys=0x80000000 user=0 nx=1 w=1\n");
        assert!(line.contains("PERMAUDIT ok=1"));
        assert!(line.contains(" w=1"));

        let mut text = Vec::new();
        write_permaudit_line::<core::convert::Infallible>(
            false,
            STRUCTURAL_FAIL as u64,
            false,
            false,
            false,
            &mut |b| {
                text.push(b);
                Ok(())
            },
        )
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(
            line,
            "PERMAUDIT ok=0 phys=0xffffffffffffffff user=0 nx=0 w=0\n"
        );
    }

    /// S04 slice 20 (REVIEW #147 risk 1): `PERMAUDIT2` second-probe line
    /// shape — same fields as `PERMAUDIT`, distinct tag so the ps1 gate
    /// can require both probes independently.
    #[test]
    fn write_permaudit2_line_embeds_phys_user_nx_w() {
        let mut text = Vec::new();
        write_permaudit2_line::<core::convert::Infallible>(
            true,
            0x1000,
            false,
            true,
            true,
            &mut |b| {
                text.push(b);
                Ok(())
            },
        )
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(line, "PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=1 w=1\n");
        assert!(line.contains("PERMAUDIT2 ok=1"));
        assert!(line.contains(" w=1"));
    }

    /// S04 slice 34 (#139 note 1): `RMBZ` line shape — ok carries
    /// walks/total plus the live NXE pin and violation count; failure
    /// carries walks shortfall (never a silent viol=0 mask), and the e2e
    /// hard-require substring (`RMBZ ok=1`) is byte-identical.
    #[test]
    fn write_rmbz_line_embeds_nxen_walks_viol() {
        let mut text = Vec::new();
        write_rmbz_line::<core::convert::Infallible>(true, true, 2, 2, 0, &mut |b| {
            text.push(b);
            Ok(())
        })
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(line, "RMBZ ok=1 nxen=1 walks=2/2 viol=0\n");
        assert!(line.contains("RMBZ ok=1"));
        assert!(line.contains(" viol=0"));

        let mut text = Vec::new();
        write_rmbz_line::<core::convert::Infallible>(false, false, 1, 2, 1, &mut |b| {
            text.push(b);
            Ok(())
        })
        .unwrap();
        let line = core::str::from_utf8(&text).unwrap();
        assert_eq!(line, "RMBZ ok=0 nxen=0 walks=1/2 viol=1\n");
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
        let (ok, status, pushed, total, key, map_size, desc_size) =
            unsafe { fill_boot_info(st.as_ptr(), &mut info) };
        assert!(ok, "dance must succeed through the pool buffer");
        assert_eq!(status, 0);
        assert_eq!(GET_MAP_CALLS.load(Ordering::Relaxed), 2);
        assert_eq!(
            ALLOC_SIZE.load(Ordering::Relaxed),
            MEMMAP_BUF_LEN + 4096 + GET_MAP_POOL_SLACK
        );
        // Invariant 2: fill parsed the pool buffer that produced the key.
        assert_eq!((pushed, total, key), (1, 1, 0x7777));
        // #59a: sizes come from the final successful call (stride 48 bytes).
        assert_eq!((map_size, desc_size), (48, 48));
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

    /// S03 #6 (agent-b REVIEW #75: must not be confused with mode 0's raw
    /// error): a stale-key EBS whose `refresh_map_key` hits the static
    /// 16 KiB ceiling — `GetMemoryMap` answers `EFI_BUFFER_TOO_SMALL` with
    /// required `map_size > MEMMAP_BUF_LEN` (map outgrown; the refresh path
    /// is single-shot by design, no pool dance) — fails closed with the
    /// original `EFI_INVALID_PARAMETER`, never fabricates a key, and never
    /// retries EBS: exactly one EBS attempt and one refresh attempt against
    /// the shared static buffer at full capacity. Takes `MEMMAP_TEST_LOCK`
    /// (`refresh_map_key` writes the shared static `MEMMAP_BUF`).
    #[test]
    fn exit_boot_services_refresh_fails_closed_when_map_outgrows_static() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        static EBS_CALLS: AtomicUsize = AtomicUsize::new(0);
        static REFRESH_CALLS: AtomicUsize = AtomicUsize::new(0);
        static REQUIRED_MAP_SIZE: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "efiapi" fn get_map_refresh_outgrown(
            map_size: *mut usize,
            buf: *mut u8,
            _key: *mut usize,
            desc_size: *mut usize,
            _desc_version: *mut u32,
        ) -> usize {
            REFRESH_CALLS.fetch_add(1, Ordering::Relaxed);
            // Single-shot refresh targets the static buffer at full capacity.
            assert_eq!(buf, core::ptr::addr_of_mut!(MEMMAP_BUF).cast::<u8>());
            assert_eq!(*map_size, MEMMAP_BUF_LEN);
            // The map has outgrown the static: required size crosses the
            // 16 KiB budget — the #6 scenario, distinct from mode 0's raw
            // EFI_NOT_FOUND-style error in the refresh-failure test.
            let required = MEMMAP_BUF_LEN + 4096;
            *map_size = required;
            REQUIRED_MAP_SIZE.store(required, Ordering::Relaxed);
            *desc_size = 48;
            EFI_BUFFER_TOO_SMALL
        }

        unsafe extern "efiapi" fn ebs_stale_after_outgrown_refresh(
            _ih: *const (),
            _key: usize,
        ) -> usize {
            EBS_CALLS.fetch_add(1, Ordering::Relaxed);
            EFI_INVALID_PARAMETER
        }

        let _guard = MEMMAP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut bs = fake_bs_with_slot(
            BOOT_SERVICES_EXIT_BOOT_SERVICES_OFF,
            ebs_stale_after_outgrown_refresh as *const (),
        );
        bs[BOOT_SERVICES_GET_MEMORY_MAP_OFF..BOOT_SERVICES_GET_MEMORY_MAP_OFF + 8]
            .copy_from_slice(&(get_map_refresh_outgrown as *const () as usize).to_le_bytes());
        let st = fake_table(SYSTEM_TABLE_SIGNATURE, bs.as_ptr());
        EBS_CALLS.store(0, Ordering::Relaxed);
        REFRESH_CALLS.store(0, Ordering::Relaxed);
        REQUIRED_MAP_SIZE.store(0, Ordering::Relaxed);

        let (ok, status) = unsafe { exit_boot_services(core::ptr::null(), st.as_ptr(), 0x1234) };
        assert!(!ok);
        assert_eq!(status, EFI_INVALID_PARAMETER);
        assert_eq!(EBS_CALLS.load(Ordering::Relaxed), 1);
        assert_eq!(REFRESH_CALLS.load(Ordering::Relaxed), 1);
        // Proof the stand really drove the outgrown-map branch (required
        // map_size > 16 KiB), not a generic raw error.
        assert!(REQUIRED_MAP_SIZE.load(Ordering::Relaxed) > MEMMAP_BUF_LEN);
    }

    #[test]
    fn guest_uses_kernel_uart_and_cpu_wrappers() {
        // S05 slice 8 UART unify (+ REVIEW #248 risk 1 for decode): pin the
        // guest to the `kernel::{uart, cpu}` wrappers so a silent revert to
        // local port-I/O or `page_table::` decode fails here, not in review.
        // Needles are built via `concat` so this test's own source does not
        // contain them verbatim (else negative asserts would self-match).
        const SRC: &str = include_str!("main.rs");
        let uart_use = ["kernel:", ":uart"].concat();
        assert!(
            SRC.contains(&uart_use),
            "guest must import kernel uart wrappers"
        );
        let cr3_wrap = ["cr3_pml4_", "base"].concat();
        let nxe_wrap = ["efer_nxe_", "enabled"].concat();
        assert!(
            SRC.contains(&cr3_wrap) && SRC.contains(&nxe_wrap),
            "guest must decode via cpu wrappers"
        );
        let forbidden: Vec<(String, &str)> = vec![
            (["unsafe fn out", "b"].concat(), "local outb"),
            (["unsafe fn in", "b"].concat(), "local inb"),
            (["unsafe fn uart_", "init"].concat(), "local uart_init"),
            (
                ["unsafe fn com1_write_", "byte"].concat(),
                "local com1_write_byte",
            ),
            (["page_table:", ":cr3_decode"].concat(), "page_table decode"),
            (
                ["page_table:", ":efer_nxe_enabled"].concat(),
                "page_table nxe",
            ),
        ];
        for (needle, label) in &forbidden {
            assert!(!SRC.contains(needle), "guest must not contain {label}");
        }
    }

    #[test]
    fn s05_unsafe_inventory_pins_boundary_shape() {
        // S05 slice 1: `docs/unsafe-inventory.md` is the exhaustive unsafe
        // inventory (sweeps recorded in the worklog row). This test pins the
        // doc against the boundary shape so a dropped site or a changed
        // constant fails here, not in review.
        // Slice 2 hardening (REVIEW #236 nit 1): substring site names alone
        // would still pass if a table row were deleted while the name
        // survived elsewhere, so every §A–E row anchor is pinned too.
        const DOC: &str = include_str!("../../../docs/unsafe-inventory.md");
        for site in [
            "outb",
            "inb",
            "uart_init",
            "com1_write_byte",
            "com1_emit",
            "load_boot_services",
            "load_get_memory_map",
            "load_allocate_pool",
            "load_locate_protocol",
            "load_exit_boot_services",
            "fill_boot_info",
            "refresh_map_key",
            "exit_boot_services",
            "fill_framebuffer",
            "read_cr3",
            "read_efer",
            "phys_to_table_ptr",
            "probe_paging",
            "probe_live_flags",
            "probe_rmbz",
            "efi_main",
        ] {
            assert!(DOC.contains(site), "inventory must document {site}");
        }
        for row in [
            "| A1 |", "| A2 |", "| A3 |", "| A4 |", "| A5 |", "| A6 |", "| A7 |", "| B1 |",
            "| B2 |", "| B3 |", "| B4 |", "| C1 |", "| C2 |", "| C3 |", "| D1 |", "| D2 |",
            "| D3 |", "| D4 |", "| D5 |", "| D6 |", "| D7 |", "| D8 |", "| D9 |", "| D10 |",
        ] {
            assert!(DOC.contains(row), "inventory must keep row {row}");
        }
        assert_eq!(SYSTEM_TABLE_BOOT_SERVICES_OFF, 96);
        assert_eq!(BOOT_SERVICES_GET_MEMORY_MAP_OFF, 56);
        assert_eq!(BOOT_SERVICES_ALLOCATE_POOL_OFF, 64);
        assert_eq!(BOOT_SERVICES_LOCATE_PROTOCOL_OFF, 320);
        assert_eq!(BOOT_SERVICES_EXIT_BOOT_SERVICES_OFF, 232);
        assert_eq!(GOP_MODE_OFF, 24);
        assert_eq!(GOP_MODE_INFO_OFF, 8);
        assert_eq!(GOP_MODE_SIZE_OF_INFO_OFF, 16);
        assert_eq!(GOP_MODE_FB_BASE_OFF, 24);
        assert_eq!(GOP_MODE_FB_SIZE_OFF, 32);
        assert_eq!(GOP_INFO_MIN, 36);
        assert_eq!(MEMMAP_BUF_LEN, 16384);
        assert_eq!(GET_MAP_MAX_ATTEMPTS, 4);
        assert_eq!(GET_MAP_POOL_SLACK, 4096);
        assert_eq!(STRUCTURAL_FAIL, usize::MAX);
    }
}
