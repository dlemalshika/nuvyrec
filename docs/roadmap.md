# Implementation sequence

Progress update: the Windows alpha now records video, system/microphone AAC
tracks, and GIF, and has a native Flutter UI. Phase 1 builds/tests have run.
The remaining phases and their release gates below still apply. See
[release status](release-status.md) for tested capabilities and missing features.

## Phase 1 — architecture and encoder/muxer foundation

Delivered: Rust workspace, config validation, CPU frame wrapper, file IO ownership,
video-only session, synthetic example, lifecycle tests, C ABI bootstrap and Dart
binding. Build verification remains dependent on Rust and an FFmpeg 8 SDK.

Gate: compile with the documented SDK; run tests and Clippy; probe and decode all
90 frames in MP4/MKV/AVI; verify existing files remain unchanged; review every
unsafe block. Only then call this a validated foundation. It is not production
release ready merely because resource management is designed defensively.

## Phase 2 — Windows vertical slice

Implement WGC display/window capture, D3D11 source ownership, BGRA→NV12 GPU
conversion and a working hardware backend. Add bounded workers and control ABI,
generate Flutter runners, bundle libraries, expose source picker and native
preview, and start/stop MKV recording. Add custom region coordinates and source
resize handling. Start with one hardware backend on available test hardware,
then test NVENC, AMF and QSV independently.

Gate: 30-minute captures on declared minimum and reference PCs; verified
zero-CPU-readback path; no unbounded RAM/VRAM growth; monotonic timestamps; playable
output after stop; correct reporting of permission/device/IO failure. Publish
measured CPU, RAM, GPU, dropped frames and encode latency with hardware details.

## Phase 3 — audio and macOS

Add system/microphone sources, resampling, drift correction and independent tracks.
Build ScreenCaptureKit/Metal/VideoToolbox adapters and macOS permission UX.

Gate: hour-long audio/video sync tests with independent devices, mute and hotplug;
accurate region mapping on mixed-DPI monitors; native Apple Silicon and Intel
builds; permission denial/revocation and device sleep recovery.

## Phase 4 — overlays and adaptive quality

Add native webcam, GPU chroma key, annotation compositor, capability probes,
adaptive workload controller and explicit segment-based resolution switching.

Gate: sustained overload tests, bounded queues, hysteresis without oscillation,
correct annotation capture exclusion, visible fallback reasons, and a measured
4K/120 result on named supported hardware. Do not promise 4K/120 universally.

## Phase 5 — mobile and exports

Implement Android consent/service/MediaCodec and iOS ReplayKit extension/
VideoToolbox. Add MKV→MP4 remux and offline GIF export. Test AVI compatibility
profiles and expose a per-platform feature capability matrix.

Gate: app/background/extension termination, permission revocation, thermal and
battery tests, producing-app audio restrictions, signing/store packaging, device
ABI matrix, and screen/audio privacy UX using platform-required indicators.

## Release engineering

Pin dependencies and SDK checksums, add reproducible CI and release manifests,
audit licenses, test installers/uninstallers, document supported devices and
measured memory/performance, add crash recovery and collect opt-in diagnostics.
Benchmark comparison with OBS/Bandicam comes after equivalent workloads are
implemented and tested, not as an assumption of the architecture.
