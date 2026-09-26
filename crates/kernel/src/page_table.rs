//! S04 stage-1 page-table abstraction (host-testable software model).
//!
//! Safe wrapper around x86-64 paging semantics: 4 KiB pages, PRESENT /
//! WRITABLE / USER / NO-EXECUTE permission bits, plus a GUARD bit for
//! stack-guard pages. The rest of the kernel must go through this module
//! instead of manipulating entries directly (implementation-guide §7).
//!
//! Stage 1 is a fixed-capacity software model (`64` mappings, sorted
//! linear scan) so map/unmap/permission/guard behavior is fully
//! unit-testable on the host with no allocator and no `unsafe`. Hardware
//! PTE encode + index/`CR3` helpers below are the second half of the slice:
//! pure `core`-only functions the guest loader will call, with the `GUARD`
//! (bit 9) masking obligation pinned by tests. The permission-check contract
//! (`check`) already mirrors the fault kinds the guest stub will assert.
//!
//! Hardware scope (this slice is encode-only, no guest wiring yet):
//! sorted insert + linear scan is `O(n)`, fine for 64 software entries but
//! must not carry to a hardware walker; the guest walker indexes each level
//! directly via [`page_table_index`]. After loading a real PTE the guest must
//! `invlpg` the page (or reload `CR3`) — stale TLB entries are not modeled
//! here. `Accessed` (bit 5) / `Dirty` (bit 6) are set by hardware and load as
//! 0 from [`pte_encode`]; software must never branch on them. Guest `CR3`
//! load + QEMU boot with e2e fault-line asserts ride with the next
//! guest-touching slice (guest untouched here).
//!
//! `no_std`-clean (`core` only).

use baseos_boot_info::BootInfo;

/// Page shift for 4 KiB pages.
pub const PAGE_SHIFT: u32 = 12;
/// Page size in bytes.
pub const PAGE_SIZE: u64 = 1 << PAGE_SHIFT;
/// Page offset mask (`virt & PAGE_OFFSET_MASK`).
pub const PAGE_OFFSET_MASK: u64 = PAGE_SIZE - 1;

/// Page-table permission bits (x86-64 PTE semantics subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlags(u64);

impl PageFlags {
    /// No bits set.
    pub const NONE: Self = Self(0);
    /// Page is present (clear means `NotPresent` on access).
    pub const PRESENT: Self = Self(1 << 0);
    /// Writes allowed (clear means `ReadOnly` on write).
    pub const WRITABLE: Self = Self(1 << 1);
    /// User-accessible (clear means supervisor-only: `UserViolation`).
    pub const USER: Self = Self(1 << 2);
    /// No-execute (set means `NoExec` on instruction fetch).
    pub const NO_EXEC: Self = Self(1 << 63);
    /// Guard page: every access faults with [`Fault::Guard`], even reads.
    pub const GUARD: Self = Self(1 << 9);

    /// Empty flag set.
    #[must_use]
    pub const fn empty() -> Self {
        Self::NONE
    }

    /// Raw bits.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Rebuild from raw bits (decode path; `GUARD` naturally absent when the
    /// bits came from [`pte_decoded_flags`]).
    ///
    /// Masked to [`TRUSTED_FLAGS_MASK`] (REVIEW #122 finding 1): raw `u64`
    /// callers cannot forge `Accessed`/`Dirty` or reserved bits; `GUARD`
    /// itself stays forgeable by construction (OS state), so treat it as
    /// trusted-only input.
    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits & TRUSTED_FLAGS_MASK)
    }

    /// True when all bits in `other` are set in `self`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Bitwise union.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl core::ops::BitOr for PageFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

/// `map` / `unmap` failure modes (all fail-closed: no table mutation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    /// `virt` or `phys` is not 4 KiB aligned.
    Misaligned,
    /// `virt` page already has a mapping.
    AlreadyMapped,
    /// `virt` page has no mapping.
    NotMapped,
    /// Fixed-capacity table is full.
    TableFull,
    /// Flags carry neither `PRESENT` nor `GUARD` (an unusable entry).
    InvalidFlags,
}

/// Page-fault kinds reported by [`PageTable::check`] (mirrors the guest
/// fault stub the e2e script will assert in a later slice).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// No mapping covers `virt` (or entry lacks `PRESENT`).
    NotPresent,
    /// Write to a read-only mapping.
    ReadOnly,
    /// Instruction fetch from a `NO_EXEC` mapping.
    NoExec,
    /// User access to a supervisor-only mapping.
    UserViolation,
    /// Any access to a `GUARD` page.
    Guard,
}

/// Access kind under test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessKind {
    /// Data read (always allowed when present and visible).
    Read,
    /// Data write (needs `WRITABLE`).
    Write,
    /// Instruction fetch (blocked by `NO_EXEC`).
    Execute,
}

/// Maximum mappings in the stage-1 model (static, no allocator).
pub const MAX_MAPPINGS: usize = 64;

/// One mapped 4 KiB page: aligned bases plus permission bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mapping {
    virt_base: u64,
    phys_base: u64,
    flags: PageFlags,
}

/// Stage-1 page table: sorted fixed-capacity map of 4 KiB pages.
///
/// Sorted by `virt_base` so `translate`/`check` scan in address order and
/// tests can pin iteration-independent behavior. `count <= MAX_MAPPINGS`.
#[derive(Debug, Clone)]
pub struct PageTable {
    entries: [Option<Mapping>; MAX_MAPPINGS],
    count: usize,
}

impl PageTable {
    /// Empty table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_MAPPINGS],
            count: 0,
        }
    }

    /// Number of mapped pages.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    /// True when no pages are mapped.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Map one 4 KiB page: `virt -> phys` with `flags`.
    ///
    /// Requires 4 KiB alignment on both addresses and at least one of
    /// `PRESENT` / `GUARD` in `flags`. Duplicate `virt` pages and a full
    /// table fail closed with no mutation.
    ///
    /// Physical aliasing is intended: mapping the same `phys` at two
    /// different `virt` pages succeeds (shared / copy-on-write foundations).
    /// Only `virt` duplicates are rejected.
    pub fn map(&mut self, virt: u64, phys: u64, flags: PageFlags) -> Result<(), MapError> {
        if virt & PAGE_OFFSET_MASK != 0 || phys & PAGE_OFFSET_MASK != 0 {
            return Err(MapError::Misaligned);
        }
        if !flags.contains(PageFlags::PRESENT) && !flags.contains(PageFlags::GUARD) {
            return Err(MapError::InvalidFlags);
        }
        if self.find_index(virt).is_some() {
            return Err(MapError::AlreadyMapped);
        }
        if self.count >= MAX_MAPPINGS {
            return Err(MapError::TableFull);
        }
        let mapping = Mapping {
            virt_base: virt,
            phys_base: phys,
            flags,
        };
        // Sorted insert at the first entry with a larger base.
        let mut idx = 0;
        while idx < self.count {
            if let Some(e) = self.entries[idx] {
                if e.virt_base > virt {
                    break;
                }
            }
            idx += 1;
        }
        for k in (idx..self.count).rev() {
            self.entries[k + 1] = self.entries[k];
        }
        self.entries[idx] = Some(mapping);
        self.count += 1;
        Ok(())
    }

    /// Remove the mapping with `virt_base == virt`.
    ///
    /// `virt` must be the page-aligned base (intra-page offsets fail with
    /// `Misaligned` — this is unmap-of-exact-page, not unmap-of-containing-page);
    /// unmapped pages fail with `NotMapped`.
    pub fn unmap(&mut self, virt: u64) -> Result<(), MapError> {
        if virt & PAGE_OFFSET_MASK != 0 {
            return Err(MapError::Misaligned);
        }
        let Some(idx) = self.find_index(virt) else {
            return Err(MapError::NotMapped);
        };
        for k in idx..self.count - 1 {
            self.entries[k] = self.entries[k + 1];
        }
        self.entries[self.count - 1] = None;
        self.count -= 1;
        Ok(())
    }

    /// Translate `virt` to `(phys, flags)`; `None` when unmapped.
    ///
    /// Raw unchecked translation — does NOT enforce permissions, including
    /// `GUARD` (a guard page still returns `Some`; bit 9 is OS-available and
    /// ignored by hardware). Use [`PageTable::check`] as the access gate;
    /// the hardware-PTE slice must mask bit 9 before loading a real PTE
    /// (see [`pte_encode`]). Kernel code outside tests must go through
    /// `check`; today `translate` has no non-test kernel callers (verified
    /// by searching for `translate(` under `crates/`), so the bypass is
    /// documentation-only until the guest walker lands.
    /// The low 12 bits pass through (`phys_base + offset`).
    #[must_use]
    pub fn translate(&self, virt: u64) -> Option<(u64, PageFlags)> {
        let base = virt & !PAGE_OFFSET_MASK;
        let offset = virt & PAGE_OFFSET_MASK;
        for slot in self.entries[..self.count].iter().flatten() {
            if slot.virt_base == base {
                return Some((slot.phys_base + offset, slot.flags));
            }
        }
        None
    }

    /// Permission check mirroring hardware fault priority: `Guard` first,
    /// then `NotPresent`, then `UserViolation`, then `ReadOnly` / `NoExec`.
    /// Returns the translated physical address on success.
    ///
    /// Priority is a software contract for the guest fault stub — hardware
    /// `#PF` sets U/S + R/W + I/D bits together with no order. `GUARD`-only
    /// (no `PRESENT`) intentionally reports `Guard`, not `NotPresent`. The
    /// `!PRESENT -> NotPresent` branch is unreachable under today's `map()`
    /// invariant (`!PRESENT` implies `GUARD`, already returned) and is
    /// retained for hardware-PTE parity where cleared-`PRESENT` entries are
    /// loadable.
    pub fn check(&self, virt: u64, kind: AccessKind, is_user: bool) -> Result<u64, Fault> {
        let base = virt & !PAGE_OFFSET_MASK;
        let offset = virt & PAGE_OFFSET_MASK;
        let mut found: Option<Mapping> = None;
        for slot in self.entries[..self.count].iter().flatten() {
            if slot.virt_base == base {
                found = Some(*slot);
                break;
            }
        }
        let m = found.ok_or(Fault::NotPresent)?;
        if m.flags.contains(PageFlags::GUARD) {
            return Err(Fault::Guard);
        }
        if !m.flags.contains(PageFlags::PRESENT) {
            return Err(Fault::NotPresent);
        }
        if is_user && !m.flags.contains(PageFlags::USER) {
            return Err(Fault::UserViolation);
        }
        match kind {
            AccessKind::Read => Ok(m.phys_base + offset),
            AccessKind::Write if !m.flags.contains(PageFlags::WRITABLE) => Err(Fault::ReadOnly),
            AccessKind::Write => Ok(m.phys_base + offset),
            AccessKind::Execute if m.flags.contains(PageFlags::NO_EXEC) => Err(Fault::NoExec),
            AccessKind::Execute => Ok(m.phys_base + offset),
        }
    }

    /// Index of the entry with `virt_base == virt`, if any.
    fn find_index(&self, virt: u64) -> Option<usize> {
        for (i, slot) in self.entries[..self.count].iter().enumerate() {
            if let Some(e) = slot {
                if e.virt_base == virt {
                    return Some(i);
                }
            }
        }
        None
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Physical-address field of a 4 KiB PTE / `CR3`: bits 12..=51.
pub const PTE_ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;
/// Page-size bit in a paging-structure entry (bit 7): when set at PDPT
/// (level 2) the entry maps a 1 GiB page, at PD (level 1) a 2 MiB page.
/// Set at PML4 (level 3) or PT (level 0) it is reserved and fails closed.
pub const PTE_PS_BIT: u64 = 1 << 7;
/// Offset mask within a 1 GiB large page (low 30 bits of `virt`).
pub const LARGE_1G_OFFSET_MASK: u64 = (1 << 30) - 1;
/// Offset mask within a 2 MiB large page (low 21 bits of `virt`).
pub const LARGE_2M_OFFSET_MASK: u64 = (1 << 21) - 1;
/// Hardware PTE flag bits this slice loads: PRESENT(0) / WRITABLE(1) /
/// USER(2) / NO_EXEC(63). `GUARD` (bit 9, OS-available) is never loaded.
pub const PTE_HW_FLAGS_MASK: u64 = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 63);
/// Trusted software flag bits: HW loadable bits plus `GUARD` (bit 9).
/// [`PageFlags::from_bits`] masks to exactly this set so callers cannot
/// forge `Accessed`/`Dirty` (bits 5/6), other status bits, or reserved bits.
pub const TRUSTED_FLAGS_MASK: u64 = PTE_HW_FLAGS_MASK | (1 << 9);

/// Encode a hardware PTE from a 4 KiB-aligned `phys` base and `flags`.
///
/// Maps `PRESENT -> bit0`, `WRITABLE -> bit1`, `USER -> bit2`,
/// `NO_EXEC -> bit63`. `GUARD` (bit 9) is always masked to 0: hardware
/// ignores bit 9, and a leaked 1 would survive into the real entry where a
/// future reader could mistake OS state for a hardware bit. `Accessed` /
/// `Dirty` (bits 5/6) load as 0; hardware sets them. Low 12 `phys` bits are
/// masked off (caller must pass an aligned base; `map` already enforces it;
/// the masking is load-compat, not a silent-accept contract; use
/// [`pte_encode_checked`] for a fail-closed caller).
#[must_use]
pub const fn pte_encode(phys_base: u64, flags: PageFlags) -> u64 {
    let mut pte = phys_base & PTE_ADDR_MASK;
    let bits = flags.bits();
    if bits & (1 << 0) != 0 {
        pte |= 1 << 0;
    }
    if bits & (1 << 1) != 0 {
        pte |= 1 << 1;
    }
    if bits & (1 << 2) != 0 {
        pte |= 1 << 2;
    }
    if bits & (1 << 63) != 0 {
        pte |= 1 << 63;
    }
    pte
}

/// Physical base decoded from a hardware PTE (low 12 bits cleared).
#[must_use]
pub const fn pte_decoded_phys(pte: u64) -> u64 {
    pte & PTE_ADDR_MASK
}

/// Fail-closed PTE encode (REVIEW #122 finding 2): `None` when `phys_base`
/// is not 4 KiB aligned, mirroring [`cr3_encode`]; `Some(pte)` otherwise.
/// Use this from the guest walker; [`pte_encode`] keeps its masking form
/// for load-compat and its test pins the truncation.
#[must_use]
pub const fn pte_encode_checked(phys_base: u64, flags: PageFlags) -> Option<u64> {
    if phys_base & PAGE_OFFSET_MASK != 0 {
        return None;
    }
    Some(pte_encode(phys_base, flags))
}

/// Permission flags decoded from a hardware PTE. `GUARD` is never
/// reconstructed: it is OS state, masked out at encode time.
#[must_use]
pub const fn pte_decoded_flags(pte: u64) -> PageFlags {
    let mut bits: u64 = 0;
    if pte & (1 << 0) != 0 {
        bits |= 1 << 0;
    }
    if pte & (1 << 1) != 0 {
        bits |= 1 << 1;
    }
    if pte & (1 << 2) != 0 {
        bits |= 1 << 2;
    }
    if pte & (1 << 63) != 0 {
        bits |= 1 << 63;
    }
    PageFlags::from_bits(bits)
}

/// 9-bit page-table index for `virt` at `level`: 3 = PML4 (shift 39),
/// 2 = PDPT (30), 1 = PD (21), 0 = PT (12). `None` for `level > 3`.
#[must_use]
pub const fn page_table_index(virt: u64, level: u8) -> Option<u16> {
    let shift = match level {
        3 => 39,
        2 => 30,
        1 => 21,
        0 => 12,
        _ => return None,
    };
    Some(((virt >> shift) & 0x1FF) as u16)
}

/// Encode a `CR3` value from a 4 KiB-aligned PML4 base. `None` when
/// misaligned. PCID / PWT / PCD low-12 controls are out of scope this slice
/// (load as 0).
#[must_use]
pub const fn cr3_encode(pml4_base: u64) -> Option<u64> {
    if pml4_base & PAGE_OFFSET_MASK != 0 {
        return None;
    }
    Some(pml4_base & PTE_ADDR_MASK)
}

/// Decode the PML4 base from a `CR3` value (low 12 bits cleared).
#[must_use]
pub const fn cr3_decode(cr3: u64) -> u64 {
    cr3 & PTE_ADDR_MASK
}

/// True when `virt` is canonical: bits 48..=63 sign-extend bit 47
/// (Intel SDM Vol. 3: non-canonical linear addresses raise `#GP(0)`,
/// not `#PF`). `const`-compatible so the guest walker can gate on it
/// before touching any table.
#[must_use]
pub const fn is_canonical(virt: u64) -> bool {
    if (virt >> 47) & 1 == 0 {
        virt >> 48 == 0
    } else {
        virt >> 48 == 0xFFFF
    }
}

/// CPU fault classification for a linear-address access: `#GP` for
/// non-canonical addresses, `#PF` kinds otherwise. Mirrors hardware
/// priority (canonical check first, then the [`PageTable::check`] order)
/// so the guest fault stub can assert one line per probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuFault {
    /// Non-canonical address (`#GP(0)`; error code 0).
    GeneralProtection,
    /// Canonical-address page fault (`#PF` with U/S+R/W+I/D bits).
    PageFault(Fault),
}

/// Classify one access: `Err(CpuFault::GeneralProtection)` when `virt`
/// is non-canonical, else the [`PageTable::check`] verdict mapped into
/// `CpuFault::PageFault`. Pure/host-testable; the guest stub calls this
/// shape before asserting its fault-line output.
pub fn classify_access(
    pt: &PageTable,
    virt: u64,
    kind: AccessKind,
    is_user: bool,
) -> Result<u64, CpuFault> {
    if !is_canonical(virt) {
        return Err(CpuFault::GeneralProtection);
    }
    pt.check(virt, kind, is_user).map_err(CpuFault::PageFault)
}

/// Hardware-walker failure modes (fail-closed: no table mutated, no
/// TLB touched). `MisalignedPhys` is the [`pte_encode_checked`] `None`
/// case surfaced as a `Result`; the walker path must use
/// [`encode_table_entry`] (checked), never unchecked [`pte_encode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkerError {
    /// `phys_base` not 4 KiB aligned (checked-encode rejection).
    MisalignedPhys,
    /// `virt` is non-canonical (hardware raises `#GP`, not `#PF`).
    NonCanonical,
    /// Table entry not present at this level.
    NotPresent,
    /// Next-table lookup missed: `lookup(base)` returned `None`
    /// (unmapped table base; the guest `phys_to_table_ptr` maps this to
    /// out-of-bounds / OOM). Distinct from cleared-`PRESENT` (`NotPresent`)
    /// so e2e fault asserts can tell a missing table from a present-but-
    /// empty entry (REVIEW #131 risk 1).
    MissingTable,
    /// `level > 3` (no such paging level).
    InvalidLevel,
}

/// Walker-only PTE encode: fail-closed wrapper over [`pte_encode_checked`].
///
/// This is the single entry point the guest walker uses to build table
/// entries, closing REVIEW #124 risk 1 (checked form with zero callers).
/// Unchecked [`pte_encode`] stays for load-compat only (its truncation is
/// pinned by test); new walker code must call this, verifiable by
/// `grep walker.*pte_encode_checked` / no `pte_encode(` in walker code.
pub const fn encode_table_entry(phys_base: u64, flags: PageFlags) -> Result<u64, WalkerError> {
    match pte_encode_checked(phys_base, flags) {
        Some(pte) => Ok(pte),
        None => Err(WalkerError::MisalignedPhys),
    }
}

/// One hardware walk step: read the `level` table entry for `virt`.
///
/// 4 KiB-only step: `PS` (bit 7) set on the entry is NOT followed as a
/// next-table pointer — it fails closed with `NotPresent`. Large-page
/// leaves (1 GiB at level 2, 2 MiB at level 1) are resolved only by
/// [`walk_full`], which reads the entry directly and stops at the leaf.
/// This pins the deliberate divergence: chaining `walk_step` over firmware
/// tables that contain `PS` leaves mis-walks silently without this gate,
/// so `walk_step` refuses instead. `PS` at level 3/0 is likewise
/// `NotPresent` (reserved), matching `walk_full`.
///
/// Validates canonical first (`NonCanonical`), then the level
/// (`InvalidLevel`), then `PRESENT` (`NotPresent`), then `PS`
/// (`NotPresent`); returns the next-level table base (`pte_decoded_phys`)
/// on success. Full 4-level walks chain four calls (PML4 -> PDPT -> PD ->
/// PT); the leaf PT step returns the 4 KiB page base and callers add
/// `virt & PAGE_OFFSET_MASK`.
/// `GUARD` never appears here: entries were built by
/// [`encode_table_entry`], which masks bit 9 to 0.
///
/// Reserved / MBZ-bit validation is out of scope this slice: only `PRESENT`
/// is checked (matches the S04 software-model parity goal). A future
/// guest-hardening slice may reject reserved bits (e.g. low-12 `CR3`
/// controls, `NX`-disabled bit 63, large-page PAT/MBZ combos) once the
/// guest `CR3` mode (PAE/NX/1 GiB pages) is pinned.
///
/// Next-level lookup contract (for the guest-`CR3` slice): the returned
/// base is a *physical* address. The guest maps it to the next
/// `&[u64; 512]` via its identity/offset mapping of physical memory
/// (`phys_to_table_ptr`, unsafe, guest-only) and calls `walk_step` again;
/// see the `walk_chain_four_levels_*` test for the host-side chaining
/// shape with plain arrays. [`walk_full`] below locks that chaining shape
/// behind a lookup closure so the guest `CR3` slice reuses it directly.
pub fn walk_step(table: &[u64; 512], virt: u64, level: u8) -> Result<u64, WalkerError> {
    if !is_canonical(virt) {
        return Err(WalkerError::NonCanonical);
    }
    let Some(idx) = page_table_index(virt, level) else {
        return Err(WalkerError::InvalidLevel);
    };
    let pte = table[idx as usize];
    if pte & 1 == 0 {
        return Err(WalkerError::NotPresent);
    }
    if pte & PTE_PS_BIT != 0 {
        // 4 KiB-only gate: never follow a PS leaf as a table pointer.
        return Err(WalkerError::NotPresent);
    }
    Ok(pte_decoded_phys(pte))
}

/// Guest `phys_to_table_ptr` bounds gate (S04 slice 13): true only when
/// `phys` is a 4 KiB-aligned, non-zero base whose whole 4 KiB table frame
/// lies inside one [`BootInfo`] region (any kind — firmware page tables may
/// live in reserved/firmware regions, not just usable ones).
///
/// Pure / `no_std`-clean so both host tests and the guest lookup share it:
/// the guest maps `false` to `None` -> [`WalkerError::MissingTable`]
/// (out-of-bounds / OOM), distinct from cleared-`PRESENT`
/// ([`WalkerError::NotPresent`]). `memory_map()` is empty on an invalid
/// header, so a corrupt handoff fails closed here too.
#[must_use]
pub fn table_base_in_boot_map(info: &BootInfo, phys: u64) -> bool {
    if phys == 0 || phys & PAGE_OFFSET_MASK != 0 {
        return false;
    }
    let Some(end) = phys.checked_add(PAGE_SIZE) else {
        return false;
    };
    info.memory_map()
        .iter()
        .any(|r| r.page_count > 0 && phys >= r.start && end <= r.end())
}

/// Full 4-level walk from a `CR3` base through a next-table lookup.
///
/// `lookup(phys_base)` maps a *physical* table base (as returned by the
/// previous [`walk_step`]) to the next `&[u64; 512]` — the host passes a
/// closure over plain arrays, the guest passes its identity/offset-mapped
/// `phys_to_table_ptr` (unsafe, guest-only). `cr3_base` is the PML4 table
/// base (low 12 bits must be clear, mirroring [`cr3_encode`]); the initial
/// PML4 itself is resolved through `lookup` so both host and guest share
/// one chaining shape.
///
/// Fail-closed: misaligned `cr3_base` -> `MisalignedPhys`, non-canonical
/// `virt` -> `NonCanonical` (via the first step), a lookup miss
/// (unmapped table base) -> `MissingTable`, cleared-`PRESENT` mid-level ->
/// `NotPresent`. Returns the 4 KiB page base plus `virt & PAGE_OFFSET_MASK`,
/// or the large-page base plus the in-page offset when a `PS` leaf is hit
/// (1 GiB at level 2, 2 MiB at level 1; `PS` at level 3/0 is reserved ->
/// `NotPresent`).
///
/// Large-page (`PS`, bit 7) policy (S04 slice 10): firmware (OVMF) identity
/// maps RAM with 1 GiB / 2 MiB pages, so a guest `CR3` walk must stop at a
/// `PS` leaf instead of treating it as a table pointer. Other reserved /
/// MBZ bits stay out of scope (only `PRESENT` + `PS`-at-level are checked;
/// `NX`-disabled bit-63 and PAT/MBZ combos ride with the guest `CR3` mode
/// pin, as before).
///
/// `lookup` is generic (`F: Fn(u64) -> Option<&[u64; 512]>`) so the guest
/// fault path inlines instead of dynamically dispatching per level.
///
/// `lookup` is taken by value: pass a `Copy`/cheap callable (a `fn` pointer
/// or a non-capturing closure, both `Copy`; existing host tests move a
/// non-capturing closure). The guest `CR3` slice reuses this shape with its
/// `phys_to_table_ptr` fn pointer, keeping the per-level call a direct
/// inlined call with no dynamic dispatch (REVIEW #133 caveat 2).
pub fn walk_full<'a, F>(cr3_base: u64, virt: u64, lookup: F) -> Result<u64, WalkerError>
where
    F: Fn(u64) -> Option<&'a [u64; 512]>,
{
    if cr3_base & PAGE_OFFSET_MASK != 0 {
        return Err(WalkerError::MisalignedPhys);
    }
    let mut base = cr3_base;
    for level in [3u8, 2, 1, 0] {
        let table = lookup(base).ok_or(WalkerError::MissingTable)?;
        // Read the entry directly so a `PS` large-page leaf stops the walk
        // instead of being misread as a next-table pointer. Order matches
        // `walk_step`: canonical -> level -> PRESENT -> PS-at-level.
        if !is_canonical(virt) {
            return Err(WalkerError::NonCanonical);
        }
        let Some(idx) = page_table_index(virt, level) else {
            return Err(WalkerError::InvalidLevel);
        };
        let pte = table[idx as usize];
        if pte & 1 == 0 {
            return Err(WalkerError::NotPresent);
        }
        if pte & PTE_PS_BIT != 0 {
            match level {
                2 => {
                    let leaf = pte & PTE_ADDR_MASK & !LARGE_1G_OFFSET_MASK;
                    return Ok(leaf + (virt & LARGE_1G_OFFSET_MASK));
                }
                1 => {
                    let leaf = pte & PTE_ADDR_MASK & !LARGE_2M_OFFSET_MASK;
                    return Ok(leaf + (virt & LARGE_2M_OFFSET_MASK));
                }
                // PML4 (3) / PT (0) never carry PS: reserved -> fail closed.
                _ => return Err(WalkerError::NotPresent),
            }
        }
        base = pte_decoded_phys(pte);
    }
    Ok(base + (virt & PAGE_OFFSET_MASK))
}

/// Full 4-level walk returning the leaf permission flags alongside the
/// physical address: `Ok((phys, flags))` where `flags` is
/// [`pte_decoded_flags`] of the leaf entry (4 KiB PTE at level 0, or the
/// `PS` leaf at level 2/1 for 1 GiB / 2 MiB pages). Mirrors [`walk_full`]
/// level-for-level (misaligned `CR3` -> `MisalignedPhys`, non-canonical ->
/// `NonCanonical`, lookup miss -> `MissingTable`, cleared-`PRESENT` ->
/// `NotPresent`, `PS` at 3/0 -> `NotPresent`) so the guest live-flag audit
/// reads exactly the bits the address walk used — no second walk, no drift.
///
/// `GUARD` never appears: firmware entries were not built by
/// [`encode_table_entry`] and bit 9 is OS-available, but the decoder only
/// reconstructs `PRESENT/WRITABLE/USER/NO_EXEC` (see [`pte_decoded_flags`]).
/// Reserved / MBZ bits stay out of scope like [`walk_full`] (#139 note 1).
pub fn walk_full_leaf_flags<'a, F>(
    cr3_base: u64,
    virt: u64,
    lookup: F,
) -> Result<(u64, PageFlags), WalkerError>
where
    F: Fn(u64) -> Option<&'a [u64; 512]>,
{
    if cr3_base & PAGE_OFFSET_MASK != 0 {
        return Err(WalkerError::MisalignedPhys);
    }
    let mut base = cr3_base;
    for level in [3u8, 2, 1, 0] {
        let table = lookup(base).ok_or(WalkerError::MissingTable)?;
        if !is_canonical(virt) {
            return Err(WalkerError::NonCanonical);
        }
        let Some(idx) = page_table_index(virt, level) else {
            return Err(WalkerError::InvalidLevel);
        };
        let pte = table[idx as usize];
        if pte & 1 == 0 {
            return Err(WalkerError::NotPresent);
        }
        if pte & PTE_PS_BIT != 0 {
            match level {
                2 => {
                    let leaf = pte & PTE_ADDR_MASK & !LARGE_1G_OFFSET_MASK;
                    return Ok((leaf + (virt & LARGE_1G_OFFSET_MASK), pte_decoded_flags(pte)));
                }
                1 => {
                    let leaf = pte & PTE_ADDR_MASK & !LARGE_2M_OFFSET_MASK;
                    return Ok((leaf + (virt & LARGE_2M_OFFSET_MASK), pte_decoded_flags(pte)));
                }
                _ => return Err(WalkerError::NotPresent),
            }
        }
        if level == 0 {
            return Ok((
                pte_decoded_phys(pte) + (virt & PAGE_OFFSET_MASK),
                pte_decoded_flags(pte),
            ));
        }
        base = pte_decoded_phys(pte);
    }
    // Unreachable: the level-0 arm above always returns.
    Err(WalkerError::InvalidLevel)
}

/// Synthetic permission-fault self-test for the guest `PERM` e2e line.
///
/// Builds a fixed 5-entry [`PageTable`] (kernel RO at `0x1000`, kernel RW
/// at `0x2000`, user RW at `0x3000`, user NX at `0x4000`, guard at
/// `0x5000`) and runs 9 [`classify_access`] probes covering the S04
/// verification kinds: read/write/execute, user-vs-supervisor, guard,
/// unmapped, and non-canonical. Pure / `no_std`-clean so host tests and
/// the post-EBS guest share it; the guest maps `ok` to `PERM ok=1`
/// (hard-required by `boot-uefi.ps1`). Returns `(ok, passed, total)`.
#[must_use]
pub fn perm_selftest() -> (bool, u32, u32) {
    let mut pt = PageTable::new();
    let k_ro = PageFlags::PRESENT;
    let k_rw = PageFlags::PRESENT.union(PageFlags::WRITABLE);
    let u_rw = k_rw.union(PageFlags::USER);
    let u_nx = PageFlags::PRESENT
        .union(PageFlags::USER)
        .union(PageFlags::NO_EXEC);
    let guard = PageFlags::PRESENT.union(PageFlags::GUARD);
    // Fixed map; `map` cannot fail here (aligned, distinct, PRESENT/GUARD).
    let _ = pt.map(0x1000, 0xA000, k_ro);
    let _ = pt.map(0x2000, 0xB000, k_rw);
    let _ = pt.map(0x3000, 0xC000, u_rw);
    let _ = pt.map(0x4000, 0xD000, u_nx);
    let _ = pt.map(0x5000, 0xE000, guard);
    let cases: [(u64, AccessKind, bool, bool); 9] = [
        // (virt, kind, is_user, expect_ok)
        (0x1000, AccessKind::Read, false, true), // kernel RO supervisor read
        (0x1000, AccessKind::Write, false, false), // kernel RO supervisor write
        (0x2000, AccessKind::Read, true, false), // kernel RW user read
        (0x3000, AccessKind::Write, true, true), // user RW user write
        (0x3000, AccessKind::Write, false, true), // supervisor may access user
        (0x4000, AccessKind::Execute, true, false), // user NX fetch
        (0x5000, AccessKind::Read, false, false), // guard faults
        (0x6000, AccessKind::Read, false, false), // unmapped
        (0x8000_0000_0000, AccessKind::Read, false, false), // non-canonical
    ];
    let mut passed: u32 = 0;
    for (virt, kind, is_user, expect_ok) in cases {
        if classify_access(&pt, virt, kind, is_user).is_ok() == expect_ok {
            passed += 1;
        }
    }
    let total = cases.len() as u32;
    (passed == total, passed, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    const K_RW: PageFlags = PageFlags::PRESENT.union(PageFlags::WRITABLE);

    #[test]
    fn map_translate_unmap_roundtrip_with_offset() {
        let mut pt = PageTable::new();
        assert!(pt.is_empty());
        pt.map(0x1000, 0xA000, K_RW).unwrap();
        assert_eq!(pt.len(), 1);
        // Base and intra-page offset both translate.
        assert_eq!(pt.translate(0x1000), Some((0xA000, K_RW)));
        assert_eq!(pt.translate(0x1ABC), Some((0xAABC, K_RW)));
        // Permission check passes the translated address through.
        assert_eq!(pt.check(0x1ABC, AccessKind::Read, false), Ok(0xAABC));
        assert_eq!(pt.check(0x1ABC, AccessKind::Write, false), Ok(0xAABC));
        pt.unmap(0x1000).unwrap();
        assert!(pt.is_empty());
        assert_eq!(pt.translate(0x1000), None);
        assert_eq!(
            pt.check(0x1000, AccessKind::Read, false),
            Err(Fault::NotPresent)
        );
    }

    #[test]
    fn double_map_misaligned_and_bad_flags_fail_closed() {
        let mut pt = PageTable::new();
        pt.map(0x2000, 0xB000, K_RW).unwrap();
        let before = pt.len();
        assert_eq!(pt.map(0x2000, 0xC000, K_RW), Err(MapError::AlreadyMapped));
        assert_eq!(pt.map(0x2001, 0xC000, K_RW), Err(MapError::Misaligned));
        assert_eq!(pt.map(0x3000, 0xC001, K_RW), Err(MapError::Misaligned));
        assert_eq!(
            pt.map(0x3000, 0xC000, PageFlags::NONE),
            Err(MapError::InvalidFlags)
        );
        assert_eq!(pt.len(), before, "failed maps must not mutate");
        assert_eq!(pt.translate(0x2000), Some((0xB000, K_RW)));
        assert_eq!(pt.unmap(0x9000), Err(MapError::NotMapped));
        assert_eq!(pt.unmap(0x2001), Err(MapError::Misaligned));
    }

    #[test]
    fn table_full_rejects_and_drains() {
        let mut pt = PageTable::new();
        for i in 0..MAX_MAPPINGS as u64 {
            pt.map(0x1000 + i * PAGE_SIZE, 0x100_0000 + i * PAGE_SIZE, K_RW)
                .unwrap();
        }
        assert_eq!(pt.len(), MAX_MAPPINGS);
        assert_eq!(
            pt.map(0xDEAD_0000, 0xBEEF_0000, K_RW),
            Err(MapError::TableFull)
        );
        assert_eq!(pt.len(), MAX_MAPPINGS);
        for i in 0..MAX_MAPPINGS as u64 {
            pt.unmap(0x1000 + i * PAGE_SIZE).unwrap();
        }
        assert!(pt.is_empty());
    }

    #[test]
    fn permission_matrix_enforced() {
        let mut pt = PageTable::new();
        let ro = PageFlags::PRESENT;
        let rw = PageFlags::PRESENT.union(PageFlags::WRITABLE);
        let rw_user = rw.union(PageFlags::USER);
        let ro_user_nx = PageFlags::PRESENT
            .union(PageFlags::USER)
            .union(PageFlags::NO_EXEC);
        pt.map(0x1000, 0xA000, ro).unwrap();
        pt.map(0x2000, 0xB000, rw).unwrap();
        pt.map(0x3000, 0xC000, rw_user).unwrap();
        pt.map(0x4000, 0xD000, ro_user_nx).unwrap();
        // Read-only: read ok, write faults.
        assert!(pt.check(0x1000, AccessKind::Read, false).is_ok());
        assert_eq!(
            pt.check(0x1000, AccessKind::Write, false),
            Err(Fault::ReadOnly)
        );
        // Supervisor-only touched from user faults.
        assert_eq!(
            pt.check(0x2000, AccessKind::Read, true),
            Err(Fault::UserViolation)
        );
        // User page works from both rings (supervisor may access user).
        assert!(pt.check(0x3000, AccessKind::Write, true).is_ok());
        assert!(pt.check(0x3000, AccessKind::Write, false).is_ok());
        // NO_EXEC blocks fetch only.
        assert!(pt.check(0x4000, AccessKind::Read, true).is_ok());
        assert_eq!(
            pt.check(0x4000, AccessKind::Execute, true),
            Err(Fault::NoExec)
        );
        // Executable supervisor page still faults user access first.
        pt.map(0x5000, 0xE000, PageFlags::PRESENT).unwrap();
        assert_eq!(
            pt.check(0x5000, AccessKind::Execute, true),
            Err(Fault::UserViolation)
        );
        assert!(pt.check(0x5000, AccessKind::Execute, false).is_ok());
    }

    #[test]
    fn guard_page_faults_on_every_access() {
        let mut pt = PageTable::new();
        let guard = PageFlags::PRESENT.union(PageFlags::GUARD);
        pt.map(0x8000, 0x9000, guard).unwrap();
        for kind in [AccessKind::Read, AccessKind::Write, AccessKind::Execute] {
            assert_eq!(pt.check(0x8000, kind, false), Err(Fault::Guard));
            assert_eq!(pt.check(0x8000, kind, true), Err(Fault::Guard));
        }
        // Guard occupies the slot: double-map rejects, unmap restores.
        assert_eq!(pt.map(0x8000, 0x9000, K_RW), Err(MapError::AlreadyMapped));
        pt.unmap(0x8000).unwrap();
        assert_eq!(
            pt.check(0x8000, AccessKind::Read, false),
            Err(Fault::NotPresent)
        );
    }

    /// REVIEW #118 edge contract: `translate` is raw (guard still `Some`),
    /// `check` is the gate (`GUARD`-only with no `PRESENT` reports `Guard`),
    /// and `unmap` requires the aligned base (intra-page offset is
    /// `Misaligned`, not unmap-of-containing-page).
    #[test]
    fn review_118_translate_raw_guard_only_unmap_aligned_contract() {
        let mut pt = PageTable::new();
        // GUARD-only (no PRESENT) is mappable and reports Guard, not NotPresent.
        pt.map(0x8000, 0x9000, PageFlags::GUARD).unwrap();
        assert_eq!(pt.check(0x8000, AccessKind::Read, false), Err(Fault::Guard));
        // translate is raw: guard pages still translate; check is the gate.
        let (phys, flags) = pt
            .translate(0x8000)
            .expect("raw translate passes guard through");
        assert_eq!(phys, 0x9000);
        assert!(flags.contains(PageFlags::GUARD));
        // unmap requires the aligned base.
        assert_eq!(pt.unmap(0x8ABC), Err(MapError::Misaligned));
        pt.unmap(0x8000).unwrap();
        assert_eq!(pt.translate(0x8000), None);
    }

    #[test]
    fn pte_encode_masks_guard_bit() {
        // REVIEW #120 risk 2: HW ignores bit 9, so the loader must clear it.
        let guard_rw = PageFlags::PRESENT
            .union(PageFlags::WRITABLE)
            .union(PageFlags::GUARD);
        let pte = pte_encode(0xA000, guard_rw);
        assert_eq!(pte & (1 << 9), 0, "GUARD must never reach hardware");
        assert_eq!(pte, pte_encode(0xA000, K_RW));
        // Only HW flag bits survive alongside the address.
        assert_eq!(pte & !PTE_ADDR_MASK & !PTE_HW_FLAGS_MASK, 0);
        assert_eq!(pte_decoded_flags(pte), K_RW);
    }

    #[test]
    fn pte_roundtrip_preserves_addr_and_perms() {
        let flags = PageFlags::PRESENT
            .union(PageFlags::WRITABLE)
            .union(PageFlags::USER)
            .union(PageFlags::NO_EXEC);
        let pte = pte_encode(0x1234_5000, flags);
        assert_eq!(pte_decoded_phys(pte), 0x1234_5000);
        assert_eq!(pte_decoded_flags(pte), flags);
        // Misaligned low-12 phys bits are masked off, Accessed/Dirty load 0.
        let sloppy = pte_encode(0x1234_5ABC, flags);
        assert_eq!(pte_decoded_phys(sloppy), 0x1234_5000);
        assert_eq!(sloppy & ((1 << 5) | (1 << 6)), 0);
    }

    #[test]
    fn page_index_extracts_four_levels() {
        // Spot-check with hand-verified 9-bit fields.
        let virt: u64 = 0x00FF_8765_4321_ABCD;
        assert_eq!(page_table_index(virt, 3), Some(0x10E));
        assert_eq!(page_table_index(virt, 2), Some(0x195));
        assert_eq!(page_table_index(virt, 1), Some(0x19));
        assert_eq!(page_table_index(virt, 0), Some(0x1A));
        // REVIEW #122 minor: every out-of-range level rejects, not just 4.
        for level in [4u8, 5, 100, 255] {
            assert_eq!(page_table_index(virt, level), None);
        }
    }

    #[test]
    fn review_122_from_bits_masks_untrusted_bits() {
        // Cheapest disconfirm from REVIEW #122 finding 1: raw MAX still
        // carries GUARD (OS state is forgeable by construction) but every
        // untrusted bit is cleared.
        let forged = PageFlags::from_bits(u64::MAX);
        assert!(forged.contains(PageFlags::GUARD));
        assert_eq!(forged.bits() & !TRUSTED_FLAGS_MASK, 0);
        assert_eq!(forged.bits(), TRUSTED_FLAGS_MASK);
        // Accessed/Dirty + a reserved bit never survive.
        let ad = PageFlags::from_bits((1 << 5) | (1 << 6) | (1 << 7));
        assert_eq!(ad.bits(), 0);
        // A raw PTE with bit 9 set still decodes without GUARD.
        let with_guard_bit = pte_encode(0xA000, K_RW) | (1 << 9);
        assert_eq!(pte_decoded_flags(with_guard_bit), K_RW);
    }

    #[test]
    fn review_122_pte_encode_checked_fails_closed_on_misalign() {
        // REVIEW #122 finding 2: checked form mirrors cr3_encode.
        assert_eq!(pte_encode_checked(0x1001, K_RW), None);
        assert_eq!(
            pte_encode_checked(0xA000, K_RW),
            Some(pte_encode(0xA000, K_RW))
        );
        // Unchecked form keeps its pinned truncation (load-compat).
        assert_eq!(pte_decoded_phys(pte_encode(0x1234_5ABC, K_RW)), 0x1234_5000);
    }

    #[test]
    fn cr3_encode_roundtrips_aligned_base() {
        assert_eq!(cr3_encode(0x1000), Some(0x1000));
        assert_eq!(cr3_decode(cr3_encode(0x7F00_1000).unwrap()), 0x7F00_1000);
        assert_eq!(cr3_encode(0x1001), None);
        assert_eq!(cr3_decode(0xFFFF_FFFF_FFFF_FFFF), PTE_ADDR_MASK);
    }

    #[test]
    fn phys_aliasing_is_shared_mapping() {
        // REVIEW #120 risk 3: same phys at two virts is intended sharing.
        let mut pt = PageTable::new();
        pt.map(0x1000, 0xA000, K_RW).unwrap();
        pt.map(0x2000, 0xA000, K_RW).unwrap();
        assert_eq!(pt.translate(0x1000), Some((0xA000, K_RW)));
        assert_eq!(pt.translate(0x2000), Some((0xA000, K_RW)));
    }

    #[test]
    fn walker_entry_wires_checked_form() {
        // REVIEW #124 risk 1: the walker path must call the checked form.
        // encode_table_entry is that path (wraps pte_encode_checked).
        assert_eq!(
            encode_table_entry(0x1001, K_RW),
            Err(WalkerError::MisalignedPhys)
        );
        assert_eq!(
            encode_table_entry(0xA000, K_RW),
            Ok(pte_encode(0xA000, K_RW))
        );
        // GUARD still masked on the walker path.
        let guard_rw = PageFlags::PRESENT
            .union(PageFlags::WRITABLE)
            .union(PageFlags::GUARD);
        let pte = encode_table_entry(0xA000, guard_rw).unwrap();
        assert_eq!(pte & (1 << 9), 0);
        assert_eq!(pte, pte_encode(0xA000, K_RW));
    }

    #[test]
    fn walk_step_reads_indexed_pte() {
        // One present entry at the PML4 index of 0x1000 (index 0).
        let mut table = [0u64; 512];
        table[0] = encode_table_entry(0xB000, K_RW).unwrap();
        assert_eq!(walk_step(&table, 0x1000, 3), Ok(0xB000));
        // Clear PRESENT -> NotPresent.
        table[0] &= !1;
        assert_eq!(walk_step(&table, 0x1000, 3), Err(WalkerError::NotPresent));
        // Bad level rejects before touching the table.
        assert_eq!(walk_step(&table, 0x1000, 4), Err(WalkerError::InvalidLevel));
        // Non-canonical gates first (0x8000_0000_0000 has bit 47 set
        // with zero upper bits).
        assert_eq!(
            walk_step(&table, 0x8000_0000_0000, 3),
            Err(WalkerError::NonCanonical)
        );
        // Canonical-high (sign-extended) passes the gate: index of
        // 0xFFFF_8000_0000_1000 at level 3 is 0x100, wire it present.
        let mut hi = [0u64; 512];
        hi[0x100] = encode_table_entry(0xC000, K_RW).unwrap();
        assert_eq!(walk_step(&hi, 0xFFFF_8000_0000_1000, 3), Ok(0xC000));
    }

    #[test]
    fn walk_step_ps_is_4k_only_fail_closed() {
        // REVIEW #137 risk 1: walk_step no longer follows a PS leaf as a
        // table pointer. walk_full stops at the same leaf (divergence is
        // deliberate and pinned here).
        let virt_2m: u64 = 0x21_3456;
        let mut pd = [0u64; 512];
        pd[page_table_index(virt_2m, 1).unwrap() as usize] =
            encode_table_entry(0x20_0000, K_RW).unwrap() | PTE_PS_BIT;
        assert_eq!(walk_step(&pd, virt_2m, 1), Err(WalkerError::NotPresent));
        // PS + !PRESENT still reports NotPresent (PRESENT checked first).
        pd[page_table_index(virt_2m, 1).unwrap() as usize] &= !1;
        assert_eq!(walk_step(&pd, virt_2m, 1), Err(WalkerError::NotPresent));
        // PS at PT (level 0) is reserved -> NotPresent (was untested).
        // REVIEW #139 note 2: PS@L3 was only covered via walk_full, so pin
        // walk_step fail-closed at L3/L2 here too (reserved/leaf-as-pointer).
        let virt_l3: u64 = 0x4001_2345;
        let mut pml4_ps = [0u64; 512];
        pml4_ps[page_table_index(virt_l3, 3).unwrap() as usize] =
            encode_table_entry(0x2000, K_RW).unwrap() | PTE_PS_BIT;
        assert_eq!(
            walk_step(&pml4_ps, virt_l3, 3),
            Err(WalkerError::NotPresent)
        );
        let mut pdpt_ps = [0u64; 512];
        pdpt_ps[page_table_index(virt_l3, 2).unwrap() as usize] =
            encode_table_entry(0x4000_0000, K_RW).unwrap() | PTE_PS_BIT;
        assert_eq!(
            walk_step(&pdpt_ps, virt_l3, 2),
            Err(WalkerError::NotPresent)
        );
        let virt_4k: u64 = 0x1000;
        let mut pt = [0u64; 512];
        pt[page_table_index(virt_4k, 0).unwrap() as usize] =
            encode_table_entry(0xB000, K_RW).unwrap() | PTE_PS_BIT;
        assert_eq!(walk_step(&pt, virt_4k, 0), Err(WalkerError::NotPresent));
        // Same 2 MiB leaf through walk_full resolves (stops at leaf).
        let mut pdpt = [0u64; 512];
        let mut pml4 = [0u64; 512];
        let mut pd_ok = [0u64; 512];
        pd_ok[page_table_index(virt_2m, 1).unwrap() as usize] =
            encode_table_entry(0x20_0000, K_RW).unwrap() | PTE_PS_BIT;
        pml4[page_table_index(virt_2m, 3).unwrap() as usize] =
            encode_table_entry(0x2000, K_RW).unwrap();
        pdpt[page_table_index(virt_2m, 2).unwrap() as usize] =
            encode_table_entry(0x3000, K_RW).unwrap();
        let lookup = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4),
                0x2000 => Some(&pdpt),
                0x3000 => Some(&pd_ok),
                _ => None,
            }
        };
        assert_eq!(walk_full(0x1000, virt_2m, lookup), Ok(0x21_3456));
    }

    #[test]
    fn classify_canonical_gp_vs_pf() {
        let mut pt = PageTable::new();
        pt.map(0x1000, 0xA000, K_RW).unwrap();
        pt.map(0x8000, 0x9000, PageFlags::PRESENT.union(PageFlags::GUARD))
            .unwrap();
        // Canonical hit passes through.
        assert_eq!(
            classify_access(&pt, 0x1000, AccessKind::Read, false),
            Ok(0xA000)
        );
        // Canonical miss is #PF NotPresent.
        assert_eq!(
            classify_access(&pt, 0x2000, AccessKind::Read, false),
            Err(CpuFault::PageFault(Fault::NotPresent))
        );
        // Guard reports #PF Guard (not NotPresent, not #GP).
        assert_eq!(
            classify_access(&pt, 0x8000, AccessKind::Read, false),
            Err(CpuFault::PageFault(Fault::Guard))
        );
        // Non-canonical is #GP even where nothing is mapped (and would
        // otherwise be NotPresent).
        assert_eq!(
            classify_access(&pt, 0x8000_0000_0000, AccessKind::Read, false),
            Err(CpuFault::GeneralProtection)
        );
        // Boundary pins: last low-canonical, first non-canonical,
        // first high-canonical.
        assert!(is_canonical(0x0000_7FFF_FFFF_FFFF));
        assert!(!is_canonical(0x0000_8000_0000_0000));
        assert!(is_canonical(0xFFFF_8000_0000_0000));
        assert!(!is_canonical(0xFFFF_7FFF_FFFF_FFFF));
    }

    #[test]
    fn walk_chain_four_levels_with_offset_and_mid_notpresent() {
        // Cheapest disconfirm for the guest-CR3 slice (REVIEW #126 risk 2):
        // four chained walk_step calls resolve a 4 KiB page base, and the
        // caller adds the page offset; a cleared mid-level entry fails
        // closed with NotPresent before deeper levels are touched.
        let virt: u64 = 0x0000_1234_5678_9ABC;
        let offset = virt & PAGE_OFFSET_MASK;
        assert_eq!(offset, 0xABC);
        // Distinct table bases per level (all 4 KiB aligned; the PML4
        // itself lives at the CR3 base, so only the three child bases
        // plus the leaf page base are wired as entries).
        let (pdpt_base, pd_base, pt_base, page_base) = (0x2000, 0x3000, 0x4000, 0x5000);
        let mut pml4 = [0u64; 512];
        let mut pdpt = [0u64; 512];
        let mut pd = [0u64; 512];
        let mut pt = [0u64; 512];
        pml4[page_table_index(virt, 3).unwrap() as usize] =
            encode_table_entry(pdpt_base, K_RW).unwrap();
        pdpt[page_table_index(virt, 2).unwrap() as usize] =
            encode_table_entry(pd_base, K_RW).unwrap();
        pd[page_table_index(virt, 1).unwrap() as usize] =
            encode_table_entry(pt_base, K_RW).unwrap();
        pt[page_table_index(virt, 0).unwrap() as usize] =
            encode_table_entry(page_base, K_RW).unwrap();
        // Happy path: chain the decoded base of each level into the next.
        assert_eq!(walk_step(&pml4, virt, 3), Ok(pdpt_base));
        assert_eq!(walk_step(&pdpt, virt, 2), Ok(pd_base));
        assert_eq!(walk_step(&pd, virt, 1), Ok(pt_base));
        assert_eq!(walk_step(&pt, virt, 0), Ok(page_base));
        assert_eq!(walk_step(&pt, virt, 0).unwrap() + offset, 0x5ABC);
        // Mid-level NotPresent: clear the PDPT entry; the PDPT step fails
        // even though PD/PT below are still present.
        pdpt[page_table_index(virt, 2).unwrap() as usize] &= !1;
        assert_eq!(walk_step(&pdpt, virt, 2), Err(WalkerError::NotPresent));
        // PML4 itself still resolves (failure is local to the cleared level).
        assert_eq!(walk_step(&pml4, virt, 3), Ok(pdpt_base));
    }

    #[test]
    fn kernel_vs_user_boundary_enforced() {
        // Kernel-vs-user split the guest-CR3 slice will load: supervisor
        // page (no USER) vs user page (USER), both rings, all access kinds.
        let mut pt = PageTable::new();
        let k_rw = PageFlags::PRESENT.union(PageFlags::WRITABLE);
        let u_rw = k_rw.union(PageFlags::USER);
        pt.map(0x1000, 0xA000, k_rw).unwrap();
        pt.map(0x2000, 0xB000, u_rw).unwrap();
        // Supervisor page: supervisor full access, user fully denied.
        assert_eq!(pt.check(0x1000, AccessKind::Read, false), Ok(0xA000));
        assert_eq!(pt.check(0x1000, AccessKind::Write, false), Ok(0xA000));
        assert_eq!(
            pt.check(0x1000, AccessKind::Read, true),
            Err(Fault::UserViolation)
        );
        assert_eq!(
            pt.check(0x1000, AccessKind::Write, true),
            Err(Fault::UserViolation)
        );
        assert_eq!(
            pt.check(0x1000, AccessKind::Execute, true),
            Err(Fault::UserViolation)
        );
        // User page: both rings pass (supervisor may access user).
        assert_eq!(pt.check(0x2000, AccessKind::Read, true), Ok(0xB000));
        assert_eq!(pt.check(0x2000, AccessKind::Write, true), Ok(0xB000));
        assert_eq!(pt.check(0x2000, AccessKind::Read, false), Ok(0xB000));
        assert_eq!(pt.check(0x2000, AccessKind::Write, false), Ok(0xB000));
        // Same split through the canonical gate: user ring on the kernel
        // page is #PF UserViolation, non-canonical stays #GP either way.
        assert_eq!(
            classify_access(&pt, 0x1000, AccessKind::Read, true),
            Err(CpuFault::PageFault(Fault::UserViolation))
        );
        assert_eq!(
            classify_access(&pt, 0x2000, AccessKind::Read, true),
            Ok(0xB000)
        );
        assert_eq!(
            classify_access(&pt, 0x8000_0000_0000, AccessKind::Read, true),
            Err(CpuFault::GeneralProtection)
        );
    }

    fn walk_four_levels_fixture(
        virt: u64,
    ) -> ([u64; 512], [u64; 512], [u64; 512], [u64; 512], u64) {
        let (pdpt_base, pd_base, pt_base, page_base) = (0x2000, 0x3000, 0x4000, 0x5000);
        let mut pml4 = [0u64; 512];
        let mut pdpt = [0u64; 512];
        let mut pd = [0u64; 512];
        let mut pt = [0u64; 512];
        pml4[page_table_index(virt, 3).unwrap() as usize] =
            encode_table_entry(pdpt_base, K_RW).unwrap();
        pdpt[page_table_index(virt, 2).unwrap() as usize] =
            encode_table_entry(pd_base, K_RW).unwrap();
        pd[page_table_index(virt, 1).unwrap() as usize] =
            encode_table_entry(pt_base, K_RW).unwrap();
        pt[page_table_index(virt, 0).unwrap() as usize] =
            encode_table_entry(page_base, K_RW).unwrap();
        (pml4, pdpt, pd, pt, page_base)
    }

    #[test]
    fn walk_full_happy_path_adds_offset() {
        // Shared walk_full helper the guest CR3 slice reuses: the lookup
        // closure plays the role of guest phys_to_table_ptr.
        let virt: u64 = 0x0000_1234_5678_9ABC;
        let pml4_base = 0x1000;
        let (pml4, pdpt, pd, pt, _) = walk_four_levels_fixture(virt);
        let lookup = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4),
                0x2000 => Some(&pdpt),
                0x3000 => Some(&pd),
                0x4000 => Some(&pt),
                _ => None,
            }
        };
        assert_eq!(walk_full(pml4_base, virt, lookup), Ok(0x5ABC));
    }

    #[test]
    fn walk_full_mid_level_notpresent_propagates() {
        let virt: u64 = 0x0000_1234_5678_9ABC;
        let pml4_base = 0x1000;
        let (pml4, mut pdpt, pd, pt, _) = walk_four_levels_fixture(virt);
        pdpt[page_table_index(virt, 2).unwrap() as usize] &= !1;
        let lookup = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4),
                0x2000 => Some(&pdpt),
                0x3000 => Some(&pd),
                0x4000 => Some(&pt),
                _ => None,
            }
        };
        assert_eq!(
            walk_full(pml4_base, virt, lookup),
            Err(WalkerError::NotPresent)
        );
    }

    #[test]
    fn walk_full_missing_table_base_is_missing_table() {
        // Lookup miss (unmapped table base) is MissingTable, distinct from
        // cleared-PRESENT NotPresent: the guest phys_to_table_ptr maps this
        // to out-of-bounds / OOM (REVIEW #131 risk 1, closes #130 note 3).
        let virt: u64 = 0x0000_1234_5678_9ABC;
        let pml4_base = 0x1000;
        let (pml4, _, _, _, _) = walk_four_levels_fixture(virt);
        let missing = |base: u64| -> Option<&[u64; 512]> {
            if base == pml4_base {
                Some(&pml4)
            } else {
                None
            }
        };
        assert_eq!(
            walk_full(pml4_base, virt, missing),
            Err(WalkerError::MissingTable)
        );
        // And the distinction is load-bearing: cleared-PRESENT stays
        // NotPresent (see walk_full_mid_level_notpresent_propagates).
    }

    #[test]
    fn walk_full_misaligned_cr3_fails_closed() {
        let virt: u64 = 0x0000_1234_5678_9ABC;
        let (pml4, pdpt, pd, pt, _) = walk_four_levels_fixture(virt);
        let lookup = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4),
                0x2000 => Some(&pdpt),
                0x3000 => Some(&pd),
                0x4000 => Some(&pt),
                _ => None,
            }
        };
        assert_eq!(
            walk_full(0x1001, virt, lookup),
            Err(WalkerError::MisalignedPhys)
        );
    }

    #[test]
    fn walk_full_noncanonical_fails_closed() {
        let virt: u64 = 0x0000_1234_5678_9ABC;
        let pml4_base = 0x1000;
        let (pml4, pdpt, pd, pt, _) = walk_four_levels_fixture(virt);
        let lookup = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4),
                0x2000 => Some(&pdpt),
                0x3000 => Some(&pd),
                0x4000 => Some(&pt),
                _ => None,
            }
        };
        assert_eq!(
            walk_full(pml4_base, 0x8000_0000_0000, lookup),
            Err(WalkerError::NonCanonical)
        );
    }

    #[test]
    fn walk_full_large_pages_stop_at_ps_leaf() {
        // OVMF identity-maps RAM with 1 GiB / 2 MiB PS leaves: walk_full
        // must stop there instead of following the leaf as a table base.
        // virt 0x4001_2345: PML4 idx 0, PDPT idx 1 (1 GiB leaf at
        // 0x4000_0000), low-30 offset 0x12_345.
        let virt_1g: u64 = 0x4001_2345;
        let pml4_base = 0x1000;
        let mut pml4 = [0u64; 512];
        let mut pdpt = [0u64; 512];
        pml4[page_table_index(virt_1g, 3).unwrap() as usize] =
            encode_table_entry(0x2000, K_RW).unwrap();
        pdpt[page_table_index(virt_1g, 2).unwrap() as usize] =
            encode_table_entry(0x4000_0000, K_RW).unwrap() | PTE_PS_BIT;
        let lookup_1g = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4),
                0x2000 => Some(&pdpt),
                _ => None,
            }
        };
        // Leaf base 0x4000_0000 + offset (virt & 30-bit mask) = 0x4001_2345.
        assert_eq!(walk_full(pml4_base, virt_1g, lookup_1g), Ok(0x4001_2345));

        // 2 MiB leaf at PD (level 1): virt 0x21_3456 -> PD entry with PS,
        // leaf base 0x200_000 + low-21 offset 0x1_3456 = 0x213_456.
        let virt_2m: u64 = 0x21_3456;
        let mut pd = [0u64; 512];
        let mut pdpt2 = [0u64; 512];
        let mut pml4_2 = [0u64; 512];
        pml4_2[page_table_index(virt_2m, 3).unwrap() as usize] =
            encode_table_entry(0x2000, K_RW).unwrap();
        pdpt2[page_table_index(virt_2m, 2).unwrap() as usize] =
            encode_table_entry(0x3000, K_RW).unwrap();
        pd[page_table_index(virt_2m, 1).unwrap() as usize] =
            encode_table_entry(0x20_0000, K_RW).unwrap() | PTE_PS_BIT;
        let lookup_2m = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4_2),
                0x2000 => Some(&pdpt2),
                0x3000 => Some(&pd),
                _ => None,
            }
        };
        assert_eq!(walk_full(pml4_base, virt_2m, lookup_2m), Ok(0x21_3456));

        // PS at PML4 (3) / PT (0) is reserved -> NotPresent, not a leaf.
        let mut pml4_bad = [0u64; 512];
        pml4_bad[page_table_index(virt_1g, 3).unwrap() as usize] =
            encode_table_entry(0x2000, K_RW).unwrap() | PTE_PS_BIT;
        let lookup_bad = |base: u64| -> Option<&[u64; 512]> {
            if base == pml4_base {
                Some(&pml4_bad)
            } else {
                None
            }
        };
        assert_eq!(
            walk_full(pml4_base, virt_1g, lookup_bad),
            Err(WalkerError::NotPresent)
        );
    }

    #[test]
    fn table_base_in_boot_map_bounds_gate() {
        use baseos_boot_info::{BootInfo, MemoryKind, MemoryRegion};
        let mut info = BootInfo::new();
        assert!(info.push_region(MemoryRegion::new(0x1000, 4, MemoryKind::Usable)));
        assert!(info.push_region(MemoryRegion::new(0x8000_0000, 256, MemoryKind::Firmware)));
        // Aligned bases fully inside a region pass (any kind).
        assert!(table_base_in_boot_map(&info, 0x1000));
        assert!(table_base_in_boot_map(&info, 0x8000_0000));
        // Zero / misaligned / outside / straddling fail closed.
        assert!(!table_base_in_boot_map(&info, 0));
        assert!(!table_base_in_boot_map(&info, 0x1001));
        assert!(!table_base_in_boot_map(&info, 0x5000));
        assert!(!table_base_in_boot_map(&info, 0x0FFF_0000));
        // Wrap-around addition fails closed instead of landing in-map.
        assert!(!table_base_in_boot_map(&info, u64::MAX - PAGE_OFFSET_MASK));
        // Corrupt handoff (invalid magic) has an empty map -> false.
        let mut bad = BootInfo::new();
        bad.magic = 0xDEAD;
        assert!(!table_base_in_boot_map(&bad, 0x1000));
    }

    #[test]
    fn churn_fill_half_drain_remap_restores() {
        let mut pt = PageTable::new();
        for i in 0..MAX_MAPPINGS as u64 {
            pt.map(0x1000 + i * PAGE_SIZE, 0x200_0000 + i * PAGE_SIZE, K_RW)
                .unwrap();
        }
        // Unmap evens, remap odds-adjacent guard-free, drain the rest.
        for i in (0..MAX_MAPPINGS as u64).step_by(2) {
            pt.unmap(0x1000 + i * PAGE_SIZE).unwrap();
        }
        assert_eq!(pt.len(), MAX_MAPPINGS / 2);
        for i in (0..MAX_MAPPINGS as u64).step_by(2) {
            pt.map(0x1000 + i * PAGE_SIZE, 0x300_0000 + i * PAGE_SIZE, K_RW)
                .unwrap();
        }
        assert_eq!(pt.len(), MAX_MAPPINGS);
        for i in 0..MAX_MAPPINGS as u64 {
            pt.unmap(0x1000 + i * PAGE_SIZE).unwrap();
        }
        assert!(pt.is_empty());
    }

    #[test]
    fn perm_selftest_all_nine_probes_pass() {
        let (ok, passed, total) = perm_selftest();
        assert_eq!((passed, total), (9, 9));
        assert!(ok);
    }

    #[test]
    fn walk_full_leaf_flags_reports_live_user_nx_bits() {
        // 4 KiB leaf preserves USER + NO_EXEC alongside the address.
        let virt: u64 = 0x0000_1234_5678_9ABC;
        let pml4_base = 0x1000;
        let (pml4, pdpt, pd, pt, _) = walk_four_levels_fixture(virt);
        // Fixture leaves are K_RW (PRESENT+WRITABLE, no USER, no NX).
        let lookup = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4),
                0x2000 => Some(&pdpt),
                0x3000 => Some(&pd),
                0x4000 => Some(&pt),
                _ => None,
            }
        };
        let (phys, flags) = walk_full_leaf_flags(pml4_base, virt, lookup).unwrap();
        assert_eq!(phys, walk_full(pml4_base, virt, lookup).unwrap());
        assert!(flags.contains(PageFlags::PRESENT));
        assert!(flags.contains(PageFlags::WRITABLE));
        assert!(!flags.contains(PageFlags::USER));
        assert!(!flags.contains(PageFlags::NO_EXEC));

        // USER + NX leaf decodes through the same path.
        let user_nx = PageFlags::PRESENT
            .union(PageFlags::WRITABLE)
            .union(PageFlags::USER)
            .union(PageFlags::NO_EXEC);
        let mut pt_nx = [0u64; 512];
        let mut pd_nx = [0u64; 512];
        let mut pdpt_nx = [0u64; 512];
        let mut pml4_nx = [0u64; 512];
        pml4_nx[page_table_index(virt, 3).unwrap() as usize] =
            encode_table_entry(0x2000, K_RW).unwrap();
        pdpt_nx[page_table_index(virt, 2).unwrap() as usize] =
            encode_table_entry(0x3000, K_RW).unwrap();
        pd_nx[page_table_index(virt, 1).unwrap() as usize] =
            encode_table_entry(0x4000, K_RW).unwrap();
        pt_nx[page_table_index(virt, 0).unwrap() as usize] =
            encode_table_entry(0x5000, user_nx).unwrap();
        let lookup_nx = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4_nx),
                0x2000 => Some(&pdpt_nx),
                0x3000 => Some(&pd_nx),
                0x4000 => Some(&pt_nx),
                _ => None,
            }
        };
        let (phys_nx, flags_nx) = walk_full_leaf_flags(pml4_base, virt, lookup_nx).unwrap();
        assert_eq!(phys_nx, 0x5000 + (virt & PAGE_OFFSET_MASK));
        assert!(flags_nx.contains(PageFlags::USER));
        assert!(flags_nx.contains(PageFlags::NO_EXEC));

        // 2 MiB PS leaf flags come from the leaf entry itself.
        let virt_2m: u64 = 0x21_3456;
        let mut pd_ps = [0u64; 512];
        let mut pdpt2 = [0u64; 512];
        let mut pml4_2 = [0u64; 512];
        pml4_2[page_table_index(virt_2m, 3).unwrap() as usize] =
            encode_table_entry(0x2000, K_RW).unwrap();
        pdpt2[page_table_index(virt_2m, 2).unwrap() as usize] =
            encode_table_entry(0x3000, K_RW).unwrap();
        pd_ps[page_table_index(virt_2m, 1).unwrap() as usize] =
            encode_table_entry(0x20_0000, user_nx).unwrap() | PTE_PS_BIT;
        let lookup_2m = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4_2),
                0x2000 => Some(&pdpt2),
                0x3000 => Some(&pd_ps),
                _ => None,
            }
        };
        let (phys_2m, flags_2m) = walk_full_leaf_flags(pml4_base, virt_2m, lookup_2m).unwrap();
        assert_eq!(phys_2m, walk_full(pml4_base, virt_2m, lookup_2m).unwrap());
        assert!(flags_2m.contains(PageFlags::USER));
        assert!(flags_2m.contains(PageFlags::NO_EXEC));

        // 1 GiB PS leaf likewise.
        let virt_1g: u64 = 0x4001_2345;
        let mut pml4_1g = [0u64; 512];
        let mut pdpt_1g = [0u64; 512];
        pml4_1g[page_table_index(virt_1g, 3).unwrap() as usize] =
            encode_table_entry(0x2000, K_RW).unwrap();
        pdpt_1g[page_table_index(virt_1g, 2).unwrap() as usize] =
            encode_table_entry(0x4000_0000, K_RW).unwrap() | PTE_PS_BIT;
        let lookup_1g = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4_1g),
                0x2000 => Some(&pdpt_1g),
                _ => None,
            }
        };
        let (phys_1g, flags_1g) = walk_full_leaf_flags(pml4_base, virt_1g, lookup_1g).unwrap();
        assert_eq!(phys_1g, 0x4001_2345);
        assert!(!flags_1g.contains(PageFlags::USER));

        // Error parity with walk_full: missing table, non-canonical,
        // misaligned CR3 propagate identically (reserved-PS + cleared
        // PRESENT parity lives in walk_full_leaf_flags_err_parity_* below).
        let missing = |base: u64| -> Option<&[u64; 512]> {
            if base == pml4_base {
                Some(&pml4)
            } else {
                None
            }
        };
        assert_eq!(
            walk_full_leaf_flags(pml4_base, virt, missing),
            Err(WalkerError::MissingTable)
        );
        assert_eq!(
            walk_full_leaf_flags(pml4_base, 0x8000_0000_0000, lookup),
            Err(WalkerError::NonCanonical)
        );
        assert_eq!(
            walk_full_leaf_flags(0x1001, virt, lookup),
            Err(WalkerError::MisalignedPhys)
        );
    }

    /// REVIEW #147 risk 3: `walk_full` vs `walk_full_leaf_flags` error parity
    /// for the reserved-PS + cleared-PRESENT cases the prior test's comment
    /// claimed but never exercised. Both walkers share the same
    /// canonical -> PRESENT -> PS-at-level order, so every failure maps to
    /// the identical [`WalkerError`]; phys agrees on every success.
    #[test]
    fn walk_full_leaf_flags_err_parity_matches_walk_full() {
        let virt: u64 = 0x0000_1234_5678_9ABC;
        let pml4_base = 0x1000;
        // Cleared-PRESENT mid-level: both fail closed with NotPresent.
        let (pml4c, mut pdptc, pdc, ptc, _) = walk_four_levels_fixture(virt);
        pdptc[page_table_index(virt, 2).unwrap() as usize] &= !1;
        let lookup_c = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4c),
                0x2000 => Some(&pdptc),
                0x3000 => Some(&pdc),
                0x4000 => Some(&ptc),
                _ => None,
            }
        };
        assert_eq!(
            walk_full(pml4_base, virt, lookup_c),
            Err(WalkerError::NotPresent)
        );
        assert_eq!(
            walk_full_leaf_flags(pml4_base, virt, lookup_c),
            Err(WalkerError::NotPresent)
        );
        // Reserved PS at PML4 (level 3): both reject, never a leaf.
        let mut pml4_ps3 = [0u64; 512];
        pml4_ps3[page_table_index(virt, 3).unwrap() as usize] =
            encode_table_entry(0x2000, K_RW).unwrap() | PTE_PS_BIT;
        let lookup_ps3 = |base: u64| -> Option<&[u64; 512]> {
            if base == pml4_base {
                Some(&pml4_ps3)
            } else {
                None
            }
        };
        assert_eq!(
            walk_full(pml4_base, virt, lookup_ps3),
            Err(WalkerError::NotPresent)
        );
        assert_eq!(
            walk_full_leaf_flags(pml4_base, virt, lookup_ps3),
            Err(WalkerError::NotPresent)
        );
        // Reserved PS at PT (level 0): both reject, never a leaf.
        let (pml4z, pdptz, pdz, mut ptz, _) = walk_four_levels_fixture(virt);
        ptz[page_table_index(virt, 0).unwrap() as usize] |= PTE_PS_BIT;
        let lookup_ps0 = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4z),
                0x2000 => Some(&pdptz),
                0x3000 => Some(&pdz),
                0x4000 => Some(&ptz),
                _ => None,
            }
        };
        assert_eq!(
            walk_full(pml4_base, virt, lookup_ps0),
            Err(WalkerError::NotPresent)
        );
        assert_eq!(
            walk_full_leaf_flags(pml4_base, virt, lookup_ps0),
            Err(WalkerError::NotPresent)
        );
        // Success parity: leaf_flags phys equals walk_full phys.
        let (pml4s, pdpts, pds, pts, _) = walk_four_levels_fixture(virt);
        let lookup_s = |base: u64| -> Option<&[u64; 512]> {
            match base {
                0x1000 => Some(&pml4s),
                0x2000 => Some(&pdpts),
                0x3000 => Some(&pds),
                0x4000 => Some(&pts),
                _ => None,
            }
        };
        let (phys_flags, _) = walk_full_leaf_flags(pml4_base, virt, lookup_s).unwrap();
        assert_eq!(phys_flags, walk_full(pml4_base, virt, lookup_s).unwrap());
    }
}
