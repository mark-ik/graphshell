# Compile-matrix for the servo-into-verso engine selectivity lane
# (see design_docs/graphshell_docs/implementation_strategy/shell/
#  2026-04-25_servo_into_verso_plan.md).
#
# Checks the three feature combos that must stay green:
#   1. default                                            (servo + wry, wgpu-only)
#   2. --no-default-features --features wry              (no-Servo Wry only)
#   3. --no-default-features --features iced-host,wry    (no-Servo iced-host + wry)

$ErrorActionPreference = "Stop"

$RootDir = Resolve-Path (Join-Path $PSScriptRoot "..\..")
Set-Location $RootDir

$matrix = @(
  @{ Name = "default";                              Cmd = @("cargo", "check", "-p", "graphshell", "--lib") }
  @{ Name = "no-default --features wry";            Cmd = @("cargo", "check", "--no-default-features", "--features", "wry") }
  @{ Name = "no-default --features iced-host,wry";  Cmd = @("cargo", "check", "--no-default-features", "--features", "iced-host,wry") }
)

$results = @()

foreach ($entry in $matrix) {
  Write-Host "==> $($entry.Name)"
  Write-Host "    $($entry.Cmd -join ' ')"
  & $entry.Cmd[0] $entry.Cmd[1..($entry.Cmd.Length - 1)]
  if ($LASTEXITCODE -eq 0) {
    $results += "PASS  $($entry.Name)"
  } else {
    $results += "FAIL  $($entry.Name)"
  }
}

Write-Host ""
Write-Host "== engine-feature matrix summary =="
$failed = $false
foreach ($r in $results) {
  Write-Host "  $r"
  if ($r.StartsWith("FAIL")) { $failed = $true }
}

if ($failed) { exit 1 } else { exit 0 }
