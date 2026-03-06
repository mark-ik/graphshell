#!/usr/bin/env pwsh
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RootDir = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
Set-Location $RootDir

$docRoots = @(
    'design_docs/graphshell_docs/technical_architecture',
    'design_docs/graphshell_docs/implementation_strategy',
    'design_docs/graphshell_docs/design',
    'design_docs/graphshell_docs/testing',
    'design_docs/DOC_README.md'
)

$files = New-Object System.Collections.Generic.List[string]
foreach ($root in $docRoots) {
    if (Test-Path $root -PathType Leaf) {
        [void]$files.Add((Resolve-Path $root).Path)
        continue
    }

    Get-ChildItem -Path $root -Recurse -File -Include *.md | ForEach-Object {
        $path = $_.FullName
        if (
            $path -match '[\\/]archive_docs[\\/]' -or
            $path -match '[\\/]research[\\/]'
        ) {
            return
        }
        [void]$files.Add($path)
    }
}

$violations = New-Object System.Collections.Generic.List[object]

function Add-Violation {
    param(
        [string]$RuleId,
        [string]$Path,
        [int]$LineNumber,
        [string]$LineText,
        [string]$Message
    )

    $violations.Add([pscustomobject]@{
        rule = $RuleId
        path = $Path
        line = $LineNumber
        text = $LineText.Trim()
        message = $Message
    }) | Out-Null
}

foreach ($file in $files) {
    $lines = Get-Content -Path $file
    $headerLines = $lines | Select-Object -First 40
    $fileDeclaresLegacyNamespaceContext = ($headerLines -join "`n") -match 'graphshell://' -and ($headerLines -join "`n") -match 'legacy|compatibility|historical|original|provisional'
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $line = [string]$lines[$i]
        $lineNumber = $i + 1
        $prevLine = if ($i -gt 0) { [string]$lines[$i - 1] } else { '' }
        $nextLine = if ($i + 1 -lt $lines.Count) { [string]$lines[$i + 1] } else { '' }
        $localContext = "$prevLine`n$line`n$nextLine"

        if ($line -match 'Current implementation.*\bEdgeType\b' -or
            $line -match 'Current code.*\bEdgeType\b') {
            Add-Violation `
                -RuleId 'stale-edge-model' `
                -Path $file `
                -LineNumber $lineNumber `
                -LineText $line `
                -Message 'Do not describe EdgeType as the current edge model in active docs.'
        }

        if ($line -match 'three-tier lifecycle' -or
            $line -match 'Active/Warm/Cold three-tier') {
            Add-Violation `
                -RuleId 'stale-lifecycle-term' `
                -Path $file `
                -LineNumber $lineNumber `
                -LineText $line `
                -Message 'Use the canonical four-state lifecycle wording, not three-tier lifecycle wording.'
        }

        if ($line -match 'graphshell://') {
            $allowedLegacyContext = $fileDeclaresLegacyNamespaceContext -or ($localContext -match 'legacy|compatibility|historical|original|alias|provisional|pending')
            if (-not $allowedLegacyContext) {
                Add-Violation `
                    -RuleId 'stale-runtime-namespace' `
                    -Path $file `
                    -LineNumber $lineNumber `
                    -LineText $line `
                    -Message 'Do not present graphshell:// as the canonical runtime namespace in active docs.'
            }
        }
    }
}

if ($violations.Count -gt 0) {
    Write-Host 'Docs parity check failed:' -ForegroundColor Red
    foreach ($v in $violations) {
        $relativePath = Resolve-Path -Relative $v.path
        Write-Host ("- [{0}] {1}:{2}" -f $v.rule, $relativePath, $v.line)
        Write-Host ("  {0}" -f $v.message)
        Write-Host ("  {0}" -f $v.text)
    }
    exit 1
}

Write-Host 'Docs parity check passed.'
