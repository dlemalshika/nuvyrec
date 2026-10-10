#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>
#include <algorithm>

#include "flutter_window.h"
#include "preview_capture.h"
#include "utils.h"

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command) {
  // Attach to console when present (e.g., 'flutter run') or create a
  // new console when running with a debugger.
  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }

  // Initialize COM, so that it is available for use in the library and/or
  // plugins.
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  flutter::DartProject project(L"data");

  std::vector<std::string> command_line_arguments =
      GetCommandLineArguments();

  const bool smoke_test = std::find(command_line_arguments.begin(),
      command_line_arguments.end(), "--smoke-test") != command_line_arguments.end();
  const bool preview_test = std::find(command_line_arguments.begin(),
      command_line_arguments.end(), "--preview-smoke-test") != command_line_arguments.end();
  const bool creator_test = std::find(command_line_arguments.begin(),
      command_line_arguments.end(), "--creator-smoke-test") != command_line_arguments.end();
  if (preview_test) {
    try {
      auto monitor = MonitorFromPoint(POINT{0, 0}, MONITOR_DEFAULTTOPRIMARY);
      flutter::EncodableMap args;
      args[flutter::EncodableValue("kind")] = flutter::EncodableValue("display");
      args[flutter::EncodableValue("id")] = flutter::EncodableValue(
          static_cast<int64_t>(reinterpret_cast<uintptr_t>(monitor)));
      auto png = CapturePreview(args);
      if (png.size() < 8 || png[0] != 137 || png[1] != 'P' || png[2] != 'N' || png[3] != 'G')
        return EXIT_FAILURE;
    } catch (...) { return EXIT_FAILURE; }
  }
  project.set_dart_entrypoint_arguments(std::move(command_line_arguments));

  FlutterWindow window(project, smoke_test || preview_test, creator_test);
  Win32Window::Point origin(10, 10);
  Win32Window::Size size(1280, 720);
  if (!window.Create(L"Frame — Screen Recorder", origin, size)) {
    return EXIT_FAILURE;
  }
  window.SetQuitOnClose(true);

  ::MSG msg;
  while (::GetMessage(&msg, nullptr, 0, 0)) {
    ::TranslateMessage(&msg);
    ::DispatchMessage(&msg);
  }

  ::CoUninitialize();
  return static_cast<int>(msg.wParam);
}
