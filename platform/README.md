# Native platform adapters (planned)

- `windows/`: Windows.Graphics.Capture, D3D11, WASAPI, Media Foundation webcam.
- `macos/`: ScreenCaptureKit, Metal, Core Audio, AVFoundation webcam.
- `android/`: Kotlin MediaProjection foreground service, JNI, MediaCodec/EGL.
- `ios/`: Swift ReplayKit extension, VideoToolbox, CVPixelBuffer/Metal.

Phase 1 intentionally contains no placeholder capture implementations. A backend
must report unsupported features rather than substitute a CPU pipeline silently.
See `docs/architecture.md` for the resource lifetime and capability contracts.
