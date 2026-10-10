#ifndef RECORDER_HOST_H
#define RECORDER_HOST_H
#include <flutter/binary_messenger.h>
#include <windows.h>
void RegisterRecorderHost(flutter::BinaryMessenger* messenger, HWND owner, bool creator_fixture = false);
bool HandleRecorderHostMessage(HWND owner, UINT message, WPARAM wparam, LPARAM lparam);
void DisposeRecorderHost(HWND owner);
#endif
