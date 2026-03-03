#!/usr/bin/env pwsh
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RootDir = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
Set-Location $RootDir

$ManifestPath = Join-Path $PSScriptRoot 'test-contracts.json'
if (-not (Test-Path $ManifestPath)) {
    throw "Missing manifest: $ManifestPath"
}

$manifest = Get-Content -Raw -Path $ManifestPath | ConvertFrom-Json
if (-not $manifest.packs) {
    throw "Manifest has no packs: $ManifestPath"
}

function Show-Usage {
@'
Usage: scripts/dev/test-select.ps1 <command> [args]

Commands:
  list
      List available validation packs.

  show <pack-id>
      Show description + exact commands for one pack.

  run <pack-id> [--dry-run] [--quiet]
      Run all commands in one pack.

  run-many <pack-id> [<pack-id> ...] [--dry-run] [--quiet]
      Run multiple packs in order.

  run-all [--dry-run] [--quiet]
      Run every pack in manifest order.

  changed [--base <git-ref>]
      Print changed-file set used for affected-pack matching.

  changed [--scope <all|base|worktree|staged|unstaged|untracked>] [--base <git-ref>] [--quiet]
      Print changed-file set with explicit scope control.

  suggest [--scope <all|base|worktree|staged|unstaged|untracked>] [--base <git-ref>] [--quiet]
      Suggest packs based on changed files in the working tree.

  run-affected [--dry-run] [--scope <all|base|worktree|staged|unstaged|untracked>] [--base <git-ref>] [--quiet]
      Run packs suggested by changed files in the working tree.

Examples:
  pwsh -NoProfile -File scripts/dev/test-select.ps1 list
  pwsh -NoProfile -File scripts/dev/test-select.ps1 show camera-lock
  pwsh -NoProfile -File scripts/dev/test-select.ps1 run input-routing
    pwsh -NoProfile -File scripts/dev/test-select.ps1 suggest --base origin/main
    pwsh -NoProfile -File scripts/dev/test-select.ps1 suggest --scope staged
        pwsh -NoProfile -File scripts/dev/test-select.ps1 run-affected --scope worktree --dry-run --quiet
'@ | Write-Host
}

function Get-PackById([string]$id) {
    return @($manifest.packs | Where-Object { $_.id -eq $id })
}

function Invoke-Pack([object]$pack, [bool]$dryRun, [bool]$quiet = $false) {
    if ($quiet) {
        if ($dryRun) {
            Write-Host ("dry-run: {0}" -f $pack.id)
        } else {
            Write-Host ("run: {0}" -f $pack.id)
        }
    } else {
        Write-Host "== pack: $($pack.id) =="
        Write-Host "   $($pack.description)"
    }

    foreach ($cmd in $pack.commands) {
        if (-not $quiet) {
            Write-Host "-> $cmd"
        }
        if (-not $dryRun) {
            & pwsh -NoProfile -Command $cmd
            if ($LASTEXITCODE -ne 0) {
                throw "Command failed in pack '$($pack.id)': $cmd"
            }
        }
    }
}

function Invoke-GitLineOutput([string[]]$gitArgs) {
    $output = @(& git @gitArgs 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw ("git {0} failed with exit code {1}`n{2}" -f ($gitArgs -join ' '), $LASTEXITCODE, ($output -join "`n"))
    }

    $lines = @()
    foreach ($line in $output) {
        $text = [string]$line
        if ([string]::IsNullOrWhiteSpace($text)) {
            continue
        }

        if (
            $text.StartsWith('warning: in the working copy of') -or
            $text.StartsWith('warning: LF will be replaced by CRLF') -or
            $text.StartsWith('warning: CRLF will be replaced by LF')
        ) {
            continue
        }

        $lines += $text
    }

    return @($lines)
}

function Resolve-SelectorOptionsFromArgs([string[]]$arguments) {
    $baseRef = $null
    $scope = 'all'
    $quiet = $false
    $validScopes = @('all', 'base', 'worktree', 'staged', 'unstaged', 'untracked')
    $cleaned = New-Object System.Collections.Generic.List[string]

    for ($i = 0; $i -lt $arguments.Count; $i++) {
        $arg = $arguments[$i]
        if ($arg -eq '--base') {
            if ($i + 1 -ge $arguments.Count) {
                throw 'Expected value after --base'
            }
            $baseRef = $arguments[$i + 1]
            $i++
            continue
        }
        if ($arg -eq '--scope') {
            if ($i + 1 -ge $arguments.Count) {
                throw 'Expected value after --scope'
            }

            $candidate = [string]$arguments[$i + 1]
            if ($validScopes -notcontains $candidate) {
                throw ("Invalid --scope '{0}'. Valid values: {1}" -f $candidate, ($validScopes -join ', '))
            }

            $scope = $candidate
            $i++
            continue
        }
        if ($arg -eq '--quiet') {
            $quiet = $true
            continue
        }
        [void]$cleaned.Add($arg)
    }

    return @{ BaseRef = $baseRef; Scope = $scope; Quiet = $quiet; Args = @($cleaned) }
}

function Get-ChangedPaths([string]$baseRef, [string]$scope) {
    $paths = New-Object 'System.Collections.Generic.HashSet[string]'

    if (($scope -eq 'all' -or $scope -eq 'base')) {
        if ([string]::IsNullOrWhiteSpace($baseRef)) {
            if ($scope -eq 'base') {
                throw "--scope base requires --base <git-ref>"
            }
        } else {
            $baseline = Invoke-GitLineOutput -gitArgs @('diff', '--name-only', '--relative', "$baseRef...HEAD")
            foreach ($p in $baseline) {
                if (-not [string]::IsNullOrWhiteSpace($p)) {
                    [void]$paths.Add(($p -replace '\\', '/'))
                }
            }
        }
    }

    if ($scope -eq 'all' -or $scope -eq 'worktree' -or $scope -eq 'unstaged') {
        $unstaged = Invoke-GitLineOutput -gitArgs @('diff', '--name-only', '--relative', 'HEAD')
        foreach ($p in $unstaged) {
            if (-not [string]::IsNullOrWhiteSpace($p)) {
                [void]$paths.Add(($p -replace '\\', '/'))
            }
        }
    }

    if ($scope -eq 'all' -or $scope -eq 'worktree' -or $scope -eq 'staged') {
        $staged = Invoke-GitLineOutput -gitArgs @('diff', '--name-only', '--relative', '--cached')
        foreach ($p in $staged) {
            if (-not [string]::IsNullOrWhiteSpace($p)) {
                [void]$paths.Add(($p -replace '\\', '/'))
            }
        }
    }

    if ($scope -eq 'all' -or $scope -eq 'worktree' -or $scope -eq 'untracked') {
        $untracked = Invoke-GitLineOutput -gitArgs @('ls-files', '--others', '--exclude-standard')
        foreach ($p in $untracked) {
            if (-not [string]::IsNullOrWhiteSpace($p)) {
                [void]$paths.Add(($p -replace '\\', '/'))
            }
        }
    }

    return @($paths | Sort-Object)
}

function Get-ChangeSetLabel([string]$scope, [string]$baseRef) {
    $withBase = -not [string]::IsNullOrWhiteSpace($baseRef)
    if ($scope -eq 'all') {
        if ($withBase) {
            return ("Changed files (scope=all, base='{0}'):" -f $baseRef)
        }
        return 'Changed files (scope=all):'
    }

    if ($scope -eq 'base') {
        return ("Changed files (scope=base, base='{0}'):" -f $baseRef)
    }

    return ("Changed files (scope={0}):" -f $scope)
}

function Get-AffectedPacks([string[]]$changedPaths) {
    $affected = @()
    foreach ($pack in $manifest.packs) {
        $matches = @($pack.matchPaths)
        if ($matches.Count -eq 0) {
            continue
        }

        $hit = $false
        foreach ($changed in $changedPaths) {
            foreach ($needle in $matches) {
                $needleNorm = ($needle -replace '\\', '/')
                if ($changed -like "*$needleNorm*") {
                    $hit = $true
                    break
                }
            }
            if ($hit) { break }
        }

        if ($hit) {
            $affected += $pack
        }
    }

    return @($affected)
}

$command = if ($args.Count -gt 0) { $args[0] } else { 'list' }
$rest = if ($args.Count -gt 1) { @($args[1..($args.Count - 1)]) } else { @() }
$rest = @($rest)

switch ($command) {
    'list' {
        Write-Host "Validation packs:"
        foreach ($pack in $manifest.packs) {
            $count = @($pack.commands).Count
            Write-Host ("- {0} ({1} commands): {2}" -f $pack.id, $count, $pack.description)
        }
    }
    'show' {
        if ($rest.Count -lt 1) {
            throw 'Usage: show <pack-id>'
        }
        $id = $rest[0]
        $pack = Get-PackById $id
        if ($pack.Count -eq 0) {
            throw "Unknown pack id: $id"
        }
        if ($pack.Count -gt 1) {
            throw "Duplicate pack id in manifest: $id"
        }
        $item = $pack[0]
        Write-Host "id: $($item.id)"
        Write-Host "description: $($item.description)"
        Write-Host 'commands:'
        foreach ($cmd in $item.commands) {
            Write-Host "- $cmd"
        }
    }
    'run' {
        if ($rest.Count -lt 1) {
            throw 'Usage: run <pack-id> [--dry-run]'
        }
        $id = $rest[0]
        $dryRun = $rest -contains '--dry-run'
        $quiet = $rest -contains '--quiet'
        $pack = Get-PackById $id
        if ($pack.Count -eq 0) {
            throw "Unknown pack id: $id"
        }
        if ($pack.Count -gt 1) {
            throw "Duplicate pack id in manifest: $id"
        }
        Invoke-Pack -pack $pack[0] -dryRun $dryRun -quiet $quiet
    }
    'run-many' {
        if ($rest.Count -lt 1) {
            throw 'Usage: run-many <pack-id> [<pack-id> ...] [--dry-run]'
        }
        $dryRun = $rest -contains '--dry-run'
        $quiet = $rest -contains '--quiet'
        $ids = @($rest | Where-Object { $_ -ne '--dry-run' -and $_ -ne '--quiet' })
        foreach ($id in $ids) {
            $pack = Get-PackById $id
            if ($pack.Count -eq 0) {
                throw "Unknown pack id: $id"
            }
            if ($pack.Count -gt 1) {
                throw "Duplicate pack id in manifest: $id"
            }
            Invoke-Pack -pack $pack[0] -dryRun $dryRun -quiet $quiet
        }
    }
    'run-all' {
        $dryRun = $rest -contains '--dry-run'
        $quiet = $rest -contains '--quiet'
        foreach ($pack in $manifest.packs) {
            Invoke-Pack -pack $pack -dryRun $dryRun -quiet $quiet
        }
    }
    'changed' {
        $parsed = Resolve-SelectorOptionsFromArgs -arguments $rest
        $baseRef = $parsed.BaseRef
        $scope = $parsed.Scope
        $quiet = [bool]$parsed.Quiet
        $changed = @(Get-ChangedPaths -baseRef $baseRef -scope $scope)

        if (-not $quiet) {
            Write-Host (Get-ChangeSetLabel -scope $scope -baseRef $baseRef)
        }

        if ($changed.Count -eq 0) {
            Write-Host '- <none>'
            break
        }

        foreach ($path in $changed) {
            Write-Host "- $path"
        }
    }
    'suggest' {
        $parsed = Resolve-SelectorOptionsFromArgs -arguments $rest
        $baseRef = $parsed.BaseRef
        $scope = $parsed.Scope
        $quiet = [bool]$parsed.Quiet
        $changed = @(Get-ChangedPaths -baseRef $baseRef -scope $scope)
        if ($changed.Count -eq 0) {
            Write-Host 'No changed files detected.'
            break
        }

        if (-not $quiet) {
            Write-Host (Get-ChangeSetLabel -scope $scope -baseRef $baseRef)
            foreach ($path in $changed) {
                Write-Host "- $path"
            }
        }

        $affected = Get-AffectedPacks -changedPaths $changed
        if ($affected.Count -eq 0) {
            Write-Host 'No matching packs found for changed files.'
            break
        }

        if ($quiet) {
            foreach ($pack in $affected) {
                Write-Host ("suggested: {0}" -f $pack.id)
            }
        } else {
            Write-Host 'Suggested packs:'
            foreach ($pack in $affected) {
                Write-Host ("- {0}: {1}" -f $pack.id, $pack.description)
            }
        }
    }
    'run-affected' {
        $parsed = Resolve-SelectorOptionsFromArgs -arguments $rest
        $baseRef = $parsed.BaseRef
        $scope = $parsed.Scope
        $quiet = [bool]$parsed.Quiet
        $effectiveArgs = @($parsed.Args)
        $dryRun = $effectiveArgs -contains '--dry-run'
        $changed = @(Get-ChangedPaths -baseRef $baseRef -scope $scope)
        if ($changed.Count -eq 0) {
            Write-Host 'No changed files detected. Nothing to run.'
            break
        }

        $affected = Get-AffectedPacks -changedPaths $changed
        if ($affected.Count -eq 0) {
            Write-Host 'No matching packs found for changed files. Nothing to run.'
            break
        }

        foreach ($pack in $affected) {
            Invoke-Pack -pack $pack -dryRun $dryRun -quiet $quiet
        }
    }
    'help' { Show-Usage }
    '-h' { Show-Usage }
    '--help' { Show-Usage }
    default {
        throw "Unknown command: $command"
    }
}
