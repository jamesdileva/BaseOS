# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice12 (two-way save-slot probe + THREAD-SAVE ok=1).
  139 host tests green (21+18+90+4+4+2), fmt/clippy host+uefi clean,
  UEFI 0 warnings. QEMU PASS 1071B
  (THREAD+THREAD-RUN+THREAD-CS iter=16+THREAD-SAVE+PF+EXC). REVIEW #297 accept stood.
- Next: full A->B->A ping-pong through two save slots; #297 risks (a)(b) carried
  (rbx/FP reservation disproof, FP pinning).
- Board: #9 active (S06 threads). OK per human #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
