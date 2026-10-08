# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S07 #21 flip landed, pending b-side re-verify. Slice66 opens the IRQ gate (record-then-defer live: IRQ tick-only, task-context owns the drain+dispatch) + asserts STARVATION_RR_BOUNDED (closes REVIEW #572 weakness). Slice65 C3 pin, slice64 WHPX n=3 holds.
- Verification: 254 workspace tests passed, 1 ignored; fmt, kernel clippy, UEFI clippy/build green; QEMU TCG x2 UEFI BOOT PASS (2022B serial, guest 166400B = slice59 baseline), TICK ticks=5 + SCHED-TICK q=4 + SWITCH-LAT count=16 on both; gate-test 41/41.
- Board: S06 #9/#16/#17/#19 done; S07 #20 done, #21 active; resume per #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
