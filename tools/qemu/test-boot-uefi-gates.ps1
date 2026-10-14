#Requires -Version 5.1
<#
.SYNOPSIS
  Checked-in regex control for boot-uefi.ps1 gates (S04 slices 22+25+27+28+34).
  Reads the live W + user/nx-shape patterns + GUARD pattern from boot-uefi.ps1
  (no duplication) and asserts:
  PERMAUDIT/W: w=1 passes, w=0 fails, w=10 fails (REVIEW #158 finding 1),
  newline-split fails (same-line gate). Covers both PERMAUDIT and PERMAUDIT2.
  PERMAUDIT user/nx shape (slice 27 #147r4): good passes, user=2 fails,
  nx=2 fails, missing-user fails, newline-split fails (value-free, both probes).
  Slice 29 (#184 nit1): tab-positive U1-tab/U2-tab (user=0<TAB>nx=0 passes)
  pins the slice-28 [ \t] acceptance; VALUE/boundary policy documented once
  in boot-uefi.ps1 (order phys/user/nx/w, value-free [01], [ \t] mid-line vs
  (\s|$) line-terminal).
  GUARD (REVIEW #168 nits 1-3): ok=1 passed=7/7 passes; ok=10 / ok=1x fail
  (boundary tightening); passed=6/7 fails (coverage pin); missing passed
  fails; newline-split fails.
  RMBZ (S04 slice 36 viol-enforcement, #139 note 1): ok=1 viol=0 passes;
  viol=10 / viol=1 / ok=0-viol=1 fail (fail-closed value gate);
  ok=10 / ok=1x fail; missing viol fails; newline-split fails.
  PFUS (S09 slice2b live-#PF U/S, REVIEW #622 risk 3): ok=1 us=0 w=0 p=0
  passes; us=1 / w=1 / p=1 fail (fail-closed value gate); ok=10 fails;
  missing p fails; newline-split fails.
  TRYSTORE (S09 slice2bt rok hard-require, #30 REVIEW #767: three green
  rok=1 boots slice2bq+br+bs): low=0 high=1 lcode=0x0 hcode=0x3 rok=1
  passes; low=1 / high=0 / wrong-hcode / rok=0 fail (fail-closed value
  gate); rok=10 fails; missing rok fails; newline-split fails.
  USTACK (S09 slice2bb live U-map attempt reverted to slice2aw ok=0
  baseline): ok=0 either-refuse passes; ok=1 valid fails; ok=0 label=valid
  drift fails; missing label fails; newline-split fails.
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
$u1 = Get-Pat "permauditUNPat"
$u2 = Get-Pat "permaudit2UNPat"
$g = Get-Pat "guardPat"
$u = Get-Pat "userPat"
$r = Get-Pat "rmbzPat"
$f = Get-Pat "pfusPat"
$y = Get-Pat "trystorePat"
$yn = Get-Pat "trystoreNmiPat"
$t = Get-Pat "trPat"
$s = Get-Pat "ustackPat"

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
# Slice 27 lock, tightened slice 28 (#182 nit2): user/nx-shape patterns
# must carry the literal `user=[01][ \t]` and `nx=[01]` pins, value-free
# so a future user=1/nx=1 policy still passes the shape. Slice 27 used a
# literal trailing space after `user=[01]`; slice 28 accepts space-or-tab
# (same-line blank). `(\s|$)` was rejected for `user` because `\s`
# matches `\n` and false-passed the newline-split case.
foreach ($p in @($u1, $u2)) {
  if ((-not $p.Contains('user=[01][ \t]')) -or (-not $p.Contains('nx=[01]'))) {
    Write-Output "GATE TEST FAIL (user/nx-shape pattern missing user=[01]/nx=[01] pins: $p)"
    exit 1
  }
}
# REVIEW #168 nit-1/nit-2 lock, tightened per REVIEW #170 nit:
# GUARD pattern must carry literal space after `ok=1` (boundary), same-line
# `[^\r\n]*passed`, pin `passed=7/7`, and `(\s|$)` boundary. The prior lock
# `*GUARD ok=1*passed=7/7(\s|$)*` let `*` match zero chars, so a regression
# to the boundary-less form still passed the lock line (caught only by G-ten).
if (($g -notlike '*GUARD ok=1 *') -or (-not $g.Contains('[^\r\n]*passed=7/7')) -or ($g -notlike '*passed=7/7(\s|$)*')) {
  Write-Output "GATE TEST FAIL (GUARD pattern missing ok=1/passed=7/7 tightening: $g)"
  exit 1
}
# S09 slice1 USER gate (REVIEW #620): mirrors the GUARD lock — literal
# space after `ok=1`, same-line `[^\r\n]*passed=9/9`, `(\s|$)` boundary.
# Slice2a (REVIEW #622 risk 1) grows 7/7 to 9/9 with the NX pair.
if (($u -notlike '*USER ok=1 *') -or (-not $u.Contains('[^\r\n]*passed=9/9')) -or ($u -notlike '*passed=9/9(\s|$)*')) {
  Write-Output "GATE TEST FAIL (USER pattern missing ok=1/passed=9/9 tightening: $u)"
  exit 1
}
# S04 slice 36 viol-enforcement (fail-closed, #139 note 1 RMBZ gate):
# pattern must carry the literal space after `ok=1` (boundary, mirrors
# GUARD), same-line `[^\r\n]*viol=0`, and a `viol=0(\s|$)` value pin so
# ok=10 / viol>0 / missing-viol / newline-split pairs fail.
if (($r -notlike '*RMBZ ok=1 *') -or (-not $r.Contains('[^\r\n]*viol=0')) -or ($r -notlike '*viol=0(\s|$)*')) {
  Write-Output "GATE TEST FAIL (RMBZ pattern missing ok=1/viol tightening: $r)"
  exit 1
}
# S09 slice2b live-#PF U/S (REVIEW #622 risk 3, value gate like RMBZ):
# pattern must carry the literal space after `ok=1` (boundary), same-line
# `us=0`/`w=0` pins with single same-line-blank `[ \t]` (mid-line: `(\s|$)`
# would match `\n` and false-pass a newline-split pair, same class as the
# PERMAUDIT `user` pin), same-line `[^\r\n]*p=0` with a `p=0(\s|$)` value pin
# so us=1 / w=1 / p=1 / missing-p / newline-split pairs fail.
if (($f -notlike '*PFUS ok=1 *') -or (-not $f.Contains('us=0')) -or (-not $f.Contains('w=0')) -or (-not $f.Contains('[^\r\n]*p=0')) -or ($f -notlike '*p=0(\s|$)*')) {
  Write-Output "GATE TEST FAIL (PFUS pattern missing ok=1/us/w/p tightening: $f)"
  exit 1
}
# S09 slice2bt TRYSTORE rok hard-require (#30, REVIEW #767: three green
# rok=1 boots slice2bq+br+bs): pattern must carry the literal space after
# `low=0` / `high=1` (boundary, mirrors PFUS), same-line `lcode=0x0` /
# `hcode=0x3` pins with the historic code (P+W present supervisor-write),
# and a `rok=1(\s|$)` value pin so low=1 / high=0 / wrong-code / rok=0 /
# missing-rok / newline-split pairs fail.
if (($y -notlike '*TRYSTORE low=0 high=1 *') -or (-not $y.Contains('lcode=0x0')) -or (-not $y.Contains('hcode=0x3')) -or ($y -notlike '*rok=1(\s|$)*')) {
  Write-Output "GATE TEST FAIL (TRYSTORE pattern missing low/high/lcode/hcode/rok tightening: $y)"
  exit 1
}
# S09 slice2by TRYSTORE-NMI live readback (#30, REVIEW #781: host-only ->
# QEMU-observed): pattern must carry the literal space after `ok=1`
# (boundary, mirrors TRYSTORE), same-line `nmi=0[ \t]` pin (mid-line:
# `(\s|$)` would match `\n` and false-pass a split pair, same class as
# PFUS us/w), and a `mce=0(\s|$)` value pin so nmi=1 / mce=1 / ok=0 /
# missing-mce / newline-split pairs fail.
if (($yn -notlike '*TRYSTORE-NMI ok=1 *') -or (-not $yn.Contains('nmi=0')) -or (-not $yn.Contains('[^\r\n]*mce=0')) -or ($yn -notlike '*mce=0(\s|$)*')) {
  Write-Output "GATE TEST FAIL (TRYSTORE-NMI pattern missing ok=1/nmi/mce tightening: $yn)"
  exit 1
}
# S09 slice2bb live U-map attempt (reverted to slice2aw ok=0 baseline after
# live QEMU refusal -- install path diagnosis pending): pattern must carry
# the literal space after `ok=0` (boundary), same-line label pin, and the
# `(perms-refused|walk-failed)` alternation so ok=1 / missing-label /
# newline-split / ok-label drift (ok=0 label=valid, ok=1 perms-refused)
# pairs fail. The ok=1 valid flip rides with the diagnosed fix.
if (($s -notlike '*USTACK ok=0 *') -or (-not $s.Contains('[^\r\n]*label=(perms-refused|walk-failed)')) -or ($s -notlike '*label=(perms-refused|walk-failed)(\s|$)*')) {
  Write-Output "GATE TEST FAIL (USTACK pattern missing ok=0/refuse-label tightening)"
  exit 1
}
# S09 slice2l TR gate (REVIEW #648 risk 1 load-bearing gap: TR liveness was
# human-reads-serial, not automated): pattern must carry the literal space
# after `ok=1` (boundary, mirrors GUARD), same-line `[^\r\n]*sel=0x28`,
# and a `sel=0x28(\s|$)` value pin so ok=0 / ok=10 / wrong-sel /
# missing-sel / newline-split pairs fail.
if (($t -notlike '*TR ok=1 *') -or (-not $t.Contains('[^\r\n]*sel=0x28')) -or ($t -notlike '*sel=0x28(\s|$)*')) {
  Write-Output "GATE TEST FAIL (TR pattern missing ok=1/sel tightening: $t)"
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
  @("U1-good", $u1, "PERMAUDIT ok=1 phys=0x80000000 user=0 nx=0 w=1`n", $true, "user/nx shape passes"),
  @("U1-tab", $u1, "PERMAUDIT ok=1 phys=0x80000000 user=0`t nx=0 w=1`n", $true, "tab separator passes"),
  @("U1-good-flip", $u1, "PERMAUDIT ok=1 phys=0x80000000 user=1 nx=1 w=1`n", $true, "flipped values still pass shape"),
  @("U1-baduser", $u1, "PERMAUDIT ok=1 phys=0x80000000 user=2 nx=0 w=1`n", $false, "user=2 fails"),
  @("U1-ten", $u1, "PERMAUDIT ok=1 phys=0x80000000 user=10 nx=0 w=1`n", $false, "user=10 fails"),
  @("U1-onex", $u1, "PERMAUDIT ok=1 phys=0x80000000 user=1x nx=0 w=1`n", $false, "user=1x fails"),
  @("U1-nouser", $u1, "PERMAUDIT ok=1 phys=0x80000000 nx=0 w=1`n", $false, "missing user fails"),
  @("U1-badnx", $u1, "PERMAUDIT ok=1 phys=0x80000000 user=0 nx=2 w=1`n", $false, "nx=2 fails"),
  @("U1-split", $u1, "PERMAUDIT ok=1 phys=0x80000000 user=0`n nx=0 w=1`n", $false, "newline-split fails"),
  @("U2-good", $u2, "PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=0 w=1`n", $true, "user/nx shape passes"),
  @("U2-tab", $u2, "PERMAUDIT2 ok=1 phys=0x1000 user=0`t nx=0 w=1`n", $true, "tab separator passes"),
  @("U2-good-flip", $u2, "PERMAUDIT2 ok=1 phys=0x1000 user=1 nx=1 w=1`n", $true, "flipped values still pass shape"),
  @("U2-baduser", $u2, "PERMAUDIT2 ok=1 phys=0x1000 user=2 nx=0 w=1`n", $false, "user=2 fails"),
  @("U2-ten", $u2, "PERMAUDIT2 ok=1 phys=0x1000 user=10 nx=0 w=1`n", $false, "user=10 fails"),
  @("U2-onex", $u2, "PERMAUDIT2 ok=1 phys=0x1000 user=1x nx=0 w=1`n", $false, "user=1x fails"),
  @("U2-nouser", $u2, "PERMAUDIT2 ok=1 phys=0x1000 nx=0 w=1`n", $false, "missing user fails"),
  @("U2-badnx", $u2, "PERMAUDIT2 ok=1 phys=0x1000 user=0 nx=2 w=1`n", $false, "nx=2 fails"),
  @("U2-split", $u2, "PERMAUDIT2 ok=1 phys=0x1000 user=0`n nx=0 w=1`n", $false, "newline-split fails"),
  @("G-good", $g, "GUARD ok=1 passed=7/7`n", $true, "ok=1 7/7 passes"),
  @("G-ten", $g, "GUARD ok=10 passed=7/7`n", $false, "ok=10 fails"),
  @("G-onex", $g, "GUARD ok=1x passed=7/7`n", $false, "ok=1x fails"),
  @("G-six", $g, "GUARD ok=1 passed=6/7`n", $false, "6/7 fails"),
  @("G-nopass", $g, "GUARD ok=1`n", $false, "missing passed fails"),
  @("G-split", $g, "GUARD ok=1`n passed=7/7`n", $false, "newline-split fails"),
  @("U-good", $u, "USER ok=1 passed=9/9`n", $true, "ok=1 9/9 passes"),
  @("U-ten", $u, "USER ok=10 passed=9/9`n", $false, "ok=10 fails"),
  @("U-onex", $u, "USER ok=1x passed=9/9`n", $false, "ok=1x fails"),
  @("U-six", $u, "USER ok=1 passed=8/9`n", $false, "8/9 fails"),
  @("U-seven", $u, "USER ok=1 passed=7/7`n", $false, "old 7/7 fails (NX shrink)"),
  @("U-nopass", $u, "USER ok=1`n", $false, "missing passed fails"),
  @("U-split", $u, "USER ok=1`n passed=9/9`n", $false, "newline-split fails"),
  @("R-good", $r, "RMBZ ok=1 nxen=1 walks=2/2 viol=0`n", $true, "ok=1 viol=0 passes"),
  @("R-viol10", $r, "RMBZ ok=1 nxen=1 walks=2/2 viol=10`n", $false, "viol=10 fails fail-closed"),
  @("R-viol1", $r, "RMBZ ok=1 nxen=1 walks=2/2 viol=1`n", $false, "viol=1 fails fail-closed"),
  @("R-ten", $r, "RMBZ ok=10 nxen=1 walks=2/2 viol=0`n", $false, "ok=10 fails"),
  @("R-onex", $r, "RMBZ ok=1x nxen=1 walks=2/2 viol=0`n", $false, "ok=1x fails"),
  @("R-ok0viol1", $r, "RMBZ ok=0 nxen=1 walks=2/2 viol=1`n", $false, "ok=0 viol=1 fails"),
  @("R-noviol", $r, "RMBZ ok=1 nxen=1 walks=2/2`n", $false, "missing viol fails"),
  @("R-split", $r, "RMBZ ok=1 nxen=1 walks=2/2`n viol=0`n", $false, "newline-split fails"),
  @("F-good", $f, "PFUS ok=1 us=0 w=0 p=0`n", $true, "ok=1 us=0 w=0 p=0 passes"),
  @("F-us1", $f, "PFUS ok=1 us=1 w=0 p=0`n", $false, "us=1 fails fail-closed"),
  @("F-w1", $f, "PFUS ok=1 us=0 w=1 p=0`n", $false, "w=1 fails fail-closed"),
  @("F-p1", $f, "PFUS ok=1 us=0 w=0 p=1`n", $false, "p=1 fails fail-closed"),
  @("F-ten", $f, "PFUS ok=10 us=0 w=0 p=0`n", $false, "ok=10 fails"),
  @("F-nop", $f, "PFUS ok=1 us=0 w=0`n", $false, "missing p fails"),
  @("F-split", $f, "PFUS ok=1 us=0 w=0`n p=0`n", $false, "newline-split fails"),
  @("Y-good", $y, "TRYSTORE low=0 high=1 lcode=0x0 hcode=0x3 rok=1`n", $true, "low/high/code/rok passes"),
  @("Y-low1", $y, "TRYSTORE low=1 high=1 lcode=0x0 hcode=0x3 rok=1`n", $false, "low=1 fails fail-closed"),
  @("Y-high0", $y, "TRYSTORE low=0 high=0 lcode=0x0 hcode=0x3 rok=1`n", $false, "high=0 fails fail-closed"),
  @("Y-badhcode", $y, "TRYSTORE low=0 high=1 lcode=0x0 hcode=0x7 rok=1`n", $false, "wrong hcode fails"),
  @("Y-rok0", $y, "TRYSTORE low=0 high=1 lcode=0x0 hcode=0x3 rok=0`n", $false, "rok=0 fails fail-closed"),
  @("Y-rok10", $y, "TRYSTORE low=0 high=1 lcode=0x0 hcode=0x3 rok=10`n", $false, "rok=10 fails"),
  @("Y-norok", $y, "TRYSTORE low=0 high=1 lcode=0x0 hcode=0x3`n", $false, "missing rok fails"),
  @("Y-split", $y, "TRYSTORE low=0 high=1 lcode=0x0 hcode=0x3`n rok=1`n", $false, "newline-split fails"),
  @("N-good", $yn, "TRYSTORE-NMI ok=1 nmi=0 mce=0`n", $true, "ok=1 nmi=0 mce=0 passes"),
  @("N-nmi1", $yn, "TRYSTORE-NMI ok=1 nmi=1 mce=0`n", $false, "nmi=1 fails fail-closed"),
  @("N-mce1", $yn, "TRYSTORE-NMI ok=1 nmi=0 mce=1`n", $false, "mce=1 fails fail-closed"),
  @("N-ok0", $yn, "TRYSTORE-NMI ok=0 nmi=0 mce=0`n", $false, "ok=0 fails fail-closed"),
  @("N-ten", $yn, "TRYSTORE-NMI ok=10 nmi=0 mce=0`n", $false, "ok=10 fails"),
  @("N-nomce", $yn, "TRYSTORE-NMI ok=1 nmi=0`n", $false, "missing mce fails"),
  @("N-split", $yn, "TRYSTORE-NMI ok=1 nmi=0`n mce=0`n", $false, "newline-split fails"),
  @("T-good", $t, "TR ok=1 sel=0x28`n", $true, "ok=1 sel=0x28 passes"),
  @("T-ok0", $t, "TR ok=0 sel=0x0`n", $false, "ok=0 fails fail-closed"),
  @("T-ten", $t, "TR ok=10 sel=0x28`n", $false, "ok=10 fails"),
  @("T-onex", $t, "TR ok=1x sel=0x28`n", $false, "ok=1x fails"),
  @("T-badsel", $t, "TR ok=1 sel=0x30`n", $false, "wrong sel fails"),
  @("T-split", $t, "TR ok=1`n sel=0x28`n", $false, "newline-split fails"),
  @("S-good-refuse-perms", $s, "USTACK ok=0 label=perms-refused`n", $true, "ok=0 perms-refused passes baseline"),
  @("S-good-refuse-walk", $s, "USTACK ok=0 label=walk-failed`n", $true, "ok=0 walk-failed passes baseline"),
  @("S-wired-ok1", $s, "USTACK ok=1 label=valid`n", $false, "ok=1 valid fails baseline gate"),
  @("S-drift", $s, "USTACK ok=0 label=valid`n", $false, "ok=0 label=valid drift fails"),
  @("S-wronglabel", $s, "USTACK ok=1 label=perms-refused`n", $false, "ok=1 wrong label fails"),
  @("S-nolabel", $s, "USTACK ok=0`n", $false, "missing label fails"),
  @("S-split", $s, "USTACK ok=0`n label=perms-refused`n", $false, "newline-split fails")
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
Write-Output "GATE TEST PASS (78/78)"
exit 0
