#include "flutter_window.h"
#include "recorder_host.h"

#include <optional>
#include <string>

#include "flutter/generated_plugin_registrant.h"

FlutterWindow::FlutterWindow(const flutter::DartProject& project, bool smoke_test, bool creator_fixture)
    : project_(project), smoke_test_(smoke_test), creator_fixture_(creator_fixture) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());
  RegisterRecorderHost(flutter_controller_->engine()->messenger(), GetHandle(), creator_fixture_);
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  flutter_controller_->engine()->SetNextFrameCallback([&]() {
    // Automated startup check: exit only after Flutter renders its first frame.
    if (smoke_test_) {
      PostMessage(GetHandle(), WM_CLOSE, 0, 0);
      return;
    }
    if (creator_fixture_) ShowWindow(GetHandle(), SW_SHOWNOACTIVATE);
    else this->Show();
  });

  // Flutter can complete the first frame before the "show window" callback is
  // registered. The following call ensures a frame is pending to ensure the
  // window is shown. It is a no-op if the first frame hasn't completed yet.
  flutter_controller_->ForceRedraw();

  return true;
}

void FlutterWindow::OnDestroy() {
  DisposeRecorderHost(GetHandle());
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  if (creator_fixture_ && message == WM_DESTROY) {
    const int exit_code = GetPropW(hwnd, L"Frame.CreatorFixtureResult") == reinterpret_cast<HANDLE>(1) ? 0 : 1;
    SetQuitOnClose(false);
    const auto result = Win32Window::MessageHandler(hwnd, message, wparam, lparam);
    PostQuitMessage(exit_code);
    return result;
  }
  if (HandleRecorderHostMessage(hwnd, message, wparam, lparam)) return 0;
  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case WM_GETMINMAXINFO: {
      auto* limits = reinterpret_cast<MINMAXINFO*>(lparam);
      limits->ptMinTrackSize.x = 960;
      limits->ptMinTrackSize.y = 640;
      return 0;
    }
    case WM_CLOSE:
    case WM_TIMER: {
      if (message == WM_TIMER && (!closing_after_recording_ || wparam != 41)) break;
      auto module = GetModuleHandleW(L"recorder_core.dll");
      if (!module) break;
      using Status = char* (*)();
      using Command = char* (*)(const char*);
      using Release = void (*)(char*);
      auto status = reinterpret_cast<Status>(GetProcAddress(module, "recorder_status_json"));
      auto command = reinterpret_cast<Command>(GetProcAddress(module, "recorder_command_json"));
      auto release = reinterpret_cast<Release>(GetProcAddress(module, "recorder_string_free"));
      if (!status || !command || !release) break;
      auto text = status();
      std::string value = text ? text : "";
      release(text);
      bool active = value.find("\"recording\"") != std::string::npos ||
                    value.find("\"preparing\"") != std::string::npos ||
                    value.find("\"paused\"") != std::string::npos ||
                    value.find("\"stopping\"") != std::string::npos;
      if (message == WM_TIMER) {
        if (!active) { KillTimer(hwnd, 41); closing_after_recording_ = false; PostMessage(hwnd, WM_CLOSE, 0, 0); }
        return 0;
      }
      if (active) {
        if (closing_after_recording_) return 0;
        if (MessageBoxW(hwnd, L"Stop and save your recording before closing?", L"Recording in progress", MB_YESNO | MB_ICONQUESTION) != IDYES) return 0;
        release(command("{\"action\":\"stop\"}"));
        closing_after_recording_ = true;
        SetTimer(hwnd, 41, 200, nullptr);
        return 0;
      }
      break;
    }
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}
