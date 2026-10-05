# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S07 slice19 committed, 220 green (21+35+154+4+4+2), fmt+clippy clean, QEMU PASS x2 1667B TICK ok=1 ticks=1 + PF/EXC, tree clean. #20 stays active (live periodic IRQ dispatch + failure backoff owed).
- Board: #9 done, #16 done, #17 done (S06 multi-hop chain); #19 preempt-IRQ gate done; S07 #20 active; resume per #79/#80.
- Slice19: IRQ-owned STATIC_SCHED + sched_init_demo_static/sched_poll_static, poll_timer_dispatch static-only (no stack sched), demo_armed_scheduler pinned host-side. Next: live periodic handler + backoff/cap (REVIEW #466 risk) + unknown-quantum policy.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
