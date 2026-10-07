# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S07 slice46 host pick baseline (7 batches x 10k calls, release Instant) n=4 RR 33/37/47 priority 54/70/110; n=16 RR 202/219/363 priority 239/247/315 ns/op min/median/max. MAX_THREADS=16; 64 invalid. Host-only, not kernel IRQ timing. Both picks O(n^2) due validate_threads; S12 is storage not SMP.
- Verification: 240 workspace tests passed, 1 ignored opt-in bench; fmt + kernel clippy clean. #20 active until #21 gate; #21 proposed, atomic flip + record-then-defer with yield/preempt semantics resolved/deferred; no QEMU this host-only slice.
- Board: S06 #9/#16/#17/#19 done; S07 #20 active, #21 proposed; resume per #79/#80.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
