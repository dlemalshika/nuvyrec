#pragma once
#include <flutter/encodable_value.h>
#include <vector>
#include <cstdint>

// Low-rate UI thumbnails only. The recording engine retains its own WGC path.
std::vector<uint8_t> CapturePreview(const flutter::EncodableMap& arguments);
