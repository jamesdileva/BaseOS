# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: performance-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S01 UEFI boot — slices 1–6 done; slice 6 GOP framebuffer discovery
  (fill_from_gop_mode + LocateProtocol signature gate + GOP ok serial line
  + FB present=1/GOP ok=1 e2e hard-require) closes finding 4; worklog
  recorded; commit pending agent-b review of this slice.
- Env (2026-09-21): Win11 host, rustc 1.98.0, QEMU 11.1.1, git 2.53.0.
- Agreement: one committable segment/cycle; small vertical slices; board states
  current; ASCII-only in .ps1; raw UEFI offsets ST+96/BS+56 GetMemoryMap/
  BS+320 LocateProtocol efiapi; 16KiB map buf; nit3 \n endings -> S35.
- Open: #1 done; #2 -> done this cycle (GOP e2e proven) pending review;
  finding 4 closed. Tests 33/33 (31 default + 2 boot-uefi); boot PASS 517B
  FB present=1 1280x800@0x80000000 (2026-09-21). Risk 2 (BUFFER_TOO_SMALL
  retry) deferred S03.
- Learning: OVMF/QEMU GOP active mode 1280x800 pitch 5120 bpp 32 base
  0x80000000 size 0x3e8000; map regions vary 98–100 across boots (cap 128).
  Format 3 = PixelBltOnly (no LFB) must fail closed. BootInfo exact size
  3128; Copy-guard E0119; try_usable_bytes separates corrupt/empty.
