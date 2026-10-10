#include "region_selector.h"
#include <algorithm>

namespace {
struct Selection {
  POINT start{}, end{};
  bool dragging = false, accepted = false;
};
LRESULT CALLBACK RegionProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
  auto* state = reinterpret_cast<Selection*>(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
  if (message == WM_NCCREATE) {
    state = static_cast<Selection*>(reinterpret_cast<CREATESTRUCT*>(lparam)->lpCreateParams);
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(state));
  }
  if (!state) return DefWindowProcW(hwnd, message, wparam, lparam);
  switch (message) {
    case WM_KEYDOWN:
      if (wparam == VK_ESCAPE) DestroyWindow(hwnd);
      return 0;
    case WM_RBUTTONDOWN: DestroyWindow(hwnd); return 0;
    case WM_LBUTTONDOWN:
      GetCursorPos(&state->start); state->end = state->start;
      state->dragging = true; SetCapture(hwnd); return 0;
    case WM_MOUSEMOVE:
      if (state->dragging) { GetCursorPos(&state->end); InvalidateRect(hwnd, nullptr, FALSE); }
      return 0;
    case WM_LBUTTONUP:
      if (state->dragging) {
        GetCursorPos(&state->end); ReleaseCapture();
        state->accepted = abs(state->end.x-state->start.x) >= 2 && abs(state->end.y-state->start.y) >= 2;
        DestroyWindow(hwnd);
      }
      return 0;
    case WM_PAINT: {
      PAINTSTRUCT paint{}; auto dc = BeginPaint(hwnd, &paint);
      RECT client{}; GetClientRect(hwnd, &client);
      auto background = CreateSolidBrush(RGB(10, 30, 35)); FillRect(dc, &client, background); DeleteObject(background);
      SetBkMode(dc, TRANSPARENT); SetTextColor(dc, RGB(255,255,255));
      RECT hint{24,24,client.right-24,80};
      DrawTextW(dc, L"Drag to select your capture area. Escape to cancel.", -1, &hint, DT_LEFT | DT_SINGLELINE);
      if (state->dragging) {
        auto start = state->start, end = state->end; ScreenToClient(hwnd, &start); ScreenToClient(hwnd, &end);
        auto pen = CreatePen(PS_SOLID, 3, RGB(120,255,210));
        auto old_pen = SelectObject(dc, pen); auto old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
        Rectangle(dc, std::min(start.x,end.x),std::min(start.y,end.y),std::max(start.x,end.x),std::max(start.y,end.y));
        SelectObject(dc, old_brush); SelectObject(dc, old_pen); DeleteObject(pen);
      }
      EndPaint(hwnd, &paint); return 0;
    }
  }
  return DefWindowProcW(hwnd, message, wparam, lparam);
}
}
std::optional<RECT> SelectCaptureRegion(HWND owner, HMONITOR monitor) {
  MONITORINFO info{}; info.cbSize = sizeof(info);
  if (!GetMonitorInfoW(monitor, &info)) return std::nullopt;
  WNDCLASSW cls{}; cls.lpfnWndProc = RegionProc; cls.hInstance = GetModuleHandleW(nullptr);
  cls.lpszClassName = L"FrameRegionSelector"; cls.hCursor = LoadCursorW(nullptr, IDC_CROSS);
  RegisterClassW(&cls);
  Selection state;
  auto bounds = info.rcMonitor;
  auto window = CreateWindowExW(WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
      cls.lpszClassName, L"Select capture region", WS_POPUP,
      bounds.left,bounds.top,bounds.right-bounds.left,bounds.bottom-bounds.top,
      owner,nullptr,cls.hInstance,&state);
  if (!window) return std::nullopt;
  SetLayeredWindowAttributes(window,0,110,LWA_ALPHA);
  ShowWindow(window, SW_SHOW); SetForegroundWindow(window); SetFocus(window);
  MSG message{};
  while (IsWindow(window)) {
    auto next = GetMessageW(&message,nullptr,0,0);
    if (next <= 0) { DestroyWindow(window); if (next == 0) PostQuitMessage(static_cast<int>(message.wParam)); break; }
    TranslateMessage(&message); DispatchMessageW(&message);
  }
  if (!state.accepted) return std::nullopt;
  RECT selected{std::min(state.start.x,state.end.x),std::min(state.start.y,state.end.y),
      std::max(state.start.x,state.end.x),std::max(state.start.y,state.end.y)};
  selected.left = std::clamp(selected.left,bounds.left,bounds.right);
  selected.right = std::clamp(selected.right,bounds.left,bounds.right);
  selected.top = std::clamp(selected.top,bounds.top,bounds.bottom);
  selected.bottom = std::clamp(selected.bottom,bounds.top,bounds.bottom);
  if (selected.right-selected.left < 2 || selected.bottom-selected.top < 2) return std::nullopt;
  selected.left -= bounds.left; selected.right -= bounds.left;
  selected.top -= bounds.top; selected.bottom -= bounds.top;
  return selected;
}
