# Worklog — what we did after each sprint, and why

Format per entry (per `implementation-guide.md` §31):

- Decision / Why / Alternatives / Expected / Measured / Final

---

## S00 — Repository and Development Environment (done)

- **Decision:** Establish reproducible foundation first: pinned toolchain,
  Cargo workspace, env-check script, QEMU smoke path, benchmark result schema,
  before any kernel code.
- **Why:** Roadmap rule — boring infrastructure first; S01+ cannot be verified
  without reproducible build/boot/measure plumbing.
- **Alternatives:** Jump straight to UEFI crate (rejected: no verified QEMU/debug
  baseline, breaks Build→Boot→Test loop).
- **Expected:** `cargo build`, `cargo test`, `tools/check-env.ps1`,
  `tools/qemu/smoke.ps1` all green on clean checkout.
- **Measured (2026-09-20, Win11):** `cargo build` OK, `cargo test` 2/2 OK
  (baseos-version), `tools/check-env.ps1` OK (rustc 1.98.0, cargo 1.98.0,
  QEMU 11.1.1, git 2.53.0), `tools/qemu/smoke.ps1` OK (QEMU boots to
  SeaBIOS/iPXE, killed after smoke window).
- **Final:** Foundation slice committed; S00 remainder (UEFI firmware files,
  debug-output capture, coding conventions doc) next cycle(s).

## S00 hardening — review #4 follow-up (2026-09-20)

- **Decision:** Harden S00 tooling per agent-b review #4 instead of starting
  S01: (a) `smoke.ps1` fails on quick QEMU exit + captures serial to
  `target/qemu-smoke-serial.log` + reports OVMF presence explicitly;
  (b) `check-env.ps1` verifies rustc/cargo against `rust-toolchain.toml`
  channel and UEFI target presence, adds `-Check` verify-only mode, writes
  record to gitignored `target/baseos-host-toolchain.txt` (tracked baseline
  untouched); (c) `Cargo.toml` members explicit (`crates/version`);
  (d) `.gitignore` covers local-only records.
- **Why:** Old smoke always exited 0 (false-positive on broken QEMU/firmware
  path); old check-env dirtied the tree and never asserted the pin it
  recorded; workspace glob would silently absorb stray dirs.
- **Alternatives:** Leave hardening for S01 (rejected: S01 UEFI boot needs a
  trustworthy fail-loud smoke + firmware probe as its baseline).
- **Expected:** `cargo build` / `cargo test` green; `check-env.ps1` and
  `-Check` green with no tree dirtying; `smoke.ps1` PASS with OVMF FOUND and
  serial log written; bogus QEMU invocation exits non-zero (proves fail-loud).
- **Measured (2026-09-20, Win11):** `cargo build` OK, `cargo test` 2/2 OK;
  `check-env.ps1` OK and `-Check` OK (rustc/cargo 1.98.0 match pin,
  `x86_64-unknown-uefi` installed); `smoke.ps1` SMOKE PASS, OVMF FOUND at
  `ucrt64/share/qemu/edk2-x86_64-code.fd`, serial log written (0 bytes, no
  guest output yet); `qemu -invalid-flag-xyz` exits 1 (old script would have
  reported SMOKE OK — false positive confirmed and now closed).
- **Final:** S00 remainder narrowed to coding-conventions doc; S01 UEFI next.

## S00 hardening — review #6 finding 1 (2026-09-20)

- **Decision:** Add `-RequireOvmf` switch to `tools/qemu/smoke.ps1`: missing
  firmware stays advisory by default, but fails fast (exit 1, before QEMU
  launch) when the flag is passed. S01 smoke/entry will pass the flag.
- **Why:** Agent-b review #6 showed `OVMF: MISSING` was report-only, so S01's
  key prerequisite was advisory — smoke could PASS without firmware.
- **Alternatives:** Make missing firmware always fatal (rejected: S00 SeaBIOS
  smoke is still useful on hosts without EDK2; opt-in assert keeps both).
- **Expected:** Firmware present + `-RequireOvmf` → PASS; firmware missing +
  `-RequireOvmf` → FAIL at OVMF assert; firmware missing without flag →
  advisory MISSING line, continues to QEMU launch attempt.
- **Measured (2026-09-20, Win11):** Present + `-RequireOvmf` → SMOKE PASS
  (exit 0, OVMF FOUND edk2-x86_64-code.fd); simulated-missing (PATH stripped,
  bogus OVMF_CODE) + `-RequireOvmf` → exit 1 at `-RequireOvmf asserted`
  before QEMU launch, vs without flag proceeds to QEMU launch; `check-env -Check`
  OK; `cargo test` 2/2 OK.
- **Final:** S01-entry risk (1) closed. Remaining for S01: serial-marker
  end-to-end proof (risk 2) + `tests/` integration harness (risk 3).

## S00 close-out — coding conventions + smoke nit (2026-09-20)

- **Decision:** Close the S00 remainder with (a) `docs/coding-conventions.md`
  baseline (Rust edition/lints, unsafe-boundary rule, PowerShell fail-loud
  style, commit/record discipline) and (b) agent-b review #8 nit fix:
  `smoke.ps1` `-RequireOvmf` missing-firmware branch now uses
  `Write-Output` + `exit 1` instead of `Write-Error` (which throws
  terminating under `$ErrorActionPreference = "Stop"` before `exit 1`).
- **Why:** S00 verification lists "establish coding conventions" with no doc
  on disk; the nit left the assert path relying on throw semantics instead
  of an explicit exit code.
- **Alternatives:** Defer doc to S01 (rejected: S00 Definition of Done
  requires it; S01 should start from a closed foundation).
- **Expected:** `cargo test` 2/2, `check-env -Check` OK, `smoke -RequireOvmf`
  PASS with OVMF FOUND; missing-firmware + `-RequireOvmf` still exits 1.
- **Measured (2026-09-20, Win11):** `cargo test` 2/2 OK; `check-env -Check`
  OK (rustc/cargo 1.98.0, `x86_64-unknown-uefi` present);
  `smoke.ps1 -RequireOvmf` SMOKE PASS exit 0, OVMF FOUND
  `edk2-x86_64-code.fd`; no `Write-Error` remains in `smoke.ps1` (exit-1
  path now explicit, semantics unchanged from prior measured exit-1).
- **Final:** S00 done (foundation + hardening + conventions). Next: S01 UEFI
  guest with serial marker + `tests/` integration harness; S01 smoke passes
  `-RequireOvmf`.

## S00 close-out — review #10 follow-up (2026-09-20)

- **Decision:** Close agent-b review #10 leftovers without starting S01:
  (a) fix S00 header `(in progress)` → `(done)`; (b) re-measure the
  `-RequireOvmf` missing-firmware path on the post-nit (`Write-Output` +
  `exit 1`) script version.
- **Why:** Review #10 accepted S00 as closed foundation but flagged the
  missing-path exit-1 claim as measured only on the pre-nit `Write-Error`
  version, plus the stale header nit. Cheapest disconfirm is a direct
  re-run, not inspection.
- **Alternatives:** Roll the re-measure into S01 (rejected: S01 must start
  from a verified assert gate; keeps S01 slice small).
- **Expected:** Present + `-RequireOvmf` → PASS exit 0, OVMF FOUND;
  simulated-missing (bogus `OVMF_CODE`, qemu hidden via stripped PATH) +
  `-RequireOvmf` → `Write-Output` MISSING + `[-RequireOvmf asserted]`,
  exit 1 before QEMU launch; same simulated-missing without flag →
  advisory MISSING, proceeds to QEMU launch attempt; no `Write-Error` in
  `smoke.ps1`.
- **Measured (2026-09-20, Win11):** `cargo test` 2/2 OK; `check-env -Check`
  OK (rustc/cargo 1.98.0, `x86_64-unknown-uefi` present); present +
  `-RequireOvmf` → SMOKE PASS exit 0, OVMF FOUND
  `ucrt64/share/qemu/edk2-x86_64-code.fd`, serial log 0 bytes (no guest
  output yet); simulated-missing + `-RequireOvmf` → MISSING +
  `[-RequireOvmf asserted]`, EXIT 1, no QEMU launch; simulated-missing
  without flag → advisory MISSING then `Start-Process` qemu-not-found
  (proves assert gate passed, launch attempted); `Write-Error` absent
  from `smoke.ps1` (only `check-env.ps1` fail-loud asserts retain it).
- **Final:** Review #10 risk (3) closed on the current script version;
  header nit fixed. S00 done. Next: S01 UEFI guest serial-marker +
  `tests/` harness (risks 1+2); S02+ blocked until S01 proves them.

## S01 slice 1 — serial-marker contract + tests/ harness + opt-in smoke assert (2026-09-20)

- **Decision:** Land S01's host-side half first as one committable slice:
  (a) new `crates/serial-marker` defining the fixed contract
  `BASEOS-S01-BOOT-MARKER` with `write_marker` (byte-callback, so the
  future UEFI guest's COM1 writer reuses it) and `contains_marker[_bytes]`
  log parsers, core-only / `no_std` outside `cfg(test)`; (b) canonical
  `tests/serial_marker.rs` harness (marker-path emission, 0-byte log parses
  as no-guest, guest capture parses as booted, smoke.ps1/marker sync check),
  executed by `cargo test` via a one-line `include!` wrapper in
  `crates/serial-marker/tests/` (virtual workspace root owns no test target);
  (c) `smoke.ps1 -ExpectSerialMarker` opt-in assert (exit 1 on absent marker).
  Real `.efi` guest + ESP/QEMU-OVMF boot is slice 2, not this commit.
- **Why:** TASK #13's full slice (UEFI guest + enforced smoke + harness) is
  too big for one safe cycle; the contract + assert gate are independently
  committable and keep every existing check green while making the 0-byte
  gap closable on demand.
- **Alternatives:** Full UEFI guest (`.efi` + FAT ESP + OVMF pflash boot)
  in one commit (rejected: registry/`uefi`-crate build, image tooling, and
  Windows QEMU pflash flags are each unproven here — high single-cycle
  risk); enforce marker unconditionally in smoke now (rejected: SeaBIOS
  path has no guest yet, would turn current PASS red with no new signal).
- **Expected:** `cargo test` 10/10 (2 version + 4 marker unit + 4 harness),
  `fmt --check` clean, `clippy -D warnings` clean, `check-env -Check` OK,
  `smoke -RequireOvmf` PASS (default, marker not asserted), `smoke
  -RequireOvmf -ExpectSerialMarker` FAIL exit 1 with explicit MISSING line.
- **Measured (2026-09-20, Win11):** `cargo test` 10/10 OK (4+4+2, 0 failed);
  `cargo fmt --check` clean (one fmt fix applied to a long assert line;
  `rustfmt`/`clippy` components added to the local pinned toolchain only,
  repo pin untouched); `cargo clippy --all-targets -- -D warnings` clean;
  `check-env -Check` OK (rustc/cargo 1.98.0, `x86_64-unknown-uefi` present);
  `smoke -RequireOvmf` SMOKE PASS exit 0, serial 0 bytes (unchanged
  default); `smoke -RequireOvmf -ExpectSerialMarker` SMOKE FAIL exit 1,
  `Serial marker: MISSING (… 0 bytes) [-ExpectSerialMarker asserted]`;
  positive-branch `.Contains` logic sanity-checked true on a marker-bearing
  string (end-to-end FOUND awaits the slice-2 guest).
- **Final:** Slice 1 committed; `tests/` gap closed (was crates-unit-only
  2/2), marker assert gate proven fail-loud. Next (slice 2): minimal UEFI
  guest writing the marker via `write_marker`, ESP image + OVMF QEMU boot,
  then flip S01 smoke to pass `-ExpectSerialMarker`.

## S01 slice 2 — UEFI guest boots under OVMF, marker FOUND end-to-end (2026-09-20)

- **Decision:** Land S01's guest half as one committable slice:
  (a) new `crates/boot-uefi` (`bootx64` bin): dependency-free `no_std`
  UEFI app (`efi_main`, no boot-services calls yet) that inits COM1
  (16550A 115200 8N1) and emits the contract via
  `baseos-serial-marker::write_marker` (single-sourced bytes), then parks
  the CPU with `hlt`; unsafe (port I/O, `hlt`) isolated behind documented
  `uart_init` / `com1_write_byte` wrappers with a crate-level
  `#![allow(unsafe_code)]` justification per `docs/coding-conventions.md`;
  (b) new `tools/qemu/boot-uefi.ps1`: builds the guest for
  `x86_64-unknown-uefi`, stages `target/esp/EFI/BOOT/BOOTX64.EFI` (QEMU
  virtual-FAT ESP — no mtools needed), boots OVMF from split pflash
  (code readonly + blank 528 KiB vars store EDK2 reformats, both
  gitignored `target/` local-only), captures COM1 to
  `target/qemu-uefi-serial.log`, and asserts marker FOUND (required, not
  opt-in); (c) `Cargo.toml` `default-members` keeps the `no_main` guest
  out of default host `cargo build/test/clippy` (checked explicitly with
  `--target x86_64-unknown-uefi`); guest test-build uses the std harness
  (`cfg_attr(not(test), no_std/no_main)`, panic handler `cfg(not(test))`);
  (d) review #15 finding 2 fix in `tests/serial_marker.rs`: temp names now
  pid+thread-id+atomic-seq (no pid+len collisions), files removed before
  asserts (cleanup on failure too). `smoke.ps1` default intentionally
  unchanged (SeaBIOS path stays green; S01 proof = `boot-uefi.ps1`).
- **Why:** Closes the review #15 BLOCKER (FOUND branch end-to-end
  unproven): the marker now travels guest-COM1 → QEMU serial file →
  required assert, reusing the slice-1 contract on both ends.
- **Alternatives:** `uefi`-crate guest with Serial I/O Protocol
  (rejected: registry/API risk, heavier; raw COM1 is sufficient for the
  serial proof and keeps the slice dependency-free); `-bios <code.fd>`
  boot (rejected: QEMU refuses the 3.5 MB image — measured "could not
  load PC BIOS" on backslash and forward-slash paths — pflash is OVMF's
  standard path); real FAT image via mtools (rejected: no Windows
  tooling; vvfat exposes the staged dir directly); enforcing the marker
  in `smoke.ps1` default (rejected: SeaBIOS path has no guest by design).
- **Expected:** `cargo test` 10/10, `fmt --check` clean, host + UEFI
  `clippy -D warnings` clean, `check-env -Check` OK, `boot-uefi.ps1`
  UEFI BOOT PASS exit 0 with marker FOUND, `smoke -RequireOvmf` still
  PASS (unchanged default).
- **Measured (2026-09-20, Win11):** `cargo test` 10/10 OK (4 unit + 4
  harness + 2 version; guest bin 0 tests); `fmt --check` clean;
  `clippy --all-targets -D warnings` clean (host) and
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings`
  clean; `check-env -Check` OK (1.98.0, uefi target present);
  `boot-uefi.ps1` PASS exit 0 on two consecutive runs (serial 333 bytes
  both, `BdsDxe: starting Boot0001 ...` then `BASEOS-S01-BOOT-MARKER`;
  guest `bootx64.efi` 4096 bytes, valid PE32+ subsystem 10/EFI_APP);
  `smoke -RequireOvmf` SMOKE PASS exit 0, serial 0 bytes (SeaBIOS path
  untouched). Debug path that proved it: manual pflash run showed OVMF
  console + marker as the serial tail.
- **Final:** S01 FOUND branch proven end-to-end; slice 2 committable.
  Next: S01 remainder (boot-info print, memory map, handoff structure
  per roadmap) or close S01 per Definition of Done review; task #1
  ready to flip to `done` once agent-b concurs the S01 verification
  bar (QEMU UEFI boot succeeds, boot info printed, memory map visible,
  failures diagnosable) is met by slices 1+2 or needs the remainder.

## S01 slice 2 hardening — review #17 follow-up (2026-09-20)

- **Decision:** Harden slice 2 per agent-b review #17 without starting
  the S01 remainder: (a) `boot-uefi.ps1` regenerates `target/ovmf-vars.fd`
  every run (delete + fresh blank 528 KiB) instead of create-once-reuse;
  (b) the `tests/serial_marker.rs` sync test now asserts the contract
  literal in BOTH `smoke.ps1` and `boot-uefi.ps1` (closes drift risk);
  (c) `SERIAL_MARKER` doc comment lists both scripts as same-commit
  update targets.
- **Why:** (1) a reused vars store carries stale NVRAM boot-order across
  runs and can mask ESP/boot-entry changes; (2) `boot-uefi.ps1` hardcodes
  the same marker literal as `smoke.ps1` but only `smoke.ps1` was covered
  by the sync test; (3)/(4) below are measured/documented, not code.
- **Alternatives:** Keep create-once vars and document the staleness
  (rejected: silent boot-order carryover is exactly the irreproducibility
  S00/S01 plumbing exists to prevent; regeneration costs one file write);
  single-script sync check (rejected: that was the drift hole).
- **Expected:** `cargo test` 10/10, `fmt --check` clean, host + UEFI
  `clippy -D warnings` clean, `boot-uefi.ps1` PASS on the regenerated-vars
  version, simulated-missing OVMF + hidden QEMU exits 1 at the OVMF assert
  before any QEMU launch, `smoke -RequireOvmf` still PASS (unchanged).
- **Measured (2026-09-20, Win11):** `cargo test` 10/10 OK (4 unit + 4
  harness incl. the extended two-script sync test + 2 version);
  `cargo fmt --check` clean; `clippy --all-targets -D warnings` clean
  (host) and `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` clean (both run explicitly — review #17 finding 4 stays
  manual until CI exists, no enforcement script added); `check-env -Check`
  OK; `boot-uefi.ps1` PASS exit 0 on two consecutive regenerated-vars
  runs (serial 333 bytes both, marker FOUND, guest 4096 bytes);
  simulated-missing (bogus `OVMF_CODE`, qemu hidden via PATH without
  msys64/qemu entries) → `OVMF: MISSING ...` exit 1 before QEMU launch
  (review #17 finding 3 OVMF-missing path now measured on the current
  script version; guest-build-fail path is the same `exit 1` assert style,
  verified by inspection only); `smoke -RequireOvmf` SMOKE PASS exit 0,
  serial 0 bytes (SeaBIOS path untouched).
- **Final:** Review #17 findings 1–3 closed on the current tree; finding 4
  (dual clippy enforcement) remains a manual two-command discipline,
  recorded here. Next: S01 remainder — memory-map print, boot-info print,
  handoff struct (+ framebuffer discovery) — one committable slice at a
  time, `boot-uefi.ps1` stays the proof vehicle; suggested first slice is
  UEFI memory-map retrieval printed to serial.

## S01 slice 3 -- boot-info handoff struct contract (2026-09-20)

- **Decision:** Land the S01-remainder handoff half as one committable slice:
  new no_std crates/boot-info (BootInfo magic/version header, inline
  MemoryRegion map up to 128 entries, optional Framebuffer descriptor,
  is_valid plus push_region plus usable_bytes plus framebuffer_opt helpers,
  saturating address math), wired into workspace members and default-members,
  with 6 host unit tests. UEFI GetMemoryMap and GOP wire-up stays for later
  slices; boot-uefi guest untouched this cycle.
- **Why:** Roadmap S01 requires memory-map retrieval plus kernel handoff
  structure; the struct contract is independently committable and testable
  on the host, keeping the guest/QEMU proof vehicle stable while the next
  slice fills the map from real boot services.
- **Alternatives:** Wire the guest to boot services in the same commit
  (rejected: raw SystemTable parsing is unproven here and doubles
  single-cycle risk); heap-allocated map with Vec (rejected: no allocator
  in early boot, inline array keeps the handoff usable pre-heap).
- **Expected:** cargo test 16/16, fmt clean, host plus UEFI clippy clean,
  boot-uefi.ps1 still UEFI BOOT PASS, smoke default untouched.
- **Measured (2026-09-20, Win11):** cargo test 16/16 OK (6 boot-info plus
  4 marker unit plus 4 harness plus 2 version); cargo fmt clean after one
  auto-format; clippy host clean and clippy UEFI guest clean;
  boot-uefi.ps1 PASS exit 0, serial 333 bytes, marker FOUND, guest 4096
  bytes (same as slice-2 baseline, guest source untouched).
- **Final:** Handoff contract pinned and tested. Next: fill BootInfo from
  UEFI GetMemoryMap (plus boot-info print to serial) and assert it in
  boot-uefi.ps1; then GOP framebuffer discovery.

## S01 slice 3 hardening — review #19 findings 1–3 (2026-09-20)

- **Decision:** Harden `crates/boot-info` per agent-b review #19 without
  starting the guest GetMemoryMap wire-up: (a) drop `Copy` from `BootInfo`
  (keep `Clone`), document pass-by-`&BootInfo` for the ~3 KB struct
  (perf-first: no per-call stack memcpy); small `MemoryRegion` /
  `Framebuffer` stay `Copy`; (b) `framebuffer_opt()` now requires
  `size != 0 && width != 0 && height != 0` in addition to the flag, so a
  set flag with an empty/degenerate descriptor returns `None`;
  (c) `memory_map()` keeps empty-on-invalid behavior but documents that
  callers MUST check `is_valid()` first (empty alone masks corruption).
  Two new tests: flag-with-empty/degenerate → `None`, Clone/`&`-passing.
  Guest untouched; finding 4 (host-only, no end-to-end fill yet) stays
  open by design — fill + serial print + GOP is the next slice.
- **Why:** Review #19 accepted the contract but flagged a 3 KB `Copy`
  handoff, a flag-only framebuffer gate, and a silent-empty map that
  masks corruption — all cheap to close before the guest depends on them.
- **Alternatives:** Wire the guest to boot services in the same commit
  (rejected: raw SystemTable parsing doubles single-cycle risk; contract
  must be right first); panic on invalid `memory_map()` (rejected: breaks
  the valid-empty case and existing callers; explicit `is_valid()` + doc
  keeps the distinction without a trap path).
- **Expected:** `cargo test` 18/18, `fmt --check` clean, host + UEFI
  `clippy -D warnings` clean, `boot-uefi.ps1` still PASS 333 B FOUND.
- **Measured (2026-09-20, Win11):** `cargo test` 18/18 OK (8 boot-info +
  4 marker unit + 4 harness + 2 version); `cargo fmt --check` clean;
  `clippy --all-targets -D warnings` clean (host) and
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings`
  clean; `boot-uefi.ps1` PASS, serial 333 bytes, marker FOUND, guest
  4096 bytes (guest source untouched, baseline unchanged).
- **Final:** Findings 1–3 closed on the current tree. Next: S01 remainder —
  fill `BootInfo` from UEFI `GetMemoryMap` + serial print, then GOP;
  task #2 stays `active` until proven in `boot-uefi.ps1`.

## S01 slice 3 hardening — review #21 residuals (2026-09-20)

- **Decision:** Close agent-b review #21 residual risks in `crates/boot-info`
  without starting the guest GetMemoryMap wire-up: (a) `framebuffer_opt()`
  now also requires `base != 0 && pitch != 0 && bpp != 0` (zero-base or
  zero-pitch/bpp with nonzero size+dims returns `None`); (b) `usable_bytes()`
  documents it returns 0 on an invalid header exactly like valid-empty, so
  callers MUST check `is_valid()` first (empty/zero alone masks corruption);
  (c) `handoff_passes_by_reference_and_clones` gains a
  `size_of::<BootInfo>() > 2048` regression guard plus a commented-out
  negative `Copy` check (`_must_not_be_copy(BootInfo::new())` must NOT
  compile) to lock the Clone-only perf-first rule. Two new tests:
  zero-base/pitch/bpp → `None`, corrupt-header masks `usable_bytes()` to 0.
  Guest untouched; finding 4 (GetMemoryMap + GOP end-to-end) stays open.
- **Why:** Review #21 concurred findings 1–3 closed in 69cb0d6 but showed a
  degenerate FB (base/pitch/bpp zero) still passes, `usable_bytes()` inherits
  the empty-on-invalid mask, and Clone-only has no guard against a silent
  `Copy` re-add — all cheap to pin before the guest depends on the contract.
- **Alternatives:** Defer to the GetMemoryMap slice (rejected: the guest
  slice should build on a locked contract, and each residual is a unit-test
  + doc change with no QEMU risk); panic/Option-return on invalid
  `usable_bytes()` (rejected: changes the helper contract mid-stream; doc +
  locked mask behavior keeps valid-empty working while warning callers).
- **Expected:** `cargo test` 20/20, `fmt --check` clean, host + UEFI
  `clippy -D warnings` clean, `boot-uefi.ps1` still PASS 333 B FOUND.
- **Measured (2026-09-20, Win11):** `cargo test` 20/20 OK (10 boot-info +
  4 marker unit + 4 harness + 2 version); `cargo fmt --check` clean
  (exit 0); `clippy --all-targets -D warnings` clean (host) and
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings`
  clean; `boot-uefi.ps1` UEFI BOOT PASS, serial 333 bytes, marker FOUND,
  guest 4096 bytes (guest source untouched, baseline unchanged).
- **Final:** Review #21 residuals (a)–(c) closed on the current tree.
  Next: S01 remainder — fill `BootInfo` from UEFI `GetMemoryMap` + serial
  print, then GOP; task #2 stays `active` until proven in `boot-uefi.ps1`.

## S01 slice 3 hardening — review #23 follow-up (2026-09-20)

- **Decision:** Close agent-b review #23 caveats in `crates/boot-info` without
  starting the guest GetMemoryMap wire-up: (a) new
  `try_usable_bytes() -> Option<u64>` (`None` on invalid header,
  `Some(n)` on valid) with `usable_bytes()` delegating via `unwrap_or(0)`,
  so new code can distinguish corrupt from valid-empty; (b) real Clone-only
  guard — module-scope coherence-conflict trait (`_BootInfoMustNotBeCopy`
  for `BootInfo` + blanket for all `T: Copy`, verified E0119 on Copy,
  clean on !Copy) plus exact `size_of::<BootInfo>() == 3128` const assert
  and unit assert (16 header + 3072 regions + 4 flag + 32 fb = 3124 payload,
  padded to 3128); (c) `framebuffer_opt()` geometry check —
  `pitch >= width*ceil(bpp/8)` and `size >= pitch*height` (saturating),
  closing the pitch*height-vs-size gap. Three tests extended/added:
  corrupt-vs-empty `try_usable_bytes`, inconsistent-geometry `None`, exact
  size. Guest untouched; finding 4 (GetMemoryMap + GOP e2e) stays open.
- **Why:** Review #23 concurred (a)–(b) closed but showed `usable_bytes`
  still doc-only, the `Copy` check still a comment, the size guard still
  loose (`> 2048` vs true 3128), and pitch*height-vs-size ungated — all
  cheap to pin before the guest depends on the contract.
- **Alternatives:** Add `static_assertions` dependency for `assert_not_impl`
  (rejected: same E0119 guarantee with zero new deps via the coherence
  trick); change `usable_bytes()` return to `Option` (rejected: breaks the
  existing helper contract; new `try_` method keeps compat while giving
  callers the real fix); defer to the GetMemoryMap slice (rejected: guest
  slice should build on a locked contract, each residual is unit-test-only
  with no QEMU risk).
- **Expected:** `cargo test` 21/21, `fmt --check` clean, host + UEFI
  `clippy -D warnings` clean, `boot-uefi.ps1` still PASS 333 B FOUND.
- **Measured (2026-09-20, Win11):** `cargo test` 21/21 OK (11 boot-info +
  4 marker unit + 4 harness + 2 version); `cargo fmt --check` clean;
  `clippy --all-targets -D warnings` clean (host, after one `div_ceil`
  fix for `manual_div_ceil`) and
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings`
  clean; `boot-uefi.ps1` UEFI BOOT PASS, serial 333 bytes, marker FOUND,
  guest 4096 bytes (guest source untouched, baseline unchanged).
- **Final:** Review #23 residuals closed on the current tree (real guards,
  no new deps). Next: S01 remainder — fill `BootInfo` from UEFI
  `GetMemoryMap` + serial print, then GOP; task #2 stays `active` until
  proven in `boot-uefi.ps1`.

## S01 slice 4 — BootInfo serial-print helpers (2026-09-20)

- **Decision:** Land the debug-print half of the S01 remainder as one
  host-testable slice in `crates/boot-info`: `write_str` / `write_u64_hex`
  / `write_u64_dec` plus `write_boot_summary` (two lines:
  `BOOTINFO valid=<0|1> regions=<n> usable=<0x...>` and
  `FB present=<0|1> base=<0x...> size=<0x...> <WxH> pitch=<n> bpp=<n>`),
  reusing the `write_marker` byte-callback sink style so the UEFI guest
  can pass its COM1 writer directly. Line 2 always prints raw geometry
  plus the gated verdict, so a `None` stays diagnosable on serial.
  Guest untouched this cycle; `GetMemoryMap`/GOP fill is the next slice.
- **Why:** Review #25 concurrence left finding 4 (GetMemoryMap+GOP e2e)
  open and warned the guest fill path must satisfy the new geometry gates
  or `framebuffer_opt` will (correctly) yield `None` — with no serial
  visibility into why. Pinning the print format + unit tests first keeps
  the guest slice small and gives it a ready-made COM1 debug print.
- **Alternatives:** Wire the guest to boot services in the same commit
  (rejected: raw SystemTable parsing doubles single-cycle risk; print
  contract must exist first); `core::fmt`/`format!` based printing
  (rejected: pulls formatting machinery into the `no_std` guest; manual
  hex/dec loops are core-only and already match the marker-callback style).
- **Expected:** `cargo test` 24/24, `fmt --check` clean, host + UEFI
  `clippy -D warnings` clean, `boot-uefi.ps1` still PASS 333 B FOUND.
- **Measured (2026-09-20, Win11):** `cargo test` 24/24 OK (14 boot-info +
  4 marker unit + 4 harness + 2 version); `cargo fmt --check` clean
  (after one auto-format); `clippy --all-targets -D warnings` clean
  (host) and `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS, serial 333 bytes,
  marker FOUND, guest 4096 bytes (guest source untouched).
- **Final:** Print contract pinned and tested (good-path + gated-None
  diagnosability locked). Next: fill `BootInfo` from UEFI `GetMemoryMap`
  + emit this summary from the guest, then GOP; task #2 stays `active`.

## S01 slice 5 — GetMemoryMap fill + guest serial summary (2026-09-20)

- **Decision:** Land the memory-map half of the S01 remainder as one
  committable slice: (a) `crates/boot-info`: `MemoryKind::from_uefi_type`
  (7→Usable, 1–4→Reclaimable, 9–10→Acpi, 5–6/11–13→Firmware, rest→Reserved),
  `BootInfo::push_efi_descriptor` and `fill_from_raw_descriptors`
  (stride parser over raw `GetMemoryMap` bytes, `no_std`-clean, stops at
  the 128-entry inline cap) plus 4 host tests (mapping, stride incl.
  48-byte firmware pad + short-stride/trailing-byte guards, corrupt-header
  summary lock, valid=0 two-line shape — closing review #27 nits 1+2 in
  the cheapest test-only form; nit 3, `\n`-only line endings, deferred:
  QEMU file capture renders fine, real COM1 terminal check rides with
  bare-metal S35); (b) `crates/boot-uefi`: dependency-free raw
  `EFI_SYSTEM_TABLE`/`EFI_BOOT_SERVICES` parsing (fixed x86-64 offsets 96
  and 56, `extern "efiapi"`, null-link guards), 16 KiB static map buffer,
  `fill_boot_info` (failure leaves a valid-but-empty `BootInfo` so serial
  stays diagnosable), then marker → `write_boot_summary` → new
  `MEMMAP ok=<0|1> status=<0x...> entries=<n>` diagnostic line on COM1;
  (c) `boot-uefi.ps1` now REQUIRES `BOOTINFO valid=1` + `MEMMAP ok=1` in
  the serial capture in addition to the marker (missing summary = FAIL).
- **Why:** Roadmap S01 verification needs "memory map is visible" and
  "failure states are diagnosable" end-to-end, not just a pinned struct;
  host-testable mapping/stride code keeps the raw-firmware guest loop thin
  while the script assert turns a silent empty map into a loud FAIL.
- **Alternatives:** `uefi`-crate guest with `BootServices::memory_map`
  (rejected: registry/API risk, heavier; raw offsets are two reads and
  already proven here); heap/`Vec` map (rejected: no allocator pre-heap,
  inline 128-cap kept); merging adjacent same-kind descriptors
  (deferred: 100 QEMU entries fit the cap, coalescing is S03-allocator
  work).
- **Expected:** `cargo test` 28/28, `fmt --check` clean, host + UEFI
  `clippy -D warnings` clean, `boot-uefi.ps1` PASS with
  `BOOTINFO valid=1` + `MEMMAP ok=1`, `smoke -RequireOvmf` still PASS.
- **Measured (2026-09-20, Win11):** `cargo test` 28/28 OK (18 boot-info +
  4 marker unit + 4 harness + 2 version); `cargo fmt --check` clean;
  `clippy --all-targets -D warnings` clean (host, after a
  `manual_range_patterns` range fix) and
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings`
  clean (after removing a dead status const + an
  `not_unsafe_ptr_arg_deref` allow on the `efi_main` FFI entry);
  `boot-uefi.ps1` UEFI BOOT PASS exit 0 on two consecutive runs (serial
  463 bytes both: marker FOUND, `BOOTINFO valid=1 regions=100
  usable=0xce95000`, `FB present=0` — no GOP yet, expected —,
  `MEMMAP ok=1 status=0x0 entries=100`; guest `bootx64.efi` 18432 bytes);
  `smoke -RequireOvmf` SMOKE PASS exit 0, serial 0 bytes (SeaBIOS path
  untouched). Incidental: `boot-uefi.ps1` edit briefly broke PS 5.1
  parsing via a non-ASCII em-dash in a UTF-8 string (script-wide parse
  errors incl. untouched lines) — replaced with ASCII `--`, re-measured
  PASS; lesson recorded (ASCII-only in `.ps1`).
- **Final:** Memory map proven end-to-end (guest → COM1 → required
  assert); review #27 nits 1+2 closed as tests. Next: GOP framebuffer
  discovery + `framebuffer_opt`/`FB present=1` in the same summary;
  task #2 stays `active` until then (S01 verification still needs
  framebuffer discovery).

## S01 slice 5 hardening — review #29 risks 1+3 (2026-09-21)

- **Decision:** Close agent-b review #29 risks 1 and 3 before starting the
  GOP slice: (a) risk 1 — `fill_from_raw_descriptors` truncation contract
  documented (caller MUST compare pushed vs total), guest now reports
  `MEMMAP ok=… entries=<pushed>/<total> truncated=<0|1>`, host test pushes
  130 fake descs asserting pushed==128 and pushed<total, and
  `boot-uefi.ps1` REQUIRES `truncated=0` (missing = FAIL); (b) risk 3 —
  `load_get_memory_map` validates `EFI_SYSTEM_TABLE.Hdr.Signature`
  (`0x5453595320494249`) before any offset deref, with a host test covering
  null table / bad sig / null BS / null GetMemoryMap slot / good links;
  (c) risk 2 — single-shot 16 KiB `GetMemoryMap` with no `BUFFER_TOO_SMALL`
  retry recorded as deferred (candidate S03) in `fill_boot_info` docs so it
  is not mistaken for a complete driver.
- **Why:** Review #29 concurred on 830eb70 but flagged silent 128-cap
  truncation (e2e would PASS truncated maps), missing header-signature check
  (stale pointer mis-parses at +96/+56), and the no-retry path — 1+3 are
  cheap to close now; 2 needs a real retry loop and belongs with S03
  allocator work.
- **Alternatives:** Emit truncated only without the host 130-desc test
  (rejected: agent-b's cheapest disconfirm was the test; script assert alone
  never exercises the cap); implement BUFFER_TOO_SMALL retry now (rejected:
  scope growth past one segment; QEMU ~100 entries fit 16 KiB, failure mode
  already fails loudly); skip signature check as documented-why (rejected:
  one unaligned read + compare is cheaper than the doc).
- **Expected:** `cargo test` 30/30 (29 default + 1 boot-uefi), `fmt --check`
  clean, host + UEFI `clippy -D warnings` clean, `boot-uefi.ps1` PASS with
  `truncated=0`, `smoke -RequireOvmf` still PASS.
- **Measured (2026-09-21, Win11):** `cargo test` 29/29 default OK
  (19 boot-info incl. truncation test + 4 marker + 4 harness + 2 version)
  plus `cargo test -p baseos-boot-uefi` 1/1 OK (signature gate);
  `cargo fmt --check` clean; `clippy --all-targets -D warnings` clean
  (host) and `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0 (serial 479
  bytes: marker FOUND, `BOOTINFO valid=1`, `MEMMAP ok=1 entries=100/100
  truncated=0`, guest `bootx64.efi` 18944 bytes); `smoke -RequireOvmf`
  SMOKE PASS exit 0.
- **Final:** Review #29 risks 1+3 closed on the current tree (6e402ec);
  risk 2 deferred-recorded (S03 candidate). Next: GOP framebuffer
  discovery + `FB present=1`; task #2 stays `active` until then.

## S01 slice 6 -- GOP framebuffer discovery (2026-09-21)

- **Decision:** Close finding 4 (the last S01-remainder item) with a
  dependency-free GOP slice: (a) `BootInfo::fill_from_gop_mode` parses raw
  `EFI_GRAPHICS_OUTPUT_MODE_INFORMATION` (36 bytes; format 0/1 -> 32 bpp,
  format 2 bitmask -> mask popcount, format 3+ BltOnly/unknown fails closed);
  (b) guest loads `LocateProtocol` (+320) through the same signature-gated
  `load_boot_services` as `GetMemoryMap`, locates GOP GUID, reads active-mode
  `Info`/`FrameBufferBase`/`FrameBufferSize`, fills the handoff; (c) serial
  adds `GOP ok=<0|1> status=<0x...>` after the MEMMAP line; (d)
  `boot-uefi.ps1` now hard-requires `FB present=1` AND `GOP ok=1` (missing
  = FAIL, per review #33/#35 fail-loud gate requirement); (e) host tests:
  2 fill_from_gop_mode (RGB parse + geometry gate; bitmask bpp + BltOnly/
  unknown fail-closed) + 1 LocateProtocol signature gate (null/bad-sig/
  null-BS/null-slot/good, mirroring the GetMemoryMap gate).
- **Why:** Task #2 stays active until GOP + `FB present=1` proven e2e;
  agent-b reviews #33/#35 both demanded the fail-loud pattern (hard-require,
  not warn-only) on the GOP path before starting it.
- **Alternatives:** uefi-rs crate for GOP (rejected: dependency-free S01
  slice, raw-offset pattern already proven); QueryMode/SetMode mode picking
  (rejected: use firmware-active mode -- no mode change needed for discovery);
  warn-only FB assert (rejected: agent-b explicit standing risk).
- **Expected:** `cargo test` 31 default + 2 boot-uefi; fmt + clippy clean
  both targets; `boot-uefi.ps1` PASS with `FB present=1`/`GOP ok=1`;
  `smoke -RequireOvmf` still PASS.
- **Measured (2026-09-21, Win11):** `cargo test` 31/31 default (21 boot-info
  incl. 2 GOP fill tests + 4 marker + 4 harness + 2 version) + `cargo test -p
  baseos-boot-uefi` 2/2 (both signature gates) = 33; `cargo fmt --check`
  clean; `clippy --all-targets -D warnings` clean (host; one
  `chunks_exact_to_as_chunks` fix) and `clippy -p baseos-boot-uefi --target
  x86_64-unknown-uefi -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS
  exit 0, serial 517 bytes: marker FOUND, `BOOTINFO valid=1 regions=98
  usable=0xce94000`, `FB present=1 base=0x80000000 size=0x3e8000 1280x800
  pitch=5120 bpp=32`, `MEMMAP ok=1 entries=98/98 truncated=0`,
  `GOP ok=1 status=0x0`, guest `bootx64.efi` 25600 bytes;
  `smoke -RequireOvmf` SMOKE PASS exit 0. Region count 98 vs prior 100 =
  OVMF map variance across boots, both well under the 128 cap.
- **Final:** Finding 4 closed: GOP framebuffer proven end-to-end with
  hard-require gates; task #2 ready to close pending agent-b review of this
  commit. Deferred risk 2 (BUFFER_TOO_SMALL retry) remains S03 candidate.

## S02 slice 1 — kernel entry handoff (2026-09-21)

- **Decision:** Open S02 with the handoff half as one committable slice:
  (a) new no_std `crates/kernel` (`baseos-kernel`, workspace
  members+default-members) exposing `kernel_entry(&BootInfo, emit)` which
  validates the S01 handoff and emits one line
  `KERNEL ok=<0|1> regions=<n> usable=<0x...> fb=<0|1>` through the same
  byte-callback sink style as the boot summary (corrupt header prints
  `ok=0` with usable/fb gated to 0 — fail-loud, never panics); (b)
  `crates/boot-uefi` calls `kernel_entry` on COM1 after the GOP line,
  still parking with `hlt` (no ExitBootServices/GDT/IDT this slice);
  (c) `boot-uefi.ps1` hard-requires `KERNEL ok=1` in the serial capture
  (missing = FAIL) and its banner now says S02 slice 1; (d) 3 host tests:
  valid map+fb exact line, corrupt-magic `ok=0` exact line, valid-empty
  exact line.
- **Why:** S01 produced a filled BootInfo but nothing ever received it —
  roadmap S01 end goal ("reach its own kernel entry point") and S02 title
  both need an actual transfer; smallest end-to-end proof is the entry
  function + serial gate before CPU-init slices.
- **Alternatives:** ExitBootServices + separate kernel image first
  (rejected: multi-risk single cycle — map-key dance, PE staging, GDT all
  unproven; entry-as-call keeps one committable slice); fold entry into
  boot-uefi main (rejected: kernel logic must live in crates/kernel to
  grow into GDT/IDT/panic slices without bloating the loader).
- **Expected:** `cargo test` 34 default (3 new kernel) + 2 boot-uefi,
  fmt + clippy clean both targets, `boot-uefi.ps1` PASS with
  `KERNEL ok=1`, `smoke -RequireOvmf` still PASS.
- **Measured (2026-09-21, Win11):** `cargo test` 34/34 default OK
  (21 boot-info + 3 kernel + 4 marker + 4 harness + 2 version);
  `cargo fmt --check` clean; `clippy --all-targets -D warnings` clean
  (host) and `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 562
  bytes: marker FOUND, BOOTINFO/MEMMAP/truncated/FB/GOP all ok,
  `KERNEL ok=1 regions=98 usable=0xce94000 fb=1`, guest `bootx64.efi`
  26624 bytes.
- **Final:** Kernel entry proven end-to-end with hard-require gate. Next:
  ExitBootServices + park-after-handoff (or GDT slice) as S02 slice 2;
  risk 2 (BUFFER_TOO_SMALL retry) stays S03; nit 3 (`\n` endings) stays
  S35.

## S02 slice 2 — ExitBootServices + GDT/IDT + controlled #DE (2026-09-21)

- **Decision:** Land the post-handoff half of S02 as one slice, fixing
  agent-b #39 REJECT blockers first: (a) ExitBootServices at raw BS+232 with
  GetMemoryMap buffer dance (16KiB, two-pass on BUFFER_TOO_SMALL only for the
  print path — first EBS attempt must succeed this slice), emit
  `EBS ok=<0|1>`; (b) after successful EBS + `cli`, install a 3-entry GDT
  (null/code/data), populate IDT vector 0 with a present interrupt gate to
  the #DE stub, `lidt`, then `div` by zero; (c) ISR stub rewritten for
  `x86_64-unknown-uefi` Microsoft x64 ABI — zero vector in `RCX` (not
  `RDI`), `and rsp,-16` + `sub rsp,32` shadow space before
  `call exception_handler_rust`, which emits `EXC ok=1 vector=0` on COM1 and
  parks; stub body lives in `de_isr_stub_asm!` shared with a host test that
  locks the ABI instructions; (d) E0793 fix: copy packed `IdtEntry` fields
  to locals before `assert_eq!`; (e) function-pointer cast via
  `as *const () as u64`; guest-only types (`Gdt`/`Idt`/`DescriptorTablePointer`
  statics) gated `cfg(not(test))` so host clippy `-D warnings` stays clean;
  (f) `boot-uefi.ps1` hard-requires `EBS ok=1`, `KERNEL ok=1` (now post-EBS),
  and `EXC ok=1 vector=0`.
- **Why:** agent-b #39 blocked the first S02-slice-2 attempt: packed-field
  refs are hard errors under `-D warnings`, the old stub put the vector in
  `RDI` (garbage `vector=266751014` under Win64 ABI), and the fn-to-int
  cast warned; keeping EBS+CPU-init+exception in one slice matches the
  roadmap verify line (controlled `#DE`, stable kernel) without splitting
  the boot script across commits.
- **Alternatives:** Split EBS and GDT/IDT into two commits (rejected: e2e
  gate needs both for `EXC` and agent-b asked for one REJECT fix cycle);
  SysV-style stub leaving `RDI` (rejected: UEFI target is Win64, not SysV);
  separate `.S` file for the stub (rejected: macro keeps one source of
  truth for guest `global_asm!` and the host ABI test).
- **Expected:** cargo test both targets green, fmt + clippy `-D warnings`
  clean host and `x86_64-unknown-uefi`, `boot-uefi.ps1` PASS with
  `EBS ok=1` + `KERNEL ok=1` + `EXC ok=1 vector=0`.
- **Measured (2026-09-21, Win11):** `cargo test` 37/37 default OK (21
  boot-info + 6 kernel incl. Win64 ABI stub test + 4 marker + 4 harness +
  2 version) + 2 doc-tests; `cargo fmt --check` clean;
  `clippy --all-targets -- -D warnings` clean; `clippy -p baseos-boot-uefi
  --target x86_64-unknown-uefi -- -D warnings` clean; `boot-uefi.ps1` UEFI
  BOOT PASS exit 0: marker FOUND, BOOTINFO/MEMMAP/truncated/FB/GOP ok,
  `EBS ok=1` first attempt, `KERNEL ok=1` post-EBS, `EXC ok=1 vector=0`,
  guest `bootx64.efi` 30720 bytes.
- **Final:** All three #39 blockers fixed and gates green; slice 2 committed
  pending agent-b review. Known-untested: stale-map-key EBS retry (EBS
  succeeded first try this boot). Risk 2 (BUFFER_TOO_SMALL retry on EBS)
  remains S03. Harness nit: prior sitrep stale dirty list still open for
  human/orchestrator.

## S03 slice 1 - diagnosability nits fold: STRUCTURAL_FAIL + clippy + smoke doc (2026-09-21)

- **Decision:** First committable slice of task #5. (a) Introduce
  `STRUCTURAL_FAIL` (`usize::MAX`, printed `0xffffffffffffffff`) so a failed
  `MEMMAP`/`GOP`/`EBS` line can never read the contradictory
  `ok=0 status=0x0` (agent-b #5 nit): raw firmware `EFI_STATUS`s pass
  through verbatim, structural failures (slot load, bogus `desc_size`,
  null GOP/`Mode`/`Info`, `fill_from_gop_mode` reject, zero map key) report
  the sentinel; (b) close agent-b clippy finding (1): three test
  `vec![0u8; N]` stands (and three new ones in this slice) -> fixed arrays,
  `useless_vec` gone under `clippy --workspace --all-targets -D warnings`
  (only finding (2), the expected host unwinding-panic on the `no_std`
  host bin, remains); (c) document `smoke.ps1 -ExpectSerialMarker` as
  SeaBIOS-path-dead by design (smoke launches no ESP disk), pointing at
  `boot-uefi.ps1` as the enforcing gate, instead of dropping the switch
  (keeps the recorded future plan of flipping smoke when it boots UEFI).
- **Why:** #5 must land nits + stale-key test + allocator; nits are the
  smallest end-to-end green segment and clear an open collaborator
  finding first. `ok=0 status=0x0` on serial is actively misleading
  during bring-up (reads as "firmware said success, result failed").
- **Alternatives:** Drop `-ExpectSerialMarker` entirely (rejected: worklog
  records a future smoke-UEFI flip and the sync test/docs would churn for
  no behavior gain); invent a fake EFI error code for structural failures
  (rejected: masquerading as firmware status is worse than a documented
  sentinel); fix MEMMAP only or GOP only (rejected: adjacent identical
  flaw would just bounce back in review).
- **Expected:** default `cargo test` 37 + `cargo test -p baseos-boot-uefi`
  5 (2 new: `fill_boot_info_failures_are_diagnosable`,
  `fill_framebuffer_failures_never_report_zero_status`); fmt clean;
  workspace clippy with only the known host-bin noise; UEFI-target clippy
  clean; `boot-uefi.ps1` PASS; `smoke -RequireOvmf` PASS unchanged.
- **Measured (2026-09-21, Win11):** `cargo test` 37/37 default OK +
  `cargo test -p baseos-boot-uefi` 5/5 OK (3 slot-gate + 2 new);
  `cargo fmt --check` clean; `cargo clippy --workspace --all-targets --
  -D warnings` reports ONLY the known `unwinding panics` host-bin error
  (agent-b finding 2, expected; `useless_vec` from finding 1 resolved);
  `cargo clippy -p baseos-boot-uefi --tests -- -D warnings` clean;
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi --
  -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS (marker FOUND,
  BOOTINFO/MEMMAP/FB/GOP/EBS/KERNEL/EXC all ok, serial shows
  `GOP ok=1 status=0x0` + `MEMMAP ok=1 status=0x0` on the success path);
  `smoke.ps1 -RequireOvmf` SMOKE PASS (0-byte SeaBIOS serial unchanged);
  guest `bootx64.efi` 30720 bytes.
- **Final:** Nits closed and committed pending agent-b review. #5 remains
  active: next slices are the stale-map-key `ExitBootServices` retry host
  test (must-have; use the new `MEMMAP_TEST_LOCK` for `MEMMAP_BUF`
  contention) and then `BUFFER_TOO_SMALL` handling + frame allocator over
  BootInfo regions with double-free/invalid-range detection. Sitrep
  harness stale dirty(3)+SKIPPED now 5th cycle - still human/orchestrator.

## S03 slice 2 - stale-map-key ExitBootServices retry host test (2026-09-21)

- **Decision:** Second committable slice of task #5: one host test
  `exit_boot_services_retries_stale_map_key_once` covering the full retry
  contract of `exit_boot_services` with a fake boot-services stand that
  populates BOTH the GetMemoryMap (+56) and ExitBootServices (+232) slots.
  Three phases: (1) zero input map key -> `(false, STRUCTURAL_FAIL)` and
  EBS never invoked; (2) first EBS returns `EFI_INVALID_PARAMETER` ->
  `refresh_map_key` runs exactly once (writes a nonzero key) and the
  second EBS call succeeds -> `(true, 0)`; (3) EBS stales twice -> fails
  closed with the raw `EFI_INVALID_PARAMETER` after exactly one refresh
  (no infinite retry). The test takes `MEMMAP_TEST_LOCK` because
  `refresh_map_key` writes the shared static `MEMMAP_BUF`.
- **Why:** The stale-key retry path shipped in S02 slice 2 was
  known-untested on the host (e2e could not force a stale key); agent-b
  review and DoD require tests for failure behavior, and this is the last
  must-have host test before the allocator slice.
- **Alternatives:** Force a stale key in QEMU e2e (rejected: firmware
  timing is nondeterministic, flaky gates are worse than none); split into
  three tests (rejected: the scripted EBS stand is one fixture, one lock
  acquisition keeps MEMMAP_BUF contention trivial).
- **Expected:** default `cargo test` 37 (new test lives in boot-uefi,
  outside default-members) + `cargo test -p baseos-boot-uefi` 6 (1 new);
  fmt clean; workspace clippy only the known host-bin unwinding noise;
  boot-uefi `--tests` and `--target x86_64-unknown-uefi` clippy clean;
  `boot-uefi.ps1` PASS; `smoke -RequireOvmf` PASS.
- **Measured (2026-09-21, Win11):** `cargo test` 37/37 default OK;
  `cargo test -p baseos-boot-uefi` 6/6 OK (3 slot-gate + 2 diagnosability
  + 1 new retry); `cargo fmt --check` clean; `cargo clippy` (default
  members) exit 0; `cargo clippy -p baseos-boot-uefi --tests -- -D
  warnings` clean; `cargo clippy -p baseos-boot-uefi --target
  x86_64-unknown-uefi -- -D warnings` clean; workspace `--all-targets`
  still only the known `unwinding panics` host-bin error (agent-b finding
  2); `boot-uefi.ps1` UEFI BOOT PASS (marker + BOOTINFO/MEMMAP/FB/GOP/EBS/
  KERNEL/EXC all ok, guest 30720 bytes); `smoke.ps1 -RequireOvmf` SMOKE
  PASS (0-byte SeaBIOS serial unchanged).
- **Final:** Stale-key retry contract now host-tested and committed
  pending agent-b review. #5 stays active: remaining slice is
  BUFFER_TOO_SMALL handling + frame allocator over BootInfo regions with
  double-free/invalid-range detection. Sitrep harness stale dirty(3)+SKIPPED
  still escalated for human/orchestrator (tree was clean this cycle).

## S03 slice 3 - BUFFER_TOO_SMALL pool dance + GOP reorder + refresh-failure asserts (2026-09-22)

- **Decision:** Third committable slice of task #5, closing review #29 risk
  2 and agent-b mail #51's three invariants. (1) `fill_boot_info` now runs
  a bounded dance: static 16 KiB first, then on `EFI_BUFFER_TOO_SMALL` an
  `AllocatePool(EfiLoaderData, required + GET_MAP_POOL_SLACK=4096)` overflow
  buffer, re-GetMemoryMap, bounded by `GET_MAP_MAX_ATTEMPTS=4` (each
  allocation perturbs the map, so a second overflow is possible); exhaustion
  or a non-BTS error returns the raw status, null/failed allocation fails
  closed (`STRUCTURAL_FAIL` / raw). One `buf` pointer threads through the
  loop, and `fill_from_raw_descriptors` reads exactly the buffer whose call
  produced the final key (invariant 2); the overflow buffer is retained
  (never FreePool'd pre-EBS). (2) `efi_main` reordered: GOP
  `LocateProtocol` (may allocate) now runs BEFORE `fill_boot_info`, so
  between the final successful GetMemoryMap and ExitBootServices nothing
  allocates — only pure port-I/O serial lines (invariant 1); print order
  (summary/MEMMAP/GOP) unchanged so e2e asserts untouched. (3) New host
  tests for invariant 3: `fill_boot_info_dances_through_pool_on_buffer_too_small`
  injects `[BUFFER_TOO_SMALL(required=16K+4K), SUCCESS]` via fake BS
  (+56 GetMemoryMap, +64 AllocatePool) and asserts AllocatePool got
  required+slack, attempt 2 ran against the pool pointer with the pool
  capacity as input, fill parsed the pool-written descriptor (pushed/total/
  key 1/1/0x7777, region fields verified); and
  `exit_boot_services_refresh_failure_fails_closed` asserts, per mode (raw
  GetMemoryMap error AND zero-key), that refresh failure fails closed with
  the original `EFI_INVALID_PARAMETER`, exactly 1 EBS call (no retry on a
  bad key) and 1 refresh call. `refresh_map_key` doc records its single-shot
  static-buffer limit (no dance there; fail-loud over key fabrication).
- **Why:** Review #51 named three invariants as the cheapest
  disconfirmation before commit: key stability (GOP order was the actual
  bug risk — map key was taken before LocateProtocol), buffer consistency
  (pool-vs-static read), and host coverage of the dance + the residual
  refresh-failure path acked in #50 (previously untested — only a print).
- **Alternatives:** Keep fill-then-GOP order and rely on the EBS stale-key
  retry (rejected: retry is a defensive fallback, not the primary path —
  taking the key after all allocating calls is strictly better and free);
  exact-size pool allocation (rejected: the AllocatePool itself can split
  a free region and grow the map, so 4 KiB slack avoids a second dance
  iteration for ~85 descriptors' worth); dance inside refresh_map_key too
  (rejected as scope growth this slice — flagged to agent-b, QEMU maps fit
  16 KiB so refresh-dance is only needed on >16 KiB bare-metal maps).
- **Expected:** default `cargo test` 37 (new tests live in boot-uefi,
  outside default-members) + `cargo test -p baseos-boot-uefi` 8 (2 new);
  fmt clean; the three clippy gates exit 0 with -D warnings;
  `boot-uefi.ps1` PASS; `smoke -RequireOvmf` PASS.
- **Measured (2026-09-22, Win11):** `cargo fmt --check` clean; default
  `cargo test` 37/37 OK (21+6+4+4+2); `cargo test -p baseos-boot-uefi`
  8/8 OK (3 slot-gate + 2 diagnosability + 1 stale-key retry + 2 new:
  dance + refresh-failure); `cargo clippy` default exit 0; `-p
  baseos-boot-uefi --tests -- -D warnings` exit 0; `-p baseos-boot-uefi
  --target x86_64-unknown-uefi -- -D warnings` exit 0;
  `boot-uefi.ps1` UEFI BOOT PASS (marker + BOOTINFO/MEMMAP/truncated=0/
  FB/GOP/EBS/KERNEL/EXC ok, guest 31232 bytes); `smoke.ps1 -RequireOvmf`
  SMOKE PASS (0-byte SeaBIOS serial unchanged).
- **Final:** Risk 2 closed and all three #51 invariants host-tested. #5
  stays active: last remaining slice is the frame allocator over BootInfo
  regions with double-free/invalid-range detection (next cycle). Sitrep
  harness stale dirty+SKIPPED and board showing only #1/#2 still escalated
  for orchestrator (done-moves flowing).

## S03 slice 4 - frame allocator over BootInfo Usable + invalid-range/double-free matrix (2026-09-22)

- **Decision:** Fourth committable slice of task #5, closing agent-b #58/#60
  test matrix. New `crates/kernel/src/frame_alloc.rs`: `FrameAllocator`
  ingests every `MemoryKind::Usable` region from `BootInfo` (sort +
  coalesce touching spans; wrap/zero page_count skipped fail-closed),
  next-fit `alloc(pages)` (no multi-span stitching this slice), and
  `free(base, pages)` with three ordered fail-closed checks: Wrap
  (`base > u64::MAX - len` or page_count byte-overflow), OutsideMap
  (range not fully inside one coalesced Usable span - catches holes,
  past-map, Reserved decoy), DoubleFree (any overlap with still-free
  memory - adjacent touch coalesces, never false-positives). Every error
  path leaves free_pages/free_span_count/cursor untouched. Public
  accessors free_pages/free_span_count/cursor/map_span_count for
  snapshot asserts. `pub mod frame_alloc` wired into baseos-kernel.
- **Why:** S03 verification requires detect double free + detect invalid
  ranges + stress; #58 matrix named the three concrete falsifiable cases
  (adjacent in-order OK / same-range double-free error; wrap and
  outside-map as separate tests; stress snapshot after fault storm).
  Standing agreement 3: previous cycles held uncommitted plan-only work -
  this lands the matrix end-to-end host-tested in one segment.
- **Alternatives:** Bitmap over physical frames (rejected: needs a bound
  on max physical address and wastes memory on sparse maps); free only
  page-granular individual bits (rejected: double-free of multi-page
  ranges and adjacent coalesce are harder to express cleanly); put the
  allocator in boot-uefi (rejected: post-EBS kernel concern, e2e guest
  stress deferred).
- **Expected:** default `cargo test` 41 (37 + 4 new frame_alloc);
  `-p baseos-kernel` 10 (6 prior + 4); `-p baseos-boot-uefi` 8
  unchanged; fmt clean; clippy 3x -D warnings exit 0; boot-uefi.ps1
  PASS; smoke PASS.
- **Measured (2026-09-22, Win11):** `cargo fmt --check` clean; default
  `cargo test` 41/41 OK (21+10+4+4+2); `-p baseos-kernel` 10/10
  (4 new: `adjacent_usable_regions_free_in_order_same_range_double_free_errors`,
  `free_rejects_wrapping_range`, `free_rejects_outside_bootinfo_map`,
  `stress_fault_injection_preserves_free_count_and_cursor_snapshot`);
  `-p baseos-boot-uefi` 8/8 OK; clippy default + boot-uefi --tests +
  boot-uefi --target x86_64-unknown-uefi + kernel --tests all exit 0
  -D warnings; `boot-uefi.ps1` UEFI BOOT PASS (guest 31232 bytes);
  `smoke.ps1 -RequireOvmf` SMOKE PASS.
- **Final:** #5 frame-allocator matrix landed (4/4 tests green). Residual
  for #5 close: guest-side alloc/free proof serial line + QEMU stress
  under S03 verification - next slice. #59(a) serial-log map_size budget
  endorsed, not yet implemented. #6 refresh_map_key remains proposed
  (owner agent-a; acceptance: host test for >16KiB refresh dance or
  documented fail-closed contract with test).

## S03 slice 5 - guest FRAMEALLOC stress line + QEMU e2e hard-require (2026-09-22)

- **Decision:** Fifth committable slice of task #5, closing the residual
  agent-b #66 guest-e2e ask. New `frame_alloc_stress(boot, emit)` in
  `crates/kernel/src/frame_alloc.rs`: 1024x single-page alloc/free sweep,
  multi-page pair (two 2-page blocks) with free count/spans fully
  restored, then three fail-closed faults (double-free of a still-free
  in-map range, wild outside-map address, wrap) each required to return
  the exact expected `FrameError` with the
  `(free_pages, free_span_count, cursor)` snapshot untouched. Emits
  `FRAMEALLOC ok=<0|1> free=<dec> spans=<n> faults=<n>`; empty Usable map
  skips the battery fail-closed (`ok=0 free=0 spans=0 faults=0`). Called
  from `efi_main` after `kernel_entry`, before bring_up (post-EBS, COM1
  only). `boot-uefi.ps1` hard-requires `FRAMEALLOC ok=1` in the Contains
  chain.
- **Why:** S03 verification residual per #59/#66: the frame-allocator
  matrix (slice 4) was host-only; the guest proof must show the same
  fail-closed behavior on the real OVMF memory map under QEMU. Cheapest
  disconfirm agent-b named: one boot-uefi.ps1 run asserting the line.
- **Alternatives:** QEMU-side stress harness poking guest memory (rejected:
  needs a debug protocol, heavy for one serial line); run the stress
  pre-EBS with boot services still live (rejected: the allocator is
  post-handoff kernel logic and must be proven on the BootInfo handoff
  itself); drop the hard-require to a soft log check (rejected: fail-loud
  agreement -- a missing/failed FRAMEALLOC line must fail the e2e).
- **Expected:** default `cargo test` 43 (41 + 2 new stress tests);
  `-p baseos-kernel` 12 (10 + 2); `-p baseos-boot-uefi` 8 unchanged;
  fmt clean; clippy 4x -D warnings exit 0; boot-uefi.ps1 PASS with
  FRAMEALLOC in the chain; smoke PASS.
- **Measured (2026-09-22, Win11):** `cargo fmt --check` clean; default
  `cargo test` 43/43 (21+12+4+4+2); `-p baseos-kernel` 12/12 (new:
  `frame_alloc_stress_healthy_map_emits_exact_ok_line` exact
  `FRAMEALLOC ok=1 free=16 spans=1 faults=3`, and
  `frame_alloc_stress_empty_map_reports_ok_zero` exact
  `FRAMEALLOC ok=0 free=0 spans=0 faults=0`); `-p baseos-boot-uefi` 8/8;
  clippy default --all-targets + boot-uefi --tests + boot-uefi --target
  x86_64-unknown-uefi + kernel --tests all exit 0 -D warnings;
  `boot-uefi.ps1` UEFI BOOT PASS (guest 53248 bytes, serial
  `FRAMEALLOC ok=1 free=52877 spans=8 faults=3` on the real OVMF map:
  98 regions, 8 coalesced Usable spans, 52877 free pages, 3/3 faults
  rejected); `smoke.ps1 -RequireOvmf` SMOKE PASS.
- **Final:** #5 guest residual closed end-to-end (host exact-line tests +
  QEMU hard-require both green); #5 peer-ACKed by agent-b REVIEW #71 at
  7a1ce76 (independent re-run, byte-identical FRAMEALLOC line) — closed.
  Next: #7/#59(a) map_size/desc_size budget serial baseline, then #6
  refresh_map_key >16KiB fail-closed host test (owner agent-a, proposed).
  Open: #59(b) host-only. Multi-span stitch deferred by design.

## S03 slice 6 — MEMMAP map_size/desc_size budget serial baseline (2026-09-22)

- **Decision:** Sixth committable slice (#7/#59(a)): `fill_boot_info` now
  returns `(…, map_size, desc_size)` from the last `GetMemoryMap` call
  (0/0 when no call ran; failure paths keep last-call sizes for
  diagnosability) and `write_memmap_line` appends
  ` map_size=<dec> desc_size=<dec>` after `truncated`. Sink is now
  generic (`emit` callback) so the exact line is host-tested. New test
  `write_memmap_line_embeds_budget_fields` locks the full line for ok=1,
  structural-fail, and truncated=1 shapes; failure-path tests assert the
  returned sizes (structural: cap 16384 + desc_size 8; raw error:
  16384/0; null table: 0/0; pool dance success: 48/48). `boot-uefi.ps1`
  hard-requires `map_size=` and `desc_size=` substrings in the PASS chain
  (fields regress → e2e fails). Existing `MEMMAP ok=1`/`truncated=0`
  substrings stay byte-identical.
- **Why:** #59(a) endorsed baseline: every boot records raw map bytes and
  descriptor stride against the 16 KiB static + 4 KiB pool-slack budget
  so #6 (refresh >16KiB fail-closed) and future buffer sizing have a
  measured OVMF reference instead of a guess. Zero-risk: append-only
  serial fields, no control-flow change.
- **Alternatives:** Separate `MAPBUDGET` line (rejected: another hard-require
  to wire for data that belongs on MEMMAP); log-only without e2e
  hard-require (rejected: fail-loud — a silent field drop would hide the
  baseline); return sizes only on success (rejected: failure-path sizes
  are free diagnosability and already flow out of the dance).
- **Expected:** default `cargo test` 43 unchanged; `-p baseos-boot-uefi`
  9 (8 + 1 new); `-p baseos-kernel` 12 unchanged; fmt clean; clippy 4x
  -D warnings; boot-uefi.ps1 PASS with map_size=/desc_size= in chain;
  smoke PASS.
- **Measured (2026-09-22, Win11):** `cargo fmt --check` clean; default
  `cargo test` 43/43 (21+12+4+4+2); `-p baseos-boot-uefi` 9/9 (new:
  `write_memmap_line_embeds_budget_fields`); `-p baseos-kernel` 12/12;
  clippy default --all-targets + boot-uefi --tests + boot-uefi --target
  x86_64-unknown-uefi + kernel --tests all exit 0 -D warnings;
  `boot-uefi.ps1` UEFI BOOT PASS (guest 53760 bytes, serial
  `MEMMAP ok=1 status=0x0 entries=98/98 truncated=0 map_size=4704
  desc_size=48` — 4704 B ≪ 16384+4096=20480 budget, 98 descriptors ×
  48 B; FRAMEALLOC ok=1 free=52876 spans=8 faults=3, EXC ok=1 vector=0);
  `smoke.ps1 -RequireOvmf` SMOKE PASS.
- **Final:** #7/#59(a) landed end-to-end (host exact-line + QEMU
  hard-require green); OVMF baseline recorded: map_size=4704,
  desc_size=48, static-fit with 15680 B headroom to 16 KiB. Next: #6
  refresh_map_key >16KiB fail-closed host test (owner agent-a,
  proposed). Open: #59(b) host-only until real-UEFI BUFFER_TOO_SMALL.

## S03 slice 7 -- #6 refresh_map_key >16KiB fail-closed host test + e2e budget value check (2026-09-22)

- **Decision:** Seventh committable slice (#6), folding agent-b REVIEW #75
  findings. New host test
  `exit_boot_services_refresh_fails_closed_when_map_outgrows_static`:
  fake BS with EBS-always-stale + GetMemoryMap refresh stand that asserts
  the single-shot refresh targets `MEMMAP_BUF` at full capacity, writes
  required `map_size = 16384+4096` (>16 KiB), and returns
  `EFI_BUFFER_TOO_SMALL`; asserts fail-closed `(false,
  EFI_INVALID_PARAMETER)` with exactly one EBS attempt and one refresh
  attempt, plus a guard that required > `MEMMAP_BUF_LEN` (proves the
  outgrown-map branch, distinct from mode 0's raw error in
  `exit_boot_services_refresh_failure_fails_closed`, per agent-b's ask).
  No guest/control-flow change (`refresh_map_key` already failed closed on
  `status != 0`; the test locks the contract). `boot-uefi.ps1` finding-1
  hardening: regex-capture `map_size`/`desc_size` off the `MEMMAP ok=1`
  line and enforce `map_size <= 20480` (16 KiB + 4 KiB slack) and
  `desc_size in [40,256]`; missing/non-numeric/out-of-range fails the
  PASS chain (presence-only checks would have let
  `map_size=999999999`/`map_size=abc` PASS). Banner -- slice 7.
- **Why:** #6 acceptance (worklog slice-4 final: host test for >16KiB
  refresh fail-closed contract) + agent-b #75 findings 1 (budget values
  not actually checked e2e) and endorsement to proceed; finding 2
  (FRAMEALLOC 1-frame delta) required attribution before leaving it.
- **Alternatives:** Drive the >16KiB refresh failure through a pool-dance
  in refresh (rejected: refresh is single-shot by design since slice 3 --
  dance would add a second allocation path post-stale-key with no
  observed need; fail-loud over silent growth); extend mode 0 with a
  BUFFER_TOO_SMALL case (rejected: agent-b explicitly asked the >16KiB
  scenario be unmistakable -- separate test with map_size guard reads
  unambiguously); budget check as a soft log line (rejected: fail-loud --
  that was finding 1); restructure ps1 into a parser module (rejected:
  scope growth for a one-regex gate).
- **Expected:** default `cargo test` 43 unchanged (boot-uefi not in
  default-members); `-p baseos-boot-uefi` 10 (9 + 1); `-p baseos-kernel`
  12 unchanged; fmt clean; clippy 4x -D warnings exit 0; boot-uefi.ps1
  PASS with `Budget check: -- ok=True`; FRAMEALLOC free=52876 stable
  across reruns (attribution: deterministic layout, not variance); smoke
  PASS.
- **Measured (2026-09-22, Win11):** `cargo fmt --check` clean; default
  `cargo test` 43/43 (21+12+4+4+2); `-p baseos-boot-uefi` 10/10 (new:
  `exit_boot_services_refresh_fails_closed_when_map_outgrows_static`);
  `-p baseos-kernel` 12/12; clippy default --all-targets + boot-uefi
  --tests + boot-uefi --target x86_64-unknown-uefi + kernel --tests all
  exit 0 -D warnings; `boot-uefi.ps1` UEFI BOOT PASS x2 consecutive runs
  (guest 53760 bytes both; `Budget check: map_size=4704 (<=20480),
  desc_size=48 ([40,256]), ok=True`; serial `MEMMAP ok=1 status=0x0
  entries=98/98 truncated=0 map_size=4704 desc_size=48`; FRAMEALLOC
  `free=52876 spans=8 faults=3` byte-identical across both runs);
  negative budget probes fail as intended (999999999->False, abc->False,
  desc_size=8->False, 4704/48->True); `smoke.ps1 -RequireOvmf` SMOKE PASS.
  Finding-2 attribution: @7a1ce76 guest was 53248 B = exactly 13 pages
  with free=52877; @da11000 guest 53760 B = 14 pages (+512 B of slice-6
  formatting code crossed the 13->14 page boundary) with free=52876 --
  the image occupies one more 4 KiB frame so Usable free drops by exactly
  1; spans=8 and faults=3 unchanged; two fresh reruns both 52876.
  Deterministic layout consequence of code growth, not OVMF variance and
  not an allocator regression.
- **Final:** #6 landed with the >16KiB BUFFER_TOO_SMALL fail-closed host
  test agent-b asked for; #59(a) e2e gate now checks budget values, not
  substrings (finding 1 closed); FRAMEALLOC delta attributed and recorded
  (finding 2 closed). Risk retained and documented: #59(b) real-UEFI
  BUFFER_TOO_SMALL still never observed -- OVMF 4704 B is one config
  only, budget headroom is not a universal guarantee. No
  PROJECT_GOAL.md added (mission lists agents.md/worklog.md only) --
   human decision if a durable goal copy is wanted. Pending peer REVIEW
   of this slice by agent-b; board moves withheld pending agent-b's board
   fix (#62/#65/#69).

## S03 slice 8 — host-only large-number drain test + skipped-verification re-green (2026-09-24)

- **Decision:** Smallest host-only S03-remaining slice (#5, #59b spirit):
  one test `alloc_drains_entire_map_and_restores` in
  `crates/kernel/src/frame_alloc.rs` — 2000-page two-span map drained one
  page at a time, exhaustion `alloc` proven `None` with snapshot untouched,
  reverse-order free restores `(2000, 2)`. No guest/script/control-flow
  change. Also re-ran the full host gates to close agent-b #86's
  two-cycles-skipped-verification risk (baseline was green before the edit
  too, so no hidden regression).
- **Why:** S03 verification needs "allocate/free large numbers of frames";
  host max was 24 pages (guest sweep reuses one frame 1024x). A full-drain
  test is the cheapest falsifiable host proof, QEMU-free per #59b host-only
  + #86 cheapest-disconfirm guidance. Guest untouched, so prior
  `boot-uefi.ps1` PASS @693bb2f stands by non-interference.
- **Alternatives:** QEMU e2e re-run this cycle (rejected: guest bytes
  unchanged, host-only slice needs no firmware proof; rerun rides with the
  next guest-touching slice); multi-span stitching alloc (rejected: scope
  growth — deferred by design, drain test first); separate stress crate
  (rejected: one test in the existing matrix file is the smallest segment).
- **Expected:** default `cargo test` 44 (43 + 1); `-p baseos-kernel` 13
  (12 + 1); `-p baseos-boot-uefi` 10 unchanged; fmt clean; clippy 4x
  `-D warnings` exit 0; QEMU skipped (host-only).
- **Measured (2026-09-24, Win11):** baseline pre-edit green (default 43,
  boot-uefi 10, fmt clean, clippy host/tests/kernel 0); post-edit default
  44/44 (21+13+4+4+2); `-p baseos-kernel` 13/13; `-p baseos-boot-uefi`
  10/10; `cargo fmt --check` clean; clippy default-all-targets +
  boot-uefi --tests + kernel --tests + boot-uefi --target
  x86_64-unknown-uefi all exit 0 `-D warnings`. QEMU `boot-uefi.ps1` /
  `smoke.ps1` SKIPPED (host-only slice; guest/script bytes untouched).
- **Final:** Large-number host proof landed; verification gap closed on the
  current tree. #5 stays active: remaining S03 work is multi-span stitch
  (deferred by design) + any peer review findings; QEMU re-verify rides
  with the next guest-touching slice.

## S03 slice 8 follow-up -- REVIEW #88 encoding + re-verify (2026-09-24)

- **Decision:** Close both agent-b REVIEW #88 findings with no code change:
  (a) replaced all 11 U+FFFD replacement chars in `worklog.md` (slice-7
  lines rewritten by the slice-8 edit: headers, trailing em-dashes,
  `PASS x2`, `13->14`, `->False/->True` probes) with ASCII equivalents,
  plus three ASCII-`?` cleanups on the same lines (`Banner -- slice 7`,
  `13->14`, `->False/->True`); file now has zero U+FFFD and stays valid
  UTF-8; (b) re-ran the full host gates reviewer asked for (default +
  `-p baseos-kernel` + `-p baseos-boot-uefi` + fmt + 4x clippy).
- **Why:** Finding 1 was a real mojibake regression from editing a file
  that mixes UTF-8 multi-byte and legacy cp1252 singletons (0x97 em-dash,
  0xD7 times) with a UTF-8 read/replace/write round-trip; finding 2 was a
  trust gap (prior gates reported, not independently re-run by reviewer).
- **Alternatives:** Convert the whole worklog to pure ASCII now (rejected:
  scope growth -- 300+ legacy non-ASCII bytes predate this fix; ASCII-only
  for touched lines closes the nit without a noisy full-file rewrite);
  QEMU re-run this cycle (rejected: guest/script bytes untouched, prior
  PASS @693bb2f stands by non-interference, same as slice 8).
- **Expected:** zero U+FFFD; default 44, kernel 13, boot-uefi 10; fmt
  clean; clippy 4x exit 0; QEMU skipped (worklog-only).
- **Measured (2026-09-24, Win11):** `worklog.md` U+FFFD count 11 -> 0;
  default `cargo test` 44/44 (21+13+4+4+2); `-p baseos-kernel` 13/13;
  `-p baseos-boot-uefi` 10/10; `cargo fmt --check` clean; clippy
  default-all-targets + boot-uefi --tests + kernel --tests +
  boot-uefi --target x86_64-unknown-uefi all exit 0 `-D warnings`.
- **Final:** REVIEW #88 findings 1+2 closed on the current tree. #5 stays
  active (multi-span stitch deferred + forward/interleaved-free variant
  noted for the stitch slice); QEMU re-verify rides with the next
  guest-touching slice. Coalescing note for #88 Q: reverse-only free
  exercises the coalesce-with-next branch on every page plus the final
  2-span restore; forward would cover coalesce-with-prev and interleaved
  the double-sided merge -- worth adding alongside stitch, not blocking.

## S03 slice 9 -- REVIEW #90 forward + interleaved coalesce coverage (2026-09-24)

- **Decision:** Close REVIEW #90 coverage gap as one host-only slice in
  `crates/kernel/src/frame_alloc.rs`: (a) `alloc_drain_free_forward_restores`
  drains the same 2000-page two-span map as slice 8 but frees in allocation
  order, exercising coalesce-with-prev on every page; (b)
  `interleaved_free_merges_both_neighbors` drains 3 contiguous pages, frees
  sides first (2 spans), then middle to prove one-insert double-sided merge
  collapses to (3, 1). No guest/script/control-flow change.
- **Why:** REVIEW #90 ACKed #89 conditional but named reverse-only as a gap
  (coalesce-with-next + final (2000,2) restore only) and required
  forward/interleaved in the stitch-slice acceptance. Cheapest disconfirm
  plus stitch acceptance precursor in one segment.
- **Alternatives:** Full multi-span stitching alloc this cycle (rejected:
  scope growth -- alloc returns one contiguous base, stitching across
  disjoint spans needs an API change; deferred by design, coverage first);
  QEMU re-run (rejected: guest/script bytes untouched, PASS @693bb2f stands
  by non-interference, same as slices 8/8-followup).
- **Expected:** default `cargo test` 46 (44 + 2); `-p baseos-kernel` 15
  (13 + 2); `-p baseos-boot-uefi` 10 unchanged; fmt clean; clippy 4x
  `-D warnings` exit 0; QEMU skipped (host-only).
- **Measured (2026-09-24, Win11):** default 46/46 (21+15+4+4+2);
  `-p baseos-kernel` 15/15 (new: forward + interleaved);
  `-p baseos-boot-uefi` 10/10; `cargo fmt --check` clean; clippy
  default-all-targets + boot-uefi --tests + kernel --tests +
  boot-uefi --target x86_64-unknown-uefi all exit 0 `-D warnings`.
  `worklog.md` U+FFFD count stays 0 (ASCII-only entry). QEMU
  `boot-uefi.ps1` / `smoke.ps1` SKIPPED (host-only; guest untouched).
- **Final:** REVIEW #90 gap closed on the current tree. #5 stays active
  (true multi-span stitch still deferred -- needs API design); QEMU
  re-verify rides with the next guest-touching slice. Evidence for #89 Q:
  `git show d822d2a --stat` is worklog-only (1 file) + U+FFFD 0.

## S03 slice 10 -- REVIEW #92 nit: ascending-order lock + multi-page double-merge (2026-09-24)

- **Decision:** Close both REVIEW #92 non-blocking nits as one host-only
  slice in `crates/kernel/src/frame_alloc.rs`: (a) forward test now
  asserts every consecutive drain base is strictly ascending and each step
  is either one PAGE_SIZE (+0x1000 inside a span) or the single span jump
  (0x5DC000 -> 0x1000000 at the 1500-page boundary), locking next-fit FIFO
  order so a future LIFO alloc cannot silently weaken the
  coalesce-with-prev coverage; (b) new
  `interleaved_free_merges_multi_page_both_neighbors` (6-page map, three
  2-page blocks, sides freed first to 2 spans / 4 pages, middle 2-page
  free collapses to (6, 1)) proving the double-sided merge for multi-page
  ranges, not just the 1-page case. No guest/script/control-flow change.
- **Why:** REVIEW #92 APPROVED slice 9 but named exactly these two
  weakenings; both are test-only, QEMU-free, and the cheapest falsifiable
  locks before the stitch-slice API design.
- **Alternatives:** Full multi-span stitching alloc this cycle (rejected:
  scope growth -- alloc returns one contiguous base, stitching across
  disjoint spans needs an API change; deferred by design, coverage first);
  QEMU re-run (rejected: guest/script bytes untouched, PASS @693bb2f stands
  by non-interference, same as slices 8/9).
- **Expected:** default `cargo test` 47 (46 + 1); `-p baseos-kernel` 16
  (15 + 1); `-p baseos-boot-uefi` 10 unchanged; fmt clean; clippy 4x
  `-D warnings` exit 0; QEMU skipped (host-only).
- **Measured (2026-09-24, Win11):** default 47/47 (21+16+4+4+2);
  `-p baseos-kernel` 16/16 (new: multi-page interleaved; forward now with
  ascending/step asserts); `-p baseos-boot-uefi` 10/10;
  `cargo fmt --check` clean; clippy default-all-targets + boot-uefi
  --tests + kernel --tests + boot-uefi --target x86_64-unknown-uefi all
  exit 0 `-D warnings`. `frame_alloc.rs` U+FFFD count 0. QEMU
  `boot-uefi.ps1` / `smoke.ps1` SKIPPED (host-only; guest untouched).
- **Final:** REVIEW #92 nits closed on the current tree. #5 stays active
  (true multi-span stitch still deferred -- needs API design); QEMU
  re-verify rides with the next guest-touching slice.

## S03 slice 11 -- stitch precursor: no-stitch contract lock (2026-09-24)

- **Decision:** Land one host-only precursor test in
  `crates/kernel/src/frame_alloc.rs` before any stitch API design:
  `alloc_requiring_multi_span_stitch_fails_closed` (two disjoint 4-page
  spans, 8 free pages total; `alloc(8)` and `alloc(5)` must return `None`
  with count/spans/cursor untouched; control `alloc(4)` still succeeds).
  No guest/script/control-flow change.
- **Why:** REVIEW #94 leaves #5 active with true multi-span stitch deferred
  pending API design. The cheapest falsifiable step is to pin the current
  contiguous-only contract so the stitch slice has a visible red-to-green
  transition and no silent behavior change.
- **Alternatives:** Design and implement scatter/gather stitch now
  (rejected: scope growth -- needs API choice like fill-a-caller-buffer
  vs iterator, plus cursor/clamp semantics; deferred by design, pin first);
  QEMU re-run (rejected: guest/script bytes untouched, PASS @693bb2f stands
  by non-interference, same as slices 8/9/10).
- **Expected:** default `cargo test` 48 (47 + 1); `-p baseos-kernel` 17
  (16 + 1); `-p baseos-boot-uefi` 10 unchanged; fmt clean; clippy 4x
  `-D warnings` exit 0; QEMU skipped (host-only).
- **Measured (2026-09-24, Win11):** default 48/48 (21+17+4+4+2);
  `-p baseos-kernel` 17/17 (new: stitch-fails-closed);
  `-p baseos-boot-uefi` 10/10; `cargo fmt --check` clean; clippy
  default-all-targets + boot-uefi --tests + kernel --tests +
  boot-uefi --target x86_64-unknown-uefi all exit 0 `-D warnings`. QEMU
  `boot-uefi.ps1` / `smoke.ps1` SKIPPED (host-only; guest untouched).
- **Final:** No-stitch contract pinned on the current tree. #5 stays active
  (stitch API design next --fill-a-caller-buffer vs iterator, cursor/clamp
  semantics); QEMU re-verify rides with the next guest-touching slice.

## S03 slice 12 -- QEMU re-verify after 4 host-only slices (2026-09-24)

- **Decision:** Re-run the full QEMU proof on the unchanged guest to close
  REVIEW #96 risk 1 (PASS @693bb2f was 4 slices stale: 8/9/10/11 all
  host-only skips by non-interference). No code change; worklog-only slice.
- **Why:** Each skip was individually justified but cumulative drift was
  untested. Cheapest disconfirm per #96 is one smoke/QEMU re-run before
  stitch lands. Stitch API design (risk 2) stays next, unblocked by this.
- **Alternatives:** Fold the re-run into the stitch slice (rejected: stitch
  needs an API choice first; a clean re-verify baseline keeps red-to-green
  visible); another host-only skip (rejected: that would make 5 stale).
- **Expected:** default 48, kernel 17, boot-uefi 10; fmt clean; clippy 4x
  exit 0; `boot-uefi.ps1` PASS with FRAMEALLOC + budget lines; smoke PASS.
- **Measured (2026-09-24, Win11):** default 48/48 (21+17+4+4+2);
  `-p baseos-kernel` 17/17; `-p baseos-boot-uefi` 10/10;
  `cargo fmt --check` clean; clippy default-all-targets + boot-uefi
  --tests + kernel --tests + boot-uefi --target x86_64-unknown-uefi all
  exit 0 `-D warnings`; `boot-uefi.ps1` UEFI BOOT PASS exit 0 (guest 53760
  bytes, serial 671 bytes: marker FOUND, BOOTINFO valid=1 regions=98
  usable=0xce8c000, FB present=1 1280x800 pitch=5120 bpp=32,
  MEMMAP ok=1 entries=98/98 truncated=0 map_size=4704 desc_size=48,
  GOP ok=1, EBS ok=1, KERNEL ok=1,
  FRAMEALLOC ok=1 free=52876 spans=8 faults=3, EXC ok=1 vector=0; Budget
  check ok=True); `smoke.ps1 -RequireOvmf` SMOKE PASS. Note:
  usable=0xce8c000 vs prior 0xce94000 is OVMF map variance (free/spans/
  faults byte-identical, regions=98 same, map_size/desc_size stable).
- **Final:** QEMU drift closed on the current tree (@dc2a1c1 + worklog).
  #5 stays active (stitch API design next -- fill-a-caller-buffer vs
  iterator, cursor/clamp semantics).

## S03 slice 13 -- stitch scatter prototype per REVIEW #100 (2026-09-24)

- **Decision:** Implement `FrameAllocator::alloc_scatter(pages, out:
  &mut [ScatterSpan]) -> Option<usize>` (single `ScatterSpan{base,
  pages}` slice, fill-caller-buffer, no_std-safe) plus two host tests on
  the 2x4-page fixture; keep `alloc` contiguous-only and keep
  `alloc_requiring_multi_span_stitch_fails_closed` as the fail-closed
  guard (not flipped green).
- **Why:** REVIEW #100 concurred fill-buffer over lazy iterator (no
  lifetimes/alloc post-EBS), rejected parallel bases+lens arrays (desync
  risk), and named cursor-clamp on the 2x4 fixture as the cheapest
  disconfirm before wiring the boot path.
- **Alternatives:** Lazy iterator scatter (rejected: lifetimes/alloc in
  EBS-exited context); dual `&mut [u64]` bases+lens arrays (rejected:
  desync risk); flipping the no-stitch guard green now (rejected: keep
  the contiguous-only contract pinned for `alloc`, scatter is additive).
- **Expected:** default 50 (48 + 2), kernel 19 (17 + 2), boot-uefi 10;
  fmt clean; clippy 4x `-D warnings` exit 0; QEMU skipped (host-only,
  guest/script untouched, PASS @22752c3 stands by non-interference).
- **Measured (2026-09-24, Win11):** default 50/50 (21+19+4+4+2);
  `-p baseos-kernel` 19/19 (new: `alloc_scatter_succeeds_across_spans`
  + `alloc_scatter_out_too_small_fails_closed`);
  `-p baseos-boot-uefi` 10/10; `cargo fmt --check` clean (after one
  auto-format); clippy default-all-targets + boot-uefi --tests +
  kernel --tests + boot-uefi --target x86_64-unknown-uefi all exit 0
  `-D warnings`. Scatter semantics locked: 5 pages on 2x4 fills
  `[(0x1000,4),(0x1000000,1)]` with partial-span cursor staying at the
  same index; exact-consume drain clamps cursor to 0; short buffer and
  over-request both return `None` with bitmap/cursor/`out` untouched.
  QEMU `boot-uefi.ps1` / `smoke.ps1` SKIPPED (host-only).
- **Final:** Stitch cursor-clamp prototype landed on the current tree.
  #5 stays active (wire scatter into guest stress/boot path or peer
  review first); contiguous `alloc` behavior unchanged.

## S03 slice 14 -- REVIEW #103 scatter edge gaps (2026-09-24)

- **Decision:** Close all three REVIEW #103 gaps as one host-only slice in
  `crates/kernel/src/frame_alloc.rs` with two tests, no API change: (a)
  `alloc_scatter_wraps_from_nonzero_cursor` (gap 1) -- pare span0 to 1 page
  with `alloc(3)`, then `alloc(2)` skips it for span1 so cursor moves to 1,
  then `alloc_scatter(3)` must fill `[(0x1002000,2),(0x4000,1)]` (span1
  remainder first, wrapping to span0) and drain to (0, 0) with cursor 0;
  (b) `alloc_scatter_zero_empty_and_tail_preserved` (gaps 2+3) --
  `alloc_scatter(0)` and empty-`out` both return `None` with state and
  buffer untouched, and an oversized 4-slot `out` for a 2-span request keeps
  `out[2..]` sentinel on success.
- **Why:** REVIEW #103 APPROVED the slice-13 prototype logic but held guest
  wiring until wrap-from-cursor, zero/empty fail-closed, and tail-slot
  preservation are host-proven; the two tests are the named cheapest
  disconfirm.
- **Alternatives:** Wire scatter into the guest stress/boot path now
  (rejected: reviewer explicitly asked for these host proofs first, and a
  second host-only skip stays within the slice-12 re-verify budget --
  guest/script bytes untouched); three separate tests (rejected: gaps 2+3
  share one fixture and snapshot, one test reads cleaner).
- **Expected:** default `cargo test` 52 (50 + 2); `-p baseos-kernel` 21
  (19 + 2); `-p baseos-boot-uefi` 10 unchanged; fmt clean; clippy 4x
  `-D warnings` exit 0; QEMU skipped (host-only).
- **Measured (2026-09-24, Win11):** default 52/52 (21+21+4+4+2);
  `-p baseos-kernel` 21/21 (new: wrap-from-cursor + zero/empty/tail);
  `-p baseos-boot-uefi` 10/10; `cargo fmt --check` clean; clippy
  default-all-targets + boot-uefi --tests + kernel --tests +
  boot-uefi --target x86_64-unknown-uefi all exit 0 `-D warnings`. QEMU
  `boot-uefi.ps1` / `smoke.ps1` SKIPPED (host-only; guest untouched, PASS
  @22752c3 stands by non-interference).
- **Final:** REVIEW #103 gaps closed on the current tree. #5 stays active
  (wire scatter into guest stress/boot path with QEMU re-verify next --
  do not stack another host-only skip); contiguous `alloc` contract +
  no-stitch guard still pinned.

## S03 slice 15 -- scatter guest wire-up + QEMU re-verify (2026-09-24)

- **Decision:** Wire `alloc_scatter` into the guest path as step 2b of
  `frame_alloc_stress` in `crates/kernel/src/frame_alloc.rs` (no script or
  serial-format change): after the multi-page pair restore, run
  `alloc_scatter(min(4, free_pages))` into a static 8-slot
  `[ScatterSpan; 8]` buffer, then free every run in fill order; freed pages
  plus `free_pages`/`free_span_count` must fully restore or `ok` clears.
  Snapshot for the fault storm is taken after 2b, so scatter cursor motion
  cannot weaken the fault assertions. New host test
  `frame_alloc_stress_two_span_map_emits_exact_ok_line` (2x4-page fixture
  expects `FRAMEALLOC ok=1 free=8 spans=2 faults=3`) proves the wired path
  host-side (need=4 fits one span, so single-span; true multi-span stitch
  stays covered by `alloc_scatter_succeeds_across_spans` etc); existing
  exact-line tests stay byte-identical.
- **Why:** REVIEW #105 APPROVED slice 14 but held that scatter was proven in
  isolation only, with two consecutive host-only skips stacking drift risk;
  cheapest disconfirm is guest integration on the real OVMF map plus a fresh
  QEMU PASS, closing both risks in one segment.
- **Alternatives:** New SCATTER serial line + script hard-require (rejected:
  scope growth -- FRAMEALLOC ok=1 already gates the battery, and 2b folds
  into the same ok bit with zero e2e churn); larger scatter request to force
  multi-span on any map (rejected: largest-span size is firmware-dependent,
  min(4, free) always fits the 8-slot buffer and still stitches when the
  cursor/fragments require it); assert spans restore only, not pages
  (rejected: pages restore is the load-bearing invariant, spans restore
  holds by insert-coalesce on all fixtures and the live map).
- **Expected:** default `cargo test` 53 (52 + 1); `-p baseos-kernel` 22
  (21 + 1); `-p baseos-boot-uefi` 10 unchanged; fmt clean; clippy 4x
  `-D warnings` exit 0; `boot-uefi.ps1` PASS with `FRAMEALLOC ok=1`;
  smoke PASS.
- **Measured (2026-09-24, Win11):** default 53/53 (21+22+4+4+2);
  `-p baseos-kernel` 22/22 (new: two-span stress exact-ok line);
  `-p baseos-boot-uefi` 10/10; `cargo fmt --check` clean; clippy
  default-all-targets + boot-uefi --tests + kernel --tests +
  boot-uefi --target x86_64-unknown-uefi all exit 0 `-D warnings` (one
  doc-lazy-continuation indent fix on the 2b doc list); `boot-uefi.ps1`
  UEFI BOOT PASS exit 0 (guest 58368 bytes, serial 671 bytes: marker FOUND,
  BOOTINFO valid=1 regions=98 usable=0xce8b000, FB present=1 1280x800
  pitch=5120 bpp=32, MEMMAP ok=1 entries=98/98 truncated=0 map_size=4704
  desc_size=48, GOP ok=1, EBS ok=1, KERNEL ok=1,
  FRAMEALLOC ok=1 free=52875 spans=8 faults=3, EXC ok=1 vector=0; Budget
  check ok=True); `smoke.ps1` not re-run this cycle (SeaBIOS path
  untouched, prior PASS stands). Attribution: guest 53760 -> 58368
  (+4608 B, crosses one 4 KiB page) so Usable free drops exactly 1
  (52876 -> 52875) and usable 0xce8c000 -> 0xce8b000 (-0x1000); spans=8
  and faults=3 unchanged; map_size/desc_size stable.
- **Final:** Scatter wire-up is now integrated guest-side and proven on the
  OVMF map as single-span `alloc_scatter` (host exact-line + QEMU
  hard-require green); true multi-span stitch stays host-only. #5 stays
  active (contiguous `alloc` no-stitch guard still pinned; stitch API
  additive only). Per REVIEW #107, 2b proves wire-up, not guest stitch.

## S03 slice 16 -- forced guest multi-span stitch (step 2c) + QEMU re-verify (2026-09-25)

- **Decision:** Close the REVIEW #107/#109 open risk (guest stitch unproven:
  2b `need=min(4,free)` fits one OVMF span) with a forced-stitch step 2c in
  `frame_alloc_stress` (`crates/kernel/src/frame_alloc.rs`, guest + host
  same path, no serial/script format change): new
  `FrameAllocator::largest_free_run()` oracle (max single free span in
  pages, 0 when empty), then 2c requests `largest + 1` into a static
  8-slot buffer and requires `filled >= 2` with page-sum == need plus full
  `free_pages`/`free_span_count` restore, else `ok` clears. Skip (stitch
  stays host-only) when < 2 free spans, total <= largest (single-span
  map), free spans > 8 (worst case exceeds the static buffer), or
  `checked_add` overflow. Two host tests: `largest_free_run_tracks_max_span`
  (2-4-2 fixture max 4, pared to 2 after `alloc(3)`, 0 when drained) and
  `frame_alloc_stress_fragmented_map_stitches_multi_span` (2-4-2 expects
  `FRAMEALLOC ok=1 free=8 spans=3 faults=3`); existing exact-line tests
  stay byte-identical (16-page single-span map skips 2c by the < 2-span
  guard; 2x4 map now exercises 2b single-span AND 2c `need=5` stitch).
- **Why:** REVIEW #109 named the cheapest next step under #5 as forcing
  multi-span on the live map (larger need sized past the biggest run) with
  restore, if the firmware map allows, else keeping the stitch claim
  host-only. `largest + 1` is the smallest need that cannot fit any one
  span on any map, so success with `filled >= 2` is a true stitch by
  construction; the skip guards keep single-span maps and oversized
  span counts fail-loud-free (ok untouched) instead of buffer-truncating.
- **Alternatives:** Log `filled`/`stitch=` in the serial line + script
  hard-require (rejected: scope growth — 4 exact-line asserts + ps1 +
  second QEMU cycle; skip-vs-ran is already decidable on this map by
  counting, see Measured); drain-to-stitch with full-map free/restore
  (rejected: heavier guest churn for the same signal; largest+1 stitches
  with a 2-span, 8-slot-bounded request); exact-size heap buffer for
  > 8 spans (rejected: no allocator post-EBS; static 8 covers the
  observed OVMF spans=8, larger maps keep the host-only claim).
- **Expected:** default `cargo test` 55 (53 + 2); `-p baseos-kernel` 24
  (22 + 2); `-p baseos-boot-uefi` 10 unchanged; fmt clean; clippy 4x
  `-D warnings` exit 0; `boot-uefi.ps1` PASS with `FRAMEALLOC ok=1`.
- **Measured (2026-09-25, Win11):** default 55/55 (21+24+4+4+2);
  `-p baseos-kernel` 24/24 (new: largest oracle + fragmented stitch);
  `-p baseos-boot-uefi` 10/10; `cargo fmt --check` clean (one auto-format);
  clippy default-all-targets + boot-uefi --tests + kernel --tests +
  boot-uefi --target x86_64-unknown-uefi all exit 0 `-D warnings`;
  `boot-uefi.ps1` UEFI BOOT PASS exit 0 (guest 60928 bytes, serial 671
  bytes: marker FOUND, `BOOTINFO valid=1 regions=98 usable=0xce8b000`,
  `FB present=1 1280x800 pitch=5120 bpp=32`,
  `MEMMAP ok=1 entries=98/98 truncated=0 map_size=4704 desc_size=48`,
  `GOP ok=1`, `EBS ok=1`, `KERNEL ok=1`,
  `FRAMEALLOC ok=1 free=52875 spans=8 faults=3`, `EXC ok=1 vector=0`;
  Budget ok=True). Attribution: guest 58368 -> 60928 (+2560 B, no page
  cross) so usable/free/spans/faults/map_size/desc_size all identical to
  slice-15 baseline. Stitch-ran argument: 8 non-empty spans with total
  52875 implies total > largest (at least 7 other pages), spans 8 <= 8,
  so no 2c skip guard can fire on this map — `ok=1` means the
  `filled >= 2` stitch succeeded and restored. `smoke.ps1` not re-run
  (SeaBIOS path untouched, prior PASS stands).
- **Final:** Guest multi-span stitch now proven on the OVMF map (host
  fragmented exact-line + QEMU hard-require green with skip impossible
  by counting). #5 stays active (contiguous `alloc` no-stitch guard still
  pinned; stitch API additive only). Open: serial-line stitch observability
  (log `filled`) remains a possible follow-up so future maps distinguish
  ran-vs-skipped on serial alone.

## S03 slice 17 -- stitch observability (`stitch=` serial field) + QEMU proof (2026-09-25)

- **Decision:** Close REVIEW #111 residual risk 1 (ran-vs-skipped inferred
  by counting, not logged) as one guest-touching slice in
  `crates/kernel/src/frame_alloc.rs`: `frame_alloc_stress` tracks
  `stitch` (step-2c `filled`, `0` when 2c skips by its guards) and the
  serial line becomes
  `FRAMEALLOC ok=<0|1> free=<n> spans=<n> faults=<n> stitch=<n>`.
  `Some(filled)` with `filled >= 2` sets `stitch = filled`; all skip
  guards and the empty-map path leave `0` (unambiguous: ran implies
  `>= 2`). Four exact-line host asserts updated (`stitch=0` on 16-page
  single-span + empty; `stitch=2` on 2x4 two-span and 2-4-2 fragmented);
  no script change (`boot-uefi.ps1` hard-require stays substring
  `FRAMEALLOC ok=1`).
- **Why:** Agent-b REVIEW #111 APPROVED slice 16 but accepted the
  proposed `filled`/`stitch=` log follow-up so future maps (different
  span counts/sizes) distinguish ran-vs-skipped on serial alone. Cheapest
  disconfirm is the field + host exact-lines + one QEMU PASS showing a
  nonzero stitch on the live map.
- **Alternatives:** Keep counting-based inference with no serial change
  (rejected: reviewer-accepted follow-up, and exotic maps could sit
  exactly on a skip guard); separate `STITCH` line + script hard-require
  (rejected: scope growth -- same `ok` bit already gates, one appended
  field is zero e2e churn); `stitch=1` for skipped (rejected: `0`
  reads as none-ran and keeps `>= 2 means stitched` crisp).
- **Expected:** default `cargo test` 55 (21+24+4+4+2, same count, 4
  exact-lines updated); `-p baseos-kernel` 24; `-p baseos-boot-uefi` 10;
  fmt clean; clippy 4x `-D warnings` exit 0; `boot-uefi.ps1` PASS with
  `FRAMEALLOC ok=1 ... stitch>=2`.
- **Measured (2026-09-25, Win11):** default 55/55;
  `-p baseos-kernel` 24/24; `-p baseos-boot-uefi` 10/10;
  `cargo fmt --check` clean; clippy default-all-targets + kernel --tests
  + boot-uefi --tests + boot-uefi --target x86_64-unknown-uefi all exit 0
  `-D warnings`; `boot-uefi.ps1` UEFI BOOT PASS exit 0 (guest 60928
  bytes, serial 680 bytes = 671 + 9 for ` stitch=5`: marker FOUND,
  `BOOTINFO valid=1 regions=98 usable=0xce8b000`,
  `FB present=1 1280x800 pitch=5120 bpp=32`,
  `MEMMAP ok=1 entries=98/98 truncated=0 map_size=4704 desc_size=48`,
  `GOP ok=1`, `EBS ok=1`, `KERNEL ok=1`,
  `FRAMEALLOC ok=1 free=52875 spans=8 faults=3 stitch=5`,
  `EXC ok=1 vector=0`; Budget ok=True). Attribution: guest size
  identical to slice-16 (60928), free/spans/faults/map_size/desc_size
  identical; `stitch=5` (>= 2) is direct serial proof the 2c stitch ran
  on the OVMF map (no counting argument needed). `smoke.ps1` not re-run
  (SeaBIOS path untouched, prior PASS stands).
- **Final:** Stitch observability landed; REVIEW #111 follow-up closed on
  the current tree. #5 stays active (contiguous `alloc` no-stitch guard
  still pinned; stitch API additive only).

## S04 slice 1 -- stage-1 page-table model: map/unmap + perms + guard + fault tests (2026-09-25)

- **Decision:** Open S04 (#6) with a host-only, `no_std`-clean software
  model in `crates/kernel/src/page_table.rs` (`pub mod page_table`):
  fixed-capacity (`MAX_MAPPINGS=64`, sorted linear scan, no allocator,
  no `unsafe`) 4 KiB page map with `PageFlags`
  (PRESENT/WRITABLE/USER/NO_EXEC/GUARD), `map`/`unmap`/`translate`, and
  `check(virt, AccessKind::{Read,Write,Execute}, is_user)` reporting
  `Fault::{NotPresent,ReadOnly,NoExec,UserViolation,Guard}` (Guard first,
  then NotPresent, User, RWX). Six host tests: roundtrip+offset,
  double/misaligned/bad-flags fail-closed, full-64 TableFull+drain,
  R/W/X+user matrix, guard-faults-all-kinds, fill/half-drain/remap churn.
  Guest untouched (no QEMU re-run this slice).
- **Why:** TASK #116 asks for the minimal S04 slice with map/unmap +
  perms + fault tests while keeping gates green; a testable permission
  contract must exist before any hardware PTE walk/`CR3` slice, per
  implementation-guide §7 (safe abstraction first, rest of kernel goes
  through it).
- **Alternatives:** Full 4-level radix with static node pool + `CR3` load
  now (rejected: needs guest asm + QEMU proof in the same cycle, doubles
  single-cycle risk; model pins the fault contract the hardware slice
  must satisfy); ad-hoc `BTreeMap`/`Vec` map (rejected: no allocator
  post-EBS; fixed array keeps the guest path usable).
- **Expected:** default `cargo test` 61 (55 + 6); `-p baseos-kernel` 30
  (24 + 6); `-p baseos-boot-uefi` 10; fmt clean; clippy host + UEFI
  `-D warnings` exit 0; no QEMU (guest untouched).
- **Measured (2026-09-25, Win11):** default 61/61
  (21+30+4+4+2); `-p baseos-kernel` 30/30 (6 new page_table);
  `-p baseos-boot-uefi` 10/10; `cargo fmt --check` clean;
  `cargo clippy --all-targets -D warnings` exit 0;
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` exit 0. Two test bugs caught red-to-green before commit:
  `unmap(0x9999)` asserted NotMapped but is Misaligned (fixed to
  `0x9000`); `rx` fixture lacked `NO_EXEC` so Execute returned Ok
  (fixed to `PRESENT|USER|NO_EXEC`). QEMU not re-run (guest sources
  untouched; S03 slice-17 PASS `stitch=5` stands).
- **Final:** S04 permission contract landed host-green; #6 active. Next:
  hardware PTE walk / guest wiring + e2e fault-line asserts, then kernel
  vs user mapping foundations.

## S04 slice 2 -- HW PTE encode/index/CR3 helpers + REVIEW #120 risk close-out (2026-09-25)

- **Decision:** Host-only, `no_std`-clean encode slice in
  `crates/kernel/src/page_table.rs` (no guest wiring, no QEMU this cycle):
  `pte_encode` (PRESENT->0/WRITABLE->1/USER->2/NO_EXEC->63, `GUARD`/bit 9
  always masked to 0, Accessed/Dirty load 0, low-12 phys masked),
  `pte_decoded_phys`/`pte_decoded_flags` (GUARD never reconstructed),
  `page_table_index(virt, level)` (3=PML4/39, 2=30, 1=21, 0=PT/12,
  `None` for level>3), `cr3_encode` (`None` if misaligned, low-12 controls
  0)/`cr3_decode`, plus `PageFlags::from_bits` for the decode path. Docs
  pin: phys aliasing intended (shared/COW, only virt-dup rejected),
  `translate` raw/`check` gate with no non-test kernel `translate(`
  callers (searched `crates/`), O(n) model must not carry to the HW walker,
  guest must `invlpg`/reload `CR3`, never branch on Accessed/Dirty. Five
  host tests: guard-mask, PTE roundtrip, 4-level index, CR3 roundtrip,
  phys-alias sharing.
- **Why:** REVIEW #120 accepted slice 1 and named 4 slice-2 risks with
  cheapest disconfirming experiments; this slice runs all four without
  touching the guest: (1) translate-bypass search = tests-only, doc-gated;
  (2) bit-9 masking pinned by failing-first test; (3) aliasing documented +
  tested as sharing; (4) scale/TLB/A-D scope note for the guest slice.
- **Alternatives:** Full guest walker + `CR3` load + QEMU e2e now (rejected:
  doubles single-cycle risk; encode contract must exist first so the guest
  slice is wiring-only); rename `translate` to `translate_raw` now
  (rejected: churn with zero non-test callers; doc gate suffices until the
  guest walker lands).
- **Expected:** default `cargo test` 67 (62 + 5); `-p baseos-kernel` 36
  (31 + 5); `-p baseos-boot-uefi` 10; fmt clean; clippy host + UEFI
  `-D warnings` exit 0; no QEMU (guest untouched).
- **Measured (2026-09-25, Win11):** default 67/67
  (21+36+4+4+2); `-p baseos-kernel` 36/36 (5 new HW PTE);
  `-p baseos-boot-uefi` 10/10; `cargo fmt --check` clean;
  `cargo clippy --all-targets -- -D warnings` exit 0;
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -- -D warnings` exit 0. One test bug caught red-to-green before commit:
  index spot-check hard-coded PML4=0xFF, true value for
  `0x00FF_8765_4321_ABCD` is 0x10E (verified via python3 shift; fixed to
  0x10E/0x195/0x19/0x1A). QEMU not re-run (guest sources untouched; S03
  slice-17 PASS stands).
- **Final:** HW PTE encode contract landed host-green; #6 active. Next:
  guest walker + `CR3` load + QEMU boot with e2e fault-line asserts, then
  kernel vs user mapping foundations.

## S04 slice 3 -- REVIEW #122 hardening: trusted from_bits + checked PTE encode (2026-09-25)

- **Decision:** Close both REVIEW #122 hardening gaps host-only in
  `crates/kernel/src/page_table.rs` (guest untouched, no QEMU this cycle):
  (a) finding 1 -- `PageFlags::from_bits` now masks to new
  `TRUSTED_FLAGS_MASK` (PRESENT/WRITABLE/USER/NO_EXEC/GUARD only), so raw
  `u64` callers cannot forge Accessed/Dirty or reserved bits (GUARD stays
  forgeable by construction as trusted OS state, documented); (b) finding 2
  -- new `pte_encode_checked(phys, flags) -> Option<u64>` (`None` on
  misaligned, mirroring `cr3_encode`), with `pte_encode` keeping its pinned
  masking form for load-compat and docs pointing at the checked caller;
  (c) minor -- `page_table_index` out-of-range test now covers 4/5/100/255
  (not just 4). Two new tests: `review_122_from_bits_masks_untrusted_bits`
  (MAX forges GUARD but equals the trusted mask; A-D/reserved cleared; raw
  PTE bit 9 still decodes without GUARD) and
  `review_122_pte_encode_checked_fails_closed_on_misalign` (None on 0x1001,
  Some == unchecked on aligned, truncation still pinned).
- **Why:** REVIEW #122 ACCEPTED slice 2 @d43c47e but named exactly these two
  non-blocking gaps for the next slice, with cheapest disconfirms
  (`from_bits(MAX).contains(GUARD)` assert; checked-encode vs silent mask);
  canonical-address validation stays deferred to the walker slice per the
  review (#GP vs #PF decision).
- **Alternatives:** Make `from_bits` private/rename to trusted-only
  (rejected: decode path needs a `const` rebuild and the mask gives the
  same forgery bound with zero API churn); change `pte_encode` itself to
  return `Option` (rejected: breaks the load-compat truncation contract
  its test pins; additive checked form keeps both); full guest walker now
  (rejected: doubles single-cycle risk; hardening must land first so the
  walker builds on locked encode semantics).
- **Expected:** default `cargo test` 69 (67 + 2); `-p baseos-kernel` 38
  (36 + 2); `-p baseos-boot-uefi` 10; fmt clean; clippy host + UEFI
  `-D warnings` exit 0; no QEMU (guest untouched).
- **Measured (2026-09-25, Win11):** default 69/69 (21+38+4+4+2);
  `-p baseos-kernel` 38/38 (2 new review_122);
  `-p baseos-boot-uefi` 10/10; `cargo fmt --check` clean (one auto-format
  of a long assert line); `cargo clippy --all-targets -- -D warnings`
  exit 0; `cargo clippy -p baseos-boot-uefi --target
  x86_64-unknown-uefi -- -D warnings` exit 0. QEMU not re-run (guest
  sources untouched; S03 slice-17 PASS stands).
- **Final:** REVIEW #122 gaps closed on the current tree; #6 stays active.
  Next: guest walker + `CR3` load + e2e fault-line asserts (with the
  canonical-address #GP-vs-#PF decision), then kernel vs user mappings.

## S04 slice 4 -- walker verbs + canonical #GP-vs-#PF (REVIEW #124 risk 1) (2026-09-25)

- **Decision:** Host-only walker-contract slice in
  `crates/kernel/src/page_table.rs` (guest untouched, no QEMU this cycle):
  (a) `is_canonical(virt)` (bit-47 sign-extension into 48..=63,
  `const`-compatible for the guest walker); (b) `CpuFault::{GeneralProtection,
  PageFault(Fault)}` + `classify_access(pt, virt, kind, is_user)` (canonical
  gate first, then `check`, so non-canonical is `#GP` and everything else
  is `#PF`); (c) `WalkerError::{MisalignedPhys, NonCanonical, NotPresent,
  InvalidLevel}` + `encode_table_entry(phys, flags)` (fail-closed wrapper
  over `pte_encode_checked` -- the single walker entry point, closing
  REVIEW #124 risk 1: checked form with zero callers) + `walk_step(table:
  &[u64; 512], virt, level)` (canonical -> level -> PRESENT order,
  returns `pte_decoded_phys`). Docs pin the dual-encode rule (walker code
  must call `encode_table_entry`, unchecked `pte_encode` is load-compat
  only). Three new tests: checked-form wiring + GUARD mask, indexed-PT
  step incl. high-canonical, GP-vs-PF matrix + canonical boundaries.
- **Why:** REVIEW #124 ACCEPTED slice 3 @1e7098d but required the walker to
  wire the checked form or the fix stays latent, with `grep walker for
  pte_encode(` as the cheapest disconfirm; canonical #GP-vs-#PF was the
  deferred decision from #122. This slice lands both host-side so the
  guest `CR3`-load + e2e fault-assert slice is wiring-only.
- **Alternatives:** Guest `CR3` load + QEMU fault-line asserts now (rejected:
  doubles single-cycle risk; walker verbs + canonical contract must exist
  first); change `pte_encode` to return `Option` (rejected: same as slice
  3 -- breaks the pinned truncation contract; additive walker path keeps
  both); `debug_assert` in `pte_encode` (rejected: fires on the pinned
  truncation test; doc rule + single walker entry point is the lighter
  gate until the guest lands).
- **Expected:** default `cargo test` 72 (69 + 3); `-p baseos-kernel` 41
  (38 + 3); `-p baseos-boot-uefi` 10; fmt clean; clippy host + UEFI
  `-D warnings` exit 0; no QEMU (guest untouched).
- **Measured (2026-09-25, Win11):** default 72/72 (21+41+4+4+2);
  `-p baseos-kernel` 41/41 (3 new walker/canonical);
  `-p baseos-boot-uefi` 10/10; `cargo fmt --check` clean;
  `cargo clippy --all-targets -- -D warnings` exit 0 (one
  `double_must_use` fix: dropped `#[must_use]` on `encode_table_entry`,
  `Result` is already must-use);
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -- -D warnings` exit 0. `git diff --stat` is `page_table.rs` only
  (guest untouched; S03 slice-17 PASS stands). QEMU not re-run.
- **Final:** Walker verbs + canonical contract landed host-green; REVIEW
  #124 risk 1 closed (walker path calls `pte_encode_checked` via
  `encode_table_entry`). #6 stays active. Next: guest `CR3` load + QEMU
  boot with e2e fault-line asserts, then kernel vs user mappings.

## S04 slice 5 -- chained 4-level walk + kernel/user boundary (REVIEW #126 risks 1+2) (2026-09-25)

- **Decision:** Host-only pre-guest slice in
  `crates/kernel/src/page_table.rs` (guest untouched, no QEMU this cycle):
  (a) walk_step docs record the reserved/MBZ position (REVIEW #126 risk 1)
  -- only PRESENT is checked, reserved-bit rejection deferred until the
  guest CR3 mode (NX/1 GiB pages) is pinned -- plus the next-level lookup
  contract (REVIEW #126 risk 3): the returned base is physical, the guest
  maps it to the next &[u64; 512] via its phys-to-table mapping (unsafe,
  guest-only); (b) new `walk_chain_four_levels_with_offset_and_mid_notpresent`
  (REVIEW #126 risk 2 cheapest disconfirm): four chained walk_step calls
  resolve 0x5000 from 0x123456789ABC with +0xABC offset to 0x5ABC, then a
  cleared PDPT entry fails NotPresent while PML4 still resolves; (c) new
  `kernel_vs_user_boundary_enforced`: supervisor page vs USER page, both
  rings, all kinds, plus the same split through classify_access (#PF
  UserViolation vs #GP on non-canonical).
- **Why:** REVIEW #126 ACCEPTED slice 4 @47b36f6 but named exactly these
  three open risks for the guest-CR3/e2e slice; chaining + boundary + docs
  are the host-provable part, leaving the guest slice as wiring-only (CR3
  load + e2e fault asserts).
- **Alternatives:** Guest CR3 load + QEMU fault-line asserts now (rejected:
  doubles single-cycle risk; chain/boundary contract must exist first);
  reserved-bit rejection now (rejected: mode-dependent, no guest CR3 mode
  pinned yet -- documented as out-of-scope instead).
- **Expected:** default `cargo test` 74 (72 + 2); `-p baseos-kernel` 43
  (41 + 2); `-p baseos-boot-uefi` 10; fmt clean; clippy host + UEFI
  `-D warnings` exit 0; no QEMU (guest untouched).
- **Measured (2026-09-25, Win11):** default 74/74 (21+43+4+4+2);
  `-p baseos-kernel` 43/43 (2 new chain/boundary);
  `-p baseos-boot-uefi` 10/10; `cargo fmt --check` clean (after one
  auto-format); `cargo clippy --all-targets -- -D warnings` exit 0 (one
  unused-variable fix: dropped the unused PML4 base binding);
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -- -D warnings` exit 0. `git diff --stat` is `page_table.rs` only
  (guest untouched; S03 slice-17 PASS stands). QEMU not re-run.
- **Final:** REVIEW #126 risks 1 (documented) + 2 (chained test) closed
  host-side; risk 3 contracted for the guest slice. #6 stays active.
  Next: guest CR3 load + QEMU boot with e2e fault-line asserts.

## S04 slice 6 -- walk_full CR3-rooted walk via lookup closure (2026-09-26)

- **Decision:** Host-only pre-guest slice in
  `crates/kernel/src/page_table.rs` (guest untouched, no QEMU this cycle):
  new `walk_full(cr3_base, virt, lookup)` chaining four `walk_step` calls
  from a CR3 PML4 base through `lookup(phys_base)` returning the next table
  (the host closure plays the guest phys_to_table_ptr role; the PML4 itself
  also resolves via `lookup` so host and guest share one shape). Fail-closed:
  misaligned CR3 gives MisalignedPhys, non-canonical gives NonCanonical,
  lookup miss or cleared-PRESENT mid-level gives NotPresent; returns page
  base plus offset. Docs record the reserved/MBZ policy as unchanged (only
  PRESENT checked; PS/NX combos deferred until the guest CR3 mode is pinned).
  One new test covers happy-path offset, mid-NotPresent, missing-table,
  misaligned-CR3, and non-canonical.
- **Why:** REVIEW #128 ACCEPTED slice 5 and left three opens for the
  guest-CR3 slice (reserved-bit policy, phys_to_table_ptr wiring, QEMU fault
  asserts); this slice locks the lookup-closure contract host-side so the
  guest slice is wiring-only (identity-map phys_to_table_ptr plus CR3 load
  plus fault lines).
- **Alternatives:** Guest CR3 load plus QEMU fault asserts now (rejected:
  replacing live OVMF tables needs an identity-preserving PML4 copy plus a
  fault handler, doubling single-cycle risk; lookup contract must exist
  first); reserved-bit rejection now (rejected: mode-dependent, no NX/1GiB
  pin yet, documented instead).
- **Expected:** default 75 (74 + 1); kernel 44 (43 + 1); boot-uefi 10; fmt
  clean; clippy host + UEFI clean; no QEMU (guest untouched).
- **Measured (2026-09-26, Win11):** default 75/75 (21+44+4+4+2);
  kernel 44/44 (1 new walk_full); boot-uefi 10/10; fmt clean;
  clippy all-targets clean (one dropping_copy_types fix: scoped blocks
  instead of drop on closures); UEFI-target clippy clean. Diff is
  page_table.rs only (guest untouched; S03 slice-17 PASS stands). QEMU
  not re-run.
- **Final:** Lookup-closure contract landed in code; reserved-bit policy plus
  guest CR3 load plus e2e fault asserts stay next. #6 stays active.

## S04 slice 7 -- walk_full generic lookup + split fail-closed tests (REVIEW #130 notes 1+2) (2026-09-26)

- **Decision:** Host-only hardening slice in
  `crates/kernel/src/page_table.rs` (guest untouched, no QEMU this cycle):
  (a) `walk_full` takes a generic `F: Fn(u64) -> Option<&[u64; 512]>`
  instead of `&dyn Fn` (REVIEW #130 note 1 -- inlinable per level on the
  perf-first guest fault path; same fail-closed behavior); (b) the one
  5-assert `walk_full_chains_lookup_closure_*` test splits into happy +
  4 failure tests (REVIEW #130 note 2) sharing a
  `walk_four_levels_fixture` helper, so an early panic can no longer hide
  later fail-closed cases; (c) missing-table test documents note 3 as
  still open (lookup-miss NotPresent vs cleared-PRESENT must be
  disambiguated by phys_to_table_ptr bounds/OOM wiring).
- **Why:** REVIEW #130 ACCEPTED slice 6 @4e68bd7 and named exactly these
  two non-blocking notes for the guest-CR3 slice; both are host-only and
  keep the guest slice wiring-only.
- **Alternatives:** Guest CR3 load + QEMU fault asserts now (rejected:
  doubles single-cycle risk; generic + split must land first);
  distinct `TableMissing` error now (rejected: note 3 explicitly defers
  the mapping to phys_to_table_ptr wiring -- no new error before the
  guest bounds contract exists).
- **Expected:** default 79 (75 + 4); kernel 48 (44 + 4); boot-uefi 10;
  fmt clean; clippy host + UEFI clean; no QEMU (guest untouched).
- **Measured (2026-09-26, Win11):** default 79/79 (21+48+4+4+2);
  kernel 48/48 (fixture + 5 walk_full tests); boot-uefi 10/10;
  `cargo fmt --check` clean (after one auto-format of the generic
  signature); `cargo clippy --all-targets -- -D warnings` clean;
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` clean. Diff is page_table.rs only (guest untouched;
  S03 slice-17 PASS stands). QEMU not re-run.
- **Final:** REVIEW #130 notes 1+2 closed on the current tree; note 3
  (bounds/OOM vs NotPresent) plus reserved-bit policy plus guest CR3
  load plus e2e fault asserts stay next. #6 stays active.

## S04 slice 8 -- MissingTable vs NotPresent split (REVIEW #131 risk 1 / #130 note 3) (2026-09-26)

- **Decision:** Host-only error-split slice in
  `crates/kernel/src/page_table.rs` (guest untouched, no QEMU this cycle):
  new `WalkerError::MissingTable` for `walk_full` lookup misses
  (`lookup(base)` returns `None`: unmapped table base, which the guest
  `phys_to_table_ptr` maps to out-of-bounds / OOM); cleared-`PRESENT`
  mid-level stays `NotPresent` via `walk_step`. `walk_full` docs updated;
  `walk_full_missing_table_base_is_notpresent` renamed to
  `walk_full_missing_table_base_is_missing_table` asserting
  `MissingTable`, with a comment pinning the distinction against
  `walk_full_mid_level_notpresent_propagates` (`NotPresent`).
- **Why:** REVIEW #131 ACCEPTED slice 7 @3b93589 but flagged the
  lookup-miss vs cleared-PRESENT conflation (both `NotPresent`) as the
  open risk for guest fault asserts / OOM-bounds mapping; REVIEW #130
  note 3 deferred exactly this mapping to the phys_to_table_ptr wiring.
  Splitting host-side keeps the guest slice wiring-only (bounds/OOM ->
  `None` -> `MissingTable`, no new error needed there). By-value generic
  `lookup: F` kept (REVIEW #131 risk 2): existing tests move the closure
  by value; guest closure must be `Copy`/cheap (fn pointer) -- pinned in
  docs on the guest slice, not this one.
- **Alternatives:** Keep conflated `NotPresent` until the guest slice
  (rejected: reviewer named the split as the cheapest disconfirm now,
  and e2e asserts need the distinction before wiring); split inside
  `walk_step` too (rejected: `walk_step` takes a live table reference,
  so a miss is impossible there -- only `walk_full` looks up).
- **Expected:** default 79 (same count, 1 renamed); kernel 48; boot-uefi
  10; fmt clean; clippy host + UEFI clean; no QEMU (guest untouched).
- **Measured (2026-09-26, Win11):** default 79/79 (21+48+4+4+2);
  kernel 48/48 (renamed missing-table test); boot-uefi 10/10;
  `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
  exit 0; `cargo clippy -p baseos-boot-uefi --target
  x86_64-unknown-uefi -- -D warnings` exit 0. Diff is page_table.rs only
  (guest untouched; S03 slice-17 PASS stands). QEMU not re-run.
- **Final:** REVIEW #131 risk 1 / #130 note 3 closed on the current tree;
  reserved-bit policy plus guest CR3 load plus e2e fault asserts stay
  next. #6 stays active.

## S04 slice 9 -- walk_full lookup Copy/cheap doc + independent gate re-run (REVIEW #133 caveats 1+2) (2026-09-26)

- **Decision:** Host-only doc slice in `crates/kernel/src/page_table.rs`
  (guest untouched, no QEMU this cycle): `walk_full` docs now pin that
  `lookup: F` is taken by value and callers must pass a `Copy`/cheap
  callable (`fn` pointer or non-capturing closure; host tests already move
  a non-capturing closure, guest `CR3` slice reuses a `phys_to_table_ptr`
  fn pointer with no per-level dynamic dispatch). No code or test-count
  change; independent re-run of the REVIEW #133-owed gates on the current
  tree.
- **Why:** REVIEW #133 ACCEPTED slice 8 @a5fcb27 but left (1) gates
  unverified this cycle (SKIPPED -- 79/48/10 + fmt/clippy claims trusted,
  independent re-run owed before any guest slice) and (2) by-value `F`
  Copy/cheap doc deferred to the guest slice. Pinning the doc now keeps
  the guest `CR3` slice wiring-only (bounds/OOM -> `None` ->
  `MissingTable`) instead of mixing a doc contract into a guest/QEMU
  cycle.
- **Alternatives:** Roll the doc into the guest `CR3` slice (rejected:
  mixes a zero-risk doc pin into a guest-touching QEMU cycle); add a
  `F: Copy` bound now (rejected: needlessly rejects capturing closures on
  the host path -- doc pins the cheap-callable contract without narrowing
  the generic).
- **Expected:** default 79 (21+48+4+4+2); kernel 48; boot-uefi 10;
  fmt clean; clippy host + UEFI clean; no QEMU (guest untouched).
- **Measured (2026-09-26, Win11):** default 79/79 (21+48+4+4+2);
  kernel 48/48; boot-uefi 10/10; `cargo fmt --check` clean;
  `cargo clippy --all-targets -- -D warnings` exit 0;
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -- -D warnings` exit 0. Diff is page_table.rs docs + this worklog entry
  (guest untouched; S03 slice-17 PASS stands). QEMU not re-run.
- **Final:** REVIEW #133 caveats 1+2 closed on the current tree;
  reserved-bit policy plus guest CR3 `phys_to_table_ptr` bounds/OOM ->
  `None` -> `MissingTable` wiring plus e2e fault asserts stay next. #6
  stays active.

## S04 slice 10 -- walk_full large-page (PS) leaves for firmware tables (2026-09-26)

- **Decision:** Host-only walker slice in
  `crates/kernel/src/page_table.rs` (guest untouched, no QEMU this cycle):
  new `PTE_PS_BIT` (bit 7) plus 1 GiB / 2 MiB offset masks; `walk_full`
  now reads each entry directly (same canonical -> level -> PRESENT order
  as `walk_step`) and stops at a `PS` leaf: level 2 returns the 1 GiB base
  plus `virt & 30-bit offset`, level 1 the 2 MiB base plus `virt & 21-bit
  offset`; `PS` at PML4 (3) / PT (0) is reserved -> `NotPresent`.
  `walk_step` unchanged (4 KiB steps only). One new test:
  `walk_full_large_pages_stop_at_ps_leaf` (1 GiB happy + 2 MiB happy +
  reserved-PS-at-PML4 fail-closed).
- **Why:** OVMF identity-maps RAM with 1 GiB / 2 MiB pages, so the planned
  guest `CR3` walk (read live `CR3` + identity `phys_to_table_ptr` +
  `PAGING` serial line) would misread every firmware large-page leaf as a
  table pointer without this stop-at-leaf rule. Pinning it host-side keeps
  the guest slice wiring-only. Other reserved/MBZ bits stay out of scope
  (only `PRESENT` + `PS`-at-level checked), documented on `walk_full`.
- **Alternatives:** Guest `CR3` walk now without PS handling (rejected:
  would fail on the first firmware large-page entry, burning a QEMU cycle
  for a host-provable rule); full reserved-bit rejection now (rejected:
  mode-dependent NX/PAT/MBZ combos still need the guest `CR3` mode pin).
- **Expected:** default 80 (79 + 1); kernel 49 (48 + 1); boot-uefi 10;
  fmt clean; clippy host + UEFI clean; no QEMU (guest untouched).
- **Measured (2026-09-26, Win11):** default 80/80 (21+49+4+4+2);
  kernel 49/49 (1 new large-page); boot-uefi 10/10;
  `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
  exit 0; `cargo clippy -p baseos-boot-uefi --target
  x86_64-unknown-uefi -- -D warnings` exit 0 (one pre-commit test fix:
  1 GiB expectation typo `0x4011_2345` -> `0x4001_2345`, caught
  red-to-green). Diff is page_table.rs + this worklog entry (guest
  untouched; S03 slice-17 PASS stands). QEMU not re-run.
- **Final:** Firmware-table large-page rule landed host-green. Next:
  guest `CR3` read + identity `phys_to_table_ptr` bounds/OOM -> `None`
  -> `MissingTable` wiring + `PAGING` serial line + e2e assert + QEMU.
  #6 stays active.

## S04 slice 11 -- walk_step 4KiB-only PS fail-closed (REVIEW #137 risk 1) (2026-09-26)

- **Decision:** Host-only walker-divergence slice in
  `crates/kernel/src/page_table.rs` (guest untouched, no QEMU this cycle):
  `walk_step` now fails closed with `NotPresent` when `PS` (bit 7) is set
  instead of returning the leaf as a next-table base, and docs pin it as
  the 4 KiB-only step (`PS` leaves resolve only via `walk_full`, which
  reads the entry directly and stops). Order stays canonical -> level ->
  PRESENT -> PS. New test `walk_step_ps_is_4k_only_fail_closed` pins the
  deliberate divergence plus two untested gaps: `PS` at PD (level 1)
  fails via `walk_step` while the same leaf resolves via `walk_full`
  (`0x21_3456`), `PS` + `!PRESENT` stays `NotPresent`, and `PS` at PT
  (level 0) is `NotPresent`.
- **Why:** REVIEW #137 ACCEPTED slice 10 @f513c37 but named the
  `walk_step`/`walk_full` divergence as the main carry risk: `walk_full`
  stopped calling `walk_step` and inlined its own loop, while `walk_step`
  still treated a `PS` leaf as a table base with no doc mention, so any
  future caller chaining `walk_step` on firmware tables mis-walks
  silently. The cheapest disconfirm/fix named in the review is exactly
  this: document `walk_step` as 4 KiB-only plus a `NotPresent`-on-`PS`
  gate with a divergence-pinning test.
- **Alternatives:** Refactor `walk_full` to call `walk_step` then PS-check
  (rejected: `walk_step` returns only the decoded base, losing the raw
  `PS` bit the leaf rule needs -- would require a second table read or
  a signature change, heavier than the gate); new `LargePage` error for
  `walk_step` (rejected: `NotPresent` already means do-not-follow here
  and keeps the fail-closed set unchanged for callers).
- **Expected:** default 81 (80 + 1); kernel 50 (49 + 1); boot-uefi 10;
  fmt clean; clippy host + UEFI clean; no QEMU (guest untouched).
- **Measured (2026-09-26, Win11):** default 81/81 (21+50+4+4+2);
  kernel 50/50 (1 new walk_step-PS); boot-uefi 10/10;
  `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
  exit 0; `cargo clippy -p baseos-boot-uefi --target
  x86_64-unknown-uefi -- -D warnings` exit 0. Diff is page_table.rs +
  this worklog entry (guest untouched; S03 slice-17 PASS stands). QEMU
  not re-run.
- **Final:** REVIEW #137 risk 1 closed on the current tree (divergence
  deliberate, documented, and test-pinned). Next: guest `CR3` read +
  identity `phys_to_table_ptr` bounds/OOM wiring + `PAGING` serial line
  + e2e fault asserts + QEMU. #6 stays active.

## S04 slice 12 -- walk_step PS@L3/L2 pins (REVIEW #139 note 2) (2026-09-26)

- **Decision:** Host-only test extension in
  `crates/kernel/src/page_table.rs` (guest untouched, no QEMU this cycle):
  extend `walk_step_ps_is_4k_only_fail_closed` with `PS` at PML4 (level 3,
  reserved) and at PDPT (level 2, 1 GiB leaf-as-pointer) fail-closed pins
  (`NotPresent`), closing REVIEW #139 note 2 (`PS@L3` was only covered via
  `walk_full`).
- **Why:** REVIEW #139 ACCEPTED slice 11 @b0538a3 but listed four
  non-blocking notes; note 2 (`PS@L3` not explicitly tested for
  `walk_step`, only L1/L0) is the cheapest host-provable close-out with
  zero API change.
- **Alternatives:** New `Reserved` error variant for PS@L3/L0 (rejected:
  diagnostics-only per #139 note 1, breaks fail-closed set for callers --
  needs a DECISION mail, not a silent slice); full guest `CR3` wiring now
  (rejected: larger guest-touching slice with QEMU, violates one small
  segment per cycle).
- **Expected:** default 81 (unchanged, extended test); kernel 50; boot-uefi
  10; fmt clean; clippy clean; no QEMU (guest untouched).
- **Measured (2026-09-26, Win11):** default 81/81 (21+50+4+4+2);
  kernel 50/50 (extended walk_step-PS); boot-uefi 10/10;
  `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets
  -- -D warnings` clean. Diff is page_table.rs + this worklog entry
  (guest untouched; S03 slice-17 PASS stands). QEMU not re-run.
- **Final:** REVIEW #139 note 2 closed (all four levels now pinned for
  `walk_step`-on-`PS` -> `NotPresent`). Remaining: #139 notes 1/3/4
  (Reserved variant diagnostics, reserved/MBZ bits, guest `CR3`
   bounds/OOM + e2e/QEMU). Next: guest `CR3` read + identity
   `phys_to_table_ptr` bounds/OOM wiring. #6 stays active.

## S04 slice 13 -- guest CR3 read + BootInfo-bounded phys_to_table_ptr + PAGING e2e (2026-09-26)

- **Decision:** First guest-touching S04 slice: (a) `kernel/page_table.rs`:
  new pure `table_base_in_boot_map(&BootInfo, phys)` (aligned + non-zero +
  no-overflow + whole 4 KiB frame inside one `BootInfo` region, any kind)
  plus `table_base_in_boot_map_bounds_gate` test (in-map usable/firmware
  pass; zero/misaligned/outside/straddle/wrap/corrupt-handoff fail closed);
  (b) `boot-uefi/main.rs`: `read_cr3` (`mov {}, cr3`), `phys_to_table_ptr`
  (bounds gate -> `None` -> `MissingTable`, else identity-map deref),
  `probe_paging` (post-EBS: `CR3` + `walk_full` over framebuffer base or
  low `0x1000`, sentinel `STRUCTURAL_FAIL` phys on failure) emitting
  `PAGING ok=<0|1> cr3=0x... phys=0x...` after `FRAMEALLOC`, plus exact-line
  `write_paging_line_embeds_cr3_and_phys` host test; (c) `boot-uefi.ps1`
  hard-requires `PAGING ok=1` (missing = FAIL).
- **Why:** REVIEW #141 accepted slice 12 and agreed next is guest `CR3`
  read + `phys_to_table_ptr` bounds/OOM; OVMF identity-maps RAM with
  large pages, so the slice-10 `PS`-leaf rule gets its first live-table
  proof here (FB probe resolves through a firmware large-page leaf).
- **Alternatives:** Bounds-check against usable-only regions (rejected:
  firmware tables live in reserved/firmware regions — any-kind containment
  admits the real PML4); probe low RAM only (rejected: FB base exercises
  the large-page path the guest slice exists to prove; `0x1000` stays the
  no-FB fallback).
- **Expected:** default 82 (81 + 1); kernel 51 (50 + 1); boot-uefi 11
  (10 + 1); fmt clean; clippy host + UEFI clean; `boot-uefi.ps1` PASS
  with `PAGING ok=1`.
- **Measured (2026-09-26, Win11):** default 82/82 (21+51+4+4+2);
  kernel 51/51 (1 new bounds-gate); boot-uefi 11/11 (1 new PAGING-line);
  `cargo fmt --check` clean (after auto-format); `cargo clippy
  --all-targets -- -D warnings` clean (one `identity_op` fix in the new
  test); `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -- -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial
  722 bytes: marker FOUND, `BOOTINFO valid=1 regions=98`,
  `FB present=1 base=0x80000000`, `MEMMAP ok=1 entries=98/98 truncated=0
  map_size=4704 desc_size=48`, `GOP/EBS/KERNEL ok=1`, `FRAMEALLOC ok=1
  free=52874 spans=8 faults=3 stitch=5`, `PAGING ok=1 cr3=0xf801000
  phys=0x80000000` (FB identity map resolved; live PML4 admitted by the
  bounds gate), `EXC ok=1 vector=0`; guest `bootx64.efi` 65536 bytes.
  Note: `free=52874` vs S03 slice-17 `52875` is 1-page OVMF map variance
  (regions 98, same as recent boots), not a regression signal.
- **Final:** Live-`CR3` walk proven end-to-end with fail-closed bounds
  (`MissingTable`) + `PAGING` hard gate. Remaining: #139 notes 1/3
  (Reserved-variant diagnostics, reserved/MBZ policy). Next: kernel vs
  user mapping foundations / permission-fault e2e. #6 stays active.

## S04 slice 14 -- synthetic permission-fault matrix + PERM e2e (2026-09-26)

- **Decision:** Second guest-touching S04 slice: (a) `kernel/page_table.rs`:
  new pure `perm_selftest() -> (ok, passed, total)` building a fixed 5-entry
  table (kernel RO `0x1000`, kernel RW `0x2000`, user RW `0x3000`, user NX
  `0x4000`, guard `0x5000`) and running 9 `classify_access` probes (RO-write,
  user-on-kernel, supervisor-on-user, NX-fetch, guard, unmapped,
  non-canonical) plus `perm_selftest_all_nine_probes_pass` test;
  (b) `boot-uefi/main.rs`: `write_perm_line` (`PERM ok=<0|1> passed=<n>/<m>`)
  plus `write_perm_line_embeds_passed_total` test, emitted post-EBS after
  `PAGING` before bring-up (pure software model, no live-table mutation);
  (c) `boot-uefi.ps1` hard-requires `PERM ok=1` (missing = FAIL).
- **Why:** REVIEW #143 accepted slice 13 and agreed next is kernel-vs-user /
  permission-fault e2e; the software `check`/`classify` matrix had host tests
  but no guest proof, and live PTE USER/NX auditing needs the reserved/MBZ
  mode pin (#139 note 1/3, still open) — a deterministic synthetic matrix is
  the smallest end-to-end permission gate before touching live flags.
- **Alternatives:** Live leaf-PTE USER/NX decode on the FB probe (rejected:
  needs NX-mode + reserved-bit policy pinned first; synthetic matrix proves
  the enforcement contract without it); fault-injection via real #PF handler
  (rejected: IDT fault path is S02-controlled-#DE only, far larger slice).
- **Expected:** default 83 (82 + 1); kernel 52 (51 + 1); boot-uefi 12
  (11 + 1); fmt clean; clippy host + UEFI clean; `boot-uefi.ps1` PASS
  with `PAGING ok=1` + `PERM ok=1`.
- **Measured (2026-09-26, Win11):** default 83/83 (21+52+4+4+2);
  kernel 52/52 (1 new perm-selftest); boot-uefi 12/12 (1 new PERM-line);
  `cargo fmt --check` clean (after auto-format of a collapsed test line);
  `cargo clippy --all-targets -- -D warnings` clean;
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings`
  clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 743 bytes:
  `BOOTINFO valid=1 regions=98 usable=0xce88000`,
  `FB present=1 base=0x80000000`, `MEMMAP ok=1 entries=98/98 truncated=0
  map_size=4704 desc_size=48`, `GOP/EBS/KERNEL ok=1`, `FRAMEALLOC ok=1
  free=52872 spans=8 faults=3 stitch=5`, `PAGING ok=1 cr3=0xf801000
  phys=0x80000000`, `PERM ok=1 passed=9/9`, `EXC ok=1 vector=0`;
  guest `bootx64.efi` 74240 bytes. Note: `free=52872` vs slice-13 `52874`
  is OVMF/image-size variance (guest grew with the PERM wiring; regions 98
  stable), not a regression signal.
- **Final:** Permission-fault matrix proven end-to-end with `PERM` hard gate.
  Remaining: #139 notes 1/3 (Reserved-variant diagnostics, reserved/MBZ
  policy incl. live USER/NX audit). Next: live permission-bit audit or
  guard-page e2e. #6 stays active.

## S04 slice 15 -- live leaf USER/NX audit + PERMAUDIT e2e (2026-09-26)

- **Decision:** Third guest-touching S04 slice, closing REVIEW #145 residual
  (1) (`PERM ok=1` was synthetic-only): (a) `kernel/page_table.rs`: new pure
  `walk_full_leaf_flags(cr3, virt, lookup) -> (phys, flags)` mirroring
  `walk_full` level-for-level (same `MisalignedPhys`/`NonCanonical`/
  `MissingTable`/`NotPresent`/PS@3-0 rules) but returning the leaf
  `pte_decoded_flags` (4 KiB PTE at L0, PS leaf at L2/L1) plus
  `walk_full_leaf_flags_reports_live_user_nx_bits` test (4 KiB USER/NX,
  2 MiB + 1 GiB PS-leaf flags, phys parity with `walk_full`, error parity);
  fixed a first-pass bug the new test caught (L0 arm returned the table base
  + offset instead of `pte_decoded_phys(pte)` + offset).
  (b) `boot-uefi/main.rs`: `probe_live_flags` (post-EBS, same probe virt as
  `PAGING`: FB base or low `0x1000`) + `write_permaudit_line`
  (`PERMAUDIT ok=<0|1> phys=<0x...> user=<0|1> nx=<0|1>`) with
  `write_permaudit_line_embeds_phys_user_nx` shape test, emitted after `PERM`
  before bring-up; reporting-only (no policy gate: baseline firmware values
  first). (c) `boot-uefi.ps1` hard-requires `PERMAUDIT ok=1`.
- **Why:** Agent-b REVIEW #145 ACCEPTED slice 14 but flagged the synthetic
  gap explicitly: firmware USER/NX misconfig cannot fail `PERM ok=1`. The
  smallest live close-out is a decoded-flags read of the exact entry the
  `PAGING` walk used (no second walk, no drift), reported on serial and
  hard-required for walk success — enforcement policy rides later once the
  baseline is recorded.
- **Alternatives:** PML4-entry USER/NX census across 512 entries (rejected:
  larger serial surface, still needs the leaf-decode helper first; PML4 bits
  alone do not determine effective access); enforce `user=0/nx=1` now
  (rejected: OVMF baseline unknown pre-measurement — this slice proves it is
  `user=0 nx=0` on the FB, so a premature gate would have failed loudly for
  the wrong reason); real `#PF` fault injection (rejected: same as slice 14,
  IDT path far larger).
- **Expected:** default 84 (83 + 1); kernel 53 (52 + 1); boot-uefi 13
  (12 + 1); fmt clean; clippy host + UEFI clean; `boot-uefi.ps1` PASS
  with `PAGING ok=1` + `PERM ok=1` + `PERMAUDIT ok=1`.
- **Measured (2026-09-26, Win11):** default 84/84 (21+53+4+4+2);
  kernel 53/53 (1 new leaf-flags); boot-uefi 13/13 (1 new PERMAUDIT-line);
  `cargo fmt --check` clean (after `cargo fmt` on the long import/call/test
  lines); `cargo clippy --all-targets -- -D warnings` clean;
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings`
  clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 786 bytes:
  `BOOTINFO valid=1 regions=98 usable=0xce88000`,
  `FB present=1 base=0x80000000`, `MEMMAP ok=1 entries=98/98 truncated=0
  map_size=4704 desc_size=48`, `GOP/EBS/KERNEL ok=1`, `FRAMEALLOC ok=1
  free=52872 spans=8 faults=3 stitch=5`, `PAGING ok=1 cr3=0xf801000
  phys=0x80000000`, `PERM ok=1 passed=9/9`,
  `PERMAUDIT ok=1 phys=0x80000000 user=0 nx=0`, `EXC ok=1 vector=0`;
  guest `bootx64.efi` 76288 bytes. Baseline recorded: OVMF marks the FB
  identity map supervisor (`user=0`) and executable (`nx=0`) — enforcement
  must account for executable MMIO before gating `nx=1`.
- **Final:** Live USER/NX decode proven end-to-end with `PERMAUDIT` hard gate
  (reporting-only). Remaining: #139 notes 1/3 (Reserved diagnostics,
  reserved/MBZ policy + enforcement gate on the now-measured baseline).
  Next: guard-page e2e or NX/USER enforcement policy slice. #6 stays active.

## S04 slice 16 -- walk_full vs leaf_flags error parity (REVIEW #147 risk 3)

- **Decision:** Host-only close-out of REVIEW #147 risk 3 (duplication drift):
  new `walk_full_leaf_flags_err_parity_matches_walk_full` test asserting
  `walk_full` and `walk_full_leaf_flags` agree exactly on the three cases
  the prior test's comment claimed but never exercised — cleared-PRESENT
  mid-level, reserved PS@L3, reserved PS@L0 (all `NotPresent`) — plus
  success-path phys parity on the 4 KiB chain. Narrowed the prior test's
  comment to the cases it really covers (missing/non-canonical/misaligned).
  Guest untouched; no serial/ps1 change.
- **Why:** Two copies of the walk logic exist by design (address-only vs
  address+flags); without a parity pin they can drift silently. The cheapest
  disconfirm is a direct both-walkers-agree test, not a shared-helper
  refactor (which would touch the guest path and need a QEMU re-run).
- **Alternatives:** Share one inner helper between the walkers (rejected:
  guest-touching refactor, larger blast radius for a parity claim a test
  pins); W-bit audit extension in the same commit (rejected: changes the
  PERMAUDIT serial shape + ps1 gate, needs QEMU — keep this slice host-only
  and green).
- **Expected:** default 85 (84 + 1); kernel 54 (53 + 1); boot-uefi 13;
  fmt clean; clippy host + UEFI clean; guest untouched (no QEMU re-run).
- **Measured (2026-09-26, Win11):** default 85/85 (21+54+4+4+2);
  kernel 54/54 (1 new parity); boot-uefi 13/13; `cargo fmt --check` clean;
  `cargo clippy --all-targets -- -D warnings` clean;
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings`
  clean. Guest untouched.
- **Final:** Duplication-drift parity pinned; REVIEW #147 risk 3 closed.
  Remaining: #147 risks 1 (single-probe coverage), 2 (W bit), 4 (gate
  brittleness); #139 note 1 (Reserved/MBZ). Next: multi-probe audit with W
  bit or guard-page e2e. #6 stays active.

## S04 slice 17 -- large-page phys+flags parity (REVIEW #149 minor gap)

- **Decision:** Host-only parity close-out in
  `crates/kernel/src/page_table.rs` (guest untouched, no QEMU this cycle):
  new `walk_full_leaf_flags_large_page_phys_flags_parity` test asserting
  `walk_full` and `walk_full_leaf_flags` agree on phys plus exact
  `pte_decoded_flags` for both PS leaf sizes (1 GiB at L2, 2 MiB at L1)
  across 4 flag variants (RW supervisor, RW user, RWX-user NX, RO NX),
  pinning the WRITABLE bit on the leaf_flags path for the first time.
- **Why:** REVIEW #149 ACCEPTED slice 16 @ae67f9e but flagged success parity
  as 4 KiB-only: the PS arms (lines 656-663 vs 712-719) could drift in phys
  masking or flag decode with no test noticing. Direct both-walkers-agree
  test is the cheapest disconfirm, not a shared-helper refactor.
- **Alternatives:** Shared inner helper between walkers (rejected: same as
  slice 16 -- guest-touching refactor for a parity claim a test pins);
  fold into the multi-probe W-bit audit with serial/ps1 change (rejected:
  needs QEMU + gate design; this host-only pin keeps one small segment).
- **Expected:** default 86 (85 + 1); kernel 55 (54 + 1); boot-uefi 13;
  fmt clean; clippy host + UEFI clean; guest untouched (no QEMU re-run).
- **Measured (2026-09-26, Win11):** default 86/86 (21+55+4+4+2);
  kernel 55/55 (1 new large-page parity); boot-uefi 13/13;
  `cargo fmt --check` clean (after one `cargo fmt`);
  `cargo clippy --all-targets -- -D warnings` exit 0;
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings`
  exit 0. Guest untouched.
- **Final:** PS-arm phys+flags parity pinned; REVIEW #149 minor gap closed.
  Remaining: #147 risks 1 (single-probe coverage), 2 (W bit e2e -- now
  partially pinned host-side), 4 (gate brittleness); #139 note 1
  (Reserved/MBZ). Next: multi-probe audit with W bit or guard-page e2e.
  #6 stays active.

## S04 slice 18 -- PERMAUDIT W-bit e2e (REVIEW #147 risk 2)

- **Decision:** First guest-touching close-out of REVIEW #147 risk 2 (W bit
  pinned host-side only since slice 17): (a) `boot-uefi/main.rs`:
  `probe_live_flags` now returns `(ok, phys, user, nx, w)` with the leaf
  `WRITABLE` bit via `pte_decoded_flags`, and `write_permaudit_line` appends
  ` w=<0|1>` (`PERMAUDIT ok=1 phys=0x... user=0|1 nx=0|1 w=0|1`); failure
  shape is `w=0`, and the `PERMAUDIT ok=1` hard-require substring in
  `boot-uefi.ps1` is byte-identical (append-only, no script change);
  (b) exact-line host test updated (`w=1` good path, `w=0` failure path).
- **Why:** REVIEW #151 ACCEPTED slice 17 but left risk 2 open (W-bit pinned
  host-side only, e2e still open) and named the multi-probe W-bit audit or
  guard-page e2e as the cheapest disconfirm. Surfacing the already-decoded
  W bit on the live FB leaf is the smallest guest e2e step: no new walker,
  no policy gate, reporting-only baseline first.
- **Alternatives:** Multi-probe (FB + low-RAM) audit in the same commit
  (rejected: doubles serial-shape churn and QEMU analysis for one cycle;
  single-probe W first keeps risk 1 visibly open); enforce `w=1` policy now
  (rejected: baseline unknown pre-measurement -- this slice proves `w=1`
  on the FB, so a premature gate had no measured basis); separate `WBIT`
  line + script hard-require (rejected: scope growth -- same `ok` bit
  already gates, one appended field is zero e2e churn).
- **Expected:** default 86 (unchanged, updated test); kernel 55; boot-uefi 13
  (updated PERMAUDIT-line); fmt clean; clippy host + UEFI exit 0;
  `boot-uefi.ps1` PASS with `PERMAUDIT ok=1 ... w=1`; smoke PASS.
- **Measured (2026-09-26, Win11):** default 86/86 (21+55+4+4+2);
  kernel 55/55; boot-uefi 13/13 (updated `write_permaudit_line_embeds_*`);
  `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
  exit 0; `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -- -D warnings` exit 0; `boot-uefi.ps1` UEFI BOOT PASS exit 0 (guest
  76800 bytes, serial 790 bytes = 786 + 4 for ` w=1`: marker FOUND,
  `BOOTINFO valid=1 regions=98 usable=0xce87000`,
  `FB present=1 base=0x80000000 1280x800 pitch=5120 bpp=32`,
  `MEMMAP ok=1 entries=98/98 truncated=0 map_size=4704 desc_size=48`,
  `GOP/EBS/KERNEL ok=1`, `FRAMEALLOC ok=1 free=52871 spans=8 faults=3
  stitch=5`, `PAGING ok=1 cr3=0xf801000 phys=0x80000000`,
  `PERM ok=1 passed=9/9`,
  `PERMAUDIT ok=1 phys=0x80000000 user=0 nx=0 w=1`, `EXC ok=1 vector=0`;
  Budget ok=True); `smoke.ps1 -RequireOvmf` SMOKE PASS. Baseline extended:
  OVMF marks the FB identity map supervisor (`user=0`), executable (`nx=0`),
  writable (`w=1`). Note: `free=52871`/`usable=0xce87000` vs slice-15
  `52872`/`0xce88000` is 1-page OVMF map variance (guest +512 B within the
  same 19-page image size; regions/map_size/desc_size stable), not a
  regression signal.
- **Final:** W-bit now proven end-to-end (host exact-line + QEMU `w=1`
  reporting); REVIEW #147 risk 2 closed as reporting (enforcement policy
  still open). Remaining: #147 risks 1 (single-probe), 4 (gate
  brittleness); #139 note 1 (Reserved/MBZ). Next: multi-probe audit or
  guard-page e2e. #6 stays active.

## S04 slice 19 -- PERMAUDIT W gate tightening + stale test rename (REVIEW #153 gaps 2+3)

- **Decision:** Close REVIEW #153 gaps (2) and (3) in one minimal slice:
  (a) `boot-uefi.ps1` now hard-requires same-line ` w=1` via regex
  `PERMAUDIT ok=1 [^\r\n]* w=1` in addition to the `PERMAUDIT ok=1`
  substring (both must hold for PASS; MISSING detail reports each);
  (b) rename stale host test `write_permaudit_line_embeds_phys_user_nx`
  to `write_permaudit_line_embeds_phys_user_nx_w` and lock the gate shape
  with an explicit `contains(" w=1")` assert plus updated doc comments.
- **Why:** REVIEW #153 ACCEPTED slice 18 but showed `ok` is walk-success,
  independent of `w`, so a `w=0` regression would still PASS e2e; and the
  test name no longer described what it pins. Both are cheap to close
  before the multi-probe audit.
- **Alternatives:** Enforce `w=1` policy in-guest (fail-closed line)
  (rejected: baseline just proven on one OVMF config -- gate the script
  first, decide enforcement after multi-probe); multi-probe audit in the
  same commit (rejected: doubles QEMU analysis; gate tightening is
  zero-guest-change).
- **Expected:** default 86 (unchanged, renamed test); kernel 55; boot-uefi 13;
  fmt clean; clippy host + UEFI clean; `boot-uefi.ps1` PASS with
  `PERMAUDIT ok=1 ... w=1`; smoke PASS; regex negative control `w=0` fails.
- **Measured (2026-09-26, Win11):** default 86/86 (21+55+4+4+2);
  kernel 55/55; boot-uefi 13/13 (renamed test);
  `cargo fmt --check` clean; `cargo clippy --all-targets` and
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi` clean;
  `boot-uefi.ps1` UEFI BOOT PASS exit 0 (guest 76800 bytes, serial 790 bytes:
  `PERMAUDIT ok=1 phys=0x80000000 user=0 nx=0 w=1`, budget ok=True);
  regex control `w=1`->True, `w=0`->False;
  `smoke.ps1 -RequireOvmf` SMOKE PASS.
- **Final:** REVIEW #153 gaps (2)+(3) closed on the current tree. Remaining:
  #147 risks 1 (single-probe coverage), 4-partial (gate now covers `w`;
  `user`/`nx` still reporting-only by design); #139 note 1. Next:
  multi-probe audit (FB + low-RAM) with W or guard-page e2e. #6 stays active.

## S04 slice 20 -- multi-probe PERMAUDIT2 (low-RAM 0x1000) e2e (REVIEW #147 risk 1)

- **Decision:** Add a second live-leaf probe of virtual `0x1000` (low-RAM)
  on its own `PERMAUDIT2 ok=<0|1> phys= user= nx= w=` serial line, gated
  by `boot-uefi.ps1` on `PERMAUDIT2 ok=1`. Implementation: factor the
  line writer into `write_tagged_permaudit_line(tag, ...)` (PERMAUDIT and
  PERMAUDIT2 share one code path), add `probe_live_flags_for(info, virt)`
  and keep `probe_live_flags` as the FB-default wrapper.
- **Why:** REVIEW #147 risk 1: every live-flags audit rested on the single
  FB leaf, so one unmapped/odd low address could hide behind it. Two
  probes (FB `0x80000000` + low-RAM `0x1000`) make the audit a multi-point
  sample of the firmware tables.
- **Alternatives:** Three+ probes (rejected: scope growth -- two points
  already break the single-probe assumption; add more only on a finding);
  gate `w=1` on PERMAUDIT2 too (rejected: keep the W gate as the
  slice-19-tightened PERMAUDIT contract; PERMAUDIT2 `w` stays
  reporting-only until measured as a baseline, noted in the ps1 comment).
- **Expected:** default 86 (unchanged, guest test lives in boot-uefi);
  kernel 55; boot-uefi 14 (13 + 1 new PERMAUDIT2 line test); fmt clean;
  clippy host + UEFI clean; `boot-uefi.ps1` PASS with both `PERMAUDIT
  ok=1 ... w=1` and `PERMAUDIT2 ok=1`; smoke PASS.
- **Measured (2026-09-26, Win11):** default 86/86 (21+55+4+4+2);
  kernel 55/55; boot-uefi 14/14 (new `write_permaudit2_line_embeds_phys_user_nx_w`);
  `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
  exit 0; `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -- -D warnings` exit 0; `boot-uefi.ps1` UEFI BOOT PASS exit 0
  (guest 77312 bytes, serial 834 bytes: `PERMAUDIT ok=1 phys=0x80000000
  user=0 nx=0 w=1` + `PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=0 w=1`,
  budget ok=True); gate substring control `ok=0`->False / `ok=1`->True;
  `smoke.ps1` SMOKE PASS. Baseline extended: OVMF low-RAM `0x1000` leaf is
  present, supervisor (`user=0`), executable (`nx=0`), writable (`w=1`) --
  same shape as the FB identity map.
- **Final:** REVIEW #147 risk 1 (single-probe) closed: audit now samples
  two distinct leaves and the script gates both. Remaining: #147 risk 4
  partial (`user`/`nx` still reporting-only by design), #139 note 1
  (Reserved/MBZ). Next: guard-page e2e or `user`/`nx` baseline gating.
  #6 stays active.

## S04 slice 21 -- PERMAUDIT2 W gate tightening (REVIEW #147 risk 4 follow-up)

- **Decision:** Mirror the slice-19 `w=1` same-line regex gate onto the
  second probe: `boot-uefi.ps1` now hard-requires
  `PERMAUDIT2 ok=1 [^\r\n]* w=1` in addition to the `PERMAUDIT2 ok=1`
  substring (both must hold for PASS; MISSING detail reports each).
  Zero guest change (guest 77312 bytes unchanged); `user`/`nx` stay
  reporting-only by design.
- **Why:** Slice 20 left PERMAUDIT2 `w` reporting-only "until measured";
  QEMU measured `w=1` on the low-RAM leaf, so the same walk-success-vs-bit
  hole slice 19 closed on PERMAUDIT still applied to PERMAUDIT2 (a `w=0`
  regression would PASS e2e).
- **Alternatives:** Enforce `user=0`/`nx=0` too (rejected: permission policy
  still open -- W is the slice-19 precedent, user/nx need a baseline-gating
  decision first); in-guest fail-closed line (rejected: script gate first,
  same as slice 19); fold into guard-page e2e (rejected: doubles QEMU
  analysis; gate is zero-guest-change).
- **Expected:** default 86; kernel 55; boot-uefi 14; fmt clean; clippy host
  + UEFI clean; `boot-uefi.ps1` PASS with both `w=1` regexes; smoke PASS;
  regex negative control `w=0` fails.
- **Measured (2026-09-27, Win11):** default 86/86 (21+55+4+4+2);
  kernel 55/55; boot-uefi 14/14;
  `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
  exit 0; `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -- -D warnings` exit 0; `boot-uefi.ps1` UEFI BOOT PASS exit 0
  (guest 77312 bytes, serial 834 bytes: `PERMAUDIT ok=1 phys=0x80000000
  user=0 nx=0 w=1` + `PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=0 w=1`,
  `PAGING ok=1 cr3=0xf801000 phys=0x80000000`, budget ok=True);
  regex control `w=1`->True, `w=0`-substituted->False;
  `smoke.ps1 -RequireOvmf` SMOKE PASS.
- **Final:** Both probes now gate `w=1` same-line. Remaining: #147 risk 4
  residual (`user`/`nx` reporting-only), #139 note 1 (Reserved/MBZ).
  Next: guard-page e2e or `user`/`nx` baseline gating. #6 stays active.

## S04 slice 22 -- PERMAUDIT W-gate `w=10` tightening + checked-in regex control (REVIEW #158 findings 1+2)

- **Decision:** Tighten both W patterns to `w=1(\s|$)` in `boot-uefi.ps1`
  (`permauditWPat`, `permaudit2WPat`) and add checked-in
  `tools/qemu/test-boot-uefi-gates.ps1`, which reads the live patterns
  from `boot-uefi.ps1` (no duplication) and asserts 8/8: w=1 passes,
  w=0 fails, w=10 fails, newline-split fails, for both probes.
  Zero guest change (host-script-only).
- **Why:** REVIEW #158 finding 1: `w=1` is a substring of `w=10`, so a
  hypothetical multi-char `w` field would PASS the gate (same prefix
  softness as slice-19 PERMAUDIT; unexploitable today since guest emits
  single-char `w`). Finding 2: the gate had no committed regression
  test (manual w=0-substitution only), so future pattern edits could
  silently soften it.
- **Alternatives:** `w=1\b` word-boundary (rejected: `\s|$` is explicit
  about the serial line-end shape and matches the existing `[^\r\n]`
  style); hardcode patterns in the test (rejected: test reads live
  patterns so gate and test cannot drift); fold into QEMU e2e
  (rejected: regex control runs without OVMF in <1s).
- **Expected:** gate test 8/8 PASS; `boot-uefi.ps1` still PASS on baseline
  OVMF serial (both probes `w=1`); no guest change; no Rust count change.
- **Measured (2026-09-27, Win11):** `test-boot-uefi-gates.ps1`
  GATE TEST PASS (8/8) exit 0 (P1+P2 x good/zero/ten/split).
- **Final:** Findings 1+2 closed. Remaining: #147 risk 4 residual
  (`user`/`nx` reporting-only), #139 note 1 (Reserved/MBZ). Next:
  guard-page e2e or `user`/`nx` baseline gating. #6 stays active.

## S04 slice 23 -- gate-test lock literal `(\s|$)` (REVIEW #162 nit)

- **Decision:** Tighten the finding-1 lock in
  `tools/qemu/test-boot-uefi-gates.ps1` from `-notlike '*w=1(*'` to
  `-notlike '*w=1(\s|$)*'` (literal substring for the exact tightening).
  Zero guest change; zero `boot-uefi.ps1` change (host-script-only).
- **Why:** REVIEW #162 ACCEPTED slice 22 @3b3d44f (gate test 8/8 re-verified)
  but noted the lock would also accept a wrong-but-parenthesized variant
  such as `w=1(x)`; the live `w=1(\s|$)` regex already rejects `w=1(x)`
  (`(` is neither whitespace nor end), only the meta-check was soft.
- **Alternatives:** Add a `w=1(x)` live-pattern case to the 8 (rejected:
  the 8 already pin live-pattern behavior; the gap was the lock string,
  one literal fixes it with no new cases); leave the nit open (rejected:
  one-line, zero-risk, reviewer-named).
- **Expected:** gate test 8/8 PASS; default 86, kernel 55, boot-uefi 14;
  fmt clean; clippy host + UEFI clean; guest untouched.
- **Measured (2026-09-27, Win11):** `test-boot-uefi-gates.ps1`
  GATE TEST PASS (8/8) exit 0; default 86/86 (21+55+4+4+2); kernel 55/55;
  boot-uefi 14/14; `cargo fmt --check` clean; `cargo clippy --all-targets
  -- -D warnings` clean; `clippy -p baseos-boot-uefi --target
  x86_64-unknown-uefi -- -D warnings` clean. Lock verified: good pattern
  contains the literal and passes, `w=1(x)` lacks it and is rejected,
  and the live regex returns False on a `w=1(x)` sample. Guest untouched
  (QEMU re-run rides with the next guest-touching slice).
- **Final:** REVIEW #162 nit closed. Remaining: #147 risk 4 residual
  (`user`/`nx` reporting-only), #139 note 1 (Reserved/MBZ). Next:
  guard-page e2e or `user`/`nx` baseline gating. #6 stays active.

## S04 slice 24 -- guard-page e2e (synthetic GUARD line + ps1 gate)

- **Decision:** First guard-page e2e close-out, mirroring the slice-14 PERM
  pattern (synthetic software model, no live-table mutation, no fault
  handler): (a) `kernel/page_table.rs`: new pure `guard_selftest() ->
  (ok, passed, total)` building a fixed 3-entry table (mapped RW `0x1000`,
  guard `0x2000`, mapped RW `0x3000`) and running 7 `check` probes with
  exact-fault pins (adjacent low-read + high-write pass, guard
  read/write/execute + intra-page `0x2ABC` read fault exactly `Guard`,
  next-unmapped `0x4000` read faults exactly `NotPresent`) plus
  `guard_selftest_adjacent_pass_guard_faults` test; (b) `boot-uefi/main.rs`:
  `write_guard_line` (`GUARD ok=<0|1> passed=<n>/<m>`) plus
  `write_guard_line_embeds_passed_total` shape test, emitted post-EBS after
  `PERM` before bring-up; (c) `boot-uefi.ps1` hard-requires `GUARD ok=1`
  (missing = FAIL) with the banner bumped to slice 24.
- **Why:** Agent-b #166 named guard-page e2e under #6 with the cheapest
  disconfirm = map guard page + prove touch faults vs adjacent mapped page
  passes, with QEMU log + exit code recorded. The software `Guard` priority
  had host tests but no guest proof; a deterministic synthetic line is the
  smallest end-to-end guard gate before real `#PF`-injection (IDT fault path
  is S02-controlled-`#DE` only, far larger slice).
- **Alternatives:** Real `#PF` fault injection via guard touch + handler
  (rejected: needs live PTE mutation + IDT fault path, doubles single-cycle
  risk; synthetic line proves the enforcement contract first); live
  firmware-guard audit (rejected: OVMF exposes no guard pages -- nothing to
  probe); fold into `user`/`nx` enforcement (rejected: separate policy
  decision, keep one gate per slice).
- **Expected:** default 87 (86 + 1); kernel 56 (55 + 1); boot-uefi 15
  (14 + 1); fmt clean; clippy host + UEFI clean; gate test 8/8 PASS;
  `boot-uefi.ps1` PASS with `GUARD ok=1 passed=7/7`; smoke PASS.
- **Measured (2026-09-27, Win11):** default 87/87 (21+56+4+4+2);
  kernel 56/56 (1 new guard-selftest); boot-uefi 15/15 (1 new GUARD-line);
  `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
  exit 0; `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (8/8);
  `boot-uefi.ps1` UEFI BOOT PASS exit 0 (guest 79360 bytes, serial 856
  bytes = 834 + 22 for the GUARD line: marker FOUND,
  `BOOTINFO valid=1 regions=98 usable=0xce86000`,
  `FB present=1 base=0x80000000`, `MEMMAP ok=1 entries=98/98 truncated=0
  map_size=4704 desc_size=48`, `GOP/EBS/KERNEL ok=1`, `FRAMEALLOC ok=1
  free=52870 spans=8 faults=3 stitch=5`, `PAGING ok=1 cr3=0xf801000
  phys=0x80000000`, `PERM ok=1 passed=9/9`, `GUARD ok=1 passed=7/7`,
  `PERMAUDIT ok=1 phys=0x80000000 user=0 nx=0 w=1`,
  `PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=0 w=1`, `EXC ok=1 vector=0`,
  budget ok=True); `smoke.ps1 -RequireOvmf` SMOKE PASS. Note:
  `free=52870`/`usable=0xce86000` vs slice-21 `52871`/`0xce87000` is 1-page
  OVMF map variance (guest grew with the GUARD wiring; regions/map_size/
  desc_size stable), not a regression signal.
- **Final:** Guard-page contract proven end-to-end with `GUARD` hard gate
  (synthetic; real `#PF` injection stays future work). Remaining: #147 risk
  4 residual (`user`/`nx` reporting-only), #139 note 1 (Reserved/MBZ).
  Next: `user`/`nx` baseline gating or live fault-injection slice. #6 stays
  active.

## S04 slice 25 -- guard-gate hardening (REVIEW #168 nits 1-3)

- **Decision:** Harden the slice-24 `GUARD` e2e gate per agent-b REVIEW #168
  (approve + 3 nits), mirroring the slice-22 PERMAUDIT W tightening
  (#158 finding 1 / #162 nit): (a) `boot-uefi.ps1`: replace the
  `Contains("GUARD ok=1")` substring with the same-line regex
  `GUARD ok=1 [^\r\n]*passed=7/7(\s|$)` — literal space after `ok=1` stops
  `ok=10`/`ok=1x` false-passing, pinning `passed=7/7` stops a future
  coverage shrink (e.g. 6/6) passing on `ok=1` alone, `[^\r\n]*` keeps the
  pin same-line; banner bumped to slice 25; (b)
  `test-boot-uefi-gates.ps1`: extend the checked-in control from 8/8 to
  14/14 with 6 GUARD cases (good passes; ok=10 / ok=1x / 6-7 / missing
  passed / newline-split fail) plus a literal
  `GUARD ok=1*passed=7/7(\s|$)` lock so the tightening cannot regress
  silently.
- **Why:** The slice-24 gate repeated the pre-#158 pattern: any future
  `ok=10`-shaped line (or `ok=1` with reduced coverage) would false-pass,
  and the new hard-require itself was unproven (gate test stayed 8/8).
  Cheapest disconfirm is the negative gate test, same as slices 22-23.
- **Alternatives:** Full `ok=1(\s|$)` + separate `passed` check (rejected:
  two gates to keep in sync; one same-line regex pins both); pin only the
  boundary without `passed=7/7` (rejected: leaves nit 2 open); re-run QEMU
  guest (rejected for this slice: guest untouched, serial contract
  byte-identical `GUARD ok=1 passed=7/7`, gate proof is host-only like
  slices 22-23 — QEMU re-verify rides with the next guest-touching slice).
- **Expected:** gate test 14/14 PASS; guest untouched; no Rust count
  changes; fmt/clippy unchanged.
- **Measured (2026-09-27, Win11):** `test-boot-uefi-gates.ps1` GATE TEST
  PASS (14/14: 8 PERMAUDIT + 6 GUARD); guest untouched (`git status`
  ps1-only); intermediate red verified during dev (first pattern draft
  failed G-good double-space, second draft false-passed ok=10/ok=1x —
  both caught by the new cases before the final literal-space form).
- **Final:** All 3 #168 nits closed; slice-24 APPROVE stands with gate
  hardened. Remaining: #147 risk 4 residual (`user`/`nx` reporting-only),
  #139 note 1, #168 nit 4 accepted (synthetic-only coverage note, needs
  live `#PF` injection). Next: `user`/`nx` gating or live `#PF`
  injection. #6 stays active.

## S04 slice 26 -- GUARD lock tightening (REVIEW #170 nit) + QEMU re-verify (2026-09-27)

- **Decision:** (a) Tighten the slice-25 GUARD meta-lock in
  `tools/qemu/test-boot-uefi-gates.ps1` per REVIEW #170 nit: replace the
  single `-notlike '*GUARD ok=1*passed=7/7(\s|$)*'` with three conjuncts --
  `-notlike '*GUARD ok=1 *'` (literal space boundary after `ok=1`),
  `$g.Contains('[^\r\n]*passed=7/7')` (same-line pin, literal), and
  `-notlike '*passed=7/7(\s|$)*'` (coverage + boundary). Zero guest change;
  zero `boot-uefi.ps1` change (host-script-only). (b) QEMU re-verify the
  unchanged guest before the next host-only run, closing the drift risk the
  same way slice 12 did for S03.
- **Why:** The slice-25 lock let `*` match zero chars, so a regression to
  the boundary-less form (`GUARD ok=1passed=7/7`) still passed the lock line
  -- only the G-ten live case caught it. The new lock fails the lock line
  itself on a boundary-less regression (defense in depth with G-ten), and
  the `Contains` keeps the same-line `[^\r\n]*passed` pin literal so a
  softened lock cannot slip back. The re-verify was owed because slice 25
  (gate-only) and slice 26 (gate-only) stacked two host-only skips on top
  of the slice-24 QEMU PASS.
- **Alternatives:** Leave the lock as-is with G-ten as the only boundary
  guard (rejected: reviewer-named one-line hardening, lock should fail
  loudly on its own); fold the re-verify into the next guest-touching
  slice (rejected: that stacks a third skip; one QEMU run now restores the
  non-interference baseline); commit a serial/log artifact (rejected:
  repo keeps serial logs gitignored under `target/`; commit message +
  MEMORY + this worklog row is the recorded proof, same as slice 12/24).
- **Expected:** gate test 14/14 PASS; default 87, kernel 56, boot-uefi 15;
  fmt clean; clippy host + UEFI clean; guest untouched; `boot-uefi.ps1`
  PASS with `GUARD ok=1 passed=7/7` on the re-verify.
- **Measured (2026-09-27, Win11):** `test-boot-uefi-gates.ps1` GATE TEST
  PASS (14/14: 8 PERMAUDIT + 6 GUARD) exit 0 -- re-run this cycle on the
  slice-26 tree, same result as the slice-26 commit gate; default
  `cargo test` 87/87 (21+56+4+4+2), `-p baseos-kernel` 56/56,
  `-p baseos-boot-uefi` 15/15, `cargo fmt --check` clean (re-run this
  cycle). QEMU re-verify @7518b37 (2026-09-27, recorded in commit message
  + MEMORY, no guest/script bytes changed between 59bb2a4 and 7518b37):
  `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 856 bytes (same as the
  slice-24 baseline), `GUARD ok=1 passed=7/7`, `PERM ok=1 passed=9/9`,
  `PERMAUDIT ok=1 ... w=1` + `PERMAUDIT2 ok=1 ... w=1`, `EXC ok=1
  vector=0`, gate-test 14/14 PASS alongside. Host-vs-guest parity:
  host default 87 / kernel 56 / boot-uefi 15 and gate 14/14 identical to
  the slice-24 baseline values; guest 856B + GUARD 7/7 + PERM 9/9 +
  PERMAUDIT x2 + EXC identical to slice-24. `git status --short` clean
  this cycle (sitrep 2-file flag stale -- third false alarm per #176).
- **Final:** REVIEW #170 nit closed; QEMU/guest parity restored on the
  current tree (@7518b37). Remaining: #147 risk 4 residual (`user`/`nx`
  reporting-only -- agreed next, smaller blast radius than live `#PF`),
  #139 note 1 (Reserved/MBZ), #168 nit 4 (live `#PF` injection). Next:
  `user`/`nx` report-only closeout (#147r4). #6 stays active.

## S04 slice 27 -- PERMAUDIT user/nx shape gate (REVIEW #147 risk 4 report-only closeout)

- **Decision:** Close REVIEW #147 risk 4 residual (`user`/`nx`
  reporting-only) with a value-free SHAPE gate, host-script-only, zero guest
  change: (a) `boot-uefi.ps1`: two new patterns
  `permauditUNPat`/`permaudit2UNPat` =
  `PERMAUDIT[2] ok=1 [^\r\n]* user=[01] [^\r\n]*nx=[01](\s|$)`, hard-required
  alongside the existing `ok=1` + `w=1` gates (banner bumped to slice 27);
  (b) `test-boot-uefi-gates.ps1`: checked-in control extended from 14/14 to
  26/26 with 12 new cases (per probe: good passes, flipped user=1/nx=1 still
  passes the shape, user=2 / missing-user / nx=2 fail, newline-split fails)
  plus a literal `Contains('user=[01] ')` / `Contains('nx=[01]')` lock so the
  pins cannot regress silently.
- **Why:** The `w=1` value gate (slices 19/21/22) was safe because writable
  is the measured OVMF baseline for both probes; `user=0`/`nx=0` is also the
  measured baseline, but pinning those VALUES would fail loudly the moment a
  future NX/SMEP hardening slice flips them -- the correct report-only
  closeout pins field PRESENCE/shape (single-char `[01]`, same-line,
  boundary-tight) while leaving values free for the enforcement-policy slice.
- **Alternatives:** Gate `user=0 nx=0` values like `w=1` (rejected: locks the
  firmware baseline against the S29 NX direction; shape gate fails a dropped
  field just as loudly with no policy commitment); in-guest fail-closed line
  (rejected: script gate first, same precedent as slices 19/21); fold into
  live `#PF` injection (rejected: IDT fault path is the larger blast radius
  this slice explicitly defers per #168 nit 4).
- **Expected:** gate test 26/26 PASS; default 87, kernel 56, boot-uefi 15
  unchanged; fmt clean; clippy host + UEFI clean; guest untouched (no QEMU
  re-run -- one host-only skip on top of the slice-26 verified tree, same
  precedent as slices 22-23/25).
- **Measured (2026-09-27, Win11):** `test-boot-uefi-gates.ps1` GATE TEST
  PASS (26/26: 8 W + 12 user/nx-shape + 6 GUARD) exit 0; intermediate red
  verified during dev (first UN-pattern draft double-counted the
  user/nx separator space -- all 4 good cases failed while the 8 negative
  cases passed -- caught by the new cases before the
  `user=[01] [^\r\n]*nx=` fix; `-like '*user=[01] *'` lock also failed
  because `[01]` is a wildcard class there, fixed to literal `Contains`);
  default `cargo test` 87/87 (21+56+4+4+2), `-p baseos-kernel` 56/56,
  `-p baseos-boot-uefi` 15/15, `cargo fmt --check` clean,
  `cargo clippy --all-targets -- -D warnings` clean,
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings`
  clean. Guest untouched (`git status` ps1-only two files). Sitrep note:
  the sitrep 2-file flag was STALE at cycle start (clean tree + empty diff
  verified before edits -- fifth false alarm); the tree is now genuinely
  dirty with this slice's two ps1 files.
- **Final:** REVIEW #147 risk 4 closed as a value-free shape gate; both
  probes now gate `ok=1` + `w=1` + user/nx-shape same-line. Remaining:
  #139 note 1 (Reserved/MBZ), #168 nit 4 (live `#PF` injection), future
  user/nx VALUE policy (NX/SMEP enforcement slice). Next: live `#PF`
  injection or Reserved/MBZ audit. #6 stays active.

## S04 slice 28 -- user-boundary hardening (REVIEW #182 nit2 carried)

- **Decision:** Close REVIEW #182 nit2 (carried from slice 27) with a
  host-script-only tightening, zero guest change: (a) `boot-uefi.ps1`:
  both `permauditUNPat`/`permaudit2UNPat` change `user=[01] ` (literal
  trailing space) to `user=[01][ \t]` (space-or-tab, same-line blank),
  banner bumped to slice 28; (b) `test-boot-uefi-gates.ps1`: lock
  updated from `Contains('user=[01] ')` to
  `Contains('user=[01][ \t]')`, control extended from 26/26 to 30/30
  with 4 new cases per probe-pair (user=10 / user=1x fail, both probes).
- **Why:** The slice-27 literal space assumed space-only mid-line
  `phys/user/nx/w` order while `nx`/`w` use `(\s|$)` -- inconsistent,
  and tab-separated output would miss the gate. `(\s|$)` was tried
  first for `user` and REJECTED: `\s` matches `\n`, so the
  newline-split `user`/`nx` pair false-passed (U1-split/U2-split went
  red, 28/30). `[ \t]` pins the same single-char boundary without
  crossing lines, keeping the split cases failing; `nx` keeps
  `(\s|$)` because it sits before ` w=` or line-end.
- **Alternatives:** `user=[01](\s|$)` for full nx/w consistency
  (rejected: false-passes newline-split, proven red this slice);
  leave the literal space with an order-note comment (rejected:
  reviewer-named one-line hardening, tab robustness is free with the
  same-line guarantee intact); fold into live `#PF` injection
  (rejected: IDT fault path is the larger blast radius, still
  deferred per #168 nit 4).
- **Expected:** gate test 30/30 PASS; default 87, kernel 56, boot-uefi
  15 unchanged; fmt/clippy clean; guest untouched (host-only skip on
  top of the slice-26 verified tree, same precedent as slices
  22-23/25/27).
- **Measured (2026-09-27, Win11):** `test-boot-uefi-gates.ps1` GATE TEST
  PASS (30/30: 8 W + 16 user/nx-shape + 6 GUARD) exit 0; intermediate
  red verified during dev (first `(\s|$)` draft failed U1-split/U2-split
  as above, caught by the existing split cases before the `[ \t]`
  fix); default `cargo test` 87/87, `-p baseos-kernel` 56/56,
  `-p baseos-boot-uefi` 15/15, `cargo fmt --check` clean,
  `cargo clippy --all-targets -- -D warnings` clean. Guest untouched
  (`git status` ps1-only two files). Sitrep note: the sitrep 2-file
  flag was STALE at cycle start (clean tree + empty diff verified
  before edits -- eighth false alarm); the tree is now genuinely dirty
  with this slice's two ps1 files.
- **Final:** REVIEW #182 nit2 closed; both probes now gate `ok=1` +
  `w=1` + user/nx-shape with `[ \t]`/`(\s|$)` boundaries documented.
  Remaining: #139 note 1 (Reserved/MBZ), #168 nit 4 (live `#PF`
  injection), future user/nx VALUE policy (NX/SMEP enforcement slice).
  Next: live `#PF` injection or Reserved/MBZ audit. #6 stays active.

## S04 slice 29 -- REVIEW #184 nits: tab-positive pin + VALUE/boundary policy

- **Decision:** Close REVIEW #184 nits (1)+(2) host-only, zero guest
  change: (a) `test-boot-uefi-gates.ps1` adds U1-tab/U2-tab
  (`user=0<TAB>nx=0` passes, `` `t `` in PS double-quoted sample),
  30/30 -> 32/32; (b) `boot-uefi.ps1` documents the VALUE/boundary
  policy once above `permauditUNPat` (order phys/user/nx/w assumed,
  values value-free `[01]`, `[ \t]` mid-line vs `(\s|$)` line-terminal
  with the `\n` rationale); (c) accept nit (3): sitrep 2-file flag was
  accurate at generation time (pre-commit dirty, post-commit clean),
  not a false alarm -- stop counting it as one.
- **Why:** Slice-28 widening to `[ \t]` had only rejection coverage
  (split fails); without a tab acceptance case a regression to the
  literal-space form still passed 30/30. The asymmetry note
  (`user` `[ \t]` vs `nx` `(\s|$)`) needed one canonical statement so
  future probes copy the right class per position.
- **Alternatives:** Leave tab unpinned as reviewer-optional polish
  (rejected: two-line control, pins the exact acceptance just added);
  switch `nx` to `[ \t]` for symmetry (rejected: `nx` sits before
  ` w=` or EOL so `(\s|$)` is the correct terminal class, consistent
  with `w=1(\s|$)`);
- **Expected:** gate test 32/32 PASS; guest untouched (host-only).
- **Measured (2026-09-27, Win11):** `test-boot-uefi-gates.ps1` GATE TEST
  PASS (32/32: 8 W + 18 user/nx-shape + 6 GUARD) exit 0; baseline
  pre-edit re-verified 30/30 PASS. Guest untouched (`git status`
  ps1-only two files + worklog).
- **Final:** REVIEW #184 nits 1-3 closed; slice28 APPROVED @db56c93
  stands. Remaining: #139 note 1 (Reserved/MBZ), #168 nit 4 (live
  `#PF` injection). Next: live `#PF` injection or Reserved/MBZ audit.
  #6 stays active.

## S04 slice 30 -- MEMORY sync to slice29 + QEMU re-verify (2026-09-28)

- **Decision:** Sync shared `MEMORY.md` to the slice29 @6975b69 tree
  (was stale at slice27 @0ab2ca7) and QEMU re-verify the unchanged
  guest before the next slice, closing the three-deep host-only stack
  (slices 27-29) the same way slice 26 did. Zero guest/script change;
  diff is `MEMORY.md` + this worklog entry. Board line updated to OK
  per human #79/#80 (supersedes ignore-board).
- **Why:** Slices 27/28/29 were host-only gate tightenings with QEMU
  skips; two stacked skips were already closed by a re-verify at slice
  26, and three stacked skips needed the same parity restore before
  the Reserved/MBZ or live-`#PF` slice touches the guest again.
- **Alternatives:** Fold the re-verify into the next guest-touching
  slice (rejected: stacks a fourth skip; one QEMU run now restores the
  non-interference baseline); commit a serial/log artifact (rejected:
  repo keeps serial logs gitignored under `target/`; commit message +
  MEMORY + this worklog row is the recorded proof, same as slice 26).
- **Expected:** gate test 32/32 PASS; default 87, kernel 56, boot-uefi
  15; fmt clean; clippy host + UEFI clean; guest untouched;
  `boot-uefi.ps1` PASS with 856B serial + `GUARD ok=1 passed=7/7`.
- **Measured (2026-09-28, Win11):** `test-boot-uefi-gates.ps1` GATE TEST
  PASS (32/32: 8 W + 18 user/nx-shape + 6 GUARD) exit 0; default
  `cargo test` 87/87 (21+56+4+4+2), `-p baseos-kernel` 56/56,
  `-p baseos-boot-uefi` 15/15, `cargo fmt --check` clean,
  `cargo clippy --all-targets -- -D warnings` clean,
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings`
  clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 856 bytes,
  guest `bootx64.efi` 79360 bytes (same as slice-24 baseline):
  `PERM ok=1 passed=9/9`, `GUARD ok=1 passed=7/7`,
  `PERMAUDIT ok=1 phys=0x80000000 user=0 nx=0 w=1`,
  `PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=0 w=1`, `EXC ok=1 vector=0`;
  `smoke.ps1 -RequireOvmf` SMOKE PASS.
- **Final:** Parity restored on the current tree; slices 27-29 stand
  verified. Remaining: #139 note 1 (Reserved/MBZ), #168 nit 4 (live
  `#PF` injection). Next: Reserved/MBZ audit or live `#PF` injection.
  #6 stays active.

## S04 slice 31 -- Reserved/MBZ host audit subset (PS@L3/L0 + NX-disabled bit63)

- **Decision:** Open the #139 note 1 Reserved/MBZ audit with the
  mode-independent subset as one host-only slice: new `const`
  `pte_reserved_violation(pte, level, nx_enabled)` in
  `crates/kernel/src/page_table.rs` returning true for `PS` at PML4 (3)
  / PT (0), `NO_EXEC` while `nx_enabled=false`, and `level > 3`;
  cleared-`PRESENT` never counts. One test
  `pte_reserved_violation_pins_ps_and_nx_disabled` pins all arms.
  Docs mark the remainder out of scope (address-size/PK/PAT need
  CPUID/CR4/EFER pins). No guest wiring, no ps1 change; guest `RMBZ`
  line rides with the next guest-touching slice.
- **Why:** Full MBZ rejection is mode-dependent with no guest `CR3` mode
  pin on record; the PS/NX-disabled subset is provable on the host with
  zero QEMU risk and gives the future guest audit a shared helper.
- **Alternatives:** Full reserved-bit rejection now (rejected:
  needs MAXPHYADDR + mode pins, unprovable here); guest `RMBZ` line in
  the same commit (rejected: doubles single-cycle risk; helper first,
  wiring next).
- **Expected:** default 88 (87 + 1), kernel 57 (56 + 1), boot-uefi 15,
  gate test 32/32; fmt clean; clippy host + UEFI clean.
- **Measured (2026-09-28, Win11):** default `cargo test` 88/88
  (21+57+4+4+2), `-p baseos-kernel` 57/57 (1 new),
  `-p baseos-boot-uefi` 15/15, `cargo fmt --check` clean,
  `cargo clippy --all-targets -- -D warnings` clean,
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings`
  clean; `test-boot-uefi-gates.ps1` GATE TEST PASS (32/32).
  Guest bytes change pending (kernel crate links into the guest);
  QEMU re-verify rides with the guest-wiring slice.
- **Final:** #139 note 1 partially closed (mode-independent subset);
  remaining MBZ (address-size/PK/PAT) + guest `RMBZ` wiring + #168 nit 4
  (live `#PF`) still open. Next: guest `RMBZ` line or live `#PF`. #6
  stays active.

## S04 slice 32 -- RMBZ audit shape: raw-leaf walker + violation counter (host-only + QEMU re-verify)

- **Decision:** Second #139 note 1 step as one host-only slice in
  `crates/kernel/src/page_table.rs`: new `walk_full_leaf_raw`
  (`Ok((phys, raw_pte, level))`, level 2/1 for 1 GiB/2 MiB `PS` leaves,
  0 for 4 KiB; same fail-closed errors as `walk_full_leaf_flags`) plus
  `const rmbz_count_violations(leaves, nx_enabled)` counting
  `pte_reserved_violation` hits over walked leaves (walk failures
  contribute no leaf; guest reports walk `ok` separately). One test
  `walk_full_leaf_raw_matches_flags_walk_and_feeds_rmbz` pins raw ==
  flags-walk phys/bits, large-page level 2, NX on/off counts
  (0 vs 1), two-leaf mixed audit, empty slice, and failure parity.
  Guest untouched; `RMBZ` serial line + `EFER.NXE` pin ride with the
  next guest-touching slice.
- **Why:** The slice-31 helper classifies one PTE but the guest has no
  shared shape to obtain `(raw, level)` from live tables; wiring the
  serial line before the walker exists doubles single-cycle risk.
  Raw walker + counter are provable on the host with zero QEMU risk.
- **Alternatives:** Guest `RMBZ` line in the same commit (rejected:
  needs `EFER.NXE` MSR read + serial/ps1/gate changes on top of the
  walker; helper first, wiring next); reuse `walk_full_leaf_flags`
  flags only (rejected: flags drop bit 63-vs-enabled distinction and
  the leaf level the audit needs).
- **Expected:** default 89 (88 + 1), kernel 58 (57 + 1), boot-uefi 15,
  gate test 32/32; fmt/clippy clean; `boot-uefi.ps1` PASS unchanged
  (guest bytes stable, host-only change).
- **Measured (2026-09-28, Win11):** default `cargo test` 89/89
  (21+58+4+4+2), `-p baseos-kernel` 58/58 (1 new),
  `-p baseos-boot-uefi` 15/15, `cargo fmt --check` clean (one
  auto-format on the level-0 return tuple),
  `cargo clippy --all-targets -- -D warnings` exit 0,
  `clippy -p baseos-boot-uefi --tests -- -D warnings` exit 0,
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS
  (32/32); `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 856 bytes,
  marker FOUND, map_size=4704/desc_size=48 budget ok, guest
  `bootx64.efi` 79360 bytes (unchanged: host-only, kernel links into
  guest but bytes stable).
- **Final:** #139 note 1 audit shape complete host-side (classify +
  walk-raw + count); remaining: guest `RMBZ` line with live `EFER.NXE`
  + address-size/PK/PAT mode pins + #168 nit 4 (live `#PF`). Next:
  guest `RMBZ` line. #6 stays active.

## S04 slice 33 -- REVIEW #192 gaps: L1 2M leaf + PS@L3/L0 invalid + non-present counter (host-only)

- **Decision:** Close REVIEW #192 gaps (a)+(b) as one host-only test
  extension in `walk_full_leaf_raw_matches_flags_walk_and_feeds_rmbz`
  (no production-code change): 2M `PS` leaf at level 1 pins raw/level/
  phys/flags parity + zero violations; reserved `PS` at PML4 (3) and PT
  (0) pin `NotPresent` on both raw and flags walkers; not-present raw
  entry pins `rmbz_count_violations == 0`. Guest untouched; `RMBZ`
  serial + `EFER.NXE` pin still ride with the next guest slice.
- **Why:** Review #192 approved S32 shape but flagged the missing L1 arm
  coverage and the untested invalid/non-present counter paths; cheapest
  fix is asserts in the existing fixture before building the guest line
  on the counter.
- **Alternatives:** New separate test fn (rejected: same fixture, one
  lock-free host test keeps the audit shape in one place); guest `RMBZ`
  line in the same commit (rejected: doubles single-cycle risk; gaps
  first, wiring next).
- **Expected:** default 89, kernel 58, boot-uefi 15, gate 32/32;
  fmt/clippy clean; guest bytes stable (host-only).
- **Measured (2026-09-28, Win11):** default `cargo test` 89/89
  (21+58+4+4+2), `-p baseos-kernel` 58/58,
  `-p baseos-boot-uefi` 15/15, `cargo fmt --check` clean,
  `cargo clippy --all-targets -- -D warnings` clean,
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -- -D warnings` clean; `test-boot-uefi-gates.ps1` GATE TEST PASS
  (32/32). Guest untouched (no QEMU re-run this cycle; bytes stable
  at 79360B per slice-32 baseline).
- **Final:** REVIEW #192 gaps (a)+(b) closed; duplication + provisional
  counter risks still open by design (single-inner refactor + guest
  `RMBZ` line next). #6 stays active.

## S04 slice 34 -- guest RMBZ line with live EFER.NXE (end-to-end)

- **Decision:** Land the guest `RMBZ` line (#139 note 1) as one
  end-to-end slice: (a) kernel `EFER_MSR` + `EFER_NXE_BIT` +
  `efer_nxe_enabled` const decode + host test; (b) guest `read_efer`
  (`rdmsr` on `EFER_MSR`), `probe_rmbz` (live NXE pin + both audit
  probes — FB-or-`0x1000` and low-RAM `0x1000`, deduped — through
  `walk_full_leaf_raw`, violations via `rmbz_count_violations` over
  successful leaves only), `write_rmbz_line`
  (`RMBZ ok=<0|1> nxen=<0|1> walks=<ok>/<total> viol=<n>`) emitted
  after `PERMAUDIT2`, before bring-up; (c) `boot-uefi.ps1`
  hard-requires `RMBZ ok=1 [^\r\n]*viol=\d+(\s|$)` (GUARD-style:
  literal space after `ok=1` stops `ok=10`/`ok=1x`, same-line pin
  stops newline-split, `viol` value-free — baseline first); (d) gate
  test grows 32 → 39 (pattern lock + good/value-free/ok=10/ok=1x/
  missing-viol/split cases).
- **Why:** Slices 32–33 built the audit shape host-only; the verification
  gap was the live pin (`EFER.NXE` assumed) and no guest proof. The
  first job is a reporting-only baseline (no policy gate on `viol`)
  before any enforcement slice.
- **Alternatives:** Assume `nx_enabled=true` (rejected: hides the
  NX-disabled MBZ case the audit exists for; `rdmsr` is one
  instruction at CPL0); gate `viol=0` now (rejected: baseline first —
  enforcement needs the measured OVMF count, which this slice
  establishes as 0); reuse flags walker (rejected in slice 32: flags
  drop the raw bits/level the audit needs).
- **Expected:** default 90 (89 + 1 efer test), kernel 59 (58 + 1),
  boot-uefi 16 (15 + 1 line-shape test), gate 39/39; fmt/clippy clean
  both targets; `boot-uefi.ps1` PASS with `RMBZ ok=1`.
- **Measured (2026-09-28, Win11):** default `cargo test` 90/90
  (21+59+4+4+2), `-p baseos-kernel` 59/59,
  `-p baseos-boot-uefi` 16/16, `cargo fmt --check` clean,
  `cargo clippy --all-targets -- -D warnings` clean,
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -- -D warnings` clean; `test-boot-uefi-gates.ps1` GATE TEST PASS
  (39/39); `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 890 bytes:
  marker FOUND, map_size=4704/desc_size=48 budget ok, regions=98,
  `RMBZ ok=1 nxen=1 walks=2/2 viol=0`, guest `bootx64.efi` 83456
  bytes (was 79360: +4096, one page of added walker/EFER code).
- **Final:** Guest `RMBZ` line proven end-to-end with live `EFER.NXE=1`
  and zero violations on OVMF (baseline pinned). Still open by design:
  duplication single-inner refactor, provisional counter
  addr-size/PK/PAT, #168n4 live `#PF`, enforcement policy on `viol`.
  #6 stays active.

## S04 slice 34 re-verify -- REVIEW #196 independent reproduction (2026-09-28)

- **Decision:** Independently reproduce slices 33/34 on the current tree
  (@b8339b2) per agent-b REVIEW #196 (claims, not approvals): full host
  gates + QEMU boot + serial RMBZ dump + negative regex control. No code
  change; diff is this worklog entry only.
- **Why:** REVIEW #196 held S33/S34 unapproved with build/test SKIPPED
  this cycle and named the cheapest disconfirm: exact commands + hashes
  for 90/90 + 39/39, serial dump showing the RMBZ line, and a forced-viol
  run proving the guard fires.
- **Alternatives:** New feature slice (refactor or enforcement) without
  re-verify (rejected: reviewer explicitly blocks close until reproduced);
  commit a serial/log artifact (rejected: logs stay gitignored under
  `target/`; hashes + this row are the proof).
- **Expected:** default 90, kernel 59, boot-uefi 16, gate 39/39, QEMU PASS
  890B with `RMBZ ok=1 nxen=1 walks=2/2 viol=0`, negative control green.
- **Measured (2026-09-28, Win11, @b8339b2):** `cargo test` 90/90 default
  (21 boot-info + 59 kernel + 4 marker + 4 harness + 2 version);
  `cargo test -p baseos-boot-uefi` 16/16;
  `tools/qemu/test-boot-uefi-gates.ps1` GATE TEST PASS (39/39);
  `cargo fmt --check` clean; `cargo clippy --all-targets -D warnings`
  exit 0; `cargo clippy -p baseos-boot-uefi --target
  x86_64-unknown-uefi -D warnings` exit 0;
  `tools/qemu/boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 890 bytes,
  guest `bootx64.efi` 83456 bytes SHA256
  1B048E096D9A798A348F12B8B494B94D3701A50632F445D5D1BBA80AA03E346A,
  serial tail `RMBZ ok=1 nxen=1 walks=2/2 viol=0` + `EXC ok=1 vector=0`,
  budget map_size=4704/desc_size=48 ok=True; negative control on the live
  `rmbzPat`/`guardPat` from `boot-uefi.ps1`: good viol=0 True, viol=1
  True (reporting-only by design), ok=0 False, missing-viol False,
  GUARD 7/7 True, 6/7 False -- NEGATIVE TEST PASS.
- **Final:** REVIEW #196 disconfirm delivered on the current tree; S33/S34
  measurement claims reproduce byte-for-byte (890B, 83456B, viol=0).
  Known limits stand: provisional counter (viol>0 reporting-only),
  duplication single-inner refactor, #168n4 live `#PF`, viol enforcement
  policy. #6 stays active.

## S04 slice35 -- walker single-inner refactor (2026-09-28)

- **Decision:** De-duplicate the triplicated 4-level walk loop in
  `crates/kernel/src/page_table.rs` behind one private `walk_full_inner`
  returning `(phys, raw_pte, level)`; `walk_full` / `walk_full_leaf_flags` /
  `walk_full_leaf_raw` become thin projections (phys-only, phys+decoded
  flags, raw triple). No public API, error, or serial/script change.
- **Why:** REVIEW #202 keeps duplication single-inner refactor as an open
  risk and #6 active; three copies of canonical -> PRESENT -> PS-at-level
  can drift (address vs flags vs audit reads).
- **Alternatives:** Keep triplication + parity tests (rejected: tests pin
  drift after the fact, single source prevents it); merge into one public
  API returning the triple (rejected: churns guest callers + e2e for no
  behavior gain).
- **Expected:** default 90, kernel 59, boot-uefi 16, gate 39/39, QEMU PASS
  890B `RMBZ ok=1 nxen=1 walks=2/2 viol=0`, fmt/clippy clean both targets.
- **Measured (2026-09-28, Win11):** `cargo test` 90/90 default
  (21 boot-info + 59 kernel + 4 marker + 4 harness + 2 version);
  `cargo test -p baseos-boot-uefi` 16/16;
  `tools/qemu/test-boot-uefi-gates.ps1` GATE TEST PASS (39/39);
  `cargo fmt --check` clean; `cargo clippy --all-targets -D warnings`
  exit 0; `cargo clippy -p baseos-boot-uefi --target
  x86_64-unknown-uefi -D warnings` exit 0;
  `tools/qemu/boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 890 bytes
  (marker FOUND, BOOTINFO/MEMMAP/FB/GOP/EBS/KERNEL/FRAMEALLOC/PAGING/PERM/
  GUARD 7/7/PERMAUDIT/PERMAUDIT2/RMBZ ok=1 viol/EXC all ok).
- **Final:** Duplication risk closed with zero behavior change. Open:
  provisional counter (viol reporting-only), #168n4 live #PF, viol
  enforcement policy. #6 stays active until enforcement + b-repro.

## S04 slice 36 -- viol-enforcement policy: fail-closed (guest + script value gate)

- **Decision:** Fail-closed over reporting-only counter. (a)
  `kernel/page_table.rs`: new `const rmbz_audit_ok(walks_ok, walks_total,
  viol)` (true only on full walks AND `viol == 0`) plus
  `rmbz_audit_ok_fails_closed_on_viol_or_short_walks` test (2/2/0 and 1/1/0
  pass; 2/2/1, 2/2/10, 1/1/1, 1/2/0, 0/2/0, 0/0/0 fail). (b)
  `boot-uefi/main.rs`: `probe_rmbz` builds `ok` via `rmbz_audit_ok` (was
  `ok_count == total` inline); docs flip reporting-only baseline to
  fail-closed. Line shape unchanged (`RMBZ ok=<0|1> nxen walks viol`).
  (c) `boot-uefi.ps1`: `rmbzPat` tightens from value-free
  `viol=\d+(\s|$)` to `viol=0(\s|$)` (banner slice 36). (d)
  `test-boot-uefi-gates.ps1`: lock pins `viol=0` value; cases 39 -> 41
  (R-viol10 flips True->False, new R-viol1 False, new R-ok0viol1 False).
- **Why:** Agent-b #207 blocks #6-done on exactly this policy plus host
  tests for the enforced path and a QEMU viol!=0 negative control; REVIEW
  #206 names the provisional counter (reporting-only, no fail-closed
  behavior) as remaining risk (2). Baseline is measured (`nxen=1
  walks=2/2 viol=0` on OVMF since slice 34), so gating zero cannot fail
  for the wrong reason the way a premature user/nx value gate could.
- **Alternatives:** Keep reporting-only counter + script shape gate
  (rejected: a viol>0 boot still prints `ok=1` and PASSES e2e — the exact
  hole #207 names); guest-only `ok` enforcement with value-free script
  (rejected: a buggy `ok=1 viol=1` line would still PASS; defense in depth
  needs both pins, same precedent as the W `w=1` guest-report + script
  value gate in slices 18/19/21/22); script-only `viol=0` with guest
  reporting-only (rejected: leaves `ok=1 viol=1` printable as success on
  serial — the line must read `ok=0` when it fails).
- **Expected:** default 91 (90 + 1), kernel 60 (59 + 1), boot-uefi 16,
  gate 41/41; fmt clean; clippy host + UEFI clean; `boot-uefi.ps1` PASS
  890B with `RMBZ ok=1 nxen=1 walks=2/2 viol=0`; synthetic viol>0 lines
  FAIL the gate.
- **Measured (2026-09-28, Win11):** default `cargo test` 91/91
  (21+60+4+4+2), `-p baseos-kernel` 60/60 (1 new),
  `-p baseos-boot-uefi` 16/16, `cargo fmt --check` clean,
  `cargo clippy --all-targets -D warnings` clean,
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings`
  clean; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41: 8 W + 18
  user/nx-shape + 6 GUARD + 8 RMBZ + P1/P2 overlap counted in file order);
  `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 890 bytes, guest
  `bootx64.efi` 84480 bytes SHA256
  388BA13BE24FD0CB7B3A9E1C8D31B786CBD02E698FEF0792D1C9270CBDE06188
  (rebuild x2 same hash — deterministic this tree):
  `BOOTINFO valid=1 regions=98 usable=0xce85000`,
  `FB present=1 base=0x80000000`, `MEMMAP ok=1 entries=98/98 truncated=0
  map_size=4704 desc_size=48`, `GOP/EBS/KERNEL ok=1`, `FRAMEALLOC ok=1
  free=52869 spans=8 faults=3 stitch=5`, `PAGING ok=1 cr3=0xf801000
  phys=0x80000000`, `PERM ok=1 passed=9/9`, `GUARD ok=1 passed=7/7`,
  `PERMAUDIT ok=1 phys=0x80000000 user=0 nx=0 w=1`,
  `PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=0 w=1`,
  `RMBZ ok=1 nxen=1 walks=2/2 viol=0`, `EXC ok=1 vector=0`;
  negative control via the checked-in gate (live `rmbzPat`):
  `viol=1`->False, `viol=10`->False, `ok=0 viol=1`->False,
  `viol=0`->True. Note: guest stayed 84480B despite added
  `rmbz_audit_ok` call (one `const` inline, page-rounded image) — size
  parity with slice 35, not a silent-revert signal (`git diff` shows the
  probe + pattern + test changes; hash differs from slice-34 1B048E09).
  REVIEW #206 risk (1) variance note: the +1024B slice-34->35 jump is
  still unattributed, but this tree rebuilds deterministically (x2 same
  SHA); a `cargo clean` double-rebuild with `rustc -Vv` capture rides
  with the next slice if agent-b still requires it.
- **Final:** Viol-enforcement policy landed fail-closed on both layers
  with host + gate + QEMU proof (#207 satisfied on the a-side). Open:
  #168n4 live #PF, clean-rebuild hash proof if still required. #6 ready
  for agent-b re-verify (recommend done on concurrence).

## S04 slice36 correction -- +1024B attribution fix + hash non-determinism note (2026-09-29)

- **Decision:** Correct the slice-34->35 +1024B label and the slice-36
  determinism claim with measured data, no code change: (a) causal diff
  `b8339b2..c9732c5` is code-only `crates/kernel/src/page_table.rs`
  35+/83- (commit `c9732c5` totals 65+/83- with its 30-line worklog row;
  `c2d03ca`/`cdda266` in range are MEMORY/worklog-only), so the
  83456B->84480B (+1024B, one page-rounded image step) is refactor
  codegen, not rebuild variance -- the prior "rebuild variance" label in
  slice-35/MEMORY and the "still unattributed" note in slice-36 are
  superseded; (b) record `rustc -Vv`: 1.98.0 (88d9e12ae 2026-08-18)
  x86_64-pc-windows-msvc, LLVM 22.1.8, cargo 1.98.0; (c) record hash
  non-determinism on the unchanged `@5436054` tree: fresh
  `boot-uefi.ps1` build yields 84480B SHA
  `E9E8076983F4237124FC2E7C588C06FC1BE91A84CBDE3B199DC9A12DBD84B70D`
  vs slice-36 recorded `388BA13B...` at the same 84480B size; immediate
  rebuild without clean keeps `E9E807...` (cached, no relink), so the
  slice-36 "rebuild x2 same hash -- deterministic" was a cached-rebuild
  observation, not a clean double-rebuild proof (likely PE timestamp
  embedded per link; `cargo clean` double-rebuild still owed if hash
  determinism is required -- size remains the stable codegen signal).
- **Why:** Agent-b REVIEW #215 held the causal claim trusted-not-verified
  (b-side SKIPPED) and required a worklog correction plus `rustc -Vv`
  and clean double-rebuild hash to close rebuild-variance vs codegen;
  this delivers the correction + toolchain pin + fresh-build datum.
- **Alternatives:** Claim hash determinism from the cached rebuild
  (rejected: disproven by the fresh-build hash mismatch at identical
  size); re-attribute +1024B to OVMF variance (rejected: image bytes
  changed with code only, serial 890B/MEMMAP/FB stable).
- **Expected:** default 91, kernel 60, boot-uefi 16, gate 41/41;
  fmt/clippy clean; `boot-uefi.ps1` PASS 890B `RMBZ ok=1 viol=0`.
- **Measured (2026-09-29, Win11, @5436054):** default `cargo test`
  91/91 (21+60+4+4+2), `-p baseos-kernel` 60/60,
  `-p baseos-boot-uefi` 16/16, gate TEST PASS 41/41;
  `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 890 bytes, guest
  84480 bytes SHA `E9E80769...D84B70D` (vs `388BA13B...` at commit),
  serial tail `RMBZ ok=1 nxen=1 walks=2/2 viol=0` + `EXC ok=1 vector=0`;
  second boot without clean keeps `E9E807...` (cache-stable).
- **Final:** +1024B closed as refactor codegen; hash determinism
  corrected to cache-stable only (clean proof still open, size is the
  signal). Open: #168n4 live #PF (sole functional open), clean-rebuild
  hash proof if required. #6 stays active pending b-side re-verify.

## S04 slice36 correction-2 -- clean double-rebuild hash proof (2026-09-29)

- **Decision:** Close REVIEW #217 determinism risk with a measured
  `cargo clean` double-rebuild hash proof, no code change: two
  consecutive `cargo clean` + `cargo build --target
  x86_64-unknown-uefi -p baseos-boot-uefi` links on the unchanged
  `@a71e6a7` tree, hashing
  `target/x86_64-unknown-uefi/debug/bootx64.efi` each time.
- **Why:** REVIEW #217 accepted the +1024B codegen causal claim but held
  determinism and b-side gates unverified; the slice-36 "x2 same hash"
  was a cached rebuild, and the fresh `@5436054` hash (`E9E80769...`
  vs recorded `388BA13B...`) already showed size-stable / hash-unstable
  output. Cheapest close is the clean double-rebuild datum.
- **Alternatives:** Claim determinism from cached rebuilds (rejected:
  disproven); normalize the PE timestamp with a post-link step
  (rejected: scope growth, reproducibility DoD is satisfied by
  size-as-signal + documented hash instability until a real need).
- **Expected:** Both clean links yield 84480B with differing SHA256;
  gates stay green: default 91, kernel 60, boot-uefi 16, gate 41/41;
  fmt/clippy `-D warnings` clean; `boot-uefi.ps1` PASS 890B viol=0.
- **Measured (2026-09-29, Win11, @a71e6a7, rustc 1.98.0 88d9e12ae
  LLVM 22.1.8):** default `cargo test` 91/91 (21+60+4+4+2),
  `-p baseos-boot-uefi` 16/16, gate TEST PASS 41/41;
  `cargo fmt --check` clean; `clippy --all-targets -D warnings` clean
  (host) and `clippy -p baseos-boot-uefi --target
  x86_64-unknown-uefi -D warnings` clean; `boot-uefi.ps1` UEFI BOOT
  PASS, serial 890 bytes, staged ESP 84480B SHA
  `E9E8076983F4237124FC2E7C588C06FC1BE91A84CBDE3B199DC9A12DBD84B70D`,
  tail `RMBZ ok=1 nxen=1 walks=2/2 viol=0` + `EXC ok=1 vector=0`;
  clean link 1: 84480B SHA
  `23AF9FA38C93C586A975D6EF4DCC2EEF3F05E7F9B34F1A53F9D93181B6594394`;
  clean link 2: 84480B SHA
  `3BCE0EA17A2D39E243D44FFF4588B8727A4C44E2165225A878F465503ADCFA86`.
  Four hashes at one size (commit `388BA13B...`, incremental
  `E9E80769...`, cleans `23AF9FA3...`/`3BCE0EA1...`) = PE link
  non-determinism (likely timestamp); size 84480B is the stable
  codegen signal.
- **Final:** Determinism risk closed as documented hash-instability
  with size-stable proof; no code impact. Open: #168n4 live #PF (sole
  functional open). #6 ready for agent-b re-verify / done on
  concurrence.

## S04 #168n5 -- IDT14 gate + PF stub wired, untriggered (2026-09-29)

- **Decision:** Wire `#PF` (vector 14) alongside `#DE` without triggering
  it: new `exception_handler_pf_rust(vector, code, cr2)` emitting
  `PF ok=1 vector=14 code=0x... addr=0x...` on COM1 then parking, new
  `pf_isr_stub_asm!` (error code from `[RSP]` with no pop, `CR2` to `R8`,
  vector 14 to `RCX`, Win64 align + shadow space), `load_idt` populates
  both gates, two host tests (stub ABI pins + gate encoding round-trip).
  `bring_up_cpu_and_prove_exception` still proves `#DE`, so e2e asserts
  are untouched.
- **Why:** Sole open #168n4 is the live `#PF` guest line (IDT14 + `CR2` +
  serial). The smallest safe segment is wire-but-don't-fire: proves the
  gate/stub/ABI contract host-side plus a QEMU no-regression boot, while
  the trigger + e2e assert rides with the next slice (a mis-wired live
  fault triple-faults, so it must not share a commit with anything else).
- **Alternatives:** Trigger a live `#PF` in the same commit (rejected:
  doubles single-cycle risk — Touch unmapped/non-canonical fault address
  selection plus handler proof in one cycle; wire-first keeps this commit
  behavior-identical); host-only without guest wiring (rejected: the gate
  registration itself is the untested part, and QEMU proves it costs
  nothing).
- **Expected:** default `cargo test` 94 (2 new kernel), kernel 63/63,
  boot-uefi 16/16, fmt + clippy host/uefi clean, `boot-uefi.ps1` PASS
  with `EXC ok=1 vector=0` and no `PF` line, guest grows (~+1.5 KiB stub
  + handler + line writers).
- **Measured (2026-09-29, Win11, rustc 1.98.0):** default 94/94
  (21+63+4+4+2), `-p baseos-boot-uefi` 16/16, `fmt --check` clean,
  `clippy --all-targets -D warnings` clean (host) and
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings`
  clean; `boot-uefi.ps1` UEFI BOOT PASS, serial 894 bytes, guest
  `bootx64.efi` 86016 bytes (+1536 vs 84480: PF stub + handler + hex/dec
  writers + second IDT gate), tail `RMBZ ok=1 viol=0` + `EXC ok=1
  vector=0` (no PF line, as designed — `#PF` never fires this slice).
- **Final:** `#PF` gate + stub + serial contract wired and proven
  no-regression; trigger + `PF ok=1 ... addr=...` e2e assert is the next
  (final) #168 slice. #6 stays active pending live-injection proof and
  b-side double-rebuild + gates + boot verify.

## S04 #168n6 -- live #PF probe chained to #DE + PF e2e gate (2026-09-29)

- **Decision:** Close the sole functional open (#168n5 wired-not-fired) as
  one guest-touching slice in `crates/kernel/src/lib.rs` + `boot-uefi.ps1`:
  (a) new `PF_PROBE_ADDR = 0x0000_5000_0000_0000` (canonical low-half,
  4 KiB-aligned, 80 TiB -- far above firmware RAM/FB, below the
  non-canonical hole) with a `black_box` host test (keeps
  `clippy -D warnings` `assertions_on_constants` clean); (b)
  `bring_up_cpu_and_prove_exception` volatile-reads the probe (unmapped
  guard -> not-present `#PF`) instead of the bare `div0`; (c)
  `exception_handler_pf_rust` emits `PF ok=1 vector=14 code=... addr=...`
  then chains via an intentional `div0` into the `#DE` handler (emits
  `EXC ok=1 vector=0`, parks) -- no `iretq` ever runs, so the CPU-pushed
  `#PF` error code is abandoned by construction (REVIEW #229 risk 2
  closed by doc: future returning handler MUST `add rsp,8` before
  `iretq`); stub docs updated accordingly; (d) `boot-uefi.ps1` hard-requires
  new same-line `$pfPat`
  (`PF ok=1 vector=14 ... code=0x[hex] ... addr=0x[hex]`, mirrors
  RMBZ/GUARD boundary style) alongside the unchanged EXC gate.
- **Why:** REVIEW #229 concurred #6 stays active and named exactly this as
  the cheapest disconfirm: trigger live `#PF` via unmapped guard touch,
  assert `PF ok=1 vector=14/code/addr` in the QEMU log, then halt. Chaining
  (rather than replacing `#DE`) keeps every existing gate green while the
  PF line proves `IDT14` gate + error-code-from-`[RSP]` + `CR2`-to-`R8` +
  Win64 align/shadow wiring end-to-end (REVIEW #229 risk 3: host ABI tests
  alone never proved the guest path).
- **Alternatives:** Replace `#DE` with `#PF` only (rejected: breaks the S02
  controlled-`#DE` proof and the EXC hard-require for one line of serial
  savings); return-via-`iretq` from the PF handler then `div0` in bring-up
  (rejected: requires error-code pop + `iretq` frame surgery this cycle --
  chain-via-fault proves both vectors with zero return-path risk); host-only
  probe-addr test without QEMU (rejected: reviewer explicitly required the
  live QEMU line).
- **Expected:** default `cargo test` 95 (94 + 1 probe test), kernel 64/64,
  boot-uefi 16/16, gate-test 41/41 unchanged, fmt + clippy host/uefi clean,
  `boot-uefi.ps1` PASS with BOTH `PF ok=1 vector=14 code=... addr=...` and
  `EXC ok=1 vector=0`, guest size unchanged (probe touch + chain only).
- **Measured (2026-09-29, Win11, rustc 1.98.0):** default 95/95
  (21+64+4+4+2), `-p baseos-boot-uefi` 16/16, gate-test 41/41 PASS,
  `fmt --check` clean, `clippy --all-targets -D warnings` clean (host) and
  `clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings`
  clean; `boot-uefi.ps1` UEFI BOOT PASS on two consecutive runs, serial 937
  bytes both (894 + 43 PF line), guest `bootx64.efi` 86016 bytes (unchanged),
  tail `PF ok=1 vector=14 code=0x0 addr=0x500000000000` + `EXC ok=1
  vector=0` (code 0x0 = not-present supervisor read, CR2 == probe, as
  designed; map_size=4704 desc_size=48 budget ok).
- **Final:** Live `#PF` fired and proven end-to-end with chained `#DE`;
  #168 trigger open closed on the A-side. #6 stays active pending b-side
  double-rebuild + gates + boot verify (owed).

## S05 slice 1 -- unsafe inventory + boundary shape pins (2026-09-29)

- **Decision:** Open S05 (#8) with the inventory half as one committable
  slice: (a) new `docs/unsafe-inventory.md` (exhaustive, not sampled --
  two `Select-String` sweeps over `crates/` for `unsafe`/`asm!`/
  `global_asm!`/`read_unaligned`/`from_raw_parts`/`addr_of`/`transmute`
  recorded below): `unsafe` is concentrated in exactly 2 files
  (`boot-uefi/main.rs` guest boundary, `kernel/lib.rs` CPU/exception
  boundary); `page_table.rs`, `frame_alloc.rs`, `boot-info`,
  `serial-marker`, `version` contain zero `unsafe`. Per-site table rows
  (A port I/O, B table/slot loads + raw offsets, C map buffers, D CPU
  regs/tables/traps, E test-only fixtures) each give why-unsafe,
  invariant/caller contract, and proof; (b) two host shape-pin tests
  (`boot-uefi s05_unsafe_inventory_pins_boundary_shape`,
  `kernel s05_unsafe_inventory_pins_cpu_boundary_shape`) asserting the doc
  names every site (`include_str!`) plus the boundary constants
  (96/56/64/320/232, GOP 24/8/16/24/32, 36, 16384, 4, 4096, STRUCTURAL_FAIL,
  0x08/0x10, vectors 0/14/32, probe 0x500000000000, COM1 0x3F8/+5/0x20).
  No guest/script change.
- **Why:** S05 verification needs the unsafe inventory first, and REVIEW
  #234 explicitly requires exhaustive (grep `unsafe` + `asm` + raw ptr)
  plus the cheapest disconfirm (`rg -n unsafe crates/` + boundary audit)
  before #8 leaves `proposed`; the doc + pins are that disconfirm in
  committed form. Safe wrappers (port I/O, CPU/table APIs) ride next.
- **Alternatives:** Sample-based inventory of "interesting" sites
  (rejected: reviewer explicitly forbade it -- a missed `transmute` or
  `from_raw_parts` is the failure mode); safe-wrapper API in the same
  commit (rejected: doubles single-cycle risk; inventory must be pinned
  before wrappers move code); runtime unsafe-counting via proc macro
  (rejected: `include_str!` + constant asserts pin doc-vs-code drift with
  zero new deps).
- **Expected:** default `cargo test` 96 (95 + 1 kernel pin; boot-uefi pin
  lives outside default-members), `-p baseos-boot-uefi` 17 (16 + 1),
  gate-test 41/41, fmt/clippy `-D warnings` clean both targets,
  `boot-uefi.ps1` PASS byte-identical to @bfc4790 (937B, 86016B).
- **Measured (2026-09-29, Win11, rustc 1.98.0):** default 96/96
  (21+65+4+4+2), `-p baseos-kernel` 65/65,
  `-p baseos-boot-uefi` 17/17 (both new pins green),
  `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41),
  `cargo fmt --check` clean,
  `cargo clippy --all-targets -D warnings` clean (host) and
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 937
  bytes, guest `bootx64.efi` 86016 bytes (both byte-identical to @bfc4790;
  map_size=4704 desc_size=48 budget ok, `PF ok=1 vector=14 code=0x0` +
  `EXC ok=1 vector=0`). Sweep datum: 158 hits across the 7 scanned files
  (guest fns + test fixtures); zero outside the 2 boundary files.
- **Final:** Inventory landed exhaustive with shape pins; #8 ready for
  agent-b review (proposed -> active this cycle; done on concurrence).
  Next: safe wrappers for port I/O, CPU/table APIs, invariant docs.

## S05 slice 2 — kernel safe port-I/O wrappers + anchor pins + re-baseline policy (2026-09-29)

- **Decision:** First safe-wrapper slice of #8: (a) new `crates/kernel/src/uart.rs`
  — private unsafe `outb`/`inb` bodies, safe `pub(crate)` wrappers
  (`com1_write_byte`, `write_dec_u64`, `write_hex_u64`, `write_exc_line`,
  `write_pf_line`); `lib.rs` drops its unsafe port-I/O copies and calls the
  safe wrappers (handlers keep `unsafe` only for the `hlt`/`div` asm);
  COM1 consts stay in `lib.rs` as the single owner (`uart` imports them);
  (b) REVIEW #236 nit 1: both shape-pin tests now also assert every table
  row anchor (`| A1 |`…`| D10 |`, 24 rows) so a deleted row fails even if
  the site name survives elsewhere; (c) REVIEW #236 nit 2: inventory header
  gains a re-baseline policy (line numbers are not the gate — the anchored
  pins are; each moving slice notes only its own new lines), §A6/A7 point
  at `uart.rs`, and the zero claim reads `zero unsafe blocks` (page_table
  hits are doc prose); (d) new host test
  `uart_wrappers_are_safe_without_unsafe_block` binds the wrappers to plain
  `fn` types — an `unsafe fn` would not coerce — proving safe-callability
  without executing port I/O on the host.
- **Why:** S05 verification needs safe callers unable to name a port or need
  an `unsafe` block for serial output; the reviewer accepted slice 1 but
  held #8 active for exactly these wrappers, and flagged both pin/dirft
  nits for slice 2.
- **Alternatives:** Ownership-token `Com1` claim-once guard (rejected: the
  exception handlers are `extern "C"` from asm and cannot carry a token —
  ordering/exclusivity stays documented, as in the inventory); shared
  `crates/hal` for boot-uefi+kernel UART (rejected: doubles the slice —
  boot-uefi keeps its pre-EBS `uart_init`/`com1_emit` this cycle, unify
  later); executing the wrappers in a host test (rejected: `in`/`out`
  faults outside the guest — the `fn`-type binding is the host proof,
  e2e is the live proof).
- **Expected:** default 97 (21+66+4+4+2), boot-uefi 17/17, fmt/clippy clean
  both targets, `boot-uefi.ps1` PASS with PF+EXC lines (guest size may move
  on the refactor — record, not assert identical).
- **Measured (2026-09-29, Win11, rustc 1.98.0):** default 97/97
  (21+66+4+4+2 — kernel 66/66 incl. the new safe-type test),
  `-p baseos-boot-uefi` 17/17 (anchor asserts green),
  `cargo fmt --check` clean (after one auto-format),
  `cargo clippy --all-targets -D warnings` clean (host) and
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 937
  bytes (marker FOUND, BOOTINFO/MEMMAP/FB/GOP/EBS/KERNEL/FRAMEALLOC/PAGING/
  PERM/GUARD/PERMAUDIT[x2]/RMBZ ok, `PF ok=1 vector=14` + `EXC ok=1
  vector=0`), guest `bootx64.efi` 85504 bytes (was 86016 @1157b60, -512B
  refactor codegen — serial contract unchanged).
- **Final:** Kernel port I/O is a safe boundary (unsafe confined to
  `uart::outb`/`inb`); both REVIEW #236 nits closed on this tree. #8 stays
  active: next is CPU/table safe wrappers (GDT/IDT/CR3/EFER) + boot-uefi
  UART unify.

## S05 slice 3 — kernel CPU/table boundary + gate/probe safe wrappers (2026-09-29)

- **Decision:** Second safe-wrapper slice of #8: (a) new
  `crates/kernel/src/cpu.rs` — moved `Gdt`/`GDT`,
  `DescriptorTablePointer`, `IdtEntry`, `Idt`/`IDT`, extern stub decls, and
  the privileged `load_gdt`/`load_idt` bodies out of `lib.rs` (unsafe
  confined to the two load bodies, as with `uart.rs`); `lib.rs`
  `bring_up_cpu_and_prove_exception` now calls `cpu::load_gdt` /
  `cpu::load_idt`, descriptor consts stay in `lib.rs` as the single owner
  (`cpu` imports them), and the `IdtEntry` re-export is test-only (guest
  names `cpu::` directly); (b) two safe `pub(crate)` wrappers alongside:
  `gate_for(handler)` pins `KERNEL_CODE_SEL` so callers cannot wire a gate
  into the null/data segment (consumed by `load_idt`), and
  `probe_addr_valid(addr)` pins the live-`#PF` guard contract
  (canonical low-half, 4 KiB-aligned, above firmware RAM) as a pure
  `const fn` — guest intentionally does not call it (no panic path
  post-EBS; `dead_code` in guest builds is documented, host tests are the
  gate); (c) two host tests (`gate_for_pins_kernel_code_selector`,
  `probe_addr_valid_matches_guard_contract`); (d) inventory §D1/D2 point
  at `cpu.rs` per the re-baseline policy, header notes the slice-3 delta,
  follow-ups narrowed to CR3/EFER wrappers + boot-uefi UART unify.
- **Why:** S05 verification needs the CPU/table unsafe concentrated behind
  documented wrappers with safe callers unable to pick a wrong segment or
  probe address; the pure move plus the two wrappers is the smallest
  end-to-end-green segment (CR3/EFER + UART unify ride next cycle).
- **Alternatives:** Move `bring_up_cpu_and_prove_exception` and the ISR
  stubs/handlers into `cpu.rs` too (rejected: handlers depend on `uart`
  and the stub macros feed host ABI tests in `lib.rs` — moving them doubles
  the diff for no new boundary); `debug_assert!(probe_addr_valid(..))` in
  the guest bring-up (rejected: adds a panic path post-EBS; host gate +
  e2e probe fire is the proof); shared `crates/hal` for boot-uefi UART
  now (rejected: same scope argument as slice 2 — unify next).
- **Expected:** default 99 (21+68+4+4+2), boot-uefi 17/17, fmt/clippy clean
  both targets, `boot-uefi.ps1` PASS with PF+EXC lines (pure move — guest
  size expected stable, record either way).
- **Measured (2026-09-29, Win11, rustc 1.98.0):** default 99/99
  (21+68+4+4+2 — kernel 68/68 incl. the 2 new cpu tests),
  `-p baseos-boot-uefi` 17/17 (anchor asserts green),
  `cargo fmt --check` clean (after one auto-format),
  `cargo clippy --all-targets -D warnings` clean (host) and
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 937
  bytes (marker FOUND, BOOTINFO/MEMMAP/FB/GOP/EBS/KERNEL/FRAMEALLOC/PAGING/
  PERM/GUARD/PERMAUDIT[x2]/RMBZ ok, `PF ok=1 vector=14` + `EXC ok=1
  vector=0`), guest `bootx64.efi` 85504 bytes (byte-identical to slice 2 —
  pure move confirmed, serial contract unchanged).
- **Final:** Kernel CPU/table unsafe is concentrated in `cpu.rs` with safe
  gate/probe wrappers beside it. #8 stays active: next is CR3/EFER safe
  wrappers + boot-uefi UART unify.

## S05 slice 4 — enforce gate selector pin + probe/guard agreement (REVIEW #240 R1+R2)

- **Decision:** Close REVIEW #240 R1+R2 as one host-test-only slice:
  (a) R1 — `IdtEntry::from_handler` is now private to `cpu.rs`, so
  `cpu::gate_for` (pins `KERNEL_CODE_SEL`) is the only in-crate path to a
  present gate (compiler-enforced, not convention); `lib.rs`
  `idt_de/pf_gate_round_trips` tests now build via `gate_for`;
  (b) R2 — new `probe_addr_valid_agrees_with_pf_probe_guard` test ties
  `cpu::probe_addr_valid` to the raw `#PF` guard contract on one table
  (probe, probe+0x1000 true; probe+1, hole, firmware RAM, null false) plus
  the raw canonical/aligned/floor predicate, so the two cannot drift;
  (c) inventory D2 notes the enforcement. R3 (independent reviewer QEMU
  re-run) stays owed by agent-b; builder re-ran QEMU on this tree.
- **Why:** R1 left the selector pin bypassable (`lib.rs:397,497` called
  `from_handler` directly); R2 left `probe_addr_valid` duplicating the
  `pf_probe` guard with no agreement test — both cheap to lock before the
  CR3/EFER + UART-unify slice grows the boundary.
- **Alternatives:** `debug_assert!(selector == KERNEL_CODE_SEL)` in
  `load_idt` instead of privatizing (rejected: runtime-only, guest-only,
  never fires on host tests — privacy is checked at compile time on every
  build); change `probe_addr_valid` to call the guard predicate (rejected:
  the guard lives as inline asserts, not a shared fn — the agreement test
  is the cheapest lock without refactoring).
- **Expected:** default 100 (21+69+4+4+2), boot-uefi 17/17, fmt/clippy clean
  both targets, `boot-uefi.ps1` PASS with guest size stable (host-only
  change — visibility + tests, no codegen).
- **Measured (2026-09-29, Win11, rustc 1.98.0):** default 100/100
  (21+69+4+4+2 — kernel 69/69 incl. the new agreement test),
  `-p baseos-boot-uefi` 17/17, `cargo fmt --check` clean (no fix needed),
  `cargo clippy --all-targets -D warnings` clean (host) and
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 937
  bytes (marker FOUND, BOOTINFO/MEMMAP/FB/GOP/EBS/KERNEL/FRAMEALLOC/PAGING/
  PERM/GUARD/PERMAUDIT[x2]/RMBZ ok, `PF ok=1 vector=14` + `EXC ok=1
  vector=0`), guest `bootx64.efi` 85504 bytes (identical to slice 3 —
  host-only confirmed).
- **Final:** R1+R2 closed on this tree; R3 still owed (agent-b independent
  QEMU re-run before S05 close). #8 stays active: next is CR3/EFER safe
  wrappers + boot-uefi UART unify.

## S05 slice 5 — single-source probe guard + CR3/EFER decode wrappers (REVIEW #242 nit)

- **Decision:** One host-test-only slice: (a) close REVIEW #242 residual nit
  — the raw `#PF` guard predicate lives ONLY in `cpu::probe_addr_valid`;
  `lib.rs` `pf_probe_addr_is_canonical_unmapped_guard` now asserts via the
  wrapper on one table (probe true, probe+0x1000 true, probe+1/hole/RAM/null
  false) and the R2 agreement test drops its duplicated raw
  `& 0xFFF`/range asserts, so 3 copies collapse to 1 + wrapper-table tests;
  (b) add safe pure `cpu::cr3_pml4_base` (over `page_table::cr3_decode`) and
  `cpu::efer_nxe_enabled` (over `page_table::efer_nxe_enabled`) with
  agreement + safeness (`fn`-coercion) host tests — decode rules join the
  boundary beside `gate_for`/`probe_addr_valid`; live `read_cr3`/`read_efer`
  asm stays guest-side in `boot-uefi` (D7/D8) until the register-read move
  slice; guest untouched. Inventory follow-ups note the delta.
- **Why:** The nit left the guard in 3 copies (agreement test detects drift
  but does not prevent it); CR3/EFER decodes are the next boundary growth
  and belong as tested wrappers before any guest rewiring or UART unify.
- **Alternatives:** Delete the agreement test as redundant (rejected: it
  remains the R2 pin that probe+neighbors hit the wrapper; cheap to keep
  single-sourced); move `read_cr3`/`read_efer` asm into `cpu` now (rejected:
  doubles single-cycle risk — guest rewiring + QEMU re-proof belongs in its
  own slice; pure decodes are independently committable).
- **Expected:** default 103 (21+72+4+4+2), boot-uefi 17/17, fmt/clippy clean
  both targets, `boot-uefi.ps1` PASS with guest size stable (host-only —
  wrappers + tests, no codegen).
- **Measured (2026-09-29, Win11, rustc 1.98.0):** default 103/103
  (21+72+4+4+2 — kernel 72/72 incl. 3 new: cr3/efer agreement + safeness),
  `-p baseos-boot-uefi` 17/17, `cargo fmt --check` clean,
  `cargo clippy --all-targets -D warnings` clean (host) and
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 937
  bytes, guest `bootx64.efi` 85504 bytes (identical to slice 4 —
  host-only confirmed).
- **Final:** #242 nit closed; CR3/EFER decode wrappers pinned host-side.
  R3 still owed (agent-b independent QEMU re-run before S05 close). #8
  stays active: next is register-read move (`read_cr3`/`read_efer` into
  `cpu`) + boot-uefi UART unify.

## S05 slice 6 — register-read move (`read_cr3`/`read_efer` into `cpu`)

- **Decision:** Move the two live privileged reads beside their decode
  wrappers: new `pub unsafe fn cpu::read_cr3` (`mov {}, cr3`) and
  `pub unsafe fn cpu::read_efer` (`rdmsr` on `page_table::EFER_MSR`),
  ex-boot-uefi D7/D8; `boot-uefi` deletes its local copies and imports
  `baseos_kernel::cpu::{read_cr3, read_efer}` for `probe_paging` /
  `probe_live_flags_for` / `probe_rmbz` (probes stay guest-side); new host
  test pins both as `unsafe fn` type (callers need an `unsafe` block, never
  called on host). Inventory D7/D8/D9 rows re-baselined to the new path.
  UART unify explicitly deferred to the next slice.
- **Why:** Inventory follow-up named this as the next boundary growth after
  the slice-5 decode wrappers: one module now owns each read + decode pair
  (`read_cr3`+`cr3_pml4_base`, `read_efer`+`efer_nxe_enabled`), so the guest
  probes cannot drift from the host-tested rules. Smallest guest-touching
  segment: no probe logic changed, only the call path.
- **Alternatives:** Move UART unify in the same commit (rejected: doubles
  single-cycle risk — port-I/O unify rewires every serial line, deserves its
  own QEMU re-proof); keep reads guest-side with agreement tests (rejected:
  agreement detects drift but does not prevent it — the #242 lesson).
- **Expected:** default 104 (21+73+4+4+2, +1 new unsafe-shape pin),
  boot-uefi 17/17, fmt clean, all three clippy gates clean,
  `boot-uefi.ps1` PASS with guest behavior identical.
- **Measured (2026-09-29, Win11, rustc 1.98.0):** default 104/104
  (21+73+4+4+2 — kernel 73/73 incl. 1 new:
  `live_register_reads_are_unsafe_fn`), `-p baseos-boot-uefi` 17/17,
  `cargo fmt --check` clean, `cargo clippy --all-targets -D warnings`
  clean (host), `cargo clippy -p baseos-boot-uefi --tests -D warnings`
  clean, `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 937
  bytes (PF ok=1 vector=14 + EXC ok=1), guest `bootx64.efi` 85504 bytes
  (byte-identical to slice 5 — move is zero-cost codegen).
- **Final:** D7/D8 moved; read + decode pairs co-located in `cpu`.
  REVIEW #244 (agent-b slice-5 ACCEPT + R3 independent PASS) closes the
  slice-5 R3 debt. #8 stays active: next is boot-uefi UART unify
  (`com1_write_byte` via `kernel::uart`, pre-EBS `uart_init` stays
  guest-side).

## S05 slice 7 — read+decode call-site unification onto `cpu` (REVIEW #246 gap)

- **Decision:** Close the REVIEW #246 non-blocking wiring gap: promote
  `cpu::cr3_pml4_base` / `cpu::efer_nxe_enabled` from `pub(crate)` to `pub`
  (`#[inline(always)]`, stale `dead_code` allows + "does not call yet"
  comments removed) and rewire all four guest decode sites
  (`probe_paging`, `probe_live_flags_for`, `probe_rmbz` x2) from
  `page_table::cr3_decode` / `page_table::efer_nxe_enabled` to the `cpu`
  wrappers; inventory D9 row drops drift-prone line numbers and records
  read+decode unified via `cpu`.
- **Why:** Slice 6 co-located read+decode in `cpu` but left the call-site
  pairing split (reads via `cpu`, decodes via `page_table`), so the drift
  the boundary exists to prevent was still possible. Smallest
  guest-touching segment: no probe logic changed, only the decode path.
- **Alternatives:** Record-and-keep direct `page_table` decode in the guest
  (rejected: reviewer offered it as an option, but unification is three
  identical substitutions plus a visibility bump — cheaper than the doc
  justifying a split); fold UART unify into the same commit (rejected:
  same single-cycle reason as slice 6).
- **Expected:** default 104, boot-uefi 17/17, fmt + all three clippy gates
  clean, `boot-uefi.ps1` PASS with unchanged serial lines.
- **Measured (2026-09-29, Win11, rustc 1.98.0):** default 104/104
  (21+73+4+4+2), `-p baseos-boot-uefi` 17/17, `cargo fmt --check` clean
  (one auto-format on the lengthened `page_table` import), host + `--tests`
  + UEFI-target clippy clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0,
  serial 937 bytes (PF ok=1 vector=14 + EXC ok=1), guest `bootx64.efi`
  86016 bytes (+512 vs 85504 slice-6 baseline — one PE file-alignment
  increment from the extra wrapper layer despite `#[inline(always)]`;
  serial output identical, no behavior change).
- **Final:** Read+decode pairs now unified at both definition and call
  sites; #246 gap closed. #8 stays active: next is boot-uefi UART unify.

## S05 slice 8 -- guest UART unify onto `kernel::uart` (+ #248 risk-1 pin)

- **Decision:** Unify guest port I/O onto `kernel::uart`: (a) new pub safe
  `uart::uart_init` (ex-boot-uefi A3 init sequence, `unsafe` confined to the
  body over private `outb`/`inb`); promote `uart::com1_write_byte` from
  `pub(crate)` to `pub`; (b) guest deletes local `outb`/`inb`/`uart_init`/
  `com1_write_byte` + `COM1_*` consts, `com1_emit` becomes a safe
  delegation to `kernel::uart::com1_write_byte` (no `unsafe` block);
  (c) new host test `guest_uses_kernel_uart_and_cpu_wrappers` pins the
  guest to `kernel::{uart,cpu}` wrappers (concat-built needles so the test
  source cannot self-match) and closes REVIEW #248 risk 1 (silent revert
  to `page_table::` decode now fails here); (d) inventory A1-A5 rows
  re-baselined to the single-owner shape.
- **Why:** Slices 6-7 unified register reads+decodes in `cpu` but the UART
  path still had two owners (guest-local `out`/`in` duplicates of the
  kernel boundary) -- the same drift the boundary exists to prevent.
  Smallest guest-touching segment: init sequence byte-identical, only the
  owner moves.
- **Alternatives:** Keep `uart_init` guest-side and unify only the write
  path (rejected: leaves two `out` owners; the init sequence is 7 plain
  register writes with the same single-threaded contract, so moving it is
  zero extra risk); separate `STITCH`-style serial field for prove-out
  (rejected: behavior-neutral unify needs no new e2e field -- the existing
  full chain is the proof).
- **Expected:** default 104, boot-uefi 18 (17 + 1 new pin), fmt + clippy
  gates clean, `boot-uefi.ps1` PASS with unchanged serial lines.
- **Measured (2026-09-30, Win11, rustc 1.98.0):** default 104/104
  (21+73+4+4+2), `-p baseos-boot-uefi` 18/18, `cargo fmt --check` clean
  (after auto-format of import order + one long assert), host + `--tests`
  + UEFI-target clippy exit 0 `-D warnings`; `boot-uefi.ps1` UEFI BOOT
  PASS exit 0 x2 consecutive runs, serial 937 bytes (marker FOUND,
  BOOTINFO/MEMMAP/FB/GOP/EBS/KERNEL/FRAMEALLOC/PAGING/PERM/GUARD/
  PERMAUDIT/PERMAUDIT2/RMBZ/PF+EXC all ok, budget ok=True), guest
  `bootx64.efi` 85504 bytes (-512 vs 86016 slice-7 baseline -- removes
  the duplicate port-I/O bodies; serial identical, behavior-neutral).
  Note: `free=52869 spans=8 faults=3 stitch=5` vs slice-7 `free=52875`
  is OVMF Usable-map variance across boots (regions=98, map_size=4704,
  desc_size=48 stable; spans/faults/stitch identical), not a regression.
- **Final:** Guest holds no port-I/O `unsafe` (only `kernel::uart`
  owns `in`/`out`); UART read+decode precedent now covers both S05
  boundaries. #8 stays active (S05 close-out: full line re-baseline +
  remaining review).

## S05 close-out -- pub-widen DECISION + needle-tripwire record + indep re-run (REVIEW #256)

- **Decision:** (a) `uart_init` + `com1_write_byte` `pub` widening from
  `pub(crate)` is INTENTIONAL and stays: single owner (`kernel::uart`
  owns the sole `out`/`in` bodies), guest calls the safe wrappers, ordering
  + exclusivity stays a documented caller contract (call once pre-handoff,
  single-threaded boot/exception path owns COM1, `cli` in effect post-EBS);
  (b) ownership-token `Com1` claim-once guard is explicitly DEFERRED, not
  forgotten; (c) the slice-8 `guest_uses_kernel_uart_and_cpu_wrappers` pin
  uses concat-built needles (`["unsafe fn uart_", "init"].concat()` etc.)
  so the test source cannot self-match its own literals -- the tripwire
  fails only on a real guest revert, never on its own text.
- **Why:** REVIEW #256 provisionally accepted slice 8 @bb0e839 but held
  full close-out on exactly these two gaps: (2) pub-widening leaves misuse
  fail-open (doc-only, no compiler bar) with the token deferral lacking an
  explicit DECISION + alternatives; (3) PROJECT_GOAL requires a worklog
  decision entry for the pub-widening and the string-needle tripwire, and
  slice 8 cited neither. This row is that entry. Independence gap (1)
  stays owed by agent-b (builder re-run != independent), so #8 stays
  `active` per the review.
- **Alternatives:** Ownership-token `Com1` (claim-once, `!Copy`, threaded
  through `efi_main` -> probes -> handlers) -- rejected now: the two live
  consumers that must share the UART (`exception_handler_rust`,
  `exception_handler_pf_rust`) are `extern "C"` from `global_asm!` stubs
  and cannot carry a token without a global `static` (which reintroduces
  the shared-state the token was meant to remove); single-threaded
  boot + `cli` + documented order is the cheaper correct contract, same
  rationale as slice-2 `Com1` rejection, re-affirmed here. Separate
  `crates/hal` for UART -- rejected: one-module move with zero new deps
  already gives a single owner; a new crate doubles the slice for no new
  bar. Keep `uart_init` guest-side, unify only the write path -- rejected
  in slice 8 (leaves two `out` owners). For the pin: plain string
  literals in the test -- rejected: `include_str!` of the guest source
  would match the test's own literal and false-pass/loop; hardcoding the
  guest path outside `include_str!` -- rejected: reintroduces drift the
  pin exists to prevent.
- **Expected:** No code change (worklog-only); default 104, boot-uefi 18,
  gate 41/41, fmt + host/`--tests`/UEFI-target clippy clean,
  `boot-uefi.ps1` PASS 937B with `PF ok=1 vector=14` + `EXC ok=1 vector=0`,
  guest 85504B stable.
- **Measured (2026-09-30, Win11, rustc 1.98.0, @bb0e839 tree):** default
  `cargo test` 104/104 (21+73+4+4+2), `-p baseos-boot-uefi` 18/18 (incl.
  the needle pin), `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41),
  `cargo fmt --check` clean, `cargo clippy --all-targets -D warnings`
  clean (host) and `cargo clippy -p baseos-boot-uefi --target
  x86_64-unknown-uefi -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS
  exit 0, serial 937 bytes (marker FOUND, BOOTINFO/MEMMAP/FB/GOP/EBS/
  KERNEL/FRAMEALLOC/PAGING/PERM/GUARD/PERMAUDIT[x2]/RMBZ/PF+EXC all ok,
  budget map_size=4704 desc_size=48 ok=True), guest `bootx64.efi` 85504
  bytes (identical to slice-8 baseline; behavior-neutral unify confirmed).
- **Final:** REVIEW #256 gaps (2)+(3) closed on this tree (explicit
  pub-widen/token-defer DECISION + needle-tripwire rationale recorded).
  Gap (1) remains: agent-b independent cargo+QEMU re-run still owed
  before #8 flips to `done`. Next: agent-b re-run, then S05 close
  (full inventory line re-baseline rides with the closing slice).

## S06 slice 1 -- thread state + stack contract + stub pick (host-only)

- **Decision:** Open S06 (#9) with a host-only, no_std-clean contract in
  `crates/kernel/src/thread.rs` (`pub mod thread`): `ThreadState`
  (Ready/Running/Done), `Thread::new` (fail-closed: null base, size <
  4 KiB, misaligned base, base+size overflow, null entry), `stack_top`,
  plus pure `switch_stub` round-robin picker (no asm, no state mutation;
  Unknown/NotReady/NoSwitch refusals). Five host tests. Guest untouched.
- **Why:** REVIEW #273 required explicit failure/tests scope before
  activating #9; live register swap rides with slice 2 + QEMU proof, so
  bad stacks/entries must fail closed on the host first.
- **Alternatives:** Live asm swap now (rejected: doubles single-cycle
  risk; contract must exist first); QEMU proof this slice (rejected:
  guest bytes untouched, prior PASS stands by non-interference).
- **Expected:** default 109 (104 + 5); kernel 78 (73 + 5); fmt clean;
  clippy clean; no QEMU.
- **Measured (2026-09-30, Win11):** default `cargo test` 109/109
  (21+78+4+4+2); `-p baseos-kernel` 78/78 (5 new thread);
  `cargo fmt --check` clean (after one auto-format);
  `cargo clippy -p baseos-kernel --all-targets -D warnings` clean.
  Guest untouched; QEMU not re-run (S05 PASS stands).
- **Final:** S06 failure/test scope pinned host-green. #9 active. Next:
  slice 2 live swap stub + QEMU proof. Carried risks: S05 pub-widen
  fail-closed decision, COM1 token, hash size-as-signal.

## S06 slice 1 hardening -- REVIEW #275 risks 1-3 (true RR + private fields + table validation)

- **Decision:** Close all three REVIEW #275 blockers host-only in
  `crates/kernel/src/thread.rs` (guest untouched, no QEMU this cycle):
  (a) risk 1 -- `switch_stub` is now true round-robin in slice order
  (first Ready after current, wrapping) instead of lowest-id pick;
  (b) risk 2 -- `Thread` fields made private with accessors plus
  `set_state` gate, `checked_top` helper, and `debug_assert` on
  `stack_top` (struct-literal bypass closed); (c) risk 3 -- `new`
  rejects misaligned size and `validate_threads` (duplicate-id /
  overlap / bad-top) fails closed as `InvalidTable`, wired into the
  picker; adjacent stacks stay valid.
- **Why:** REVIEW #275 verified d159e15 but held slice 2 on exactly these
  three gaps (doc claimed RR, pub fields bypassed `new`, no table
  checks); cheapest disconfirm is the named host tests, keeping the
  live-swap plus QEMU slice wiring-only.
- **Alternatives:** Rename to pick-lowest-ready (rejected: roadmap needs
  RR for slice 2); keep pub fields plus debug_assert only (rejected:
  assert fires after invalid state exists); table checks only in slice 2
  (rejected: picker must already refuse corrupt tables).
- **Expected:** default 111 (109 + 2); kernel 80 (78 + 2); fmt clean;
  clippy `-D warnings` clean; no QEMU (guest untouched).
- **Measured (2026-09-30, Win11):** default `cargo test` 111/111
  (21+80+4+4+2); `-p baseos-kernel` 80/80 (2 new: RR order plus
  corrupt-table); `cargo fmt --check` clean (after one auto-format);
  `cargo clippy -p baseos-kernel --all-targets -D warnings` clean (one
  double_must_use fix). Guest untouched; QEMU not re-run (S05 PASS
  stands by non-interference).
- **Final:** REVIEW #275 risks 1-3 closed on the current tree. #9 stays
  active. Next: slice 2 live swap plus QEMU proof.

## S06 slice 2 -- lifecycle enforcement (closes REVIEW #277 carried risk a)

- **Decision:** Close REVIEW #277 carried risk (a) host-only in
  `crates/kernel/src/thread.rs` (guest untouched, no QEMU this cycle):
  `set_state` now enforces Ready->Running, Running->Ready (yield),
  Ready->Done (cancel), Running->Done (exit) and returns `bool`
  (`#[must_use]`, `false` with no mutation on any other edge, including
  all `Done` resurrection and same-state no-ops). Two call sites updated
  to assert the `true` path; one new test
  `lifecycle_is_enforced_no_resurrection` pins dispatch/yield/exit,
  cancel-before-run, and terminal-`Done`.
- **Why:** REVIEW #277 approved slice-1 hardening @d8f9267 but carried the
  unrestricted gate as non-blocking risk (a): any->any incl.
  Done->Ready resurrection; picker stays safe (only Ready picked) but no
  lifecycle enforcement. Live swap must build on a terminal-`Done`.
- **Alternatives:** Keep `()` setter + doc-only lifecycle (rejected:
  invalid state exists before any assert fires, same pattern as the
  slice-1 pub-field bypass); new error enum over `bool` (rejected: four
  legal edges vs one refusal need no diagnostics yet; `bool` keeps the
  gate minimal and host-pinned).
- **Expected:** default 112 (111 + 1); kernel 81 (80 + 1); fmt clean;
  clippy `-D warnings` clean; no QEMU (guest untouched).
- **Measured (2026-09-30, Win11):** default `cargo test` 112/112
  (21+81+4+4+2); `-p baseos-kernel` 81/81 (1 new lifecycle);
  workspace 130/130 incl. `-p baseos-boot-uefi` 18/18;
  `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --tests
  -D warnings` clean. Guest untouched; QEMU not re-run (S05 PASS
  stands by non-interference).
- **Final:** Carried risk (a) closed on the current tree. #9 stays active.
  Remaining carried: (b) validate O(n^2) per-pick, (c) id-0/max-threads/
  containment, (d) saturating `stack_top`. Next: slice 3 live swap stub
  + QEMU proof (or id-0 reserve before swap per risk c).

## S06 slice 3 -- id-0 reserve + table cap + fail-loud top (closes REVIEW #277 risks c/d, documents b)

- **Decision:** Close REVIEW #277 carried risks (c)+(d) host-only in
  `crates/kernel/src/thread.rs` (guest untouched, no QEMU this cycle):
  (a) `new` rejects `id == 0` with new `ThreadError::InvalidId` (0 is the
  reserved null-thread sentinel for the future table); (b) new
  `MAX_THREADS = 16` cap enforced as the first gate in `validate_threads`
  (over-cap table -> `InvalidTable`, so the picker refuses before any
  register touch); (c) `stack_top` overflow arm changed from
  `saturating_add` to `checked_add` with `u64::MAX` sentinel (misaligned,
  so `validate_threads` rejects it instead of masking the bug with a
  clamped-but-plausible top); (d) `MAX_THREADS` docs pin the O(n^2) bound
  (16^2 = 256 pair checks worst case; validate-at-insert or cache is the
  named follow-up). One new test `id_zero_reserved_and_table_capped`
  pins InvalidId, MAX-exact valid, MAX+1 InvalidTable on both validate
  and pick paths.
- **Why:** Risk (c) named id-0/max/containment as the reserve-before-swap
  gate; risk (d) named the saturating top as fail-silent in release.
  Both are host-pinning with zero QEMU risk, so the live-swap slice
  builds on a reserved id space and a loud top. Stack containment stays
  deferred (no kernel memory map pin yet -- overlap gate already covers
  thread-vs-thread).
- **Alternatives:** Silent `Copy` of saturating form (rejected: clamps to
  a plausible aligned value, masking the unreachable-bypass bug);
  `Option<u64>` return for `stack_top` (rejected: breaks the pinned
  accessor contract; sentinel keeps the shape while failing validation);
  larger cap like 64 (rejected: 16 bounds the per-pick scan for the
  pre-scheduler kernel; raising it later is one constant + test).
- **Expected:** default 113 (112 + 1); kernel 82 (81 + 1); fmt clean;
  clippy `-D warnings` clean; no QEMU (guest untouched).
- **Measured (2026-09-30, Win11):** default `cargo test` 113/113
  (21+82+4+4+2); `-p baseos-kernel` 82/82 (1 new id/cap);
  `cargo fmt --check` clean (one auto-format: blank line after the cap
  gate); `cargo clippy -p baseos-kernel --tests -D warnings` clean.
  Guest untouched; QEMU not re-run (S05 PASS stands by
  non-interference).
- **Final:** Carried risks (c)+(d) closed, (b) documented with a bound on
  the current tree. #9 stays active. Next: live swap stub + QEMU proof.

## S06 slice4 — swap-target resolution + id-0 smuggle gate + QEMU (2026-09-30)

- **Decision:** Close all three REVIEW #279 notes in one host-testable
  slice, still no asm: (a) new `swap_target(threads, current) ->
  (next_id, stack_top, entry)` resolving through `switch_stub` then
  `checked_top` (never `stack_top()` alone), so the `u64::MAX` sentinel
  can never reach a future `RSP` load — any `None`/misaligned top
  returns `InvalidTable` with no values; (b) `validate_threads`
  defensively rejects `id == 0` (struct-literal smuggle path; `new()`
  already rejects), pinned by a test-only literal; (c) `SwitchError::
  InvalidTable` doc now names id-0 + MAX cap. Guest untouched; QEMU
  re-run as non-interference proof.
- **Why:** #279 approved slice3 but made these the cheapest disconfirm
  before live asm: lethal-RSP, sentinel smuggle, stale doc. All are
  unit-test-only with zero guest risk.
- **Alternatives:** Take `&mut` threads and mutate state in swap_target
  (rejected: picker stays pure; caller moves via `set_state` gate);
  return `Option` instead of `Result` (rejected: refusal reason needed
  for serial diagnosability); live RSP asm now (rejected: needs guest
  stacks + QEMU fault proof — next slice).
- **Expected:** default 114 (113 + 1); kernel 83 (82 + 1); fmt/clippy
  clean; QEMU PASS unchanged.
- **Measured (2026-09-30, Win11):** default `cargo test` 114/114
  (21+83+4+4+2); `-p baseos-kernel` 83/83 (1 new swap test, 1 extended
  corrupt-table); `cargo fmt --check` clean (one auto-format);
  host + UEFI `clippy -D warnings` exit 0; `boot-uefi.ps1` UEFI BOOT
  PASS (serial 937B, guest 85504B, PF+EXC ok=1).
- **Final:** #279 notes closed on-tree. #9 stays active. Next: live RSP
  swap asm + QEMU fault proof.

## S06 slice5 — validate_threads enforces all new() invariants + swap entry gate (2026-09-30)

- **Decision:** Close the REVIEW #281 must-fix smuggle gap in one
  host-testable slice, still no asm: (a) `validate_threads` now rejects
  every `new()` invariant — `stack_base == 0`, `stack_size < MIN_STACK`,
  misaligned base/size, `entry == 0` — in addition to the existing
  id-0/overflow/align-top/dup/overlap/len gates, so a smuggled
  null-entry or degenerate-stack literal fails closed as `InvalidTable`
  before any pick/swap; (b) `swap_target` re-checks `entry == 0` after
  `switch_stub` (defense-in-depth alongside the existing top re-check,
  same rationale as the id-0 gate); (c) `ThreadError`/`SwitchError`/
  picker docs name the full gate set. One new test
  `smuggled_stack_entry_invariants_fail_closed` pins five literals
  (null entry, null base, 16 B size, misaligned base with aligned top,
  misaligned size) against `validate`/`switch_stub`/`swap_target`.
  Guest untouched.
- **Why:** #281 approved slice4 but showed a smuggled `entry == 0`,
  `stack_base == 0`, or 16 B `stack_size` passes validation and
  `swap_target` returns `Ok((id, top, 0))` with lethal RIP/RSP — the
  same bypass/deserialize path the id-0 gate already assumes. Cheapest
  disconfirm is the five-literal test, not inspection.
- **Alternatives:** Gate only `entry == 0` (rejected: null base and
  sub-page/misaligned stacks are the same lethal-RSP class and free in
  the same loop); rely on private fields instead of table validation
  (rejected: fields block outside-crate literals today but not the
  documented future bypass/deserialize path); live RSP asm in the same
  commit (rejected: needs guest stacks + QEMU fault proof — next slice).
- **Expected:** default 115 (114 + 1); kernel 84 (83 + 1); fmt/clippy
  clean; QEMU unchanged (guest untouched).
- **Measured (2026-09-30, Win11):** default `cargo test` 115/115
  (21+84+4+4+2); `--workspace --exclude uefi-guest` 133 total
  (21+18+84+4+4+2, +1 kernel); `cargo fmt --check` clean (one
  auto-format); `cargo clippy --all-targets -- -D warnings` exit 0.
- **Final:** #281 gap closed on-tree. #9 stays active. Next: live RSP
  swap asm + QEMU fault proof.

## S06 slice6 — shared stack/entry gate closes REVIEW #283 solo-check gap (2026-09-30)

- **Decision:** Close the REVIEW #283 must-fix before live RSP asm in one
  host-testable slice, still no asm: (a) factor `check_stack_entry()`
  (base==0, size<MIN_STACK, base/size 16B align, overflow/top-align,
  entry==0) as the single source shared by `new()` / `validate_threads()`
  / `swap_target()` — `new()` delegates via `match` (no `?` in `const fn`),
  `validate_threads` maps any `Err` to `InvalidTable`, `swap_target`'s
  post-`switch_stub` defense-in-depth re-check calls the full gate instead
  of entry==0 + top-only; (b) one new test
  `swap_target_solo_gate_matches_new_invariants` pins the two load-bearing
  top-only bypasses (16 B size, misaligned-base-with-aligned-top) against
  the shared gate + `new()` agreement. Guest untouched.
- **Why:** #283 approved slice5 but showed the solo re-check passes an
  aligned `top` from a 16 B or misaligned-base stack, so a future
  `validate_threads` bypass would still hand asm a lethal/unsafe RSP.
  Cheapest disconfirm is the shared-gate + two-literal test, not
  inspection.
- **Alternatives:** Live RSP asm in the same commit (rejected: needs guest
  stacks + QEMU fault proof — next slice, now unblocked); duplicate the
  five checks inline in `swap_target` (rejected: drift risk — one function
  keeps new/validate/swap in lockstep).
- **Expected:** default 116 (115 + 1); kernel 85 (84 + 1); fmt/clippy
  clean; QEMU unchanged (guest untouched).
- **Measured (2026-09-30, Win11):** default `cargo test` 116/116
  (21+85+4+4+2); `cargo fmt --check` clean;
  `cargo clippy --all-targets -- -D warnings` exit 0.
- **Final:** #283 gap closed on-tree. #9 stays active. Next: live RSP
  swap asm + QEMU fault proof.

## S06 slice7 -- live RSP swap + QEMU fault proof (2026-09-30)

- **Decision:** First live-context slice: (a) `kernel/thread.rs`: new
  guest-only `unsafe fn run_on_thread_stack(top, func)` (stash caller RSP
  in callee-saved `R15` via push/mov, load `new_top`, `call func` so the
  return address lands on the NEW stack, restore; host stub is a no-op so
  call sites stay symmetric) plus two host tests (unsafe-fn shape pin,
  `swap_target` triple is stack-safe: aligned `Some(top)`, never the
  `u64::MAX` sentinel, nonzero entry); (b) `boot-uefi/main.rs`: two 8 KiB
  `align(16)` static stacks, `thread_b_entry` (`extern "C" fn`, emits
  `THREAD-RUN id=2` on the new stack, returns), `probe_thread_swap`
  (builds `Thread::new(1/2)`, resolves via `swap_target`, runs the entry
  on the target stack, emits `THREAD ok=1 id=<n> top=<hex> entry=<hex>`,
  fail-closed `ok=0` line on any refusal) called post-EBS after `RMBZ`
  before bring-up; (c) `boot-uefi.ps1` hard-requires `THREAD ok=1` +
  `THREAD-RUN id=2` (missing = FAIL).
- **Why:** Slices 1-6 pinned the data contract host-only; S06 verification
  needs a live `RSP` load with a fault-loud proof before any register
  save/restore. Smallest end-to-end step is one stack switch + return.
- **Alternatives:** Full callee-saved context switch now (rejected:
  doubles single-cycle risk; save/restore set unproven here); static-slot
  `SAVED_RSP` via `in(reg)` (rejected after measured QEMU fault: callee
  clobbered the slot-address register, `THREAD-RUN` printed then crash
  before `THREAD ok=1`; `R15` stash fixes the class, documented inline).
- **Expected:** default 118 (116 + 2); kernel 87 (85 + 2); fmt/clippy
  clean both targets; `boot-uefi.ps1` PASS with `THREAD ok=1` +
  `THREAD-RUN id=2` ahead of `PF`/`EXC`.
- **Measured (2026-09-30, Win11):** default `cargo test` 118/118
  (21+87+4+4+2); `-p baseos-kernel` 87/87 (new: unsafe-shape +
  stack-safe triple); `cargo fmt --check` clean;
  `cargo clippy --all-targets -- -D warnings` exit 0;
  `cargo clippy -p baseos-boot-uefi --tests -- -D warnings` exit 0;
  `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi
  -D warnings` exit 0 (one `dead_code` allow on the stack wrapper);
  `boot-uefi.ps1` UEFI BOOT PASS exit 0 (guest 91136 bytes, serial 1000
  bytes: marker FOUND, BOOTINFO/MEMMAP/FB/GOP/EBS/KERNEL/FRAMEALLOC/
  PAGING/PERM/GUARD/PERMAUDIT/PERMAUDIT2/RMBZ all ok,
  `THREAD-RUN id=2` then `THREAD ok=1 id=2 top=0x... entry=0x...`,
  `PF ok=1 vector=14` chained to `EXC ok=1 vector=0`).
- **Final:** Live `RSP` swap proven end-to-end with fail-loud lines. #9
  stays active. Next: save/restore callee-saved regs + round-trip
  switch-back stress.


## S06 slice8 -- REVIEW #287 risks 1-3: R15 reserve + entry-through call + post-EBS cli (2026-09-30)

- **Decision:** Close all three REVIEW #287 risks in one minimal guest slice: (a) risk 1 (must-fix) -- kernel/thread.rs run_on_thread_stack asm gains out("r15") _ so the allocator excludes literal-r15 scratch from both in(reg) inputs (conservative: push/pop preserves r15, claiming clobber is sound), with inline docs recording the collision and the do-not-remove rule; (b) risk 2 -- boot-uefi/main.rs probe_thread_swap transmutes the returned entry half (entry as ptr -> extern C fn, null-checked) and calls THAT on the new RSP, so the emitted entry= hex is the address actually executed (plus host test swap_entry_round_trips_through_u64 pinning fn->u64->fn callability); (c) risk 3 -- efi_main executes one cli immediately after successful EBS before any post-EBS probe, making the documented cli-in-effect contract true (was doc-only, zero cli in tree).
- **Why:** Agent-b REVIEW #287 APPROVED slice7 @94e59e6 but named exactly these three risks for slice8 in value order; R15 collision is soundness (single QEMU PASS cannot disprove misallocation), entry-half was reporting-only, cli was contract-without-instruction.
- **Alternatives:** Pin inputs to fixed regs (in r14 / in r12) instead of out r15 (rejected: narrower than a reservation -- future input growth re-opens the hole); assert entry == thread_b_entry without calling through it (rejected: still reporting-only, not a callability proof); defer cli to the bring-up slice (rejected: thread probe already documents cli, one instruction now closes all callers).
- **Expected:** default 119 (118 + 1); kernel 88 (87 + 1); boot-uefi 18; fmt clean; clippy host + UEFI exit 0; boot-uefi.ps1 PASS with THREAD ok=1 + THREAD-RUN id=2 + PF/EXC.
- **Measured (2026-09-30, Win11):** default cargo test 119/119 (21+88+4+4+2); -p baseos-kernel 88/88 (new: entry round-trip); -p baseos-boot-uefi 18/18; cargo fmt --check clean; cargo clippy --all-targets -- -D warnings exit 0; cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings exit 0; boot-uefi.ps1 UEFI BOOT PASS exit 0 (guest 91648 bytes, serial 1000 bytes: marker FOUND, BOOTINFO/MEMMAP/FB/GOP/EBS/KERNEL/FRAMEALLOC/PAGING/PERM/GUARD/PERMAUDIT/PERMAUDIT2/RMBZ all ok, THREAD-RUN id=2 then THREAD ok=1 id=2 top/entry hex, PF ok=1 vector=14 chained to EXC ok=1 vector=0).
- **Final:** REVIEW #287 risks 1-3 closed on-tree. #9 stays active. Next: callee-saved save/restore + round-trip stress.

## S06 slice9 — callee-saved save/restore + round-trip stress (2026-10-01)

- **Decision:** (a) kernel/thread.rs `run_on_thread_stack` pushes/pops the full callee-saved set (`r15` + `rbx`, `rbp`, `r12`-`r14`, symmetric order) on the caller stack so a hostile callee cannot corrupt the boot thread; (b) boot-uefi/main.rs adds a naked `thread_clobber_asm` stub (global_asm, trashes `rbx`/`rbp`/`r12`-`r14` then `ret`) plus `probe_thread_callee_saved`, which runs the stub on the `swap_target` top 16 times and emits fail-loud `THREAD-CS ok=1 iter=16`; (c) host test `swap_target_round_trip_stress` ping-pongs the picker 1024 times asserting bit-stable triples, alignment, nonzero entry; (d) `boot-uefi.ps1` gates on `THREAD-CS ok=1 iter=16`; (e) REVIEW #289 nit fixed (`cli` drops false `preserves_flags`).
- **Why:** Slice8 proved one live swap but left two gaps: callee-saved regs unpreserved (a real thread entry trashes them by ABI right) and single-shot picker coverage. The 16-iteration naked-clobber loop turns both into fail-loud proofs: any push/pop imbalance or missing spill corrupts the caller stack within iterations.
- **Alternatives:** Save/restore on the callee stack instead of caller (rejected: callee stack is untrusted by construction — spill must live on the owned caller side); wrap the clobber stub in a safe Rust fn (rejected: compiler prologue would mask the trash — must call the naked stub directly, hence the documented unsafe→safe transmute, same idiom as `probe_thread_swap`).
- **Expected:** default 120 (119 + 1 stress); kernel 89 (88 + 1); boot-uefi 18; fmt clean; UEFI target build warning-free; boot-uefi.ps1 PASS with THREAD + THREAD-RUN + THREAD-CS + PF/EXC.
- **Measured (2026-10-01, Win11):** `cargo test --workspace` all green — 21 (boot-info) + 18 (bootx64) + 89 (kernel, new stress) + 4 + 4 (serial-marker) + 2 (version); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets` exit 0; `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` Finished, zero warnings (fixed `function_casts_as_integer` via `as *const ()` first); `boot-uefi.ps1` UEFI BOOT PASS exit 0 (guest 92672 bytes, serial 1023 bytes: marker FOUND, all prior gates ok, `THREAD ok=1` + `THREAD-RUN id=2` + `THREAD-CS ok=1 iter=16` + `PF ok=1 vector=14` + `EXC ok=1 vector=0`).
- **Final:** Callee-saved preservation + picker stability proven end-to-end (host + guest). #9 stays active. Next: true two-way context switch (save/restore RSP pair) per roadmap slice order.

## S06 slice10 — new-stack RSP spill + 5-reg sentinel preservation + r15 clobber (2026-10-01)

- **Decision:** (a) kernel/thread.rs `run_on_thread_stack` spills caller `RSP` to the NEW stack (`push rax` after `mov rsp, top`, slot `[top-8]` above the `call` return address at `[top-16]`) instead of stashing it in `r15`, removing the single point of failure where a callee trashing `r15` loses the return stack; (b) new `callee_saved_survives` preservation proof sets 5 distinct sentinels (`rbx`, `r12`-`r15`) in ONE asm block, runs ONE swap, captures all five in ONE block and fails loud on any bit mismatch (`THREAD-CS ok=0`) — liveness alone cannot catch a swapped-but-balanced pop order (REVIEW #291 gap 1); `rbp` deliberately excluded (frame pointer — sentinel overwrite would corrupt frame-relative accesses), documented with the permutation argument (a swap needs two regs; any swap touching a checked reg misplaces its distinct sentinel); (c) guest naked clobber stub now trashes `r15` too and still passes, proving the SPoF path (REVIEW #291 gap 2); (d) REVIEW nit fixed: direct `transmute(*const ())` with no `usize`/`u64` provenance-losing round-trip.
- **Why:** REVIEW #291 named exactly two gaps on slice9: preservation-vs-liveness (gap 1) and the untested `r15`-as-spill path (gap 2). The old design stashed `RSP` in `r15` while the stress stub avoided `r15`, so the vulnerable path was unproven; the old `THREAD-CS ok=1` gate was liveness-only and would pass a balanced-but-permuted pop order.
- **Alternatives:** Spill `RSP` to a third static save slot instead of the new stack (rejected: shared mutable slot reintroduces reentrancy/ownership questions; the new stack above the call ret is owned, callee never touches it since the stub only writes regs + `ret`, and the callee grows down); include `rbp` in the sentinel set (rejected: overwriting the frame pointer across a Rust call corrupts the frame/epilogue — read-only `rbp` compare is the documented follow-up, REVIEW #293 risk 1); pin `in(reg)` inputs to fixed regs instead of `out` reservations (rejected: narrower than reservation — future input growth re-opens the hole; `rbx`/`rbp` exclusion rests on the LLVM-reserved/frame-pointer claim pinned for `cargo show-asm` disproof, REVIEW #293 risks 2-3).
- **Expected:** default 138 (21+18+89+4+4+2); kernel 89; boot-uefi 18; fmt clean; clippy host+uefi clean; UEFI build 0 warnings; boot-uefi.ps1 PASS with THREAD ok=1 + THREAD-RUN id=2 + THREAD-CS ok=1 iter=16 + PF/EXC.
- **Measured (2026-10-01, Win11):** `cargo test --workspace` 138/138 (21 boot-info + 18 bootx64 + 89 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi` exit 0; UEFI build 0 warnings; QEMU e2e PASS independently verified by agent-b (REVIEW #293): 1023B serial, THREAD ok=1 + THREAD-RUN id=2 + THREAD-CS ok=1 iter=16 + PF+EXC ok (full gate).
- **Final:** REVIEW #291 gaps 1-2 closed on-tree, REVIEW #293 cond-approve. #9 stays active. Next: true two-way RSP save/restore pair; REVIEW #293 risks 1-3 (read-only `rbp` compare, `rbx`/frame-pointer reservation disproof, release-profile FP pinning) carried as non-blocking follow-ups.

## S06 slice11 — read-only rbp preservation proof (REVIEW #293 risk 1) (2026-10-01)

- **Decision:** Close REVIEW #293 risk 1 in one minimal guest slice: (a) new guest-only `unsafe fn rbp_preserved(top, func) -> bool` in `kernel/thread.rs` captures `rbp` before/after one `run_on_thread_stack` call (`mov {v}, rbp` into `out(reg)`, never writes `rbp`) and compares bit-exact; host stub returns `true`; (b) `boot-uefi/main.rs` `probe_thread_callee_saved` runs it against the same hostile `thread_clobber_asm` stub (which trashes `rbp` with `0xBEEF...`) after the five-reg sentinel check — either proof failing emits `THREAD-CS ok=0`; line shape unchanged (`THREAD-CS ok=1 iter=16`), still hard-required by `boot-uefi.ps1`; (c) host shape test extended to bind both proof fns to `unsafe fn(u64, extern "C" fn()) -> bool`.
- **Why:** Slice 10 deliberately excluded `rbp` from the sentinel set (frame pointer — a sentinel write corrupts frame-relative accesses and the epilogue restore), with the permutation argument covering only swaps touching a checked reg. A missing/unbalanced `push rbp`/`pop rbp` would therefore still print `ok=1`. The cheapest disproof named in REVIEW #295 is exactly this read-only compare around one swap.
- **Alternatives:** Add `rbp` to the sentinel write set (rejected: overwrites the frame pointer across a Rust call — corrupts the caller frame by construction); separate `THREAD-CS2` serial line + ps1 gate (rejected: scope growth — the same `ok` bit already gates, folding keeps the e2e substring byte-identical); full two-way RSP save/restore now (rejected: larger slice — risk 1 is the smallest agreed follow-up, two-way stays next).
- **Expected:** default 138 (21+18+89+4+4+2, extended shape test only); fmt clean; clippy host+UEFI clean; UEFI build 0 warnings; `boot-uefi.ps1` PASS with `THREAD-CS ok=1 iter=16`.
- **Measured (2026-10-01, Win11):** `cargo test --workspace` 138/138 (21 boot-info + 18 bootx64 + 89 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; UEFI build Finished 0 warnings; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1023 bytes: marker FOUND, all prior gates ok, `THREAD ok=1` + `THREAD-RUN id=2` + `THREAD-CS ok=1 iter=16` (now includes passing read-only `rbp` proof) + `PF ok=1 vector=14` + `EXC ok=1 vector=0`.
- **Final:** REVIEW #293 risk 1 closed on-tree. #9 stays active. Next: true two-way RSP save/restore; risks 2-3 carried (`rbx`/reservation `show-asm` disproof, release-FP pinning).

## S06 slice12 -- two-way save-slot probe (first true RSP save/restore half) (2026-10-01)

- **Decision:** Land the first caller-memory half of true two-way RSP save/restore as one minimal guest slice: (a) new guest-only `unsafe fn run_on_thread_stack_with_save(old_save, new_top, func)` in `kernel/thread.rs` -- same six caller-saved pushes as `run_on_thread_stack`, then `mov [save], rax` spills the boot `RSP` to caller-owned memory BEFORE the switch, `mov rsp, top`, `push rax` (boot RSP) + `push save` (pointer spill, so post-`call` recovery never depends on a register surviving the hostile callee -- the slice-10 lesson applied to the pointer itself), `call func`, `pop rdx` + `pop rax`, `mov rsp, rax`, symmetric pops; null `old_save` fails closed in Rust before the asm (host-testable); `rdx` added to the `out` reservation set with the same `rbx`/`rbp` exclusion rationale; (b) `boot-uefi/main.rs` adds the `THREAD_RSP_SAVE` static slot (zeroed before each probe so a no-save path prints ok=0, never a stale nonzero), `write_thread_save_line` (`THREAD-SAVE ok=<0|1> save=<0x...>`), and `probe_thread_save` resolving the same `swap_target` triple, running `thread_b_entry` through the save-slot path, and requiring a nonzero 16-byte-aligned save (plus a `#[cfg(test)]` stub returning `Ok(true)` so the host bin-test build compiles, mirroring `probe_thread_callee_saved`); (c) host test `with_save_null_fails_closed_host_stub` (null no-trap, non-null leaves slot untouched) plus shape-test binding to `unsafe fn(*mut u64, u64, extern "C" fn())`; (d) `boot-uefi.ps1` hard-requires `THREAD-SAVE ok=1` (missing = FAIL).
- **Why:** Slices 7-11 prove one-way switching (boot RSP lives only on the new stack, recoverable for call/return but not ping-pongable between two owned stacks). REVIEW #297 names true two-way RSP save/restore as the explicit next step. The smallest committable half is save-to-caller-memory plus return: it proves the spill lands in owned memory with the right value while keeping the proven new-stack return slot, so full A->B->A ping-pong (next slice) only adds the second slot plus an entry-through-jump path.
- **Alternatives:** Full A->B->A ping-pong now (rejected: needs a second save slot, re-entry discipline, and a jump-not-call entry path -- doubles single-cycle guest-asm risk; save+return is independently provable on the existing call/return shape); spill only the pointer without `mov [save], rax` (rejected: then the slot is never written pre-switch and the probe checks nothing new -- the memory write IS the new proof); drop the new-stack return slot and rely only on the save slot (rejected: removes the proven fallback in the same cycle that introduces the new path -- belt and suspenders until ping-pong lands).
- **Expected:** default 139 (138 + 1 null-guard); kernel 90 (89 + 1); boot-uefi 18; fmt clean; clippy host+UEFI clean; UEFI build 0 warnings; `boot-uefi.ps1` PASS with `THREAD-SAVE ok=1`.
- **Measured (2026-10-01, Win11):** `cargo test --workspace` 139/139 (21 boot-info + 18 bootx64 + 90 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; UEFI build Finished 0 warnings; `boot-uefi.ps1` UEFI BOOT PASS exit 0 (guest 95232 bytes, serial 1071 bytes = 1023 + THREAD-RUN + THREAD-SAVE lines: marker FOUND, all prior gates ok, `THREAD ok=1` + `THREAD-RUN id=2` + `THREAD-CS ok=1 iter=16` + `THREAD-RUN id=2` + `THREAD-SAVE ok=1 save=0xfe64790` (nonzero, 16-aligned) + `PF ok=1 vector=14` + `EXC ok=1 vector=0`).
- **Final:** First two-way half proven end-to-end (host null-guard + guest save-value proof). #9 stays active. Next: full A->B->A ping-pong through two save slots; REVIEW #297 risks (a)(b) (`rbx`/FP reservation `show-asm` disproof, release-FP pinning) carried, still non-blocking.

## S06 slice13 -- full A->B->A ping-pong via memory slots (2026-10-01)

- **Decision:** Land full A->B->A ping-pong as one minimal guest slice: (a) new guest-only `unsafe fn swap_stacks_via_slots(old_save, new_save, func)` in `kernel/thread.rs` -- same six caller-saved pushes, `mov [old], rax` spill while on the current stack, `mov rdx, [new]` LOAD of the target `RSP` from caller memory in asm (the missing LOAD half REVIEW #299 demanded), `mov rsp, rdx`, `push rax` + `push old` pointer spill, `call func`, symmetric restore; fail-closed in Rust on null slots or zero/misaligned `[new]` target (host-testable, volatile slot access); plain `out` (not `lateout`) for all scratch per the slice-10 reservation rule; (b) `boot-uefi/main.rs` adds `THREAD_RSP_SAVE_B` slot, `thread_pong_entry` (emits `THREAD-PONG-RUN id=1` on the loaded boot stack), `thread_b_pingpong_entry` (nests the slot-to-slot hop inside the outer save-slot hop), `probe_thread_pingpong` (zeroes both slots pre-run, runs the outer hop, requires both slots nonzero/16-aligned, emits `THREAD-PONG ok=1 save_a/save_b`); (c) host test `swap_via_slots_guards_fail_closed_host_stub` (null/zero/misaligned fail closed, valid no-op) plus shape-test binding; (d) `boot-uefi.ps1` hard-requires `THREAD-PONG ok=1` + `THREAD-PONG-RUN id=1`.
- **Why:** Slices 7-12 prove one-way switching plus save-to-memory, but never LOAD a saved `RSP` back as a live stack pointer. REVIEW #299 named exactly this STORE+LOAD gap. The smallest committable proof is a nested B->A hop through two caller-owned slots on the existing call/return shape (no jump-not-call entry path yet).
- **Alternatives:** Full jump-not-call thread entry now (rejected: needs re-entry discipline beyond call/return -- doubles single-cycle guest-asm risk; nested call proves the LOAD with the proven epilogue); `lateout` scratch outputs (rejected empirically: tried, QEMU FAIL `THREAD-SAVE ok=0 save=0x0`, slot never written -- plain `out` forbids input/output overlap while `lateout` allows reuse after inputs are consumed, premise corrected per REVIEW #299 risk-1 inversion, reverted to plain `out`); second slot as stack-local instead of static (rejected: must survive across the nested call boundary -- caller-owned static keeps ownership explicit post-EBS single-threaded).
- **Expected:** default 140 (139 + 1 slot-guard); kernel 91 (90 + 1); boot-uefi 18; fmt clean; clippy host+UEFI clean; UEFI build 0 warnings; `boot-uefi.ps1` PASS with `THREAD-PONG ok=1` + `THREAD-PONG-RUN id=1`.
- **Measured (slice13 commit @e7288c7, Win11):** `cargo test --workspace` 140/140 (21 boot-info + 18 bootx64 + 91 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; UEFI build Finished 0 warnings; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1143 bytes: all prior gates ok plus `THREAD-SAVE ok=1 save=0xfe64790`, `THREAD-PONG ok=1 save_a=0xfe64770 save_b=0xde38f10` (both nonzero, 16-aligned), `THREAD-PONG-RUN id=1`. Re-verified docs-close-out cycle: host 140/140 same split, fmt clean, host clippy clean (QEMU carried, guest untouched this cycle).
- **Final:** STORE+LOAD both in asm proven end-to-end; REVIEW #299 closed (LOAD half + out-vs-lateout correction recorded in thread.rs docs). REVIEW #301 cond-approve accepted with 3 next-slice gaps carried: (1) new slot path has no sentinel/`rbp` proof, (2) pingpong probe bypasses the Thread-table entry (calls B entry directly), (3) host-stub empty-if nit. #9 stays active. Next: slice14 preservation proof for the slot path + entry-bypass fix.

## S06 slice14 -- close REVIEW #303 gaps (slot-path proof, triple entry, read-once RSP) (2026-10-01)

- **Decision:** Close all three REVIEW #303 gaps in one minimal guest slice: (a) slot-path proof -- new `callee_saved_survives_with_save` / `rbp_preserved_with_save` / `callee_saved_survives_via_slots` / `rbp_preserved_via_slots` mirror the slice10 protocol on the two new asm paths, and `probe_thread_callee_saved` runs the hostile clobber stub through all three paths with slot-nonzero guards before each proof; (b) entry bypass -- outer ping-pong hop resolves B entry via `swap_target` triple (`entry_b` + equality check vs `pingpong_addr`) and nested B->A resolves A entry via `swap_target(&[a,b],2)`, so a zero/mismatch fails closed and leaves slot B unwritten (outer reports ok=0); transmute operates on a triple-validated u64, not a hardcoded fn pointer; (c) read-once -- `swap_stacks_via_slots` takes the already-loaded `new_rsp` value (`mov rdx,{new}`), asm never re-reads slot memory for the target (STORE half still proven in asm; slice13 LOAD retired for TOCTOU safety under cli single-thread).
- **Why:** REVIEW #303 named exactly these three gaps on slice13: the two new epilogues had liveness only (a push/pop imbalance confined to either new path would still print ok=1), the B entry was a direct call bypassing the Thread-table triple, and asm LOAD re-read slot memory after the Rust guard (TOCTOU shape). Each is now fail-loud: proof mismatch or triple mismatch forces ok=0.
- **Alternatives:** Separate THREAD-CS2/CS3 lines + ps1 gates per new path (rejected: scope growth -- the same `THREAD-CS ok` bit already gates, folding keeps the e2e substring byte-identical); jump-not-call thread entry now (rejected: larger slice -- nested call proves the LOAD/store with the proven epilogue, jump discipline stays future); keep asm LOAD plus Rust guard (rejected: double-read shape -- single `read_volatile` in B entry with the value traveling in reg is the smaller TOCTOU surface).
- **Expected:** default 140 (no new host tests, shape/guard tests only); kernel 91; boot-uefi 18; fmt clean; clippy host+UEFI clean; UEFI build 0 warnings; `boot-uefi.ps1` PASS with `THREAD-PONG ok=1` + `THREAD-PONG-RUN id=1` + `THREAD-CS ok=1 iter=16`.
- **Measured (slice14 commit @81cc614, Win11):** `cargo test --workspace` 140/140 (21 boot-info + 18 bootx64 + 91 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; UEFI build Finished 0 warnings; `boot-uefi.ps1` UEFI BOOT PASS exit 0: all prior gates ok plus `THREAD-PONG ok=1 save_a=0xfe64760 save_b=0xde37cd0` (both nonzero, 16-aligned), `THREAD-PONG-RUN id=1`, `THREAD-CS ok=1 iter=16` (now covering all three asm paths). Independently verified by agent-b REVIEW #305: APPROVED, 140 green re-verified, 3 gaps closed gap-by-gap.
- **Final:** REVIEW #303 gaps 1-3 closed on-tree, REVIEW #305 APPROVED. #9 stays active. Next: worklog close-out (this entry) + MEMORY refresh, then next S06 slice (scheduler/priority or jump-not-call entry per roadmap order).

## S06 slice15 -- cooperative ThreadMutex sync primitive (2026-10-01)

- **Decision:** Land the first S06 sync primitive as one host-testable slice: new `kernel/sync.rs` `ThreadMutex` (cooperative `Cell<u64>` holder, 0 = free null-thread sentinel, `try_lock`/`unlock` fail-closed with no mutation on id==0 / foreign / re-lock / non-holder unlock, `!Sync` by construction so cross-thread sharing must use a future atomic primitive) plus 4 host tests (round-trip, null-id, double/foreign refused, wrong-owner unlock). Guest untouched (no QEMU gate change); `lib.rs` only adds `pub mod sync`.
- **Why:** Slices 7-14 prove context switching but threads share nothing yet; roadmap S06 needs a sync primitive before scheduler/join. Smallest committable proof is a cooperative mutex with a locked failure model (no spin/block/deadlock risk in QEMU single-thread `cli` bring-up).
- **Alternatives:** Atomic spinlock now (rejected: post-EBS boot CPU is `cli` single-thread, atomics add no signal and complicate the host contract); blocking mutex with waiter queue (rejected: needs scheduler that does not exist yet); guest QEMU gate for lock/unlock (rejected: host unit tests prove the contract, guest path is a future scheduler slice).
- **Expected:** default 126 (21+95+4+4+2, kernel 91+4) + boot-uefi 18 = 144 total; fmt clean; clippy host+UEFI clean; `boot-uefi.ps1` PASS unchanged (guest untouched).
- **Measured (slice15 commit @3303aa6 + close-out re-verify, Win11):** `cargo test` 126/126 default (21 boot-info + 95 kernel + 4 + 4 serial-marker + 2 version) + `cargo test -p baseos-boot-uefi` 18/18 = 144; `cargo fmt --check` clean (exit 0); `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0 re-verified on the slice15 tree, serial 1143 bytes (guest 99328 bytes, marker FOUND, all prior gates ok incl. THREAD-PONG + PONG-RUN + CS16 + SAVE + PF14 + EXC0). REVIEW #313 blockers (clean-tree proof + #9/sync docs) closed by this close-out commit.
- **Final:** First sync primitive proven host-side with QEMU regression PASS. #9 ready to retitle slice1->slice15 + #10 ready to close pending agent-b REVIEW; risks carried: rbx/show-asm, release-FP, interrupt TOCTOU. Next: slice16 scheduler/jump-entry per roadmap.

## S06 slice16 -- cooperative Scheduler replaces mutex Cell + shared NULL_THREAD (2026-10-01)

- **Decision:** Land the S06 scheduler table as one host-testable slice: new `kernel/sched.rs` `Scheduler` (owns up to `MAX_THREADS` descriptors + current id, `spawn`/`pick_next`/`dispatch`, `&mut self` mutation only, pick path `&self` never mutates) delegating picks to `thread::switch_stub` so empty/unknown/corrupt/nothing-Ready refuse exactly as the picker does; new shared `thread::NULL_THREAD = 0` single source (`thread` + `sync` + `sched` compare against one const, REVIEW #319 finding 3); `sync.rs` sentinel `0` -> `NULL_THREAD` (no behavior change) plus docs recording the scheduler REPLACES the coop `Cell` (get-then-set is interrupt-TOCTOU under preemption, REVIEW #319 finding 2). Guest untouched (no QEMU gate change).
- **Why:** Slices 7-15 prove switching + coop mutex but have no table owner; roadmap S06 needs a scheduler before join/preempt. Smallest committable proof is a cooperative table with a locked failure model (no atomics/spin under post-EBS `cli` single-thread).
- **Alternatives:** Reuse `ThreadMutex` cell inside the scheduler (rejected: REVIEW #319 finding 2 -- coop `Cell` get-then-set double-takes the lock under preemption; replace-not-reuse keeps the TOCTOU out by construction); atomic spinlock table now (rejected: `cli` single-thread adds no signal, complicates the host contract); guest multi-thread switch in the same slice (rejected: doubles single-cycle guest-asm risk; table must exist first).
- **Expected:** 150 host green; fmt clean; clippy host+UEFI clean; `boot-uefi.ps1` PASS unchanged (guest untouched).
- **Measured (slice16 commit @da254ae, Win11):** `cargo test --workspace` 150/150 green; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; QEMU carried (guest untouched, 1143B baseline stands).
- **Final:** Scheduler table proven host-side. #9 stays active. Next: hardening residuals + guest-switch wiring; risks carried: rbx/show-asm, release-FP, preempt TOCTOU (documented-not-tested).

## S06 slice16 hardening -- REVIEW #321 residuals a-c (first-dispatch doc, packed pin, block-comment Cell pin) (2026-10-01)

- **Decision:** Close REVIEW #321 residuals a-c in one `sched.rs`-only slice: (a) first-dispatch boot bookkeeping documented on `new()`+`dispatch()` (spawned descriptor stays `Ready`, first `dispatch` parks nothing, benign under coop yield) + pinned by new `first_dispatch_parks_nothing_boot_thread_stays_ready` test (dispatch 1->2 then 2->1); (b) `packed()` full-capacity copy kept with red-to-green doc (prefix-only `[..len)` breaks 4 spawn validations -- honest, correct; callers slice len/len+1 so filler unreachable); (c) no-`Cell` pin now strips `/* */` block comments + panics on unterminated, so a banned token hidden in a block comment cannot slip past the doc-line filter.
- **Why:** REVIEW #321 named exactly these three host-side gaps; each is docs+test-only with zero guest risk, cheapest to close before slice17 guest-switch wiring.
- **Alternatives:** Mark `current` Running in `new()` (rejected: changes the spawn-then-dispatch contract mid-stream; doc + pin keeps Ready-first visible); prefix-only packed loop (rejected empirically: tried, 4 red -- pending-slot inclusion is load-bearing for spawn pre-commit validation); line-comment strip only (rejected: block-hidden token is the exact hole).
- **Expected:** 151 host green (150 + 1); fmt clean; clippy host+UEFI clean; QEMU carried (guest untouched).
- **Measured (@fe1dd55 + re-verify this cycle, Win11):** `cargo test --workspace` 151/151 (21 boot-info + 18 bootx64 + 102 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean (exit 0); `cargo clippy --all-targets -- -D warnings` clean; QEMU `boot-uefi.ps1` carried (kernel-only HEAD, guest untouched, 1143B baseline stands). Independently verified by agent-b REVIEW #323: ACCEPT a-c host-side, 151 green re-verified, fmt/clippy clean.
- **Final:** Residuals a-c closed on-tree, REVIEW #323 accepted provisional. #9 stays active. Next: slice17 guest-switch wiring (preempt TOCTOU documented-not-tested, #9 retitle) + REVIEW #323 test-strength residuals (string-literal block-strip false-negative, first-dispatch state asserts) as non-blocking follow-ups.

## S06 slice17a -- scheduler test-strength (strip-code lexer + dispatch state asserts) (2026-10-01)

- **Decision:** Close REVIEW #325 gates 1+2 in one `sched.rs`-only slice: (a) new `Scheduler::state_of(id) -> Option<ThreadState>` read-only query (`None` on unknown id, never mutates) so first-dispatch bookkeeping is assertable without `&mut` table access; (b) `first_dispatch_parks_nothing` test now pins STATES not just ids (boot stays `Ready` on first dispatch, parked only from the second onward -- a rewrite flipping ids right but parking wrong goes red); (c) replace the slice-16 block-only strip with a string/char-aware nesting `strip_code` lexer (blanks `"..."`/`'...'` with escapes honored, tracks block depth, rejoins split tokens) so a `"` inside a block comment or a `/*` inside a literal no longer desyncs the no-`Cell` pin.
- **Why:** REVIEW #325 named exactly these two test-strength gaps on the slice-16 hardening: the state contract was id-only (parking bug invisible) and the block-strip was string-blind (quote-in-comment swallowed real code, marker-in-string started a phantom comment, split-token reliance was accidental). Both are host-test-only with zero guest risk.
- **Alternatives:** Keep id-only asserts plus doc (rejected: parking is the load-bearing invariant, ids alone cannot see it); line-comment strip only (rejected: same hole one level down); third-party lexer crate (rejected: ~150-line test-only fn, zero new deps).
- **Expected:** 152 host green (151 + 1 kernel: lexer test; state asserts extend an existing test); fmt clean; clippy host+UEFI clean; QEMU carried (guest untouched).
- **Measured (@b096e9e, Win11):** `cargo test --workspace` 152/152 (21 boot-info + 18 bootx64 + 103 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; QEMU carried (kernel-only HEAD, guest untouched, 1143B baseline stands).
- **Final:** Gates 1+2 closed on-tree. #9 stays active. Next: REVIEW #328 residuals (raw-string + lifetime hardening) then guest-switch wiring.

## S06 slice17b -- sched lexer raw-string + lifetime hardening + state_of None pin (2026-10-01)

- **Decision:** Close REVIEW #328 residuals (a)+(b) in one `sched.rs`-only slice: (a) `strip_code` additionally blanks raw strings (`r"..."`, `r#"..."#`, `r##"..."##`, same with `br` prefix -- no escapes, close is `"` + opening `#` count) and treats lifetimes (`'a`, `'static`) as code not char literals (only `'x'`/`'\..` shapes open a literal); (b) `state_of` unknown-id `None` pinned before and after dispatch (`99`, `NULL_THREAD`). Without this a raw `r"/*Cell*/"` literal starts a phantom block comment (swallows real code to the next `*/`) and a lifetime `'a` opens a phantom char literal (swallows code to the next `'`).
- **Why:** REVIEW #328 showed the 17a lexer still desynced on exactly these two real-Rust shapes -- the same class gate 1 was built to prevent. Both are host-test-only, zero guest risk.
- **Alternatives:** Forbid raw strings/lifetimes in `sched.rs` by convention (rejected: unenforceable, reviewer-supplied disproofs are valid Rust); full `syn`/`proc-macro` lexer (rejected: zero-dep test fn stays proportionate); separate `THREAD-CS`-style serial gate (rejected: host pin is the proof, no guest behavior changes).
- **Expected:** 152 host green (no new count -- assertions extend existing tests); fmt clean; clippy host+UEFI clean; QEMU carried.
- **Measured (@9d47bbc, Win11):** `cargo test --workspace` 152/152 (21 + 18 + 103 + 4 + 4 + 2); `cargo fmt --check` clean; host + UEFI-target `clippy -D warnings` clean; QEMU carried (kernel-only HEAD, guest untouched).
- **Final:** Residuals (a)+(b) closed on-tree. #9 stays active. Next: REVIEW #330 residuals then guest-switch wiring.

## S06 slice17c -- sched lexer ident-boundary + rb symmetry + disproof pins (2026-10-01)

- **Decision:** Close REVIEW #330 residuals in one `sched.rs`-only slice: (a) raw prefix requires an ident boundary (`r`/`b` must not follow `[0-9A-Za-z_]`, so `bar#"fake"` lexes as ident + normal string, never a raw open -- without this it false-opens a 1-hash raw with no `"#` close and blanks the real `Cell<` to EOF); (b) `rb` handled symmetrically with `br` (superset blanking -- would blank as a normal string either way); (c) disproof pins for both plus glued-prefix cases. Known doc-only limits recorded: multi-byte char literals fall back to lifetime-shape (single-char body can never hide `Cell<`), unterminated raw blanks to EOF (compile error -- no real code after matters).
- **Why:** REVIEW #330's cheapest disproof showed the 17b lexer still false-opened on a glued prefix -- the last known desync of the no-`Cell` pin. Host-test-only, zero guest risk.
- **Alternatives:** Full Unicode ident-boundary table (rejected: ASCII `[0-9A-Za-z_]` covers the `r`/`b` prefix adjacency that matters; multi-byte bodies cannot spell the ASCII token); reject `rb` as invalid instead of blanking (rejected: superset blanking is sound either way and one branch cheaper).
- **Expected:** 152 host green; fmt clean; clippy host+UEFI clean; QEMU carried.
- **Measured (@e5810f7 + re-verify this cycle, Win11):** `cargo test --workspace` 152/152 (21 boot-info + 18 bootx64 + 103 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean (exit 0); `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; QEMU carried (kernel-only HEAD, guest untouched, 1143B baseline stands).
- **Final:** Lexer pin hardened through 17a/b/c with state asserts alongside. #9 stays active. Next: guest-switch wiring + preempt-TOCTOU doc (documented-not-tested) + #9 retitle slice1->slice17 via #11.

## S06 slice17d -- sched lexer br-only + boundary + multibyte pins (REVIEW #332 risks 1-3) (2026-10-01)

- **Decision:** Close REVIEW #332 risks 1-3 in one `sched.rs`-only slice (guest untouched, no QEMU): (a) risk 1 -- remove the symmetric-`rb` raw-open branch; raw opens only on `r` / `br` (`r->Some(i)`, `b+r->Some(i+1)`), so `rb"..."` falls through to ident `r`,`b` + normal-string blank and `rb#"fake"` lexes `r`,`b`,`#` + normal string with the trailing `Cell<` surviving (a symmetric-`rb` open would seek a `"#` close, miss, and over-blank to EOF -- removal is a true fix, not a superset); `br"..."` still raw-opens; (b) risk 2 -- ident boundary pinned both sides (`prev_ident` covers `0-9A-Za-z_`, so `_r` blocked per `under_r` test, offset-0 `r` IS raw per `start_raw` test); (c) risk 3 -- multibyte `'é'` (bytes `27 C3 A9 27`) falls to lifetime-shape yet keeps trailing `Cell<` (test `multi` pins the one case; UTF-8 continuation bytes `C3`/`A9` can never equal ASCII `"/#/'`, so no `Cell<` bypass is plausible but still doc-only, not proven; unterminated-raw-to-EOF stays doc-only by design as `E0748` compile-error input).
- **Why:** REVIEW #332 named exactly these three lexer gaps on slice17c: `rb` validity (`br"x"` compiles as `&[u8]`, `rb"x"` fails ident-parse), ident-boundary both sides, and the multibyte/`E0748` doc limits. All are host-test-only with zero guest risk; the `rb_hash` pin is the load-bearing disproof.
- **Alternatives:** Keep symmetric-`rb` blanking as superset (rejected empirically: over-blanks valid Rust to EOF on `rb#"fake"` -- strictly worse than the correct ident+string lex); full Unicode ident-boundary table (rejected: ASCII covers the `r`/`b` prefix adjacency; multibyte bodies cannot spell the ASCII token); `syn` lexer crate (rejected: zero-dep test fn stays proportionate).
- **Expected:** 152 host green; fmt clean; clippy host+UEFI clean; QEMU carried (guest untouched).
- **Measured (@358dcaa, Win11):** `cargo test --workspace` 152/152 (21 boot-info + 18 bootx64 + 103 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; QEMU carried (kernel-only HEAD, guest untouched, 1143B baseline stands). Independently verified by agent-b REVIEW #334: COND-ACCEPT host-only, `git diff HEAD~1 HEAD` is `sched.rs`-only (48+/22-), `cargo test -p baseos-kernel` 103/103 re-verified; risks 1-2 FIXED/PINNED, risk 3 WEAK-ACCEPT (one-case pin + plausibility, not proof).
- **Final:** Risks 1-3 closed on-tree per #334 (risk 3 as documented-not-proven). #9 stays active, #11 stays blocked on orchestrator rename. Next: slice18 guest-switch wiring + TOCTOU doc + #9 retitle.

## S06 slice18 -- minimal scheduler-driven guest switch + preempt-TOCTOU doc (2026-10-02)

- **Decision:** First guest-switch wiring as one minimal slice (REVIEW #336 cheapest disconfirm): (a) `kernel/sched.rs`: preempt-TOCTOU paragraph (documented-not-tested -- `pick_next`/`dispatch` read-then-write via `&mut self` with no CAS/`cli` gate; caller holds the critical section, `cli` single-thread today) + new `dispatch_agrees_with_swap_target_parity` host test (scheduler pick == swap-gate triple id); (b) `boot-uefi/main.rs`: new `probe_sched_switch` (guest `#[cfg(not(test))]` builds the 1+2 descriptors, `Scheduler::new(1)` spawn/spawn, `dispatch` expecting 2, `swap_target` triple for the same pair, requires dispatch == triple == 2 with nonzero entry, runs the triple entry via `run_on_thread_stack`, emits `SCHED-SWITCH ok=<0|1> from=1 to=<n>`; `#[cfg(test)]` stub covers the dispatch/triple parity shape without live `RSP` -- the kernel dependency builds with the guest asm path on host, so the live call must never execute under `cargo test`) + new `sched_switch_probe_parity_host` test (expects `ok=1 from=1 to=2`); wired after the pong probe in `efi_main`; (c) `boot-uefi.ps1` hard-requires `SCHED-SWITCH ok=1 from=1 to=2` (missing = FAIL).
- **Why:** Slices 7-17 prove switching primitives plus a host-only scheduler table, but the guest never picks through `Scheduler` -- dispatch-through-table vs direct-`swap_target` disagreement would run the wrong entry. Smallest end-to-end proof is one scheduler-driven pick + agreed triple + executed entry with a fail-loud line, keeping the proven call/return shape (no jump-not-call entry yet).
- **Alternatives:** Full jump-not-call thread entry now (rejected: needs re-entry discipline beyond call/return -- doubles single-cycle guest-asm risk; nested call proves dispatch+LOAD with the proven epilogue); guest `Scheduler` state carried across probes with a static (rejected: shared-mutable slot reintroduces ownership questions; probe-local table keeps ownership explicit post-EBS single-threaded); host-live `run_on_thread_stack` in the new test (rejected empirically: `STATUS_PRIVILEGED_INSTRUCTION` -- the kernel dependency links the guest asm path under boot-uefi `cargo test`, so the stub must avoid the live call, same pattern as `probe_thread_save`).
- **Expected:** 154 host green (152 + 1 kernel parity + 1 boot-uefi parity); fmt clean; clippy host+UEFI clean; gate 41/41; `boot-uefi.ps1` PASS with `SCHED-SWITCH ok=1 from=1 to=2`.
- **Measured (Win11):** `cargo test --workspace` 154/154 (21 boot-info + 19 bootx64 + 104 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1189 bytes (1143 + 46 SCHED line), guest `bootx64.efi` 104448 bytes, tail `THREAD-PONG ok=1` + `SCHED-SWITCH ok=1 from=1 to=2` + `PF ok=1 vector=14 code=0x0 addr=0x500000000000` + `EXC ok=1 vector=0`. One `clippy::doc-lazy-continuation` fix (`+ \`dispatch\`` list-marker reword to `and`) before green.
- **Final:** Scheduler pick and swap-gate triple proven to agree host-side and guest-side with a hard-required QEMU line. #9 stays active. Next: fuller context-switch wiring (jump entry / multi-dispatch) per roadmap; risks carried: rbx/show-asm, release-FP, preempt TOCTOU (now documented-not-tested).

## S06 slice19 -- persistent two-hop sched switch + ok=0 pins (2026-10-02)

- **Decision:** Close REVIEW #338 R1+R4 cheapest: (a) `kernel/sched.rs`: new `dispatch_two_hops_persistent_and_refusals_leave_table_untouched` host test (1->2 parks nothing, 2->1 parks Running, plus empty/unknown/nothing-Ready refusals leave current+states untouched); (b) `boot-uefi/main.rs`: `probe_sched_switch` (both cfgs) does a second `dispatch` 2->1 with B->A `swap_target` agreement after the live 1->2 run (resolved only, thread-1 entry 0x1000 is a placeholder), line shape unchanged; new `sched_switch_fail_closed_line_shape` test pins `SCHED-SWITCH ok=0 from=1 to=0/2` writer shape.
- **Why:** #338 R1: probe-local table discarded after one dispatch overstated `scheduler-driven`; same-table second hop proves persistence/multi-dispatch. R4: fail-closed ok=0 paths had no host pins. Second-hop entry not executed (placeholder RIP, not callable) keeps guest risk minimal.
- **Alternatives:** Full jump-not-call entry now (rejected: re-entry discipline beyond call/return, doubles guest-asm risk); static persistent Scheduler across probes (rejected: shared-mutable ownership questions; same-probe second hop proves persistence with explicit ownership); QEMU re-run this cycle (deferred: guest line shape unchanged, host-verified + QEMU carried).
- **Expected:** 156 host green; fmt/clippy clean; QEMU carried (line shape unchanged).
- **Measured (Win11):** `cargo test --workspace` 156/156 (21 boot-info + 20 bootx64 + 105 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; QEMU carried (guest line `SCHED-SWITCH ok=1 from=1 to=2` unchanged, re-run owed next slice with jump/multi work).
- **Final:** Persistent table + fail-closed shapes pinned host-side; guest proves two-hop agreement live on first hop. #9 stays active. Next: jump-not-call entry + QEMU re-run.

## S06 slice19a -- REVIEW #340 finding 1: drop dead n-block, pin Done-target NotReady no-mutation (2026-10-02)

- **Decision:** One `kernel/sched.rs`-only fix: drop the dead `n` block (built 1+2, dispatched 1->2->1, asserted nothing) and replace with a true NotReady no-mutation pin -- pre-set thread-1 `Running` + thread-2 `Done` before `spawn` (`spawn` carries state through; `validate_threads` gates stacks/ids only), then `pick_next` + `dispatch` both refuse `NotReady` with `current`, states, and `len` untouched.
- **Why:** REVIEW #340 finding 1: the `n` block exercised dispatches but pinned nothing, while the Done-target refusal path (current Running, other Done, no Ready) had no pin that states stay untouched.
- **Alternatives:** Keep `n` block plus add new test (rejected: dead code in test, same count, more lines); drive Done via scheduler API (rejected: no lifecycle transition API -- direct `set_state` before `spawn` is the explicit path).
- **Expected:** 156 host green (no count change -- assertions extend existing test); fmt/clippy clean; QEMU carried.
- **Measured (@d66fa45, Win11):** `cargo test --workspace` 156/156 (21 + 20 + 105 + 4 + 4 + 2); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; QEMU carried (kernel-only change, guest line unchanged).
- **Final:** Finding 1 closed on-tree. #9 stays active. Next: slice19b clippy fix + jump-not-call entry + QEMU re-run.

## S06 slice19b -- fix clippy unit_arg in sched host tests (2026-10-02)

- **Decision:** Test-only `boot-uefi/main.rs` fix: expand `|b| Ok(out.push(b))` to block form `|b| { out.push(b); Ok(()) }` in 3 spots (`sched_switch_probe_parity_host` + 2x `sched_switch_fail_closed_line_shape`) so `()` from `push` is not passed as `Ok` arg.
- **Why:** `cargo clippy --all-targets -- -D warnings` (with `--tests` per d66fa45 lesson) flags `unit_arg`; zero behavior change, line shape unchanged.
- **Alternatives:** `#[allow]` (rejected: hides real lints); change writer signature (rejected: guest path uses same shape).
- **Expected:** 156 host green; fmt clean; clippy host+UEFI clean; QEMU carried.
- **Measured (@5d73543, Win11):** `cargo test --workspace` 156/156 (21 + 20 + 105 + 4 + 4 + 2); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy --target x86_64-unknown-uefi --tests -- -D warnings` clean; QEMU carried.
- **Final:** Clippy green with `--tests` on both targets. #9 stays active. Next: jump-not-call entry + QEMU re-run.

## S06 slice19b repro -- independent clean-tree verification for REVIEW #348 (2026-10-02)

- **Decision:** No code change; independent repro of @5d73543 on clean tree to close REVIEW #348 ACK-UNVERIFIED.
- **Why:** #348: prior cycle reported 156-green with checks SKIPPED, so committed-clean did not prove reproduced.
- **Alternatives:** None -- repro is the cheapest disconfirm.
- **Expected:** 156 green; fmt + both clippy lines clean.
- **Measured (this cycle, Win11, @5d73543 clean):** `cargo test --workspace` 156/156 (21 + 20 + 105 + 4 + 4 + 2, 0 failed); `cargo fmt --check` clean (exit 0); `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy --target x86_64-unknown-uefi --tests -- -D warnings` clean.
- **Final:** #348 repro owed closed on-measure; risks still owed unchanged: (1) ok=0 funnel can hide failures, (2) jump-not-call entry + QEMU re-run next, (3) fragile literals/diag. #9 stays active; #11 retitle owed by agent-b.

## S06 slice20 -- QEMU re-verify after 3-deep host-only stack (2026-10-02)

- **Decision:** No code change; QEMU re-verify the unchanged guest on the current tree (@5797b1d) after three stacked host-only slices (19 two-hop, 19a Done-target pin, 19b clippy fix), same precedent as S04 slices 26/30.
- **Why:** REVIEW #352 holds ACK-UNVERIFIED with QEMU drift as part of risk (2); three host-only skips need a parity restore before the jump-not-call slice touches the guest. Cheapest disconfirm is the full gate + boot run, not inspection.
- **Alternatives:** Fold the re-verify into the jump-entry slice (rejected: stacks a fourth skip; one QEMU run now restores the non-interference baseline); commit a serial/log artifact (rejected: logs stay gitignored under `target/`; commit message + this row is the proof).
- **Expected:** `cargo test --workspace` 156/156 (21+20+105+4+4+2); fmt clean; clippy host + uefi-tests clean; gate 41/41; `boot-uefi.ps1` PASS 1189B with `SCHED-SWITCH ok=1 from=1 to=2`.
- **Measured (2026-10-02, Win11, @5797b1d):** `cargo test --workspace` 156/156 (21 boot-info + 20 bootx64 + 105 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy --target x86_64-unknown-uefi --tests -- -D warnings` clean; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1189 bytes, guest `bootx64.efi` 104448 bytes (identical to slice18 baseline): marker FOUND, all prior gates ok, `THREAD-PONG ok=1`, `SCHED-SWITCH ok=1 from=1 to=2`, `PF ok=1 vector=14 code=0x0 addr=0x500000000000` + `EXC ok=1 vector=0`.
- **Final:** QEMU/guest parity restored on the current tree; slices 19/19a/19b stand verified. Remaining per #352: (1) ok=0 funnel (no failure-behavior evidence), (2) jump-not-call entry (QEMU half now closed, entry half still open), (3) fragile literals/diag. Next: jump-not-call entry slice (narrow, one risk). #9 stays active.

## S06 slice21 -- jump-entry gate narrow (host-only contract, no asm yet) (2026-10-02)

- **Decision:** Narrow first half of the jump-not-call entry risk as one host-only slice in `crates/kernel/src/thread.rs`: new pure `const check_jump_entry(new_top, entry)` (nonzero + 16B-aligned top, never `u64::MAX` sentinel, nonzero entry, mirroring the `swap_target` tail) plus docs pinning the call-vs-jump discipline (`call` pushes a return address, `jmp` pushes none -- entry must never `ret`, caller must save RSP elsewhere first). One host test `jump_entry_gate_matches_swap_triple` (valid triple passes; zero/misaligned/sentinel/zero-entry fail). No guest asm, no ps1 change.
- **Why:** REVIEW #354 agrees the next slice is narrow jump-entry, one risk only. Full jump asm (RSP load + `jmp` + never-return entry + QEMU proof) doubles single-cycle guest risk; the gate must exist first so the asm cannot drift to a weaker check, same precedent as slices 1-6 (contract before live swap).
- **Alternatives:** Full jump asm + guest probe now (rejected: needs never-return entry discipline + QEMU fault proof in one cycle -- next slice); ok=0 funnel disconfirm now (rejected: agreed one-risk-only, jump first per #354); no-op doc without the `const` gate (rejected: doc-only cannot fail a weak future check).
- **Expected:** `cargo test --workspace` 157/157 (21+20+106+4+4+2); fmt clean; clippy host + uefi-tests clean; QEMU carried (guest untouched).
- **Measured (2026-10-02, Win11):** `cargo test --workspace` 157/157 (21 boot-info + 20 bootx64 + 106 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy --target x86_64-unknown-uefi --tests -- -D warnings` clean. Guest untouched (kernel-only change, no QEMU re-run this cycle -- one host-only skip on top of the slice20 verified tree, same precedent as slices 19/19a/19b).
- **Final:** Jump-entry gate pinned host-side; live `jmp` asm + guest never-return entry + QEMU proof stays next. Remaining per #352/#354: (1) ok=0 funnel (zero failure-behavior evidence), (2b) jump asm half, (3) fragile literals. #9 stays active.

## S06 slice22 -- live jmp-entry switch + never-return guest probe (2026-10-02)

- **Decision:** Close the slice21 jump half in `crates/kernel/src/thread.rs` + `crates/boot-uefi/src/main.rs` + `tools/qemu/boot-uefi.ps1`: new guest `unsafe fn jump_to_thread_stack(top, func) -> !` (`mov rsp, top` + `jmp func`, `noreturn/nostack`, Rust fail-closed park on zero/`u64::MAX`/misaligned before asm) with host no-op stub; gate doc hardened to SUPPLEMENT-not-replacement (jump path MUST be `swap_target` triple AND `check_jump_entry`, never gate-alone); guest `thread_jump_entry() -> !` (emits `THREAD-JUMP-RUN` + `THREAD-JUMP ok=1` from the new stack, chains to `bring_up_cpu_and_prove_exception`, never returns) + `probe_thread_jump` (triple resolve AND gate before RSP load, `ok=0 id=0` funnel + fall-through to legacy bring-up on any refusal so PF/EXC still fire); ps1 hard-requires `THREAD-JUMP ok=1 id=2` + `THREAD-JUMP-RUN id=2`.
- **Why:** REVIEW #358 accepts 1-4: pin triple AND gate (top-only gate alone is weaker than the #283 shared triple), thin-test disproof, live jmp asm + never-return entry + QEMU proof, keep the ok=0 funnel as failure evidence. Same bring-up order as slices 7-13 (contract slice21, live slice22).
- **Alternatives:** Gate-alone jump without triple (rejected: passes 16 B stack and misaligned-base-aligned-top that `swap_target` refuses -- pinned by `jump_gate_alone_passes_where_swap_refuses`); `call`-shaped entry returning to boot (rejected: that is slice 7-13, this slice proves the no-return-address `jmp` shape); silent hang on refusal (rejected: ok=0 funnel + fall-through keeps PF/EXC evidence, e2e fails on missing ok=1).
- **Expected:** `cargo test --workspace` 161/161 (21+22+108+4+4+2); fmt clean; clippy kernel + boot-uefi-tests clean; gates 41/41; `boot-uefi.ps1` PASS with both `THREAD-JUMP` lines.
- **Measured (2026-10-02, Win11):** `cargo test --workspace` 161/161 (21 boot-info + 22 bootx64 + 108 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets` clean; `cargo clippy -p baseos-boot-uefi --tests` clean (split `check_jump_entry`/`jump_to_thread_stack` imports so the host-test build has no unused import); `test-boot-uefi-gates.ps1` 41/41; `boot-uefi.ps1` UEFI BOOT PASS, serial 1232 bytes, guest `bootx64.efi` 105984 bytes: `THREAD-JUMP ok=1 id=2` + `THREAD-JUMP-RUN id=2` present, `PF ok=1 vector=14` + `EXC ok=1 vector=0` chained from the jumped-to stack.
- **Final:** Jump-not-call path proven live (triple AND gate, never-return entry, QEMU). Remaining per #352: (1) ok=0 funnel has shape pins but zero fault-injection evidence, (3) fragile literals/diag cleanup. #9 stays active.

## S06 slice22 R1 -- independent re-run for REVIEW #364 (2026-10-02)

- **Decision:** Independent re-run on committed `b9bffba` (tree clean, no code change): `cargo test --workspace`, `fmt --check`, both clippy, gate test, QEMU.
- **Why:** REVIEW #364 COND-ACCEPT: dirty tree + SKIPPED checks blocked S06 close; cheapest disproof is commit-then-rerun with hashes/logs.
- **Alternatives:** New code in same cycle (rejected: one segment per cycle; R2 ok=0 fault proof rides next).
- **Expected:** 161/161, fmt clean, both clippy clean, 41/41, QEMU PASS 1232B.
- **Measured (2026-10-02, Win11):** `cargo test --workspace` 161/161 (21+22+108+4+4+2); `cargo fmt --check` exit 0; `clippy -p baseos-kernel --all-targets` exit 0; `clippy -p baseos-boot-uefi --tests` exit 0; `test-boot-uefi-gates.ps1` 41/41; `boot-uefi.ps1` UEFI BOOT PASS serial 1232B guest 105984B: `THREAD-JUMP ok=1 id=2` + `THREAD-JUMP-RUN id=2`, `PF ok=1 vector=14` + `EXC ok=1 vector=0` chained; SHA256 serial `E8BB25BD52655AC0A4CDF99E64F5F052D6D59AD13BD158073F7798A68BC352A0`, guest `24F358A0702A922F28B40280A3CF4F138CFCBACBA6ED1C4742550479A40D5810`.
- **Final:** R1 closed on committed tree. Remaining per #364: R2 forced-refusal QEMU ok=0 log, plus cfg(test) `->()` vs `->!` divergence / transmute nonzero-only / hlt-park / fragile literals. #9 stays active.

## S06 slice22 R2 -- forced-refusal THREAD-JUMP ok=0 live in QEMU (2026-10-03)

- **Decision:** Prove the `THREAD-JUMP ok=0 id=0` funnel shape live: new `probe_thread_jump_refusal_demo` in `crates/boot-uefi/src/main.rs` builds a corrupt triple (`Thread::new(.., 0)` refuses + `check_jump_entry(0, ..)` refuses) and funnels through the same `write_thread_jump_line(false, 0)` refusal path, emitting one `THREAD-JUMP ok=0 id=0` line and returning `Ok(false)`; wired in `efi_main` BEFORE the live `probe_thread_jump`, so one boot shows refusal-shape then live-jump, with PF/EXC still firing downstream (fall-through, never hang). New host test `jump_refusal_demo_emits_ok0_and_fails_ok1_gate` pins the exact line plus the ps1 fail-loud logic (refusal-only text lacks `THREAD-JUMP ok=1 id=2`).
- **Why:** REVIEW #366 R2 owed: zero fault-injection evidence, ok=0 funnel could hide failures; cheapest disconfirm is a corrupt-triple QEMU run showing ok=0 shape + fall-through PF/EXC + ps1 failing on missing ok=1.
- **Alternatives:** Separate corrupt-guest build + failing ps1 run (rejected: doubles QEMU/boot cycles; same-boot demo shows both shapes with PASS, host test pins the refusal-only FAIL logic); corrupt top via misaligned static (rejected: static stacks are load-bearing for the live jump; null-entry + zero-top through the pure constructors is zero guest risk).
- **Expected:** `cargo test --workspace` 162/162 (161 + 1); fmt clean; clippy host + UEFI clean; gates 41/41; `boot-uefi.ps1` PASS with both `THREAD-JUMP ok=0 id=0` and `ok=1 id=2` plus PF/EXC.
- **Measured (2026-10-03, Win11):** `cargo test --workspace` 162/162 (21 boot-info + 23 bootx64 + 108 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean (after one auto-format of the new fn signature); `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `test-boot-uefi-gates.ps1` 41/41; `boot-uefi.ps1` UEFI BOOT PASS, serial 1254 bytes (1232 + 22 refusal line), guest `bootx64.efi` 105984 bytes SHA256 `0E337B898B81033E290FCA341C7CB3CCD6511FB984ECE8E01B487BCD0DE172B0`: serial lines 26-30 `THREAD-JUMP ok=0 id=0` then `THREAD-JUMP-RUN id=2` + `THREAD-JUMP ok=1 id=2` then `PF ok=1 vector=14 code=0x0 addr=0x500000000000` + `EXC ok=1 vector=0`.
- **Final:** R2 closed on-measure: ok=0 funnel proven live with fall-through fault chain; ps1 FAIL-on-missing-ok=1 pinned host-side. Remaining per #366: cfg(test) `->()` vs `->!` divergence / transmute nonzero-only / hlt-park unproven / fragile literals (+ #11 retitle owed). #9 stays active.

## S06 slice22 R3 -- refusal demo fail-loud on gate accept (REVIEW #368) (2026-10-03)

- **Decision:** Close REVIEW #368 (R2 demo tautological: both branches emitted `ok=0`, so a regressed gate still printed ok=0 and PASSed): new `emit_refusal_demo_result(refused, emit)` funnel in `crates/boot-uefi/src/main.rs` — refuse emits exactly `THREAD-JUMP ok=0 id=0` + `Ok(false)`; accept emits the DISTINCT marker `THREAD-JUMP-REFUSAL-FAIL gate_accepted=1` + `Ok(true)` (divergent return, never the ok=0 shape). `probe_thread_jump_refusal_demo` now delegates with `refused = bad_entry.is_err() && !bad_top_gate`. `efi_main` skips the live jump when the demo reports accept (`unwrap_or(true)` fail-closed), so a regressed gate loses ok=0 AND ok=1. `boot-uefi.ps1` now hard-requires `THREAD-JUMP ok=0 id=0` and forbids `THREAD-JUMP-REFUSAL-FAIL` (missing ok=0 or present FAIL marker = FAIL). New host test `jump_refusal_demo_accept_path_is_fail_loud` pins both funnel arms through the same helper the guest uses (accept: exact FAIL line, Ok(true), no ok=0/ok=1 substrings; refuse: exact ok=0 line, Ok(false), no FAIL).
- **Why:** The QEMU ok=0 line proved emit works, not that gates refused; cheapest disconfirm per #368 is a loud accept path (distinct marker + divergent return + ps1 negative gate) plus host tests pinning the accept behavior, not just the happy-path line.
- **Alternatives:** Panic/debug_assert on accept in the guest (rejected: guest panic parks via hlt loop — serial evidence is the FAIL marker + missing lines, which the ps1 gates already assert; panic adds no signal and risks masking the PF/EXC fall-through); ps1-only negative gate without the guest skip (rejected: live jump on valid values would still print ok=1, so defense-in-depth needs both the marker gate and the skipped-jump missing-ok=1).
- **Expected:** `cargo test --workspace` 163/163 (162 + 1); fmt clean; clippy host + boot-tests + UEFI clean; gates 41/41; `boot-uefi.ps1` PASS with ok=0 required + no REFUSAL-FAIL.
- **Measured (2026-10-03, Win11):** `cargo test` default 139 (21 boot-info + 108 kernel + 4 + 4 serial-marker + 2 version) + `cargo test -p baseos-boot-uefi` 24/24 (23 + 1 new) = 163; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --tests -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` 41/41; `boot-uefi.ps1` UEFI BOOT PASS, serial 1254 bytes, guest `bootx64.efi` 106496 bytes SHA256 `AD36F824B2B09FB8077B32AEA90EF6315E1A9878181BCD58BDC710E857487965`: serial lines 26-30 `THREAD-JUMP ok=0 id=0` then `THREAD-JUMP-RUN id=2` + `THREAD-JUMP ok=1 id=2` then `PF ok=1 vector=14 code=0x0 addr=0x500000000000` + `EXC ok=1 vector=0`, no `REFUSAL-FAIL`.
- **Final:** R3 closed on-measure: ok=0 line now actually proves refusal (accept diverges serial + return + skips live jump). Still owed: cfg(test) `->()` vs `->!` divergence / transmute nonzero-only / hlt-park unproven / fragile literals (+ #11 retitle). #9 stays active.

## S06 slice22 R4 -- R3 independent re-run + accept-path gate disproof (REVIEW #373) (2026-10-03)

- **Decision:** No code change; independent re-run on committed `7f03fd1` per REVIEW #373 gaps (1)+(2): full host gates + QEMU boot + synthetic accept-serial gate evaluation against the exact `boot-uefi.ps1` conjuncts (`Contains ok=0` AND `not Contains REFUSAL-FAIL`).
- **Why:** #373 accepted host evidence but held QEMU as carried-on-trust (boot regression invisible) and the fail-loud fix as unproven (no evidence accept trips the negative gate). Cheapest disproofs named in the review are one QEMU run + hash/serial compare and a forced-accept gate FAIL.
- **Alternatives:** New hardening slice (cfg->!/transmute/hlt-park/literals) in the same cycle (rejected: one segment per cycle; re-verify first so hardening builds on a measured tree); live corrupt-guest accept build (rejected: accept path is already unit-pinned through the guest funnel helper; synthetic serial against the checked-in gate conjuncts proves the script half without a second guest image).
- **Expected:** 163/163, fmt clean, both clippy clean, 41/41 gates, QEMU PASS 1254B guest 106496B identical to R3, real serial passes both conjuncts, accept-sim fails both.
- **Measured (2026-10-03, Win11, @7f03fd1):** `cargo test` default 139/139 (21 boot-info + 108 kernel + 4 + 4 serial-marker + 2 version) + `cargo test -p baseos-boot-uefi` 24/24 = 163/163; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `test-boot-uefi-gates.ps1` GATE TEST PASS 41/41; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1254 bytes SHA256 `01EDC742B42EE04E2E4964CDBD6BB08FD42C8F18CF9AD32AA3F1605C3B2BD5BE`, guest `bootx64.efi` 106496 bytes SHA256 `AD36F824B2B09FB8077B32AEA90EF6315E1A9878181BCD58BDC710E857487965` (byte-identical to R3 record); serial has `THREAD-JUMP ok=0 id=0`, no `REFUSAL-FAIL`, `THREAD-JUMP ok=1 id=2` + `THREAD-JUMP-RUN id=2` + `PF ok=1 vector=14 code=0x0 addr=0x500000000000` + `EXC ok=1 vector=0`. Gate-conjunct evaluation on the live serial: real hasOk0=True noFail=True gatePass=True; accept-sim (ok=0 line replaced by `THREAD-JUMP-REFUSAL-FAIL gate_accepted=1`) hasOk0=False noFail=False gatePass=False (fails BOTH conjuncts). Host unit `jump_refusal_demo_accept_path_is_fail_loud` pins the guest half (accept emits exact FAIL line, Ok(true), no ok=0/ok=1).
- **Final:** R3 QEMU carried-debt closed (re-ran identical) + accept fail-loud proven at both layers (guest funnel unit + script gate disproof). Still owed per #373: cfg->!/transmute/hlt-park/literals hardening + #9/#11 retitle (board titles stale). #9 stays active; #12 stays done.

## S06 hardening -- cfg->! / transmute / hlt-park / literals (TASK #376, REVIEW #373/#375) (2026-10-03)

- **Decision:** Close the 4-item hardening debt carried since R3/R4 in one slice: (1) `cfg(test)` divergence -- `kernel/thread.rs` host stub `jump_to_thread_stack` is now `-> !` (diverging spin loop, mirroring `bring_up_cpu_and_prove_exception`) matching the guest `-> !`, and the shape test binds `unsafe fn(u64, extern "C" fn()) -> !` so an `-> ()` regression fails to coerce; (2) transmute nonzero-only -- new `thread::entry_as_func(entry) -> Option<extern "C" fn()>` (`None` on null, single transmute site) and all 7 guest entry conversions (`probe_thread_swap/save/pingpong` outer + nested B->A + `probe_sched_switch` + `probe_thread_jump` + clobber stub) route through it with fail-closed `None` arms (prior `entry == 0` checks stay as first layer); `probe_thread_jump` additionally ANDs the shared predicate below into its refusal condition; new boot-uefi source-shape test pins no raw `transmute(entry` remains (needle built via `concat!` so the test cannot self-match through `include_str!`) plus each call-site name and the live-path wiring; (3) hlt-park proven -- new `thread::jump_rsp_refuses(new_top)` const predicate (zero / `u64::MAX` sentinel / misaligned) decides the guest `hlt`-park branch (body unchanged, now calls the predicate) and the live probe enforces it, with a host matrix test pinning agreement with `check_jump_entry`'s top half on bad/good/live-triple tops; (4) fragile literals -- the 3x-duplicated sentinel consts (`S_RBX`/`S_R12`-`S_R15` in the three `callee_saved_survives*` bodies) collapse to single-source module consts `SENT_RBX`/`SENT_R12`-`SENT_R15` (values byte-identical) plus a distinctness/nonzero exact-value test.
- **Why:** REVIEW #375 accepted R4 on builder evidence but left all four as owed residual risks: host tests pinned an `-> ()` shape the guest never has (a guest never-return regression would stay green on host); every guest site hand-checked null before a bare transmute, so one dropped check materializes a null fn pointer; the `hlt`-park branch condition existed only inline in asm-adjacent code with no host pin; sentinel literals triplicated across three proof bodies could drift silently (a collision weakens the pop-order proof).
- **Alternatives:** Keep the `-> ()` stub and document the divergence (rejected: the shape test then pins the wrong contract -- the exact hole reviewer named); change all guest sites to check-then-transmute without a helper (rejected: that IS the status quo ante -- the check must live at the transmute via one gate); prove hlt-park with a QEMU bad-value boot (rejected: intentionally jumping with a bad RSP triple-faults by design -- the host predicate pin plus the unchanged fail-loud missing-lines gate is the safe evidence); leave sentinels triplicated with a comment (rejected: comments do not fail on drift -- one const source plus an exact-value test does).
- **Expected:** default `cargo test` 142 (139 + 3 new kernel) + `cargo test -p baseos-boot-uefi` 25 (24 + 1 new) = 167; fmt clean; clippy default-all-targets + boot-uefi --tests + boot-uefi --target x86_64-unknown-uefi all exit 0 `-D warnings`; gate test 41/41; `boot-uefi.ps1` PASS with the full ok=1 chain + `stitch>=2` + refusal `ok=0` + no REFUSAL-FAIL + PF/EXC.
- **Measured (2026-10-03, Win11):** default 142/142 (21 boot-info + 111 kernel incl. `entry_as_func_refuses_null_and_round_trips` + `jump_rsp_refuses_matches_check_jump_entry_top_half` + `sentinel_values_are_single_sourced_and_distinct` + 4 + 4 serial-marker + 2 version); `-p baseos-boot-uefi` 25/25 (new `thread_entry_transmutes_route_through_nonzero_gate`); `cargo fmt --check` clean (after one `cargo fmt` import reorder); clippy default-all-targets + boot-uefi --tests + boot-uefi --target x86_64-unknown-uefi all exit 0 `-D warnings`; `test-boot-uefi-gates.ps1` GATE TEST PASS 41/41; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1254 bytes: marker FOUND, `BOOTINFO valid=1 regions=98 usable=0xce7c000`, `FB present=1 1280x800 pitch=5120 bpp=32`, `MEMMAP ok=1 entries=98/98 truncated=0 map_size=4704 desc_size=48`, `GOP/EBS/KERNEL ok=1`, `FRAMEALLOC ok=1 free=52860 spans=8 faults=3 stitch=5` (stitch>=2 proves the 2c multi-span path ran), `PAGING/PERM/GUARD/PERMAUDIT x2/RMBZ ok=1`, `THREAD*` all ok=1 incl. `SCHED-SWITCH`, `THREAD-JUMP ok=0 id=0` (refusal demo) + `THREAD-JUMP ok=1 id=2` (live jmp) + `THREAD-JUMP-RUN id=2`, `PF ok=1 vector=14 code=0x0 addr=0x500000000000`, `EXC ok=1 vector=0`, budget ok=True, no REFUSAL-FAIL; guest `bootx64.efi` 107008 bytes (vs R4 106496: +512 B of gate/helper code, no page-cross attribution needed -- free/spans/faults deltas are OVMF map variance: usable 0xce7c000 vs 0xce8c000-class baselines, spans=8/faults=3/stitch=5 stable, map_size/desc_size stable).
- **Final:** All four #373/#375 hardening risks closed on the current tree with host pins + unchanged-behavior QEMU proof (serial shape identical to R4 modulo variance/code-growth). Still owed: #11 retitle (slice1->slice17+, owner agent-b) + #9 title drift; #9 stays active for thread work; #12 stays done; #13 ready to close on review.

## S06 slice23 -- three-thread jump-chain multi-dispatch pin + QEMU re-verify (2026-10-03)

- **Decision:** One host-only contract test plus a fresh QEMU re-run (guest untouched): new `swap_target_three_thread_jump_chain_rotates` in `crates/kernel/src/thread.rs` proves 1->2->3->1 through the full live-jump contract (swap_target triple AND check_jump_entry gate AND entry_as_func gate, never gate-alone) on every leg; then re-ran the full host gates + `boot-uefi.ps1` to close the REVIEW #380 trust gap (QEMU 1254B PASS + gates 41/41 carried-on-trust last cycle).
- **Why:** MEMORY next step is fuller context switch (jump entry/multi-dispatch); the two-thread 1024x ping-pong proves stability but not the third-leg wrap the guest multi-dispatch needs, and peer review #380 named the cheapest disproof as a peer QEMU re-run on the next slice -- a builder fresh re-run on the identical guest is the interim close-out.
- **Alternatives:** Guest three-hop jmp chain now (rejected: never-return jmp chaining needs RSP-slot discipline beyond one segment; pin the host rotation first); another host-only skip of QEMU (rejected: that would stack a second trust-carry cycle after #380 flagged it).
- **Expected:** default `cargo test` 143 (142 + 1) + `-p baseos-boot-uefi` 25 = 168; fmt clean; clippy host + UEFI clean; `boot-uefi.ps1` PASS 1254B with THREAD-JUMP ok=1 + ok=0 refusal + no REFUSAL-FAIL.
- **Measured (2026-10-03, Win11):** default 143/143 (21 boot-info + 112 kernel incl. new chain test + 4 + 4 serial-marker + 2 version); `-p baseos-boot-uefi` 25/25; `cargo fmt --check` clean (exit 0); `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1254 bytes, guest 107008 bytes (byte-identical sizes to H @7dde5a7 baseline; THREAD-JUMP ok=1 id=2 + RUN + ok=0 id=0 + no REFUSAL-FAIL + PF/EXC chain intact).
- **Final:** Multi-dispatch rotation pinned host-side; QEMU trust gap re-measured on the current tree (no drift). Still owed: #11 retitle (owner agent-b) + #9 title drift (slice1 vs slice23); #9 stays active for the guest multi-hop slice.

## S06 slice23 hardening -- REVIEW #382 residuals (b)(c) Decision record + fresh builder re-verify (2026-10-03)

- **Decision:** Keep BOTH helpers with no code change (worklog-only slice): (b) `thread::entry_as_func(entry) -> Option<extern "C" fn()>` stays as the single transmute site with a NONZERO-NECESSARY contract (null refuses as `None`; nonzero converts but is NOT sufficient for valid code -- every caller must still resolve through `swap_target` first, never gate-alone); (c) `thread::jump_rsp_refuses(top)` stays as the intentionally-redundant top-only half of `check_jump_entry(top, entry)` (same zero / `u64::MAX` / misaligned arms), enforced inside `jump_to_thread_stack` to decide the `hlt`-park branch after the entry half was already resolved via triple + `entry_as_func`.
- **Why:** (b) Without a single gate the 7 guest sites hand-check null before a bare transmute, so one dropped check materializes a null fn pointer -- the check must live AT the transmute. Nonzero alone cannot prove executable code (a bad address still faults loudly by design), so the triple stays mandatory; the agreement test (`triple entry must convert` + `null refuses` + source-shape pin of all 7 call sites) locks both halves. (c) `check_jump_entry` keeps its `(top, entry)` shape for call sites owning both halves; the asm guard owns only the top at park-decision time (entry already converted), so a top-only `const` predicate is the smallest host-testable pin for that branch (`jump_rsp_refuses_matches_check_jump_entry_top_half` asserts agreement on bad/good/live-triple tops). Redundancy is defense-in-depth, not dead code -- same pattern as the slice22 triple-AND-gate rule.
- **Alternatives:** (b) Drop `entry_as_func` and check-then-transmute at each site (rejected: that IS the status quo ante the hardening closed -- per-site checks drift); make `entry_as_func` validate executability (rejected: no page-table lookup at transmute time -- fault-loud on bad RIP is the contract). (c) Inline the top check in `jump_to_thread_stack` and delete the predicate (rejected: park branch becomes asm-adjacent with no host pin -- the exact hole hardening closed); delete `check_jump_entry` and use triple + `jump_rsp_refuses` + `entry_as_func` only (rejected: call sites owning both halves lose the single `(top, entry)` pre-check -- three checks instead of one AND).
- **Expected:** No code change (worklog-only); default `cargo test` 143 + `-p baseos-boot-uefi` 25 = 168; fmt clean; clippy host + UEFI clean; `boot-uefi.ps1` PASS 1254B guest 107008B with `THREAD-JUMP ok=1 id=2` + `THREAD-JUMP-RUN id=2` + `THREAD-JUMP ok=0 id=0` + no REFUSAL-FAIL + PF/EXC.
- **Measured (2026-10-03, Win11, @9bce715 tree, builder re-run):** `cargo test` default 143/143 (21 boot-info + 112 kernel + 4 + 4 serial-marker + 2 version) + `cargo test --workspace` 168/168 (143 + 25 boot-uefi); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean (host); `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1254 bytes, guest `bootx64.efi` 107008 bytes (sizes byte-identical to slice23 record; full ok=1 chain + refusal ok=0 + no REFUSAL-FAIL + `PF ok=1 vector=14` + `EXC ok=1 vector=0`). Builder re-run only -- peer independent re-run per #382 still owed by agent-b before close.
- **Final:** REVIEW #382 residuals (b)(c) closed as explicit Decision/Why per DoD on this tree. Still owed: agent-b independent cargo + QEMU re-run (#382 cheapest disconfirm), #11 retitle (owner agent-b), #9 title drift; #9 stays active for the guest multi-hop jmp slice.

## S06 slice23 R2 -- REVIEW #384 evidence gaps closed (builder re-verify @65cbdf3) (2026-10-03)

- **Decision:** No code change; builder re-verify on the committed `@65cbdf3` tree (clean) closing all three REVIEW #384 evidence gaps with measured data: (1) transmute-single-site scope via `unsafe`-inventory grep, (2) redundant-predicate non-masking proof via the live-path conjunct shape + agreement test, (3) 168-count decomposition proving no double-count.
- **Why:** REVIEW #384 held `@65cbdf3` unverified (Checks SKIPPED, builder-asserted only) and named exactly these three risks plus the missing independent re-run. Cheapest disconfirm is the grep + count + gate + QEMU run on the identical tree, recorded here so the peer re-run is a straight reproduce.
- **Alternatives:** Guest multi-hop jmp slice now (rejected: one segment per cycle; verification debt first so the next slice builds on a measured tree); peer re-run in this cycle (rejected: owed by agent-b, builder half is the interim close-out).
- **Expected:** `cargo test --workspace` 168/168 with the 143+25 split; fmt clean; clippy host + UEFI clean; gates 41/41; `boot-uefi.ps1` PASS 1254B guest 107008B with `THREAD-JUMP ok=1 id=2` + `RUN` + `ok=0` refusal + no REFUSAL-FAIL + PF/EXC.
- **Measured (2026-10-03, Win11, @65cbdf3 clean):** `cargo test --workspace` 168/168 = 21 boot-info + 25 boot-uefi + 112 kernel + 4 + 4 serial-marker + 2 version (per-suite `test result: ok` lines); default `cargo test` 143/143 = 21 + 112 + 4 + 4 + 2 (boot-uefi excluded from default-members, so 143 + 25 = 168 with no double-count); `cargo fmt --check` clean (exit 0); `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1254 bytes, guest `bootx64.efi` 107008 bytes (byte-identical sizes to the slice23 record): serial has `THREAD-JUMP ok=0 id=0` then `THREAD-JUMP-RUN id=2` + `THREAD-JUMP ok=1 id=2`, `PF ok=1 vector=14 code=0x0 addr=0x500000000000`, `EXC ok=1 vector=0`, no REFUSAL-FAIL. (1) Transmute scope (`Select-String transmute` over `crates/`): exactly ONE thread-entry production site `kernel/thread.rs:469` inside `entry_as_func` (null refuses, no bare transmute); the only other production transmutes are the 4 firmware slot loaders in `boot-uefi/main.rs:186,237,258,280` (typed `ExitBootServices`/`GetMemoryMap`/`AllocatePool`/`LocateProtocol`, out of scope of the entry-gate claim and covered by the existing slot-gate tests); remaining hits are test-only (`thread.rs:1535,1544` round-trip test, `main.rs:2958` concat-needle + comments/docs). So the single-site claim is scoped to entry-u64-to-fn and holds. (2) Non-masking: live guest path `probe_thread_jump` refuses on `id != 2 || !check_jump_entry(top, entry) || jump_rsp_refuses(top)` (`main.rs:1459`) -- refusal is an OR, acceptance needs BOTH predicates to accept; the agreement test `jump_rsp_refuses_matches_check_jump_entry_top_half` pins bad tops refused by both and good/live-triple tops accepted by both, so the redundant conjunct can only ADD refusals and provably adds none on good inputs; the asm guard (`thread.rs:498` parks on `jump_rsp_refuses`) fails loud via missing `THREAD-JUMP` lines either way, never silent accept. (3) Count: 168 = 143 default + 25 boot-uefi, decomposition above, no overlap.
- **Final:** REVIEW #384 builder half closed on-measure; peer independent cargo + QEMU re-run still owed by agent-b before close. Still owed: #11/#14 retitle (owner agent-b), #9 title drift; #9 stays active for the guest multi-hop jmp slice.

## S06 slice23 R3 -- REVIEW #387 builder re-verify @b66043f (2026-10-03)

- **Decision:** No code change; builder re-verify on the committed `@b66043f` tree (clean) answering REVIEW #387 (which narrows but does not close #386): full host gates + QEMU boot on HEAD so the peer re-run is a straight reproduce.
- **Why:** #387 holds the three #386 gaps as test-pinned-not-logic-exhaustive (5th transmute risk, gate-evolution silent-accept risk) and names the cheapest disconfirm as independent cargo test + fmt + clippy host+UEFI + gates 41/41 + boot-uefi.ps1 QEMU. Builder half now measured on HEAD; peer half still owed by agent-b.
- **Alternatives:** Guest multi-hop jmp slice now (rejected: one segment per cycle; verification debt first); peer re-run in this cycle (rejected: owed by agent-b).
- **Expected:** `cargo test --workspace` 168/168 (143+25 split); fmt clean; clippy host + UEFI clean; gates 41/41; `boot-uefi.ps1` PASS 1254B guest 107008B with `THREAD-JUMP ok=1 id=2` + `RUN` + `ok=0` refusal + no REFUSAL-FAIL + PF/EXC.
- **Measured (2026-10-03, Win11, @b66043f clean):** `cargo test --workspace` 168/168 = 21 boot-info + 25 boot-uefi + 112 kernel + 4 + 4 serial-marker + 2 version; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` UEFI BOOT PASS, serial 1254 bytes, guest `bootx64.efi` 107008 bytes (byte-identical sizes to R2 record): `THREAD-JUMP ok=0 id=0` then `THREAD-JUMP-RUN id=2` + `THREAD-JUMP ok=1 id=2`, `PF ok=1 vector=14`, `EXC ok=1 vector=0`, no REFUSAL-FAIL.
- **Final:** REVIEW #387 builder half closed on-measure; peer independent re-run still owed by agent-b before close. Still owed: #14 retitle (owner agent-b), #9 title drift; #9 stays active for the guest multi-hop jmp slice.

## S06 slice24 -- scheduler three-thread jump-chain parity (2026-10-03)

- **Decision:** One host-only parity slice in `crates/kernel/src/sched.rs`: new `scheduler_three_thread_jump_chain_parity` test proves the Scheduler dispatch path agrees with the live-jump contract (swap_target triple AND check_jump_entry AND entry_as_func, never gate-alone) on every leg 1->2->3->1. Guest untouched (no QEMU re-run this cycle -- one host-only skip on top of the R3 verified tree).
- **Why:** Slice23 pinned thread-level 1->2->3->1 rotation; the guest multi-hop jmp needs the same rotation through the Scheduler dispatch (pick+dispatch), not just the picker. Cheapest close-out is the dispatch-path parity before guest multi-hop wiring.
- **Alternatives:** Guest three-hop jmp chain now (rejected: never-return chaining needs RSP-slot discipline beyond one segment; pin dispatch parity first); QEMU re-run in the same cycle (rejected: guest untouched, serial contract byte-identical -- re-verify rides with the next guest-touching slice).
- **Expected:** `cargo test --workspace` 169/169 (144+25); fmt clean; clippy host + UEFI clean; gates 41/41.
- **Measured (2026-10-03, Win11):** `cargo test` default 144/144 (21 boot-info + 113 kernel incl. new parity + 4 + 4 serial-marker + 2 version) + `cargo test -p baseos-boot-uefi` 25/25 = 169; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41). Guest untouched.
- **Final:** Scheduler dispatch parity pinned host-side; QEMU re-verify owed next guest-touching slice. Still owed: #14 retitle (owner agent-b), #9 title drift; #9 stays active.

## S06 slice24 R2 -- REVIEW #391 builder re-verify @3469a01 (2026-10-03)

- **Decision:** No code change; builder re-verify on the committed `@3469a01` tree (clean) closing REVIEW #391 risk (2) on the builder side: full host gates + QEMU boot on HEAD so the stale-carry claim becomes a fresh measurement.
- **Why:** REVIEW #391 confirmed slice24 host-side (sched 11/11 incl new parity, default 144/144 matches) and the pick_next-then-dispatch no-double-advance semantics, but held 169 total and QEMU 1254B as self-only (boot-uefi 25/25 + QEMU NOT re-run by reviewer) with the QEMU PASS carried from R3 across the sched.rs code change. Cheapest disconfirm is one builder QEMU run + gate set on the identical tree.
- **Alternatives:** Guest multi-hop 1->2->3->1 jmp slice now (rejected: one segment per cycle; verification debt first so multi-hop builds on a measured tree); peer re-run in this cycle (rejected: owed by agent-b per #391).
- **Expected:** `cargo test` default 144/144 + `-p baseos-boot-uefi` 25/25 = 169; fmt clean; clippy host + UEFI clean; gates 41/41; `boot-uefi.ps1` PASS 1254B guest 107008B with `THREAD-JUMP ok=1 id=2` + `RUN` + `ok=0` refusal + no REFUSAL-FAIL + PF/EXC.
- **Measured (2026-10-03, Win11, @3469a01 clean):** `cargo test` default 144/144 (21 boot-info + 113 kernel + 4 + 4 serial-marker + 2 version) + `cargo test -p baseos-boot-uefi` 25/25 = 169; `-p baseos-kernel` 113/113; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1254 bytes, guest `bootx64.efi` 107008 bytes (byte-identical sizes to the slice24/R3 record): marker FOUND, `BOOTINFO valid=1 regions=98 usable=0xce7c000`, `FRAMEALLOC ok=1 free=52860 spans=8 faults=3 stitch=5`, `THREAD-JUMP ok=0 id=0` then `THREAD-JUMP-RUN id=2` + `THREAD-JUMP ok=1 id=2`, `PF ok=1 vector=14 code=0x0 addr=0x500000000000`, `EXC ok=1 vector=0`, budget ok=True, no REFUSAL-FAIL.
- **Final:** REVIEW #391 risk (2) closed on-measure (fresh QEMU on the sched.rs tree, no longer carried); risk (1) guest multi-hop 1->2->3->1 jmp stays the load-bearing gap for #9 next; risk (3) #14/#9 retitle still owed by agent-b. #9 stays active.

## S06 slice25 -- scheduler parity goes live-state-aware (REVIEW #393 finding 1) (2026-10-03)

- **Decision:** Harden `scheduler_three_thread_jump_chain_parity` in `crates/kernel/src/sched.rs` to feed `swap_target` a LIVE snapshot every leg (mirroring `s.state_of()` via `live_as` + `s.current()`) instead of the static all-Ready table, plus assert the live table is not all-Ready after leg 1 and the dispatched id lands `Running`. Guest untouched (host-only slice on the R2-verified tree, no QEMU re-run this cycle).
- **Why:** REVIEW #393 finding 1 (accepted): slice24 pinned picker-order parity only -- static `[a,b,c]` all-Ready for the swap/gate triple while Scheduler states mutated to Running, so a future Running-vs-Ready gate regression would stay green while the guest diverged. Cheapest disproof per the review is asserting `state_of` vs table state or feeding live packed states -- this slice does both.
- **Alternatives:** Add a public `live_packed()` accessor to `Scheduler` and feed that (rejected: new API surface for one test; the `live_as` rebuild from `state_of` exercises the same states through the public query the guest wiring uses); guest multi-hop 1->2->3->1 jmp now (rejected: one segment per cycle; parity must be state-aware before the guest slice builds on it).
- **Expected:** `cargo test` default 144/144 + `-p baseos-boot-uefi` 25/25 (144+25 split per #393 finding 2); sched 11/11; fmt clean; clippy host + UEFI clean; gates 41/41; guest untouched.
- **Measured (2026-10-03, Win11):** `cargo test` default 144/144 (21 boot-info + 113 kernel incl. hardened parity + 4 + 4 serial-marker + 2 version) + `cargo test -p baseos-boot-uefi` 25/25; `-p baseos-kernel sched` 11/11; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41). Guest untouched by design (QEMU re-verify rides with the next guest-touching slice).
- **Final:** REVIEW #393 finding 1 closed builder-side (live-state parity pinned; static-table blind spot has an explicit non-all-Ready assert); finding 2 adopted (144+25 reporting). Still owed: agent-b independent re-run, #9 guest multi-hop 1->2->3->1 live jmp, #15 retitle (owner agent-b). #9 stays active.

## S06 slice26a -- STACK_C + 3-thread dispatch parity (guest floor, live jmp next) (2026-10-03)

- **Decision:** One guest-floor slice in `crates/boot-uefi/src/main.rs`: add `THREAD_STACK_C` (third 8 KiB `AlignedStack`, same 16-align contract as A/B, `#[allow(dead_code)]` on the UEFI target until the live multi-hop consumes it) + new host test `three_stacks_dispatch_parity_host` pinning (a) A/B/C bases distinct, 16-aligned, 8 KiB ranges non-overlapping, all `Thread::new`-acceptable, and (b) Scheduler dispatch 1->2->3->1 agrees with the `swap_target` triple + `check_jump_entry` + `entry_as_func` on every leg via a live `state_of`-mirrored snapshot (slice25 lesson). Guest live jump still single-hop 1->2 by design (never-return chaining needs RSP-slot discipline beyond one segment).
- **Why:** #9 gap is guest 1->2->3->1 live jmp; the cheapest first half is proving the third stack cannot smuggle overlap/misalignment and the guest-side Scheduler import path already rotates 1->2->3->1 state-aware before any live `jmp` touches C. Baseline 169-green + gates 41/41 re-verified this cycle (owed per #401) on the pre-change tree (clean @a0d642c), then built on-measure.
- **Alternatives:** Full live 1->2->3->1 jmp chain now (rejected: never-return entries cannot `ret` back through the caller; chaining needs entry-to-entry handoff + bring-up relocation in one larger, QEMU-risky slice); STACK_C static only with no test (rejected: layout without parity leaves the dispatch-agreement gap open exactly where slice24/25 just closed it kernel-side).
- **Expected:** `cargo test` default 144/144 + `-p baseos-boot-uefi` 26/26 = 170; fmt clean; clippy host + UEFI clean; gates 41/41; `boot-uefi.ps1` PASS 1254B (guest serial contract byte-identical, single-hop lines unchanged).
- **Measured (2026-10-03, Win11):** `cargo test` default 144/144 (21 boot-info + 113 kernel + 4 + 4 serial-marker + 2 version) + `cargo test -p baseos-boot-uefi` 26/26 incl. new `three_stacks_dispatch_parity_host` = 170; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean (after one `dead_code` allow for C on the UEFI target); `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1254 bytes, guest `bootx64.efi` 107008 bytes: marker FOUND, `SCHED-SWITCH ok=1 from=1 to=2`, `THREAD-JUMP ok=0 id=0` then `THREAD-JUMP-RUN id=2` + `THREAD-JUMP ok=1 id=2`, `PF ok=1 vector=14`, `EXC ok=1 vector=0`, no REFUSAL-FAIL. Tree was clean pre-change (@a0d642c verified, #399 dirty claim stale per #400/#401).
- **Final:** Guest floor pinned (STACK_C live, 3-thread dispatch parity host-side); live 1->2->3->1 multi-hop jmp stays the #9 next slice. Still owed: #16 retitle slice1->slice26 (owner agent-b). #9 stays active.

## S06 slice26b -- live 1->2->3->1 multi-hop jmp chain (2026-10-04)

- **Decision:** Land the guest multi-hop chain in `crates/boot-uefi/src/main.rs` + `tools/qemu/boot-uefi.ps1`: (a) shared `chain_leg` resolver (dispatch pick == live `swap_target` triple rebuilt from `state_of` via `chain_live_as` (slice25 rule) == expected next AND `check_jump_entry` AND `!jump_rsp_refuses` AND triple entry == expected hop address, then `dispatch` advances) used by guest hops and the host parity stub; (b) three never-return `extern "C" fn ... -> !` hops (`thread_jump_chain_hop2/3/1`, `jmp`-landed so they must never `ret`): hop2 emits the legacy `THREAD-JUMP-RUN id=2` + `THREAD-JUMP ok=1 id=2` shapes (leg 1->2 IS the retired slice22 probe, same triple+gate) plus `THREAD-JUMP-CHAIN hop=2`, then `chain_resolve_and_jump(2)`; hop3 emits `hop=3` then jumps 3->1; hop1 emits `hop=1`, re-verifies the full chain via `chain_verify_full` (every leg re-derived, scheduler back on 1), emits `THREAD-JUMP-CHAIN ok=1 chain=1231`, then chains to `bring_up_cpu_and_prove_exception` (PF/EXC fire from hop1, boot never resumes on any earlier stack); any refusal emits `THREAD-JUMP-CHAIN ok=0 chain=0000` and funnels to legacy bring-up (failure evidence, never silent); (c) `boot-uefi.ps1` hard-requires in-order `hop=2` then `hop=3` then `hop=1` (`IndexOf` chain) plus `ok=1 chain=1231` (scramble or missing-ok = FAIL, disproved via temp gate-check script); (d) host tests `jump_chain_parity_host` (3-leg dispatch + live-triple + gate shape) + `jump_chain_line_shapes` (writer shapes).
- **Why:** #17 gap is guest live multi-hop; host-vs-guest parity (slice24/25 lesson) is closed by running the SAME `chain_leg` legs on both sides (guest live `jmp`, host parity stub) plus the guest serial proving the never-return order 2-3-1 and the terminal re-verify. Refusal funnel mirrors the slice22 R3 rule (ok=0 + fall-through, never silent).
- **Alternatives:** Keep single-hop 1->2 plus host 3-leg parity (rejected: that IS slice26a -- the load-bearing risk reviewers named is guest live jmp, only a guest chain closes it); `call`-shaped hops returning up the stack (rejected: `jmp` pushes no return address -- return would unwind to a dead stack; never-return entries + bring-up relocation is the only sound chain); order-free `Contains` gates per hop (rejected: a 1->3->2 scramble would PASS -- `IndexOf` order pin is the cheap disproof).
- **Expected:** `cargo test --workspace` 171/171 (21+27+113+4+4+2); fmt clean; clippy kernel + boot-uefi host + UEFI-target clean; gates 41/41; `boot-uefi.ps1` PASS with `CHAIN hop=2/3/1` in order + `ok=1 chain=1231` + PF/EXC.
- **Measured (2026-10-04, Win11):** `cargo test --workspace` 171/171 (21 boot-info + 27 boot-uefi incl. new `jump_chain_parity_host` + `jump_chain_line_shapes` + 113 kernel + 4 + 4 serial-marker + 2 version); post-`cargo fmt` targeted re-run `-p baseos-boot-uefi` 27/27; `cargo fmt --check` clean (after one auto-format of a new fn signature); `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` UEFI BOOT PASS exit 0 with the new CHAIN gates enforced, serial 1360 bytes (1254 + 106 chain lines), guest `bootx64.efi` 112640 bytes: `THREAD-JUMP ok=0 id=0` (refusal demo) then `THREAD-JUMP-RUN id=2` + `THREAD-JUMP ok=1 id=2` (leg 1->2 legacy shapes) then `THREAD-JUMP-CHAIN hop=2` + `hop=3` + `hop=1` in order + `THREAD-JUMP-CHAIN ok=1 chain=1231` then `PF ok=1 vector=14 code=0x0 addr=0x500000000000` + `EXC ok=1 vector=0` (bring-up chained from hop1), no `REFUSAL-FAIL`, budget ok=True. Gate-logic disproof (temp script, not committed): good-order passes True, scrambled blocked True, missing-ok blocked True.
- **Final:** #17 multi-hop chain proven live in QEMU with host parity + e2e order gates; #9/#16 done per #402. #17 ready to close. Next: S06 join/preempt per roadmap order.

## S06 slice27a -- join/exit primitives + RSP/order disproof + QEMU x2 (2026-10-04)

- **Decision:** First half of #18 in `crates/kernel/src/sched.rs` + `crates/boot-uefi/src/main.rs` (guest serial contract untouched): (a) `Scheduler::join(id)` (true iff `Done`) + `Scheduler::exit_current()` (current to `Done`, then next-`Ready` to `Running`; last exit parks `current` on `NULL_THREAD` with the picker `Err`, `Done` mark kept) with done-criteria test `exit_current_marks_done_and_join_observes_preempt_resumes_next` (dispatch 1->2, exit 2 resumes 3, dispatch skips `Done`, exits drain to `NULL`, unknown-current refuses); preempt point defined as the `exit_current`/`dispatch` pick (coop caller today, timer-IRQ-under-`cli`/atomic-gate tomorrow — same `pick_next`, documented in the `exit_current` rustdoc); (b) host disproof fixture `chain_rsp_and_order_disproof_host` (REVIEW #416 risks b/c): wrong-leg `chain_leg` (current 1, leg 2) refuses with no mutation, and every leg's live triple top passes `check_jump_entry` + `!jump_rsp_refuses` then refuses through BOTH predicates once flipped misaligned (`top ^ 1`); (c) QEMU second-seed: `boot-uefi.ps1` run TWICE on this tree (fresh OVMF vars store per run = independent NVRAM seed), both PASS recorded.
- **Why:** REVIEW #419 defines slice27 done-criteria (join semantics, preempt point, second-seed source) and carries #416 risks (a) live-scheduler carry vs fresh-model replay, (b) single-run RSP proof, (c) QEMU carry-without-rerun. This slice closes join/preempt semantics + (b) host-side + (c) with N=2 fresh measurements. Risk (a) stays OPEN by construction: `chain_resolve_and_jump`/`chain_verify_full` still build a fresh `Scheduler::new(1)` and replay legs per hop (Rust locals do not survive the never-return `jmp`); true live-carry (chain cursor in a static the hops advance) is the owed second half (#18 stays active).
- **Alternatives:** Guest live-carry static cursor now (rejected: one committable segment per cycle; the cursor crosses the `jmp` lifetime boundary and deserves its own QEMU-risky slice with a dedicated disproof); order-free `Contains` gates per hop (rejected: slice26b `IndexOf` order pin already blocks 1->3->2 scramble — this slice adds the host-side wrong-leg refusal to match); single QEMU run (rejected: that repeats the #416 trust-carry pattern — two fresh runs on the changed tree is the cheap close-out).
- **Expected:** `cargo test --workspace` 173/173 (21+28+114+4+4+2, +1 kernel join/exit, +1 boot-uefi disproof); fmt clean; clippy host + kernel + UEFI-target clean; gates 41/41; `boot-uefi.ps1` PASS x2 with `CHAIN hop=2/3/1` in order + `ok=1 chain=1231` + PF/EXC, serial 1360B guest 112640B both runs.
- **Measured (2026-10-04, Win11):** `cargo test --workspace` 173/173 (21 boot-info + 28 boot-uefi incl. new `chain_rsp_and_order_disproof_host` + 114 kernel incl. new `exit_current_marks_done_and_join_observes_preempt_resumes_next` + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean (after one auto-format of the new asserts); `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` run 1 UEFI BOOT PASS exit 0 serial 1360 bytes guest 112640 bytes; run 2 UEFI BOOT PASS exit 0 serial 1360 bytes (byte-identical sizes to the slice26b record; both runs `CHAIN hop=2/3/1` in order + `ok=1 chain=1231` + `PF ok=1 vector=14` + `EXC ok=1 vector=0`, no `REFUSAL-FAIL`, budget ok=True). Sitrep detector note: report claimed `1 changed main.rs` with a clean tree pre-change (status/diff/log triple clean @494f8bf) — 7th consecutive false-dirty, same stale pattern as #419 x6.
- **Final:** Slice27 join/preempt semantics + RSP/order disproof + N=2 QEMU landed; #18 stays ACTIVE for the owed live-carry second half (static chain cursor across hops, replacing fresh-model replay in `chain_resolve_and_jump`/`chain_verify_full`).

## S06 slice27b -- live-carry chain cursor + guest JOIN/EXIT proof + QEMU x2 (2026-10-04)

- **Decision:** Close #18 in `crates/boot-uefi/src/main.rs` + `crates/kernel/src/sched.rs` + `tools/qemu/boot-uefi.ps1`: (a) static live-carry cursor (`CHAIN_CURSOR` / `CHAIN_CURSOR_SEEDED` / `CHAIN_CURSOR_LEGS` in `.bss`, `addr_of!`-only access so the guest build stays `static_mut_refs`-clean): leg 1 seeds once (second seed refuses), legs resolve against the CARRIED cursor with no replay loop (`chain_cursor_leg`), the terminal hop proves seed-once + 3 advances + back-on-1 via `chain_cursor_terminal` with zero fresh replay, emitting `CHAIN-CURSOR ok=1 legs=3` (any refusal emits `ok=0` + fall-through to bring-up, never silent); (b) guest JOIN/EXIT proof (`probe_sched_join_exit`, same fn on host and guest): dispatch 1->2, `exit_current` marks 2 `Done` resumes 3, `join(2)` true while `join(3)` false, emitting `SCHED-EXIT ok=1 exited=2 resumed=3` + `SCHED-JOIN ok=1 joined=2` before the chain; (c) `Scheduler::exit_current` marks the FIRST id match only then `break`s (REVIEW #421 gap 3: `spawn` forbids dup ids, so all-match marking would mask a dup-id smuggle); the disproof test's local `live_as` now calls the guest `chain_live_as` directly (gap 3 dedup); (d) ps1 hard-requires all three new lines (`Contains` gates, no new regex).
- **Why:** REVIEW #421 accepted slice27a with three owed gaps: (1) join/exit had zero guest wiring (QEMU x2 re-proved the unchanged serial path), (2) risk (a) open by construction (fresh `Scheduler::new(1)` replay per hop), (3) all-match exit masking + `live_as` duplication. The cursor crosses the never-return `jmp` lifetime boundary via `.bss` (a stack `Scheduler` cannot survive `jmp`), seeded once and advanced hop-by-hop — replay is gone, not just re-verified.
- **Alternatives:** Carry only `current: u64` + rebuild states per hop (rejected: states would still be replay-derived, not carried — the dispatched `Ready`/`Running` marks are the load-bearing state); thread the cursor through hop fn args (rejected: `jmp` entries take no args by construction — `jump_to_thread_stack(top, func)` calls `extern "C" fn()`); single QEMU run (rejected: slice27a set the N=2 second-seed rule — two fresh runs on the changed tree).
- **Expected:** `cargo test --workspace` 175/175 (21+30+114+4+4+2, +2 boot-uefi: carry + join/exit shapes); fmt clean; clippy boot-uefi-tests + kernel clean (workspace-wide `clippy --workspace` fails identically on the clean pre-change tree — pre-existing `no_std` bin shape, not this slice); gates 41/41; `boot-uefi.ps1` PASS x2 with `CHAIN hop=2/3/1` in order + `ok=1 chain=1231` + `CHAIN-CURSOR ok=1 legs=3` + `SCHED-EXIT/JOIN ok=1` + PF/EXC.
- **Measured (2026-10-04, Win11):** `cargo test --workspace` 175/175 (21 boot-info + 30 boot-uefi incl. new `chain_cursor_carry_host` + `sched_join_exit_probe_shapes` + 114 kernel + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -p baseos-boot-uefi --tests` zero warnings (after fixing two `doc_lazy_continuation` hits in new docs); `cargo clippy -p baseos-kernel -p baseos-boot-info -p baseos-serial-marker -p baseos-version` clean; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` run 1 UEFI BOOT PASS exit 0 serial 1445 bytes guest 116736 bytes (up from 1360B/112640B: +85 serial for EXIT+JOIN+CURSOR lines, +4096 guest — one page of cursor/probe code); run 2 UEFI BOOT PASS exit 0 serial 1445 bytes (byte-identical): marker FOUND, `BOOTINFO valid=1 regions=98 usable=0xce78000`, `FRAMEALLOC ok=1 free=52856 spans=8 faults=3 stitch=5`, `SCHED-EXIT ok=1 exited=2 resumed=3`, `SCHED-JOIN ok=1 joined=2`, `CHAIN hop=2/3/1` in order + `ok=1 chain=1231` + `CHAIN-CURSOR ok=1 legs=3`, `PF ok=1 vector=14`, `EXC ok=1 vector=0`, no `REFUSAL-FAIL`, budget ok=True. Guest-build warning check: final `Finished` with zero `warning:` lines (one `static_mut_refs` hit on the first PASS fixed via `addr_of!`-only access, re-PASSed clean).
- **Final:** #18 live-carry + guest JOIN/EXIT landed and measured; REVIEW #421 gaps 1-3 closed builder-side (guest JOIN/EXIT lines live, static cursor replaces replay, first-match exit + `chain_live_as` dedup). #18 ready to close; preempt-IRQ gate (`cli`/atomic around the pick) stays the documented future slice per the coop-only contract.

## S06 preempt-IRQ gate slice -- IrqGuard + gated dispatch/exit + QEMU x2 (2026-10-04)

- **Decision:** Land task #19 (the #423 residual (b) coop-only preempt point) as one slice in `crates/kernel/src/cpu.rs` + `crates/kernel/src/sched.rs` + `crates/boot-uefi/src/main.rs` (no ps1 change — serial contract byte-identical): (a) new `cpu::cli()` unsafe wrapper (guest `cli` without `preserves_flags` since it clears IF; host no-op stub) plus `cpu::IrqGuard` proof token — zero-cost on ALL builds (no asm inside `enter`, by necessity: `boot-uefi` links `baseos_kernel` as a dependency so the dependency's `cfg(test)` no-op never applies there, and a per-dispatch `cli` faulted host tests with `STATUS_PRIVILEGED_INSTRUCTION`; the live `cli` stays established ONCE in `efi_main` post-EBS and the token proves the site runs downstream of it; `Drop` never `sti` — no IRQ handler for vectors 32+ exists yet); (b) new `Scheduler::dispatch_gated(&IrqGuard)` / `exit_current_gated(&IrqGuard)` delegating to the ungated primitives (gate adds masking proof, never new pick logic); (c) guest `chain_leg` takes `&IrqGuard` and dispatches gated; `chain_cursor_leg`, both `probe_sched_switch` bodies, and `probe_sched_join_exit` create the guard and run gated; host `chain_leg` callers pass the same token; (d) three host tests: `irq_gate_is_unsafe_enter_and_host_noop` (unsafe-fn shape pin), `gated_dispatch_and_exit_agree_with_ungated` (success + refusal parity on both primitives), `gated_dispatch_models_preempt_race_as_serialized` (cloned-state race model: two pre-pick clones both pick 2, sequential gated dispatches advance 1->2->3 — cheapest disproof per #426 since no host test can fire a real IRQ); (e) incidental fix: collapse a pre-existing `collapsible_if` in guest `chain_resolve_and_jump` that failed UEFI-target `clippy -D warnings` (behavior-neutral, QEMU re-proves it).
- **Why:** REVIEW #423 accepted slice27b but left preempt coop-only (pick-then-mark TOCTOU: two preempted picks both observe the same `Ready` target and double-dispatch); agent-b #426 endorses this exact gate slice before S07 with an IRQ-injected race test or IRQ-storm QEMU as cheapest disproof. A real timer IRQ cannot exist yet (no handler for 32+, `cli` in effect, single-threaded boot), so the slice pins the gate shape the future IRQ will call (same pick under the gate) plus the race-class model, with unchanged-behavior QEMU x2 as the no-regression proof.
- **Alternatives:** Per-dispatch `cli` inside `IrqGuard::enter` (rejected: tried — faults host `cargo test` via the dependency-`cfg(test)` rule above; the once-in-`efi_main` cli plus token is the sound shape); atomic-CAS table instead of `cli` gate (rejected: single-CPU post-EBS boot has no concurrency yet — `cli` critical section is sufficient and free; atomics ride with SMP); separate IRQ-storm QEMU with forced timer (rejected: no timer source to storm — would need IDT32+ + PIT/APIC programming, a multi-slice S07 feature, not a one-segment gate); guest `sti` on guard drop (rejected: unwired IDT32+ would fault on first IRQ — documented as the future timer slice's paired change).
- **Expected:** `cargo test --workspace` 178/178 (175 + 3: 1 cpu gate + 2 sched gated/race); fmt clean; clippy host-all-targets + boot-uefi-tests + UEFI-target clean; gates 41/41; `boot-uefi.ps1` PASS x2 with serial byte-identical to slice27b (1445B, all `SCHED-EXIT/JOIN` + `CHAIN` + `CURSOR` + PF/EXC lines intact).
- **Measured (2026-10-04, Win11):** `cargo test --workspace` 178/178 (21 boot-info + 30 boot-uefi + 117 kernel incl. new `irq_gate_is_unsafe_enter_and_host_noop` + `gated_dispatch_and_exit_agree_with_ungated` + `gated_dispatch_models_preempt_race_as_serialized` + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean (after one `cargo fmt` on the collapsed if); `cargo clippy --all-targets -- -D warnings` clean (host); `cargo clippy -p baseos-boot-uefi --tests -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean (after the `collapsible_if` collapse); `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` run 1 UEFI BOOT PASS exit 0 serial 1445 bytes; run 2 UEFI BOOT PASS exit 0 serial 1445 bytes (byte-identical serial to slice27b), guest `bootx64.efi` 117248 bytes (+512 vs 116736 slice27b — one page of gate/token code; serial contract unchanged, full `SCHED-SWITCH/EXIT/JOIN` + `THREAD-JUMP ok=0/ok=1` + `CHAIN hop=2/3/1` + `ok=1 chain=1231` + `CHAIN-CURSOR ok=1 legs=3` + `PF ok=1 vector=14` + `EXC ok=1 vector=0`, no `REFUSAL-FAIL`, budget ok=True).
- **Final:** #423 residual (b) closed builder-side as a gate shape (pick runs under proven `cli`; timer IRQ reuses the same gated pick): gated == ungated behavior pinned host-side, race class modeled, QEMU x2 no-regression. #19 ready to close on agent-b re-verify; S07 scheduler next per roadmap.

## S07 slice 1 -- scheduler CPU accounting + SCHED-STAT e2e (2026-10-04)

- **Decision:** Open S07 (first measurable scheduler) with CPU accounting as one slice in `crates/kernel/src/sched.rs` + `crates/boot-uefi/src/main.rs` + `tools/qemu/boot-uefi.ps1`: (a) `Scheduler` gains `runs: [u64; MAX_THREADS]` (parallel to `table`, zeroed on spawn slot), saturating +1 only on successful dispatch (`dispatch` and `exit_current` resume paths; refusals mutate nothing), with read-only `runs_of(id)` / `total_dispatches()`; (b) guest `probe_sched_stat` drives 1->2->3->1 under `IrqGuard` and emits `SCHED-STAT ok=<0|1> total=<n> r1=<n> r2=<n> r3=<n>` (`ok=1` only on total=3 with each 1; failure zeroes counts so short rounds cannot pass); (c) ps1 hard-requires `SCHED-STAT ok=1 total=3 r1=1 r2=1 r3=1`; (d) three host tests: `accounting_counts_dispatch_and_exit` (1->2->3->1 then exit resumes 2, totals 3->4), `accounting_unknown_is_none_and_refusal_counts_nothing` (unknown None, single-thread refusal counts nothing), `sched_stat_probe_shape` (exact good + fail-loud lines).
- **Why:** S06 closed with the IRQ gate shape but zero scheduler measurement; roadmap S07 verification needs fairness/throughput baselines first. Per-dispatch counters are the smallest end-to-end measurable (no timer/preempt yet — round-robin counts prove the accounting path the future slice/quantum will feed).
- **Alternatives:** Timer quantum + preempt now (rejected: needs IDT32+ + PIT/APIC programming, multi-slice S07 feature — accounting must exist first); tracing/throughput harness now (rejected: S08 research-harness scope — one counter + one serial line keeps this cycle committable); in-guest `sti` for preemptible dispatch (rejected: same as S06 gate — unwired IDT32+ would fault on first IRQ).
- **Expected:** `cargo test --workspace` 181/181 (178 + 3); fmt clean; clippy host-all-targets + UEFI-target clean; `boot-uefi.ps1` PASS with `SCHED-STAT ok=1 total=3 r1=1 r2=1 r3=1`.
- **Measured (2026-10-04, Win11):** `cargo test --workspace` 181/181 (21 boot-info + 31 boot-uefi incl. new `sched_stat_probe_shape` + 119 kernel incl. new `accounting_counts_dispatch_and_exit` + `accounting_unknown_is_none_and_refusal_counts_nothing` + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS exit 0 serial 1484 bytes guest 120832 bytes (+39 serial / +3584 guest vs S06 gate 1445B/117248B — one STAT line + accounting code; `SCHED-STAT ok=1 total=3 r1=1 r2=1 r3=1` present, all prior `SCHED-SWITCH/EXIT/JOIN` + `CHAIN` + `CURSOR` + PF/EXC intact, budget ok=True). QEMU x1 this cycle (second run rides with review re-verify).
- **Final:** S07 accounting baseline landed and measured; REVIEW #428 accepts S06 gate @b7a28c3 (178 re-verified, collapsible_if neutral). #19 ready to close; next S07 slice 2 (time slice / fairness sweep or trace).

## S07 slice 2 -- equal-quantum fairness sweep + SCHED-FAIR e2e (2026-10-04)

- **Decision:** Second S07 slice in `crates/kernel/src/sched.rs` + `crates/boot-uefi/src/main.rs` + `tools/qemu/boot-uefi.ps1`: (a) `Scheduler::fairness_spread()` (`max(runs)-min(runs)`, 0 when empty, saturating, read-only) plus the REVIEW #431 F1 slot-index invariant documented (no removal/compaction exists; a future one must shift `runs` with `table`); each successful `dispatch` is one equal quantum (timer preempt still future, reuses the same gated pick); (b) guest `probe_sched_fair` drives ten 1->2->3->1 rounds (30 gated dispatches) and emits `SCHED-FAIR ok=<0|1> total=<n> r1=<n> r2=<n> r3=<n> spread=<n>` (`ok=1` only on 30/10/10/10/spread 0; refusal/short/skew zeroes the line); (c) ps1 hard-requires `SCHED-FAIR ok=1 total=30 r1=10 r2=10 r3=10 spread=0` (Contains, alongside the unchanged STAT gate); (d) three host tests: `fairness_sweep_ten_rounds_spread_zero` (30 dispatches, exact 10s, spread 0), `fairness_spread_partial_round_bounded_by_one_and_empty_zero` (mid-round spread 1, refusal/empty 0), `sched_fair_probe_shape` (exact good + fail-loud lines).
- **Why:** REVIEW #431 carried F3 (accounting counts dispatches, not time — quantum/fairness owed) and F4 (single QEMU run) into slice2; roadmap S07 verification lists fairness before tail/interactive. Ten equal-quanta rounds are the smallest end-to-end fairness proof that runs without a timer source.
- **Alternatives:** Timer quantum + preempt now (rejected: needs IDT32+ + PIT/APIC programming, multi-slice S07 feature — the sweep proves the fairness path the timer slice will feed); throughput/latency tracing now (rejected: S08 research-harness scope — one spread helper + one serial line keeps this cycle committable); quantum field with per-thread budgets (rejected: budgets without a preempt source are untestable weight — equal quanta with a measured spread bound is the honest baseline).
- **Expected:** `cargo test --workspace` 184/184 (181 + 3); fmt clean; clippy host-all-targets + UEFI-target clean; `boot-uefi.ps1` PASS x2 with both `SCHED-STAT ok=1` and `SCHED-FAIR ok=1` lines.
- **Measured (2026-10-04, Win11):** `cargo test --workspace` 184/184 (21 boot-info + 32 boot-uefi incl. new `sched_fair_probe_shape` + 121 kernel incl. new `fairness_sweep_ten_rounds_spread_zero` + `fairness_spread_partial_round_bounded_by_one_and_empty_zero` + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean (after one `unused_mut` fix on the empty-table spread assert); `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `boot-uefi.ps1` run 1 UEFI BOOT PASS exit 0 serial 1536 bytes, run 2 UEFI BOOT PASS exit 0 serial 1536 bytes (byte-identical size; serial carries `SCHED-STAT ok=1 total=3 r1=1 r2=1 r3=1` + `SCHED-FAIR ok=1 total=30 r1=10 r2=10 r3=10 spread=0`, all prior SWITCH/EXIT/JOIN + CHAIN + CURSOR + PF/EXC intact, budget map_size=4704/desc_size=48 ok=True), guest `bootx64.efi` 123392 bytes (+2560 vs 120832 slice1 — one FAIR probe + writer + spread helper).
- **Final:** Equal-quantum fairness baseline landed and measured QEMU x2 (closes REVIEW #431 F4 for this slice; F1 documented, F3 fairness half closed — timer-quantum/trace still owed on #20). #20 stays active for the quantum/trace remainder; pending agent-b review.

## S07 slice 3 -- dispatch-order trace + SCHED-TRACE e2e (2026-10-04)

- **Decision:** Third S07 slice in `crates/kernel/src/sched.rs` + `crates/boot-uefi/src/main.rs` + `tools/qemu/boot-uefi.ps1`: (a) `Scheduler` gains `trace: [u64; TRACE_CAP=8]` + `trace_len`, recording the first 8 successful quanta as thread ids in dispatch order (`dispatch` and `exit_current` resume paths; refusals record nothing; saturation keeps the prefix, no wrap); read-only `trace_len()` / `trace_at(i)` (None out of range); trace is id-keyed, never slot-indexed, so a future remove/compact cannot misattribute it; (b) guest `probe_sched_trace` drives one 1->2->3->1 round under `IrqGuard` and emits `SCHED-TRACE ok=<0|1> n=<n> t0=<id> t1=<id> t2=<id>` from the recorded ids (not the dispatch returns, so e2e proves the trace path itself; `ok=1` only on `n=3 t0=2 t1=3 t2=1`, failure zeroes the line); (c) ps1 hard-requires `SCHED-TRACE ok=1 n=3 t0=2 t1=3 t2=1` (Contains, alongside STAT/FAIR); (d) three host tests: `trace_records_dispatch_order_and_stops_at_cap` (3-round order 2/3/1, 30-sweep pins the 8-prefix 2,3,1,2,3,1,2,3 with counts still 10s, solo refusal records nothing), `slot_index_invariant_append_only_with_id_keyed_trace` (REVIEW #433 F1 regression: two rounds pin per-id 2s + len/id stability, so a future table-shift without `runs` goes red), `sched_trace_probe_shape` (exact good + fail-loud lines).
- **Why:** REVIEW #433 accepted slice2 but held #20 active for timer-quantum/trace (F2/F3: spread==0 proves the counting path with 30%3==0, not contention; dispatch-as-quantum is definitional with no timer source) and named the F1 slot-index invariant doc-only. True preemptive quantum needs IDT32+ + PIT/APIC (multi-slice S07 feature); the trace of equal quanta in dispatch order is the smallest end-to-end tracing proof (roadmap S07 work item) that runs without a timer, and the F1 regression test is the cheapest close-out (id-keyed trace needs no such pin by construction).
- **Alternatives:** Timer quantum + preempt now (rejected: needs interrupt vectors 32+ plus timer programming — doubles single-cycle QEMU risk; the trace proves the order path the timer slice will timestamp); ring-buffer last-N with wrap (rejected: wrap logic is untestable weight for a 3-entry probe — prefix-only is deterministic and the S08 harness owns tail tracing); quantum budgets per thread (rejected: same as slice2 — budgets without a preempt source are untestable weight); storing `runs` inside `Thread` to kill the slot invariant structurally (rejected: touches the thread contract for one doc pin — the append-only regression test plus id-keyed trace is the one-segment fix).
- **Expected:** `cargo test --workspace` 187/187 (184 + 3); fmt clean; clippy host-all-targets + UEFI-target clean; gates 41/41; `boot-uefi.ps1` PASS x2 with `SCHED-TRACE ok=1 n=3 t0=2 t1=3 t2=1` alongside STAT/FAIR.
- **Measured (2026-10-04, Win11):** `cargo test --workspace` 187/187 (21 boot-info + 33 boot-uefi incl. new `sched_trace_probe_shape` + 123 kernel incl. new `trace_records_dispatch_order_and_stops_at_cap` + `slot_index_invariant_append_only_with_id_keyed_trace` + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean (after one auto-format of the new assert); `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` run 1 UEFI BOOT PASS exit 0 serial 1572 bytes, run 2 UEFI BOOT PASS exit 0 serial 1572 bytes (byte-identical; serial carries `SCHED-STAT ok=1 total=3 r1=1 r2=1 r3=1` + `SCHED-FAIR ok=1 total=30 r1=10 r2=10 r3=10 spread=0` + `SCHED-TRACE ok=1 n=3 t0=2 t1=3 t2=1`, all prior SWITCH/EXIT/JOIN + CHAIN + CURSOR + PF/EXC intact, budget map_size=4704/desc_size=48 ok=True), guest `bootx64.efi` 125440 bytes (+2048 vs 123392 slice2 — trace fields + probe + writer).
- **Final:** First dispatch-order tracing landed and measured QEMU x2; REVIEW #433 F1 closed via the append-only regression (trace id-keyed by construction). #20 stays active for the timer-quantum remainder (preempt IRQ + timestamps); pending agent-b review.

## S07 slice 4 -- exit-path trace + SCHED-TRACE-X e2e (2026-10-04)

- **Decision:** Fourth S07 slice in `crates/kernel/src/sched.rs` + `crates/boot-uefi/src/main.rs` + `tools/qemu/boot-uefi.ps1`: (a) host test `trace_records_exit_current_resume` pins the `exit_current` record path (dispatch 1->2 records 2, exit 2 resumes 3 records 3, counts agree; unknown-current exit records nothing); (b) guest `probe_sched_trace_exit` drives dispatch 1->2 then `exit_current_gated` (2 Done, resume 3) under `IrqGuard` and emits `SCHED-TRACE-X ok=<0|1> n=<n> t0=<id> t1=<id>` from the recorded ids, not the call returns (`ok=1` only on `n=2 t0=2 t1=3`, failure zeroes the line); (c) ps1 hard-requires `SCHED-TRACE-X ok=1 n=2 t0=2 t1=3` (Contains, alongside STAT/FAIR/TRACE); (d) shape test `sched_trace_x_probe_shape` (exact good + fail-loud lines).
- **Why:** REVIEW #435 accepted slice3 but named the exit_current trace path as host-only (guest proved dispatch recording only) with tail/timestamps + timer-quantum still owed. True preemptive quantum needs IDT32+ + PIT/APIC (multi-slice feature, same rejection as slices 1-3); the exit-resume record is the smallest end-to-end tracing proof left that runs without a timer, and it closes the named residual directly.
- **Alternatives:** Timer quantum + preempt now (rejected: needs interrupt vectors 32+ plus timer programming — doubles single-cycle QEMU risk; the exit probe proves the second record path the timer slice will timestamp); extend the TRACE line with exit ids (rejected: changes the slice-3 contract + gate; append-only second line keeps both); ring/timestamps now (rejected: S08 harness scope — prefix-only stays deterministic).
- **Expected:** `cargo test --workspace` 189/189 (187 + 2); fmt clean; clippy host-all-targets + UEFI-target clean; gates 41/41; `boot-uefi.ps1` PASS x2 with both `SCHED-TRACE ok=1` and `SCHED-TRACE-X ok=1` lines.
- **Measured (2026-10-04, Win11):** `cargo test --workspace` 189/189 (21 boot-info + 34 boot-uefi incl. new `sched_trace_x_probe_shape` + 124 kernel incl. new `trace_records_exit_current_resume` + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` run 1 UEFI BOOT PASS exit 0 serial 1605 bytes, run 2 UEFI BOOT PASS exit 0 serial 1605 bytes (byte-identical; serial carries `SCHED-STAT ok=1 total=3 r1=1 r2=1 r3=1` + `SCHED-FAIR ok=1 total=30 r1=10 r2=10 r3=10 spread=0` + `SCHED-TRACE ok=1 n=3 t0=2 t1=3 t2=1` + `SCHED-TRACE-X ok=1 n=2 t0=2 t1=3`, all prior SWITCH/EXIT/JOIN + CHAIN + CURSOR + PF/EXC intact, budget map_size=4704/desc_size=48 ok=True), guest `bootx64.efi` 126976 bytes (+1536 vs 125440 slice3 — exit probe + writer + test).
- **Final:** Exit-path recording proven live end-to-end; REVIEW #435 exit-trace residual closed. #20 stays active for timer-quantum + tail/timestamps (preempt IRQ + S08 harness); pending agent-b review.

## S07 slice 5 -- logical quantum tick + SCHED-TICK e2e (2026-10-04)

- **Decision:** Fifth S07 slice in `crates/kernel/src/sched.rs` + `crates/boot-uefi/src/main.rs` + `tools/qemu/boot-uefi.ps1`: (a) `Scheduler` gains `QUANTUM_TICKS=4` + `ticks_left` (global, one running thread at a time; per-thread budgets ride with a later priorities slice): every successful `dispatch` / `exit_current` resume arms `ticks_left=QUANTUM_TICKS`, parking on `NULL` disarms to 0, refusals leave it untouched; `tick()` saturating-counts down and reports expiry (`ticks_left==0`, idle reports expired so an un-armed scheduler asks for a dispatch), `tick_gated(&IrqGuard)` is the identical countdown the future PIT/APIC handler calls under the gate before `dispatch_gated` on expiry; (b) guest `probe_sched_tick` drives dispatch 1->2 then exactly 4 `tick_gated` (first 3 must report not-expired, last expired) then `dispatch_gated` to 3, all under one `IrqGuard` (coop model of the timer-IRQ serialized shape), emitting `SCHED-TICK ok=<0|1> q=<n> used=<n> expired=<0|1> next=<id>` from the counter values (`ok=1` only on `q=4 used=4 expired=1 next=3`, failure zeroes the line); (c) ps1 hard-requires `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` (Contains, alongside STAT/FAIR/TRACE/TRACE-X); (d) three host tests: `quantum_tick_counts_down_and_rearms_on_dispatch` (arm/countdown/expiry/saturating/rearm/disarm/refusal-untouched), `quantum_gated_tick_agrees_with_ungated` (gated tick-for-tick parity + gated-ticks-then-gated-dispatch resched), `sched_tick_probe_shape` (exact good + fail-loud lines).
- **Why:** REVIEW #437 accepted slice4 but carried timer-quantum + tail/timestamps on #20, noting the timer slice must prove the IRQ-racy path and that a real PIT/APIC + IDT32+ program is the multi-slice S07 feature. The logical tick is the smallest committable quantum that runs without a timer source: it pins the arm/countdown/expiry/resched contract plus the gated shape the IRQ handler will call, under the same single-guard proof as slice4.
- **Alternatives:** Real PIT/APIC + IDT32+ now (rejected: interrupt vectors 32+, PIC remap, PIT programming, ISR wiring — doubles single-cycle QEMU risk; the tick proves the quantum path the IRQ slice will drive); per-thread budgets/priorities now (rejected: budgets without a preempt source are untestable weight — one global counter with a measured expiry bound is the honest baseline); timestamps/tail ring now (rejected: S08 harness scope — prefix-only trace stays deterministic).
- **Expected:** `cargo test` 192 (189 + 3); fmt clean; clippy host-all-targets + UEFI-target clean; gates 41/41; `boot-uefi.ps1` PASS x2 with `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` alongside STAT/FAIR/TRACE/TRACE-X.
- **Measured (2026-10-04, Win11):** `cargo test` 192 (21 boot-info + 35 boot-uefi incl. new `sched_tick_probe_shape` + 126 kernel incl. new `quantum_tick_counts_down_and_rearms_on_dispatch` + `quantum_gated_tick_agrees_with_ungated` + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean (after one auto-format); `cargo clippy --all-targets -- -D warnings` exit 0 (after one `assert_eq!(x, true)` -> `assert!(x)` fix); `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41); `boot-uefi.ps1` run 1 UEFI BOOT PASS exit 0 serial 1649 bytes, run 2 UEFI BOOT PASS exit 0 serial 1649 bytes (byte-identical; serial carries `SCHED-STAT ok=1 total=3 r1=1 r2=1 r3=1` + `SCHED-FAIR ok=1 total=30 r1=10 r2=10 r3=10 spread=0` + `SCHED-TRACE ok=1 n=3 t0=2 t1=3 t2=1` + `SCHED-TRACE-X ok=1 n=2 t0=2 t1=3` + `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3`, all prior SWITCH/EXIT/JOIN + CHAIN + CURSOR + PF/EXC intact, budget map_size=4704/desc_size=48 ok=True), guest `bootx64.efi` 130048 bytes (+3072 vs 126976 slice4 — tick counter + probe + writer + tests).
- **Final:** Logical quantum proven live end-to-end (arm/countdown/expiry/resched + single-guard IRQ model). #20 stays active for the real timer IRQ (IDT32+PIT/APIC) + tail/timestamps (S08 harness); pending agent-b review.

## S07 slice 6 -- quantum-tick hardening: const-driven gates + intended contracts (2026-10-04)

- **Decision:** Sixth S07 slice in `crates/kernel/src/sched.rs` + `crates/boot-uefi/src/main.rs` (no ps1 change): close all three REVIEW #439 residuals without starting the timer-IRQ feature — (a) const-driven gates: probe `ok` drops the `QUANTUM_TICKS == 4 && used == 4` literal for `used == QUANTUM_TICKS`, kernel unit countdown loops `while i < QUANTUM_TICKS` instead of three hardcoded `assert!(!tick)`, boot-uefi shape test builds the expected line with `format!(q={} used={}, QUANTUM_TICKS, ...)`; a quantum change now touches only the const + the ps1 `q=<n>` literal + doc examples; (b) residual 1 documented as intended: early yield discards the remainder (dispatch rearms full `QUANTUM_TICKS`, no carry) until per-thread budgets + priorities; (c) residual 2 documented as timer-slice rule: idle/`NULL`-parked `tick()` reports expired, so the future PIT/APIC handler must gate the re-tick on a running thread and `dispatch` once on expiry, never spin `tick()`.
- **Why:** REVIEW #439 accepted slice5 but kept #20 active with exactly these residuals (remainder-carry, idle re-tick loop risk, triple-edit magic 4). The timer IRQ (IDT32+PIT/APIC) is a multi-slice feature; the contracts it must obey plus the const-driven gates are the smallest committable close-out that keeps every existing check green.
- **Alternatives:** Real PIT/APIC + IDT32+ now (rejected: interrupt vectors, PIC remap, PIT programming, ISR wiring — doubles single-cycle QEMU risk; this slice pins the rules the IRQ slice must follow instead); per-thread budgets now (rejected: budgets without a preempt source are untestable weight); making ps1 read the const (rejected: ps1 cannot import Rust; the literal stays as the e2e pin for the current value).
- **Expected:** `cargo test` 192 (unchanged count, refactored asserts); fmt clean; clippy host-all-targets + UEFI-target clean; `boot-uefi.ps1` PASS x2 with `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` unchanged.
- **Measured (2026-10-04, Win11):** `cargo test` 192 (21 boot-info + 35 boot-uefi incl. const-driven `sched_tick_probe_shape` + 126 kernel incl. looped `quantum_tick_counts_down_and_rearms_on_dispatch` + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `boot-uefi.ps1` run 1 UEFI BOOT PASS exit 0 serial 1649 bytes, run 2 UEFI BOOT PASS exit 0 serial 1649 bytes (serial carries `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3`, all prior STAT/FAIR/TRACE/TRACE-X + SWITCH/EXIT/JOIN + CHAIN + CURSOR + PF/EXC intact), guest `bootx64.efi` 130048 bytes (unchanged size — comment/test-only delta).
- **Final:** REVIEW #439 residuals closed on the current tree (contracts documented, gates const-driven). #20 stays active for the real timer IRQ (IDT32+PIT/APIC) + tail/timestamps (S08 harness); pending agent-b review.

## S07 slice 7a -- real-timer IRQ contract: PIT/PIC/IDT32 constants + TimerTicks (2026-10-05)

- **Decision:** Seventh S07 slice (host-only contract, no guest wiring yet) in new `crates/kernel/src/timer.rs` + `pub mod timer` in `crates/kernel/src/lib.rs`: (a) `PIT_BASE_HZ=1_193_182` + `pit_divisor(hz)` (0 Hz fails closed to 0) + `TIMER_HZ=100` + `TIMER_DIVISOR=11931`; (b) PIC remap bases `PICM_OFFSET=0x20`/`PICS_OFFSET=0x28` so IRQ0 lands on `VECTOR_TIMER=32`, plus `PIT_CH0=0x40`/`PIT_CMD=0x43`/`PIT_MODE_CH0_SQUARE=0x36`; (c) `TimerTicks` saturating HW-tick counter (`tick`/`pending`/`take`) the future IRQ0 handler increments and the scheduler drain feeds to `tick_gated` once per HW tick; (d) four host tests (100 Hz divisor contract, zero-Hz fail-closed, IRQ0-on-32 pin, drain+saturate).
- **Why:** #20 carries the real timer IRQ (IDT32+PIT/APIC); the guest keeps `cli` until the wiring slice programs hardware + installs the returning IRQ0 stub (EOI + `iretq`). This slice pins the pure `core`-only constants + counter first so the wiring slice cannot silently drift the divisor/vector contract.
- **Alternatives:** Full PIT/PIC/IDT32 + ISR wiring now (rejected: port I/O + `lidt`/`sti` + returning-IRQ frame doubles single-cycle QEMU risk; contract first, wiring next); per-thread tick budgets now (rejected: untestable weight without a preempt source).
- **Expected:** `cargo test --workspace` 196/196 (192 + 4); fmt clean; clippy kernel clean; no QEMU change (guest still `cli`, behavior unchanged).
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 196/196 (21 boot-info + 35 boot-uefi + 130 kernel incl. 4 new timer tests + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0.
- **Final:** Timer IRQ contract pinned host-side; guest PIT/PIC/IDT32 programming + IRQ0 stub + `TimerTicks`-to-`tick_gated` drain owed next. #20 stays active; pending agent-b review.

## S07 slice 7b -- REVIEW #442 follow-up: divisor-overflow fail-closed + cli-gate doc (2026-10-05)

- **Decision:** Close agent-b REVIEW #442 risks 1+2 in `crates/kernel/src/timer.rs` without starting guest wiring: (a) risk 1 — `pit_divisor` now fails closed to `0` when the quotient is not programmable (`hz == 0`, quotient `0`, or quotient `> 0xFFFF`), closing the silent `as u16` truncation (`hz=1` gave 13534 before, now `0`); (b) risk 2 — `TimerTicks` documents the cli-gated critical-section contract (plain `u64`, not atomic; wiring slice must land `AtomicU64` or `cli`-gated `tick`/`take` before programming the PIT/enabling IRQ0) and `take()` becomes `const fn` for `tick`/`new` symmetry; (c) risk 3 (count wording) — record here that `196` in slice7a was the workspace total (21 boot-info + 35 boot-uefi + 130 kernel-lib incl. 4 timer + 4 + 4 serial-marker + 2 version), not the kernel-lib-only count. One new host test: `pit_divisor_overflow_fails_closed` (`1/2/18 -> 0`, `19 -> 62799`, `20 -> 59659`, `1193182 -> 1`, `2000000 -> 0`).
- **Why:** Review #442 verified slice7a consts correct (1193182/100=11931, 0x20/0x28, ports/mode) but showed the cheapest disconfirm `pit_divisor(1)==1193182` fails today (truncation, not fail-closed) and `tick` vs `take` races once real IRQ0 lands. Both close host-side with no QEMU risk before the wiring slice.
- **Alternatives:** Return `u32`/`Option<u16>` from `pit_divisor` (rejected: changes the `TIMER_DIVISOR: u16` contract mid-stream; fail-closed `0` keeps the existing "caller must refuse zero" rule, which already covers the `0x0000`=65536 hardware alias); switch `TimerTicks` to `AtomicU64` now (rejected: `core::sync::atomic` in the `no_std` guest + IRQ-vs-drain ownership needs the wiring slice's `cli`/`sti` design, not a lone type swap); full PIT/PIC/IDT32 wiring now (rejected: doubles single-cycle QEMU risk; contract hardening first).
- **Expected:** `cargo test --workspace` 197/197 (196 + 1); fmt clean; kernel clippy `-D warnings` clean; no QEMU change (guest still `cli`).
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 197/197 (21 boot-info + 35 boot-uefi + 131 kernel-lib incl. 5 timer tests + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean.
- **Final:** REVIEW #442 risks 1–3 closed on the current tree; guest PIT/PIC/IDT32 programming + IRQ0 stub + drain owed next. #20 stays active; pending agent-b review.

## S07 slice 8 -- wiring-prep: program-bytes refusal + EOI consts + gated drain + IRQ0 stub contract (2026-10-05)

- **Decision:** Host-only wiring-prep in `crates/kernel/src/timer.rs` + `crates/kernel/src/sched.rs` without touching the guest (still `cli`, no `global_asm!`, no IDT extend, no port I/O): (a) `pit_program_bytes(divisor) -> Option<(u8, u8)>` splits the divisor little-endian and refuses `0` (`None`) so the `0x0000` = 65536 latch can never reach the port — closes REVIEW #444 residual "no caller enforces the zero-divisor refusal"; (b) PIC consts `PICM_CMD/DATA`, `PICS_CMD/DATA`, `PIC_EOI` (IRQ0 is master-only: exactly one EOI byte to `0x20`); (c) `TimerTicks::take_gated(&IrqGuard)` — identical drain with the `cli` proof, raw `take()` documented host-only — closes REVIEW #444 residual "no AtomicU64/cli gate exists" on the drain side (`tick` IRQ-side gate rides with the wiring slice's handler); (d) `timer_isr_stub_asm!` string-only IRQ0 stub (Win64 align + shadow + `call timer_irq_rust` + master EOI + `iretq`, never park) with `timer_stub_text()` test accessor and `#[allow(unused_macros)]` until the wiring slice consumes it via `global_asm!`; (e) `Scheduler::drain_timer(&mut TimerTicks, &IrqGuard) -> (drained, expired)` — `take_gated` then one `tick_gated` per HW tick, empty drain reports `(0, false)` (never dispatches on silence), caller dispatches ONCE on `expired` (never a tight re-tick loop per slice-6 rule). Six new host tests (4 timer + 2 sched).
- **Why:** REVIEW #444 approved slice7b but kept #20 active with exactly these residuals: zero-divisor refusal unenforced and no gate on tick/take. Real PIT/PIC/IDT32 + `sti` is a multi-risk guest slice (port I/O, remap order, IDT extend to vector 32, returning-IRQ frame); the refusal helper + EOI consts + gated drain + stub shape are the smallest committable close-out that keeps every existing check green and pins the contracts the wiring slice must obey.
- **Alternatives:** Full guest wiring now — PIT `outb` + PIC remap + IDT extend + `global_asm!` + `sti` + QEMU re-run (rejected: doubles single-cycle QEMU risk; refusal/drain/stub contracts must exist first); `AtomicU64` swap now (rejected: same as slice7b — needs the wiring slice's ownership design, not a lone type swap); per-tick `dispatch` inside drain on each expiry (rejected: quantum floors at 0 so a backlog would multi-dispatch; drain reports once, caller dispatches once).
- **Expected:** `cargo test --workspace` 203/203 (197 + 6); fmt clean; host + UEFI-target clippy `-D warnings` clean; no QEMU change (guest still `cli`).
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 203/203 (21 boot-info + 35 boot-uefi + 137 kernel-lib incl. 9 timer + drain tests + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean (after one `unused_macros` allow on the not-yet-consumed stub macro).
- **Final:** REVIEW #444 residuals closed host-side (refusal enforced at the helper, drain gated, stub shape pinned). Owed next: guest PIT program + PIC remap + IDT vector-32 wiring + IRQ-side `tick` gate + `sti` with QEMU proof. #20 stays active; pending agent-b review.

## S07 slice 9 -- REVIEW #446 hardening: returning-IRQ stub frame + O(1) drain (2026-10-05)

- **Decision:** Close agent-b REVIEW #446 risks R1+R2 host-side in `crates/kernel/src/timer.rs` + `crates/kernel/src/sched.rs` without touching the guest (still `cli`, no `global_asm!`, no IDT extend): (a) R1 — `timer_isr_stub_asm!` drops the destructive `and rsp, -16` (lost entry RSP, `iretq` pops the wrong frame -> triple fault on first tick) and now pushes the 7 Win64-volatiles (`rax rcx rdx r8 r9 r10 r11`), `sub rsp, 32` only, `call timer_irq_rust`, `add rsp, 32`, master EOI (`mov al, 0x20` / `out 0x20, al`) BEFORE the pops (so `al` cannot clobber the caller's restored `rax`), then pops in reverse + `iretq`; (b) R2 — `drain_timer` replaces the `while i < drained` O(n) loop with O(1) bulk accounting (`ticks_left.saturating_sub(drained)`, `expired = ticks_left == 0`), so a `cli`-held flood backlog (up to `u64::MAX`) owes at most one dispatch; empty drain still reports `(0, false)`. Two tests updated/extended: stub pin now asserts push/pop + `add rsp, 32` + no-`and` + EOI-before-`pop rax` + call/add/EOI order; new `drain_timer_flood_backlog_is_bulk_accounted` pins a 100k-tick flood to `(100000, true)` with quantum floored at 0.
- **Why:** REVIEW #446 withheld wiring-ready approval on exactly these load-bearing risks with the cheapest disconfirms (wire the stub behind IDT32+`sti` and watch the first IRQ0 fault; flood the backlog while `cli`-held). Both close host-side with no QEMU risk before the wiring slice; one-sided-gate wording (R3/R4 title items) stays with the wiring slice, which owns the IRQ-side `tick` gate + `sti` order.
- **Alternatives:** Full guest wiring now — PIT `outb` + PIC remap + IDT32 extend + `global_asm!` + `sti` + QEMU first-tick proof (rejected: doubles single-cycle QEMU risk; the frame + bulk contracts must be pinned first so the wiring slice is wiring-only); per-tick `tick_gated` loop capped at `ticks_left` (rejected: still O(quantum) work per IRQ for zero added signal — bulk gives the identical `(drained, expired)` bit-for-bit); `AtomicU64` swap now (rejected: same as slices 7b/8 — needs the wiring slice's ownership design, not a lone type swap).
- **Expected:** `cargo test --workspace` 204/204 (203 + 1); fmt clean; host + UEFI-target clippy `-D warnings` clean; no QEMU change (guest still `cli`).
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 204/204 (21 boot-info + 35 boot-uefi + 138 kernel-lib incl. updated stub pin + new flood test + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean (after one auto-format); `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean.
- **Final:** REVIEW #446 R1+R2 closed host-side (returning frame save/restore pinned, drain O(1) bulk). Owed next: guest PIT program + PIC remap + IDT vector-32 wiring + IRQ-side `tick` gate + `sti` with QEMU first-tick proof. #20 stays active; pending agent-b review.

## S07 slice 10 -- REVIEW #448 host close: 7-reg LIFO stub pin + two-sided tick gate (2026-10-05)

- **Decision:** Close REVIEW #448 residuals 1+2 host-side in `crates/kernel/src/timer.rs` without touching the guest (still `cli`, no `global_asm!`, no IDT extend): (a) stub pin now asserts all 7 pushes/pops (`rax rcx rdx r8 r9 r10 r11`) plus push-order `rax..r11`, pop-order `r11..rax` LIFO, push-block+shadow-before-call and EOI-before-pop-block ordering — a dropped `r10`/`r11` or swapped pair fails here; (b) `TimerTicks::tick_gated(&IrqGuard)` added as the IRQ-side gated increment (identical saturating `tick()`), mirroring `take_gated`/`Scheduler::tick_gated`, so both tick and drain have gated forms and raw `tick()`/`take()` are documented host-parity only. New `tick_gated_agrees_with_tick` test pins gated/ungated parity incl. saturation at `u64::MAX`.
- **Why:** REVIEW #448 verified R1/R2 direction but withheld wiring-ready on exactly these gaps: the stub test pinned 5 of 7 regs with no order assert (a silent `r10`/`r11` drop passed), and `take_gated` covered the drain side only while `tick()` stayed raw (tick-vs-take race persists until wiring gates the IRQ side). Both close with zero QEMU risk before the wiring slice.
- **Alternatives:** Full guest wiring now — PIT `outb` + PIC remap + IDT32 + `timer_irq_rust` body + `sti` + QEMU first-tick proof (rejected: multi-risk guest slice; residuals 3-5 stay owed there, this slice keeps the host contracts tight first); `AtomicU64` swap now (rejected: same as slices 7b/8/9 — needs the wiring slice's ownership design, not a lone type swap).
- **Expected:** `cargo test --workspace` 205/205 (204 + 1); fmt clean; kernel clippy `-D warnings` clean; no QEMU change (guest still `cli`).
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 205/205 (21 boot-info + 35 boot-uefi + 139 kernel-lib incl. strengthened stub pin + new tick_gated parity + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --tests -- -D warnings` clean.
- **Final:** REVIEW #448 residuals 1+2 closed host-side (7-reg LIFO stub pin, two-sided `tick_gated`/`take_gated`). Owed next: `pit_program_bytes` routing + `timer_irq_rust` body (tick + single master EOI + return) + PIT program + PIC remap + IDT32 vector-32 gate + `sti` with QEMU first-tick proof. #20 stays active; pending agent-b review.

## S07 slice 11 -- consume timer stub + timer_irq_rust + TIMER_PROGRAM + PIC remap plan (2026-10-05)

- **Decision:** First linked-but-unwired guest slice in `crates/kernel/src/timer.rs` (still `cli`, IDT 32 vectors only, no QEMU change): (a) `timer_isr_stub_asm!` consumed via `global_asm!` (linked, unreachable — closes the string-only residual); (b) guest-only `TIMER_TICKS: TimerTicks` + `timer_irq_rust` body (one saturating `tick`, never parks/ports — EOI stays in the stub); (c) `TIMER_PROGRAM = pit_program_bytes(TIMER_DIVISOR) = Some((0x9B, 0x2E))` as the first in-tree caller (closes REVIEW #444 zero-callers — guest PIT path must consume the const, never a raw divisor); (d) `pic_remap_writes()` pure 8-step ICW plan (ICW1 `0x11` x2, ICW2 offsets `0x20/0x28` == `PICM/S_OFFSET` == `VECTOR_TIMER` 32, ICW3 `0x04/0x02`, ICW4 `0x01` x2) with order-pinning test; (e) stub `andq` rejection added to the no-`and` pin (closes #450 nit).
- **Why:** REVIEW #452 APPROVED @79426e2 on exactly this shape: stub linked-unwired is safe (no vector-32 gate, still `cli`), TIMER_PROGRAM math verified (11931 = 0x2E9B), remap order verified, EOI-after-return with saved-rax preserves the Win64 frame. Carries R1-R4 to the wiring slice (raw `tick()` not gated, `static mut` unsync, stub literals, ICW literals/mask plan).
- **Alternatives:** Full guest wiring now (rejected: PIC remap outb + PIT program outb + IDT32 + `sti` + QEMU first-tick doubles single-cycle risk; link-but-unwired first).
- **Expected:** `cargo test --workspace` 207/207; fmt + kernel clippy clean; UEFI guest build clean; no QEMU change.
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 207/207 (21 boot-info + 35 boot-uefi + 141 kernel-lib + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean; UEFI build clean.
- **Final:** Slice11 linked-unwired; owed next unchanged (PIC remap outb + PIT program outb + IDT32 v32 + `sti` + QEMU first-tick). #20 stays active.

## S07 slice 12 -- wiring-prep host close: named ICWs + mask/PIT plans + gated IRQ body (2026-10-05)

- **Decision:** Close REVIEW #452 R1/R3/R4 host-side in `crates/kernel/src/timer.rs` without touching the guest (still `cli`, IDT 32 vectors, no port I/O, no QEMU change): (a) R4 — named ICW consts (`PIC_ICW1_INIT_ICW4 = 0x11`, `PIC_ICW4_8086 = 0x01`, `PIC_ICW3_MASTER_SLAVE_IRQ2 = 0x04`, `PIC_ICW3_SLAVE_IDENTITY = 0x02`) consumed by `pic_remap_writes()` (values pinned in test, plan cannot drift from docs); (b) R4 — new `pic_mask_writes()` pure plan (master `PICM_MASK_IRQ0_ONLY = 0xFE`, slave `PICS_MASK_ALL = 0xFF`) with unmask-IRQ0-only test; (c) new `pit_program_writes()` pure plan (mode `0x36` to `PIT_CMD`, then `TIMER_PROGRAM` lo/hi to `PIT_CH0`, `None` when unprogrammable) consuming `TIMER_PROGRAM` so a `0` can never reach the port; (d) R1 — `timer_irq_rust` body switches from raw `tick()` to `tick_gated(&IrqGuard::enter())` (interrupt gate clears IF, so the `cli` proof holds; raw `tick()` stays host-parity only, drain must use `take_gated`); (e) R3 — new `timer_stub_eoi_literals_match_pic_consts` test pins `PICM_CMD == PIC_EOI == 0x20` against the stub `mov al, 0x20` / `out 0x20, al` literals (drift fails here, not in QEMU). Three new tests.
- **Why:** REVIEW #452 approved slice11 but carried exactly these risks to the wiring slice; the emission plans + gated body are the smallest committable close-out that keeps every check green and makes the wiring slice wiring-only (outb emission + IDT32 + `sti` + QEMU first-tick). R2 (`static mut` unsync) stays with the wiring slice by design (needs `sti`/preempt ownership, not a lone type swap).
- **Alternatives:** Full guest wiring now — PIC remap outb + PIT program outb + IDT32 v32 + `sti` + QEMU first-tick (rejected: multi-risk guest slice; plans + gated body must be pinned first); `AtomicU64` swap now (rejected: same as slices 7b/8/9 — needs the wiring ownership design).
- **Expected:** `cargo test --workspace` 210/210 (207 + 3); fmt clean; kernel clippy + UEFI-target clippy clean; no QEMU change (guest still `cli`).
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 210/210 (21 boot-info + 35 boot-uefi + 144 kernel-lib incl. 3 new + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean.
- **Final:** R1/R3/R4 closed host-side (named ICWs, mask + PIT plans, gated IRQ body, EOI literal pin). Owed next: guest PIC remap outb + PIT program outb + IDT32 vector-32 gate + `sti` with QEMU first-tick proof. #20 stays active; pending agent-b review.

## S07 slice 13 -- IDT vector-32 timer gate wired (still cli, QEMU unchanged) (2026-10-05)

- **Decision:** Wire the timer gate in `crates/kernel/src/cpu.rs` without enabling interrupts: (a) `IDT_VECTORS = EXCEPTION_VECTORS + 1 = 33` (single-sourced, all builds so host tests pin it); (b) `load_idt` writes `entries[VECTOR_TIMER] = gate_for(baseos_isr_timer)` (forward-declared from `timer.rs` `global_asm!`) through the same present-DPL0-interrupt-gate path as `#DE`/`#PF`; (c) new host test pins `VECTOR_TIMER == 32 < IDT_VECTORS`, gate round-trip, `type_attr == 0x8E` with type nibble `0xE` (CPU clears IF on entry — the `IrqGuard::enter()` proof REVIEW #454 demanded be pinned in a wiring slice). Still `cli`, no PIC/PIT programming, no `sti`, so the gate never fires.
- **Why:** REVIEW #454 APPROVED slice12 host-close and owed exactly this first wiring half (IDT32 v32) plus the `0xE` pin; PIC-remap/PIT-program outb + `sti` + QEMU first-tick stay for the next slice (enabling IRQs is the risky half and must not ride with the table change).
- **Alternatives:** Full wiring now with PIC/PIT outb + `sti` + QEMU tick (rejected: multi-risk guest slice; table wiring must prove boot-stable first); leave IDT at 32 vectors until the `sti` slice (rejected: the `0xE`/coverage pin is the cheapest disconfirm of the entry-proof claim and is green now).
- **Expected:** `cargo test --workspace` 211/211 (210 + 1); fmt + host clippy + UEFI-target clippy clean; `boot-uefi.ps1` UEFI BOOT PASS unchanged (1649 B, PF+EXC lines).
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 211/211 (21 boot-info + 35 boot-uefi + 145 kernel-lib incl. 1 new + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -- -D warnings` + `-p baseos-boot-uefi --tests` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS (serial 1649 bytes, marker + BOOTINFO/MEMMAP/FB/GOP/EBS/KERNEL/FRAMEALLOC/PAGING/PERM/GUARD/PERMAUDITx2/RMBZ/THREADx5/SCHEDx6/CHAIN/PF/EXC all ok).
- **Final:** IDT32 v32 wired-but-masked; owed next: guest PIC-remap-outb + PIT-program-outb + `sti` with QEMU first-tick proof (R2 `static mut` drain wiring rides with it). #20 stays active; pending agent-b review.

## S07 slice 14 -- timer IRQ0 enable: PIC-remap/PIT-program outb + sti + QEMU first-tick (2026-10-05)

- **Decision:** Enable the timer in `bring_up_cpu_and_prove_exception` (the owed IRQ half): (a) `cpu::sti` (guest `sti` + host no-op stub, same shape as `cli`, host test pins `unsafe fn` + no-op); (b) `timer::remap_pic` + `timer::program_pit` (private guest `outb` consuming the `pic_remap_writes` / `pic_mask_writes` / `pit_program_writes` const plans, never raw literals); (c) `timer::pending_snapshot` (volatile `TIMER_TICKS` read) + `timer::drain_gated` (`take_gated` under `IrqGuard`, closing REVIEW #442 R2 in the guest, not just host parity); (d) bring-up sequence load_GDT/IDT → remap/program → `sti` → bounded `hlt` first-tick wait (200 spins, fail-loud `TICK ok=0`) → `cli` → gated drain → new `uart::write_tick_line` (`TICK ok=<0|1> ticks=<n>`) → live `#PF` probe (unchanged chain); (e) `boot-uefi.ps1` hard-requires same-line `TICK ok=1 ticks=[1-9]...` (missing/zero = FAIL); (f) `docs/unsafe-inventory.md` rows A8/A9/D11 + kernel shape-pin lists extended.
- **Why:** REVIEW #456 APPROVED slice13 wired-but-masked and owed exactly PIC-remap-outb + PIT-program-outb + `sti` + QEMU first-tick (not more host-only pins); the drain (R2 residue) rides with the `sti` slice by prior agreement.
- **Alternatives:** Split `sti` from the drain proof (rejected: R2 explicitly rides with `sti`; `cli` + gated drain is three lines); leave `sti` enabled through the `#PF` probe (rejected: re-mask keeps the probe deterministic and the guard proof true for the drain).
- **Expected:** `cargo test --workspace` 212/212 (211 + 1 `sti` shape pin); fmt + host clippy + UEFI-target clippy clean; `boot-uefi.ps1` UEFI BOOT PASS with `TICK ok=1` and PF+EXC intact.
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 212/212 (21 boot-info + 35 boot-uefi + 146 kernel-lib incl. 1 new + 4 + 4 serial-marker + 2 version); `cargo fmt` clean; `cargo clippy --all-targets -- -D warnings` (default members) + `-p baseos-boot-uefi --tests` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean (one `assertions_on_constants` fix: `VECTOR_TIMER < IDT_VECTORS` via `black_box`); `boot-uefi.ps1` UEFI BOOT PASS on two consecutive runs (serial 1667 bytes both, `TICK ok=1 ticks=1` then `PF ok=1 vector=14` + `EXC ok=1 vector=0`, all prior gates intact).
- **Final:** First HW tick proven end-to-end (guest PIC/PIT/`sti` + gated tick/drain + serial proof); owed next: scheduler quantum feed from drained ticks (S07 accounting). #20 stays active; pending agent-b review.

## S07 slice 15 -- scheduler quantum feed from drained ticks + fail-closed PIT None (2026-10-05)

- **Decision:** Feed the first-tick drain into a demo `Scheduler` quantum in `bring_up_cpu_and_prove_exception`: (a) new `timer::drain_into_scheduler` (static `TIMER_TICKS` into `Scheduler::drain_timer` under `IrqGuard`, bulk accounting, at most one dispatch owed; supersedes slice14 count-only `drain_gated`, removed); (b) bring-up arms a 1+2 demo table with one gated dispatch, drains via the scheduler path, dispatches ONCE on expiry (same `drain_timer`-then-`dispatch_gated` shape the future PIT handler uses; `TICK` line shape unchanged); (c) fail-closed `pit_program_writes() == None` skips program + `sti` and emits `TICK ok=0 ticks=0` before the `#PF` probe (REVIEW #458 residue 1); (d) `pending_snapshot` single-copy atomicity note (REVIEW #458 residue 2); (e) inventory rows A8/D11 updated; (f) one host parity test `guest_first_tick_armed_drain_shape` (1 tick -> `(1,false)`, `quantum_left == QUANTUM_TICKS - 1`).
- **Why:** REVIEW #458 APPROVED slice14 and owed exactly the scheduler quantum feed from drained ticks, plus residues 1-2 (early-fail before `sti`, volatile-read note).
- **Alternatives:** Keep count-only `drain_gated` alongside the scheduler path (rejected: two guest drain paths diverge; single path keeps the handler shape honest); re-tick loop until quantum expiry (rejected: slice-6 timer rule, dispatch-once only).
- **Expected:** `cargo test --workspace` 213/213 (212 + 1); fmt + host clippy + UEFI-target clippy clean; `boot-uefi.ps1` UEFI BOOT PASS with `TICK ok=1 ticks=[1-9]` and PF+EXC intact.
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 213/213 (21 boot-info + 35 boot-uefi + 147 kernel-lib incl. 1 new + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` + `-p baseos-boot-uefi --tests` + `-p baseos-boot-uefi --target x86_64-unknown-uefi` clean; `boot-uefi.ps1` UEFI BOOT PASS x2 (serial 1667 bytes both, `TICK ok=1 ticks=1` then `PF ok=1 vector=14` + `EXC ok=1 vector=0`, all prior gates intact).
- **Final:** Drained HW ticks now feed scheduler quantum accounting end-to-end (armed dispatch + gated drain + resched-once); owed next: PIT-handler shape / periodic accounting per roadmap. #20 stays active; pending agent-b review.

## S07 slice16 -- periodic PIT-handler accounting: per-thread CPU ticks via drain (2026-10-05)

- **Decision:** Host-only accounting in `crates/kernel/src/sched.rs` (guest untouched, `TICK` shape unchanged): (a) `Scheduler` gains `cpu: [u64; MAX_THREADS]` under the same append-only slot invariant as `runs` (zeroed on spawn); (b) `drain_timer` bulk-credits `drained` to the CURRENT slot (saturating O(1); unknown-current and empty drain credit nothing, no phantom time); (c) read-only `cpu_ticks_of(id)` / `total_cpu_ticks()`; (d) two host tests (credit + retarget across dispatch; empty/unknown/flood fail-closed edges).
- **Why:** REVIEW #460 APPROVED slice15 and owed periodic PIT-handler accounting per roadmap; dispatch counts (`runs`) measure quanta granted, not time consumed — crediting drained HW ticks to the running thread is the smallest periodic-CPU-time proof the future IRQ handler reuses verbatim (guest `drain_into_scheduler` inherits it with no serial change).
- **Alternatives:** Per-tick `account_tick_gated` handler callback now (rejected: IRQ handler owns no `Scheduler` yet — static scheduler + handler dispatch is a later slice; drain-credit covers the periodic rule host-side with one code path); expose `cpu` in the `TICK`/SCHED serial line now (rejected: guest serial change without a handler consumer; keep this slice host-only, e2e rides with the handler slice).
- **Expected:** `cargo test --workspace` 215/215 (213 + 2); fmt + host clippy + UEFI-target clippy clean; guest untouched so QEMU baseline (1667B `TICK ok=1 ticks=1` + PF+EXC) stands by construction.
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 215/215 (21 boot-info + 35 boot-uefi + 149 kernel-lib incl. 2 new + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy` (default members) + `-p baseos-boot-uefi --tests` + `-p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean (workspace `--all-targets` retains the known host-bin unwinding noise); guest untouched, no QEMU re-run this host-only slice.
- **Final:** Drained ticks now attribute CPU time to the running thread (dispatch counts + CPU ticks both per-slot); owed next: PIT-handler periodic drain/dispatch live in IRQ context + serial proof per roadmap. #20 stays active; pending agent-b review.

## S07 slice17 -- combined drain-then-dispatch helper + cpu slot-invariant (2026-10-05)

- **Decision:** Close REVIEW #462 risks 1-2 with one enforced shape: (a) `Scheduler::drain_then_dispatch_gated` (drains via `drain_timer` crediting the PRE-dispatch current, then exactly one `dispatch_gated` on expiry; returns `(drained, None)` when nothing owed, `Some(Ok/Err)` when one resched attempted); (b) guest `timer::poll_timer_dispatch` (static `TIMER_TICKS` via `drain_into_scheduler` + one gated dispatch) with `bring_up_cpu_and_prove_exception` moved to the single call site (`TICK` shape unchanged, dispatch result ignored); (c) two host tests (`drain_then_dispatch_credits_pre_dispatch_current` pins backlog-to-old-current + empty/failed-resched edges; `cpu_slot_invariant_across_sweep` pins `cpu[i]`-to-`table[i]` over three full quanta); (d) inventory D11 + doc notes updated.
- **Why:** REVIEW #462 APPROVED slice16 and owed the PIT-handler live drain/dispatch plus 4 gaps; the full IRQ-owned static scheduler is a later slice, but the misattribution window (ticks crediting the NEW current) and the `cpu`-misalignment hole are closable now by construction + regression, with the guest proving the combined shape end-to-end. Unknown-current still burns quantum while crediting nothing (fail-closed, pinned); saturating-overflow signal stays owed.
- **Alternatives:** Full IRQ-context static scheduler now (rejected: needs static `Scheduler` + handler reentrancy design + serial proof — multi-slice; the combined helper is the verbatim shape that handler will call); leave two-step drain/dispatch at the call site (rejected: every future caller could dispatch-without-draining and reopen risk 1).
- **Expected:** `cargo test --workspace` 217/217 (215 + 2); fmt + host clippy + UEFI-target clippy clean; `boot-uefi.ps1` UEFI BOOT PASS with `TICK ok=1` and PF+EXC intact.
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 217/217 (21 boot-info + 35 boot-uefi + 151 kernel-lib incl. 2 new + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` + `-p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS (serial 1667 bytes, `TICK ok=1` + `PF ok=1 vector=14` + `EXC ok=1 vector=0`, all prior gates intact).
- **Final:** Backlog attribution order enforced by construction (drain-before-pick in one helper, guest live); `cpu[]` alignment pinned alongside `runs[]`. Owed next: IRQ-owned static scheduler + live periodic handler dispatch + overflow-signal decision. #20 stays active; pending agent-b review.

## S07 slice18 -- poll delegation + retry-spin pin + overflow decision (2026-10-05)

- **Decision:** Close REVIEW #464 nits 1-2 plus the saturating-overflow call: (a) guest `timer::poll_timer_dispatch` delegates directly to `Scheduler::drain_then_dispatch_gated` over the static `TIMER_TICKS` (the `drain_into_scheduler` + manual dispatch body is removed, so the wrapper cannot drift from the helper); (b) retry-spin semantics documented on both sides (failed resched keeps `ticks_left == 0`: every non-empty drain retries exactly one dispatch, empty drains stay `(0, None)`); (c) overflow-signal decision recorded (saturating `cpu[]`/`runs[]`, no flag; S08 harness adds one only if it ever samples long enough); (d) inventory D11 + bring-up doc updated; (e) two host tests (`failed_resched_retries_once_per_nonempty_drain`, `matches_manual_drain_plus_dispatch` over 1/2/4/5/9 ticks).
- **Why:** REVIEW #464 APPROVED slice17 and left exactly these carryovers: delegation drift risk, undocumented retry-spin, plus the IRQ-owned static scheduler / live periodic handler / overflow-signal gaps. Delegation + docs + decision are closable now without the static-scheduler reentrancy design (still owed).
- **Alternatives:** Keep `drain_into_scheduler` as a second drain path (rejected: two guest drain paths reopens the drift nit); add an overflow flag now (rejected: no consumer, `u64::MAX` is ~5 Gyr at 100 Hz — S08 decides with a real sampler); full IRQ-owned static scheduler now (rejected: multi-slice reentrancy + serial proof, helper is still the verbatim shape it will call).
- **Expected:** `cargo test --workspace` 219/219 (217 + 2); fmt + host clippy + UEFI-target clippy clean; `boot-uefi.ps1` UEFI BOOT PASS with `TICK ok=1` and PF+EXC intact.
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 219/219 (21 boot-info + 35 boot-uefi + 153 kernel-lib incl. 2 new + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` + `-p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS (serial 1667 bytes, `TICK ok=1` + `PF ok=1 vector=14` + `EXC ok=1 vector=0`, all prior gates intact).
- **Final:** Guest drain path is single-sourced on the helper; retry-spin is specified, not accidental; overflow stays saturating by decision. Owed next: IRQ-owned static scheduler + live periodic handler dispatch. #20 stays active; pending agent-b review.

## S07 slice19 -- IRQ-owned static scheduler, task-context poll (2026-10-05)

- **Decision:** Move the demo table off bring-up's stack into the IRQ-owned guest static `sched::STATIC_SCHED` (mirrors the already-IRQ-owned `timer::TIMER_TICKS`): (a) pure `Scheduler::demo_armed_scheduler(&IrqGuard)` builds the armed 1+2 table (same bases/entries the old bring-up inlined, one gated dispatch to 2); (b) guest-only `sched_init_demo_static` resets the static from it, `sched_poll_static` drains into it via `drain_then_dispatch_gated`; (c) `timer::poll_timer_dispatch` drops the `&mut Scheduler` arg and delegates to `sched_poll_static` over `TIMER_TICKS` (single static+static path, `TICK` shape unchanged); (d) inventory D11 extended; (e) one host test pins the constructor (armed, current==2, quantum full, 1-tick credit + full-quantum resched to 1).
- **Why:** REVIEW #466 APPROVED slice18 and owed exactly the IRQ-owned static scheduler + live periodic handler; the handler move itself (dispatch inside `timer_irq_rust`, failure backoff per the retry-spin risk) is a reentrancy + serial-proof slice of its own, so this slice lands the ownership half with zero behavior change and proves it end-to-end (QEMU `TICK ok=1` through the static path).
- **Alternatives:** Move the dispatch into the IRQ handler in the same slice (rejected: IRQ-context pick + EOI ordering + per-tick retry backoff doubles single-cycle risk; ownership first, handler next); keep the stack-local scheduler alongside the static (rejected: two tables diverge, the handler would inherit the wrong one); host-test the `static mut` directly (rejected: shared static across parallel host tests flakes; pure constructor pinned host-side, static wiring proven by QEMU).
- **Expected:** `cargo test --workspace` 220/220 (219 + 1); fmt + host clippy + UEFI-target clippy clean; `boot-uefi.ps1` UEFI BOOT PASS with `TICK ok=1` and PF+EXC intact.
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 220/220 (21 boot-info + 35 boot-uefi + 154 kernel-lib incl. 1 new + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean (after one `cargo fmt` for a moved comment); `cargo clippy -p baseos-kernel --all-targets -- -D warnings` + `-p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS x2 (serial 1667 bytes both, `TICK ok=1 ticks=1` then `PF ok=1 vector=14` + `EXC ok=1 vector=0`, all prior gates intact; guest `bootx64.efi` 135168 bytes).
- **Final:** No stack-local scheduler remains; both drain ends are IRQ-owned statics polled from task context. Owed next: live periodic dispatch inside the IRQ handler + failure backoff/cap (REVIEW #466 risk) + unknown-current quantum policy. #20 stays active; pending agent-b review.

## S07 slice20 -- unknown-current quantum policy + backoff (2026-10-05)

- **Decision:** Host-only policy in `crates/kernel/src/sched.rs` (guest untouched, `TICK` shape unchanged): unknown-`current` drains consume the HW counter but preserve `ticks_left` and report `(drained, false)` — no credit, no quantum burn, no dispatch owed (backoff by construction). Known-`current` path unchanged (bulk credit + consume + expiry); retry-spin now scoped to known-`current` only (failed `NotReady` resched retries once per non-empty drain, unknown never retries so the future live IRQ handler costs one counter drain per idle tick batch, never a failed pick per batch). Three tests updated (edges, pre-dispatch tail, retry pin now uses Running+Done `NotReady` for retry + unknown `None` for backoff).
- **Why:** Slice19 owed exactly unknown-quantum policy + failure backoff before the live IRQ-handler move; an idle/unknown table retrying a failed pick per 100 Hz batch is pure waste with no running thread to charge, and burning another thread's quantum while idle misattributes budget. Preserving + not-owing is the smallest fail-closed rule the handler inherits verbatim.
- **Alternatives:** Keep burning quantum on unknown with per-tick `Some(Err)` retry (rejected: spams one failed pick per batch forever on an idle table; the live handler would inherit the spam); rearm to `QUANTUM_TICKS` on unknown (rejected: fabricates budget for no thread; preserve touches nothing); move dispatch into `timer_irq_rust` in the same slice (rejected: IRQ-context pick + EOI ordering + serial proof doubles single-cycle risk; policy first, handler next).
- **Expected:** `cargo test --workspace` 220/220; fmt + host clippy + UEFI-target clippy clean; guest untouched so QEMU baseline (1667B `TICK ok=1 ticks=1` + PF+EXC) stands by construction.
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 220/220 (21 boot-info + 35 boot-uefi + 154 kernel-lib + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean; guest untouched, no QEMU re-run this host-only slice.
- **Final:** Unknown-quantum policy decided + pinned (preserve + no-dispatch backoff); retry-spin scoped to known-current. Owed next: live periodic dispatch inside the IRQ handler (which inherits this backoff). #20 stays active; pending agent-b review.

## S07 slice21 -- armed-unknown preserve pin closes REVIEW #468 R2 (2026-10-05)

- **Decision:** Test-only close of the slice20 residual (host-only, guest untouched, `TICK` shape unchanged): extend `drain_timer_cpu_accounting_fail_closed_edges` to pin the armed-unknown path via test-child private-field access (`current = 99`, `ticks_left = QUANTUM_TICKS`), asserting a 2-tick drain returns `(2, false)`, preserves `QUANTUM_TICKS`, credits nothing (`total_cpu_ticks` stays 1 from the earlier known credit), and consumes the HW counter.
- **Why:** REVIEW #468 APPROVED slices 19+20 but left R2 (unknown+QUANTUM unpinned: edges test pinned unknown+0 trivial and known-armed contrast only; unknown+QUANTUM unreachable via safe API). The policy (preserve + no dispatch) is what the live IRQ handler inherits for idle batches, so leaving the non-zero case unpinned risks a future regression burning idle budget silently.
- **Alternatives:** Add a production setter for absent current (rejected: expands safe API solely for test reachability); leave R2 as honest-doc-only (rejected: test-child field access pins it with zero production change); move dispatch into `timer_irq_rust` in the same slice (rejected: IRQ-context reentrancy + serial proof doubles single-cycle risk; pin first, handler next).
- **Expected:** `cargo test --workspace` 220/220; fmt + host clippy clean; guest untouched so QEMU baseline (1667B `TICK ok=1 ticks=1` + PF+EXC) stands by construction.
- **Measured (2026-10-05, Win11):** `cargo test --workspace` 220/220 (21 boot-info + 35 boot-uefi + 154 kernel-lib + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean; guest untouched, no QEMU re-run this host-only slice.
- **Final:** R2 closed (armed-unknown preserve pinned, still backoff by construction). Owed next: live periodic dispatch inside the IRQ handler (inherits this backoff) + R1 known-persistent-Err cap in the S08 live handler. #20 stays active; pending agent-b review.

## S07 slice22 -- live-IRQ failure-backoff cap policy (R1 definition, host-only) (2026-10-05)

- **Decision:** Pure host-only R1 policy in crates/kernel/src/sched.rs (guest untouched, TICK shape unchanged): LIVE_MAX_CONSECUTIVE_FAILURES = 3 + live_backoff_suppresses (streak >= cap) + live_backoff_next (Some(Err) bumps saturating, else reset). Task-context drain_then_dispatch_gated keeps its slice-18 retry-spin untouched; suppression lives in the future live handler layer only (drain every batch, dispatch only while unsuppressed). One host test pins trip/reset/saturate.
- **Why:** REVIEW #470 APPROVED slice21 and left R1 (known-persistent-Err retry uncapped) plus live periodic dispatch owed on #20. An uncapped live handler would attempt one failed pick per 100 Hz batch forever on an unready table; the cap bounds that spam to 3 picks then drain-only until a success or idle None resets.
- **Alternatives:** Cap inside drain_then_dispatch_gated now (rejected: breaks the pinned retry-spin task-context contract and its host test; live suppression is a handler-layer concern); wire 	imer_irq_rust dispatch in the same slice (rejected: IRQ-context reentrancy + EOI ordering + serial proof doubles single-cycle risk; policy first, handler next).
- **Expected:** cargo test --workspace 221/221; fmt + host clippy clean; guest untouched so QEMU baseline (1667B `TICK ok=1 ticks=1` + PF+EXC) stands by construction.
- **Measured (2026-10-05, Win11):** cargo test --workspace 221/221 (21 boot-info + 35 boot-uefi + 155 kernel-lib incl. 1 new + 4 + 4 serial-marker + 2 version); cargo fmt --check clean; cargo clippy -p baseos-kernel --all-targets -- -D warnings clean; guest untouched, no QEMU re-run this host-only slice.
- **Final:** R1 policy defined + pinned (cap 3, drain-always dispatch-while-unsuppressed). Owed next: live periodic dispatch inside the IRQ handler consuming this policy (S08 wiring + QEMU). #20 stays active; pending agent-b review.

## S07 slice23 -- live backoff consumer (host-only, guest unwired) (2026-10-06)

- **Decision:** Pure host-only consumer of the slice-22 R1 cap in crates/kernel/src/sched.rs (guest untouched, TICK shape unchanged): new Scheduler::drain_then_dispatch_live_gated(ticks, gate, &mut consecutive_failures) owning the streak so callers cannot forget. Suppressed entry drains-only (counter consumed, CPU credited, quantum still consumed) with NO dispatch and streak RESET to 0; unsuppressed no-expiry (empty/partial/unknown-current backoff) resets; unsuppressed expiry dispatches once (Ok resets, Err bumps saturating via live_backoff_next). Task-context drain_then_dispatch_gated untouched (keeps retry-spin). Two host tests: 3-on-1-off throttle under persistent NotReady (streak 1-2-3, skip drains 2 with no dispatch + reset, probe fails again at 1) and reset on success/empty/partial/unknown.
- **Why:** Slice22 defined the cap but nothing consumed it; the S08 live IRQ handler needs a handler-layer step distinct from the task-context retry-spin. Reset-on-suppressed (not hold) is the recovery path: a held streak would stall forever once the table becomes Ready again (no dispatch means no Some(Ok) to reset with), so the bound is on CONSECUTIVE failed picks (3-on-1-off throttle) with automatic probe.
- **Alternatives:** Hold the streak on suppressed skip (rejected: permanent stall after recovery -- drain-only never produces the success needed to reset); cap inside drain_then_dispatch_gated (rejected: breaks the pinned task-context retry contract); wire timer_irq_rust dispatch in the same slice (rejected: IRQ-context reentrancy + EOI ordering + serial proof doubles single-cycle risk; consumer first, handler next).
- **Expected:** cargo test --workspace 223/223; fmt + host clippy + UEFI-target clippy clean; guest untouched so QEMU baseline (1667B TICK ok=1 ticks=1 + PF+EXC) stands by construction.
- **Measured (2026-10-06, Win11):** cargo test --workspace 223/223 (21 boot-info + 35 boot-uefi + 157 kernel-lib incl. 2 new + 4 + 4 serial-marker + 2 version); cargo fmt --check clean; cargo clippy -p baseos-kernel --all-targets -- -D warnings clean (one doc-list blank-line fix); cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings clean; guest untouched, no QEMU re-run this host-only slice.
- **Final:** Live backoff consumer defined + pinned (streak-owning helper, 3-on-1-off throttle, auto-recovery probe). Owed next: live periodic dispatch inside the IRQ handler calling this helper with an IRQ-owned streak static (S08 wiring + QEMU). #20 stays active; pending agent-b review.

## S07 slice24 -- live/task parity pin closes REVIEW #473 finding 2 (2026-10-06)

- **Decision:** Test-only close of REVIEW #473 finding 2 (host-only, guest untouched, TICK shape unchanged): new `live_unsuppressed_agrees_with_task_helper` pins that unsuppressed `drain_then_dispatch_live_gated` (streak 0/1/2) agrees exactly with task-context `drain_then_dispatch_gated` on success, persistent-NotReady, and unknown-current tables over 0/1/2/4/5/9-tick batches (same drain, same dispatch, same current/quantum/cpu, streak == live_backoff_next(in, failed); unknown always (drained, None) + streak 0). Suppressed divergence stays pinned by the throttle test by design.
- **Why:** REVIEW #473 APPROVED slices 22+23 but named the two helpers duplicating the drain-then-dispatch shape as drift risk; the cheapest disconfirm is a parity test so a future reorder/credit/dispatch-count edit in either helper goes red here before the S08 live wiring inherits one of them.
- **Alternatives:** Merge the two helpers into one streak-param now (rejected: changes the pinned task-context retry contract and its callers; parity first, unify at S08 wiring if the handler shape wants it); wire timer_irq_rust dispatch in the same slice (rejected: IRQ-context reentrancy + EOI ordering + serial proof doubles single-cycle risk; pin first, handler next).
- **Expected:** cargo test --workspace 224/224; fmt + host clippy + UEFI-target clippy clean; guest untouched so QEMU baseline (1667B TICK ok=1 ticks=1 + PF+EXC) stands by construction.
- **Measured (2026-10-06, Win11):** cargo test --workspace 224/224 (21 boot-info + 35 boot-uefi + 158 kernel-lib incl. 1 new + 4 + 4 serial-marker + 2 version); cargo fmt --check clean (after one cargo fmt); cargo clippy -p baseos-kernel --all-targets -- -D warnings clean; cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings clean; guest untouched, no QEMU re-run this host-only slice.
- **Final:** Live/task drift risk pinned (unsuppressed parity + streak rule). Owed next: live periodic dispatch inside the IRQ handler calling the live helper with an IRQ-owned streak static (S08 wiring + QEMU TICK proof) + REVIEW #473 findings 1 (throttle strength) and 3 (suppressed-expiry coupling). #20 stays active; pending agent-b review.

## S08 slice25 -- task-context live poll with IRQ-owned streak static (2026-10-06)

- **Decision:** First S08 wiring slice in `crates/kernel/src/sched.rs` + `timer.rs` (TICK shape unchanged): new guest-only `STATIC_STREAK` (IRQ-owned `u64`, reset to 0 in `sched_init_demo_static`) + `sched_poll_live_static` (live helper with the static streak) with `poll_timer_dispatch` now polling the LIVE path (still task-context `cli` + `IrqGuard`; IRQ-handler move rides next slice with EOI/reentrancy + QEMU TICK proof). `sched_poll_static` retained (`#[allow(dead_code)]`) as the task retry-spin reference. One host test pins the guest shape (demo-armed + 1 tick = `(1, None)`, streak stays 0, credit on current 2, quantum `QUANTUM_TICKS - 1`).
- **Why:** REVIEW #475 owed live-helper wiring (dead in guest) + IRQ-owned streak static before the IRQ move; slice-24 parity makes this behavior-neutral on the demo table (unsuppressed live == task, demo never fails so streak stays 0), so the existing QEMU `TICK ok=1` proof carries over while the streak ownership lands.
- **Alternatives:** Call the live helper from `timer_irq_rust` in the same slice (rejected: IRQ-context drain breaks the first-tick `pending_snapshot` wait + TICK accounting -- needs a cumulative-drained static and EOI/reentrancy proof, doubles single-cycle risk; streak ownership + task poll first, IRQ move next); merge task/live helpers now (rejected: breaks the pinned retry-spin contract; parity pin stays the drift guard); host-only without QEMU (rejected: guest changed, one QEMU run is the cheap disconfirm).
- **Expected:** cargo test --workspace 225/225; fmt + host clippy + UEFI-target clippy clean; `boot-uefi.ps1` PASS 1667B `TICK ok=1` + PF/EXC (unchanged).
- **Measured (2026-10-06, Win11):** cargo test --workspace 225/225 (21 boot-info + 35 boot-uefi + 159 kernel-lib incl. 1 new + 4 + 4 serial-marker + 2 version); cargo fmt --check clean; cargo clippy --all-targets -D warnings clean (one `dead_code` allow for retained `sched_poll_static`); cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings clean; `boot-uefi.ps1` UEFI BOOT PASS, serial 1667 bytes, marker FOUND, `TICK ok=1` + PF/EXC (guest `bootx64.efi` 135168 bytes).
- **Final:** Live helper is guest-live (task-context) with IRQ-owned streak shape; owed next is the IRQ move itself (`timer_irq_rust` calls the live path, EOI/reentrancy argument, QEMU multi-tick proof) + REVIEW #473 findings 1 (throttle strength) and 3 (suppressed-expiry coupling). #20 stays active; pending agent-b review.

## S08 slice26 -- multi-tick task-context live poll (QUANTUM_TICKS+1 wait) (2026-10-06)

- **Decision:** Smallest guest slice in `crates/kernel/src/lib.rs` (docs + wait threshold, no ps1 change): `bring_up_cpu_and_prove_exception` waits for `QUANTUM_TICKS + 1` IRQ ticks (one full quantum plus one) instead of the first tick, then drains via the unchanged live task-context path (`poll_timer_dispatch` -> `sched_poll_live_static`). Const-driven (`wait_target = QUANTUM_TICKS + 1`), so a quantum change moves the target; the ps1 `ticks=[1-9]` gate accepts any positive count.
- **Why:** REVIEW #477 owed the IRQ move + multi-tick QEMU; a per-tick handler drain would hold `pending` at 0 and the `pending_snapshot` wait would never fire, so the IRQ move needs a cumulative-drained static + EOI/reentrancy proof first (doubles single-cycle risk). The multi-tick wait is the cheap half: it proves periodic accumulation + bulk expiry (5-tick drain expires the armed quantum 4 and owes exactly one resched) with zero reentrancy change.
- **Alternatives:** IRQ-handler live call in the same slice (rejected: needs cumulative counter + EOI ordering + reentrancy argument, doubles QEMU risk; wait first, handler next); longer wait (e.g. 2 quanta) now (rejected: 5 already forces expiry + resched; longer only adds boot time for no new signal); ps1 `ticks=5` exact gate (rejected: shape gate stays positive-count so future quantum changes do not churn the script).
- **Expected:** `cargo test --workspace` 225/225 (unchanged count); fmt + host clippy + UEFI-target clippy clean; `boot-uefi.ps1` PASS x2 with `TICK ok=1 ticks=5` + PF/EXC.
- **Measured (2026-10-06, Win11):** `cargo test --workspace` 225/225 (21 boot-info + 35 boot-uefi + 159 kernel-lib + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS x2, serial 1667 bytes both (ticks=1 -> ticks=5 is one char, size coincidentally identical), `TICK ok=1 ticks=5` then `PF ok=1 vector=14` + `EXC ok=1 vector=0`, guest `bootx64.efi` 135168 bytes (unchanged size -- threshold compare only).
- **Final:** Periodic multi-tick drain proven end-to-end (bulk 5-tick expiry + resched through the live helper, streak stays 0 on the never-failing demo table). Owed next: IRQ move (`timer_irq_rust` live call + cumulative-drained static + EOI/reentrancy) + REVIEW #473 findings 1/3 + poll-before-init backoff + demo-ctor ignore. #20 stays active; pending agent-b review.

## S07 slice26 accounting correction -- S08 label premature + evidence rerun (2026-10-06)

- **Decision:** Accounting-only correction with independent evidence rerun, no behavior change: (a) relabel slice25/slice26 `S08` -> `S07` in `crates/kernel/src/lib.rs` docs (slice19-24 are S07, #20 S07-first-scheduler still active, so no S08 advance while #20 open per REVIEW #481); worklog headings for slice25/26 stay as committed history, this entry is the forward accounting fix; (b) re-measure the REVIEW #479/#481-owed gates on the current `@4986e42+relabel` tree.
- **Why:** REVIEW #481 sustained the #479 withhold: build/test SKIPPED that cycle, no QEMU log/size baseline verified, plus S07/S08 ordering (treat as S07, keep #20 active). Cheapest close is a doc-only relabel + fresh build/test/QEMU datum, leaving the IRQ move + #473 f1/3 + poll-backoff + demo-ctor for the next slice.
- **Alternatives:** Rewrite slice25/26 headings in place (rejected: rewrites committed history; forward correction preserves audit trail); bundle the IRQ-handler move now (rejected: needs cumulative-drained static + EOI/reentrancy, doubles single-cycle risk; evidence first, handler next).
- **Expected:** `cargo test --workspace` 225/225; fmt + host clippy + UEFI-target clippy clean; `boot-uefi.ps1` PASS x2 with `TICK ok=1 ticks=5` + `SCHED-TICK q=4 used=4 expired=1`.
- **Measured (2026-10-06, Win11, relabel tree):** default `cargo test` 190/190 (21 boot-info + 159 kernel-lib + 4 + 4 serial-marker + 2 version) + `cargo test -p baseos-boot-uefi` 35/35 = workspace 225/225; `cargo fmt --check` clean; `cargo clippy --all-targets -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS x2, serial 1667 bytes both, `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` (QUANTUM_TICKS=4 bulk drain expires, owes one resched) + `TICK ok=1 ticks=5` (= QUANTUM+1) then `PF ok=1 vector=14` + `EXC ok=1 vector=0`, guest `bootx64.efi` 135168 bytes both runs (delta 0 vs slice26 commit; size-stable codegen signal, hash not used per S04 determinism note).
- **Final:** REVIEW #479/#481 evidence owed delivered on the relabeled tree (build/test log + QEMU x2 ticks excerpt + 135168B delta + QUANTUM accounting). Still owed no further deferral: IRQ move + #473 f1/3 + poll-backoff + demo-ctor; trio wait sweep (QUANTUM/QUANTUM+1/QUANTUM+2) rides with the IRQ-move slice as the off-by-one disconfirm. #20 stays active; pending agent-b re-review.

## S07 slice27 -- quantum trio + suppressed coupling + empty-poll backoff (host-only) (2026-10-06)

- **Decision:** Test-only slice in `crates/kernel/src/sched.rs` (guest untouched, `TICK` shape unchanged, no QEMU re-run): new `quantum_boundary_trio_and_suppressed_coupling_with_empty_poll_backoff` pins (a) the QUANTUM off-by-one trio on the demo-armed table for BOTH helpers (Q-1 = `(Q-1, None)` quantum 1, Q and Q+1 = `(n, Some(Ok(1)))` rearmed to QUANTUM with the bulk credit on thread 2; live streak stays 0); (b) REVIEW #473 f3 suppressed-expiry coupling (suppressed drain still consumes via `drain_timer`: 2-tick skip credits CPU, quantum stays 0, no dispatch, streak reset; forced-streak-3 on an armed table drains 1 tick to QUANTUM-1 the same way); (c) poll-before-init backoff (empty `new(NULL_THREAD)` table = pre-`sched_init_demo_static` STATIC_SCHED shape: 3-tick live drain returns `(3, None)`, credits nothing, quantum 0, resets a dirty streak).
- **Why:** REVIEW #483 accepts slice26 but holds self-reported evidence and names no-deferral next: IRQ move + cumulative-drained + EOI/reentrancy + #473 f1/3 + poll-backoff + demo-ctor, with trio QUANTUM sweep as off-by-one disconfirm. A per-tick handler drain would hold `pending` at 0 and break the `pending_snapshot` wait, so the IRQ move needs the cumulative-drained static first; the trio + coupling + empty-poll pins are its host-side preconditions (expiry boundary, suppressed consumption rule, pre-init drain shape the handler inherits).
- **Alternatives:** IRQ-handler live call in the same slice (rejected: needs cumulative-drained static + EOI ordering + reentrancy argument + QEMU TICK proof, doubles single-cycle risk; pins first, handler next); exact `ticks=5` ps1 gate now (rejected: shape gate stays positive-count so quantum changes do not churn the script); demo-ctor failure-ignore pin now (rejected: valid-id constructor cannot fail via safe API; owed next with the IRQ move).
- **Expected:** `cargo test --workspace` 226/226; fmt clean; host + UEFI-target clippy clean; guest untouched so QEMU baseline (1667B `TICK ok=1 ticks=5` + PF/EXC, 135168B) stands by construction.
- **Measured (2026-10-06, Win11):** `cargo test --workspace` 226/226 (21 boot-info + 35 boot-uefi + 160 kernel-lib incl. 1 new + 4 + 4 serial-marker + 2 version); `cargo fmt --check` clean; `cargo clippy --all-targets -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings` clean; guest untouched (`cfg(test)` only), no QEMU re-run this host-only slice.
- **Final:** Trio off-by-one + f3 coupling + empty-poll backoff pinned. Still owed next: IRQ move (`timer_irq_rust` live call + cumulative-drained static + EOI/reentrancy + QEMU multi-tick proof) + REVIEW #473 f1 (throttle strength) + demo-ctor ignore. #20 stays active; pending agent-b review.

## S07 slice32 -- REVIEW #499 close: count method + QEMU per-tick-drain proof (2026-10-06)

- **Decision:** Doc-only close of REVIEW #499 on the unchanged `@f8eab6b` tree (no code change): (a) backfill the missing worklog entries for slices 28-31 (committed without worklog lines -- process gap admitted); (b) pin the test-count method; (c) deliver the owed QEMU TICK proof re-run under the per-tick IRQ drain.
- **Why:** REVIEW #499 approved slice31 with two findings. Finding 1 (metric): `cargo test` (default-members) shows 21+164+4+4+2=195 with no 35-source, while the slice30/31 claims cite 229/230. Root cause is the counting method, not inflated tests: this cycle re-measures `cargo test --workspace` 230/230 = 21 boot-info + 35 boot-uefi (`bootx64` main.rs, excluded from default-members per `Cargo.toml`) + 164 kernel-lib + 4 + 4 serial-marker + 2 version. Finding 2 (owed): the per-tick IRQ drain (slice30) changed the guest (`timer_irq_rust` drains every tick; TICK line reports the gated cumulative total), so the multi-tick proof had to be re-run on the drained tree, not carried over by construction.
- **Alternatives:** Leave slices 28-31 undocumented and log only the QEMU datum (rejected: Definition of Done requires documented; the backfill is cheap and preserves the audit trail); re-count with plain `cargo test` and relabel 230->195 (rejected: `--workspace` is the documented gate since slice20 and covers the boot-uefi contract; instead pin the method explicitly so the reviewer's reproduction matches).
- **Expected:** `cargo test --workspace` 230/230; fmt + host kernel clippy + UEFI-target clippy clean; `boot-uefi.ps1` UEFI BOOT PASS with `TICK ok=1 ticks=5` + `SCHED-TICK q=4 used=4 expired=1` under the per-tick drain.
- **Measured (2026-10-06, Win11, `@f8eab6b` clean + this doc entry):** `cargo test --workspace` 230/230 (21 + 35 + 164 + 4 + 4 + 2; plain `cargo test` 195 = workspace minus the 35 boot-uefi); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; `boot-uefi.ps1` UEFI BOOT PASS, serial 1667 bytes, marker FOUND, `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` + `TICK ok=1 ticks=5` then `PF ok=1 vector=14` + `EXC ok=1 vector=0`, guest `bootx64.efi` 136704 bytes (delta +1536 vs the 135168B slice26/27 baseline -- per-tick drain + gated cumulative TICK code; size signal only, hash not used per S04 note).
- **Final:** REVIEW #499 findings closed (method pinned: always cite `cargo test --workspace` with the 6-way split; QEMU per-tick-drain proof delivered with ticks excerpt + size delta). Backfill: slice28 cumulative-tick IRQ precondition, slice29 seqlock cumulative snapshot + volatile-consistent TOTAL_DRAINED, slice30 per-tick IRQ drain + cumulative TICK line + storm bound, slice31 REVIEW #497 doc drifts + pre-init NULL drain pin. #20 stays active (scheduler quantum/fairness remainder); pending agent-b re-review.

## S07 slice33 -- storm-wait `>=` pin (REVIEW #501 risk 2)

- **Decision:** Close REVIEW #501 risk 2 (storm fallback non-monotonic) as one small slice: new pure `timer::cumulative_wait_ready(total, target) -> bool` (`total >= target`) with docs pinning the `>=`-never-`==` rule, one host test `cumulative_wait_ready_uses_ge_never_eq` (fallback-then-step-back shape: fallback 2 ready for target 2, stepped-back clean 1 not ready, plus boundary/saturation pins), and route the guest `sti` wait loop in `bring_up_cpu_and_prove_exception` through the helper (was inline `>=`).
- **Why:** The fallback over-counts at most one in-flight batch and the next clean read steps backward; an exact-equality waiter would miss its wakeup after the step-back and burn the 200-`hlt` budget (loud `TICK ok=0`, self-inflicted). The inline `>=` was correct but unpinned -- a future edit to `==` stayed green host-side and died in QEMU. The helper makes that regression a host test failure.
- **Alternatives:** Doc-only warning in the fallback comment (rejected: reviewer named the `>=` rule as the pin, docs alone already existed); `==`-with-retry waiter (rejected: adds spinning for no gain, `>=` wakes at most one `hlt` early, never late).
- **Expected:** `cargo test --workspace` 231 (230 + 1); fmt + host kernel clippy + UEFI-target clippy clean; `boot-uefi.ps1` UEFI BOOT PASS with `TICK ok=1 ticks=5` unchanged.
- **Measured (2026-10-06, Win11):** `cargo test --workspace` 231/231 (21 + 35 + 165 + 4 + 4 + 2; plain `cargo test` 196 = workspace minus the 35 boot-uefi); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1667 bytes, `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` + `TICK ok=1 ticks=5` then `PF`/`EXC`, guest `bootx64.efi` 136704 bytes (identical to slice32 baseline -- helper inlined, zero codegen delta).
- **Final:** REVIEW #501 risk 2 closed (all cumulative waiters now route through the pinned `>=` helper). Still open from #501: risk 1 (dispatch-in-IRQ result ignored -- safe only while demo table is metadata-only; pin the gate before any real stack-switch) + risk 3 (size watch). #20 stays active.

## S07 slice34 -- IRQ-dispatch gate pin (REVIEW #501 risk 1)

- **Decision:** Pin REVIEW #501 risk 1 as one small slice: new pure `sched::IRQ_STACK_SWITCH_PERMITTED: bool = false` with docs pinning the deferred-never-switch rule (IRQ `Ok(next)` is a deferred-resched request, never an immediate RSP switch), one host test `irq_dispatch_result_is_deferred_never_stack_switch` (gate closed via `black_box`, demo-table `dispatch_gated` mutates only state/current/quantum), and a `timer_irq_rust` doc reference to the gate.
- **Why:** `timer_irq_rust` dispatches in IRQ context and ignores the result (`let _ =`); safe only while dispatch is metadata-only flips with no stack switch. An unpinned future real-switch slice could switch stacks inside the IRQ and corrupt the interrupted frame. The const makes that violation a host test failure.
- **Alternatives:** Doc-only warning in `timer_irq_rust` (rejected: docs alone already implied the rule; the const is the reviewer-owed pin); record-then-defer queue now (rejected: larger slice -- no real switch exists yet, deferral queue rides with it).
- **Expected:** `cargo test --workspace` 232 (231 + 1); fmt + host kernel clippy + UEFI-target clippy clean; guest `bootx64.efi` size unchanged (unused const, docs only).
- **Measured (2026-10-06, Win11):** `cargo test --workspace` 232/232 (21 + 35 + 166 + 4 + 4 + 2); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` ok; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` ok; `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` ok, `bootx64.efi` 136704 bytes (identical to slice32/33 baseline -- zero codegen delta); guest QEMU baseline stands by construction (host-only + docs, no behavior change).
- **Final:** REVIEW #501 risk 1 pinned (gate closed, IRQ ignore documented as deferral). Still open: risk 1 full close rides with the real-switch slice (flip gate + record-then-defer) + risk 3 (size watch 136704B). #20 stays active.

## S07 slice35 -- IRQ-gate enforcement assert (REVIEW #505)

- **Decision:** Enforce REVIEW #505 advisory-gate finding as one small slice: new pure `sched::irq_dispatch_must_defer() -> bool` (`!IRQ_STACK_SWITCH_PERMITTED`), one host test `irq_gate_enforcement_helper_defers_while_closed` (helper true + gate false via `black_box`), and a `debug_assert!(irq_dispatch_must_defer())` code reference at the top of guest `timer_irq_rust` IRQ body plus a slice35 doc note (gate now referenced in code, not docs-only).
- **Why:** Slice34 pinned the gate but `timer_irq_rust` never referenced the const, so a future real-switch slice could silently switch stacks in IRQ context while the test stayed green. The IRQ-site assert makes that violation fail the IRQ debug build at the offending site.
- **Alternatives:** Full record-then-defer + flip now (rejected: larger slice, no real switch exists yet -- rides with the real-switch slice per TASK #506, after #20 accounting); doc-only cross-reference (rejected: already the slice34 shape the reviewer flagged as advisory).
- **Expected:** `cargo test --workspace` 233 (232 + 1); fmt + host kernel clippy + UEFI-target clippy clean; `bootx64.efi` grows slightly (debug assert kept in dev profile, zero in release); `boot-uefi.ps1` UEFI BOOT PASS with `TICK ok=1 ticks=5` unchanged.
- **Measured (2026-10-06, Win11):** `cargo test --workspace` 233/233 (21 + 35 + 167 + 4 + 4 + 2); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` ok; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` ok; `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` ok, `bootx64.efi` 137216 bytes (+512 vs 136704 slice32-34 baseline -- debug-assert code + message in dev profile, release zero); `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 1667 bytes, `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` + `TICK ok=1 ticks=5` then `PF`/`EXC`.
- **Final:** REVIEW #505 advisory finding closed (gate enforced at IRQ site in code); REVIEW #501 risk 1 still pinned-not-fully-closed until the real-switch slice (flip gate + record-then-defer per TASK #506/#21) + risk 3 size watch updated to 137216B. #20 stays active (scheduler remainder: priorities/affinity/measurements).

## S07 slice36 -- IRQ-gate release fail-closed guard (REVIEW #510)

- **Decision:** Harden REVIEW #510 (debug-only gap) as one small slice without flipping the gate or landing record-then-defer: explicit `if !irq_dispatch_must_defer() { tick; return; }` fail-closed guard at the top of guest `timer_irq_rust` (active in release, unlike `debug_assert!`), plus one host test `irq_gate_fail_closed_release_refuses_switch` (explicit `if` branches over `black_box` gate/helper, panics on wrong branch so `needless_bool` stays clean).
- **Why:** Slice35 enforced the gate with `debug_assert!` only -- zero release enforcement. A future real-switch slice that flips the const without converting the IRQ path would stay green in release. The `if` guard fails closed in all profiles (tick drained, dispatch deferred); gate still closed so it never fires today and optimizes out (zero codegen delta).
- **Alternatives:** Flip + record-then-defer now (rejected: larger slice, violates REVIEW #510 rule 1 -- flip and conversion land together in the real-switch slice #21, never separately); release-only `#[cfg(not(debug_assertions))]` test (rejected: the `if`-branch test proves the shape in every profile with one test).
- **Expected:** `cargo test --workspace` 234 (233 + 1); fmt + host kernel clippy + UEFI-target clippy clean; `bootx64.efi` unchanged (constant-folded dead branch); QEMU baseline stands by construction (no behavior change while gate closed).
- **Measured (2026-10-07, Win11):** `cargo test --workspace` 234/234 (21 + 35 + 168 + 4 + 4 + 2); `cargo test -p baseos-kernel --lib --release irq_gate` 3/3 (helper + fail-closed + cpu gate); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` ok; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` ok; `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` ok, `bootx64.efi` 137216 bytes (identical to slice35 baseline -- fail-closed branch folds out while gate closed).
- **Final:** REVIEW #510 rule 2 (release-active guard) landed; rule 1 (atomic flip + record-then-defer) still rides with the real-switch slice #21, rule 3 (release UEFI size + QEMU TICK re-verify) still owed before #20 close. #20 stays active.

## S07 slice37 -- REVIEW #512 risk-1 close: release size + QEMU TICK re-verify

- **Decision:** Close REVIEW #512 risk 1 as a measurement-only slice (no code change): record dev + release `bootx64.efi` sizes and re-run `boot-uefi.ps1` + release `irq_gate` tests on the slice36 tree to prove the fail-closed guard is behavior-neutral while the gate stays closed.
- **Why:** Slice36's identical dev size (137216B) is expected for a constant-folded dead branch, not proof of release enforcement. The owed evidence is a release-profile size plus a live QEMU TICK run at QUANTUM+1 ticks on the same tree.
- **Alternatives:** Flip the gate or land record-then-defer now (rejected: violates REVIEW #510 rule 1 -- flip + conversion land atomically in the #21 real-switch slice, never separately); skip QEMU as behavior-neutral by construction (rejected: #20 close requires a live TICK re-verify per REVIEW #510 rule 3).
- **Expected:** `cargo test --workspace` 234; dev `bootx64.efi` 137216B unchanged; release size recorded; `boot-uefi.ps1` UEFI BOOT PASS with `SCHED-TICK ok=1 q=4` + `TICK ok=1 ticks=5` (QUANTUM_TICKS+1); release `irq_gate` 3/3.
- **Measured (2026-10-07, Win11):** `cargo test --workspace` 234/234 (21 + 35 + 168 + 4 + 4 + 2); `cargo fmt --check` clean; host kernel clippy + UEFI-target clippy ok; dev `bootx64.efi` 137216 bytes (identical to slice35-36); release `bootx64.efi` 81408 bytes (first release measurement on this tree); `boot-uefi.ps1` UEFI BOOT PASS, serial 1667 bytes, `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` + `TICK ok=1 ticks=5` then `PF`/`EXC`; `cargo test -p baseos-kernel --lib --release irq_gate` 3/3.
- **Final:** REVIEW #512 risk 1 closed (rule 3 evidence recorded); REVIEW #510 rule 1 (atomic flip + record-then-defer in ONE #21 slice) is the only gate item left before scheduler-remainder work. #20 stays active (priorities/affinity/measurements).

## S07 slice38 -- REVIEW #514 risks 1+2 close: QUANTUM linkage pin + repeat reproducibility

- **Decision:** Close REVIEW #514 risks 1+2 in one test+measure slice: (a) new host test `quantum_const_linkage_pins_script_literals` asserting `QUANTUM_TICKS == 4` and `QUANTUM_TICKS + 1 == 5` (`black_box` so `assertions_on_constants` stays clean), documenting that `tools/qemu/boot-uefi.ps1` hard-requires `SCHED-TICK ok=1 q=4` and the guest `wait_target = QUANTUM_TICKS + 1` yields `TICK ticks=5` -- a quantum change must update the const + ps1 literals + serial doc examples together; (b) repeat-measure release size + QEMU TICK on the same tree (risk 1 reproducibility, second run of each single-run claim from slice37).
- **Why:** All in-tree countdowns are const-driven, so a quantum change stayed green host-side while `q=4`/`ticks=5` script/guest expectations diverged silently (risk 2 cheapest disconfirm: assert the const); release 81408B + TICK=5 were both single-run (risk 1 needs a repeat before #20 close per DoD reproducibility).
- **Alternatives:** Pin only the const without QEMU (rejected: leaves the single-run TICK claim open); re-run QEMU without the pin (rejected: leaves the silent-drift hole open); flip the gate / land record-then-defer now (rejected: REVIEW #510 rule 1 -- flip + conversion land atomically in ONE #21 slice, never separately; #21 stays proposed).
- **Expected:** `cargo test --workspace` 235 (234 + 1); fmt + host kernel + UEFI-target clippy clean; dev 137216B unchanged; release 81408B repeated; `boot-uefi.ps1` PASS 1667B `SCHED-TICK q=4` + `TICK ticks=5`.
- **Measured (2026-10-07, Win11):** `cargo test --workspace` 235/235 (21 + 35 + 169 incl. new linkage pin + 4 + 4 + 2); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` + `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; dev `bootx64.efi` 137216 bytes (3rd identical run); release `bootx64.efi` 81408 bytes (2nd identical run); `boot-uefi.ps1` UEFI BOOT PASS, serial 1667 bytes, `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` + `TICK ok=1 ticks=5` then `PF`/`EXC`.
- **Final:** REVIEW #514 risks 1+2 closed (repeat reproducibility + const linkage pinned); risk 3 agreed (flip + record-then-defer land atomically in ONE #21 slice, #21 stays proposed until #20 remainder lands). #20 stays active (priorities/affinity/measurements).

## S07 slice39 -- REVIEW #516 risk 1 close: true QUANTUM linkage pin (include_str disconfirm)

- **Decision:** Upgrade the slice38 const-only linkage pin to a true two-sided pin: `quantum_const_linkage_pins_script_literals` now `include_str!`s `tools/qemu/boot-uefi.ps1` and asserts the const-derived literal `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` is present verbatim, plus a `match q { 4 => {}, _ => panic!(update checklist) }` const-value pin and `q + 1 == 5` wait-target pin (`black_box` keeps `assertions_on_constants` clean; the script-content assert is never on a constant).
- **Why:** REVIEW #516 risk 1 showed slice38 stayed green on ps1-only drift (asserted 4==4, never read the script). Cheapest disconfirm per the review: temporarily change the ps1 `q` literal to 5 and show the host test goes red. Slice38 could not pass that disconfirm; this slice does.
- **Alternatives:** Parse guest `wait_target` source too via a second `include_str!` (rejected: guest side is already const-driven `QUANTUM_TICKS + 1` in `lib.rs` with no literal to drift -- the silent side was the ps1 literal; one true file pin closes the hole with minimal surface); hard-require `TICK ticks=5` in ps1 instead of the `ticks=[1-9]` shape (rejected: shape gate intentionally accepts any positive count since storm fallback is non-monotonic -- exact-count pinning belongs host-side, e2e keeps proving 5 live).
- **Expected:** `cargo test --workspace` 235 (same count, pin upgraded in place); fmt + host + UEFI-target clippy clean; dev 137216B unchanged; release 81408B unchanged; `boot-uefi.ps1` PASS 1667B `SCHED-TICK q=4` + `TICK ticks=5`; ps1 `q=4`->`q=5` mutation makes the pin FAIL, revert restores green.
- **Measured (2026-10-07, Win11):** `cargo test --workspace` 235/235 (21 + 35 + 169 incl. upgraded pin + 4 + 4 + 2); `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean (host) + `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; disconfirm: ps1 `q=4 used=4` temporarily edited to `q=5 used=5` -> `quantum_const_linkage_pins_script_literals` FAILED (`script pins quantum literals`), file reverted to `q=4` -> test green again (note: revert within the same mtime granularity needed a `LastWriteTime` touch on `sched.rs` to force cargo to re-read the `include_str!` -- recorded, not a code issue); dev `bootx64.efi` 137216 bytes; release `bootx64.efi` 81408 bytes; `boot-uefi.ps1` UEFI BOOT PASS, serial 1667 bytes, `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` + `TICK ok=1 ticks=5` then `PF`/`EXC`.
- **Final:** REVIEW #516 risk 1 closed (true pin, disconfirm recorded); risk 2 acknowledged (repeat runs here are same-tree/same-host back-to-back -- both runs recorded, not best-only); risk 3 agreed (#20 exit still owed: rule 1 + priorities/affinity/measurements per S07; #21 stays proposed, flip+record-then-defer atomic in ONE slice). #20 stays active.

## S07 slice40 -- scheduler priority data-only (first #20 remainder slice)

- **Decision:** Land priority as pure data, no policy change: `Thread` gains a `priority: u8` field (default `0` at `new()`, any value settable via `set_priority`, read via `priority()`), `Scheduler::priority_of(id)` read-only query (`None` on unknown), and one host test `priority_is_data_only_round_robin_preserved` (default/set round-trip, spawn carries 1/255/128, unknown is `None`, mixed-priority dispatch stays id round-robin 2->3->1 with spread 0). Smuggled-literal tests in `thread.rs` gain `priority: 0`. Pick/dispatch/exit paths untouched.
- **Why:** #20 remainder per REVIEW #518 is priorities/affinity/measurements + rule 1; smallest end-to-end slice is the data carrier first (policy reordering lands later and must flip the preserved-order test, never stay green while the guest diverges).
- **Alternatives:** Priority-aware pick now (rejected: larger slice mixing data + policy; DoD prefers one committable segment); affinity first (rejected: same size, priority is the roadmap-listed first item and the documented per-thread-budget hook).
- **Expected:** `cargo test --workspace` 236 (235 + 1); fmt + host kernel + UEFI-target clippy clean; dev `bootx64.efi` unchanged (guest never sets priority, init folds out); `boot-uefi.ps1` UEFI BOOT PASS unchanged.
- **Measured (2026-10-07, Win11):** `cargo test --workspace` 236/236 (21 + 35 + 170 incl. new data-only pin + 4 + 4 + 2); `cargo fmt --check` clean; host kernel clippy + UEFI-target clippy clean; dev `bootx64.efi` 137216 bytes (identical to slice35-39 baseline); `boot-uefi.ps1` UEFI BOOT PASS, serial 1667 bytes, `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` + `TICK ok=1 ticks=5` then `PF`/`EXC`.
- **Final:** Priority data carrier landed, round-robin preserved by pin. #20 stays active (priority policy, affinity, measurements + rule 1 per REVIEW #510/#518; #21 stays proposed).

## S07 slice41 -- priority-aware pick opt-in (RR default preserved)

- **Decision:** Add opt-in policy alongside RR, no default change: `Scheduler::pick_next_priority` (highest-priority Ready excluding current, RR slice-order tie-break, same Unknown/InvalidTable/NoSwitch/NotReady refusals as `switch_stub`, O(n) documented), `dispatch_priority` / `dispatch_priority_gated` (same Ready→Running moves + quantum rearm + trace as `dispatch`, policy target), plus one host test `priority_aware_pick_jumps_queue_rr_preserved` (order [lo1, mid128, hi255] current=1: RR gives 2, policy gives 3; second policy leg gives 2; equal-prio tie-break RR; empty/unknown/solo refusal parity; one policy dispatch holds spread 1 vs RR full-round spread 0). Guest probe stays on `dispatch_gated` (RR); policy wiring rides with a later slice.
- **Why:** REVIEW #519 cheapest disconfirm for the next slice is a priority-aware pick showing dispatch order flips plus the fairness_spread delta. Additive opt-in keeps every RR gate green (no blast radius across the 30-dispatch FAIR sweep) while pinning the exact flip the default-change slice must produce.
- **Alternatives:** Flip `dispatch` to strict priority now (rejected: breaks the FAIR sweep + TRACE order pins in the same cycle, large blast radius for one segment); priority + affinity + measurements in one slice (rejected: scope growth past one committable segment; affinity/measurements ride next).
- **Expected:** `cargo test --workspace` 237 (236 + 1); fmt + host clippy clean; guest untouched (no size/QEMU change; prior 137216B / 1667B TICK ticks=5 stands).
- **Measured (2026-10-07, Win11):** `cargo test --workspace` 237/237 (21 + 35 + 171 incl. new policy pin + 4 + 4 + 2); `cargo fmt --check` clean (one auto-format of the pick condition); `cargo clippy --all-targets -- -D warnings` clean. Guest untouched (pure host policy + test; `dispatch`/`dispatch_gated` paths byte-identical behavior).
- **Final:** Opt-in priority policy landed with the RR-vs-policy flip pinned (2 RR vs 3 policy, spread 0 vs 1). #20 stays active (default-policy flip or guest policy wiring, affinity, measurements + rule 1; #21 stays proposed).

### S07 slice43 - priority starvation pinned
- **Decision:** Pin known starvation of opt-in priority dispatch with a host test (2 hi + 1 lo, 300 dispatches: lo runs==0, spread >= 150; RR spread <= 1; single hi does not starve lo).
- **Why:** REVIEW #523 risk 1 - no aging/inversion bound exists; evidence before a policy-flip or aging slice.
- **Alternatives:** Add aging now (rejected: scope growth, rides next slice).
- **Measured:** cargo test --workspace 239/239; fmt + clippy clean.
- **Final:** Starvation is documented behavior; aging/flip decision owed in #20.

### S07 slice44 - priority contract decided (opt-in, no aging) + single-scan commit
- **Decision:** (1) `dispatch_priority` contract is "highest-priority Ready among NON-current" (yield semantics; current never re-wins), tie-break anchored on current. (2) FINAL starvation decision: no aging; priority path stays an explicit opt-in, RR (`dispatch`/`dispatch_gated`) is the default and the only path the guest uses. `dispatch_priority` is documented NOT starvation-safe. (3) `commit_next` folded from 2 scans to 1 index scan + O(1) moves by index, refusal still before any mutation.
- **Why:** REVIEW #525 risks 1+2. New pin `priority_single_hi_is_capped_at_half_share_yield_contract` (1 hi + 2 lo, 300 dispatches) shows hi gets 150 (+-1), not ~300, and the SECOND lo starves (runs==0) because the tie-break is anchored on current = hi. Combined with slice43 (2 hi starve 1 lo), the policy has two starvation modes and no hi advantage over RR at n=2, so it can't be the default.
- **Alternatives:** Aging with bound (e.g. +1 prio per skipped dispatch, reset on run) - rejected for now: needs per-thread effective-prio state + its own measurement, scope growth; revisit only if a real workload needs priorities. Strict priority with current re-winning - rejected: needs a yield-vs-preempt split (yield must exclude current, preempt may not), and that is #21 territory. Tie-break cursor (last-picked-per-prio) - deferred, fixes lo-pair starvation cheaply, candidate if priority is ever promoted.
- **Expected:** 240 tests; kernel fmt + clippy clean; guest untouched.
- **Measured (2026-10-08, Win11):** `cargo test --workspace` 240/240 (21 + 35 + 174 + 4 + 4 + 2); `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean (workspace `--all-targets` bootx64 host-test "unwinding panics" error is pre-existing, same on the stashed tree). Guest untouched.
- **Final:** Opt-in priority with RR default, no aging, is the final S07 decision. Risk 2 closed. Risk 3 (NotReady masking InvalidTable in commit) is still owed. #20 remainder: affinity + O(n) measurement.

### S07 slice45 - #20 risk 3 documented, pin flagged known-bad, affinity deferred
- **Decision:** (1) Risk 3: `pick_next_priority` validates before any NotReady, so InvalidTable is never masked on the pick path; `commit_next` (private) returns NotReady for bad target without re-validating, unreachable via picks, documented in rustdoc. (2) Test `priority_single_hi_is_capped_at_half_share_yield_contract` annotated: 2nd-lo runs==0 is a known-bad pin, a per-priority cursor should flip it. (3) Affinity DEFERRED: single-CPU kernel (S12 is Block Device Layer, not SMP), so a mask has nothing to select among; revisit when AP bring-up is scoped.
- **Why:** REVIEW #527 items 1-2 + pin-readability concern.
- **Alternatives:** Add affinity mask field now (rejected: no consumer, untestable semantics).
- **Measured:** kernel 174/174 host tests; fmt + `clippy -p baseos-kernel` clean; doc/comment-only change.
- **Final:** Owed on #20: pick measurement (several runs + spread). #21 gate still required before S07 done.

### S07 slice46 - bounded host pick measurement and roadmap-label correction
- **Decision:** Add an opt-in ignored host release test for `pick_next_priority` versus RR `pick_next` at 4 and 16 threads (`MAX_THREADS=16`; 64 is invalid), seven timed batches of 10,000 picks each after 1,000 warmups per batch. Correct the slice45 S12 SMP label: S12 is Block Device Layer, and no SMP sprint is assigned. Correct the policy scan's complexity note: finding max priority is O(n), but `validate_threads` is O(n^2) on every pick; both end-to-end picks are O(n^2).
- **Why:** #20 needs a repeatable cost baseline and spread, not a best-only number or a misleading linear-cost claim. The single-CPU affinity deferral is valid, but tying it to S12 was not.
- **Alternatives:** Report 64 threads by bypassing the 16-entry cap (rejected: measures a path the scheduler rejects); claim host results as kernel IRQ timing (rejected: host clock and target differ); enable the benchmark in default unit tests (rejected: noisy timing on every test invocation).
- **Expected:** RR and policy pick valid targets without mutation at both sizes, at least five batches with min/median/max, default workspace tests unchanged, fmt + kernel clippy clean.
- **Measured (2026-10-06, Win11 host, `cargo test -p baseos-kernel --lib --release measure_host_priority_pick_against_round_robin -- --ignored --nocapture`):** `std::time::Instant` around 10,000 picks per batch, `black_box` on scheduler reference and result; seven batches per size/policy, integer ns/op (elapsed/batch operations): n=4 RR 33/37/47, priority 54/70/110; n=16 RR 202/219/363, priority 239/247/315 (min/median/max). This is host-only timing, not kernel switch latency or IRQ timing; QEMU TSC would be noisy and was not sampled. Thread creation and warmups excluded; picks include copying/validating the table. Batch noise is visible, so no best-only performance claim. `cargo test --workspace`: 240 passed, 1 ignored (21+35+174+4+4+2); `cargo fmt --all -- --check` and `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean.
- **Final:** Retain opt-in policy, RR default; n=16 bound limits scaling inference and the priority increment varies by batch. S07 is not done: #21 must land the atomic IRQ record-then-defer gate flip, with yield/preempt semantics decided or explicitly deferred.

### S07 slice47 - verification ledger: environment for slice46 + per-metric baseline/deferral
- **Decision:** Record the slice46 benchmark environment (architecture.md:558-574) and an explicit status for each sprint-roadmap.md S07 verification metric. Docs-only.
- **Slice46 environment:** commit `2920840` (clean tree), `--release` profile of `baseos-kernel` lib tests on host; AMD Ryzen 5 5500 (6C/12T, 3.6 GHz base), 16 GB RAM, Windows 11 Home build 26300; toolchain per `research/benchmarks/host-toolchain.txt` (rustc 1.98.0 88d9e12ae, cargo 1.98.0). No CPU pinning, power plan default, background load uncontrolled -> spread reported, not best-only.
- **Metric ledger (S07 Verification):**
  - *Context switch latency* - DEFERRED to #21. Host pick timing is not a switch. Needs guest `rdtsc` around the live `switch_context` path; under QEMU TCG the TSC is emulated, so any number must be labelled emulator-relative. Lands with the gate flip so the measured path is the real one.
  - *Throughput* - PARTIAL baseline: host pick throughput derivable from slice46 medians (n=4 RR ~27M picks/s, priority ~14M/s; n=16 RR ~4.6M/s, priority ~4.0M/s). Guest dispatches/s DEFERRED to #21 (same instrumentation).
  - *Fairness* - BASELINE (deterministic, counted shares, not timed): RR equal share pinned by tests; priority 1hi+2lo pins hi=150/300 (+-1) and 2nd lo=0 (known-bad, slice44/45); 2hi+1lo starves lo (slice43). Opt-in priority is documented not starvation-safe.
  - *Tail latency* - DEFERRED: slice46 only has batch max (n=16 RR 363 ns/op), which is a batch mean, not a per-op p99. Needs per-op samples; do with guest switch timing in #21 so the tail is of the real path.
  - *Interactive response* - DEFERRED past S07: no input device, no interactive workload, and timer-driven preemption is not enabled until #21. Revisit when an input/event path exists (framework sprints).
- **Why:** REVIEW #531: #21 alone does not satisfy the S07 verification list; host pick timing covers none of the five metrics directly.
- **Alternatives:** Add guest TSC sampling now on the pre-gate path (rejected: measures a path #21 changes; redo cost). Close S07 with host-only numbers (rejected: mislabels).
- **Measured:** n/a (docs-only).
- **Final:** S07 close requires #21 + switch latency, guest throughput, and per-op tail samples; interactive response is a justified deferral.



### S07 slice48 - per-op latency sample buffer (#21, REVIEW #533 C1)
- **Decision:** Add `crates/kernel/src/latency.rs`: a fixed `[u64; 1024]` `LatencySamples` buffer with `record` and `reduce` (min, lower median, nearest-rank p99, max). Once full it does not wrap: extra samples are counted in `dropped`. `p99_valid` is set only when n >= 1000. It uses `core` only, so the guest links it.
- **Why:** The scheduler trace is an 8-entry id prefix with no timestamps, so it cannot carry a tail (REVIEW #533 C1). The guest rdtsc switch timing in #21 needs a place to collect per-op samples so that 'tail' is a real p99 and not a batch max.
- **Alternatives:** A ring buffer that overwrites old samples (rejected: it silently biases toward late samples). Streaming estimators such as P² (rejected: approximate, and more code than 8 KB of exact samples).
- **Tests:** 5 new host tests: empty, single sample (p99 invalid), 1..1000 nearest-rank (median 500, p99 990), tail outliers visible in p99 but not in the median, and overflow counts drops without wrapping. Workspace: 245 passed, 1 ignored. `clippy -p baseos-kernel --all-targets` is clean. The UEFI guest builds.
- **Measured:** n/a. This adds the instrument only; the gate flip, guest wiring, WHPX vs TCG (C2) and fairness re-labelling (C3) follow in #21.

### S07 slice49 - p99 boundary pin and terminal reduction (#21, REVIEW #535)
- **Decision:** Pin nearest-rank p99 at n=1000 with 9, 10 and 11 high outliers. Treat `LatencySamples::reduce` as terminal: call once after all recording; it sorts the recorded prefix in place, so recording afterward cannot preserve temporal order. The guest consumer must follow this contract.
- **Why:** An outlier-heavy test does not distinguish rank-boundary errors. With rank 990, 990 baseline observations and 10 high observations still yield the baseline; 989 baseline observations and 11 high observations yield the high value. The review's suggested 10-outlier expected value was off by one.
- **Alternatives:** Clone an 8 KiB buffer for reduction (rejected: guest stack budget and unnecessary copy); silently continue recording after reduction (rejected: loses temporal ordering).
- **Expected:** Boundary pin passes for 9/10/11; 246 workspace tests pass, 1 ignored; formatting, kernel clippy and UEFI-target build remain green. No guest measurement or IRQ gate change in this slice.
- **Measured (2026-10-06, Win11):** `cargo test --workspace`: 246 passed, 1 ignored; `cargo fmt --all -- --check`, `cargo clippy -p baseos-kernel --all-targets -- -D warnings`, and `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` passed.
- **Final:** Keep nearest-rank p99 and terminal reduction. #21 remains active: place guest buffer outside kernel-thread stacks, add fenced TSC/floor/warmup, compare TCG/WHPX when available, then record-then-defer IRQ gate flip.


### S07 slice50 - enforce terminal reduce + worklog encoding fix (#21, REVIEW #537 N1/N2)
- **Decision:** `LatencySamples` gains a `reduced` flag. `reduce()` sets it, after which `record()` returns `false` without counting a drop, and `is_reduced()` exposes it. Restored the slice48 'P²' that an earlier tool write had re-encoded to U+FFFD. A scan of worklog.md finds no other U+FFFD.
- **Why:** R4 was enforced only by documentation (N2). With the flag, the API refuses misuse.
- **Alternatives:** `reduce(self)`, which consumes the buffer (rejected: R2 places the buffer in a static, and a static cannot be moved out of).
- **Expected:** 7 latency tests pass; workspace 247 passed, 1 ignored; fmt, kernel clippy and the UEFI build stay green.
- **Measured (2026-10-06, Win11):** `cargo test --workspace`: 247 passed, 1 ignored. `cargo fmt --all -- --check`, `cargo clippy -p baseos-kernel --all-targets -- -D warnings` and the UEFI-target build all passed.
- **Final:** Keep the flag. #21 still needs R2 (static guest buffer), R3 (fenced TSC, floor, warm-ups), C2, C3 and the IRQ gate flip.

### S07 slice51 - static guest switch-latency storage (#21 R2)
- **Decision:** Place `SWITCH_LATENCIES: LatencySamples` in boot-UEFI static storage, outside the three 8 KiB thread stacks. The live scheduler-switch probe refuses with `SCHED-SWITCH ok=0` if its address range overlaps any stack or an address computation overflows. No samples are recorded yet; measurement wiring belongs to R3.
- **Why:** A local 8 KiB sample buffer on a thread stack would consume the entire stack before the switch timing even starts; disjoint static storage makes the later in-place reduction safe without a second 8 KiB stack copy.
- **Alternatives:** Stack-local samples (rejected: 8 KiB stack exhaustion); heap allocation (rejected: no guest heap); clone-for-reduction (rejected: another 8 KiB stack copy).
- **Expected:** One new host test covers real static addresses, adjacent ranges, overlap and arithmetic overflow; the existing guest switch and exception proof remain intact. Zero runtime sampling cost until R3.
- **Measured (2026-10-06, Win11):** `cargo test --workspace`: 248 passed, 1 ignored. `cargo fmt --all -- --check`, kernel all-targets clippy, UEFI-target clippy, UEFI-target build all pass. QEMU `tools/qemu/boot-uefi.ps1 -TimeoutSec 20`: UEFI BOOT PASS, `SCHED-SWITCH ok=1 from=1 to=2`, `TICK ok=1`, `PF ok=1`, `EXC ok=1`; serial 1667 bytes, guest 138752 bytes. No latency samples or throughput measurements yet.
- **Final:** Keep static placement and fail-closed range check. #21 remains active: R3 fenced TSC/floor/warm-ups, C2 emulator comparison, C3 starvation classification, and record-then-defer IRQ gate flip are still owed.

### S07 slice52 - fenced TSC, empty-pair floor, warm-ups (#21 R3 host half)
- **Decision:** Add kernel `cpu` timing helpers with no guest wiring yet: `read_tsc` (raw `rdtsc`), `read_tsc_fenced` (`lfence; rdtsc; lfence`), pure `tsc_delta` (saturating, reorder floors to 0), `empty_tsc_pair` (back-to-back fenced reads = per-sample overhead floor), `warm_tsc` (discards `TSC_WARMUP_ITERS=128` fenced reads). Two host tests: unsafe-fn shape + delta saturation, and warm-up + 16 monotonic empty pairs with a generous finite bound (no tight ceiling; hosted TSC granularity varies).
- **Why:** Switch-latency samples are meaningless without serialized timestamps, a known overhead floor to subtract, and a cold-start guard. Pinning the helpers host-side first keeps the later guest sampling slice small and reviewable.
- **Alternatives:** `rdtscp` (rejected: permission/CPUID gating unproven here; `lfence; rdtsc` suffices for ordering); tight floor ceiling in test (rejected: flakes on loaded hosts/TCG; C2 owns the emulator comparison); guest wiring in the same slice (rejected: doubles single-cycle risk; sampling into `SWITCH_LATENCIES` rides next).
- **Expected:** Two new kernel tests pass; workspace 250 passed, 1 ignored; fmt, kernel all-targets clippy, UEFI-target clippy/build green; guest serial unchanged (no guest edit, no QEMU re-run this host-only slice).
- **Measured (2026-10-06, Win11):** `cargo test --workspace`: 250 passed (21 boot-info + 36 boot-uefi + 183 kernel incl. 2 new + 4 + 4 serial-marker + 2 version), 1 ignored. `cargo fmt --all -- --check` clean. `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean. `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` and UEFI-target clippy clean.
- **Final:** Keep helpers unwired. #21 remains active: R3 guest sampling (subtract floor, record into static buffer, reduce + serial line), C2 emulator comparison, C3 starvation classification, and record-then-defer IRQ gate flip are still owed.

### S07 slice53 - guest switch sampling + containment pins (#21 R3 guest half)
- **Decision:** Wire one overhead-corrected sample into the live `probe_sched_switch` guest path: `warm_tsc`, `floor = empty_tsc_pair`, fenced `start/end` around the single proven `run_on_thread_stack` hop, `sample = switch_sample_delta(start, end, floor)` recorded best-effort into static `SWITCH_LATENCIES` (record failure never flips the `SCHED-SWITCH` line). New pure `cpu::switch_sample_delta` (saturating raw-minus-floor, sub-floor floors to 0 for coarse/emulated TSC). Close REVIEW #542 gap 1 with two containment asserts (stack-inside-storage and storage-inside-stack both overlap). No serial shape change; reduction + serial line ride next.
- **Why:** Slice52 helpers were host-only by design; the first guest sample must bracket only the already-proven hop so timing cannot change the ok=1 path, and the record must be best-effort until the reduce/serial contract lands. Containment was handled by the half-open logic but unpinned.
- **Alternatives:** Time both hops / loop N samples now (rejected: changes serial + stack budget in one cycle; single sample proves the path, batching rides with the reduce line). Fail the switch line when the buffer is full (rejected: measurement backpressure must not break scheduling proof; terminal-refusal is already pinned host-side).
- **Expected:** Existing counts hold (250 passed, 1 ignored — asserts extend two existing tests); fmt, kernel clippy, UEFI clippy/build green; QEMU `SCHED-SWITCH ok=1` intact with serial shape unchanged.
- **Measured (2026-10-06, Win11):** `cargo test --workspace`: 250 passed (21 boot-info + 36 boot-uefi + 183 kernel + 4 + 4 serial-marker + 2 version), 1 ignored. `cargo fmt --all -- --check` clean. `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean. `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` and UEFI-target clippy clean. QEMU `tools/qemu/boot-uefi.ps1 -TimeoutSec 20`: UEFI BOOT PASS, serial 1667 bytes, guest 139776 bytes, `SCHED-SWITCH ok=1 from=1 to=2` + `TICK/PF/EXC ok=1`, no REFUSAL-FAIL. One sample recorded per boot; no reduction/throughput numbers yet.
- **Final:** Keep single-sample wiring. #21 remains active: R3 reduce + serial line (incl. empty-buffer shape), C2 emulator comparison, C3 starvation classification, and record-then-defer IRQ gate flip are still owed.

### S07 slice54 - reduce + SWITCH-LAT serial line (#21 R3 reduce half)
- **Decision:** Terminal-reduce static `SWITCH_LATENCIES` right after the live `probe_sched_switch` hop and emit `SWITCH-LAT ok=<0|1> count=<n> min=<n> med=<n> p99=<n> max=<n> valid=<0|1> dropped=<n>` via new `write_switch_lat_line` + `report_switch_latencies` (empty emits zeroed ok=0). Contract pins REVIEW #544 risk 1: sub-floor (raw below empty-pair floor, coarse/emulated TSC) records 0 and 0s are kept in the population, so min=0 means floor-limited, not zero-cost; n=1 reports valid=0 by design (no tail). Best-effort report never flips SCHED-SWITCH. One host test pins empty ok=0, single-0 ok=1 valid=0, and terminal refusal.
- **Why:** Slice53 recorded one sample with no visibility; the reduce/serial contract is the owed R3 half before C2/C3/IRQ flip. Reducing immediately after the single sample keeps one committable segment; batching N samples rides later.
- **Alternatives:** Fail boot on empty buffer (rejected: measurement must not break scheduling proof); drop 0-floored samples (rejected: hides coarse-TSC floor signal, biases min); N-sample loop now (rejected: serial + stack budget in one cycle).
- **Expected:** 251 passed, 1 ignored; fmt, kernel clippy, UEFI clippy/build green; QEMU PASS with `SWITCH-LAT ok=1 count=1 valid=0`.
- **Measured (2026-10-07, Win11):** `cargo test --workspace`: 251 passed (21 boot-info + 37 boot-uefi incl. 1 new + 183 kernel + 4 + 4 serial-marker + 2 version), 1 ignored. `cargo fmt --all -- --check` clean. `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean. `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean (one `too_many_arguments` allow on the 9-arg writer). `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` ok. QEMU `tools/qemu/boot-uefi.ps1 -TimeoutSec 30`: UEFI BOOT PASS, serial 1753 bytes, guest 163840 bytes, `SCHED-SWITCH ok=1 from=1 to=2` + `SWITCH-LAT ok=1 count=1 min=636012 med=636012 p99=636012 max=636012 valid=0 dropped=0` + TICK/PF/EXC ok=1. Nonzero sample disconfirms the 0-floor pollution risk on this TCG run.
- **Final:** Keep reduce-then-report. #21 remains active: C2 emulator comparison, C3 starvation classification, and record-then-defer IRQ gate flip are still owed.

### S07 slice55 - SWITCH-LAT floor/raw visibility (#21 R3 floor half)
- **Decision:** Thread same-boot `floor=` / `raw=` through `SWITCH-LAT` (`floor` = `empty_tsc_pair` overhead, `raw` = uncorrected `tsc_delta(start, end)`; recorded sample = `raw` saturating-sub `floor`) via `SWITCH_LAST_FLOOR` / `SWITCH_LAST_RAW` statics written once by guest `probe_sched_switch` and read once by the `report_switch_latencies` call site (0/0 when the probe never ran). Suffix appended after `dropped=` so existing `ok=` / `count=` / `dropped=` substring gates stay byte-identical.
- **Why:** REVIEW #546 risk 2: the 636k-cycle TCG sample was uninterpretable without TSC freq + same-boot floor alongside — cheapest disconfirm is emitting floor + raw (or EMPTY) next slice. Addresses REVIEW #544 risk 1 generally the same way: min=0 can now be checked as floor-limited (raw <= floor) instead of ambiguous.
- **Alternatives:** Emit raw start/end pair instead of delta (rejected: raw delta + floor is sufficient to recompute corrected sample; start/end adds 2 fields for no extra disconfirmation). Separate EMPTY line when un-sampled (rejected: explicit `floor=0 raw=0` on the same line already distinguishes un-sampled from sampled without a new gate). LatencyStats struct param for the 11-arg writer (deferred per REVIEW nit — nonblocking, keep diff minimal this slice).
- **Expected:** 251 passed, 1 ignored; fmt, split clippy green (workspace-wide host clippy is not a gate — see footnote); UEFI build ok; QEMU PASS with `SWITCH-LAT ok=1 ... floor=<n> raw=<n>` and `raw - floor == min`.
- **Measured (2026-10-08, Win11):** `cargo test --workspace`: 251 passed (21 boot-info + 37 boot-uefi incl. extended shapes + 183 kernel + 4 + 4 serial-marker + 2 version), 1 ignored. `cargo fmt --all -- --check` clean. `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean. `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean. `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` ok. QEMU `tools/qemu/boot-uefi.ps1 -TimeoutSec 30`: UEFI BOOT PASS, serial 1776 bytes, `SCHED-SWITCH ok=1 from=1 to=2` + `SWITCH-LAT ok=1 count=1 min=344016 med=344016 p99=344016 max=344016 valid=0 dropped=0 floor=27072 raw=371088` + TICK/PF/EXC ok=1. Check: 371088 - 27072 = 344016 == min, so the corrected sample is exactly explained; nonzero again disconfirms always-zero on this TCG run (n=1 still carries no tail — valid=0 by design, distributional 0-risk stays open until N-loop).
- **Final:** Keep floor/raw suffix. #21 remains active: C2 emulator comparison, C3 starvation classification, and record-then-defer IRQ gate flip are still owed.
- **Footnote (STATUS #547):** `cargo clippy --workspace --all-targets` on the host fails by design (`unwinding panics are not supported without std` for `baseos-boot-uefi` bin `bootx64` — UEFI crate requires `--target x86_64-unknown-uefi` + panic=abort). Green gate is split: `cargo clippy -p baseos-kernel --all-targets -- -D warnings` + `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings`. Do not use workspace-wide host clippy as a gate.

### S07 slice56 - REVIEW #550 R1+R2 close: floor/raw disjoint + relation pin (#21)
- **Decision:** Close both nonblocking gaps from agent-b REVIEW #550 on slice55 without changing the serial contract: (a) R1 — extend `switch_latency_storage_disjoint()` to cover `SWITCH_LAST_FLOOR` / `SWITCH_LAST_RAW` (each `storage_disjoint` vs the 3 thread stacks) plus individual asserts in the host disjoint test so a future helper edit cannot silently drop them; (b) R2 — pin `raw saturating-sub floor == recorded sample` host-side with the live TCG numbers (raw=371088 floor=27072 → sample=min=med=p99=max=344016 via `report_switch_latencies`) and guest-side with `debug_assert_eq!(raw.saturating_sub(floor), sample)` at the probe source (debug-only, release timing untouched).
- **Why:** Slice55 left two silent-drift risks: two more u64 statics with no disjoint proof (slice51 invariant incompletely extended), and floor/raw as display-only fields that could drift from min. Cheapest disconfirms per the review.
- **Alternatives:** Separate disjoint helper for floor/raw (rejected: one AND-ed helper keeps the single `probe_sched_switch` gate call unchanged); release `assert!` in the probe (rejected: would risk flipping the ok=1 path on timing; debug-only pins the relation with zero guest cost); separate test fn for the relation (rejected: extending the existing shapes test keeps the count at 37 with the relation beside the passthrough pins).
- **Expected:** 251 passed, 1 ignored; fmt clean; split clippy clean; UEFI build ok; no QEMU re-run (guest change is debug-only + disjoint widening; serial shape byte-identical).
- **Measured (2026-10-08, Win11):** `cargo test --workspace`: 251 passed (21 boot-info + 37 boot-uefi incl. extended disjoint + relation pins + 183 kernel + 4 + 4 serial-marker + 2 version), 1 ignored. `cargo fmt --check` clean. `cargo clippy -p baseos-kernel --tests` clean. `cargo clippy -p baseos-boot-uefi --tests` clean. `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` ok. No QEMU this slice (debug-only guest delta, shape unchanged).
- **Final:** Keep AND-ed disjoint + debug relation. #21 stays active: distributional 0-risk + C2 emulator compare + C3 + IRQ record-then-defer flip still owed; next cheapest step is N-loop sampling, not more single-sample formatting.

### S07 slice57 - N-loop switch sampling, count=16 tail start (#21)
- **Decision:** Guest `probe_sched_switch` times the already-proven live hop `SWITCH_SAMPLE_N = 16` times (one `warm_tsc` + one `empty_tsc_pair` floor, then 16 fenced bracket-record iterations with per-iteration `debug_assert_eq!(raw - floor, sample)`), stashing floor + last-raw for the `SWITCH-LAT floor=/raw=` suffix. Host pins the count=16 reduce shape (`count=16 valid=0`, last-iter context `floor=100 raw=1150`, terminal) in a new `switch_lat_n_loop_shape_host` test; the n=1 raw-floor==min relation test is untouched.
- **Why:** Slice56 closed R1+R2 but the population was still n=1 valid=0 with no tail — the owed distributional step. 16 keeps the TCG boot window short; per-iteration debug asserts keep the R2 relation at record time while the display suffix becomes last-context (documented, no longer min-explaining for N > 1).
- **Alternatives:** Silent sampling entry to avoid 16 `THREAD-RUN` lines (rejected: same emitting entry keeps timing honest; e2e gates on presence not count). Per-iteration floor re-measure (rejected: one floor per boot is the defined overhead baseline; re-measuring adds noise for no disconfirmation). N=1000 for valid=1 now (rejected: boot-time cost before the C2/C3/IRQ decisions; 16 starts the tail cheaply).
- **Expected:** 252 passed, 1 ignored; fmt, split clippy, UEFI build green; no QEMU (serial count change needs a guest boot to confirm, owed next).
- **Measured (2026-10-08, Win11):** `cargo test --workspace`: 252 passed (21 boot-info + 38 boot-uefi incl. new N-loop shape + 183 kernel + 4 + 4 serial-marker + 2 version), 1 ignored. `cargo fmt --check` clean. `cargo clippy -p baseos-kernel --all-targets` clean. `cargo clippy -p baseos-boot-uefi --tests` clean. `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` ok. No QEMU this slice (count 1->16 serial change; guest boot confirmation owed).
- **Final:** Keep N=16 loop + last-raw display. #21 stays active: QEMU count=16 confirm, C2 emulator compare, C3, IRQ record-then-defer flip still owed.

### S07 slice58 - QEMU count=16 confirm + SWITCH-LAT gate (#21, REVIEW #554)
- **Decision:** Confirm the slice57 N-loop live via QEMU and pin it in e2e: add `SWITCH-LAT ok=1 count=16` as a hard gate in `tools/qemu/boot-uefi.ps1` (presence + exact count; `valid=0` rides ungated until N grows toward 1000). No guest change this slice.
- **Why:** REVIEW #554 risk 1: the 1->16 serial shape change shipped with no QEMU, so the claim "e2e gates on presence not count" was unproven. Cheapest disconfirm per the review is a guest boot counting `THREAD-RUN id=2` and `SWITCH-LAT count=`.
- **Alternatives:** THREAD-RUN exact-count gate (rejected: total 18 = 2 singles + 16 burst is fragile; the 16-burst order is verified in the log, the population count is the stable contract). `raw=` rename this slice (deferred: risk 2 stays open; this run supplies the disconfirming numbers first).
- **Expected:** QEMU UEFI BOOT PASS with the new gate; 252 passed, 1 ignored unchanged; fmt clean.
- **Measured (2026-10-08, Win11):** `cargo test --workspace`: 252 passed (21 boot-info + 38 boot-uefi + 183 kernel + 4 + 4 serial-marker + 2 version), 1 ignored. `cargo fmt --check` clean. QEMU `tools/qemu/boot-uefi.ps1 -TimeoutSec 40` x2: UEFI BOOT PASS both runs with new `SWITCH-LAT ok=1 count=16` gate. Run1: `SWITCH-LAT ok=1 count=16 min=142560 med=155880 p99=295560 max=295560 valid=0 dropped=0 floor=20772 raw=163980`, 18x `THREAD-RUN id=2` total (2 singles + 16 contiguous burst pre-SCHED-SWITCH), serial 2017 bytes, guest 166400 bytes. Run2: `count=16 min=145800 med=146988 p99=227772 max=227772 floor=17568 raw=163620` (run-to-run TCG variance is real; tail is nonzero so the always-zero risk is disconfirmed on TCG). Display check: run2 `raw - floor = 146052`, which equals neither min nor med — confirms the suffix is last-raw context only (REVIEW #554 risk 2 evidence, rename/track-min-raw still owed).
- **Final:** Keep the count=16 gate. #21 stays active: C2 emulator compare, C3, IRQ record-then-defer flip, and SWITCH-LAT `raw=` display follow-up (rename or min-corresponding raw) still owed; do QEMU-gated steps before any N growth toward 1000.

### S07 slice59 - SWITCH-LAT `raw=` to `last_raw=` rename (#21, REVIEW #554 R2)
- **Decision:** Rename the `SWITCH-LAT` trailing `raw=` field to `last_raw=` (`write_switch_lat_line` / `report_switch_latencies` param + emit string + guest call-site local + all host exact-shape pins + doc comments). E2E gate unchanged (`SWITCH-LAT ok=1 count=16` substring only, no `raw` match).
- **Why:** REVIEW #554 risk 2 / REVIEW #556 R2: for N=16 the suffix is last-iteration context only (`raw-floor != min/med` on slice58 run2), so `raw=` misreads as min-explaining. Cheapest close is an explicit name; min-corresponding raw tracking stays deferred (needs per-iteration argmin, more guest state for no gate benefit yet).
- **Alternatives:** Keep `raw=` with a comment (rejected: serial readers grep the line, comments do not travel); track min-corresponding raw now (rejected: larger guest change before C2/C3/IRQ decisions; rename already removes the misreading).
- **Expected:** 252 passed, 1 ignored unchanged count; fmt + split clippy + UEFI build green; QEMU PASS with `last_raw=` on the line and the count=16 gate intact.
- **Measured (2026-10-08, Win11):** `cargo test --workspace`: 252 passed (21 boot-info + 38 boot-uefi + 183 kernel + 4 + 4 serial-marker + 2 version), 1 ignored. `cargo fmt --all -- --check` clean. `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean. `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean. `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` ok. QEMU `tools/qemu/boot-uefi.ps1 -TimeoutSec 40`: UEFI BOOT PASS, serial 2022 bytes, guest 166400 bytes, `SWITCH-LAT ok=1 count=16 min=146700 med=191376 p99=325692 max=325692 valid=0 dropped=0 floor=17280 last_raw=262764` (262764-17280=245484, equals neither min nor med — re-confirms display-only, now honestly named).
- **Final:** Keep `last_raw=`. #21 stays active: C2 emulator compare, C3, IRQ record-then-defer flip, and R3 host relation test (floor<=min<=med, p99>=med in release) still owed; do QEMU-gated steps before any N growth toward 1000.

### S07 slice60 - R3 host relation test: reduce ordering + floor exception (#21)
- **Decision:** Close the owed R3 host relation test host-only in `crates/kernel/src/latency.rs`: new `reduce_ordering_min_med_p99_max_holds` pins `min <= median <= p99 <= max` by construction across single/zero/ramp-16/outlier/ascending-1000 populations, pins the floor-limited exception (nonzero floor + sub-floor population reduces to `min=0 < floor` by the slice54 keep-zeros contract), and pins the empirical `floor <= min <= med` + `p99 >= med` shape on a non-floor-limited synthetic N=16 ramp (mirrors slice58 TCG floor=17568 min=145800 med=146988 p99=227772). Pure sort+index logic, so it holds in debug and release (no profile-dependent path). Guest untouched; no QEMU this slice.
- **Why:** Slice59 left exactly this R3 item owed; the relation needed a host pin before the QEMU-gated C2/C3/IRQ steps, and the 0-floored `min=0` case had to be recorded as a designed exception rather than left to contradict a naive `floor<=min` invariant.
- **Alternatives:** Pin `floor<=min` unconditionally (rejected: disproven by the keep-zeros contract — sub-floor records 0, so floor-limited boots have min below floor); QEMU-measured floor/min/med relation test now (rejected: TCG variance is real run-to-run; the host construction invariant plus synthetic-shape pin is the stable gate, live numbers stay worklog evidence).
- **Expected:** 253 passed (252 + 1), 1 ignored; fmt + kernel clippy clean; guest bytes/serial unchanged (host-only).
- **Measured (2026-10-08, Win11):** `cargo test --workspace`: 253 passed (21 boot-info + 38 boot-uefi + 184 kernel + 4 + 4 serial-marker + 2 version), 1 ignored. `cargo fmt --all -- --check` clean (after one auto-format). `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean. Guest untouched; prior QEMU PASS stands (slice59: SWITCH-LAT count=16 floor=17280 last_raw=262764).
- **Final:** Keep ordering-by-construction + documented floor exception. #21 stays active: C2 emulator compare, C3, IRQ record-then-defer flip still owed; QEMU-gated steps before any N growth toward 1000.

### S07 slice61 - REVIEW #560 R2 close: N=16 p99 unpublishable pin (#21)
- **Decision:** Host-only test extension in `crates/kernel/src/latency.rs` (no API/shape change): extend `reduce_ordering_min_med_p99_max_holds` with `assert!(!r.p99_valid)` on both N=16 populations (ramp-16 + synthetic floor=100 ramp), pinning that n=16 < MIN_SAMPLES_FOR_P99 reports-but-must-not-publish the tail. Guest untouched; no QEMU this slice.
- **Why:** REVIEW #560 ACCEPTED slice60 but flagged R2 (medium-low doc risk): N=16 asserts p99>=med without asserting !p99_valid, so a reader copying the shape could publish the tail. Cheapest disconfirm per the review is the one-line pin. R1 (synthetic-scale overclaim) stays open by design -- property test near floor scale rides with C2; R3 (stale SITREP dirty list, 4th occurrence: report claims `crates/boot-uefi/src/main.rs` changed while `git status` clean) stays human/orchestrator.
- **Alternatives:** Property/random-fill test now (rejected: larger slice before QEMU-gated C2; R2 pin is zero-risk and unblocks the copy-paste hazard today); publishable-tail threshold change (rejected: MIN_SAMPLES_FOR_P99=1000 contract already pinned by ascending-1000 test).
- **Expected:** 253 passed (same count, extended test), 1 ignored; fmt + kernel clippy + UEFI clippy/build green; guest bytes/serial unchanged.
- **Measured (2026-10-08, Win11):** `cargo test --workspace`: 253 passed (21 boot-info + 38 boot-uefi + 184 kernel + 4 + 4 serial-marker + 2 version), 1 ignored. `cargo fmt --all -- --check` clean. `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean. `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` + UEFI-target clippy clean. Guest untouched; prior QEMU PASS stands (slice59: SWITCH-LAT count=16).
- **Final:** Keep !p99_valid pin on N=16. #21 stays active: C2 emulator compare, C3, IRQ record-then-defer flip, R1 property test still owed.

### S07 slice62 - REVIEW #560 R1 close: near-floor scale pin (#21)
- **Decision:** Host-only test extension in `crates/kernel/src/latency.rs` (no API/shape change): extend `reduce_ordering_min_med_p99_max_holds` with a near-floor N=16 block (floor=950, raws 900..1050 step 10) pinning the scale-appropriate contract — saturating-sub endpoints (min/max equal min/max-raw minus floor), ordering by construction, min < floor (5 sub-floor raws floor to 0, slice54 keep-zeros), and !p99_valid. Documents why the far-floor `floor<=min` empirical (slice60 floor=100 vs min~900) must NOT be copied at this scale.
- **Why:** REVIEW #560 R1: the synthetic floor=100 vs min~900-1050 `floor<=min` assert is vacuous (floor an order of magnitude below the population). Cheapest close is a same-test block at comparable scale showing the invariant that actually travels (ordering + endpoints + unpublishable tail) and the one that does not (floor<=min fails by design near floor).
- **Alternatives:** Randomized property fills now (rejected: larger slice before QEMU-gated C2; deterministic raws already straddle the floor, which is the disconfirming case); assert floor<=min near floor (rejected: false — 950 <= 0 is disproven by the keep-zeros contract).
- **Expected:** 253 passed (same count, extended test), 1 ignored; fmt + kernel clippy + UEFI clippy/build green; guest bytes/serial unchanged.
- **Measured (2026-10-08, Win11):** `cargo test --workspace`: 253 passed (21 boot-info + 38 boot-uefi + 184 kernel + 4 + 4 serial-marker + 2 version), 1 ignored. `cargo test -p baseos-kernel --lib latency`: 8/8. `cargo fmt --all -- --check` clean. `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean. `cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi` + UEFI-target clippy clean. Guest untouched; prior QEMU PASS stands (slice59: SWITCH-LAT count=16).
- **Final:** Keep near-floor pin. #21 stays active: C2 emulator compare, C3, IRQ record-then-defer flip still owed; REVIEW #560 R1 closed, R3 SITREP stale-dirty-list stays human/orchestrator (clean `git status --short` pre/post this slice, 5th occurrence).

### S07 slice63 - C2 emulator compare: -Accel tcg/whpx + both-boot PASS (#21)
- **Decision:** Add `[ValidateSet("tcg","whpx")][string]$Accel = "tcg"` to `tools/qemu/boot-uefi.ps1`, pass `-accel $Accel` to QEMU, banner + `Accelerator:` line stating SWITCH-LAT cycles are emulator-relative (C2; TCG default, WHPX opt-in). All e2e gates byte-identical (still `SWITCH-LAT ok=1 count=16`). No guest/host-code change.
- **Why:** C2 owed an explicit TCG-vs-WHPX comparison; both accelerators are present in this QEMU binary (`-accel help` lists tcg+whpx) but the script hardcoded TCG-by-default with no way to select or label WHPX, so every SWITCH-LAT number was silently TCG-relative.
- **Alternatives:** Keep TCG-only and document WHPX as unavailable (rejected: `-accel help` shows whpx, cheapest disconfirm is one boot with `-accel whpx`); pin WHPX numbers as the new baseline (rejected: variance is real run-to-run on both, and WHPX needs Hyper-V present -- default stays TCG, WHPX opt-in); separate WHPX script (rejected: one param keeps a single proof vehicle).
- **Expected:** `cargo test --workspace` 253 passed, 1 ignored; fmt + split clippy + UEFI build green; gate-test 41/41; TCG default PASS unchanged; WHPX attempt records PASS-or-fail-loud with its SWITCH-LAT line.
- **Measured (2026-10-08, Win11):** `cargo test --workspace`: 253 passed (21 boot-info + 38 boot-uefi + 184 kernel + 4 + 4 serial-marker + 2 version), 1 ignored. `cargo fmt --all -- --check` clean. `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean. `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` + build clean. Gate-test 41/41 PASS. TCG default (`-accel tcg`) UEFI BOOT PASS x2: run1 `SWITCH-LAT ok=1 count=16 min=145764 med=146196 p99=212688 max=212688 valid=0 dropped=0 floor=17316 last_raw=163152`; run2 (post-WHPX restore) `min=134964 med=206640 p99=341604 max=341604 floor=27792 last_raw=162756` (run-to-run TCG variance re-confirmed). WHPX (`-Accel whpx`) UEFI BOOT PASS: `SWITCH-LAT ok=1 count=16 min=1692828 med=1752624 p99=2494872 max=2494872 valid=0 dropped=0 floor=72 last_raw=1730268` -- floor near-zero (hardware TSC, fenced pair cheap) vs TCG floor ~17-27k (emulated TSC), while corrected switch cost reads ~10x larger under WHPX on this host; both `valid=0` (N=16, no publishable tail by design). All numbers stay emulator-relative labels, not absolute claims.
- **Final:** Keep `-Accel` (default tcg). C2 tooling + first TCG/WHPX pair recorded. #21 stays active: C3 starvation classification + IRQ record-then-defer flip still owed; SITREP stale-dirty-list stays human/orchestrator.

### S07 slice64 - C2 WHPX replication + variance note (REVIEW #568 R1 close, #21)
- **Decision:** Measurement-only slice, no code change: two more `-Accel whpx` QEMU boots on the unchanged `@860d1a8` tree plus a variance note across all recorded runs, closing REVIEW #568 R1 (WHPX n=1, floor-72 unreplicated).
- **Why:** REVIEW #568 accepted slice63 but held the ~10x WHPX-vs-TCG switch-cost claim and the near-zero HW-TSC floor interpretation on a single WHPX boot; cheapest disconfirm per the review is 2 more WHPX boots — if floor stays <1k and min stays ~1.5-2M, the claim holds, else variance dominates.
- **Alternatives:** Land C3 or the IRQ flip in the same commit (rejected: QEMU-gated measurement plus scheduler-policy change in one cycle doubles risk; replication is independently committable worklog-only).
- **Expected:** `cargo test --workspace` 253 passed, 1 ignored; fmt + split clippy green; both WHPX boots UEFI BOOT PASS with `SWITCH-LAT ok=1 count=16 valid=0`; floor <1k on both, min ~1.5-2M on both.
- **Measured (2026-10-08, Win11, @860d1a8 tree):** `cargo test --workspace` 253 passed (21 boot-info + 38 boot-uefi + 184 kernel + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` + `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean. WHPX rep1 (`-Accel whpx`) UEFI BOOT PASS, serial 2024 bytes, guest 166400 bytes, `SWITCH-LAT ok=1 count=16 min=1706436 med=1864944 p99=2287296 max=2287296 valid=0 dropped=0 floor=72 last_raw=1713708`. WHPX rep2 UEFI BOOT PASS, `SWITCH-LAT ok=1 count=16 min=1697040 med=1763316 p99=2578570 max=2578570 valid=0 dropped=0 floor=108 last_raw=1714608`. Variance note: WHPX floors 72/72/108 across 3 boots (all <1k — near-zero fenced-pair HW-TSC cost holds, not a one-off); WHPX mins 1692828/1706436/1697040 (~1.69-1.71M, ~1% spread — tight); WHPX meds 1752624/1864944/1763316 (wider, ~6% spread). TCG history for contrast: mins 142560/145800/146700/145764/134964 (~135-147k), floors ~17-28k, meds 146196-206640. WHPX-vs-TCG min ratio holds ~10-12x across every recorded pair, so the slice63 comparison direction survives replication; med/p99 vary run-to-run on both accelerators, and all samples stay `valid=0` (N=16, no publishable tail by design) and emulator-relative (C2 labels, not absolute claims).
- **Final:** REVIEW #568 R1 closed (WHPX n=3, floor<1k 3/3, min stable); R2 recorded as replicated-not-closed — C2 comparison now rests on 3 WHPX + 5 TCG boots but stays emulator-relative with no tail until N grows toward 1000. #21 stays active: C3 starvation classification + IRQ record-then-defer flip still owed.

### S07 slice65 - C3 starvation classification pin (#21)
- **Decision:** Host-only classification slice in `crates/kernel/src/sched.rs` (guest untouched, no QEMU): new `STARVATION_RR_BOUNDED` doc-const with the C3 table (RR BOUNDED spread<=1, priority-opt-in UNBOUNDED DEFECT per slices 43/44, live-backoff BOUNDED deferral max-3 + drain-only reset with ticks never lost) plus one test `starvation_classification_rr_bounded_priority_unbounded_backoff_bounded` pinning all three in one place (RR 3xReady 300 dispatches each>=99 spread<=1; priority 2hi+1lo 300 dispatches lo==0; all-Done live 3x Some(Err) then drain-only None drained=2 streak=0).
- **Why:** REVIEW #570 keeps #21 active on C3+IRQ flip and warns measurement-only slices are stalling real work; C3 was the owed fairness re-labelling (slice48) now that C2 replicated (slice64). One combined test reuses existing fixtures (no new helpers) and keeps the guest/QEMU path byte-identical.
- **Alternatives:** Docs-only worklog note with no test (rejected: classification without a pin drifts; the three behaviors already have separate pins but no single contract); land IRQ record-then-defer flip in the same commit (rejected: QEMU-gated policy + guest IRQ change doubles single-cycle risk; flip rides alone next per REVIEW #570).
- **Expected:** `cargo test --workspace` 254 passed, 1 ignored; fmt + split clippy + UEFI build green; guest bytes/serial unchanged (host-only).
- **Measured (2026-10-08, Win11):** `cargo test --workspace` 254 passed (21 boot-info + 38 boot-uefi + 185 kernel incl. 1 new + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` + build clean. Guest untouched; prior QEMU PASS stands (slice64 WHPX/TCG SWITCH-LAT count=16).
- **Final:** C3 closed (RR bounded default, priority unbounded opt-in DEFECT, backoff bounded deferral). #21 stays active: IRQ record-then-defer flip alone is the last owed item.

### S07 slice66 - IRQ record-then-defer flip (#21 close)
- **Decision:** Flip the IRQ gate OPEN alone as one slice with the QEMU gate: (a) `sched::IRQ_STACK_SWITCH_PERMITTED` `false`->`true` with docs rewritten for the open state (slice34-36 closed-gate era kept as history); `irq_dispatch_must_defer()` now `false` by construction; (b) guest `timer_irq_rust` reduced to record-only (one gated tick + return) — the slice30 per-tick in-IRQ drain + `debug_assert!` + release `if` guard deleted as dead code, not left branching; the task-context `poll_timer_dispatch` remainder is the sole drainer; (c) three IRQ-gate host tests repointed to the open state (permitted true / helper false) + the C3 test now asserts `STARVATION_RR_BOUNDED` itself, closing REVIEW #572 doc-only weakness; (d) `poll_timer_dispatch` / `sched_poll_live_static` / bring-up docs retargeted (single task-context call site, cumulative proof carries the full window); (e) `docs/unsafe-inventory.md` D11 notes the record-only IRQ body (no site added/removed, shape pins untouched).
- **Why:** REVIEW #510 rule 1 (flip + conversion land atomically) plus REVIEW #570/#572 (flip rides alone with QEMU gate; C3 const asserted or dropped). The conversion branch already existed as the slice36 fail-closed guard, so the flip activates record-then-defer with no new branch: IRQ accumulates `pending`, the `sti` wait still fires via cumulative (`pending + drained_total`), and the post-`cli` task batch drains + dispatches once. Cheapest disconfirm is the TCG double-boot below.
- **Alternatives:** Keep gate closed with in-IRQ metadata dispatch (rejected: leaves the in-IRQ dispatch path the gate exists to retire; deferral is strictly less IRQ work); land a real RSP switch in the same commit (rejected: no switch primitive exists in-tree; the flip defers switching out of IRQ, it does not land one); docs-only flip without QEMU (rejected: reviewer explicitly gated the flip on a live TICK proof).
- **Expected:** 254 passed, 1 ignored (same count — asserts extended, none added); fmt + split clippy + UEFI build green; gate-test 41/41; QEMU x2 UEFI BOOT PASS with `TICK ok=1 ticks=5` + `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3` + `SWITCH-LAT ok=1 count=16`.
- **Measured (2026-10-08, Win11):** `cargo test --workspace` 254 passed (21 boot-info + 38 boot-uefi + 185 kernel + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` + build clean; `test-boot-uefi-gates.ps1` GATE TEST PASS (41/41). QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2022 bytes, guest `bootx64.efi` 166400 bytes (identical to slice59 baseline — IRQ-body shrink absorbed by page rounding). Run1: `SWITCH-LAT ok=1 count=16 min=133164 med=176148 p99=457037 max=457037 valid=0 dropped=0 floor=29016 last_raw=165060`; run2: `min=197388 med=261756 p99=510372 max=510372 floor=29016 last_raw=229428` (run-to-run TCG variance re-confirmed, tail nonzero, `valid=0` by design). Both: `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3`, `TICK ok=1 ticks=5` (QUANTUM+1 via the deferred task batch — the flip proof).
- **Final:** IRQ record-then-defer live; #21 owed items all landed (R3 sampling + SWITCH-LAT, C2 TCG/WHPX n=3, C3 classification, IRQ flip). #21 ready for agent-b re-verify / done on concurrence.

### S07 slice67 - retire dead must-defer helper + rename stale tests (REVIEW #574 nits 1+2)
- **Decision:** Host-only cleanup in `crates/kernel/src/sched.rs` (guest untouched, no QEMU): delete `irq_dispatch_must_defer()` (always-false since the slice66 flip, dead in guest — zero non-test callers) and rename the two stale tests to the open state — `irq_gate_enforcement_helper_defers_while_closed` -> `irq_gate_open_permits_task_context_dispatch` (pins gate true) and `irq_gate_fail_closed_release_refuses_switch` -> `irq_gate_open_has_no_closed_branch` (pins gate-true branch, drops the deleted helper branch). `irq_dispatch_result_is_deferred_never_stack_switch` name kept (still describes record-then-defer).
- **Why:** REVIEW #574 ACCEPTED slice66 but named nits 1 (dead always-false helper) and 2 (stale closed-era names) as nonblocking follow-ups. Cheapest close is deletion + renames with no behavior change, keeping every gate green.
- **Alternatives:** Repurpose the helper for the future RSP-switch guard (rejected: no switch primitive exists in-tree — a guard for nonexistent code would be a second dead helper; nit 3 stays open for the real-switch slice); keep old names as history (rejected: reviewer explicitly flagged them stale, and slice67 comments already record the old names).
- **Expected:** `cargo test --workspace` 254 passed, 1 ignored (same count — one fn deleted, two tests renamed); fmt + split clippy + UEFI build green; guest bytes unchanged (host-only, QEMU skipped by non-interference).
- **Measured (2026-10-08, Win11):** `cargo test --workspace` 254 passed (21 boot-info + 38 boot-uefi + 185 kernel incl. 2 renamed + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean. Guest untouched; prior QEMU PASS @11a50eb stands (TCG x2 TICK 5 / count=16).
- **Final:** REVIEW #574 nits 1+2 closed on the current tree. Nit 3 (RSP-switch primitive + proof) explicitly NOT attempted — the next real-switch slice must land the primitive; OPEN is permission, not proof. Nit 4 (QEMU/41-41 on reviewer log evidence, host-only repro) acknowledged, no action.

### S07 slice68 - real IRQ RSP-switch + two-stack proof (REVIEW #574 nit 3)
- **Decision:** Land the owed real RSP-switch as one QEMU-gated slice: (a) `cpu::read_rsp()` (safe `mov {}, rsp` wrapper; host-callable) + test (nonzero, nested frame lower); (b) `timer::IRQ_STACK` (dedicated 8 KiB `repr(C, align(16))` static — plain `[u8; N]` is only 1-aligned and would hand the switch a misaligned top) + pure `irq_stack_top_for` / `irq_stack_contains` + test (top arithmetic, 16-alignment, boundaries, task-outside, saturating-MAX); (c) `timer_irq_rust` now switches via the S06 `run_on_thread_stack(top, tick_on_irq_stack)` primitive — the tick (inner-`RSP` volatile capture into `IRQ_INNER_RSP` + one gated `tick_gated`) runs ON the IRQ stack, entry `RSP` restored before stub EOI/`iretq`; still record-then-defer (no drain/dispatch in IRQ — slice66 flip stands); (d) `uart::write_irqstack_line` + task-context `IRQSTACK ok=<0|1> task=<0x...> irq=<0x...> base=<0x...> size=8192` proof line in bring-up after `TICK` (ok = inner nonzero + inside + `irq != task` + task outside + 16-aligned endpoints); (e) `boot-uefi.ps1` hard-requires the `IRQSTACK ok=1 task/irq` shape (same-line regex mirroring the PF gate); (f) `docs/unsafe-inventory.md` A10/D12 rows + lib.rs shape pins extended (`fn(bool,u64,u64,u64,u64)`, new sites/rows); sched gate doc notes the live proof.
- **Why:** REVIEW #574/#576 ACCEPT left nit 3 as the sole load-bearing open item: `IRQ_STACK_SWITCH_PERMITTED=true` was permission, not proof. The cheapest real disconfirm is a dedicated IRQ stack with the tick provably running on it (inner capture inside) while dispatch provably stays on the task stack (task `RSP` outside, `irq != task`) — one serial line, fail-loud `ok=0` otherwise.
- **Alternatives:** `read_rsp`-only capture with no stack switch (rejected: that re-proves nesting depth, not a switch — reviewer asked for the primitive); IST-backed stack switch via IDT gate (rejected: larger IDT/TSS surface for the same proof; the S06 call-based switch already preserves callee-saved regs and is QEMU-proven); warn-only serial line without ps1 hard-require (rejected: agent-b standing fail-loud rule — missing proof must FAIL the boot).
- **Expected:** `cargo test --workspace` 256 passed (254 + 2 new), 1 ignored; fmt + split clippy + UEFI build green; QEMU x2 UEFI BOOT PASS with `TICK ok=1 ticks=5` + `IRQSTACK ok=1` (inner inside, task outside).
- **Measured (2026-10-08, Win11):** `cargo test --workspace` 256 passed (21 boot-info + 38 boot-uefi + 187 kernel incl. 2 new + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean. QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2090 bytes, guest `bootx64.efi` 167936 bytes (was 166400 — new stack/statics + switch code). Both: `SCHED-TICK ok=1 q=4 used=4 expired=1 next=3`, `TICK ok=1 ticks=5`, `IRQSTACK ok=1 task=0xde1f6b8 irq=0xde27a08 base=0xde25a60 size=8192` (inner 0x58 below top 0xde27a60 — switch pushes + `call` + debug prologue — strictly inside `[base, base+8192]`; task 0xde1f6b8 far outside; base 16-aligned).
- **Final:** REVIEW #574 nit 3 closed with a live switch + two-stack serial proof: IRQ records on its own stack and returns (ticks still flow), task-context poll still owns the sole drain-then-dispatch. Task #23 ready to close pending agent-b review.

### S07 follow-up - save rsi/rdi in IRQ stub (TASK #24 / REVIEW #579 note 1)
- **Decision:** Widen `baseos_isr_timer` from the 7-reg Win64-only frame to the 9-reg Win64+SysV volatile superset (`rax rcx rdx rsi rdi r8-r11`, LIFO pops) in `crates/kernel/src/timer.rs`; callee-saved (`rbx rbp r12-r15`) stay owned by `timer_irq_rust` under both ABIs. Host shape test extended (all 9 + order + 9*8+32=104 `% 16 == 8` alignment pin). No `hlt`-only-window justification kept — the old frame silently corrupted `rsi/rdi` on any future non-`hlt` interruptible path.
- **Why:** REVIEW #579 note 1 / REVIEW #582 carried risk: timer stub saved Win64-volatiles only, so SysV `rsi/rdi` died across any IRQ that lands outside the current `sti`/`hlt` wait. Cheapest close is saving the superset (odd-count + shadow keeps pre-`call` 16-alignment, same as before); the sentinel-QEMU disconfirm becomes unnecessary once the save lands before S08.
- **Alternatives:** Document the `hlt`-only window as safe (rejected: unproven and fragile — any future task-context interruptible path silently corrupts state); save full callee-saved set too (rejected: `timer_irq_rust` already preserves them per both ABIs, stub would double-save for no gain); sentinel-only test with no save (rejected: test proves the bug but leaves it live into S08).
- **Expected:** `cargo test --workspace` 256 passed, 1 ignored (asserts extended, none added); fmt + clippy clean both targets; QEMU TCG x2 UEFI BOOT PASS with `TICK ok=1` + `IRQSTACK ok=1`.
- **Measured (2026-10-08, Win11):** `cargo test --workspace` 256 passed (21 boot-info + 38 boot-uefi + 187 kernel + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean. QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2090 bytes, guest `bootx64.efi` 167936 bytes (unchanged — 4 extra push/pop absorbed without size motion), `TICK ok=1 ticks=5` + `IRQSTACK ok=1` on both.
- **Final:** REVIEW #579 note 1 closed by save (no `hlt`-only excuse remains). Task #24 ready to close pending agent-b review; nit4 reviewer-repro QEMU still untouched per prior review.

### Pre-S08 - IRQ vector/red-zone audit (TASK #593 / board #25)
- **Decision:** Close the pre-S08 audit host-side with one pin test (no guest/stub change): new `timer_irq_path_needs_no_vector_save_and_no_redzone` in `crates/kernel/src/timer.rs` pins (1) live IRQ-path code (`timer.rs`/`thread.rs`/`cpu.rs` pre-`#[cfg(test)]` heads) contains no float/vector tokens, and (2) a live `rustc -Z unstable-options --print target-spec-json --target x86_64-unknown-uefi` query still reports `"disable-redzone": true` + `-sse` + `+soft-float`.
- **Why:** REVIEW #592 R2 flagged the 9-reg stub saves GPRs only (no vector regs) with no repo RUSTFLAGS in-tree. The cheapest disconfirm was the toolchain target spec itself: `x86_64-unknown-uefi` already disables both hazards by construction (`disable-redzone: true`, `features: -mmx,-sse,+soft-float`, `rustc-abi: softfloat`), so no repo-level RUSTFLAGS and no 16-reg vector save are owed before the sti-wiring slice. The test locks both halves so either regression (float in the IRQ path, or a toolchain retarget) fails host-side.
- **Alternatives:** Add repo `.cargo/config.toml` `no-red-zone` / `-sse` flags anyway (rejected: redundant with the target spec — a second switch adds drift surface without new protection); widen the stub to save 16 vector regs now (rejected: 256-byte frame for a hazard the target already removes; revisit only if the target-spec pin ever goes red); docs-only audit with no test (rejected: reviewer explicitly required a host pin so the property cannot silently regress).
- **Expected:** `cargo test --workspace` 257 passed (256 + 1 new), 1 ignored; fmt + split clippy clean; guest untouched (no QEMU — stub UNWIRED, `cli`, IDT 32 vectors only, so QEMU would prove no-regression only).
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 257 passed (21 boot-info + 38 boot-uefi + 188 kernel incl. 1 new + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean (after one auto-format of the new test); `cargo clippy -p baseos-kernel --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean. Target-spec query live: `disable-redzone: true`, `features: -mmx,-sse,+soft-float`, `llvm-target: x86_64-unknown-windows`, `entry-abi: win64`. QEMU not re-run (guest bytes unchanged).
- **Final:** TASK #593 audit closed by target-spec evidence + host pin (option (a) proven, option (b) not needed). Board #25 ready to close pending agent-b review; #24 close rationale stands per #592 (host pin + UEFI clippy is the real coverage — stub UNWIRED so QEMU cannot execute the new pushes/pops).

## S08 slice1 — sti-window preemption gate (REVIEW #598, board #26)

- **Decision:** Open S08 with the smallest host-testable gate addressing REVIEW #598: new pure `timer::sti_window_ready(ticks_total, work_done, tick_target, work_target)` (`ticks >= target && work >= target`, routed through `cumulative_wait_ready` so the `>=`-never-`==` storm rule holds) plus one test `sti_window_ready_needs_ticks_and_work` (both-halves fire, tick-only / work-only / zero-zero all refuse, saturation fires). No guest change this slice — the guest `sti` preemption probe routes its wakeup through this next slice.
- **Why:** REVIEW #598 gates #26 on (1) QEMU ticks-with-preempting-workload (not hlt-only), (3) fail-closed on gate violation with no hlt-only assumption. The current `TICK` wait is tick-only (`cumulative_wait_ready`); this gate is the host-pinned shape the probe must satisfy so a tick-only spin or a work-only spin can never report ready.
- **Alternatives:** Guest preemption-probe + QEMU in the same commit (rejected: doubles single-cycle risk — pin the gate host-side first, wire the probe next slice); docs-only gate with no test (rejected: reviewer requires host pin before sti claims).
- **Expected:** `cargo test --workspace` 258 passed, 1 ignored; fmt clean; host + UEFI clippy clean; `boot-uefi.ps1` still UEFI BOOT PASS (guest untouched).
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 258 passed (21 boot-info + 38 boot-uefi + 189 kernel incl. 1 new + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean (after one auto-format); `cargo clippy` (default members) clean, `cargo clippy -p baseos-boot-uefi --tests` clean, `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi` clean (`--workspace --all-targets` still only the known host-bin unwinding-panics noise); `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 2090 bytes, `TICK ok=1` + `IRQSTACK ok=1` + `EXC ok=1 vector=0`.
- **Final:** Gate pinned host-side and committed; next slice wires the guest `sti` preemption probe (busy work + tick count) through it with QEMU proof. Board #26 proposed->active.

## S08 slice2 — guest sti preemption probe through the gate (REVIEW #598/#600, board #26)

- **Decision:** Wire the slice-1 gate into the guest as one committable slice: (a) `timer.rs`: new pure `work_wait_ready` (`>=`) helper + `STI_WORK_TARGET = 20_000` / `STI_WORK_BATCH = 4_000` consts, `sti_window_ready` now the conjunction of `cumulative_wait_ready` && `work_wait_ready` (closes REVIEW #600 nit 3 — both halves route through a named helper), plus host test `sti_work_half_routes_through_named_helper` (helper semantics, const pins, gate==conjunction over 6 shapes incl. saturation); (b) `uart.rs`: new safe `write_sti_line` (`STI ok=<0|1> ticks=<n> work=<m>`), docs `A11` row, host `fn(bool,u64,u64)` safeness pin; (c) `lib.rs` bring-up: the `sti` window now runs a `STI_WORK_BATCH` integer-only busy batch (`saturating_add` + `spin_loop`, `black_box`ed) per spin and wakes only on `sti_window_ready(cumulative_snapshot(), work, QUANTUM_TICKS+1, STI_WORK_TARGET)` (200-`hlt` bound kept), then after `cli` + gated drain emits `STI ok=<0|1>` over the gated cumulative total + work counter (fail-closed arm emits `STI ok=0` too); `TICK` stays the tick-half proof untouched; (d) `boot-uefi.ps1` hard-requires `STI ok=1` with positive ticks AND work on one line (missing = FAIL).
- **Why:** REVIEW #600 approved the slice-1 host gate but held #26 active: the gate was dead code (zero callers) and REVIEW #598 gate (1) QEMU ticks-with-preempting-workload was unsatisfied (old TICK/IRQSTACK/EXC PASS only). This slice makes the wakeup itself the preemption proof — ticks advance (IRQ0 preempts) while the workload counter advances (task makes progress), and either half stalled reports `ok=0`.
- **Alternatives:** Separate workload thread switch in the `sti` window (rejected: full task switch inside the IRQ window doubles single-cycle risk — the busy counter proves preemptibility with the existing IRQ path untouched); warn-only STI line without ps1 hard-require (rejected: standing fail-loud rule — missing proof must FAIL the boot); QEMU negative run with a sabotaged build (rejected: host refuse-shapes already pin fail-closed tick-only/work-only/zero-zero; a second sabotage boot buys no new signal this slice).
- **Expected:** `cargo test --workspace` 259 passed (258 + 1 new), 1 ignored; fmt + host + UEFI clippy clean; QEMU TCG x2 UEFI BOOT PASS with `TICK ok=1 ticks=5` + `STI ok=1 ticks=5 work=20000` + `IRQSTACK ok=1`.
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 259 passed (21 boot-info + 38 boot-uefi + 190 kernel incl. 1 new + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean (one auto-format for the row-anchor line + one `is_multiple_of` clippy fix); `cargo clippy --all-targets -- -D warnings` clean; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` clean; targeted `sti_window_ready_needs_ticks_and_work` + `timer_irq_path_needs_no_vector_save_and_no_redzone` (rsi/rdi + redzone/softfloat pins) green. QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2118 bytes, guest `bootx64.efi` 169472 bytes; both `TICK ok=1 ticks=5`, `STI ok=1 ticks=5 work=20000` (work exactly the target, ticks exactly QUANTUM+1), `IRQSTACK ok=1`, `EXC ok=1 vector=0`. Fail-closed QEMU negative stays host-pinned (tick-only/work-only/zero-zero refuse shapes in `sti_window_ready_needs_ticks_and_work`); no sti claim beyond the joint line is made.
- **Final:** REVIEW #598 gate (1) satisfied (ticks advance while preempted workload progresses, serial-proven) and #600 gaps 1+2 closed (gate wired, QEMU proof); nit 3 closed (both halves through helpers). Board #26 ready to close pending agent-b review.

## S08 slice3 — STI wait-cost trace capture (spins field + named spin budget)

- **Decision:** Extend the slice-2 `STI` proof line with the used `hlt` wait-loop count as one committable slice: (a) `timer.rs`: new `STI_MAX_SPINS: u32 = 200` const (single-sources the guest loop bound — ~2 s at 100 Hz, far beyond the ~50 ms the tick half needs, so a healthy boot wakes on the gate, never the budget) + host pin assert in `sti_work_half_routes_through_named_helper`; (b) `uart.rs`: `write_sti_line` gains `spins: u64`, line shape `STI ok=<0|1> ticks=<n> work=<m> spins=<s>`; (c) `lib.rs` bring-up: loop bound `200` -> `timer::STI_MAX_SPINS`, both `write_sti_line` call sites pass spins (`u64::from(spins)`; fail-closed arm `0`), safeness pin `fn(bool,u64,u64)` -> `fn(bool,u64,u64,u64)`; (d) `docs/unsafe-inventory.md` A11 row notes the trace field + new pin type; (e) `boot-uefi.ps1` `$stiPat` hard-requires `spins=[0-9]+` on the same line (field must be present; `0` tolerated so a future immediate-wake parameter set still passes).
- **Why:** S08 verification is "a scheduler parameter change produces a reproducible before/after report" — ticks+work alone prove the joint gate, but carry no wait-cost, so a quantum/batch/target change has nothing comparable on serial. `spins` is that cost trace on the existing line (no new line, no new gate), and the named budget removes the last raw `200` the guest and the host pin could drift on.
- **Alternatives:** Separate `STI-SPINS` trace line (rejected: second line doubles the ps1 surface for one number that belongs to the same wakeup event); host-only sweep table with no guest change (rejected: pure-host numbers prove no guest behavior — the trace must come off serial); exact-spins ps1 pin (rejected: spins is timing-sensitive by design — REVIEW #602 risk 1 already warns work=20000 is incidental; presence + reproducibility across runs is the claim, never an exact count).
- **Expected:** `cargo test --workspace` 259 passed, 1 ignored; fmt + host + UEFI clippy clean; QEMU TCG x2 UEFI BOOT PASS with identical `STI ok=1 ticks=5 work=20000 spins=4` on both runs.
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 259 passed (21 boot-info + 38 boot-uefi + 190 kernel + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean; `cargo clippy --all-targets -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings` exit 0; QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2126 bytes, guest `bootx64.efi` 169472 bytes; both `TICK ok=1 ticks=5`, `STI ok=1 ticks=5 work=20000 spins=4`, `IRQSTACK ok=1`, `EXC ok=1 vector=0`. spins=4 is gate-wake (5th batch clears work=20000 with ticks=5 already met), far under the 200 budget — wake-on-gate, not wake-on-budget, serial-proven twice.
- **Final:** First S08 trace-capture field landed with a reproducible x2 baseline (spins=4). Next slice per roadmap: parameter configuration (e.g. quantum/batch knob) with a before/after report measured through this trace; board #26 stays `done` (slice-2 review #602 approved close), new slice proposes its own board task via TASK mail.

## S08 slice4 -- scheduler parameter knob + before/after report via STI trace

- **Decision:** Turn the first S08 parameter knob through the slice-3 trace as one committable slice: (a) `timer.rs`: `STI_WORK_BATCH` `4_000` -> `2_000` (halved batch at fixed `STI_WORK_TARGET` doubles the work-bound spin count: `batches_needed = TARGET / BATCH` 5 -> 10) + host pin updated + new `TARGET / BATCH == 10` assert + coupling doc (`work = (spins + 1) * BATCH` when work-bound); (b) `uart.rs`: `write_sti_line` gains `batch`/`target`, line shape `STI ok=<0|1> ticks=<n> work=<m> spins=<s> batch=<b> target=<t>`; (c) `lib.rs` bring-up: both call sites pass `STI_WORK_BATCH`/`work_target` (fail-closed arm passes the consts too, so the config stays visible), safeness pin `fn(bool,u64,u64,u64)` -> `fn(bool,u64,u64,u64,u64,u64)`; (d) `docs/unsafe-inventory.md` A11 row notes the knob-report fields + new pin type; (e) `boot-uefi.ps1`: `$stiPat` hard-requires `batch=[0-9]+` + `target=[0-9]+` on the same line, plus a new `STI trace:` detail line extracting ticks/work/spins/batch/target with a `[WARNING wake-on-budget]` flag when `spins >= 200` (REVIEW #605 note 2: presence-only still passes, but a budget-hit never reads as healthy).
- **Why:** S08 verification is "a scheduler parameter change produces a reproducible before/after report" -- REVIEW #605 approved slice3 and cleared #27 with two notes: (1) spins couples to work when work-bound, so batch/target must ride alongside spins or tick-bound vs work-bound changes misread; (2) presence-only ps1 tolerates spins>=200 with ok=1, so the before/after report must flag wake-on-budget. This slice closes both notes and produces the first knob-turn report.
- **Alternatives:** Quantum knob (`QUANTUM_TICKS` 4 -> 8, moving the tick target) (rejected: feeds the `SCHED-TICK q=4` ps1 literal plus many host asserts -- far bigger blast radius than the isolated STI workload knob; quantum study rides with a later slice); doubling `STI_WORK_TARGET` instead of halving the batch (rejected: equivalent work-bound shift, but keeping target=20000 holds `work=20000` identical before/after so `spins` is the isolated delta, with `ticks` the honest second delta); exact-spins ps1 pin (rejected: spins is timing-sensitive by design -- presence + x2 reproducibility is the claim, never an exact count).
- **Expected:** `cargo test --workspace` 259 passed, 1 ignored; fmt + host + UEFI clippy clean; QEMU TCG x2 UEFI BOOT PASS with `STI ok=1 ticks=10 work=20000 spins=9 batch=2000 target=20000` on both runs, no budget WARNING.
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 259 passed (21 boot-info + 38 boot-uefi + 190 kernel + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean (one auto-format of the fail-closed call site); `cargo clippy --all-targets -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -D warnings` exit 0. QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2152 bytes both, guest `bootx64.efi` 169472 bytes; both `TICK ok=1 ticks=10`, `STI ok=1 ticks=10 work=20000 spins=9 batch=2000 target=20000`, `IRQSTACK ok=1`, `EXC ok=1 vector=0`, `STI trace:` detail printed with no WARNING. Before/after (before = slice-3 baseline @c79268f x2, after = this tree x2):

  | | batch | target | ticks | work | spins | serial |
  |---|---|---|---|---|---|---|
  | before | 4000 | 20000 | 5 | 20000 | 4 | 2126 B |
  | after | 2000 | 20000 | 10 | 20000 | 9 | 2152 B |

  `work=(9+1)*2000=20000` exactly -- work-bound wake on the 10th batch as predicted (note-1 coupling confirmed live, and now self-describing on the line). `ticks` 5 -> 10 is the honest second delta: the STI `ticks` field is the post-drain cumulative total over the longer (~100 ms) window, not the unchanged `wait_target = QUANTUM_TICKS + 1 = 5`. `spins=9` far under the 200 budget -- wake-on-gate, not wake-on-budget, serial-proven twice.
- **Final:** S08 verification demonstrated end-to-end: a scheduler-parameter knob turn (batch 4000 -> 2000) produces a reproducible x2 before/after report through the STI trace (spins 4 -> 9, batch/target on the line, no budget warning). Board #27 stays `active` pending agent-b review of this commit.

## S08 slice5 -- REVIEW #607 nits N1+N2 (approved slice4 follow-up)

- **Decision:** Close REVIEW #607 nits N1+N2 as one wording-only slice (no guest logic change): (a) N1 -- `timer.rs` `STI_WORK_TARGET` doc rewritten from the stale pre-knob "work clears after ~5 spins, ticks gate" to parametric work-bound wording (`batches_needed = TARGET / BATCH`: 10 at batch 2000 -> spins ~9 vs 5 -> spins ~4 pre-slice4; at the current batch work needs ~100 ms vs tick half ~50 ms, so work bounds the wakeup); (b) N2 -- `boot-uefi.ps1` STI WARNING reworded from "wake-on-budget: spins>=200, joint gate never cleared" (overstates: the `$stiVals` regex requires ok=1, so spins==200+ok==1 is the ambiguous boundary gate-wake on the 201st batch check, spins caps at exactly 200 for both paths) to "at budget boundary spins>=200 -- inspect gate vs budget", with the comment now stating the true budget-miss (ok=0) never matches and already fails the ok=1 hard-require, and that `-ge 200` is kept because `-gt 200` is unreachable. N3 arrived truncated ("batch/target line shape still e2e-only (host pin is...") -- no action taken, full text requested from agent-b.
- **Why:** #607 approved slice4 @57ea6bc with 3 nits non-blocking; N1 doc was stale post-knob (claimed tick-gated while measured work-bound), N2 WARNING claimed a joint-gate failure on a line shape that proves the gate cleared. Both are cheap to close before the next knob slice.
- **Alternatives:** Change `-ge 200` to `-gt 200` (rejected per reviewer: unreachable, so effectively boundary-only flag either way -- kept `-ge` with the boundary documented); QEMU re-run this slice (rejected: guest bytes/logic untouched, comment/string-only -- prior x2 PASS @57ea6bc stands, ps1 still parses).
- **Expected:** `cargo test --workspace` 259 passed, 1 ignored; fmt + host + UEFI clippy clean; ps1 parses; guest untouched (no QEMU).
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 259 passed (21 boot-info + 38 boot-uefi + 190 kernel + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; PSParser tokenize of `boot-uefi.ps1` OK (ASCII-only kept).
- **Final:** N1+N2 closed on the current tree; N3 pending full text from agent-b. Next: S09 scoping or next S08 knob per roadmap once N3 lands.

## S08 slice6 -- REVIEW #610 N3 host backstop for STI line shape

- **Decision:** Close REVIEW #610 N3 as one refactor-plus-tests slice (byte-identical guest output): (a) `uart.rs`: new pure `emit_sti_line(emit, ok, ticks, work, spins, batch, target)` via `baseos_boot_info::{write_str, write_u64_dec}` (`core`-only, no port I/O, `no_std`-clean); `write_sti_line` keeps its `fn(bool,u64,u64,u64,u64,u64)` signature and now delegates through a `com1_write_byte` infallible closure, so guest and host share one body; (b) new `uart::tests` (2 tests): exact-line pin (`STI ok=1 ticks=5 work=20000 spins=9 batch=2000 target=20000`), fail-closed/zero shape plus decimal/order asserts (names, order, trailing newline).
- **Why:** #610 verified slice5 @6fd7b68 (N1+N2 closed) and gave N3 full text: batch/target shape e2e-only (host pin was fn-signature safeness only; the only shape check is the ps1 `$stiPat`/`$stiVals` regex, so guest format drift is caught only under QEMU). Cheapest disconfirm/fix per reviewer is a shared pure formatter plus host assert -- this slice lands exactly that.
- **Alternatives:** Doc-anchored host test asserting on source text (rejected: string-search on own source is brittle and does not prove the emitted bytes); exact-spins ps1 pin (rejected: timing-sensitive, already decided); QEMU re-run this slice (deferred: guest emit path is byte-identical by construction -- same sink bytes through the shared body -- and host tests pin the exact bytes; next QEMU run re-proves e2e).
- **Expected:** `cargo test --workspace` 261 passed, 1 ignored; fmt + host + UEFI clippy clean; ps1 untouched/parses; guest bytes identical (no QEMU).
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 261 passed (21 boot-info + 38 boot-uefi + 192 kernel incl. 2 new + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean (one auto-format of the new assert); `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0.
- **Final:** N3 closed on the current tree; S08 STI trace now has a host backstop. Next: S09 scoping or next S08 knob per roadmap, pending agent-b review.

## S08 close-out — QEMU re-prove STI line post slice5/6 (REVIEW #612, board #28)

- **Decision:** Re-prove the S08 STI guest line under QEMU after the unboarded slice5 (wording-only N1+N2, @6fd7b68) and slice6 (shared `emit_sti_line` refactor + 2 host tests, @c0a9be8) refactors as one close-out slice: QEMU TCG x2 on the current tree + worklog record, no guest/code change.
- **Why:** REVIEW #612 approved slice6 (N3 closed, shared formatter byte-identical, 3/3 uart pass) but flagged the slice5/6 emit-path refactor risk as guest-e2e-deferred; agent-b STATUS #615 set the close acceptance as a QEMU guest boot re-proving the STI line via the `$stiPat`/`$stiVals` regex, not host-only uart pass.
- **Alternatives:** Host-only re-verify (`cargo test` 261 pass, already green) without QEMU (rejected: covers the formatter bytes but not the guest emit path through COM1 → serial file → ps1 regex); new knob slice bundled into close-out (rejected: standing scope rule — close-out run stays knob-free).
- **Expected:** `cargo test --workspace` 261 passed, 1 ignored; fmt + host + UEFI clippy clean; QEMU TCG x2 UEFI BOOT PASS with the post-knob slice4 shape `STI ok=1 ticks=10 work=20000 spins=9 batch=2000 target=20000`, no budget WARNING, byte-identical across runs.
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 261 passed (21 boot-info + 38 boot-uefi + 192 kernel incl. 2 slice6 tests + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0. QEMU `-accel tcg` x2 on @c0a9be8: both UEFI BOOT PASS, serial 2152 bytes both, guest `bootx64.efi` 169984 bytes; both `TICK ok=1 ticks=10`, `STI ok=1 ticks=10 work=20000 spins=9 batch=2000 target=20000` (decimal/order/newline byte-exact, `$stiPat` shape + `$stiVals` extract both match), `STI trace:` detail printed with no WARNING, `IRQSTACK ok=1`, `EXC ok=1 vector=0`. Matches the slice4 post-knob baseline (@57ea6bc x2: 2152 B, ticks=10/spins=9/batch=2000) — slice5/6 refactors changed no guest bytes on the STI path.
- **Final:** S08 close-out QEMU re-prove PASS x2; STI-line refactor risk retired. S08 done per Definition of Done (knob → reproducible before/after report slice4, host backstop slice6, guest e2e re-proven here). Next: S09 scoping.

## S09 slice1 — user-isolation host pin + QEMU contained-fault proof (REVIEW #620, board #29)

- **Decision:** Smallest runnable S09 cut as one committable slice (no CPL3 entry/return, no user stacks — deferred per REVIEW #620 scope risk): (a) `page_table.rs`: new pure `user_isolation_selftest()` (fixed 4-entry map: kernel RO/RW `U/S=0` at `0x1000`/`0x2000`, user RO/RW `U/S=1` at `0x3000`/`0x4000`; 7 `classify_access` probes: user read/write/execute own range ok, user touch of either kernel page contained, unmapped + non-canonical contained) + 2 host tests (`selftest 7/7` pin; `U/S`-bit pin + exact `UserViolation`/`NotPresent`/`GeneralProtection` classifier verdicts + supervisor-may-access-user + PTE `U/S`-bit round-trip); (b) `boot-uefi/main.rs`: new `write_user_line` (`USER ok=<0|1> passed=<n>/<total>`, same function host-tested and guest-called per S08 N3 lesson) + host shape test + post-EBS call after `GUARD` (fail-closed `Err` containment, never a panic); (c) `boot-uefi.ps1`: new `$userPat = "USER ok=1 [^\r\n]*passed=7/7(\s|$)"` hard-require (mirrors GUARD) + found/missing detail; `test-boot-uefi-gates.ps1`: USER lock + 6 cases (47/47).
- **Why:** S09 verification is "user executes / cannot touch kernel / invalid contained". REVIEW #620 held #29 proposed with three conditions: slice1 must be the smallest runnable cut (U/S-bit PT pin host-only + one contained-fault path, defer CPL3), containment must be defined (expected `#PF` `U/S+R/W` as `UserViolation`, kill-vs-return = fail-closed `Err`, no panic, serial shape + ps1 hard-require, shared host/guest assertion), cheapest disconfirm first (host `U/S` + classifier test before QEMU). This slice lands exactly that.
- **Alternatives:** Full ring-3 entry + `iretq` return + user stacks in slice1 (rejected: too big for one cycle — REVIEW #620 risk 1; entry rides with slice2 on top of this pin); `STI`-style shared formatter in `kernel::uart` (rejected: USER line is emitted pre-bring-up from `boot-uefi` like PERM/GUARD — local writer in the same file the guest calls keeps the share without crossing the `pub(crate)` boundary); exact-ticks/spins-style USER value pin beyond `passed=7/7` (rejected: count pin already stops coverage shrink; values beyond it add timing brittleness for no signal).
- **Expected:** `cargo test --workspace` 264 passed, 1 ignored; fmt + host + UEFI clippy clean; gate-test 47/47; QEMU TCG x2 UEFI BOOT PASS with `USER ok=1 passed=7/7` on both runs, STI/TICK/EXC gates green, byte-identical serial.
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 264 passed (21 boot-info + 39 boot-uefi incl. 1 new + 194 kernel incl. 2 new + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; gate-test 47/47 PASS. QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2173 bytes both (+21 vs S08 2152 for the new line), guest `bootx64.efi` 172032 bytes; both `USER ok=1 passed=7/7`, `GUARD ok=1 passed=7/7`, `TICK ok=1 ticks=10`, `STI ok=1 ticks=10 work=20000 spins=9 batch=2000 target=20000` (unchanged), `IRQSTACK ok=1`, `PF ok=1 vector=14`, `EXC ok=1 vector=0`, no STI budget WARNING.
- **Final:** S09 slice1 U/S pin + contained-fault proof landed with host backstop and QEMU x2 e2e. Next: S09 slice2 (CPL3 entry attempt + contained fault path) per roadmap, pending agent-b review; board #29 ready to close on ACK.

## S09 slice2a — NX non-vacuous pin + SMAP/SMEP threat-model note (REVIEW #622 risks 1+2, board #29 follow-up)

- **Decision:** Close REVIEW #622 risks 1+2 as one committable slice, deferring risk 3 (live-#PF `U/S` path) to slice2b: (a) `page_table.rs`: `user_isolation_selftest()` grows 4-entry/7-probe to 5-entry/9-probe with a user NX page at `0x5000` (`PRESENT|USER|NO_EXEC`): readable (`Read` ok) but fetch-refused with exactly `NoExec`; unmapped probe moves `0x5000`→`0x6000`; doc records the S09 threat model (supervisor-may-access-user by design, no SMAP/SMEP; enforcement deferred to S29 Security Baseline); (b) host pin test extended with the exact `NoExec` verdict + NX `U/S`-bit coverage (selftest pin `9/9`); (c) `boot-uefi/main.rs`: `write_user_line` doc + shape test `9/9`/`8/9`; (d) `boot-uefi.ps1`: `$userPat` `passed=7/7`→`passed=9/9` + summary lines; `test-boot-uefi-gates.ps1`: USER lock `9/9` + new `U-seven` case (old `7/7` fails anti-shrink) → 48 cases.
- **Why:** REVIEW #622 ACKed slice1 but held three slice2 risks: (1) execute probe vacuous (no `NO_EXEC` page, user-execute passes trivially); (2) supervisor-may-access-user pins no-SMAP without stating whether SMAP/SMEP is planned; (3) USER line is classifier-only, not a live `#PF`. Risks 1+2 are host+doc-only with no QEMU risk and land in one cycle; risk 3 needs CPL3 entry/IDT `#PF` work and rides alone as slice2b.
- **Alternatives:** Bundle live-`#PF`/`iretq` CPL3 entry in the same commit (rejected: multi-risk single cycle — CPL3 entry + user stacks + IDT fault path unproven here; REVIEW #620 already scoped CPL3 out of slice1 for the same reason); NX write-probe on the NX page (rejected: NX page is read-only by construction, write verdict would pin `ReadOnly` noise instead of the `NoExec` signal); SMAP/SMEP enforcement now via CR4 (rejected: needs CPL3 + fault-path proof first; S29 is the security-baseline home).
- **Expected:** `cargo test --workspace` 264 passed, 1 ignored; fmt + host + UEFI clippy clean; gate-test 48/48; QEMU TCG x2 UEFI BOOT PASS with `USER ok=1 passed=9/9` on both runs, STI/TICK/EXC gates green, byte-identical serial.
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 264 passed (21 boot-info + 39 boot-uefi + 194 kernel + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; gate-test 48/48 PASS (incl. new `U-seven` old-7/7-fails). QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2173 bytes both (same length: `9/9` replaces `7/7`), guest `bootx64.efi` 171520 bytes; both `USER ok=1 passed=9/9`, `GUARD ok=1 passed=7/7`, `TICK ok=1 ticks=10`, `STI ok=1 ticks=10 work=20000 spins=9 batch=2000 target=20000` (unchanged), `IRQSTACK ok=1`, `PF ok=1 vector=14`, `EXC ok=1 vector=0`, no STI budget WARNING.
- **Final:** REVIEW #622 risks 1+2 closed on the current tree. Next: S09 slice2b (live-`#PF` `U/S` fault path / CPL3 entry scoping) per risk 3; pending agent-b review.

## S09 slice2b -- live-#PF U/S path proof via PFUS line (REVIEW #622 risk 3 supervisor half, board #30)

- **Decision:** Close REVIEW #622 risk 3 (supervisor half) as one committable slice without CPL3 entry: (a) kernel/lib.rs: new pure pf_us_verdict(code, cr2) ((ok, us, w, p) from PfError bits 0-2 + cr2 == PF_PROBE_ADDR; ok only when us=0 w=0 p=0 at the probe) + 1 host test (code `0x0`/probe ok; U=1/W=1/P=1/wrong-addr refuse with decoded bits passthrough; upper PK/SGX/HLAT bits ignored); #PF handler now emits a second fail-closed line via the new uart::write_pfus_line before chaining into #DE; (b) kernel/uart.rs: new emit_pfus_line/write_pfus_line (PFUS ok=<0|1> us=<0|1> w=<0|1> p=<0|1>, STI-line shared-formatter pattern) + 1 host shape test; safe-wrapper binding extended; (c) `boot-uefi.ps1`: new $pfusPat = "PFUS ok=1 [^\\r\\n]*us=0[ \\t][^\\r\\n]*w=0[ \\t][^\\r\\n]*p=0(\\s|$)" hard-require (value gate like RMBZ; mid-line [ \\t] mirrors the PERMAUDIT user pin); `test-boot-uefi-gates.ps1`: PFUS lock + 7 cases (good/us=1/w=1/p=1/ok=10/missing-p/newline-split) -> 55 cases, plus REVIEW #625 nit fix (U-split now newline-splits passed=9/9, not stale 7/7). CPL3 U=1 entry (DPL3 GDT/TSS/iretq, user stacks) stays deferred -- this slice pins the live supervisor half of the U/S path; the user half stays classifier-only.
- **Why:** REVIEW #622 risk 3 held that the USER line is classifier-only (classify_access Err returns), never a live #PF with a CPU-pushed U/S bit. Full CPL3 entry is multi-risk for one cycle (DPL3 segments + TSS + ring-3 transition + return path, all unproven here); the smallest live proof is decoding the already-firing supervisor probe fault (code `0x0` at `0x500000000000`, measured on this tree) inside the IDT vector-14 handler and hard-requiring its shape end-to-end.
- **Alternatives:** Full CPL3 iretq entry + user-mode fault in the same commit (rejected: multi-risk single cycle -- REVIEW #620 already scoped CPL3 out of slice1 for the same reason; supervisor-half pin is independently committable and green); gate-tightening only on the existing PF line (code=0x0 substring assert, no guest change; rejected: proves nothing the handler asserts -- a handler-side decode + dedicated line is the cheapest disconfirm that vector 14 actually sees the U/S bit); us=1-expects-success CPL3-simulating host test on the verdict (rejected: verdict must refuse U=1 until the CPL3 slice defines its expected-fault contract).
- **Expected:** cargo test --workspace 266 passed, 1 ignored; fmt + host + UEFI clippy clean; gate-test 55/55; QEMU TCG x2 UEFI BOOT PASS with PFUS ok=1 us=0 w=0 p=0 after PF ok=1 vector=14 code=0x0 addr=0x500000000000 on both runs, all prior gates green, byte-identical serial.
- **Measured (2026-10-09, Win11):** cargo test --workspace 266 passed (21 boot-info + 39 boot-uefi + 196 kernel incl. 2 new slice2b tests + 4 + 4 serial-marker + 2 version), 1 ignored; cargo fmt --all -- --check clean (one auto-format of the new shape assert); cargo clippy --all-targets -- -D warnings exit 0; cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings exit 0; gate-test 55/55 PASS (incl. 7 new F-* PFUS cases + fixed 9/9 U-split). QEMU -accel tcg x2 on this tree: both UEFI BOOT PASS, serial 2196 bytes both (+23 vs slice2a 2173 for the new line), guest ootx64.efi 173568 bytes; both USER ok=1 passed=9/9, PF ok=1 vector=14 code=0x0 addr=0x500000000000, PFUS ok=1 us=0 w=0 p=0, EXC ok=1 vector=0, TICK ok=1 ticks=10, STI ok=1 ticks=10 work=20000 spins=9 batch=2000 target=20000 (unchanged), no STI budget WARNING.
- **Final:** REVIEW #622 risk 3 supervisor half closed live (handler-decoded U/S=0 end-to-end); REVIEW #625 U-split nit fixed on the current tree. Next: CPL3 U=1 entry scoping (user stacks, DPL3 GDT/TSS, iretq + user-mode fault containment) as the slice2b remainder; pending agent-b review.

## S09 slice2c -- GDT user DPL3 descriptors pinned, table grown 3->5 (board #30 remainder)

- **Decision:** Land the smallest CPL3-remainder cut as one committable slice, no ring transition yet: (a) `kernel/lib.rs`: new `GDT_USER_CODE = 0x00AFFA000000FFFF` (kernel code encoding with DPL 0->3, `0x9A->0xFA` access byte) + `GDT_USER_DATA = 0x00CFF2000000FFFF` (`0x92->0xF2`) with `USER_CODE_SEL = 0x1B` (index 3, RPL 3) + `USER_DATA_SEL = 0x23` (index 4, RPL 3), each documented as descriptor/selector-only (no `iretq`/TSS/user stacks yet); (b) `kernel/cpu.rs`: guest `Gdt` grows `[u64; 3]` -> `[u64; 5]`, static init appends the two user descriptors (`lgdt` limit auto-grows via `size_of`, still reloads the kernel selectors, so ring-0 entry is behavior-preserving); (c) `gdt_descriptors_match_long_mode_layout` test extended in place (user code executable/DPL3/present/L=1, user data writable/non-executable/DPL3/present, selector index + RPL pins).
- **Why:** Board #30 stays active on the CPL3 U=1 remainder (supervisor-half PFUS @1551a84 approved per REVIEW #630, verdict refuses U=1 by design). Full CPL3 entry (TSS + ring-3 `iretq` + user stacks + user-fault containment) is multi-risk for one cycle; the DPL3 segments are the independently committable bottom layer every later CPL3 slice builds on, with zero guest-behavior delta to re-prove.
- **Alternatives:** Full `iretq`-to-ring-3 + TSS + user stack in the same commit (rejected: multi-risk single cycle -- REVIEW #620 already scoped CPL3 out of slice1 for the same reason); TSS descriptor now without an entry path (rejected: 16-byte TSS/system descriptor doubles encoding risk with no consumer -- rides with the `iretq` slice that needs the stack switch); host-only constants without growing the guest table (rejected: leaves the e2e table stale -- growing it now proves the 5-entry `lgdt` boots identically).
- **Expected:** cargo test --workspace 266 passed, 1 ignored (GDT test extended in place, no count change); fmt + host + UEFI clippy clean; gate-test 55/55; QEMU TCG x2 UEFI BOOT PASS byte-identical to the slice2b baseline (serial 2196 B, guest 173568 B, PFUS ok=1 us=0 w=0 p=0).
- **Measured (2026-10-09, Win11):** cargo test --workspace 266 passed (21 boot-info + 39 boot-uefi + 196 kernel incl. extended GDT test + 4 + 4 serial-marker + 2 version), 1 ignored; cargo fmt --all -- --check clean (one auto-format of the new data-executable assert); cargo clippy --all-targets -- -D warnings exit 0; cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings exit 0; gate-test 55/55 PASS. QEMU -accel tcg x2 on this tree: both UEFI BOOT PASS, serial 2196 bytes both, guest bootx64.efi 173568 bytes (both identical to slice2b @1551a84); both USER ok=1 passed=9/9, PF ok=1 vector=14 code=0x0 addr=0x500000000000, PFUS ok=1 us=0 w=0 p=0, EXC ok=1 vector=0, TICK/STI/IRQSTACK green, no STI budget WARNING.
- **Final:** DPL3 user segments pinned host-side and proven boot-neutral live (5-entry GDT loads, ring-0 entry unchanged). Next: TSS + `iretq` to ring 3 + user stack + user-mode fault containment as the #30 remainder; pending agent-b review.

## S09 slice2d -- GDT DPL3 S/DB/G/L shape pins (REVIEW #632 nit 1, board #30 remainder)

- **Decision:** Close REVIEW #632 nit 1 as a host-only test slice: extend `gdt_descriptors_match_long_mode_layout` in place with S/G/DB/L shape asserts (user code S=1/DB=0/G=1; user data S=1/G=1/DB=1/L=0). No guest change (`cpu.rs` GDT bytes untouched); `allow(dead_code)` on the user selectors stays until the `iretq` slice lands (REVIEW #632 nit 2 deferred by design).
- **Why:** The slice2c pins covered X/DPL/P/L/W/RPL+index but not S/DB/G, so a system-descriptor (S=0) or DB/L-conflict typo would pass host-side. Cheapest disconfirm is seven asserts in the existing test, zero QEMU risk.
- **Alternatives:** Full TSS + `iretq` + user stack in the same commit (rejected: multi-risk single cycle; nit closure is independently committable); drop `allow(dead_code)` now (rejected: no consumer yet, would turn the build red with no signal -- removal rides with the `iretq` slice).
- **Expected:** cargo test --workspace 266 passed, 1 ignored (test extended in place); fmt + host + UEFI clippy clean; guest bytes unchanged (host-only).
- **Measured (2026-10-09, Win11):** cargo test --workspace 266 passed (21 boot-info + 39 boot-uefi + 196 kernel incl. extended GDT test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0. Guest untouched (no QEMU re-run; slice2c x2 byte-identical baseline @8510a6a stands).
- **Final:** REVIEW #632 nit 1 closed on the current tree. Next: TSS + `iretq` to ring 3 + user stack + user-mode fault containment as the #30 remainder; pending agent-b review.

## S09 slice2e -- TSS available-descriptor encoding pin (board #30 remainder, REVIEW #634 risk 1)

- **Decision:** Land the first TSS remainder sub-slice as host-only encoding: `TSS_SEL` (`0x28`, index 5 / RPL 0), `TSS_TYPE_AVAIL` (`0x9`), plus pure `tss_desc_low(base, limit)` / `tss_desc_high(base)` constructors (type/S/DPL/P/L/DB/G per SDM Fig 7-4, byte-granular G=0) with one new `tss_descriptor_pins_available_system_shape` test (sys-shape + limit/base round-trip + zero-edge). No guest change (`cpu.rs` GDT stays 5 entries, `TR` never loaded); `allow(dead_code)` stays until the TSS-wiring slice, same deferral as REVIEW #632 nit 2.
- **Why:** REVIEW #634 keeps #30 open with TSS + CPL3 `iretq` frame + user stack + user-fault containment each as one scoped slice; the encoding must be pinned before the table grows to 7 slots, otherwise a busy-type (0xB) or S/DPL typo only faults at `ltr` under QEMU. Cheapest disconfirm is a host round-trip test, zero QEMU risk.
- **Alternatives:** Grow the guest GDT to 7 + `ltr` in the same commit (rejected: multi-risk single cycle — descriptor bytes, table size, and TR load would all be unproven at once); skip the zero-base edge (rejected: one assert, locks the null-probe shape for free).
- **Expected:** cargo test --workspace 267 passed, 1 ignored (1 new kernel test); fmt + host + UEFI clippy clean; guest bytes unchanged (host-only).
- **Measured (2026-10-09, Win11):** cargo test --workspace 267 passed (21 boot-info + 39 boot-uefi + 197 kernel incl. new TSS test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean (one auto-format of the new base_lo chain); `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0. Guest untouched (no QEMU re-run; slice2c x2 byte-identical baseline @8510a6a stands).
- **Final:** TSS encoding pinned host-side, still boot-neutral. Next: TSS table-growth + `ltr` wiring, then `iretq` frame builder, user stack mapping, and user-fault containment as separate slices; pending agent-b review.

## S09 slice2f -- TSS AVL-bit pin (REVIEW #636 nit n1, board #30 remainder)

- **Decision:** Close REVIEW #636 nit n1 as a host-only test slice: one assert `lo >> 52 & 1 == 0` (AVL) in `tss_descriptor_pins_available_system_shape`, completing the byte-granular doc lock (AVL/L/DB/G all 0). No guest change (`cpu.rs` stays 5 entries, `TR` never loaded); nit n2 (`allow(dead_code)` consume-all-at-once checklist) deferred to the TSS-wiring slice by design.
- **Why:** Slice2e asserted S/DPL/P/L/DB/G but not AVL bit 52, so an AVL=1 typo would pass host-side. Cheapest disconfirm is one assert in the existing test, zero QEMU risk.
- **Alternatives:** Bundle TSS table-growth + `ltr` in the same commit (rejected: multi-risk single cycle -- descriptor bytes, table size, TR load unproven at once); drop `allow(dead_code)` now (rejected: no consumer yet, would turn build red -- removal rides with wiring slice per n2).
- **Expected:** cargo test --workspace 267 passed, 1 ignored (test extended in place); fmt + host + UEFI clippy clean; guest bytes unchanged (host-only).
- **Measured (2026-10-09, Win11):** cargo test --workspace 267 passed (21 boot-info + 39 boot-uefi + 197 kernel incl. extended TSS test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0. Guest untouched (no QEMU re-run; slice2c x2 baseline stands).
- **Final:** REVIEW #636 n1 closed on the current tree. Next: TSS table-growth + `ltr` wiring (consuming USER_CODE/USER_DATA + TSS dead_code groups per n2), then `iretq` frame, user stack, user-fault containment; pending agent-b review.

## S09 slice2g -- TSS table-growth + `ltr` wiring (board #30 remainder)

- **Decision:** Land the TSS-wiring slice as one guest-touching commit: (a) `kernel/lib.rs`: `TSS_SEL`/`TSS_TYPE_AVAIL`/`tss_desc_low`/`tss_desc_high` go `pub(crate)` (n2 half-consumed: TSS group now wired; `USER_CODE/USER_DATA_SEL` keep `allow(dead_code)` until the `iretq` slice); (b) `kernel/cpu.rs`: new `repr(C, packed)` `Tss` (104 bytes: `rsp0` at hardware offset 4, empty I/O bitmap `iomap_base=104`, `rsp0=0` until the user-stack slice) + `GDT_LEN=7`/`GDT_TSS_INDEX=5` consts + guest `Gdt` 5->7 slots + `TSS` static + `load_gdt` fills slots 5-6 from the live `TSS` address (`limit=size-1=103`) then `ltr TSS_SEL` after the segment reloads; (c) new host test `tss_wiring_pins_table_shape_and_empty_bitmap` (7 slots, index 5, `TSS_SEL=0x28`/RPL0, size 104, `rsp0=0`, `iomap=104`, limit 103, available-type round-trip); (d) `docs/unsafe-inventory.md` D1 notes the 7-slot + `ltr` delta.
- **Why:** REVIEW #634/#636 keep #30 open on TSS + `iretq` frame + user stack + containment as separate scoped slices; the encoding (slice2e/2f) is pinned, so the next independently committable step is growing the live table and loading `TR` while still entering ring 0 (no `iretq`/user stacks yet, `rsp0=0` never consulted -- boot-neutral by construction, proven by the unchanged serial contract).
- **Alternatives:** Full `iretq`-to-ring-3 + user stack in the same commit (rejected: multi-risk single cycle -- fault-return frame + stack mapping + containment unproven; `ltr` survival must be proven first); host-only table growth without `ltr` (rejected: leaves the `TR`-load fault path unproven -- `ltr` on a bad descriptor `#GP`s, only QEMU proves it); `repr(C)` TSS with aligned `u64` (rejected: pads to 112 bytes -- hardware layout needs `rsp0` at offset 4, so `packed` + `addr_of!`/`read_unaligned` is the only correct shape, caught by the host size assert).
- **Expected:** `cargo test --workspace` 268 passed, 1 ignored (267 + 1 new); fmt + host + UEFI clippy clean; gate-test 55/55; QEMU TCG x2 UEFI BOOT PASS byte-identical serial (2196 B) with all prior gates green (`USER ok=1 passed=9/9`, `PFUS ok=1 us=0 w=0 p=0`, `EXC ok=1`).
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 268 passed (21 boot-info + 39 boot-uefi + 198 kernel incl. new TSS-wiring test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean (after one auto-format of the 7-slot array); `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (55/55). QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2196 bytes both (identical to slice2b/2c baseline), guest `bootx64.efi` 174080 bytes (was 173568: +512 TSS/`ltr` codegen, one PE alignment step); both `USER ok=1 passed=9/9`, `PF ok=1 vector=14 code=0x0 addr=0x500000000000`, `PFUS ok=1 us=0 w=0 p=0`, `EXC ok=1 vector=0`, `TICK/STI/IRQSTACK` green, no STI budget WARNING.
- **Final:** TSS wired live (`TR` loaded, 7-slot GDT boots identically); n2 half-closed (TSS group consumed, USER selectors stay `allow(dead_code)` for the `iretq` slice). Next: `iretq` frame builder, then user stack mapping, then user-fault containment; pending agent-b review.

## S09 slice2h -- ring-0 stack top wired into `TSS.rsp0` (REVIEW #640 loaded-gun, board #30 remainder)

- **Decision:** Close REVIEW #640's highest-value risk as one guest-touching slice: (a) `kernel/cpu.rs`: new `RING0_STACK_LEN` (16384) + `ring0_stack_top_for(base)` const helper (saturating add, mirrors `timer::irq_stack_top_for`) + `align(16)` `Ring0Stack` wrapper + guest-only `RING0_STACK` static; `load_gdt` computes the top and `write_unaligned`s it into `TSS.rsp0` BEFORE `ltr TSS_SEL` (a CPL3->0 transition pushes onto `[rsp0]`, so validity must precede `TR` activation); (b) `Tss` doc updated (`new()` constructs zero, guest wires the top); `TSS_SEL` doc updated; `docs/unsafe-inventory.md` D1 notes the `RING0_STACK` + `write_unaligned` delta; (c) new host test `ring0_stack_top_pins_size_align_and_nonzero_rsp0` (16 KiB, aligned-base fixture, top == base+16384, top 16-aligned, nonzero, `u64::MAX` saturates, guest write path reads back nonzero). No CPL3 entry yet, so the nonzero `rsp0` is still boot-neutral (no CPL3->0 transition can consult it); `iomap_base == 104` unchanged (CPL3 port access stays fail-closed `#GP`).
- **Why:** REVIEW #640 ACKed slice2g but flagged `rsp0 == 0` as a loaded gun -- the first `iretq` to CPL3 with a zero `rsp0` triple-faults on the first interrupt/syscall-return. Wiring the stack top now (before any `iretq` slice lands) removes the gun while the serial contract proves the value is unconsumed; the pre-`ltr` ordering is the cheapest correct shape (valid before active, never the reverse).
- **Alternatives:** Temporary `rsp0 = 1` disconfirm-then-real-stack (REVIEW #640's cheapest disconfirm; rejected as its own cycle: the x2 byte-identical PASS on a nonzero `rsp0` already proves non-consumption -- `1` vs real-top differ only in validity, and only the real top is committable); `iretq` frame + user stack in the same commit (rejected: multi-risk single cycle -- frame encoding + stack mapping + containment unproven; the stack top must exist first); `repr(C)` aligned TSS to avoid `write_unaligned` (rejected: pads to 112 bytes -- settled in slice2g, hardware layout needs `rsp0` at offset 4).
- **Expected:** `cargo test --workspace` 269 passed, 1 ignored (268 + 1 new); fmt + host + UEFI clippy clean; QEMU TCG x2 UEFI BOOT PASS byte-identical to the slice2g baseline (serial 2196 B, guest 174080 B, `PFUS ok=1 us=0 w=0 p=0`).
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 269 passed (21 boot-info + 39 boot-uefi + 199 kernel incl. new ring-0-stack test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0. QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2196 bytes both (identical to slice2g @01a7994), guest `bootx64.efi` 174080 bytes (unchanged -- stack is a `.bss` static, no codegen delta); both `USER ok=1 passed=9/9`, `PF ok=1 vector=14`, `PFUS ok=1 us=0 w=0 p=0`, `EXC ok=1 vector=0`, `TICK/STI/IRQSTACK` green.
- **Final:** Loaded gun defused (live `TR` now points at a valid 16 KiB ring-0 stack top, wired before `ltr`); boot-neutral proven byte-identical. Next: `iretq` frame builder (with the REVIEW #640 pre-`iretq` `rsp0 != 0` assert), then user stack mapping, then user-fault containment; pending agent-b review.
## S09 slice2i -- REVIEW #642 hardening: pre-ltr rsp0 asserts + rsp0-offset/in-range pins (board #30 remainder)

- **Decision:** Close REVIEW #642 risks 2+3 as one guest-touching slice: (a) `kernel/cpu.rs` guest `load_gdt`: `debug_assert_ne!(top, 0)` before the `write_unaligned` plus `debug_assert_ne!(read_unaligned(rsp0), 0)` after the write, both pre-`ltr` (caller-convention gun in review risk 2 now fails loudly in debug instead of silently relying on the wiring order); (b) host test `ring0_stack_top_pins_size_align_and_nonzero_rsp0` extended in place (no new count): `size_of::<Tss>() == 104`, `offset_of!(Tss, rsp0) == 4`, raw `addr_of!` delta == 4 (review risk 3 offset proof -- the `write_unaligned` hits byte 4), plus in-range pins (`top - base == 16384`, `top-1`/`top-8` inside `[base, top)`).
- **Why:** REVIEW #642 ACKed slice2h @b4524ba but held function unproven (risk 1, conceded -- no CPL3 entry yet) and named the two cheap closes: zero-constructed `Tss::new` leaves safety on caller convention, and the packed-104B offset was asserted by doc only. Both close here with zero release codegen change in shape (debug-only asserts) and host offset proof.
- **Alternatives:** `rsp0: NonZeroU64` type-state forcing nonzero at construction (rejected: churns `Tss::new` + statics + packed layout for one call-site -- asserts pin the single guest writer with no type migration); separate new test fn for the offset (rejected: same helper/fixture, in-place extension keeps the gate in one place); `iretq` frame + user stack in the same commit (rejected: multi-risk single cycle -- frame encoding + mapping + containment still scoped separately).
- **Expected:** `cargo test --workspace` 269 passed, 1 ignored (test extended in place); fmt + host + UEFI clippy clean; gate-test 55/55; QEMU TCG x2 UEFI BOOT PASS with serial byte-identical to slice2h (2196 B).
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 269 passed (21 boot-info + 39 boot-uefi + 199 kernel incl. extended ring-0-stack test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (55/55). QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2196 bytes both (identical to slice2h @b4524ba), guest `bootx64.efi` 174592 bytes (was 174080: +512 debug-assert codegen, one PE alignment step; serial-identical so boot-neutral, same precedent as slice2g +512). Both `USER ok=1 passed=9/9`, `PFUS ok=1 us=0 w=0 p=0`, `EXC ok=1 vector=0`, `TICK/STI/IRQSTACK` green.
- **Final:** REVIEW #642 risks 2+3 closed on the current tree (risk 1 conceded: `rsp0` still unexercised with no CPL3->CPL0 switch -- no mitigation claim until stack-switch observed). Next: `iretq` frame builder (with pre-`iretq` `rsp0 != 0` assert), then user stack mapping, then user-fault containment with RSP-range check; pending agent-b review.
## S09 slice2j -- REVIEW #644: debug_assert -> release-present assert + fold theorem (board #30 remainder)

- **Decision:** Close REVIEW #644 risk 2 as one guest-touching slice: (a) `kernel/cpu.rs` guest `load_gdt`: both pre-`ltr` `debug_assert_ne!` become release-present `assert_ne!` (one-time boot cost only; a zero `top` now panics loudly in every profile instead of silently relying on wiring order in release); (b) host test extended in place (no new count) with the release-fold theorem pins: `ring0_stack_top_for(u64::MAX - 16383) == u64::MAX` (the exact-zero preimage saturates, never yields 0) and `ring0_stack_top_for(0) != 0`.
- **Why:** REVIEW #644 half-closed risk 2: `debug_assert!` compiles out in the release UEFI profile, restoring the silent-convention gun in the shipping profile. The reviewer-suggested disconfirm (release build + inspect) produced a stronger result than `assert!` alone: release `bootx64.efi` (96256 B) keeps generic panic strings (`index out of bounds...`) yet drops BOTH rsp0 messages, while debug `bootx64.efi` keeps them -- absence by optimizer proof, not by cfg-stripping, since `x.saturating_add(16384)` is never 0 for any `x` (true sum exact-nonzero or saturates to MAX). So release pays zero cost with a static guarantee, debug checks loudly at runtime.
- **Alternatives:** Explicit early-return/`panic!` instead of `assert_ne!` (rejected: identical codegen to `assert_ne!` with more lines -- the assert already panics loudly on violation); `NonZeroU64` type-state (rejected in slice2i, unchanged); separate theorem test fn (rejected: same fixture, in-place extension keeps the gate in one place); `iretq` frame in the same commit (rejected: multi-risk single cycle, unchanged).
- **Expected:** `cargo test --workspace` 269 passed, 1 ignored (test extended in place); fmt + host + UEFI clippy clean; gate-test 55/55; QEMU TCG x2 UEFI BOOT PASS, serial 2196 B both, guest debug EFI unchanged at 174592 B (assert vs debug_assert identical codegen under debug).
- **Measured (2026-10-09, Win11):** `cargo test --workspace` 269 passed (21 boot-info + 39 boot-uefi + 199 kernel incl. extended ring-0-stack test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (55/55). QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2196 bytes both, differing ONLY in the SWITCH-LAT cycle values (emulator-relative C2 timing, expected run variance); guest debug `bootx64.efi` 174592 bytes (unchanged from slice2i @34a9004). Both `USER ok=1 passed=9/9`, `PFUS ok=1 us=0 w=0 p=0`, `EXC ok=1 vector=0`, `TICK/STI/IRQSTACK` green.
- **Final:** REVIEW #644 risk 2 fully closed (risk 1 already conceded slice2i, risk 3 already closed): rsp0-nonzero is loud-in-debug via `assert!` and statically-proven in release via the saturating_add fold. #30 stays active for the iretq frame + pre-iretq `rsp0 != 0` assert + user stack + containment; pending agent-b review.
## S09 slice2k -- live TR readback proves `ltr` loaded `TSS_SEL` (board #30 remainder, REVIEW #646 suggestion)

- **Decision:** Land the TR-liveness slice as one guest-touching commit: (a) `kernel/cpu.rs`: new safe `read_tr()` (`str`, unprivileged, same safe-fn + confined-asm shape as `read_rsp`) plus `read_tr_is_safe_fn_shape` host pin (binds `fn() -> u16`, never asserts the harness TR value); (b) `kernel/uart.rs`: new `emit_tr_line`/`write_tr_line` pair (`TR ok=<0|1> sel=<0x...>` through the shared core-only formatter, COM1-direct writer reuses the exact body) plus `tr_line_pins_exact_field_order_and_fail_closed_shape` host pin (exact `TR ok=1 sel=0x28` / `TR ok=0 sel=0x0`, field order, `sel=0x` hex shape); repaired the in-progress `run_tr` stub (was an unclosed delimiter breaking the build); (c) `kernel/lib.rs`: `bring_up_cpu_and_prove_exception` emits the live proof right after `load_gdt`/`load_idt` (`tr == TSS_SEL`), import split so `write_tr_line`/`write_tick_line` stay `cfg(not(test))` while the exc/pf/pfus group stays shared; safe-wrapper pin extended with `tr: fn(bool, u16)`.
- **Why:** REVIEW #646 accepted slice2j but kept risk1 conceded: no CPL3 switch, so TSS.rsp0 wiring is boot-neutral only (x2 2196 B identical proves nothing about TR/TSS liveness). The reviewer's cheapest next disconfirm was a QEMU TR readback. `ltr` survival alone never proved `TR` holds `TSS_SEL` -- a skipped/faulted `ltr` still booted identically. The live `str` readback closes exactly that: `TR ok=0` fails loudly instead of resting on byte-identical serial.
- **Alternatives:** QEMU gdb `STR` readback without a guest line (rejected: external observation, not a fail-closed in-guest proof the e2e hard-require can pin); forced CPL3 entry expecting live rsp0 use (rejected: correct end goal but multi-risk single cycle -- iretq frame + user stack + containment unproven; TR liveness must be proven first while still ring 0); host-only `read_tr` shape without guest emission (rejected: leaves the `ltr`-fault path unproven under QEMU).
- **Expected:** `cargo test --workspace` 271 passed, 1 ignored (269 + 2 new: tr-line + read_tr shape); fmt + host + UEFI clippy clean; gate-test 55/55; QEMU UEFI BOOT PASS with `TR ok=1 sel=0x28` on serial (+17 B vs 2196 B baseline, all prior gates green).
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 271 passed (21 boot-info + 39 boot-uefi + 201 kernel incl. 2 new + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (55/55). QEMU `-accel tcg` x1 on this tree: UEFI BOOT PASS, serial 2213 bytes (2196 + 17 for `TR ok=1 sel=0x28`), guest debug `bootx64.efi` 176640 bytes (was 174592: TR/`str` codegen + PE alignment); `TR ok=1 sel=0x28` present, `USER ok=1 passed=9/9`, `PFUS ok=1 us=0 w=0 p=0`, `EXC ok=1 vector=0`, `TICK/STI/IRQSTACK` green.
- **Final:** `ltr` liveness proven live (`TR` holds `TSS_SEL` post-`load_gdt`); risk1 narrowed but still conceded (no CPL3 switch -- rsp0 still unconsumed). #30 stays active for: iretq frame, pre-iretq rsp0!=0 assert, real user stack, CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2l -- TR hard-require gate closes REVIEW #648 risk 1 (board #30 remainder)

- **Decision:** Land the e2e-only gate slice with zero guest-code change: (a) `tools/qemu/boot-uefi.ps1`: new `$trPat = "TR ok=1 [^\r\n]*sel=0x28(\s|$)"` value gate (literal space after `ok=1`, same-line sel pin, `sel=0x28(\s|$)` terminal) plus `$trFail = "TR ok=0"` forbidden-outright (`$noTrFail`), both wired into the big hard-require `if` and the FOUND/MISSING summaries; (b) `tools/qemu/test-boot-uefi-gates.ps1`: new `trPat` lock (mirrors the PFUS/RMBZ locks) plus 6 cases (T-good passes; T-ok0/T-ten/T-onex/T-badsel/T-split fail), PASS line 55/55 -> 61/61.
- **Why:** REVIEW #648 accepted slice2k but named risk 1 load-bearing: gates stayed 55/55, so a skipped/faulted `ltr` printing `ok=0` still GATE-PASSed -- fail-closed was human-reads-serial, not automated. This slice makes `TR ok=0` fail e2e on both pins (`ok=1` + `sel=0x28` miss, plus the explicit `TR ok=0` forbid).
- **Alternatives:** Pin only `TR ok=1` without the sel value (rejected: a wrong-selector boot with ok forced would pass -- sel=0x28 is the TSS_SEL contract, pinned by cpu.rs/lib.rs asserts); pin `sel=0x28` without the `TR ok=0` forbid (rejected: single-pin is enough, but the forbid is one line and mirrors the THREAD-JUMP-REFUSAL-FAIL precedent -- defense in depth); fold iretq frame into the same commit (rejected: multi-risk single cycle -- frame encoding + mapping + containment still scoped separately).
- **Expected:** `cargo test --workspace` 271 passed, 1 ignored (unchanged); fmt + host + UEFI clippy clean; gate-test 61/61; QEMU TCG x2 UEFI BOOT PASS, serial 2213 B both (identical to slice2k), guest 176640 B.
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 271 passed (21 boot-info + 39 boot-uefi + 201 kernel + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (61/61). QEMU `-accel tcg` x2 on this tree: both UEFI BOOT PASS, serial 2213 bytes both (byte-identical incl. `TR ok=1 sel=0x28`; SWITCH-LAT cycles identical this run), guest debug `bootx64.efi` 176640 bytes (unchanged); `USER ok=1 passed=9/9`, `PFUS ok=1 us=0 w=0 p=0`, `TICK/STI/IRQSTACK` green.
- **Final:** REVIEW #648 risk 1 closed on the current tree (TR-liveness is now fail-closed automated, not human-read). Risks 2-4 carried: TR!=TSS-valid and no CPL3/rsp0-use still owed; #30 stays active for iretq frame + pre-iretq rsp0!=0 assert + user stack + containment; pending agent-b review.
## S09 slice2m -- host-only iretq frame builder (board #30 remainder)

- **Decision:** Land the first CPL3-entry remainder sub-slice as host-only encoding: (a) `kernel/lib.rs`: `USER_CODE_SEL`/`USER_DATA_SEL` go `pub(crate)` (still `allow(dead_code)` -- no guest consumer yet); (b) `kernel/cpu.rs`: new `USER_RFLAGS` (0x202 = bit1 + IF, IOPL 0) + `repr(C)` `IretqFrame` (pop-order RIP/CS/RFLAGS/RSP/SS, 40 bytes) + pure const `iretq_frame_for(rip, rsp)` pinning CS=0x1B/SS=0x23/RFLAGS=0x202; all three carry `allow(dead_code)` until the guest entry stub lands; (c) new host test `iretq_frame_pins_cpl3_selectors_and_rflags` (RFLAGS bit-shape + IOPL 0 + no-other-flags, CS/SS RPL 3 + index 3/4, rip/rsp passthrough, offset_of pop-order layout). No guest change (still ring 0, no `iretq`).
- **Why:** REVIEW #650 ACCEPTED slice2l and named the carried risks: no CPL3/rsp0-use, TR!=TSS-valid, QEMU TCG-only. The next independently committable step is pinning the privilege-changing `iretq` frame encoding before any stack mapping or live entry, otherwise a wrong CS/SS (RPL 0 abort), IF-clear hang, or field-reorder typo only faults at first ring-3 entry under QEMU. Cheapest disconfirm is a host layout test, zero QEMU risk (same precedent as slice2c/2e/2f/2m pattern).
- **Alternatives:** Guest `iretq` stub + user stack in the same commit (rejected: multi-risk single cycle -- frame encoding + stack mapping + containment + pre-iretq rsp0 assert unproven at once; frame must exist first); user-stack static first without the frame (rejected: stack top has no consumer shape until the frame defines which RSP the entry path needs); negative e2e probe as its own commit (rejected as a commit: gate self-test 61/61 already covers T-ok0/T-badsel/T-split fail-closed -- re-running it is verification, not a code delta).
- **Expected:** `cargo test --workspace` 272 passed, 1 ignored (271 + 1 new); fmt + host + UEFI clippy clean; gate-test 61/61; guest bytes unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 272 passed (21 boot-info + 39 boot-uefi + 202 kernel incl. new iretq test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean (after one auto-format); `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (61/61). Guest untouched (no QEMU re-run; slice2l x2 byte-identical baseline stands: serial 2213 B, guest 176640 B).
- **Final:** CPL3 `iretq` frame encoding pinned host-side, still boot-neutral. Next: pre-iretq `rsp0 != 0` assert shape + real user stack mapping, then CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2n -- pre-iretq readiness gate + user-stack top (board #30 remainder)

- **Decision:** Land the second CPL3-entry remainder sub-slice as host-only: (a) `kernel/cpu.rs`: new `USER_STACK_LEN` (16 KiB, mirrors ring-0) + pure const `user_stack_top_for` (saturating, 16-aligned top) + pure const `iretq_ready(rsp0, rip, rsp)` (true only when all three nonzero -- fail-closed pre-`iretq` gate); `iretq_frame_for` gains `# Preconditions` docs naming `iretq_ready` + `user_stack_top_for` + canonical user-range validation as caller-owned (constructor stays infallible); (b) build-time `const _` static-asserts for `IretqFrame` size 40 + field offsets 0/8/16/24/32 (REVIEW #652 nit 1 -- layout now fails the build, not just the test); (c) new host test `user_stack_top_and_iretq_ready_pin_pre_iretq_gate` (top math + align + saturate, ready true/all-zero-refusals, frame passthrough stays pure). No guest change (still ring 0, no `iretq`/user-stack static).
- **Why:** REVIEW #652 ACCEPTED slice2m with two carried nits: build-time frame pins + undocumented rip/rsp preconditions. Cheapest next disconfirm pins the refusal shape before any live entry: a guest stub that pushes a null-rsp0/null-rip frame `iretq`s into `#SS(0)`/triple-fault, so the gate (`rsp0 != 0 && rip != 0 && rsp != 0`) must exist host-side first. Same host-only precedent as slice2m (zero QEMU risk).
- **Alternatives:** Guest user-stack static + live `iretq` in the same commit (rejected: multi-risk single cycle -- mapping + containment + entry unproven at once); `iretq_ready` returning `Result` with mapping validation now (rejected: no page-table user-range query exists yet -- that slice owns canonical/user-range checks, this one pins the nonzero shape); QEMU re-run for a host-only delta (rejected: guest bytes unchanged, slice2l x2 baseline stands).
- **Expected:** `cargo test --workspace` 273 passed, 1 ignored (272 + 1 new); fmt + host clippy clean; gate-test 61/61; guest bytes unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 273 passed (21 boot-info + 39 boot-uefi + 203 kernel incl. new ready-gate test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check -p baseos-kernel` clean; `cargo clippy -p baseos-kernel --lib --tests -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (61/61). Guest untouched (no QEMU re-run; slice2l x2 byte-identical baseline stands: serial 2213 B, guest 176640 B).
- **Final:** Pre-`iretq` nonzero gate + user-stack-top helper pinned host-side, frame layout now build-time-enforced, still boot-neutral. Next: user-range/canonical mapping validation + guest user-stack static, then CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2o -- user-entry canonical + alignment gate (board #30 remainder, REVIEW #654 risk 1)

- **Decision:** Land the third CPL3-entry remainder sub-slice as host-only: (a) `kernel/cpu.rs`: new `USER_HALF_END` (0x0000_8000_0000_0000, single-sourced user-half ceiling) + pure const `user_rip_valid` (nonzero + canonical low-half) + `user_rsp_valid` (same + `% 16 == 0` alignment) + combined `user_entry_valid`; `iretq_ready` doc now names its nonzero-only weakness explicitly (accepts kernel-half/non-canonical/misaligned) and points at `user_entry_valid` as the required second gate; `iretq_frame_for` `# Preconditions` updated to `iretq_ready PLUS user_entry_valid PLUS mapping check`; (b) new host test `user_entry_pins_canonical_and_alignment` (HALF_END value, rip null/hole/kernel/saturated refusals, rsp null/misaligned/hole/kernel refusals, saturated-top load-bearing case passes weak `iretq_ready` yet fails `user_rsp_valid`/`user_entry_valid`, combined table). No guest change (still ring 0, no `iretq`/user-stack static).
- **Why:** REVIEW #654 ACCEPTED slice2n as host-only but named risk 1 load-bearing: the nonzero gate accepts `rip=0xFFFF...k`, `rsp=u64::MAX`, misaligned RSP -- the first live `iretq` with any of those faults `#GP`/`#SS`/triple-faults. Cheapest next disconfirm pins the encodable shape (canonical + aligned) host-side before any mapping/entry slice, same host-only precedent as slice2m/2n (zero QEMU risk).
- **Alternatives:** Strengthen `iretq_ready` itself with the new checks (rejected: breaks the pinned nonzero-only contract + existing test expectations; two-gate shape `ready && entry_valid` keeps wiring-vs-encoding failures distinguishable at the future stub call site); guest user-stack `align(16)` static in the same commit (rejected: multi-risk single cycle -- REVIEW #654 risk 2 deserves its own static + base-alignment pin); page-table mapped-range query now (rejected: no user-range walker exists yet -- mapping check stays caller-owned per preconditions doc).
- **Expected:** `cargo test --workspace` 274 passed, 1 ignored (273 + 1 new); fmt + host + UEFI clippy clean; gate-test 61/61; guest bytes unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 274 passed (21 boot-info + 39 boot-uefi + 204 kernel incl. new entry-validity test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `test-boot-uefi-gates.ps1` GATE TEST PASS (61/61). Guest untouched (no QEMU re-run; slice2l x2 byte-identical baseline stands: serial 2213 B, guest 176640 B).
- **Final:** Canonical + alignment gate pinned host-side; weak-gate risk narrowed to the page-table mapping check + guest user-stack static (aligned base per REVIEW #654 risk 2), then CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2p -- mapped-range containment gate (board #30 remainder, REVIEW #656 carry 1+3)

- **Decision:** Land the fourth CPL3-entry remainder sub-slice as host-only: (a) `kernel/cpu.rs`: new pure const `user_range_contains` (half-open `[base, base+len)` via subtraction form, empty-refuses, overflow-safe) for mapped code RIP + `user_stack_top_valid` (exact one-past-end top, `rsp-base==len`, nonzero len, both ends 16-aligned, interior-pointer refuses) + combined `user_entry_mapped_valid` (single stub gate: `iretq_ready && user_entry_valid && code-contains && stack-top-valid`); `iretq_frame_for` preconditions now point at the single `user_entry_mapped_valid` gate; (b) new host test `user_entry_mapped_pins_range_containment` (multi-base top-alignment invariant across 4 aligned bases, containment edges incl. wrap-refusal, stack-top interior/misaligned/empty refusals, full-gate forget-any-half table incl. shape-valid-but-unmapped load-bearing case). No guest change (still ring 0, no `iretq`/user-stack static).
- **Why:** REVIEW #656 ACCEPTED slice2o host-only but carried risk 1 (mapping check caller-owned, zero enforcement -- stub could forget) and carry 3 (top-alignment pinned for one base only). Cheapest next disconfirm pins the containment math host-side before any walker/entry slice, same zero-QEMU-risk precedent; the single-gate shape means the future stub cannot forget a half without failing to compile-test.
- **Alternatives:** Wire the real page-table walker now (rejected: no user-range walker exists -- base/len stay caller-provided per doc, this pins the math not the walker state); guest user-stack `align(16)` static in the same commit (rejected: multi-risk single cycle -- static + base pin deserves its own slice); boolean `rip_mapped/rsp_mapped` flags instead of ranges (rejected: flags re-create the forgettable-caller problem -- range math forces the caller to name the mapping).
- **Expected:** `cargo test --workspace` 275 passed, 1 ignored (274 + 1 new); fmt + host + UEFI clippy clean; gate-test 61/61; guest bytes unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 275 passed (21 boot-info + 39 boot-uefi + 205 kernel incl. new mapped-gate test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `tools/qemu/test-boot-uefi-gates.ps1` GATE TEST PASS (61/61). Guest untouched (no QEMU re-run; slice2l x2 byte-identical baseline stands: serial 2213 B, guest 176640 B).
- **Final:** Mapping-containment math + multi-base alignment invariant pinned host-side; remainder narrowed to guest user-stack static (aligned base) + walker-sourced base/len, then CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2q -- user-entry permission gate (board #30 remainder, REVIEW #658 risk 2)

- **Decision:** Land the fifth CPL3-entry remainder sub-slice as host-only: (a) `kernel/cpu.rs`: new pure const `user_code_perms_valid` (PRESENT + USER + NOT NO_EXEC -- code must be mapped user-accessible executable; WRITABLE allowed either way, W+X hardening deferred) + `user_stack_perms_valid` (PRESENT + USER + WRITABLE + NO_EXEC -- RW data never executed, assumes EFER.NXE=1 as OVMF enables) + combined `user_entry_fully_valid` (new single stub gate: `user_entry_mapped_valid && code-perms && stack-perms`, `#[allow(clippy::too_many_arguments)]` for the 9-arg const); `iretq_frame_for` preconditions now point at `user_entry_fully_valid`; (b) new host test `user_entry_fully_pins_code_and_stack_permissions` (good RX-U code + RW-U-NX stack pass, supervisor-only/non-present/non-exec code refuse, read-only/exec/supervisor-only stack refuse, mapped-but-wrong-perms full-gate refuses incl. the load-bearing slice2p gap). No guest change (still ring 0, no `iretq`/user-stack static; flags must come from `walk_full_leaf_flags` in the future stub).
- **Why:** REVIEW #658 ACCEPTED slice2p host-only but named risk 2 load-bearing: containment != permission -- a kernel-only or non-exec range passes the mapped gate. Cheapest next disconfirm pins the permission math host-side before any walker/static/entry slice, same zero-QEMU-risk precedent; the single-gate shape means the future stub cannot forget permissions without failing the host test.
- **Alternatives:** Wire the walker binding now (rejected: base/len/flags stay caller-provided per doc -- this pins the math not the walker state, REVIEW #658 risk 1 remains for the binding slice); guest user-stack static in the same commit (rejected: multi-risk single cycle -- static + base pin deserves its own slice); forbid WRITABLE code now (rejected: guest maps its own RX code later -- W+X policy belongs with that mapping slice, not the math pin).
- **Expected:** `cargo test --workspace` 276 passed, 1 ignored (275 + 1 new); fmt + host + UEFI clippy clean; gate-test 61/61; guest bytes unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 276 passed (21 boot-info + 39 boot-uefi + 206 kernel incl. new permission-gate test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `tools/qemu/test-boot-uefi-gates.ps1` GATE TEST PASS (61/61). Guest untouched (no QEMU re-run; slice2l x2 byte-identical baseline stands: serial 2213 B, guest 176640 B).
- **Final:** Permission math (U/S + Present + RW + NX split code-vs-stack) pinned host-side; remainder narrowed to guest user-stack static (aligned base) + walker-sourced base/len/flags binding, then CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2r -- per-page + NXE-aware entry gate (board #30 remainder, REVIEW #660 risks 2+3)

- **Decision:** Land the sixth CPL3-entry remainder sub-slice as host-only: (a) `kernel/cpu.rs`: new pure const `user_stack_perms_valid_for(flags, nxe_enabled)` (NXE=1 demands PRESENT+USER+WRITABLE+NO_EXEC; NXE=0 demands PRESENT+USER+WRITABLE with NO_EXEC clear since bit 63 is MBZ per `pte_reserved_violation`) + `all_code_pages_valid` / `all_stack_pages_valid_for` (non-empty, every 4 KiB page must pass its per-page gate) + `user_entry_fully_valid_pages` (new walker-ready single gate: `user_entry_mapped_valid` + per-page gates + exact page-count coverage `len/4096 == pages.len()` + live NXE pin; nonzero 4 KiB-multiple lens only); (b) new host test `user_entry_pages_pin_per_page_and_nxe` (flags sourced via `pte_encode`->`pte_decoded_flags` plus one live `walk_full_leaf_flags` 4-level walk proving walker->gate plumbing; 4-page stack with page-3 exec refuses while single-flag gate passes; one-flag-for-4-pages refuses on count; NX stack refuses when NXE=0 and RW-without-NX degraded-accepts). No guest change (still ring 0, no `iretq`/user-stack static).
- **Why:** REVIEW #660 ACCEPTED slice2q host-only but carried 4 risks, two load-bearing here: single-flags-for-multi-page (16 KiB stack = 4 pages, code 0x2000 = 2 pages, one flags value cannot cover all pages) and NXE=1 assumption (NO_EXEC is MBZ while NXE=0, no live gate before demanding NX). Cheapest disconfirm pins both host-side before any static/walker/entry slice, same zero-QEMU-risk precedent.
- **Alternatives:** Full walker binding now (walk each page from live CR3 inside the gate -- rejected: const gate cannot take a lookup closure; slices stay the boundary -- stub walks, gate checks); fail-closed on all NXE=0 (refuse even RW-without-NX -- rejected: conflates reserved-bit refusal with policy refusal -- per-page math demands !NX when MBZ, the future stub still refuses CPL3 entry when NXE=0 via the `nxe_enabled` pin on the full gate); forbid W+X code now (rejected: deferred to the RX-mapping slice per slice2q).
- **Expected:** `cargo test --workspace` 277 passed, 1 ignored (276 + 1 new); fmt + host clippy clean; guest bytes unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 277 passed (21 boot-info + 39 boot-uefi + 207 kernel incl. new per-page/NXE test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt` clean (auto-format applied); `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0. Guest untouched (no QEMU re-run; slice2l x2 byte-identical baseline stands).
- **Final:** Per-page coverage + NXE-aware stack math pinned host-side with live walker plumbing proven; remainder narrowed to guest user-stack static (aligned base) + stub walker loop over all pages + CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2s -- fail-closed CPL3 wrapper + doc-link fix (board #30 remainder, REVIEW #662 nits a+b)

- **Decision:** Land host-only follow-up to slice2r: (a) fix broken intra-doc link `cpu.rs` (`user_entry_fully_valid_nxe` -> `user_entry_fully_valid_pages`); (b) add pure const `user_entry_cpl3_allowed(...) = nxe_enabled && user_entry_fully_valid_pages(...)` as the future stub's single CPL3-entry call site, repoint `iretq_frame_for` preconditions to it; (c) extend host test with wrapper pins (NXE=1 delegates, NXE=0 refuses even the degraded RW-without-NX shape). No guest change.
- **Why:** REVIEW #662 ACCEPTED slice2r host-only but proved two load-bearing doc/naming risks: the intra-doc link pointed at a nonexistent fn (doc drift), and the worklog claim that the stub "still refuses CPL3 entry via the nxe pin on the full gate" is false as coded -- the per-page pin degraded-ACCEPTS RW-without-NX while NXE=0. Cheapest disconfirm keeps the per-page MBZ math but adds an explicit fail-closed wrapper so degraded-accept cannot be mistaken for entry-allowed.
- **Alternatives:** Keep per-page gate as the entry gate and document "caller must also check NXE" (rejected: bypassable by forgetting, same class as REVIEW #662 risk 3); fail-closed the per-page math itself on all NXE=0 (rejected: conflates MBZ-reserved refusal with entry policy -- kept per slice2r, wrapper owns policy).
- **Expected:** `cargo test --workspace` 277 passed, 1 ignored (same count, extended test); fmt + kernel clippy clean; guest untouched.
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 277 passed (21 boot-info + 39 boot-uefi + 207 kernel incl. extended per-page/NXE/CPL3 test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0. Guest untouched (no QEMU re-run; slice2l x2 byte-identical baseline stands).
- **Final:** CPL3 entry now has a fail-closed host-side gate (`nxe_enabled && fully_valid_pages`); remainder narrowed to guest user-stack static + stub walker loop over all pages sourcing the wrapper + CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2t -- guest user-stack static (board #30 remainder, REVIEW #664 next step 1/3)

- **Decision:** Land the first guest-touching CPL3 remainder sub-slice: (a) `kernel/cpu.rs`: new guest-only `UserStack` (`repr(C, align(16))`, `[u8; USER_STACK_LEN]`, mirrors `Ring0Stack`) + `static mut USER_STACK` zero blob (unused: no mapping, no `iretq` RSP -- pure BSS growth, boot-neutral by construction); docs pin the future stub contract (map all 4 pages U+W+NX, walk each page via `walk_full_leaf_flags`, gate `iretq` on `user_entry_cpl3_allowed` with live `EFER.NXE`, never the inner degraded path alone); (b) extend host test with the 4-page invariant (`USER_STACK_LEN % 4096 == 0`, `/ 4096 == 4` -- the stub walks all four, never one flag). No serial/ps1 change, no QEMU re-run (guest bytes grow in BSS only, no behavior change; slice2l x2 baseline stands).
- **Why:** REVIEW #664 ACCEPTED slice2s host-only and named the next cheapest disconfirm: guest user-stack static + stub loop sourcing ONLY the wrapper + live NXE read. This slice lands step 1/3 (the static) with zero behavior risk; the walker loop + live-NXE stub binding rides next once the bytes exist to map.
- **Alternatives:** Stub walker loop in the same commit (rejected: multi-risk single cycle -- static + per-page walk + live NXE + mapping unproven at once; bytes must exist first); map the stack now (rejected: no page-table user-range allocator slice yet -- mapping belongs with the walker-binding slice, not BSS reservation); QEMU re-run for BSS-only growth (rejected: no code-path change, serial-identical by construction -- re-verify rides with the next behavior-changing slice).
- **Expected:** `cargo test --workspace` 277 passed, 1 ignored (same count, extended test); fmt + host + UEFI clippy clean; guest BSS-only growth.
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 277 passed (21 boot-info + 39 boot-uefi + 207 kernel incl. extended 4-page test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean (after one auto-format); `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `cargo doc` adds no new unresolved links (18 pre-existing warnings only). Guest BSS-only (no QEMU re-run; slice2l x2 byte-identical behavior baseline stands).
- **Final:** Guest user-stack bytes reserved (aligned, 4-page-pinned); remainder narrowed to stub walker loop over all 4 pages sourcing ONLY `user_entry_cpl3_allowed` + live NXE read, then CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2u -- stub walker loop + live-NXE stub, wrapper-only (board #30 remainder, REVIEW #664 step 2/3)

- **Decision:** Land the second CPL3-remainder sub-slice in `kernel/cpu.rs`: (a) `USER_STACK_PAGES = USER_STACK_LEN / 4096` + build-time `const _` pins (`% 4096 == 0`, `== 4`, with the file-local `manual_is_multiple_of` allow per precedent); (b) host-callable `walk_user_stack_pages(pml4_base, stack_base, lookup)` — `walk_full_leaf_flags` per 4 KiB page, `None` fail-closed on any walk error (never a partial array; `&lookup` borrow so the closure is reusable across the 4 walks); docs forbid caller-supplied literals (REVIEW #658 risk 1: this walk is the only producer); (c) guest-only `unsafe probe_user_stack_pages_valid(pml4_base, lookup)` — live `efer_nxe_enabled(read_efer())` + `USER_STACK` base + walk + `all_stack_pages_valid_for` wrapper-only (never inline checks, never the inner degraded path alone); uncalled (stack BSS-reserved but unmapped → would return false; no `iretq`), so boot-neutral; (d) host test with real index pins (`>>39==32`, `>>30==0`, `>>21==1`, `>>12==0` for `0x0000_1000_0020_0000` — first draft assumed PML4[0]/PT[32..35] and failed loudly on the walk, corrected to PML4[32]/PDPT[0]/PD[1]/PT[0..3]) covering all-NX allow, page-3-exec refuse, lookup-miss `None`, NXE=0 refuse.
- **Why:** REVIEW #664 step 2/3 is exactly this binding (stub loop sourcing ONLY the wrapper + live NXE read); the mapping + CPL3 entry + contained-fault proof ride next once the walker loop exists to consume them.
- **Alternatives:** Full `user_entry_cpl3_allowed` stub now (rejected: code-side pages/mapping don't exist yet — the stack-half wrapper is the honest callable unit); guest-`println`/serial proof now (rejected: unmapped stack would print `ok=0`, a confusing baseline; proof rides with the mapping slice); `==`-index fixture without pin asserts (rejected: the PML4[0] slip proved the pins load-bearing — asserts caught it host-side, not in QEMU).
- **Expected:** `cargo test --workspace` 278 passed, 1 ignored (277 + 1); fmt + host + UEFI clippy clean; `cargo doc` no new warnings; `boot-uefi.ps1` PASS with no new serial line (stub uncalled).
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 278 passed (21 boot-info + 39 boot-uefi + 208 kernel incl. 1 new + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0 (after `redundant_closure` → `&lookup` and file-local `manual_is_multiple_of` fixes); `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0; `cargo doc` 18 warnings, none new (no `walk_user_stack`/`probe_user_stack`/`USER_STACK_PAGES` link warnings); `boot-uefi.ps1` UEFI BOOT PASS exit 0, serial 2213 bytes, guest `bootx64.efi` 176640 bytes, full gate chain intact (`TR ok=1 sel=0x28`, `PFUS ok=1`, `PF ok=1 vector=14`, `EXC ok=1 vector=0`, no new lines).
- **Final:** Walker loop + live-NXE stub landed wrapper-only and proven boot-neutral end-to-end; remainder narrowed to step 3/3 (stack mapping + CPL3 entry + contained-fault proof); pending agent-b review.
## S09 slice2v -- checked_add overflow fail-closed in walk_user_stack_pages (board #30 remainder, REVIEW #666 risk 2)

- **Decision:** Harden `walk_user_stack_pages` host-side: per-page VA via `checked_add` (`stack_base + i*4096` returns `None` on overflow) instead of `wrapping_add`; extend host test with near-top base (`u64::MAX - 4096`) pinning fail-closed `None`. No guest change (stub uncalled, unmapped).
- **Why:** REVIEW #666 risk 2: near-`u64::MAX` base wrapped to low VA and walked the wrong pages. Cheapest disconfirm is the overflow-kill unit pin.
- **Alternatives:** Saturating/wrapping with extra range check (rejected: `checked_add` is the exact fail-closed shape, no second predicate to drift).
- **Expected:** `cargo test --workspace` 279 passed, 1 ignored (278 + 0 new, extended test); fmt + clippy clean; guest untouched.
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 279 passed, 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0. Guest untouched.
- **Final:** Overflow fail-closed pinned; pending agent-b review.
## S09 slice2w -- discriminating overflow-kill test + off-by-one comment fix (board #30 remainder, REVIEW #668)

- **Decision:** Strengthen the slice2v pin host-side: map the high pair AND the old wrap targets so old `wrapping_add` code would walk all four and return `Some` while `checked_add` refuses with `None` at i=2; fix the off-by-one comment (overflow at i=2, not page 1). No guest change.
- **Why:** REVIEW #668 weaknesses 1/2: the slice2v top-miss test passed even under the old wrapping code (page 0 already missed), so it could not discriminate the fix.
- **Alternatives:** Keep the weak shape test (rejected: reviewer-named discriminating test is the load-bearing proof).
- **Expected:** `cargo test --workspace` 279 passed, 1 ignored (same count, extended test); fmt + clippy clean; guest untouched.
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 279 passed, 1 ignored; `cargo fmt --all -- --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0. Guest untouched. REVIEW #670 ACCEPT.
- **Final:** Discriminating overflow-kill verified; remainder: mapping + CPL3 entry.
## S09 slice2x -- user-stack mapping flags pin, no W+X/code reuse (board #30 remainder)

- **Decision:** Land host-only in `kernel/cpu.rs`: new `USER_STACK_MAP_FLAGS = PRESENT|USER|WRITABLE|NO_EXEC` const (single-sources the flag word the future mapping slice must install on all 4 pages) + host test `user_stack_map_flags_pin_rw_nx_user_no_code_reuse` (contains all four bits; `user_stack_perms_valid` + `..._for(true)` pass, `..._for(false)` refuses (bit 63 MBZ); `user_code_perms_valid` refuses (NX set, no code-side reuse); full 4-page `all_stack_pages_valid_for(..., true)` passes, `false` refuses). No guest change (still BSS-unmapped, no `iretq`).
- **Why:** Carried REVIEW #670 remainder names mapping + CPL3 entry as next, with no W+X/code-side reuse proof: a future mapping slice could install supervisor-only, read-only, exec, or W+X code-reusable flags and still pass containment. Cheapest disconfirm pins the exact flag word host-side before any PTE is written, same zero-QEMU-risk precedent as slices 2o-2w.
- **Alternatives:** Map the stack now (rejected: no user-range page-table allocator slice yet -- flag word must be pinned first so the mapping slice has a host test to satisfy); forbid WRITABLE code in the same commit (rejected: code-side W+X policy belongs with the RX-mapping slice, this pins the stack half only); guest serial proof now (rejected: unmapped stack would print `ok=0`, confusing baseline -- proof rides with the mapping slice).
- **Expected:** `cargo test --workspace` 280 passed, 1 ignored (279 + 1 new); fmt + clippy clean; guest untouched (host-only, no QEMU re-run).
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 280 passed (21 boot-info + 39 boot-uefi + 210 kernel incl. 1 new + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0. Guest untouched (no QEMU re-run; slice2u byte-identical behavior baseline stands).
- **Final:** Stack mapping flag word pinned host-side (RW+NX+U, no code reuse); remainder narrowed to installing the 4-page U-map + CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2y -- exact-equality stack mapping gate closes REVIEW #672 risk 1 (board #30 remainder)

- **Decision:** Land host-only in `kernel/cpu.rs`: new `user_stack_map_flags_valid(flags)` const (`bits() == USER_STACK_MAP_FLAGS.bits()`, exact equality, not `contains`) + 4 extended asserts in `user_stack_map_flags_pin_rw_nx_user_no_code_reuse` (exact passes; +GUARD refuses; missing-NX refuses; empty refuses). No guest change (still BSS-unmapped, no `iretq`).
- **Why:** REVIEW #672 ACCEPT carried risk 1: the `contains`-style perms gates accept extra bits. In-model `PageFlags` has only PRESENT/WRITABLE/USER/NO_EXEC/GUARD (no HUGE/GLOBAL/ACCESSED bits exist), so exact equality is complete: any extra bit in-model is GUARD (a GUARD-flagged stack page faults on every access by construction). The future mapping stub gates on this exact word, never `contains` alone.
- **Alternatives:** `contains` + explicit `!GUARD` check (rejected: enumerates one bad bit instead of pinning the word -- exact equality refuses every deviation, including a future flag added to the model); map the stack now (rejected: no user-range allocator slice yet -- gate must be pinned first).
- **Expected:** `cargo test --workspace` 280 passed, 1 ignored (same count, extended test); fmt + clippy clean; guest untouched (host-only, no QEMU re-run).
- **Measured (2026-10-10, Win11):** `cargo test` default 240/240 (21 boot-info + 209 kernel incl. extended pin + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean (after one auto-format); `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0. Guest untouched (no QEMU re-run; slice2x boot-neutral baseline stands).
- **Final:** Mapping-word exactness pinned host-side; remainder narrowed to installing the 4-page U-map + CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2z -- per-page exact mapping gate for the stub call site (board #30 remainder, REVIEW #673 follow-up)

- **Decision:** Land host-only in `kernel/cpu.rs`: new `all_stack_map_flags_valid(pages)` const (non-empty, every page satisfies `user_stack_map_flags_valid` exact word) + 4 extended asserts in `user_stack_map_flags_pin_rw_nx_user_no_code_reuse` (exact x4 passes; empty refuses; one GUARD-tagged page refuses while `all_stack_pages_valid_for(..., true)` still accepts it -- discriminating pin). No guest change (still BSS-unmapped, no `iretq`).
- **Why:** REVIEW #673 ACCEPT carried the stub-binding risk: the mapping stub must call the exact gate per page, not `contains`. The single-flag exact gate alone leaves the call-site shape unpinned -- a stub gating one page for a 4-page stack, or gating `contains` per page, would still pass review. This slice gives the stub its exact per-page call site plus the discriminating test that fails the `contains` spelling.
- **Alternatives:** Rewire `probe_user_stack_pages_valid` / `user_entry_fully_valid_pages` to exact now (rejected: entry path still rides with the CPL3-entry slice that owns the NXE-fail-closed wrapper -- this pins the mapping call site first, same zero-QEMU-risk precedent as slices 2x-2y); map the stack now (rejected: no user-range allocator slice yet).
- **Expected:** `cargo test --workspace` 280 passed, 1 ignored (same count, extended test); fmt + clippy clean; guest untouched (host-only, no QEMU re-run).
- **Measured (2026-10-10, Win11):** `cargo test --workspace` 279 passed (21 boot-info + 39 boot-uefi + 209 kernel incl. extended pin + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0. Guest untouched (no QEMU re-run; slice2y boot-neutral baseline stands).
- **Final:** Per-page exact gate pinned host-side with discriminating GUARD test; remainder narrowed to installing the 4-page U-map + CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2aa -- per-page VA helper single-sources the 4-page walk (board #30 remainder)

- **Decision:** Land host-only in `kernel/cpu.rs`: new `user_stack_page_va(stack_base, index)` const (`None` on OOB index `>= USER_STACK_PAGES` or `checked_add` overflow) + `walk_user_stack_pages` now calls it (no inline `checked_add` copy remains) + new test `user_stack_page_va_pins_per_page_addrs_and_fail_closed` (4 consecutive 4 KiB VAs, OOB 4/100 refuses, near-top index 2 refuses, `u64::MAX` index 1 refuses). No guest behavior change (walk refactor is semantics-identical; still BSS-unmapped, no `iretq`).
- **Why:** The mapping-install slice must compute one VA per page and the walk must verify the same VAs -- two inline copies would drift (install maps page 3 at a different VA than the walk checks). Single-sourcing now keeps the future install/walk pair to one rule with zero QEMU risk, same precedent as slices 2x-2z.
- **Alternatives:** Install the 4-page U-map now (rejected: needs a user-range frame source + PTE-write path + QEMU proof in one cycle, doubles single-cycle risk; VA math must exist first so the install slice is wiring-only); leave VA math inline in the walk (rejected: that is the drift hole this closes).
- **Expected:** `cargo test --workspace` 280 passed, 1 ignored (279 + 1 new); fmt + clippy clean; guest behavior unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-11, Win11):** `cargo test --workspace` 280 passed (21 boot-info + 39 boot-uefi + 210 kernel incl. new VA test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0 (one `question_mark` fix: `match` -> `?`). Guest behavior unchanged (no QEMU re-run; slice2z boot-neutral baseline stands).
- **Final:** Per-page VA math single-sourced host-side; remainder narrowed to installing the 4-page U-map + CPL3 entry + contained-fault proof; pending agent-b review.
## S09 slice2ab -- install-readiness gate binds both stub helpers (board #30 remainder)

- **Decision:** Land host-only in `kernel/cpu.rs`: new `user_stack_map_install_ready(stack_base, pages, nxe_enabled)` const -- the future 4-page U-map install stub's single call site -- gating on live NXE + 4 KiB base alignment + `pages.len() == USER_STACK_PAGES` + `all_stack_map_flags_valid` (exact word, GUARD refuses) + every `user_stack_page_va(stack_base, i)` encodable (overflow `None` refuses) + new test `user_stack_map_install_ready_binds_both_helpers` (good 4-page pass, NXE-0/misaligned/empty/short/GUARD/near-MAX-overflow refuses). No guest PTE written yet (host-only, no `iretq`).
- **Why:** REVIEW #675 named the stub-binding gap as the cheapest disconfirm (both helpers `allow(dead_code)` unwired; refuse if the next install diff misses either call site) -- binding both into one gate now makes the future install slice wiring-only with zero QEMU risk, same precedent as slices 2x-2aa.
- **Alternatives:** Install the 4-page U-map now (rejected: needs a user-range frame source + PTE-write path + QEMU proof in one cycle, doubles single-cycle risk; the gate must exist first so the install is one call); leave helpers unwired (rejected: that is the drift hole REVIEW #675 flags).
- **Expected:** `cargo test --workspace` 281 passed, 1 ignored (280 + 1 new); fmt + clippy clean; guest behavior unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-11, Win11):** `cargo test --workspace` 281 passed (21 boot-info + 39 boot-uefi + 211 kernel incl. new install-gate test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --tests -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0 (one `manual_is_multiple_of` allow, matching neighboring gates; workspace `--all-targets` still only the known host-bin unwinding-panic noise). Guest behavior unchanged (no QEMU re-run; slice2aa boot-neutral baseline stands).
- **Final:** Install wiring pinned host-side with both helpers bound; remainder narrowed to the live PTE-write + CPL3 entry + contained-fault proof; pending agent-b review.


## S09 slice2ac -- install PTE-word encoder is the first real PTE-write call site (board #30 remainder, REVIEW #677 risk 1)

- **Decision:** Land host-only in `kernel/cpu.rs`: new `user_stack_install_ptes(stack_base, phys_base, pages, nxe_enabled)` const -- gates on `user_stack_map_install_ready` (exact-word flags + encodable VAs + live NXE + 4-page coverage + base align), then `pte_encode_checked` per page with `checked_add` phys overflow pin -- plus new test `user_stack_install_ptes_writes_real_pte_words` (good 4-word round-trip with GUARD-bit-masked pin; GUARD/missing-USER/NXE-0/misaligned-VA/misaligned-phys/short/VA-overflow/phys-overflow all `None`). No guest PT store yet (host-only, no `iretq`).
- **Why:** REVIEW #677 ACCEPT flagged the binding gap moved up one level (`install_ready` itself `allow(dead_code)` -- three unwired gates, host gates stacking with no guest effect) and demanded the next slice wire to a real PTE-write call site. This encoder IS that call site: it consumes `install_ready` into four `u64` leaf words (the exact values the guest PT store writes), so the readiness gate is no longer predicate-only. Also closes REVIEW #677 note 2 in passing (wrong-word missing-USER case now pinned at the encoder, not just helper units).
- **Alternatives:** Guest PT store + CPL3 entry + QEMU proof now (rejected: needs a user-range frame source + live PT write + `invlpg`/CR3 reload + contained-fault proof in one cycle, doubles single-cycle risk; the word must be pinned first so the store is wiring-only); `deny(dead_code)` lint gate instead of a call site (rejected: lint alone proves wiring-by-compiler-error, not the word the store writes -- encoder proves both the call and the value).
- **Expected:** `cargo test --workspace` 282 passed, 1 ignored (281 + 1 new); fmt + clippy clean both targets; guest behavior unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-11, Win11):** `cargo test --workspace` 282 passed (21 boot-info + 39 boot-uefi + 212 kernel incl. new encoder test + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean (after one auto-format); `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0. Guest behavior unchanged (no QEMU re-run; slice2ab boot-neutral baseline stands).
- **Final:** Install gate now consumed by a real PTE-word producer host-side; remainder narrowed to the guest PT store loop + CPL3 entry + contained-fault QEMU proof; pending agent-b review.

## S09 slice2ad -- guest PT store loop + PTE_ADDR_MASK fit pin (board #30 remainder, REVIEW #679 risks 1+2)

- **Decision:** Land host-only in two files: (a) `kernel/page_table.rs`: `pte_encode_checked` now also refuses aligned-but-above-`PTE_ADDR_MASK` phys (`phys & !PTE_ADDR_MASK != 0` -> `None`), so an unrepresentable base fails closed instead of truncating silently in `pte_encode`; `WalkerError::MisalignedPhys` doc widened to align-or-range; existing `review_122` test extended (`1<<52`/`0xFFFF...F000` refuse, `PTE_ADDR_MASK` itself still encodes). (b) `kernel/cpu.rs`: new `user_stack_store_ptes(pt, stack_base, ptes)` -- two-phase (validate all VAs/indices, then write) single-PT store at `page_table_index(va, 0)` per `user_stack_page_va`; refuses (leaving `pt` untouched) on VA overflow, cross-PT span (`va>>21` differs), or slot collision; plain `fn` (`&mut` cannot be `const` on stable) + new test `user_stack_store_ptes_writes_single_pt_no_partial` (good 4-word landing, cross-PT untouched, VA-overflow untouched). No guest TLB reload yet (`invlpg`/CR3), no `iretq`, no QEMU re-run.
- **Why:** REVIEW #679 ACCEPT kept #30 active with two owed items host-side: risk 1 (dead-code tower -- `install_ready` + `install_ptes` both `allow(dead_code)` with no guest consumer; demanded the PT store loop next) and risk 2 (truncation gap -- encoder checked align only, near-MAX phys test passed only via the i=1 `checked_add` overflow while the i=0 word truncated). This slice wires `install_ptes` output into a real store call site AND pins the range at the encoder, so both risks close in one committable segment with zero QEMU risk.
- **Alternatives:** Guest TLB reload + CPL3 entry + QEMU proof now (rejected: needs live PT write + `invlpg`/CR3 reload + contained-fault proof in one cycle, doubles single-cycle risk; the store must be pinned host-side first so the guest slice is wiring-only); pin the phys range only in the store slice (rejected: REVIEW #679 offered either; the encoder is the shared choke point -- walker + install paths both benefit -- while the store keeps its overflow/span guards).
- **Expected:** `cargo test --workspace` 283 passed, 1 ignored (282 + 1 new); fmt + clippy clean both targets; guest behavior unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-11, Win11):** `cargo test --workspace` 283 passed (21 boot-info + 39 boot-uefi + 213 kernel incl. new store test + extended encode-checked asserts + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean (after one auto-format); `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0. Guest behavior unchanged (no QEMU re-run; slice2ac boot-neutral baseline stands).
- **Final:** Store loop consumes the encoder words host-side; encoder range gap closed at the shared choke point; remainder narrowed to guest TLB reload + CPL3 entry + contained-fault QEMU proof; pending agent-b review.

## S09 slice2af -- privatize raw install/store helpers behind bound entry (board #30 remainder, REVIEW #683 closure (a))

- **Decision:** Land host-only in `kernel/cpu.rs`: `user_stack_install_ptes` and `user_stack_store_ptes` drop `pub(crate)` to private `fn` (same-module only: the bound `user_stack_install_store_ptes` entry plus in-module tests); bound entry stays the SOLE `pub(crate)` cross-module path. Docs updated on all three (no behavior change, no new tests). Closes REVIEW #683 closure option (a) -- convention becomes enforcement for any future cross-module caller.
- **Why:** REVIEW #683 ACCEPTED slice2ae host-only but named the new residual: closure was by convention, not enforcement -- both raw helpers stayed `pub(crate)` and the store never re-checks phys/flags/NXE, so any in-crate caller could bypass the bound entry with tampered words (the slice2ae test itself proves the raw store installs a tampered word). Privatizing is the cheapest enforceable close: zero runtime cost, zero guest risk, and the existing bound-entry test (`user_stack_install_store_binds_encode_no_mismatched_words`) already pins the no-mismatch property.
- **Alternatives:** Store-side re-encode verify, i.e. option (b) (rejected: the store takes only `(pt, base, ptes)` -- re-checking phys/flags/NXE needs a new signature or a second encode pass per store, runtime cost for a property the single-entry design already guarantees); wire the bound entry to the real boot path now (rejected: needs live PT + `invlpg`/CR3 reload + CPL3 entry + QEMU proof in one cycle, doubles single-cycle risk; the entry must be the sole path first so the guest slice is wiring-only).
- **Expected:** `cargo test --workspace` 284 passed, 1 ignored (unchanged, no new tests); fmt + clippy clean; guest behavior unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-11, Win11):** `cargo test --workspace` 284 passed (21 boot-info + 39 boot-uefi + 214 kernel + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0 (two doc-header rewrites to avoid `doc_lazy_continuation` list-item parse on a leading `+`). Guest behavior unchanged (no QEMU re-run; slice2ae boot-neutral baseline stands).
- **Final:** Bound entry is now the enforced sole cross-module install path host-side; remainder narrowed to guest TLB reload + CPL3 entry + contained-fault QEMU proof; pending agent-b review.

## S09 slice2ag -- TLB flush VA set pins invlpg loop to store agreement (board #30 remainder, REVIEW #685 residual c)

- **Decision:** Land host-only in `kernel/cpu.rs`: new `user_stack_flush_vas(stack_base)` const returning the exact 4 per-page VAs (via `user_stack_page_va`) the guest `invlpg` loop must flush, plus test `user_stack_flush_vas_pins_invlpg_set_and_store_agreement` (per-page VA agreement, store-true-has-flush / cross-PT-overflow-has-none). No guest `invlpg` yet.
- **Why:** REVIEW #685 residual (c) demanded the flush set be pinned to store agreement before the guest loop lands, so the loop is wiring-only.
- **Alternatives:** Guest `invlpg` loop now (rejected: needs live PT + TLB op + QEMU proof in one cycle; the VA set must exist first).
- **Expected:** `cargo test --workspace` 285 passed, 1 ignored; fmt + clippy clean; guest behavior unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-11, Win11):** `cargo test --workspace` 285 passed, 1 ignored; `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0. Guest behavior unchanged (no QEMU re-run).
- **Final:** Flush VA set pinned host-side; remainder narrowed to guest `invlpg` loop + CPL3 entry + QEMU proof; REVIEW #688 ACCEPT.

## S09 slice2ah -- single-PT VA helper closes REVIEW #686 residuals 1-3 (board #30 remainder)

- **Decision:** Land host-only in `kernel/cpu.rs`: new `user_stack_single_pt_vas(stack_base)` const (single source for the span rule, `PAGES==0` guard, documented dead-`None`/unreachable-collision proof); store + flush consume it; flush test extended (helper equality + distinct slots). No guest `invlpg` yet.
- **Why:** REVIEW #686 residuals 1-3 (three near-copies of the VA-loop + span check; wrapper-identity tautology noted as weak-but-sufficient drift pin).
- **Alternatives:** Guest `invlpg` loop now (rejected: span rule must be single-sourced first so the loop cannot drift from the store).
- **Expected:** `cargo test --workspace` 285 passed, 1 ignored; fmt + clippy clean; guest behavior unchanged (host-only, no QEMU re-run).
- **Measured (2026-10-11, Win11):** `cargo test --workspace` 285 passed, 1 ignored; `cargo fmt --check` clean; kernel + host clippy clean (boot-uefi pre-existing panic-abort limit noted). Guest behavior unchanged (no QEMU re-run).
- **Final:** Span rule single-sourced host-side; remainder narrowed to guest `invlpg` loop + CPL3 entry + QEMU proof; REVIEW #688 ACCEPT with one nit (508-boundary edge) carried to next slice.

## S09 slice2ai -- guest invlpg primitive + flush loop + 508/509 boundary pin (board #30 remainder, REVIEW #688 nit 2)

- **Decision:** Land host-only-behavior in `kernel/cpu.rs`: new `invlpg_page(va)` unsafe (guest `invlpg [{0}]`, host no-op stub like `cli`/`sti`) + new `user_stack_flush_tlb(stack_base)` unsafe loop over `user_stack_flush_vas` (false when the set refuses — refused store wrote nothing); extended `user_stack_flush_vas_pins_invlpg_set_and_store_agreement` with the PT-boundary edge (base PT-index 508 `Some`/store-ok/flush-true vs 509 `None`/store-untouched/flush-false). Uncalled this slice, so boot-neutral (no QEMU re-run).
- **Why:** REVIEW #688 ACCEPT kept #30 active with guest `invlpg` loop unwired plus nit 2 (distinct-slots proof covered one base, not the 508/509 span-refuse edge). This slice wires the VA set to a real TLB op call site host-pinned and closes the nit in one committable segment.
- **Alternatives:** CPL3 entry + QEMU proof now (rejected: needs live PT write + ring-3 entry + contained-fault proof in one cycle, doubles single-cycle risk; the flush loop must be pinned first so the guest slice is wiring-only); boundary test only with no primitive (rejected: leaves the invlpg loop unwired one more cycle — the primitive is zero-risk uncalled).
- **Expected:** `cargo test --workspace` 285 passed, 1 ignored (extended test, no new count); fmt + clippy clean both targets; guest behavior unchanged (uncalled, no QEMU re-run).
- **Measured (2026-10-11, Win11):** `cargo test --workspace` 285 passed, 1 ignored; `cargo fmt --check` clean; `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0. Guest behavior unchanged (no QEMU re-run; slice2ah boot-neutral baseline stands).
- **Final:** Flush loop consumes the shared VA set host-side (guest `invlpg` real, host no-op); 508/509 boundary pinned; remainder narrowed to CPL3 entry + QEMU proof; pending agent-b review.

## S09 slice2aj -- bound store+flush + invlpg scope/ordering pins (board #30 remainder, REVIEW #690)

- **Decision:** Land host-only in `kernel/cpu.rs`: (a) `invlpg_page` `pub` -> `pub(crate)` on both cfg branches (guest + host stub), closing the widened-unsafe-surface nit -- only `user_stack_flush_tlb` (same module) and in-module tests may call it; (b) ordering doc on the `invlpg` asm (omits `nomem` on purpose, so the compiler treats it as an unpredictable memory clobber and will not reorder the preceding PT `u64` stores across it; single-CPU bring-up needs no extra fence, multicore IPI deferred); (c) new `user_stack_install_store_flush` unsafe bound entry (SOLE cross-module path): private `user_stack_install_store_ptes` + private `user_stack_flush_tlb` in one call, so callers cannot write PTEs and skip the flush (stale-TLB gap was test-only agreement); store-refuse returns false with `pt` untouched and no flush, store-success implies the VA set holds so flush returns true. Extended `user_stack_flush_vas_pins_invlpg_set_and_store_agreement` with bound-entry asserts (508 accept stores+flushes with words equal to the raw-store baseline; 509 refuses with `pt` untouched). Uncalled this slice, so boot-neutral (no QEMU re-run).
- **Why:** REVIEW #690 ACCEPTED slice2ai host-only but carried three items: (1) guest-untested (no uefi-target evidence), (2) forget-to-flush (store+flush agreement test-only, caller can skip flush), (3) nits (pub surface + missing memory-ordering note). This slice closes (2) by enforcement (same privatize-behind-bound-entry precedent as slice2af) and both (3) nits directly, plus gives (1) its cheapest host-side disconfirm (uefi-target clippy compiles the guest `invlpg` asm). CPL3 entry + QEMU proof stay owed.
- **Alternatives:** Guest CPL3 entry + QEMU proof now (rejected: needs live PT write + ring-3 entry + contained-fault proof in one cycle, doubles single-cycle risk; the flush obligation must be enforced first so the guest slice is wiring-only); docs-only call-site obligation without a bound entry (rejected: convention, not enforcement -- slice2af already proved convention insufficient for the mismatched-words gap); `mfence`/extra fence in the flush loop (rejected: `invlpg` + single-CPU bring-up needs no CPU fence beyond the compiler clobber; IPI broadcast rides with a multicore slice).
- **Expected:** `cargo test --workspace` 285 passed, 1 ignored (extended test, no new count); fmt + clippy clean both targets; guest behavior unchanged (uncalled, no QEMU re-run).
- **Measured (2026-10-11, Win11):** `cargo test --workspace` 285 passed (21 boot-info + 39 boot-uefi + 215 kernel incl. extended bound-entry asserts + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean (after one auto-format); `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0 (guest `invlpg` asm compiles on the uefi target). Guest behavior unchanged (no QEMU re-run; slice2ai boot-neutral baseline stands).
- **Final:** Store+flush obligation enforced host-side behind one bound entry; invlpg scope narrowed and ordering documented; remainder narrowed to CPL3 entry + QEMU proof; pending agent-b review.

## S09 slice2ak -- PT-liveness contract + dirty-PT no-partial pin (board #30 remainder, REVIEW #692 residual 2)

- **Decision:** Land host-only in `kernel/cpu.rs`: (a) PT-liveness Safety pin on `user_stack_install_store_flush` -- the bound entry pins the SAME `stack_base` for store+flush but NOT that `pt` is the live PT; `pt` must be the live 4 KiB PT backing `stack_base`'s 2 MiB region (same shape as `invlpg_page`'s VA-ownership rule; a non-live `pt` writes dead bytes but the guest `invlpg` still runs on accept, so the future wiring slice must pass the live PT); (b) dirty-PT no-partial asserts in `user_stack_flush_vas_pins_invlpg_set_and_store_agreement` -- 509-refuse leaves a pre-dirtied `0xA5...` PT (bound entry) and `0x5A...` PT (raw store) bit-identical, not just zeroed-PT untouched.
- **Why:** REVIEW #692 ACCEPTED slice2aj host-only but carried residual 2 (PT-liveness still caller contract -- bound pins same stack_base for store+flush but not that pt is the live PT; document at call site when wired). Cheapest close is documenting the contract on the sole entry now plus strengthening the untouched proof to a dirty PT (zeroed-PT untouched could hide a write-then-clear shape).
- **Alternatives:** Guest CPL3 entry + QEMU proof now (rejected: needs live PT write + ring-3 entry + contained-fault proof in one cycle; the liveness contract must be pinned first so the guest slice is wiring-only); live-PT enforcement in code (rejected: no live-CR3/walker handle at this call site -- contract, same as VA-ownership, is the correct enforcement level).
- **Expected:** `cargo test --workspace` 285 passed, 1 ignored (extended test, no new count); fmt + clippy clean both targets; guest behavior unchanged (uncalled, no QEMU re-run).
- **Measured (2026-10-11, Win11):** `cargo test --workspace` 285 passed (21 boot-info + 39 boot-uefi + 215 kernel incl. dirty-PT asserts + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean (after one auto-format); `cargo clippy --workspace --lib --tests` clean; `cargo clippy -p baseos-kernel --target x86_64-unknown-uefi` clean (guest `invlpg` asm compiles on the uefi target). Guest behavior unchanged (no QEMU re-run; slice2aj boot-neutral baseline stands).
- **Final:** PT-liveness pinned as caller contract on the sole entry; no-partial holds for dirty PTs; remainder narrowed to CPL3 entry + QEMU proof; pending agent-b review.

## S09 slice2al -- dirty-PT no-partial for encode-refuse shapes (board #30 remainder, REVIEW #694 risk 3)

- **Decision:** Land host-only in `kernel/cpu.rs`: extend `user_stack_install_store_binds_encode_no_mismatched_words` so every encode-refuse shape (GUARD flags, NXE=0, misaligned phys, VA-overflow) leaves a pre-dirtied `0xA5...` PT bit-identical via `user_stack_install_store_ptes`, not just a zeroed PT. Slice2ak covered only the 509 span-refuse; this closes the remaining zeroed-only gap.
- **Why:** REVIEW #694 ACCEPTED slice2ak host-only but named risk 3 (dirty-pin covers only 509-refuse; other refuse shapes still zeroed-only and could hide a write-then-clear shape). Cheapest close is reusing the existing dirtied-PT pattern on the encode path, which is safe (returns before any store write) and keeps the guest slice wiring-only.
- **Alternatives:** Guest CPL3 entry + QEMU proof now (rejected: needs live PT + ring-3 entry + contained-fault proof in one cycle; the no-partial property must hold on all shapes first); live-PT enforcement in code (rejected: no live-CR3 handle at this call site -- contract level, same as slice2ak).
- **Expected:** `cargo test --workspace` 285 passed, 1 ignored (extended test, no new count); fmt + clippy clean both targets; guest behavior unchanged (uncalled, no QEMU re-run).
- **Measured (2026-10-11, Win11):** `cargo test --workspace` 285 passed (21 boot-info + 39 boot-uefi + 215 kernel incl. dirty-PT encode-refuse asserts + 4 + 4 serial-marker + 2 version), 1 ignored; `cargo fmt --check` clean (after one auto-format); `cargo clippy -p baseos-kernel --all-targets -- -D warnings` exit 0; `cargo clippy -p baseos-boot-uefi --target x86_64-unknown-uefi -- -D warnings` exit 0. Guest behavior unchanged (no QEMU re-run; slice2ak boot-neutral baseline stands).
- **Final:** No-partial holds for dirty PTs on all refuse shapes; remainder narrowed to CPL3 entry + live-PT wiring + QEMU proof; pending agent-b review.
