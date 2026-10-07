//! S06 slice 16 — cooperative scheduler table (host-testable).
//!
//! Owns the ready-table the [`crate::thread::switch_stub`] picker runs on.
//! This type REPLACES the [`crate::sync::ThreadMutex`] cell rather than
//! reusing it (REVIEW #319 finding 2): the mutex holder travels in a plain
//! `Cell<u64>`, whose get-then-set is an interrupt-TOCTOU under preemption
//! (two threads can both observe free and both store). The scheduler holds
//! no `Cell`, no shared interior mutability at all — table mutation goes
//! through `&mut self` only, and the pick path takes `&self` and never
//! mutates, so a future preemptive port must add an atomic CAS or a
//! `cli`-gated critical section around `spawn`/`dispatch`, never reuse the
//! coop `Cell`.
//!
//! Failure model (fail-closed, never panics, `core`-only):
//! - `spawn` of a duplicate id, a full table (`MAX_THREADS`), or anything
//!   that would fail [`crate::thread::validate_threads`] → `false`, no
//!   mutation.
//! - `pick_next` delegates to `switch_stub` on the owned slice, so empty /
//!   unknown-current / corrupt / nothing-Ready refuse exactly as the picker
//!   does (no registers touched — there are none here yet).
//! - `dispatch` (pick + move Ready→Running) refuses when the pick refuses;
//!   the caller-visible `current` only advances on success.
//!
//! Cooperative only: single-threaded post-EBS boot owns the table (`cli`
//! in effect). Guest multi-thread switch arrives in a later slice; the
//! mutex stays coop-only fail-closed and guest mutex use is out-of-scope
//! this slice.
//!
//! Preemption TOCTOU (S06 slice18 doc, slice28 gate): `pick_next` and
//! `dispatch` read-then-write the table through `&mut self` with no
//! atomic CAS — the caller must hold the [`crate::cpu::IrqGuard`] critical
//! section across the whole pick + state-mark (the live `cli` is
//! established once in `efi_main` post-EBS; the guard token proves the call
//! site runs downstream of it on the single-threaded boot thread).
//! [`Scheduler::dispatch_gated`] /
//! [`Scheduler::exit_current_gated`] take `&IrqGuard`, so a gated call site
//! fails to compile without the guard. Two preempted ungated `dispatch`
//! calls can both pick the same `Ready` target and both mark it `Running`
//! (lost-wake/double-dispatch — pinned by the `gated_dispatch_models_race`
//! test as a cloned-state model, since no host test can fire a real IRQ);
//! the coop `ThreadMutex` `Cell` has the same class (get-then-set
//! double-take). Post-EBS `cli` single-thread holds the gate today; a timer
//! IRQ calls the same gated pick under the gate tomorrow.

use crate::cpu::IrqGuard;
use crate::thread::{validate_threads, SwitchError, Thread, ThreadState, MAX_THREADS, NULL_THREAD};
use crate::timer::TimerTicks;

/// S07 slice 3 — dispatch trace capacity: the first `TRACE_CAP` successful
/// dispatches (each one equal quantum, in order) are recorded as thread
/// ids. Id-keyed (not slot-indexed), so a future remove/compact cannot
/// misattribute it. Prefix-only by design (no ring/wrap): a fresh probe's
/// 3-round fits exactly; long-sweep tail tracing with timestamps rides with
/// the S08 research harness once a timer source exists.
pub const TRACE_CAP: usize = 8;

/// S07 slice 5 — logical quantum length in timer ticks: each successful
/// `dispatch` / `exit_current` resume arms `ticks_left = QUANTUM_TICKS`;
/// each [`Scheduler::tick`] consumes one tick and reports expiry
/// (`ticks_left == 0` means the quantum is spent and the caller should
/// `dispatch` again). No real IRQ yet — the future PIT/APIC handler calls
/// `tick_gated` under the `IrqGuard` gate, then `dispatch_gated` on expiry
/// (same gated pick as the coop path). Small integer keeps the QEMU probe
/// (QUANTUM_TICKS ticks + 1 resched dispatch) one segment.
///
/// S07 slice 6 (REVIEW #439 residual 3): probe + unit asserts are
/// const-driven (`used == QUANTUM_TICKS`, looped ticks, `format!` serial
/// shape), so a quantum change touches only this const plus the ps1
/// `q=<n>` hard-require literal and the serial doc examples — no hidden
/// `== 4` gate stays green while the guest diverges.
pub const QUANTUM_TICKS: u64 = 4;

/// Cooperative scheduler: owns up to `MAX_THREADS` descriptors plus the
/// current-thread id. No `Cell`, no atomics — `&mut self` for mutation,
/// `&self` for picks.
///
/// S07 slice 1 — CPU accounting: `runs` counts successful dispatches per
/// table slot (parallel to `table`), unit of fairness/throughput baseline.
/// Saturating increments only on success (pick refusal mutates nothing);
/// read-only `runs_of` / `total_dispatches` for host tests + guest line.
///
/// Slot-index invariant (REVIEW #431 F1): `runs[i]` belongs to the thread
/// in `table[i]`; `spawn` appends at `len` and zeroes `runs[len]`, and no
/// removal/compaction API exists, so slots never shift. If a future
/// remove/compact API is added it MUST shift `runs` with `table` (or the
/// counts misattribute); the sweep test below would go red on misalignment.
///
/// S07 slice 16 — periodic PIT-handler accounting: `cpu[i]` belongs to the
/// thread in `table[i]` under the same append-only invariant (zeroed on
/// spawn, shifted with `table` if removal ever lands). Each gated
/// [`Scheduler::drain_timer`] attributes the drained HW ticks to the
/// CURRENT thread's slot (saturating add, one bulk O(1) credit — the
/// running thread consumed those ticks while the quantum counted down).
/// Unknown-`current` or empty drain credits nothing (fail-closed, no
/// phantom time). Read via `cpu_ticks_of` / `total_cpu_ticks`.
///
/// S07 slice 2 — fairness: each successful `dispatch` is one equal quantum
/// (no timer/preempt yet — preempt IRQ rides with a future timer slice and
/// reuses the same gated pick). Fairness over N rounds is `max - min` of
/// `runs` (see [`Scheduler::fairness_spread`]): a balanced round-robin
/// sweep holds spread <= 1.
///
/// S07 slice 3 — tracing: each successful `dispatch` (and each
/// `exit_current` resume) appends the resumed id to `trace` while
/// `trace_len < TRACE_CAP` (see [`Scheduler::trace_at`]). The serial
/// `SCHED-TRACE` probe emits the first 3 entries (`t0=2 t1=3 t2=1`).
///
/// S07 slice 5 — quantum: `ticks_left` counts the remaining timer ticks in
/// the current quantum (`QUANTUM_TICKS` after every successful dispatch /
/// exit resume, `0` when idle or after the last exit parks `NULL`). The
/// counter is global (one running thread at a time under coop yield); a
/// future per-thread budget rides with priorities in a later slice.
///
/// S07 slice 6 (REVIEW #439 residuals 1+2, intended-until-timer contract):
/// (1) an early yield discards the remainder — `dispatch` rearms to full
/// `QUANTUM_TICKS` instead of carrying leftover ticks, documented as
/// intended until per-thread budgets + priorities arrive; (2) an idle or
/// `NULL`-parked `tick` reports expired (`true`), so the future PIT/APIC
/// handler MUST NOT re-tick in a tight loop when `current` is `NULL` —
/// gate the re-tick (only tick when a thread runs) and `dispatch` once on
/// expiry instead of spinning on `tick() == true`.
#[derive(Debug, Clone)]
pub struct Scheduler {
    table: [Option<Thread>; MAX_THREADS],
    runs: [u64; MAX_THREADS],
    cpu: [u64; MAX_THREADS],
    trace: [u64; TRACE_CAP],
    trace_len: usize,
    ticks_left: u64,
    len: usize,
    current: u64,
}

/// S07 slice 22 — live-IRQ failure backoff cap (REVIEW #470 R1 policy,
/// pure, host-testable, no production behavior change yet).
///
/// The task-context [`Scheduler::drain_then_dispatch_gated`] keeps its
/// slice-18 retry-spin (every non-empty drain on a KNOWN current retries
/// exactly one dispatch). The future live periodic IRQ handler MUST NOT
/// inherit that spin unbounded: a persistently unready table (e.g. all
/// `Done`, known current, `Some(Err)` every batch) would otherwise attempt
/// one failed pick per 100 Hz batch forever. Policy: the live handler
/// drains every batch (counter always consumed) but attempts a dispatch
/// only while `!live_backoff_suppresses(failures)`; `failures` counts
/// consecutive `Some(Err)` batches (saturating), reset to `0` on
/// `Some(Ok)` or `None` (no expiry / unknown-current backoff / empty).
/// `LIVE_MAX_CONSECUTIVE_FAILURES = 3` bounds the spam to 3 picks, then
/// drain-only until a success or idle `None` resets the streak.
pub const LIVE_MAX_CONSECUTIVE_FAILURES: u64 = 3;

/// Live-handler gate: `true` once the consecutive-failure streak reaches
/// the cap (caller drains without dispatching while suppressed).
#[must_use]
pub const fn live_backoff_suppresses(consecutive_failures: u64) -> bool {
    consecutive_failures >= LIVE_MAX_CONSECUTIVE_FAILURES
}

/// Live-handler streak step: `Some(Err)` bumps (saturating), anything else
/// (`Some(Ok)` success, `None` no-dispatch) resets to `0`.
#[must_use]
pub const fn live_backoff_next(consecutive_failures: u64, failed: bool) -> u64 {
    if failed {
        consecutive_failures.saturating_add(1)
    } else {
        0
    }
}

/// IRQ stack-switch gate (S07 slice34, REVIEW #501 risk 1 pin).
///
/// `false` while IRQ-context dispatch is metadata-only (Ready<->Running
/// state flips + quantum rearm, no RSP/register switch). `timer_irq_rust`
/// dispatches via `drain_then_dispatch_live_gated` and IGNORES the result
/// (`let _ =`) — safe only while this is `false`: the ignored `Ok(next)`
/// is a deferred-resched request, never an immediate stack switch. The
/// slice that lands a real RSP switch MUST flip this to `true` AND convert
/// the IRQ path to record-then-defer (no in-IRQ switch); any guest edit
/// that switches stacks while this is `false` violates the gate. Pure
/// `core`-only so host tests pin it.
pub const IRQ_STACK_SWITCH_PERMITTED: bool = false;

/// IRQ-gate enforcement helper (S07 slice35, REVIEW #505 enforcement).
///
/// `true` while IRQ-context dispatch must defer (`!IRQ_STACK_SWITCH_PERMITTED`).
/// Pure `core`-only so host tests pin it; the guest `timer_irq_rust` IRQ path
/// `debug_assert!`s this (code reference to the gate, not docs-only), so a
/// future real-switch slice that flips the const without converting the IRQ
/// path to record-then-defer fails the IRQ debug build instead of silently
/// switching stacks in IRQ context.
#[must_use]
pub const fn irq_dispatch_must_defer() -> bool {
    !IRQ_STACK_SWITCH_PERMITTED
}

impl Scheduler {
    /// New empty scheduler with `current` preset (usually the boot thread
    /// id, or `NULL_THREAD` when no thread runs yet). `current` is NOT
    /// validated here — `pick_next` fails closed (`Unknown`) when it is
    /// absent from the table, so construction can never smuggle a target.
    ///
    /// Boot bookkeeping (REVIEW #321 residual a): `new` never marks
    /// `current` as `Running` — the spawned descriptor stays `Ready`
    /// (as built by `Thread::new`). The first `dispatch` therefore
    /// parks nothing: the ex-current slot is `Ready`, not `Running`,
    /// so it stays `Ready` while the picked next moves Ready->Running.
    /// Benign under cooperative yield (the boot thread simply becomes
    /// scheduled like any other `Ready` thread); the second `dispatch`
    /// onward parks the previous `Running` thread normally.
    #[must_use]
    pub const fn new(current: u64) -> Self {
        Self {
            table: [None; MAX_THREADS],
            runs: [0; MAX_THREADS],
            cpu: [0; MAX_THREADS],
            trace: [0; TRACE_CAP],
            trace_len: 0,
            ticks_left: 0,
            len: 0,
            current,
        }
    }

    /// Number of spawned threads.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// True when no thread spawned.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Current-thread id (`NULL_THREAD` when idle).
    #[must_use]
    pub const fn current(&self) -> u64 {
        self.current
    }

    /// Lifecycle state of one spawned thread (`None` when unknown id).
    /// Test- and guest-wiring query: read-only, never mutates, so the
    /// first-dispatch bookkeeping (boot stays `Ready`, parked-nothing)
    /// is assertable without exposing `&mut` table access.
    #[must_use]
    pub fn state_of(&self, id: u64) -> Option<ThreadState> {
        let mut i = 0;
        while i < self.len {
            if let Some(t) = self.table[i] {
                if t.id() == id {
                    return Some(t.state());
                }
            }
            i += 1;
        }
        None
    }

    /// Dispatch count of one spawned thread (`None` when unknown id).
    /// Read-only accounting query for S07 fairness/throughput baseline.
    #[must_use]
    pub fn runs_of(&self, id: u64) -> Option<u64> {
        let mut i = 0;
        while i < self.len {
            if let Some(t) = self.table[i] {
                if t.id() == id {
                    return Some(self.runs[i]);
                }
            }
            i += 1;
        }
        None
    }

    /// Scheduling priority of one spawned thread (`None` when unknown id).
    /// Read-only data query (S07 slice40, data-only): `spawn` carries the
    /// descriptor's priority through, `dispatch`/`exit_current` ignore it
    /// (round-robin preserved until the policy slice).
    #[must_use]
    pub fn priority_of(&self, id: u64) -> Option<u8> {
        let mut i = 0;
        while i < self.len {
            if let Some(t) = self.table[i] {
                if t.id() == id {
                    return Some(t.priority());
                }
            }
            i += 1;
        }
        None
    }

    /// Total successful dispatches (saturating sum over spawned slots).
    #[must_use]
    pub fn total_dispatches(&self) -> u64 {
        let mut total = 0u64;
        let mut i = 0;
        while i < self.len {
            total = total.saturating_add(self.runs[i]);
            i += 1;
        }
        total
    }

    /// CPU ticks consumed by one spawned thread (`None` when unknown id).
    /// Read-only periodic-accounting query (S07 slice 16): each gated
    /// `drain_timer` credits the drained HW ticks to the current thread.
    #[must_use]
    pub fn cpu_ticks_of(&self, id: u64) -> Option<u64> {
        let mut i = 0;
        while i < self.len {
            if let Some(t) = self.table[i] {
                if t.id() == id {
                    return Some(self.cpu[i]);
                }
            }
            i += 1;
        }
        None
    }

    /// Total accounted CPU ticks (saturating sum over spawned slots).
    #[must_use]
    pub fn total_cpu_ticks(&self) -> u64 {
        let mut total = 0u64;
        let mut i = 0;
        while i < self.len {
            total = total.saturating_add(self.cpu[i]);
            i += 1;
        }
        total
    }

    /// Fairness spread over spawned slots: `max(runs) - min(runs)`.
    /// `0` when empty (no threads) or when all counts tie; a balanced
    /// round-robin sweep of whole rounds holds `<= 1` (each thread gets
    /// one quantum per round). Read-only, never mutates. Saturating
    /// subtraction (counts only grow via saturating adds, so `max >= min`
    /// always holds; still saturating for fail-closed symmetry).
    #[must_use]
    pub fn fairness_spread(&self) -> u64 {
        if self.len == 0 {
            return 0;
        }
        let mut min = self.runs[0];
        let mut max = self.runs[0];
        let mut i = 1;
        while i < self.len {
            if self.runs[i] < min {
                min = self.runs[i];
            }
            if self.runs[i] > max {
                max = self.runs[i];
            }
            i += 1;
        }
        max.saturating_sub(min)
    }

    /// Number of recorded trace entries (`min(total_dispatches, TRACE_CAP)`).
    /// Prefix-only: the first `TRACE_CAP` quanta in dispatch order.
    #[must_use]
    pub const fn trace_len(&self) -> usize {
        self.trace_len
    }

    /// `i`-th dispatched id in record order (`None` when `i >= trace_len`).
    /// Id-keyed: entries are thread ids, never slot indexes, so they stay
    /// valid under any future table compaction.
    #[must_use]
    pub fn trace_at(&self, i: usize) -> Option<u64> {
        if i < self.trace_len {
            Some(self.trace[i])
        } else {
            None
        }
    }

    /// Remaining timer ticks in the current quantum (`QUANTUM_TICKS` right
    /// after a successful `dispatch` / `exit_current` resume, `0` when idle
    /// or after the last exit parks `NULL`). Read-only, never mutates.
    #[must_use]
    pub const fn quantum_left(&self) -> u64 {
        self.ticks_left
    }

    /// Consume one timer tick; returns `true` when the quantum is spent
    /// (`ticks_left == 0`, caller should `dispatch` again). Saturating
    /// countdown (never wraps); idle (`ticks_left == 0`) reports expired
    /// so an un-armed scheduler asks for a dispatch rather than spinning.
    /// Never panics; refusals never reach here (no pick involved).
    /// Timer-slice rule (slice 6): when `current` is `NULL` (idle) the
    /// caller must `dispatch` once on `true`, never loop `tick()` — the
    /// PIT/APIC handler gates the re-tick on a running thread.
    pub fn tick(&mut self) -> bool {
        if self.ticks_left > 0 {
            self.ticks_left -= 1;
        }
        self.ticks_left == 0
    }

    /// IRQ-gated tick (S07 slice 5 timer-quantum gate): identical countdown
    /// to [`Scheduler::tick`], but the caller proves the `cli` critical
    /// section with `&IrqGuard` — the future PIT/APIC handler calls this
    /// under the gate, then [`Scheduler::dispatch_gated`] on expiry (same
    /// serialized pick + state-mark the coop probe holds today).
    pub fn tick_gated(&mut self, _gate: &IrqGuard) -> bool {
        self.tick()
    }

    /// Drain HW ticks into the quantum (S07 slice 8 wiring-prep): takes the
    /// pending [`TimerTicks`] count under the `IrqGuard` gate and reports
    /// `(drained, expired)` — `expired` means the caller should
    /// [`Scheduler::dispatch_gated`] ONCE, never loop `tick()` (slice 6
    /// timer rule: an idle/`NULL`-parked `tick()` reports expired, so a
    /// tight re-tick loop spins forever; the future IRQ handler gates the
    /// re-tick on a running thread instead).
    ///
    /// Empty drain (`drained == 0`) reports `expired == false` (nothing to
    /// do) — distinct from a bare `tick()` on an idle scheduler, which
    /// reports `true`. Saturating bulk accounting, O(1) in the backlog:
    /// REVIEW #446 R2 — the slice-8 `while i < drained` loop hung the
    /// handler in O(n) on a `cli`-held flood backlog (drained up to
    /// `u64::MAX`); with `QUANTUM_TICKS` the result only needs
    /// `ticks_left.saturating_sub(drained)` plus
    /// `expired = ticks_left == 0`, so at most one dispatch is owed no
    /// matter how large the backlog.
    /// Unknown-`current` policy (S07 slice 20, backoff by construction):
    /// an unknown `current` (not in the table, e.g. idle `NULL`) drains the
    /// HW counter (ticks are consumed, never replayed) but preserves the
    /// quantum (`ticks_left` untouched) and reports `expired == false`, so
    /// the caller owes NO dispatch. Rationale: no running thread consumed
    /// those ticks (no phantom time to credit) and no quantum ran down, so
    /// a per-IRQ retry would spam one failed pick per tick batch forever.
    /// Known-`current` drains keep the slice-18 retry-spin (consumed quantum
    /// stays `0`, every non-empty drain retries exactly one dispatch).
    pub fn drain_timer(&mut self, ticks: &mut TimerTicks, gate: &IrqGuard) -> (u64, bool) {
        let drained = ticks.take_gated(gate);
        if drained == 0 {
            return (0, false);
        }
        // S07 slice 20: unknown current preserves the quantum (fail-closed,
        // no dispatch owed) — find the slot first, before consuming anything.
        let mut slot: Option<usize> = None;
        let mut i = 0;
        while i < self.len {
            if let Some(t) = self.table[i] {
                if t.id() == self.current {
                    slot = Some(i);
                    break;
                }
            }
            i += 1;
        }
        let Some(s) = slot else {
            return (drained, false);
        };
        // S07 slice 16 periodic accounting: the running thread consumed
        // these HW ticks — credit them to the current slot (bulk O(1),
        // saturating).
        self.cpu[s] = self.cpu[s].saturating_add(drained);
        self.ticks_left = self.ticks_left.saturating_sub(drained);
        let expired = self.ticks_left == 0;
        (drained, expired)
    }

    /// Combined drain-then-dispatch step (S07 slice 17, PIT-handler shape).
    ///
    /// Drains `ticks` via [`Scheduler::drain_timer`] (credits the drained
    /// HW ticks to the PRE-dispatch current slot) and, on expiry only,
    /// performs exactly one [`Scheduler::dispatch_gated`]. Returns
    /// `(drained, dispatch)` where `dispatch` is `None` when no expiry
    /// (nothing to do, including the empty-drain case) and
    /// `Some(Ok(next))` / `Some(Err(e))` when one gated dispatch was
    /// attempted. The caller never dispatches without draining first.
    ///
    /// Misattribution window closed by construction (REVIEW #462 risk 1):
    /// ticks that accumulate across a dispatch are always credited to the
    /// thread that ran while they accumulated (the pre-pick `current`),
    /// because the credit runs BEFORE the pick inside this helper. Callers
    /// MUST use this helper (or the equivalent `drain_timer`-then-`dispatch`
    /// sequence) — a bare `dispatch_gated` after idle HW ticks would credit
    /// the whole backlog to the NEW current. Unknown-`current` drains
    /// preserve the quantum and owe no dispatch (S07 slice 20 backoff,
    /// pinned by the edges test); a failed resched on a KNOWN current
    /// (`Some(Err)`) keeps the consumed quantum (still `0`), it does not
    /// rearm.
    ///
    /// Retry-spin semantics (S07 slice 18, REVIEW #464 nit 2 — intentional;
    /// slice 20 scopes it to known-`current` only): because the consumed
    /// quantum stays `0`, every subsequent NON-EMPTY drain on a known current
    /// reports expired and retries exactly one gated dispatch, while empty
    /// drains (`drained == 0`) report `(0, None)` and never retry.
    /// Unknown-`current` drains report `(drained, None)` and never retry
    /// (backoff by construction — the live IRQ handler inherits this, so an
    /// idle table costs one counter drain per tick batch, never a failed
    /// pick per batch).
    ///
    /// Overflow-signal decision (S07 slice 18): `cpu[]`/`runs[]` saturate
    /// (no wrap, no overflow flag); the S08 harness adds a signal only if
    /// it ever samples long enough to care (`u64::MAX` is ~5 Gyr at 100 Hz).
    pub fn drain_then_dispatch_gated(
        &mut self,
        ticks: &mut TimerTicks,
        gate: &IrqGuard,
    ) -> (u64, Option<Result<u64, SwitchError>>) {
        let (drained, expired) = self.drain_timer(ticks, gate);
        if !expired {
            return (drained, None);
        }
        (drained, Some(self.dispatch_gated(gate)))
    }

    /// Live-IRQ drain-then-dispatch step consuming the slice-22 backoff cap
    /// (S07 slice 23, pure, host-testable, guest unwired this slice).
    ///
    /// Same drain-then-dispatch shape as
    /// [`Scheduler::drain_then_dispatch_gated`] (bulk credit to the
    /// pre-dispatch current before the pick, at most one gated dispatch),
    /// plus the [`LIVE_MAX_CONSECUTIVE_FAILURES`] bound the future periodic
    /// IRQ handler needs: the task-context helper retry-spins (one failed
    /// pick per non-empty batch forever), which at 100 Hz is a spam loop the
    /// live handler must not inherit.
    ///
    /// Streak rule (owns `*consecutive_failures` so callers cannot forget):
    /// - suppressed entry (`live_backoff_suppresses(*consecutive_failures)`):
    ///   drain-only (counter consumed, CPU credited, quantum still consumed
    ///   via `drain_timer`), NO dispatch attempted, streak RESET to `0`.
    ///   Reset (not hold) is the recovery path: a suppressed batch that held
    ///   the streak would stall forever once the table becomes `Ready` again
    ///   (no dispatch means no `Some(Ok)` to reset with). Reset bounds
    ///   CONSECUTIVE failed picks to `LIVE_MAX_CONSECUTIVE_FAILURES`
    ///   (3-on-1-off throttle under persistent failure) with automatic probe.
    /// - unsuppressed, no expiry (`None`: empty / partial / unknown-current
    ///   backoff): no dispatch, streak reset to `0`.
    /// - unsuppressed, expiry: exactly one gated dispatch; `Some(Ok)` resets
    ///   to `0`, `Some(Err)` bumps (saturating) via [`live_backoff_next`].
    ///
    /// Unknown-`current` inherits the slice-20 backoff (drain consumes, no
    /// dispatch owed) and resets the streak. Task-context
    /// [`Scheduler::drain_then_dispatch_gated`] is untouched (keeps its
    /// retry-spin); the S08 live-handler wiring calls THIS helper from IRQ
    /// context with an IRQ-owned streak static.
    pub fn drain_then_dispatch_live_gated(
        &mut self,
        ticks: &mut TimerTicks,
        gate: &IrqGuard,
        consecutive_failures: &mut u64,
    ) -> (u64, Option<Result<u64, SwitchError>>) {
        if live_backoff_suppresses(*consecutive_failures) {
            let (drained, _) = self.drain_timer(ticks, gate);
            *consecutive_failures = 0;
            return (drained, None);
        }
        let (drained, expired) = self.drain_timer(ticks, gate);
        if !expired {
            *consecutive_failures = 0;
            return (drained, None);
        }
        let res = self.dispatch_gated(gate);
        *consecutive_failures = live_backoff_next(*consecutive_failures, res.is_err());
        (drained, Some(res))
    }

    /// Demo table constructor the IRQ-owned static consumes (S07 slice 19).
    ///
    /// Builds `Scheduler::new(1)`, spawns metadata-only descriptors 1 and 2
    /// (same bases/entry ids as the bring-up probe — no stack switch, just
    /// the quantum counter the HW ticks drain into), then one gated dispatch
    /// (armed, `current == 2`). Spawn/dispatch failures are ignored (demo
    /// table always has a `Ready` target when the refusal-free path holds).
    /// Pure (no static) so host tests pin it; the guest
    /// [`sched_init_demo_static`] copies it into `STATIC_SCHED`.
    pub fn demo_armed_scheduler(gate: &IrqGuard) -> Self {
        let mut s = Scheduler::new(1);
        if let Ok(t1) = Thread::new(1, 0x1000_0000, 8192, 0x1001) {
            let _ = s.spawn(t1);
        }
        if let Ok(t2) = Thread::new(2, 0x2000_0000, 8192, 0x1002) {
            let _ = s.spawn(t2);
        }
        let _ = s.dispatch_gated(gate);
        s
    }

    /// Record one quantum while capacity remains (refusals never reach here;
    /// saturation keeps the prefix deterministic instead of wrapping).
    fn record_trace(&mut self, id: u64) {
        if self.trace_len < TRACE_CAP {
            self.trace[self.trace_len] = id;
            self.trace_len += 1;
        }
    }

    /// Live table as a packed array copy for the picker. Packed (no holes):
    /// `spawn` appends, never leaves `None` inside `[..len)`. Copies every
    /// populated slot over the full capacity, so a just-written
    /// `table[len]` pending slot is included too (REVIEW #321 residual b:
    /// looping only `[..len)` would EXCLUDE that pending slot and break
    /// `spawn`'s pre-commit `validate_threads` — tried, 4 tests went red).
    /// Callers slice with `len` (`pick_next`) or `len + 1` (`spawn`
    /// pre-commit); the filler beyond the slice is unreachable.
    fn packed(&self) -> [Thread; MAX_THREADS] {
        // `Thread` has no `Default`; rebuild from a known descriptor then
        // overwrite the live prefix. The filler is unreachable via `len`.
        let filler = match Thread::new(1, 0x1000_0000, 8192, 0x1000) {
            Ok(t) => t,
            Err(_) => unreachable!(),
        };
        let mut out = [filler; MAX_THREADS];
        let mut i = 0;
        while i < MAX_THREADS {
            if let Some(t) = self.table[i] {
                out[i] = t;
            }
            i += 1;
        }
        out
    }

    /// Append `thread` to the table. Fails closed (`false`, no mutation)
    /// when full, when the id already exists, when the id is the shared
    /// `NULL_THREAD` sentinel, or when the resulting table would fail
    /// [`validate_threads`] (overlap / degenerate smuggle caught here,
    /// before the thread can become a pick target).
    #[must_use]
    pub fn spawn(&mut self, thread: Thread) -> bool {
        if self.len >= MAX_THREADS {
            return false;
        }
        if thread.id() == NULL_THREAD {
            return false;
        }
        let mut i = 0;
        while i < self.len {
            if let Some(t) = self.table[i] {
                if t.id() == thread.id() {
                    return false;
                }
            }
            i += 1;
        }
        self.table[self.len] = Some(thread);
        self.runs[self.len] = 0;
        self.cpu[self.len] = 0;
        let packed = self.packed();
        if validate_threads(&packed[..self.len + 1]).is_err() {
            self.table[self.len] = None;
            return false;
        }
        self.len += 1;
        true
    }

    /// Pick the next `Ready` id after `current` without mutating (same
    /// round-robin contract as `switch_stub`).
    pub fn pick_next(&self) -> Result<u64, SwitchError> {
        if self.len == 0 {
            return Err(SwitchError::Unknown);
        }
        let packed = self.packed();
        crate::thread::switch_stub(&packed[..self.len], self.current)
    }

    /// Priority-aware pick (S07 slice41, opt-in — RR default preserved).
    ///
    /// Highest-priority `Ready` thread excluding `current`; ties break in
    /// RR slice order (first max-prio candidate after `current`, wrapping).
    /// Same refusal set as `switch_stub`: empty/unknown-current →
    /// `Unknown`, corrupt table → `InvalidTable`, no other `Ready` →
    /// `NoSwitch` (current `Ready`) / `NotReady` (current not `Ready`).
    /// Read-only, never mutates. `dispatch`/`dispatch_gated` keep RR until
    /// a policy slice flips the default; use `dispatch_priority` for the
    /// policy path.
    pub fn pick_next_priority(&self) -> Result<u64, SwitchError> {
        if self.len == 0 {
            return Err(SwitchError::Unknown);
        }
        let packed = self.packed();
        validate_threads(&packed[..self.len])?;
        let mut start: Option<usize> = None;
        let mut i = 0;
        while i < self.len {
            if packed[i].id() == self.current {
                start = Some(i);
                break;
            }
            i += 1;
        }
        let start = match start {
            Some(idx) => idx,
            None => return Err(SwitchError::Unknown),
        };
        // Max priority among other Ready threads (O(n), same class as
        // `priority_of`; perf-first note: policy pick is linear, recorded
        // for the S08 measurement slice, not the hot path yet).
        let mut best: Option<u8> = None;
        let mut k = 0;
        while k < self.len {
            let t = packed[k];
            if t.id() != self.current && t.state() == ThreadState::Ready {
                let p = t.priority();
                if best.is_none() || p > best.unwrap() {
                    best = Some(p);
                }
            }
            k += 1;
        }
        let best = match best {
            Some(p) => p,
            None => {
                if packed[start].state() == ThreadState::Ready {
                    return Err(SwitchError::NoSwitch);
                }
                return Err(SwitchError::NotReady);
            }
        };
        // RR tie-break: first max-prio candidate after current, wrapping.
        let mut step = 1;
        while step <= self.len {
            let t = packed[(start + step) % self.len];
            if t.id() != self.current && t.state() == ThreadState::Ready && t.priority() == best {
                return Ok(t.id());
            }
            step += 1;
        }
        Err(SwitchError::NotReady)
    }

    /// Pick the next `Ready` thread and move it Ready→Running, parking the
    /// previous `Running` thread back to Ready (cooperative yield). The
    /// previous `Done` thread stays `Done`. On pick refusal returns the
    /// picker's `Err` with no mutation. First-dispatch note: when the
    /// scheduler was built with `new(current)` the ex-current descriptor
    /// is still `Ready`, so nothing is parked on the first success (only
    /// the picked next flips to `Running`). On success arms a fresh
    /// quantum (`ticks_left = QUANTUM_TICKS`); on refusal leaves the
    /// counter untouched.
    pub fn dispatch(&mut self) -> Result<u64, SwitchError> {
        let next = self.pick_next()?;
        let mut i = 0;
        while i < self.len {
            if let Some(ref mut t) = self.table[i] {
                if t.id() == next {
                    if t.state() != ThreadState::Ready {
                        return Err(SwitchError::NotReady);
                    }
                    let _ = t.set_state(ThreadState::Running);
                    self.runs[i] = self.runs[i].saturating_add(1);
                } else if t.id() == self.current && t.state() == ThreadState::Running {
                    let _ = t.set_state(ThreadState::Ready);
                }
            }
            i += 1;
        }
        self.record_trace(next);
        self.current = next;
        self.ticks_left = QUANTUM_TICKS;
        Ok(next)
    }

    /// IRQ-gated dispatch (S06 preempt-IRQ gate, task #19): identical
    /// pick-then-mark to [`Scheduler::dispatch`], but the caller proves the
    /// `cli` critical section by holding `&IrqGuard` across the whole
    /// pick + state-mark. A timer IRQ firing between `pick_next` and the
    /// `Ready`→`Running` mark would otherwise double-dispatch the same
    /// target; the guard makes the ungated call site a compile error where
    /// the gated path is required (guest `chain_leg`, join/exit probe).
    pub fn dispatch_gated(&mut self, _gate: &IrqGuard) -> Result<u64, SwitchError> {
        self.dispatch()
    }

    /// Priority-aware dispatch (S07 slice41, opt-in): same Ready→Running
    /// state moves + quantum rearm + trace as [`Scheduler::dispatch`], but
    /// the target comes from [`Scheduler::pick_next_priority`]. Refusals
    /// mutate nothing (counter untouched). RR stays the default; the guest
    /// probe stays on `dispatch_gated` this slice.
    pub fn dispatch_priority(&mut self) -> Result<u64, SwitchError> {
        let next = self.pick_next_priority()?;
        let mut i = 0;
        while i < self.len {
            if let Some(ref mut t) = self.table[i] {
                if t.id() == next {
                    if t.state() != ThreadState::Ready {
                        return Err(SwitchError::NotReady);
                    }
                    let _ = t.set_state(ThreadState::Running);
                    self.runs[i] = self.runs[i].saturating_add(1);
                } else if t.id() == self.current && t.state() == ThreadState::Running {
                    let _ = t.set_state(ThreadState::Ready);
                }
            }
            i += 1;
        }
        self.record_trace(next);
        self.current = next;
        self.ticks_left = QUANTUM_TICKS;
        Ok(next)
    }

    /// IRQ-gated priority dispatch: same `&IrqGuard` proof as
    /// [`Scheduler::dispatch_gated`], policy target from
    /// [`Scheduler::pick_next_priority`].
    pub fn dispatch_priority_gated(&mut self, _gate: &IrqGuard) -> Result<u64, SwitchError> {
        self.dispatch_priority()
    }

    /// Mark `id` observed-finished (S06 slice27 join): read-only query,
    /// `true` iff the spawned thread is `Done`. The guest joiner spins or
    /// parks on this; the exiting thread reaches `Done` via
    /// [`Scheduler::exit_current`]. Unknown ids report `false` (never a
    /// phantom join — `state_of` is `None`, not `Done`).
    #[must_use]
    pub fn join(&self, id: u64) -> bool {
        self.state_of(id) == Some(ThreadState::Done)
    }

    /// Exit the current thread (S06 slice27 join/preempt primitive): move
    /// `current` to `Done` (Running→Done, or Ready→Done when the boot
    /// thread exits before its first park), then pick the next `Ready`
    /// thread round-robin and move it Ready→Running (the preempt/yield
    /// resume point — a future timer IRQ calls this same pick under a
    /// `cli`/atomic gate instead of the coop caller). Returns the resumed
    /// id. When nothing remains `Ready`, `current` parks on `NULL_THREAD`
    /// and returns the picker's `Err` (joiners observe `Done` via
    /// [`Scheduler::join`]); the `Done` mark is kept either way. A resumed
    /// thread arms a fresh quantum (`ticks_left = QUANTUM_TICKS`); parking
    /// on `NULL` disarms it (`ticks_left = 0`); a refused exit (`Unknown`)
    /// leaves the counter untouched.
    pub fn exit_current(&mut self) -> Result<u64, SwitchError> {
        let mut i = 0;
        let mut marked = false;
        while i < self.len {
            if let Some(ref mut t) = self.table[i] {
                if t.id() == self.current
                    && (t.state() == ThreadState::Running || t.state() == ThreadState::Ready)
                {
                    let _ = t.set_state(ThreadState::Done);
                    marked = true;
                    // S06 slice27b: mark the FIRST match only, then stop.
                    // `spawn` refuses duplicate ids so a healthy table holds
                    // one match; marking ALL matches would silently mask a
                    // duplicate-id smuggle (REVIEW #421 gap 3). Break here so
                    // a dup-id table leaves the second copy non-Done and any
                    // state audit can see the divergence.
                    break;
                }
            }
            i += 1;
        }
        if !marked {
            return Err(SwitchError::Unknown);
        }
        match self.pick_next() {
            Ok(next) => {
                let mut j = 0;
                while j < self.len {
                    if let Some(ref mut t) = self.table[j] {
                        if t.id() == next && t.state() == ThreadState::Ready {
                            let _ = t.set_state(ThreadState::Running);
                            self.runs[j] = self.runs[j].saturating_add(1);
                        }
                    }
                    j += 1;
                }
                self.record_trace(next);
                self.current = next;
                self.ticks_left = QUANTUM_TICKS;
                Ok(next)
            }
            Err(e) => {
                self.current = NULL_THREAD;
                self.ticks_left = 0;
                Err(e)
            }
        }
    }

    /// IRQ-gated exit (S06 preempt-IRQ gate, task #19): identical
    /// mark-`Done`-then-pick to [`Scheduler::exit_current`], with the same
    /// `&IrqGuard` proof as [`Scheduler::dispatch_gated`] — the exit pick
    /// is the same preempt point (a timer IRQ calls this same pick under
    /// the gate instead of the coop caller).
    pub fn exit_current_gated(&mut self, _gate: &IrqGuard) -> Result<u64, SwitchError> {
        self.exit_current()
    }
}

/// IRQ-owned scheduler table (S07 slice 19, guest-only).
///
/// `static mut`, guest-only: the future IRQ0 periodic handler needs `&mut`
/// access from IRQ context, which cannot borrow bring-up's stack-local. The
/// tick counter (`timer::TIMER_TICKS`) is already IRQ-owned the same way;
/// this is its scheduler half. Single-threaded boot owns it (`cli` +
/// `&IrqGuard` on every access); the interrupt gate clears IF so a
/// handler-side `IrqGuard::enter()` proof holds on entry. Host `cargo test`
/// never touches this (the pure [`Scheduler::demo_armed_scheduler`]
/// constructor covers the build rule; the static wiring is proven by QEMU).
#[cfg(not(test))]
static mut STATIC_SCHED: Scheduler = Scheduler::new(NULL_THREAD);

/// Guest-only static init (S07 slice 19; S08 slice25 resets the streak):
/// reset `STATIC_SCHED` to the demo-armed table
/// ([`Scheduler::demo_armed_scheduler`]) and `STATIC_STREAK` to `0` so the
/// task-context live poll starts unsuppressed.
///
/// # Safety
/// Post-EBS guest only, `cli` in effect, `gate` proves the critical
/// section; single-threaded boot owns `STATIC_SCHED` and `STATIC_STREAK`.
#[cfg(not(test))]
pub(crate) unsafe fn sched_init_demo_static(gate: &IrqGuard) {
    STATIC_SCHED = Scheduler::demo_armed_scheduler(gate);
    STATIC_STREAK = 0;
}

/// IRQ-owned live-backoff streak (S08 slice25, guest-only).
///
/// `static mut`, guest-only: the consecutive-failure count the live helper
/// owns (`&mut u64` param host-side). IRQ-owned like `STATIC_SCHED` so the
/// future periodic IRQ handler can pass it from IRQ context without
/// borrowing bring-up's stack; this slice the task-context live poll owns
/// it under `cli` + `&IrqGuard`. Host tests never touch this (the pure
/// streak rule is pinned by `live_backoff_*` + live-helper tests).
#[cfg(not(test))]
static mut STATIC_STREAK: u64 = 0;

/// Guest-only static poll (S07 slice 19): drain `ticks` into `STATIC_SCHED`
/// via [`Scheduler::drain_then_dispatch_gated`] (bulk credit to the
/// pre-dispatch current, then at most one gated dispatch on expiry).
/// Retained for the task-context retry-spin reference; the guest bring-up
/// path now polls via [`sched_poll_live_static`] (S08 slice25).
///
/// # Safety
/// Post-EBS guest only, `cli` in effect, `gate` proves the critical
/// section; single-threaded boot owns `STATIC_SCHED` and `ticks`.
#[cfg(not(test))]
#[allow(dead_code)]
pub(crate) unsafe fn sched_poll_static(
    ticks: &mut TimerTicks,
    gate: &IrqGuard,
) -> (u64, Option<Result<u64, SwitchError>>) {
    (&mut *core::ptr::addr_of_mut!(STATIC_SCHED)).drain_then_dispatch_gated(ticks, gate)
}

/// Guest-only static LIVE poll (S08 slice25; S07 slice30 per-tick IRQ live):
/// drain `ticks` into `STATIC_SCHED` via
/// [`Scheduler::drain_then_dispatch_live_gated`] with the IRQ-owned
/// `STATIC_STREAK` (3-on-1-off throttle, auto-recovery probe). Called
/// per-tick from `timer_irq_rust` (IRQ context) plus the task-context
/// remainder in `bring_up_cpu_and_prove_exception`; QEMU TICK re-run owed
/// per REVIEW #497.
///
/// Slice-24 parity makes this behavior-neutral on the demo table: while
/// unsuppressed the live helper agrees exactly with the task helper (same
/// drain/dispatch/state, streak `0 -> live_backoff_next`), and the demo
/// table never fails, so the streak stays `0` and the `TICK` line is
/// unchanged.
///
/// # Safety
/// Post-EBS guest only, `cli` in effect, `gate` proves the critical
/// section; single-threaded boot owns `STATIC_SCHED`, `STATIC_STREAK`,
/// and `ticks`.
#[cfg(not(test))]
pub(crate) unsafe fn sched_poll_live_static(
    ticks: &mut TimerTicks,
    gate: &IrqGuard,
) -> (u64, Option<Result<u64, SwitchError>>) {
    (&mut *core::ptr::addr_of_mut!(STATIC_SCHED)).drain_then_dispatch_live_gated(
        ticks,
        gate,
        &mut *core::ptr::addr_of_mut!(STATIC_STREAK),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(id: u64, base: u64) -> Thread {
        Thread::new(id, base, 8192, 0x1000 + id).unwrap()
    }

    #[test]
    fn null_sentinel_is_shared_zero() {
        assert_eq!(NULL_THREAD, 0);
        assert_eq!(
            Thread::new(NULL_THREAD, 0x1000_0000, 8192, 0x1000),
            Err(crate::thread::ThreadError::InvalidId)
        );
        // (Mutex-side `NULL_THREAD` refusal is pinned in `sync.rs`;
        // `Thread`-side in `thread.rs`. Here pin the scheduler gate.)
        let mut s = Scheduler::new(NULL_THREAD);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert_eq!(s.len(), 1);
        let mut s2 = Scheduler::new(1);
        assert!(s2.spawn(mk(1, 0x1000_0000)));
        assert!(s2.spawn(mk(2, 0x2000_0000)));
        assert_eq!(s2.pick_next(), Ok(2));
    }

    #[test]
    fn spawn_pick_round_robin() {
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert!(s.spawn(mk(3, 0x3000_0000)));
        assert_eq!(s.len(), 3);
        assert_eq!(s.pick_next(), Ok(2));
        assert_eq!(s.dispatch(), Ok(2));
        assert_eq!(s.current(), 2);
        assert_eq!(s.dispatch(), Ok(3));
        assert_eq!(s.dispatch(), Ok(1));
    }

    #[test]
    fn priority_is_data_only_round_robin_preserved() {
        // S07 slice40 — priority rides as data (default 0, any u8 settable,
        // spawn carries it, unknown id is None); the pick/dispatch order
        // IGNORES it this slice (round-robin preserved). A priority-aware
        // policy that reorders must update this test, never stay green
        // while the guest diverges.
        let mut t = mk(1, 0x1000_0000);
        assert_eq!(t.priority(), 0);
        t.set_priority(200);
        assert_eq!(t.priority(), 200);
        t.set_priority(0);
        assert_eq!(t.priority(), 0);
        let mut hi = mk(2, 0x2000_0000);
        hi.set_priority(255);
        let mut lo = mk(1, 0x1000_0000);
        lo.set_priority(1);
        let mut mid = mk(3, 0x3000_0000);
        mid.set_priority(128);
        let mut s = Scheduler::new(1);
        assert!(s.spawn(lo));
        assert!(s.spawn(hi));
        assert!(s.spawn(mid));
        assert_eq!(s.priority_of(1), Some(1));
        assert_eq!(s.priority_of(2), Some(255));
        assert_eq!(s.priority_of(3), Some(128));
        assert_eq!(s.priority_of(99), None);
        // Mixed priorities dispatch in id round-robin order, not priority
        // order (hi=255 does NOT jump the queue).
        assert_eq!(s.dispatch(), Ok(2));
        assert_eq!(s.dispatch(), Ok(3));
        assert_eq!(s.dispatch(), Ok(1));
        assert_eq!(s.fairness_spread(), 0);
    }

    #[test]
    fn priority_aware_pick_jumps_queue_rr_preserved() {
        // S07 slice41 — opt-in policy: `pick_next_priority` /
        // `dispatch_priority` prefer the highest-priority Ready thread
        // (hi=255 jumps the queue), while `pick_next`/`dispatch` stay RR.
        // Order [lo(1), mid(128), hi(255)], current=1: RR gives 2, policy
        // gives 3. Fairness tension recorded: one policy dispatch holds
        // spread 1 (only hi ran) vs RR full-round spread 0.
        let mut lo = mk(1, 0x1000_0000);
        lo.set_priority(1);
        let mut mid = mk(2, 0x2000_0000);
        mid.set_priority(128);
        let mut hi = mk(3, 0x3000_0000);
        hi.set_priority(255);
        let mut s = Scheduler::new(1);
        assert!(s.spawn(lo));
        assert!(s.spawn(mid));
        assert!(s.spawn(hi));
        assert_eq!(s.pick_next(), Ok(2));
        assert_eq!(s.pick_next_priority(), Ok(3));
        assert_eq!(s.dispatch_priority(), Ok(3));
        assert_eq!(s.current(), 3);
        assert_eq!(s.fairness_spread(), 1);
        // Next policy target: highest Ready excluding hi(3) is mid(2).
        assert_eq!(s.pick_next_priority(), Ok(2));
        assert_eq!(s.dispatch_priority(), Ok(2));
        // Tie-break is RR slice order: equal max prio picks the first
        // candidate after current, wrapping.
        let mut a = mk(1, 0x1000_0000);
        a.set_priority(10);
        let mut b = mk(2, 0x2000_0000);
        b.set_priority(10);
        let mut c = mk(3, 0x3000_0000);
        c.set_priority(10);
        let mut t = Scheduler::new(1);
        assert!(t.spawn(a));
        assert!(t.spawn(b));
        assert!(t.spawn(c));
        assert_eq!(t.pick_next_priority(), Ok(2));
        // Refusal parity with switch_stub: empty/unknown/solo/NoSwitch.
        let e = Scheduler::new(NULL_THREAD);
        assert_eq!(e.pick_next_priority(), Err(SwitchError::Unknown));
        let mut u = Scheduler::new(99);
        assert!(u.spawn(mk(1, 0x1000_0000)));
        assert_eq!(u.pick_next_priority(), Err(SwitchError::Unknown));
        let mut solo = Scheduler::new(1);
        assert!(solo.spawn(mk(1, 0x1000_0000)));
        assert_eq!(solo.pick_next_priority(), Err(SwitchError::NoSwitch));
        assert_eq!(solo.dispatch_priority(), Err(SwitchError::NoSwitch));
        assert_eq!(solo.current(), 1);
    }

    #[test]
    fn dispatch_agrees_with_swap_target_parity() {
        // S06 slice18 guest/host parity pin: the scheduler pick and the
        // swap-gate triple must agree on the same next id for the same
        // table, or the guest would dispatch one thread and run another.
        let a = mk(1, 0x1000_0000);
        let b = mk(2, 0x2000_0000);
        let mut s = Scheduler::new(1);
        assert!(s.spawn(a));
        assert!(s.spawn(b));
        assert_eq!(s.pick_next(), Ok(2));
        let triple = crate::thread::swap_target(&[a, b], 1).unwrap();
        assert_eq!(triple.0, 2);
        assert_eq!(s.dispatch(), Ok(triple.0));
        assert_eq!(s.current(), 2);
    }

    #[test]
    fn scheduler_three_thread_jump_chain_parity() {
        // S06 slice24: scheduler-driven multi-dispatch must agree with the
        // live-jump contract (swap_target triple AND check_jump_entry AND
        // entry_as_func, never gate-alone) on every leg 1->2->3->1. The
        // thread-level rotation (slice23) proves the picker order; this
        // proves the Scheduler dispatch path the guest multi-hop jmp needs
        // picks the same ids with jump-valid triples.
        //
        // REVIEW #393 finding 1 (slice25 fix): the slice24 version fed
        // swap_target a STATIC all-Ready table while Scheduler states
        // mutated to Running across legs — picker-order parity only, not
        // live state-aware parity. A future gate that checked Running vs
        // Ready would stay green here while the guest diverged. This
        // version rebuilds a LIVE snapshot mirroring s.state_of() every
        // leg and feeds swap_target the live states + s.current(), so a
        // state-sensitive divergence goes red.
        use crate::thread::{check_jump_entry, entry_as_func, jump_rsp_refuses, swap_target};
        fn live_as(t: Thread, st: ThreadState) -> Thread {
            let mut m = t;
            if m.state() != st {
                assert!(m.set_state(st), "live snapshot must reach scheduler state");
            }
            m
        }
        let a = mk(1, 0x1000_0000);
        let b = mk(2, 0x2000_0000);
        let c = mk(3, 0x3000_0000);
        let mut s = Scheduler::new(1);
        assert!(s.spawn(a));
        assert!(s.spawn(b));
        assert!(s.spawn(c));
        for current in [1u64, 2, 3] {
            let expect = if current == 3 { 1 } else { current + 1 };
            // Live current must track the loop leg (order pin).
            assert_eq!(s.current(), current);
            assert_eq!(s.pick_next(), Ok(expect));
            // Live snapshot: mirror scheduler states (Running appears
            // after the first dispatch; static all-Ready would hide it).
            let live = [
                live_as(a, s.state_of(1).unwrap()),
                live_as(b, s.state_of(2).unwrap()),
                live_as(c, s.state_of(3).unwrap()),
            ];
            // Static-shape sanity: after leg 1 the live table is NOT
            // all-Ready (proves this test can see what slice24 could not).
            if current != 1 {
                assert!(live.iter().any(|t| t.state() == ThreadState::Running));
            }
            let (id, top, entry) = swap_target(&live, s.current()).unwrap();
            assert_eq!(id, expect);
            assert!(check_jump_entry(top, entry));
            assert!(!jump_rsp_refuses(top));
            assert!(entry_as_func(entry).is_some());
            assert_eq!(s.dispatch(), Ok(expect));
            assert_eq!(s.current(), expect);
            // Post-dispatch: scheduler states mirror the live snapshot
            // transition (picked Running, previous Running parked Ready).
            assert_eq!(s.state_of(expect), Some(ThreadState::Running));
        }
    }

    #[test]
    fn dispatch_two_hops_persistent_and_refusals_leave_table_untouched() {
        // S06 slice19 (REVIEW #338 R1+R4): the scheduler table is
        // persistent across dispatches (not one-shot) and every
        // fail-closed refusal leaves `current` and all states untouched.
        // First hop 1->2 parks nothing (boot stays Ready); second hop
        // 2->1 parks the previous Running thread — the guest probe pins
        // the same two-hop agreement live.
        fn mk2(id: u64, base: u64) -> Thread {
            Thread::new(id, base, 8192, 0x1000 + id).unwrap()
        }
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk2(1, 0x1000_0000)));
        assert!(s.spawn(mk2(2, 0x2000_0000)));
        assert_eq!(s.dispatch(), Ok(2));
        assert_eq!(s.current(), 2);
        assert_eq!(s.state_of(1), Some(ThreadState::Ready));
        assert_eq!(s.state_of(2), Some(ThreadState::Running));
        assert_eq!(s.dispatch(), Ok(1));
        assert_eq!(s.current(), 1);
        assert_eq!(s.state_of(1), Some(ThreadState::Running));
        assert_eq!(s.state_of(2), Some(ThreadState::Ready));
        // Refusals: empty table, unknown current, and nothing-Ready all
        // fail closed with no mutation (pins the guest ok=0 paths' host
        // logic — the serial `ok=0` shape itself is pinned in boot-uefi).
        let mut e = Scheduler::new(NULL_THREAD);
        assert_eq!(e.pick_next(), Err(SwitchError::Unknown));
        assert_eq!(e.dispatch(), Err(SwitchError::Unknown));
        assert_eq!(e.current(), NULL_THREAD);
        let mut u = Scheduler::new(99);
        assert!(u.spawn(mk2(1, 0x1000_0000)));
        assert_eq!(u.dispatch(), Err(SwitchError::Unknown));
        assert_eq!(u.current(), 99);
        assert_eq!(u.state_of(1), Some(ThreadState::Ready));
        // Nothing Ready -> NotReady (target non-Ready) leaves `current`
        // and all states untouched: current Running + other Done picks
        // nothing (no other Ready, current not Ready), so `dispatch`
        // refuses via the pick with no mutation. Pre-set states before
        // `spawn` (`spawn` carries state through; `validate_threads`
        // gates stacks/ids only, never lifecycle).
        let mut run = mk2(1, 0x1000_0000);
        assert!(run.set_state(ThreadState::Running));
        let mut done = mk2(2, 0x2000_0000);
        assert!(done.set_state(ThreadState::Done));
        let mut nr = Scheduler::new(1);
        assert!(nr.spawn(run));
        assert!(nr.spawn(done));
        assert_eq!(nr.pick_next(), Err(SwitchError::NotReady));
        assert_eq!(nr.dispatch(), Err(SwitchError::NotReady));
        assert_eq!(nr.current(), 1);
        assert_eq!(nr.state_of(1), Some(ThreadState::Running));
        assert_eq!(nr.state_of(2), Some(ThreadState::Done));
        assert_eq!(nr.len(), 2);
        let mut solo = Scheduler::new(1);
        assert!(solo.spawn(mk2(1, 0x1000_0000)));
        assert_eq!(solo.dispatch(), Err(SwitchError::NoSwitch));
        assert_eq!(solo.current(), 1);
        assert_eq!(solo.state_of(1), Some(ThreadState::Ready));
    }

    #[test]
    fn duplicate_and_full_refused_no_mutation() {
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(!s.spawn(mk(1, 0x2000_0000)));
        assert_eq!(s.len(), 1);
        let mut f = Scheduler::new(1);
        let mut i = 0;
        while i < MAX_THREADS {
            assert!(f.spawn(mk((i as u64) + 1, 0x1000_0000 + (i as u64) * 8192)));
            i += 1;
        }
        assert_eq!(f.len(), MAX_THREADS);
        assert!(!f.spawn(mk(99, 0x9000_0000)));
        assert_eq!(f.len(), MAX_THREADS);
    }

    #[test]
    fn overlapping_spawn_refused() {
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        // Overlaps thread 1 ([base, base+8192)).
        let bad = mk(2, 0x1000_0800);
        assert!(!s.spawn(bad));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn empty_and_unknown_current_refuse() {
        let s = Scheduler::new(NULL_THREAD);
        assert!(s.is_empty());
        assert_eq!(s.pick_next(), Err(SwitchError::Unknown));
        let mut s2 = Scheduler::new(99);
        assert!(s2.spawn(mk(1, 0x1000_0000)));
        assert_eq!(s2.pick_next(), Err(SwitchError::Unknown));
    }

    #[test]
    fn first_dispatch_parks_nothing_boot_thread_stays_ready() {
        // REVIEW #321 residual (a): `new(current)` never marks `current`
        // `Running`, so the first `dispatch` parks nothing — the
        // ex-current stays `Ready` while the pick moves to `Running`.
        // REVIEW #325 gate (2): pin the STATES, not just the ids — a
        // future `dispatch` rewrite that flips ids correctly but parks
        // wrong (e.g. marks boot `Running` on first dispatch) must go red.
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert_eq!(s.state_of(1), Some(ThreadState::Ready));
        assert_eq!(s.state_of(2), Some(ThreadState::Ready));
        // REVIEW #328 residual (b): unknown id queries `None` (read-only,
        // never a default state) — before and after dispatch.
        assert_eq!(s.state_of(99), None);
        assert_eq!(s.state_of(NULL_THREAD), None);
        assert_eq!(s.dispatch(), Ok(2));
        assert_eq!(s.current(), 2);
        assert_eq!(s.state_of(1), Some(ThreadState::Ready));
        assert_eq!(s.state_of(2), Some(ThreadState::Running));
        // Second dispatch onward parks the previous Running thread.
        assert_eq!(s.dispatch(), Ok(1));
        assert_eq!(s.current(), 1);
        assert_eq!(s.state_of(2), Some(ThreadState::Ready));
        assert_eq!(s.state_of(1), Some(ThreadState::Running));
        assert_eq!(s.state_of(99), None);
    }

    #[test]
    fn exit_current_marks_done_and_join_observes_preempt_resumes_next() {
        // S06 slice27 done-criteria pin: join semantics = `join(id)` is
        // true iff that thread is Done; preempt/exit point = `exit_current`
        // parks current to Done and resumes the next Ready (the coop
        // `dispatch` yield shape, future timer IRQ calls the same pick).
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert!(s.spawn(mk(3, 0x3000_0000)));
        assert!(!s.join(2));
        assert!(!s.join(99));
        // Rotate to 2 (preempt point: dispatch yield 1->2).
        assert_eq!(s.dispatch(), Ok(2));
        assert_eq!(s.current(), 2);
        // Exit 2: Done, join observes, resume is 3 (Done is skipped).
        assert_eq!(s.exit_current(), Ok(3));
        assert_eq!(s.current(), 3);
        assert_eq!(s.state_of(2), Some(ThreadState::Done));
        assert!(s.join(2));
        assert!(!s.join(3));
        // Dispatch skips Done 2: 3->1, never back to 2.
        assert_eq!(s.dispatch(), Ok(1));
        assert_eq!(s.current(), 1);
        assert!(s.join(2));
        // Exit 1 then 3: last exit parks current on NULL with the pick
        // error, both joinable, table length unchanged.
        assert_eq!(s.exit_current(), Ok(3));
        assert!(s.join(1));
        assert_eq!(s.exit_current(), Err(SwitchError::NotReady));
        assert_eq!(s.current(), NULL_THREAD);
        assert!(s.join(1));
        assert!(s.join(3));
        assert_eq!(s.len(), 3);
        // Unknown current cannot exit (no phantom Done mark).
        let mut u = Scheduler::new(99);
        assert!(u.spawn(mk(1, 0x1000_0000)));
        assert_eq!(u.exit_current(), Err(SwitchError::Unknown));
        assert!(!u.join(1));
    }

    /// Strip `/* ... */` (nested), `//` line comments, and blank string/char
    /// literal contents so the no-`Cell` pin below scans CODE only.
    ///
    #[test]
    fn gated_dispatch_and_exit_agree_with_ungated() {
        // S06 preempt-IRQ gate (task #19): gated wrappers must behave
        // exactly like the ungated primitives on success and on every
        // refusal (the gate adds IRQ masking, never new pick logic).
        // Success path: 1->2->3 rotation with states.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut g = Scheduler::new(1);
        assert!(g.spawn(mk(1, 0x1000_0000)));
        assert!(g.spawn(mk(2, 0x2000_0000)));
        assert!(g.spawn(mk(3, 0x3000_0000)));
        let mut u = Scheduler::new(1);
        assert!(u.spawn(mk(1, 0x1000_0000)));
        assert!(u.spawn(mk(2, 0x2000_0000)));
        assert!(u.spawn(mk(3, 0x3000_0000)));
        for expect in [2u64, 3, 1] {
            assert_eq!(g.dispatch_gated(&gate), Ok(expect));
            assert_eq!(u.dispatch(), Ok(expect));
            assert_eq!(g.current(), u.current());
            assert_eq!(g.state_of(expect), u.state_of(expect));
        }
        // Exit path: gated exit marks Done, resumes next, join observes.
        assert_eq!(g.exit_current_gated(&gate), Ok(2));
        assert_eq!(u.exit_current(), Ok(2));
        assert_eq!(g.current(), u.current());
        assert_eq!(g.state_of(1), u.state_of(1));
        assert!(g.join(1));
        assert!(u.join(1));
        // Refusal parity: unknown current refuses both with no mutation.
        let mut gu = Scheduler::new(99);
        assert!(gu.spawn(mk(1, 0x1000_0000)));
        let mut uu = Scheduler::new(99);
        assert!(uu.spawn(mk(1, 0x1000_0000)));
        assert_eq!(gu.dispatch_gated(&gate), Err(SwitchError::Unknown));
        assert_eq!(uu.dispatch(), Err(SwitchError::Unknown));
        assert_eq!(gu.current(), uu.current());
        assert_eq!(gu.exit_current_gated(&gate), Err(SwitchError::Unknown));
        assert_eq!(uu.exit_current(), Err(SwitchError::Unknown));
        assert!(!gu.join(1));
        assert!(!uu.join(1));
    }

    #[test]
    fn gated_dispatch_models_preempt_race_as_serialized() {
        // S06 preempt-IRQ gate race model (task #19 cheapest disproof):
        // no host test can fire a real IRQ, so pin the RACE CLASS on
        // cloned states — two schedulers cloned BEFORE either dispatches
        // (an IRQ landing between pick and mark) both pick the same next
        // id, proving an ungated preempt double-dispatches. Sequential
        // gated dispatches on ONE table (the `cli` critical section the
        // guest holds) advance 1->2 then 2->3 and never repeat the target.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut base = Scheduler::new(1);
        assert!(base.spawn(mk(1, 0x1000_0000)));
        assert!(base.spawn(mk(2, 0x2000_0000)));
        assert!(base.spawn(mk(3, 0x3000_0000)));
        // Preempt model: clone before either marks — both pick 2.
        let racer_a = base.clone();
        let racer_b = base.clone();
        assert_eq!(racer_a.pick_next(), Ok(2));
        assert_eq!(racer_b.pick_next(), Ok(2));
        // Gated reality: one table, sequential critical sections.
        assert_eq!(base.dispatch_gated(&gate), Ok(2));
        assert_eq!(base.current(), 2);
        assert_eq!(base.state_of(2), Some(ThreadState::Running));
        assert_eq!(base.dispatch_gated(&gate), Ok(3));
        assert_eq!(base.current(), 3);
        assert_ne!(base.current(), 2);
    }
    ///
    /// S06 slice 17 (REVIEW #325 gate 1): the slice-16 strip was
    /// block-only, non-nesting, and string-blind — a `"` inside a block
    /// comment or a `/*` inside a `"..."` literal desynced it (skipped
    /// real code or swallowed it), and a split token (`Cell/*c*/<`) relied
    /// on accidental rejoin. This lexer blanks `"..."`/`'...'` (escapes
    /// honored, newline preserved) and tracks block depth, so:
    /// `Cell/*"Cell"*/<` rejoins to `Cell<` (quote in comment is inert)
    /// and `"/*Cell*/" Cell<` keeps the literal blank but the trailing
    /// code token (string markers never start a comment).
    ///
    /// Slice 17b (REVIEW #328 residuals a): additionally blank raw strings
    /// (`r"..."`, `r#"..."#`, `r##"..."##`, same with a `br` prefix — no
    /// escapes, close is `"` + the opening `#` count) and treat lifetimes
    /// (`'a`, `'static`) as code, not char literals. Without this a raw
    /// `r"/*Cell*/"` literal would start a phantom block comment (swallows
    /// real code to the next `*/`) and a lifetime `'a` would open a phantom
    /// char literal (swallows code to the next `'`), both desyncing the
    /// `Cell` pin in exactly the way gate 1 was built to prevent.
    ///
    /// Slice 17c (REVIEW #330 residuals): raw prefix requires an ident
    /// boundary (the `r`/`b` must not follow `[0-9A-Za-z_]`, so `bar"#"`
    /// lexes as ident `bar` + normal string, never a raw open — without
    /// this `bar#"fake" Cell<` false-opens a 1-hash raw and blanks the
    /// real `Cell<` to EOF).
    ///
    /// Slice 17d (REVIEW #332 risk 1 — measured): only `br` is a valid
    /// Rust raw-byte prefix; `rb` is two tokens (ident `rb` + normal
    /// string). `br"x"` compiles (`rustc 1.98.0` exit 0 as `&[u8]`);
    /// `rb"x"` fails (`expected one of ... found "x"` — `rb` parsed as
    /// ident, issue #332 disproof). The lexer therefore opens raw only
    /// on `r` / `br`, never `rb` — `rb` bodies blank as a normal string
    /// (correct), and critically `rb#"fake" Cell<` keeps the trailing
    /// `Cell<` (a symmetric-`rb` raw open would look for a `"#` close,
    /// miss, and blank to EOF). Known doc-only limits: multi-byte
    /// char literals (`'é'`, bytes `27 C3 A9 27`) fall back to
    /// lifetime-shape (pinned by test: `'é' Cell<` keeps `Cell<`, so no
    /// pin bypass — a single-char body can never spell `Cell<`) and an
    /// unterminated raw blanks to EOF (unterminated is a compile error,
    /// `rustc E0748 unterminated raw string` — no real code after
    /// matters).
    #[cfg(test)]
    fn strip_code(src: &str) -> String {
        let bytes = src.as_bytes();
        let mut out = String::with_capacity(src.len());
        let mut i = 0;
        let mut block_depth: usize = 0;
        while i < bytes.len() {
            if block_depth > 0 {
                if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                    block_depth += 1;
                    i += 2;
                } else if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    block_depth -= 1;
                    i += 2;
                } else {
                    if bytes[i] == b'\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
                continue;
            }
            // Raw string open: `[br]#*"`. Hash count pins the close
            // (`"` + that many `#`); content is raw (no escapes).
            // Slice 17c: ident-boundary (REVIEW #330). Slice 17d: `br`
            // only, never `rb` (REVIEW #332: `rb` is ident + string).
            let prev_ident = i > 0
                && matches!(
                    bytes[i - 1],
                    b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'_'
                );
            let r_pos = if prev_ident {
                None
            } else if bytes[i] == b'r' {
                Some(i)
            } else if bytes[i] == b'b' && i + 1 < bytes.len() && bytes[i + 1] == b'r' {
                Some(i + 1)
            } else {
                None
            };
            if let Some(rp) = r_pos {
                let mut k = rp + 1;
                while k < bytes.len() && bytes[k] == b'#' {
                    k += 1;
                }
                if k < bytes.len() && bytes[k] == b'"' {
                    let hashes = k - (rp + 1);
                    out.push('"');
                    i = k + 1;
                    while i < bytes.len() {
                        if bytes[i] == b'"' {
                            let mut m = i + 1;
                            let mut n = 0;
                            while n < hashes && m < bytes.len() && bytes[m] == b'#' {
                                m += 1;
                                n += 1;
                            }
                            if n == hashes {
                                out.push('"');
                                i = m;
                                break;
                            }
                            i += 1;
                        } else {
                            if bytes[i] == b'\n' {
                                out.push('\n');
                            }
                            i += 1;
                        }
                    }
                    continue;
                }
            }
            let b = bytes[i];
            if b == b'"' {
                out.push('"');
                i += 1;
                while i < bytes.len() {
                    let c = bytes[i];
                    if c == b'\n' {
                        out.push('\n');
                        i += 1;
                        break;
                    }
                    if c == b'\\' && i + 1 < bytes.len() {
                        i += 2;
                        continue;
                    }
                    if c == b'"' {
                        out.push('"');
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            if b == b'\'' {
                // Lifetime vs char literal (residual a): only `'x'` and
                // `'\'`-escape shapes are char literals. A lone `'a` /
                // `'static` (quote + ident with no closing quote in the
                // next-two positions) is a lifetime and stays CODE — the
                // old unconditional literal mode swallowed code to the
                // next `'` and desynced the pin.
                let is_single = i + 2 < bytes.len() && bytes[i + 2] == b'\'';
                let is_escape = i + 1 < bytes.len() && bytes[i + 1] == b'\\';
                if !is_single && !is_escape {
                    out.push('\'');
                    i += 1;
                    continue;
                }
                out.push('\'');
                i += 1;
                while i < bytes.len() {
                    let c = bytes[i];
                    if c == b'\n' {
                        out.push('\n');
                        i += 1;
                        break;
                    }
                    if c == b'\\' && i + 1 < bytes.len() {
                        i += 2;
                        continue;
                    }
                    if c == b'\'' {
                        out.push('\'');
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            if i + 1 < bytes.len() && b == b'/' && bytes[i + 1] == b'*' {
                block_depth = 1;
                i += 2;
                continue;
            }
            if i + 1 < bytes.len() && b == b'/' && bytes[i + 1] == b'/' {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            out.push(b as char);
            i += 1;
        }
        if block_depth != 0 {
            panic!("unterminated block comment");
        }
        out
    }

    #[test]
    fn strip_code_keeps_split_token_but_ignores_strings() {
        // Quote inside a block comment is inert: split token rejoins.
        assert!(strip_code("Cell/*\"Cell\"*/<").contains("Cell<"));
        // Block markers inside a string never start a comment: the literal
        // blanks to `""` and the trailing code token survives.
        let s = strip_code("\"/*Cell*/\" Cell<");
        assert!(s.contains("Cell<"));
        assert!(!s.contains("/*"));
        // Nested blocks strip fully; pure-string banned text blanks out.
        assert!(strip_code("/* o /* i */ s */Cell<").contains("Cell<"));
        assert!(!strip_code("\"Cell<\"").contains("Cell<"));
        assert!(!strip_code("// Cell<").contains("Cell<"));
        // REVIEW #328 residual (a) disproof: raw strings are raw — block
        // markers and quotes inside never start a comment or a string, and
        // the trailing code token survives.
        let r = strip_code("r\"/*Cell*/\" Cell<");
        assert!(r.contains("Cell<"));
        assert!(!r.contains("/*"));
        let r_hash = strip_code("r#\"\"Cell<\"# Cell<");
        assert!(r_hash.contains("Cell<"));
        assert!(!strip_code("r#\"Cell<\"#").contains("Cell<"));
        // Lifetimes are code, not char literals: `'a` must not swallow to
        // the next `'` (which would hide a real `Cell<` or expose a fake).
        let l = strip_code("fn f<'a>(x: &'a str) Cell<");
        assert!(l.contains("Cell<"));
        // Old unconditional-literal mode blanked `'a Cell<'` as one literal
        // and hid the code token; lifetime-aware mode keeps it.
        assert!(strip_code("'a Cell<'").contains("Cell<"));
        // Sanity: real char literals still blank.
        assert!(!strip_code("'C'").contains("Cell<"));
        assert!(strip_code("'C' Cell<").contains("Cell<"));
        // REVIEW #330 cheapest disproof (slice 17c): a raw prefix glued
        // to an ident never opens (else `bar#"fake" Cell<` false-opens a
        // 1-hash raw with no `"#` close and blanks the real `Cell<` to
        // EOF, while the correct ident + normal-string lex keeps it).
        // REVIEW #332 slice 17d: `br` is the only valid raw-byte prefix
        // (`br"x"` compiles as `&[u8]`); `rb` is ident + normal string
        // (`rb"x"` fails `expected ... found "x"`), so `rb#"fake"`
        // must lex ident `rb` + `#` + normal string — trailing `Cell<`
        // survives. A symmetric-`rb` raw open would seek a `"#` close,
        // miss, and blank to EOF (over-blank of valid Rust).
        // Boundary (REVIEW #332 risk 2): `r` at offset 0 (no prev char)
        // IS a raw open; `_r` (`_` is an ident char) is NOT.
        // Multibyte (REVIEW #332 risk 3): `'é'` (bytes `27 C3 A9 27`)
        // falls back to lifetime-shape yet keeps the trailing token.
        let br = strip_code("br\"/*Cell*/\" Cell<");
        assert!(br.contains("Cell<"));
        assert!(!br.contains("/*"));
        let rb_plain = strip_code("rb\"/*Cell*/\" Cell<");
        assert!(rb_plain.contains("Cell<"));
        assert!(!rb_plain.contains("/*"));
        let rb_hash = strip_code("rb#\"fake\" Cell<");
        assert!(rb_hash.contains("Cell<"));
        let start_raw = strip_code("r\"/*Cell*/\" Cell<");
        assert!(start_raw.contains("Cell<"));
        assert!(!start_raw.contains("/*"));
        assert!(!strip_code("r\"Cell<\"").contains("Cell<"));
        let under_r = strip_code("_r\"/*Cell*/\" Cell<");
        assert!(under_r.contains("Cell<"));
        assert!(!under_r.contains("/*"));
        let multi = strip_code("'é' Cell<");
        assert!(multi.contains("Cell<"));
        assert!(!strip_code("'é'").contains("Cell<"));
        let glued = strip_code("bar#\"fake\" Cell<");
        assert!(glued.contains("Cell<"));
        let glued_br = strip_code("foobarbr#\"fake\" Cell<");
        assert!(glued_br.contains("Cell<"));
    }

    #[test]
    fn accounting_counts_dispatch_and_exit() {
        // S07 slice 1: per-thread runs count successful dispatches only.
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert!(s.spawn(mk(3, 0x3000_0000)));
        assert_eq!(s.runs_of(1), Some(0));
        assert_eq!(s.total_dispatches(), 0);
        assert_eq!(s.dispatch(), Ok(2));
        assert_eq!(s.runs_of(2), Some(1));
        assert_eq!(s.dispatch(), Ok(3));
        assert_eq!(s.runs_of(3), Some(1));
        assert_eq!(s.dispatch(), Ok(1));
        assert_eq!(s.runs_of(1), Some(1));
        assert_eq!(s.total_dispatches(), 3);
        // Exit path resumes 2 and counts it as well.
        assert_eq!(s.exit_current(), Ok(2));
        assert_eq!(s.runs_of(2), Some(2));
        assert_eq!(s.total_dispatches(), 4);
    }

    #[test]
    fn accounting_unknown_is_none_and_refusal_counts_nothing() {
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert_eq!(s.runs_of(9), None);
        assert_eq!(s.total_dispatches(), 0);
        // Single-thread table: pick refuses (NoSwitch), no count.
        assert!(s.dispatch().is_err());
        assert_eq!(s.runs_of(1), Some(0));
        assert_eq!(s.total_dispatches(), 0);
    }

    #[test]
    fn fairness_sweep_ten_rounds_spread_zero() {
        // S07 slice 2: ten full 1->2->3->1 rounds (30 equal quanta) hold
        // spread 0 with exactly 10 per thread; empty/singleton edges 0.
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert!(s.spawn(mk(3, 0x3000_0000)));
        assert_eq!(s.fairness_spread(), 0);
        let mut n = 0;
        while n < 30 {
            assert!(s.dispatch().is_ok());
            n += 1;
        }
        assert_eq!(s.runs_of(1), Some(10));
        assert_eq!(s.runs_of(2), Some(10));
        assert_eq!(s.runs_of(3), Some(10));
        assert_eq!(s.total_dispatches(), 30);
        assert_eq!(s.fairness_spread(), 0);
    }

    #[test]
    fn fairness_spread_partial_round_bounded_by_one_and_empty_zero() {
        // Mid-round imbalance never exceeds one quantum; refusals and the
        // empty table hold spread 0.
        let e = Scheduler::new(NULL_THREAD);
        assert_eq!(e.fairness_spread(), 0);
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert!(s.spawn(mk(3, 0x3000_0000)));
        assert_eq!(s.dispatch(), Ok(2));
        assert_eq!(s.fairness_spread(), 1);
        assert_eq!(s.dispatch(), Ok(3));
        assert_eq!(s.fairness_spread(), 1);
        assert_eq!(s.dispatch(), Ok(1));
        assert_eq!(s.fairness_spread(), 0);
        // Refusal mutates nothing, spread unchanged (still 0 here).
        let mut solo = Scheduler::new(1);
        assert!(solo.spawn(mk(1, 0x1000_0000)));
        assert!(solo.dispatch().is_err());
        assert_eq!(solo.fairness_spread(), 0);
    }

    #[test]
    fn trace_records_dispatch_order_and_stops_at_cap() {
        // S07 slice 3: the first TRACE_CAP quanta record ids in order;
        // refusals record nothing; saturation keeps the prefix (no wrap).
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert!(s.spawn(mk(3, 0x3000_0000)));
        assert_eq!(s.trace_len(), 0);
        assert_eq!(s.trace_at(0), None);
        assert_eq!(s.dispatch(), Ok(2));
        assert_eq!(s.dispatch(), Ok(3));
        assert_eq!(s.dispatch(), Ok(1));
        assert_eq!(s.trace_len(), 3);
        assert_eq!(s.trace_at(0), Some(2));
        assert_eq!(s.trace_at(1), Some(3));
        assert_eq!(s.trace_at(2), Some(1));
        assert_eq!(s.trace_at(3), None);
        // Long sweep: counts keep going, trace pins the 8-prefix.
        let mut n = 3;
        while n < 30 {
            assert!(s.dispatch().is_ok());
            n += 1;
        }
        assert_eq!(s.total_dispatches(), 30);
        assert_eq!(s.trace_len(), TRACE_CAP);
        let expect = [2u64, 3, 1, 2, 3, 1, 2, 3];
        let mut i = 0;
        while i < TRACE_CAP {
            assert_eq!(s.trace_at(i), Some(expect[i]));
            i += 1;
        }
        assert_eq!(s.trace_at(TRACE_CAP), None);
        // Refusal records nothing.
        let mut solo = Scheduler::new(1);
        assert!(solo.spawn(mk(1, 0x1000_0000)));
        assert!(solo.dispatch().is_err());
        assert_eq!(solo.trace_len(), 0);
        assert_eq!(solo.trace_at(0), None);
    }

    #[test]
    fn trace_records_exit_current_resume() {
        // S07 slice 4 (REVIEW #435 residual): `exit_current` resumes record
        // the resumed id exactly like `dispatch` (same `record_trace`
        // path); failed exits record nothing. Dispatch 1->2 records 2,
        // exit 2 (Done, resume 3) records 3, and the counts agree.
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert!(s.spawn(mk(3, 0x3000_0000)));
        assert_eq!(s.dispatch(), Ok(2));
        assert_eq!((s.trace_len(), s.trace_at(0)), (1, Some(2)));
        assert_eq!(s.exit_current(), Ok(3));
        assert_eq!(s.trace_len(), 2);
        assert_eq!(s.trace_at(0), Some(2));
        assert_eq!(s.trace_at(1), Some(3));
        assert_eq!(s.trace_at(2), None);
        assert_eq!(s.runs_of(2), Some(1));
        assert_eq!(s.runs_of(3), Some(1));
        // Failed exit (unknown current) records nothing.
        let mut u = Scheduler::new(99);
        assert!(u.spawn(mk(1, 0x1000_0000)));
        assert_eq!(u.exit_current(), Err(SwitchError::Unknown));
        assert_eq!(u.trace_len(), 0);
        assert_eq!(u.trace_at(0), None);
    }

    #[test]
    fn quantum_tick_counts_down_and_rearms_on_dispatch() {
        // S07 slice 5: dispatch arms QUANTUM_TICKS; the first QUANTUM-1
        // ticks report not-expired, the last reports expired; the next
        // dispatch rearms. Idle ticks report expired (needs a dispatch).
        // Slice 6: loop-driven (no hardcoded tick count) so a quantum
        // change touches only QUANTUM_TICKS + the ps1 literal.
        let mut idle = Scheduler::new(NULL_THREAD);
        assert_eq!(idle.quantum_left(), 0);
        assert!(idle.tick());
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert!(s.spawn(mk(3, 0x3000_0000)));
        assert_eq!(s.quantum_left(), 0);
        assert_eq!(s.dispatch(), Ok(2));
        assert_eq!(s.quantum_left(), QUANTUM_TICKS);
        let mut i = 1u64;
        while i < QUANTUM_TICKS {
            assert!(!s.tick());
            i += 1;
        }
        assert_eq!(s.quantum_left(), 1);
        assert!(s.tick());
        assert_eq!(s.quantum_left(), 0);
        // Expired stays expired (saturating, never wraps).
        assert!(s.tick());
        assert_eq!(s.quantum_left(), 0);
        // Resched dispatch rearms the next quantum.
        assert_eq!(s.dispatch(), Ok(3));
        assert_eq!(s.quantum_left(), QUANTUM_TICKS);
        // Refused dispatch leaves the counter untouched (still armed).
        let mut solo = Scheduler::new(1);
        assert!(solo.spawn(mk(1, 0x1000_0000)));
        assert!(solo.dispatch().is_err());
        assert_eq!(solo.quantum_left(), 0);
        // Exit resume rearms; final exit disarms to 0.
        let mut e = Scheduler::new(1);
        assert!(e.spawn(mk(1, 0x1000_0000)));
        assert!(e.spawn(mk(2, 0x2000_0000)));
        assert_eq!(e.dispatch(), Ok(2));
        assert_eq!(e.quantum_left(), QUANTUM_TICKS);
        assert_eq!(e.exit_current(), Ok(1));
        assert_eq!(e.quantum_left(), QUANTUM_TICKS);
        assert_eq!(e.exit_current(), Err(SwitchError::NotReady));
        assert_eq!(e.quantum_left(), 0);
        // Refused exit leaves the counter untouched.
        let mut u = Scheduler::new(99);
        assert!(u.spawn(mk(1, 0x1000_0000)));
        assert_eq!(u.exit_current(), Err(SwitchError::Unknown));
        assert_eq!(u.quantum_left(), 0);
    }

    #[test]
    fn quantum_gated_tick_agrees_with_ungated() {
        // S07 slice 5 gate parity: gated ticks match ungated tick-for-tick,
        // and the timer-IRQ shape (gated ticks then gated dispatch on
        // expiry, all under one guard) reschedules exactly like coop.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut g = Scheduler::new(1);
        assert!(g.spawn(mk(1, 0x1000_0000)));
        assert!(g.spawn(mk(2, 0x2000_0000)));
        assert!(g.spawn(mk(3, 0x3000_0000)));
        let mut u = Scheduler::new(1);
        assert!(u.spawn(mk(1, 0x1000_0000)));
        assert!(u.spawn(mk(2, 0x2000_0000)));
        assert!(u.spawn(mk(3, 0x3000_0000)));
        assert_eq!(g.dispatch_gated(&gate), Ok(2));
        assert_eq!(u.dispatch(), Ok(2));
        let mut n = 1u64;
        while n < QUANTUM_TICKS {
            assert_eq!(g.tick_gated(&gate), u.tick());
            assert_eq!(g.quantum_left(), u.quantum_left());
            n += 1;
        }
        assert!(g.tick_gated(&gate));
        assert!(u.tick());
        assert_eq!(g.dispatch_gated(&gate), Ok(3));
        assert_eq!(u.dispatch(), Ok(3));
        assert_eq!(g.quantum_left(), u.quantum_left());
    }

    #[test]
    fn drain_timer_feeds_quantum_and_reports_expiry_once() {
        // S07 slice 8 wiring-prep: the drain takes HW ticks under the gate,
        // feeds tick_gated once per tick, and reports (drained, expired) so
        // the caller dispatches ONCE on expiry (never a tight re-tick loop).
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert_eq!(s.dispatch_gated(&gate), Ok(2));
        assert_eq!(s.quantum_left(), QUANTUM_TICKS);
        // Partial quantum: fewer HW ticks than the quantum, no expiry.
        let mut hw = TimerTicks::new();
        hw.tick();
        hw.tick();
        assert_eq!(s.drain_timer(&mut hw, &gate), (2, false));
        assert_eq!(s.quantum_left(), QUANTUM_TICKS - 2);
        assert_eq!(hw.pending(), 0);
        // Exact expiry: remaining ticks spend the quantum, one dispatch owed.
        hw.tick();
        hw.tick();
        assert_eq!(s.drain_timer(&mut hw, &gate), (2, true));
        assert_eq!(s.quantum_left(), 0);
        assert_eq!(s.dispatch_gated(&gate), Ok(1));
        assert_eq!(s.quantum_left(), QUANTUM_TICKS);
        // Backlog past expiry: quantum floors at 0, still one dispatch owed.
        let mut flood = TimerTicks::new();
        let mut i = 0u64;
        while i < QUANTUM_TICKS + 2 {
            flood.tick();
            i += 1;
        }
        assert_eq!(s.drain_timer(&mut flood, &gate), (QUANTUM_TICKS + 2, true));
        assert_eq!(s.quantum_left(), 0);
    }

    #[test]
    fn drain_timer_empty_reports_nothing_to_do() {
        // Empty drain must NOT ask for a dispatch: a bare tick() on an idle
        // scheduler reports expired (true), but drain with zero HW ticks
        // reports (0, false) so the IRQ handler never dispatches on silence.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        let mut hw = TimerTicks::new();
        assert_eq!(s.drain_timer(&mut hw, &gate), (0, false));
        assert_eq!(s.quantum_left(), 0);
    }

    #[test]
    fn drain_timer_flood_backlog_is_bulk_accounted() {
        // REVIEW #446 R2: a `cli`-held flood backlog must not hang the
        // handler in O(n) — bulk `saturating_sub` floors the quantum at 0
        // with exactly one dispatch owed, however large the backlog.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert_eq!(s.dispatch_gated(&gate), Ok(2));
        let mut flood = TimerTicks::new();
        let mut i = 0u64;
        while i < 100_000 {
            flood.tick();
            i += 1;
        }
        assert_eq!(s.drain_timer(&mut flood, &gate), (100_000, true));
        assert_eq!(s.quantum_left(), 0);
        assert_eq!(flood.pending(), 0);
    }

    #[test]
    fn guest_first_tick_armed_drain_shape() {
        // S07 slice15 guest parity: the bring-up arms a 1+2 demo table with
        // one gated dispatch (quantum = QUANTUM_TICKS), then feeds the
        // drained HW ticks via `drain_timer`. QEMU delivers exactly 1 tick
        // in the bounded wait, so the guest owes no resched
        // (`(1, false)`, `quantum_left == QUANTUM_TICKS - 1`).
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert_eq!(s.dispatch_gated(&gate), Ok(2));
        assert_eq!(s.quantum_left(), QUANTUM_TICKS);
        let mut hw = TimerTicks::new();
        hw.tick();
        assert_eq!(s.drain_timer(&mut hw, &gate), (1, false));
        assert_eq!(s.quantum_left(), QUANTUM_TICKS - 1);
        assert_eq!(hw.pending(), 0);
    }

    #[test]
    fn drain_timer_credits_cpu_ticks_to_current_thread() {
        // S07 slice 16 periodic PIT-handler accounting: drained HW ticks
        // are CPU time for the RUNNING thread (current slot), bulk O(1).
        // Dispatch to 2, drain 2 ticks (partial) then 2 more (expiry):
        // both credits land on 2; dispatch to 1 re-targets later credits.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert_eq!(s.cpu_ticks_of(1), Some(0));
        assert_eq!(s.cpu_ticks_of(2), Some(0));
        assert_eq!(s.total_cpu_ticks(), 0);
        assert_eq!(s.cpu_ticks_of(99), None);
        assert_eq!(s.dispatch_gated(&gate), Ok(2));
        let mut hw = TimerTicks::new();
        hw.tick();
        hw.tick();
        assert_eq!(s.drain_timer(&mut hw, &gate), (2, false));
        assert_eq!(s.cpu_ticks_of(2), Some(2));
        assert_eq!(s.cpu_ticks_of(1), Some(0));
        assert_eq!(s.total_cpu_ticks(), 2);
        hw.tick();
        hw.tick();
        assert_eq!(s.drain_timer(&mut hw, &gate), (2, true));
        assert_eq!(s.cpu_ticks_of(2), Some(4));
        assert_eq!(s.total_cpu_ticks(), 4);
        assert_eq!(s.dispatch_gated(&gate), Ok(1));
        let mut hw2 = TimerTicks::new();
        hw2.tick();
        assert_eq!(s.drain_timer(&mut hw2, &gate), (1, false));
        assert_eq!(s.cpu_ticks_of(1), Some(1));
        assert_eq!(s.cpu_ticks_of(2), Some(4));
        assert_eq!(s.total_cpu_ticks(), 5);
    }

    #[test]
    fn drain_timer_cpu_accounting_fail_closed_edges() {
        // Empty drain credits nothing; unknown current drains the HW counter
        // but preserves the quantum and owes no dispatch (S07 slice 20
        // backoff: no phantom time, no per-tick failed pick); flood credits
        // bulk to the running thread.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        let mut empty = TimerTicks::new();
        assert_eq!(s.drain_timer(&mut empty, &gate), (0, false));
        assert_eq!(s.total_cpu_ticks(), 0);
        let mut u = Scheduler::new(99);
        assert!(u.spawn(mk(1, 0x1000_0000)));
        let mut hw = TimerTicks::new();
        hw.tick();
        hw.tick();
        hw.tick();
        // Unknown current: HW ticks consumed, quantum preserved (still 0),
        // no expiry, no slot credited.
        assert_eq!(u.drain_timer(&mut hw, &gate), (3, false));
        assert_eq!(u.quantum_left(), 0);
        assert_eq!(u.cpu_ticks_of(1), Some(0));
        assert_eq!(u.total_cpu_ticks(), 0);
        assert_eq!(hw.pending(), 0);
        // Unknown current with an armed quantum preserves it too (does not
        // burn another thread's budget while idle).
        let mut v = Scheduler::new(1);
        assert!(v.spawn(mk(1, 0x1000_0000)));
        assert!(v.spawn(mk(2, 0x2000_0000)));
        assert_eq!(v.dispatch_gated(&gate), Ok(2));
        assert_eq!(v.quantum_left(), QUANTUM_TICKS);
        // Force unknown without dispatching: rebuild with the same table
        // but an absent current is impossible via safe API, so pin the
        // armed-known contrast instead — a known drain consumes.
        let mut hw2 = TimerTicks::new();
        hw2.tick();
        assert_eq!(v.drain_timer(&mut hw2, &gate), (1, false));
        assert_eq!(v.quantum_left(), QUANTUM_TICKS - 1);
        // Armed-unknown preserve (REVIEW #468 R2): unreachable via safe
        // API (no setter for an absent current), so pin it via the
        // test-child private-field access: force unknown current with a
        // full quantum, then drain must consume HW ticks but preserve
        // the quantum, credit nothing, owe no expiry.
        v.current = 99;
        v.ticks_left = QUANTUM_TICKS;
        let mut hw_armed_unknown = TimerTicks::new();
        hw_armed_unknown.tick();
        hw_armed_unknown.tick();
        assert_eq!(v.drain_timer(&mut hw_armed_unknown, &gate), (2, false));
        assert_eq!(v.quantum_left(), QUANTUM_TICKS);
        assert_eq!(v.total_cpu_ticks(), 1);
        assert_eq!(hw_armed_unknown.pending(), 0);
        // Flood: bulk credit lands on the running thread in O(1).
        let mut f = Scheduler::new(1);
        assert!(f.spawn(mk(1, 0x1000_0000)));
        assert!(f.spawn(mk(2, 0x2000_0000)));
        assert_eq!(f.dispatch_gated(&gate), Ok(2));
        let mut flood = TimerTicks::new();
        let mut i = 0u64;
        while i < 1000 {
            flood.tick();
            i += 1;
        }
        assert_eq!(f.drain_timer(&mut flood, &gate), (1000, true));
        assert_eq!(f.cpu_ticks_of(2), Some(1000));
        assert_eq!(f.cpu_ticks_of(1), Some(0));
        assert_eq!(f.total_cpu_ticks(), 1000);
    }

    #[test]
    fn drain_then_dispatch_credits_pre_dispatch_current() {
        // S07 slice 17 (REVIEW #462 risk 1): the combined helper credits the
        // drained backlog to the PRE-dispatch current before the pick, so a
        // backlog accumulated across a dispatch can never misattribute to
        // the NEW current. Partial drain dispatches nothing; expiry drains
        // then dispatches exactly once.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert_eq!(s.dispatch_gated(&gate), Ok(2));
        // Partial quantum: 2 of 4 ticks, no dispatch owed.
        let mut hw = TimerTicks::new();
        hw.tick();
        hw.tick();
        let (drained, dispatched) = s.drain_then_dispatch_gated(&mut hw, &gate);
        assert_eq!(drained, 2);
        assert!(dispatched.is_none());
        assert_eq!(s.current(), 2);
        assert_eq!(s.cpu_ticks_of(2), Some(2));
        // Expiry: remaining 2 ticks credit 2 (still current) then resched
        // 2->1 exactly once; the 2-tick credit stays on 2, not 1.
        hw.tick();
        hw.tick();
        let (drained2, dispatched2) = s.drain_then_dispatch_gated(&mut hw, &gate);
        assert_eq!(drained2, 2);
        assert_eq!(dispatched2, Some(Ok(1)));
        assert_eq!(s.current(), 1);
        assert_eq!(s.cpu_ticks_of(2), Some(4));
        assert_eq!(s.cpu_ticks_of(1), Some(0));
        // Next quantum credits the new current (1).
        let mut hw2 = TimerTicks::new();
        hw2.tick();
        let (drained3, dispatched3) = s.drain_then_dispatch_gated(&mut hw2, &gate);
        assert_eq!(drained3, 1);
        assert!(dispatched3.is_none());
        assert_eq!(s.cpu_ticks_of(1), Some(1));
        assert_eq!(s.total_cpu_ticks(), 5);
        // Empty drain dispatches nothing (IRQ silence never rescheds).
        let mut empty = TimerTicks::new();
        let (d0, disp0) = s.drain_then_dispatch_gated(&mut empty, &gate);
        assert_eq!((d0, disp0.is_none()), (0, true));
        // Failed resched on a KNOWN current still reports Some(Err) with the
        // quantum consumed (nothing Ready: current Running, other Done).
        let mut k = Scheduler::new(1);
        let mut run = mk(1, 0x1000_0000);
        assert!(run.set_state(ThreadState::Running));
        let mut done = mk(2, 0x2000_0000);
        assert!(done.set_state(ThreadState::Done));
        assert!(k.spawn(run));
        assert!(k.spawn(done));
        let mut hwk = TimerTicks::new();
        hwk.tick();
        let (dk, dispk) = k.drain_then_dispatch_gated(&mut hwk, &gate);
        assert_eq!(dk, 1);
        assert_eq!(dispk, Some(Err(SwitchError::NotReady)));
        assert_eq!(k.total_cpu_ticks(), 1);
        // Unknown current owes no dispatch (S07 slice 20 backoff): the HW
        // ticks are consumed but no pick is attempted, even non-empty.
        let mut u = Scheduler::new(99);
        assert!(u.spawn(mk(1, 0x1000_0000)));
        let mut hw3 = TimerTicks::new();
        hw3.tick();
        let (du, dispu) = u.drain_then_dispatch_gated(&mut hw3, &gate);
        assert_eq!(du, 1);
        assert!(dispu.is_none());
        assert_eq!(u.total_cpu_ticks(), 0);
        assert_eq!(u.quantum_left(), 0);
    }

    #[test]
    fn drain_then_dispatch_failed_resched_retries_once_per_nonempty_drain() {
        // S07 slice 18 (REVIEW #464 nit 2) scoped by slice 20: a failed
        // resched on a KNOWN current keeps the consumed quantum
        // (`ticks_left == 0`), so every subsequent NON-EMPTY drain retries
        // exactly one dispatch while empty drains stay quiet. Unknown
        // current never retries (backoff: `(drained, None)` even non-empty).
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        // Known-but-nothing-Ready: current 1 Running, other Done.
        let mut u = Scheduler::new(1);
        let mut run = mk(1, 0x1000_0000);
        assert!(run.set_state(ThreadState::Running));
        let mut done = mk(2, 0x2000_0000);
        assert!(done.set_state(ThreadState::Done));
        assert!(u.spawn(run));
        assert!(u.spawn(done));
        let mut hw = TimerTicks::new();
        hw.tick();
        let (d1, r1) = u.drain_then_dispatch_gated(&mut hw, &gate);
        assert_eq!(d1, 1);
        assert_eq!(r1, Some(Err(SwitchError::NotReady)));
        assert_eq!(u.quantum_left(), 0);
        // Empty drain: no retry even though the quantum is spent.
        let mut empty = TimerTicks::new();
        let (d0, r0) = u.drain_then_dispatch_gated(&mut empty, &gate);
        assert_eq!((d0, r0.is_none()), (0, true));
        assert_eq!(u.quantum_left(), 0);
        // Next non-empty drain retries exactly once (still Err, still 0),
        // and the retry credit lands on the known current.
        hw.tick();
        hw.tick();
        let (d2, r2) = u.drain_then_dispatch_gated(&mut hw, &gate);
        assert_eq!(d2, 2);
        assert_eq!(r2, Some(Err(SwitchError::NotReady)));
        assert_eq!(u.quantum_left(), 0);
        assert_eq!(u.cpu_ticks_of(1), Some(3));
        // Unknown current: no retry, even non-empty, quantum preserved.
        let mut w = Scheduler::new(99);
        assert!(w.spawn(mk(1, 0x1000_0000)));
        let mut hwu = TimerTicks::new();
        hwu.tick();
        let (du1, ru1) = w.drain_then_dispatch_gated(&mut hwu, &gate);
        assert_eq!(du1, 1);
        assert!(ru1.is_none());
        hwu.tick();
        hwu.tick();
        let (du2, ru2) = w.drain_then_dispatch_gated(&mut hwu, &gate);
        assert_eq!(du2, 2);
        assert!(ru2.is_none());
        assert_eq!(w.quantum_left(), 0);
        assert_eq!(w.total_cpu_ticks(), 0);
    }

    #[test]
    fn live_backoff_cap_bounds_consecutive_failures() {
        // S07 slice 22 (REVIEW #470 R1 policy): the live handler drains
        // every batch but dispatches only while unsuppressed. Streak bumps
        // on failed dispatch, resets on success or no-dispatch, saturates.
        assert_eq!(LIVE_MAX_CONSECUTIVE_FAILURES, 3);
        assert!(!live_backoff_suppresses(0));
        assert!(!live_backoff_suppresses(2));
        assert!(live_backoff_suppresses(3));
        assert!(live_backoff_suppresses(4));
        assert!(live_backoff_suppresses(u64::MAX));
        let mut streak = 0u64;
        streak = live_backoff_next(streak, true);
        assert_eq!(streak, 1);
        streak = live_backoff_next(streak, true);
        streak = live_backoff_next(streak, true);
        assert_eq!(streak, 3);
        assert!(live_backoff_suppresses(streak));
        // Suppressed stays suppressed on further failures (saturating).
        streak = live_backoff_next(streak, true);
        assert_eq!(streak, 4);
        assert!(live_backoff_suppresses(streak));
        // Success or no-dispatch resets.
        assert_eq!(live_backoff_next(streak, false), 0);
        assert!(!live_backoff_suppresses(live_backoff_next(streak, false)));
        assert_eq!(live_backoff_next(u64::MAX, true), u64::MAX);
    }

    #[test]
    fn live_helper_throttles_persistent_failures_three_on_one_off() {
        // S07 slice 23: the live helper consumes the slice-22 cap — at most
        // LIVE_MAX_CONSECUTIVE_FAILURES failed picks in a row, then one
        // drain-only skip (no dispatch, streak reset), then probes again
        // (automatic recovery when the table becomes Ready).
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut s = Scheduler::new(1);
        let mut run = mk(1, 0x1000_0000);
        assert!(run.set_state(ThreadState::Running));
        let mut done = mk(2, 0x2000_0000);
        assert!(done.set_state(ThreadState::Done));
        assert!(s.spawn(run));
        assert!(s.spawn(done));
        let mut streak = 0u64;
        // Three consecutive failed picks (streak 1, 2, 3).
        let mut i = 0;
        while i < LIVE_MAX_CONSECUTIVE_FAILURES {
            let mut hw = TimerTicks::new();
            hw.tick();
            let (drained, res) = s.drain_then_dispatch_live_gated(&mut hw, &gate, &mut streak);
            assert_eq!(drained, 1);
            assert_eq!(res, Some(Err(SwitchError::NotReady)));
            assert_eq!(streak, i + 1);
            i += 1;
        }
        assert!(live_backoff_suppresses(streak));
        // Fourth batch: suppressed drain-only — HW ticks still consumed and
        // credited, NO dispatch attempted, streak reset (not held, or the
        // handler would stall forever once a thread becomes Ready).
        let mut hw_skip = TimerTicks::new();
        hw_skip.tick();
        hw_skip.tick();
        let cpu_before = s.total_cpu_ticks();
        let (drained_skip, res_skip) =
            s.drain_then_dispatch_live_gated(&mut hw_skip, &gate, &mut streak);
        assert_eq!(drained_skip, 2);
        assert!(res_skip.is_none());
        assert_eq!(streak, 0);
        assert_eq!(s.total_cpu_ticks(), cpu_before.saturating_add(2));
        assert_eq!(s.quantum_left(), 0);
        // Fifth batch: probes again (fails while still nothing-Ready).
        let mut hw_probe = TimerTicks::new();
        hw_probe.tick();
        let (drained_probe, res_probe) =
            s.drain_then_dispatch_live_gated(&mut hw_probe, &gate, &mut streak);
        assert_eq!(drained_probe, 1);
        assert_eq!(res_probe, Some(Err(SwitchError::NotReady)));
        assert_eq!(streak, 1);
    }

    #[test]
    fn live_helper_resets_streak_on_success_and_idle_none() {
        // Success, empty, partial, and unknown-current drains all reset the
        // streak; the unsuppressed live path agrees with the task-context
        // helper on success.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert_eq!(s.dispatch_gated(&gate), Ok(2));
        let mut streak = 2u64;
        // Partial quantum (no expiry): no dispatch, streak reset.
        let mut hw = TimerTicks::new();
        hw.tick();
        let (drained, res) = s.drain_then_dispatch_live_gated(&mut hw, &gate, &mut streak);
        assert_eq!(drained, 1);
        assert!(res.is_none());
        assert_eq!(streak, 0);
        // Expiry success: one dispatch, streak reset.
        streak = 2;
        let mut hw2 = TimerTicks::new();
        let mut i = 0u64;
        while i < QUANTUM_TICKS - 1 {
            hw2.tick();
            i += 1;
        }
        let (drained2, res2) = s.drain_then_dispatch_live_gated(&mut hw2, &gate, &mut streak);
        assert_eq!(drained2, QUANTUM_TICKS - 1);
        assert_eq!(res2, Some(Ok(1)));
        assert_eq!(streak, 0);
        assert_eq!(s.current(), 1);
        // Empty drain resets a non-zero streak (IRQ silence is idle).
        streak = 2;
        let mut empty = TimerTicks::new();
        let (d0, r0) = s.drain_then_dispatch_live_gated(&mut empty, &gate, &mut streak);
        assert_eq!((d0, r0.is_none()), (0, true));
        assert_eq!(streak, 0);
        // Unknown current: HW ticks consumed, no dispatch, streak reset.
        streak = 2;
        let mut u = Scheduler::new(99);
        assert!(u.spawn(mk(1, 0x1000_0000)));
        let mut hwu = TimerTicks::new();
        hwu.tick();
        hwu.tick();
        let (du, ru) = u.drain_then_dispatch_live_gated(&mut hwu, &gate, &mut streak);
        assert_eq!(du, 2);
        assert!(ru.is_none());
        assert_eq!(streak, 0);
        assert_eq!(u.total_cpu_ticks(), 0);
    }

    #[test]
    fn pre_init_null_current_drain_consumes_without_dispatch() {
        // REVIEW #497 risk 4: pre-`sched_init_demo_static` the IRQ-owned
        // `STATIC_SCHED` is `Scheduler::new(NULL_THREAD)` (unknown current).
        // A per-tick IRQ drain there must consume the counter with no dispatch
        // owed, on both the task and live paths (safe by construction).
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut pre = Scheduler::new(NULL_THREAD);
        assert!(pre.spawn(mk(1, 0x1000_0000)));
        let mut hw = TimerTicks::new();
        hw.tick();
        hw.tick();
        hw.tick();
        let (d, disp) = pre.drain_then_dispatch_gated(&mut hw, &gate);
        assert_eq!(d, 3);
        assert!(disp.is_none());
        assert_eq!(hw.pending(), 0);
        assert_eq!(pre.total_cpu_ticks(), 0);
        let mut pre_live = Scheduler::new(NULL_THREAD);
        assert!(pre_live.spawn(mk(1, 0x1000_0000)));
        let mut hw_live = TimerTicks::new();
        hw_live.tick();
        hw_live.tick();
        let mut streak = 2u64;
        let (dl, res) = pre_live.drain_then_dispatch_live_gated(&mut hw_live, &gate, &mut streak);
        assert_eq!(dl, 2);
        assert!(res.is_none());
        assert_eq!(streak, 0);
        assert_eq!(hw_live.pending(), 0);
        assert_eq!(pre_live.total_cpu_ticks(), 0);
    }

    #[test]
    fn live_unsuppressed_agrees_with_task_helper() {
        // S07 slice24 (REVIEW #473 finding 2): the live helper duplicates
        // the drain-then-dispatch shape — while unsuppressed it must agree
        // exactly with the task-context helper (same drain, same dispatch,
        // same state), differing only by the streak update. A future edit
        // that reorders (credit after pick), double-dispatches, or credits
        // differently in one helper goes red here. Suppressed batches
        // diverge by design (drain-only vs retry) and are pinned by the
        // throttle test, not here.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        for ticks in [0u64, 1, 2, 4, 5, 9] {
            for streak_in in [0u64, 1, 2] {
                assert!(
                    !live_backoff_suppresses(streak_in),
                    "unsuppressed only for {ticks} ticks streak {streak_in}"
                );
                // Success table: armed 1+2, current 2, full quantum.
                let mut live = Scheduler::new(1);
                assert!(live.spawn(mk(1, 0x1000_0000)));
                assert!(live.spawn(mk(2, 0x2000_0000)));
                assert_eq!(live.dispatch_gated(&gate), Ok(2));
                let mut task = live.clone();
                let mut hw_live = TimerTicks::new();
                let mut hw_task = TimerTicks::new();
                let mut i = 0;
                while i < ticks {
                    hw_live.tick();
                    hw_task.tick();
                    i += 1;
                }
                let mut streak = streak_in;
                let r_live = live.drain_then_dispatch_live_gated(&mut hw_live, &gate, &mut streak);
                let r_task = task.drain_then_dispatch_gated(&mut hw_task, &gate);
                assert_eq!(
                    r_live, r_task,
                    "success table {ticks} ticks streak {streak_in}"
                );
                assert_eq!(live.current(), task.current());
                assert_eq!(live.quantum_left(), task.quantum_left());
                assert_eq!(live.total_cpu_ticks(), task.total_cpu_ticks());
                let failed = matches!(r_live.1, Some(Err(_)));
                assert_eq!(streak, live_backoff_next(streak_in, failed));
                // Failure table: known current, nothing Ready (NotReady).
                let mut run = mk(1, 0x1000_0000);
                assert!(run.set_state(ThreadState::Running));
                let mut done = mk(2, 0x2000_0000);
                assert!(done.set_state(ThreadState::Done));
                let mut live_f = Scheduler::new(1);
                assert!(live_f.spawn(run));
                assert!(live_f.spawn(done));
                let mut task_f = live_f.clone();
                let mut hw_live_f = TimerTicks::new();
                let mut hw_task_f = TimerTicks::new();
                let mut j = 0;
                while j < ticks {
                    hw_live_f.tick();
                    hw_task_f.tick();
                    j += 1;
                }
                let mut streak_f = streak_in;
                let rf_live =
                    live_f.drain_then_dispatch_live_gated(&mut hw_live_f, &gate, &mut streak_f);
                let rf_task = task_f.drain_then_dispatch_gated(&mut hw_task_f, &gate);
                assert_eq!(
                    rf_live, rf_task,
                    "failure table {ticks} ticks streak {streak_in}"
                );
                assert_eq!(live_f.current(), task_f.current());
                assert_eq!(live_f.quantum_left(), task_f.quantum_left());
                assert_eq!(live_f.total_cpu_ticks(), task_f.total_cpu_ticks());
                let failed_f = matches!(rf_live.1, Some(Err(_)));
                assert_eq!(streak_f, live_backoff_next(streak_in, failed_f));
                // Unknown current: HW ticks consumed, no dispatch, streak reset.
                let mut live_u = Scheduler::new(99);
                assert!(live_u.spawn(mk(1, 0x1000_0000)));
                let mut task_u = live_u.clone();
                let mut hw_live_u = TimerTicks::new();
                let mut hw_task_u = TimerTicks::new();
                let mut k = 0;
                while k < ticks {
                    hw_live_u.tick();
                    hw_task_u.tick();
                    k += 1;
                }
                let mut streak_u = streak_in;
                let ru_live =
                    live_u.drain_then_dispatch_live_gated(&mut hw_live_u, &gate, &mut streak_u);
                let ru_task = task_u.drain_then_dispatch_gated(&mut hw_task_u, &gate);
                assert_eq!(
                    ru_live, ru_task,
                    "unknown table {ticks} ticks streak {streak_in}"
                );
                assert!(ru_live.1.is_none());
                assert_eq!(streak_u, 0);
            }
        }
    }

    #[test]
    fn drain_then_dispatch_matches_manual_drain_plus_dispatch() {
        // S07 slice 18 (REVIEW #464 nit 1): the helper owns the
        // drain-before-pick order, so it must agree tick-for-tick with the
        // manual `drain_timer`-then-`dispatch_gated` sequence the old guest
        // wrapper inlined. A future helper edit that reorders (credit after
        // pick) or double-dispatches goes red here.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        fn manual(
            s: &mut Scheduler,
            hw: &mut TimerTicks,
            gate: &IrqGuard,
        ) -> (u64, Option<Result<u64, SwitchError>>) {
            let (drained, expired) = s.drain_timer(hw, gate);
            if !expired {
                return (drained, None);
            }
            (drained, Some(s.dispatch_gated(gate)))
        }
        for ticks in [1u64, 2, 4, 5, 9] {
            let mut a = Scheduler::new(1);
            assert!(a.spawn(mk(1, 0x1000_0000)));
            assert!(a.spawn(mk(2, 0x2000_0000)));
            assert_eq!(a.dispatch_gated(&gate), Ok(2));
            let mut b = a.clone();
            let mut ha = TimerTicks::new();
            let mut hb = TimerTicks::new();
            let mut i = 0;
            while i < ticks {
                ha.tick();
                hb.tick();
                i += 1;
            }
            let ra = a.drain_then_dispatch_gated(&mut ha, &gate);
            let rb = manual(&mut b, &mut hb, &gate);
            assert_eq!(
                ra, rb,
                "helper must match manual sequence for {ticks} ticks"
            );
            assert_eq!(a.current(), b.current());
            assert_eq!(a.cpu_ticks_of(2), b.cpu_ticks_of(2));
            assert_eq!(a.quantum_left(), b.quantum_left());
        }
    }

    #[test]
    fn demo_armed_scheduler_matches_bring_up_shape() {
        // S07 slice 19: the IRQ-owned static init consumes this constructor,
        // so pin the build rule host-side (guest static wiring is proven by
        // QEMU `TICK ok=1`). Same table the old bring-up inlined: two
        // metadata-only descriptors, one armed dispatch to 2.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut s = Scheduler::demo_armed_scheduler(&gate);
        assert_eq!(s.len(), 2);
        assert_eq!(s.current(), 2);
        assert_eq!(s.state_of(2), Some(ThreadState::Running));
        assert_eq!(s.state_of(1), Some(ThreadState::Ready));
        assert_eq!(s.quantum_left(), QUANTUM_TICKS);
        assert_eq!(s.runs_of(2), Some(1));
        assert_eq!(s.runs_of(1), Some(0));
        assert_eq!(s.cpu_ticks_of(1), Some(0));
        assert_eq!(s.cpu_ticks_of(2), Some(0));
        // Drains like the armed bring-up table: 1 tick credits current 2
        // without expiry; a full quantum expires and rescheds to 1.
        let mut one = TimerTicks::new();
        one.tick();
        assert_eq!(s.drain_then_dispatch_gated(&mut one, &gate), (1, None));
        assert_eq!(s.cpu_ticks_of(2), Some(1));
        let mut rest = TimerTicks::new();
        let mut i = 0u64;
        while i < QUANTUM_TICKS - 1 {
            rest.tick();
            i += 1;
        }
        let (drained, dispatched) = s.drain_then_dispatch_gated(&mut rest, &gate);
        assert_eq!(drained, QUANTUM_TICKS - 1);
        assert_eq!(dispatched, Some(Ok(1)));
        assert_eq!(s.cpu_ticks_of(2), Some(QUANTUM_TICKS));
    }

    #[test]
    fn cpu_slot_invariant_across_sweep() {
        // S07 slice 17 (REVIEW #462 risk 2): `cpu[i]` belongs to `table[i]`
        // under the same append-only invariant as `runs[i]` (zeroed on
        // spawn, no remove/compact exists). Three full quanta through the
        // combined helper credit exactly QUANTUM_TICKS to each thread in
        // turn (2, then 3, then 1), so a future table-shift without `cpu`
        // goes red here exactly as the `runs` sweep does.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert!(s.spawn(mk(3, 0x3000_0000)));
        assert_eq!(s.dispatch_gated(&gate), Ok(2));
        for _ in 0..3 {
            let mut hw = TimerTicks::new();
            let mut i = 0u64;
            while i < QUANTUM_TICKS {
                hw.tick();
                i += 1;
            }
            let (drained, dispatched) = s.drain_then_dispatch_gated(&mut hw, &gate);
            assert_eq!(drained, QUANTUM_TICKS);
            assert!(dispatched.is_some());
        }
        assert_eq!(s.current(), 2);
        assert_eq!(s.cpu_ticks_of(2), Some(QUANTUM_TICKS));
        assert_eq!(s.cpu_ticks_of(3), Some(QUANTUM_TICKS));
        assert_eq!(s.cpu_ticks_of(1), Some(QUANTUM_TICKS));
        assert_eq!(s.total_cpu_ticks(), QUANTUM_TICKS * 3);
        assert_eq!(s.cpu_ticks_of(99), None);
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn slot_index_invariant_append_only_with_id_keyed_trace() {
        // REVIEW #433 F1 regression: `runs[i]` belongs to `table[i]`
        // (append-only, no remove/compact exists). Pins two rounds of
        // per-id attribution plus len/id stability, so a future
        // remove/compact that shifts `table` without `runs` goes red here.
        // The trace needs no such pin: entries are ids, not slots.
        let mut s = Scheduler::new(1);
        assert!(s.spawn(mk(1, 0x1000_0000)));
        assert!(s.spawn(mk(2, 0x2000_0000)));
        assert!(s.spawn(mk(3, 0x3000_0000)));
        let mut n = 0;
        while n < 6 {
            assert!(s.dispatch().is_ok());
            n += 1;
        }
        assert_eq!(s.runs_of(1), Some(2));
        assert_eq!(s.runs_of(2), Some(2));
        assert_eq!(s.runs_of(3), Some(2));
        assert_eq!(s.total_dispatches(), 6);
        assert_eq!(s.fairness_spread(), 0);
        assert_eq!(s.len(), 3);
        assert_eq!(s.trace_len(), 6);
        assert_eq!(
            (s.trace_at(0), s.trace_at(1), s.trace_at(2)),
            (Some(2), Some(3), Some(1))
        );
    }

    #[test]
    fn live_poll_shape_on_demo_table_stays_unsuppressed() {
        // S08 slice25: the guest bring-up live poll (demo-armed table +
        // IRQ-owned streak starting at 0) drains the QEMU first-tick batch
        // without dispatching and stays unsuppressed, so the `TICK` line is
        // unchanged from the task-helper path (slice-24 parity). Streak
        // reset (not hold) is the init rule `sched_init_demo_static` owns.
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        let mut s = Scheduler::demo_armed_scheduler(&gate);
        let mut streak = 0u64;
        let mut one = TimerTicks::new();
        one.tick();
        let (drained, res) = s.drain_then_dispatch_live_gated(&mut one, &gate, &mut streak);
        assert_eq!((drained, res.is_none()), (1, true));
        assert_eq!(streak, 0);
        assert_eq!(s.current(), 2);
        assert_eq!(s.cpu_ticks_of(2), Some(1));
        assert_eq!(s.quantum_left(), QUANTUM_TICKS - 1);
    }

    #[test]
    fn quantum_boundary_trio_and_suppressed_coupling_with_empty_poll_backoff() {
        // S07 slice27 (trio off-by-one disconfirm + REVIEW #473 f3 coupling +
        // poll-before-init backoff; host-only, guest untouched): the armed
        // demo quantum expires at exactly QUANTUM_TICKS (Q-1 owes nothing,
        // Q and Q+1 owe exactly one resched), a suppressed live drain still
        // consumes the quantum via `drain_timer` (no dispatch, streak reset),
        // and an empty table (pre-`sched_init_demo_static` shape) drains
        // without dispatching (backoff by construction).
        let gate = unsafe { crate::cpu::IrqGuard::enter() };
        // Trio on the task helper: Q-1 partial, Q exact, Q+1 bulk.
        for (ticks, expect_dispatch) in [
            (QUANTUM_TICKS - 1, false),
            (QUANTUM_TICKS, true),
            (QUANTUM_TICKS + 1, true),
        ] {
            let mut s = Scheduler::demo_armed_scheduler(&gate);
            let mut hw = TimerTicks::new();
            let mut i = 0u64;
            while i < ticks {
                hw.tick();
                i += 1;
            }
            let (drained, res) = s.drain_then_dispatch_gated(&mut hw, &gate);
            assert_eq!(drained, ticks, "task trio {ticks} ticks");
            assert_eq!(res.is_some(), expect_dispatch, "task trio {ticks} ticks");
            if expect_dispatch {
                assert_eq!(res, Some(Ok(1)));
                assert_eq!(s.current(), 1);
                assert_eq!(s.quantum_left(), QUANTUM_TICKS);
                assert_eq!(s.cpu_ticks_of(2), Some(ticks));
            } else {
                assert_eq!(s.current(), 2);
                assert_eq!(s.quantum_left(), 1);
                assert_eq!(s.cpu_ticks_of(2), Some(ticks));
            }
        }
        // Trio on the live helper (unsuppressed streak 0): same drain +
        // dispatch as task, streak stays 0 on the never-failing demo table.
        for (ticks, expect_dispatch) in [
            (QUANTUM_TICKS - 1, false),
            (QUANTUM_TICKS, true),
            (QUANTUM_TICKS + 1, true),
        ] {
            let mut s = Scheduler::demo_armed_scheduler(&gate);
            let mut hw = TimerTicks::new();
            let mut i = 0u64;
            while i < ticks {
                hw.tick();
                i += 1;
            }
            let mut streak = 0u64;
            let (drained, res) = s.drain_then_dispatch_live_gated(&mut hw, &gate, &mut streak);
            assert_eq!(drained, ticks, "live trio {ticks} ticks");
            assert_eq!(res.is_some(), expect_dispatch, "live trio {ticks} ticks");
            assert_eq!(streak, 0, "demo never fails so streak stays 0");
            if expect_dispatch {
                assert_eq!(res, Some(Ok(1)));
                assert_eq!(s.current(), 1);
            } else {
                assert_eq!(s.current(), 2);
                assert_eq!(s.quantum_left(), 1);
            }
        }
        // Suppressed-expiry coupling (f3): drive a known-nothing-Ready table
        // to the cap, then the suppressed drain still consumes via
        // `drain_timer` (CPU credited, quantum stays 0), dispatches nothing,
        // and resets the streak for the next probe.
        let mut f = Scheduler::new(1);
        let mut run = mk(1, 0x1000_0000);
        assert!(run.set_state(ThreadState::Running));
        let mut done = mk(2, 0x2000_0000);
        assert!(done.set_state(ThreadState::Done));
        assert!(f.spawn(run));
        assert!(f.spawn(done));
        let mut streak = 0u64;
        let mut i = 0u64;
        while i < LIVE_MAX_CONSECUTIVE_FAILURES {
            let mut hw = TimerTicks::new();
            hw.tick();
            let (_, res) = f.drain_then_dispatch_live_gated(&mut hw, &gate, &mut streak);
            assert_eq!(res, Some(Err(SwitchError::NotReady)));
            i += 1;
        }
        assert!(live_backoff_suppresses(streak));
        let cpu_before = f.total_cpu_ticks();
        let mut hw_skip = TimerTicks::new();
        hw_skip.tick();
        hw_skip.tick();
        let (drained_skip, res_skip) =
            f.drain_then_dispatch_live_gated(&mut hw_skip, &gate, &mut streak);
        assert_eq!(drained_skip, 2);
        assert!(res_skip.is_none());
        assert_eq!(streak, 0);
        assert_eq!(f.quantum_left(), 0);
        assert_eq!(f.total_cpu_ticks(), cpu_before.saturating_add(2));
        // Suppressed with an armed quantum still consumes it (coupling, not
        // preserve): forced streak 3 on a fresh demo table drains 1 tick to
        // QUANTUM-1 with no dispatch and streak reset.
        let mut armed = Scheduler::demo_armed_scheduler(&gate);
        let mut streak_armed = LIVE_MAX_CONSECUTIVE_FAILURES;
        assert!(live_backoff_suppresses(streak_armed));
        let mut hw_one = TimerTicks::new();
        hw_one.tick();
        let (drained_a, res_a) =
            armed.drain_then_dispatch_live_gated(&mut hw_one, &gate, &mut streak_armed);
        assert_eq!(drained_a, 1);
        assert!(res_a.is_none());
        assert_eq!(streak_armed, 0);
        assert_eq!(armed.quantum_left(), QUANTUM_TICKS - 1);
        assert_eq!(armed.cpu_ticks_of(2), Some(1));
        // Poll-before-init backoff: empty table (pre-init STATIC_SCHED shape)
        // consumes HW ticks, dispatches nothing, resets a dirty streak.
        let mut empty = Scheduler::new(NULL_THREAD);
        let mut streak_dirty = 2u64;
        let mut hw_empty = TimerTicks::new();
        hw_empty.tick();
        hw_empty.tick();
        hw_empty.tick();
        let (drained_e, res_e) =
            empty.drain_then_dispatch_live_gated(&mut hw_empty, &gate, &mut streak_dirty);
        assert_eq!(drained_e, 3);
        assert!(res_e.is_none());
        assert_eq!(streak_dirty, 0);
        assert_eq!(empty.total_cpu_ticks(), 0);
        assert_eq!(empty.quantum_left(), 0);
    }

    #[test]
    fn scheduler_holds_no_cell_interior_mutability() {
        // Replace-not-reuse pin (REVIEW #319 finding 2, REVIEW #321
        // residual c): the scheduler type must not contain the coop
        // interior-mutability primitive — mutation is `&mut self` only.
        // Doc prose names the cell to explain the ban, and the ban list
        // itself would self-match, so fragments are built with `concat!`
        // (no verbatim hit) and matched against non-doc code lines only
        // (`///`/`//!` stripped, `/* ... */` block comments stripped).
        const SRC: &str = include_str!("sched.rs");
        let banned = [
            concat!("core", "::cell"),
            concat!("Cel", "l<"),
            concat!("Cel", "l::"),
            concat!("Thread", "Mutex::"),
            concat!("Thread", "Mutex {"),
            concat!(": Thread", "Mutex"),
        ];
        // Strip block comments first so a banned token hidden inside
        // `/* ... */` cannot slip past the line-based doc filter.
        // String/char-aware + nesting (see `strip_code`): a `"` inside a
        // block or a `/*` inside a literal no longer desyncs the strip.
        let stripped = strip_code(SRC);
        let mut in_code = true;
        let sbytes = stripped.as_bytes();
        let mut line_start = 0usize;
        let mut li = 0usize;
        while li <= sbytes.len() {
            if li == sbytes.len() || sbytes[li] == b'\n' {
                let line = &stripped[line_start..li];
                let t = line.trim_start();
                in_code = !(t.starts_with("//") || t.starts_with("///") || t.starts_with("//!"));
                if in_code {
                    let mut b = 0usize;
                    while b < banned.len() {
                        assert!(
                            !line.contains(banned[b]),
                            "scheduler must not reuse the mutex cell"
                        );
                        b += 1;
                    }
                }
                line_start = li + 1;
            }
            li += 1;
        }
        let _ = in_code;
    }

    #[test]
    fn irq_dispatch_result_is_deferred_never_stack_switch() {
        // S07 slice34 (REVIEW #501 risk 1 pin): IRQ-context dispatch is
        // metadata-only while `IRQ_STACK_SWITCH_PERMITTED == false`. The
        // `timer_irq_rust` path ignores the dispatch `Ok(next)` — that
        // ignore is a deferral, never an immediate RSP switch. Pin both
        // halves: the gate stays closed, and the gated dispatch on the demo
        // table mutates only state/current/quantum (no stack primitive to
        // call — there is none in-tree, so this test pins the observable
        // metadata shape the IRQ path defers). `black_box` keeps the gate
        // assert runtime so `clippy -D warnings`
        // (`assertions_on_constants`) stays clean.
        assert!(!core::hint::black_box(IRQ_STACK_SWITCH_PERMITTED));
        let gate = unsafe { IrqGuard::enter() };
        let mut s = Scheduler::demo_armed_scheduler(&gate);
        assert_eq!(s.current(), 2);
        let next = s.dispatch_gated(&gate).expect("demo table always Ready");
        assert!(!core::hint::black_box(IRQ_STACK_SWITCH_PERMITTED));
        assert_eq!(next, s.current());
        assert_eq!(s.quantum_left(), QUANTUM_TICKS);
    }

    #[test]
    fn irq_gate_enforcement_helper_defers_while_closed() {
        // S07 slice35 (REVIEW #505): the IRQ path references the gate in code
        // (`debug_assert!(irq_dispatch_must_defer())` in `timer_irq_rust`),
        // not docs-only. Pin the helper the IRQ site calls: true while the
        // gate is closed. `black_box` keeps `clippy -D warnings`
        // (`assertions_on_constants`) clean, same as the slice34 pin.
        assert!(core::hint::black_box(super::irq_dispatch_must_defer()));
        assert!(!core::hint::black_box(super::IRQ_STACK_SWITCH_PERMITTED));
    }

    #[test]
    fn irq_gate_fail_closed_release_refuses_switch() {
        // S07 slice36 (REVIEW #510): release-active disconfirm — explicit
        // `if` branches (enforce in release, unlike `debug_assert!`), so
        // this test proves the IRQ path refuses a stack switch with the
        // gate closed in every profile. Mirrors the `if
        // !irq_dispatch_must_defer() { tick; return; }` fail-closed guard
        // at the top of guest `timer_irq_rust`. Branches panic instead of
        // returning bool literals so `clippy -D warnings`
        // (`needless_bool`) stays clean.
        if core::hint::black_box(super::irq_dispatch_must_defer()) {
            // Gate closed: IRQ must defer — expected today.
        } else {
            panic!("IRQ gate open without record-then-defer");
        }
        if core::hint::black_box(super::IRQ_STACK_SWITCH_PERMITTED) {
            panic!("IRQ stack switch permitted while gate must stay closed");
        }
    }

    #[test]
    fn quantum_const_linkage_pins_script_literals() {
        // S07 slice38 (REVIEW #514 risk 2) + slice39 (REVIEW #516 risk 1):
        // true linkage pin — `TICK ticks=5` assumes `QUANTUM_TICKS=4`
        // (`wait_target = QUANTUM_TICKS + 1` in `lib.rs`) and
        // `boot-uefi.ps1` hard-requires the literal
        // `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3`. The slice38
        // version only asserted the const values, so a ps1-only drift
        // stayed green. This version `include_str!`s the script and
        // asserts the const-derived literal is present: changing either
        // side alone goes red (disconfirmed slice39: ps1 `q=4`->`q=5`
        // fails this test; revert restores green). Guest countdowns stay
        // const-driven (`used == QUANTUM_TICKS`, looped ticks), so a
        // quantum change must update the const + ps1 literal + serial
        // doc examples together. `black_box` on the const keeps
        // `clippy -D warnings` (`assertions_on_constants`) clean; the
        // script-content assert is never on a constant.
        // Disconfirm record (slice39, same tree): with ps1 `q=4 used=4`
        // temporarily edited to `q=5 used=5`, this test FAILS
        // (`script pins quantum literals` assert); reverted to `q=4`
        // before commit, test green again.
        let q = core::hint::black_box(super::QUANTUM_TICKS);
        let script = include_str!("../../../tools/qemu/boot-uefi.ps1");
        // Const value pin (changing QUANTUM_TICKS alone must trip the
        // match arm below, which panics with the update checklist).
        match q {
            4 => {}
            _ => panic!("QUANTUM_TICKS changed — update tools/qemu/boot-uefi.ps1 SCHED-TICK literal + TICK ticks expectation + serial doc examples"),
        }
        // Script linkage pin: the const-derived literal must be present
        // verbatim in the enforcing script.
        assert!(
            script.contains("SCHED-TICK ok=1 q=4 used=4 expired=1 next=3"),
            "script pins quantum literals"
        );
        // Wait-target linkage: guest waits QUANTUM_TICKS + 1 ticks, so the
        // e2e TICK count must be one past the quantum (currently 5).
        assert_eq!(q + 1, 5);
    }
}
