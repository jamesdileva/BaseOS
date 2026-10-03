# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice26a done: STACK_C live + three_stacks_dispatch_parity_host (170 green = 144+26, fmt+clippy clean, gates 41/41, QEMU 1254B PASS single-hop unchanged). #9 active (guest multi-hop 1->2->3->1 live jmp next), #16 retitle owed by -b.
- Next: S06 slice26b guest multi-hop 1->2->3->1 live jmp (never-return chain via C + bring-up relocation) + #16 retitle slice1->slice26 owed via -b. #9 active.
- Board: #9 active (S06 threads) resume per #79/#80; #16 retitle slice1->slice26 owner agent-b.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
