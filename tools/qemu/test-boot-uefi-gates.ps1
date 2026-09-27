#Requires -Version 5.1
<#
.SYNOPSIS
  Checked-in regex control for boot-uefi.ps1 gates (S04 slices 22+25).
  Reads the live W patterns + GUARD pattern from boot-uefi.ps1 (no
  duplication) and asserts:
  PERMAUDIT/W: w=1 passes, w=0 fails, w=10 fails (REVIEW #158 finding 1),
  newline-split fails (same-line gate). Covers both PERMAUDIT and PERMAUDIT2.
  GUARD (REVIEW #168 nits 1-3): ok=1 passed=7/7 passes; ok=10 / ok=1x fail
  (boundary tightening); passed=6/7 fails (coverage pin); missing passed
  fails; newline-split fails.
  Exits 0 on PASS, 1 on FAIL.
#>
$ErrorActionPreference = "Stop"

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$gate = Join-Path $here "boot-uefi.ps1"
$text = Get-Content -LiteralPath $gate -Raw

function Get-Pat([string]$name) {
  $m = [regex]::Match($text, '\$' + $name + '\s*=\s*"([^"]+)"')
  if (-not $m.Success) {
    Write-Output "GATE TEST FAIL (pattern $name not found in boot-uefi.ps1)"
    exit 1
  }
  return $m.Groups[1].Value
}

$w1 = Get-Pat "permauditWPat"
$w2 = Get-Pat "permaudit2WPat"
$g = Get-Pat "guardPat"

# Finding-1 lock: both patterns must carry the literal `w=1(\s|$)` tightening
# so w=10 cannot match via the w=1 substring. Match the literal suffix
# (REVIEW #162 nit: `*w=1(*` would also accept a wrong-but-parenthesized
# variant such as `w=1(x)`).
foreach ($p in @($w1, $w2)) {
  if ($p -notlike '*w=1(\s|$)*') {
    Write-Output "GATE TEST FAIL (pattern missing w=1(\s|$) tightening: $p)"
    exit 1
  }
}
# REVIEW #168 nit-1/nit-2 lock: GUARD pattern must carry the `ok=1` boundary
# tightening and pin `passed=7/7` on the same line.
if ($g -notlike '*GUARD ok=1*passed=7/7(\s|$)*') {
  Write-Output "GATE TEST FAIL (GUARD pattern missing ok=1/passed=7/7 tightening: $g)"
  exit 1
}

$cases = @(
  # tag, pattern, sample, expected, label
  @("P1-good", $w1, "PERMAUDIT ok=1 phys=0x80000000 user=0 nx=0 w=1`n", $true, "w=1 passes"),
  @("P1-zero", $w1, "PERMAUDIT ok=1 phys=0x80000000 user=0 nx=0 w=0`n", $false, "w=0 fails"),
  @("P1-ten", $w1, "PERMAUDIT ok=1 phys=0x80000000 user=0 nx=0 w=10`n", $false, "w=10 fails"),
  @("P1-split", $w1, "PERMAUDIT ok=1 phys=0x80000000 user=0 nx=0`n w=1`n", $false, "newline-split fails"),
  @("P2-good", $w2, "PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=0 w=1`n", $true, "w=1 passes"),
  @("P2-zero", $w2, "PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=0 w=0`n", $false, "w=0 fails"),
  @("P2-ten", $w2, "PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=0 w=10`n", $false, "w=10 fails"),
  @("P2-split", $w2, "PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=0`n w=1`n", $false, "newline-split fails"),
  @("G-good", $g, "GUARD ok=1 passed=7/7`n", $true, "ok=1 7/7 passes"),
  @("G-ten", $g, "GUARD ok=10 passed=7/7`n", $false, "ok=10 fails"),
  @("G-onex", $g, "GUARD ok=1x passed=7/7`n", $false, "ok=1x fails"),
  @("G-six", $g, "GUARD ok=1 passed=6/7`n", $false, "6/7 fails"),
  @("G-nopass", $g, "GUARD ok=1`n", $false, "missing passed fails"),
  @("G-split", $g, "GUARD ok=1`n passed=7/7`n", $false, "newline-split fails")
)

$failed = 0
foreach ($c in $cases) {
  $got = [regex]::IsMatch($c[2], $c[1])
  $ok = ($got -eq $c[3])
  if ($ok) {
    Write-Output ("GATE OK $($c[0]) $($c[4])")
  } else {
    Write-Output ("GATE TEST FAIL $($c[0]) $($c[4]): got=$got want=$($c[3]) pat=$($c[1])")
    $failed += 1
  }
}

if ($failed -gt 0) {
  Write-Output "GATE TEST FAIL ($failed cases)"
  exit 1
}
Write-Output "GATE TEST PASS (14/14)"
exit 0
