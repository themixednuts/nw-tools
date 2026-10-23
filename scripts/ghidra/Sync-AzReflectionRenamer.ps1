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
$body = [System.Collections.Generic.List[string]]::new()
foreach ($name in @('AzReflectionRenamer.java', 'GhidraCli.java')) {
    $sourceText = Get-Content -LiteralPath (Join-Path $SourceDir $name) -Raw
    if ([string]::IsNullOrWhiteSpace($sourceText) -or !$sourceText.TrimEnd().EndsWith('}')) {
        throw "Source is empty or incomplete: $name. Installed scripts were not changed."
    }
    foreach ($line in $sourceText -split '\r?\n') {
        if ($line -match '^\s*import\s+.+;\s*$') {
            $imports[$line.Trim()] = $true
        }
        elseif ($line -match '^//@category' -or $line -match '^// Import .*reflection') {
            continue
        }
        else {
            $body.Add($line)
        }
    }
}
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
