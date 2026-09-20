# AGENTS — BaseOS Overview

> Required by mission: overview of who does what, how we work, and where things live.

## Mission

Build an experimental performance-first desktop OS (x86-64, UEFI, Rust-first)
by following `architecture.md`, `implementation-guide.md`, and `sprint-roadmap.md`.

Core loop for every sprint: **Build → Boot → Test → Measure → Record → Decide → Continue**

A sprint is done only when: feature works, failure behavior is understood,
tests exist, performance is measured where relevant, result is reproducible,
result is documented, nothing existing was silently broken.

## Roles

| Agent | Role | Focus |
|---|---|---|
| `agent-a` (Builder) | Build and improve software in shared workspace | Implement sprint slices, keep build green, commit incrementally |
| `agent-b` (Collaborator) | Peer-review, second workstream, verification | Review builder output, own parallel/verification tasks |
| orchestrator | Coordination | Issues SITUATION REPORTs, maintains task board state |

Human is mission author. Standing directives from human are authoritative;
cite their IDs as provenance when acting on them.

## Working agreements

1. Before each work segment: check task board, read unread mail, review `MEMORY.md`.
2. Complete sprints/tasks one at a time, in order: finish → test → commit → update board → next.
3. One committable segment per cycle. If a task cannot finish in one cycle,
   commit partial progress and continue next cycle. Never hold uncommitted work.
4. Small cycles beat big ones. Prefer the smallest implementation that runs end-to-end.
5. Peer review: non-trivial kernel/architecture changes get a `REVIEW` mail before merge
   where practical; reviewer leaves findings as mail and/or task state.
6. Benchmarks: change → benchmark → record → compare → decide. Never publish only the best run.
7. Every architectural change records: Decision / Why / Alternatives /
   Expected result / Measured result / Final decision (see `worklog.md`).

## Workspace map

```text
architecture.md            Baseline architecture (what/why)
implementation-guide.md    How to build it (vertical slices, bring-up order)
sprint-roadmap.md          Ordered sprints S00–S55 + Definition of Done
AGENTS.md                  This file (who/how)
MEMORY.md                  Shared working memory (review before each segment)
worklog.md                 What we did after each sprint, and why
README.md                  Public entry point
Cargo.toml                 Cargo workspace root
rust-toolchain.toml        Pinned toolchain (reproducibility)
tools/                     Build / QEMU / benchmark helpers
research/benchmarks/       Result schema + stored reports
crates/                    OS crates (boot, kernel, framework, …) — grows per sprint
tests/                     Unit / integration / performance tests
```

## Task board protocol

- Source of truth is the orchestrator-provided task board in the SITUATION REPORT.
- Agent moves are expressed via structured output `taskMoves` (`taskId` + `state` + `owner`).
- States: `proposed` → `active` → `done`; use `blocked` with reason, `dropped` with rationale.
- Keep states current: mark `active` when starting, `done` only after verification.

## Mail protocol

- Structured output `mails` with `to`, `type` (QUESTION|IDEA|TASK|REVIEW|WARNING|DECISION|STATUS|HELP),
  `subject` (≤120 chars), `body`, `priority` 1–9.
- Builders respond to collaborator mail within one cycle.
- Use `WARNING` for broken builds / blocked progress, `DECISION` for architecture choices.

## Definition of Done (short)

From `sprint-roadmap.md`: works, failures understood, tests exist, measured,
reproducible, documented, no regressions.
