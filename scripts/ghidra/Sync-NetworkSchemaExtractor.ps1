<#
.SYNOPSIS
Builds the generated NetworkSchemaExtractor Ghidra script.
.PARAMETER SourceDir
Directory containing the checked-in helper sources and fragments.
.PARAMETER OutDir
Directory where NetworkSchemaExtractor.java is written.
.PARAMETER Force
Replaces an existing generated script.
.PARAMETER DryRun
Validates inputs and prints the output plan without changing files.
#>
[CmdletBinding()]
param(
    [string] $SourceDir = $PSScriptRoot,
    [string] $OutDir = "$HOME/ghidra_scripts",
    [switch] $Force,
    [switch] $DryRun,
    [switch] $Version
)

$ErrorActionPreference = 'Stop'

if ($Version) {
    Write-Output 'Sync-NetworkSchemaExtractor 1.0.0'
    return
}

$sourceNames = @(
    'GhidraCli.java',
    'NetworkSchemaAddressFormatter.java',
    'NetworkSchemaX86.java',
    'NetworkSchemaText.java',
    'NetworkSchemaJson.java',
    'NetworkSchemaModels.java',
    'NetworkSchemaTypeModels.java',
    'NetworkSchemaContainerModels.java',
    'NetworkSchemaMessageModels.java',
    'NetworkSchemaStack.java',
    'NetworkSchemaPcode.java'
    'NetworkSchemaControlFlow.java'
    'NetworkSchemaFlowSequence.java'
    'NetworkSchemaLoopSequence.java'
    'NetworkSchemaBranchFlow.java'
    'NetworkSchemaNaturalLoop.java'
    'NetworkSchemaCodecClassifier.java'
    'NetworkSchemaIntegerEvaluator.java'
    'NetworkSchemaPcodeConstants.java'
    'NetworkSchemaWireShape.java'
    'NetworkSchemaRunMetrics.java'
)

$extractorDir = Join-Path $SourceDir 'network_schema_extractor'
$extractorFragments = Get-ChildItem -LiteralPath $extractorDir -Filter 'NetworkSchemaExtractor.*.javafrag' |
    Sort-Object @{ Expression = { $_.Name -eq 'NetworkSchemaExtractor.codec_trace.javafrag' } }, Name

if ($extractorFragments.Count -eq 0) {
    throw "Missing extractor fragments under: $extractorDir"
}

$maxSourceLines = 1000
$checkedSources = @()
$checkedSources += $sourceNames | ForEach-Object { Join-Path $SourceDir $_ }
$checkedSources += $extractorFragments | ForEach-Object { $_.FullName }
foreach ($path in $checkedSources) {
    $lineCount = (Get-Content -LiteralPath $path).Count
    if ($lineCount -gt $maxSourceLines) {
        throw "Source file exceeds $maxSourceLines lines: $path ($lineCount lines)"
    }
}

$bundlePath = Join-Path $OutDir 'NetworkSchemaExtractor.java'
if ((Test-Path -LiteralPath $bundlePath) -and !$Force -and !$DryRun) {
    throw "Output already exists: $bundlePath. Pass -Force to replace it."
}
if ($DryRun) {
    Write-Output "Dry run: would bundle $($checkedSources.Count) source file(s) into $bundlePath"
    return
}
if (!(Test-Path -LiteralPath $OutDir)) {
    New-Item -ItemType Directory -Path $OutDir | Out-Null
}

$imports = [ordered] @{}
$body = [System.Collections.Generic.List[string]]::new()

foreach ($name in $sourceNames) {
    $path = Join-Path $SourceDir $name
    if (!(Test-Path -LiteralPath $path)) {
        throw "Missing source file: $path"
    }

    $lines = Get-Content -LiteralPath $path
    if ($name -ne 'NetworkSchemaExtractor.java') {
        $body.Add('')
        $body.Add("// ---- bundled from $name ----")
    }

    foreach ($line in $lines) {
        if ($line -match '^\s*import\s+.+;\s*$') {
            $imports[$line.Trim()] = $true
            continue
        }
        if ($name -eq 'NetworkSchemaExtractor.java' -and
            ($line -match '^// Extract network type' -or $line -match '^//@category')) {
            continue
        }
        $body.Add($line)
    }
}

foreach ($fragment in $extractorFragments) {
    $body.Add('')
    $body.Add("// ---- bundled from network_schema_extractor/$($fragment.Name) ----")

    foreach ($line in Get-Content -LiteralPath $fragment.FullName) {
        if ($line -match '^\s*import\s+.+;\s*$') {
            $imports[$line.Trim()] = $true
            continue
        }
        if ($line -match '^// Extract network type' -or $line -match '^//@category') {
            continue
        }
        $body.Add($line)
    }
}

$output = [System.Collections.Generic.List[string]]::new()
$output.Add('// Extract network type and field registration evidence from typeregistry.json and Ghidra.')
$output.Add('//@category NewWorld')
$output.Add('')
foreach ($import in $imports.Keys) {
    $output.Add($import)
}
$output.Add('')
foreach ($line in $body) {
    $output.Add($line)
}

$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllLines($bundlePath, [string[]] $output, $utf8NoBom)

foreach ($name in $sourceNames) {
    if ($name -eq 'NetworkSchemaExtractor.java') {
        continue
    }
    $path = Join-Path $OutDir $name
    if (Test-Path -LiteralPath $path) {
        Remove-Item -LiteralPath $path -Force
    }
}

Get-ChildItem -LiteralPath $OutDir -Filter 'NetworkSchemaExtractor.*.javafrag' -ErrorAction SilentlyContinue |
    Remove-Item -Force

$outFragmentDir = Join-Path $OutDir 'network_schema_extractor'
if (Test-Path -LiteralPath $outFragmentDir) {
    Remove-Item -LiteralPath $outFragmentDir -Recurse -Force
}

Write-Output $bundlePath
