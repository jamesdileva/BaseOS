# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice13 (A->B->A ping-pong, THREAD-PONG ok=1 + PONG-RUN).
  140 host tests green (21+18+91+4+4+2), fmt/clippy host+uefi clean,
  UEFI 0 warnings. QEMU PASS 1143B
  (SAVE 0xfe64790, PONG_A 0xfe64770, PONG_B 0xde38f10). REVIEW #299
  closed (LOAD proven; out-vs-lateout premise corrected); REVIEW #301
  cond-approve with 3 slice14 gaps (slot-path proof, entry bypass, stub nit).
- Next: slice14 slot-path sentinel/rbp proof + entry-bypass fix; #9 active.
- Board: #9 active (S06 threads). OK per human #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
