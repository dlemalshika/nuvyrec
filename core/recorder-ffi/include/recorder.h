#ifndef RECORDER_CORE_H
#define RECORDER_CORE_H
#include <stdint.h>
#include <stddef.h>
#if defined(_WIN32)
#define RECORDER_API __declspec(dllimport)
#else
#define RECORDER_API
#endif
#ifdef __cplusplus
extern "C" {
#endif
RECORDER_API uint32_t recorder_abi_version(void);
/* Small UTF-8 JSON records, never pixel buffers. Returned strings are owned by
 * Rust and must be freed exactly once with recorder_string_free. */
RECORDER_API char* recorder_sources_json(void);
/* May enumerate slowly: use a background UI isolate. Does not open a camera. */
RECORDER_API char* recorder_cameras_json(void);
/* Shared preview/recording reader; webcam settings and low_resource boolean.
 * enabled=false releases the preview lease. No camera microphone is opened. */
RECORDER_API char* recorder_webcam_preview_request_json(const char* request);
/* Latest premultiplied RGBA thumbnail, <=320x240. Null pending/off/stale.
 * Free non-null bytes with recorder_preview_free and original length. */
RECORDER_API uint8_t* recorder_webcam_preview_rgba(size_t* length, uint32_t* width, uint32_t* height);
/* Owned cached overlay: metadata id(0..3),pixel_width,pixel_height,x,y,width,
 * opacity; tightly packed straight-alpha RGBA (<=1024x512), copied on return.
 * JSON response uses recorder_string_free. Total native cache <=4 MiB. */
RECORDER_API char* recorder_layer_rgba_json(const char* metadata, const uint8_t* pixels, size_t length);
/* Queue a bounded asynchronous window thumbnail (id,width<=480,height<=270).
 * The returned JSON string uses recorder_string_free. Poll the PNG cache below.
 * NULL means pending; exactly one matching request is cached. */
RECORDER_API char* recorder_preview_request_json(const char* request);
RECORDER_API uint8_t* recorder_preview_png(size_t* length);
/* Free a non-null PNG exactly once with the original length. */
RECORDER_API void recorder_preview_free(uint8_t* bytes, size_t length);
RECORDER_API char* recorder_command_json(const char* command);
RECORDER_API char* recorder_status_json(void);
RECORDER_API void recorder_string_free(char* response);
#ifdef __cplusplus
}
#endif
#endif
