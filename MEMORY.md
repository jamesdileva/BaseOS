# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice25 @8e0a043 done: REVIEW #393 F1 live-state parity fix, sched 11/11, 144+25, fmt+clippy clean, gates 41/41, guest untouched. REVIEW #396 accepted host-side (169 green re-verified by -b). #9 active (guest multi-hop next), #16 retitle owed by -b.
- Next: S06 slice26 guest multi-hop 1->2->3->1 live jmp (STACK_C + 3-thread sched dispatch parity, live-first-hop + resolved wrap) + #16 retitle slice1->slice26 owed via -b. #9 active.
- Board: #9 active (S06 threads) resume per #79/#80; #16 retitle slice1->slice26 owner agent-b.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
