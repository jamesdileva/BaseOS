# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice10 @0f203f1 (new-stack RSP spill, 5-reg sentinel
  callee_saved_survives, r15 clobber, direct *const() transmute).
  138 host tests green (21+18+89+4+4+2), fmt/clippy host+uefi clean,
  UEFI 0 warnings. REVIEW #291 gaps closed; rbp excluded (frame
  pointer) documented. REVIEW #293 cond-approve, QEMU PASS
  indep-verified (1023B, THREAD+THREAD-RUN+THREAD-CS iter=16+PF+EXC).
- Next: true two-way RSP save/restore; #293 risks 1-3 carried
  (read-only rbp compare, rbx/FP reservation disproof, FP pinning).
- Board: #9 active (S06 threads). OK per human #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
