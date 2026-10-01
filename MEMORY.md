# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice16 hardening done @fe1dd55 (151 green
  21+18+102+4+4+2, fmt/clippy clean, QEMU 1143B carry-forward
  kernel-only HEAD, guest untouched). REVIEW #323 ACCEPT a-c.
- Next: slice17 guest-switch wiring (preempt TOCTOU doc-not-tested,
  #9 retitle slice1->slice16 owed) + #323 test-strength residuals
  (string-literal block-strip, dispatch state asserts).
- Board: #9 active (S06 threads). OK per human #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
