# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S04 s36 correction-2 @e8570f9 (worklog-only on @a71e6a7).
  Clean 2x rebuild 84480B 23AF9FA3 vs 3BCE0EA1 (+E9E80769 incr,
  388BA13B commit) = hash-unstable/size-stable, PE timestamp.
  rustc 1.98.0 88d9e12ae LLVM22.1.8. Gates: 91/91 + 16/16 + 41/41
  + fmt/clippy -D clean + QEMU 890B viol=0. #7 done, #6 active,
  sole open #168n4 live #PF. REVIEW #219 holds #6 active pending
  b-side verify -- concur.
- FRAMEALLOC delta ATTRIBUTED (agent-b #75 f2): 53248B=13p/free=52877
  @7a1ce76 → 53760B=14p/free=52876 — slice6 +512B crossed page boundary;
  2 stable reruns, spans=8 faults=3 unchanged. Not a regression.
- OVMF baseline: map_size=4704 desc_size=48 ≪ 20480; budget now e2e-enforced.
- Next: live #PF injection (#168 nit4, sole functional open blocking
  S04 closure); b-side double-rebuild+gate+QEMU verify owed by agent-b.
- Open: #168 nit4 (live #PF) only. Closed: #139 RMBZ (s36 viol fail-closed),
  slice29/28/27/26/25/24/23/22/21/19, #7 viol-policy.
- Board: OK per human #79/#80 (resume using it; supersedes ignore-board).
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-29): Win11 rustc 1.98.0 88d9e12ae LLVM22.1.8.
  Gates: default 91, kernel 60, boot-uefi 16, gate-test 41/41;
  fmt clean; clippy host+uefi clean; QEMU PASS 890B guest 84480B viol=0.
