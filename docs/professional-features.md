# Professional recorder feature candidates

Research date: 2026-10-10. The research tables below describe the target product.
The implementation status is recorded separately here and in release-status.md.
Priorities are recommendations for Nuvyrec's lightweight recorder focus.

## Current implementation status

| Capability | Windows feature alpha |
|---|---|
| Hotkeys/tray, drag region, countdown | Implemented; native compilation/startup checked; drag interaction needs manual testing |
| Audio endpoints, gain/mute, meters, mix | Implemented; actual three-track capture and DSP verified |
| Quality presets, schedules, auto-stop | Implemented; schedules require Nuvyrec open; duration stop verified |
| Library, screenshots, remux, trim/crop | Implemented; synthetic export/probe/decode tests passed |
| Share/export presets | Implemented; landscape, vertical, square, small-file MP4; audio-track selection and bounded video threading |
| Replay history | Implemented; bounded encoded-packet history, MKV save, keyframe/timestamp tests |
| Recovery segmentation | Implemented for MKV; finalized parts retained; current part not guaranteed |
| Drawing/arrows/boxes/spotlight | Implemented in preview and recorded CPU compositor; Unicode titles prepared before recording |
| Cursor highlight/click rings, shortcut display | Implemented; shortcuts opt-in, ordinary typing excluded, GPU composition / CPU fallback |
| GPU overlays/compositing | Implemented on Windows D3D11; hardware encoder choice preserved; generated-pixel GPU validation passed |
| Webcam/chroma key | Implemented; bounded camera worker, actual capture verified |
| Text/logo layers and saved creator layouts | Implemented; four cached layers, twenty layouts; live visual editing through the Scenes panel |
| Live visual scenes and transitions | Implemented; twenty scenes on one capture source, atomic takes, Cut / Fade through black and live webcam placement; multiple capture sources pending |
| RTMP/RTMPS multistream and reconnect | Implemented Windows alpha; real platform validation pending |
| GitHub release notifications | Implemented; daily idle checks, Stable/Beta, Later/Skip and browser downloads; automatic installation pending |
| Windows Setup.exe | Implemented per-user installer, shortcuts and uninstall; local lifecycle smoke passed; signing and clean-machine QA pending |
| SRT, platform account integrations, automatic captions/smart zoom | Pending |
| Adaptive quality | Implemented for segmented MKV; measured overload reduces quality without increasing resources |
| HEVC/AV1/HDR | Pending |
| macOS, Android, iOS | Native capture integration pending |

Optional CPU effects are off by default. Enable them before recording; audio
gain and mute can change live. Configurations/quality presets and library
metadata persist per user. Scheduled starts are deliberately session-only.

## First priority: everyday recording and reliability

| Feature | User benefit | Implementation notes |
|---|---|---|
| Global start/stop/pause hotkeys and tray controls | Control recording without reopening the app | Native registration; show conflicts; protect active recording on exit |
| Drag-to-select region and capture countdown | Set the exact area visually and prepare before capture | Native overlay, physical-pixel/DPI handling, exclude controls from capture |
| Audio device selection, meters and mute/gain | Confirm the right microphone and avoid silent/clipped takes | Bounded RMS/peak telemetry from the audio worker; mixed playback track plus separate source tracks |
| Saved recording presets | Quickly switch between low-end, tutorials and high-quality work | Capability-based recommendations; preferences saved separately from sessions |
| Recoverable recording and MKV-to-MP4 remux | Reduce loss after a crash and export without re-encoding | Periodic segmentation/checkpointing; never promise recovery of all MP4 failures |
| Disk space, dropped-frame and encoder-load indicators | Notice storage/performance problems before a take is lost | Real telemetry; measured limits; no arbitrary percentage claims |
| Cursor highlight, click indicators and keystroke overlay | Make tutorials easier to follow | GPU compositor; configurable keystrokes; protect sensitive input |
| Recording library and screenshot button | Find/play recordings and grab stills quickly | Local metadata, native reveal, avoid indexing private folders automatically |

OBS documents global hotkeys, tray integration, replay buffer, multiple audio
devices and audio filters in its [overview](https://obsproject.com/kb/OBS-Studio-Overview).
Its [recording output guide](https://obsproject.com/kb/standard-recording-output-guide)
documents remuxing. Bandicam documents [cursor/click effects](https://www.bandicam.com/guide/overlay-cursor/).
Countdown, drag selection, telemetry and presets above are our product recommendations;
this table is not a claim that every competitor implements every listed behavior.

## Second priority: tutorials and creator workflows

| Feature | User benefit | Notes |
|---|---|---|
| Webcam overlay and chroma key | Presenter alongside the captured screen | GPU composition, camera permissions, movable/resizable layer |
| Drawing, arrows, text and spotlight | Explain details while recording | Native annotation overlay routed into the compositor |
| Noise suppression, noise gate and limiter | Clearer speech and fewer distracting sounds | Prefer efficient local DSP; keep optional expensive processing disabled on weak machines |
| Scheduled start and duration/file-size auto-stop | Record unattended sessions predictably | Explicit scheduling UI, clock changes, sleep/resume behavior |
| Trim, crop and export presets | Produce shareable clips without a full editor | Remux cuts where possible; re-encode only when required |

OBS documents chroma key, noise suppression and noise gate in its
[overview](https://obsproject.com/kb/OBS-Studio-Overview). Bandicam provides
[scheduled recording](https://www.bandicam.com/guide/scheduled-recording/) and
[automatic stop](https://www.bandicam.com/guide/auto-stop-recording/).
[Camtasia's features](https://www.techsmith.com/camtasia/features/) cover editing,
cursor effects, noise removal and transcription/captions.

## Later: advanced capabilities

- **Replay buffer / instant replay:** retain the last 30–120 seconds as encoded
  packets with a strict memory budget. Avoid raw-frame buffering on low-RAM PCs.
- **Scenes and source layers:** saved layouts, text/logo overlays, transitions
  and multiple displays/cameras. OBS documents these scene workflows.
- **Streaming:** RTMP/RTMPS multistream now shares encoding with local recording.
  SRT, separate recording quality and platform account integrations remain pending.
  See [live streaming](live-streaming.md).
- **Modern codecs / HDR:** capability-tested HEVC/AV1, HDR metadata and proper
  SDR tone mapping. Account for target-player compatibility and build licensing.
- **Automatic captions and smart zoom:** optional post-processing; heavier models
  should not compete with capture/encoding on low-end hardware.
- **Adaptive quality:** lower encoder cost or FPS under load. Resolution changes
  need explicit segmented-file handling because many encoders/containers cannot
  change resolution transparently inside one session.

Replay buffer and scenes are described by [OBS](https://obsproject.com/kb/OBS-Studio-Overview).
Captions are described by [Camtasia](https://www.techsmith.com/camtasia/features/).
The remaining items are Nuvyrec architecture/product recommendations.

## Implemented appearance setting

Dark and Light themes are selectable in Settings → Appearance. The workspace,
settings cards, text, borders and controls adapt together. Windows stores the
selection per user at `HKCU\Software\NuvyrecScreenRecorder\Theme`; it is restored
at startup. Changing appearance does not restart or reconfigure a recording.
The capture preview keeps a dark image backdrop in both themes.
