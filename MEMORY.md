# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S04 slice31 @789159f (pte_reserved_violation PS@L3/L0 + NX-disabled + level>3; !PRESENT exempt).
  QEMU re-verify PASS: gate 32/32, default 88, kernel 57, boot-uefi 15,
  856B serial, guest 79360B unchanged (kernel links into guest but bytes stable).
  #6 active. REVIEW #188 gaps 1-3 closed by this re-verify + sync.
- FRAMEALLOC delta ATTRIBUTED (agent-b #75 f2): 53248B=13p/free=52877
  @7a1ce76 → 53760B=14p/free=52876 — slice6 +512B crossed page boundary;
  2 stable reruns, spans=8 faults=3 unchanged. Not a regression.
- OVMF baseline: map_size=4704 desc_size=48 ≪ 20480; budget now e2e-enforced.
- Next: live #PF injection (#168 nit4) or Reserved/MBZ audit
  (#139 note1); future user/nx VALUE policy (NX/SMEP slice).
- Open: #139 note1 (Reserved/MBZ), #168 nit4 (live #PF). Closed
  slice29/28/27/26/25/24/23/22/21/19.
- Board: OK per human #79/#80 (resume using it; supersedes ignore-board).
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-28): Win11 rustc 1.98.0. Gates: default 88,
  kernel 57, boot-uefi 15, gate-test 32/32; fmt; clippy host clean (kernel);
  slice31 QEMU re-verify PASS (guest unchanged 79360B).
