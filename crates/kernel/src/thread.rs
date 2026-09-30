//! S06 slice 1 — kernel thread state + stack contract (host-testable).
//!
//! No live context switch yet: this slice pins the data contract the
//! future `swap` stub will consume, so bad stacks/entries fail closed on
//! the host instead of triple-faulting in QEMU.
//!
//! Failure model (fail-closed, never panics in guest path):
//! - `stack_base == 0` → `InvalidStack` (null).
//! - `stack_size < MIN_STACK` (4 KiB) or `stack_size == 0` → `InvalidStack`.
//! - `stack_base` not 16-byte aligned → `InvalidStack` (ABI requirement).
//! - `stack_size` not 16-byte aligned → `InvalidStack` (keeps top aligned).
//! - `stack_base + stack_size` overflow or misaligned top → `InvalidStack`.
//! - `entry == 0` → `InvalidEntry` (null entry point).
//! - `switch` to same id / non-Ready / unknown id → `NoSwitch` /
//!   `NotReady` / `Unknown`, never a register write.
//! - Corrupt table (duplicate id, overlapping stacks, misaligned top)
//!   → `InvalidTable`, never a register write.
//! - Illegal lifecycle (`Done` resurrection, same-state no-op) → refused
//!   (`false`, no mutation); legal: Ready→Running, Running→Ready (yield),
//!   Ready→Done (cancel), Running→Done (exit).
//!
//! Fields are private so `new()` cannot be bypassed by struct-literal
//! construction (closes the S05 pub-widen fail-open pattern).

/// Minimum kernel stack: one page (4 KiB).
pub const MIN_STACK: u64 = 4096;

/// Thread lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    /// Created, stacks validated, never ran.
    Ready,
    /// Currently running.
    Running,
    /// Finished / parked.
    Done,
}

/// Validation failure for thread creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadError {
    /// Bad stack (null base, too small, misaligned, overflow).
    InvalidStack,
    /// Null entry point.
    InvalidEntry,
}

/// Switch refusal (no registers touched).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchError {
    /// Unknown thread id.
    Unknown,
    /// Target not `Ready`.
    NotReady,
    /// Target is already current (no-op refused loudly).
    NoSwitch,
    /// Corrupt table: duplicate id, overlapping stacks, or misaligned top.
    InvalidTable,
}

/// One kernel thread descriptor (data only, no registers yet).
/// Fields are private: construct only via [`Thread::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thread {
    /// Small thread id (index into a future table).
    id: u64,
    /// Current lifecycle state.
    state: ThreadState,
    /// Stack low address (inclusive).
    stack_base: u64,
    /// Stack size in bytes.
    stack_size: u64,
    /// Entry point address (guest `fn()` target).
    entry: u64,
}

impl Thread {
    /// Validate stack + entry, returning the descriptor on success.
    /// Pure `core`-only so host tests and the guest share one rule.
    pub const fn new(
        id: u64,
        stack_base: u64,
        stack_size: u64,
        entry: u64,
    ) -> Result<Self, ThreadError> {
        if stack_base == 0 {
            return Err(ThreadError::InvalidStack);
        }
        if stack_size < MIN_STACK {
            return Err(ThreadError::InvalidStack);
        }
        if stack_base & 0xF != 0 {
            return Err(ThreadError::InvalidStack);
        }
        if stack_size & 0xF != 0 {
            return Err(ThreadError::InvalidStack);
        }
        let top = match stack_base.checked_add(stack_size) {
            Some(t) => t,
            None => return Err(ThreadError::InvalidStack),
        };
        if top & 0xF != 0 {
            return Err(ThreadError::InvalidStack);
        }
        if entry == 0 {
            return Err(ThreadError::InvalidEntry);
        }
        Ok(Self {
            id,
            state: ThreadState::Ready,
            stack_base,
            stack_size,
            entry,
        })
    }

    /// Thread id.
    #[must_use]
    pub const fn id(self) -> u64 {
        self.id
    }

    /// Current lifecycle state.
    #[must_use]
    pub const fn state(self) -> ThreadState {
        self.state
    }

    /// Stack low address (inclusive).
    #[must_use]
    pub const fn stack_base(self) -> u64 {
        self.stack_base
    }

    /// Stack size in bytes.
    #[must_use]
    pub const fn stack_size(self) -> u64 {
        self.stack_size
    }

    /// Entry point address.
    #[must_use]
    pub const fn entry(self) -> u64 {
        self.entry
    }

    /// Cooperative state transition (the picker itself never mutates;
    /// the caller moves Ready->Running->Done through this gate).
    /// Enforced: Ready→Running, Running→Ready (yield), Ready→Done
    /// (cancel), Running→Done (exit). All other edges — including any
    /// `Done` resurrection and same-state no-ops — return `false` with
    /// no mutation (closes the unrestricted-gate risk; picker stays safe
    /// because it only ever picks `Ready`).
    #[must_use]
    pub fn set_state(&mut self, next: ThreadState) -> bool {
        let ok = matches!(
            (self.state, next),
            (ThreadState::Ready, ThreadState::Running)
                | (ThreadState::Running, ThreadState::Ready)
                | (ThreadState::Ready, ThreadState::Done)
                | (ThreadState::Running, ThreadState::Done)
        );
        if ok {
            self.state = next;
        }
        ok
    }

    /// Stack top (exclusive end). Valid only after `new` succeeds.
    #[must_use]
    pub const fn stack_top(self) -> u64 {
        // `new` proved base+size neither overflows nor misaligns, and
        // fields are private so no other construction path exists.
        // Keep the debug assert so a future bypass fails loudly on host.
        debug_assert!(
            self.stack_base.checked_add(self.stack_size).is_some()
                && (self.stack_base.wrapping_add(self.stack_size) & 0xF) == 0
        );
        self.stack_base.saturating_add(self.stack_size)
    }

    /// Checked stack top: `None` if the range overflows (defensive;
    /// `None` is unreachable for `new()`-built threads).
    #[must_use]
    pub const fn checked_top(self) -> Option<u64> {
        self.stack_base.checked_add(self.stack_size)
    }
}

/// Table-level validation shared by the picker: duplicate ids, stack
/// overlaps, and misaligned/overflowing tops all fail closed.
pub fn validate_threads(threads: &[Thread]) -> Result<(), SwitchError> {
    let mut i = 0;
    while i < threads.len() {
        let a = threads[i];
        let a_top = match a.checked_top() {
            Some(t) => t,
            None => return Err(SwitchError::InvalidTable),
        };
        if a_top & 0xF != 0 {
            return Err(SwitchError::InvalidTable);
        }
        let mut j = 0;
        while j < threads.len() {
            if i != j {
                let b = threads[j];
                if a.id == b.id {
                    return Err(SwitchError::InvalidTable);
                }
                let b_top = match b.checked_top() {
                    Some(t) => t,
                    None => return Err(SwitchError::InvalidTable),
                };
                // Half-open overlap: [base, top).
                if a.stack_base < b_top && b.stack_base < a_top {
                    return Err(SwitchError::InvalidTable);
                }
            }
            j += 1;
        }
        i += 1;
    }
    Ok(())
}

/// Pure cooperative pick: true round-robin in slice order — the first
/// `Ready` thread after `current` (wrapping), not the lowest id.
/// No asm, no state mutation — the caller performs the state transition
/// after a successful pick via [`Thread::set_state`].
///
/// Errors: empty table → `Unknown`; current unknown → `Unknown`;
/// corrupt table (duplicate id / overlap / bad top) → `InvalidTable`;
/// no other `Ready` thread → `NotReady` (or `NoSwitch` when the only
/// `Ready` thread is current).
pub fn switch_stub(threads: &[Thread], current: u64) -> Result<u64, SwitchError> {
    if threads.is_empty() {
        return Err(SwitchError::Unknown);
    }
    validate_threads(threads)?;
    let mut current_idx: Option<usize> = None;
    let mut i = 0;
    while i < threads.len() {
        if threads[i].id == current {
            current_idx = Some(i);
            break;
        }
        i += 1;
    }
    let start = match current_idx {
        Some(idx) => idx,
        None => return Err(SwitchError::Unknown),
    };
    // Scan forward from current, wrapping: true RR in slice order.
    let mut step = 1;
    while step <= threads.len() {
        let t = threads[(start + step) % threads.len()];
        if t.id != current && t.state == ThreadState::Ready {
            return Ok(t.id);
        }
        step += 1;
    }
    // No other Ready thread: distinguish no-op from nothing-Ready.
    let current_ready = threads[start].state == ThreadState::Ready;
    if current_ready {
        Err(SwitchError::NoSwitch)
    } else {
        Err(SwitchError::NotReady)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_thread_reports_stack_top() {
        let t = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        assert_eq!(t.state(), ThreadState::Ready);
        assert_eq!(t.id(), 1);
        assert_eq!(t.stack_top(), 0x1000_0000 + 8192);
        assert_eq!(t.checked_top(), Some(0x1000_0000 + 8192));
    }

    #[test]
    fn bad_stacks_fail_closed() {
        // Null base.
        assert_eq!(
            Thread::new(1, 0, 8192, 0x1000),
            Err(ThreadError::InvalidStack)
        );
        // Zero / sub-page size.
        assert_eq!(
            Thread::new(1, 0x1000_0000, 0, 0x1000),
            Err(ThreadError::InvalidStack)
        );
        assert_eq!(
            Thread::new(1, 0x1000_0000, 2048, 0x1000),
            Err(ThreadError::InvalidStack)
        );
        // Misaligned base.
        assert_eq!(
            Thread::new(1, 0x1000_0001, 8192, 0x1000),
            Err(ThreadError::InvalidStack)
        );
        // Misaligned size (top would break 16B ABI).
        assert_eq!(
            Thread::new(1, 0x1000_0000, 5000, 0x1000),
            Err(ThreadError::InvalidStack)
        );
        // Overflow.
        assert_eq!(
            Thread::new(1, u64::MAX - 100, 8192, 0x1000),
            Err(ThreadError::InvalidStack)
        );
    }

    #[test]
    fn null_entry_fails_closed() {
        assert_eq!(
            Thread::new(1, 0x1000_0000, 8192, 0),
            Err(ThreadError::InvalidEntry)
        );
    }

    #[test]
    fn switch_picks_next_ready() {
        let a = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        let b = Thread::new(2, 0x2000_0000, 8192, 0x2000).unwrap();
        assert_eq!(switch_stub(&[a, b], 1), Ok(2));
        assert_eq!(switch_stub(&[a, b], 2), Ok(1));
    }

    #[test]
    fn switch_is_true_round_robin_in_slice_order() {
        // Review #275 counterexample: [1,2,3] current=2 must yield 3
        // (next in slice order), not 1 (lowest id).
        let a = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        let b = Thread::new(2, 0x2000_0000, 8192, 0x2000).unwrap();
        let c = Thread::new(3, 0x3000_0000, 8192, 0x3000).unwrap();
        assert_eq!(switch_stub(&[a, b, c], 2), Ok(3));
        assert_eq!(switch_stub(&[a, b, c], 3), Ok(1));
        assert_eq!(switch_stub(&[a, b, c], 1), Ok(2));
        // Skips non-Ready in wrap order: b Running, current=1 -> 3.
        let mut b_run = b;
        assert!(b_run.set_state(ThreadState::Running));
        assert_eq!(switch_stub(&[a, b_run, c], 1), Ok(3));
    }

    #[test]
    fn switch_refusals_never_touch_registers() {
        let mut running = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        assert!(running.set_state(ThreadState::Running));
        let mut done = Thread::new(2, 0x2000_0000, 8192, 0x2000).unwrap();
        assert!(done.set_state(ThreadState::Done));
        // Nothing Ready (current Running) -> NotReady.
        assert_eq!(switch_stub(&[running, done], 1), Err(SwitchError::NotReady));
        // Unknown current.
        let ready = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        assert_eq!(switch_stub(&[ready], 99), Err(SwitchError::Unknown));
        assert_eq!(switch_stub(&[], 1), Err(SwitchError::Unknown));
        // Only current Ready -> NoSwitch.
        assert_eq!(switch_stub(&[ready, done], 1), Err(SwitchError::NoSwitch));
    }

    #[test]
    fn lifecycle_is_enforced_no_resurrection() {
        let mut t = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        // Same-state no-op refused.
        assert!(!t.set_state(ThreadState::Ready));
        assert_eq!(t.state(), ThreadState::Ready);
        // Dispatch + yield + exit.
        assert!(t.set_state(ThreadState::Running));
        assert!(!t.set_state(ThreadState::Running));
        assert!(t.set_state(ThreadState::Ready));
        assert!(t.set_state(ThreadState::Running));
        assert!(t.set_state(ThreadState::Done));
        // Terminal: any Done edge refused, no mutation.
        assert!(!t.set_state(ThreadState::Ready));
        assert!(!t.set_state(ThreadState::Running));
        assert!(!t.set_state(ThreadState::Done));
        assert_eq!(t.state(), ThreadState::Done);
        // Cancel-before-run is legal.
        let mut u = Thread::new(2, 0x2000_0000, 8192, 0x2000).unwrap();
        assert!(u.set_state(ThreadState::Done));
        assert!(!u.set_state(ThreadState::Ready));
    }

    #[test]
    fn corrupt_tables_fail_closed_before_pick() {
        // Duplicate id.
        let a = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        let dup = Thread::new(1, 0x2000_0000, 8192, 0x2000).unwrap();
        assert_eq!(switch_stub(&[a, dup], 1), Err(SwitchError::InvalidTable));
        assert_eq!(validate_threads(&[a, dup]), Err(SwitchError::InvalidTable));
        // Overlapping stacks.
        let b = Thread::new(2, 0x1000_0800, 8192, 0x2000).unwrap();
        assert_eq!(switch_stub(&[a, b], 1), Err(SwitchError::InvalidTable));
        // Adjacent (touching but not overlapping) is valid.
        let c = Thread::new(2, 0x1000_0000 + 8192, 8192, 0x2000).unwrap();
        assert_eq!(validate_threads(&[a, c]), Ok(()));
        assert_eq!(switch_stub(&[a, c], 1), Ok(2));
    }
}
