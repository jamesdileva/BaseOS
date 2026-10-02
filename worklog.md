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
