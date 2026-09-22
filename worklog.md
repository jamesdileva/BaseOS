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

## S01 slice 3 � boot-info handoff struct contract (2026-09-20)

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
