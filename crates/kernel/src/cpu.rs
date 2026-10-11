//! Safe CPU/table boundary (S05 slice 3).
//!
//! Privileged descriptor loads (`lgdt`/`lidt` + segment reloads) have no
//! safe equivalent in a `no_std` guest, so the `unsafe` stays confined to
//! the two bodies ([`load_gdt`] / [`load_idt`]) below plus the stable
//! statics they consume. Everything else in this module is a **safe**
//! wrapper or pure helper: [`gate_for`] pins the kernel code selector so
//! callers cannot wire a gate into the wrong segment, and
//! [`probe_addr_valid`] pins the live-`#PF` guard contract (canonical
//! low-half, 4 KiB-aligned, above firmware-mapped RAM) without touching
//! hardware. Slice 5 adds [`cr3_pml4_base`] / [`efer_nxe_enabled`] pure
//! decode wrappers over `page_table` so probe code shares one rule.
//! Slice 6 moves the live register reads ([`read_cr3`] / [`read_efer`],
//! ex-boot-uefi D7/D8) beside the decode wrappers so one module owns the
//! read + decode pair; `boot-uefi` probes call them via `crate::cpu::read_*`.
//! Caller contract for the unsafe loads (documented, not
//! compiler-enforced): post-`ExitBootServices` boot CPU, `cli` in effect,
//! statics outlive use.

use super::page_table::PageFlags;
use super::KERNEL_CODE_SEL;
use super::{tss_desc_high, tss_desc_low, TSS_SEL, USER_CODE_SEL, USER_DATA_SEL};
#[cfg(not(test))]
use super::{GDT_CODE, GDT_DATA, GDT_NULL, GDT_USER_CODE, GDT_USER_DATA, VECTOR_DE, VECTOR_PF};
#[cfg(not(test))]
use crate::timer::VECTOR_TIMER;

/// Number of GDT slots: null + kernel code/data + user code/data (S09
/// slice2c) + one 16-byte TSS descriptor occupying slots 5-6 (S09 slice2g).
/// Single-sources the guest table length so the `TSS_SEL` index pin and the
/// `lgdt` limit cannot drift apart.
pub(crate) const GDT_LEN: usize = 7;
/// GDT index of the TSS descriptor (low half); `TSS_SEL == index * 8`.
pub(crate) const GDT_TSS_INDEX: usize = 5;

/// Minimal 64-bit Task State Segment (Intel SDM Vol 3 Fig 7-4, 104 bytes):
/// only `rsp0` matters for CPL3->0 stack switching; every IST entry stays 0
/// (single stack, no IST this slice) and `iomap_base == size` marks an empty
/// I/O bitmap. `Tss::new()` constructs `rsp0 == 0`; the guest `load_gdt`
/// overwrites it with the ring-0 stack top before `ltr`, so a live `TR`
/// never points at a zero `rsp0`.
///
/// `repr(C, packed)`: the hardware layout puts `rsp0` at offset 4
/// (unaligned `u64`); plain `repr(C)` would insert 4 padding bytes and yield
/// 112 bytes. Field access must go through `addr_of!` + `read_unaligned`.
#[repr(C, packed)]
pub(crate) struct Tss {
    pub(crate) reserved0: u32,
    pub(crate) rsp0: u64,
    pub(crate) rsp1: u64,
    pub(crate) rsp2: u64,
    pub(crate) reserved1: u64,
    pub(crate) ist: [u64; 7],
    pub(crate) reserved2: u64,
    pub(crate) reserved3: u16,
    pub(crate) iomap_base: u16,
}

impl Tss {
    /// Zero TSS with an empty I/O bitmap (`iomap_base == size`, no bitmap).
    pub(crate) const fn new() -> Self {
        Self {
            reserved0: 0,
            rsp0: 0,
            rsp1: 0,
            rsp2: 0,
            reserved1: 0,
            ist: [0; 7],
            reserved2: 0,
            reserved3: 0,
            iomap_base: 104,
        }
    }
}

/// Minimal long-mode GDT: null, kernel code (`0x08`), kernel data (`0x10`),
/// user code DPL3 (`0x1B`), user data DPL3 (`0x23`), plus the 16-byte TSS
/// descriptor in slots 5-6 (S09 slice2g wires `TR` via `ltr`; the guest still
/// enters ring 0 with no `iretq`/user stacks yet, so the wider table is
/// behavior-preserving).
/// Static so `lgdt` keeps a stable base after the function returns.
/// Guest-only (`cfg(not(test))`) — host tests cover the descriptor constants
/// without constructing the table.
#[cfg(not(test))]
#[repr(C, align(16))]
struct Gdt {
    entries: [u64; GDT_LEN],
}

#[cfg(not(test))]
static mut GDT: Gdt = Gdt {
    entries: [
        GDT_NULL,
        GDT_CODE,
        GDT_DATA,
        GDT_USER_CODE,
        GDT_USER_DATA,
        0,
        0,
    ],
};

/// Ring-0 stack for CPL3->0 transitions (S09 slice2h, #30 remainder):
/// 16 KiB, single stack, no IST. `TSS.rsp0` points at the top (one-past-end)
/// so a future CPL3->0 entry has somewhere to push SS/RSP/RFLAGS/CS/RIP +
/// the error code. Wired in `load_gdt` (guest-only); `ltr` alone never
/// consults it, so the nonzero value is still boot-neutral with no CPL3
/// entry yet (QEMU serial-identical). Fail-closed note: `iomap_base == 104`
/// keeps any CPL3 port access `#GP` until an I/O policy slice exists.
pub(crate) const RING0_STACK_LEN: usize = 16384;

/// Top (one-past-end) of a ring-0 stack blob: `RSP` must start 16-byte
/// aligned per the SysV/Win64 ABI, and the `align(16)` wrapper below keeps
/// every real base 16-aligned so the top stays aligned too.
pub(crate) const fn ring0_stack_top_for(base: u64) -> u64 {
    base.saturating_add(RING0_STACK_LEN as u64)
}

/// `repr(C, align(16))` so the base — and hence the `ring0_stack_top_for`
/// top — is 16-byte aligned. Mirrors the `timer::IrqStack` wrapper shape.
#[cfg(not(test))]
#[repr(C, align(16))]
struct Ring0Stack([u8; RING0_STACK_LEN]);

/// Future CPL3 user-stack blob (S09 slice2t, #30 remainder): 16 KiB
/// (`USER_STACK_LEN`), same `align(16)` shape as [`Ring0Stack`] so the base
/// — and hence the [`user_stack_top_for`] top — is 16-byte aligned
/// (SysV/Win64 ABI entry requirement, shared with [`user_rsp_valid`]).
/// Guest-only static `USER_STACK` below owns the bytes; unused this slice
/// (no mapping, no `iretq`), so boot-neutral by construction
/// (QEMU serial-identical). Future stub must map all 4 pages U+W+NX and
/// source per-page flags from the walker (`walk_full_leaf_flags` per page)
/// into [`user_entry_cpl3_allowed`] with live `EFER.NXE` — never the inner
/// [`user_entry_fully_valid_pages`] degraded path alone.
#[cfg(not(test))]
#[allow(dead_code)]
#[repr(C, align(16))]
struct UserStack([u8; USER_STACK_LEN]);

/// RFLAGS image for a future `iretq` to CPL3 (S09 slice2m, #30 remainder):
/// bit 1 always-1 + IF (bit 9) set, everything else clear (IOPL 0, VM 0,
/// NT 0, AC/TF/DF 0). No guest `iretq` yet — host-only pin so a wrong
/// flags word (e.g. IF-clear hang, or IOPL!=0 leak) fails host-side before
/// any ring-3 entry slice lands.
#[allow(dead_code)]
pub(crate) const USER_RFLAGS: u64 = 0x202;

/// `iretq` return frame for CPL3 entry (Intel SDM Vol 3 §6.14: the CPU pops
/// RIP, CS, RFLAGS, RSP, SS in that order on a privilege-changing `iretq`).
/// `repr(C)` field order matches pop order so a future guest stub can push
/// `ss/rsp/rflags/cs/rip` and execute `iretq` with no reordering. Pure const
/// constructor — no guest `iretq` yet, so boot-neutral by construction.
#[allow(dead_code)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IretqFrame {
    pub(crate) rip: u64,
    pub(crate) cs: u64,
    pub(crate) rflags: u64,
    pub(crate) rsp: u64,
    pub(crate) ss: u64,
}

/// User stack length for a future CPL3 thread (S09 slice2n, #30 remainder):
/// 16 KiB, mirrors [`RING0_STACK_LEN`]. Host-only pin this slice — no guest
/// user-stack static yet, so boot-neutral by construction.
#[allow(dead_code)]
pub(crate) const USER_STACK_LEN: usize = 16384;

/// Top (one-past-end) of a user stack blob: same 16-byte-aligned-top rule as
/// [`ring0_stack_top_for`].
#[allow(dead_code)]
pub(crate) const fn user_stack_top_for(base: u64) -> u64 {
    base.saturating_add(USER_STACK_LEN as u64)
}

/// Pages in the future CPL3 user stack (S09 slice2u, #30 remainder):
/// `USER_STACK_LEN / 4096 == 4` — the stub walks all four via
/// [`walk_user_stack_pages`], never one flag for the whole range.
#[allow(dead_code)]
pub(crate) const USER_STACK_PAGES: usize = USER_STACK_LEN / 4096;

/// Required leaf flags for the future CPL3 user-stack mapping (S09 slice2x,
/// #30 remainder): PRESENT+USER+WRITABLE+NO_EXEC. Single-sources the flag
/// word the mapping slice must install on all 4 pages so a W+X or
/// supervisor-only mapping fails host-side before any guest PTE is written.
/// Must satisfy [`user_stack_perms_valid_for`] with NXE=1 and must NOT
/// satisfy [`user_code_perms_valid`] (NX set) — code-side reuse refuses.
#[allow(dead_code)]
pub(crate) const USER_STACK_MAP_FLAGS: PageFlags = PageFlags::PRESENT
    .union(PageFlags::USER)
    .union(PageFlags::WRITABLE)
    .union(PageFlags::NO_EXEC);

// REVIEW #664 step 2/3 (slice2u): build-time pins so a resized `USER_STACK_LEN`
// that breaks the 4-page walk fails the build, not just the test run.
// REVIEW #672 risk 1 (slice2y): the `contains`-style perms gates accept extra
// bits (in-model: GUARD) — the mapping slice must install EXACTLY
// `USER_STACK_MAP_FLAGS`, so this exact-equality gate (not `contains`) is the
// word the mapping stub gates on. `PageFlags` models only
// PRESENT/WRITABLE/USER/NO_EXEC/GUARD (no HUGE/GLOBAL/ACCESSED bits exist),
// so exact equality is complete: any extra bit in-model is GUARD.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn user_stack_map_flags_valid(flags: PageFlags) -> bool {
    flags.bits() == USER_STACK_MAP_FLAGS.bits()
}

/// Per-page exact mapping gate (S09 slice2z, #30 remainder, REVIEW #673
/// follow-up): the future mapping stub's call site — every 4 KiB page must
/// satisfy [`user_stack_map_flags_valid`] (exact word, not `contains`).
/// Closes the `contains`-accepts-GUARD gap in [`all_stack_pages_valid_for`]:
/// a GUARD-tagged page passes the perms gate but refuses here. Empty
/// coverage refuses. NXE=1 shape only (NXE=0 entry refuses fail-closed via
/// [`user_entry_cpl3_allowed`]); the stub installs this word only when the
/// live NXE pin holds.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn all_stack_map_flags_valid(pages: &[PageFlags]) -> bool {
    if pages.is_empty() {
        return false;
    }
    let mut i = 0usize;
    while i < pages.len() {
        if !user_stack_map_flags_valid(pages[i]) {
            return false;
        }
        i += 1;
    }
    true
}
#[allow(dead_code)]
#[allow(clippy::manual_is_multiple_of)]
const _: () = {
    assert!(USER_STACK_LEN % 4096 == 0);
    assert!(USER_STACK_PAGES == 4);
};

/// Pre-`iretq` readiness gate (S09 slice2n, #30 remainder): the future guest
/// entry stub must refuse (`ok=0` fail-closed) unless `rsp0 != 0` (TSS ring-0
/// stack wired by `load_gdt`) AND `user_rsp != 0` (user stack mapped, top
/// from [`user_stack_top_for`]). `user_rip == 0` (null entry) also refuses.
/// Pure const so host tests and the future guest stub share one rule; no
/// guest `iretq` yet. Weak by design: nonzero-only (accepts kernel-half,
/// non-canonical, misaligned values) — the stub must ALSO gate on
/// [`user_entry_valid`] below before pushing a frame.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn iretq_ready(rsp0: u64, user_rip: u64, user_rsp: u64) -> bool {
    rsp0 != 0 && user_rip != 0 && user_rsp != 0
}

/// Canonical user-half ceiling (S09 slice2o, #30 remainder): every `u64`
/// below this is canonical low-half; `0x0000_8000_0000_0000` itself is the
/// non-canonical hole start and anything at/above it (incl. kernel-half
/// `0xFFFF...`) is rejected by [`user_rip_valid`]/[`user_rsp_valid`].
/// Single-sources the user-range pin so the two validators cannot drift.
#[allow(dead_code)]
pub(crate) const USER_HALF_END: u64 = 0x0000_8000_0000_0000;

/// User-RIP validity (S09 slice2o, REVIEW #654 risk 1): nonzero canonical
/// low-half. Rejects null, the non-canonical hole, and kernel-half
/// addresses (e.g. `0xFFFF...`). Caller-owned mapping check (is the page
/// actually mapped user-accessible?) stays with the future stub — this pins
/// the encodable shape, not the page-table state.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn user_rip_valid(user_rip: u64) -> bool {
    user_rip != 0 && user_rip < USER_HALF_END
}

/// User-RSP validity (S09 slice2o, REVIEW #654 risk 1): [`user_rip_valid`]
/// plus 16-byte alignment (SysV/Win64 ABI entry requirement). Rejects null,
/// kernel-half, non-canonical, and misaligned stacks — notably the
/// `u64::MAX` saturated top, which passes the weak [`iretq_ready`]
/// nonzero gate but must never be a live `iretq` RSP.
#[allow(dead_code)]
#[allow(clippy::manual_is_multiple_of)]
#[must_use]
pub(crate) const fn user_rsp_valid(user_rsp: u64) -> bool {
    user_rsp != 0 && user_rsp < USER_HALF_END && user_rsp % 16 == 0
}

/// Combined user-entry validity (S09 slice2o): the future guest stub gates
/// `iretq_frame_for` on `iretq_ready(rsp0, rip, rsp) && user_entry_valid(rip,
/// rsp)` — nonzero wiring PLUS canonical user-half RIP plus aligned
/// canonical user-half RSP. Pure const so host tests and the stub share one
/// rule; still no guest `iretq` yet. Mapping containment (`rip` inside mapped
/// code, `rsp` the mapped stack top) lives in [`user_entry_mapped_valid`].
#[allow(dead_code)]
#[must_use]
pub(crate) const fn user_entry_valid(user_rip: u64, user_rsp: u64) -> bool {
    user_rip_valid(user_rip) && user_rsp_valid(user_rsp)
}

/// Mapped-range containment (S09 slice2p, #30 remainder, REVIEW #654 risk 1
/// remainder): `addr` in `[map_base, map_base + map_len)` — the page-table
/// mapping half the slice2o preconditions left caller-owned. Subtraction
/// form (`addr - map_base < map_len`, guarded by `addr >= map_base`) so a
/// `map_base + map_len` overflow can never wrap-accept; empty ranges refuse.
/// Used for the code RIP (`rip` must be INSIDE mapped user code); the stack
/// top uses [`user_stack_top_valid`] instead (`rsp` is one-past-end, so the
/// generic half-open check would refuse the valid top). `map_base`/`map_len`
/// must come from the page-table walker (future slice); this pins the
/// containment math, not the walker state.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn user_range_contains(map_base: u64, map_len: u64, addr: u64) -> bool {
    map_len != 0 && addr >= map_base && addr.wrapping_sub(map_base) < map_len
}

/// Mapped user-stack-top validity (S09 slice2p): `user_rsp` is the one-past-end
/// top of the mapped stack `[stack_base, stack_base + stack_len)` — i.e.
/// `user_rsp - stack_base == stack_len` with `stack_base <= user_rsp` (no
/// wrap-accept), `stack_len != 0`, and both ends 16-byte aligned (SysV/Win64
/// ABI entry requirement; matches [`user_rsp_valid`]). An interior pointer
/// (`base <= rsp < base + len`) refuses: `iretq` RSP must be the empty-stack
/// top, not mid-stack. `stack_base`/`stack_len` must come from the mapped
/// user-stack allocation (future guest static + walker slice).
#[allow(dead_code)]
#[allow(clippy::manual_is_multiple_of)]
#[must_use]
pub(crate) const fn user_stack_top_valid(stack_base: u64, stack_len: u64, user_rsp: u64) -> bool {
    stack_len != 0
        && stack_base % 16 == 0
        && user_rsp % 16 == 0
        && stack_base <= user_rsp
        && user_rsp.wrapping_sub(stack_base) == stack_len
}

/// Fully-gated user-entry validity (S09 slice2p): the single call site the
/// future guest stub must gate `iretq_frame_for` on — `iretq_ready` (nonzero
/// wiring) AND `user_entry_valid` (canonical + aligned) AND `rip` inside
/// mapped user code AND `rsp` the mapped stack top. A stub that forgets any
/// half (null wiring, kernel-half, misaligned, unmapped rip, interior rsp)
/// refuses fail-closed. Pure const so host tests and the stub share one rule;
/// still no guest `iretq` yet.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn user_entry_mapped_valid(
    user_rip: u64,
    user_rsp: u64,
    rsp0: u64,
    code_base: u64,
    code_len: u64,
    stack_base: u64,
    stack_len: u64,
) -> bool {
    iretq_ready(rsp0, user_rip, user_rsp)
        && user_entry_valid(user_rip, user_rsp)
        && user_range_contains(code_base, code_len, user_rip)
        && user_stack_top_valid(stack_base, stack_len, user_rsp)
}

/// User-code permission validity (S09 slice2q, #30 remainder, REVIEW #658
/// risk 2): `containment != permission` — slice2p's mapped gate passes a
/// kernel-only or non-exec range. The future stub must source `flags` from
/// the page-table walker (`walk_full_leaf_flags` on the live CR3) for the
/// page containing `user_rip`: `PRESENT` (mapped) AND `USER` (U/S ring-3)
/// AND NOT `NO_EXEC` (executable). `WRITABLE` is allowed either way this
/// slice (W+X code hardening deferred — the guest maps its own RX code).
/// Pure const so host tests and the stub share one rule; still no guest
/// `iretq` yet.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn user_code_perms_valid(flags: PageFlags) -> bool {
    flags.contains(PageFlags::PRESENT)
        && flags.contains(PageFlags::USER)
        && !flags.contains(PageFlags::NO_EXEC)
}

/// User-stack permission validity (S09 slice2q): `flags` from the walker for
/// the page containing `stack_base` (the mapped user-stack allocation):
/// `PRESENT` AND `USER` AND `WRITABLE` AND `NO_EXEC` (stack is RW data, never
/// executed; assumes EFER.NXE=1, which OVMF enables — the guest maps its own
/// NX stack). Pure const; companion to [`user_code_perms_valid`].
#[allow(dead_code)]
#[must_use]
pub(crate) const fn user_stack_perms_valid(flags: PageFlags) -> bool {
    flags.contains(PageFlags::PRESENT)
        && flags.contains(PageFlags::USER)
        && flags.contains(PageFlags::WRITABLE)
        && flags.contains(PageFlags::NO_EXEC)
}

/// Fully-gated user-entry validity with permissions (S09 slice2q): the new
/// single call site the future guest stub must gate `iretq_frame_for` on —
/// [`user_entry_mapped_valid`] (wiring + shape + containment) PLUS
/// [`user_code_perms_valid`] PLUS [`user_stack_perms_valid`]. A stub that
/// forgets permissions (supervisor-only, non-present, non-exec code, exec or
/// read-only stack) refuses fail-closed. `flags` must come from the walker,
/// never caller-supplied literals (REVIEW #658 risk 1 remains: this pins the
/// permission math, not the walker binding).
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
#[must_use]
pub(crate) const fn user_entry_fully_valid(
    user_rip: u64,
    user_rsp: u64,
    rsp0: u64,
    code_base: u64,
    code_len: u64,
    stack_base: u64,
    stack_len: u64,
    code_flags: PageFlags,
    stack_flags: PageFlags,
) -> bool {
    user_entry_mapped_valid(
        user_rip, user_rsp, rsp0, code_base, code_len, stack_base, stack_len,
    ) && user_code_perms_valid(code_flags)
        && user_stack_perms_valid(stack_flags)
}

/// User-stack permission validity with a live NXE pin (S09 slice2r, #30
/// remainder, REVIEW #660 risk 3): `nxe_enabled` is the live
/// `EFER.NXE` bit (`efer_nxe_enabled(read_efer())` in the future stub).
/// When enabled, same as [`user_stack_perms_valid`] (PRESENT+USER+WRITABLE+
/// NO_EXEC). When disabled, bit 63 is MBZ (`pte_reserved_violation` with
/// `nx_enabled=false`), so a NO_EXEC PTE is a reserved violation and must
/// refuse — the gate demands PRESENT+USER+WRITABLE with NO_EXEC *clear*
/// (degraded accept: HW cannot enforce NX; the future stub must still
/// refuse CPL3 entry when NXE=0 — see [`user_entry_fully_valid_pages`]
/// and the fail-closed [`user_entry_cpl3_allowed`] wrapper).
/// Pure const so host tests and the stub share one rule; still no guest
/// `iretq` yet.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn user_stack_perms_valid_for(flags: PageFlags, nxe_enabled: bool) -> bool {
    if nxe_enabled {
        user_stack_perms_valid(flags)
    } else {
        flags.contains(PageFlags::PRESENT)
            && flags.contains(PageFlags::USER)
            && flags.contains(PageFlags::WRITABLE)
            && !flags.contains(PageFlags::NO_EXEC)
    }
}

/// Per-page code permission gate (S09 slice2r, REVIEW #660 risk 2): every
/// page in `pages` (one [`PageFlags`] per 4 KiB page, sourced from
/// `walk_full_leaf_flags` per page) must satisfy [`user_code_perms_valid`].
/// Empty coverage refuses (a zero-length walk proves nothing). This closes
/// the single-flags-for-multi-page gap: `code_len 0x2000` is 2 pages, so a
/// caller passing one good flags value for both pages bypasses an exec
/// check on the second page — the future stub must walk each page.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn all_code_pages_valid(pages: &[PageFlags]) -> bool {
    if pages.is_empty() {
        return false;
    }
    let mut i = 0usize;
    while i < pages.len() {
        if !user_code_perms_valid(pages[i]) {
            return false;
        }
        i += 1;
    }
    true
}

/// Per-page stack permission gate with NXE pin (S09 slice2r, REVIEW #660
/// risks 2+3): every page in `pages` must satisfy
/// [`user_stack_perms_valid_for`] with the same live `nxe_enabled`.
/// Empty coverage refuses. The 16 KiB user stack is 4 pages — one flags
/// value cannot cover it; the future stub walks all four.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn all_stack_pages_valid_for(pages: &[PageFlags], nxe_enabled: bool) -> bool {
    if pages.is_empty() {
        return false;
    }
    let mut i = 0usize;
    while i < pages.len() {
        if !user_stack_perms_valid_for(pages[i], nxe_enabled) {
            return false;
        }
        i += 1;
    }
    true
}

/// Per-page VA for the future CPL3 user-stack mapping stub (S09 slice2aa,
/// #30 remainder): page `index` of `[stack_base, stack_base +
/// USER_STACK_LEN)` — the exact VA the stub maps/walks per 4 KiB page.
/// `None` fail-closed on OOB index (`>= USER_STACK_PAGES`) or
/// `checked_add` overflow (near-`u64::MAX` base). Single-sources the VA
/// math so the mapping install and [`walk_user_stack_pages`] cannot drift
/// apart (one inline `checked_add` copy remains: none — the walk calls
/// this). Pure const so host tests and the stub share one rule.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn user_stack_page_va(stack_base: u64, index: usize) -> Option<u64> {
    if index >= USER_STACK_PAGES {
        return None;
    }
    stack_base.checked_add((index as u64) * 4096)
}

/// Install-readiness gate (S09 slice2ab, #30 remainder): the future 4-page
/// U-map install stub's single call site before writing any guest PTE.
/// Binds the two stub helpers so they cannot drift unwired: gates on
/// [`all_stack_map_flags_valid`] (exact word on every page, GUARD refuses)
/// AND [`user_stack_page_va`] (every per-page VA encodable — OOB/overflow
/// `None` refuses). Also pins `nxe_enabled` (live `EFER.NXE`; NX stack needs
/// it), `pages.len() == USER_STACK_PAGES` (no short/long coverage), and
/// 4 KiB base alignment. Canonical/containment stays with the entry gates
/// ([`user_entry_valid`]/[`user_entry_mapped_valid`]) — this pins the
/// install wiring, not the entry shape. Pure const; still no guest PTE
/// written yet.
#[allow(dead_code)]
#[allow(clippy::manual_is_multiple_of)]
#[must_use]
pub(crate) const fn user_stack_map_install_ready(
    stack_base: u64,
    pages: &[PageFlags],
    nxe_enabled: bool,
) -> bool {
    if !nxe_enabled {
        return false;
    }
    if stack_base % 4096 != 0 {
        return false;
    }
    if pages.len() != USER_STACK_PAGES {
        return false;
    }
    if !all_stack_map_flags_valid(pages) {
        return false;
    }
    let mut i = 0usize;
    while i < USER_STACK_PAGES {
        if user_stack_page_va(stack_base, i).is_none() {
            return false;
        }
        i += 1;
    }
    true
}

/// Install PTE-word encoder (S09 slice2ac/slice2af, #30 remainder,
/// REVIEW #677 risk 1, REVIEW #683 closure (a)): PRIVATE helper, the first
/// REAL PTE-write call site — consumes [`user_stack_map_install_ready`]
/// (exact-word flags + encodable VAs + live NXE + 4-page coverage + base align)
/// and produces the four `u64` leaf words the guest PT store writes. Fail-closed
/// `None` on ANY refusal (readiness gate, `phys_base` misalign, per-page phys
/// `checked_add` overflow, or [`pte_encode_checked`](super::page_table::pte_encode_checked)
/// reject) so a partial install never reaches the store. `GUARD` naturally
/// masks to 0 at encode (HW ignores bit 9) — but GUARD never reaches encode:
/// the readiness gate refuses it first. Pure const; the guest store loop
/// (PT index per [`user_stack_page_va`], `invlpg`/CR3 reload, QEMU proof)
/// rides with the next slice — this pins the word the store writes.
/// Private since slice2af: only [`user_stack_install_store_ptes`] (same
/// module) and in-module tests may call it; cross-module callers must use
/// the bound entry so tampered words cannot bypass encode.
#[allow(dead_code)]
#[allow(clippy::manual_is_multiple_of)]
#[must_use]
const fn user_stack_install_ptes(
    stack_base: u64,
    phys_base: u64,
    pages: &[PageFlags],
    nxe_enabled: bool,
) -> Option<[u64; USER_STACK_PAGES]> {
    if !user_stack_map_install_ready(stack_base, pages, nxe_enabled) {
        return None;
    }
    if phys_base % 4096 != 0 {
        return None;
    }
    let mut ptes = [0u64; USER_STACK_PAGES];
    let mut i = 0usize;
    while i < USER_STACK_PAGES {
        let Some(phys) = phys_base.checked_add((i as u64) * 4096) else {
            return None;
        };
        let Some(pte) = super::page_table::pte_encode_checked(phys, pages[i]) else {
            return None;
        };
        ptes[i] = pte;
        i += 1;
    }
    Some(ptes)
}

/// Single-PT VA set (S09 slice2ah, #30 remainder, REVIEW #686 residuals
/// 1-3): the exact per-page VAs of `[stack_base, stack_base +
/// USER_STACK_LEN)` that fit in ONE 4 KiB PT. Pure const over
/// [`user_stack_page_va`]; the single source for the span rule so
/// [`user_stack_store_ptes`] and [`user_stack_flush_vas`] cannot drift
/// apart (previously two near-copies of the VA-loop + `(va >> 21)` check,
/// plus a third near-copy in [`user_stack_map_install_ready`]'s VA loop —
/// that one stays separate by design: install checks encodability only, so
/// a cross-PT base encodes then store-refuses with `pt` untouched).
/// Fail-closed `None` on ANY refusal: `USER_STACK_PAGES == 0` (empty stack
/// would index `vas[0]` — guarded here so a future `PAGES == 0` const panics
/// nowhere), any per-page VA `None` (overflow), or pages spanning two PTs
/// (`va >> 21` differs — the store owns one PT).
///
/// Explicit non-mirrors (REVIEW #686 residual 1 — documented, not drift):
/// `page_table_index(va, 0)`-is-`None` needs no mirror — dead today: level 0
/// always returns `Some` (`page_table.rs:425` matches level 0 to shift 12,
/// only `level > 3` returns `None`). Slot-collision needs no mirror —
/// unreachable for the `Some` shape: 4 consecutive 4 KiB VAs inside one
/// 2 MiB region (`va >> 21` equal) have consecutive PT indices
/// `((va >> 12) & 0x1FF)` that cannot wrap (wrap would cross the region
/// boundary, refused above), so with `base_idx <= 508` the set is
/// `base_idx..base_idx+4`, pairwise distinct. The store still pins the
/// collision check defense-in-depth; the flush set agrees with the store on
/// every reachable shape (`Some` iff the store's VA+span preconditions hold).
#[allow(dead_code)]
#[must_use]
pub(crate) const fn user_stack_single_pt_vas(stack_base: u64) -> Option<[u64; USER_STACK_PAGES]> {
    if USER_STACK_PAGES == 0 {
        return None;
    }
    let mut vas = [0u64; USER_STACK_PAGES];
    let mut i = 0usize;
    while i < USER_STACK_PAGES {
        let Some(va) = user_stack_page_va(stack_base, i) else {
            return None;
        };
        vas[i] = va;
        i += 1;
    }
    let pt_base = vas[0] >> 21;
    let mut j = 1usize;
    while j < USER_STACK_PAGES {
        if (vas[j] >> 21) != pt_base {
            return None;
        }
        j += 1;
    }
    Some(vas)
}

/// Guest PT store loop (S09 slice2ad/slice2af/slice2ah, #30 remainder,
/// REVIEW #679 risk 1, REVIEW #683 closure (a), REVIEW #686 residual 2):
/// PRIVATE helper, writes the four bound-entry words into the caller's
/// single 4 KiB PT at `page_table_index(va, 0)` per [`user_stack_page_va`].
/// VA+span preconditions come from [`user_stack_single_pt_vas`] (shared with
/// the flush set, so the span rule has one source).
/// Two-phase (validate all VAs/indices first, then write) so refusal leaves
/// `pt` untouched — no partial install. Fail-closed `false` on ANY refusal:
/// any per-page VA `None` (overflow), pages spanning two PTs (`va >> 21`
/// differs — the store owns one PT), or two pages sharing one PT slot
/// (overwrite; unreachable via consecutive VAs but pinned defense-in-depth).
/// Plain `fn` (`&mut` store cannot be `const` on stable); host-testable.
/// The guest TLB reload (`invlpg`/CR3) + CPL3 entry + QEMU proof ride next.
/// Private since slice2af: only the bound entry (same module) and in-module
/// tests may call it; it never re-checks phys/flags/NXE by design.
#[allow(dead_code)]
#[must_use]
fn user_stack_store_ptes(
    pt: &mut [u64; 512],
    stack_base: u64,
    ptes: [u64; USER_STACK_PAGES],
) -> bool {
    let Some(vas) = user_stack_single_pt_vas(stack_base) else {
        return false;
    };
    let mut idx = [0usize; USER_STACK_PAGES];
    let mut i = 0usize;
    while i < USER_STACK_PAGES {
        let Some(slot) = super::page_table::page_table_index(vas[i], 0) else {
            return false;
        };
        idx[i] = slot as usize;
        i += 1;
    }
    let mut a = 0usize;
    while a < USER_STACK_PAGES {
        let mut b = a + 1;
        while b < USER_STACK_PAGES {
            if idx[a] == idx[b] {
                return false;
            }
            b += 1;
        }
        a += 1;
    }
    let mut i = 0usize;
    while i < USER_STACK_PAGES {
        pt[idx[i]] = ptes[i];
        i += 1;
    }
    true
}

/// Bound install+store entry (S09 slice2ae/slice2af, #30 remainder,
/// REVIEW #681 residual 1, REVIEW #683 closure (a)): the SOLE cross-module
/// entry, encodes via the private installer then stores via the private
/// store in one call so callers cannot inject a
/// mismatched `ptes` array (the residual-1 silent-install gap: `store`
/// alone trusts caller-supplied words and never re-checks they match
/// `base`/`phys`/`flags`). Fail-closed `false` on ANY refusal (encode
/// `None` or store `false`) with `pt` untouched — encode writes only its
/// local array, and store is two-phase (untouched on refusal), so the
/// bound path inherits no-partial. Plain `fn`; host-testable. Guest TLB
/// reload (`invlpg`/CR3) + CPL3 entry + QEMU proof still ride next.
#[allow(dead_code)]
#[must_use]
pub(crate) fn user_stack_install_store_ptes(
    pt: &mut [u64; 512],
    stack_base: u64,
    phys_base: u64,
    pages: &[PageFlags],
    nxe_enabled: bool,
) -> bool {
    let Some(ptes) = user_stack_install_ptes(stack_base, phys_base, pages, nxe_enabled) else {
        return false;
    };
    user_stack_store_ptes(pt, stack_base, ptes)
}

/// TLB flush VA set (S09 slice2ag/2ah, #30 remainder, REVIEW #685 residual c,
/// REVIEW #686 residuals 1-3): the exact per-page VAs the guest `invlpg`
/// loop must flush after the bound install+store writes one PT. Thin wrapper
/// over [`user_stack_single_pt_vas`] so the flush set and the store share one
/// span rule and cannot drift apart (single-source VA rule extended to the
/// span rule). Agreement is exact on every reachable shape: `Some` iff the
/// store's VA+span preconditions hold (overflow/`None` and cross-PT span
/// refuse in both, so a refused store wrote nothing and there is nothing to
/// flush). The two remaining store refusals need no mirror — see
/// [`user_stack_single_pt_vas`]: `page_table_index` `None` is dead (level 0
/// always `Some`) and slot-collision is unreachable for 4 consecutive VAs
/// inside one PT (proven there; the store keeps the check defense-in-depth).
/// Host-testable; the guest `invlpg` loop + CPL3 entry + QEMU proof ride next.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn user_stack_flush_vas(stack_base: u64) -> Option<[u64; USER_STACK_PAGES]> {
    user_stack_single_pt_vas(stack_base)
}

/// Single-page TLB invalidate (S09 slice2ai, #30 remainder): guest `invlpg`
/// for one VA after the bound install+store writes one PT. Guest-only
/// contract: post-EBS CPL0 with the edited PT live (same contract as
/// [`read_cr3`]); host build is a no-op stub so `cargo test` never executes
/// privileged `invlpg`.
///
/// # Safety
/// Same contract as [`read_cr3`]: boot CPU only, post-`ExitBootServices`.
/// Caller must own the VA (it must be mapped/unmapped consistently with the
/// PT edit); the [`user_stack_flush_tlb`] wrapper sources VAs only from
/// [`user_stack_flush_vas`] so a refused store (nothing written) flushes
/// nothing.
#[cfg(not(test))]
pub unsafe fn invlpg_page(va: u64) {
    core::arch::asm!("invlpg [{0}]", in(reg) va, options(nostack, preserves_flags));
}

/// Host-test stub: no-op (never touches hardware).
///
/// # Safety
/// Host `cargo test` only: no-op.
#[cfg(test)]
pub unsafe fn invlpg_page(_va: u64) {}

/// Guest TLB flush loop (S09 slice2ai, #30 remainder, REVIEW #688 nit 2
/// follow-up): `invlpg` every VA in [`user_stack_flush_vas`]. Returns
/// `false` (flushes nothing) when the VA set refuses (overflow or
/// cross-PT span — the store wrote nothing, so there is nothing to flush).
/// Uncalled this slice (no guest PTE written yet), so boot-neutral by
/// construction. CPL3 entry + QEMU proof ride next.
#[allow(dead_code)]
#[must_use]
pub(crate) unsafe fn user_stack_flush_tlb(stack_base: u64) -> bool {
    let Some(vas) = user_stack_flush_vas(stack_base) else {
        return false;
    };
    let mut i = 0usize;
    while i < USER_STACK_PAGES {
        // SAFETY: caller contract of `invlpg_page` (post-EBS CPL0, VAs
        // sourced from the just-written store set).
        unsafe {
            invlpg_page(vas[i]);
        }
        i += 1;
    }
    true
}

/// Walk every page of the future CPL3 user stack (S09 slice2u+2v, #30 remainder,
/// REVIEW #664 step 2/3 + REVIEW #666 risk 2): `walk_full_leaf_flags` per 4 KiB page over
/// `[stack_base, stack_base + USER_STACK_LEN)`, collecting one [`PageFlags`]
/// per page. Returns `None` fail-closed on ANY walk error (unmapped page,
/// non-canonical base, lookup miss) — a partial walk proves nothing, so the
/// caller gates on the full [`USER_STACK_PAGES`] array or refuses.
///
/// Slice2v overflow pin: per-page VA uses `checked_add` — a `stack_base` near
/// `u64::MAX` refuses with `None` instead of `wrapping_add` wrapping to low
/// VA and walking the wrong pages.
///
/// The flags array feeds [`all_stack_pages_valid_for`] with the live
/// `EFER.NXE` pin (see [`probe_user_stack_pages_valid`]); callers MUST NOT
/// substitute caller-supplied literals (REVIEW #658 risk 1) — the only
/// producer of this array is this walk.
pub(crate) fn walk_user_stack_pages<'a, F>(
    pml4_base: u64,
    stack_base: u64,
    lookup: F,
) -> Option<[PageFlags; USER_STACK_PAGES]>
where
    F: Fn(u64) -> Option<&'a [u64; 512]>,
{
    let mut pages = [PageFlags::empty(); USER_STACK_PAGES];
    let mut i = 0usize;
    while i < USER_STACK_PAGES {
        let va = user_stack_page_va(stack_base, i)?;
        match super::page_table::walk_full_leaf_flags(pml4_base, va, &lookup) {
            Ok((_phys, flags)) => pages[i] = flags,
            Err(_) => return None,
        }
        i += 1;
    }
    Some(pages)
}

/// Guest-only stub binding the 4-page walk to the live NXE pin (S09 slice2u,
/// REVIEW #664 step 2/3): reads live `EFER.NXE` (`efer_nxe_enabled(read_efer())`),
/// walks the guest `USER_STACK` bytes via [`walk_user_stack_pages`] with the
/// live `pml4_base` + caller lookup, and gates on [`all_stack_pages_valid_for`]
/// — wrapper-only (never inline flag checks, never the inner degraded path
/// alone: NXE=0 refuses here even though the per-page math degraded-accepts).
/// Uncalled this slice (the stack is BSS-reserved but unmapped, so it would
/// return false; no `iretq`), so boot-neutral by construction. The CPL3-entry
/// slice wires the call site with the identity-map lookup + code-side pages.
///
/// # Safety
/// Post-EBS only, on the boot thread: reads `EFER` and firmware page tables
/// through `lookup` (identity-mapped). Never called on the host.
#[cfg(not(test))]
#[allow(dead_code)]
pub(crate) unsafe fn probe_user_stack_pages_valid<'a, F>(pml4_base: u64, lookup: F) -> bool
where
    F: Fn(u64) -> Option<&'a [u64; 512]>,
{
    let nxe_enabled = efer_nxe_enabled(read_efer());
    let base = core::ptr::addr_of!(USER_STACK) as u64;
    match walk_user_stack_pages(pml4_base, base, lookup) {
        Some(pages) => all_stack_pages_valid_for(&pages, nxe_enabled),
        None => false,
    }
}

/// Fully-gated user-entry validity over per-page walks (S09 slice2r): the
/// future stub's single call site once the walker binding lands —
/// [`user_entry_mapped_valid`] PLUS per-page code/stack gates PLUS exact
/// page-count coverage (`code_len`/`stack_len` must be nonzero 4 KiB
/// multiples and `pages.len()` must equal `len / 4096`, so a one-flag
/// shortcut for a 4-page stack refuses on count even if the single page
/// is good) PLUS the live `nxe_enabled` pin. Pure const; still no guest
/// `iretq` yet.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::manual_is_multiple_of)]
#[must_use]
pub(crate) const fn user_entry_fully_valid_pages(
    user_rip: u64,
    user_rsp: u64,
    rsp0: u64,
    code_base: u64,
    code_len: u64,
    stack_base: u64,
    stack_len: u64,
    code_pages: &[PageFlags],
    stack_pages: &[PageFlags],
    nxe_enabled: bool,
) -> bool {
    if code_len == 0 || code_len % 4096 != 0 || stack_len == 0 || stack_len % 4096 != 0 {
        return false;
    }
    if code_pages.len() as u64 != code_len / 4096 {
        return false;
    }
    if stack_pages.len() as u64 != stack_len / 4096 {
        return false;
    }
    user_entry_mapped_valid(
        user_rip, user_rsp, rsp0, code_base, code_len, stack_base, stack_len,
    ) && all_code_pages_valid(code_pages)
        && all_stack_pages_valid_for(stack_pages, nxe_enabled)
}

/// Fail-closed CPL3-entry gate (S09 slice2s, REVIEW #662 nits a+b): the
/// future guest stub's single call site before `iretq`. `nxe_enabled`
/// MUST be live `EFER.NXE` — CPL3 entry refuses whenever NXE=0, even
/// though [`user_entry_fully_valid_pages`] degraded-accepts an
/// RW-without-NX stack while NX is MBZ. Per-page math alone ACCEPTS
/// that degraded shape (proven by test); only this `nxe_enabled &&`
/// wrapper turns it into entry-refused. Pure const; still no guest
/// `iretq` yet.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
#[must_use]
pub(crate) const fn user_entry_cpl3_allowed(
    user_rip: u64,
    user_rsp: u64,
    rsp0: u64,
    code_base: u64,
    code_len: u64,
    stack_base: u64,
    stack_len: u64,
    code_pages: &[PageFlags],
    stack_pages: &[PageFlags],
    nxe_enabled: bool,
) -> bool {
    nxe_enabled
        && user_entry_fully_valid_pages(
            user_rip,
            user_rsp,
            rsp0,
            code_base,
            code_len,
            stack_base,
            stack_len,
            code_pages,
            stack_pages,
            nxe_enabled,
        )
}

/// Build the CPL3 `iretq` frame for `user_rip` with user stack top
/// `user_rsp`: CS/SS pin the DPL3 selectors, RFLAGS pins [`USER_RFLAGS`].
/// Pure so host tests and the future guest entry stub share one rule.
///
/// # Preconditions
/// Caller owns validation: gate on [`user_entry_cpl3_allowed`] (which folds
/// in [`user_entry_fully_valid_pages`] wiring/shape/containment PLUS code/stack
/// page permissions PLUS the fail-closed live-NXE pin — otherwise a kernel-only,
/// non-exec, or NXE=0-degraded range passes).
/// The weak [`iretq_ready`] nonzero gate alone accepts kernel-half,
/// non-canonical, misaligned, and unmapped values. This constructor does NOT
/// check them (const, infallible) — the guest entry stub must gate on
/// [`user_entry_cpl3_allowed`] before pushing this frame and executing
/// `iretq`.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn iretq_frame_for(user_rip: u64, user_rsp: u64) -> IretqFrame {
    IretqFrame {
        rip: user_rip,
        cs: USER_CODE_SEL as u64,
        rflags: USER_RFLAGS,
        rsp: user_rsp,
        ss: USER_DATA_SEL as u64,
    }
}

// REVIEW #652 nit 1 (slice2m): build-time pins so a wrong frame layout fails
// the build, not just the test run (the `#[test]` size/offset asserts remain
// as the readable host-side pins).
#[allow(dead_code)]
const _: () = {
    assert!(core::mem::size_of::<IretqFrame>() == 40);
    assert!(core::mem::offset_of!(IretqFrame, rip) == 0);
    assert!(core::mem::offset_of!(IretqFrame, cs) == 8);
    assert!(core::mem::offset_of!(IretqFrame, rflags) == 16);
    assert!(core::mem::offset_of!(IretqFrame, rsp) == 24);
    assert!(core::mem::offset_of!(IretqFrame, ss) == 32);
};

#[cfg(not(test))]
static mut RING0_STACK: Ring0Stack = Ring0Stack([0; RING0_STACK_LEN]);

/// Future CPL3 user stack (S09 slice2t, #30 remainder): 16 KiB zero blob.
/// Not wired anywhere this slice (no page-table mapping, no `iretq` RSP),
/// so the bytes are pure BSS growth with no guest behavior change.
#[cfg(not(test))]
#[allow(dead_code)]
static mut USER_STACK: UserStack = UserStack([0; USER_STACK_LEN]);

#[cfg(not(test))]
static mut TSS: Tss = Tss::new();

/// Packed `lgdt` operand: 16-bit limit + 64-bit base (Intel SDM format).
#[cfg(not(test))]
#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

/// One 16-byte x86-64 IDT gate descriptor.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(crate) struct IdtEntry {
    pub(crate) offset_low: u16,
    pub(crate) selector: u16,
    pub(crate) ist: u8,
    pub(crate) type_attr: u8,
    pub(crate) offset_mid: u16,
    pub(crate) offset_high: u32,
    pub(crate) reserved: u32,
}

impl IdtEntry {
    /// Absent gate (type_attr present-bit clear): a vector that hits this
    /// slot raises `#GP` instead of jumping to garbage.
    pub(crate) const fn missing() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    /// Interrupt gate (type `0xE`, DPL 0, present) targeting `handler`
    /// with code selector `selector` (IST 0 — same stack this slice).
    /// Private to this module so [`gate_for`] is the only in-crate path to
    /// a present gate (REVIEW #240 R1: the `KERNEL_CODE_SEL` pin is
    /// compiler-enforced, not convention — `lib.rs` tests reach encoding
    /// via `gate_for`).
    const fn from_handler(handler: u64, selector: u16) -> Self {
        Self {
            offset_low: handler as u16,
            selector,
            ist: 0,
            type_attr: 0x8E,
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }

    /// Reassemble the 64-bit handler offset from the split gate fields
    /// (host-testable encoding round-trip; guest `lidt` consumes raw bytes).
    #[cfg(test)]
    pub(crate) fn handler_offset(&self) -> u64 {
        u64::from(self.offset_low)
            | (u64::from(self.offset_mid) << 16)
            | (u64::from(self.offset_high) << 32)
    }
}

/// IDT covering the 32 exception vectors plus the timer IRQ0 vector 32
/// (S07 slice13); `#DE` + `#PF` + timer are wired this slice (others stay
/// [`IdtEntry::missing`] and would `#GP` if hit — with `cli` and
/// single-threaded bring-up only `#DE` is expected; `#PF` is
/// wired-but-untriggered until the live-injection slice; timer is
/// wired-but-masked until the PIC/PIT/`sti` slice).
/// Guest-only IDT storage; host tests exercise [`IdtEntry`] encoding
/// without instantiating the full table.
#[cfg(not(test))]
#[repr(C, align(16))]
struct Idt {
    entries: [IdtEntry; IDT_VECTORS],
}

#[cfg(not(test))]
static mut IDT: Idt = Idt {
    entries: [IdtEntry::missing(); IDT_VECTORS],
};

/// Build a present DPL0 interrupt gate for `handler` in the kernel code
/// segment.
///
/// Safe wrapper: pins [`KERNEL_CODE_SEL`] so callers cannot wire a gate
/// into the null/data segment (which would `#GP` on first delivery).
/// Pure encoding — no hardware touched; [`load_idt`] consumes the result.
pub(crate) const fn gate_for(handler: u64) -> IdtEntry {
    IdtEntry::from_handler(handler, KERNEL_CODE_SEL)
}

/// Check the live-`#PF` probe-guard contract without touching hardware:
///
/// Safe pure helper sharing one contract between the guest probe constant
/// and host tests (mirrors `pf_probe_addr_is_canonical_unmapped_guard`).
/// Guest intentionally does not call this (no panic path post-EBS); the
/// host tests are the gate, so `dead_code` in guest builds is expected.
///
/// Single source (REVIEW #242 nit): the raw canonical/aligned/floor
/// predicate lives ONLY here — `lib.rs` probe-guard tests assert via this
/// function (table-driven), never with duplicated raw `& 0xFFF` / range
/// asserts, so the contract cannot drift across copies.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn probe_addr_valid(addr: u64) -> bool {
    addr & 0xFFF == 0 && addr < 0x0000_8000_0000_0000 && addr > 0x1_0000_0000
}

/// PML4 base from a live `CR3` value (S05 slice 5).
///
/// Safe pure wrapper over [`crate::page_table::cr3_decode`] so guest probe
/// code (`probe_paging`, `probe_live_flags_for`, `probe_rmbz` in
/// `boot-uefi`) and host tests share one decode rule beside the other
/// `cpu` wrappers. No hardware touched; the live [`read_cr3`] (`mov {}, cr3`)
/// lives beside it in this module (S05 slice 6 register-read move).
/// Guest calls this with the live `read_cr3()` value (slice 7 wiring);
/// host tests pin agreement with `page_table`.
#[inline(always)]
#[must_use]
pub const fn cr3_pml4_base(cr3: u64) -> u64 {
    super::page_table::cr3_decode(cr3)
}

/// `NXE` pin from a live `EFER` value (S05 slice 5).
///
/// Safe pure wrapper over [`crate::page_table::efer_nxe_enabled`] (bit 11)
/// so the `RMBZ nxen=` verdict and host tests share one decode rule.
/// No hardware touched; the live [`read_efer`] (`rdmsr` on `EFER_MSR`)
/// lives beside it in this module (S05 slice 6 register-read move).
/// Guest calls this with the live `read_efer()` value (slice 7 wiring);
/// host tests pin agreement with `page_table`.
#[inline(always)]
#[must_use]
pub const fn efer_nxe_enabled(efer: u64) -> bool {
    super::page_table::efer_nxe_enabled(efer)
}

/// Mask maskable interrupts on the boot CPU (`cli`, S06 preempt-IRQ gate).
///
/// Guest-only contract: post-`ExitBootServices`, CPL0 boot/exception path.
/// `cli` clears IF (hence NO `preserves_flags` on the asm — same rule as the
/// `efi_main` cli site). Host builds are a no-op stub (the harness must
/// never execute `cli`).
///
/// # Safety
/// Same contract as [`read_cr3`]: boot CPU only, post-EBS. Callers that need
/// a scoped critical section should use [`IrqGuard`] instead of raw `cli`.
#[cfg(not(test))]
pub unsafe fn cli() {
    core::arch::asm!("cli", options(nomem, nostack));
}

/// Host-test stub: no-op (never touches flags).
///
/// # Safety
/// Host `cargo test` only: no-op.
#[cfg(test)]
pub unsafe fn cli() {}

/// Enable maskable interrupts on the boot CPU (`sti`, S07 slice14 timer
/// enable).
///
/// Guest-only contract: post-`ExitBootServices`, CPL0 boot path, IDT vector
/// 32 wired to the returning IRQ0 stub with PIC remapped + PIT programmed
/// first (otherwise the first tick vectors into a missing gate or the PIT
/// never fires). `sti` sets IF (hence NO `preserves_flags` — same rule as
/// [`cli`]). Host builds are a no-op stub (the harness must never execute
/// `sti`).
///
/// # Safety
/// Same contract as [`cli`]: boot CPU only, post-EBS. The timer wiring
/// slice calls this exactly once after the PIC/PIT program sequence.
#[cfg(not(test))]
pub unsafe fn sti() {
    core::arch::asm!("sti", options(nomem, nostack));
}

/// Host-test stub: no-op (never touches flags).
///
/// # Safety
/// Host `cargo test` only: no-op.
#[cfg(test)]
pub unsafe fn sti() {}

/// Preempt-IRQ gate token (S06 slice28 / task #19): proves the holder runs
/// with maskable IRQs masked (`cli` in effect) across a scheduler
/// pick-then-mark critical section (`dispatch` / `exit_current` pick the next
/// `Ready` thread and flip states — two preempted picks without a gate can
/// both observe the same `Ready` target and double-dispatch it).
///
/// Zero-cost proof token on ALL builds (host and guest): constructing it
/// executes NO `cli` itself. The live `cli` is established ONCE in
/// `efi_main` post-EBS (the pre-existing `asm!("cli")` site, still the only
/// `cli` in the tree); every gated scheduler site runs downstream of it on
/// the single-threaded boot thread, so holding the token means holding that
/// critical section. (A token that re-executed `cli` per dispatch would
/// fault host `cargo test`: `boot-uefi` links `baseos_kernel` as a
/// dependency, so the dependency's `cfg(test)` no-op never applies there —
/// the same reason the live-swap stubs stay guest-only. The token must be
/// asm-free to keep host and guest call sites identical.)
///
/// `Drop` is a no-op (never `sti`: enabling interrupts belongs to the timer
/// wiring slice, which programs the PIC/PIT and wires IDT 32 first — a
/// `Drop`-time `sti` would enable IRQs at an arbitrary pick site. The paired
/// [`sti`] lands with that slice, after the handler is wired).
///
/// Contract: create the guard BEFORE the pick and hold the `&` across the
/// whole pick + state-mark (`Scheduler::dispatch_gated` /
/// `exit_current_gated` take `&IrqGuard`, so an ungated call site fails to
/// compile where the gated path is required).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrqGuard {
    _private: (),
}

impl IrqGuard {
    /// Enter the IRQ-masked critical section: proof token only (no asm on
    /// any build — see the type docs for why the live `cli` stays in
    /// `efi_main` instead of here).
    ///
    /// # Safety
    /// Same contract as [`cli`]: boot CPU only, post-EBS on the
    /// single-threaded boot/exception path with `cli` already in effect
    /// (established by `efi_main`). Host `cargo test` never executes
    /// privileged instructions through this token.
    #[must_use]
    pub unsafe fn enter() -> Self {
        Self { _private: () }
    }
}
/// Read the live `CR3` (PML4 base + flags) on the boot CPU (S05 slice 6).
///
/// Moved from `boot-uefi` (D7) so the privileged register read lives
/// behind the `cpu` boundary with the decode rule ([`cr3_pml4_base`]).
/// Pure register read; valid on x86-64 at CPL0 (the guest runs with
/// privilege post-`ExitBootServices`).
///
/// # Safety
/// Boot CPU only, post-`ExitBootServices` on the single-threaded
/// boot/exception path. Host `cargo test` never calls this (would fault
/// outside the guest); host coverage is the `unsafe fn` shape pin below.
pub unsafe fn read_cr3() -> u64 {
    let value: u64;
    core::arch::asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
    value
}

/// Read the live `EFER` (Extended Feature Enable Register) on the boot CPU
/// (S05 slice 6).
///
/// Moved from `boot-uefi` (D8) so the `rdmsr` read lives behind the `cpu`
/// boundary with the decode rule ([`efer_nxe_enabled`]). Bit 11 (`NXE`)
/// gates whether `NO_EXEC`/bit 63 in a PTE is legal or MBZ (#139 note 1).
///
/// # Safety
/// Same contract as [`read_cr3`]: CPL0 guest, boot thread post-`EBS`;
/// `ECX` is pinned to [`crate::page_table::EFER_MSR`]. Host tests never
/// call this; host coverage is the `unsafe fn` shape pin below.
pub unsafe fn read_efer() -> u64 {
    let lo: u32;
    let hi: u32;
    core::arch::asm!(
        "rdmsr",
        in("ecx") super::page_table::EFER_MSR,
        out("eax") lo,
        out("edx") hi,
        options(nomem, nostack, preserves_flags),
    );
    (u64::from(hi) << 32) | u64::from(lo)
}

/// Live `RSP` read (S07 slice68, #21 nit 3 RSP-switch proof).
///
/// Unprivileged `mov {}, rsp` — safe on all builds, so host `cargo test`
/// calls it too (the value is the harness stack there; only the
/// nonzero/nesting shape is asserted). The guest IRQ body records its inner
/// `RSP` while switched to the dedicated IRQ stack and the task-context
/// proof line reports both `RSP`s, so QEMU serial proves the tick ran on
/// the IRQ stack while dispatch stayed on the task stack.
#[inline(always)]
#[must_use]
pub fn read_rsp() -> u64 {
    let value: u64;
    // SAFETY: pure register read, no privilege requirement; never writes.
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Live `TR` (task register) readback (S09 slice2k, #30 remainder).
///
/// `str` (store task register) proves the guest `load_gdt` `ltr TSS_SEL`
/// actually loaded the task register: the guest emits `TR ok=1 sel=...`
/// right after `load_gdt`/`load_idt`, so a faulted/skipped `ltr` fails the
/// e2e hard-require loudly instead of resting on boot-identical bytes.
/// `str` is unprivileged on x86-64, so this is a safe `fn` with the
/// `unsafe` confined to the asm body (same shape as [`read_rsp`]); host
/// `cargo test` may call it (returns the harness TR there — only the
/// safe-fn shape is asserted, never the value).
#[inline(always)]
#[must_use]
pub fn read_tr() -> u16 {
    let value: u16;
    // SAFETY: pure register read, unprivileged `str`; never writes.
    unsafe {
        core::arch::asm!("str {0:x}", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// TSC warm-up iterations before a latency run (S07 slice52, #21 R3).
///
/// Warms the branch predictor / i-cache on the `lfence; rdtsc` path so the
/// first recorded sample is not a cold outlier. Discarded reads only; the
/// guest runs this before filling `SWITCH_LATENCIES`.
pub const TSC_WARMUP_ITERS: usize = 128;

/// Raw `rdtsc` read (S07 slice52, #21 R3).
///
/// Unfenced: callers that need ordering must use [`read_tsc_fenced`].
/// User-mode legal on x86-64; host tests may call this.
///
/// # Safety
/// No privilege requirement, but the value is only meaningful on x86-64
/// with a running TSC. Guest: post-EBS boot thread. Host: test harness.
pub unsafe fn read_tsc() -> u64 {
    let lo: u32;
    let hi: u32;
    core::arch::asm!(
        "rdtsc",
        out("eax") lo,
        out("edx") hi,
        options(nomem, nostack, preserves_flags),
    );
    (u64::from(hi) << 32) | u64::from(lo)
}

/// Fenced `rdtsc` read (S07 slice52, #21 R3): `lfence` before and after
/// so the timestamp brackets exactly the intended region and prior loads
/// cannot bleed into the sample.
///
/// # Safety
/// Same contract as [`read_tsc`].
pub unsafe fn read_tsc_fenced() -> u64 {
    let lo: u32;
    let hi: u32;
    core::arch::asm!(
        "lfence",
        "rdtsc",
        "lfence",
        out("eax") lo,
        out("edx") hi,
        options(nomem, nostack, preserves_flags),
    );
    (u64::from(hi) << 32) | u64::from(lo)
}

/// Saturating TSC delta (S07 slice52, #21 R3): `end - start`, floored at 0
/// so a reordered/coarse pair never underflows; pure so host tests pin it.
#[inline(always)]
#[must_use]
pub const fn tsc_delta(start: u64, end: u64) -> u64 {
    end.saturating_sub(start)
}

/// Overhead-corrected switch sample (S07 slice53, #21 R3): raw
/// [`tsc_delta`] minus the [`empty_tsc_pair`] floor, saturating to 0 so a
/// sub-floor sample (coarse/emulated TSC) records 0 instead of underflowing.
/// Pure so host tests pin it; the guest records the result into
/// `SWITCH_LATENCIES`.
#[inline(always)]
#[must_use]
pub const fn switch_sample_delta(start: u64, end: u64, floor: u64) -> u64 {
    tsc_delta(start, end).saturating_sub(floor)
}

/// Empty-pair overhead floor (S07 slice52, #21 R3): two back-to-back
/// fenced reads with no work between; the delta is the per-sample
/// measurement overhead the guest subtracts (saturating) from raw switch
/// deltas before recording into `LatencySamples`.
///
/// # Safety
/// Same contract as [`read_tsc_fenced`].
pub unsafe fn empty_tsc_pair() -> u64 {
    let a = read_tsc_fenced();
    let b = read_tsc_fenced();
    tsc_delta(a, b)
}

/// Discard `TSC_WARMUP_ITERS` fenced reads (S07 slice52, #21 R3).
///
/// # Safety
/// Same contract as [`read_tsc_fenced`].
pub unsafe fn warm_tsc() {
    let mut i = 0usize;
    while i < TSC_WARMUP_ITERS {
        let _ = read_tsc_fenced();
        i += 1;
    }
}

// Forward-declared asm stubs (defined in `global_asm!` in the parent).
#[cfg(not(test))]
extern "C" {
    fn baseos_isr_div0();
    fn baseos_isr_pf();
    /// S07 slice13: timer IRQ0 stub (defined in `timer.rs` `global_asm!`).
    /// Wired into the IDT by [`load_idt`] below; still `cli` so it never
    /// fires until the PIC/PIT/`sti` slice lands.
    fn baseos_isr_timer();
}

/// Number of IDT vectors the guest table covers (S07 slice13).
///
/// 32 exception vectors (0..=31) plus the timer IRQ0 vector 32
/// ([`crate::timer::VECTOR_TIMER`]). The timer gate is wired but never
/// fires this slice (guest keeps `cli`, no PIC/PIT programming yet), so
/// QEMU behavior is unchanged. Un-gated (all builds) so host tests pin the
/// coverage without instantiating the guest-only table. Single-sourced off
/// [`super::EXCEPTION_VECTORS`] (32 exception vectors + timer vector 32).
const IDT_VECTORS: usize = super::EXCEPTION_VECTORS + 1;

/// Load the S02 GDT, wire the TSS descriptor + ring-0 stack top, load `TR`,
/// and reload visible segment registers + CS.
///
/// # Safety
/// Guest-only post-`ExitBootServices` path: `GDT`/`TSS`/`RING0_STACK` are
/// stable statics, the far-return reloads CS to [`KERNEL_CODE_SEL`], and
/// data selectors point at the kernel data selector. Wiring `TSS.rsp0` to
/// the ring-0 stack top here (before `ltr`) guarantees no future `iretq`
/// to CPL3 can land with `rsp0 == 0` (`#SS(0)`/triple-fault); still no CPL3
/// entry this slice, so the nonzero `rsp0` is boot-neutral. Host builds
/// are a no-op (no `lgdt`/`ltr` on the test harness); the OVMF e2e path proves
/// the descriptors by surviving past load into the intentional `#DE`.
#[cfg(not(test))]
pub(crate) unsafe fn load_gdt() {
    use super::KERNEL_DATA_SEL;
    let base = core::ptr::addr_of!(TSS) as u64;
    let limit = (core::mem::size_of::<Tss>() - 1) as u32;
    (*core::ptr::addr_of_mut!(GDT)).entries[GDT_TSS_INDEX] = tss_desc_low(base, limit);
    (*core::ptr::addr_of_mut!(GDT)).entries[GDT_TSS_INDEX + 1] = tss_desc_high(base);
    // Ring-0 stack top first: a CPL3->0 transition pushes onto `[rsp0]`,
    // so `rsp0` must be valid before `ltr` makes the TSS active, not after.
    // SAFETY: `Tss` is `repr(C, packed)`; `rsp0` is an unaligned `u64`.
    let top = ring0_stack_top_for(core::ptr::addr_of!(RING0_STACK) as u64);
    // REVIEW #644: release-present `assert_ne!` (not `debug_assert!` — the
    // latter compiles out in the release UEFI profile, silently restoring
    // the caller-convention gun). One-time boot cost only; a zero `top`
    // fails loudly via panic instead of a later `#SS(0)`/triple-fault.
    assert_ne!(top, 0, "TSS.rsp0 must be nonzero before ltr");
    core::ptr::addr_of_mut!((*core::ptr::addr_of_mut!(TSS)).rsp0).write_unaligned(top);
    assert_ne!(
        core::ptr::addr_of!((*core::ptr::addr_of!(TSS)).rsp0).read_unaligned(),
        0,
        "TSS.rsp0 write must land (offset 4) before ltr"
    );
    let ptr = DescriptorTablePointer {
        limit: (core::mem::size_of::<Gdt>() - 1) as u16,
        base: core::ptr::addr_of!(GDT) as u64,
    };
    core::arch::asm!("lgdt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
    core::arch::asm!(
        "mov ss, {data:x}",
        "mov ds, {data:x}",
        "mov es, {data:x}",
        "xor eax, eax",
        "mov fs, ax",
        "mov gs, ax",
        "push {code}",
        "lea rax, [rip + 771f]",
        "push rax",
        "retfq",
        "771:",
        data = in(reg) KERNEL_DATA_SEL,
        code = in(reg) u64::from(KERNEL_CODE_SEL),
        options(preserves_flags),
    );
    core::arch::asm!("ltr {sel:x}", sel = in(reg) TSS_SEL, options(nostack, preserves_flags));
}

/// Populate `#DE` (vector 0) + `#PF` (vector 14) + timer IRQ0 (vector 32)
/// and `lidt` the table.
///
/// The timer gate is a present DPL0 interrupt gate (type `0xE`, same
/// [`gate_for`] encoding as the exception gates): on delivery the CPU
/// clears IF in hardware, which is what makes the
/// [`crate::timer::timer_irq_rust`] `IrqGuard::enter()` proof true on
/// entry. Still `cli` with no PIC/PIT programming this slice, so the timer
/// gate never fires and QEMU behavior is unchanged.
///
/// # Safety
/// Same guest-only contract as [`load_gdt`]: `IDT` static must outlive the
/// CPU, gates target [`baseos_isr_div0`] / [`baseos_isr_pf`] /
/// [`baseos_isr_timer`] in the kernel code segment (via [`gate_for`]), and
/// `cli` is already in effect so unhandled vectors cannot fire this slice.
/// Host builds are a no-op; e2e proves BOTH exception gates (`PF ok=1` via
/// the live probe touch, then `EXC ok=1` via the chained `#DE`).
#[cfg(not(test))]
pub(crate) unsafe fn load_idt() {
    let de_handler = baseos_isr_div0 as *const () as u64;
    (*core::ptr::addr_of_mut!(IDT)).entries[VECTOR_DE] = gate_for(de_handler);
    let pf_handler = baseos_isr_pf as *const () as u64;
    (*core::ptr::addr_of_mut!(IDT)).entries[VECTOR_PF] = gate_for(pf_handler);
    let timer_handler = baseos_isr_timer as *const () as u64;
    (*core::ptr::addr_of_mut!(IDT)).entries[VECTOR_TIMER] = gate_for(timer_handler);
    let ptr = DescriptorTablePointer {
        limit: (core::mem::size_of::<Idt>() - 1) as u16,
        base: core::ptr::addr_of!(IDT) as u64,
    };
    core::arch::asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tss_wiring_pins_table_shape_and_empty_bitmap() {
        // S09 slice2g (#30 remainder): the `ltr` wiring slice grows the guest
        // GDT to 7 slots with the TSS descriptor at index 5; host pins the
        // shape the guest `load_gdt` consumes (selector/index/limit/size).
        assert_eq!(GDT_LEN, 7, "null + kcode + kdata + ucode + udata + tss x2");
        assert_eq!(GDT_TSS_INDEX, 5);
        assert_eq!(TSS_SEL, 0x28);
        assert_eq!(TSS_SEL >> 3, GDT_TSS_INDEX as u16);
        assert_eq!(TSS_SEL & 0b11, 0, "TSS RPL 0");
        assert_eq!(core::mem::size_of::<Tss>(), 104);
        let tss = Tss::new();
        // SAFETY: `Tss` is `repr(C, packed)` (hardware layout); host test only.
        let rsp0 = unsafe { core::ptr::addr_of!(tss.rsp0).read_unaligned() };
        let iomap = unsafe { core::ptr::addr_of!(tss.iomap_base).read_unaligned() };
        assert_eq!(rsp0, 0, "no ring-0 stack until the user-stack slice");
        assert_eq!(iomap, 104, "empty I/O bitmap");
        assert_eq!(
            (core::mem::size_of::<Tss>() - 1) as u32,
            103,
            "byte-limit for tss_desc_low"
        );
        // Wiring consumes the available-type encoding (busy never initial).
        assert_eq!(super::super::TSS_TYPE_AVAIL, 0x9);
        let lo = tss_desc_low(0x0000_ABCD_1234_5000, 103);
        assert_eq!(lo >> 40 & 0xF, 0x9, "wired descriptor stays available");
        assert_eq!(
            tss_desc_high(0x0000_ABCD_1234_5000),
            0x0000_ABCD_1234_5000_u64 >> 32
        );
    }

    #[test]
    fn ring0_stack_top_pins_size_align_and_nonzero_rsp0() {
        // S09 slice2h (#30 remainder): the `ltr` wiring must never leave a
        // live TR with `rsp0 == 0` (first CPL3->0 entry would #SS(0)/
        // triple-fault). Host pins the helper the guest `load_gdt` consumes:
        // 16 KiB, 16-aligned top, nonzero, overflow-saturating.
        // REVIEW #642/#644: guest `load_gdt` now `assert_ne!`s the top and
        // the written `rsp0` pre-`ltr` (release-present per #644; this test
        // the written `rsp0` pre-`ltr`; this test is the host side of that
        // gate (same invariants, plus offset + in-range).
        assert_eq!(RING0_STACK_LEN, 16384, "one 16 KiB ring-0 stack");
        // Packed hardware layout: `reserved0: u32` then `rsp0: u64`, so the
        // guest `write_unaligned` must hit byte offset 4 in the 104 B TSS.
        assert_eq!(core::mem::size_of::<Tss>(), 104);
        assert_eq!(core::mem::offset_of!(Tss, rsp0), 4);
        let tss = Tss::new();
        let base_ptr = core::ptr::addr_of!(tss) as usize;
        let rsp0_ptr = core::ptr::addr_of!(tss.rsp0) as usize;
        assert_eq!(rsp0_ptr.wrapping_sub(base_ptr), 4, "rsp0 byte offset");
        let base = 0xFFFF_8000_0010_0000_u64;
        assert_eq!(base % 16, 0, "wrapper keeps every real base aligned");
        let top = ring0_stack_top_for(base);
        assert_eq!(top, base + 16384);
        assert_eq!(top % 16, 0, "RSP top stays 16-aligned");
        assert_ne!(top, 0, "wired rsp0 is never the loaded gun");
        // In-range: top is one-past-end; the pushed frame lives below it.
        assert!(top > base, "top above base");
        assert_eq!(top.wrapping_sub(base), RING0_STACK_LEN as u64);
        assert!(base <= top.wrapping_sub(1) && top.wrapping_sub(1) < top);
        assert!(base <= top.wrapping_sub(8) && top.wrapping_sub(8) < top);
        assert_eq!(ring0_stack_top_for(u64::MAX), u64::MAX, "saturates");
        // REVIEW #644 release-fold theorem: `saturating_add(16384)` can never
        // yield 0 (true sum is either exact-nonzero or saturates to MAX), so
        // the release optimizer folds the guest `assert_ne!(top, 0)` by proof
        // (release EFI keeps generic panic strings but drops ours) while
        // debug builds still check loudly at runtime. Pin the exact
        // would-be-zero preimage: base == -16384 overflows, saturates to MAX.
        assert_eq!(
            ring0_stack_top_for(u64::MAX - 16383),
            u64::MAX,
            "exact-zero preimage saturates, never 0"
        );
        assert_ne!(
            ring0_stack_top_for(0),
            0,
            "zero base still yields nonzero top"
        );
        // The constructor stays zero; only `load_gdt` wires the top.
        let mut tss = Tss::new();
        unsafe {
            core::ptr::addr_of_mut!(tss.rsp0).write_unaligned(top);
            assert_ne!(
                core::ptr::addr_of!(tss.rsp0).read_unaligned(),
                0,
                "guest write path yields nonzero rsp0"
            );
        }
    }

    #[test]
    fn iretq_frame_pins_cpl3_selectors_and_rflags() {
        // S09 slice2m (#30 remainder): pin the future CPL3 `iretq` frame
        // host-side before any guest ring-3 entry lands. CS/SS must be the
        // DPL3 selectors with RPL 3; RFLAGS must be bit1 + IF only (IF-clear
        // would hang user code with `cli`-equivalent masking; IOPL!=0 would
        // leak port access). Field order matches the CPU pop order
        // (RIP/CS/RFLAGS/RSP/SS) so the future stub pushes in reverse with no
        // reorder; no guest change this slice (still ring 0, no `iretq`).
        assert_eq!(USER_RFLAGS, 0x202, "bit1 always-1 + IF only");
        assert_eq!(USER_RFLAGS & (1 << 1), 1 << 1, "bit1 must be 1");
        assert_eq!(USER_RFLAGS & (1 << 9), 1 << 9, "IF must be set");
        assert_eq!(USER_RFLAGS >> 12 & 0b11, 0, "IOPL must be 0");
        assert_eq!(USER_RFLAGS & !0x202, 0, "no other flags");
        assert_eq!(super::super::USER_CODE_SEL, 0x1B);
        assert_eq!(super::super::USER_DATA_SEL, 0x23);
        let frame = iretq_frame_for(0x1_0000, 0x2_0000);
        assert_eq!(frame.rip, 0x1_0000);
        assert_eq!(frame.rsp, 0x2_0000);
        assert_eq!(frame.cs, 0x1B);
        assert_eq!(frame.rflags, 0x202);
        assert_eq!(frame.ss, 0x23);
        assert_eq!(frame.cs & 0b11, 3, "CS RPL 3");
        assert_eq!(frame.ss & 0b11, 3, "SS RPL 3");
        assert_eq!(frame.cs >> 3, 3, "CS GDT index 3");
        assert_eq!(frame.ss >> 3, 4, "SS GDT index 4");
        // Pop-order layout: RIP(0) CS(8) RFLAGS(16) RSP(24) SS(32).
        assert_eq!(core::mem::size_of::<IretqFrame>(), 40);
        assert_eq!(core::mem::offset_of!(IretqFrame, rip), 0);
        assert_eq!(core::mem::offset_of!(IretqFrame, cs), 8);
        assert_eq!(core::mem::offset_of!(IretqFrame, rflags), 16);
        assert_eq!(core::mem::offset_of!(IretqFrame, rsp), 24);
        assert_eq!(core::mem::offset_of!(IretqFrame, ss), 32);
    }

    #[test]
    fn user_stack_top_and_iretq_ready_pin_pre_iretq_gate() {
        // S09 slice2n (#30 remainder, REVIEW #652 nit 2): the future guest
        // entry stub must refuse unless rsp0 != 0 (ring-0 stack wired) AND a
        // nonzero user rip/rsp exist. Host-only pin; no guest `iretq` yet.
        assert_eq!(USER_STACK_LEN, 16384, "one 16 KiB user stack");
        assert_eq!(
            USER_STACK_LEN % 4096,
            0,
            "4 KiB-multiple for the per-page walk"
        );
        assert_eq!(
            USER_STACK_LEN / 4096,
            4,
            "4 pages: the stub walks all four, never one flag"
        );
        let base = 0xFFFF_8000_0020_0000_u64;
        let top = user_stack_top_for(base);
        assert_eq!(top, base + 16384);
        assert_eq!(top % 16, 0, "user RSP top stays 16-aligned");
        assert_ne!(top, 0);
        assert_eq!(user_stack_top_for(u64::MAX), u64::MAX, "saturates");
        assert_ne!(user_stack_top_for(0), 0);
        // Ready gate: all-nonzero passes; any zero refuses fail-closed.
        assert!(iretq_ready(0x1000, 0x1_0000, top));
        assert!(!iretq_ready(0, 0x1_0000, top), "rsp0==0 refuses");
        assert!(!iretq_ready(0x1000, 0, top), "null rip refuses");
        assert!(!iretq_ready(0x1000, 0x1_0000, 0), "null rsp refuses");
        assert!(!iretq_ready(0, 0, 0));
        // The frame constructor stays pure/infallible (no gate inside) —
        // the gate lives at the call site per the `# Preconditions` doc.
        let frame = iretq_frame_for(0x1_0000, top);
        assert_eq!((frame.rip, frame.rsp), (0x1_0000, top));
    }

    #[test]
    fn user_entry_pins_canonical_and_alignment() {
        // S09 slice2o (#30 remainder, REVIEW #654 risk 1): the weak
        // `iretq_ready` nonzero gate accepts kernel-half, non-canonical, and
        // misaligned values, so the future stub must ALSO gate on
        // `user_entry_valid`. Host-only pin; no guest `iretq` yet.
        assert_eq!(USER_HALF_END, 0x0000_8000_0000_0000);
        let base = 0x0000_1000_0020_0000_u64;
        let top = user_stack_top_for(base);
        assert_eq!(top % 16, 0);
        // RIP: nonzero canonical low-half passes; null/hole/kernel fail.
        assert!(user_rip_valid(0x1_0000));
        assert!(!user_rip_valid(0), "null rip refuses");
        assert!(!user_rip_valid(USER_HALF_END), "hole start refuses");
        assert!(
            !user_rip_valid(0xFFFF_8000_0010_0000),
            "kernel-half refuses"
        );
        assert!(!user_rip_valid(u64::MAX), "saturated refuses");
        // RSP: aligned canonical passes; null/misaligned/hole/kernel fail.
        assert!(user_rsp_valid(top));
        assert!(!user_rsp_valid(0), "null rsp refuses");
        assert!(!user_rsp_valid(top - 1), "misaligned refuses");
        assert!(!user_rsp_valid(top | 0x8), "misaligned refuses");
        assert!(!user_rsp_valid(USER_HALF_END), "hole refuses");
        assert!(
            !user_rsp_valid(0xFFFF_8000_0020_0000),
            "kernel-half refuses"
        );
        // REVIEW #654 load-bearing case: the saturated top passes the weak
        // nonzero gate yet must fail the canonical gate.
        assert_eq!(user_stack_top_for(u64::MAX), u64::MAX);
        assert!(iretq_ready(0x1000, 0x1_0000, u64::MAX));
        assert!(!user_rsp_valid(u64::MAX));
        assert!(!user_entry_valid(0x1_0000, u64::MAX));
        // Combined: valid pair passes; any half-invalid refuses.
        assert!(user_entry_valid(0x1_0000, top));
        assert!(!user_entry_valid(0, top));
        assert!(!user_entry_valid(0x1_0000, 0));
        assert!(!user_entry_valid(0xFFFF_8000_0010_0000, top));
        assert!(!user_entry_valid(0x1_0000, top - 1));
        assert!(!user_entry_valid(0, 0));
    }

    #[test]
    fn user_entry_mapped_pins_range_containment() {
        // S09 slice2p (#30 remainder, REVIEW #656 carry 1): the slice2o
        // preconditions left mapping caller-owned with zero enforcement, so
        // a stub could forget it. This pins the containment math host-side:
        // `rip` inside mapped code (`user_range_contains`), `rsp` the mapped
        // stack top (`user_stack_top_valid`), folded into the single
        // `user_entry_mapped_valid` gate. Host-only; no guest `iretq` yet.
        let code_base = 0x0000_1000_0010_0000_u64;
        let code_len = 0x2000_u64;
        let stack_base = 0x0000_1000_0020_0000_u64;
        let stack_len = USER_STACK_LEN as u64;
        let rip = code_base + 0x100;
        let rsp = user_stack_top_for(stack_base);
        assert_eq!(rsp, stack_base + stack_len);
        // REVIEW #656 carry 3: every 16-aligned base yields a 16-aligned
        // top (not just one base in-test); saturated tops stay refused by
        // the canonical gate, never silently accepted.
        for base in [
            0x0000_1000_0020_0000_u64,
            0x0000_0000_0010_0000_u64,
            0x0000_1000_0030_0010_u64,
            0x0000_7FFF_FFFF_0000_u64,
        ] {
            assert_eq!(base % 16, 0);
            let top = user_stack_top_for(base);
            assert_eq!(top, base + USER_STACK_LEN as u64);
            assert_eq!(top % 16, 0, "aligned base => aligned top");
            assert!(user_rsp_valid(top));
        }
        // Range containment: inside passes; base-1 / end / empty refuse;
        // wrap-around (`base + len` overflow) refuses via subtraction form.
        assert!(user_range_contains(code_base, code_len, rip));
        assert!(user_range_contains(code_base, code_len, code_base));
        assert!(!user_range_contains(code_base, code_len, code_base - 1));
        assert!(!user_range_contains(
            code_base,
            code_len,
            code_base + code_len
        ));
        assert!(!user_range_contains(code_base, 0, code_base));
        assert!(!user_range_contains(u64::MAX - 15, 32, 0x10));
        // Stack top: exact top passes; interior / base / top+16 / misaligned
        // / empty / base-misaligned refuse.
        assert!(user_stack_top_valid(stack_base, stack_len, rsp));
        assert!(!user_stack_top_valid(stack_base, stack_len, stack_base));
        assert!(!user_stack_top_valid(
            stack_base,
            stack_len,
            stack_base + 16
        ));
        assert!(!user_stack_top_valid(stack_base, stack_len, rsp + 16));
        assert!(!user_stack_top_valid(stack_base, stack_len, rsp - 1));
        assert!(!user_stack_top_valid(stack_base, 0, rsp));
        assert!(!user_stack_top_valid(stack_base + 1, stack_len, rsp));
        // Full gate: mapped pair passes; forgetting any half refuses.
        assert!(user_entry_mapped_valid(
            rip, rsp, 0x1000, code_base, code_len, stack_base, stack_len
        ));
        assert!(
            !user_entry_mapped_valid(rip, rsp, 0, code_base, code_len, stack_base, stack_len),
            "rsp0==0 refuses"
        );
        assert!(
            !user_entry_mapped_valid(
                rip,
                rsp,
                0x1000,
                code_base,
                code_len,
                stack_base,
                stack_len - 16
            ),
            "unmapped stack top refuses"
        );
        assert!(
            !user_entry_mapped_valid(
                code_base + code_len,
                rsp,
                0x1000,
                code_base,
                code_len,
                stack_base,
                stack_len
            ),
            "unmapped rip refuses"
        );
        assert!(
            !user_entry_mapped_valid(
                0xFFFF_8000_0010_0000,
                rsp,
                0x1000,
                0xFFFF_8000_0010_0000,
                code_len,
                stack_base,
                stack_len
            ),
            "kernel-half refuses even when mapped"
        );
        assert!(
            !user_entry_mapped_valid(
                rip,
                rsp - 1,
                0x1000,
                code_base,
                code_len,
                stack_base,
                stack_len
            ),
            "misaligned rsp refuses"
        );
        assert!(
            !user_entry_mapped_valid(
                rip,
                u64::MAX,
                0x1000,
                code_base,
                code_len,
                stack_base,
                stack_len
            ),
            "saturated top refuses"
        );
        // Load-bearing: shape-valid but mapping-forgotten refuses — the
        // slice2o gap this closes (`ready && entry_valid` alone passes).
        assert!(iretq_ready(0x1000, rip, rsp) && user_entry_valid(rip, rsp));
        assert!(!user_entry_mapped_valid(
            rip, rsp, 0x1000, code_base, code_len, stack_base, 0
        ));
    }

    #[test]
    fn user_entry_fully_pins_code_and_stack_permissions() {
        // S09 slice2q (#30 remainder, REVIEW #658 risk 2): containment !=
        // permission — slice2p's mapped gate passes a kernel-only or non-exec
        // range. This pins the permission math host-side: code needs
        // PRESENT+USER+exec, stack needs PRESENT+USER+WRITABLE+NX, folded into
        // the single `user_entry_fully_valid` gate. Flags must come from the
        // walker in the future stub; this pins the math, not the binding.
        use super::super::page_table::PageFlags;
        let code_base = 0x0000_1000_0010_0000_u64;
        let code_len = 0x2000_u64;
        let stack_base = 0x0000_1000_0020_0000_u64;
        let stack_len = USER_STACK_LEN as u64;
        let rip = code_base + 0x100;
        let rsp = user_stack_top_for(stack_base);
        let code_rxu = PageFlags::PRESENT | PageFlags::USER;
        let stack_rwu_nx =
            PageFlags::PRESENT | PageFlags::USER | PageFlags::WRITABLE | PageFlags::NO_EXEC;
        // Good pair passes both halves and the full gate.
        assert!(user_code_perms_valid(code_rxu));
        assert!(user_stack_perms_valid(stack_rwu_nx));
        assert!(user_entry_fully_valid(
            rip,
            rsp,
            0x1000,
            code_base,
            code_len,
            stack_base,
            stack_len,
            code_rxu,
            stack_rwu_nx
        ));
        // Code: supervisor-only (U/S=0), non-present, or non-exec refuses.
        assert!(!user_code_perms_valid(PageFlags::PRESENT));
        assert!(!user_code_perms_valid(PageFlags::USER));
        assert!(!user_code_perms_valid(code_rxu | PageFlags::NO_EXEC));
        assert!(!user_code_perms_valid(PageFlags::empty()));
        // Stack: read-only, supervisor-only, non-present, or exec refuses.
        assert!(!user_stack_perms_valid(
            PageFlags::PRESENT | PageFlags::USER | PageFlags::NO_EXEC
        ));
        assert!(!user_stack_perms_valid(
            PageFlags::PRESENT | PageFlags::USER | PageFlags::WRITABLE
        ));
        assert!(!user_stack_perms_valid(
            PageFlags::USER | PageFlags::WRITABLE | PageFlags::NO_EXEC
        ));
        assert!(!user_stack_perms_valid(PageFlags::empty()));
        // Full gate: mapped-but-wrong-perms refuses even when containment
        // passes (the load-bearing slice2p gap).
        assert!(user_entry_mapped_valid(
            rip, rsp, 0x1000, code_base, code_len, stack_base, stack_len
        ));
        assert!(
            !user_entry_fully_valid(
                rip,
                rsp,
                0x1000,
                code_base,
                code_len,
                stack_base,
                stack_len,
                PageFlags::PRESENT,
                stack_rwu_nx
            ),
            "supervisor-only code refuses"
        );
        assert!(
            !user_entry_fully_valid(
                rip,
                rsp,
                0x1000,
                code_base,
                code_len,
                stack_base,
                stack_len,
                code_rxu,
                PageFlags::PRESENT | PageFlags::USER | PageFlags::WRITABLE
            ),
            "exec stack refuses"
        );
    }

    #[test]
    fn user_entry_pages_pin_per_page_and_nxe() {
        // S09 slice2r (#30 remainder, REVIEW #660 risks 2+3): single-flags
        // gates cannot cover multi-page regions, and the slice2q stack gate
        // assumed NXE=1. Flags here are walker-shaped: sourced via
        // `pte_encode` -> `pte_decoded_flags` (the same decode rule
        // `walk_full_leaf_flags` uses), plus one live `walk_full_leaf_flags`
        // call proving the walker->gate plumbing. Host-only; no guest iretq.
        use super::super::page_table::{
            pte_decoded_flags, pte_encode, walk_full_leaf_flags, PageFlags,
        };
        let code_base = 0x0000_1000_0010_0000_u64;
        let code_len = 0x2000_u64; // 2 pages
        let stack_base = 0x0000_1000_0020_0000_u64;
        let stack_len = USER_STACK_LEN as u64; // 16384 = 4 pages
        let rip = code_base + 0x100;
        let rsp = user_stack_top_for(stack_base);
        // Walker-shaped decode: encode then decode (never hand literals).
        let src = |flags: PageFlags| pte_decoded_flags(pte_encode(0x1_0000, flags));
        let code_rxu = src(PageFlags::PRESENT | PageFlags::USER);
        let stack_nx =
            src(PageFlags::PRESENT | PageFlags::USER | PageFlags::WRITABLE | PageFlags::NO_EXEC);
        let stack_exec = src(PageFlags::PRESENT | PageFlags::USER | PageFlags::WRITABLE);
        // NXE pin agrees with the page-table decode (bit 11).
        assert!(super::super::page_table::efer_nxe_enabled(
            super::super::page_table::EFER_NXE_BIT
        ));
        assert!(!super::super::page_table::efer_nxe_enabled(0));
        // NXE-aware stack math: NXE=1 demands NX; NXE=0 demands !NX (bit 63
        // is MBZ while NXE=0 — a NO_EXEC PTE is a reserved violation).
        assert!(user_stack_perms_valid_for(stack_nx, true));
        assert!(!user_stack_perms_valid_for(stack_exec, true));
        assert!(
            !user_stack_perms_valid_for(stack_nx, false),
            "NX is MBZ while NXE=0"
        );
        assert!(
            user_stack_perms_valid_for(stack_exec, false),
            "degraded accept, HW can't enforce"
        );
        assert_eq!(
            user_stack_perms_valid_for(stack_nx, true),
            user_stack_perms_valid(stack_nx)
        );
        // Live walker plumbing: one real `walk_full_leaf_flags` leaf feeds
        // the code gate (proves the future stub's source, not just literals).
        // virt 0x1000: PML4[0] -> PDPT -> PD -> PT[1] = RXU leaf.
        use super::super::page_table::encode_table_entry;
        let mut pml4: [u64; 512] = [0; 512];
        let mut pdpt: [u64; 512] = [0; 512];
        let mut pd: [u64; 512] = [0; 512];
        let mut ptable: [u64; 512] = [0; 512];
        let next = PageFlags::PRESENT | PageFlags::WRITABLE;
        pml4[0] = encode_table_entry(0x2000, next).unwrap();
        pdpt[0] = encode_table_entry(0x3000, next).unwrap();
        pd[0] = encode_table_entry(0x4000, next).unwrap();
        ptable[1] = pte_encode(0x2_0000, PageFlags::PRESENT | PageFlags::USER);
        let lookup = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4),
                0x2000 => Some(&pdpt),
                0x3000 => Some(&pd),
                0x4000 => Some(&ptable),
                _ => None,
            }
        };
        let (walk_phys, walk_flags) = walk_full_leaf_flags(0x1000, 0x1000, lookup).unwrap();
        assert_eq!(walk_phys, 0x2_0000);
        assert_eq!(walk_flags, code_rxu);
        assert!(user_code_perms_valid(walk_flags));
        // Per-page: 4-page stack with page 3 exec must refuse even though
        // the single-page gate with page-0 flags passes (load-bearing risk 2).
        let good4 = [stack_nx, stack_nx, stack_nx, stack_nx];
        let bad4 = [stack_nx, stack_nx, stack_nx, stack_exec];
        assert!(all_stack_pages_valid_for(&good4, true));
        assert!(
            !all_stack_pages_valid_for(&bad4, true),
            "page 3 exec refuses"
        );
        assert!(!all_stack_pages_valid_for(&[], true), "empty refuses");
        assert!(!all_code_pages_valid(&[]), "empty refuses");
        assert!(all_code_pages_valid(&[code_rxu, code_rxu]));
        let code_nx = src(PageFlags::PRESENT | PageFlags::USER | PageFlags::NO_EXEC);
        assert!(!user_code_perms_valid(code_nx), "non-exec code refuses");
        assert!(!all_code_pages_valid(&[code_rxu, code_nx]));
        // Full per-page gate: good 2+4 pages pass; bad page-3 refuses even
        // though `user_entry_fully_valid` with page-0 flags alone passes.
        let good_code = [code_rxu, code_rxu];
        assert!(user_entry_fully_valid(
            rip, rsp, 0x1000, code_base, code_len, stack_base, stack_len, code_rxu, stack_nx
        ));
        assert!(user_entry_fully_valid_pages(
            rip, rsp, 0x1000, code_base, code_len, stack_base, stack_len, &good_code, &good4, true
        ));
        assert!(
            !user_entry_fully_valid_pages(
                rip, rsp, 0x1000, code_base, code_len, stack_base, stack_len, &good_code, &bad4,
                true
            ),
            "single good flag cannot cover exec page 3"
        );
        // Count coverage: one flag for a 4-page stack refuses on count.
        assert!(
            !user_entry_fully_valid_pages(
                rip,
                rsp,
                0x1000,
                code_base,
                code_len,
                stack_base,
                stack_len,
                &good_code,
                &[stack_nx],
                true
            ),
            "one flag for 4 pages refuses"
        );
        // NXE=0 full gate: NX stack refuses (MBZ), matching per-page rule.
        assert!(!user_entry_fully_valid_pages(
            rip, rsp, 0x1000, code_base, code_len, stack_base, stack_len, &good_code, &good4, false
        ));
        let degraded4 = [stack_exec, stack_exec, stack_exec, stack_exec];
        assert!(user_entry_fully_valid_pages(
            rip, rsp, 0x1000, code_base, code_len, stack_base, stack_len, &good_code, &degraded4,
            false
        ));
        // S09 slice2s (REVIEW #662 nit b): fail-closed CPL3 wrapper — NXE=0
        // refuses even the degraded-accept shape, NXE=1 delegates to the
        // per-page gate.
        assert!(user_entry_cpl3_allowed(
            rip, rsp, 0x1000, code_base, code_len, stack_base, stack_len, &good_code, &good4, true
        ));
        assert!(!user_entry_cpl3_allowed(
            rip, rsp, 0x1000, code_base, code_len, stack_base, stack_len, &good_code, &bad4, true
        ));
        assert!(
            !user_entry_cpl3_allowed(
                rip, rsp, 0x1000, code_base, code_len, stack_base, stack_len, &good_code,
                &degraded4, false
            ),
            "degraded accept is never entry-allowed while NXE=0"
        );
        assert!(!user_entry_cpl3_allowed(
            rip, rsp, 0x1000, code_base, code_len, stack_base, stack_len, &good_code, &good4, false
        ));
    }

    #[test]
    fn user_stack_pages_walk_pins_four_page_wrapper_binding() {
        // S09 slice2u (#30 remainder, REVIEW #664 step 2/3): the future stub
        // must walk ALL 4 user-stack pages via `walk_full_leaf_flags` (never
        // one flag for the range — REVIEW #658 risk 1: no caller-supplied
        // literals) and gate the array on `all_stack_pages_valid_for` with
        // the live NXE pin. Host pins the walk core the guest
        // `probe_user_stack_pages_valid` stub composes; the stub itself is
        // guest-only (uncalled, unmapped stack → false, no `iretq` yet).
        use super::super::page_table::{
            encode_table_entry, pte_decoded_flags, pte_encode, PageFlags,
        };
        assert_eq!(USER_STACK_PAGES, 4);
        assert_eq!(USER_STACK_LEN % 4096, 0);
        // stack_base 0x0000_1000_0020_0000 (16 KiB-aligned, one PT span):
        // PML4[32] -> PDPT[0] -> PD[1] -> PT[0..3] (4 consecutive leaves).
        let stack_base = 0x0000_1000_0020_0000_u64;
        assert_eq!(stack_base % 16384, 0, "all 4 pages share one PT");
        // Index pins (recomputed, not trusted): a wrong slot fails the walk
        // below loudly instead of silently covering the wrong pages.
        assert_eq!((stack_base >> 39) & 511, 32);
        assert_eq!((stack_base >> 30) & 511, 0);
        assert_eq!((stack_base >> 21) & 511, 1);
        assert_eq!((stack_base >> 12) & 511, 0);
        let src = |flags: PageFlags| pte_decoded_flags(pte_encode(0x1_0000, flags));
        let stack_nx =
            src(PageFlags::PRESENT | PageFlags::USER | PageFlags::WRITABLE | PageFlags::NO_EXEC);
        let stack_exec = src(PageFlags::PRESENT | PageFlags::USER | PageFlags::WRITABLE);
        let next = PageFlags::PRESENT | PageFlags::WRITABLE;
        let build_tables = |leaf: PageFlags| {
            let mut pml4: [u64; 512] = [0; 512];
            let mut pdpt: [u64; 512] = [0; 512];
            let mut pd: [u64; 512] = [0; 512];
            let mut ptable: [u64; 512] = [0; 512];
            pml4[32] = encode_table_entry(0x2000, next).unwrap();
            pdpt[0] = encode_table_entry(0x3000, next).unwrap();
            pd[1] = encode_table_entry(0x4000, next).unwrap();
            let mut i = 0usize;
            while i < USER_STACK_PAGES {
                ptable[i] = pte_encode(0x60_0000 + (i as u64) * 4096, leaf);
                i += 1;
            }
            (pml4, pdpt, pd, ptable)
        };
        // All-NX pages: walk yields Some(all-NX), wrapper allows while NXE=1.
        let (pml4, pdpt, pd, ptable) = build_tables(
            PageFlags::PRESENT | PageFlags::USER | PageFlags::WRITABLE | PageFlags::NO_EXEC,
        );
        let lookup = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4),
                0x2000 => Some(&pdpt),
                0x3000 => Some(&pd),
                0x4000 => Some(&ptable),
                _ => None,
            }
        };
        let pages = walk_user_stack_pages(0x1000, stack_base, lookup)
            .expect("all 4 pages mapped, walk must succeed");
        assert_eq!(pages, [stack_nx; USER_STACK_PAGES]);
        assert!(all_stack_pages_valid_for(&pages, true));
        // NX pages with NXE=0 refuse (bit 63 MBZ while NXE=0).
        assert!(!all_stack_pages_valid_for(&pages, false));
        // Page-3 exec: walk still succeeds (present) but the wrapper refuses
        // — a single good flag for page 0 cannot cover it.
        let (pml4e, pdpte, pde, mut ptablee) = build_tables(
            PageFlags::PRESENT | PageFlags::USER | PageFlags::WRITABLE | PageFlags::NO_EXEC,
        );
        ptablee[3] = pte_encode(
            0x60_0000 + 3 * 4096,
            PageFlags::PRESENT | PageFlags::USER | PageFlags::WRITABLE,
        );
        let lookupe = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4e),
                0x2000 => Some(&pdpte),
                0x3000 => Some(&pde),
                0x4000 => Some(&ptablee),
                _ => None,
            }
        };
        let bad_pages = walk_user_stack_pages(0x1000, stack_base, lookupe)
            .expect("present-but-exec page still walks");
        assert_eq!(bad_pages[3], stack_exec);
        assert!(
            !all_stack_pages_valid_for(&bad_pages, true),
            "page 3 exec refuses"
        );
        // Unmapped PT (lookup miss): walk fails closed with None (no partial
        // array), so the stub refuses instead of gating 3 of 4 pages.
        let lookup_miss = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4),
                0x2000 => Some(&pdpt),
                0x3000 => Some(&pd),
                _ => None,
            }
        };
        assert_eq!(walk_user_stack_pages(0x1000, stack_base, lookup_miss), None);
        // S09 slice2v (REVIEW #666 risk 2): near-top base refuses instead of
        // wrapping to low VA — `u64::MAX - 4096` (0xFFFF_FFFF_FFFF_EFFF)
        // overflows at i=2 (`checked_add` fails on base + 8192), not page 1
        // (i=1 is exactly `u64::MAX`, no overflow). Page 0 already misses
        // here, so this pins the fail-closed shape only.
        let lookup_top_miss = |_: u64| -> Option<&[u64; 512]> { None };
        assert_eq!(
            walk_user_stack_pages(0x1000, u64::MAX - 4096, lookup_top_miss),
            None
        );
        // S09 slice2w (REVIEW #668 weakness 1/2): discriminating overflow
        // kill — map the high pair (i=0 `...EFFF` PT[510], i=1 `...FFFF`
        // PT[511]) AND the old wrap targets (i=2 `0xFFF` PT_low[0], i=3
        // `0x1FFF` PT_low[1]), so old `wrapping_add` code would walk all
        // four and return `Some` while `checked_add` refuses with `None`
        // at i=2 without touching low VA.
        let mut pml4t: [u64; 512] = [0; 512];
        let mut pdpt_lo: [u64; 512] = [0; 512];
        let mut pdpt_hi: [u64; 512] = [0; 512];
        let mut pd_lo: [u64; 512] = [0; 512];
        let mut pd_hi: [u64; 512] = [0; 512];
        let mut pt_lo: [u64; 512] = [0; 512];
        let mut pt_hi: [u64; 512] = [0; 512];
        pml4t[0] = encode_table_entry(0x2000, next).unwrap();
        pml4t[511] = encode_table_entry(0x3000, next).unwrap();
        pdpt_lo[0] = encode_table_entry(0x4000, next).unwrap();
        pdpt_hi[511] = encode_table_entry(0x5000, next).unwrap();
        pd_lo[0] = encode_table_entry(0x6000, next).unwrap();
        pd_hi[511] = encode_table_entry(0x7000, next).unwrap();
        let leaf_nx =
            PageFlags::PRESENT | PageFlags::USER | PageFlags::WRITABLE | PageFlags::NO_EXEC;
        pt_hi[510] = pte_encode(0x70_0000, leaf_nx);
        pt_hi[511] = pte_encode(0x71_0000, leaf_nx);
        pt_lo[0] = pte_encode(0x72_0000, leaf_nx);
        pt_lo[1] = pte_encode(0x73_0000, leaf_nx);
        let lookup_top_wrap = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4t),
                0x2000 => Some(&pdpt_lo),
                0x3000 => Some(&pdpt_hi),
                0x4000 => Some(&pd_lo),
                0x5000 => Some(&pd_hi),
                0x6000 => Some(&pt_lo),
                0x7000 => Some(&pt_hi),
                _ => None,
            }
        };
        assert_eq!(
            walk_user_stack_pages(0x1000, u64::MAX - 4096, lookup_top_wrap),
            None
        );
    }

    #[test]
    fn user_stack_map_flags_pin_rw_nx_user_no_code_reuse() {
        // S09 slice2x (#30 remainder): pin the flag word the future mapping
        // slice must install — RW data, never W+X code. Host-only; no guest
        // PTE written yet.
        use super::super::page_table::PageFlags;
        let f = USER_STACK_MAP_FLAGS;
        assert!(f.contains(PageFlags::PRESENT));
        assert!(f.contains(PageFlags::USER));
        assert!(f.contains(PageFlags::WRITABLE));
        assert!(f.contains(PageFlags::NO_EXEC));
        // Stack gate passes with live NXE=1, refuses with NXE=0 (bit 63 MBZ).
        assert!(user_stack_perms_valid(f));
        assert!(user_stack_perms_valid_for(f, true));
        assert!(!user_stack_perms_valid_for(f, false));
        // Code gate refuses: NX set, so the word cannot be reused code-side.
        assert!(!user_code_perms_valid(f));
        // Full 4-page coverage with this word passes the per-page stack gate.
        assert!(all_stack_pages_valid_for(&[f; USER_STACK_PAGES], true));
        assert!(!all_stack_pages_valid_for(&[f; USER_STACK_PAGES], false));
        // Slice2y: exact-equality mapping gate — exact word passes, GUARD
        // extra refuses (contains-gate would accept), any missing bit refuses.
        assert!(user_stack_map_flags_valid(f));
        assert!(!user_stack_map_flags_valid(f.union(PageFlags::GUARD)));
        assert!(!user_stack_map_flags_valid(
            PageFlags::PRESENT
                .union(PageFlags::USER)
                .union(PageFlags::WRITABLE)
        ));
        assert!(!user_stack_map_flags_valid(PageFlags::empty()));
        // Slice2z: per-page exact gate — the mapping stub's call site.
        // Exact word on all 4 pages passes; one GUARD-tagged page refuses
        // even though the contains-gate accepts it (discriminating pin);
        // empty refuses.
        assert!(all_stack_map_flags_valid(&[f; USER_STACK_PAGES]));
        assert!(!all_stack_map_flags_valid(&[]));
        let mut guard4 = [f; USER_STACK_PAGES];
        guard4[2] = f.union(PageFlags::GUARD);
        assert!(!all_stack_map_flags_valid(&guard4));
        assert!(all_stack_pages_valid_for(&guard4, true));
    }

    #[test]
    fn user_stack_page_va_pins_per_page_addrs_and_fail_closed() {
        // S09 slice2aa (#30 remainder): the mapping stub maps all 4 pages
        // one VA at a time — pin the VA math host-side: consecutive 4 KiB
        // VAs for an aligned base, OOB index refuses, near-top overflow
        // refuses instead of wrapping to low VA. Host-only; no guest PTE
        // written yet.
        let base = 0x0000_1000_0020_0000_u64;
        assert_eq!(user_stack_page_va(base, 0), Some(base));
        assert_eq!(user_stack_page_va(base, 1), Some(base + 4096));
        assert_eq!(user_stack_page_va(base, 2), Some(base + 8192));
        assert_eq!(user_stack_page_va(base, 3), Some(base + 12288));
        assert_eq!(user_stack_page_va(base, USER_STACK_PAGES), None);
        assert_eq!(user_stack_page_va(base, 100), None);
        // Near-top: index 0/1 still encodable, index 2 overflows.
        let top = u64::MAX - 4096;
        assert_eq!(user_stack_page_va(top, 0), Some(top));
        assert_eq!(user_stack_page_va(top, 1), Some(u64::MAX));
        assert_eq!(user_stack_page_va(top, 2), None);
        assert_eq!(user_stack_page_va(u64::MAX, 0), Some(u64::MAX));
        assert_eq!(user_stack_page_va(u64::MAX, 1), None);
        // Walk still succeeds through the helper (non-interference).
        assert_eq!(USER_STACK_PAGES, 4);
    }

    #[test]
    fn user_stack_map_install_ready_binds_both_helpers() {
        // S09 slice2ab (#30 remainder): the install stub must gate the 4-page
        // U-map on BOTH helpers — exact-word flags AND encodable per-page VAs
        // — plus live NXE, 4-page coverage, and base alignment. Host-only;
        // no guest PTE written yet.
        let base = 0x0000_1000_0020_0000_u64;
        let good = [USER_STACK_MAP_FLAGS; USER_STACK_PAGES];
        assert!(user_stack_map_install_ready(base, &good, true));
        assert!(!user_stack_map_install_ready(base, &good, false));
        assert!(!user_stack_map_install_ready(base + 1, &good, true));
        assert!(!user_stack_map_install_ready(base, &[], true));
        assert!(!user_stack_map_install_ready(base, &good[..3], true));
        let mut guard4 = good;
        guard4[2] = USER_STACK_MAP_FLAGS.union(PageFlags::GUARD);
        assert!(!user_stack_map_install_ready(base, &guard4, true));
        // Aligned near-`u64::MAX` base: per-page `checked_add` overflows, so
        // the VA arm refuses instead of wrapping to low VA.
        assert!(!user_stack_map_install_ready(
            0xFFFF_FFFF_FFFF_F000,
            &good,
            true
        ));
    }

    #[test]
    fn user_stack_install_ptes_writes_real_pte_words() {
        // S09 slice2ac (#30 remainder, REVIEW #677 risk 1): the install gate
        // must CONSUME into a real PTE-write call site — this encoder calls
        // `user_stack_map_install_ready` then `pte_encode_checked` per page.
        // Host-only; the guest PT store + CPL3 entry + QEMU proof remain.
        use super::super::page_table::{pte_decoded_flags, pte_decoded_phys};
        let base = 0x0000_1000_0020_0000_u64;
        let phys = 0x0000_0000_0060_0000_u64;
        let good = [USER_STACK_MAP_FLAGS; USER_STACK_PAGES];
        // Good: Some with exact per-page words (phys + flags round-trip,
        // GUARD bit masked to 0 at encode).
        let ptes = user_stack_install_ptes(base, phys, &good, true)
            .expect("good install must encode 4 PTE words");
        let mut i = 0usize;
        while i < USER_STACK_PAGES {
            let expect = phys + (i as u64) * 4096;
            assert_eq!(pte_decoded_phys(ptes[i]), expect);
            assert_eq!(pte_decoded_flags(ptes[i]), USER_STACK_MAP_FLAGS);
            assert_eq!(ptes[i] & (1 << 9), 0, "GUARD never reaches HW");
            i += 1;
        }
        // GUARD-tagged page: readiness refuses before encode (no partial).
        let mut guard4 = good;
        guard4[2] = USER_STACK_MAP_FLAGS.union(PageFlags::GUARD);
        assert_eq!(user_stack_install_ptes(base, phys, &guard4, true), None);
        // Wrong-word flags (missing USER — REVIEW #677 note 2 gap): refuses.
        let mut nouser4 = good;
        nouser4[1] = PageFlags::PRESENT | PageFlags::WRITABLE | PageFlags::NO_EXEC;
        assert_eq!(user_stack_install_ptes(base, phys, &nouser4, true), None);
        // Live NXE=0 refuses (NX stack needs NXE).
        assert_eq!(user_stack_install_ptes(base, phys, &good, false), None);
        // Misaligned stack base refuses via the readiness gate.
        assert_eq!(user_stack_install_ptes(base + 1, phys, &good, true), None);
        // Misaligned phys base refuses at encode (no readiness cover).
        assert_eq!(user_stack_install_ptes(base, phys + 1, &good, true), None);
        // Short coverage refuses (no partial install).
        assert_eq!(user_stack_install_ptes(base, phys, &good[..3], true), None);
        // Aligned near-MAX VA base: per-page VA overflow refuses.
        assert_eq!(
            user_stack_install_ptes(0xFFFF_FFFF_FFFF_F000, phys, &good, true),
            None
        );
        // Aligned near-MAX phys base: per-page phys overflow refuses.
        assert_eq!(
            user_stack_install_ptes(base, 0xFFFF_FFFF_FFFF_F000, &good, true),
            None
        );
    }

    #[test]
    fn user_stack_store_ptes_writes_single_pt_no_partial() {
        // S09 slice2ad (#30 remainder, REVIEW #679 risk 1): the store loop
        // consumes install_ptes words into one PT at page_table_index(va,0).
        use super::super::page_table::page_table_index;
        let base = 0x0000_1000_0020_0000_u64;
        let phys = 0x0000_0000_0060_0000_u64;
        let good = [USER_STACK_MAP_FLAGS; USER_STACK_PAGES];
        let ptes = user_stack_install_ptes(base, phys, &good, true)
            .expect("good install must encode 4 PTE words");
        // Good: true, each word lands at its per-page PT slot.
        let mut pt = [0u64; 512];
        assert!(user_stack_store_ptes(&mut pt, base, ptes));
        let mut i = 0usize;
        while i < USER_STACK_PAGES {
            let va = user_stack_page_va(base, i).expect("good VA encodable");
            let slot = page_table_index(va, 0).expect("level 0 always Some") as usize;
            assert_eq!(pt[slot], ptes[i]);
            i += 1;
        }
        // Cross-PT base (last page of a 2 MiB PT range): false, PT untouched.
        let mut cross = [0u64; 512];
        assert!(!user_stack_store_ptes(&mut cross, 0x1FF000, ptes));
        assert_eq!(cross, [0u64; 512]);
        // VA-overflow base: false, PT untouched (no partial of page 0).
        let mut over = [0u64; 512];
        assert!(!user_stack_store_ptes(
            &mut over,
            0xFFFF_FFFF_FFFF_F000,
            ptes
        ));
        assert_eq!(over, [0u64; 512]);
    }

    #[test]
    fn user_stack_install_store_binds_encode_no_mismatched_words() {
        // S09 slice2ae (#30 remainder, REVIEW #681 residual 1): the bound
        // entry re-encodes from (base, phys, flags) so a mismatched caller
        // `ptes` array can never install silently — there is no caller
        // array to mismatch. Host-only; guest TLB/CPL3/QEMU remain.
        use super::super::page_table::{page_table_index, pte_decoded_phys};
        let base = 0x0000_1000_0020_0000_u64;
        let phys = 0x0000_0000_0060_0000_u64;
        let good = [USER_STACK_MAP_FLAGS; USER_STACK_PAGES];
        // Good: true, words land at per-page slots with expected phys.
        let mut pt = [0u64; 512];
        assert!(user_stack_install_store_ptes(
            &mut pt, base, phys, &good, true
        ));
        let mut i = 0usize;
        while i < USER_STACK_PAGES {
            let va = user_stack_page_va(base, i).expect("good VA encodable");
            let slot = page_table_index(va, 0).expect("level 0 always Some") as usize;
            assert_eq!(pte_decoded_phys(pt[slot]), phys + (i as u64) * 4096);
            i += 1;
        }
        // Residual-1 proof: a tampered word installs via the raw store but
        // can never flow through the bound entry (it re-encodes internally).
        let ptes =
            user_stack_install_ptes(base, phys, &good, true).expect("good install must encode");
        let mut tampered = ptes;
        tampered[0] ^= 0x0000_0000_0010_0000;
        let mut raw = [0u64; 512];
        assert!(user_stack_store_ptes(&mut raw, base, tampered));
        assert_ne!(
            raw, pt,
            "raw store trusts caller words; bound path must not"
        );
        // Bound refusals leave PT untouched: bad flags, NXE=0, misaligned
        // phys, cross-PT base, VA-overflow base.
        let mut guard4 = good;
        guard4[2] = USER_STACK_MAP_FLAGS.union(PageFlags::GUARD);
        let mut refused = [0u64; 512];
        assert!(!user_stack_install_store_ptes(
            &mut refused,
            base,
            phys,
            &guard4,
            true
        ));
        assert_eq!(refused, [0u64; 512]);
        assert!(!user_stack_install_store_ptes(
            &mut refused,
            base,
            phys,
            &good,
            false
        ));
        assert_eq!(refused, [0u64; 512]);
        assert!(!user_stack_install_store_ptes(
            &mut refused,
            base,
            phys + 1,
            &good,
            true
        ));
        assert_eq!(refused, [0u64; 512]);
        assert!(!user_stack_install_store_ptes(
            &mut refused,
            0x1FF000,
            phys,
            &good,
            true
        ));
        assert_eq!(refused, [0u64; 512]);
        assert!(!user_stack_install_store_ptes(
            &mut refused,
            0xFFFF_FFFF_FFFF_F000,
            phys,
            &good,
            true
        ));
        assert_eq!(refused, [0u64; 512]);
    }

    #[test]
    fn user_stack_flush_vas_pins_invlpg_set_and_store_agreement() {
        // S09 slice2ag (#30 remainder, REVIEW #685 residual c): the guest
        // `invlpg` loop must flush exactly the VAs the store wrote — one
        // rule (`user_stack_page_va`) with store-matching refusal shape.
        let base = 0x0000_1000_0020_0000_u64;
        let vas = user_stack_flush_vas(base).expect("good base must yield 4 VAs");
        let mut i = 0usize;
        while i < USER_STACK_PAGES {
            assert_eq!(
                vas[i],
                user_stack_page_va(base, i).expect("good VA encodable")
            );
            assert_eq!(vas[i], base + (i as u64) * 4096);
            i += 1;
        }
        // Agreement: store-true base has a flush set; store-refusal shapes
        // (cross-PT span, VA overflow) have none — nothing was written, so
        // nothing to flush.
        let phys = 0x0000_0000_0060_0000_u64;
        let good = [USER_STACK_MAP_FLAGS; USER_STACK_PAGES];
        let ptes =
            user_stack_install_ptes(base, phys, &good, true).expect("good install must encode");
        let mut pt = [0u64; 512];
        assert!(user_stack_store_ptes(&mut pt, base, ptes));
        assert!(user_stack_flush_vas(base).is_some());
        assert!(user_stack_flush_vas(0x1FF000).is_none());
        assert!(user_stack_flush_vas(0xFFFF_FFFF_FFFF_F000).is_none());
        // S09 slice2ah (REVIEW #686 residuals 1-2): the flush set IS the
        // shared single-PT helper (no drift), level-0 index is always Some
        // (dead refusal shape), and the 4 consecutive VAs in one PT map to
        // pairwise-distinct slots (slot-collision unreachable).
        use super::super::page_table::page_table_index;
        assert_eq!(user_stack_flush_vas(base), user_stack_single_pt_vas(base));
        let mut slots = [0usize; USER_STACK_PAGES];
        let mut k = 0usize;
        while k < USER_STACK_PAGES {
            slots[k] = page_table_index(vas[k], 0).expect("level 0 always Some") as usize;
            k += 1;
        }
        let mut a = 0usize;
        while a < USER_STACK_PAGES {
            let mut b = a + 1;
            while b < USER_STACK_PAGES {
                assert_ne!(slots[a], slots[b], "consecutive VAs must not collide");
                b += 1;
            }
            a += 1;
        }
        // S09 slice2ai (REVIEW #688 nit 2): PT-boundary edge — base PT
        // index 508 (508..511) still fits one PT (`Some`), base index 509
        // (509..509+3 wraps past slot 511 into the next 2 MiB region)
        // span-refuses (`None`) in both the helper and the flush set, and
        // the store leaves `pt` untouched while the flush loop refuses.
        let base508 = 508u64 * 4096;
        let base509 = 509u64 * 4096;
        assert!(user_stack_single_pt_vas(base508).is_some());
        assert!(user_stack_single_pt_vas(base509).is_none());
        assert!(user_stack_flush_vas(base508).is_some());
        assert!(user_stack_flush_vas(base509).is_none());
        let ptes508 =
            user_stack_install_ptes(base508, phys, &good, true).expect("508 install must encode");
        let mut pt508 = [0u64; 512];
        assert!(user_stack_store_ptes(&mut pt508, base508, ptes508));
        let mut pt509 = [0u64; 512];
        let ptes509 = user_stack_install_ptes(base509, phys, &good, true)
            .expect("509 install encodes (span is store-side)");
        assert!(!user_stack_store_ptes(&mut pt509, base509, ptes509));
        assert_eq!(pt509, [0u64; 512]);
        // SAFETY: host `invlpg_page` stub is a no-op; asserts only the
        // refuse-nothing-written / accept-flush agreement.
        unsafe {
            assert!(user_stack_flush_tlb(base508));
            assert!(!user_stack_flush_tlb(base509));
            assert!(!user_stack_flush_tlb(0x1FF000));
        }
    }

    #[test]
    fn gate_for_pins_kernel_code_selector() {
        // Safe wrapper must pin KERNEL_CODE_SEL so a gate can never target
        // the null/data segment; encoding still round-trips the handler.
        let handler = 0x0000_0000_DEAD_BEEF_u64;
        let e = gate_for(handler);
        assert_eq!(e.handler_offset(), handler);
        let (selector, type_attr, ist, reserved) = (e.selector, e.type_attr, e.ist, e.reserved);
        assert_eq!(selector, KERNEL_CODE_SEL);
        assert_eq!(selector, 0x08);
        assert_eq!(type_attr, 0x8E, "present interrupt gate DPL0");
        assert_eq!(ist, 0);
        assert_eq!(reserved, 0);
    }

    #[test]
    fn timer_gate_uses_interrupt_gate_covering_vector_32() {
        // S07 slice13: the wiring slice wires VECTOR_TIMER through the same
        // `gate_for` path as the exception gates — a present DPL0 interrupt
        // gate (type 0xE, which clears IF in HW so the IrqGuard entry-proof
        // holds), in the kernel code segment, with the table covering the
        // vector. Still `cli` + no PIC/PIT this slice, so QEMU is unchanged.
        use super::super::timer::VECTOR_TIMER;
        assert_eq!(VECTOR_TIMER, 32);
        assert_eq!(IDT_VECTORS, 33);
        // `black_box` keeps the coverage assert runtime (not
        // const-evaluable) so `clippy -D warnings`
        // (`assertions_on_constants`) stays clean — same pattern as the
        // probe-guard test in `lib.rs`.
        assert!(core::hint::black_box(VECTOR_TIMER) < IDT_VECTORS);
        let handler = 0x0000_0000_1234_5678_u64;
        let e = gate_for(handler);
        assert_eq!(e.handler_offset(), handler);
        let (selector, type_attr, ist, reserved) = (e.selector, e.type_attr, e.ist, e.reserved);
        assert_eq!(selector, KERNEL_CODE_SEL);
        assert_eq!(type_attr, 0x8E, "present interrupt gate DPL0");
        assert_eq!(
            type_attr & 0x0F,
            0x0E,
            "gate type must be 0xE (IF cleared by HW)"
        );
        assert_eq!(ist, 0);
        assert_eq!(reserved, 0);
    }

    #[test]
    fn probe_addr_valid_matches_guard_contract() {
        use super::super::PF_PROBE_ADDR;
        // The live-probe constant must pass its own gate; neighbors that
        // break alignment / canonicity / RAM-floor must fail.
        assert!(probe_addr_valid(PF_PROBE_ADDR));
        assert_eq!(PF_PROBE_ADDR, 0x0000_5000_0000_0000);
        assert!(!probe_addr_valid(PF_PROBE_ADDR + 1), "unaligned must fail");
        assert!(!probe_addr_valid(0x0000_8000_0000_0000), "hole must fail");
        assert!(!probe_addr_valid(0x8000_0000), "firmware RAM must fail");
        assert!(!probe_addr_valid(0), "null must fail");
    }

    #[test]
    fn cr3_pml4_base_agrees_with_page_table() {
        // S05 slice 5: safe wrapper must match `page_table::cr3_decode` on
        // flag-masked, zero, and all-ones inputs (PML4 base = addr bits).
        use super::super::page_table::{cr3_decode, PTE_ADDR_MASK};
        for cr3 in [0u64, 0x7F00_1000, 0x7F00_1003, u64::MAX] {
            assert_eq!(cr3_pml4_base(cr3), cr3_decode(cr3));
        }
        assert_eq!(cr3_pml4_base(0xFFFF_FFFF_FFFF_FFFF), PTE_ADDR_MASK);
        assert_eq!(cr3_pml4_base(0x7F00_1000), 0x7F00_1000);
    }

    #[test]
    fn efer_nxe_enabled_agrees_with_page_table() {
        // S05 slice 5: safe wrapper must match
        // `page_table::efer_nxe_enabled` (bit 11) exactly.
        use super::super::page_table::{efer_nxe_enabled as raw_nxe, EFER_MSR, EFER_NXE_BIT};
        assert_eq!(EFER_MSR, 0xC000_0080);
        for efer in [0u64, EFER_NXE_BIT, !EFER_NXE_BIT, u64::MAX] {
            assert_eq!(efer_nxe_enabled(efer), raw_nxe(efer));
        }
        assert!(efer_nxe_enabled(EFER_NXE_BIT));
        assert!(!efer_nxe_enabled(0));
    }

    #[test]
    fn cr3_efer_wrappers_are_safe_without_unsafe_block() {
        // S05 slice 5: the decode wrappers are safe pure `const fn` — bind
        // to the plain `fn` type to prove safeness at compile time (an
        // `unsafe fn` would not coerce), mirroring the UART pin in lib.rs.
        let cr3_base: fn(u64) -> u64 = cr3_pml4_base;
        let nxe: fn(u64) -> bool = efer_nxe_enabled;
        let probe: fn(u64) -> bool = probe_addr_valid;
        let _ = (cr3_base, nxe, probe);
    }

    #[test]
    fn live_register_reads_are_unsafe_fn() {
        // S05 slice 6: the moved `read_cr3` / `read_efer` must stay
        // `unsafe fn` (privileged `mov {}, cr3` / `rdmsr`) — bind to the
        // `unsafe fn` type to prove callers need an `unsafe` block. A
        // safe-ified signature would fail to coerce here. Never called
        // (would fault outside the guest).
        let cr3: unsafe fn() -> u64 = read_cr3;
        let efer: unsafe fn() -> u64 = read_efer;
        let _ = (cr3, efer);
    }

    #[test]
    fn irq_gate_is_unsafe_enter_and_host_noop() {
        // S06 preempt-IRQ gate (task #19): `cli` + `IrqGuard::enter` must
        // stay `unsafe fn` (privileged `cli` on the guest) — bind to the
        // `unsafe fn` type so a safe-ified signature fails to coerce.
        // `enter` is a pure token on every build (no asm — the live `cli`
        // stays once in `efi_main`), so constructing it on the host never
        // touches flags; the guest runs it downstream of `cli`.
        let cli_fn: unsafe fn() = cli;
        let enter: unsafe fn() -> IrqGuard = IrqGuard::enter;
        let _ = (cli_fn, enter);
        let gate = unsafe { IrqGuard::enter() };
        let gate2 = gate;
        assert_eq!(gate, gate2, "guard is a Copy token on the host");
        unsafe { cli() };
    }

    #[test]
    fn sti_gate_is_unsafe_fn_and_host_noop() {
        // S07 slice14: `sti` is the paired enable for the timer IRQ0 path —
        // must stay `unsafe fn` (privileged, same as `cli`) so callers need
        // an `unsafe` block. Host stub is a no-op (never touches flags).
        let sti_fn: unsafe fn() = sti;
        let _ = sti_fn;
        unsafe { sti() };
    }

    #[test]
    fn read_rsp_is_nonzero_and_nests_down() {
        // S07 slice68 (#21 nit 3): `read_rsp` is a safe `fn` (bind proves
        // no `unsafe` block needed at call sites), nonzero on a live
        // stack, and a nested call observes a LOWER address (x86-64
        // stacks grow down) — the shape the guest IRQ/task proof relies
        // on (`irq != task`).
        let rsp_fn: fn() -> u64 = read_rsp;
        let _ = rsp_fn;
        #[inline(never)]
        fn inner() -> u64 {
            read_rsp()
        }
        let outer = read_rsp();
        let inner_rsp = inner();
        assert_ne!(outer, 0);
        assert_ne!(inner_rsp, 0);
        assert!(
            inner_rsp < outer,
            "nested frame must sit below the caller: inner={inner_rsp:#x} outer={outer:#x}"
        );
    }

    #[test]
    fn read_tr_is_safe_fn_shape() {
        // S09 slice2k: `str` is unprivileged, so `read_tr` is a safe `fn`
        // (bind proves no `unsafe` block at call sites). Never asserts the
        // value (host TR is the harness's, not TSS_SEL).
        let tr_fn: fn() -> u16 = read_tr;
        let _ = tr_fn;
    }

    #[test]
    fn tsc_reads_are_unsafe_fn_and_delta_saturates() {
        // S07 slice52 (#21 R3): fenced/unfenced reads stay `unsafe fn`;
        // `tsc_delta` is a pure saturating `const fn` (reorder -> 0).
        let raw: unsafe fn() -> u64 = read_tsc;
        let fenced: unsafe fn() -> u64 = read_tsc_fenced;
        let pair: unsafe fn() -> u64 = empty_tsc_pair;
        let warm: unsafe fn() = warm_tsc;
        let _ = (raw, fenced, pair, warm);
        assert_eq!(TSC_WARMUP_ITERS, 128);
        let d: fn(u64, u64) -> u64 = tsc_delta;
        assert_eq!(d(10, 25), 15);
        assert_eq!(d(25, 25), 0);
        assert_eq!(d(25, 10), 0);
        assert_eq!(d(0, u64::MAX), u64::MAX);
        // S07 slice53 (#21 R3): overhead-corrected sample floors at 0.
        let s: fn(u64, u64, u64) -> u64 = switch_sample_delta;
        assert_eq!(s(10, 110, 20), 80);
        assert_eq!(s(10, 25, 100), 0);
        assert_eq!(s(25, 10, 0), 0);
    }

    #[test]
    fn fenced_tsc_is_monotonic_with_finite_empty_floor() {
        // S07 slice52 (#21 R3): warm-up completes, back-to-back fenced
        // pairs are monotonic with a finite floor. No tight upper bound:
        // hosted TSC granularity varies (TCG/WHPX comparison is C2).
        unsafe {
            warm_tsc();
            let a = read_tsc_fenced();
            let b = read_tsc_fenced();
            assert!(b >= a);
            assert_eq!(tsc_delta(a, b), b - a);
            let mut worst = 0u64;
            let mut i = 0usize;
            while i < 16 {
                let floor = empty_tsc_pair();
                assert!(floor <= 10_000_000, "empty pair must stay finite");
                if floor > worst {
                    worst = floor;
                }
                i += 1;
            }
            let _ = worst;
        }
    }
}
