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

/// Pure cumulative-tick observation (S07 slice28 IRQ-move precondition).
///
/// `pending + drained_total` (saturating): the `sti` spin-wait must observe
/// total ticks ever produced, not just the undrained pending count. A future
/// per-tick IRQ-handler drain would hold `pending` near 0 forever and the
/// `pending_snapshot`-only wait would never fire; waiting on the cumulative
/// keeps the same target working before and after the handler move.
/// Pure `core`-only so host tests pin the saturating rule; the guest
/// [`cumulative_snapshot`] reads both statics volatile under the same
/// single-copy `u64` note as [`pending_snapshot`].
#[must_use]
pub const fn cumulative_total(pending: u64, drained_total: u64) -> u64 {
    pending.saturating_add(drained_total)
}

/// Pure join for the seqlock-style cumulative read (S07 slice29).
///
/// `Some(pending.saturating_add(drained))` when the two `drained` reads
/// agree (no IRQ-side accumulate interleaved), `None` on disagreement so
/// the guest snapshot retries. Pure `core`-only so host tests pin it.
#[must_use]
pub const fn cumulative_join(drained_before: u64, pending: u64, drained_after: u64) -> Option<u64> {
    if drained_before != drained_after {
        return None;
    }
    Some(cumulative_total(pending, drained_after))
}

/// Pure wait-target check for the cumulative `sti` wait (S07 slice33).
///
/// `total >= target`: the storm fallback (`cumulative_snapshot` bounded
/// retry, see above) is NOT monotonic — a fallback read can over-count one
/// batch (stale-high pending + fresh-high drained double-counts the
/// in-flight tick) and the next clean read goes BACKWARD. An exact-equality
/// waiter (`== target`) would miss its wakeup after such a step-back and
/// spin until the 200-`hlt` budget expires (loud `TICK ok=0`, but a
/// self-inflicted timeout); a `>=` waiter only ever wakes at most one
/// `hlt` early, never late. All cumulative waiters MUST route through this
/// helper so a future `==` regression fails host-side, not in QEMU.
/// Pure `core`-only so host tests pin it; the guest `sti` wait calls it.
#[must_use]
pub const fn cumulative_wait_ready(total: u64, target: u64) -> bool {
    total >= target
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
#[allow(dead_code)] // S07 slice28: wait moved to cumulative; kept for pending-only diff/QEMU
pub(crate) unsafe fn pending_snapshot() -> u64 {
    core::ptr::addr_of!(TIMER_TICKS).read_volatile().pending()
}

/// Guest-only cumulative drained total (S07 slice28, IRQ-move precondition).
///
/// `static mut`, guest-only: total ticks ever drained by
/// [`poll_timer_dispatch`] (saturating). IRQ-owned like `TIMER_TICKS` so a
/// future periodic IRQ handler can accumulate from IRQ context without
/// borrowing bring-up's stack; this slice the task-context poll owns it
/// under `cli` + `&IrqGuard`. Host tests never touch this (the pure
/// [`cumulative_total`] rule is pinned host-side; the static wiring is
/// proven by QEMU).
#[cfg(not(test))]
static mut TOTAL_DRAINED: u64 = 0;

/// Guest-only cumulative observation (S07 slice28; slice29 seqlock retry).
///
/// Volatile `pending + drained_total` via [`cumulative_total`] so the `sti`
/// wait survives the future per-tick handler drain (pending-only would stall
/// at 0 once the handler drains every tick).
///
/// Race note (REVIEW #491 risk 1): the `sti` wait runs with IF set, so it
/// cannot hold `&IrqGuard` — a per-tick IRQ accumulate between the two
/// reads would double-count (pending read, IRQ moves pending into drained,
/// drained read) or undercount (reverse order). This wait path reads
/// drained-first (`d0`), then pending, then drained-again (`d1`) via the
/// pure [`cumulative_join`], retrying (bounded: 3 tries) while `d0 != d1`.
/// Residual staleness is at most benign for a `>= target` wait: an
/// occasional off-by-one wakes one `hlt` early or spins once more — never
/// a hang, never a wrap (saturating). Exact reads belong to
/// [`cumulative_snapshot_gated`] under `cli` + `&IrqGuard`.
///
/// Each read is single-copy on x86-64 (`u64`, 8-byte aligned).
///
/// # Safety
/// Guest IRQ-enabled wait window only; single-threaded boot owns both statics.
#[cfg(not(test))]
pub(crate) unsafe fn cumulative_snapshot() -> u64 {
    let mut i = 0;
    loop {
        let d0: u64 = core::ptr::addr_of!(TOTAL_DRAINED).read_volatile();
        let pending = core::ptr::addr_of!(TIMER_TICKS).read_volatile().pending();
        let d1: u64 = core::ptr::addr_of!(TOTAL_DRAINED).read_volatile();
        if let Some(total) = cumulative_join(d0, pending, d1) {
            return total;
        }
        i += 1;
        if i >= 3 {
            // Bounded retry exhausted under a tick-storm: fall back to the
            // latest drained-side view (saturating) rather than spinning
            // forever inside the observer. NOT monotonic: the fallback can
            // over-count one batch (stale-high pending + fresh-high drained
            // double-counts the in-flight tick), so a later clean read can
            // return a SMALLER value. Benign for a `>= target` wait (at most
            // one `hlt` early) but callers must never assume monotonicity or
            // exact equality — pinned by the storm-bound test below.
            return cumulative_total(pending, d1);
        }
    }
}

/// Guest-only exact cumulative observation (S07 slice29; wired S07 slice30).
///
/// Single-pass volatile `pending + drained_total` for `cli`-gated contexts
/// (drain path, TICK proof line): the `&IrqGuard` proves IF is masked so
/// no accumulate can interleave and the two-read race cannot occur. The
/// `sti` wait cannot use this (it needs IF set) and uses
/// [`cumulative_snapshot`] instead. Slice30 wires this into
/// `bring_up_cpu_and_prove_exception`'s TICK line (post-`cli` + gate), so
/// the gated path is proven by the QEMU `TICK ok=1` proof, not dead code.
///
/// # Safety
/// `cli` in effect (`gate` proves it); single-threaded boot owns both statics.
#[cfg(not(test))]
pub(crate) unsafe fn cumulative_snapshot_gated(_gate: &IrqGuard) -> u64 {
    let pending = core::ptr::addr_of!(TIMER_TICKS).read_volatile().pending();
    let drained: u64 = core::ptr::addr_of!(TOTAL_DRAINED).read_volatile();
    cumulative_total(pending, drained)
}

/// Guest-only cumulative reset (S07 slice28).
///
/// Zeroes `TOTAL_DRAINED` before the `sti` window so the wait target starts
/// from a known base on every boot (static starts 0 anyway; explicit reset
/// keeps a future warm-reuse honest).
///
/// # Safety
/// Post-EBS guest only, `cli` in effect (pre-`sti`), single-threaded boot
/// owns `TOTAL_DRAINED`.
#[cfg(not(test))]
pub(crate) unsafe fn reset_total_drained() {
    // REVIEW #491 risk 2: volatile-consistent with the snapshot reads —
    // plain `write` here would mix orderings with `read_volatile` there.
    core::ptr::addr_of_mut!(TOTAL_DRAINED).write_volatile(0);
}

/// Guest-only combined PIT-handler step (S07 slice 17; slice 18 delegates
/// to the helper instead of re-implementing it; slice 19 is static-only;
/// S08 slice25 polls the LIVE helper with the IRQ-owned streak static;
/// S07 slice30 calls this same helper per-tick from `timer_irq_rust`).
///
/// Drains the IRQ-owned static `TIMER_TICKS` into the IRQ-owned static
/// scheduler via [`crate::sched::sched_poll_live_static`] (bulk credit to the
/// pre-dispatch current, then at most one gated dispatch on expiry under the
/// 3-on-1-off backoff cap) and
/// returns `(drained, dispatch)` — `dispatch` is `None` when no expiry
/// (nothing to do) and `Some(Ok/Err)` when one resched was attempted.
/// Two call sites (`timer_irq_rust` per-tick + `bring_up_cpu_and_prove_exception`
/// remainder) share this same helper so the guest can never dispatch without
/// draining first (REVIEW #462 risk 1), and the helper owns the order so this
/// wrapper cannot drift from it (REVIEW #464 nit 1). The `TICK` proof line
/// reports the gated cumulative total, never `drained`-only (REVIEW #495 tail
/// 1: the per-tick drain holds the task-context remainder near 0); the
/// dispatch result is intentionally ignored (demo table always has a `Ready`
/// target, so the streak stays `0` and slice-24 parity keeps this identical
/// to the task helper on this table).
///
/// Throttle note (S08 slice25): a failed resched bumps the IRQ-owned streak
/// (suppressed every 4th consecutive failure drains without dispatching, then
/// probes again); the demo table never fails so the guest `TICK` proof is
/// unchanged.
///
/// Static note (S07 slice 19): both ends are IRQ-owned statics now — the
/// counter (`TIMER_TICKS`) and the table (`sched::STATIC_SCHED`, armed once
/// via `sched_init_demo_static` which also resets `sched::STATIC_STREAK`).
/// No stack-local scheduler remains; S07 slice30 calls this same live path
/// per-tick from IRQ context (`timer_irq_rust`), with the task-context
/// remainder proving the gated cumulative total (QEMU TICK re-run owed per
/// REVIEW #497).
///
/// # Safety
/// Post-EBS guest only, `cli` in effect, `gate` proves the critical
/// section; single-threaded boot owns `TIMER_TICKS` and the static sched.
#[cfg(not(test))]
pub(crate) unsafe fn poll_timer_dispatch(
    gate: &IrqGuard,
) -> (u64, Option<Result<u64, crate::thread::SwitchError>>) {
    let (drained, dispatch) =
        crate::sched::sched_poll_live_static(&mut (*core::ptr::addr_of_mut!(TIMER_TICKS)), gate);
    // S07 slice28 cumulative precondition: every drained batch is added to
    // `TOTAL_DRAINED` (saturating) so `cumulative_snapshot` keeps counting
    // across drains. This slice drains once (task-context), so the total
    // equals that batch; the future per-tick IRQ handler accumulates the
    // same way and the `sti` wait (now cumulative) survives the move.
    let total_ptr = core::ptr::addr_of_mut!(TOTAL_DRAINED);
    // REVIEW #491 risk 2: volatile-consistent with snapshot/reset (single
    // ordering family; still `cli` + `&IrqGuard` single-threaded).
    let total = total_ptr.read_volatile();
    total_ptr.write_volatile(total.saturating_add(drained));
    (drained, dispatch)
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

/// Rust body of the IRQ0 stub (S07 slice11, guest-only; slice30 per-tick drain).
///
/// Called from `baseos_isr_timer` with the Win64-volatile frame saved and
/// 32-byte shadow space reserved; the stub performs the single master EOI
/// AFTER this returns (while the saved `rax` still sits on the stack) and
/// `iretq`s. Body records exactly one saturating tick and drains it the same
/// way the task-context poll does (same [`poll_timer_dispatch`] helper, so
/// the two paths cannot drift) — never parks, never touches ports (EOI
/// lives in the stub so `mov al` cannot clobber the caller's `rax` before
/// the pops).
///
/// Reentrancy: safe by construction — the CPU clears IF on entry (interrupt
/// gate) and the PIC withholds this IRQ until the stub's EOI, so no nested
/// IRQ0 can interleave this body. Single-core boot owns `TIMER_TICKS`,
/// `TOTAL_DRAINED`, `STATIC_SCHED`, `STATIC_STREAK` under the entry gate.
/// Pre-`sched_init_demo_static` ticks drain safely (static starts as
/// `Scheduler::new(NULL_THREAD)`: unknown-current drains consume the counter
/// with no dispatch owed, slice-20 backoff by construction).
///
/// S07 slice34 (REVIEW #501 risk 1 pin): the ignored dispatch result is a
/// DEFERRED-resched request, never an immediate stack switch — safe only
/// while `crate::sched::IRQ_STACK_SWITCH_PERMITTED == false` (dispatch is
/// metadata-only state flips, no RSP switch). The real-switch slice must
/// convert this path to record-then-defer.
///
/// S07 slice35 (REVIEW #505 enforcement): the IRQ path `debug_assert!`s
/// `crate::sched::irq_dispatch_must_defer()` on entry (code reference to the
/// gate, not docs-only) — a future real-switch slice cannot switch stacks
/// here while the gate is closed without tripping the debug assert; the
/// real-switch slice flips the const AND converts this path to
/// record-then-defer together.
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
        // S07 slice35 (REVIEW #505): enforce the IRQ stack-switch gate at the
        // IRQ site (not docs-only) — while closed this must defer, never
        // switch stacks in IRQ context.
        debug_assert!(
            crate::sched::irq_dispatch_must_defer(),
            "IRQ stack switch forbidden while gate closed"
        );
        (*core::ptr::addr_of_mut!(TIMER_TICKS)).tick_gated(&gate);
        // S07 slice30 per-tick drain: same combined drain-then-dispatch the
        // task-context poll uses (scheduler credit + TOTAL_DRAINED
        // accumulate + at most one backoff-capped dispatch). Holds pending
        // near 0 by design — the `sti` wait survives via cumulative, and the
        // post-`cli` TICK line reports the cumulative total (never the ~0
        // task-context remainder). Dispatch result ignored in IRQ context
        // (demo table always has a `Ready` target while unsuppressed).
        let _ = poll_timer_dispatch(&gate);
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

    #[test]
    fn cumulative_total_survives_handler_drain() {
        // S07 slice28 IRQ-move precondition: the `sti` wait must observe
        // total ticks ever produced, not just undrained pending. A future
        // per-tick handler drain holds pending near 0; pending-only would
        // stall the wait at 0 while total keeps counting.
        assert_eq!(cumulative_total(0, 0), 0);
        assert_eq!(cumulative_total(5, 0), 5);
        // Handler-drained shape: pending drained to 0-1, total carries the
        // history — the wait target still fires.
        assert_eq!(cumulative_total(0, 5), 5);
        assert_eq!(cumulative_total(1, 4), 5);
        assert_eq!(cumulative_total(5, 3), 8);
        // Saturates, never wraps (u64::MAX is ~5 Gyr at 100 Hz).
        assert_eq!(cumulative_total(u64::MAX, 1), u64::MAX);
        assert_eq!(cumulative_total(u64::MAX - 1, 5), u64::MAX);
        // Wait-target shape: QUANTUM_TICKS + 1 fires on pending-only today
        // (drained 0) and on drained-heavy totals after the handler move.
        assert!(cumulative_total(crate::sched::QUANTUM_TICKS + 1, 0) > crate::sched::QUANTUM_TICKS);
        assert!(cumulative_total(0, crate::sched::QUANTUM_TICKS + 1) > crate::sched::QUANTUM_TICKS);
        assert!(cumulative_total(1, crate::sched::QUANTUM_TICKS) > crate::sched::QUANTUM_TICKS);
    }

    #[test]
    fn cumulative_join_retries_only_on_interleave() {
        // S07 slice29 (REVIEW #491 risk 1): drained-first seqlock join.
        assert_eq!(cumulative_join(4, 1, 4), Some(5));
        assert_eq!(cumulative_join(0, 0, 0), Some(0));
        assert_eq!(cumulative_join(0, 5, 0), Some(5));
        assert_eq!(cumulative_join(u64::MAX, 1, u64::MAX), Some(u64::MAX));
        // Interleaved IRQ accumulate between the reads -> retry.
        assert_eq!(cumulative_join(4, 1, 5), None);
        assert_eq!(cumulative_join(5, 0, 4), None);
        // Old two-read shape would double-count this interleave
        // (pending=1 read, IRQ moves 1 to drained, drained=1 read => 2 for
        // 1 real tick); the join refuses it instead of reporting 2.
        let stale_two_read = cumulative_total(1, 1);
        assert_eq!(stale_two_read, 2);
        assert_eq!(cumulative_join(0, 1, 1), None);
    }

    #[test]
    fn cumulative_storm_fallback_bounds_overcount_and_pins_nonmonotonic() {
        // S07 slice30 (REVIEW #495 tail 2, risk-1a storm pin): the bounded
        // 3-retry fallback returns `cumulative_total(pending, d1)`, which
        // over-counts exactly the in-flight batch when a tick-storm
        // interleaves every attempt — benign for a `>= target` wait (at most
        // one `hlt` early, never a hang) but NOT monotonic.
        //
        // Storm shape: 1 real tick exists. Read d0=0, then pending=1, then
        // the IRQ moves that 1 into drained so d1=1. True total is 1, the
        // fallback reports 2 (over-count 1 = the in-flight batch, never more:
        // one pending value plus one drained value can share at most the
        // pending batch once).
        let fallback = cumulative_total(1, 1);
        assert_eq!(fallback, 2);
        // The NEXT clean read (no interleave: d0=d1=1, pending=0) reports
        // the true 1 — going BACKWARD from the fallback's 2. Successive
        // fallback-then-clean reads are non-monotonic by construction, so
        // callers must only wait on `>= target`, never exact equality.
        let clean_next = cumulative_join(1, 0, 1).expect("clean read agrees");
        assert_eq!(clean_next, 1);
        assert!(
            clean_next < fallback,
            "storm fallback must be allowed to go backward"
        );
        // Bound: over-count is at most the stale pending batch (here 1).
        assert_eq!(fallback.saturating_sub(clean_next), 1);
        // Saturation still holds on the fallback path (never wraps).
        assert_eq!(cumulative_total(u64::MAX, u64::MAX), u64::MAX);
    }

    #[test]
    fn cumulative_wait_ready_uses_ge_never_eq() {
        // S07 slice33 (REVIEW #501 risk 2): all cumulative waiters route
        // through `cumulative_wait_ready` (`>=`), never `==`. Storm shape
        // from the test above: 1 real tick, fallback reports 2, next clean
        // read steps BACK to 1. For target 2 the fallback is ready (wakes
        // at most one `hlt` early — benign) while the stepped-back clean
        // read is not (waiter loops again — never a missed wakeup). An
        // `==` waiter would have fired on the phantom 2 and could then
        // never observe 2 again once the count steps back to 1.
        assert!(cumulative_wait_ready(2, 2));
        assert!(cumulative_wait_ready(3, 2));
        assert!(!cumulative_wait_ready(1, 2));
        // The step-back itself: fallback-ready then clean-not-ready.
        let fallback = cumulative_total(1, 1);
        let clean_next = cumulative_join(1, 0, 1).expect("clean read agrees");
        assert_eq!((fallback, clean_next), (2, 1));
        assert!(cumulative_wait_ready(fallback, 2));
        assert!(!cumulative_wait_ready(clean_next, 2));
        // Boundary + saturation: exact target fires, zero target always
        // fires, saturated totals never wrap past the target.
        assert!(cumulative_wait_ready(0, 0));
        assert!(!cumulative_wait_ready(0, 1));
        assert!(cumulative_wait_ready(u64::MAX, u64::MAX));
        assert!(cumulative_wait_ready(
            crate::sched::QUANTUM_TICKS + 1,
            crate::sched::QUANTUM_TICKS + 1
        ));
    }
}
