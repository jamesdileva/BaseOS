# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: H @7dde5a7 hardening done: stub ->! closes cfg divergence; entry_as_func single transmute gate (7 sites); jump_rsp_refuses pins hlt-park decision + live path; SENT_* single-source + test. 167/167 (142+25), fmt+clippy clean, gates 41/41, QEMU PASS 1254B guest 107008B stitch=5 ok0+ok1 PF/EXC. #13 done, #9 active, #11 retitle owed by -b. REVIEW #378 accepts.
- Next: fuller context switch (jump entry/multi-dispatch) + #9 retitle slice1->slice22+ owed via #11. #9 active.
- Board: #9 active (S06 threads) resume per #79/#80; #11 retitle owner agent-b.
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
