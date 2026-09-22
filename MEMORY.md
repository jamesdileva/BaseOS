# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S03 #5 ACTIVE. Slice 1 nits COMMITTED 2a0132b; slice 2 stale-key EBS
  retry host test committed this cycle (fake BS GetMemoryMap+56 + EBS+232;
  zero key -> STRUCTURAL_FAIL no EBS call; stale once -> 1 refresh + ok;
  stale twice -> fail closed raw EFI_INVALID_PARAMETER; holds
  MEMMAP_TEST_LOCK). boot-uefi host tests 6.
- Next #5 slice (last): BUFFER_TOO_SMALL handling + frame allocator over
  BootInfo regions + double-free/invalid-range detect. fn->usize test cast
  needs `as *const () as usize`.
- Conventions: STRUCTURAL_FAIL = usize::MAX sentinel; raw EFI status passthrough;
  ASCII .ps1; raw UEFI ST+96/BS+56/232/320 efiapi; 16KiB map buf; serial
  hard-require BOOTINFO/MEMMAP/GOP/KERNEL/EBS/EXC vector=0.
- Env (2026-09-21): Win11, rustc 1.98.0, QEMU 11.1.1. Gates: cargo test 37 +
  `cargo test -p baseos-boot-uefi` 6; fmt; clippy workspace (known #2 noise
  only) + `-p boot-uefi --tests` + `--target x86_64-unknown-uefi`;
  boot-uefi.ps1 PASS; smoke -RequireOvmf PASS. boot-uefi NOT in
  default-members — host tests need explicit -p.
- Sitrep harness: stale dirty(3)+SKIPPED while tree clean — escalated, not
  blocking. nits \n endings -> S35. OVMF GOP 1280x800 pitch5120 base0x80000000.
