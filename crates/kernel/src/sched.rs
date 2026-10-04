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
/// S07 slice 2 — fairness: each successful `dispatch` is one equal quantum
/// (no timer/preempt yet — preempt IRQ rides with a future timer slice and
/// reuses the same gated pick). Fairness over N rounds is `max - min` of
/// `runs` (see [`Scheduler::fairness_spread`]): a balanced round-robin
/// sweep holds spread <= 1.
#[derive(Debug, Clone)]
pub struct Scheduler {
    table: [Option<Thread>; MAX_THREADS],
    runs: [u64; MAX_THREADS],
    len: usize,
    current: u64,
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

    /// Pick the next `Ready` thread and move it Ready→Running, parking the
    /// previous `Running` thread back to Ready (cooperative yield). The
    /// previous `Done` thread stays `Done`. On pick refusal returns the
    /// picker's `Err` with no mutation. First-dispatch note: when the
    /// scheduler was built with `new(current)` the ex-current descriptor
    /// is still `Ready`, so nothing is parked on the first success (only
    /// the picked next flips to `Running`).
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
        self.current = next;
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
    /// [`Scheduler::join`]); the `Done` mark is kept either way.
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
                self.current = next;
                Ok(next)
            }
            Err(e) => {
                self.current = NULL_THREAD;
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
}
