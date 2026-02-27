#!/usr/bin/env pwsh
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RootDir = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Set-Location $RootDir

$Mode = 'check'
$InstallPwsh = $false

foreach ($arg in $args) {
    switch ($arg) {
        '--install' { $Mode = 'install' }
        '--install-pwsh' { $InstallPwsh = $true }
        '-h' { }
        '--help' { }
        default {
            throw "Unknown argument: $arg`nUse --help for usage."
        }
    }
}

if ($args -contains '-h' -or $args -contains '--help') {
@'
Usage: scripts/dev/bootstrap-dev-env.ps1 [--install] [--install-pwsh]

Modes:
  (default)      Check which recommended tools are installed
  --install      Install Windows baseline tools and Rust cargo helpers
  --install-pwsh Install PowerShell if missing (no-op in most Windows setups)

Notes:
  - Uses winget for package installation when available.
  - If winget is unavailable, prints fallback install suggestions.
'@ | Write-Host
    exit 0
}

function Test-HasCommand {
    param([Parameter(Mandatory = $true)][string]$Name)
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Show-ToolStatus {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][string[]]$Commands
    )

    foreach ($cmd in $Commands) {
        $resolved = Get-Command $cmd -ErrorAction SilentlyContinue
        if ($resolved) {
            Write-Host ("  [ok]   {0,-12} -> {1} ({2})" -f $Label, $resolved.Source, $cmd)
            return
        }
    }

    Write-Host ("  [miss] {0,-12}" -f $Label)
}

function Install-WithWinget {
    param([Parameter(Mandatory = $true)][string[]]$Ids)

    if (-not (Test-HasCommand 'winget')) {
        Write-Host '[bootstrap.ps1] winget not found; skipping automatic Windows package install.'
        Write-Host '[bootstrap.ps1] install suggestions:'
        Write-Host '  Git.Git GitHub.cli Rustlang.Rustup BurntSushi.ripgrep sharkdp.fd junegunn.fzf jqlang.jq eza-community.eza sharkdp.bat ajeetdsouza.zoxide'
        return
    }

    foreach ($id in $Ids) {
        Write-Host "[bootstrap.ps1] ensuring $id"
        & winget install --id $id --exact --silent --accept-package-agreements --accept-source-agreements | Out-Host
    }
}

function Install-CargoHelpers {
    if (-not (Test-HasCommand 'cargo')) {
        Write-Host '[bootstrap.ps1] cargo not found, skipping cargo helper installation'
        return
    }

    if (-not (Test-HasCommand 'cargo-binstall')) {
        cargo install cargo-binstall
    }

    if (Test-HasCommand 'cargo-binstall') {
        cargo binstall -y sccache cargo-nextest cargo-watch cargo-edit
    } else {
        cargo install sccache cargo-nextest cargo-watch cargo-edit
    }
}

function Install-Pwsh {
    if (Test-HasCommand 'pwsh') {
        Write-Host "[bootstrap.ps1] pwsh already installed: $((Get-Command pwsh).Source)"
        return
    }

    Install-WithWinget -Ids @('Microsoft.PowerShell')
}

function Print-Header {
    $platformLabel = 'Windows/local lane'
    if ($IsLinux) {
        if ($env:WSL_DISTRO_NAME -or $env:WSL_INTEROP) {
            $platformLabel = 'WSL/Linux lane'
        } else {
            $platformLabel = 'Linux lane'
        }
    } elseif ($IsMacOS) {
        $platformLabel = 'macOS lane'
    }

    Write-Host '[bootstrap.ps1] Graphshell dev environment helper'
    Write-Host "[bootstrap.ps1] repo: $RootDir"
    Write-Host "[bootstrap.ps1] mode: $Mode"
    Write-Host "[bootstrap.ps1] platform: $platformLabel"
}

function Check-Tools {
    Write-Host '[bootstrap.ps1] checking core tools'
    Show-ToolStatus -Label 'git' -Commands @('git')
    Show-ToolStatus -Label 'gh' -Commands @('gh')
    Show-ToolStatus -Label 'jq' -Commands @('jq')
    Show-ToolStatus -Label 'rg' -Commands @('rg')
    Show-ToolStatus -Label 'fd' -Commands @('fd')
    Show-ToolStatus -Label 'fzf' -Commands @('fzf')
    Show-ToolStatus -Label 'bat' -Commands @('bat')
    Show-ToolStatus -Label 'zoxide' -Commands @('zoxide')
    Show-ToolStatus -Label 'eza' -Commands @('eza')
    Show-ToolStatus -Label 'rustc' -Commands @('rustc')
    Show-ToolStatus -Label 'cargo' -Commands @('cargo')
    Show-ToolStatus -Label 'rustup' -Commands @('rustup')
    Show-ToolStatus -Label 'pwsh' -Commands @('pwsh')

    Write-Host '[bootstrap.ps1] checking optional cargo helpers'
    Show-ToolStatus -Label 'cargo-binstall' -Commands @('cargo-binstall')
    Show-ToolStatus -Label 'sccache' -Commands @('sccache')
    Show-ToolStatus -Label 'cargo-nextest' -Commands @('cargo-nextest')
    Show-ToolStatus -Label 'cargo-watch' -Commands @('cargo-watch')
    Show-ToolStatus -Label 'cargo-add' -Commands @('cargo-add')
}

function Install-WindowsBaseline {
    Install-WithWinget -Ids @(
        'Git.Git',
        'GitHub.cli',
        'Rustlang.Rustup',
        'BurntSushi.ripgrep',
        'sharkdp.fd',
        'junegunn.fzf',
        'jqlang.jq',
        'eza-community.eza',
        'sharkdp.bat',
        'ajeetdsouza.zoxide'
    )

    Install-CargoHelpers
}

function Print-NextSteps {
@'

[bootstrap.ps1] recommended aliases/profile helpers:
  Set-Alias c cargo
  function cc { cargo check -q }
  function ct { cargo test -q }

[bootstrap.ps1] Graphshell lane-safe commands:
  pwsh -File scripts/dev/smoke-matrix.ps1 status
  pwsh -File scripts/dev/smoke-matrix.ps1 quick
  pwsh -File scripts/dev/smoke-matrix.ps1 cargo build --release
'@ | Write-Host
}

Print-Header
if ($InstallPwsh) {
    Install-Pwsh
}
if ($Mode -eq 'install') {
    Install-WindowsBaseline
}
Check-Tools
Print-NextSteps
