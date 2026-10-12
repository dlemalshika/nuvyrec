# Live visual scenes

Run `artifacts/Nuvyrec-Windows-Scenes/Nuvyrec.exe` with its whole bundle intact.

1. Choose the screen, window or region and a new recording file as usual.
2. Open **Scenes & live overlays** beside the capture-source controls. Enable
   **live scenes for the next recording**. For a camera, choose its device and
   enable it in Settings before starting. Live takes reuse that camera reader.
3. Add draft Unicode text or local logo images (up to four), adjust position,
   width and opacity, and choose camera visibility, position, width and mirror.
   **Save draft as scene** stores up to twenty named visual descriptions per user.
4. **Apply layout** selects the initial visual layout before recording. Start an
   H.264 recording or live stream. Reopen the panel, load/edit a draft and press
   **Take live**. The play button beside a saved scene loads and takes it directly.
5. Choose **Cut** (default), or a 300/600/1000 ms fade through black. A fade dims
   the old scene, swaps the complete layout at the midpoint, then reveals the new
   scene. This is a dip-to-black transition, not an overlapping crossfade.

Draft edits do not affect the on-air scene. All Unicode/logo rasters must be
ready before committing; a missing logo or incomplete upload leaves the previous
native and UI layout intact. Logo paths are saved locally; image bytes are not
embedded. Replace a missing file or remove its layer before taking that scene.
The main preview adopts the layout at the transition midpoint; it remains a
low-rate source thumbnail and does not display the encoded fade animation.

## Scope and recording continuity

Scenes share the **same capture source** and output dimensions. They do not switch
windows, crop areas, screens, capture devices, output files, audio controls or
encoding/streaming settings. A take changes only cached visual layers and camera
composition. Source-changing scenes, multiple simultaneous capture sources,
full-screen break cards, crossfades and Studio Mode preview/program are future
work. This panel's draft workflow is not a claim of a complete OBS scene graph.

The same composed video is encoded once and reaches the local recording and all
enabled stream destinations. No encoder or network worker restarts on a take.
Existing annotation and cursor effects remain session settings. Pause/starting/
stopping/transitioning sessions reject new live takes; pause waits for an active
transition to finish. GIF is excluded. Use the original creator settings editor
for idle preparation of full layouts including quality/audio choices.

## Native contract and resource limits

ABI 1.6 adds JSON scene commands and an optional upload transaction field:

- `scene_begin` returns a monotonically increasing `transaction` token, opening
  one empty four-slot staged bank. A newer begin invalidates the prior token.
- `recorder_layer_rgba_json` uploads each bounded straight-alpha raster with
  that token. Active recording rejects legacy uploads without a transaction.
- `scene_commit` receives `scene: {transaction, count, name, duration_ms, webcam}`.
  Slots `0..count` must all exist, other slots must be empty, names are limited
  to 48 characters, and duration is 0–1000 ms. Invalid takes retain the old bank.
- `scene_abort` releases only the staged bank matching its token. Stale aborts
  cannot destroy newer work. `clear_layers` is idle-only.

Each bank is bounded to 8 MiB of RGBA and cached fallback colors. At most an
active and a staged/transition bank coexist (16 MiB of these CPU caches). Current
GPU textures and temporary Flutter draft/render buffers add their own bounded
overhead; this is not a total-process RAM ceiling. Arc readers retain images
without copying them per frame. A transition blocks another staged bank.

Fade is one six-vertex solid draw on the existing D3D11 target. There is no second
capture, encoder, full-frame transition buffer or added GPU readback. Software
fallback blends limited-range YUV planes with copy-on-write safety. CPU fallback
fade costs a full-frame pass; choose Cut and Low resource on a weak machine.
Live scenes are off by default so ordinary recording avoids their compositor
work. Saving a scene while idle enables scenes for the next H.264 recording.

## Validation

Rust tests cover budget rejection, incomplete and stale transactions, atomic
replacement, transition timing, valid placement and shared-buffer CPU fading.
Flutter tests cover a missing logo retaining the previous usable layout,
compact dark/light controls and live-take gating. The owned-window integration
test uses generated pixels and three localhost receivers, never real accounts:

```powershell
python tools/window_capture_smoke.py --library target/gpu-overlays/release/recorder_core.dll --pan-probe target/gpu-overlays/release/examples/pan_fixture.exe --gpu-probe target/gpu-overlays/release/examples/gpu_fixture.exe --multistream --scenes
```

Actual recorded and received pixels verify Cut, a full-canvas black midpoint,
the second scene's position, and complete clearing. Hardware webcam placement,
device diversity, sustained low-end throughput and real platform ingest remain
manual validation tasks.
