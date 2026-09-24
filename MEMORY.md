# MEMORY — shared working memory

Review before each work segment. Keep ≤20 lines, compact.

- Goal: perf-first x86-64 UEFI desktop OS, Rust-first; docs in repo root.
- Now: S03 slice 8 landed (host-only 2000-page drain test,
  `alloc_drains_entire_map_and_restores`); gates green, committing. #5
  active (#59b host-only + S03-remaining). Board healthy per #79/#80.
  Workspace clean pre-edit, host gates re-greened (#86 gap closed).
- FRAMEALLOC delta ATTRIBUTED (agent-b #75 f2): 53248B=13p/free=52877
  @7a1ce76 → 53760B=14p/free=52876 — slice6 +512B crossed page boundary;
  2 stable reruns, spans=8 faults=3 unchanged. Not a regression.
- OVMF baseline: map_size=4704 desc_size=48 ≪ 20480; budget now e2e-enforced.
- Next: S03 remaining (multi-span stitch deferred) / peer review; QEMU
  re-verify rides with next guest-touching slice (guest untouched, PASS
  @693bb2f stands).
- Open: #59(b) host-only until real-UEFI BUFFER_TOO_SMALL (baseline is one
  OVMF config — don't generalize). Multi-span stitch deferred.
- Board channel broken (human #69, D#324): mail+worklog = source of
  truth; sitrep file/check fields stale (#4 false alarm confirmed again).
- Conventions: STRUCTURAL_FAIL = usize::MAX; ASCII .ps1; raw EFI;
  ST+96/BS+56/64/232/320 efiapi; 16KiB map+slack 4096; serial hard-req
  BOOTINFO/MEMMAP+budget/GOP/KERNEL/EBS/FRAMEALLOC/EXC=0.
- Env (2026-09-24): Win11 rustc 1.98.0. Gates: default 44,
  kernel 13, boot-uefi 10; fmt; clippy 4x; QEMU SKIPPED (host-only).
