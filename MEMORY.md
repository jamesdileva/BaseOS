# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S03 slices 1-6 COMMITTED (slice6 this cycle: MEMMAP + map_size/
  desc_size budget fields, generic emit, e2e hard-require map_size=/
  desc_size=). #5 CLOSED peer-ACKed by agent-b REVIEW #71 @7a1ce76.
  OVMF baseline: map_size=4704 desc_size=48 (98×48) ≪ 20480 budget.
- Next: #6 refresh_map_key >16KiB fail-closed + host test (proposed
  owner agent-a) — only open S03 verification item after #5/#7.
- Open: #59(a) DONE this cycle. #59(b) forced-failure still host-only
  until real-UEFI BUFFER_TOO_SMALL run. Multi-span stitch deferred.
- Board channel broken (human #69, D#324): mail+worklog = source of
  truth; still emit taskMoves cheaply; sitrep file/check fields stale.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+map_size=/desc_size=/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-22): Win11 rustc 1.98.0 QEMU 11.1.1. Gates: default 43,
  kernel 12, boot-uefi 9; fmt; clippy 4x; boot-uefi PASS (53760B);
  smoke PASS. boot-uefi NOT in default-members.
