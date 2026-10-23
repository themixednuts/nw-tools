<#
.SYNOPSIS
Installs the unified reflection importer and removes its obsolete entrypoints.
#>
[CmdletBinding()]
param(
    [string] $SourceDir = $PSScriptRoot,
    [string] $OutDir = "$HOME/ghidra_scripts",
    [switch] $Force,
    [switch] $DryRun
)

$ErrorActionPreference = 'Stop'
$bundlePath = Join-Path $OutDir 'AzReflectionRenamer.java'
if ((Test-Path -LiteralPath $bundlePath) -and !$Force -and !$DryRun) {
    throw "Output already exists: $bundlePath. Pass -Force to replace it."
}

$imports = [ordered] @{}
$sources = [ordered] @{}
foreach ($name in @('AzReflectionRenamer.java', 'GhidraCli.java')) {
    $sourceText = Get-Content -LiteralPath (Join-Path $SourceDir $name) -Raw
    if ([string]::IsNullOrWhiteSpace($sourceText) -or !$sourceText.TrimEnd().EndsWith('}')) {
        throw "Source is empty or incomplete: $name. Installed scripts were not changed."
    }
    $lines = [System.Collections.Generic.List[string]]::new()
    foreach ($line in $sourceText.TrimEnd() -split '\r?\n') {
        if ($line -match '^\s*import\s+.+;\s*$') {
            $imports[$line.Trim()] = $true
        }
        elseif ($line -match '^//@category' -or $line -match '^// Import .*reflection') {
            continue
        }
        else {
            $lines.Add($line)
        }
    }
    $sources[$name] = $lines
}

# Ghidra compiles each script in the directory into a shared bundle and keeps only the classes
# named for a script file; with another script beside it, a second top-level GhidraCli is dropped
# and the run fails with NoClassDefFoundError. Nested classes travel with their script.
$helper = $sources['GhidraCli.java']
$declaration = $helper.FindIndex([Predicate[string]] { param($line) $line -match '^final class GhidraCli\s*\{' })
if ($declaration -lt 0) {
    throw "GhidraCli.java no longer declares 'final class GhidraCli'. Installed scripts were not changed."
}
$body = [System.Collections.Generic.List[string]]::new()
$renamer = $sources['AzReflectionRenamer.java']
$body.AddRange($renamer.GetRange(0, $renamer.Count - 1))
$body.Add('')
for ($i = $declaration; $i -lt $helper.Count; $i++) {
    $line = if ($i -eq $declaration) { $helper[$i] -replace '^final class', 'static final class' } else { $helper[$i] }
    $body.Add($(if ($line.Length -gt 0) { '    ' + $line } else { $line }))
}
$body.Add($renamer[$renamer.Count - 1])
if ($DryRun) {
    Write-Output "Dry run: would bundle reflection importer into $bundlePath and remove its obsolete wrappers"
    return
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$output = @('// Import names, datatypes and layouts from AZ reflection resources.', '//@category NewWorld', '')
$output += @($imports.Keys)
$output += ''
$output += @($body)
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
$pending = [ordered] @{}
try {
    $bundleTemp = Join-Path $OutDir ('.AzReflectionRenamer.' + [guid]::NewGuid() + '.tmp')
    $pending[$bundleTemp] = $bundlePath
    [System.IO.File]::WriteAllLines($bundleTemp, [string[]] $output, $utf8NoBom)
    foreach ($temporary in $pending.Keys) {
        $destination = $pending[$temporary]
        if (Test-Path -LiteralPath $destination) {
            [System.IO.File]::Replace($temporary, $destination,
                [System.Management.Automation.Language.NullString]::Value)
        }
        else {
            [System.IO.File]::Move($temporary, $destination)
        }
        Write-Output $destination
    }
    foreach ($name in @('AzSerializeContainers.java', 'AzRepairReflectionNames.java', 'AzBuildReflectionDatatypes.java')) {
        $obsolete = Join-Path $OutDir $name
        if (Test-Path -LiteralPath $obsolete) {
            Remove-Item -LiteralPath $obsolete -Force
            Write-Output "Removed obsolete entrypoint: $obsolete"
        }
    }
}
finally {
    foreach ($temporary in $pending.Keys) {
        if (Test-Path -LiteralPath $temporary) {
            Remove-Item -LiteralPath $temporary -Force
        }
    }
}
