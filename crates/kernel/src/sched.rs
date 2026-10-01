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

use crate::thread::{validate_threads, SwitchError, Thread, ThreadState, MAX_THREADS, NULL_THREAD};

/// Cooperative scheduler: owns up to `MAX_THREADS` descriptors plus the
/// current-thread id. No `Cell`, no atomics — `&mut self` for mutation,
/// `&self` for picks.
#[derive(Debug, Clone)]
pub struct Scheduler {
    table: [Option<Thread>; MAX_THREADS],
    len: usize,
    current: u64,
}

impl Scheduler {
    /// New empty scheduler with `current` preset (usually the boot thread
    /// id, or `NULL_THREAD` when no thread runs yet). `current` is NOT
    /// validated here — `pick_next` fails closed (`Unknown`) when it is
    /// absent from the table, so construction can never smuggle a target.
    #[must_use]
    pub const fn new(current: u64) -> Self {
        Self {
            table: [None; MAX_THREADS],
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

    /// Live table as a packed array copy for the picker. Packed (no holes):
    /// `spawn` appends, never leaves `None` inside `[..len)`. Copies every
    /// populated slot (so a just-written `table[len]` is included), which
    /// keeps `spawn`'s pre-commit `validate_threads` exact.
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
    /// picker's `Err` with no mutation.
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
                } else if t.id() == self.current && t.state() == ThreadState::Running {
                    let _ = t.set_state(ThreadState::Ready);
                }
            }
            i += 1;
        }
        self.current = next;
        Ok(next)
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
    fn scheduler_holds_no_cell_interior_mutability() {
        // Replace-not-reuse pin (REVIEW #319 finding 2): the scheduler
        // type must not contain the coop interior-mutability primitive —
        // mutation is `&mut self` only. Doc prose names the cell to
        // explain the ban, and the ban list itself would self-match, so
        // fragments are built with `concat!` (no verbatim hit) and matched
        // against non-doc code lines only (`///`/`//!` stripped).
        const SRC: &str = include_str!("sched.rs");
        let banned = [
            concat!("core", "::cell"),
            concat!("Cel", "l<"),
            concat!("Cel", "l::"),
            concat!("Thread", "Mutex::"),
            concat!("Thread", "Mutex {"),
            concat!(": Thread", "Mutex"),
        ];
        let mut in_code = true;
        let bytes = SRC.as_bytes();
        let mut line_start = 0usize;
        let mut li = 0usize;
        while li <= bytes.len() {
            if li == bytes.len() || bytes[li] == b'\n' {
                let line = &SRC[line_start..li];
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
