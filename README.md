# BaseOS — Experimental Performance-First Desktop OS

Target: x86-64, UEFI, Rust-first. See `architecture.md` for what/why,
`implementation-guide.md` for how, `sprint-roadmap.md` for ordered sprints.

## Current state

S00 (repository + reproducible dev environment) in progress.
See `worklog.md` for per-sprint record.

## Prerequisites (Windows 11 host)

- Rust pinned in `rust-toolchain.toml`
- QEMU (`qemu-system-x86_64`)
- Git

## Quick start

```powershell
# verify environment
powershell -ExecutionPolicy Bypass -File tools/check-env.ps1

# build workspace
cargo build

# run tests
cargo test

# QEMU smoke test (no OS image yet — verifies firmware + debug output path)
powershell -ExecutionPolicy Bypass -File tools/qemu/smoke.ps1
```

## Layout

See `AGENTS.md` → Workspace map.

## Core loop

Build → Boot → Test → Measure → Record → Decide → Continue.
A sprint is done only per Definition of Done in `sprint-roadmap.md`.
