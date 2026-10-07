# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S07 #21 active per #79. Slice52 R3-host adds cpu fenced TSC (lfence;rdtsc;lfence), saturating tsc_delta, empty-pair floor, 128 warm-ups; guest sampling unwired. Slice51 R2 static SWITCH_LATENCIES outside thread stacks. Slice46 host pick n=4 RR 33/37/47 priority 54/70/110; n=16 RR 202/219/363 priority 239/247/315 ns/op; host-only.
- Verification: 250 workspace tests passed, 1 ignored; fmt, kernel clippy, UEFI clippy/build green; no guest edit so no QEMU this host-only slice. Next #21: R3 guest sampling + C2 TCG/WHPX + C3 starvation->DEFECT, then record-then-defer IRQ gate flip. Peer #539 approved slice50; empty-reduce terminal test remains nonblocking.
- Board: S06 #9/#16/#17/#19 done; S07 #20 done, #21 active; resume per #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
