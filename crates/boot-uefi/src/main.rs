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
use baseos_kernel::sched::Scheduler;
use baseos_kernel::thread::check_jump_entry;
use baseos_kernel::thread::entry_as_func;
#[cfg(not(test))]
use baseos_kernel::thread::jump_rsp_refuses;
#[cfg(not(test))]
use baseos_kernel::thread::jump_to_thread_stack;
#[cfg(not(test))]
use baseos_kernel::thread::swap_stacks_via_slots;
#[cfg(not(test))]
use baseos_kernel::thread::{callee_saved_survives, rbp_preserved, run_on_thread_stack_with_save};
#[cfg(not(test))]
use baseos_kernel::thread::{
    callee_saved_survives_via_slots, callee_saved_survives_with_save, rbp_preserved_via_slots,
    rbp_preserved_with_save,
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

/// S06 slice 7 live-RSP probe: three 8 KiB stacks, 16-byte aligned so
/// `Thread::new` accepts them. Static (not stack) so the boot stack stays
/// small; single-threaded boot owns them exclusively post-EBS. Slice26
/// adds `THREAD_STACK_C` for the guest 1->2->3->1 multi-hop (live jmp
/// multi-hop itself is the next slice; this slice pins the layout +
/// host dispatch parity so the guest cannot smuggle an overlapping or
/// misaligned third stack).
#[repr(align(16))]
#[allow(dead_code)]
struct AlignedStack([u8; 8192]);

static mut THREAD_STACK_A: AlignedStack = AlignedStack([0; 8192]);
static mut THREAD_STACK_B: AlignedStack = AlignedStack([0; 8192]);
// Slice26a: live multi-hop consumes C next slice; allow dead_code on the
// UEFI target until then (host tests already pin the layout + parity).
#[allow(dead_code)]
static mut THREAD_STACK_C: AlignedStack = AlignedStack([0; 8192]);

/// Caller-owned two-way save slot (S06 slice 12): `run_on_thread_stack_
/// with_save` spills the boot `RSP` here before switching, so the save
/// lives in caller memory (ping-pong capable) rather than only on the new
/// stack. Zero until the `THREAD-SAVE` probe runs.
#[cfg(not(test))]
static mut THREAD_RSP_SAVE: u64 = 0;

/// Second caller-owned slot (S06 slice 13): the nested B->A hop spills
/// the thread-B `RSP` here via `swap_stacks_via_slots`, so the full
/// A->B->A ping-pong leaves both directions' stacks in memory. Zero
/// until the `THREAD-PONG` probe runs.
#[cfg(not(test))]
static mut THREAD_RSP_SAVE_B: u64 = 0;

/// Second-thread entry: runs ON the swapped-to stack (the `call` in
/// `run_on_thread_stack` pushes its return address there, and this body
/// spills its own frame there). Emits the proof line, then returns so the
/// probe restores the boot stack. If the new `RSP` were unmapped or
/// misaligned this faults and e2e fails on the missing lines.
extern "C" fn thread_b_entry() {
    let mut emit = com1_emit;
    let _ = write_str(&mut emit, "THREAD-RUN id=2\n");
}

// S06 slice 10: naked hostile callee — trashes the full callee-saved set
// (`rbx`, `rbp`, `r12`-`r15`) then returns. `global_asm` (no compiler
// prologue/epilogue), so the trash is real as seen by the caller:
// `run_on_thread_stack`'s caller-stack push/pop must undo it, or the
// boot thread corrupts within a few stress iterations (fail-loud via
// missing `THREAD-CS` line, never silent). Slice 9 covered only
// `rbx`/`rbp`/`r12`-`r14`, leaving the old `r15`-as-RSP spill untested
// (REVIEW #291 gap 2); slice 10 spills old `RSP` to the new stack and
// trashes `r15` too, so the SPoF path is proven.
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
        movabs r15, 0x1515151515151515
        ret
"#
);

#[cfg(not(test))]
extern "C" {
    fn thread_clobber_asm();
}

/// Emit the one-line `THREAD-SAVE` diagnostic (S06 slice 12, first true
/// two-way half): `THREAD-SAVE ok=<0|1> save=<0x...>` — `ok` is 1 only
/// when `run_on_thread_stack_with_save` ran `thread_b_entry` on the
/// swap-target stack, returned to the boot stack, AND the caller-owned
/// save slot holds a nonzero 16-byte-aligned boot `RSP` (proves the save
/// to memory happened, not just the new-stack return slot).
#[cfg(not(test))]
fn write_thread_save_line<E>(
    ok: bool,
    save: u64,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "THREAD-SAVE ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " save=")?;
    write_u64_hex(emit, save)?;
    write_str(emit, "\n")
}

/// Two-way save-slot probe (S06 slice 12): resolve the same `swap_target`
/// triple, run `thread_b_entry` through `run_on_thread_stack_with_save`
/// with `THREAD_RSP_SAVE` as the caller-owned slot, then check the slot
/// is a nonzero 16-byte-aligned `RSP` and emit the `THREAD-SAVE` line.
/// Liveness (return) + save proof; full A->B->A ping-pong is the next
/// slice. Never panics.
///
/// # Safety
/// Post-EBS only, same contract as [`probe_thread_swap`]: owns the static
/// stacks plus the save slot, `cli` in effect, COM1 exclusive.
#[cfg(not(test))]
unsafe fn probe_thread_save<E>(emit: &mut impl FnMut(u8) -> Result<(), E>) -> Result<bool, E> {
    let base_a = core::ptr::addr_of!(THREAD_STACK_A) as u64;
    let base_b = core::ptr::addr_of!(THREAD_STACK_B) as u64;
    let entry_b = thread_b_entry as *const () as u64;
    let (a, b) = match (
        Thread::new(1, base_a, 8192, 0x1000),
        Thread::new(2, base_b, 8192, entry_b),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        _ => {
            write_thread_save_line::<E>(false, 0, emit)?;
            return Ok(false);
        }
    };
    let (_, top, entry) = match swap_target(&[a, b], 1) {
        Ok(t) => t,
        Err(_) => {
            write_thread_save_line::<E>(false, 0, emit)?;
            return Ok(false);
        }
    };
    if entry == 0 {
        write_thread_save_line::<E>(false, 0, emit)?;
        return Ok(false);
    }
    // Nonzero-only gate at the transmute (S06 hardening): the `entry == 0`
    // check above stays as the first fail-closed layer; `entry_as_func`
    // re-enforces it where the fn pointer materializes, so a future edit
    // cannot drop the check without the transmute refusing.
    let func = match entry_as_func(entry) {
        Some(f) => f,
        None => {
            write_thread_save_line::<E>(false, 0, emit)?;
            return Ok(false);
        }
    };
    let slot = core::ptr::addr_of_mut!(THREAD_RSP_SAVE);
    // Fail-closed: zero the slot first so a no-save path prints ok=0
    // instead of a stale nonzero value.
    core::ptr::write_volatile(slot, 0);
    run_on_thread_stack_with_save(slot, top, func);
    let saved = core::ptr::read_volatile(slot);
    let ok = saved != 0 && saved & 0xF == 0;
    write_thread_save_line::<E>(ok, if ok { saved } else { 0 }, emit)?;
    Ok(ok)
}

#[cfg(test)]
unsafe fn probe_thread_save<E>(emit: &mut impl FnMut(u8) -> Result<(), E>) -> Result<bool, E> {
    // Host covers the triple-resolution shape (no live RSP swap).
    let _ = emit;
    let a = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
    let b = Thread::new(2, 0x2000_0000, 8192, 0x2000).unwrap();
    Ok(swap_target(&[a, b], 1).is_ok())
}

/// Pong entry (S06 slice 13): runs ON THE BOOT STACK inside the nested
/// B->A hop (`swap_stacks_via_slots` loaded the boot `RSP` from
/// `THREAD_RSP_SAVE`). Emits the nested proof line, then returns so the
/// nested epilogue restores thread-B and the outer epilogue restores boot.
/// If the slot LOAD were bogus this faults and e2e fails on the missing
/// lines. Kept separate from `thread_b_entry` so the earlier THREAD /
/// THREAD-SAVE probes keep their simple-return shape.
#[cfg(not(test))]
extern "C" fn thread_pong_entry() {
    let mut emit = com1_emit;
    let _ = write_str(&mut emit, "THREAD-PONG-RUN id=1\n");
}

/// Ping-pong thread-B entry (S06 slices 13-14): runs ON THREAD-B via the
/// outer `run_on_thread_stack_with_save` (which spilled boot `RSP` to
/// `THREAD_RSP_SAVE`). Performs the nested B->A hop: read-once
/// `read_volatile` of slot A (REVIEW #303 risk 3: exactly one read, value
/// travels in a register — asm never re-reads the slot), resolve the pong
/// `entry` through the `swap_target` triple for B->A (REVIEW #303 risk 2:
/// no hardcoded entry — the same gate future `swap` asm must use), and run
/// it on the loaded boot `RSP` via the slot-to-value switch. Return resumes
/// B, then the outer epilogue resumes boot — the full A->B->A round trip.
/// Kept separate from `thread_b_entry` so the earlier probes are untouched.
#[cfg(not(test))]
extern "C" fn thread_b_pingpong_entry() {
    unsafe {
        // Read-once: single volatile read of the boot-RSP slot; the value
        // below travels in a register into `swap_stacks_via_slots`.
        let boot_rsp = core::ptr::read_volatile(core::ptr::addr_of!(THREAD_RSP_SAVE));
        if boot_rsp == 0 || boot_rsp & 0xF != 0 {
            return;
        }
        // Entry via the triple (B->A), not a hardcoded reference: rebuild
        // the same descriptors the outer probe used (A entry = pong, B
        // entry = self) and resolve through `swap_target`.
        let base_a = core::ptr::addr_of!(THREAD_STACK_A) as u64;
        let base_b = core::ptr::addr_of!(THREAD_STACK_B) as u64;
        let pong_addr = thread_pong_entry as *const () as u64;
        let self_addr = thread_b_pingpong_entry as *const () as u64;
        let (a, b) = match (
            Thread::new(1, base_a, 8192, pong_addr),
            Thread::new(2, base_b, 8192, self_addr),
        ) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return,
        };
        let (_, _, entry_a) = match swap_target(&[a, b], 2) {
            Ok(t) => t,
            Err(_) => return,
        };
        if entry_a == 0 {
            return;
        }
        let pong = match entry_as_func(entry_a) {
            Some(f) => f,
            None => return,
        };
        swap_stacks_via_slots(core::ptr::addr_of_mut!(THREAD_RSP_SAVE_B), boot_rsp, pong);
    }
}

/// Emit the one-line `THREAD-PONG` diagnostic (S06 slice 13, the LOAD
/// half that closes REVIEW #299): `THREAD-PONG ok=<0|1> save_a=<0x...>
/// save_b=<0x...>` — `ok` is 1 only when the outer save-slot hop ran
/// `thread_b_pingpong_entry` on the swap-target stack, the nested
/// slot-to-slot hop ran `thread_pong_entry` on the loaded boot `RSP`
/// (`THREAD-PONG-RUN` line above proves the LOAD executed), both hops
/// returned, AND both caller-owned slots hold nonzero 16-byte-aligned
/// `RSP` values (proves STORE + LOAD in both directions through memory).
#[cfg(not(test))]
fn write_thread_pong_line<E>(
    ok: bool,
    save_a: u64,
    save_b: u64,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "THREAD-PONG ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " save_a=")?;
    write_u64_hex(emit, save_a)?;
    write_str(emit, " save_b=")?;
    write_u64_hex(emit, save_b)?;
    write_str(emit, "\n")
}

/// Full A->B->A ping-pong probe (S06 slices 13-14): resolve the
/// `swap_target` triple for A->B (thread B entry IS `thread_b_pingpong_
/// entry`, so the triple `entry` half is the function actually called —
/// REVIEW #303 risk 2: no discarded `(_, top, _)` + hardcoded transmute),
/// zero both slots (stale-nonzero fail-open guard), run the triple `entry`
/// through `run_on_thread_stack_with_save` with `THREAD_RSP_SAVE` as slot
/// A; the B entry nests the slot-to-value switch (slot B <- B `RSP`,
/// target = read-once boot `RSP` value, entry = B->A triple entry) running
/// `thread_pong_entry` on the loaded boot stack. Checks both slots and
/// emits the `THREAD-PONG` line. Never panics.
///
/// # Safety
/// Post-EBS only, same contract as [`probe_thread_save`]: owns the static
/// stacks plus both save slots, `cli` in effect, COM1 exclusive.
#[cfg(not(test))]
unsafe fn probe_thread_pingpong<E>(emit: &mut impl FnMut(u8) -> Result<(), E>) -> Result<bool, E> {
    let base_a = core::ptr::addr_of!(THREAD_STACK_A) as u64;
    let base_b = core::ptr::addr_of!(THREAD_STACK_B) as u64;
    // Both entries are real fn addresses in the triple: A = pong (used by
    // the nested B->A hop via its own `swap_target` resolve), B = pingpong
    // (called below through the outer triple `entry`, not hardcoded).
    let pong_addr = thread_pong_entry as *const () as u64;
    let pingpong_addr = thread_b_pingpong_entry as *const () as u64;
    let (a, b) = match (
        Thread::new(1, base_a, 8192, pong_addr),
        Thread::new(2, base_b, 8192, pingpong_addr),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        _ => {
            write_thread_pong_line::<E>(false, 0, 0, emit)?;
            return Ok(false);
        }
    };
    let (_, top, entry_b) = match swap_target(&[a, b], 1) {
        Ok(t) => t,
        Err(_) => {
            write_thread_pong_line::<E>(false, 0, 0, emit)?;
            return Ok(false);
        }
    };
    if entry_b == 0 || entry_b != pingpong_addr {
        write_thread_pong_line::<E>(false, 0, 0, emit)?;
        return Ok(false);
    }
    let slot_a = core::ptr::addr_of_mut!(THREAD_RSP_SAVE);
    let slot_b = core::ptr::addr_of_mut!(THREAD_RSP_SAVE_B);
    // Fail-closed: zero both slots first so a no-save path prints ok=0
    // instead of a stale nonzero value (same guard as `probe_thread_save`).
    core::ptr::write_volatile(slot_a, 0);
    core::ptr::write_volatile(slot_b, 0);
    let func_b = match entry_as_func(entry_b) {
        Some(f) => f,
        None => {
            write_thread_pong_line::<E>(false, 0, 0, emit)?;
            return Ok(false);
        }
    };
    run_on_thread_stack_with_save(slot_a, top, func_b);
    let saved_a = core::ptr::read_volatile(slot_a);
    let saved_b = core::ptr::read_volatile(slot_b);
    let ok = saved_a != 0 && saved_a & 0xF == 0 && saved_b != 0 && saved_b & 0xF == 0;
    write_thread_pong_line::<E>(
        ok,
        if ok { saved_a } else { 0 },
        if ok { saved_b } else { 0 },
        emit,
    )?;
    Ok(ok)
}

#[cfg(test)]
unsafe fn probe_thread_pingpong<E>(emit: &mut impl FnMut(u8) -> Result<(), E>) -> Result<bool, E> {
    // Host covers the triple-resolution shape (no live RSP swap): the same
    // descriptors the guest builds must resolve through `swap_target`.
    let _ = emit;
    let a = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
    let b = Thread::new(2, 0x2000_0000, 8192, 0x2000).unwrap();
    Ok(swap_target(&[a, b], 1).is_ok())
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

/// Emit the one-line `SCHED-SWITCH` diagnostic (S06 slice18, minimal
/// guest-switch wiring, slice19 persistent two-hop): `SCHED-SWITCH ok=<0|1> from=<n> to=<m>` — `ok`
/// is 1 only when a `Scheduler` table (boot 1 + thread 2) dispatched
/// 1->2, the `swap_target` triple for the same pair agreed on id 2, and
/// the triple entry ran on the target stack and returned, plus a second
/// 2->1 dispatch/triple agreement on the same table (resolved only).
/// First-dispatch parks nothing (boot stays `Ready`), so the line pins guest/host parity:
/// scheduler pick == swap-gate pick == executed entry.
fn write_sched_line<E>(
    ok: bool,
    from: u64,
    to: u64,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "SCHED-SWITCH ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " from=")?;
    write_u64_dec(emit, from)?;
    write_str(emit, " to=")?;
    write_u64_dec(emit, to)?;
    write_str(emit, "\n")
}

/// Minimal scheduler-driven switch post-`ExitBootServices`: build the same
/// two descriptors the `THREAD` probe uses, drive the pick through
/// `Scheduler::dispatch` (not `switch_stub` directly), resolve the live
/// `RSP`/`RIP` through `swap_target`, require both picks to agree, run the
/// triple entry on the target stack, and emit the `SCHED-SWITCH` line.
/// Fail-closed (`ok=0`) on any spawn/dispatch/triple mismatch — never runs
/// a disagreeing entry. Never panics.
///
/// S06 slice19 (REVIEW #338 R1): the table is persistent, not one-shot —
/// after the live 1->2 hop a second `dispatch` must return 2->1 with the
/// B->A `swap_target` triple agreeing on id 1 (resolved only, not
/// executed: thread-1 entry `0x1000` is a placeholder, not a callable fn).
/// The emitted line is unchanged (`ok=1 from=1 to=2` covers the live hop).
///
/// # Safety
/// Post-EBS only, same contract as [`probe_thread_swap`]: owns the static
/// stacks, `cli` in effect, COM1 exclusive.
///
/// Host `cargo test` builds link `baseos_kernel` as a dependency (its
/// `cfg(test)` live-swap stubs do NOT apply), so the live `run_on_thread_
/// stack` path must never execute on the host — the `#[cfg(test)]` stub
/// below covers the dispatch/triple parity shape instead (live proof is
/// e2e-only, same pattern as `probe_thread_save`).
#[cfg(not(test))]
unsafe fn probe_sched_switch<E>(emit: &mut impl FnMut(u8) -> Result<(), E>) -> Result<bool, E> {
    let base_a = core::ptr::addr_of!(THREAD_STACK_A) as u64;
    let base_b = core::ptr::addr_of!(THREAD_STACK_B) as u64;
    let entry_b = thread_b_entry as *const () as u64;
    let (a, b) = match (
        Thread::new(1, base_a, 8192, 0x1000),
        Thread::new(2, base_b, 8192, entry_b),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        _ => {
            write_sched_line::<E>(false, 1, 0, emit)?;
            return Ok(false);
        }
    };
    let mut sched = Scheduler::new(1);
    if !sched.spawn(a) || !sched.spawn(b) {
        write_sched_line::<E>(false, 1, 0, emit)?;
        return Ok(false);
    }
    let next = match sched.dispatch() {
        Ok(n) => n,
        Err(_) => {
            write_sched_line::<E>(false, 1, 0, emit)?;
            return Ok(false);
        }
    };
    let (id, top, entry) = match swap_target(&[a, b], 1) {
        Ok(t) => t,
        Err(_) => {
            write_sched_line::<E>(false, 1, 0, emit)?;
            return Ok(false);
        }
    };
    if next != id || next != 2 || entry == 0 {
        write_sched_line::<E>(false, 1, id, emit)?;
        return Ok(false);
    }
    let func = match entry_as_func(entry) {
        Some(f) => f,
        None => {
            write_sched_line::<E>(false, 1, id, emit)?;
            return Ok(false);
        }
    };
    run_on_thread_stack(top, func);
    // Persistent-table proof: second hop 2->1 agrees with the B->A triple.
    let next2 = match sched.dispatch() {
        Ok(n) => n,
        Err(_) => {
            write_sched_line::<E>(false, 1, next, emit)?;
            return Ok(false);
        }
    };
    let (id2, _, _) = match swap_target(&[a, b], 2) {
        Ok(t) => t,
        Err(_) => {
            write_sched_line::<E>(false, 1, next, emit)?;
            return Ok(false);
        }
    };
    if next2 != 1 || id2 != 1 {
        write_sched_line::<E>(false, 1, next, emit)?;
        return Ok(false);
    }
    write_sched_line::<E>(true, 1, next, emit)?;
    Ok(true)
}

#[cfg(test)]
unsafe fn probe_sched_switch<E>(emit: &mut impl FnMut(u8) -> Result<(), E>) -> Result<bool, E> {
    // `baseos_kernel` dependency builds use the guest asm path here).
    let a = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
    let b = Thread::new(2, 0x2000_0000, 8192, 0x2000).unwrap();
    let mut sched = Scheduler::new(1);
    if !sched.spawn(a) || !sched.spawn(b) {
        write_sched_line::<E>(false, 1, 0, emit)?;
        return Ok(false);
    }
    let next = match sched.dispatch() {
        Ok(n) => n,
        Err(_) => {
            write_sched_line::<E>(false, 1, 0, emit)?;
            return Ok(false);
        }
    };
    let (id, _, entry) = match swap_target(&[a, b], 1) {
        Ok(t) => t,
        Err(_) => {
            write_sched_line::<E>(false, 1, 0, emit)?;
            return Ok(false);
        }
    };
    if next != id || next != 2 || entry == 0 {
        write_sched_line::<E>(false, 1, id, emit)?;
        return Ok(false);
    }
    // Persistent-table proof (slice19): second hop 2->1, same as guest.
    let next2 = match sched.dispatch() {
        Ok(n) => n,
        Err(_) => {
            write_sched_line::<E>(false, 1, next, emit)?;
            return Ok(false);
        }
    };
    let (id2, _, _) = match swap_target(&[a, b], 2) {
        Ok(t) => t,
        Err(_) => {
            write_sched_line::<E>(false, 1, next, emit)?;
            return Ok(false);
        }
    };
    if next2 != 1 || id2 != 1 {
        write_sched_line::<E>(false, 1, next, emit)?;
        return Ok(false);
    }
    write_sched_line::<E>(true, 1, next, emit)?;
    Ok(true)
}

/// Never-return jump entry (S06 slice22): runs ON the jumped-to stack (no
/// return address was pushed — `jmp`, not `call`), so this function MUST
/// NEVER `ret`. Emits the `THREAD-JUMP-RUN` proof line plus the
/// `THREAD-JUMP ok=1` line from the new stack, then chains into
/// `bring_up_cpu_and_prove_exception` (GDT/IDT + live `#PF` touch, which
/// emits `PF ok=1` then chains `#DE` → `EXC ok=1`) — the boot never
/// resumes on the old stack by construction (the caller `RSP` was already
/// proven through the save-slot probes; loss is intended here).
#[cfg(not(test))]
extern "C" fn thread_jump_entry() -> ! {
    let mut emit = com1_emit;
    let _ = write_str(&mut emit, "THREAD-JUMP-RUN id=2\n");
    let _ = write_thread_jump_line::<core::convert::Infallible>(true, 2, &mut emit);
    unsafe {
        baseos_kernel::bring_up_cpu_and_prove_exception();
    }
}

/// Emit the one-line `THREAD-JUMP` diagnostic (S06 slice22):
/// `THREAD-JUMP ok=<0|1> id=<n>` — `ok` is 1 only when the
/// `swap_target` triple AND the supplemental [`check_jump_entry`] gate
/// both passed on the same pair and the `jmp` landed (the
/// `THREAD-JUMP-RUN` line above proves the new stack executed). `ok=0`
/// is emitted by the probe without jumping (fail-closed funnel: the
/// caller stays on the boot stack and falls through to the legacy
/// bring-up, so `PF`/`EXC` still fire — e2e fails on the missing ok=1,
/// never on a silent hang).
fn write_thread_jump_line<E>(
    ok: bool,
    id: u64,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "THREAD-JUMP ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " id=")?;
    write_u64_dec(emit, id)?;
    write_str(emit, "\n")
}

/// Live `jmp`-entry probe (S06 slice22): build the same two descriptors
/// the `THREAD` probe uses (B entry IS `thread_jump_entry`), resolve the
/// `swap_target` triple for A->B, AND the supplemental
/// [`check_jump_entry`] gate on the same `(top, entry)` BEFORE loading
/// `RSP` (never gate-alone — the top-only gate passes lethal stacks the
/// triple refuses, pinned host-side). On any refusal emit
/// `THREAD-JUMP ok=0 id=0` and return `Ok(false)` (caller falls through
/// to the legacy bring-up — failure evidence, never silent). On success
/// `jmp` to the entry and never return (the entry chains to bring-up
/// from the new stack).
///
/// # Safety
/// Post-EBS only, same contract as [`probe_thread_swap`]: owns the static
/// stacks, `cli` in effect, COM1 exclusive.
///
/// Host `cargo test` builds cover the triple+gate parity shape only (no
/// live `RSP` swap — same pattern as `probe_sched_switch`).
#[cfg(not(test))]
unsafe fn probe_thread_jump<E>(emit: &mut impl FnMut(u8) -> Result<(), E>) -> Result<bool, E> {
    let base_a = core::ptr::addr_of!(THREAD_STACK_A) as u64;
    let base_b = core::ptr::addr_of!(THREAD_STACK_B) as u64;
    let entry_b = thread_jump_entry as *const () as u64;
    let (a, b) = match (
        Thread::new(1, base_a, 8192, 0x1000),
        Thread::new(2, base_b, 8192, entry_b),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        _ => {
            write_thread_jump_line::<E>(false, 0, emit)?;
            return Ok(false);
        }
    };
    let (id, top, entry) = match swap_target(&[a, b], 1) {
        Ok(t) => t,
        Err(_) => {
            write_thread_jump_line::<E>(false, 0, emit)?;
            return Ok(false);
        }
    };
    // Triple AND shared `RSP`-refusal predicate (S06 hardening): the
    // `jump_rsp_refuses` conjunct is redundant with `check_jump_entry`'s
    // top half by construction, wiring the host-pinned park decision into
    // the live path so a future gate edit cannot silently drop it.
    if id != 2 || !check_jump_entry(top, entry) || jump_rsp_refuses(top) {
        write_thread_jump_line::<E>(false, 0, emit)?;
        return Ok(false);
    }
    let func = match entry_as_func(entry) {
        Some(f) => f,
        None => {
            write_thread_jump_line::<E>(false, 0, emit)?;
            return Ok(false);
        }
    };
    jump_to_thread_stack(top, func);
}

#[cfg(test)]
unsafe fn probe_thread_jump<E>(emit: &mut impl FnMut(u8) -> Result<(), E>) -> Result<bool, E> {
    // Host covers the triple+gate parity shape (no live RSP swap).
    let a = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
    let jump_addr = 0x2000u64;
    let b = Thread::new(2, 0x2000_0000, 8192, jump_addr).unwrap();
    let (id, top, entry) = match swap_target(&[a, b], 1) {
        Ok(t) => t,
        Err(_) => {
            write_thread_jump_line::<E>(false, 0, emit)?;
            return Ok(false);
        }
    };
    if id != 2 || !check_jump_entry(top, entry) {
        write_thread_jump_line::<E>(false, 0, emit)?;
        return Ok(false);
    }
    write_thread_jump_line::<E>(true, id, emit)?;
    Ok(true)
}

/// Refusal-demo outcome funnel (S06 slice22 R3, REVIEW #368 fix): the
/// single place the `THREAD-JUMP ok=0 id=0` shape is emitted vs the
/// fail-loud gate-accept marker. `refused=true` (both bad values refused)
/// emits exactly `THREAD-JUMP ok=0 id=0` and returns `Ok(false)` (caller
/// falls through, PF/EXC still fire downstream). `refused=false` (a gate
/// accepted a corrupt value — regressed gate) emits the DISTINCT marker
/// `THREAD-JUMP-REFUSAL-FAIL gate_accepted=1` — never the `ok=0` shape —
/// and returns `Ok(true)` (divergent return so host tests and `efi_main`
/// can tell accept from refuse). The distinct marker plus the missing
/// `ok=0` line makes `boot-uefi.ps1` fail visibly (negative
/// `THREAD-JUMP-REFUSAL-FAIL` gate + required `THREAD-JUMP ok=0 id=0`
/// gate); `efi_main` additionally skips the live jump on `Ok(true)` so
/// the `ok=1` line goes missing too. Never panics.
fn emit_refusal_demo_result<E>(
    refused: bool,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<bool, E> {
    if refused {
        write_thread_jump_line::<E>(false, 0, emit)?;
        return Ok(false);
    }
    write_str(emit, "THREAD-JUMP-REFUSAL-FAIL gate_accepted=1\n")?;
    Ok(true)
}

/// Forced-refusal demo (S06 slice22 R2, hardened R3 per REVIEW #368):
/// proves the `THREAD-JUMP ok=0 id=0` funnel shape live in QEMU. Builds a
/// deliberately bad triple (null entry, so `Thread::new` refuses; plus a
/// `check_jump_entry` refusal on a zero top) and funnels the outcome
/// through [`emit_refusal_demo_result`]: healthy gates emit exactly one
/// `THREAD-JUMP ok=0 id=0` line and return `Ok(false)`; the caller falls
/// through (PF/EXC still fire downstream), so the serial proves refusal
/// does not hang or skip the fault chain. A regressed gate (either bad
/// value accepted) emits `THREAD-JUMP-REFUSAL-FAIL gate_accepted=1` and
/// returns `Ok(true)` — never the `ok=0` shape — so the `ok=0` serial line
/// actually proves refusal (the old R2 code emitted `ok=0` on both
/// branches, a tautology REVIEW #368). Runs BEFORE the live jump probe so
/// one healthy boot shows both `ok=0` (refusal shape) and `ok=1` (live jump).
fn probe_thread_jump_refusal_demo<E>(
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<bool, E> {
    // Corrupt entry: null entry refuses at construction.
    let bad_entry = Thread::new(1, 0x1000_0000, 8192, 0);
    // Corrupt top: zero top refuses at the supplemental gate.
    let bad_top_gate = check_jump_entry(0, 0x2000);
    let refused = bad_entry.is_err() && !bad_top_gate;
    emit_refusal_demo_result::<E>(refused, emit)
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
    let func = match entry_as_func(entry) {
        Some(f) => f,
        None => {
            write_thread_line::<E>(false, 0, 0, 0, emit)?;
            return Ok((false, 0, 0, 0));
        }
    };
    run_on_thread_stack(top, func);
    write_thread_line::<E>(true, id, top, entry, emit)?;
    Ok((true, id, top, entry))
}

/// S06 slice 10 callee-saved stress + preservation, slice 11 `rbp`
/// read-only proof: resolve the same `swap_target` triple, run the naked
/// clobber stub on the target stack 16 times through
/// `run_on_thread_stack` (liveness), then run ONE sentinel preservation
/// check via `callee_saved_survives` plus ONE read-only `rbp` check via
/// `rbp_preserved` (REVIEW #293 risk 1: `rbp` can never take a sentinel
/// write as frame pointer, so it is captured before/after and compared)
/// and emit the `THREAD-CS` line. Any push/pop imbalance corrupts within
/// a few iterations and faults loudly; a swapped-but-balanced pop order
/// (REVIEW #291 gap 1) passes liveness but fails the sentinel compare ->
/// `ok=0`; a missing/unbalanced `push rbp`/`pop rbp` passes the five-reg
/// sentinel (which excludes `rbp`) but fails the read-only compare ->
/// `ok=0`. Returns `Ok(true)` only on full 16-iteration run PLUS both
/// proofs. Never panics.
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
        // `thread_clobber_asm` is a naked asm stub: it only writes the
        // callee-saved regs and returns, and `run_on_thread_stack` preserves
        // exactly that set on the caller stack. Transmute the `*const ()`
        // directly (REVIEW #291 nit: the old `as usize as u64 -> as *const ()`
        // round-trip loses provenance for no reason).
        let clobber_ptr = thread_clobber_asm as *const ();
        if clobber_ptr.is_null() {
            write_thread_cs_line::<E>(false, 0, emit)?;
            return Ok(false);
        }
        // Same nonzero-only gate (the null check above stays as the first
        // layer; the helper refuses it again at the transmute).
        let clobber = match entry_as_func(clobber_ptr as u64) {
            Some(f) => f,
            None => {
                write_thread_cs_line::<E>(false, 0, emit)?;
                return Ok(false);
            }
        };
        let mut k = 0;
        while k < 16 {
            run_on_thread_stack(top, clobber);
            k += 1;
        }
        if !callee_saved_survives(top, clobber) {
            write_thread_cs_line::<E>(false, 16, emit)?;
            return Ok(false);
        }
        // S06 slice 11 (REVIEW #293 risk 1): read-only `rbp` proof against
        // the same hostile stub (which trashes `rbp` with 0xBEEF...). The
        // five-reg sentinel above excludes `rbp` by design (frame pointer),
        // so without this a `push rbp`/`pop rbp` omission would still print
        // `ok=1`. `THREAD-CS` line shape unchanged (`ok=1 iter=16`), still
        // hard-required by `boot-uefi.ps1`.
        if !rbp_preserved(top, clobber) {
            write_thread_cs_line::<E>(false, 16, emit)?;
            return Ok(false);
        }
        // S06 slice 14 (REVIEW #303 risk 1): the two newer asm paths get
        // the same hostile-callee treatment — clobber stub through
        // `with_save` (slot-owned save) and through the slot-to-value
        // switch (read-once boot-RSP-shaped value `top`), each with its own
        // sentinel + read-only `rbp` proof. The base-path checks above
        // would miss a push/pop imbalance confined to either new epilogue;
        // ` THREAD-CS` line shape unchanged (`ok=1 iter=16`), still
        // hard-required by `boot-uefi.ps1`.
        let slot = core::ptr::addr_of_mut!(THREAD_RSP_SAVE);
        core::ptr::write_volatile(slot, 0);
        run_on_thread_stack_with_save(slot, top, clobber);
        if core::ptr::read_volatile(slot) == 0 {
            write_thread_cs_line::<E>(false, 16, emit)?;
            return Ok(false);
        }
        if !callee_saved_survives_with_save(slot, top, clobber) {
            write_thread_cs_line::<E>(false, 16, emit)?;
            return Ok(false);
        }
        if !rbp_preserved_with_save(slot, top, clobber) {
            write_thread_cs_line::<E>(false, 16, emit)?;
            return Ok(false);
        }
        core::ptr::write_volatile(slot, 0);
        swap_stacks_via_slots(slot, top, clobber);
        if core::ptr::read_volatile(slot) == 0 {
            write_thread_cs_line::<E>(false, 16, emit)?;
            return Ok(false);
        }
        if !callee_saved_survives_via_slots(slot, top, clobber) {
            write_thread_cs_line::<E>(false, 16, emit)?;
            return Ok(false);
        }
        if !rbp_preserved_via_slots(slot, top, clobber) {
            write_thread_cs_line::<E>(false, 16, emit)?;
            return Ok(false);
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
            // S06 slice 12: two-way save-slot probe — THREAD-SAVE ok=1
            // (fail-loud on missing/zero/misaligned save).
            let _ = probe_thread_save::<Infallible>(&mut emit);
            // S06 slice 13: full A->B->A ping-pong — THREAD-PONG ok=1
            // (fail-loud on missing RUN line or zero/misaligned slot A/B).
            let _ = probe_thread_pingpong::<Infallible>(&mut emit);
            // S06 slice18: minimal scheduler-driven switch — SCHED-SWITCH
            // ok=1 from=1 to=2 (fail-loud on dispatch/triple mismatch).
            let _ = probe_sched_switch::<Infallible>(&mut emit);
            // S06 slice22 R2: forced-refusal demo first — emits
            // `THREAD-JUMP ok=0 id=0` through the refusal funnel on a corrupt
            // triple, then falls through (PF/EXC still fire downstream).
            // R3 (REVIEW #368): a regressed gate emits
            // `THREAD-JUMP-REFUSAL-FAIL` with Ok(true) instead of ok=0 —
            // skip the live jump so the `ok=1` line goes missing too and
            // e2e fails on two missing lines plus the FAIL marker, never
            // on a silent same-shape pass.
            let demo_gate_failed =
                probe_thread_jump_refusal_demo::<Infallible>(&mut emit).unwrap_or(true);
            // S06 slice22: live `jmp`-entry switch — THREAD-JUMP-RUN +
            // THREAD-JUMP ok=1 from the new stack, then the entry chains to
            // bring-up (PF/EXC) there and never returns. On refusal the
            // probe emits THREAD-JUMP ok=0 and returns, falling through to
            // the legacy bring-up (failure evidence, never silent).
            if !demo_gate_failed {
                let _ = probe_thread_jump::<Infallible>(&mut emit);
            }
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
    fn sched_switch_probe_parity_host() {
        // S06 slice18 guest/host parity: the scheduler-driven probe must
        // agree (dispatch 1->2 == swap_target 1->2) and emit the
        // `SCHED-SWITCH ok=1 from=1 to=2` line on the host too (live RSP
        // swap is a no-op stub here; the QEMU boot proves the live path).
        let mut out: Vec<u8> = Vec::new();
        let ok = unsafe {
            probe_sched_switch::<core::convert::Infallible>(&mut |b| {
                out.push(b);
                Ok(())
            })
        }
        .unwrap();
        assert!(ok);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("SCHED-SWITCH ok=1 from=1 to=2"));
    }

    #[test]
    fn sched_switch_fail_closed_line_shape() {
        // S06 slice19 (REVIEW #338 R4): the fail-closed `ok=0` paths must
        // emit the same `SCHED-SWITCH ok=0 from=<n> to=<m>` shape the ps1
        // gates on — pin the writer directly (spawn/dispatch/triple
        // mismatches all funnel through `write_sched_line(false, ..)`).
        let mut out: Vec<u8> = Vec::new();
        write_sched_line::<core::convert::Infallible>(false, 1, 0, &mut |b| {
            out.push(b);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "SCHED-SWITCH ok=0 from=1 to=0\n"
        );
        let mut out: Vec<u8> = Vec::new();
        write_sched_line::<core::convert::Infallible>(false, 1, 2, &mut |b| {
            out.push(b);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "SCHED-SWITCH ok=0 from=1 to=2\n"
        );
    }

    #[test]
    fn three_stacks_dispatch_parity_host() {
        // S06 slice26a: third stack layout + 3-thread dispatch parity.
        // The guest 1->2->3->1 live multi-hop needs STACK_C to be a
        // Thread-acceptable, non-overlapping 8 KiB stack BEFORE any live
        // jmp touches it, and the Scheduler dispatch path must agree with
        // the swap_target triple on every leg (live-state-aware, mirroring
        // the slice25 sched.rs pin — static all-Ready would hide Running).
        // Live multi-hop jmp itself is the next slice; this pins the floor.
        let base_a = core::ptr::addr_of!(THREAD_STACK_A) as u64;
        let base_b = core::ptr::addr_of!(THREAD_STACK_B) as u64;
        let base_c = core::ptr::addr_of!(THREAD_STACK_C) as u64;
        for base in [base_a, base_b, base_c] {
            assert_ne!(base, 0);
            assert_eq!(base & 0xF, 0, "static stacks stay 16-byte aligned");
        }
        assert!(base_a != base_b && base_b != base_c && base_a != base_c);
        // 8 KiB ranges must not overlap (linker could pack them adjacently,
        // never overlapping — overlap would make Thread::new refuse below).
        for (x, y) in [(base_a, base_b), (base_b, base_c), (base_a, base_c)] {
            let (lo, hi) = if x < y { (x, y) } else { (y, x) };
            assert!(lo.saturating_add(8192) <= hi, "stacks must not overlap");
        }
        let a = Thread::new(1, base_a, 8192, 0x1000).unwrap();
        let b = Thread::new(2, base_b, 8192, 0x2000).unwrap();
        let c = Thread::new(3, base_c, 8192, 0x3000).unwrap();
        let mut s = Scheduler::new(1);
        assert!(s.spawn(a));
        assert!(s.spawn(b));
        assert!(s.spawn(c));
        for current in [1u64, 2, 3] {
            let expect = if current == 3 { 1 } else { current + 1 };
            assert_eq!(s.current(), current);
            assert_eq!(s.pick_next(), Ok(expect));
            // Live snapshot: mirror scheduler states (Running appears
            // after the first dispatch; static all-Ready would hide it).
            fn live_as(t: Thread, st: baseos_kernel::thread::ThreadState) -> Thread {
                let mut m = t;
                if m.state() != st {
                    assert!(m.set_state(st), "live snapshot must reach scheduler state");
                }
                m
            }
            let live = [
                live_as(
                    Thread::new(1, base_a, 8192, 0x1000).unwrap(),
                    s.state_of(1).unwrap(),
                ),
                live_as(
                    Thread::new(2, base_b, 8192, 0x2000).unwrap(),
                    s.state_of(2).unwrap(),
                ),
                live_as(
                    Thread::new(3, base_c, 8192, 0x3000).unwrap(),
                    s.state_of(3).unwrap(),
                ),
            ];
            let (id, top, entry) = swap_target(&live, s.current()).unwrap();
            assert_eq!(id, expect);
            assert!(check_jump_entry(top, entry));
            assert!(entry_as_func(entry).is_some());
            assert_eq!(s.dispatch(), Ok(expect));
            assert_eq!(s.current(), expect);
        }
    }

    #[test]
    fn jump_probe_parity_host() {
        // S06 slice22 guest/host parity: the jmp-entry probe must resolve
        // the `swap_target` triple AND the supplemental `check_jump_entry`
        // gate on the same pair and emit `THREAD-JUMP ok=1 id=2` on the
        // host too (live RSP jmp is e2e-only; QEMU proves the live path).
        let mut out: Vec<u8> = Vec::new();
        let ok = unsafe {
            probe_thread_jump::<core::convert::Infallible>(&mut |b| {
                out.push(b);
                Ok(())
            })
        }
        .unwrap();
        assert!(ok);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("THREAD-JUMP ok=1 id=2"));
    }

    #[test]
    fn jump_fail_closed_line_shape() {
        // S06 slice22 ok=0 funnel: every refusal emits the same
        // `THREAD-JUMP ok=0 id=0` shape the ps1 gates on — pin the writer
        // directly (all probe refusals funnel through it).
        let mut out: Vec<u8> = Vec::new();
        write_thread_jump_line::<core::convert::Infallible>(false, 0, &mut |b| {
            out.push(b);
            Ok(())
        })
        .unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "THREAD-JUMP ok=0 id=0\n");
        let mut out: Vec<u8> = Vec::new();
        write_thread_jump_line::<core::convert::Infallible>(true, 2, &mut |b| {
            out.push(b);
            Ok(())
        })
        .unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "THREAD-JUMP ok=1 id=2\n");
    }

    #[test]
    fn jump_refusal_demo_emits_ok0_and_fails_ok1_gate() {
        // S06 slice22 R2: the corrupt-triple demo emits exactly the
        // `THREAD-JUMP ok=0 id=0` refusal shape and returns false (fall
        // through, never jump). A refusal-only serial therefore FAILS the
        // ps1 `THREAD-JUMP ok=1 id=2` gate (Contains check).
        let mut out: Vec<u8> = Vec::new();
        let ok = probe_thread_jump_refusal_demo::<core::convert::Infallible>(&mut |b| {
            out.push(b);
            Ok(())
        })
        .unwrap();
        assert!(!ok);
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text, "THREAD-JUMP ok=0 id=0\n");
        assert!(!text.contains("THREAD-JUMP ok=1 id=2"));
    }

    #[test]
    fn jump_refusal_demo_accept_path_is_fail_loud() {
        // S06 slice22 R3 (REVIEW #368): the accept path — a regressed gate
        // that lets a corrupt value through — must NOT emit the `ok=0`
        // shape (the R2 tautology: both branches printed ok=0, so a broken
        // gate still PASSed). It emits the distinct
        // `THREAD-JUMP-REFUSAL-FAIL` marker and returns Ok(true)
        // (divergent from the refuse path's Ok(false)), so the serial
        // shape differs and boot-uefi.ps1 fails on the missing ok=0 plus
        // the FAIL marker. Pinned through the same
        // `emit_refusal_demo_result` funnel the guest uses.
        let mut out: Vec<u8> = Vec::new();
        let ok = emit_refusal_demo_result::<core::convert::Infallible>(false, &mut |b| {
            out.push(b);
            Ok(())
        })
        .unwrap();
        assert!(ok, "accept path must diverge from the refuse path");
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text, "THREAD-JUMP-REFUSAL-FAIL gate_accepted=1\n");
        assert!(
            !text.contains("THREAD-JUMP ok=0"),
            "accept path must never emit the refusal shape"
        );
        assert!(
            !text.contains("THREAD-JUMP ok=1 id=2"),
            "accept path must never emit the live-jump shape"
        );
        // Refuse path through the same funnel: exact ok=0 shape, Ok(false).
        let mut out: Vec<u8> = Vec::new();
        let ok = emit_refusal_demo_result::<core::convert::Infallible>(true, &mut |b| {
            out.push(b);
            Ok(())
        })
        .unwrap();
        assert!(!ok);
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text, "THREAD-JUMP ok=0 id=0\n");
        assert!(!text.contains("REFUSAL-FAIL"));
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

    #[test]
    fn thread_entry_transmutes_route_through_nonzero_gate() {
        // S06 hardening (transmute nonzero-only + live-path wiring): every
        // thread-entry transmute site (entry u64 -> fn pointer) must route
        // through `entry_as_func`, and the live jump probe must enforce the
        // shared `jump_rsp_refuses` predicate. The firmware slot loaders
        // (`ExitBootServicesFn`/`GetMemoryMapFn`/`AllocatePoolFn`/
        // `LocateProtocolFn`) keep their own typed transmutes and are out
        // of scope — this pin covers the `entry` domain only.
        // NOTE: the raw needle is built via concat so this test's own
        // source cannot self-match it through `include_str!`.
        const SRC: &str = include_str!("main.rs");
        const NEEDLE: &str = concat!("transmute", "(entry");
        assert!(
            !SRC.contains(NEEDLE),
            "no raw entry transmute may remain outside entry_as_func"
        );
        assert!(
            SRC.contains("entry_as_func(entry)"),
            "thread probes must convert the triple entry via entry_as_func"
        );
        assert!(
            SRC.contains("entry_as_func(entry_a)"),
            "nested B->A hop must convert via entry_as_func"
        );
        assert!(
            SRC.contains("entry_as_func(entry_b)"),
            "ping-pong outer entry must convert via entry_as_func"
        );
        assert!(
            SRC.contains("entry_as_func(clobber_ptr as u64)"),
            "clobber stub must convert via entry_as_func"
        );
        assert!(
            SRC.contains("jump_rsp_refuses(top)"),
            "live jump probe must enforce the shared RSP-refusal predicate"
        );
    }
}
