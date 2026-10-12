# Local integration smoke: installs only into a fresh workspace-owned folder.
# Does not record the screen or install into the user's real application folder.
[CmdletBinding()]
param(
    [ValidatePattern('^Nuvyrec-Windows(?:-[A-Za-z0-9]+)*$')]
    [string]$BundleName = 'Nuvyrec-Windows-Release'
)
$ErrorActionPreference = 'Stop'
$smokeRoot = Split-Path -Parent $PSScriptRoot
$smokeBundle = Join-Path $smokeRoot "artifacts\$BundleName"
$smokeVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $smokeBundle 'Nuvyrec.exe')).ProductVersion.Split('+')[0]
$smokeSetup = Join-Path $smokeRoot "artifacts\github-release\Nuvyrec-Windows-x64-v$smokeVersion-Setup.exe"
$smokeInstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{7CFD2025-7177-4616-AE47-46BD74DA390A}_is1'
if (Test-Path -LiteralPath $smokeInstallKey) { throw 'A real Nuvyrec install exists. Use a separate test Windows account.' }
if (-not (Test-Path -LiteralPath $smokeSetup)) { throw 'Build the installer first.' }
$smokeDirectory = Join-Path $smokeRoot ("artifacts\installer-smoke-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $smokeDirectory | Out-Null
$smokeDestination = Join-Path $smokeDirectory 'installed'
$smokeUninstaller = Join-Path $smokeDestination 'unins000.exe'

function Invoke-SmokeProcess([string]$Executable, [string[]]$Arguments, [int]$Expected = 0) {
    $smokeProcess = Start-Process -FilePath $Executable -ArgumentList $Arguments -WindowStyle Hidden -PassThru
    if (-not $smokeProcess.WaitForExit(45000)) {
        Stop-Process -Id $smokeProcess.Id -ErrorAction SilentlyContinue
        throw "Owned smoke process timed out: $Executable"
    }
    if ($smokeProcess.ExitCode -ne $Expected) { throw "Expected exit $Expected, received $($smokeProcess.ExitCode): $Executable" }
}

$smokeSetupArguments = @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/TASKS=',
    "/DIR=`"$smokeDestination`"", '/GROUP="Nuvyrec Installer Smoke"')
try {
    Invoke-SmokeProcess $smokeSetup ($smokeSetupArguments + "/LOG=`"$smokeDirectory\install.log`"")
    $smokeManifest = Get-Content -LiteralPath (Join-Path $smokeBundle 'manifest.json') -Raw | ConvertFrom-Json
    foreach ($smokeFile in $smokeManifest) {
        $smokeInstalledFile = Join-Path $smokeDestination $smokeFile.path
        if ((Get-FileHash -LiteralPath $smokeInstalledFile -Algorithm SHA256).Hash.ToLowerInvariant() -ne $smokeFile.sha256) {
            throw "Installed hash mismatch: $($smokeFile.path)"
        }
    }
    Write-Output "Install hashes: $(@($smokeManifest).Count) passed"
    Invoke-SmokeProcess (Join-Path $smokeDestination 'Nuvyrec.exe') @('--smoke-test')
    Write-Output 'Installed application startup: passed'
    # Same-version reinstall exercises the existing-install path without forging
    # a new app version. A real different-version upgrade still needs release QA.
    Invoke-SmokeProcess $smokeSetup ($smokeSetupArguments + "/LOG=`"$smokeDirectory\reinstall.log`"")
    Write-Output 'Existing-install reinstall: passed'
    $smokeFileLock = [System.IO.File]::Open((Join-Path $smokeDestination 'Nuvyrec.exe'),
        [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
    try {
        Invoke-SmokeProcess $smokeSetup ($smokeSetupArguments + "/LOG=`"$smokeDirectory\locked.log`"") 7
    } finally { $smokeFileLock.Dispose() }
    Write-Output 'In-use executable refusal: passed'
} finally {
    if (Test-Path -LiteralPath $smokeUninstaller) {
        Invoke-SmokeProcess $smokeUninstaller @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART', "/LOG=`"$smokeDirectory\uninstall.log`"")
    }
}
if (Test-Path -LiteralPath (Join-Path $smokeDestination 'Nuvyrec.exe')) { throw 'Uninstall left Nuvyrec.exe behind.' }
if (Test-Path -LiteralPath $smokeInstallKey) { throw 'Uninstall registration was not removed.' }
Write-Output 'Uninstall: passed'
Write-Output "Smoke logs: $smokeDirectory"
