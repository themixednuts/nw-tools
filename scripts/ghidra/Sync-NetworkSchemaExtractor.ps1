param(
    [string] $SourceDir = $PSScriptRoot,
    [string] $OutDir = "$HOME/ghidra_scripts"
)

$ErrorActionPreference = 'Stop'

$sourceNames = @(
    'NetworkSchemaAddressFormatter.java',
    'NetworkSchemaX86.java',
    'NetworkSchemaText.java',
    'NetworkSchemaTextParser.java',
    'NetworkSchemaJson.java',
    'NetworkSchemaModels.java',
    'NetworkSchemaTypeModels.java',
    'NetworkSchemaStack.java',
    'NetworkSchemaPcode.java'
)

if (!(Test-Path -LiteralPath $OutDir)) {
    New-Item -ItemType Directory -Path $OutDir | Out-Null
}

$extractorDir = Join-Path $SourceDir 'network_schema_extractor'
$extractorFragments = Get-ChildItem -LiteralPath $extractorDir -Filter 'NetworkSchemaExtractor.*.javafrag' |
    Sort-Object Name

if ($extractorFragments.Count -eq 0) {
    throw "Missing extractor fragments under: $extractorDir"
}

$maxSourceLines = 1000
$checkedSources = @()
$checkedSources += $sourceNames | ForEach-Object { Join-Path $SourceDir $_ }
$checkedSources += $extractorFragments | ForEach-Object { $_.FullName }
foreach ($path in $checkedSources) {
    $lineCount = (Get-Content -LiteralPath $path | Measure-Object -Line).Lines
    if ($lineCount -gt $maxSourceLines) {
        throw "Source file exceeds $maxSourceLines lines: $path ($lineCount lines)"
    }
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

$bundlePath = Join-Path $OutDir 'NetworkSchemaExtractor.java'
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
