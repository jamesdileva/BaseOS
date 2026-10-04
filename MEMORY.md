# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S07 slice2 fairness committed, 184 green (21+32+121+4+4+2), QEMU PASS x2 1536B SCHED-FAIR ok=1 (30/10/10/10/spread 0), tree clean. #20 stays active (timer-quantum/trace owed). Next: S07 slice3 quantum/trace or close per review.
- Next: S06 preempt-IRQ gate slice (cli/atomic around pick) per #423 residual (b); then S07 scheduler per roadmap.
- Board: #9 done, #16 done, #17 done (S06 multi-hop chain); resume per #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
