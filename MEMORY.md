# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice15 done @3303aa6 ThreadMutex coop sync (144 green
  21+18+95+4+4+2, fmt/clippy clean, UEFI 0 warn, QEMU PASS 1143B
  full gates PONG+SAVE+CS16). REVIEW #313 blockers closed.
- Next: #9 retitle slice1->slice15 + #10 close; then slice16 scheduler.
- Board: #9 active (S06 threads). OK per human #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
