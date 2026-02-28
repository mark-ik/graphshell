#!/usr/bin/env pwsh
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RootDir = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
Set-Location $RootDir

if (-not $env:TARGET_TEST -or [string]::IsNullOrWhiteSpace($env:TARGET_TEST)) {
    $env:TARGET_TEST = 'graph_split_intent_tests'
}

function Test-IsWsl {
    if ($env:WSL_DISTRO_NAME -or $env:WSL_INTEROP) { return $true }
    try {
        $kernel = Get-Content -Path '/proc/sys/kernel/osrelease' -ErrorAction Stop | Select-Object -First 1
        return $kernel -match '(?i)microsoft|wsl'
    } catch {
        return $false
    }
}

function Get-HostLane {
    if ($env:GRAPHSHELL_CARGO_LANE) {
        return $env:GRAPHSHELL_CARGO_LANE
    }

    if ($IsWindows) { return 'windows' }
    if ($IsMacOS) { return 'macos' }
    if ($IsLinux) { return 'linux' }
    return 'unknown'
}

function Get-PlatformLabel {
    if (Test-IsWsl) { return 'WSL/Linux lane' }
    if ($IsLinux) { return 'Linux lane' }
    if ($IsMacOS) { return 'macOS lane' }
    if ($IsWindows) { return 'Windows/local lane' }
    return 'Unknown lane'
}

function Get-ResolvedTargetDir {
    $lane = Get-HostLane
    switch ($lane) {
        'linux' {
            $suffix = ''
            if ($env:GRAPHSHELL_LINUX_TARGET_FLAVOR) {
                $suffix = "-$($env:GRAPHSHELL_LINUX_TARGET_FLAVOR)"
            } elseif ((Test-IsWsl) -and $env:GRAPHSHELL_SPLIT_WSL_TARGET) {
                $suffix = '-wsl'
            }
            return (Join-Path $RootDir "target/linux_target$suffix")
        }
        'windows' { return (Join-Path $RootDir 'target/windows_target') }
        'macos' { return (Join-Path $RootDir 'target/macos_target') }
        default { return (Join-Path $RootDir 'target/host_target') }
    }
}

function Set-CargoTargetDir {
    if ($env:CARGO_TARGET_DIR -and -not [string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
        New-Item -ItemType Directory -Force -Path $env:CARGO_TARGET_DIR | Out-Null
        Write-Host "[smoke-matrix.ps1] Using caller-provided CARGO_TARGET_DIR=$($env:CARGO_TARGET_DIR)"
        return
    }

    $env:CARGO_TARGET_DIR = Get-ResolvedTargetDir
    New-Item -ItemType Directory -Force -Path $env:CARGO_TARGET_DIR | Out-Null
    Write-Host "[smoke-matrix.ps1] Using CARGO_TARGET_DIR=$($env:CARGO_TARGET_DIR)"
}

function Set-WslGlFallback {
    if (-not (Test-IsWsl)) { return }
    if ($env:GRAPHSHELL_DISABLE_WSL_SOFTWARE_FALLBACK) { return }

    if (-not $env:LIBGL_ALWAYS_SOFTWARE) { $env:LIBGL_ALWAYS_SOFTWARE = '1' }
    if (-not $env:MESA_LOADER_DRIVER_OVERRIDE) { $env:MESA_LOADER_DRIVER_OVERRIDE = 'llvmpipe' }
    if (-not $env:GALLIUM_DRIVER) { $env:GALLIUM_DRIVER = 'llvmpipe' }

    Write-Host '[smoke-matrix.ps1] WSL detected, software GL fallback enabled.'
}

function Show-Usage {
@'
Usage: scripts/dev/smoke-matrix.ps1 <command> [args...]

Commands:
  status   Print platform/runtime summary
    quick    Run non-GUI validation: cargo check --locked + one targeted lib test
  run      Start graphshell (applies WSL software GL fallback automatically)
  cargo    Run an arbitrary cargo subcommand with managed target dir

Environment knobs:
  TARGET_TEST=<test_name>                         Override targeted test for quick mode
  GRAPHSHELL_CARGO_LANE=<linux|windows|macos>    Override host lane detection
  GRAPHSHELL_LINUX_TARGET_FLAVOR=<name>          Optional linux target suffix (e.g. ubuntu, wsl)
  GRAPHSHELL_SPLIT_WSL_TARGET=1                  Auto-split WSL into linux_target-wsl
  CARGO_TARGET_DIR=<path>                        Fully override target directory selection
'@ | Write-Host
}

$Command = if ($args.Count -gt 0) { $args[0] } else { 'quick' }

switch ($Command) {
    'status' {
        Write-Host "repo: $RootDir"
        try {
            Write-Host "uname: $(uname -a)"
        } catch {
            Write-Host "uname: <unavailable>"
        }
        Write-Host "lane: $(Get-HostLane)"
        Write-Host "resolved target: $(Get-ResolvedTargetDir)"
        try { Write-Host "rust: $(rustc --version)" } catch { Write-Host 'rust: missing' }
        try { Write-Host "cargo: $(cargo --version)" } catch { Write-Host 'cargo: missing' }
        Write-Host "platform: $(Get-PlatformLabel)"
        Write-Host "env LIBGL_ALWAYS_SOFTWARE=$($env:LIBGL_ALWAYS_SOFTWARE ?? '<unset>')"
        Write-Host "env MESA_LOADER_DRIVER_OVERRIDE=$($env:MESA_LOADER_DRIVER_OVERRIDE ?? '<unset>')"
        Write-Host "env GALLIUM_DRIVER=$($env:GALLIUM_DRIVER ?? '<unset>')"
    }
    'quick' {
        Set-CargoTargetDir
        cargo check --locked
        cargo test --locked --lib $env:TARGET_TEST
    }
    'run' {
        Set-CargoTargetDir
        Set-WslGlFallback
        cargo run
    }
    'cargo' {
        if ($args.Count -lt 2) {
            throw 'Usage: scripts/dev/smoke-matrix.ps1 cargo <cargo args...>'
        }
        Set-CargoTargetDir
        $cargoArgs = @($args[1..($args.Count - 1)])
        & cargo @cargoArgs
    }
    'help' { Show-Usage }
    '-h' { Show-Usage }
    '--help' { Show-Usage }
    default {
        throw "Unknown command: $Command"
    }
}
