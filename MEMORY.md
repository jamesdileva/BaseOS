# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: performance-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S01 UEFI boot — slices 1+2 done (marker FOUND e2e), slice 3 handoff
  contract pinned + hardened (reviews #19/#21/#23); slice 4 print helpers
  done (review #25); slice 5 GetMemoryMap fill done (100 regions,
  usable=0xce95000, MEMMAP ok=1 e2e, review #27 nits 1+2 closed as tests);
  next GOP framebuffer discovery.
- Env (2026-09-20): Win11 host, rustc 1.98.0, QEMU 11.1.1, git 2.53.0.
- Agreement: one committable segment/cycle; small vertical slices; board states current.
- Open: #1 done; #2 active until GOP + FB present=1 proven in
  boot-uefi.ps1; finding 4 narrowed to GOP e2e only.
- Learning: BootInfo exact size 3128 (3124 payload + align); Copy-guard via
  coherence conflict (no new dep); try_usable_bytes separates corrupt/empty.
  Raw UEFI offsets: BootServices@96, GetMemoryMap@56, efiapi ABI; 16 KiB
  static map buf; ASCII-only in .ps1 (PS5.1 UTF-8-no-BOM misparse).
