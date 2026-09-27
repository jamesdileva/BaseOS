#Requires -Version 5.1
<#
.SYNOPSIS
  Checked-in regex control for boot-uefi.ps1 PERMAUDIT gates (S04 slice 22).
  Reads the live W patterns from boot-uefi.ps1 (no duplication) and asserts:
  w=1 passes, w=0 fails, w=10 fails (REVIEW #158 finding 1), newline-split
  fails (same-line gate). Covers both PERMAUDIT and PERMAUDIT2.
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

# Finding-1 lock: both patterns must carry the (\s|$) tightening so w=10
# cannot match via the w=1 substring.
foreach ($p in @($w1, $w2)) {
  if ($p -notlike '*w=1(*') {
    Write-Output "GATE TEST FAIL (pattern missing w=1 tightening: $p)"
    exit 1
  }
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
  @("P2-split", $w2, "PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=0`n w=1`n", $false, "newline-split fails")
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
Write-Output "GATE TEST PASS (8/8)"
exit 0
