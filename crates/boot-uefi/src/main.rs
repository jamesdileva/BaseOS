//! S01 UEFI guest: emits the serial-marker contract on COM1, then parks the CPU.
//!
//! Minimal `no_std` UEFI application (`efi_main`, no boot-services calls
//! yet): it reuses [`baseos_serial_marker::write_marker`] so the emitted
//! bytes are single-sourced with the host-side contract, initializes COM1
//! (16550A) directly, and halts. `tools/qemu/boot-uefi.ps1` builds this for
//! `x86_64-unknown-uefi`, stages it as `EFI/BOOT/BOOTX64.EFI` on a QEMU
//! virtual-FAT ESP, boots OVMF, and asserts the marker in the serial capture.
//! Memory-map / framebuffer discovery arrive in later S01 slices.

#![cfg_attr(not(test), no_main)]
#![cfg_attr(not(test), no_std)]
// SAFETY (unsafe boundary, per `docs/coding-conventions.md`): port I/O
// (`in`/`out`) and `hlt` are privileged CPU instructions with no safe
// equivalent in a `no_std` guest, so they live in this smallest possible
// hardware boundary behind the safe wrappers `uart_init` / `com1_write_byte`
// below. Tested end-to-end by `tools/qemu/boot-uefi.ps1` (OVMF boot asserts
// the marker in the QEMU serial capture).
#![allow(unsafe_code)]

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

/// UEFI entry point: emit the serial-marker contract, then park the CPU.
///
/// The firmware passes `(image_handle, system_table)` per the UEFI spec;
/// this slice needs neither (no boot-services calls yet — S01 later slices
/// add memory-map / framebuffer discovery), so both are ignored.
#[no_mangle]
pub extern "efiapi" fn efi_main(_image_handle: *const (), _system_table: *const ()) -> usize {
    // SAFETY: single-threaded boot environment, COM1 is ours exclusively;
    // init-then-write order upheld by construction here. The sink is
    // infallible, so `unwrap` cannot panic.
    unsafe {
        uart_init();
        write_marker::<Infallible>(|b| {
            com1_write_byte(b);
            Ok(())
        })
        .unwrap();
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
