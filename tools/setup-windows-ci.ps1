# Only for a fresh GitHub-hosted Windows runner; never installs system-wide tools.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') { throw 'This bootstrap is for GitHub-hosted Windows CI.' }
$ciRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'native-env.ps1')
New-Item -ItemType Directory -Path (Join-Path $ciRoot '.deps') -Force | Out-Null
function Get-VerifiedDependency([string]$Url, [string]$Destination, [string]$Sha256) {
    Invoke-WebRequest -Uri $Url -OutFile $Destination
    if ((Get-FileHash -LiteralPath $Destination).Hash.ToLowerInvariant() -ne $Sha256) { throw "Dependency hash mismatch: $Destination" }
}
$ciFfmpegArchive = Join-Path $ciRoot '.deps\ffmpeg-sdk.7z'
Get-VerifiedDependency 'https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.1.2-full_build-shared.7z' $ciFfmpegArchive 'cba748035c21ce1431d0823c7a3a711f38616f89f87a265dceddf9b7f6749d2d'
& 7z x $ciFfmpegArchive "-o$ciRoot\.deps\ffmpeg" -y
if ($LASTEXITCODE -ne 0) { throw 'FFmpeg extraction failed.' }
& python -m pip install --only-binary=:all: --no-deps --require-hashes -r (Join-Path $PSScriptRoot 'ci-libclang-requirements.txt') --target (Join-Path $ciRoot '.deps\clang')
if ($LASTEXITCODE -ne 0) { throw 'libclang install failed.' }
& rustup toolchain install 1.99.0 --profile minimal --component clippy,rustfmt
if ($LASTEXITCODE -ne 0) { throw 'Rust toolchain install failed.' }
& rustup default 1.99.0
if ($LASTEXITCODE -ne 0) { throw 'Rust toolchain selection failed.' }
$ciFlutter = Join-Path $ciRoot '.deps\flutter'
& git init $ciFlutter
& git -C $ciFlutter remote add origin https://github.com/flutter/flutter.git
& git -C $ciFlutter fetch --depth 1 origin 2c9eb20739dfec95e2c74bd3dfa4601b0a8a36aa
if ($LASTEXITCODE -ne 0) { throw 'Flutter source fetch failed.' }
& git -C $ciFlutter checkout --detach FETCH_HEAD
if ($LASTEXITCODE -ne 0) { throw 'Flutter checkout failed.' }
"$ciFlutter\bin" | Add-Content -LiteralPath $env:GITHUB_PATH
$ciCompiler = Join-Path $ciRoot '.deps\innosetup-6.7.3.exe'
Get-VerifiedDependency 'https://github.com/jrsoftware/issrc/releases/download/is-6_7_3/innosetup-6.7.3.exe' $ciCompiler '9c73c3bae7ed48d44112a0f48e66742c00090bdb5bef71d9d3c056c66e97b732'
$ciSignature = Get-AuthenticodeSignature -LiteralPath $ciCompiler
if ($ciSignature.Status -ne 'Valid' -or $ciSignature.SignerCertificate.Subject -notlike '*Pyrsys*') { throw 'Untrusted Inno compiler signature.' }
$ciCompilerProcess = Start-Process -FilePath $ciCompiler -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/CURRENTUSER','/NOICONS',"/DIR=`"$ciRoot\.deps\inno-setup`"" -WindowStyle Hidden -PassThru
if (-not $ciCompilerProcess.WaitForExit(45000) -or $ciCompilerProcess.ExitCode -ne 0) { throw 'Inno compiler installation failed.' }
