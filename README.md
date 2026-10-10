# Frame — native screen recorder

Frame 0.2 is a working **Windows alpha** with a Rust/FFmpeg engine and a native
Flutter desktop interface. The original four-platform professional specification
is still in progress; see [release status](docs/release-status.md) for exact gaps.

## Run the local Windows build

Open the latest UI build at `artifacts/Frame-Windows-Themes/Frame.exe`. Keep the entire folder together: Flutter
assets, recorder_core.dll and the matching FFmpeg runtime DLLs are required.
Choose a display, window or display region, choose a new output file, then record.
The main workspace shows a live thumbnail preview. Open **Settings** for video,
audio and output-format options. Recording controls stay visible at the bottom.
Settings → Appearance lets you select Dark or Light. Your choice is saved for
the next launch, and can be changed during a recording.
Pause/resume and stop/save are available while recording. Closing the app during
recording asks to stop and finalize first.

## Current features

- Full display, window and numeric custom-region capture through native WGC.
- MP4/MKV/AVI H.264 recording and animated GIF at up to 720p/15 fps.
- Native system-audio and microphone capture saved as separate AAC tracks.
- Automatic NVENC → AMF → Quick Sync → software encoder negotiation.
- D3D11 GPU conversion for NVENC/AMF; CPU conversion for Quick Sync/software.
- Separate settings panel for resolution, frame rate, bitrate, audio and format.
- Lightweight native live preview (up to 480 × 270, one update per second), with
  an off toggle and automatic suspension while the app is minimized.
- Native file dialogs, output reveal, status/timer and safe recording shutdown.
- Atomic refusal to overwrite existing recordings.

Selecting an encoder is not a guarantee of device support. This machine recorded
with Quick Sync and software encoding; its driver rejected the NVENC/AMF GPU
texture path. Automatic fallback reasons are retained in native session status.
GIF uses a basic fixed palette and contains no audio. Multi-track players may play
only one audio track by default; a combined mix track is not implemented yet.

## Build and test

See [build instructions](docs/build.md). On the prepared local workspace:

```powershell
. ./tools/native-env.ps1
./tools/verify.ps1
./tools/package-windows.ps1
```

Packaging defaults to `artifacts/Frame-Windows`. To recreate the preview build
alongside an open app, use `./tools/package-windows.ps1 -BundleName Frame-Windows-Themes`.

The original [architecture plan](docs/architecture.md) explains the target
monorepo, media ownership and future platform integrations. The current code adds
`capture.rs`, `wasapi.rs`, `audio.rs`, native worker/control modules, a Dart FFI
package, and a generated Flutter Windows host. The macOS runner is generated but
has no capture backend; Android/iOS are not implemented.

## Remaining work

Webcam/chroma-key and annotations, adaptive quality, mix bus and long-duration
sync, full GPU Quick Sync interop, macOS/Android/iOS backends, full-rate GPU preview,
recovery/export, signed installers and sustained low-end/4K120 validation remain.
This alpha is not an OBS/Bandicam-equivalent production release.

## License

Original project sources are MIT. The local SDK includes GPL FFmpeg/libx264
components, so redistribution of the linked application needs GPL-compatible
licensing, notices and corresponding dependency source/build information.
See [FFmpeg licensing](https://ffmpeg.org/legal.html) and the release status.
