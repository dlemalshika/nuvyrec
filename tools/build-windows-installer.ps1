[CmdletBinding()]
param(
    [ValidatePattern('^Nuvyrec-Windows(?:-[A-Za-z0-9]+)*$')]
    [string]$BundleName = 'Nuvyrec-Windows-Release',
    [string]$CompilerPath,
    [ValidatePattern('^[A-Za-z0-9-]+$')]
    [string]$OutputFolder = 'github-release'
)
$ErrorActionPreference = 'Stop'
$installerRoot = Split-Path -Parent $PSScriptRoot
$installerBundle = Join-Path $installerRoot "artifacts\$BundleName"
$installerExe = Join-Path $installerBundle 'Nuvyrec.exe'
if (-not (Test-Path -LiteralPath $installerExe -PathType Leaf)) { throw 'Package a Windows bundle first.' }
$installerVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($installerExe).ProductVersion
if ($installerVersion -notmatch '^([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?)(?:\+[0-9A-Za-z.-]+)?$') {
    throw "Unsupported executable version: $installerVersion"
}
$installerTag = $Matches[1]
# Fail before compilation if a file was altered, added or omitted after packaging.
$installerManifest = @(Get-Content -LiteralPath (Join-Path $installerBundle 'manifest.json') -Raw | ConvertFrom-Json)
$installerBoundary = [System.IO.Path]::GetFullPath($installerBundle) + [System.IO.Path]::DirectorySeparatorChar
$installerListed = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($installerFile in $installerManifest) {
    $installerResolved = [System.IO.Path]::GetFullPath((Join-Path $installerBundle $installerFile.path))
    if (-not $installerResolved.StartsWith($installerBoundary, [System.StringComparison]::OrdinalIgnoreCase)) { throw 'Manifest entry outside bundle.' }
    if (-not $installerListed.Add($installerResolved)) { throw 'Duplicate manifest entry.' }
    if ((Get-FileHash -LiteralPath $installerResolved -Algorithm SHA256).Hash.ToLowerInvariant() -ne $installerFile.sha256) {
        throw "Checksum mismatch: $($installerFile.path)"
    }
}
foreach ($installerActual in Get-ChildItem -LiteralPath $installerBundle -File -Recurse) {
    if ($installerActual.Name -eq 'manifest.json' -and $installerActual.DirectoryName -eq $installerBundle) { continue }
    if (-not $installerListed.Contains($installerActual.FullName)) { throw "Unlisted bundle file: $($installerActual.FullName)" }
}
foreach ($installerRequired in @('Nuvyrec.exe','recorder_core.dll','ffmpeg.exe','LICENSE','data\flutter_assets\AssetManifest.bin')) {
    if (-not (Test-Path -LiteralPath (Join-Path $installerBundle $installerRequired) -PathType Leaf)) { throw "Missing $installerRequired" }
}
if (-not $CompilerPath) {
    $installerCandidates = @(
        (Join-Path $installerRoot '.deps\inno-setup\ISCC.exe'),
        'C:\Program Files (x86)\Inno Setup 6\ISCC.exe',
        'C:\Program Files\Inno Setup 7\ISCC.exe'
    )
    $CompilerPath = $installerCandidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
    if (-not $CompilerPath) {
        $installerCommand = Get-Command ISCC.exe -ErrorAction SilentlyContinue
        if ($installerCommand) { $CompilerPath = $installerCommand.Source }
    }
}
if (-not $CompilerPath -or -not (Test-Path -LiteralPath $CompilerPath -PathType Leaf)) {
    throw 'Install Inno Setup 6.3+ from https://jrsoftware.org/isdl.php or supply -CompilerPath.'
}
$installerOutput = Join-Path $installerRoot "artifacts\$OutputFolder"
New-Item -ItemType Directory -Path $installerOutput -Force | Out-Null
$installerAsset = Join-Path $installerOutput "Nuvyrec-Windows-x64-v$installerTag-Setup.exe"
if (Test-Path -LiteralPath $installerAsset) { throw 'Installer already exists; use a new version or a different validated release bundle.' }
$installerInfo = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($installerExe)
$installerBinaryVersion = "$($installerInfo.FileMajorPart).$($installerInfo.FileMinorPart).$($installerInfo.FileBuildPart).$($installerInfo.FilePrivatePart)"
& $CompilerPath "/DBundleDir=$installerBundle" "/DReleaseVersion=$installerTag" "/DFullVersion=$installerVersion" "/DBinaryVersion=$installerBinaryVersion" "/DOutputDir=$installerOutput" (Join-Path $installerRoot 'packaging\windows\frame.iss')
if ($LASTEXITCODE -ne 0) { throw 'Installer compiler failed.' }
$installerDigest = (Get-FileHash -LiteralPath $installerAsset -Algorithm SHA256).Hash.ToLowerInvariant()
"$installerDigest  $([System.IO.Path]::GetFileName($installerAsset))" | Set-Content -LiteralPath "$installerAsset.sha256" -Encoding ascii
Write-Output "Installer: $installerAsset"
Write-Output "Checksum: $installerAsset.sha256"
Write-Output 'Prepared locally; nothing was published or installed for end users.'
