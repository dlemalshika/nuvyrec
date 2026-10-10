# Build and verification

## Tested local tools

- Rust 1.99.0, x86_64-pc-windows-msvc, Cargo/rustfmt/Clippy.
- Visual Studio 2026 C++ build tools, C++20 and Windows SDK 10.0.26100.0
  including its C++/WinRT headers for the native WGC adapter.
- Flutter 3.41.5, Dart 3.11.3, native Windows runner.
- FFmpeg 8.1.2 full shared SDK; ffmpeg-sys-next pinned to 8.1.0.
- libclang 18.1.1 installed inside .deps/clang for bindgen.

FFmpeg headers, import libraries and runtime DLLs must match. A CLI-only FFmpeg
installation is insufficient. The engine links avcodec, avformat, avutil, avfilter
avdevice, swscale and swresample; recording does not spawn the ffmpeg executable. CLI tools are
used only for independent probe/decode verification.
The engine's Windows-only Cargo build script compiles a small C++/WinRT/D3D11
source adapter and links windowsapp, runtimeobject, d3d11, dxgi, d3dcompiler,
dwmapi and user32. Rust owns its lifetime on the media thread; the GPU frames
feed an ordinary FFmpeg buffer source. `FFMPEG_DIR` supplies the matching headers.

## Build this prepared workspace

```powershell
. ./tools/native-env.ps1
cargo build --workspace --release --locked
./tools/verify.ps1
./tools/package-windows.ps1
```

The environment helper uses project-local Rust, libclang and FFmpeg paths and
disables incremental compilation to avoid restricted-filesystem hard-link issues.
Run Flutter commands with access to its SDK cache and native build tools. The
packaging script builds the Rust library and Flutter executable, then creates
`artifacts/Frame-Windows` including all required runtime DLLs and assets.

On another machine install Rust stable, Flutter and Visual Studio's desktop C++
workload. Prepare the SDK layout below or adjust tools/native-env.ps1 to your
installed paths. The current source requires a modern compiler supporting C string
literals, is_multiple_of and is_none_or; the validated compiler is 1.99.0.
Cargo.lock and the Flutter pubspec.lock are checked into the source directory.

```text
.deps/
  cargo/bin/cargo.exe
  rustup/toolchains/...
  clang/clang/native/libclang.dll
  ffmpeg/ffmpeg-8.1.2-full_build-shared/
    include/libavcodec/avcodec.h
    include/libavformat/avformat.h
    include/libavfilter/avfilter.h
    lib/avcodec.lib
    lib/avformat.lib
    lib/avfilter.lib
    lib/swresample.lib
    bin/avcodec-62.dll
    bin/avformat-62.dll
    bin/avfilter-11.dll
    bin/avutil-60.dll
    bin/swresample-6.dll
```

SDK URL and SHA-256 are recorded in release-status.md. The directory is ignored;
installers and dependency binaries are not project source.

## Checks

`tools/verify.ps1` runs Rust tests and Clippy, writes unique synthetic recordings,
then probes and fully decodes all 90 frames in MP4, MKV and AVI. Lifecycle tests
cover timestamp rejection, file preservation and independent audio sample rates.

```powershell
cd apps/desktop
flutter analyze
flutter test --timeout 1m
```

The UI interaction test covers the separate settings panel, frame-rate changes,
preview toggling, region coordinates, start/pause/resume/stop and the minimum
960 × 640 layout. Theme tests cover Light/Dark switching, restoration through a
mocked native preference interface and changes during active recording.
It renders `artifacts/desktop-ui.png`, `artifacts/desktop-ui-light.png` and
`artifacts/desktop-settings.png` using a generated preview fixture, not real
desktop content.

`Frame.exe --preview-smoke-test` checks a native desktop thumbnail is a PNG,
renders the first Flutter frame and exits. Run it from the packaged folder with
desktop access. The ordinary `--smoke-test` checks only the first-frame startup.

`python tools/capture_smoke.py` exercises the C ABI using actual Windows capture.
It writes a short desktop clip into artifacts. Options include --format gif,
--region, --pause and --audio system|microphone|both. Native capture/audio tests
require desktop/audio-device access and intentionally fail with an error when
permissions are denied. Artifacts may contain desktop/audio content and are
ignored by version control. They are not copied into the app bundle.

## Other platforms

The core abstractions can be compiled for other targets with their matching SDKs,
but the shipped capture worker is Windows-only. macOS needs ScreenCaptureKit/
VideoToolbox and signed dylib packaging. Android requires NDK/JNI plus a
MediaProjection service; iOS requires static libraries and a ReplayKit extension.
These are remaining implementation work, not available build targets today.

For an isolated native build directory (useful when generated import libraries
are locked), use cargo build -p recorder-ffi --release --locked --target-dir
target/webcam-final and package with -NativeTargetDir target/webcam-final.
