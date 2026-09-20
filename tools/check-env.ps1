#Requires -Version 5.1
<#
.SYNOPSIS
  S00 environment check: toolchain versions verified, QEMU present.
  Exit non-zero on failure.

.DESCRIPTION
  Verifies the active Rust toolchain matches rust-toolchain.toml and that
  the x86_64-unknown-uefi target is installed. Records versions to a
  gitignored path (target/baseos-host-toolchain.txt) so repeated runs do
  not dirty the tracked tree. The tracked baseline at
  research/benchmarks/host-toolchain.txt is left untouched.

  Use -Check for verify-only mode (no file writes).

  Addresses agent-b review #4 finding 2.
#>
param(
  [switch]$Check
)
$ErrorActionPreference = "Stop"

Write-Output "=== BaseOS env check (S00) ==="

# 1. Expected channel from rust-toolchain.toml (single source of truth).
$toolchainFile = Join-Path $PSScriptRoot "../rust-toolchain.toml"
if (-not (Test-Path -LiteralPath $toolchainFile)) {
  Write-Error "Missing rust-toolchain.toml at $toolchainFile"
  exit 1
}
$toml = Get-Content -LiteralPath $toolchainFile -Raw
if ($toml -notmatch 'channel\s*=\s*"([^"]+)"') {
  Write-Error "Could not parse toolchain channel from $toolchainFile"
  exit 1
}
$expected = $Matches[1]
Write-Output "Expected toolchain channel: $expected (from rust-toolchain.toml)"

# 2. Verify rustc/cargo match the pinned channel.
$rustcVer = (rustc --version)
$cargoVer = (cargo --version)
Write-Output $rustcVer
Write-Output $cargoVer
if ($rustcVer -notmatch [regex]::Escape($expected)) {
  Write-Error "rustc version mismatch: got '$rustcVer', expected channel '$expected'"
  exit 1
}
if ($cargoVer -notmatch [regex]::Escape($expected)) {
  Write-Error "cargo version mismatch: got '$cargoVer', expected channel '$expected'"
  exit 1
}
rustup show active-toolchain

# 3. Verify the UEFI target is installed (S01 needs it).
$installed = (rustup target list --installed)
Write-Output ($installed -join "`n")
if ($installed -notcontains "x86_64-unknown-uefi") {
  Write-Error "Missing target x86_64-unknown-uefi. Run: rustup target add x86_64-unknown-uefi"
  exit 1
}

# 4. Verify QEMU + git presence (explicit, fail fast).
foreach ($cmd in @("qemu-system-x86_64", "git")) {
  if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
    Write-Error "Required command not found on PATH: $cmd"
    exit 1
  }
}
qemu-system-x86_64 --version
git --version

if ($Check) {
  Write-Output "ENV CHECK OK (verify-only, no files written)"
  exit 0
}

# 5. Record versions to a gitignored path (does not dirty the tree).
$record = @(
  $rustcVer,
  $cargoVer,
  (qemu-system-x86_64 --version | Select-Object -First 1),
  (git --version)
)
$outDir = Join-Path $PSScriptRoot "../target"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$outFile = Join-Path $outDir "baseos-host-toolchain.txt"
$record | Set-Content -Path $outFile
Write-Output "Wrote $outFile (gitignored; tracked baseline left untouched)"
Write-Output "ENV CHECK OK"
