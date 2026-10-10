# Project-local build tools; this script does not change the user's environment.
$taskRoot = Split-Path -Parent $PSScriptRoot
$env:CARGO_HOME = Join-Path $taskRoot '.deps\cargo'
$env:RUSTUP_HOME = Join-Path $taskRoot '.deps\rustup'
$env:FFMPEG_DIR = Join-Path $taskRoot '.deps\ffmpeg\ffmpeg-8.1.2-full_build-shared'
$env:LIBCLANG_PATH = Join-Path $taskRoot '.deps\clang\clang\native'
$env:PATH = "$env:CARGO_HOME\bin;$env:FFMPEG_DIR\bin;$env:PATH"
$env:CARGO_INCREMENTAL = '0'
