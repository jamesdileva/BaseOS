//! S07 slice 7a — real-timer IRQ contract (host-testable, no guest wiring yet).
//!
//! Pure `core`-only constants + counter the future PIT/PIC/IDT32 slice will
//! consume. S07 slice14 adds the guest-only program path (`remap_pic` +
//! `program_pit` consuming the const plans via `outb`) plus the volatile
//! snapshot / gated drain the bring-up uses around `sti`; the IRQ0 stub +
//! EOI + `iretq` and the IDT-32 gate landed in earlier slices.
//!
//! Contracts pinned here:
//! - `PIT_BASE_HZ = 1_193_182`, [`pit_divisor`] (integer division, fails
//!   closed to `0` on `hz == 0`, quotient `0`, or quotient `> 0xFFFF`),
//!   [`TIMER_HZ = 100`] + [`TIMER_DIVISOR`].
//! - PIC remap bases `PICM_OFFSET = 0x20` / `PICS_OFFSET = 0x28`, so IRQ0
//!   lands on [`VECTOR_TIMER = 32`] (first maskable vector past the 32
//!   exception vectors).
//! - [`TimerTicks`] saturating HW-tick counter the future IRQ0 handler
//!   increments (one `tick()` per IRQ0). Wraps nothing, saturates at
//!   `u64::MAX`; the scheduler slice will drain it via `take()` and feed
//!   `tick_gated` once per HW tick (dispatch-once on expiry, never a tight
//!   re-tick loop — see `sched.rs` residual 2).
//!
//! Failure model: all pure, never panics, `core`-only.

use crate::cpu::IrqGuard;

/// PIT input clock (Intel 8254, 1.193182 MHz).
pub const PIT_BASE_HZ: u32 = 1_193_182;

/// Target timer IRQ rate (100 Hz → 10 ms period).
pub const TIMER_HZ: u32 = 100;

/// PIC master remap base (IRQ0 lands on VECTOR_TIMER).
pub const PICM_OFFSET: u8 = 0x20;
/// PIC slave remap base.
pub const PICS_OFFSET: u8 = 0x28;

/// IDT vector for PIT IRQ0 (first maskable vector past exceptions 0..=31).
pub const VECTOR_TIMER: usize = 32;

/// PIT channel-0 data port.
pub const PIT_CH0: u16 = 0x40;
/// PIT mode/command port.
pub const PIT_CMD: u16 = 0x43;

/// PIT mode byte for channel 0, lobyte/hibyte, mode 3 square-wave.
pub const PIT_MODE_CH0_SQUARE: u8 = 0x36;

/// PIC master command port (EOI target for IRQ0; IRQ0 is master-only).
pub const PICM_CMD: u16 = 0x20;
/// PIC master data port (remap/programming path, wiring slice).
pub const PICM_DATA: u16 = 0x21;
/// PIC slave command port (remap-programming path only; IRQ0 needs no slave EOI).
pub const PICS_CMD: u16 = 0xA0;
/// PIC slave data port (remap-programming path only).
pub const PICS_DATA: u16 = 0xA1;
/// PIC end-of-interrupt byte (master; IRQ0 handler issues exactly one).
pub const PIC_EOI: u8 = 0x20;

/// PIC ICW1: edge-triggered, cascade mode, ICW4 needed (`0x11`).
pub const PIC_ICW1_INIT_ICW4: u8 = 0x11;
/// PIC ICW4: 8086 mode (`0x01`).
pub const PIC_ICW4_8086: u8 = 0x01;
/// PIC ICW3 master: slave present at IRQ2 (`0x04`).
pub const PIC_ICW3_MASTER_SLAVE_IRQ2: u8 = 0x04;
/// PIC ICW3 slave: cascade identity (`0x02`).
pub const PIC_ICW3_SLAVE_IDENTITY: u8 = 0x02;
/// PIC master mask with IRQ0 unmasked only (`0xFE`).
pub const PICM_MASK_IRQ0_ONLY: u8 = 0xFE;
/// PIC slave mask with all lines masked (`0xFF`; IRQ0 is master-only).
pub const PICS_MASK_ALL: u8 = 0xFF;

/// Integer PIT divisor for `hz` (`PIT_BASE_HZ / hz`); fails closed to `0`
/// when the quotient is not programmable: `hz == 0`, quotient `0`
/// (`hz > PIT_BASE_HZ`), or quotient `> 0xFFFF` (e.g. `hz == 1` gives
/// 1193182, which must NOT silently truncate to 13534 via `as u16`).
/// Caller must refuse a zero divisor — programming it latches
/// `0x0000` = 65536, not "stop".
#[must_use]
pub const fn pit_divisor(hz: u32) -> u16 {
    if hz == 0 {
        return 0;
    }
    let q = PIT_BASE_HZ / hz;
    if q == 0 || q > 0xFFFF {
        return 0;
    }
    q as u16
}

/// Divisor for [`TIMER_HZ`] (1193182 / 100 = 11931).
pub const TIMER_DIVISOR: u16 = pit_divisor(TIMER_HZ);

/// Programmed `(lobyte, hibyte)` pair for [`TIMER_DIVISOR`] (S07 slice11).
///
/// First in-tree caller of [`pit_program_bytes`]: closes the REVIEW #444
/// residual (zero callers — nothing enforced the zero-divisor refusal).
/// `TIMER_DIVISOR` is non-zero by construction (`11931`), so this is
/// `Some((0x9B, 0x2E))`; the guest PIT-program path consumes this const
/// (never a raw divisor) so a `0` can never reach the port (`0x0000`
/// latches 65536, not "stop"). Pure `core`-only; the actual `outb`
/// sequence rides with the guest wiring slice (still `cli` until the IRQ0
/// stub + EOI + `iretq` land first).
pub const TIMER_PROGRAM: Option<(u8, u8)> = pit_program_bytes(TIMER_DIVISOR);

/// PIC remap `outb` sequence (S07 slice11, pure `core`-only plan).
///
/// Returns the `(port, value)` pairs the guest wiring slice emits in order
/// to remap the 8259 pair so IRQ0 lands on [`VECTOR_TIMER`] (32):
/// ICW1 ([`PIC_ICW1_INIT_ICW4`]) to both command ports, ICW2 (offsets
/// [`PICM_OFFSET`] / [`PICS_OFFSET`]), ICW3 (master
/// [`PIC_ICW3_MASTER_SLAVE_IRQ2`] = slave at IRQ2, slave
/// [`PIC_ICW3_SLAVE_IDENTITY`] = cascade identity), ICW4
/// ([`PIC_ICW4_8086`] = 8086 mode). Masks ride separately
/// ([`pic_mask_writes`]: master [`PICM_MASK_IRQ0_ONLY`], slave
/// [`PICS_MASK_ALL`]).
/// String/const-only this slice — no port I/O, guest keeps `cli`, so QEMU
/// behavior is unchanged. Host tests pin the order so the wiring slice
/// cannot silently swap an offset or drop ICW3/ICW4 (which would wedge
/// IRQ0 after the first tick).
#[must_use]
pub const fn pic_remap_writes() -> [(u16, u8); 8] {
    [
        (PICM_CMD, PIC_ICW1_INIT_ICW4),
        (PICS_CMD, PIC_ICW1_INIT_ICW4),
        (PICM_DATA, PICM_OFFSET),
        (PICS_DATA, PICS_OFFSET),
        (PICM_DATA, PIC_ICW3_MASTER_SLAVE_IRQ2),
        (PICS_DATA, PIC_ICW3_SLAVE_IDENTITY),
        (PICM_DATA, PIC_ICW4_8086),
        (PICS_DATA, PIC_ICW4_8086),
    ]
}

/// PIC mask `outb` sequence (S07 slice12, pure `core`-only plan).
///
/// Returns the `(port, value)` pairs the guest wiring slice emits AFTER
/// [`pic_remap_writes`] to unmask IRQ0 only: master
/// [`PICM_MASK_IRQ0_ONLY`] (`0xFE`), slave [`PICS_MASK_ALL`] (`0xFF`).
/// IRQ0 is master-only so the slave stays fully masked. String/const-only
/// this slice — no port I/O, guest keeps `cli`.
#[must_use]
pub const fn pic_mask_writes() -> [(u16, u8); 2] {
    [(PICM_DATA, PICM_MASK_IRQ0_ONLY), (PICS_DATA, PICS_MASK_ALL)]
}

/// PIT program `outb` sequence (S07 slice12, pure `core`-only plan).
///
/// Returns the `(port, value)` triples the guest wiring slice emits in
/// order to arm channel 0 at [`TIMER_HZ`]: mode byte
/// [`PIT_MODE_CH0_SQUARE`] to [`PIT_CMD`], then the [`TIMER_PROGRAM`]
/// lobyte/hibyte pair to [`PIT_CH0`]. Consumes [`TIMER_PROGRAM`] (never
/// a raw divisor) so a `0` can never reach the port (`0x0000` latches
/// 65536, not "stop"): returns `None` when [`TIMER_PROGRAM`] is `None`.
/// String/const-only this slice — no port I/O, guest keeps `cli`.
#[must_use]
pub const fn pit_program_writes() -> Option<[(u16, u8); 3]> {
    match TIMER_PROGRAM {
        None => None,
        Some((lo, hi)) => Some([(PIT_CMD, PIT_MODE_CH0_SQUARE), (PIT_CH0, lo), (PIT_CH0, hi)]),
    }
}

/// Split a programmable divisor into the `(lobyte, hibyte)` pair the wiring
/// slice writes to [`PIT_CH0`] after [`PIT_MODE_CH0_SQUARE`] goes to
/// [`PIT_CMD`]. Fails closed to `None` on `0` — the caller MUST refuse it:
///
/// programming `0x0000` latches 65536, not "stop" (same rule as
/// [`pit_divisor`]'s fail-closed `0`). Pure `core`-only; the actual `outb`
/// sequence rides with the guest wiring slice (still `cli` until the IRQ0
/// stub + EOI + `iretq` land first).
#[must_use]
pub const fn pit_program_bytes(divisor: u16) -> Option<(u8, u8)> {
    if divisor == 0 {
        return None;
    }
    Some((divisor as u8, (divisor >> 8) as u8))
}

/// Guest-only port write for the PIC/PIT program path (S07 slice14).
///
/// Private to this module: safe callers use [`remap_pic`] / [`program_pit`]
/// without naming a port. COM1 lives in `uart`; this body covers only the
/// timer-programming ports consumed from the const plans above.
#[cfg(not(test))]
#[inline(always)]
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags),
    );
}

/// Guest-only PIC remap + mask (S07 slice14).
///
/// Emits [`pic_remap_writes`] then [`pic_mask_writes`] in order via `outb`,
/// so IRQ0 lands on [`VECTOR_TIMER`] with IRQ0 unmasked only. Consumes the
/// const plans (never raw literals) so a swapped offset or dropped ICW fails
/// at the host plan tests, not silently in QEMU.
///
/// # Safety
/// Post-EBS guest only, `cli` in effect, single-threaded boot owns the PIC.
#[cfg(not(test))]
pub(crate) unsafe fn remap_pic() {
    let remap = pic_remap_writes();
    let mut i = 0;
    while i < remap.len() {
        let (port, value) = remap[i];
        outb(port, value);
        i += 1;
    }
    let masks = pic_mask_writes();
    let mut j = 0;
    while j < masks.len() {
        let (port, value) = masks[j];
        outb(port, value);
        j += 1;
    }
}

/// Guest-only PIT arm at [`TIMER_HZ`] (S07 slice14).
///
/// Emits [`pit_program_writes`] in order via `outb` (mode byte to
/// [`PIT_CMD`], then lobyte/hibyte to [`PIT_CH0`]). Consumes
/// [`TIMER_PROGRAM`] (never a raw divisor) so a `0` can never reach the
/// port. No-op when [`TIMER_PROGRAM`] is `None` (fail-closed: the caller
/// must refuse to `sti` — `bring_up` checks the tick, not this helper).
///
/// # Safety
/// Post-EBS guest only, `cli` in effect, single-threaded boot owns the PIT.
#[cfg(not(test))]
pub(crate) unsafe fn program_pit() {
    let Some(seq) = pit_program_writes() else {
        return;
    };
    let mut i = 0;
    while i < seq.len() {
        let (port, value) = seq[i];
        outb(port, value);
        i += 1;
    }
}

/// Guest-only volatile tick snapshot (S07 slice14).
///
/// Volatile whole-struct read of `TIMER_TICKS` so the `sti` spin-wait
/// observes IRQ-side increments without a data race on the host model
/// (guest runs with IF set here; the IRQ body is the only writer).
/// Atomicity note (REVIEW #458 residue 2): single-core boot, `u64` is
/// 8-byte aligned so each x86-64 load/store is single-copy — the IRQ-side
/// `tick_gated` write cannot tear this read; the drain itself runs gated
/// under `cli` + `IrqGuard` anyway.
///
/// # Safety
/// Guest IRQ-enabled window only; single-threaded boot owns the read.
#[cfg(not(test))]
pub(crate) unsafe fn pending_snapshot() -> u64 {
    core::ptr::addr_of!(TIMER_TICKS).read_volatile().pending()
}

/// Guest-only combined PIT-handler step (S07 slice 17; slice 18 delegates
/// to the helper instead of re-implementing it; slice 19 is static-only).
///
/// Drains the IRQ-owned static `TIMER_TICKS` into the IRQ-owned static
/// scheduler via [`crate::sched::sched_poll_static`] (bulk credit to the
/// pre-dispatch current, then at most one gated dispatch on expiry) and
/// returns `(drained, dispatch)` — `dispatch` is `None` when no expiry
/// (nothing to do) and `Some(Ok/Err)` when one resched was attempted.
/// Single call site (`bring_up_cpu_and_prove_exception`) so the guest can
/// never dispatch without draining first (REVIEW #462 risk 1), and the
/// helper owns the order so this wrapper cannot drift from it (REVIEW #464
/// nit 1). The `TICK` line still reports `drained` only; the dispatch result
/// is intentionally ignored (demo table always has a `Ready` target).
///
/// Retry-spin note (REVIEW #464 nit 2): a failed resched (`Some(Err)`)
/// keeps the consumed quantum (`ticks_left == 0`), so every subsequent
/// non-empty drain retries exactly one dispatch; empty drains stay `None`.
///
/// Static note (S07 slice 19): both ends are IRQ-owned statics now — the
/// counter (`TIMER_TICKS`) and the table (`sched::STATIC_SCHED`, armed once
/// via `sched_init_demo_static`). No stack-local scheduler remains, so the
/// future live periodic IRQ handler can call this same path from IRQ
/// context (still task-context polled this slice; the handler move rides
/// with the S08 live periodic slice together with its failure backoff).
///
/// # Safety
/// Post-EBS guest only, `cli` in effect, `gate` proves the critical
/// section; single-threaded boot owns `TIMER_TICKS` and the static sched.
#[cfg(not(test))]
pub(crate) unsafe fn poll_timer_dispatch(
    gate: &IrqGuard,
) -> (u64, Option<Result<u64, crate::thread::SwitchError>>) {
    crate::sched::sched_poll_static(&mut (*core::ptr::addr_of_mut!(TIMER_TICKS)), gate)
}

/// Shared IRQ0 ISR stub source (AT&T). Same Win64-ABI contract as the
/// `#DE`/`#PF` stubs in the parent (`RCX` first arg, 32-byte shadow space,
/// `RSP` 16-byte aligned before `call`): the handler body is a future
/// `timer_irq_rust` (tick + single master EOI + `iretq`, never a park).
/// String-only this slice — no `global_asm!` wiring, no IDT extend, guest
/// keeps `cli`, so QEMU behavior is unchanged. Host tests pin the shape so
/// the wiring slice cannot silently drop the EOI or the return.
///
/// REVIEW #446 R1 (returning-IRQ frame): unlike `#DE`/`#PF` (never return),
/// IRQ0 MUST return via `iretq`, so the stub MUST NOT use the destructive
/// `and rsp, -16` (loses the entry RSP, `iretq` pops the wrong frame ->
/// triple fault) and MUST preserve caller state. Entry pushes the 7
/// Win64-volatile regs (`rax rcx rdx r8-r11`), then `sub rsp, 32` only
/// (no `and`), so `add rsp, 32` + pops restore the exact entry frame.
/// EOI runs BEFORE the pops (while the saved `rax` still sits on the
/// stack), so `mov al, 0x20` cannot clobber the caller's `rax`.
//
// NOTE: `unused_macros` until the wiring slice consumes this via
// `global_asm!` (same pattern as the `#DE`/`#PF` stubs, which are consumed
// in the parent). Host tests reach the text through `timer_stub_text`.
#[allow(unused_macros)]
macro_rules! timer_isr_stub_asm {
    () => {
        r#"
    .global baseos_isr_timer
    baseos_isr_timer:
        push rax
        push rcx
        push rdx
        push r8
        push r9
        push r10
        push r11
        sub rsp, 32
        call timer_irq_rust
        add rsp, 32
        mov al, 0x20
        out 0x20, al
        pop r11
        pop r10
        pop r9
        pop r8
        pop rdx
        pop rcx
        pop rax
        iretq
    "#
    };
}

/// Re-export the stub text for host tests (`macro_rules!` is textual scope;
/// this `const fn` gives tests a plain `&str` without exporting the macro).
#[cfg(test)]
pub(crate) const fn timer_stub_text() -> &'static str {
    timer_isr_stub_asm!()
}

// S07 slice11 — consume the stub in the guest binary (closes the
// string-only residual: `timer_isr_stub_asm!` now has one `global_asm!`
// caller, same pattern as the `#DE`/`#PF` stubs in the parent). Linked
// but UNWIRED this slice: `cpu::load_idt` still covers the 32 exception
// vectors only, guest keeps `cli`, so QEMU behavior is unchanged. The
// wiring slice adds IDT vector 32 + PIC remap + PIT program + `sti`.
#[cfg(not(test))]
core::arch::global_asm!(timer_isr_stub_asm!());

/// Guest IRQ0 tick counter (S07 slice11).
///
/// `static mut`, guest-only: the IRQ0 stub runs with IF cleared by the
/// interrupt gate (plus the post-EBS `cli` until the wiring slice adds
/// `sti`), single-threaded boot owns it. The Rust body
/// [`timer_irq_rust`] records one saturating tick per IRQ0; the scheduler
/// drain (gated `take_gated` under `&IrqGuard`) rides with the wiring
/// slice. Host `cargo test` never touches this (pure `TimerTicks` unit
/// tests cover the counting rule).
#[cfg(not(test))]
static mut TIMER_TICKS: TimerTicks = TimerTicks::new();

/// Rust body of the IRQ0 stub (S07 slice11, guest-only).
///
/// Called from `baseos_isr_timer` with the Win64-volatile frame saved and
/// 32-byte shadow space reserved; the stub performs the single master EOI
/// AFTER this returns (while the saved `rax` still sits on the stack) and
/// `iretq`s. Body records exactly one saturating tick and returns — never
/// parks, never touches ports (EOI lives in the stub so `mov al` cannot
/// clobber the caller's `rax` before the pops).
///
/// S07 slice12: IRQ side uses the gated form (`tick_gated` under
/// `IrqGuard::enter()` — the interrupt gate clears IF, so the `cli`
/// proof holds on entry). Raw `tick()` stays for host parity tests only;
/// the scheduler drain must use `take_gated` under the same guard.
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn timer_irq_rust() {
    // SAFETY: guest IRQ context only (interrupt gate, IF cleared);
    // single-threaded boot owns `TIMER_TICKS` until the wiring slice adds
    // the gated drain. Host builds never link this symbol.
    unsafe {
        let gate = IrqGuard::enter();
        (*core::ptr::addr_of_mut!(TIMER_TICKS)).tick_gated(&gate);
    }
}

/// Saturating HW-tick counter: the future IRQ0 handler calls [`tick`]
/// once per interrupt; the scheduler drain calls [`take`].
///
/// Concurrency gate (REVIEW #442 risk 2): this counter is plain `u64`,
/// NOT atomic. Until the wiring slice lands, all uses are host-only
/// single-threaded tests. Once real IRQ0 lands, `tick()` (IRQ context)
/// vs `take()` (drain context) race and `take()` loses ticks unless the
/// wiring slice EITHER switches to `AtomicU64` OR gates both sides in a
/// `cli`-held critical section. The wiring slice MUST NOT program the
/// PIT/enable IRQ0 until one of those gates is in place.
///
/// [`tick`]: TimerTicks::tick
/// [`take`]: TimerTicks::take
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimerTicks {
    pending: u64,
}

impl TimerTicks {
    /// New empty counter.
    #[must_use]
    pub const fn new() -> Self {
        Self { pending: 0 }
    }

    /// Record one HW tick (saturates at `u64::MAX`).
    ///
    /// Host-only single-threaded form: once real IRQ0 lands, the IRQ-side
    /// caller must use [`TimerTicks::tick_gated`] under `&IrqGuard` (same
    /// serialized shape as `take_gated` on the drain side); raw `tick()`
    /// stays for host parity tests only.
    pub const fn tick(&mut self) {
        self.pending = self.pending.saturating_add(1);
    }

    /// IRQ-gated tick: identical saturating increment to [`TimerTicks::tick`],
    /// but the caller proves the `cli` critical section with `&IrqGuard`.
    /// Together with [`TimerTicks::take_gated`] this closes the two-sided
    /// gate host-side (REVIEW #448 residual 2): the wiring slice must call
    /// the gated forms on both sides and must not program the PIT/enable
    /// IRQ0 until the gate is held on both paths.
    pub fn tick_gated(&mut self, _gate: &IrqGuard) {
        self.tick();
    }

    /// Pending (undrained) ticks.
    #[must_use]
    pub const fn pending(self) -> u64 {
        self.pending
    }

    /// Drain pending ticks (returns count, resets to 0).
    pub const fn take(&mut self) -> u64 {
        let n = self.pending;
        self.pending = 0;
        n
    }

    /// IRQ-gated drain: identical to [`TimerTicks::take`], but the caller
    /// proves the `cli` critical section with `&IrqGuard`. Raw `take()` is
    /// host-only single-threaded; once real IRQ0 lands, `tick()` (IRQ
    /// context) vs drain (task context) race without the gate. The wiring
    /// slice MUST NOT program the PIT/enable IRQ0 until both sides hold it.
    pub fn take_gated(&mut self, _gate: &IrqGuard) -> u64 {
        self.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pit_divisor_matches_100hz_contract() {
        assert_eq!(PIT_BASE_HZ, 1_193_182);
        assert_eq!(TIMER_HZ, 100);
        assert_eq!(pit_divisor(100), 11931);
        assert_eq!(TIMER_DIVISOR, 11931);
        assert_eq!(TIMER_DIVISOR as u32 * TIMER_HZ, 1_193_100);
    }

    #[test]
    fn pit_divisor_zero_hz_fails_closed() {
        assert_eq!(pit_divisor(0), 0);
    }

    #[test]
    fn pit_divisor_overflow_fails_closed() {
        // REVIEW #442 risk 1: quotient > u16::MAX must fail closed to 0,
        // never silently truncate via `as u16` (hz=1 gave 13534 before).
        assert_eq!(pit_divisor(1), 0);
        assert_eq!(pit_divisor(2), 0);
        assert_eq!(pit_divisor(18), 0); // 1193182/18 = 66287 > 65535
        assert_eq!(pit_divisor(19), 62799); // largest hz still programmable below 18
        assert_eq!(pit_divisor(20), 59659);
        assert_eq!(pit_divisor(1193182), 1);
        assert_eq!(pit_divisor(2_000_000), 0); // quotient 0 also fails closed
    }

    #[test]
    fn pic_offsets_land_irq0_on_vector_32() {
        assert_eq!(PICM_OFFSET, 0x20);
        assert_eq!(PICS_OFFSET, 0x28);
        assert_eq!(VECTOR_TIMER, 32);
        assert_eq!(PIT_CH0, 0x40);
        assert_eq!(PIT_CMD, 0x43);
        assert_eq!(PIT_MODE_CH0_SQUARE, 0x36);
    }

    #[test]
    fn timer_ticks_drain_and_saturate() {
        let mut t = TimerTicks::new();
        assert_eq!(t.pending(), 0);
        assert_eq!(t.take(), 0);
        t.tick();
        t.tick();
        t.tick();
        assert_eq!(t.pending(), 3);
        assert_eq!(t.take(), 3);
        assert_eq!(t.pending(), 0);
        // Saturate, never wrap.
        t.pending = u64::MAX - 1;
        t.tick();
        t.tick();
        t.tick();
        assert_eq!(t.pending(), u64::MAX);
    }

    #[test]
    fn pit_program_bytes_refuses_zero_and_splits_little_endian() {
        // REVIEW #444 residual: no caller enforced the zero-divisor refusal
        // (0x0000 latches 65536, not "stop"). The wiring slice must route
        // through this helper so a 0 can never reach the port.
        assert_eq!(pit_program_bytes(0), None);
        assert_eq!(pit_program_bytes(pit_divisor(0)), None);
        assert_eq!(pit_program_bytes(pit_divisor(1)), None);
        assert_eq!(pit_program_bytes(TIMER_DIVISOR), Some((0x9B, 0x2E)));
        assert_eq!(TIMER_DIVISOR, 11931);
        assert_eq!(pit_program_bytes(1), Some((1, 0)));
        assert_eq!(pit_program_bytes(0xFFFF), Some((0xFF, 0xFF)));
    }

    #[test]
    fn timer_program_const_is_first_caller_and_pins_100hz_bytes() {
        // S07 slice11: closes the zero-callers residual — `TIMER_PROGRAM`
        // is the first in-tree caller of `pit_program_bytes`, so the
        // refusal rule is enforced by construction (guest PIT path consumes
        // this const, never a raw divisor).
        assert_eq!(TIMER_PROGRAM, Some((0x9B, 0x2E)));
        assert_eq!(TIMER_PROGRAM, pit_program_bytes(TIMER_DIVISOR));
        let (lo, hi) = TIMER_PROGRAM.expect("TIMER_DIVISOR is programmable");
        assert_eq!(u16::from(lo) | (u16::from(hi) << 8), TIMER_DIVISOR);
    }

    #[test]
    fn pic_remap_sequence_pins_offsets_and_icw_order() {
        // S07 slice11: pure plan the wiring slice emits in order. Offsets
        // must equal the PICM/S_OFFSET consts (IRQ0 -> VECTOR_TIMER); a
        // swapped ICW2 or a dropped ICW3/ICW4 must go red here.
        // S07 slice12: ICW bytes are named consts so the plan cannot drift
        // from the documented values.
        assert_eq!(PIC_ICW1_INIT_ICW4, 0x11);
        assert_eq!(PIC_ICW3_MASTER_SLAVE_IRQ2, 0x04);
        assert_eq!(PIC_ICW3_SLAVE_IDENTITY, 0x02);
        assert_eq!(PIC_ICW4_8086, 0x01);
        let seq = pic_remap_writes();
        assert_eq!(seq.len(), 8);
        assert_eq!(seq[0], (PICM_CMD, PIC_ICW1_INIT_ICW4));
        assert_eq!(seq[1], (PICS_CMD, PIC_ICW1_INIT_ICW4));
        assert_eq!(seq[2], (PICM_DATA, PICM_OFFSET));
        assert_eq!(seq[3], (PICS_DATA, PICS_OFFSET));
        assert_eq!(seq[2].1, 0x20);
        assert_eq!(seq[3].1, 0x28);
        assert_eq!(VECTOR_TIMER, 32);
        assert_eq!(seq[4], (PICM_DATA, PIC_ICW3_MASTER_SLAVE_IRQ2));
        assert_eq!(seq[5], (PICS_DATA, PIC_ICW3_SLAVE_IDENTITY));
        assert_eq!(seq[6], (PICM_DATA, PIC_ICW4_8086));
        assert_eq!(seq[7], (PICS_DATA, PIC_ICW4_8086));
    }

    #[test]
    fn pic_mask_sequence_unmasks_irq0_only() {
        // S07 slice12 (REVIEW #452 R4): mask plan the wiring slice emits
        // after the remap — master IRQ0 only, slave fully masked.
        assert_eq!(PICM_MASK_IRQ0_ONLY, 0xFE);
        assert_eq!(PICS_MASK_ALL, 0xFF);
        let seq = pic_mask_writes();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq[0], (PICM_DATA, PICM_MASK_IRQ0_ONLY));
        assert_eq!(seq[1], (PICS_DATA, PICS_MASK_ALL));
    }

    #[test]
    fn pit_program_sequence_consumes_timer_program_const() {
        // S07 slice12: guest PIT path consumes TIMER_PROGRAM (never a raw
        // divisor) — mode byte to PIT_CMD, then lo/hi to PIT_CH0.
        let seq = pit_program_writes().expect("TIMER_DIVISOR is programmable");
        assert_eq!(TIMER_PROGRAM, Some((0x9B, 0x2E)));
        assert_eq!(seq.len(), 3);
        assert_eq!(seq[0], (PIT_CMD, PIT_MODE_CH0_SQUARE));
        assert_eq!(seq[0].1, 0x36);
        let (lo, hi) = TIMER_PROGRAM.expect("programmable");
        assert_eq!(seq[1], (PIT_CH0, lo));
        assert_eq!(seq[2], (PIT_CH0, hi));
        assert_eq!(u16::from(lo) | (u16::from(hi) << 8), TIMER_DIVISOR);
    }

    #[test]
    fn timer_stub_eoi_literals_match_pic_consts() {
        // S07 slice12 (REVIEW #452 R3): the stub hardcodes `mov al, 0x20`
        // / `out 0x20, al` as text, so pin the consts to the same literals
        // here — a const/rename drift fails here, not silently in QEMU.
        assert_eq!(PICM_CMD, 0x20);
        assert_eq!(PIC_EOI, 0x20);
        let stub = timer_stub_text();
        assert!(
            stub.contains("mov al, 0x20"),
            "EOI byte must match PIC_EOI: {stub}"
        );
        assert!(
            stub.contains("out 0x20, al"),
            "EOI port must match PICM_CMD: {stub}"
        );
    }

    #[test]
    fn take_gated_agrees_with_take() {
        // Gated drain adds the IrqGuard proof, never new drain logic.
        let gate = unsafe { IrqGuard::enter() };
        let mut g = TimerTicks::new();
        let mut u = TimerTicks::new();
        for _ in 0..3 {
            g.tick();
            u.tick();
        }
        assert_eq!(g.take_gated(&gate), u.take());
        assert_eq!(g.pending(), u.pending());
        assert_eq!(g.take_gated(&gate), 0);
    }

    #[test]
    fn tick_gated_agrees_with_tick() {
        // REVIEW #448 residual 2 (two-sided gate): `tick_gated` adds the
        // IrqGuard proof, never new increment logic — gated tick-for-tick
        // parity with raw `tick()`, plus saturation parity at u64::MAX.
        let gate = unsafe { IrqGuard::enter() };
        let mut g = TimerTicks::new();
        let mut u = TimerTicks::new();
        for _ in 0..3 {
            g.tick_gated(&gate);
            u.tick();
        }
        assert_eq!(g.pending(), u.pending());
        assert_eq!(g.pending(), 3);
        g.pending = u64::MAX - 1;
        u.pending = u64::MAX - 1;
        g.tick_gated(&gate);
        g.tick_gated(&gate);
        u.tick();
        u.tick();
        assert_eq!(g.pending(), u64::MAX);
        assert_eq!(g.pending(), u.pending());
    }

    #[test]
    fn pic_eoi_ports_pinned_for_irq0() {
        // IRQ0 is master-only: exactly one EOI byte to the master command
        // port. Slave ports are remap-path only this slice.
        assert_eq!(PICM_CMD, 0x20);
        assert_eq!(PICM_DATA, 0x21);
        assert_eq!(PICS_CMD, 0xA0);
        assert_eq!(PICS_DATA, 0xA1);
        assert_eq!(PIC_EOI, 0x20);
    }

    #[test]
    fn timer_isr_stub_pins_win64_eoi_iretq() {
        // Same Win64-ABI contract as the #DE/#PF stubs (RCX-first-arg
        // calling convention needs aligned RSP + 32-byte shadow space
        // before `call`): the wiring slice must not drop the EOI or the
        // return (a parking stub would hang the timer; a missing EOI
        // would wedge IRQ0 after the first tick).
        // REVIEW #446 R1: returning-IRQ frame — the stub MUST save/restore
        // the Win64-volatiles and MUST NOT use destructive `and rsp, -16`
        // (loses entry RSP, `iretq` pops the wrong frame -> triple fault
        // on the first tick); EOI must precede the pops so `mov al`
        // cannot clobber the caller's restored rax.
        let stub = timer_stub_text();
        assert!(
            stub.contains("baseos_isr_timer"),
            "stub must define the timer entry: {stub}"
        );
        assert!(
            !stub.contains("and rsp"),
            "returning IRQ stub must not destroy RSP with and: {stub}"
        );
        assert!(
            !stub.contains("andq"),
            "returning IRQ stub must not destroy RSP with andq: {stub}"
        );
        for reg in [
            "push rax", "push rcx", "push rdx", "push r8", "push r9", "push r10", "push r11",
        ] {
            assert!(stub.contains(reg), "stub must save {reg}: {stub}");
        }
        for reg in [
            "pop r11", "pop r10", "pop r9", "pop r8", "pop rdx", "pop rcx", "pop rax",
        ] {
            assert!(stub.contains(reg), "stub must restore {reg}: {stub}");
        }
        // REVIEW #448 residual 1: pin all 7 + LIFO order — a dropped r10/r11
        // or a swapped push/pop pair must fail here, not pass silently.
        let pushes = [
            "push rax", "push rcx", "push rdx", "push r8", "push r9", "push r10", "push r11",
        ];
        let pops = [
            "pop r11", "pop r10", "pop r9", "pop r8", "pop rdx", "pop rcx", "pop rax",
        ];
        let mut last = 0;
        for reg in pushes {
            let i = stub.find(reg).unwrap();
            assert!(i >= last, "push order must be rax..r11: {stub}");
            last = i;
        }
        last = 0;
        for reg in pops {
            let i = stub.find(reg).unwrap();
            assert!(i >= last, "pop order must be r11..rax (LIFO): {stub}");
            last = i;
        }
        // Full push block precedes the call; full pop block follows the EOI
        // and precedes iretq.
        let first_push = stub.find("push rax").unwrap();
        let last_push = stub.find("push r11").unwrap();
        let first_pop = stub.find("pop r11").unwrap();
        let last_pop = stub.find("pop rax").unwrap();
        let sub = stub.find("sub rsp, 32").unwrap();
        let call_idx = stub.find("call timer_irq_rust").unwrap();
        assert!(
            first_push < last_push && last_push < sub && sub < call_idx,
            "push block + shadow must precede call: {stub}"
        );
        assert!(
            first_pop < last_pop,
            "pop block must be LIFO r11..rax: {stub}"
        );
        assert!(
            stub.contains("sub rsp, 32"),
            "32-byte shadow space required by Win64 ABI: {stub}"
        );
        assert!(
            stub.contains("add rsp, 32"),
            "shadow space must be released before iretq: {stub}"
        );
        assert!(
            stub.contains("call timer_irq_rust"),
            "stub must transfer to the Rust body: {stub}"
        );
        assert!(
            stub.contains("out 0x20, al"),
            "stub must EOI the master PIC: {stub}"
        );
        // EOI (clobbers al) must precede the rax restore.
        let eoi = stub.find("out 0x20, al").unwrap();
        let pop_rax = stub.find("pop rax").unwrap();
        assert!(eoi < pop_rax, "EOI must run before the rax restore: {stub}");
        let call = stub.find("call timer_irq_rust").unwrap();
        let add = stub.find("add rsp, 32").unwrap();
        assert!(call < add && add < eoi, "call/add/EOI order: {stub}");
        assert!(
            stub.contains("iretq"),
            "returning stub must iretq, never park: {stub}"
        );
    }
}
