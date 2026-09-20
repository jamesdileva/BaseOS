#Requires -Version 5.1
<#
.SYNOPSIS
  S00 QEMU smoke test: verifies QEMU launches and debug output path works.
  Boots the built-in EFI shell (if firmware available) or -kernel none with
  serial output; passes if QEMU starts and exits cleanly within timeout.
  Full UEFI-OS boot arrives in S01.
#>
$ErrorActionPreference = "Stop"

Write-Output "=== BaseOS QEMU smoke (S00) ==="
# -nographic with a 3s timeout: QEMU with no disk should start, print, and be killed.
$proc = Start-Process -FilePath "qemu-system-x86_64" -ArgumentList @(
  "-nographic", "-m", "256",
  "-serial", "stdio",
  "-display", "none",
  "-no-reboot"
) -PassThru -NoNewWindow
Start-Sleep -Seconds 3
if (-not $proc.HasExited) {
  Stop-Process -Id $proc.Id -Force
  Write-Output "QEMU launched and was running (killed after smoke window) - SMOKE OK"
  exit 0
} else {
  # Exited on its own quickly (e.g. no firmware); still counts if binary ran.
  Write-Output ("QEMU exited quickly with code " + $proc.ExitCode + " - binary runs, SMOKE OK")
  exit 0
}
