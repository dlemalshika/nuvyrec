# Creator overlays and layouts

Open Settings → **Text, logos & creator layouts** in the Creator Windows bundle.
Add a title or choose a local PNG/JPG/JPEG/WebP logo. Enable **Record these
layers** to include them in the saved video. The preview shows placement when
preview is enabled. Select a layer to adjust its corner, X/Y position, width,
opacity and ordering. Text supports Sinhala and other Unicode through Flutter's
native font shaping; available system fonts determine glyph coverage. Titles
can use a readable background plate and selected color/font size.

Prepare these creator layouts before starting. This settings editor is locked
while a recording or schedule is active. For live edits use the main workspace's
**Scenes & live overlays** panel and enable scenes before recording. Its drafts
are committed atomically; see [live scenes](scenes.md). Missing or invalid logo files show an error; enabled
incomplete layers prevent recording from starting. Save up to twenty named
creator layouts containing layers, feature settings and video quality. Loading
a layout reloads its local logo files. Layouts do not select a different capture
source or output file. They are saved per user; image bytes are not embedded.

## Shortcut display

Enable the keyboard-shortcut display explicitly. Ctrl/Alt/Win combinations and
F1–F12 appear briefly in a bottom corner; display duration is one to five seconds.
Ordinary typing, Shift-only typing and clipboard contents are not displayed.
The media worker polls current key-down state at most every 20 ms; no keyboard
hook, key history or text log is stored. Shortcuts are global while recording
and may include combinations typed in another app. Very brief presses between
polls can be missed. Pausing suspends the recording loop.

## Resource policy and limits

All effects are off by default. Text/logo and shortcut effects use D3D11 GPU
composition when available, with a software H.264 CPU fallback; GIF is excluded. Automatic/Low resource policies
still cap the canvas and encoder work. Fonts and logos are rendered on edits,
not on recording frames. At most four assets occupy an 8 MiB native cache (RGBA plus CPU fallback colors);
upload buffers are capped at 2 MiB each. Flutter assets are downscaled to
768 × 256; source files are limited to 8 MiB and decoded dimensions to 8192 per
side / 32 million pixels. Transparent RGBA pixels retain the recorded background.
Moving a cached layer does not reload its pixels.

GPU capture keeps one source-size texture plus a two-frame WGC pool and scaled
output frames. GPU effects compose before encoder conversion; NVENC/AMF can keep
the screen surface on the GPU. Quick Sync/software still read back the composed
canvas. Small raster/camera textures are uploaded from RAM. See
[GPU composition](gpu-composition.md) for bounds and verification.
Saved layouts are a single screen/window/region plus optional webcam and raster
layers. Cut and fade-through-black visual scenes are available; multiple
simultaneous screens/cameras and transitions between capture sources remain work.

## Repeatable integration check

`Frame.exe --creator-smoke-test <absolute-output-directory>` launches a dedicated
synthetic red/blue Flutter window, without reading or writing user preferences.
It renders a Sinhala title and transparent logo, uploads their actual pixels
through the C ABI, records only its own static window and auto-stops after three
seconds. It writes test metadata/PNG/RGBA fixtures locally, then exits. It does
not activate other applications, move the pointer or poll shortcuts.

```powershell
python tools/verify_creator_fixture.py --bundle artifacts/Frame-Windows-Creator artifacts/<fixture-directory>
```

The verifier compares encoded glyph/alpha pixels against the uploaded assets
and fully decodes the video. Unit/widget tests cover cache ownership, budgets,
shortcut filtering/expiry, saved layout restoration and active-recording locks.
