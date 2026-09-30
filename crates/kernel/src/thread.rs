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
//! - `stack_base + stack_size` overflow → `InvalidStack`.
//! - `entry == 0` → `InvalidEntry` (null entry point).
//! - `switch` to same id / non-Ready / unknown id → `NoSwitch` /
//!   `NotReady` / `Unknown`, never a register write.
//!
//! `switch_stub` is a pure next-thread picker (round-robin over Ready)
//! with zero asm — the real register swap rides with slice 2 + QEMU proof.

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
}

/// One kernel thread descriptor (data only, no registers yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thread {
    /// Small thread id (index into a future table).
    pub id: u64,
    /// Current lifecycle state.
    pub state: ThreadState,
    /// Stack low address (inclusive).
    pub stack_base: u64,
    /// Stack size in bytes.
    pub stack_size: u64,
    /// Entry point address (guest `fn()` target).
    pub entry: u64,
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
        if stack_base.checked_add(stack_size).is_none() {
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

    /// Stack top (exclusive end). Valid only after `new` succeeds.
    #[must_use]
    pub const fn stack_top(self) -> u64 {
        // `new` already proved no overflow, so saturating add == exact add.
        self.stack_base.saturating_add(self.stack_size)
    }
}

/// Pure cooperative pick: first `Ready` thread with `id != current`,
/// round-robin by id order of the slice. No asm, no state mutation —
/// the caller performs the state transition after a successful pick.
///
/// Errors: empty table → `Unknown`; current unknown → `Unknown`;
/// no other `Ready` thread → `NotReady` (or `NoSwitch` when the only
/// `Ready` thread is current).
pub fn switch_stub(threads: &[Thread], current: u64) -> Result<u64, SwitchError> {
    if threads.is_empty() {
        return Err(SwitchError::Unknown);
    }
    let mut found_current = false;
    for t in threads {
        if t.id == current {
            found_current = true;
            break;
        }
    }
    if !found_current {
        return Err(SwitchError::Unknown);
    }
    // Collect candidates: Ready and not current, lowest id first.
    let mut best: Option<u64> = None;
    for t in threads {
        if t.id != current && t.state == ThreadState::Ready {
            match best {
                None => best = Some(t.id),
                Some(b) => {
                    if t.id < b {
                        best = Some(t.id);
                    }
                }
            }
        }
    }
    match best {
        Some(id) => Ok(id),
        None => {
            // Distinguish "only current is Ready" (no-op) from "nothing Ready".
            let current_ready = threads
                .iter()
                .any(|t| t.id == current && t.state == ThreadState::Ready);
            if current_ready {
                Err(SwitchError::NoSwitch)
            } else {
                Err(SwitchError::NotReady)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_thread_reports_stack_top() {
        let t = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        assert_eq!(t.state, ThreadState::Ready);
        assert_eq!(t.stack_top(), 0x1000_0000 + 8192);
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
    fn switch_refusals_never_touch_registers() {
        let mut running = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        running.state = ThreadState::Running;
        let mut done = Thread::new(2, 0x2000_0000, 8192, 0x2000).unwrap();
        done.state = ThreadState::Done;
        // Nothing Ready (current Running) -> NotReady.
        assert_eq!(switch_stub(&[running, done], 1), Err(SwitchError::NotReady));
        // Unknown current.
        let ready = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        assert_eq!(switch_stub(&[ready], 99), Err(SwitchError::Unknown));
        assert_eq!(switch_stub(&[], 1), Err(SwitchError::Unknown));
        // Only current Ready -> NoSwitch.
        assert_eq!(switch_stub(&[ready, done], 1), Err(SwitchError::NoSwitch));
    }
}
