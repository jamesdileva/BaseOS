# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S03 #5 ACTIVE. Slices 1-3 COMMITTED (2a0132b, b8361ae, slice3 this
  cycle): STRUCTURAL_FAIL nits; stale-key EBS retry test; BUFFER_TOO_SMALL
  pool dance (required+4KiB slack, max 4 attempts, retained buffer, one
  buf pointer fills from the key-producing call) + GOP LocateProtocol moved
  BEFORE fill_boot_info (key stability) + #51 invariant tests: dance
  [BTS->SUCCESS] through fake BS (+56/+64) and refresh-failure (raw err /
  zero key) fail-closed 1-EBS/1-refresh. boot-uefi host tests 8.
- Next #5 slice (last): frame allocator over BootInfo regions +
  double-free/invalid-range detect + stress. FLAGGED to agent-b: no dance
  in refresh_map_key (single-shot static, >16KiB maps fail closed).
- Conventions: STRUCTURAL_FAIL = usize::MAX sentinel; raw EFI status passthrough;
  ASCII .ps1; raw UEFI ST+96/BS+56/64/232/320 efiapi; 16KiB map buf +
  GET_MAP_POOL_SLACK 4096; serial hard-require
  BOOTINFO/MEMMAP/GOP/KERNEL/EBS/EXC vector=0.
- Env (2026-09-22): Win11, rustc 1.98.0, QEMU 11.1.1. Gates: cargo test 37
  + `-p baseos-boot-uefi` 8; fmt; clippy default + `-p boot-uefi --tests` +
  `--target x86_64-unknown-uefi` (all -D warnings exit 0); boot-uefi.ps1
  PASS (guest 31232B); smoke -RequireOvmf PASS. boot-uefi NOT in
  default-members — host tests need explicit -p.
- Sitrep harness: stale dirty+SKIPPED + board only #1/#2 (3rd cycle) —
  escalated, not blocking. nits \n endings -> S35. OVMF GOP 1280x800
  pitch5120 base0x80000000.
