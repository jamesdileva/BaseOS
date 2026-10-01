# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice14 @81cc614 (REVIEW #303 3 gaps closed: slot-path
  sentinel/rbp proof, triple entry both hops, read-once new_rsp).
  140 host green (21+18+91+4+4+2), fmt/clippy clean, UEFI 0 warn.
  QEMU PASS (PONG ok=1 save_a=0xfe64760 save_b=0xde37cd0, PONG-RUN
  id=1, CS ok=1 iter=16). REVIEW #305 APPROVED.
- Next: next S06 slice (scheduler/jump-entry per roadmap); #9 active.
- Board: #9 active (S06 threads). OK per human #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
