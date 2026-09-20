#Requires -Version 5.1
<#
.SYNOPSIS
  S00 QEMU smoke test: verifies QEMU launches and stays up; captures
  serial output to a file; reports OVMF/EDK2 firmware presence explicitly.
  Exits 0 on PASS, non-zero on FAIL (no false-positive quick-exit pass).
  Full UEFI-OS boot arrives in S01. Pass -RequireOvmf when firmware
  is mandatory (S01 entry); without it a missing OVMF is advisory only.

  Addresses agent-b review #4 findings 1 and 5, review #6 finding 1.
#>
param(
  [int]$TimeoutSec = 5,
  [string]$LogDir = "target",
  [switch]$RequireOvmf
)
$ErrorActionPreference = "Stop"

Write-Output "=== BaseOS QEMU smoke (S00) ==="

# --- OVMF/EDK2 firmware detection (explicit assert, S01 prerequisite) ---
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
if ($ovmf) {
  Write-Output "OVMF: FOUND at $ovmf"
} else {
  $msg = "OVMF: MISSING (S01 UEFI boot requires edk2-x86_64-code.fd; set OVMF_CODE env var)"
  if ($RequireOvmf) {
    Write-Output ($msg + " [-RequireOvmf asserted]")
    exit 1
  }
  Write-Output $msg
}

# --- Serial-to-file capture setup ---
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
$serialLog = Join-Path $LogDir "qemu-smoke-serial.log"
$stdoutLog = Join-Path $LogDir "qemu-smoke-stdout.log"
foreach ($f in @($serialLog, $stdoutLog)) {
  if (Test-Path -LiteralPath $f) { Remove-Item -LiteralPath $f -Force }
}
New-Item -ItemType File -Force -Path $serialLog | Out-Null

# NOTE: -display none only (previous -nographic + -display none was redundant).
# QEMU with no disk should start SeaBIOS and stay running; serial goes to file.
$qemuArgs = @(
  "-display", "none",
  "-m", "256",
  "-serial", ("file:" + $serialLog),
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
  $size = 0
  if (Test-Path -LiteralPath $serialLog) { $size = (Get-Item -LiteralPath $serialLog).Length }
  Write-Output "QEMU stayed running for ${TimeoutSec}s (killed after smoke window)"
  Write-Output "Serial log: $serialLog (${size} bytes)"
  Write-Output "SMOKE PASS"
  exit 0
} else {
  $code = $proc.ExitCode
  $tail = ""
  if (Test-Path -LiteralPath $stdoutLog) {
    $tail = (Get-Content -LiteralPath $stdoutLog -Raw -ErrorAction SilentlyContinue)
    if ($tail -and $tail.Length -gt 2000) { $tail = $tail.Substring(0, 2000) }
  }
  Write-Output ("QEMU exited quickly with code " + $code + " - SMOKE FAIL (quick exit means firmware/args broken)")
  if ($tail) { Write-Output ("--- qemu stdout (truncated) ---`n" + $tail) }
  Write-Output ("See serial log: " + $serialLog)
  exit 1
}
