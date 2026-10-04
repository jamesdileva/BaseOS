# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S07 slice4 exit-trace committed, 189 green (21+34+124+4+4+2), QEMU PASS x2 1605B SCHED-TRACE-X ok=1 (n=2 t0=2 t1=3), tree clean. #20 stays active (timer-quantum + tail/timestamps owed). Next: S07 slice5 timer-quantum or close per review.
- Board: #9 done, #16 done, #17 done (S06 multi-hop chain); #19 preempt-IRQ gate done; S07 #20 active; resume per #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
