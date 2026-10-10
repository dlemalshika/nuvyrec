# Frame 0.2 — Windows alpha

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
- Windows GDI/GDI+ thumbnail preview, limited to 480 × 270 at one update per second;
  updates stop when disabled or the app is not active. Thumbnails stay in memory.
- NVENC/AMF GPU-surface paths, Quick Sync with CPU conversion, and libx264 fallback.
- WASAPI default-system and default-microphone capture, saved as separate AAC
  tracks, converted to 48 kHz stereo through libswresample.
- CPU fallback that does not require allocating NV12 GPU encoder textures.
- Atomic refusal to overwrite existing files; bounded frame/audio processing.

## Verified on this machine

- Rust compilation, Clippy and configuration/lifecycle regression tests.
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

NVENC/AMF paths could not allocate NV12 textures on this machine; the software
fallback worked and Quick Sync subsequently recorded using CPU-side conversion.
This does not validate the NVENC/AMF hardware paths on other GPUs. Frame loss was
observed in short smoke tests; sustained real-time throughput is not guaranteed.

## Required work before the original specification is complete

- Tested macOS ScreenCaptureKit/Metal implementation and signed packaging.
- Android MediaProjection service and iOS ReplayKit broadcast-extension apps.
- Webcam compositor, green-screen keying and live on-screen annotation.
- Audio mix bus, device selection, accurate discontinuity/drift correction and
  long-duration synchronization/recovery tests. Current tracks are separate.
- Automatic adaptive bitrate and controlled resolution segmentation.
- Zero-CPU-readback Quick Sync interop and validation on actual NVENC/AMF devices.
- Full-rate GPU preview, tray/global hotkeys, persistence, permissions recovery,
  crash-resistant remux/export, installers and reproducible release CI.
- Verified low-end hardware limits and sustained 4K/120 performance measurements.

The generated macOS runner and mobile reserved directory are not functional
recording products. No cross-platform or production-readiness claim is made.

The lightweight preview is separate from the WGC recording pipeline. Protected,
minimized or GPU-rendered windows can have unavailable or incomplete thumbnails.
Preview does not display the recorded cursor or verify the final encoded output.

## Distribution

This local build uses the checksum-verified Gyan FFmpeg 8.1.2 full shared SDK.
That SDK includes GPL components (including libx264). Original project sources
are MIT, but redistributing the linked application requires GPL-compatible
distribution, dependency notices and corresponding source/build information.
This local test bundle is not a signed, audited or published release.

Source SDK: https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.1.2-full_build-shared.7z

SHA-256: cba748035c21ce1431d0823c7a3a711f38616f89f87a265dceddf9b7f6749d2d

Rust tested: 1.99.0 MSVC. Flutter tested: 3.41.5 / Dart 3.11.3.
