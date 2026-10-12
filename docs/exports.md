# Local MP4 export presets

Open **Recordings**, use a recording's menu → **Trim / crop / export**, and
choose **Share / compress**. Choose a preset and audio mode, optionally enter a
start time and duration, then **Export** to a new filename. Import also accepts
local video files; Nuvyrec does not search folders automatically.

| Preset | Output |
|---|---|
| Original size | Even source dimensions; source frame rate, CRF 20 |
| YouTube / landscape | At most 1920 × 1080 and 30 fps; CRF 21 |
| Vertical / stories | Padded 1080 × 1920 canvas, at most 30 fps; CRF 23 |
| Square / social | Padded 1080 × 1080 canvas, at most 30 fps; CRF 23 |
| Small file | At most 1280 × 720 and 30 fps; CRF 28 |

These are Nuvyrec defaults, not upload-platform requirements. Share presets preserve
aspect ratio, normalize non-square pixels and never enlarge source content.
Social canvases add black padding rather than crop or stretch the recording.
Slower source frame rates remain unchanged. Trim/crop operations can also use a
preset: crop first, then fit the remaining region to the output bounds.

Re-encoding uses H.264 YUV420P, AAC 128 kbit/s per selected track, and MP4 fast-start.
Video decoder, encoder and filter pools each have a two-thread limit; that is
not a two-thread limit for the whole process. One export runs at a time, with
progress and cancellation. Optional exports use bundled FFmpeg tools locally;
screen capture remains in the Rust library. These exports currently use software
encoding. Duration/file size and speed depend on the source and CPU; a CRF preset
does not guarantee an exact file size.

Audio can preserve all tracks, keep only the first track, or be removed. Share
presets default to the first track for straightforward player compatibility.
That track is the combined mix only if the recording contains a mix first;
otherwise it may contain just system audio or the microphone. Export does not mix
separate tracks. No-audio recordings also export successfully.

**MP4 remux** copies encoded tracks without resizing or re-encoding; trimming and
preset controls are hidden in that mode. Unsupported source codecs may fail to
remux into MP4; use Share / compress to convert them. An existing output filename
is refused, and the input is never overwritten. Cancellation can leave a partial
output for inspection; the source stays intact.

Synthetic integration tests cover all four share profiles, high/low frame rates,
no upscaling, black social padding, three audio modes, full decode, precise crop,
trim and overwrite refusal. Widget tests check both themes at 960 × 640.
