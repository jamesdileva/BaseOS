//! S01 UEFI guest: emits the serial-marker contract on COM1, fills a
//! [`baseos_boot_info::BootInfo`] from UEFI `GetMemoryMap` plus the GOP
//! framebuffer, prints the [`baseos_boot_info::write_boot_summary`] debug
//! summary, transfers the handoff to [`baseos_kernel::kernel_entry`]
//! (S02), then parks the CPU.
//!
//! Minimal `no_std` UEFI application (`efi_main`): it parses the raw
//! `EFI_SYSTEM_TABLE`/`EFI_BOOT_SERVICES` tables with fixed byte offsets
//! (no `uefi` crate — dependency-free slice), calls `GetMemoryMap` into a
//! static 16 KiB buffer, locates the Graphics Output Protocol via
//! `LocateProtocol`, fills `BootInfo` via
//! [`baseos_boot_info::BootInfo::fill_from_raw_descriptors`] /
//! [`baseos_boot_info::BootInfo::fill_from_gop_mode`], and reports the
//! result on COM1. `tools/qemu/boot-uefi.ps1` builds this for
//! `x86_64-unknown-uefi`, boots OVMF, and asserts the marker plus the
//! `BOOTINFO valid=1`, `MEMMAP ok=1`, `FB present=1`, and `KERNEL ok=1`
//! summary lines in the serial capture.

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
/// Byte offset of `LocateProtocol` in `EFI_BOOT_SERVICES` (header 24 + the
/// 37 preceding function pointers/reserved slots through
/// `LocateHandleBuffer`, 8 bytes each = 320).
const BOOT_SERVICES_LOCATE_PROTOCOL_OFF: usize = 320;
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

/// Raw `GetMemoryMap` scratch: 16 KiB holds ~340 descriptors at the usual
/// 48-byte stride; QEMU/OVMF reports far fewer. Static (not stack) so the
/// boot stack stays small; local-only to the single-threaded boot path.
static mut MEMMAP_BUF: [u8; 16384] = [0; 16384];

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

/// Call `GetMemoryMap` into the static buffer and fill `info`.
///
/// Returns `(ok, status, pushed, total)`: `ok` is true only when the
/// firmware call succeeded; `status` is the raw `EFI_STATUS`; `pushed` is
/// the number of regions stored (0 on failure); `total` is the firmware
/// descriptor count (`map_size / desc_size`, 0 on failure). `pushed < total`
/// means the 128-entry inline map truncated the firmware map — reported on
/// serial as `truncated=1` so e2e fails loudly (review #29 risk 1). Never
/// panics; a failure still leaves `info` valid-but-empty so the serial
/// summary stays diagnosable.
///
/// Deferred (review #29 risk 2, candidate S03): single-shot 16 KiB call
/// with no `BUFFER_TOO_SMALL` retry. Fine for QEMU (~100 entries) but a
/// larger bare-metal map fails loudly with `(false, status, 0, 0)` and no
/// recovery path — recorded here so it is not mistaken for a complete
/// driver.
///
/// # Safety
/// Must run pre-`ExitBootServices` on the boot thread with `system_table`
/// live. Exclusively borrows the static buffer for the call duration.
unsafe fn fill_boot_info(
    system_table: *const u8,
    info: &mut BootInfo,
) -> (bool, usize, usize, usize) {
    let Some(get_map) = load_get_memory_map(system_table) else {
        return (false, usize::MAX, 0, 0);
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
    if status != 0 || desc_size < 40 || map_size > buf.len() {
        return (false, status, 0, 0);
    }
    let total = map_size / desc_size;
    let pushed = info.fill_from_raw_descriptors(&buf[..map_size], desc_size);
    (true, status, pushed, total)
}

/// Emit the one-line `MEMMAP` diagnostic after the two-line boot summary:
/// `MEMMAP ok=<0|1> status=<0x...> entries=<pushed>/<total> truncated=<0|1>`.
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
/// the raw `EFI_STATUS` of `LocateProtocol` (0 also for structural failures
/// after a successful locate — diagnose via the `FB present=` line's raw
/// fields). Never panics; a failure leaves the framebuffer unset so
/// `framebuffer_opt()` reports `FB present=0` and the e2e script fails
/// loudly (review #33/#35 fail-loud gate requirement).
///
/// # Safety
/// Must run pre-`ExitBootServices` on the boot thread with `system_table`
/// live; dereferences firmware-owned GOP `Mode`/`Info` pointers for the
/// duration of the call only.
unsafe fn fill_framebuffer(system_table: *const u8, info: &mut BootInfo) -> (bool, usize) {
    let Some(locate) = load_locate_protocol(system_table) else {
        return (false, usize::MAX);
    };
    let mut gop: *mut u8 = core::ptr::null_mut();
    let status = locate(GOP_GUID.as_ptr(), core::ptr::null(), &mut gop);
    if status != 0 || gop.is_null() {
        return (false, status);
    }
    let mode = (gop.add(GOP_MODE_OFF) as *const *const u8).read_unaligned();
    if mode.is_null() {
        return (false, status);
    }
    let info_ptr = (mode.add(GOP_MODE_INFO_OFF) as *const *const u8).read_unaligned();
    let size_of_info = (mode.add(GOP_MODE_SIZE_OF_INFO_OFF) as *const usize).read_unaligned();
    let fb_base = (mode.add(GOP_MODE_FB_BASE_OFF) as *const u64).read_unaligned();
    let fb_size = (mode.add(GOP_MODE_FB_SIZE_OFF) as *const usize).read_unaligned();
    if info_ptr.is_null() || size_of_info < GOP_INFO_MIN {
        return (false, status);
    }
    let mode_info = core::slice::from_raw_parts(info_ptr, GOP_INFO_MIN);
    let ok = info.fill_from_gop_mode(fb_base, fb_size as u64, mode_info);
    (ok, status)
}

/// Emit the one-line `GOP` diagnostic after the `MEMMAP` line:
/// `GOP ok=<0|1> status=<0x...>` — `ok` covers locate + mode-info parse;
/// `status` is the raw `LocateProtocol` `EFI_STATUS`.
fn write_gop_line(ok: bool, status: usize) {
    let mut emit = com1_emit;
    let _ = write_str(&mut emit, "GOP ok=");
    let _ = write_u64_dec(&mut emit, u64::from(ok));
    let _ = write_str(&mut emit, " status=");
    let _ = write_u64_hex(&mut emit, status as u64);
    let _ = write_str(&mut emit, "\n");
}

/// UEFI entry point: marker, memory-map + framebuffer fill, summary, park.
///
/// The firmware passes `(image_handle, system_table)` per the UEFI spec;
/// the handle is unused (no `Unload` path), the table feeds `GetMemoryMap`
/// and GOP `LocateProtocol`.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "efiapi" fn efi_main(_image_handle: *const (), system_table: *const u8) -> usize {
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
        let (ok, status, pushed, total) = fill_boot_info(system_table, &mut info);
        let (gop_ok, gop_status) = fill_framebuffer(system_table, &mut info);
        let mut emit = com1_emit;
        write_boot_summary::<Infallible>(&info, &mut emit).unwrap();
        write_memmap_line(ok, status, pushed, total);
        write_gop_line(gop_ok, gop_status);
        // S02 kernel entry: transfer the handoff to crates/kernel and emit
        // the KERNEL ok=<0|1> proof line (e2e hard-require in boot-uefi.ps1).
        kernel_entry::<Infallible>(&info, &mut emit).unwrap();
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
        let bs_empty = vec![0u8; BOOT_SERVICES_GET_MEMORY_MAP_OFF + 8];
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
        let bs_empty = vec![0u8; BOOT_SERVICES_LOCATE_PROTOCOL_OFF + 8];
        let no_slot = fake_table(SYSTEM_TABLE_SIGNATURE, bs_empty.as_ptr());
        assert!(unsafe { load_locate_protocol(no_slot.as_ptr()) }.is_none());
    }
}
