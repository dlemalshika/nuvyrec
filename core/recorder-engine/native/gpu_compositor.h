// D3D11 compositor owned by the capture thread. No screen readback: only small
// camera/raster textures and bounded triangle geometry are uploaded from RAM.
#pragma once
#include <array>
#include <vector>

struct FrameGpuVertex { float position[2], uv[2], color[4]; };
struct FrameGpuDraw { uint32_t mode, slot, first, count; float parameters[4]; };
struct FrameGpuImage { const uint8_t* pixels; uint64_t identity; uint32_t width, height, bgra, slot; };

class GpuCompositor {
    struct Texture {
        ComPtr<ID3D11Texture2D> texture;
        ComPtr<ID3D11ShaderResourceView> view;
        uint64_t identity = 0;
        uint32_t width = 0, height = 0;
    };
    ID3D11Device* device;
    ID3D11DeviceContext* context;
    std::array<Texture, 6> textures;
    ComPtr<ID3D11VertexShader> vertex;
    ComPtr<ID3D11PixelShader> pixel;
    ComPtr<ID3D11InputLayout> layout;
    ComPtr<ID3D11Buffer> constants, geometry;
    ComPtr<ID3D11BlendState> blend;
    ComPtr<ID3D11SamplerState> sampler;
    std::vector<FrameGpuVertex> cached;
    std::vector<FrameGpuDraw> draws;
    uint32_t capacity = 0;
public:
    GpuCompositor(ID3D11Device* d, ID3D11DeviceContext* c) : device(d), context(c) {
        constexpr char code[] = R"(
cbuffer Settings : register(b0) { float4 viewportMode; float4 parameters; };
Texture2D image : register(t0); SamplerState sampleImage : register(s0);
struct V { float2 position : POSITION; float2 uv : TEXCOORD; float4 color : COLOR; };
struct P { float4 position : SV_POSITION; float2 uv : TEXCOORD; float4 color : COLOR; };
P vs(V v) { P p; p.position=float4(v.position*viewportMode.xy*float2(2,-2)+float2(-1,1),0,1); p.uv=v.uv; p.color=v.color; return p; }
float4 ps(P p) : SV_TARGET {
    if (viewportMode.z == 1) return p.color;
    if (viewportMode.z == 2) {
        // Darken outside the spotlight with black straight-alpha blending.
        float a=distance(p.position.xy, parameters.xy)>parameters.z ? 0.6666667 : 0;
        return float4(0,0,0,a);
    }
    float4 color=image.Sample(sampleImage,p.uv);
    if (viewportMode.z == 3) {
        // Same normalized RGB distance to pure green as the CPU compositor.
        color.a=saturate((distance(color.rgb,float3(0,1,0))/sqrt(3.0)-parameters.y)/parameters.z);
    }
    color.a *= parameters.x;
    return color;
}
)";
        ComPtr<ID3DBlob> vs, ps, error;
        winrt::check_hresult(D3DCompile(code, sizeof(code)-1, nullptr, nullptr, nullptr, "vs", "vs_4_0", D3DCOMPILE_OPTIMIZATION_LEVEL3, 0, &vs, &error));
        winrt::check_hresult(D3DCompile(code, sizeof(code)-1, nullptr, nullptr, nullptr, "ps", "ps_4_0", D3DCOMPILE_OPTIMIZATION_LEVEL3, 0, &ps, &error));
        winrt::check_hresult(device->CreateVertexShader(vs->GetBufferPointer(), vs->GetBufferSize(), nullptr, &vertex));
        winrt::check_hresult(device->CreatePixelShader(ps->GetBufferPointer(), ps->GetBufferSize(), nullptr, &pixel));
        D3D11_INPUT_ELEMENT_DESC elements[] = {
            {"POSITION",0,DXGI_FORMAT_R32G32_FLOAT,0,0,D3D11_INPUT_PER_VERTEX_DATA,0},
            {"TEXCOORD",0,DXGI_FORMAT_R32G32_FLOAT,0,8,D3D11_INPUT_PER_VERTEX_DATA,0},
            {"COLOR",0,DXGI_FORMAT_R32G32B32A32_FLOAT,0,16,D3D11_INPUT_PER_VERTEX_DATA,0}
        };
        winrt::check_hresult(device->CreateInputLayout(elements, 3, vs->GetBufferPointer(), vs->GetBufferSize(), &layout));
        D3D11_BUFFER_DESC buffer{}; buffer.ByteWidth=32; buffer.Usage=D3D11_USAGE_DEFAULT; buffer.BindFlags=D3D11_BIND_CONSTANT_BUFFER;
        winrt::check_hresult(device->CreateBuffer(&buffer, nullptr, &constants));
        D3D11_BLEND_DESC b{}; auto& target=b.RenderTarget[0]; target.BlendEnable=TRUE;
        target.SrcBlend=D3D11_BLEND_SRC_ALPHA; target.DestBlend=D3D11_BLEND_INV_SRC_ALPHA; target.BlendOp=D3D11_BLEND_OP_ADD;
        target.SrcBlendAlpha=D3D11_BLEND_ONE; target.DestBlendAlpha=D3D11_BLEND_INV_SRC_ALPHA; target.BlendOpAlpha=D3D11_BLEND_OP_ADD;
        target.RenderTargetWriteMask=D3D11_COLOR_WRITE_ENABLE_ALL;
        winrt::check_hresult(device->CreateBlendState(&b, &blend));
        D3D11_SAMPLER_DESC s{}; s.Filter=D3D11_FILTER_MIN_MAG_MIP_POINT;
        s.AddressU=s.AddressV=s.AddressW=D3D11_TEXTURE_ADDRESS_CLAMP; s.MaxLOD=D3D11_FLOAT32_MAX;
        winrt::check_hresult(device->CreateSamplerState(&s, &sampler));
    }
    void update(const FrameGpuImage* images, size_t imageCount, const FrameGpuVertex* vertices,
                size_t vertexCount, const FrameGpuDraw* commands, size_t commandCount) {
        if (imageCount>6 || vertexCount>110000 || commandCount>80) throw std::runtime_error("GPU overlay budget exceeded");
        std::array<bool,6> used{};
        for (size_t i=0;i<imageCount;++i) {
            const auto& image=images[i];
            if (image.slot>=6 || used[image.slot] || !image.pixels || !image.width || !image.height || image.width>1024 || image.height>512)
                throw std::runtime_error("Invalid GPU overlay image");
            used[image.slot]=true;
            auto& t=textures[image.slot];
            D3D11_TEXTURE2D_DESC old{}; if(t.texture) t.texture->GetDesc(&old);
            auto format=image.bgra ? DXGI_FORMAT_B8G8R8A8_UNORM : DXGI_FORMAT_R8G8B8A8_UNORM;
            if (!t.texture || t.width!=image.width || t.height!=image.height || old.Format!=format) {
                t=Texture{};
                D3D11_TEXTURE2D_DESC desc{}; desc.Width=image.width; desc.Height=image.height; desc.MipLevels=desc.ArraySize=1;
                desc.Format=format; desc.SampleDesc.Count=1; desc.Usage=D3D11_USAGE_DEFAULT; desc.BindFlags=D3D11_BIND_SHADER_RESOURCE;
                winrt::check_hresult(device->CreateTexture2D(&desc, nullptr, &t.texture));
                winrt::check_hresult(device->CreateShaderResourceView(t.texture.Get(), nullptr, &t.view));
                t.width=image.width; t.height=image.height;
            }
            if (!t.identity || t.identity!=image.identity) {
                context->UpdateSubresource(t.texture.Get(),0,nullptr,image.pixels,image.width*4,0);
                t.identity=image.identity;
            }
        }
        for(size_t i=0;i<6;++i) if(!used[i]) textures[i]=Texture{};
        for(size_t i=0;i<commandCount;++i) {
            const auto& draw=commands[i];
            if(draw.mode>3 || draw.first>vertexCount || draw.count>vertexCount-draw.first || draw.count%3 ||
               ((draw.mode==0 || draw.mode==3) && (draw.slot>=6 || !used[draw.slot])))
                throw std::runtime_error("Invalid GPU overlay draw");
        }
        draws.assign(commands, commands+commandCount);
        if(vertexCount && (cached.size()!=vertexCount || std::memcmp(cached.data(),vertices,vertexCount*sizeof(FrameGpuVertex)))) {
            if(capacity<vertexCount) {
                geometry.Reset(); capacity=static_cast<uint32_t>(vertexCount);
                D3D11_BUFFER_DESC desc{}; desc.ByteWidth=capacity*sizeof(FrameGpuVertex); desc.Usage=D3D11_USAGE_DYNAMIC;
                desc.BindFlags=D3D11_BIND_VERTEX_BUFFER; desc.CPUAccessFlags=D3D11_CPU_ACCESS_WRITE;
                winrt::check_hresult(device->CreateBuffer(&desc,nullptr,&geometry));
            }
            D3D11_MAPPED_SUBRESOURCE mapped{};
            winrt::check_hresult(context->Map(geometry.Get(),0,D3D11_MAP_WRITE_DISCARD,0,&mapped));
            std::memcpy(mapped.pData,vertices,vertexCount*sizeof(FrameGpuVertex)); context->Unmap(geometry.Get(),0);
            cached.assign(vertices,vertices+vertexCount);
        } else if(!vertexCount) cached.clear();
    }
    // Capture has already bound a fresh BGRA output RTV, viewport and rasterizer.
    void render(uint32_t width, uint32_t height) {
        if(draws.empty()) return;
        UINT stride=sizeof(FrameGpuVertex), offset=0;
        context->IASetInputLayout(layout.Get()); context->IASetVertexBuffers(0,1,geometry.GetAddressOf(),&stride,&offset);
        context->VSSetShader(vertex.Get(),nullptr,0); context->PSSetShader(pixel.Get(),nullptr,0);
        context->VSSetConstantBuffers(0,1,constants.GetAddressOf()); context->PSSetConstantBuffers(0,1,constants.GetAddressOf());
        context->OMSetBlendState(blend.Get(),nullptr,0xffffffff); context->PSSetSamplers(0,1,sampler.GetAddressOf());
        for(const auto& draw:draws) {
            float settings[8]={1.0f/width,1.0f/height,static_cast<float>(draw.mode),0,draw.parameters[0],draw.parameters[1],draw.parameters[2],draw.parameters[3]};
            context->UpdateSubresource(constants.Get(),0,nullptr,settings,0,0);
            ID3D11ShaderResourceView* source=(draw.mode==0 || draw.mode==3) ? textures[draw.slot].view.Get() : nullptr;
            context->PSSetShaderResources(0,1,&source); context->Draw(draw.count,draw.first);
        }
        ID3D11ShaderResourceView* empty=nullptr; context->PSSetShaderResources(0,1,&empty);
        context->IASetInputLayout(nullptr); ID3D11Buffer* noBuffer=nullptr; context->IASetVertexBuffers(0,1,&noBuffer,&stride,&offset);
    }
};
