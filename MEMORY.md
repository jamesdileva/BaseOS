# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S03 #5 ACTIVE. Slices 1-4 COMMITTED (... slice4 this cycle:
  frame_alloc.rs — BootInfo Usable ingest+coalesce, next-fit alloc,
  free with Wrap/OutsideMap/DoubleFree ordered checks, snapshot-stable
  faults). Matrix 4/4: adjacent-OK+double-free, wrap, outside-map,
  stress free_count/cursor. Default tests 41; kernel 10; boot-uefi 8.
- Next #5: guest FRAMEALLOC serial line + QEMU alloc/free stress (S03
  verification residual); no multi-span stitching yet (by design).
- Open: #59(a) endorse serial-log map_size/desc_size vs 16KiB+4KiB budget
  every boot — zero-risk baseline, not coded yet; (b) forced-failure
  build still host-only until real-UEFI BUFFER_TOO_SMALL run. #6
  refresh_map_key >16KiB fail-closed: owner agent-a, proposed, needs
  host test + acceptance note.
- Conventions: STRUCTURAL_FAIL = usize::MAX; raw EFI status; ASCII .ps1;
  raw UEFI ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial
  hard-require BOOTINFO/MEMMAP/GOP/KERNEL/EBS/EXC vector=0.
- Env (2026-09-22): Win11, rustc 1.98.0, QEMU 11.1.1. Gates: default
  cargo test 41 + `-p baseos-boot-uefi` 8 + `-p baseos-kernel` 10; fmt;
  clippy 4x -D warnings; boot-uefi.ps1 PASS (31232B); smoke -RequireOvmf
  PASS. boot-uefi NOT in default-members.
- Sitrep harness: stale dirty+SKIPPED + board only #1/#2 (4th cycle) —
  escalated, not blocking. Tree clean pre-commit this cycle.
