# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S06 slice17d done @358dcaa (152 green
  21+18+103+4+4+2, fmt/clippy clean, guest untouched no QEMU).
  REVIEW #334 cond-accept host-only (r1 fixed br-only, r2 pinned
  boundary, r3 weak-accept multibyte/E0748 doc-only). Worklog 17d done.
- Next: slice18 guest-switch wiring (preempt TOCTOU doc-not-tested,
  #9 retitle slice1->slice17 owed) + #330 cheapest disproof re-verify.
- Board: #9 active (S06 threads). OK per human #79/#80.
- Board: #9 active (S06 threads). OK per human #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
