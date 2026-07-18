param(
    [string] $GhidraHome = $env:GHIDRA_HOME,
    [string] $SourceDir = $PSScriptRoot
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($GhidraHome)) {
    throw 'Pass -GhidraHome or set GHIDRA_HOME.'
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
$argsFile = Join-Path $workDir 'javac.args'

try {
    New-Item -ItemType Directory -Path $bundleDir, $classDir -Force | Out-Null
    & (Join-Path $SourceDir 'Sync-NetworkSchemaExtractor.ps1') `
        -SourceDir $SourceDir `
        -OutDir $bundleDir | Out-Null

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

    Write-Output 'NetworkSchemaExtractor.java compiled successfully.'
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
