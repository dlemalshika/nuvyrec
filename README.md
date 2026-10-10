# Frame — native screen recorder

Frame is a working **Windows performance alpha** with a Rust/FFmpeg engine and a native
Flutter desktop interface. The original four-platform professional specification
is still in progress; see [release status](docs/release-status.md) for exact gaps.

## Run the local Windows build

Open the latest build at `artifacts/Frame-Windows-Performance/Frame.exe`. Keep the entire folder together: Flutter
assets, recorder_core.dll and the matching FFmpeg runtime DLLs are required.
Choose a display, window or display region, choose a new output file, then record.
The main workspace shows a live thumbnail preview. Open **Settings** for video,
audio and output-format options. Recording controls stay visible at the bottom.
Settings → Appearance lets you select Dark or Light. Your choice is saved for
the next launch, and can be changed during a recording.
Pause/resume and stop/save are available while recording. Closing the app during
recording asks to stop and finalize first.

See [performance modes and validation](docs/performance.md) for Automatic, Low resource,
Custom and adaptive MKV recording. Start with Automatic on an unfamiliar PC.

## Current features

- Full display/window capture, numeric regions and native drag-to-select regions.
- MP4/MKV/AVI H.264 recording and animated GIF at up to 720p/15 fps.
- Native system-audio and microphone capture saved as separate AAC tracks.
- Automatic NVENC → AMF → Quick Sync → software encoder negotiation.
- D3D11 GPU conversion for NVENC/AMF; CPU conversion for Quick Sync/software.
- Separate settings panel for resolution, frame rate, bitrate, audio and format.
- Lightweight native live preview (up to 480 × 270, one update per second), with
  an off toggle and automatic suspension while the app is minimized.
- Native file dialogs, output reveal, status/timer and safe recording shutdown.
- Atomic refusal to overwrite existing recordings.
- Global hotkeys/tray controls, countdown, saved quality presets and scheduled start.
- Device selection, live audio meters/gain/mute, optional mixed playback track,
  microphone noise suppression/gate and limiter.
- Duration/file-size auto-stop, optional finalized MKV recovery segments.
- Encoded replay buffer with an explicit memory budget and MKV replay export.
- Draw/arrow/box/spotlight annotations; display/region cursor highlight/click effects.
  These effects use software encoding and are off by default.
- Local recording library/import/playback, PNG screenshots, MP4 remux, trim/crop
  exports with progress/cancellation and protected app shutdown during export.

Selecting an encoder is not a guarantee of device support. This machine recorded
with Quick Sync and software encoding; its driver rejected the NVENC/AMF GPU
texture path. Automatic fallback reasons are retained in native session status.
GIF uses a basic fixed palette and contains no audio. Multi-track players may play
only one audio track by default. With both sources enabled, the optional combined
track is written first and separate source tracks follow.

## Build and test

See [build instructions](docs/build.md). On the prepared local workspace:

```powershell
. ./tools/native-env.ps1
./tools/verify.ps1
./tools/package-windows.ps1
```

Packaging defaults to `artifacts/Frame-Windows`. To recreate the feature build
alongside an open app, use `./tools/package-windows.ps1 -BundleName Frame-Windows-Performance`.

The original [architecture plan](docs/architecture.md) explains the target
monorepo, media ownership and future platform integrations. The current code adds
`capture.rs`, `wasapi.rs`, `audio.rs`, native worker/control modules, a Dart FFI
package, and a generated Flutter Windows host. The macOS runner is generated but
has no capture backend; Android/iOS are not implemented.

## Remaining work

Webcam/chroma key, Unicode text/keystroke overlays, scene/source layers, streaming,
automatic captions/smart zoom, HEVC/AV1/HDR, long-duration sync,
full GPU Quick Sync interop, macOS/Android/iOS backends, full-rate GPU preview,
fault-injection recovery testing, signed installers and sustained low-end/4K120
validation remain. See [feature status](docs/professional-features.md).
This alpha is not an OBS/Bandicam-equivalent production release.

## License

Original project sources are MIT. The local SDK includes GPL FFmpeg/libx264
components, so redistribution of the linked application needs GPL-compatible
licensing, notices and corresponding dependency source/build information.
See [FFmpeg licensing](https://ffmpeg.org/legal.html) and the release status.
