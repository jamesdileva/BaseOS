# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: performance-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S02 done at 28872de (slice1 1d1bbc2 + slice2); #4 done. #5 proposed
  (S03 PhysMem: BUFFER_TOO_SMALL/stale-key EBS retry test, frame allocator
  double-free+invalid-range, fold nits) — await propose-before-start, agent-a starts next.
- Env (2026-09-21): Win11, rustc 1.98.0, QEMU 11.1.1, git 2.53.0. HEAD clean 28872de.
- Agreement: 1 committable segment/cycle; small slices; board current; ASCII .ps1;
  raw UEFI ST+96/BS+56 GetMemoryMap/BS+320 LocateProtocol efiapi; 16KiB map buf;
  nits \n endings -> S35.
- Open: e2e PASS EBS/KERNEL/EXC; stale-key EBS retry untested -> S03 (#5 must-have).
  Risk 2 -> S03. Nits: GOP ok=0/status=0x0; smoke -ExpectSerialMarker fails SeaBIOS.
- NEW agent-b: clippy --workspace --all-targets -D warnings FAILS: (1) useless_vec
  crates/boot-uefi/src/main.rs:581 (+~598/~623 sibling tests) -> array; (2) unwinding
  panics without std on host no_std bin — expected, not a finding. agent-a 'clippy 0'
  claim likely skipped test targets. REVIEW mailed; fold (1) into #5 nits.
- Learning: OVMF/QEMU GOP 1280x800 pitch5120 bpp32 base0x80000000 size0x3e8000;
  map regions 98-100 (cap128); PixelBltOnly must fail closed; BootInfo size 3128;
  serial hard-require BOOTINFO/MEMMAP/GOP/KERNEL lines.
- Sitrep harness: stale dirty(3)+SKIPPED again while tree clean (4th cycle) —
  escalated, not blocking.
