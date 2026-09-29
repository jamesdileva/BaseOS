//! Safe COM1 port-I/O boundary (S05 slice 2).
//!
//! Raw `in`/`out` instructions have no safe equivalent in a `no_std` guest,
//! so the `unsafe` stays confined to the two private bodies ([`outb`] /
//! [`inb`]) below. Everything else in this module is a **safe** wrapper:
//! safe callers emit bytes without an `unsafe` block. The only remaining
//! caller contract (documented, not compiler-enforced) is ordering +
//! exclusivity: S01 `uart_init` ran pre-handoff, and only the single-threaded
//! boot/exception path owns COM1 (post-EBS plain port I/O, `cli` in effect).

use super::{COM1_LSR, COM1_PORT, LSR_THR_EMPTY};

/// Write one byte to an ISA I/O port.
///
/// # Safety
/// Caller must pass a valid, writable port for this platform (here: COM1
/// UART registers, always present on the QEMU `pc` target). Private to this
/// module — safe callers never name a port.
#[inline(always)]
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!(
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
/// Private to this module.
#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!(
        "in al, dx",
        out("al") value,
        in("dx") port,
        options(nomem, nostack, preserves_flags),
    );
    value
}

/// Poll THR-empty, then write one byte to COM1 (post-EBS path: no firmware,
/// plain port I/O only).
///
/// Safe wrapper: the `unsafe` (port I/O) is confined to this body. Caller
/// contract is ordering + exclusivity only — COM1 was initialized via the
/// S01 guest's `uart_init` before `kernel_entry`, and the single-threaded
/// boot/exception path owns the UART.
pub(crate) fn com1_write_byte(byte: u8) {
    // SAFETY: COM1 initialized pre-handoff; post-EBS COM1 is plain port I/O;
    // polls a read-only status bit, then writes the transmit register.
    unsafe {
        while inb(COM1_LSR) & LSR_THR_EMPTY == 0 {}
        outb(COM1_PORT, byte);
    }
}

/// Decimal writer for the COM1 interrupt path (no `core::fmt`, no
/// allocation; mirrors `write_exc_line`).
///
/// Safe wrapper: only calls [`com1_write_byte`]. Caller contract is COM1
/// initialized + exception context only.
pub(crate) fn write_dec_u64(mut v: u64) {
    if v == 0 {
        com1_write_byte(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut n = 0;
    while v > 0 {
        buf[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    for i in (0..n).rev() {
        com1_write_byte(buf[i]);
    }
}

/// Hex writer for the COM1 interrupt path (`0x` + lowercase digits).
///
/// Safe wrapper: only calls [`com1_write_byte`]. Caller contract is COM1
/// initialized + exception context only.
pub(crate) fn write_hex_u64(v: u64) {
    for b in b"0x" {
        com1_write_byte(*b);
    }
    let mut started = false;
    let mut i = 15;
    loop {
        let digit = ((v >> (i * 4)) & 0xF) as u8;
        if digit != 0 || started || i == 0 {
            started = true;
            com1_write_byte(if digit < 10 {
                b'0' + digit
            } else {
                b'a' + digit - 10
            });
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
}

/// Emit `EXC ok=<0|1> vector=<n>\n` directly on COM1 (interrupt context
/// cannot use the caller-supplied `kernel_entry` sink).
///
/// Safe wrapper: only calls [`com1_write_byte`]. Requires COM1 initialized
/// (S01 `uart_init` ran before handoff); single-threaded exception context
/// owns the UART exclusively.
pub(crate) fn write_exc_line(ok: bool, vector: u64) {
    for b in b"EXC ok=" {
        com1_write_byte(*b);
    }
    com1_write_byte(if ok { b'1' } else { b'0' });
    for b in b" vector=" {
        com1_write_byte(*b);
    }
    write_dec_u64(vector);
    com1_write_byte(b'\n');
}

/// Emit `PF ok=<0|1> vector=<n> code=<0x...> addr=<0x...>\n` directly on
/// COM1 (interrupt context cannot use the caller-supplied sink).
///
/// Safe wrapper: only calls [`com1_write_byte`] plus the safe decimal/hex
/// helpers. Requires COM1 initialized; single-threaded exception context
/// owns the UART exclusively.
pub(crate) fn write_pf_line(ok: bool, vector: u64, code: u64, cr2: u64) {
    for b in b"PF ok=" {
        com1_write_byte(*b);
    }
    com1_write_byte(if ok { b'1' } else { b'0' });
    for b in b" vector=" {
        com1_write_byte(*b);
    }
    write_dec_u64(vector);
    for b in b" code=" {
        com1_write_byte(*b);
    }
    write_hex_u64(code);
    for b in b" addr=" {
        com1_write_byte(*b);
    }
    write_hex_u64(cr2);
    com1_write_byte(b'\n');
}
