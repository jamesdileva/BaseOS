//! S06 slice 1 — kernel thread state + stack contract (host-testable).
//!
//! No live context switch yet: this slice pins the data contract the
//! future `swap` stub will consume, so bad stacks/entries fail closed on
//! the host instead of triple-faulting in QEMU.
//!
//! Failure model (fail-closed, never panics in guest path):
//! - `id == 0` → `InvalidId` (0 is the reserved null-thread sentinel).
//! - `stack_base == 0` → `InvalidStack` (null).
//! - `stack_size < MIN_STACK` (4 KiB) or `stack_size == 0` → `InvalidStack`.
//! - `stack_base` not 16-byte aligned → `InvalidStack` (ABI requirement).
//! - `stack_size` not 16-byte aligned → `InvalidStack` (keeps top aligned).
//! - `stack_base + stack_size` overflow or misaligned top → `InvalidStack`.
//! - `entry == 0` → `InvalidEntry` (null entry point).
//! - `switch` to same id / non-Ready / unknown id → `NoSwitch` /
//!   `NotReady` / `Unknown`, never a register write.
//! - Corrupt table (duplicate id, id 0 smuggled in, null/degenerate
//!   stack or entry smuggled in, overlapping stacks,
//!   misaligned top, or more than `MAX_THREADS` entries) → `InvalidTable`,
//!   never a register write.
//! - Illegal lifecycle (`Done` resurrection, same-state no-op) → refused
//!   (`false`, no mutation); legal: Ready→Running, Running→Ready (yield),
//!   Ready→Done (cancel), Running→Done (exit).
//!
//! Fields are private so `new()` cannot be bypassed by struct-literal
//! construction (closes the S05 pub-widen fail-open pattern).

/// Minimum kernel stack: one page (4 KiB).
pub const MIN_STACK: u64 = 4096;

/// Maximum threads per pick table (bounds the O(n^2) `validate_threads`
/// scan: 16^2 = 256 pair checks worst case; validate-at-insert or cache
/// is the documented follow-up if the hot path ever grows past this).
pub const MAX_THREADS: usize = 16;

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
    /// Bad id (0 is reserved: null-thread sentinel for the future table).
    InvalidId,
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
    /// Corrupt table: duplicate id, smuggled id 0, smuggled null/
    /// degenerate stack or entry (null base, sub-page/misaligned size,
    /// misaligned base, null entry), overlapping stacks,
    /// misaligned/overflowing top, or more than `MAX_THREADS` entries.
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

/// Shared stack+entry gate: the exact `new()` invariants for the
/// load-bearing values future `swap` asm will consume as RSP/RIP.
///
/// Single source (REVIEW #283 must-fix): `new()`, `validate_threads()`,
/// and `swap_target()` all enforce this one function, so the
/// defense-in-depth re-check in `swap_target` cannot drift back to a
/// top-only gate (a 16 B stack or a misaligned-base-with-aligned-top
/// yields an aligned `top` that passes a solo top check but is
/// lethal/unsafe as a live RSP).
const fn check_stack_entry(
    stack_base: u64,
    stack_size: u64,
    entry: u64,
) -> Result<(), ThreadError> {
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
    Ok(())
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
        if id == 0 {
            return Err(ThreadError::InvalidId);
        }
        match check_stack_entry(stack_base, stack_size, entry) {
            Ok(()) => {}
            Err(e) => return Err(e),
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
        // Fail loud (not saturating) on the unreachable overflow: u64::MAX
        // is 16-byte-misaligned so `validate_threads` rejects it instead
        // of masking the bug with a clamped-but-plausible top.
        debug_assert!(
            self.stack_base.checked_add(self.stack_size).is_some()
                && (self.stack_base.wrapping_add(self.stack_size) & 0xF) == 0
        );
        match self.stack_base.checked_add(self.stack_size) {
            Some(t) => t,
            None => u64::MAX,
        }
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
/// Rejects tables longer than `MAX_THREADS` (bounds the O(n^2) scan),
/// any `id == 0` entry (the reserved null-thread sentinel can only
/// enter via a bypass/deserialize path since `new()` rejects it —
/// validated here so it can never become a pick/swap target), and any
/// entry that violates the other `new()` invariants (`stack_base == 0`,
/// `stack_size < MIN_STACK`, misaligned base/size, `entry == 0`).
/// Without this, a smuggled null entry or degenerate stack would pass
/// validation and `swap_target` would return it as a lethal RIP/RSP
/// (REVIEW #281 gap: `new()` is bypassable by a future
/// bypass/deserialize path exactly as `id == 0` was).
pub fn validate_threads(threads: &[Thread]) -> Result<(), SwitchError> {
    if threads.len() > MAX_THREADS {
        return Err(SwitchError::InvalidTable);
    }
    let mut i = 0;
    while i < threads.len() {
        let a = threads[i];
        if a.id == 0 {
            return Err(SwitchError::InvalidTable);
        }
        // Full `new()` stack+entry invariants via the shared gate
        // (REVIEW #283: `swap_target` solo re-check must match this).
        if check_stack_entry(a.stack_base, a.stack_size, a.entry).is_err() {
            return Err(SwitchError::InvalidTable);
        }
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
/// corrupt table (duplicate id / smuggled id 0 / smuggled null or
/// degenerate stack/entry / overlap / bad top /
/// over `MAX_THREADS`) → `InvalidTable`;
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

/// Live-swap target resolution (host-testable half of the future `swap`
/// stub; still no asm, no register writes).
///
/// Contract the guest `swap` asm MUST obey: resolve through this function
/// (or equivalently `validate_threads` + `checked_top`) BEFORE touching
/// `RSP` — never `stack_top()` alone. `stack_top()` fail-loud sentinel
/// `u64::MAX` is 16-byte-misaligned precisely so a bypass is rejected
/// here, but `u64::MAX` as a raw `RSP` would be lethal, so only the
/// `checked_top()` `Some(top)` value below ever leaves this function.
///
/// Returns `(next_id, next_stack_top, next_entry)` for the same
/// round-robin target `switch_stub` would pick. Any refusal
/// (`Unknown`/`NotReady`/`NoSwitch`/`InvalidTable`, including a `None`
/// top or a misaligned top) yields `Err` with no values to load.
pub fn swap_target(threads: &[Thread], current: u64) -> Result<(u64, u64, u64), SwitchError> {
    let next = switch_stub(threads, current)?;
    let mut i = 0;
    while i < threads.len() {
        if threads[i].id == next {
            // Defense-in-depth: `switch_stub` already ran `validate_threads`,
            // but re-check the FULL `new()` invariants via the shared gate
            // before handing values to future asm (REVIEW #283: a top-only
            // re-check passes a 16 B stack or a misaligned-base-with-
            // aligned-top as an aligned `top`, which is lethal as RSP).
            if check_stack_entry(
                threads[i].stack_base,
                threads[i].stack_size,
                threads[i].entry,
            )
            .is_err()
            {
                return Err(SwitchError::InvalidTable);
            }
            let top = match threads[i].checked_top() {
                Some(t) => t,
                None => return Err(SwitchError::InvalidTable),
            };
            if top & 0xF != 0 {
                return Err(SwitchError::InvalidTable);
            }
            return Ok((next, top, threads[i].entry));
        }
        i += 1;
    }
    Err(SwitchError::Unknown)
}

/// Live RSP swap probe (S06 slices 7-10): run `func` on `new_top`, then
/// restore the caller stack.
///
/// Contract the guest MUST obey: `new_top` is the `Some(top)` value from
/// [`swap_target`] (16-byte aligned, validated through the shared
/// [`check_stack_entry`] gate) — never `stack_top()` alone (its
/// fail-loud `u64::MAX` sentinel would be lethal as a raw `RSP`).
/// `func` is an `extern "C" fn()` whose address was the `entry` half of
/// the same `swap_target` triple. Post-`ExitBootServices` boot CPU,
/// `cli` in effect, single-threaded boot owns both stacks.
///
/// Mechanism (slice 10, REVIEW #291 gap 2 fix): the caller `RSP` is
/// spilled to the NEW stack (`push rax` after `mov rsp, top`), never to
/// a register. The slice 7-9 design stashed it in `R15` (`mov r15,rsp` /
/// `mov rsp,r15`), which assumes the callee preserves `r15` — a thread
/// entry that trashes `r15` loses the return stack and triple-faults, and
/// the slice 9 stress stub deliberately avoided `r15` so the vulnerable
/// path was untested. The new-stack slot removes that single point of
/// failure: even a callee that overwrites every callee-saved register
/// (including `r15`, now covered by the extended clobber stub) cannot
/// lose the return address because it lives in memory on the new stack,
/// above the `call` return address (`[top-8]` vs `[top-16]`), which the
/// stub never touches (it only writes registers + `ret`).
/// `call func` pushes its return address onto the NEW stack, so an
/// unmapped/guard stack faults loudly instead of silently passing. A bad
/// `RSP` triple-faults or `#PF`s and the e2e script fails on the missing
/// `THREAD` lines — fail-loud, never silent.
///
/// Slice 9-10: the probe additionally preserves the full callee-saved set
/// (`rbx`, `rbp`, `r12`-`r15`) with symmetric `push`/`pop` on the caller
/// stack, so a hostile callee that trashes them cannot corrupt the boot
/// thread. Push order is the pop order reversed (balanced by construction);
/// the e2e `THREAD-CS` stress loop calls a naked clobber stub 16 times and
/// fails loudly on any imbalance. Slice 10 proves PRESERVATION (not just
/// liveness, REVIEW #291 gap 1): the caller-side sentinel check
/// (`probe_thread_callee_saved` in the guest) sets known values in all six
/// regs around one `run_on_thread_stack` call and fails loud (`ok=0`) on
/// any bit mismatch — a swapped pop order (e.g. r13/r14) still balances
/// the stack and would print `ok=1` under the old liveness-only gate.
///
/// REVIEW #287 risk 1 (must-fix): the template names literal regs as
/// scratch, which the register allocator does NOT reserve on its own —
/// an `in(reg)` input allocated in e.g. `rbx` would make `push rbx` spill
/// the input instead of the caller value and corrupt the restore. The body
/// below declares `out(...) _` for every literal reg it touches (`rbx`,
/// `rbp`, `r12`-`r15` plus `rax` scratch) so the allocator excludes them
/// from both inputs (conservative: push/pop actually preserves the six,
/// but claiming clobber is sound — the compiler simply saves around us
/// if it needed them). Do not remove those lines without a
/// `cargo show-asm` / regalloc-stress disproof.
///
/// # Safety
/// See contract above. Host `cargo test` builds compile this out (the
/// host harness must never execute `mov rsp, ...`).
#[cfg(not(test))]
pub unsafe fn run_on_thread_stack(new_top: u64, func: extern "C" fn()) {
    // `rbx`/`rbp` need no `out` reservation: LLVM reserves `rbx`
    // internally and `rbp` is the frame pointer, so `in(reg)` inputs are
    // never allocated there (the compiler rejects `out("rbx")`/`out("rbp")`
    // outright). `r12`-`r15`+`rax` ARE allocatable, hence the `out`s below.
    core::arch::asm!(
        "push r15",
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "mov rax, rsp",
        "mov rsp, {top}",
        "push rax",
        "call {func}",
        "pop rax",
        "mov rsp, rax",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",
        "pop r15",
        top = in(reg) new_top,
        func = in(reg) func,
        out("rax") _,
        out("r12") _,
        out("r13") _,
        out("r14") _,
        out("r15") _,
        clobber_abi("C"),
    );
}

/// Host-test stub mirroring the guest signature so call sites stay
/// symmetric; never invoked by tests (live `RSP` proof is e2e).
///
/// # Safety
/// Host `cargo test` only: no-op, never touches registers. The real
/// guest body (`cfg(not(test))`) carries the full post-EBS contract.
#[cfg(test)]
pub unsafe fn run_on_thread_stack(_new_top: u64, _func: extern "C" fn()) {}

/// Two-way `RSP` save/restore probe (S06 slice 12): like
/// [`run_on_thread_stack`], but the caller `RSP` is additionally saved to
/// caller-owned memory (`*old_save`) BEFORE the stack switch, so two
/// thread stacks can ping-pong through memory slots instead of a single
/// return slot.
///
/// Mechanism: caller-saved pushes (same six as `run_on_thread_stack`),
/// then `mov [old_save], rax` spills the boot `RSP` to memory while still
/// on the boot stack; `mov rsp, top`; `push rax` (boot `RSP`) + `push
/// old_save` (pointer spill, so post-`call` recovery never depends on a
/// register surviving the hostile callee — the slice 10 lesson applied to
/// the pointer itself); `call func`; `pop rdx` (pointer) + `pop rax`
/// (boot `RSP`); `mov rsp, rax`; caller-saved pops. Fail-closed: null
/// `old_save` returns with no register or stack touch (Rust guard before
/// the asm, host-testable).
///
/// # Safety
/// Same contract as [`run_on_thread_stack`]: guest-only, post-EBS, `cli`
/// in effect, `new_top`/`func` from [`swap_target`], `old_save` non-null,
/// 8-byte aligned, writable caller memory. Host `cargo test` builds
/// compile to a no-op stub (never touches registers or memory).
#[cfg(not(test))]
pub unsafe fn run_on_thread_stack_with_save(
    old_save: *mut u64,
    new_top: u64,
    func: extern "C" fn(),
) {
    if old_save.is_null() {
        return;
    }
    // Same reservation rule as `run_on_thread_stack`: `rbx`/`rbp` need no
    // `out` (LLVM-reserved / frame pointer, compiler rejects the `out`);
    // every other literal reg touched (`r12`-`r15` + `rax`/`rdx` scratch)
    // is declared `out` so inputs are never allocated there.
    core::arch::asm!(
        "push r15",
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "mov rax, rsp",
        "mov [{save}], rax",
        "mov rsp, {top}",
        "push rax",
        "push {save}",
        "call {func}",
        "pop rdx",
        "pop rax",
        "mov rsp, rax",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",
        "pop r15",
        save = in(reg) old_save,
        top = in(reg) new_top,
        func = in(reg) func,
        out("rax") _,
        out("rdx") _,
        out("r12") _,
        out("r13") _,
        out("r14") _,
        out("r15") _,
        clobber_abi("C"),
    );
}

/// Host-test stub: null fails closed (no touch), non-null is a register-
/// safe no-op (live `RSP` proof is e2e-only).
///
/// # Safety
/// Host `cargo test` only: no-op.
#[cfg(test)]
pub unsafe fn run_on_thread_stack_with_save(
    old_save: *mut u64,
    _new_top: u64,
    _func: extern "C" fn(),
) {
    if old_save.is_null() {}
}

/// Caller-side preservation proof (S06 slice 10, REVIEW #291 gap 1).
///
/// Liveness (`THREAD-CS` 16x returns) does NOT prove preservation: pops in
/// the wrong order (e.g. swapped `r13`/`r14`) still balance the stack and
/// print `ok=1`. This check sets known sentinels in five callee-saved regs
/// (`rbx`, `r12`-`r15`) in ONE asm block, runs ONE `run_on_thread_stack`
/// call, then captures all five in ONE asm block and compares bit-exact.
/// Any mismatch (wrong pop order, missing spill) returns `false` and the
/// guest fails loud (`THREAD-CS ok=0`).
///
/// `rbp` is deliberately EXCLUDED: it is the frame pointer, so overwriting
/// it with a sentinel across a Rust call would corrupt frame-relative
/// accesses and the epilogue restore. Five distinct sentinels still catch
/// any pop-order permutation: a permutation that misplaces only `rbp`
/// alone is impossible (a swap needs two regs, and any swap touching a
/// checked reg misplaces its distinct sentinel).
///
/// `rbx` needs no `out`: LLVM reserves it, so `in(reg)` inputs are never
/// allocated there (the compiler rejects `out("rbx")` outright, same for
/// `rbp`). `r12`-`r15` ARE allocatable, hence the `out`s in the set block
/// so sentinels cannot alias an input. The capture block moves each reg to
/// a general output (`mov {v}, rbx`) with no pinned outs.
///
/// # Safety
/// Same contract as [`run_on_thread_stack`]: guest-only, post-EBS, `cli`
/// in effect, `new_top`/`func` from [`swap_target`]. Host builds compile
/// to a stub returning `true` (never touches registers).
#[cfg(not(test))]
pub unsafe fn callee_saved_survives(new_top: u64, func: extern "C" fn()) -> bool {
    const S_RBX: u64 = 0x1B1B_1B1B_1B1B_1B1B;
    const S_R12: u64 = 0x1212_1212_1212_1212;
    const S_R13: u64 = 0x1313_1313_1313_1313;
    const S_R14: u64 = 0x1414_1414_1414_1414;
    const S_R15: u64 = 0x1515_1515_1515_1515;
    core::arch::asm!(
        "mov rbx, {s_rbx}",
        "mov r12, {s_r12}",
        "mov r13, {s_r13}",
        "mov r14, {s_r14}",
        "mov r15, {s_r15}",
        s_rbx = in(reg) S_RBX,
        s_r12 = in(reg) S_R12,
        s_r13 = in(reg) S_R13,
        s_r14 = in(reg) S_R14,
        s_r15 = in(reg) S_R15,
        out("r12") _,
        out("r13") _,
        out("r14") _,
        out("r15") _,
        options(nomem, nostack, preserves_flags),
    );
    run_on_thread_stack(new_top, func);
    let (v_rbx, v_r12, v_r13, v_r14, v_r15): (u64, u64, u64, u64, u64);
    core::arch::asm!(
        "mov {v_rbx}, rbx",
        "mov {v_r12}, r12",
        "mov {v_r13}, r13",
        "mov {v_r14}, r14",
        "mov {v_r15}, r15",
        v_rbx = out(reg) v_rbx,
        v_r12 = out(reg) v_r12,
        v_r13 = out(reg) v_r13,
        v_r14 = out(reg) v_r14,
        v_r15 = out(reg) v_r15,
        options(nomem, nostack, preserves_flags),
    );
    v_rbx == S_RBX && v_r12 == S_R12 && v_r13 == S_R13 && v_r14 == S_R14 && v_r15 == S_R15
}

/// Host stub: preservation proof is e2e-only; always `true` so host tests
/// stay register-safe.
///
/// # Safety
/// Host `cargo test` only: no-op.
#[cfg(test)]
pub unsafe fn callee_saved_survives(_new_top: u64, _func: extern "C" fn()) -> bool {
    true
}

/// Read-only `rbp` preservation proof (S06 slice 11, REVIEW #293 risk 1).
///
/// `rbp` is the frame pointer, so it can never be covered by the sentinel
/// write in [`callee_saved_survives`] (overwriting it across a Rust call
/// corrupts frame-relative accesses and the epilogue restore). This check
/// therefore never WRITES `rbp`: it captures `rbp` before and after one
/// `run_on_thread_stack` call and compares bit-exact. A missing or
/// unbalanced `push rbp` / `pop rbp` in `run_on_thread_stack` (or a
/// callee that escapes the caller-stack restore) returns `false` and the
/// guest fails loud (`THREAD-CS ok=0`).
///
/// Capture uses `mov {v}, rbp` into a general `out(reg)` with no pinned
/// outs; `rbx`/`rbp` need no reservation (LLVM reserves `rbx` internally
/// and `rbp` is the frame pointer, so `in(reg)`/`out(reg)` are never
/// allocated there — the compiler rejects `out("rbx")`/`out("rbp")`
/// outright, same rule documented on [`run_on_thread_stack`]).
///
/// # Safety
/// Same contract as [`run_on_thread_stack`]: guest-only, post-EBS, `cli`
/// in effect, `new_top`/`func` from [`swap_target`]. Host builds compile
/// to a stub returning `true` (never touches registers).
#[cfg(not(test))]
pub unsafe fn rbp_preserved(new_top: u64, func: extern "C" fn()) -> bool {
    let before: u64;
    core::arch::asm!(
        "mov {v}, rbp",
        v = out(reg) before,
        options(nomem, nostack, preserves_flags),
    );
    run_on_thread_stack(new_top, func);
    let after: u64;
    core::arch::asm!(
        "mov {v}, rbp",
        v = out(reg) after,
        options(nomem, nostack, preserves_flags),
    );
    before == after
}

/// Host stub: `rbp` proof is e2e-only; always `true` so host tests stay
/// register-safe.
///
/// # Safety
/// Host `cargo test` only: no-op.
#[cfg(test)]
pub unsafe fn rbp_preserved(_new_top: u64, _func: extern "C" fn()) -> bool {
    true
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
    fn id_zero_reserved_and_table_capped() {
        // Id 0 is the reserved null-thread sentinel.
        assert_eq!(
            Thread::new(0, 0x1000_0000, 8192, 0x1000),
            Err(ThreadError::InvalidId)
        );
        // MAX_THREADS adjacent valid threads pass; one more fails closed.
        let mut ok_table = [Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap(); MAX_THREADS];
        let mut i = 0;
        while i < MAX_THREADS {
            ok_table[i] = Thread::new(
                (i as u64) + 1,
                0x1000_0000 + (i as u64) * 8192,
                8192,
                0x1000 + (i as u64),
            )
            .unwrap();
            i += 1;
        }
        assert_eq!(validate_threads(&ok_table), Ok(()));
        let mut big = [Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap(); MAX_THREADS + 1];
        let mut j = 0;
        while j < MAX_THREADS + 1 {
            big[j] = Thread::new(
                (j as u64) + 1,
                0x1000_0000 + (j as u64) * 8192,
                8192,
                0x1000 + (j as u64),
            )
            .unwrap();
            j += 1;
        }
        assert_eq!(validate_threads(&big), Err(SwitchError::InvalidTable));
        // Over-cap pick refuses before touching registers too.
        assert_eq!(switch_stub(&big, 1), Err(SwitchError::InvalidTable));
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
        // Smuggled id 0 (bypasses `new()` via struct literal — the test
        // module can name private fields; outside code cannot).
        let smuggled = Thread {
            id: 0,
            state: ThreadState::Ready,
            stack_base: 0x3000_0000,
            stack_size: 8192,
            entry: 0x3000,
        };
        assert_eq!(
            validate_threads(&[a, smuggled]),
            Err(SwitchError::InvalidTable)
        );
        assert_eq!(
            switch_stub(&[a, smuggled], 1),
            Err(SwitchError::InvalidTable)
        );
        assert_eq!(
            swap_target(&[a, smuggled], 1),
            Err(SwitchError::InvalidTable)
        );
        // Overlapping stacks.
        let b = Thread::new(2, 0x1000_0800, 8192, 0x2000).unwrap();
        assert_eq!(switch_stub(&[a, b], 1), Err(SwitchError::InvalidTable));
        // Adjacent (touching but not overlapping) is valid.
        let c = Thread::new(2, 0x1000_0000 + 8192, 8192, 0x2000).unwrap();
        assert_eq!(validate_threads(&[a, c]), Ok(()));
        assert_eq!(switch_stub(&[a, c], 1), Ok(2));
    }

    #[test]
    fn smuggled_stack_entry_invariants_fail_closed() {
        // REVIEW #281 gap: `validate_threads` must enforce every `new()`
        // invariant, since a future bypass/deserialize path can smuggle
        // them exactly as it smuggles id 0. Each literal below passes the
        // old gate (aligned non-overflowing top) but must still fail.
        let a = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        let cases = [
            // Null entry -> lethal RIP.
            Thread {
                id: 2,
                state: ThreadState::Ready,
                stack_base: 0x2000_0000,
                stack_size: 8192,
                entry: 0,
            },
            // Null stack base.
            Thread {
                id: 2,
                state: ThreadState::Ready,
                stack_base: 0,
                stack_size: 8192,
                entry: 0x2000,
            },
            // Sub-page size (16-byte aligned, so the old top-only gate
            // would pass it).
            Thread {
                id: 2,
                state: ThreadState::Ready,
                stack_base: 0x2000_0000,
                stack_size: 16,
                entry: 0x2000,
            },
            // Misaligned base with an aligned top (base 1 + size 8191 =
            // aligned top, so the old top-only gate would pass it).
            Thread {
                id: 2,
                state: ThreadState::Ready,
                stack_base: 0x2000_0001,
                stack_size: 8191,
                entry: 0x2000,
            },
            // Misaligned size.
            Thread {
                id: 2,
                state: ThreadState::Ready,
                stack_base: 0x2000_0000,
                stack_size: 5000,
                entry: 0x2000,
            },
        ];
        let mut k = 0;
        while k < cases.len() {
            let bad = cases[k];
            assert_eq!(validate_threads(&[a, bad]), Err(SwitchError::InvalidTable));
            assert_eq!(switch_stub(&[a, bad], 1), Err(SwitchError::InvalidTable));
            assert_eq!(swap_target(&[a, bad], 1), Err(SwitchError::InvalidTable));
            k += 1;
        }
    }

    #[test]
    fn swap_target_solo_gate_matches_new_invariants() {
        // REVIEW #283: the old `swap_target` solo re-check (entry==0 +
        // top None/align only) passes both cases below with an aligned
        // `top`, so a future `validate_threads` bypass would hand asm a
        // lethal/unsafe RSP. The shared `check_stack_entry` gate must
        // reject them exactly as `new()` does.
        assert_eq!(
            check_stack_entry(0x2000_0000, 16, 0x2000),
            Err(ThreadError::InvalidStack)
        );
        assert_eq!(
            check_stack_entry(0x2000_0001, 8191, 0x2000),
            Err(ThreadError::InvalidStack)
        );
        // Sanity: valid triple passes, and `new()` agrees on all three.
        assert_eq!(check_stack_entry(0x2000_0000, 8192, 0x2000), Ok(()));
        assert_eq!(
            Thread::new(9, 0x2000_0000, 16, 0x2000),
            Err(ThreadError::InvalidStack)
        );
        assert_eq!(
            Thread::new(9, 0x2000_0001, 8191, 0x2000),
            Err(ThreadError::InvalidStack)
        );
    }

    #[test]
    fn swap_target_resolves_checked_top_and_entry() {
        let a = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        let b = Thread::new(2, 0x2000_0000, 8192, 0x2000).unwrap();
        // Same RR target as the picker, plus the only RSP-safe top.
        assert_eq!(swap_target(&[a, b], 1), Ok((2, 0x2000_0000 + 8192, 0x2000)));
        assert_eq!(swap_target(&[a, b], 2), Ok((1, 0x1000_0000 + 8192, 0x1000)));
        // Refusals carry no values to load into registers.
        let mut running = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        assert!(running.set_state(ThreadState::Running));
        let mut done = Thread::new(2, 0x2000_0000, 8192, 0x2000).unwrap();
        assert!(done.set_state(ThreadState::Done));
        assert_eq!(swap_target(&[running, done], 1), Err(SwitchError::NotReady));
        assert_eq!(swap_target(&[], 1), Err(SwitchError::Unknown));
    }

    extern "C" fn dummy_thread_entry() {}

    #[test]
    fn live_swap_probe_is_unsafe_fn_shape() {
        // S06 slice 7: the live `RSP` swap must stay `unsafe fn` (raw
        // `mov rsp, ...` + indirect `call`) — bind to the `unsafe fn`
        // type to prove callers need an `unsafe` block. Never called
        // (would switch stacks outside the guest).
        let probe: unsafe fn(u64, extern "C" fn()) = run_on_thread_stack;
        let _ = probe;
        // S06 slice 11: both preservation proofs keep the same shape.
        let sentinel: unsafe fn(u64, extern "C" fn()) -> bool = callee_saved_survives;
        let _ = sentinel;
        let rbp_only: unsafe fn(u64, extern "C" fn()) -> bool = rbp_preserved;
        let _ = rbp_only;
        // S06 slice 12: two-way save-slot probe keeps the unsafe-fn shape
        // (raw `mov [save], rsp` + `mov rsp, ...` + indirect `call`).
        let with_save: unsafe fn(*mut u64, u64, extern "C" fn()) = run_on_thread_stack_with_save;
        let _ = with_save;
    }

    #[test]
    fn swap_target_values_are_stack_safe_for_live_swap() {
        // S06 slice 7 pre-condition pin: the triple the guest hands to
        // `run_on_thread_stack` is the `swap_target` triple — aligned
        // `Some(top)` from `checked_top` (never the `u64::MAX`
        // `stack_top()` sentinel) plus a nonzero entry, agreeing with
        // `new()` on both stacks.
        let a = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        let b = Thread::new(2, 0x2000_0000, 8192, 0x2000).unwrap();
        let (id, top, entry) = swap_target(&[a, b], 1).unwrap();
        assert_eq!(id, 2);
        assert_eq!(top, b.checked_top().unwrap());
        assert_eq!(top & 0xF, 0);
        assert_ne!(top, u64::MAX);
        assert_eq!(entry, 0x2000);
        assert_ne!(entry, 0);
        // Host-side shape: a dummy `extern "C" fn()` coerces to the
        // probe's second parameter type.
        let f: extern "C" fn() = dummy_thread_entry;
        let _ = f;
    }

    #[test]
    fn swap_entry_round_trips_through_u64() {
        // REVIEW #287 risk 2 host half: the guest calls through the `entry`
        // half of the `swap_target` triple (u64 -> fn transmute), not a
        // direct reference. Pin the round-trip host-side: fn -> u64 ->
        // fn preserves the address and stays callable (called here on the
        // host stack — no RSP swap — to prove the transmute shape).
        extern "C" fn probe_entry() {}
        let addr = probe_entry as *const () as u64;
        assert_ne!(addr, 0);
        let back: extern "C" fn() = unsafe { core::mem::transmute(addr as *const ()) };
        back();
        // And the `swap_target` triple carries a round-trippable entry:
        // store a real fn address as the entry and resolve it back.
        let a = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        let b = Thread::new(2, 0x2000_0000, 8192, addr).unwrap();
        let (id, _top, entry) = swap_target(&[a, b], 1).unwrap();
        assert_eq!(id, 2);
        assert_eq!(entry, addr);
        let back2: extern "C" fn() = unsafe { core::mem::transmute(entry as *const ()) };
        back2();
    }

    #[test]
    fn with_save_null_fails_closed_host_stub() {
        // S06 slice 12: null `old_save` must fail closed (no touch, no
        // trap) on the host stub; non-null is a register-safe no-op that
        // leaves the slot untouched (live save is e2e-only).
        extern "C" fn dummy() {}
        unsafe {
            run_on_thread_stack_with_save(core::ptr::null_mut(), 0x2000_0000, dummy);
            let mut slot: u64 = 0x1234_5678_9ABC_DEF0;
            run_on_thread_stack_with_save(&mut slot as *mut u64, 0x2000_0000, dummy);
            assert_eq!(slot, 0x1234_5678_9ABC_DEF0);
        }
    }

    #[test]
    fn swap_target_round_trip_stress() {
        // S06 slice 9: ping-pong the round-robin target 1024 times; the
        // triple must be bit-stable every iteration (no hidden counter or
        // first-call-only path in the picker). Also pins the callee-saved
        // contract the guest asm upholds: both tops stay 16-byte aligned
        // and nonzero-entry across the whole run.
        let a = Thread::new(1, 0x1000_0000, 8192, 0x1000).unwrap();
        let b = Thread::new(2, 0x2000_0000, 8192, 0x2000).unwrap();
        let pair = [a, b];
        let mut current = 1u64;
        let mut i = 0;
        while i < 1024 {
            let (id, top, entry) = swap_target(&pair, current).unwrap();
            if current == 1 {
                assert_eq!((id, top, entry), (2, 0x2000_0000 + 8192, 0x2000));
            } else {
                assert_eq!((id, top, entry), (1, 0x1000_0000 + 8192, 0x1000));
            }
            assert_eq!(top & 0xF, 0);
            assert_ne!(entry, 0);
            current = id;
            i += 1;
        }
    }
}
