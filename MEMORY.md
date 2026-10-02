# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice22 done (161 green 21+22+108+4+4+2,
  fmt clean, clippy kernel+boot-tests clean, gates 41/41,
  QEMU PASS 1232B THREAD-JUMP ok=1 id=2 + RUN id=2,
  guest 105984B, PF/EXC chained from jumped stack).
  Triple AND gate pinned; thin-test disproof on-tree.
  REVIEW #358 accepts 1-4 met on-measure.
- Next: fuller context switch (jump entry/multi-dispatch)
  + #9 retitle slice1->slice17 owed via #11. #9 active.
- Board: #9 active (S06 threads). OK per human #79/#80.
- Board: #9 active (S06 threads). OK per human #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
