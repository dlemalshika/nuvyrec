#include "recorder_host.h"
#include "preview_capture.h"
#include "region_selector.h"
#include <flutter/method_channel.h>
#include <flutter/standard_method_codec.h>
#include <shobjidl.h>
#include <shlobj.h>
#include <wrl/client.h>
#include <memory>
#include <string>
#include <filesystem>
#include <algorithm>
#include <mmdeviceapi.h>
#include <functiondiscoverykeys_devpkey.h>

namespace {
std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel;
constexpr UINT kTrayMessage = WM_APP + 23;
bool tray_added = false;
bool media_busy = false;
std::vector<int> registered_hotkeys;
void SendAction(const char* action) {
  if (channel) channel->InvokeMethod("hostAction", std::make_unique<flutter::EncodableValue>(action));
}
std::wstring Wide(const std::string& text) {
  int size = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), nullptr, 0);
  std::wstring result(size, L'\0');
  if (size > 0) MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), result.data(), size);
  return result;
}
std::string Utf8(const wchar_t* text) {
  int size = WideCharToMultiByte(CP_UTF8, 0, text, -1, nullptr, 0, nullptr, nullptr);
  std::string result(size, '\0');
  WideCharToMultiByte(CP_UTF8, 0, text, -1, result.data(), size, nullptr, nullptr);
  if (!result.empty()) result.pop_back();
  return result;
}
std::string Argument(const flutter::MethodCall<flutter::EncodableValue>& call, const char* key) {
  if (!call.arguments()) return {};
  const auto* map = std::get_if<flutter::EncodableMap>(call.arguments());
  if (!map) return {};
  auto found = map->find(flutter::EncodableValue(key));
  if (found == map->end()) return {};
  const auto* value = std::get_if<std::string>(&found->second);
  return value ? *value : std::string();
}
}
void RegisterRecorderHost(flutter::BinaryMessenger* messenger, HWND owner, bool creator_fixture) {
  channel = std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(messenger, "org.screenrecorder/host", &flutter::StandardMethodCodec::GetInstance());
  channel->SetMethodCallHandler([owner, creator_fixture](const auto& call, auto result) {
    if (call.method_name() == "creatorFixtureHandle") {
      if (!creator_fixture) { result->Error("fixture", "Fixture mode is not active"); return; }
      result->Success(flutter::EncodableValue(static_cast<int64_t>(reinterpret_cast<uintptr_t>(owner))));
    } else if (call.method_name() == "finishCreatorFixture") {
      if (!creator_fixture) { result->Error("fixture", "Fixture mode is not active"); return; }
      const auto* args = call.arguments() ? std::get_if<flutter::EncodableMap>(call.arguments()) : nullptr;
      bool ok = false;
      if (args) {
        auto it = args->find(flutter::EncodableValue("ok"));
        if (it != args->end()) if (auto value = std::get_if<bool>(&it->second)) ok = *value;
      }
      result->Success();
      // Tear down the window/Flutter controller before leaving its COM
      // apartment. Posting WM_QUIT here bypasses OnDestroy and leaves a live
      // Flutter HWND during controller destruction.
      SetPropW(owner, L"Frame.CreatorFixtureResult", reinterpret_cast<HANDLE>(static_cast<uintptr_t>(ok ? 1 : 2)));
      PostMessageW(owner, WM_CLOSE, 0, 0);
    } else if (call.method_name() == "setMediaBusy") {
      const auto* value = call.arguments() ? std::get_if<bool>(call.arguments()) : nullptr;
      media_busy = value && *value;
      result->Success();
    } else if (call.method_name() == "chooseRecording" || call.method_name() == "chooseOverlayImage") {
      Microsoft::WRL::ComPtr<IFileOpenDialog> dialog;
      if (FAILED(CoCreateInstance(CLSID_FileOpenDialog,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&dialog)))) {
        result->Error("dialog","Could not open recording picker"); return;
      }
      COMDLG_FILTERSPEC filter{L"Recordings",L"*.mkv;*.mp4;*.avi;*.mov;*.gif"};
      const bool overlay = call.method_name() == "chooseOverlayImage";
      if (overlay) filter = {L"Overlay images",L"*.png;*.jpg;*.jpeg;*.webp"};
      dialog->SetFileTypes(1,&filter);
      dialog->SetTitle(overlay ? L"Choose a logo or watermark image" : L"Import a recording for playback or export");
      const auto hr=dialog->Show(owner);
      if (hr==HRESULT_FROM_WIN32(ERROR_CANCELLED)) { result->Success(); return; }
      Microsoft::WRL::ComPtr<IShellItem> item; PWSTR path=nullptr;
      if (FAILED(hr) || FAILED(dialog->GetResult(&item)) || FAILED(item->GetDisplayName(SIGDN_FILESYSPATH,&path))) {
        result->Error("dialog","Could not read the recording path"); return;
      }
      auto selected=Utf8(path); CoTaskMemFree(path); result->Success(flutter::EncodableValue(selected));
    } else if (call.method_name() == "selectRegion") {
      const auto* args = call.arguments() ? std::get_if<flutter::EncodableMap>(call.arguments()) : nullptr;
      if (!args) { result->Error("region", "Choose a display first"); return; }
      auto found = args->find(flutter::EncodableValue("id"));
      int64_t id = 0;
      if (found != args->end()) {
        if (auto number = std::get_if<int64_t>(&found->second)) id = *number;
        else if (auto small_number = std::get_if<int32_t>(&found->second)) id = *small_number;
      }
      auto area = SelectCaptureRegion(owner, reinterpret_cast<HMONITOR>(static_cast<uintptr_t>(id)));
      if (!area) { result->Success(); return; }
      flutter::EncodableList values{flutter::EncodableValue(static_cast<int32_t>(area->left)),
        flutter::EncodableValue(static_cast<int32_t>(area->top)),
        flutter::EncodableValue(static_cast<int32_t>(area->right-area->left)),
        flutter::EncodableValue(static_cast<int32_t>(area->bottom-area->top))};
      result->Success(flutter::EncodableValue(values));
    } else if (call.method_name() == "audioDevices") {
      Microsoft::WRL::ComPtr<IMMDeviceEnumerator> enumerator;
      if (FAILED(CoCreateInstance(__uuidof(MMDeviceEnumerator),nullptr,CLSCTX_ALL,IID_PPV_ARGS(&enumerator)))) {
        result->Error("audio","Audio devices unavailable"); return;
      }
      flutter::EncodableList devices;
      for (auto flow : {eRender,eCapture}) {
        Microsoft::WRL::ComPtr<IMMDeviceCollection> collection;
        if (FAILED(enumerator->EnumAudioEndpoints(flow,DEVICE_STATE_ACTIVE,&collection))) continue;
        UINT count=0; collection->GetCount(&count);
        for (UINT i=0;i<count;++i) {
          Microsoft::WRL::ComPtr<IMMDevice> device;
          Microsoft::WRL::ComPtr<IPropertyStore> properties;
          LPWSTR id=nullptr; PROPVARIANT name{};
          if (FAILED(collection->Item(i,&device)) || FAILED(device->GetId(&id))) continue;
          std::string title="Audio device";
          if (SUCCEEDED(device->OpenPropertyStore(STGM_READ,&properties)) &&
              SUCCEEDED(properties->GetValue(PKEY_Device_FriendlyName,&name)) && name.vt==VT_LPWSTR && name.pwszVal)
            title=Utf8(name.pwszVal);
          devices.emplace_back(flutter::EncodableMap{
            {flutter::EncodableValue("id"),flutter::EncodableValue(Utf8(id))},
            {flutter::EncodableValue("name"),flutter::EncodableValue(title)},
            {flutter::EncodableValue("kind"),flutter::EncodableValue(flow==eRender?"system":"microphone")}});
          PropVariantClear(&name); CoTaskMemFree(id);
        }
      }
      result->Success(flutter::EncodableValue(devices));
    } else if (call.method_name() == "saveScreenshot") {
      const auto* args=call.arguments()?std::get_if<flutter::EncodableMap>(call.arguments()):nullptr;
      if (!args) { result->Error("screenshot","Choose a source"); return; }
      try {
        auto png=CapturePreview(*args);
        auto path=Wide(Argument(call,"path"));
        auto file=CreateFileW(path.c_str(),GENERIC_WRITE,0,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr);
        if (file==INVALID_HANDLE_VALUE) { result->Error("screenshot","Choose a new PNG filename"); return; }
        DWORD written=0; auto success=WriteFile(file,png.data(),static_cast<DWORD>(png.size()),&written,nullptr);
        CloseHandle(file);
        if (!success || written!=png.size()) result->Error("screenshot","Could not save the complete screenshot"); else result->Success();
      } catch (const std::exception& error) { result->Error("screenshot",error.what()); }
    } else if (call.method_name() == "enableControls") {
      flutter::EncodableList conflicts;
      for (int i = 0; i < 3; ++i) {
        if (std::find(registered_hotkeys.begin(),registered_hotkeys.end(),601+i) != registered_hotkeys.end()) continue;
        if (RegisterHotKey(owner,601+i,MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,VK_F9+i)) registered_hotkeys.push_back(601+i);
        else conflicts.emplace_back(std::string("Ctrl+Shift+F") + std::to_string(9+i));
      }
      if (!tray_added) {
        NOTIFYICONDATAW icon{}; icon.cbSize = sizeof(icon); icon.hWnd = owner; icon.uID = 1;
        icon.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP; icon.uCallbackMessage = kTrayMessage;
        icon.hIcon = LoadIconW(GetModuleHandleW(nullptr), MAKEINTRESOURCEW(101));
        if (!icon.hIcon) icon.hIcon = LoadIconW(nullptr,IDI_APPLICATION);
        wcscpy_s(icon.szTip,L"Frame screen recorder");
        tray_added = Shell_NotifyIconW(NIM_ADD,&icon) != FALSE;
      }
      result->Success(flutter::EncodableValue(conflicts));
    } else if (call.method_name() == "hideToTray") {
      if (!tray_added) result->Error("tray", "Tray icon unavailable");
      else { ShowWindow(owner, SW_HIDE); result->Success(); }
    } else if (call.method_name() == "openRecording") {
      auto path = Wide(Argument(call,"path"));
      auto opened = reinterpret_cast<INT_PTR>(ShellExecuteW(owner,L"open",path.c_str(),nullptr,nullptr,SW_SHOWNORMAL));
      if (opened <= 32) result->Error("file","Could not open recording"); else result->Success();
    } else if (call.method_name() == "diskSpace") {
      auto path = std::filesystem::path(Wide(Argument(call,"path"))).parent_path();
      ULARGE_INTEGER available{};
      if (!path.empty() && GetDiskFreeSpaceExW(path.c_str(),&available,nullptr,nullptr))
        result->Success(flutter::EncodableValue(static_cast<int64_t>(available.QuadPart)));
      else result->Success();
    } else if (call.method_name() == "getPreferences") {
      DWORD length = 0;
      auto status = RegGetValueW(HKEY_CURRENT_USER,L"Software\\FrameScreenRecorder",L"Preferences",RRF_RT_REG_SZ,nullptr,nullptr,&length);
      if (status != ERROR_SUCCESS || length > 1024*1024 || length < sizeof(wchar_t)) { result->Success(flutter::EncodableValue("{}")); return; }
      std::vector<wchar_t> value(length/sizeof(wchar_t)+1,0);
      status = RegGetValueW(HKEY_CURRENT_USER,L"Software\\FrameScreenRecorder",L"Preferences",RRF_RT_REG_SZ,nullptr,value.data(),&length);
      result->Success(flutter::EncodableValue(status == ERROR_SUCCESS ? Utf8(value.data()) : "{}"));
    } else if (call.method_name() == "setPreferences") {
      const auto value = Wide(Argument(call,"json"));
      if (value.size() > 256*1024) { result->Error("preferences","Preferences too large"); return; }
      HKEY key = nullptr;
      auto status = RegCreateKeyExW(HKEY_CURRENT_USER,L"Software\\FrameScreenRecorder",0,nullptr,0,KEY_SET_VALUE,nullptr,&key,nullptr);
      if (status == ERROR_SUCCESS) {
        status = RegSetValueExW(key,L"Preferences",0,REG_SZ,reinterpret_cast<const BYTE*>(value.c_str()),static_cast<DWORD>((value.size()+1)*sizeof(wchar_t)));
        RegCloseKey(key);
      }
      if (status == ERROR_SUCCESS) result->Success(); else result->Error("preferences","Could not save settings");
    } else if (call.method_name() == "getTheme") {
      wchar_t value[16]{};
      DWORD length = sizeof(value);
      auto status = RegGetValueW(HKEY_CURRENT_USER, L"Software\\FrameScreenRecorder",
          L"Theme", RRF_RT_REG_SZ, nullptr, value, &length);
      result->Success(flutter::EncodableValue(status == ERROR_SUCCESS &&
          std::wstring(value) == L"light" ? "light" : "dark"));
    } else if (call.method_name() == "setTheme") {
      const auto theme = Argument(call, "theme");
      if (theme != "dark" && theme != "light") { result->Error("theme", "Invalid theme"); return; }
      HKEY key = nullptr;
      auto status = RegCreateKeyExW(HKEY_CURRENT_USER, L"Software\\FrameScreenRecorder",
          0, nullptr, 0, KEY_SET_VALUE, nullptr, &key, nullptr);
      if (status == ERROR_SUCCESS) {
        const auto value = Wide(theme);
        status = RegSetValueExW(key, L"Theme", 0, REG_SZ,
            reinterpret_cast<const BYTE*>(value.c_str()),
            static_cast<DWORD>((value.size() + 1) * sizeof(wchar_t)));
        RegCloseKey(key);
      }
      if (status == ERROR_SUCCESS) result->Success();
      else result->Error("theme", "Could not save theme preference");
    } else if (call.method_name() == "capturePreview") {
      const auto* args = call.arguments() ? std::get_if<flutter::EncodableMap>(call.arguments()) : nullptr;
      if (!args) { result->Error("preview", "Choose a preview source"); return; }
      try {
        auto png = CapturePreview(*args);
        if (png.empty()) result->Success();
        else result->Success(flutter::EncodableValue(std::move(png)));
      }
      catch (const std::exception& error) { result->Error("preview", error.what()); }
    } else if (call.method_name() == "chooseOutput") {
      auto format = Argument(call, "format");
      if (format != "mp4" && format != "mkv" && format != "avi" && format != "gif" && format != "png") { result->Error("format", "Unsupported output format"); return; }
      Microsoft::WRL::ComPtr<IFileSaveDialog> dialog;
      HRESULT hr = CoCreateInstance(CLSID_FileSaveDialog, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&dialog));
      if (FAILED(hr)) { result->Error("dialog", "Could not open save dialog"); return; }
      auto extension = Wide(format);
      auto pattern = L"*." + extension;
      COMDLG_FILTERSPEC filter{ L"Video recording", pattern.c_str() };
      dialog->SetFileTypes(1, &filter);
      dialog->SetDefaultExtension(extension.c_str());
      dialog->SetTitle(L"Save a new recording");
      SYSTEMTIME time{}; GetLocalTime(&time);
      wchar_t name[100]{};
      swprintf_s(name, L"Recording-%04u%02u%02u-%02u%02u%02u", time.wYear,time.wMonth,time.wDay,time.wHour,time.wMinute,time.wSecond);
      dialog->SetFileName(name);
      hr = dialog->Show(owner);
      if (hr == HRESULT_FROM_WIN32(ERROR_CANCELLED)) { result->Success(); return; }
      Microsoft::WRL::ComPtr<IShellItem> item;
      PWSTR path = nullptr;
      if (FAILED(hr) || FAILED(dialog->GetResult(&item)) || FAILED(item->GetDisplayName(SIGDN_FILESYSPATH, &path))) { result->Error("dialog", "Could not read selected file"); return; }
      auto selected = Utf8(path); CoTaskMemFree(path);
      result->Success(flutter::EncodableValue(selected));
    } else if (call.method_name() == "revealOutput") {
      auto path = Wide(Argument(call, "path"));
      PIDLIST_ABSOLUTE item = nullptr;
      HRESULT hr = SHParseDisplayName(path.c_str(), nullptr, &item, 0, nullptr);
      if (SUCCEEDED(hr)) { hr = SHOpenFolderAndSelectItems(item, 0, nullptr, 0); CoTaskMemFree(item); }
      if (FAILED(hr)) result->Error("file", "Saved file could not be opened"); else result->Success();
    } else { result->NotImplemented(); }
  });
}

bool HandleRecorderHostMessage(HWND owner, UINT message, WPARAM wparam, LPARAM lparam) {
  if (message == WM_CLOSE && media_busy) {
    MessageBoxW(owner,L"An export is running. Finish it or cancel it in the recording library before closing.",L"Export in progress",MB_OK | MB_ICONINFORMATION);
    return true;
  }
  if (message == WM_HOTKEY && wparam >= 601 && wparam <= 603) {
    SendAction(wparam == 601 ? "toggle" : wparam == 602 ? "pause" : "replay"); return true;
  }
  if (message == kTrayMessage) {
    if (lparam == WM_LBUTTONDBLCLK) { ShowWindow(owner,SW_RESTORE); SetForegroundWindow(owner); }
    if (lparam == WM_RBUTTONUP) {
      auto menu = CreatePopupMenu();
      AppendMenuW(menu,MF_STRING,801,L"Show Frame");
      AppendMenuW(menu,MF_STRING,802,L"Start / stop recording\tCtrl+Shift+F9");
      AppendMenuW(menu,MF_STRING,803,L"Pause / resume\tCtrl+Shift+F10");
      AppendMenuW(menu,MF_STRING,804,L"Save replay\tCtrl+Shift+F11");
      AppendMenuW(menu,MF_SEPARATOR,0,nullptr); AppendMenuW(menu,MF_STRING,805,L"Exit");
      POINT cursor{}; GetCursorPos(&cursor); SetForegroundWindow(owner);
      auto action = TrackPopupMenu(menu,TPM_RETURNCMD | TPM_RIGHTBUTTON,cursor.x,cursor.y,0,owner,nullptr);
      DestroyMenu(menu);
      if (action == 801) { ShowWindow(owner,SW_RESTORE); SetForegroundWindow(owner); }
      if (action == 802) SendAction("toggle");
      if (action == 803) SendAction("pause");
      if (action == 804) SendAction("replay");
      if (action == 805) PostMessageW(owner,WM_CLOSE,0,0);
    }
    return true;
  }
  // Explorer may restart; re-add the tray icon through the Dart host setup.
  static const auto taskbar_created = RegisterWindowMessageW(L"TaskbarCreated");
  if (message == taskbar_created) { tray_added = false; SendAction("restoreControls"); return true; }
  return false;
}
void DisposeRecorderHost(HWND owner) {
  for (auto id : registered_hotkeys) UnregisterHotKey(owner,id);
  registered_hotkeys.clear();
  if (tray_added) { NOTIFYICONDATAW icon{}; icon.cbSize=sizeof(icon); icon.hWnd=owner; icon.uID=1; Shell_NotifyIconW(NIM_DELETE,&icon); tray_added=false; }
  channel.reset();
}
