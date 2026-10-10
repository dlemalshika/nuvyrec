# Frame — Windows capture alpha

This is a runnable Windows screen recorder, not the completed four-platform
OBS/Bandicam competitor requested in the original specification.

## Implemented

- Flutter native Windows executable with a Rust shared-library engine.
- Windows.Graphics.Capture display/window sources and custom display regions.
- H.264 recording to MKV, MP4 and AVI, plus low-resolution animated GIF.
- Start, pause, resume, explicit stop/finalization and guarded app shutdown.
- Native save dialog, output-file reveal, resolution/frame-rate/bitrate controls.
- Separate settings panel and a main preview workspace with fixed recording controls.
- Dark/Light theme selector with a saved per-user Windows preference.
- Windows WGC window thumbnails and GDI/GDI+ display/region thumbnails,
  limited to 480 × 270 at one update per second;
  updates stop when disabled or the app is not active. Thumbnails stay in memory.
- Full-window bounds including title bars, physical DPI coordinates, idle source
  refresh every three seconds and explicit minimized-window restore status.
- Smooth fixed-size region Follow mouse with in-place GPU crop updates;
  pointer stays on the selected display and cursor recording is included.
- NVENC/AMF GPU-surface paths, Quick Sync with CPU conversion, and libx264 fallback.
- WASAPI default-system and default-microphone capture, saved as separate AAC
  tracks, converted to 48 kHz stereo through libswresample.
- CPU fallback that does not require allocating NV12 GPU encoder textures.
- Atomic refusal to overwrite existing files; bounded frame/audio processing.
- Native global start/stop, pause/resume and replay hotkeys with conflict reporting;
  tray menu and hide/restore controls.
- Mouse-drag region selection and cancellable 3/5/10-second countdown.
- Selected WASAPI endpoints, live gain/mute, peak meters, microphone FFT noise
  suppression/gate and limiter. Optional combined playback plus two source tracks.
- Saved quality presets; future-time recording while the app remains open;
  duration/file-size auto-stop.
- Recording library, explicit import, default-player playback, folder reveal,
  full-resolution PNG screenshots, local MP4 remux, precise trim and crop exports.
- Encoded-packet replay history (5–120 seconds via ABI; UI 15/30/60/120),
  4–128 MiB payload budget and 32,768-packet cap. Metadata adds a small bounded
  overhead. Replay saves independent keyframe-aligned MKV files.
- Draw/arrow/box/spotlight annotations placed on the preview and composited into
  recorded YUV420P frames. Maximum 64 objects, 256 points per object, one spotlight.
  Cursor highlight/click rings support display/region recording. These optional
  effects explicitly use software encoding and increase CPU work.
- Optional MKV recovery segments: completed files are finalized/synced individually;
  up to 1024 parts. The active file remains vulnerable to abrupt failure. Replay
  history restarts at every segment. Segments may have small boundary timing gaps.
- File-size/skipped-frame-slot telemetry and free disk space at file selection.
  Skipped slots are clock gaps, not a count reported by the capture device.

- Automatic PC budgets, Low-resource and Custom modes; GPU capture canvas scaling
  before readback; effective-quality and CPU/RAM telemetry.
- Adaptive MKV quality reduction under sustained overload, with finalized parts,
  cooldown and a manual Reduce recording load action. See [performance](performance.md).

- Windows DirectShow webcam worker and YUV420P compositor with mirror, corner/size
  placement and soft green key; latest-frame buffering and camera-error telemetry.
- Four cached Unicode title/logo layers, position/size/opacity/order controls,
  and twenty saved creator layouts. Local image files are reloaded on restore.
- Opt-in shortcut display with bounded expiry, global modifier combinations and
  function keys; ordinary typed text and clipboard contents are excluded.
- Non-blocking native WGC/D3D11 sampling with one retained GPU surface. Static
  windows continue recording without delaying worker stop/audio/control polling.
  Webcam is opt-in and uses software encoding. See [webcam limitations](webcam.md).

## Verified on this machine

- Rust compilation, Clippy and configuration/lifecycle regression tests.
- 26 Rust tests and fourteen Flutter tests passed for the Creator revision;
  Clippy with warnings denied and Flutter analysis passed.
- Owned static Flutter window recorded 73–74 frames at 640 × 360 / 24 fps and
  automatically finalized. Encoded Sinhala glyph pixels matched the uploaded
  native Flutter raster; transparent logo/background and full decode verified.
- Final packaged creator fixture exited successfully in 4.2 seconds; ordinary
  startup smoke test also exited with code zero.
- Creator settings/layout persistence and recording locks tested at 960 × 640.
- Packaged Low resource Follow mouse region recording with Quick Sync,
  physical viewport telemetry and pause/resume finalized successfully; decoded
  video was verified at 640 × 360 / 24 fps.
- Owned synthetic window: full title bar and bottom-corner video/preview pixels,
  minimized-source rejection without creating an output, and left/right GPU
  panning on the same graph verified. See [capture validation](capture.md).
- Independent audio-track encoding with 44.1 kHz and 48 kHz input devices.
- Actual desktop capture, GIF region capture, and MP4 region capture with pause.
- Actual system-audio and two-track system/microphone recording.
- Intel Quick Sync initialization/recording; software encoder fallback.
- Recorded video/audio probing and decoding.
- Native Flutter Windows release compilation and recording-control UI tests.
- Packaged-release AVI region capture, pause/resume, and two decoded audio tracks.
- Packaged executable rendered its first Flutter frame and exited cleanly using
  `Frame.exe --smoke-test`.
- Native PNG thumbnail capture and Flutter startup using `--preview-smoke-test`.
- Separate-settings interactions, selected FPS in start configuration, region
  preview coordinates, preview-off suspension and layout at 960 × 640.
- Light/Dark switching in workspace and settings, saved-theme restoration through
  the mocked native preference interface, and switching during active recording.
- Synthetic audio DSP/resampling, annotation bounds/frame ownership and replay
  keyframe/audio/timestamp tests. Replay saves do not overwrite existing files.
- Real six-second packaged display-region recording with annotations, audio
  DSP, three AAC tracks, replay export and automatic duration stop.
- Synthetic-media remux, precise trim/crop, track preservation, output refusal
  and full exported-media decoding through the packaged FFmpeg version.
- Actual five-second MKV capture split into three independently decoded parts;
  each part and the replay clip retained all three AAC tracks.
- Cancellable countdown and normalized live-arrow/clear interaction tests.
- Slow-clock audio catch-up and fractional segment-origin timestamp regressions.
- Actual stop-during-pause capture/replay decoded with paused time excluded.

- Quality policy regressions and actual 1080p to 720p recording transition;
  both parts and replay fully decoded with three AAC tracks.
- Low-resource 24 fps region recording using Quick Sync and system audio.

- Native webcam discovery found two devices; actual ten-second webcam/display
  recording with drawing, green key, three AAC tracks, replay and 1080p to 720p
  transition produced fully decoded MKV parts and replay.
- Synthetic camera compositor video fully decoded; webcam settings/persistence
  and active-recording locks verified at 960 x 640.

- Packaged low-resource webcam recording and replay decoded with three AAC tracks;
  missing-camera error did not prevent screen recording finalization.

NVENC/AMF paths could not allocate NV12 textures on this machine; the software
fallback worked and Quick Sync subsequently recorded using CPU-side conversion.
This does not validate the NVENC/AMF hardware paths on other GPUs. Frame loss was
observed in short smoke tests; sustained real-time throughput is not guaranteed.

## Required work before the original specification is complete

- Tested macOS ScreenCaptureKit/Metal implementation and signed packaging.
- Android MediaProjection service and iOS ReplayKit broadcast-extension apps.
- Live Unicode text editing during recording. Current drawing takes place inside
  Frame's preview; title/logo layers are prepared before starting.
- Accurate discontinuity/drift correction and long-duration synchronization,
  fault-injection and power-loss recovery tests.
- Zero-CPU-readback Quick Sync interop and validation on actual NVENC/AMF devices.
- Full-rate GPU preview, permission recovery, scene/source-layer composition,
  RTMP/SRT streaming with reconnect, automatic captions/smart zoom, modern
  codecs/HDR, installers and reproducible release CI.
- Verified low-end hardware limits and sustained 4K/120 performance measurements.

The generated macOS runner and mobile reserved directory are not functional
recording products. No cross-platform or production-readiness claim is made.

The lightweight preview is separate from the recording pipeline. Window
thumbnails use WGC, supporting GPU-rendered windows. Protected windows can still
refuse capture; minimized windows must be restored. Full-resolution window
screenshots capture the visible desktop bounds and can include overlapping apps.
Preview does not display native recorded cursor effects or verify the final
encoded output; annotation guides are rendered over the preview separately.

## Distribution

This local build uses the checksum-verified Gyan FFmpeg 8.1.2 full shared SDK.
That SDK includes GPL components (including libx264). Original project sources
are MIT, but redistributing the linked application requires GPL-compatible
distribution, dependency notices and corresponding source/build information.
This local test bundle is not a signed, audited or published release.

Source SDK: https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.1.2-full_build-shared.7z

SHA-256: cba748035c21ce1431d0823c7a3a711f38616f89f87a265dceddf9b7f6749d2d

Rust tested: 1.99.0 MSVC. Flutter tested: 3.41.5 / Dart 3.11.3.
