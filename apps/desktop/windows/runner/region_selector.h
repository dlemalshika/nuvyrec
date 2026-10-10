#pragma once
#include <windows.h>
#include <optional>
// Returns physical desktop coordinates. Escape/right-click cancels.
std::optional<RECT> SelectCaptureRegion(HWND owner, HMONITOR monitor);
