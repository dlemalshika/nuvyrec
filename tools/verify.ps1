# Run from any directory after installing the documented build prerequisites.
# Writes unique smoke outputs inside artifacts; never replaces recordings.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
foreach ($toolName in @('cargo', 'ffprobe', 'ffmpeg')) {
    if (-not (Get-Command $toolName -ErrorAction SilentlyContinue)) {
        throw "Missing $toolName. See docs/build.md."
    }
}
Push-Location $taskRoot
try {
    & cargo test --workspace
    if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
    & cargo clippy --workspace --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Clippy failed.' }
    $artifactRoot = Join-Path $taskRoot 'artifacts'
    New-Item -ItemType Directory -Path $artifactRoot -Force | Out-Null
    $runId = [guid]::NewGuid().ToString('N')
    foreach ($extension in @('mp4', 'mkv', 'avi')) {
        $outputPath = Join-Path $artifactRoot "synthetic-$runId.$extension"
        & cargo run -p recorder-engine --example synthetic -- $outputPath
        if ($LASTEXITCODE -ne 0) { throw "Synthetic encode failed: $extension" }
        $probeText = & ffprobe -v error -count_frames -show_entries stream=codec_name,width,height,nb_read_frames -of json $outputPath
        if ($LASTEXITCODE -ne 0) { throw "Probe failed: $extension" }
        $probe = ($probeText -join "`n") | ConvertFrom-Json
        $stream = $probe.streams[0]
        if ($probe.streams.Count -ne 1 -or $stream.codec_name -ne 'mpeg4' -or
            $stream.width -ne 640 -or $stream.height -ne 360 -or
            [int]$stream.nb_read_frames -ne 90) {
            throw "Unexpected stream properties or frame loss: $extension"
        }
        & ffmpeg -v error -xerror -i $outputPath -f null -
        if ($LASTEXITCODE -ne 0) { throw "Decode failed: $extension" }
        Write-Host "Verified $extension`: 640x360, 90 decodable frames."
    }
} finally {
    Pop-Location
}
