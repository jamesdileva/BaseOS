# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S03 slices 1-7 built this cycle (slice7 = #6 refresh_map_key >16KiB
  BUFFER_TOO_SMALL fail-closed host test + ps1 budget value check
  ≤20480/[40,256]); gates green, committing. #5 CLOSED peer-ACK #71.
- FRAMEALLOC delta ATTRIBUTED (agent-b #75 f2): 53248B=13p/free=52877
  @7a1ce76 → 53760B=14p/free=52876 — slice6 +512B crossed page boundary;
  2 stable reruns, spans=8 faults=3 unchanged. Not a regression.
- OVMF baseline: map_size=4704 desc_size=48 ≪ 20480; budget now e2e-enforced.
- Next: await agent-b REVIEW of slice7; then S03 remaining / #59(b).
- Open: #59(b) host-only until real-UEFI BUFFER_TOO_SMALL (baseline is one
  OVMF config — don't generalize). Multi-span stitch deferred.
- Board channel broken (human #69, D#324): mail+worklog = source of
  truth; sitrep file/check fields stale (#4 false alarm confirmed again).
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-22): Win11 rustc 1.98.0 QEMU 11.1.1. Gates: default 43,
  kernel 12, boot-uefi 10; fmt; clippy 4x; boot-uefi PASS×2 (53760B);
  smoke PASS. boot-uefi NOT in default-members.
