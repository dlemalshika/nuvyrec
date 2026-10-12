# Nuvyrec — native screen recorder

![Nuvyrec logo](apps/desktop/assets/branding/nuvyrec-logo.png)

Project repository: [dlemalshika/nuvyrec](https://github.com/dlemalshika/nuvyrec).
See the [Code signing policy](docs/code-signing-policy.md) and [Privacy policy](docs/privacy.md).
SignPath integration is being prepared; existing installers remain unsigned.
See [branding and release setup](docs/branding.md) for the Nuvyrec 1.0.0 first release.

Nuvyrec is a working **Windows capture alpha** with a Rust/FFmpeg engine and a native
Flutter desktop interface. The original four-platform professional specification
is still in progress; see [release status](docs/release-status.md) for exact gaps.

## Run the local Windows build

Open the latest build at `artifacts/Nuvyrec-Windows-Release/Nuvyrec.exe`. Keep the entire folder together: Flutter
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

See [webcam setup](docs/webcam.md) for camera overlays and green-screen removal.
See [window capture and Follow mouse](docs/capture.md) for full-window previews and moving region capture.
See [creator overlays](docs/creator-overlays.md) for Sinhala/Unicode titles, logos,
saved layouts and the optional keyboard-shortcut display.
See [local exports](docs/exports.md) for landscape, vertical, square and small-file
MP4 presets, audio-track selection and trim/crop combinations.
See [GPU composition](docs/gpu-composition.md) for accelerated overlays, encoder
selection, memory bounds and the remaining CPU readback paths.
See [live streaming](docs/live-streaming.md) for YouTube/Facebook/TikTok setup,
shared encoding, upload requirements and account-specific limits.
See [GitHub updates](docs/updates.md) for daily checks, Stable/Beta channels,
update notifications and preparing a versioned release ZIP.
See [Windows installation](docs/windows-installer.md) for Setup.exe packaging,
Start Menu/Desktop shortcuts, upgrades and uninstall.
See [live scenes](docs/scenes.md) for saved visual layouts, atomic live editing,
webcam placement and Cut / Fade-through-black transitions on one capture source.

## Current features

- Full display/window capture, numeric regions and native drag-to-select regions.
- Full-window WGC thumbnails, automatic source refresh, minimized-window status
  and smooth fixed-size region capture following the mouse on one display.
- MP4/MKV/AVI H.264 recording and animated GIF at up to 720p/15 fps.
- Native system-audio and microphone capture saved as separate AAC tracks.
- Automatic NVENC → AMF → Quick Sync → software encoder negotiation.
- D3D11 GPU conversion for NVENC/AMF; CPU conversion for Quick Sync/software.
- Separate settings panel for resolution, frame rate, bitrate, audio and format.
- Lightweight native live preview (up to 480 × 270, one update per second), with
  an off toggle and automatic suspension while the app is minimized.
- Native file dialogs, output reveal, status/timer and safe recording shutdown.
- Atomic refusal to overwrite existing recordings.
- Native RTMP/RTMPS fan-out to up to three platforms with shared H.264/AAC encoding,
  isolated bounded queues, reconnect, masked in-memory keys and concurrent local recording.
- Global hotkeys/tray controls, countdown, saved quality presets and scheduled start.
- Device selection, live audio meters/gain/mute, optional mixed playback track,
  microphone noise suppression/gate and limiter.
- Duration/file-size auto-stop, optional finalized MKV recovery segments.
- Encoded replay buffer with an explicit memory budget and MKV replay export.
- Local share/compression presets, aspect-ratio padding, 30 fps caps and export
  audio selection; source files are preserved and video threading is bounded.
- Webcam overlay with camera selection, four-corner placement, size, mirror
  and adjustable green-background removal (GPU composition / CPU fallback). Live webcam preview
  updates at 5 fps, or 2 fps in Low resource; the main-screen On/Off button can
  hide/show the overlay in a recording started with webcam enabled.
- Draw/arrow/box/spotlight annotations; display/region cursor highlight/click effects.
  These effects use GPU composition when available, with CPU fallback, and are off by default.
- Up to four Unicode text/logo layers, preview placement, opacity/order controls,
  twenty saved creator layouts and an opt-in shortcut display. Assets are cached
  before recording; these effects preserve the selected hardware encoder.
- Non-blocking native WGC sampling: static windows keep recording and remain
  stoppable; scaling and region panning happen on the GPU before CPU readback.
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

Packaging defaults to `artifacts/Nuvyrec-Windows`. To recreate the feature build
alongside an open app, use `./tools/package-windows.ps1 -BundleName Nuvyrec-Windows-Multistream`.

The original [architecture plan](docs/architecture.md) explains the target
monorepo, media ownership and future platform integrations. The current code adds
`capture.rs`, `wasapi.rs`, `audio.rs`, native worker/control modules, a Dart FFI
package, and a generated Flutter Windows host. The macOS runner is generated but
has no capture backend; Android/iOS are not implemented.

## Remaining work

Multiple simultaneous capture sources, source-changing transitions, SRT/platform integrations,
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
