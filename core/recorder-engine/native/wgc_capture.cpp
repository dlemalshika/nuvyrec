// Non-blocking Windows.Graphics.Capture adapter. All entry points and GPU work
// belong to one media thread. Free-threaded WGC supplies frames without a UI
// dispatcher; TryGetNextFrame never waits for the desktop to change.
// The last surface is retained in a private GPU texture, so static sources can
// still be sampled, panned and encoded at the requested cadence.
#include <windows.h>
#include <dwmapi.h>
#include <d3d11.h>
#include <d3dcompiler.h>
#include <windows.graphics.capture.interop.h>
#include <windows.graphics.directx.direct3d11.interop.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Graphics.Capture.h>
#include <winrt/Windows.Graphics.DirectX.Direct3D11.h>
#include <winrt/Windows.Graphics.DirectX.h>
#include <wrl/client.h>
#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstring>
#include <memory>
#include <array>
#include <vector>
#include <stdexcept>
extern "C" {
#include <libavutil/hwcontext.h>
#include <libavutil/hwcontext_d3d11va.h>
#include <libavutil/frame.h>
}

using Microsoft::WRL::ComPtr;
using namespace winrt::Windows::Graphics::Capture;
using namespace winrt::Windows::Graphics::DirectX;
using namespace winrt::Windows::Graphics::DirectX::Direct3D11;

struct FrameCaptureOptions {
    uint64_t handle;
    int32_t window, width, height;
    uint32_t fps;
    int32_t cursor, border, aspect;
    uint32_t left, top, right, bottom;
};
namespace {
struct PhysicalCoordinates {
    DPI_AWARENESS_CONTEXT previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    ~PhysicalCoordinates() { if(previous) SetThreadDpiAwarenessContext(previous); }
};
struct Apartment {
    // WGC surfaces and DWM bounds are physical pixels. GetClientRect and
    // ClientToScreen must use the same units, including CLI/test callers.
    PhysicalCoordinates coordinates;
    Apartment() { winrt::init_apartment(winrt::apartment_type::multi_threaded); }
    ~Apartment() {
        // Cached activation factories can retain apartment-bound proxies after
        // the final RoUninitialize. Release the cache while COM is still alive.
        winrt::clear_factory_cache();
        winrt::uninit_apartment();
    }
};
struct AvRef {
    AVBufferRef* value = nullptr;
    ~AvRef() { av_buffer_unref(&value); }
};
void checked(int code, const char* label) {
    if (code < 0) throw std::runtime_error(label);
}
void report(char* error, size_t length) noexcept {
    if (!error || !length) return;
    try { throw; }
    catch (const winrt::hresult_error& e) {
        auto message = winrt::to_string(e.message());
        std::snprintf(error, length, "Windows capture: %s (0x%08x)", message.c_str(), static_cast<unsigned>(e.code().value));
    } catch (const std::exception& e) { std::snprintf(error, length, "%s", e.what()); }
    catch (...) { std::snprintf(error, length, "Unknown native capture error"); }
}
// FFmpeg shares the D3D11 immediate context with its hardware filters.
struct DeviceLock {
    AVD3D11VADeviceContext* device;
    explicit DeviceLock(AVD3D11VADeviceContext* d) : device(d) { device->lock(device->lock_ctx); }
    ~DeviceLock() { device->unlock(device->lock_ctx); }
};
constexpr char shader[] = R"(
cbuffer Geometry : register(b0) { float4 crop; float4 destination; };
Texture2D image : register(t0); SamplerState linearSampler : register(s0);
struct Vertex { float4 position : SV_POSITION; float2 uv : TEXCOORD0; };
Vertex vertex(uint id : SV_VertexID) {
    Vertex v;
    v.uv = float2((id << 1) & 2, id & 2);
    v.position = float4(v.uv * float2(2, -2) + float2(-1, 1), 0, 1);
    return v;
}
float4 pixel(Vertex v) : SV_TARGET {
    float2 p = (v.uv - destination.xy) / destination.zw;
    if (any(p < 0) || any(p > 1)) return float4(0, 0, 0, 1);
    return float4(image.Sample(linearSampler, crop.xy + p * crop.zw).rgb, 1);
}
)";

// Kept in this owning translation unit so the compositor uses the exact same
// device/context and FFmpeg lock as capture and hardware conversion.
#include "gpu_compositor.h"

class Capture {
    Apartment apartment;
    AvRef deviceRef, framesRef;
    AVD3D11VADeviceContext* hardware = nullptr;
    ComPtr<ID3D11Device> device;
    ComPtr<ID3D11DeviceContext> context;
    ComPtr<ID3D11Texture2D> cached;
    ComPtr<ID3D11ShaderResourceView> sourceView;
    ComPtr<ID3D11VertexShader> vertex;
    ComPtr<ID3D11PixelShader> pixel;
    ComPtr<ID3D11SamplerState> sampler;
    ComPtr<ID3D11Buffer> constants;
    ComPtr<ID3D11RasterizerState> rasterizer;
    ComPtr<ID3D11DepthStencilState> depth;
    ComPtr<ID3D11BlendState> blend;
    std::unique_ptr<GpuCompositor> compositor;
    IDirect3DDevice runtimeDevice{nullptr};
    GraphicsCaptureItem item{nullptr};
    Direct3D11CaptureFramePool pool{nullptr};
    GraphicsCaptureSession session{nullptr};
    winrt::Windows::Graphics::SizeInt32 poolSize{};
    uint32_t sourceWidth = 0, sourceHeight = 0;
    std::chrono::steady_clock::time_point start = std::chrono::steady_clock::now();
    std::chrono::steady_clock::time_point next{};
public:
    FrameCaptureOptions options;
    explicit Capture(FrameCaptureOptions o) : options(o) {
        if (!o.handle || o.width < 2 || o.height < 2 || !o.fps || o.fps > 240)
            throw std::runtime_error("Invalid native capture dimensions or source");
        // OS capability is stable for this process; avoid a broker query for
        // every new source or thumbnail. Permissions are checked on capture.
        static const bool supported = GraphicsCaptureSession::IsSupported();
        if (!supported) throw std::runtime_error("Windows Graphics Capture is unavailable");
        checked(av_hwdevice_ctx_create(&deviceRef.value, AV_HWDEVICE_TYPE_D3D11VA, nullptr, nullptr, 0), "Create D3D11 capture device");
        hardware = reinterpret_cast<AVD3D11VADeviceContext*>(reinterpret_cast<AVHWDeviceContext*>(deviceRef.value->data)->hwctx);
        device = hardware->device;
        context = hardware->device_context;
        ComPtr<IDXGIDevice> dxgi;
        winrt::check_hresult(device.As(&dxgi));
        winrt::com_ptr<IInspectable> inspectable;
        winrt::check_hresult(CreateDirect3D11DeviceFromDXGIDevice(dxgi.Get(), inspectable.put()));
        runtimeDevice = inspectable.as<IDirect3DDevice>();
        auto factory = winrt::get_activation_factory<GraphicsCaptureItem, IGraphicsCaptureItemInterop>();
        if (o.window) winrt::check_hresult(factory->CreateForWindow(reinterpret_cast<HWND>(o.handle), winrt::guid_of<GraphicsCaptureItem>(), winrt::put_abi(item)));
        else winrt::check_hresult(factory->CreateForMonitor(reinterpret_cast<HMONITOR>(o.handle), winrt::guid_of<GraphicsCaptureItem>(), winrt::put_abi(item)));
        poolSize = item.Size();
        if (poolSize.Width < 2 || poolSize.Height < 2) throw std::runtime_error("Capture source has no visible surface");
        framesRef.value = av_hwframe_ctx_alloc(deviceRef.value);
        if (!framesRef.value) throw std::bad_alloc();
        auto frames = reinterpret_cast<AVHWFramesContext*>(framesRef.value->data);
        frames->format = AV_PIX_FMT_D3D11;
        frames->sw_format = AV_PIX_FMT_BGRA;
        frames->width = o.width;
        frames->height = o.height;
        auto d3dFrames = reinterpret_cast<AVD3D11VAFramesContext*>(frames->hwctx);
        d3dFrames->BindFlags = D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE;
        checked(av_hwframe_ctx_init(framesRef.value), "Initialize GPU capture frames");
        initializeShaders();
        pool = Direct3D11CaptureFramePool::CreateFreeThreaded(runtimeDevice, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, poolSize);
        session = pool.CreateCaptureSession(item);
        session.IsCursorCaptureEnabled(o.cursor != 0);
        session.StartCapture();
    }
    ~Capture() {
        // Close in the owning apartment before releasing the device/context.
        try { if (session) session.Close(); } catch (...) {}
        try { if (pool) pool.Close(); } catch (...) {}
    }
    AVBufferRef* frames() { return av_buffer_ref(framesRef.value); }
    void enableCompositor() {
        DeviceLock lock(hardware);
        compositor = std::make_unique<GpuCompositor>(device.Get(), context.Get());
    }
    void overlays(const FrameGpuImage* images, size_t imageCount, const FrameGpuVertex* vertices,
                  size_t vertexCount, const FrameGpuDraw* commands, size_t commandCount) {
        if(!compositor) throw std::runtime_error("GPU compositor is not enabled");
        DeviceLock lock(hardware);
        compositor->update(images,imageCount,vertices,vertexCount,commands,commandCount);
    }
    void initializeShaders() {
        ComPtr<ID3DBlob> vs, ps, errors;
        winrt::check_hresult(D3DCompile(shader, sizeof(shader)-1, nullptr, nullptr, nullptr, "vertex", "vs_4_0", D3DCOMPILE_OPTIMIZATION_LEVEL3, 0, &vs, &errors));
        winrt::check_hresult(D3DCompile(shader, sizeof(shader)-1, nullptr, nullptr, nullptr, "pixel", "ps_4_0", D3DCOMPILE_OPTIMIZATION_LEVEL3, 0, &ps, &errors));
        winrt::check_hresult(device->CreateVertexShader(vs->GetBufferPointer(), vs->GetBufferSize(), nullptr, &vertex));
        winrt::check_hresult(device->CreatePixelShader(ps->GetBufferPointer(), ps->GetBufferSize(), nullptr, &pixel));
        D3D11_SAMPLER_DESC sample{};
        sample.Filter = D3D11_FILTER_MIN_MAG_MIP_LINEAR;
        sample.AddressU = sample.AddressV = sample.AddressW = D3D11_TEXTURE_ADDRESS_CLAMP;
        sample.MaxLOD = D3D11_FLOAT32_MAX;
        winrt::check_hresult(device->CreateSamplerState(&sample, &sampler));
        D3D11_BUFFER_DESC buffer{};
        buffer.ByteWidth = 32; buffer.Usage = D3D11_USAGE_DEFAULT; buffer.BindFlags = D3D11_BIND_CONSTANT_BUFFER;
        winrt::check_hresult(device->CreateBuffer(&buffer, nullptr, &constants));
        D3D11_RASTERIZER_DESC raster{};
        raster.FillMode = D3D11_FILL_SOLID; raster.CullMode = D3D11_CULL_NONE; raster.DepthClipEnable = TRUE;
        winrt::check_hresult(device->CreateRasterizerState(&raster, &rasterizer));
        D3D11_DEPTH_STENCIL_DESC depthDesc{};
        winrt::check_hresult(device->CreateDepthStencilState(&depthDesc, &depth));
        D3D11_BLEND_DESC blendDesc{};
        blendDesc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL;
        winrt::check_hresult(device->CreateBlendState(&blendDesc, &blend));
    }
    int poll(AVFrame* output) {
        if (options.window && (!IsWindow(reinterpret_cast<HWND>(options.handle)) || IsIconic(reinterpret_cast<HWND>(options.handle))))
            throw std::runtime_error("Selected window was closed or minimized");
        auto now = std::chrono::steady_clock::now();
        if (now < next) return 0;
        auto frame = pool.TryGetNextFrame();
        if (frame) {
            auto size = frame.ContentSize();
            if (size.Width > 0 && size.Height > 0) {
                auto access = frame.Surface().as<::Windows::Graphics::DirectX::Direct3D11::IDirect3DDxgiInterfaceAccess>();
                ComPtr<ID3D11Texture2D> surface;
                winrt::check_hresult(access->GetInterface(__uuidof(ID3D11Texture2D), reinterpret_cast<void**>(surface.GetAddressOf())));
                D3D11_TEXTURE2D_DESC desc{};
                surface->GetDesc(&desc);
                DeviceLock lock(hardware);
                D3D11_TEXTURE2D_DESC old{};
                if (cached) cached->GetDesc(&old);
                if (!cached || old.Width != desc.Width || old.Height != desc.Height) {
                    sourceView.Reset(); cached.Reset();
                    desc.Usage = D3D11_USAGE_DEFAULT; desc.BindFlags = D3D11_BIND_SHADER_RESOURCE;
                    desc.CPUAccessFlags = 0; desc.MiscFlags = 0; desc.ArraySize = 1; desc.MipLevels = 1;
                    winrt::check_hresult(device->CreateTexture2D(&desc, nullptr, &cached));
                    winrt::check_hresult(device->CreateShaderResourceView(cached.Get(), nullptr, &sourceView));
                }
                context->CopyResource(cached.Get(), surface.Get());
                sourceWidth = std::min(desc.Width, static_cast<UINT>(size.Width));
                sourceHeight = std::min(desc.Height, static_cast<UINT>(size.Height));
            }
            frame.Close();
            if (size.Width > 0 && size.Height > 0 && (size.Width != poolSize.Width || size.Height != poolSize.Height)) {
                poolSize = size;
                pool.Recreate(runtimeDevice, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, poolSize);
            }
        }
        if (!cached || sourceWidth < 2 || sourceHeight < 2) return 0;
        checked(av_hwframe_get_buffer(framesRef.value, output, 0), "Allocate GPU output frame");
        render(output);
        output->pts = std::chrono::duration_cast<std::chrono::nanoseconds>(now-start).count()/100;
        output->color_range = AVCOL_RANGE_JPEG;
        output->colorspace = AVCOL_SPC_RGB;
        output->color_primaries = AVCOL_PRI_BT709;
        output->color_trc = AVCOL_TRC_IEC61966_2_1;
        next = now + std::chrono::nanoseconds(1000000000ULL/options.fps);
        return 1;
    }
    void render(AVFrame* frame) {
        DeviceLock lock(hardware);
        D3D11_TEXTURE2D_DESC desc{}; cached->GetDesc(&desc);
        float baseX = 0, baseY = 0, baseW = static_cast<float>(sourceWidth), baseH = static_cast<float>(sourceHeight);
        if (options.window && !options.border) {
            RECT client{}, bounds{};
            auto hwnd = reinterpret_cast<HWND>(options.handle);
            if (GetClientRect(hwnd, &client) && SUCCEEDED(DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &bounds, sizeof(bounds)))) {
                POINT origin{}; ClientToScreen(hwnd, &origin);
                baseX = static_cast<float>(std::max(0L, origin.x-bounds.left));
                baseY = static_cast<float>(std::max(0L, origin.y-bounds.top));
                baseW = static_cast<float>(client.right); baseH = static_cast<float>(client.bottom);
            }
        }
        float w = baseW-options.left-options.right, h = baseH-options.top-options.bottom;
        if (w < 2 || h < 2 || baseX+options.left+w > sourceWidth || baseY+options.top+h > sourceHeight)
            throw std::runtime_error("Capture region no longer fits the selected surface");
        float geometry[8] = {(baseX+options.left)/desc.Width, (baseY+options.top)/desc.Height, w/desc.Width, h/desc.Height, 0, 0, 1, 1};
        if (options.aspect) {
            float factor = std::min(options.width/w, options.height/h);
            geometry[6] = w*factor/options.width; geometry[7] = h*factor/options.height;
            geometry[4] = (1-geometry[6])/2; geometry[5] = (1-geometry[7])/2;
        }
        auto texture = reinterpret_cast<ID3D11Texture2D*>(frame->data[0]);
        D3D11_TEXTURE2D_DESC target{}; texture->GetDesc(&target);
        D3D11_RENDER_TARGET_VIEW_DESC rtv{};
        rtv.Format = target.Format;
        if (target.ArraySize > 1) {
            rtv.ViewDimension = D3D11_RTV_DIMENSION_TEXTURE2DARRAY;
            rtv.Texture2DArray.ArraySize = 1;
            rtv.Texture2DArray.FirstArraySlice = static_cast<UINT>(reinterpret_cast<uintptr_t>(frame->data[1]));
        } else rtv.ViewDimension = D3D11_RTV_DIMENSION_TEXTURE2D;
        ComPtr<ID3D11RenderTargetView> view;
        winrt::check_hresult(device->CreateRenderTargetView(texture, &rtv, &view));
        D3D11_VIEWPORT viewport{}; viewport.Width = static_cast<float>(options.width); viewport.Height = static_cast<float>(options.height); viewport.MaxDepth = 1;
        context->UpdateSubresource(constants.Get(), 0, nullptr, geometry, 0, 0);
        context->RSSetViewports(1, &viewport); context->RSSetState(rasterizer.Get());
        context->OMSetRenderTargets(1, view.GetAddressOf(), nullptr);
        context->OMSetDepthStencilState(depth.Get(), 0); context->OMSetBlendState(blend.Get(), nullptr, 0xffffffff);
        context->IASetInputLayout(nullptr); context->IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        context->VSSetShader(vertex.Get(), nullptr, 0); context->GSSetShader(nullptr, nullptr, 0);
        context->PSSetShader(pixel.Get(), nullptr, 0);
        context->PSSetShaderResources(0, 1, sourceView.GetAddressOf()); context->PSSetSamplers(0, 1, sampler.GetAddressOf());
        context->PSSetConstantBuffers(0, 1, constants.GetAddressOf());
        context->Draw(3, 0);
        if(compositor) compositor->render(options.width, options.height);
        ID3D11ShaderResourceView* noSource = nullptr;
        context->PSSetShaderResources(0, 1, &noSource); context->OMSetRenderTargets(0, nullptr, nullptr);
        context->Flush();
    }
};
}
extern "C" void* frame_capture_open(const FrameCaptureOptions* options, AVBufferRef** frames, char* error, size_t length) noexcept {
    try {
        auto capture = std::make_unique<Capture>(*options);
        *frames = capture->frames();
        if (!*frames) throw std::bad_alloc();
        return capture.release();
    } catch (...) { report(error, length); return nullptr; }
}
extern "C" int frame_capture_next(void* pointer, AVFrame* output, char* error, size_t length) noexcept {
    try { return static_cast<Capture*>(pointer)->poll(output); }
    catch (...) { report(error, length); return -1; }
}
extern "C" void frame_capture_pan(void* pointer, uint32_t left, uint32_t top, uint32_t right, uint32_t bottom) noexcept {
    auto& options = static_cast<Capture*>(pointer)->options;
    options.left = left; options.top = top; options.right = right; options.bottom = bottom;
}
extern "C" void frame_capture_close(void* pointer) noexcept { delete static_cast<Capture*>(pointer); }
extern "C" int frame_capture_gpu_enable(void* pointer, char* error, size_t length) noexcept {
    try { static_cast<Capture*>(pointer)->enableCompositor(); return 0; }
    catch(...) { report(error,length); return -1; }
}
extern "C" int frame_capture_gpu_update(void* pointer, const FrameGpuImage* images, size_t imageCount,
    const FrameGpuVertex* vertices, size_t vertexCount, const FrameGpuDraw* commands, size_t commandCount,
    char* error, size_t length) noexcept {
    try { static_cast<Capture*>(pointer)->overlays(images,imageCount,vertices,vertexCount,commands,commandCount); return 0; }
    catch(...) { report(error,length); return -1; }
}
