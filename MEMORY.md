# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S08 done @1640347 — QEMU TCG x2 PASS, STI ok=1 ticks=10 work=20000 spins=9 batch=2000 target=20000 byte-identical, 2152B serial, guest 169984B. Slice4 knob before/after + slice6 host backstop + close-out re-prove per DoD. cargo 261 pass 1 ignored, fmt/clippy clean.
- Board: S08 #26/#27/#28 done; S09 scoping next (slice1: U/S isolation host pin + QEMU contained-fault proof). Resume per #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
