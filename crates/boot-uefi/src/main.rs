//! S01 UEFI guest: emits the serial-marker contract on COM1, fills a
//! [`baseos_boot_info::BootInfo`] from UEFI `GetMemoryMap`, prints the
//! [`baseos_boot_info::write_boot_summary`] debug summary, then parks the CPU.
//!
//! Minimal `no_std` UEFI application (`efi_main`): it parses the raw
//! `EFI_SYSTEM_TABLE`/`EFI_BOOT_SERVICES` tables with fixed byte offsets
//! (no `uefi` crate — dependency-free slice), calls `GetMemoryMap` into a
//! static 16 KiB buffer, fills `BootInfo` via
//! [`baseos_boot_info::BootInfo::fill_from_raw_descriptors`], and reports the
//! result on COM1. `tools/qemu/boot-uefi.ps1` builds this for
//! `x86_64-unknown-uefi`, boots OVMF, and asserts the marker plus the
//! `BOOTINFO valid=1` summary in the serial capture. Framebuffer (GOP)
//! discovery arrives in a later S01 slice.

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
/// Byte offset of `GetMemoryMap` in `EFI_BOOT_SERVICES`
/// (header 24 + `RaiseTPL`/`RestoreTPL`/`AllocatePages`/`FreePages` 32 = 56).
const BOOT_SERVICES_GET_MEMORY_MAP_OFF: usize = 56;

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

/// Load the `GetMemoryMap` entry point from the raw system table.
///
/// # Safety
/// `system_table` must point to a live `EFI_SYSTEM_TABLE`; reads the
/// `BootServices` pointer at the fixed x86-64 offset, then the
/// `GetMemoryMap` slot at its fixed offset. Returns `None` on null links
/// instead of dereferencing them.
unsafe fn load_get_memory_map(system_table: *const u8) -> Option<GetMemoryMapFn> {
    if system_table.is_null() {
        return None;
    }
    let boot_services = (system_table.add(SYSTEM_TABLE_BOOT_SERVICES_OFF) as *const *const u8)
        .read_unaligned()
        .cast::<u8>();
    if boot_services.is_null() {
        return None;
    }
    let slot =
        (boot_services.add(BOOT_SERVICES_GET_MEMORY_MAP_OFF) as *const *const ()).read_unaligned();
    if slot.is_null() {
        return None;
    }
    Some(core::mem::transmute::<*const (), GetMemoryMapFn>(slot))
}

/// Call `GetMemoryMap` into the static buffer and fill `info`.
///
/// Returns `(ok, status, entries)`: `ok` is true only when the firmware
/// call succeeded; `status` is the raw `EFI_STATUS`; `entries` is the
/// number of regions pushed (0 on failure). Never panics; a failure still
/// leaves `info` valid-but-empty so the serial summary stays diagnosable.
///
/// # Safety
/// Must run pre-`ExitBootServices` on the boot thread with `system_table`
/// live. Exclusively borrows the static buffer for the call duration.
unsafe fn fill_boot_info(system_table: *const u8, info: &mut BootInfo) -> (bool, usize, usize) {
    let Some(get_map) = load_get_memory_map(system_table) else {
        return (false, usize::MAX, 0);
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
        return (false, status, 0);
    }
    let pushed = info.fill_from_raw_descriptors(&buf[..map_size], desc_size);
    (true, status, pushed)
}

/// Emit the one-line `MEMMAP` diagnostic after the two-line boot summary:
/// `MEMMAP ok=<0|1> status=<0x...> entries=<n>`.
fn write_memmap_line(ok: bool, status: usize, entries: usize) {
    let mut emit = com1_emit;
    let _ = write_str(&mut emit, "MEMMAP ok=");
    let _ = write_u64_dec(&mut emit, u64::from(ok));
    let _ = write_str(&mut emit, " status=");
    let _ = write_u64_hex(&mut emit, status as u64);
    let _ = write_str(&mut emit, " entries=");
    let _ = write_u64_dec(&mut emit, entries as u64);
    let _ = write_str(&mut emit, "\n");
}

/// UEFI entry point: marker, memory-map fill, summary, then park the CPU.
///
/// The firmware passes `(image_handle, system_table)` per the UEFI spec;
/// the handle is unused (no `Unload` path), the table feeds `GetMemoryMap`.
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
        let (ok, status, entries) = fill_boot_info(system_table, &mut info);
        let mut emit = com1_emit;
        write_boot_summary::<Infallible>(&info, &mut emit).unwrap();
        write_memmap_line(ok, status, entries);
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
