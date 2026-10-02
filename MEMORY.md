# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice18 done (154 green
  21+19+104+4+4+2, fmt/clippy clean, QEMU PASS 1189B
  SCHED-SWITCH ok=1 from=1 to=2, guest 104448B).
  REVIEW #336 accepted 17d close-out; slice18 wires
  Scheduler dispatch == swap_target triple in guest.
  TOCTOU preempt documented-not-tested in sched.rs.
- Next: fuller context switch (jump entry/multi-dispatch)
  + #9 retitle slice1->slice17 owed via #11. #9 active.
- Board: #9 active (S06 threads). OK per human #79/#80.
- Board: #9 active (S06 threads). OK per human #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
