# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: performance-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S03 #5 ACTIVE. Slice 1 (nits) built+gated green, commit this cycle:
  STRUCTURAL_FAIL sentinel (ok=0 never status=0x0), clippy useless_vec fixed
  (workspace clippy now only known host-bin unwinding noise = agent-b #2),
  smoke -ExpectSerialMarker documented SeaBIOS-dead (kept for future flip).
  New tests: fill_boot_info + fill_framebuffer failure diagnosability (5/5
  boot-uefi host).
- Next slices for #5 (in order): (1) stale-map-key EBS retry host test —
  fake BS needs BOTH GetMemoryMap(+56)+EBS(+232) slots, EBS returns
  EFI_INVALID_PARAMETER once then 0, GetMemoryMap writes nonzero key; take
  MEMMAP_TEST_LOCK (static Mutex in tests mod) — MEMMAP_BUF race vs
  fill_boot_info tests; (2) BUFFER_TOO_SMALL handling + frame allocator over
  BootInfo regions + double-free/invalid-range detect.
- Conventions: STRUCTURAL_FAIL = usize::MAX documented sentinel; raw EFI
  status passes through verbatim; ASCII .ps1; raw UEFI ST+96/BS+56/232/320
  efiapi; 16KiB map buf; serial hard-require BOOTINFO/MEMMAP/GOP/KERNEL/EBS/
  EXC vector=0.
- Env (2026-09-21): Win11, rustc 1.98.0, QEMU 11.1.1. Gates: cargo test 37 +
  `cargo test -p baseos-boot-uefi` 5; fmt; clippy workspace (known #2 noise
  only) + `-p baseos-boot-uefi --tests` + `--target x86_64-unknown-uefi`;
  boot-uefi.ps1 PASS; smoke -RequireOvmf PASS. boot-uefi is NOT in
  default-members — host tests need explicit -p.
- Sitrep harness: stale dirty(3)+SKIPPED 5th cycle while tree clean —
  escalated to human/orchestrator, not blocking.
- nits \n endings -> S35. OVMF GOP 1280x800 pitch5120 base0x80000000.
