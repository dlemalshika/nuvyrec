# Window capture and Follow mouse

Use the latest local bundle at `artifacts/Nuvyrec-Windows-Creator/Nuvyrec.exe`.
Keep the runtime files beside the executable.

## Windows

Select **Window**, then the app in **Source**. Capture now includes the title bar
and full visible window bounds. Dimensions and pointer coordinates use physical
pixels under a per-monitor DPI context. The source list refreshes every three
seconds while idle. An explicitly selected window that disappears is never
silently replaced with a different app. Minimized windows are listed with a
restore label and cannot start a recording until restored.

Window preview uses an independent Windows.Graphics.Capture/D3D11 worker, rather
than a window DC. It scales on the GPU before reading a thumbnail into memory.
Flutter polls one cached PNG per second (at most 480 × 270, 2 MiB), with one latest
request and one capture graph. Switching sources replaces the graph; idle preview
releases its capture pool after two seconds. Static windows keep their last PNG.
Low resource mode suspends previews during recording.

Windows capture protection, elevated-app permissions, unavailable graphics
drivers and minimized apps can still prevent capture. Restore the window before
recording. Full-resolution window **screenshots** use visible desktop bounds,
so overlapping applications may appear in those screenshots.

## Follow mouse

Choose **Region**, select the display and drag an area, then enable **Follow mouse**.
The initial selection defines the fixed capture width/height. During recording,
the viewport eases toward the pointer over about 160 ms, clamps at monitor edges,
and keeps fast pointer moves inside the viewport. Moving onto another display
leaves the viewport on its selected display. Pause freezes movement.

Cursor recording is included when Follow mouse is enabled. The preview samples
the current recording viewport at its existing one-second rate. Recording FPS
and resolution are controlled separately in Settings. The option is persisted,
but sent to the core only for Region mode. It is locked during recording.

The capture owner updates the native D3D11 shader crop before sampling frames.
Texture pools, capture source, canvas dimensions and encoder stay allocated.
This avoids graph reconstruction on pointer movement. Free-threaded WGC is
polled without waiting for a frame event; one private GPU texture retains the
last screen surface. Static windows therefore continue producing samples and
respond to Stop/Save. An FFmpeg buffer source receives the GPU frames and runs
conversion/encoding. There is no FFmpeg source-filter event wait or private
AVOption mutation. Adaptive quality reinitialization restores the viewport.

## Validation

Rust viewport tests cover smooth movement, fast jumps, monitor boundaries,
unchanged dimensions and missing pointer samples. Flutter tests cover saved
preferences, mode-specific start configuration and the one-second preview rate.

`tools/window_capture_smoke.py` creates only its own non-activating synthetic
window. It checks decoded full-window video and native thumbnail title bars and
bottom corners, verifies minimized-window rejection without an output file, and
uses `core/recorder-engine/examples/pan_fixture.rs` to assert that changing GPU
crop offsets on one graph produces red-left and blue-right frames. Neither probe
moves the system pointer. Media stays in the local artifacts folder.

```powershell
. ./tools/native-env.ps1
cargo build -p recorder-ffi --release --target-dir target/creator-final
cargo build -p recorder-engine --example pan_fixture --release --target-dir target/creator-final
python tools/window_capture_smoke.py --library target/creator-final/release/recorder_core.dll --pan-probe target/creator-final/release/examples/pan_fixture.exe
```
