# Verify returned SignPath artifacts before changing a bundle or release output.
[CmdletBinding()]
param(
    [ValidateSet('Payload','Installer')][string]$Stage = 'Payload',
    [ValidatePattern('^Nuvyrec-Windows(?:-[A-Za-z0-9]+)*$')][string]$BundleName = 'Nuvyrec-Windows-CI',
    [Parameter(Mandatory)][string]$SignedDirectory,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{40}$')][string]$CertificateThumbprint
)
$ErrorActionPreference = 'Stop'
$signedWorkspace = Split-Path -Parent $PSScriptRoot
$signedBoundary = [IO.Path]::GetFullPath($signedWorkspace) + [IO.Path]::DirectorySeparatorChar
$signedInputPath = if ([IO.Path]::IsPathRooted($SignedDirectory)) { $SignedDirectory } else { Join-Path $signedWorkspace $SignedDirectory }
$signedInput = [IO.Path]::GetFullPath($signedInputPath)
if (-not $signedInput.StartsWith($signedBoundary, [StringComparison]::OrdinalIgnoreCase)) { throw 'Signed input must be inside this workspace.' }
$signedBundle = Join-Path $signedWorkspace "artifacts\$BundleName"
$signedApp = Join-Path $signedBundle 'Nuvyrec.exe'
$signedVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($signedApp).ProductVersion
if ($signedVersion -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+\d+)?$') { throw 'Invalid bundled app version.' }
$signedTag = $signedVersion.Split('+')[0]
$signedInstallerName = "Nuvyrec-Windows-x64-v$signedTag-Setup.exe"
$signedExpected = if ($Stage -eq 'Payload') { @('Nuvyrec.exe','recorder_core.dll') } else { @($signedInstallerName) }
$signedFiles = @(Get-ChildItem -LiteralPath $signedInput -File -Recurse)
if ($signedFiles.Count -ne $signedExpected.Count) { throw 'Unexpected number of signed artifacts.' }
foreach ($signedName in $signedExpected) {
    $signedPath = Join-Path $signedInput $signedName
    if (-not (Test-Path -LiteralPath $signedPath -PathType Leaf)) { throw "Missing signed file: $signedName" }
    $signedIdentity = Get-AuthenticodeSignature -LiteralPath $signedPath
    if ($signedIdentity.Status -ne 'Valid' -or -not $signedIdentity.TimeStamperCertificate -or
        $signedIdentity.SignerCertificate.Thumbprint -ne $CertificateThumbprint) { throw "Untrusted, unsigned or untimestamped artifact: $signedName" }
    $signedMetadata = [Diagnostics.FileVersionInfo]::GetVersionInfo($signedPath)
    if ($signedMetadata.ProductName -ne 'Nuvyrec' -or $signedMetadata.ProductVersion -ne $signedVersion) { throw "Signed metadata mismatch: $signedName" }
}
if ($Stage -eq 'Payload') {
    # Verify the untouched original bundle before replacing ONLY our own files.
    $signedManifestPath = Join-Path $signedBundle 'manifest.json'
    $signedManifest = @(Get-Content -LiteralPath $signedManifestPath -Raw | ConvertFrom-Json)
    foreach ($signedEntry in $signedManifest) {
        $signedResolved = [IO.Path]::GetFullPath((Join-Path $signedBundle $signedEntry.path))
        if (-not $signedResolved.StartsWith([IO.Path]::GetFullPath($signedBundle) + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Manifest escape.' }
        if ((Get-FileHash -LiteralPath $signedResolved).Hash.ToLowerInvariant() -ne $signedEntry.sha256) { throw "Original bundle mismatch: $($signedEntry.path)" }
    }
    foreach ($signedName in $signedExpected) { Copy-Item -LiteralPath (Join-Path $signedInput $signedName) -Destination (Join-Path $signedBundle $signedName) -Force }
    foreach ($signedEntry in $signedManifest) {
        if ($signedExpected -contains $signedEntry.path) { $signedEntry.sha256 = (Get-FileHash -LiteralPath (Join-Path $signedBundle $signedEntry.path)).Hash.ToLowerInvariant() }
    }
    $signedManifest | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath $signedManifestPath -Encoding utf8
} else {
    $signedOutput = Join-Path $signedWorkspace 'artifacts\signed-release'
    New-Item -ItemType Directory -Path $signedOutput -Force | Out-Null
    $signedDestination = Join-Path $signedOutput $signedInstallerName
    if (Test-Path -LiteralPath $signedDestination) { throw 'Signed release already exists.' }
    Copy-Item -LiteralPath (Join-Path $signedInput $signedInstallerName) -Destination $signedDestination
    $signedHash = (Get-FileHash -LiteralPath $signedDestination).Hash.ToLowerInvariant()
    "$signedHash  $signedInstallerName" | Set-Content -LiteralPath "$signedDestination.sha256" -Encoding ascii
}
Write-Output "Verified $Stage signatures, timestamps, publisher thumbprint and product metadata."
