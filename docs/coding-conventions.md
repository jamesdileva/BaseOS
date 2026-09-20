# Coding Conventions (S00)

Minimal baseline. Expand only when a sprint needs it.

## Rust

- Edition 2021 (see `Cargo.toml` `[workspace.package]`); toolchain pinned in
  `rust-toolchain.toml` — do not upgrade without a `worklog.md` entry.
- `cargo fmt` must be clean before commit; `cargo clippy -- -D warnings`
  on touched crates where practical.
- Workspace lints apply (`Cargo.toml` `[workspace.lints.rust]`):
  `unsafe_code = "warn"`, `missing_docs = "warn"` — public items need doc
  comments; do not silence with `allow` without a comment explaining why.

## Unsafe boundary (per `implementation-guide.md` §3)

- `unsafe` lives in the privileged framework only (hardware access, CPU
  instructions, page tables, interrupts, MMIO).
- Every `unsafe` block documents: why unsafe is necessary, what invariant it
  depends on, what safe abstraction contains it, how it is tested.
- Assembly stays tiny, explicit, and isolated behind a safe wrapper.

## PowerShell tooling (`tools/`)

- `#Requires -Version 5.1`; scripts run on Windows PowerShell 5.1.
- Fail-loud: exit 0 on PASS, non-zero on FAIL; no false-positive passes.
- Prefer `Write-Output` + `exit 1` for asserted failures (do not rely on
  `Write-Error` under `$ErrorActionPreference = "Stop"`, which throws
  terminating before `exit` is reached).
- Paths: repo-relative defaults (`target/` for logs); never dirty the tree —
  generated records go to gitignored `target/` (tracked baselines stay clean).

## Commits and records

- One committable segment per cycle; small vertical slices end-to-end.
- Every architectural change records in `worklog.md`: Decision / Why /
  Alternatives / Expected / Measured / Final (see `implementation-guide.md` §31).
- Benchmarks: change → benchmark → record → compare → decide; never publish
  only the best run.
