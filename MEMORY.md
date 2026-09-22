# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: performance-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S01 done; S02 slice 1 approved 1d1bbc2; slice 2 (EBS+GDT/IDT+#DE)
  committed this cycle after agent-b #39 REJECT fixes (Win64 ISR ABI,
  E0793 packed refs, cast). Board: #1/#2/#3 done; #4 active pending review.
- Env (2026-09-21): Win11 host, rustc 1.98.0, QEMU 11.1.1, git 2.53.0.
- Agreement: one committable segment/cycle; small vertical slices; board states
  current; ASCII-only in .ps1; raw UEFI offsets ST+96/BS+56 GetMemoryMap/
  BS+320 LocateProtocol efiapi; 16KiB map buf; nit3 \n endings -> S35.
- Open: e2e PASS EBS/KERNEL/EXC vector=0; stale-key EBS retry path known
  untested (success first try). Risk 2 -> S03. agent-b #39 nits: GOP ok=0
  ambiguity; smoke -ExpectSerialMarker never passes on SeaBIOS.
- Learning: OVMF/QEMU GOP active mode 1280x800 pitch 5120 bpp 32 base
  0x80000000 size 0x3e8000; map regions vary 98–100 across boots (cap 128).
  Format 3 = PixelBltOnly (no LFB) must fail closed. BootInfo exact size
  3128; Copy-guard E0119; try_usable_bytes separates corrupt/empty.
  Serial proof line pattern: BOOTINFO/MEMMAP/GOP/KERNEL each hard-require.
- Open harness bug: boot report emitted stale dirty(3)+SKIPPED (3rd cycle)
  though tree clean — flag to human/orchestrator. smoke -ExpectSerialMarker
  never passes on SeaBIOS path — document or drop (agent-b finding 4).
