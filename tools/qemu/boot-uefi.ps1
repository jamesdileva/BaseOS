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
  [string]$LogDir = "target"
)
$ErrorActionPreference = "Stop"

Write-Output "=== BaseOS UEFI guest boot (S04 slice 24: guard-page e2e + GUARD line) ==="

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
  $guard = "GUARD ok=1"
  $permaudit = "PERMAUDIT ok=1"
  # REVIEW #153 gap (2): `ok` is walk-success, independent of `w`, so a w=0
  # regression still matched the substring above. Hard-require the W bit on
  # the same line via regex (slice19 gate tightening; baseline OVMF is w=1).
  # Slice 22 tightens `w=1` with `(\s|$)` so `w=10` no longer matches
  # (REVIEW #158 finding 1); see tools/qemu/test-boot-uefi-gates.ps1.
  $permauditWPat = "PERMAUDIT ok=1 [^\r\n]* w=1(\s|$)"
  # S04 slice 20 (REVIEW #147 risk 1 multi-probe): second low-RAM probe on
  # its own line. Slice 21 gates `w=1` same-line (baseline measured w=1 on
  # OVMF in slice 20); `user`/`nx` stay reporting-only by design.
  # Slice 22 tightens both W patterns with `(\s|$)` (finding 1).
  $permaudit2 = "PERMAUDIT2 ok=1"
  $permaudit2WPat = "PERMAUDIT2 ok=1 [^\r\n]* w=1(\s|$)"
  $exc = "EXC ok=1 vector=0"
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
  $hasGuard = $serialText.Contains($guard)
  $hasPermaudit = $serialText.Contains($permaudit)
  $hasPermauditW = [regex]::IsMatch($serialText, $permauditWPat)
  $hasPermaudit2 = $serialText.Contains($permaudit2)
  $hasPermaudit2W = [regex]::IsMatch($serialText, $permaudit2WPat)
  $hasExc = $serialText.Contains($exc)
  if ($hasBootinfo -and $hasMemmap -and $hasNotrunc -and $hasMapsize -and $hasDescsize -and $budgetOk -and $hasFb -and $hasGop -and $hasEbs -and $hasKernel -and $hasFramealloc -and $hasPaging -and $hasPerm -and $hasGuard -and $hasPermaudit -and $hasPermauditW -and $hasPermaudit2 -and $hasPermaudit2W -and $hasExc) {
    Write-Output ("Boot summary: FOUND ($bootinfo + $memmap + $notrunc + $mapsize + $descsize + " +
      "budget ok + $fb + $gop + $ebs + $kernel + $framealloc + $paging + $perm + $guard + $permaudit + $permaudit2 + $exc)")
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
      "GUARD ok=1 present=$hasGuard, " +
      "PERMAUDIT ok=1 present=$hasPermaudit, w=1 present=$hasPermauditW, " +
      "PERMAUDIT2 ok=1 present=$hasPermaudit2, w=1 present=$hasPermaudit2W, " +
      "EXC ok=1 vector=0 present=$hasExc; " +
      "guest GetMemoryMap/GOP/ExitBootServices/kernel-entry/frame-alloc/#DE path failed " +
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
