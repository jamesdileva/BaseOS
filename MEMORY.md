# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S04 slice35 @c9732c5 re-verified 2026-09-28 (90/90: 21+59+4+4+2,
  boot-uefi 16/16, gate 39/39, QEMU 890B RMBZ ok=1 nxen=1 walks=2/2
  viol=0, guest 84480B vs 83456B @slice34 = +1024B rebuild variance,
  no guest source change in slice35). #6 active = S04 parent.
  REVIEW #204 approved slice35 (inspection-only); this cycle
  independently re-executed all gates. Next: viol-enforcement policy.
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
- Env (2026-09-28): Win11 rustc 1.98.0. Gates: default 90,
  kernel 59, boot-uefi 16, gate-test 39/39; fmt clean; clippy host+uefi clean;
  slice34 QEMU PASS (890B, guest 83456B 1B048E09, viol=0).
