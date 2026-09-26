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
