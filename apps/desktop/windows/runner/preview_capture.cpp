#include "preview_capture.h"
#include <windows.h>
#include <objidl.h>
#include <gdiplus.h>
#include <wrl/client.h>
#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <string>
#include <dwmapi.h>

namespace {
int64_t Number(const flutter::EncodableMap& args, const char* key, int64_t fallback = 0) {
  auto it = args.find(flutter::EncodableValue(key));
  if (it == args.end()) return fallback;
  if (auto value = std::get_if<int32_t>(&it->second)) return *value;
  if (auto value = std::get_if<int64_t>(&it->second)) return *value;
  throw std::runtime_error("Invalid preview coordinates");
}
struct Imaging {
  ULONG_PTR token = 0;
  Imaging() {
    Gdiplus::GdiplusStartupInput input;
    if (Gdiplus::GdiplusStartup(&token, &input, nullptr) != Gdiplus::Ok)
      throw std::runtime_error("Preview imaging is unavailable");
  }
  ~Imaging() { Gdiplus::GdiplusShutdown(token); }
};
// Every GDI allocation is released even if encoding or capture fails.
struct Surface {
  HWND window = nullptr;
  HDC source = nullptr, target = nullptr;
  HBITMAP bitmap = nullptr;
  HGDIOBJ previous = nullptr;
  ~Surface() {
    if (previous) SelectObject(target, previous);
    if (bitmap) DeleteObject(bitmap);
    if (target) DeleteDC(target);
    if (source) ReleaseDC(window, source);
  }
};
}

std::vector<uint8_t> CapturePreview(const flutter::EncodableMap& args) {
  static Imaging imaging;
  auto kind = args.find(flutter::EncodableValue("kind"));
  if (kind == args.end() || !std::holds_alternative<std::string>(kind->second))
    throw std::runtime_error("Choose a source to preview");
  const auto& name = std::get<std::string>(kind->second);
  const auto id = static_cast<uintptr_t>(Number(args, "id"));
  Surface surface;
  RECT bounds{};
  int origin_x = 0, origin_y = 0;
  if (name == "display") {
    MONITORINFO info{};
    info.cbSize = sizeof(info);
    if (!GetMonitorInfoW(reinterpret_cast<HMONITOR>(id), &info))
      throw std::runtime_error("Display disconnected. Refresh your sources.");
    bounds = info.rcMonitor;
    origin_x = bounds.left;
    origin_y = bounds.top;
  } else if (name == "window") {
    auto window = reinterpret_cast<HWND>(id);
    if (!IsWindow(window) || IsIconic(window) || !GetWindowRect(window, &bounds))
      throw std::runtime_error("Open the selected window to see its preview.");
    DwmGetWindowAttribute(window, DWMWA_EXTENDED_FRAME_BOUNDS, &bounds, sizeof(bounds));
    origin_x = bounds.left;
    origin_y = bounds.top;
  } else {
    throw std::runtime_error("Unsupported preview source");
  }
  const int64_t full_width = bounds.right - bounds.left;
  const int64_t full_height = bounds.bottom - bounds.top;
  const auto left = Number(args, "left"), top = Number(args, "top");
  const auto width = Number(args, "width", full_width);
  const auto height = Number(args, "height", full_height);
  if (left < 0 || top < 0 || width < 2 || height < 2 ||
      width > full_width || height > full_height ||
      left > full_width - width || top > full_height - height)
    throw std::runtime_error("Enter a region inside the selected display.");
  const auto max_width=Number(args,"max_width",480), max_height=Number(args,"max_height",270);
  if (max_width<1 || max_width>7680 || max_height<1 || max_height>4320)
    throw std::runtime_error("Invalid thumbnail dimensions");
  const double ratio = std::min({1.0,static_cast<double>(max_width) / static_cast<double>(width),
      static_cast<double>(max_height) / static_cast<double>(height)});
  const int thumbnail_width = std::max(1, static_cast<int>(std::round(width * ratio)));
  const int thumbnail_height = std::max(1, static_cast<int>(std::round(height * ratio)));
  if (name == "window" && max_width <= 480 && max_height <= 270) {
    // Flutter loads the core before previews. Never load a DLL from PATH.
    auto module = GetModuleHandleW(L"recorder_core.dll");
    using Request = char* (*)(const char*);
    using Read = uint8_t* (*)(size_t*);
    using Free = void (*)(uint8_t*, size_t);
    using FreeString = void (*)(char*);
    auto request = reinterpret_cast<Request>(GetProcAddress(module, "recorder_preview_request_json"));
    auto read = reinterpret_cast<Read>(GetProcAddress(module, "recorder_preview_png"));
    auto release = reinterpret_cast<Free>(GetProcAddress(module, "recorder_preview_free"));
    auto release_string = reinterpret_cast<FreeString>(GetProcAddress(module, "recorder_string_free"));
    if (!request || !read || !release || !release_string)
      throw std::runtime_error("Update the native core to enable window preview");
    auto json = std::string("{\"id\":") + std::to_string(id) + ",\"width\":" +
        std::to_string(thumbnail_width) + ",\"height\":" + std::to_string(thumbnail_height) + "}";
    auto response = request(json.c_str());
    if (!response) throw std::runtime_error("Preview allocation failed");
    const std::string status(response);
    release_string(response);
    if (status.find("\"ok\":true") == std::string::npos)
      throw std::runtime_error("Window preview unavailable; restore the window");
    size_t length = 0;
    auto png = read(&length);
    if (!png) return {}; // async capture pending; keep the previous thumbnail
    struct Owned { uint8_t* data; size_t size; Free release; ~Owned() { release(data,size); } } owned{png,length,release};
    if (length > 2 * 1024 * 1024) throw std::runtime_error("Invalid thumbnail size");
    return std::vector<uint8_t>(png,png+length);
  }
  // Full-size screenshots use the visible desktop compositor and full bounds.
  surface.source = GetDC(surface.window);
  if (!surface.source) throw std::runtime_error("Preview source unavailable");
  surface.target = CreateCompatibleDC(surface.source);
  BITMAPINFO info{};
  info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
  info.bmiHeader.biWidth = thumbnail_width;
  info.bmiHeader.biHeight = -thumbnail_height;
  info.bmiHeader.biPlanes = 1;
  info.bmiHeader.biBitCount = 32;
  info.bmiHeader.biCompression = BI_RGB;
  void* pixels = nullptr;
  surface.bitmap = CreateDIBSection(surface.source, &info, DIB_RGB_COLORS, &pixels, nullptr, 0);
  if (!surface.target || !surface.bitmap || !pixels)
    throw std::runtime_error("Preview allocation failed");
  surface.previous = SelectObject(surface.target, surface.bitmap);
  SetStretchBltMode(surface.target, HALFTONE);
  SetBrushOrgEx(surface.target, 0, 0, nullptr);
  if (!StretchBlt(surface.target, 0, 0, thumbnail_width, thumbnail_height,
      surface.source, origin_x + static_cast<int>(left), origin_y + static_cast<int>(top),
      static_cast<int>(width), static_cast<int>(height), SRCCOPY | CAPTUREBLT))
    throw std::runtime_error("This source cannot show a preview.");
  GdiFlush();
  Gdiplus::Bitmap image(thumbnail_width, thumbnail_height, thumbnail_width * 4,
      PixelFormat32bppRGB, static_cast<BYTE*>(pixels));
  // The built-in PNG encoder CLSID is stable across supported Windows versions.
  const CLSID png = {0x557cf406, 0x1a04, 0x11d3, {0x9a,0x73,0x00,0x00,0xf8,0x1e,0xf3,0x2e}};
  Microsoft::WRL::ComPtr<IStream> stream;
  if (FAILED(CreateStreamOnHGlobal(nullptr, TRUE, &stream)) ||
      image.Save(stream.Get(), &png) != Gdiplus::Ok)
    throw std::runtime_error("Preview encoding failed");
  STATSTG stat{};
  if (FAILED(stream->Stat(&stat, STATFLAG_NONAME)) || stat.cbSize.QuadPart > 128 * 1024 * 1024)
    throw std::runtime_error("Invalid preview image");
  LARGE_INTEGER beginning{};
  stream->Seek(beginning, STREAM_SEEK_SET, nullptr);
  std::vector<uint8_t> bytes(static_cast<size_t>(stat.cbSize.QuadPart));
  ULONG count = 0;
  if (FAILED(stream->Read(bytes.data(), static_cast<ULONG>(bytes.size()), &count)) || count != bytes.size())
    throw std::runtime_error("Preview read failed");
  return bytes;
}
