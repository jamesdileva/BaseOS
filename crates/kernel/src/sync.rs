//! S06 slice 15 — cooperative thread mutex (host-testable).
//!
//! First synchronization primitive for kernel threads: a single-holder
//! mutex keyed by thread id. Cooperative (no spinning, no blocking):
//! `try_lock` either takes a free lock or refuses loudly, so a bad
//! lock/unlock order fails closed on the host instead of deadlocking
//! in QEMU.
//!
//! Failure model (fail-closed, never panics, `core`-only):
//! - `id == 0` → refused (`false`, no mutation; 0 is the reserved
//!   null-thread sentinel shared with [`crate::thread::Thread`]).
//! - Lock held by another thread → refused (`false`, holder kept).
//! - Same owner re-lock (non-recursive) → refused (`false`, holder kept).
//! - `unlock` by non-holder (including when free) → refused (`false`).
//! - Legal: free → holder (`try_lock` true), holder → free (`unlock` true).
//!
//! Single-threaded cooperative use only: no atomic RMW is needed for the
//! guest path (post-EBS boot CPU, `cli` in effect). The holder travels in
//! a plain `Cell<u64>` so the type stays `!Sync` by construction —
//! sharing across threads must go through a future atomic/interrupt-gated
//! primitive, not this one. The scheduler (slice16 `sched::Scheduler`)
//! REPLACES this cell rather than reusing it: under preemption a
//! `Cell` get-then-set is an interrupt-TOCTOU (two threads can both
//! observe free and both store), so the preemptive path needs an
//! atomic CAS or `cli`-gated critical section, never this `Cell`.
//!
//! Guest scope: out-of-scope for the guest this slice — no guest caller
//! uses the mutex yet (post-EBS single-threaded boot owns all stacks);
//! guest use arrives with the first multi-thread guest switch slice.

use crate::thread::NULL_THREAD;
use core::cell::Cell;

/// Cooperative thread mutex: at most one holder thread id at a time.
/// `0` means free (the reserved null-thread sentinel can never hold it).
#[derive(Debug)]
pub struct ThreadMutex {
    holder: Cell<u64>,
}

impl ThreadMutex {
    /// New free mutex (`holder == 0`).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            holder: Cell::new(NULL_THREAD),
        }
    }

    /// Current holder (`0` when free).
    #[must_use]
    pub fn holder(&self) -> u64 {
        self.holder.get()
    }

    /// True when free.
    #[must_use]
    pub fn is_free(&self) -> bool {
        self.holder.get() == NULL_THREAD
    }

    /// Try to take the lock for `id`. Returns `true` only on
    /// free → `id` with `id != 0`; every other case returns `false`
    /// with no mutation.
    #[must_use]
    pub fn try_lock(&self, id: u64) -> bool {
        if id == NULL_THREAD {
            return false;
        }
        if self.holder.get() != NULL_THREAD {
            return false;
        }
        self.holder.set(id);
        true
    }

    /// Release the lock held by `id`. Returns `true` only when `id`
    /// is the current holder (`id != 0` implied); every other case
    /// returns `false` with no mutation.
    #[must_use]
    pub fn unlock(&self, id: u64) -> bool {
        if id == NULL_THREAD {
            return false;
        }
        if self.holder.get() != id {
            return false;
        }
        self.holder.set(NULL_THREAD);
        true
    }
}

impl Default for ThreadMutex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_unlock_round_trip() {
        let m = ThreadMutex::new();
        assert!(m.is_free());
        assert_eq!(m.holder(), 0);
        assert!(m.try_lock(1));
        assert_eq!(m.holder(), 1);
        assert!(!m.is_free());
        assert!(m.unlock(1));
        assert!(m.is_free());
    }

    #[test]
    fn null_id_never_locks_or_unlocks() {
        let m = ThreadMutex::new();
        assert!(!m.try_lock(0));
        assert!(m.is_free());
        assert!(m.try_lock(1));
        assert!(!m.unlock(0));
        assert_eq!(m.holder(), 1);
        assert!(m.unlock(1));
    }

    #[test]
    fn double_lock_and_foreign_lock_refused() {
        let m = ThreadMutex::new();
        assert!(m.try_lock(1));
        // Non-recursive: same owner re-lock refused, holder kept.
        assert!(!m.try_lock(1));
        assert_eq!(m.holder(), 1);
        // Foreign thread refused, holder kept.
        assert!(!m.try_lock(2));
        assert_eq!(m.holder(), 1);
        assert!(m.unlock(1));
        // After release the waiter can take it.
        assert!(m.try_lock(2));
        assert!(m.unlock(2));
    }

    #[test]
    fn wrong_owner_unlock_refused_no_mutation() {
        let m = ThreadMutex::new();
        // Unlock of a free lock refused.
        assert!(!m.unlock(1));
        assert!(m.is_free());
        assert!(m.try_lock(1));
        // Foreign unlock refused, holder kept.
        assert!(!m.unlock(2));
        assert_eq!(m.holder(), 1);
        assert!(m.unlock(1));
    }
}
