# Performance and PC compatibility

Nuvyrec adapts recording work to available resources. These are conservative
budgets, not benchmark-derived promises that every PC can record smoothly.
The current implementation is Windows only.

## Modes

| Mode | Initial ceiling | CPU encoder threads | Preview during recording |
| --- | --- | --- | --- |
| Automatic (default) | 720p / 24 fps / 3 Mbps on constrained machines; otherwise 1080p / 30 fps / 8 Mbps | 1 on at most 2 logical CPUs; otherwise 2 | User controlled |
| Low resource | 720p / 24 fps / 3 Mbps | 1 | Suspended |
| Custom / maximum | Selected dimensions, FPS and bitrate | Existing encoder configuration | User controlled |

Constrained means at most two logical CPUs, at most 4096 MiB physical RAM,
or less than 1024 MiB currently available RAM. Unknown memory readings do not
count as low memory. Budgets only reduce the requested settings; small regions
are not enlarged. Aspect ratio is maintained with even output dimensions.
Custom permits requesting 4K/120; this does not establish hardware throughput.
The High quality preset selects Custom, and the Low-end PC preset selects
Low resource. Saved presets also retain the chosen performance mode.

## Capture and bounded work

Windows.Graphics.Capture uses a D3D11 target canvas at the recording dimensions.
Scaling happens on the GPU before any CPU readback. Software, GIF and Quick Sync
still read pixels back and convert them on the CPU; NVENC/AMF use GPU conversion.
Optional effects now use D3D11 composition when available, with CPU fallback.
Hardware encoder selection is preserved. See [GPU composition](gpu-composition.md). Weak GPUs,
driver restrictions and capture permissions can still prevent recording.

Thumbnail preview is independent of recording, limited to 480 x 270 at one
update per second, with one capture in flight. Disabled/inactive previews stop
updating. Low-resource recording suspends it automatically and restores it when
recording ends. Audio queues, annotation geometry, replay memory and segment
counts have explicit bounds. Replay retains the user's selected memory budget;
it is off by default. These bounds do not imply a fixed total app RAM ceiling.

## Adaptive recording

Adaptive quality is enabled by default in the Flutter UI for MKV. The legacy
native API defaults to manual quality with adaptation disabled. MP4, AVI and GIF
use initial budgets but do not change quality during recording.

Every five seconds, the worker samples available physical memory, app working
set, system CPU and app CPU. App CPU is normalized over logical CPUs; it is not
the percentage of one core. On systems with more than 64 CPUs, Windows processor
groups can limit the meaning of the system CPU reading. Failed readings are
unknown rather than fabricated zero CPU measurements.

Three consecutive overloaded observations trigger a reduction. Overload means
average encode time exceeds 80% of a frame interval, system CPU exceeds 90% with
app CPU exceeding 25%, or known available memory falls below 384 MiB. A
15-second observation cooldown follows. Static screens with few new captured
frames do not by themselves indicate overload. Quality never automatically rises.

The reduction ladder is: cap resolution at 1080p, FPS at 30, resolution at 720p,
FPS at 24, resolution at 480p, then FPS at 15. Each step reduces bitrate in
proportion to pixel rate, with a 500 kbps floor that never raises the prior
bitrate. Already smaller/slower settings remain smaller/slower.

Before changing encoder dimensions/FPS/bitrate, Nuvyrec finalizes the current MKV
and starts `-part0002.mkv`, etc. The library lists each completed part separately.
Parts can have different dimensions and small timing gaps. Replay history resets
at each boundary. Up to 1024 parts are allowed. The active part is not guaranteed
recoverable after power loss. If capture cannot reopen, the session reports a
failure and finalized earlier parts remain available. Encoder reopening may
temporarily retain both old and new encoding resources.

**Reduce recording load** in Recording health requests one immediate reduction
using the same real segment transition, when adaptive MKV recording is enabled.
Health also displays effective recording settings and five-second CPU/RAM
measurements; selected settings can differ from the effective budget.

## Validation and remaining hardware work

Policy tests cover constrained hardware, preserving manual choices, unknown
memory, transient pressure, cooldown and a monotonic quality ladder. Real native
capture tested 1080p to 720p reduction with annotations, three AAC tracks, replay
and automatic stop. Both MKV parts and replay decoded fully. Low-resource region
capture recorded at 24 fps with Quick Sync and system audio on this machine.
These tests exercise capture and transitions, not a sustained throughput score.

NVENC/AMF texture allocation failed on this machine; fallback succeeded. Actual
weak dual-core/4 GB PCs, several NVIDIA/AMD/Intel drivers, sustained 4K/120,
long-session A/V drift, thermals and concurrent workloads still require hardware
validation. macOS/mobile performance backends are not implemented.

Implementation: `core/recorder-ffi/src/performance.rs` and `worker.rs`.
Capture canvas options follow the [FFmpeg gfxcapture implementation](https://www.ffmpeg.org/doxygen/8.1/vsrc__gfxcapture_8c_source.html).
CPU accounting follows [Windows GetSystemTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getsystemtimes).
