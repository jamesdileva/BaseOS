# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice27b @07f3eba done + REVIEW #423 accept (live-carry cursor + guest JOIN/EXIT, gaps 1-3 closed), 175 green (145+30), QEMU PASS x2 1445B/116736B, tree clean. #18 done.
- Next: S06 preempt-IRQ gate slice (cli/atomic around pick) per #423 residual (b); then S07 scheduler per roadmap.
- Board: #9 done, #16 done, #17 done (S06 multi-hop chain); resume per #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
