#Requires -Version 5.1
<#
.SYNOPSIS
  S01 UEFI guest boot: builds baseos-boot-uefi for x86_64-unknown-uefi,
  stages it as EFI/BOOT/BOOTX64.EFI on a QEMU virtual-FAT ESP, boots OVMF
  with serial-to-file capture, and asserts the S01 guest marker
  (BASEOS-S01-BOOT-MARKER, see crates/serial-marker/src/lib.rs) is present.
  Exits 0 on PASS, non-zero on FAIL (no false-positive passes).

  Unlike tools/qemu/smoke.ps1 (SeaBIOS, no guest), the marker assert here is
  REQUIRED: a missing marker means the guest never ran. OVMF firmware is
  likewise required (fail fast when absent).
#>
param(
  [int]$TimeoutSec = 20,
  [string]$LogDir = "target",
  [ValidateSet("tcg","whpx")][string]$Accel = "tcg"
)
$ErrorActionPreference = "Stop"

Write-Output "=== BaseOS UEFI guest boot (S07 slice63: C2 emulator compare, accel=$Accel) ==="
Write-Output "Accelerator: $Accel (SWITCH-LAT cycles are emulator-relative, C2; TCG default, WHPX opt-in)"

# --- 1. Build the guest (debug; .efi artifact, never committed) ---
cargo build -p baseos-boot-uefi --target x86_64-unknown-uefi
if ($LASTEXITCODE -ne 0) {
  Write-Output "GUEST BUILD FAIL (cargo build exit $LASTEXITCODE)"
  exit 1
}
$efi = Join-Path $PSScriptRoot "../../target/x86_64-unknown-uefi/debug/bootx64.efi"
$efi = [System.IO.Path]::GetFullPath($efi)
if (-not (Test-Path -LiteralPath $efi)) {
  Write-Output "GUEST BUILD FAIL (missing artifact $efi)"
  exit 1
}
Write-Output ("Guest: " + $efi + " (" + (Get-Item -LiteralPath $efi).Length + " bytes)")

# --- 2. Stage the ESP (gitignored target/, local-only) ---
$esp = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../../target/esp"))
$bootdir = Join-Path $esp "EFI/BOOT"
New-Item -ItemType Directory -Force -Path $bootdir | Out-Null
Copy-Item -LiteralPath $efi -Destination (Join-Path $bootdir "BOOTX64.EFI") -Force
Write-Output "ESP staged at $bootdir\BOOTX64.EFI"

# --- 3. OVMF/EDK2 firmware (required for S01: fail fast when absent) ---
function Find-OvmfCode {
  $candidates = @()
  if ($env:OVMF_CODE) { $candidates += $env:OVMF_CODE }
  try {
    $qemuExe = (Get-Command "qemu-system-x86_64" -ErrorAction Stop).Source
    $qemuBin = Split-Path $qemuExe
    $qemuShare = Join-Path (Split-Path $qemuBin) "share/qemu"
    $candidates += (Join-Path $qemuShare "edk2-x86_64-code.fd")
    $candidates += (Join-Path $qemuShare "OVMF_CODE.fd")
  } catch { }
  $candidates += @(
    "/usr/share/OVMF/OVMF_CODE.fd",
    "/usr/share/edk2/x86_64/OVMF_CODE.fd",
    "/usr/share/qemu/edk2-x86_64-code.fd",
    "C:\Program Files\qemu\share\edk2-x86_64-code.fd"
  )
  foreach ($c in $candidates) {
    if ($c -and (Test-Path -LiteralPath $c)) { return $c }
  }
  return $null
}

$ovmf = Find-OvmfCode
if (-not $ovmf) {
  Write-Output "OVMF: MISSING (S01 UEFI boot requires edk2-x86_64-code.fd; set OVMF_CODE env var)"
  exit 1
}
Write-Output "OVMF: FOUND at $ovmf"

# --- 4. Serial-to-file capture setup (log dir first: vars live here too) ---
$logAbs = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../../$LogDir"))
New-Item -ItemType Directory -Force -Path $logAbs | Out-Null
$serialLog = Join-Path $logAbs "qemu-uefi-serial.log"
$stdoutLog = Join-Path $logAbs "qemu-uefi-stdout.log"
foreach ($f in @($serialLog, $stdoutLog)) {
  if (Test-Path -LiteralPath $f) { Remove-Item -LiteralPath $f -Force }
}
New-Item -ItemType File -Force -Path $serialLog | Out-Null

# --- 5. OVMF vars store (writable NVRAM, gitignored target/, local-only) ---
# OVMF runs from split flash images: code (readonly) + vars (writable).
# Regenerated every run (review #17 finding 1): a reused vars store carries
# stale NVRAM boot-order across runs, which can mask ESP/boot-entry changes.
# A blank 528 KiB store is enough — EDK2 reformats it on first boot.
$varsFd = Join-Path $logAbs "ovmf-vars.fd"
if (Test-Path -LiteralPath $varsFd) { Remove-Item -LiteralPath $varsFd -Force }
[System.IO.File]::WriteAllBytes($varsFd, [byte[]]::new(540672))
Write-Output "OVMF vars: $varsFd"

# QEMU virtual-FAT drive: the ESP directory is exposed as a disk, so no
# FAT-image tooling (mtools) is needed on the Windows host. OVMF boots via
# pflash (its standard path: code readonly + vars writable) — `-bios` cannot
# load the 3.5 MB code image ("could not load PC BIOS"). Forward slashes
# keep backslashes (and the drive-letter colon) unambiguous to QEMU's option
# parser.
$espFat = $esp -replace "\\", "/"
$ovmfQemu = $ovmf -replace "\\", "/"
$varsQemu = $varsFd -replace "\\", "/"
$serialQemu = $serialLog -replace "\\", "/"
$qemuArgs = @(
  "-display", "none",
  "-accel", $Accel,
  "-m", "256",
  "-drive", ("if=pflash,format=raw,unit=0,file=" + $ovmfQemu + ",readonly=on"),
  "-drive", ("if=pflash,format=raw,unit=1,file=" + $varsQemu),
  "-drive", ("file=fat:rw:" + $espFat + ",format=raw"),
  "-serial", ("file:" + $serialQemu),
  "-no-reboot"
)
Write-Output ("QEMU args: qemu-system-x86_64 " + ($qemuArgs -join " "))

$proc = Start-Process -FilePath "qemu-system-x86_64" -ArgumentList $qemuArgs `
  -PassThru -NoNewWindow `
  -RedirectStandardOutput $stdoutLog -RedirectStandardError ($stdoutLog + ".err")
Start-Sleep -Seconds $TimeoutSec

if (-not $proc.HasExited) {
  Stop-Process -Id $proc.Id -Force
  $proc.WaitForExit(5000) | Out-Null
}

$size = 0
if (Test-Path -LiteralPath $serialLog) { $size = (Get-Item -LiteralPath $serialLog).Length }
Write-Output "Serial log: $serialLog (${size} bytes)"

# Must match crates/serial-marker/src/lib.rs::SERIAL_MARKER (see also
# tests/serial_marker.rs sync test); literal here because PS cannot
# import the Rust const without a build step.
$marker = "BASEOS-S01-BOOT-MARKER"
$serialText = ""
if (Test-Path -LiteralPath $serialLog) {
  $serialText = (Get-Content -LiteralPath $serialLog -Raw -ErrorAction SilentlyContinue)
}
if ($serialText -and $serialText.Contains($marker)) {
  Write-Output "Serial marker: FOUND ($marker)"
  $bootinfo = "BOOTINFO valid=1"
  $memmap = "MEMMAP ok=1"
  $notrunc = "truncated=0"
  # #59a/#7: budget baseline fields sit on the MEMMAP line after truncated;
  # hard-require them so a regression that drops the fields fails e2e.
  $mapsize = "map_size="
  $descsize = "desc_size="
  $fb = "FB present=1"
  $gop = "GOP ok=1"
  $ebs = "EBS ok=1"
  $kernel = "KERNEL ok=1"
  $framealloc = "FRAMEALLOC ok=1"
  $paging = "PAGING ok=1"
  $perm = "PERM ok=1"
  # S04 slice 25 (REVIEW #168 nits 1+2): GUARD gate is a same-line regex
  # `GUARD ok=1 [^\r\n]*passed=7/7(\s|$)` — the literal space after `ok=1`
  # stops `ok=10`/`ok=1x` false-passing via the `ok=1` substring (same class
  # as #158 finding 1 / #162 nit for PERMAUDIT W), pinning `passed=7/7`
  # stops a future coverage shrink (e.g. 6/6) passing on `ok=1` alone, and
  # `[^\r\n]*` keeps the pin same-line so a newline-split `ok=1` / passed
  # pair fails. See tools/qemu/test-boot-uefi-gates.ps1 for the control.
  $guardPat = "GUARD ok=1 [^\r\n]*passed=7/7(\s|$)"
  # S09 slice1 (REVIEW #620 containment definition): USER gate mirrors the
  # GUARD gate — `USER ok=1 [^\r\n]*passed=9/9(\s|$)` pins the U/S-isolation
  # pin (user-ok / NX-refuse / kernel-refuse / invalid-contained) plus the
  # coverage count, same-line so a newline-split pair fails. Slice2a
  # (REVIEW #622 risk 1) grows 7/7 to 9/9 with the NX readable-but-no-exec
  # pair. See tools/qemu/test-boot-uefi-gates.ps1 for the control.
  $userPat = "USER ok=1 [^\r\n]*passed=9/9(\s|$)"
  $permaudit = "PERMAUDIT ok=1"
  # REVIEW #153 gap (2): `ok` is walk-success, independent of `w`, so a w=0
  # regression still matched the substring above. Hard-require the W bit on
  # the same line via regex (slice19 gate tightening; baseline OVMF is w=1).
  # Slice 22 tightens `w=1` with `(\s|$)` so `w=10` no longer matches
  # (REVIEW #158 finding 1); see tools/qemu/test-boot-uefi-gates.ps1.
  $permauditWPat = "PERMAUDIT ok=1 [^\r\n]* w=1(\s|$)"
  # S04 slice 27 (#147r4 user/nx report-only closeout): `user`/`nx` stay
  # value-free by design (baseline OVMF is user=0 nx=0, but a future NX/SMEP
  # policy may flip nx/user -- do NOT pin 0). Hard-require the field SHAPE
  # same-line instead so a dropped/truncated/multi-char field fails e2e:
  # ` user=[01] ` (literal trailing space = single-char boundary, consumed
  # once) then `[^\r\n]*nx=[01](\s|$)` (space before w=1 or line end).
  # S04 slice 28 (#182 nit2 carried): `user=[01]` now uses `[ \t]`
  # instead of the slice-27 literal space. `[ \t]` pins the same
  # single-char boundary (space/tab, same-line) without assuming only
  # spaces; `(\s|$)` was rejected mid-line here because `\s` matches
  # `\n` and let a newline-split `user`/`nx` pair false-pass (caught by
  # the gate test this slice). `nx` keeps `(\s|$)` because it sits
  # before ` w=` or line-end, consistent with `w=1(\s|$)`.
  # VALUE/boundary policy (REVIEW #184 nit2, one place): PERMAUDIT lines
  # assume field order phys/user/nx/w. VALUES stay value-free ([01] shape,
  # never pinned 0) so a future NX/SMEP policy flipping user/nx still
  # passes shape. BOUNDARIES are trailing-class choices per position:
  # mid-line `user` uses `[ \t]` (single same-line blank: space/tab,
  # consumed once) because `(\s|$)` matches `\n` and false-passed the
  # newline-split pair; line-terminal `nx`/`w` use `(\s|$)` (space or
  # line-end) since a field or EOL always follows. Same rule mirrors to
  # PERMAUDIT2.
  $permauditUNPat = "PERMAUDIT ok=1 [^\r\n]* user=[01][ \t][^\r\n]*nx=[01](\s|$)"
  # S04 slice 20 (REVIEW #147 risk 1 multi-probe): second low-RAM probe on
  # its own line. Slice 21 gates `w=1` same-line (baseline measured w=1 on
  # OVMF in slice 20); `user`/`nx` stay reporting-only by design.
  # Slice 22 tightens both W patterns with `(\s|$)` (finding 1).
  $permaudit2 = "PERMAUDIT2 ok=1"
  $permaudit2WPat = "PERMAUDIT2 ok=1 [^\r\n]* w=1(\s|$)"
  # Slice 27 mirror for the low-RAM probe (same shape gate, same rationale).
  $permaudit2UNPat = "PERMAUDIT2 ok=1 [^\r\n]* user=[01][ \t][^\r\n]*nx=[01](\s|$)"
  # S04 slice 36 viol-enforcement policy (fail-closed, #139 note 1 RMBZ):
  # `RMBZ ok=1 [^\r\n]*viol=0(\s|$)` mirrors the GUARD + W gates — the
  # literal space after `ok=1` stops `ok=10`/`ok=1x` false-passing,
  # same-line `[^\r\n]*` stops a newline-split `ok=1` / viol pair, and
  # `viol=0(\s|$)` pins the count at zero so any viol>0 fails e2e.
  # Value gate (not value-free): reserved violations must always be zero,
  # unlike user/nx which stay shape-only for the future NX/SMEP flip.
  # Defense in depth with the guest: probe_rmbz forces ok=0 on viol>0 via
  # rmbz_audit_ok, so a viol>0 boot fails both the ok=1 and viol=0 pins.
  # `nxen`/`walks` ride along reporting-only (not gated) this slice.
  $rmbzPat = "RMBZ ok=1 [^\r\n]*viol=0(\s|$)"
  $thread = "THREAD ok=1"
  $threadRun = "THREAD-RUN id=2"
  $threadCs = "THREAD-CS ok=1 iter=16"
  $threadSave = "THREAD-SAVE ok=1"
  $threadPong = "THREAD-PONG ok=1"
  $threadPongRun = "THREAD-PONG-RUN id=1"
  $schedSwitch = "SCHED-SWITCH ok=1 from=1 to=2"
  # S07 slice58 (QEMU count=16 confirm, REVIEW #554 risk 1): the N-loop emits
  # 16 timed hops per boot, so e2e must pin the population count, not just
  # presence -- `count=16` fails if sampling regresses to n=1 or balloons.
  # `valid=0` rides along (no tail yet until N grows toward 1000).
  $switchLat = "SWITCH-LAT ok=1 count=16"
  $threadJump = "THREAD-JUMP ok=1 id=2"
  $threadJumpRun = "THREAD-JUMP-RUN id=2"
  # S06 slice22 R3 (REVIEW #368 fail-loud refusal): the forced-refusal demo
  # must prove the gates refused -- require the `THREAD-JUMP ok=0 id=0`
  # refusal shape AND forbid the `THREAD-JUMP-REFUSAL-FAIL` gate-accept
  # marker. The old demo emitted ok=0 on both branches (tautology), so a
  # regressed gate still printed ok=0 and passed; now accept emits only
  # the FAIL marker (no ok=0) and skips the live jump (no ok=1 either).
  $threadJumpRefusal = "THREAD-JUMP ok=0 id=0"
  $refusalFail = "THREAD-JUMP-REFUSAL-FAIL"
  # S06 slice26b (multi-hop 1->2->3->1 live jmp chain): each live `jmp`
  # landing prints its hop before resolving the next leg, so the serial
  # must carry hop=2 then hop=3 then hop=1 IN ORDER plus the terminal
  # `THREAD-JUMP-CHAIN ok=1 chain=1231` line. Order matters: a 1->3->2
  # scramble must fail even though all three hop lines are present.
  $chainHop2 = "THREAD-JUMP-CHAIN hop=2"
  $chainHop3 = "THREAD-JUMP-CHAIN hop=3"
  $chainHop1 = "THREAD-JUMP-CHAIN hop=1"
  $chainOk = "THREAD-JUMP-CHAIN ok=1 chain=1231"
  # S06 slice27b (live-carry cursor + guest join/exit): the static cursor
  # must prove 3 carried legs with no fresh replay, and the join/exit
  # probe must prove exit_current + join live (REVIEW #421 gaps 1+2).
  $chainCursor = "CHAIN-CURSOR ok=1 legs=3"
  $schedExit = "SCHED-EXIT ok=1 exited=2 resumed=3"
  $schedJoin = "SCHED-JOIN ok=1 joined=2"
  $schedStat = "SCHED-STAT ok=1 total=3 r1=1 r2=1 r3=1"
  $schedFair = "SCHED-FAIR ok=1 total=30 r1=10 r2=10 r3=10 spread=0"
  $schedTrace = "SCHED-TRACE ok=1 n=3 t0=2 t1=3 t2=1"
  $schedTraceX = "SCHED-TRACE-X ok=1 n=2 t0=2 t1=3"
   $schedTick = "SCHED-TICK ok=1 q=4 used=4 expired=1 next=3"
   # S07 slice14 (timer IRQ0 enable): PIC remap + PIT 100Hz + sti must deliver
   # the first tick before the #PF/#DE chain. Same-line shape mirrors the
   # GUARD gate (literal space after ok=1 stops ok=10 false-passing).
   $tickPat = "TICK ok=1 [^\r\n]*ticks=[1-9][0-9]*(\s|$)"
  # S08 slice2 (preemption probe): the sti window must prove BOTH halves of
  # the sti_window_ready gate - cumulative ticks AND preempted-workload
  # progress - on one line. Same-line shape mirrors the TICK gate (literal
  # space after ok=1 stops ok=10 false-passing, [1-9] stops ticks=0/work=0).
  # S08 slice3 (trace capture): the used hlt-spin count rides on the same
  # line so a parameter change is comparable before/after; spins=[0-9]+
  # requires the field present while tolerating a 0-spin immediate wakeup.
  # S08 slice4 (knob report, REVIEW #605 note 1): batch/target ride on the
  # same line because spins couples to work when work-bound - both required
  # so a tick-bound vs work-bound change stays legible.
  $stiPat = "STI ok=1 [^\r\n]*ticks=[1-9][0-9]*[^\r\n]*work=[1-9][0-9]*[^\r\n]*spins=[0-9]+[^\r\n]*batch=[0-9]+[^\r\n]*target=[0-9]+(\s|$)"
  # S07 slice68 (nit 3 RSP-switch proof): the IRQ body runs on the dedicated
  # IRQ stack while dispatch stays on the task stack. Same-line regex mirrors
  # the PF gate: literal space after ok=1 stops ok=10 false-passing,
  # [^\r\n]* keeps both hex fields same-line, (\s|$) terminates the irq field.
  $irqstackPat = "IRQSTACK ok=1 [^\r\n]*task=0x[0-9a-f]+[^\r\n]*irq=0x[0-9a-f]+(\s|$)"
   $exc = "EXC ok=1 vector=0"
  # S04 #168n6 live-#PF (REVIEW #229 cheapest disconfirm): the guest touches
  # the unmapped PF_PROBE_ADDR guard, so the PF stub must emit
  # `PF ok=1 vector=14 code=0x... addr=0x...` before chaining into #DE.
  # Same-line regex mirrors the RMBZ/GUARD gates: literal space after ok=1
  # stops ok=10 false-passing, [^\r\n]* keeps code/addr same-line, hex pins
  # stop dropped fields, (\s|$) terminates the addr field.
  $pfPat = "PF ok=1 vector=14 [^\r\n]*code=0x[0-9a-f]+[^\r\n]*addr=0x[0-9a-f]+(\s|$)"
  # S09 slice2b live-#PF U/S path (REVIEW #622 risk 3: USER is
  # classifier-only, so the CPU-pushed U/S bit needs its own live proof):
  # the #PF handler decodes the error code and emits
  # `PFUS ok=1 us=0 w=0 p=0` for the supervisor-read-not-present probe.
  # Value gate (not value-free): us/w/p must all be 0 — any U/S=1, write,
  # or present fault prints ok=0 fail-closed. Mid-line `us`/`w` use
  # `[ \t]` (single same-line blank, mirrors the PERMAUDIT user pin:
  # `(\s|$)` would match `\n` and false-pass a newline-split pair);
  # line-terminal `p` uses `(\s|$)`. CPL3 U=1 entry stays deferred.
  $pfusPat = "PFUS ok=1 [^\r\n]*us=0[ \t][^\r\n]*w=0[ \t][^\r\n]*p=0(\s|$)"
  # S09 slice2bt TRYSTORE rok hard-require (#30, REVIEW #765 R1 + #767:
  # three green rok=1 boots slice2bq+br+bs): the bounded #PF-catch must prove
  # walk==CPU (low=0 high=1) with the historic code (hcode=0x3 = P+W present
  # supervisor-write) AND the live emission proof (rok=1 = fault RIP decodes
  # to 48 89 07). Value gate (not value-free): any low=1, high=0, wrong code,
  # or rok=0 prints fail-closed. Same-line shape mirrors the PFUS/RMBZ gates:
  # literal space after TRYSTORE stops prefix false-passing, [^\r\n]* keeps
  # all fields same-line, (\s|$) terminates rok. lcode=0x0 rides pinned (low
  # no-fault reports 0). See tools/qemu/test-boot-uefi-gates.ps1.
  $trystorePat = "TRYSTORE low=0 high=1 [^\r\n]*lcode=0x0[^\r\n]*hcode=0x3[^\r\n]*rok=1(\s|$)"
  # S09 slice2bb live U-map attempt (#30): the guest installs the 4-page
  # U+W+NX map (alloc-4 + bound store+flush through the live PT, 4K base)
  # then re-walks. Live QEMU 2026-10-13 still prints
  # `USTACK ok=0 label=perms-refused` (install refuses fail-closed), so
  # the gate stays at the slice2aw ok=0 baseline until the refusal is
  # diagnosed (candidates: 2M PS leaf, PT-bounds gate, NXE pin, PT straddle).
  # Same-line shape mirrors the PFUS gate (literal space after ok=0 stops
  # ok=01 false-passing, [^\r\n]* keeps the label same-line, (\s|$)
  # terminates it). The ok=1 valid flip rides with the diagnosed fix.
  $ustackPat = "USTACK ok=0 [^\r\n]*label=(perms-refused|walk-failed)(\s|$)"
  # S09 slice2l (REVIEW #648 risk 1 load-bearing gate gap: TR liveness was
  # human-reads-serial, not automated): `TR ok=1 [^\r\n]*sel=0x28(\s|$)`
  # mirrors the PFUS/RMBZ value gates — the literal space after `ok=1`
  # stops `ok=10`/`ok=1x` false-passing, same-line `[^\r\n]*` stops a
  # newline-split `ok=1` / sel pair, and `sel=0x28(\s|$)` pins the exact
  # TSS_SEL so a skipped/faulted `ltr` printing `ok=0` fails e2e.
  # Value gate (not value-free): TR must equal TSS_SEL == 0x28.
  # Defense in depth with the guest: write_tr_line forces ok=0 on
  # tr != TSS_SEL, so a faulted ltr fails both the ok=1 and sel pins.
  # `TR ok=0` is additionally forbidden outright (refusal-FAIL shape).
  $trPat = "TR ok=1 [^\r\n]*sel=0x28(\s|$)"
  $trFail = "TR ok=0"
  # #59a value check (agent-b REVIEW #75 finding 1): presence of the
  # substrings alone would let map_size=999999999 or map_size=abc PASS.
  # Regex-capture the actual values off the MEMMAP ok=1 line and enforce
  # the 16 KiB static + 4 KiB pool-slack budget (20480) plus a plausible
  # descriptor stride [40,256]; missing/non-numeric/out-of-range fails.
  $budgetOk = $false
  $budgetDetail = ""
  $mmVals = [regex]::Match($serialText, "MEMMAP ok=1 [^\r\n]*map_size=(\d+) desc_size=(\d+)")
  if ($mmVals.Success) {
    $mv = 0L
    $dv = 0L
    if ([long]::TryParse($mmVals.Groups[1].Value, [ref]$mv) -and
        [long]::TryParse($mmVals.Groups[2].Value, [ref]$dv)) {
      $budgetOk = ($mv -le 20480) -and ($dv -ge 40) -and ($dv -le 256)
      $budgetDetail = "map_size=$mv (<=20480), desc_size=$dv ([40,256]), ok=$budgetOk"
    } else {
      $budgetDetail = "non-numeric map_size/desc_size ($($mmVals.Groups[1].Value)/$($mmVals.Groups[2].Value))"
    }
  } else {
    $budgetDetail = "MEMMAP ok=1 line with map_size=<dec> desc_size=<dec> not found"
  }
  Write-Output "Budget check: $budgetDetail"
  # S08 slice4 (knob report, REVIEW #605 note 2; slice5 REVIEW #607 N2):
  # extract the STI wait-cost trace for the before/after report.
  # Presence-only still passes, but spins>=200 rides at the budget boundary
  # and must never read as healthy. Note the regex above requires ok=1, so
  # a true budget-miss (ok=0, gate never cleared) never matches here and
  # already fails the ok=1 hard-require -- this print-only flag marks the
  # ambiguous spins==200 + ok==1 boundary (gate cleared on the 201st batch
  # check, spins caps at exactly 200 for both gate-wake and budget-exit).
  # -ge 200 kept (-gt 200 is unreachable since spins caps at 200).
  $stiDetail = "STI ok=1 line with ticks/work/spins/batch/target not found"
  $stiVals = [regex]::Match($serialText, "STI ok=1 [^\r\n]*ticks=(\d+)[^\r\n]*work=(\d+)[^\r\n]*spins=(\d+)[^\r\n]*batch=(\d+)[^\r\n]*target=(\d+)")
  if ($stiVals.Success) {
    $stiDetail = "ticks=$($stiVals.Groups[1].Value) work=$($stiVals.Groups[2].Value) spins=$($stiVals.Groups[3].Value) batch=$($stiVals.Groups[4].Value) target=$($stiVals.Groups[5].Value)"
    $stiSpins = 0L
    if ([long]::TryParse($stiVals.Groups[3].Value, [ref]$stiSpins) -and ($stiSpins -ge 200)) {
      $stiDetail = $stiDetail + " [WARNING at budget boundary spins>=200 -- inspect gate vs budget]"
    }
  }
  Write-Output "STI trace: $stiDetail"
  $hasBootinfo = $serialText.Contains($bootinfo)
  $hasMemmap = $serialText.Contains($memmap)
  $hasNotrunc = $serialText.Contains($notrunc)
  $hasMapsize = $serialText.Contains($mapsize)
  $hasDescsize = $serialText.Contains($descsize)
  $hasFb = $serialText.Contains($fb)
  $hasGop = $serialText.Contains($gop)
  $hasEbs = $serialText.Contains($ebs)
  $hasKernel = $serialText.Contains($kernel)
  $hasFramealloc = $serialText.Contains($framealloc)
  $hasPaging = $serialText.Contains($paging)
  $hasPerm = $serialText.Contains($perm)
  $hasGuard = [regex]::IsMatch($serialText, $guardPat)
  $hasUser = [regex]::IsMatch($serialText, $userPat)
  $hasPermaudit = $serialText.Contains($permaudit)
  $hasPermauditW = [regex]::IsMatch($serialText, $permauditWPat)
  $hasPermauditUN = [regex]::IsMatch($serialText, $permauditUNPat)
  $hasPermaudit2 = $serialText.Contains($permaudit2)
  $hasPermaudit2W = [regex]::IsMatch($serialText, $permaudit2WPat)
  $hasPermaudit2UN = [regex]::IsMatch($serialText, $permaudit2UNPat)
  $hasRmbz = [regex]::IsMatch($serialText, $rmbzPat)
  $hasThread = $serialText.Contains($thread)
  $hasThreadRun = $serialText.Contains($threadRun)
  $hasThreadCs = $serialText.Contains($threadCs)
  $hasThreadSave = $serialText.Contains($threadSave)
  $hasThreadPong = $serialText.Contains($threadPong)
  $hasThreadPongRun = $serialText.Contains($threadPongRun)
  $hasSchedSwitch = $serialText.Contains($schedSwitch)
  $hasSwitchLat = $serialText.Contains($switchLat)
  $hasThreadJump = $serialText.Contains($threadJump)
  $hasThreadJumpRun = $serialText.Contains($threadJumpRun)
  $hasThreadJumpRefusal = $serialText.Contains($threadJumpRefusal)
  $noRefusalFail = -not $serialText.Contains($refusalFail)
  $hasChainOk = $serialText.Contains($chainOk)
  $hasChainCursor = $serialText.Contains($chainCursor)
  $hasSchedExit = $serialText.Contains($schedExit)
  $hasSchedJoin = $serialText.Contains($schedJoin)
  $hasSchedStat = $serialText.Contains($schedStat)
  $hasSchedFair = $serialText.Contains($schedFair)
  $hasSchedTrace = $serialText.Contains($schedTrace)
  $hasSchedTraceX = $serialText.Contains($schedTraceX)
   $hasSchedTick = $serialText.Contains($schedTick)
   $hasTick = [regex]::IsMatch($serialText, $tickPat)
   $hasSti = [regex]::IsMatch($serialText, $stiPat)
   $hasIrqstack = [regex]::IsMatch($serialText, $irqstackPat)
  $chainOrderOk = $false
  $ci2 = $serialText.IndexOf($chainHop2)
  $ci3 = $serialText.IndexOf($chainHop3)
  if ($ci2 -ge 0 -and $ci3 -gt $ci2) {
    $ci1 = $serialText.IndexOf($chainHop1, $ci3)
    $chainOrderOk = ($ci1 -gt $ci3) -and $hasChainOk
  }
   $hasPf = [regex]::IsMatch($serialText, $pfPat)
   $hasPfus = [regex]::IsMatch($serialText, $pfusPat)
   $hasTrystore = [regex]::IsMatch($serialText, $trystorePat)
   $hasUstack = [regex]::IsMatch($serialText, $ustackPat)
  $hasTr = [regex]::IsMatch($serialText, $trPat)
  $noTrFail = -not $serialText.Contains($trFail)
  $hasExc = $serialText.Contains($exc)
   if ($hasBootinfo -and $hasMemmap -and $hasNotrunc -and $hasMapsize -and $hasDescsize -and $budgetOk -and $hasFb -and $hasGop -and $hasEbs -and $hasKernel -and $hasFramealloc -and $hasPaging -and $hasPerm -and $hasGuard -and $hasUser -and $hasPermaudit -and $hasPermauditW -and $hasPermauditUN -and $hasPermaudit2 -and $hasPermaudit2W -and $hasPermaudit2UN -and $hasRmbz -and $hasThread -and $hasThreadRun -and $hasThreadCs -and $hasThreadSave -and $hasThreadPong -and $hasThreadPongRun -and $hasSchedSwitch -and $hasSwitchLat -and $hasSchedExit -and $hasSchedJoin -and $hasSchedStat -and $hasSchedFair -and $hasSchedTrace -and $hasSchedTraceX -and $hasSchedTick -and $hasTick -and $hasSti -and $hasIrqstack -and $hasThreadJump -and $hasThreadJumpRun -and    $hasThreadJumpRefusal -and $noRefusalFail -and $chainOrderOk -and $hasChainCursor -and    $hasPf -and $hasPfus -and $hasTrystore -and $hasUstack -and $hasTr -and $noTrFail -and $hasExc) {
    Write-Output ("Boot summary: FOUND (" + $bootinfo + " + " + $memmap + " + " + $notrunc + " + " + $mapsize + " + " + $descsize + " + " +
      "budget ok + $fb + $gop + $ebs + $kernel + $framealloc + $paging + $perm + GUARD ok=1 passed=7/7 + USER ok=1 passed=9/9 + $permaudit + $permaudit2 + RMBZ ok=1 viol + THREAD ok=1 + THREAD-RUN id=2 + THREAD-CS ok=1 iter=16 + THREAD-SAVE ok=1 + THREAD-PONG ok=1 + THREAD-PONG-RUN id=1 + SCHED-SWITCH ok=1 from=1 to=2 + SWITCH-LAT ok=1 count=16 + SCHED-EXIT ok=1 + SCHED-JOIN ok=1 + SCHED-STAT ok=1 + SCHED-FAIR ok=1 + SCHED-TRACE ok=1 + SCHED-TRACE-X ok=1 + SCHED-TICK ok=1 + TICK ok=1 + STI ok=1 + IRQSTACK ok=1 + THREAD-JUMP ok=1 id=2 + THREAD-JUMP-RUN id=2 + THREAD-JUMP ok=0 id=0 + no REFUSAL-FAIL + CHAIN hop=2/3/1 + CHAIN ok=1 chain=1231 + CHAIN-CURSOR ok=1 legs=3 + PF ok=1 vector=14 + PFUS ok=1 us=0 w=0 p=0 + TRYSTORE low=0 high=1 lcode=0x0 hcode=0x3 rok=1 + USTACK ok=0 label + TR ok=1 sel=0x28 + no TR ok=0 + $exc)")
    Write-Output "UEFI BOOT PASS"
    exit 0
  } else {
    Write-Output ("Boot summary: MISSING (BOOTINFO valid=1 present=$hasBootinfo, " +
      "MEMMAP ok=1 present=$hasMemmap, truncated=0 present=$hasNotrunc, " +
      "map_size= present=$hasMapsize, desc_size= present=$hasDescsize, " +
      "budget ok=$budgetOk ($budgetDetail), " +
      "FB present=1 present=$hasFb, GOP ok=1 present=$hasGop, " +
      "EBS ok=1 present=$hasEbs, KERNEL ok=1 present=$hasKernel, " +
      "FRAMEALLOC ok=1 present=$hasFramealloc, " +
      "PAGING ok=1 present=$hasPaging, " +
      "PERM ok=1 present=$hasPerm, " +
      "GUARD ok=1 passed=7/7 present=$hasGuard, " +
      "USER ok=1 passed=9/9 present=$hasUser, " +
      "PERMAUDIT ok=1 present=$hasPermaudit, w=1 present=$hasPermauditW, user/nx shape present=$hasPermauditUN, " +
      "PERMAUDIT2 ok=1 present=$hasPermaudit2, w=1 present=$hasPermaudit2W, user/nx shape present=$hasPermaudit2UN, " +
      "RMBZ ok=1 viol-shape present=$hasRmbz, " +
      "THREAD ok=1 present=$hasThread, " +
      "THREAD-RUN id=2 present=$hasThreadRun, " +
      "THREAD-CS ok=1 iter=16 present=$hasThreadCs, " +
      "THREAD-SAVE ok=1 present=$hasThreadSave, " +
      "THREAD-PONG ok=1 present=$hasThreadPong, " +
      "THREAD-PONG-RUN id=1 present=$hasThreadPongRun, " +
      "SCHED-SWITCH ok=1 from=1 to=2 present=$hasSchedSwitch, " +
      "SWITCH-LAT ok=1 count=16 present=$hasSwitchLat, " +
      "SCHED-EXIT ok=1 exited=2 resumed=3 present=$hasSchedExit, " +
      "SCHED-JOIN ok=1 joined=2 present=$hasSchedJoin, " +
      "SCHED-STAT ok=1 present=$hasSchedStat, " +
      "SCHED-FAIR ok=1 present=$hasSchedFair, " +
       "SCHED-TRACE ok=1 present=$hasSchedTrace, " +
       "SCHED-TRACE-X ok=1 present=$hasSchedTraceX, " +
       "SCHED-TICK ok=1 present=$hasSchedTick, " +
       "TICK ok=1 ticks-shape present=$hasTick, " +
       "STI ok=1 ticks/work/spins/batch/target-shape present=$hasSti, " +
      "IRQSTACK ok=1 task/irq-shape present=$hasIrqstack, " +
      "THREAD-JUMP ok=1 id=2 present=$hasThreadJump, " +
      "THREAD-JUMP-RUN id=2 present=$hasThreadJumpRun, " +
      "THREAD-JUMP ok=0 id=0 present=$hasThreadJumpRefusal, " +
      "no REFUSAL-FAIL present=$noRefusalFail, " +
      "CHAIN hop=2/3/1 in-order + ok=1 chain=1231 present=$chainOrderOk, " +
      "CHAIN-CURSOR ok=1 legs=3 present=$hasChainCursor, " +
      "PF ok=1 vector=14-shape present=$hasPf, " +
      "PFUS ok=1 us=0 w=0 p=0-shape present=$hasPfus, " +
      "TRYSTORE low=0 high=1 lcode=0x0 hcode=0x3 rok=1-shape present=$hasTrystore, " +
      "USTACK ok=0 label-shape present=$hasUstack, " +
      "TR ok=1 sel=0x28-shape present=$hasTr, " +
      "no TR ok=0 present=$noTrFail, " +
      "EXC ok=1 vector=0 present=$hasExc; " +
      "guest GetMemoryMap/GOP/ExitBootServices/kernel-entry/frame-alloc/#PF/#DE path failed " +
      "-- see serial log)")
    Write-Output "UEFI BOOT FAIL"
    exit 1
  }
} else {
  Write-Output "Serial marker: MISSING ($marker not in $serialLog, ${size} bytes)"
  if (Test-Path -LiteralPath $stdoutLog) {
    $tail = (Get-Content -LiteralPath $stdoutLog -Raw -ErrorAction SilentlyContinue)
    if ($tail -and $tail.Length -gt 2000) { $tail = $tail.Substring(0, 2000) }
    if ($tail) { Write-Output ("--- qemu stdout (truncated) ---`n" + $tail) }
  }
  Write-Output "UEFI BOOT FAIL"
  exit 1
}
