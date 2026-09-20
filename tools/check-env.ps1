#Requires -Version 5.1
<#
.SYNOPSIS
  S00 environment check: toolchain versions recorded, QEMU present.
  Exit non-zero on failure.
#>
$ErrorActionPreference = "Stop"

Write-Output "=== BaseOS env check (S00) ==="
rustc --version
cargo --version
rustup show active-toolchain
qemu-system-x86_64 --version
git --version

# Record versions for reproducibility (stdout + file)
$record = @(
  (rustc --version),
  (cargo --version),
  (qemu-system-x86_64 --version | Select-Object -First 1),
  (git --version)
)
New-Item -ItemType Directory -Force -Path "research/benchmarks" | Out-Null
$record | Set-Content -Path "research/benchmarks/host-toolchain.txt"
Write-Output "Wrote research/benchmarks/host-toolchain.txt"
Write-Output "ENV CHECK OK"
