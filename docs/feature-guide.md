# Windows feature build

Run `artifacts/Frame-Windows-Multistream/Frame.exe`, keeping the bundle together.

Open **Live** in the header to configure YouTube, Facebook and TikTok server/key
destinations and start streaming with local recording. See [live setup](live-streaming.md)
for account access, upload capacity, reconnect and shared-picture limitations.

## Recording

1. Choose Display, Window or Region. Region offers **Select area with mouse**;
   drag on the selected display, or cancel with Esc/right-click.
2. Choose a new output file. Existing recordings are never overwritten.
3. Open Settings for quality, audio, appearance and optional features.
4. Start recording; use pause/resume or stop/save at the bottom.

Ctrl+Shift+F9 starts/stops, F10 pauses/resumes, F11 saves replay. Settings reports
shortcut conflicts. The tray menu provides the same controls and restores Frame.

The Low-end PC preset selects 720p/24fps/3Mbps. Keep optional CPU effects off and
Low-resource mode suspends the thumbnail preview during recording. See
[performance controls](performance.md) for automatic budgets and adaptive MKV parts. Hardware encoding is
negotiated against the installed driver; a selected encoder can be unavailable.

## Webcam

Settings → Webcam overlay & green screen → Find cameras. Choose a device and
enable the overlay; choose its corner/size, mirror and green-background controls.
The saved video includes the camera; the lightweight screen preview does not.
See [webcam setup and limitations](webcam.md). Composition uses D3D11 when available; decoding still uses CPU work.

## Audio and effects

Enable system/microphone recording in the main audio settings; select devices in
Audio devices & processing. Gain/mute work live. Noise suppression/gate/limiter
are configured before recording. Both sources can produce a combined first
playback track plus two independent AAC source tracks.

Enable Live annotations before starting. Draw/arrows/boxes/spotlight tools appear
above the preview during recording, with colors, undo and clear. Coordinates map
to the captured source. These effects and display/region cursor/click rings use
GPU composition when available, with CPU fallback. Native cursor effects are visible in the saved video, not in
the lightweight thumbnail. Unicode text/logo layers and an opt-in shortcut display are available in Creator Studio.

## Replay, automation and recovery

Enable replay and choose its duration and encoded-payload memory budget before
starting. Save with the replay button or F11, selecting a new MKV file. A small
budget can shorten high-bitrate history; clips begin on an available keyframe.

Schedule a future start while Frame stays open. Duration/file-size limits stop
and finalize the recording automatically; file-size limits can overshoot by
buffered packets. Closing Frame cancels pending schedules/countdowns.

MKV recovery segments finalize playable parts at the selected interval. Earlier
completed parts survive an abrupt exit; the active part is not guaranteed. Replay
history restarts on rotation. Up to 1024 parts are supported. Import recovered
parts through the library. This is not a promise of arbitrary MP4 crash recovery.

## Library and exports

The header's library button shows indexed captures/replays. Import adds an
explicitly selected local video. Click to play in the default player; the menu
reveals, exports or removes metadata while keeping the original file.

MP4 remux copies compatible encoded tracks without quality loss. Trim/crop
re-encode with two CPU threads; enter seconds and source-pixel crop coordinates.
Progress/cancellation appear in the library. Finish/cancel exports before closing
Frame. The camera button saves a new PNG still from the selected source.

See [implementation status](professional-features.md) and
[tested scope](release-status.md) for pending advanced/platform capabilities.
