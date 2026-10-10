#ifndef RECORDER_CORE_H
#define RECORDER_CORE_H
#include <stdint.h>
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
RECORDER_API char* recorder_command_json(const char* command);
RECORDER_API char* recorder_status_json(void);
RECORDER_API void recorder_string_free(char* response);
#ifdef __cplusplus
}
#endif
#endif
