# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice26b done: live 1->2->3->1 jmp chain (171 green = 21+27+113+4+4+2, fmt+clippy clean, gates 41/41, QEMU 1360B PASS w/ CHAIN order gates). #9/#16 done per #402, #17 done this cycle.
- Next: S06 next slice (join/preempt per roadmap) + worklog close-out entry landed.
- Board: #9 done, #16 done, #17 done (S06 multi-hop chain); resume per #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
