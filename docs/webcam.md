# Webcam overlay and green background removal

The Windows engine can add a DirectShow webcam to recorded H.264 video. This is
an explicit opt-in feature; discovery enumerates devices without opening their
capture streams. No camera audio stream is opened. System audio and microphone
continue to use the selected WASAPI endpoints.

## Use

1. Open Settings → Webcam overlay & green screen → Find cameras.
2. Choose a camera, then enable Record webcam overlay.
3. Choose a corner and overlay width (10–50% of the recording). Camera aspect
   ratio is preserved; oversized portrait overlays are fitted inside the output.
4. Enable Mirror camera if desired. For a green backdrop, enable Remove green
   background and adjust Green similarity and Edge softness.
5. Choose a new MKV, MP4 or AVI output and start recording. Recording health
   reports whether camera frames are arriving and displays any camera error.

The main screen's **Webcam On/Off** button enables/disables the overlay without
opening Settings. If no device is selected, it discovers cameras and selects the
first available one; choose a different camera in Settings. With preview enabled,
the camera appears live at its selected corner, with mirror and green removal.
Preview updates at 5 fps, or 2 fps in Low resource, independently of the one-fps
screen thumbnail. The camera remains visible in Low resource preview while the
screen thumbnail is suspended during recording.

Enable the webcam before starting a take. You can then hide/show its overlay
from the main screen during recording or pause. The reader remains open during
the take so showing it again is immediate; **Off** hides it in both the preview
and subsequent recorded frames. In idle mode Off releases the preview's camera
lease. Turning the entire preview off or minimizing suspends camera thumbnails;
an active recording keeps its own camera lease. GIF does not support the overlay.

Settings persist per user; camera selection/placement lock during recording.
Preview camera pixels are a separate thumbnail layer; recording composition
continues in the native engine. Screenshots capture the source, not the overlay.

## Resources and ownership

Webcam scaling, blending and green key use the D3D11 GPU compositor when available.
Hardware encoder selection is preserved; CPU composition remains an initialization
fallback with software H.264. See [GPU composition](gpu-composition.md). Camera reading
and decoding run on a dedicated thread, with a single decoder thread and an
8 MiB DirectShow packet-buffer request. Some driver allocations are outside this
budget. Output BGRA images are limited to 640 x 480; Low resource uses a maximum
width of 320. Source inputs above 4096 x 2160 are rejected. Native camera mode is
chosen by the driver, so decoding may still process more than 15 frames/second.
Conversion/publication occurs at most once per 67 ms; only the latest image is
retained, with a reusable conversion buffer. JPEG full-range pixel formats are
normalized with explicit color-range conversion so the scaler context can be
reused. Recording frames stay in Rust. Preview and recording share one reader;
at most one <=320×240 premultiplied RGBA thumbnail crosses the binary FFI at a
time. Flutter retains one image and disposes replaced/stale images, without
filling its global image cache. Pixels are never JSON/base64 encoded.

The recording worker retains one reference-counted clean screen frame when the
camera is enabled. Reusing it keeps the camera moving when the desktop is static;
composition first obtains a writable frame, preserving the cached background.
Camera overlays are applied before drawing annotations and cursor effects.
Adaptive MKV changes retain the camera reader and update placement using the new
recording dimensions. Replay includes the encoded webcam composite.

Green removal uses RGB distance from green with adjustable similarity and a soft
alpha transition. This is a basic color key; it does not provide spill suppression,
AI background removal or a studio-grade matte. Chroma is blended in 2 x 2 blocks
for YUV420P, so fine edges have the format's usual chroma-resolution limitations.

## Failure behavior

If a camera is missing, occupied, denied permission, disconnected or stalled,
recording health displays the failure and screen/audio recording continues.
Images older than two seconds are omitted rather than held on screen indefinitely.
Pause prevents video encoding while camera reading continues.

Stop requests cancellation with FFmpeg's interrupt callback. Driver reads use a
five-second deadline and initialization uses eight seconds after enumeration.
Some device drivers ignore cancellation, and enumeration itself has no hard
timeout. Such a camera thread retains its own resources instead of blocking
recording finalization. A process-wide lease prevents opening another camera
worker until the old one exits; this bounds the number of retained camera threads
to one. In that case a later webcam recording reports that the camera is still
closing. Restarting Frame may be necessary for a broken driver.

The UI enumerates devices on a background Dart isolate, with one request in
flight. Camera discovery and recording require a driver recognized by DirectShow.
Windows desktop-camera privacy permissions must allow the app.

## Validation

Tests cover green transparency, foreground preservation, mirror orientation,
frame reference ownership, geometry limits and incomplete pixel-buffer rejection.
The UI test covers camera selection, enabling recording/keying, persistence and
locking during recording at 960 x 640. `webcam_fixture` produces deterministic
composited video without using desktop or camera pixels.

On this machine, two camera devices were enumerated and real webcam frames
were received during a ten-second desktop/audio recording with drawing, green
key, replay and a 1080p-to-720p transition. Both parts and replay fully decoded
with three AAC tracks. This does not validate every camera driver or prove key
quality on a physical green-screen setup.

The packaged low-resource build also recorded real camera frames, annotations,
three AAC tracks and replay successfully at a 24 fps target. The revised camera
converter reused its scaler without legacy JPEG-format warnings. A deliberately
missing camera produced a visible native error while screen recording finalized
and decoded successfully. These are short functionality tests, not sustained
performance measurements.

The native reader follows [FFmpeg's DirectShow device API](https://www.ffmpeg.org/ffmpeg-devices.html#dshow).
macOS/mobile camera integration and GPU webcam composition remain future work.

The WebcamPreview revision verified real camera thumbnails while recording only
an owned synthetic Win32 window. Twenty-four thumbnail reads, live overlay
hide/show, native screen thumbnails, full-window bounds and GPU pan checks passed;
the recording fully decoded. No camera thumbnail was saved or displayed by the
probe. Widget tests exercise changing images, delayed-frame cancellation, camera
errors, minimized/Low resource suspension and the main On/Off button at 960×640.

```powershell
python tools/window_capture_smoke.py --library target/webcam-preview/release/recorder_core.dll --pan-probe target/webcam-preview/release/examples/pan_fixture.exe --webcam-preview
```

The optional test opens a real camera and writes its composite into the local
owned-window test video; do not publish that video as a synthetic fixture.
