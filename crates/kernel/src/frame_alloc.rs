//! S03 physical frame allocator over the [`BootInfo`] Usable map.
//!
//! Ingest coalesces every `MemoryKind::Usable` region into a sorted free
//! list; `alloc` hands out frames next-fit from that list and `free`
//! returns them with three fail-closed checks (agent-b #58 matrix):
//! wrap (`base > u64::MAX - len`), outside-the-BootInfo-Usable-map, and
//! double-free (any overlap with still-free memory — adjacent-touching
//! ranges that merely touch coalesce instead, so in-order frees of
//! neighboring regions never false-positive). Fault paths leave the free
//! count, span count, and next-fit cursor untouched so a rejected free or
//! allocation cannot corrupt allocator state.

use baseos_boot_info::{write_str, write_u64_dec, BootInfo, MemoryKind, PAGE_SIZE};

/// Maximum ingested Usable spans after coalescing (never exceeds the
/// handoff's region cap).
const MAX_SPANS: usize = baseos_boot_info::MAX_MEM_REGIONS;

/// Frame-operation failure modes (host-testable; no `EFI_STATUS` here —
/// this is post-handoff kernel logic, not firmware).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// `base + len` wraps past `u64::MAX` (checked before map/free tests).
    Wrap,
    /// Range is not fully covered by the ingested Usable map (includes
    /// holes between Usable spans and addresses past the map).
    OutsideMap,
    /// Range overlaps memory that is already free (double-free, or a free
    /// of a never-allocated in-map range that never left the free list).
    DoubleFree,
}

/// Half-open byte span `[start, end)`; `end >= start` by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    start: u64,
    end: u64,
}

impl Span {
    const fn len(self) -> u64 {
        self.end - self.start
    }

    /// True when `other` lies entirely inside `self` (endpoints inclusive
    /// of the half-open bound: empty `other` at `self.end` counts).
    const fn contains(self, other: Span) -> bool {
        other.start >= self.start && other.end <= self.end
    }

    /// True when the spans share any byte (strict overlap, not mere touch).
    const fn overlaps(self, other: Span) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// True when the spans are disjoint but abut (`self.end == other.start`
    /// or the reverse) — the coalesce-only, never-double-free case.
    const fn touches(self, other: Span) -> bool {
        self.end == other.start || other.end == self.start
    }
}

/// Physical frame allocator: sorted free list over BootInfo Usable spans.
///
/// `map` is the immutable ingested Usable union (outside-map oracle);
/// `free` is the mutable sorted, non-overlapping, non-adjacent free list
/// (coalesced on every successful `free`). `cursor` is a next-fit index
/// into `free` so repeated small allocations sweep the whole list instead
/// of always restarting at span 0.
#[derive(Debug, Clone)]
pub struct FrameAllocator {
    map: [Span; MAX_SPANS],
    map_count: usize,
    free: [Span; MAX_SPANS],
    free_count: usize,
    free_pages: u64,
    cursor: usize,
}

impl FrameAllocator {
    /// Ingest every `MemoryKind::Usable` region of `boot` as free memory.
    ///
    /// Overflowing Usable regions (`page_count * PAGE_SIZE` or
    /// `start + size` wraps) are skipped fail-closed rather than admitted
    /// with a broken span; an invalid header yields an empty allocator.
    /// Spans are sorted and adjacent/overlapping Usable regions coalesce
    /// so the map and free list start as the same disjoint union.
    pub fn from_boot_info(boot: &BootInfo) -> Self {
        let mut alloc = Self {
            map: [Span { start: 0, end: 0 }; MAX_SPANS],
            map_count: 0,
            free: [Span { start: 0, end: 0 }; MAX_SPANS],
            free_count: 0,
            free_pages: 0,
            cursor: 0,
        };
        for region in boot.memory_map() {
            if region.kind != MemoryKind::Usable {
                continue;
            }
            let Some(size) = region.page_count.checked_mul(PAGE_SIZE) else {
                continue;
            };
            if size == 0 {
                continue;
            }
            let Some(end) = region.start.checked_add(size) else {
                continue;
            };
            alloc.map[alloc.map_count] = Span {
                start: region.start,
                end,
            };
            alloc.map_count += 1;
        }
        // Sort Usable spans by start, then coalesce overlapping/touching
        // neighbors into the map union (insertion into a sorted array —
        // map_count <= 128 so O(n^2) is fine at boot).
        sort_spans(&mut alloc.map, alloc.map_count);
        alloc.map_count = coalesce(&mut alloc.map, alloc.map_count);
        // Free list starts as the full map union.
        alloc.free[..alloc.map_count].copy_from_slice(&alloc.map[..alloc.map_count]);
        alloc.free_count = alloc.map_count;
        for span in &alloc.free[..alloc.free_count] {
            alloc.free_pages = alloc.free_pages.saturating_add(span.len() / PAGE_SIZE);
        }
        alloc
    }

    /// Total free frames currently available.
    pub fn free_pages(&self) -> u64 {
        self.free_pages
    }

    /// Number of disjoint free spans (diagnostic; shrinks on coalesce).
    pub fn free_span_count(&self) -> usize {
        self.free_count
    }

    /// Next-fit cursor index into the free list (diagnostic snapshot).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Number of ingested (coalesced) Usable map spans.
    pub fn map_span_count(&self) -> usize {
        self.map_count
    }

    /// Allocate `pages` contiguous free frames; returns the physical base.
    ///
    /// Next-fit from `cursor`: returns `None` on `pages == 0`, a size that
    /// overflows `u64` bytes, `pages > free_pages()`, or when no single
    /// free span is large enough (no multi-span stitching this slice).
    /// A failed allocation never mutates free count, spans, or cursor.
    pub fn alloc(&mut self, pages: u64) -> Option<u64> {
        if pages == 0 {
            return None;
        }
        let size = pages.checked_mul(PAGE_SIZE)?;
        if pages > self.free_pages {
            return None;
        }
        if self.free_count == 0 {
            return None;
        }
        let n = self.free_count;
        for i in 0..n {
            let idx = (self.cursor + i) % n;
            let span = self.free[idx];
            if span.len() < size {
                continue;
            }
            let base = span.start;
            if span.len() == size {
                // Remove idx; shift the tail left; re-clamp cursor.
                for k in idx..self.free_count - 1 {
                    self.free[k] = self.free[k + 1];
                }
                self.free_count -= 1;
                if self.free_count == 0 {
                    self.cursor = 0;
                } else {
                    self.cursor = idx % self.free_count;
                }
            } else {
                // Carve from the front; remainder keeps a sorted position.
                self.free[idx].start = span.start + size;
                self.cursor = idx;
            }
            self.free_pages -= pages;
            return Some(base);
        }
        None
    }

    /// Free `pages` frames at `base` back into the list.
    ///
    /// Checks, in order: wrap ([`FrameError::Wrap`]), coverage by the
    /// ingested Usable map ([`FrameError::OutsideMap`]), overlap with
    /// still-free memory ([`FrameError::DoubleFree`]); touching (but not
    /// overlapping) free neighbors coalesce on success. Every error path
    /// leaves `free_pages` / `free_count` / `cursor` exactly as they were.
    pub fn free(&mut self, base: u64, pages: u64) -> Result<(), FrameError> {
        if pages == 0 {
            return Err(FrameError::OutsideMap);
        }
        let size = pages.checked_mul(PAGE_SIZE).ok_or(FrameError::Wrap)?;
        if base > u64::MAX - size {
            return Err(FrameError::Wrap);
        }
        let range = Span {
            start: base,
            end: base + size,
        };
        // Outside-map: must sit entirely inside one coalesced Usable span
        // (map is maximally coalesced, so a range crossing a hole between
        // Usable spans cannot be contained in any single span).
        if !self.map[..self.map_count]
            .iter()
            .any(|span| span.contains(range))
        {
            return Err(FrameError::OutsideMap);
        }
        // Double-free: any byte of `range` still on the free list.
        if self.free[..self.free_count]
            .iter()
            .any(|span| span.overlaps(range))
        {
            return Err(FrameError::DoubleFree);
        }
        if self.free_count == MAX_SPANS {
            // No room to insert a new disjoint span (should not happen if
            // free never exceeds map coverage) — fail closed, no mutation.
            return Err(FrameError::OutsideMap);
        }
        self.insert_coalesce(range, pages);
        Ok(())
    }

    /// Insert `range` into the sorted free list and coalesce touching
    /// neighbors; adds `pages` to `free_pages`. Precondition: `range` has
    /// been checked for wrap, map coverage, and free-list overlap.
    fn insert_coalesce(&mut self, range: Span, pages: u64) {
        // Find insertion index (first free span with start > range.start).
        let mut idx = 0;
        while idx < self.free_count && self.free[idx].start < range.start {
            idx += 1;
        }
        // Shift tail right.
        for k in (idx..self.free_count).rev() {
            self.free[k + 1] = self.free[k];
        }
        self.free[idx] = range;
        self.free_count += 1;
        // Coalesce with previous span if it touches (overlap impossible).
        if idx > 0 && self.free[idx - 1].touches(self.free[idx]) {
            let merged = Span {
                start: self.free[idx - 1].start,
                end: self.free[idx - 1].end.max(self.free[idx].end),
            };
            // Remove current idx (now absorbed into idx-1).
            for k in idx..self.free_count - 1 {
                self.free[k] = self.free[k + 1];
            }
            self.free_count -= 1;
            self.free[idx - 1] = merged;
            idx -= 1;
        }
        // Coalesce with next span if it touches.
        if idx + 1 < self.free_count && self.free[idx].touches(self.free[idx + 1]) {
            self.free[idx].end = self.free[idx].end.max(self.free[idx + 1].end);
            for k in idx + 1..self.free_count - 1 {
                self.free[k] = self.free[k + 1];
            }
            self.free_count -= 1;
        }
        // Cursor stays a valid index into the (possibly shorter) list.
        if self.free_count == 0 {
            self.cursor = 0;
        } else if self.cursor >= self.free_count {
            self.cursor = self.free_count - 1;
        }
        self.free_pages = self.free_pages.saturating_add(pages);
    }
}

/// Guest-side stress proof for S03 task #5: drive the allocator over
/// `boot`'s Usable map, inject the three fail-closed faults, and emit
/// `FRAMEALLOC ok=<0|1> free=<dec> spans=<n> faults=<n>\n` on `emit`
/// (the `tools/qemu/boot-uefi.ps1` hard-require is `FRAMEALLOC ok=1`).
///
/// Battery, in order (any failure clears `ok` but the line still prints):
/// 1. 1024× single-page `alloc`+`free` sweep — every step must succeed.
/// 2. Multi-page pair (two 2-page blocks) allocated and both freed, with
///    `free_pages`/`free_span_count` fully restored to the pre-pair values.
/// 3. Three faults — double-free of a still-free in-map range
///    ([`FrameError::DoubleFree`]), a wild address
///    ([`FrameError::OutsideMap`]), and a wrap ([`FrameError::Wrap`]) —
///    each must return exactly that error and leave the
///    `(free_pages, free_span_count, cursor)` snapshot untouched.
///
/// `faults` counts faults that failed with the expected kind (3 on a full
/// battery). An empty Usable map (`free_pages() == 0` after ingest) skips
/// the battery entirely and reports `ok=0 free=0 spans=0 faults=0`.
/// Never panics; returns only the sink's error (COM1 is infallible in the
/// guest; host tests collect bytes).
pub fn frame_alloc_stress<E>(
    boot: &BootInfo,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    let mut fa = FrameAllocator::from_boot_info(boot);
    let mut ok = fa.free_pages() > 0;
    let mut faults = 0usize;
    if ok {
        // (1) 1024x single-page sweep — every alloc/free must succeed.
        for _ in 0..1024 {
            let Some(base) = fa.alloc(1) else {
                ok = false;
                break;
            };
            if fa.free(base, 1).is_err() {
                ok = false;
                break;
            }
        }
        // (2) Multi-page pair; free count/spans must fully restore.
        let free0 = fa.free_pages();
        let spans0 = fa.free_span_count();
        let a = fa.alloc(2);
        let b = fa.alloc(2);
        let mut pair_ok = a.is_some() && b.is_some();
        for block in [a, b].into_iter().flatten() {
            if fa.free(block, 2).is_err() {
                pair_ok = false;
            }
        }
        if fa.free_pages() != free0 || fa.free_span_count() != spans0 {
            pair_ok = false;
        }
        if !pair_ok {
            ok = false;
        }
        // (3) Fault storm against a stable snapshot: each must Err with
        // the expected kind, and no state may move.
        let snap = (fa.free_pages(), fa.free_span_count(), fa.cursor());
        // Map union was fully restored above, so map span 0 is free —
        // its first page is the canonical still-free double-free target.
        let dbl_base = fa.map[0].start;
        let results = [
            (fa.free(dbl_base, 1), FrameError::DoubleFree),
            (fa.free(0xFFFF_DEAD_BEEF, 1), FrameError::OutsideMap),
            (fa.free(u64::MAX - 0x800, 4), FrameError::Wrap),
        ];
        for (result, expected) in results {
            if result == Err(expected) {
                faults += 1;
            } else {
                ok = false;
            }
        }
        if (fa.free_pages(), fa.free_span_count(), fa.cursor()) != snap {
            ok = false;
        }
    }
    write_str(emit, "FRAMEALLOC ok=")?;
    write_u64_dec(emit, u64::from(ok))?;
    write_str(emit, " free=")?;
    write_u64_dec(emit, fa.free_pages())?;
    write_str(emit, " spans=")?;
    write_u64_dec(emit, fa.free_span_count() as u64)?;
    write_str(emit, " faults=")?;
    write_u64_dec(emit, faults as u64)?;
    write_str(emit, "\n")
}

/// Sort the first `count` spans of `spans` by `start` (insertion sort:
/// stable, no allocator, tiny `count`).
fn sort_spans(spans: &mut [Span], count: usize) {
    for i in 1..count {
        let mut j = i;
        while j > 0 && spans[j].start < spans[j - 1].start {
            spans.swap(j, j - 1);
            j -= 1;
        }
    }
}

/// Coalesce overlapping/touching neighbors in a sorted span array; returns
/// the new count. `spans` must already be sorted by `start`.
fn coalesce(spans: &mut [Span], count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    let mut out = 1usize;
    for i in 1..count {
        let cur = spans[i];
        let last = out - 1;
        if spans[last].overlaps(cur) || spans[last].touches(cur) {
            spans[last].end = spans[last].end.max(cur.end);
        } else {
            spans[out] = cur;
            out += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use baseos_boot_info::{MemoryKind, MemoryRegion};

    /// BootInfo with the given Usable regions (plus one Reserved decoy so
    /// ingest filtering is exercised on every fixture).
    fn boot_with(usable: &[(u64, u64)]) -> BootInfo {
        let mut boot = BootInfo::new();
        for &(start, pages) in usable {
            assert!(boot.push_region(MemoryRegion::new(start, pages, MemoryKind::Usable)));
        }
        assert!(boot.push_region(MemoryRegion::new(0xDEAD_0000, 1, MemoryKind::Reserved)));
        boot
    }

    /// #58 matrix (1): adjacent-touching Usable regions must free in order
    /// with no false-positive double-free; re-freeing the exact same range
    /// must error `DoubleFree` (and leave the snapshot untouched).
    #[test]
    fn adjacent_usable_regions_free_in_order_same_range_double_free_errors() {
        // Two Usable regions that touch at 0x5000 (4 pages each).
        let boot = boot_with(&[(0x1000, 4), (0x5000, 4)]);
        let mut fa = FrameAllocator::from_boot_info(&boot);
        // Ingest coalesces the touching pair into ONE map/free span.
        assert_eq!(fa.map_span_count(), 1);
        assert_eq!(fa.free_span_count(), 1);
        assert_eq!(fa.free_pages(), 8);

        // Allocate the full coalesced span as two adjacent 4-page blocks.
        let a = fa.alloc(4).expect("first half");
        let b = fa.alloc(4).expect("second half");
        assert_eq!((a, b), (0x1000, 0x5000));
        assert_eq!(fa.free_pages(), 0);

        // Free in address order: b abuts a's range once a is free — must OK.
        assert_eq!(fa.free(a, 4), Ok(()));
        assert_eq!(fa.free(b, 4), Ok(()), "adjacent in-order free must OK");
        // Coalesced back to one span, full 8 pages restored.
        assert_eq!((fa.free_pages(), fa.free_span_count()), (8, 1));

        // Same-range double-free of `a` (now fully free) must error and
        // must not change free count / span count / cursor.
        let before = (fa.free_pages(), fa.free_span_count(), fa.cursor());
        assert_eq!(fa.free(a, 4), Err(FrameError::DoubleFree));
        assert_eq!((fa.free_pages(), fa.free_span_count(), fa.cursor()), before);
    }

    /// #58 matrix (2a): `base > u64::MAX - len` rejects as `Wrap` before
    /// any map/free test (separate from outside-map).
    #[test]
    fn free_rejects_wrapping_range() {
        let boot = boot_with(&[(0x1000, 4)]);
        let mut fa = FrameAllocator::from_boot_info(&boot);
        let before = (fa.free_pages(), fa.free_span_count(), fa.cursor());

        // base = u64::MAX - 0x1000, 2 pages => len = 0x2000 => base + len wraps.
        let base = u64::MAX - 0x1000;
        assert_eq!(fa.free(base, 2), Err(FrameError::Wrap));
        // Also: page_count itself overflowing u64 bytes.
        let huge_pages = (u64::MAX / PAGE_SIZE) + 1;
        assert_eq!(fa.free(0x1000, huge_pages), Err(FrameError::Wrap));
        // Snapshot unchanged after both rejects.
        assert_eq!((fa.free_pages(), fa.free_span_count(), fa.cursor()), before);
    }

    /// #58 matrix (2b): in-map-looking but uncovered addresses reject as
    /// `OutsideMap` (separate from wrap): hole between Usable spans, and
    /// past the end of the map.
    #[test]
    fn free_rejects_outside_bootinfo_map() {
        // Two Usable spans with a 1-page hole at 0x5000.
        let boot = boot_with(&[(0x1000, 4), (0x6000, 4)]);
        let mut fa = FrameAllocator::from_boot_info(&boot);
        assert_eq!(fa.map_span_count(), 2);
        let before = (fa.free_pages(), fa.free_span_count(), fa.cursor());

        // Hole at 0x5000: inside the bounding box, outside every span.
        assert_eq!(fa.free(0x5000, 1), Err(FrameError::OutsideMap));
        // Past the end of the map entirely (still non-wrapping).
        assert_eq!(fa.free(0xF000_0000, 1), Err(FrameError::OutsideMap));
        // Reserved decoy region address was never ingested.
        assert_eq!(fa.free(0xDEAD_0000, 1), Err(FrameError::OutsideMap));
        // Zero-page free is degenerate outside-map.
        assert_eq!(fa.free(0x1000, 0), Err(FrameError::OutsideMap));
        assert_eq!((fa.free_pages(), fa.free_span_count(), fa.cursor()), before);
    }

    /// #58 matrix (3): stress — mixed alloc/free to move the cursor, then
    /// inject double-free + oversize/wild allocation faults; free-pages,
    /// span-count, and cursor snapshots must be identical before/after the
    /// fault storm, and a healthy alloc still works afterward.
    #[test]
    fn stress_fault_injection_preserves_free_count_and_cursor_snapshot() {
        let boot = boot_with(&[(0x1000, 16), (0x20000, 8)]);
        let mut fa = FrameAllocator::from_boot_info(&boot);
        assert_eq!(fa.free_pages(), 24);

        // Drive the allocator: alloc/free mix so cursor and spans move.
        let a = fa.alloc(4).expect("a");
        let b = fa.alloc(8).expect("b");
        let c = fa.alloc(2).expect("c");
        assert_eq!(fa.free(b, 8), Ok(()));
        assert_eq!(fa.free(a, 4), Ok(()));
        // Leave `c` allocated so free list has holes for outside/double tests.
        let snap = (fa.free_pages(), fa.free_span_count(), fa.cursor());
        assert!(fa.free_pages() > 0);

        // Fault storm — every one must fail and mutate nothing:
        // (i) double-free of `b` (already returned).
        assert_eq!(fa.free(b, 8), Err(FrameError::DoubleFree));
        // (ii) double-free of a subrange of still-free `b`.
        assert_eq!(fa.free(b, 2), Err(FrameError::DoubleFree));
        // (iii) oversize alloc (more frames than free).
        assert!(fa.alloc(fa.free_pages() + 1).is_none());
        // (iv) oversize alloc with u64-overflowing page count.
        assert!(fa.alloc((u64::MAX / PAGE_SIZE) + 1).is_none());
        // (v) zero-page alloc.
        assert!(fa.alloc(0).is_none());
        // (vi) wild free outside the map.
        assert_eq!(fa.free(0xFFFF_DEAD_BEEF, 1), Err(FrameError::OutsideMap));
        // (vii) wrap free.
        assert_eq!(fa.free(u64::MAX - 0x800, 4), Err(FrameError::Wrap));
        // (viii) free of the never-allocated Reserved decoy.
        assert_eq!(fa.free(0xDEAD_0000, 1), Err(FrameError::OutsideMap));

        // Snapshot must be bit-identical after every fault.
        assert_eq!(
            (fa.free_pages(), fa.free_span_count(), fa.cursor()),
            snap,
            "fault injection must not mutate allocator state"
        );

        // Allocator still healthy: free the live allocation and reclaim all.
        assert_eq!(fa.free(c, 2), Ok(()));
        assert_eq!(fa.free_pages(), 24);
        // Two disjoint map spans (16 + 8 pages) — next-fit hands each out;
        // no multi-span stitching this slice.
        let first = fa.alloc(16);
        let second = fa.alloc(8);
        assert_eq!((first, second), (Some(0x1000), Some(0x20000)));
        assert_eq!(fa.free_pages(), 0);
        // Reverse-order free of the two spans still succeeds and restores.
        assert_eq!(fa.free(second.unwrap(), 8), Ok(()));
        assert_eq!(fa.free(first.unwrap(), 16), Ok(()));
        assert_eq!(fa.free_pages(), 24);
    }

    /// S03 #5 host half of the guest stress: a healthy 16-page Usable map
    /// completes sweep + multi-page pair + three faults and emits the
    /// exact `FRAMEALLOC ok=1 free=16 spans=1 faults=3` line.
    #[test]
    fn frame_alloc_stress_healthy_map_emits_exact_ok_line() {
        let boot = boot_with(&[(0x1000, 16)]);
        let mut out = Vec::new();
        frame_alloc_stress::<core::convert::Infallible>(&boot, &mut |b| {
            out.push(b);
            Ok(())
        })
        .unwrap();
        let text = std::string::String::from_utf8(out).unwrap();
        assert_eq!(text, "FRAMEALLOC ok=1 free=16 spans=1 faults=3\n");
    }

    /// Empty Usable map: no frames to sweep — battery skipped fail-closed,
    /// line reports `ok=0 free=0 spans=0 faults=0` (e2e hard-require of
    /// `ok=1` would fail loudly).
    #[test]
    fn frame_alloc_stress_empty_map_reports_ok_zero() {
        let boot = boot_with(&[]);
        let mut out = Vec::new();
        frame_alloc_stress::<core::convert::Infallible>(&boot, &mut |b| {
            out.push(b);
            Ok(())
        })
        .unwrap();
        let text = std::string::String::from_utf8(out).unwrap();
        assert_eq!(text, "FRAMEALLOC ok=0 free=0 spans=0 faults=0\n");
    }

    /// S03 verification (host-only #59b): allocate/free large numbers of
    /// frames — drain a 2000-page two-span map one page at a time, prove
    /// exhaustion fails closed without state motion, then free all in
    /// reverse and prove full restore (count + spans).
    #[test]
    fn alloc_drains_entire_map_and_restores() {
        let boot = boot_with(&[(0x1000, 1500), (0x100_0000, 500)]);
        let mut fa = FrameAllocator::from_boot_info(&boot);
        assert_eq!((fa.free_pages(), fa.free_span_count()), (2000, 2));
        let mut bases = std::vec::Vec::with_capacity(2000);
        for _ in 0..2000 {
            bases.push(fa.alloc(1).expect("drain must succeed"));
        }
        assert_eq!(fa.free_pages(), 0);
        let snap = (fa.free_pages(), fa.free_span_count(), fa.cursor());
        assert!(fa.alloc(1).is_none(), "exhausted alloc must fail");
        assert_eq!(
            (fa.free_pages(), fa.free_span_count(), fa.cursor()),
            snap,
            "failed alloc must not mutate state"
        );
        for base in bases.into_iter().rev() {
            assert_eq!(fa.free(base, 1), Ok(()));
        }
        assert_eq!((fa.free_pages(), fa.free_span_count()), (2000, 2));
    }
}
