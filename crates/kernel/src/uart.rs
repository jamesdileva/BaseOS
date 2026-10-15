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

/// Initialize COM1 (16550A, 115200 8N1, FIFOs on) so bytes reach QEMU's
/// serial backend even if firmware left the UART in an odd state.
///
/// Safe wrapper (S05 slice 8 UART unify, ex-boot-uefi A3): the `unsafe`
/// (port I/O) is confined to this body. Caller contract is ordering +
/// exclusivity only — call once before any byte on the single-threaded
/// boot path that exclusively owns COM1.
pub fn uart_init() {
    // SAFETY: COM1 UART registers only, always present on the QEMU `pc`
    // target; single-threaded boot path owns the UART pre-handoff.
    unsafe {
        outb(COM1_PORT + 1, 0x00); // Disable interrupts.
        outb(COM1_PORT + 3, 0x80); // Enable DLAB (set baud-rate divisor).
        outb(COM1_PORT, 0x01); // Divisor low byte: 115200 baud.
        outb(COM1_PORT + 1, 0x00); // Divisor high byte.
        outb(COM1_PORT + 3, 0x03); // 8 data bits, no parity, one stop bit.
        outb(COM1_PORT + 2, 0xC7); // Enable FIFOs, clear them, 14-byte threshold.
        outb(COM1_PORT + 4, 0x0B); // DTR + RTS set, IRQs off.
    }
}

/// Poll THR-empty, then write one byte to COM1 (post-EBS path: no firmware,
/// plain port I/O only).
///
/// Safe wrapper: the `unsafe` (port I/O) is confined to this body. Caller
/// contract is ordering + exclusivity only — COM1 was initialized via
/// [`uart_init`] before `kernel_entry`, and the single-threaded
/// boot/exception path owns the UART.
pub fn com1_write_byte(byte: u8) {
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

/// Emit `TICK ok=<0|1> ticks=<n>\n` directly on COM1 (S07 slice14 first-tick
/// proof; slice30 cumulative: `ok` is 1 only when the gated cumulative total
/// `pending + drained_total` is nonzero — the per-tick IRQ drain holds the
/// task-context remainder near 0, so drained-only would misreport ok=0).
///
/// Safe wrapper: only calls [`com1_write_byte`] plus the safe decimal
/// helper. Requires COM1 initialized; single-threaded bring-up context owns
/// the UART exclusively.
pub(crate) fn write_tick_line(ok: bool, ticks: u64) {
    for b in b"TICK ok=" {
        com1_write_byte(*b);
    }
    com1_write_byte(if ok { b'1' } else { b'0' });
    for b in b" ticks=" {
        com1_write_byte(*b);
    }
    write_dec_u64(ticks);
    com1_write_byte(b'\n');
}

/// Emit `IRQSTACK ok=<0|1> task=<0x...> irq=<0x...> base=<0x...> size=<n>\n`
/// directly on COM1 (S07 slice68, #21 nit 3 RSP-switch proof).
///
/// `task` is the task-context `RSP` (bring-up, gated), `irq` the last
/// IRQ0 inner capture (running on the dedicated IRQ stack), `base`/`size`
/// locate that stack. `ok` is computed by the caller: inner nonzero,
/// inside `[base, base+size]`, 16-aligned endpoints, and `irq != task`
/// with `task` outside the range (two-stack proof). Fail-loud by shape:
/// the e2e gate requires the `ok=1` + both hex fields on one line.
///
/// Safe wrapper: only calls [`com1_write_byte`] plus the safe decimal/hex
/// helpers. Requires COM1 initialized; single-threaded bring-up context
/// owns the UART exclusively.
pub(crate) fn write_irqstack_line(ok: bool, task_rsp: u64, irq_rsp: u64, base: u64, size: u64) {
    for b in b"IRQSTACK ok=" {
        com1_write_byte(*b);
    }
    com1_write_byte(if ok { b'1' } else { b'0' });
    for b in b" task=" {
        com1_write_byte(*b);
    }
    write_hex_u64(task_rsp);
    for b in b" irq=" {
        com1_write_byte(*b);
    }
    write_hex_u64(irq_rsp);
    for b in b" base=" {
        com1_write_byte(*b);
    }
    write_hex_u64(base);
    for b in b" size=" {
        com1_write_byte(*b);
    }
    write_dec_u64(size);
    com1_write_byte(b'\n');
}

/// Pure STI line formatter shared by the guest writer and host tests
/// (S08 slice6, REVIEW #610 N3: batch/target line shape was e2e-only — the
/// only shape check was the ps1 `$stiPat`/`$stiVals` regex, so a guest
/// format drift was caught only under QEMU).
///
/// Emits `STI ok=<0|1> ticks=<n> work=<m> spins=<s> batch=<b> target=<t>\n`
/// through a byte callback (`core`-only, no port I/O), so host tests pin
/// the literal field order, names, and decimal shape while
/// [`write_sti_line`] reuses this exact body for its COM1-direct write.
/// `no_std`-clean; the guest passes a `com1_write_byte` closure (infallible).
pub(crate) fn emit_sti_line<E>(
    emit: &mut impl FnMut(u8) -> Result<(), E>,
    ok: bool,
    ticks: u64,
    work: u64,
    spins: u64,
    batch: u64,
    target: u64,
) -> Result<(), E> {
    use baseos_boot_info::{write_str, write_u64_dec};
    write_str(emit, "STI ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " ticks=")?;
    write_u64_dec(emit, ticks)?;
    write_str(emit, " work=")?;
    write_u64_dec(emit, work)?;
    write_str(emit, " spins=")?;
    write_u64_dec(emit, spins)?;
    write_str(emit, " batch=")?;
    write_u64_dec(emit, batch)?;
    write_str(emit, " target=")?;
    write_u64_dec(emit, target)?;
    write_str(emit, "\n")
}

/// Emit `STI ok=<0|1> ticks=<n> work=<m> spins=<s> batch=<b> target=<t>\n`
/// directly on COM1 (S08 slice 2 preemption probe: `ok` is 1 only when the
/// joint [`crate::timer::sti_window_ready`] gate clears — cumulative ticks
/// AND preempted-workload progress — so a tick-only `hlt` spin or a
/// work-only spin reports `ok=0` fail-closed; S08 slice 3 trace capture:
/// `spins` is the used `hlt` wait-loop count, so a scheduler-parameter
/// change produces a comparable before/after wait cost on the same line;
/// S08 slice 4 knob report (REVIEW #605 note 1): `batch`/`target` ride on
/// the same line because `spins` is coupled to work when work-bound
/// (`work = (spins + 1) * BATCH` at a fixed batch) — without them a
/// tick-bound vs work-bound change misreads).
///
/// Safe wrapper: only calls [`com1_write_byte`] via [`emit_sti_line`].
/// Requires COM1 initialized; single-threaded bring-up context owns
/// the UART exclusively.
pub(crate) fn write_sti_line(ok: bool, ticks: u64, work: u64, spins: u64, batch: u64, target: u64) {
    let mut out = |b: u8| {
        com1_write_byte(b);
        Ok::<(), core::convert::Infallible>(())
    };
    let _ = emit_sti_line(&mut out, ok, ticks, work, spins, batch, target);
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

/// Pure PFUS line formatter shared by the guest writer and host tests
/// (S09 slice2b, REVIEW #622 risk 3: the USER line is classifier-only, so the
/// live `#PF` U/S path needs its own e2e-pinned shape).
///
/// Emits `PFUS ok=<0|1> us=<0|1> w=<0|1> p=<0|1>\n` through a byte callback
/// (`core`-only, no port I/O), so host tests pin the literal field order,
/// names, and decimal shape while [`write_pfus_line`] reuses this exact body
/// for its COM1-direct write. `us`/`w`/`p` are the decoded `#PF` error-code
/// U/S (bit 2), W/R (bit 1), P (bit 0) bits; `ok` is the caller-computed
/// [`crate::pf_us_verdict`] fail-closed verdict. `no_std`-clean; the guest
/// passes a `com1_write_byte` closure (infallible).
pub(crate) fn emit_pfus_line<E>(
    emit: &mut impl FnMut(u8) -> Result<(), E>,
    ok: bool,
    us: bool,
    w: bool,
    p: bool,
) -> Result<(), E> {
    use baseos_boot_info::{write_str, write_u64_dec};
    write_str(emit, "PFUS ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " us=")?;
    write_u64_dec(emit, u64::from(us))?;
    write_str(emit, " w=")?;
    write_u64_dec(emit, u64::from(w))?;
    write_str(emit, " p=")?;
    write_u64_dec(emit, u64::from(p))?;
    write_str(emit, "\n")
}

/// Emit `PFUS ok=<0|1> us=<0|1> w=<0|1> p=<0|1>\n` directly on COM1
/// (S09 slice2b live-`#PF` U/S path: `ok` is 1 only when the live handler
/// decoded the supervisor-read-not-present shape `us=0 w=0 p=0` at the
/// expected probe address — any U/S=1, write, or present fault prints `ok=0`
/// fail-closed; CPL3 `U=1` entry stays deferred, see [`crate::pf_us_verdict`]).
///
/// Safe wrapper: only calls [`com1_write_byte`] via [`emit_pfus_line`].
/// Requires COM1 initialized; single-threaded exception context owns
/// the UART exclusively.
pub(crate) fn write_pfus_line(ok: bool, us: bool, w: bool, p: bool) {
    let mut out = |b: u8| {
        com1_write_byte(b);
        Ok::<(), core::convert::Infallible>(())
    };
    let _ = emit_pfus_line(&mut out, ok, us, w, p);
}

/// Pure PFCPL line formatter shared by the future guest CPL3 handler and
/// host tests (S09 slice2cf, #30 remainder, REVIEW #799: the `PFUS` line
/// pins the supervisor `U/S=0` half — the live CPL3 `U=1` half needs its
/// own e2e-pinned shape).
///
/// Emits `PFCPL ok=<0|1> us=<0|1> cpl=<n>\n` through a byte callback
/// (`core`-only, no port I/O), so host tests pin the literal field order,
/// names, and decimal shape while [`write_pfcpl_line`] reuses this exact
/// body for its COM1-direct write. `us` is the decoded `#PF` error-code
/// U/S bit; `cpl` is the faulting `CS & 3`; `ok` is the caller-computed
/// [`crate::pf_cpl_verdict`] fail-closed verdict. `no_std`-clean; the guest
/// passes a `com1_write_byte` closure (infallible).
pub(crate) fn emit_pfcpl_line<E>(
    emit: &mut impl FnMut(u8) -> Result<(), E>,
    ok: bool,
    us: bool,
    cpl: u64,
) -> Result<(), E> {
    use baseos_boot_info::{write_str, write_u64_dec};
    write_str(emit, "PFCPL ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " us=")?;
    write_u64_dec(emit, u64::from(us))?;
    write_str(emit, " cpl=")?;
    write_u64_dec(emit, cpl)?;
    write_str(emit, "\n")
}

/// Emit `PFCPL ok=<0|1> us=<0|1> cpl=<n>\n` directly on COM1
/// (S09 slice2cf live-`#PF` CPL3 path: `ok` is 1 only when the live handler
/// decoded vector 14 + `U/S=1` at `CPL=3` — any supervisor fault, wrong
/// vector, or CPL0 delivery prints `ok=0` fail-closed).
///
/// Wired by the slice2cg guest `#PF` stub CS capture + `iretq` entry.
///
/// Safe wrapper: only calls [`com1_write_byte`] via [`emit_pfcpl_line`].
/// Requires COM1 initialized; single-threaded exception context owns
/// the UART exclusively.
pub(crate) fn write_pfcpl_line(ok: bool, us: bool, cpl: u64) {
    let mut out = |b: u8| {
        com1_write_byte(b);
        Ok::<(), core::convert::Infallible>(())
    };
    let _ = emit_pfcpl_line(&mut out, ok, us, cpl);
}

/// Pure TR line formatter shared by the guest writer and host tests
/// (S09 slice2k, #30 remainder: `ltr` survival alone never proved `TR`
/// holds `TSS_SEL` — boot-identical bytes only).
///
/// Emits `TR ok=<0|1> sel=<0x...>\n` through a byte callback
/// (`core`-only, no port I/O), so host tests pin the literal field order,
/// names, and hex shape while [`write_tr_line`] reuses this exact body
/// for its COM1-direct write. `ok` is the caller-computed
/// `read_tr() == TSS_SEL` verdict. `no_std`-clean; the guest passes a
/// `com1_write_byte` closure (infallible).
pub(crate) fn emit_tr_line<E>(
    emit: &mut impl FnMut(u8) -> Result<(), E>,
    ok: bool,
    sel: u16,
) -> Result<(), E> {
    use baseos_boot_info::{write_str, write_u64_dec, write_u64_hex};
    write_str(emit, "TR ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " sel=")?;
    write_u64_hex(emit, u64::from(sel))?;
    write_str(emit, "\n")
}

/// Emit `TR ok=<0|1> sel=<0x...>\n` directly on COM1
/// (S09 slice2k TR-liveness proof: `ok` is 1 only when the live `str`
/// readback equals `TSS_SEL` right after `ltr` — a skipped/faulted `ltr`
/// prints `ok=0` fail-closed).
///
/// Safe wrapper: only calls [`com1_write_byte`] via [`emit_tr_line`].
/// Requires COM1 initialized; single-threaded bring-up context owns
/// the UART exclusively.
pub(crate) fn write_tr_line(ok: bool, sel: u16) {
    let mut out = |b: u8| {
        com1_write_byte(b);
        Ok::<(), core::convert::Infallible>(())
    };
    let _ = emit_tr_line(&mut out, ok, sel);
}

/// Pure USTACK line formatter shared by the bound report body and host tests
/// (S09 slice2at, #30 remainder, REVIEW #709 R2: the `ok`+`label` strings
/// were host-pinned but no serial body bound them, so labels could drift).
///
/// Emits `USTACK ok=<0|1> label=<valid|perms-refused|walk-failed>\n`
/// through a byte callback (`core`-only, no port I/O), so host tests pin
/// the literal field order, names, and label spelling while
/// [`emit_ustack_report`] reuses this exact body for the bound report.
/// `ok` is [`crate::cpu::user_stack_walk_ok`] and `label` is
/// [`crate::cpu::user_stack_walk_label`] — guest code must go through
/// [`emit_ustack_report`] / [`write_ustack_report`] so the pair cannot
/// drift apart (REVIEW #711: no raw `ok`+`label` COM1 writer exists).
/// `no_std`-clean; the guest passes a `com1_write_byte` closure (infallible).
pub(crate) fn emit_ustack_line<E>(
    emit: &mut impl FnMut(u8) -> Result<(), E>,
    ok: bool,
    label: &str,
) -> Result<(), E> {
    use baseos_boot_info::{write_str, write_u64_dec};
    write_str(emit, "USTACK ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " label=")?;
    write_str(emit, label)?;
    write_str(emit, "\n")
}

/// Bound USTACK report emitter (S09 slice2at): the single call site the
/// future guest `USTACK` line gates on — sources `ok` + `label` from the
/// tri-state `outcome` via [`crate::cpu::user_stack_walk_ok`] /
/// [`crate::cpu::user_stack_walk_label`] in one call so a hand-built
/// `ok`/`label` pair cannot mismatch the report (e.g. `ok=1` with
/// `label=walk-failed`). Pure `core`-only body over [`emit_ustack_line`].
/// `pub` since slice2av: the boot-uefi guest formats through this body.
pub fn emit_ustack_report<E>(
    emit: &mut impl FnMut(u8) -> Result<(), E>,
    outcome: crate::cpu::UserStackWalkOutcome,
) -> Result<(), E> {
    emit_ustack_line(
        emit,
        crate::cpu::user_stack_walk_ok(outcome),
        crate::cpu::user_stack_walk_label(outcome),
    )
}

/// Bound COM1 USTACK report writer (S09 slice2au): [`emit_ustack_report`]
/// over COM1 — the SOLE guest COM1 call site for the `USTACK` line, so a
/// hand-built `ok`+`label` mismatch (e.g. `ok=1` + `walk-failed`) cannot
/// reach serial (REVIEW #711: raw `write_ustack_line(ok, label)` deleted).
/// `pub` since slice2av: the boot-uefi guest calls this live post-EBS.
pub fn write_ustack_report(outcome: crate::cpu::UserStackWalkOutcome) {
    let mut out = |b: u8| {
        com1_write_byte(b);
        Ok::<(), core::convert::Infallible>(())
    };
    let _ = emit_ustack_report(&mut out, outcome);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_tr(ok: bool, sel: u16) -> String {
        let mut out = Vec::new();
        emit_tr_line::<core::convert::Infallible>(
            &mut |b| {
                out.push(b);
                Ok(())
            },
            ok,
            sel,
        )
        .unwrap();
        String::from_utf8(out).unwrap()
    }

    fn run_pfus(ok: bool, us: bool, w: bool, p: bool) -> String {
        let mut out = Vec::new();
        emit_pfus_line::<core::convert::Infallible>(
            &mut |b| {
                out.push(b);
                Ok(())
            },
            ok,
            us,
            w,
            p,
        )
        .unwrap();
        String::from_utf8(out).unwrap()
    }

    fn run_pfcpl(ok: bool, us: bool, cpl: u64) -> String {
        let mut out = Vec::new();
        emit_pfcpl_line::<core::convert::Infallible>(
            &mut |b| {
                out.push(b);
                Ok(())
            },
            ok,
            us,
            cpl,
        )
        .unwrap();
        String::from_utf8(out).unwrap()
    }

    fn run_sti(ok: bool, ticks: u64, work: u64, spins: u64, batch: u64, target: u64) -> String {
        let mut out = Vec::new();
        emit_sti_line::<core::convert::Infallible>(
            &mut |b| {
                out.push(b);
                Ok(())
            },
            ok,
            ticks,
            work,
            spins,
            batch,
            target,
        )
        .unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn sti_line_pins_exact_field_order_names_and_decimals() {
        // REVIEW #610 N3: host backstop for the e2e-only batch/target shape.
        // The guest writer shares this body, so a format drift fails here.
        assert_eq!(
            run_sti(true, 5, 20_000, 9, 2_000, 20_000),
            "STI ok=1 ticks=5 work=20000 spins=9 batch=2000 target=20000\n"
        );
    }

    #[test]
    fn sti_line_fail_closed_shape_and_zero_values() {
        assert_eq!(
            run_sti(false, 0, 0, 0, 2_000, 20_000),
            "STI ok=0 ticks=0 work=0 spins=0 batch=2000 target=20000\n"
        );
        // Decimal, never hex/padded: values above 9 prove base-10 shape.
        let line = run_sti(true, 12, 34, 56, 78, 90);
        assert_eq!(
            line,
            "STI ok=1 ticks=12 work=34 spins=56 batch=78 target=90\n"
        );
        // Field order is load-bearing for the ps1 regex groups.
        let (ticks_i, work_i, spins_i, batch_i, target_i) = (
            line.find(" ticks=").unwrap(),
            line.find(" work=").unwrap(),
            line.find(" spins=").unwrap(),
            line.find(" batch=").unwrap(),
            line.find(" target=").unwrap(),
        );
        assert!(ticks_i < work_i && work_i < spins_i && spins_i < batch_i && batch_i < target_i);
        assert!(line.starts_with("STI ok="));
        assert!(line.ends_with('\n'));
    }

    #[test]
    fn pfus_line_pins_exact_field_order_and_fail_closed_shape() {
        // S09 slice2b: host backstop for the live-#PF U/S e2e shape. The
        // guest PF handler shares this body, so a format drift fails here.
        assert_eq!(
            run_pfus(true, false, false, false),
            "PFUS ok=1 us=0 w=0 p=0\n"
        );
        assert_eq!(
            run_pfus(false, true, false, false),
            "PFUS ok=0 us=1 w=0 p=0\n"
        );
        assert_eq!(
            run_pfus(false, false, true, true),
            "PFUS ok=0 us=0 w=1 p=1\n"
        );
        let line = run_pfus(true, false, false, false);
        let (us_i, w_i, p_i) = (
            line.find(" us=").unwrap(),
            line.find(" w=").unwrap(),
            line.find(" p=").unwrap(),
        );
        assert!(us_i < w_i && w_i < p_i);
        assert!(line.starts_with("PFUS ok="));
        assert!(line.ends_with('\n'));
    }

    #[test]
    fn pfcpl_line_pins_exact_field_order_and_fail_closed_shape() {
        // S09 slice2cf: host backstop for the live-CPL3 `#PF` e2e shape.
        // The future guest CPL3 handler shares this body, so a format
        // drift fails here.
        assert_eq!(run_pfcpl(true, true, 3), "PFCPL ok=1 us=1 cpl=3\n");
        assert_eq!(run_pfcpl(false, false, 0), "PFCPL ok=0 us=0 cpl=0\n");
        assert_eq!(run_pfcpl(false, true, 0), "PFCPL ok=0 us=1 cpl=0\n");
        let line = run_pfcpl(true, true, 3);
        let (us_i, cpl_i) = (line.find(" us=").unwrap(), line.find(" cpl=").unwrap());
        assert!(us_i < cpl_i);
        assert!(line.starts_with("PFCPL ok="));
        assert!(line.ends_with('\n'));
    }

    #[test]
    fn tr_line_pins_exact_field_order_and_fail_closed_shape() {
        // S09 slice2k: host backstop for the TR-liveness e2e shape. The
        // guest bring-up shares this body, so a format drift fails here.
        assert_eq!(run_tr(true, 0x28), "TR ok=1 sel=0x28\n");
        assert_eq!(run_tr(false, 0x0), "TR ok=0 sel=0x0\n");
        let line = run_tr(true, 0x28);
        assert!(line.starts_with("TR ok="));
        assert!(line.contains(" sel=0x"));
        assert!(line.ends_with('\n'));
    }

    fn run_ustack(ok: bool, label: &str) -> String {
        let mut out = Vec::new();
        emit_ustack_line::<core::convert::Infallible>(
            &mut |b| {
                out.push(b);
                Ok(())
            },
            ok,
            label,
        )
        .unwrap();
        String::from_utf8(out).unwrap()
    }

    fn run_ustack_report(outcome: crate::cpu::UserStackWalkOutcome) -> String {
        let mut out = Vec::new();
        emit_ustack_report::<core::convert::Infallible>(
            &mut |b| {
                out.push(b);
                Ok(())
            },
            outcome,
        )
        .unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn ustack_line_pins_ok_label_shape_and_report_binding() {
        // S09 slice2at (REVIEW #709 R1/R2): the `ok`+`label` pair finally has
        // a serial consumer, and the bound report is the call site the future
        // guest line gates on — a hand-built mismatch cannot reach serial.
        // REVIEW #713 (2): the binding assert below is agreement-only
        // (report vs hand-built share emit_ustack_line, so tautological) —
        // pin every outcome's EXACT literal directly so a label/ok drift
        // fails here even though both sides share the body.
        assert_eq!(
            run_ustack_report(crate::cpu::UserStackWalkOutcome::Valid),
            "USTACK ok=1 label=valid\n"
        );
        assert_eq!(
            run_ustack_report(crate::cpu::UserStackWalkOutcome::PermsRefused),
            "USTACK ok=0 label=perms-refused\n"
        );
        assert_eq!(
            run_ustack_report(crate::cpu::UserStackWalkOutcome::WalkFailed),
            "USTACK ok=0 label=walk-failed\n"
        );
        assert_eq!(run_ustack(true, "valid"), "USTACK ok=1 label=valid\n");
        assert_eq!(
            run_ustack(false, "perms-refused"),
            "USTACK ok=0 label=perms-refused\n"
        );
        assert_eq!(
            run_ustack(false, "walk-failed"),
            "USTACK ok=0 label=walk-failed\n"
        );
        // Bound report agrees with the hand-built lines on every outcome and
        // sources both fields from the shared cpu helpers (no literals here).
        for outcome in [
            crate::cpu::UserStackWalkOutcome::Valid,
            crate::cpu::UserStackWalkOutcome::PermsRefused,
            crate::cpu::UserStackWalkOutcome::WalkFailed,
        ] {
            assert_eq!(
                run_ustack_report(outcome),
                run_ustack(
                    crate::cpu::user_stack_walk_ok(outcome),
                    crate::cpu::user_stack_walk_label(outcome)
                )
            );
        }
        let line = run_ustack_report(crate::cpu::UserStackWalkOutcome::PermsRefused);
        assert!(line.starts_with("USTACK ok="));
        assert!(line.contains(" label="));
        assert!(line.ends_with('\n'));
    }
}
