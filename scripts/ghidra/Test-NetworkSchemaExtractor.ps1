<#
.SYNOPSIS
Compiles the generated NetworkSchemaExtractor without running binary analysis.
.PARAMETER GhidraHome
Ghidra installation root. Defaults from NW_GHIDRA_HOME.
.PARAMETER SourceDir
Directory containing the checked-in Ghidra script sources.
#>
[CmdletBinding()]
param(
    [string] $GhidraHome = $env:NW_GHIDRA_HOME,
    [string] $SourceDir = $PSScriptRoot,
    [switch] $Version
)

$ErrorActionPreference = 'Stop'

if ($Version) {
    Write-Output 'Test-NetworkSchemaExtractor 1.0.0'
    return
}

if ([string]::IsNullOrWhiteSpace($GhidraHome)) {
    throw 'Pass -GhidraHome or set NW_GHIDRA_HOME.'
}

$ghidraRoot = (Resolve-Path -LiteralPath $GhidraHome).Path
$jars = Get-ChildItem -LiteralPath $ghidraRoot -Recurse -Filter '*.jar' -File |
    ForEach-Object { $_.FullName.Replace('\', '/') }
if ($jars.Count -eq 0) {
    throw "No Ghidra jars found under: $ghidraRoot"
}

$tempRoot = (Resolve-Path -LiteralPath $env:TEMP).Path
$workDir = Join-Path $tempRoot ("nw-network-schema-javac-" + [guid]::NewGuid())
$bundleDir = Join-Path $workDir 'scripts'
$classDir = Join-Path $workDir 'classes'
$standaloneClassDir = Join-Path $workDir 'standalone-classes'
$argsFile = Join-Path $workDir 'javac.args'
$standaloneArgsFile = Join-Path $workDir 'javac-standalone.args'

try {
    New-Item -ItemType Directory -Path $bundleDir, $classDir, $standaloneClassDir -Force | Out-Null
    & (Join-Path $SourceDir 'Sync-NetworkSchemaExtractor.ps1') `
        -SourceDir $SourceDir `
        -OutDir $bundleDir `
        -Force | Out-Null

    $source = (Join-Path $bundleDir 'NetworkSchemaExtractor.java').Replace('\', '/')
    $classPath = $jars -join ';'
    $arguments = @(
        '-proc:none'
        '-d'
        '"' + $classDir.Replace('\', '/') + '"'
        '-classpath'
        '"' + $classPath + '"'
        '"' + $source + '"'
    )
    [System.IO.File]::WriteAllLines(
        $argsFile,
        $arguments,
        [System.Text.UTF8Encoding]::new($false)
    )

    & javac ('@' + $argsFile)
    if ($LASTEXITCODE -ne 0) {
        throw "javac failed with exit code $LASTEXITCODE"
    }

    $standaloneSources = Get-ChildItem -LiteralPath $SourceDir -Filter '*.java' -File |
        Sort-Object Name |
        ForEach-Object { '"' + $_.FullName.Replace('\', '/') + '"' }
    $standaloneArguments = @(
        '-proc:none'
        '-d'
        '"' + $standaloneClassDir.Replace('\', '/') + '"'
        '-classpath'
        '"' + $classPath + '"'
    ) + $standaloneSources
    [System.IO.File]::WriteAllLines(
        $standaloneArgsFile,
        $standaloneArguments,
        [System.Text.UTF8Encoding]::new($false)
    )

    & javac ('@' + $standaloneArgsFile)
    if ($LASTEXITCODE -ne 0) {
        throw "standalone Ghidra script javac failed with exit code $LASTEXITCODE"
    }

    Write-Output 'Generated and standalone Ghidra scripts compiled successfully.'
}
finally {
    if (Test-Path -LiteralPath $workDir) {
        $resolvedWorkDir = (Resolve-Path -LiteralPath $workDir).Path
        if (!$resolvedWorkDir.StartsWith($tempRoot)) {
            throw "Refusing to remove temporary path outside $tempRoot`: $resolvedWorkDir"
        }
        Remove-Item -LiteralPath $resolvedWorkDir -Recurse -Force
    }
}
