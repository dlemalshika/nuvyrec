[CmdletBinding()]
param(
    [ValidatePattern('^Frame-Windows(?:-[A-Za-z0-9]+)*$')]
    [string]$BundleName = 'Frame-Windows-Updates'
)
$ErrorActionPreference = 'Stop'
$releaseWorkspace = Split-Path -Parent $PSScriptRoot
$releaseBundle = Join-Path $releaseWorkspace "artifacts\$BundleName"
$releaseExe = Join-Path $releaseBundle 'Frame.exe'
if (-not (Test-Path -LiteralPath $releaseExe -PathType Leaf)) { throw 'Package the Windows bundle first.' }
# Use the executable version: --build-name overrides may differ from pubspec.
$releaseVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($releaseExe).ProductVersion
if ($releaseVersion -notmatch '^([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?)(?:\+[0-9A-Za-z.-]+)?$') {
    throw "Unsupported executable version: $releaseVersion"
}
$releaseTag = $Matches[1]
$releaseManifest = Get-Content -LiteralPath (Join-Path $releaseBundle 'manifest.json') -Raw | ConvertFrom-Json
foreach ($releaseFile in $releaseManifest) {
    $releaseResolved = [System.IO.Path]::GetFullPath((Join-Path $releaseBundle $releaseFile.path))
    $releaseBoundary = [System.IO.Path]::GetFullPath($releaseBundle) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $releaseResolved.StartsWith($releaseBoundary, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'Manifest entry outside the bundle.'
    }
    if ((Get-FileHash -LiteralPath $releaseResolved -Algorithm SHA256).Hash.ToLower() -ne $releaseFile.sha256) {
        throw "Checksum mismatch: $($releaseFile.path)"
    }
}
$releaseOutput = Join-Path $releaseWorkspace 'artifacts\github-release'
New-Item -ItemType Directory -Path $releaseOutput -Force | Out-Null
$releaseArchive = Join-Path $releaseOutput "Frame-Windows-x64-v$releaseTag.zip"
if (Test-Path -LiteralPath $releaseArchive) { throw 'Archive already exists. Use a new version; do not replace published releases.' }
# The complete folder includes FFmpeg, Flutter assets and licensing notices.
Compress-Archive -LiteralPath $releaseBundle -DestinationPath $releaseArchive -CompressionLevel Optimal
$releaseDigest = (Get-FileHash -LiteralPath $releaseArchive -Algorithm SHA256).Hash.ToLower()
"$releaseDigest  $([System.IO.Path]::GetFileName($releaseArchive))" |
    Set-Content -LiteralPath "$releaseArchive.sha256" -Encoding ascii
Write-Output "GitHub release tag: v$releaseTag"
Write-Output "Upload asset: $releaseArchive"
Write-Output "Upload checksum: $releaseArchive.sha256"
Write-Output 'Prepared locally; nothing was pushed or published to GitHub.'
