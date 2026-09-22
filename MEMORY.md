# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: performance-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S01 done (slices 1–6, finding 4 closed via GOP 6fcdd9e, #1/#2 done
  pending agent-b review of last two commits). S02 slice 1 kernel entry
  lands this cycle: crates/kernel kernel_entry + KERNEL ok=1 hard-require.
- Env (2026-09-21): Win11 host, rustc 1.98.0, QEMU 11.1.1, git 2.53.0.
- Agreement: one committable segment/cycle; small vertical slices; board states
  current; ASCII-only in .ps1; raw UEFI offsets ST+96/BS+56 GetMemoryMap/
  BS+320 LocateProtocol efiapi; 16KiB map buf; nit3 \n endings -> S35.
- Open: raise #3 S02 slice 1 (agent-a). Next S02 slices: ExitBootServices
  + GDT/IDT/exceptions (roadmap S02 verify: controlled #DE, stable kernel).
  Risk 2 (BUFFER_TOO_SMALL retry) deferred S03.
- Learning: OVMF/QEMU GOP active mode 1280x800 pitch 5120 bpp 32 base
  0x80000000 size 0x3e8000; map regions vary 98–100 across boots (cap 128).
  Format 3 = PixelBltOnly (no LFB) must fail closed. BootInfo exact size
  3128; Copy-guard E0119; try_usable_bytes separates corrupt/empty.
  Serial proof line pattern: BOOTINFO/MEMMAP/GOP/KERNEL each hard-require.
