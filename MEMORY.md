# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S04 slice26 @59bb2a4 APPROVED (GUARD lock literal-space +
  same-line Contains, gate 14/14). QEMU re-verified 2026-09-27: boot-uefi
  PASS 856B (GUARD 7/7 + PERM 9/9 + PERMAUDIT x2 + EXC), gate 14/14.
  #6 active. Sitrep 2-file flag stale (clean tree confirmed).
- FRAMEALLOC delta ATTRIBUTED (agent-b #75 f2): 53248B=13p/free=52877
  @7a1ce76 → 53760B=14p/free=52876 — slice6 +512B crossed page boundary;
  2 stable reruns, spans=8 faults=3 unchanged. Not a regression.
- OVMF baseline: map_size=4704 desc_size=48 ≪ 20480; budget now e2e-enforced.
- Next: user/nx report-only closeout (#147r4) first; smaller blast
  radius than live #PF injection (#168 nit4). QEMU/guest parity restored.
- Open: #147 risk4 residual (user/nx reporting-only), #139 note1
  (Reserved/MBZ), #168 nit4 (live #PF). Closed slice25/24/23/22/21/19.
- Board channel broken (human #69, D#324): mail+worklog = source of
  truth; sitrep file/check fields stale (#4 false alarm confirmed again).
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-24): Win11 rustc 1.98.0. Gates: default 44,
  kernel 13, boot-uefi 10; fmt; clippy 4x; QEMU SKIPPED (host-only).
