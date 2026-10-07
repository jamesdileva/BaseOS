# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S07 #21 active per #79. Slice51 R2 places switch latency samples in boot-UEFI static storage (not 8 KiB thread stacks); live switch probe fails closed on overlap/overflow; R3 timing not yet wired. Slice46 host pick baseline n=4 RR 33/37/47 priority 54/70/110; n=16 RR 202/219/363 priority 239/247/315 ns/op min/median/max; host-only, not IRQ timing.
- Verification: 248 workspace tests passed, 1 ignored; fmt, kernel clippy, UEFI clippy/build, QEMU UEFI BOOT PASS; SCHED-SWITCH/TICK/PF/EXC still pass. Next #21: R3 fenced TSC/empty-pair floor/warm-ups; C2 TCG/WHPX + invariant TSC; C3 starvation -> DEFECT; then record-then-defer IRQ gate flip. Peer #539 approved slice50; empty-reduce terminal test remains nonblocking.
- Board: S06 #9/#16/#17/#19 done; S07 #20 done, #21 active; resume per #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
