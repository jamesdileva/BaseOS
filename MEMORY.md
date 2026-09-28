# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S04 slice30 QEMU re-verify of slice29 @6975b69 tree.
  Gate 32/32 PASS, default 87 kernel 56 boot-uefi 15, 856B serial,
  guest 79360B unchanged. #6 active. #184 nits 1-3 closed slice29.
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
- Env (2026-09-28): Win11 rustc 1.98.0. Gates: default 87,
  kernel 56, boot-uefi 15, gate-test 32/32; fmt; clippy host+UEFI clean;
  slice30 QEMU re-verify PASS (guest untouched).
