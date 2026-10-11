# Native multistreaming (Windows alpha)

Frame can send one encoded H.264 picture and one AAC playback track to up to
three independent RTMP/RTMPS destinations: YouTube, Facebook and TikTok. A local
MKV or MP4 recording is kept at the same time. This feature uses FFmpeg libraries
in the Rust core; it does not launch an encoder process or embed a browser.

## Setup

1. Choose the capture source, webcam/overlays and a **new** local MKV/MP4 output.
   Enable system sound and/or microphone as needed.
2. Open **Live** in the workspace header. Select the desired platforms and paste
   each platform's server URL and stream key. Prefer RTMPS when provided.
3. Click **Start live + record**. Frame requests up to 720p, 30 fps and 4 Mbps,
   bounded by the selected quality and machine performance mode. Both audio
   sources are mixed into the first playback track; local source tracks remain.
   With both sources disabled, a silent AAC track is generated for ingest
   compatibility; no audio device is opened.
4. Check destination health and the platforms' own previews. Some dashboards
   require their own **Go Live** action; sending packets is not proof of a public
   broadcast. Manage title, audience/privacy and scheduling on each platform.
5. **Stop live · keep recording** interrupts network workers without stopping
   local recording. The workspace Stop button stops both. Recording limits,
   hotkey Stop and closing the app also stop the streams.
   End the broadcast in each platform's dashboard/app as well.

TikTok must provide encoder server/key access for your account. Frame cannot
grant LIVE eligibility or bypass restrictions. YouTube and Facebook also require
an eligible account and a valid destination/key. Enter credentials locally;
do not paste them into an issue, screenshot or chat.

Official setup references: [YouTube encoder setup](https://support.google.com/youtube/answer/2907883),
[YouTube encoder recommendations](https://support.google.com/youtube/answer/2853702),
[Facebook Live Producer](https://www.facebook.com/live/producer), and
[TikTok encoder connection instructions](https://inapp.tiktokv.com/falcon/webcast_mt/page/obs_intro/index.html).

## Resource and connection behavior

- Encoding is shared: three destinations do not run three video encoders. CPU
  work still includes network muxing, packet copies and RTMPS encryption.
- Upload bandwidth is multiplied by destination count. At 4 Mbps video plus
  160 kbps AAC, three targets carry about 12.5 Mbps before protocol overhead;
  allow roughly 13–15 Mbps or more of stable upload capacity. Lower quality for
  weaker connections. The UI's estimate is a planning allowance, not a speed test.
- Each receiver has a separate worker, an encoded queue capped at 4 MiB and
  256 packets, and at most one in-flight packet (up to 4 MiB). Immutable encoded
  payloads are shared across queued destinations; raw frames are never queued.
- A connection/write has a five-second FFmpeg interrupt deadline and a
  five-second protocol IO timeout. Failed outputs retry with 1, 2, 4, 8 and
  then 10-second delays. OS DNS behavior can exceed the protocol timeout.
- Queue overflow discards that destination's backlog and restarts its connection
  at a new video keyframe. It does not discard arbitrary predictive frames and
  continue a corrupted chain. Keyframes are requested every two seconds.
- Each connection resets timestamps at its first keyframe. Delayed audio from
  before that point is excluded. A failed target leaves the others and local
  muxer running.
- Stop-live signals all network interrupt callbacks immediately; joining workers
  happens at session teardown. Capture does not wait for network shutdown.

## Credentials and limits

Keys are masked and kept in process memory only. They are excluded from saved
preferences, creator layouts, status and application errors. URLs with user-info,
query/fragment fields, whitespace or control characters are rejected; signed
tokens belong in the key field. Only RTMP/RTMPS and their TCP/TLS transports are
allowed. FFmpeg's process-wide logger becomes quiet when streaming is enabled,
because its protocol messages may include credential-bearing URLs. Numeric
error codes and fixed operation names remain available in native status.
RTMPS explicitly enables peer certificate verification; the packaged GnuTLS
backend checks the certificate against its trust store. Trust failures remain
connection failures and never silently downgrade to unverified TLS or RTMP.

All targets currently receive **the same aspect ratio, quality and audio mix**.
Separate portrait TikTok and landscape YouTube feeds would need a second
composition/encoding pipeline. Live streaming disables segmented recovery and
adaptive resolution for that session: reconnecting to a changing codec profile
is not implemented. Pause is unavailable until live streaming is stopped.
Connecting streaming to an already-running recording, separate stream-only
sessions, SRT, platform OAuth, chat aggregation and cloud relay are future work.

## Local verification

`tools/streaming_smoke.py` starts owned loopback FFmpeg receivers and drives
`core/recorder-engine/examples/stream_fixture.rs` with generated video and audio.
It checks three simultaneous destinations, a stalled receiver, disconnect/retry,
playable H.264/AAC outputs, independent local audio preservation, and stopping
publishing while local recording continues. It never captures a real desktop or
contacts a platform account. Real platform acceptance, TLS ingest and sustained
upload/hardware performance require account-specific testing before release.

```powershell
. ./tools/native-env.ps1
cargo build --locked --release -p recorder-engine --example stream_fixture --target-dir target/gpu-overlays
python tools/streaming_smoke.py --ffmpeg "$env:FFMPEG_DIR/bin/ffmpeg.exe"
```

Add `--tls-test` to also check rejection of a generated, untrusted self-signed
TLS receiver. This optional fixture requires Python's `cryptography` package;
its test certificate is generated locally and is never installed as trusted.

`tools/window_capture_smoke.py --multistream` additionally verifies the packaged
JSON FFI using an owned synthetic window and three loopback receivers. It checks
generated silent AAC with both devices off, secret-free status, pause protection,
and Stop-live while capture continues, alongside window/preview/GPU regressions.
