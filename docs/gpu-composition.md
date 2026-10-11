# Windows GPU composition

Enable optional effects in Settings/Creator Studio and keep the desired encoder
selected. Recording health reports **Overlays: D3D11 GPU** or **CPU fallback**.
Automatic tries NVENC, AMF, Quick Sync, then software H.264; overlays no longer
force software encoding. Explicit encoder choices remain explicit.

The capture owner's D3D11 immediate context renders a fresh scaled BGRA output,
then blends the webcam, annotations, raster layers/shortcut label and cursor
effects in order. Mirror and soft green key are evaluated in the pixel shader.
Annotation strokes/arrows/boxes use bounded triangle geometry; spotlight uses
one shaded quad. Alpha is straight, and transparent logo pixels preserve the
background. Physical DPI coordinates are used for native client-area crops.

NVENC/AMF conversion can keep the composed screen surface on the GPU. Quick Sync
and software encoding still read the scaled canvas back to RAM. Webcam decoding
and text/logo rasterization remain CPU work: small images are uploaded to the
GPU, so this is not an end-to-end zero-copy camera pipeline. It also does not
implement Metal/mobile backends or a full-rate GPU UI preview.

Resource limits:

- Six GPU image slots: four creator layers, one shortcut label, one webcam.
- Creator images at most 1024 × 512; camera at most 640 × 480, latest frame only.
- Native creator cache reserves 8 MiB for both RGBA and CPU fallback colors,
  preserving the previous 4 MiB source-pixel capacity. GPU textures are additional.
- Raster uploads occur when identity changes; camera uploads when its frame
  changes. Removed slots are released. Layer identities retain their Rust Arc.
- At most 64 annotations, 256 points each, plus two generated cursor objects.
  Native draw packets cap at 110,000 vertices / 80 commands; geometry uploads
  only when it changes. Effect refresh is capped at the configured recording FPS.
- GPU work and texture uploads stay on the media thread under FFmpeg's shared
  device lock. No per-effect full-screen CPU image is allocated.

If GPU compositor initialization fails, Automatic can continue to software H.264
and CPU composition. Initialization failure is reported in fallback reasons;
explicit hardware requests fail clearly. A later device failure terminates the
take with an error rather than silently omitting effects. CPU fallback retains
the existing frame ownership, bounds checks and low-resource encoder policy.

Validation uses a caller-owned synthetic window and generated camera pixels.
The native GPU fixture checks annotation color, raster alpha, webcam mirror/key,
clearing stale layers and spotlight shading. Rust packet tests check worst-case
geometry, invalid inputs, draw order and retained identities.
The packaged creator fixture also recorded Unicode/logo layers with D3D11 plus
Intel Quick Sync and passed full decode and glyph/alpha checks. Capture teardown
releases cached WinRT factories before apartment shutdown, allowing subsequent
preview creation on another media thread.

Hardware encoder interop and sustained performance still require testing on actual NVENC/AMF GPUs;
no universal low-end or 4K/120 throughput guarantee is made.

```powershell
. ./tools/native-env.ps1
cargo build -p recorder-ffi --release --locked --target-dir target/gpu-overlays
cargo build -p recorder-engine --examples --release --locked --target-dir target/gpu-overlays
python tools/window_capture_smoke.py --library target/gpu-overlays/release/recorder_core.dll --pan-probe target/gpu-overlays/release/examples/pan_fixture.exe --gpu-probe target/gpu-overlays/release/examples/gpu_fixture.exe
```
