[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [ValidatePattern('^Frame-Windows(?:-[A-Za-z0-9]+)*$')]
    [string]$BundleName = 'Frame-Windows',
    [ValidatePattern('^target(?:[\\/][A-Za-z0-9_-]+)*$')]
    [string]$NativeTargetDir = 'target'
)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'native-env.ps1')
Push-Location $taskRoot
try {
    if (-not $SkipBuild) {
        & cargo build -p recorder-ffi --release --locked --target-dir $NativeTargetDir
        if ($LASTEXITCODE -ne 0) { throw 'Native build failed' }
        Push-Location (Join-Path $taskRoot 'apps\desktop')
        try {
            & flutter build windows --release
            if ($LASTEXITCODE -ne 0) { throw 'Flutter build failed' }
        } finally { Pop-Location }
    }
    $bundleRoot = Join-Path (Join-Path $taskRoot 'artifacts') $BundleName
    $existingExecutable = Join-Path $bundleRoot 'Frame.exe'
    if (Test-Path -LiteralPath $existingExecutable) {
        try {
            $check = [System.IO.File]::Open($existingExecutable, [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
            $check.Dispose()
        } catch {
            throw 'The bundle is in use. Close it or supply a new -BundleName before packaging.'
        }
    }
    New-Item -ItemType Directory -Path $bundleRoot -Force | Out-Null
    Copy-Item -Path (Join-Path $taskRoot 'apps\desktop\build\windows\x64\runner\Release\*') -Destination $bundleRoot -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $taskRoot "$NativeTargetDir\release\recorder_core.dll") -Destination $bundleRoot -Force
    Copy-Item -Path "$env:FFMPEG_DIR\bin\*.dll" -Destination $bundleRoot -Force
    Copy-Item -LiteralPath "$env:FFMPEG_DIR\bin\ffmpeg.exe","$env:FFMPEG_DIR\bin\ffprobe.exe" -Destination $bundleRoot -Force
    $runtimeRoot = 'C:\Program Files\Microsoft Visual Studio\18\Community\VC\Redist\MSVC\14.51.36231\x64\Microsoft.VC145.CRT'
    if (Test-Path -LiteralPath $runtimeRoot) {
        Copy-Item -Path (Join-Path $runtimeRoot '*.dll') -Destination $bundleRoot -Force
    }
    Copy-Item -LiteralPath (Join-Path $taskRoot 'LICENSE'),(Join-Path $taskRoot 'docs\release-status.md') -Destination $bundleRoot -Force
    $noticeRoot = Join-Path $bundleRoot 'third-party'
    New-Item -ItemType Directory -Path $noticeRoot -Force | Out-Null
    Copy-Item -Path "$env:FFMPEG_DIR\LICENSE*","$env:FFMPEG_DIR\README*" -Destination $noticeRoot -Force
    Copy-Item -LiteralPath (Join-Path $taskRoot 'README.md') -Destination $bundleRoot -Force
    $documentationRoot = Join-Path $bundleRoot 'docs'
    New-Item -ItemType Directory -Path $documentationRoot -Force | Out-Null
    Copy-Item -Path (Join-Path $taskRoot 'docs\*.md') -Destination $documentationRoot -Force
    $manifest = Get-ChildItem -LiteralPath $bundleRoot -File -Recurse | Where-Object { $_.FullName -ne (Join-Path $bundleRoot 'manifest.json') } | ForEach-Object {
        [pscustomobject]@{ path = [System.IO.Path]::GetRelativePath($bundleRoot, $_.FullName); sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLower() }
    }
    $manifest | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $bundleRoot 'manifest.json') -Encoding utf8
    Write-Host "Runnable bundle: $bundleRoot\Frame.exe"
} finally { Pop-Location }
