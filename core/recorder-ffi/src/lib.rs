//! Native control plane. JSON carries configuration/status only, never pixels.
mod camera_preview;
mod follow;
mod layers;
mod performance;
#[cfg(windows)]
mod preview;
mod shortcuts;
mod sources;
mod worker;
use serde_json::json;
use std::{
    ffi::{c_char, CStr, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    sync::atomic::Ordering,
    sync::{Mutex, OnceLock},
};

static WORKER: OnceLock<Mutex<Option<worker::Worker>>> = OnceLock::new();
fn guard(operation: impl FnOnce() -> serde_json::Value) -> *mut c_char {
    let value = catch_unwind(AssertUnwindSafe(operation))
        .unwrap_or_else(|_| json!({"ok":false,"error":"Native control operation panicked"}));
    CString::new(value.to_string()).map_or(std::ptr::null_mut(), CString::into_raw)
}
#[no_mangle]
pub extern "C" fn recorder_abi_version() -> u32 {
    0x0001_0500
}
/// # Safety
/// text is valid NUL-terminated JSON containing webcam and low_resource.
#[no_mangle]
pub unsafe extern "C" fn recorder_webcam_preview_request_json(text: *const c_char) -> *mut c_char {
    guard(|| {
        if text.is_null() {
            return json!({"ok":false,"error":"Null camera preview request"});
        }
        let result = serde_json::from_slice(unsafe { CStr::from_ptr(text) }.to_bytes())
            .map_err(|e| e.to_string())
            .and_then(camera_preview::request);
        match result {
            Ok(()) => camera_preview::status(),
            Err(error) => json!({"ok":false,"error":error}),
        }
    })
}
/// # Safety
/// Output arguments point to writable size_t/u32 storage. Non-null results
/// contain length premultiplied RGBA bytes, at most 320x240. Release once with
/// recorder_preview_free using exactly that length. Null means pending/stale/off.
#[no_mangle]
pub unsafe extern "C" fn recorder_webcam_preview_rgba(
    length: *mut usize,
    width: *mut u32,
    height: *mut u32,
) -> *mut u8 {
    if length.is_null() || width.is_null() || height.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        *length = 0;
        *width = 0;
        *height = 0;
    }
    let frame = catch_unwind(AssertUnwindSafe(camera_preview::frame))
        .ok()
        .flatten();
    if let Some((w, h, bytes)) = frame {
        unsafe {
            *length = bytes.len();
            *width = w;
            *height = h;
        }
        return Box::into_raw(bytes.into_boxed_slice()).cast::<u8>();
    }
    std::ptr::null_mut()
}
#[no_mangle]
pub extern "C" fn recorder_sources_json() -> *mut c_char {
    guard(
        || json!({"ok":true,"sources":sources::enumerate(),"platform":std::env::consts::OS,"performance":performance::Machine::detect()}),
    )
}
/// Enumerates cameras without opening a capture stream. Call on a background
/// isolate: some DirectShow drivers enumerate slowly.
#[no_mangle]
pub extern "C" fn recorder_cameras_json() -> *mut c_char {
    guard(|| match recorder_engine::webcam::devices() {
        Ok(devices) => {
            json!({"ok": true, "devices": devices.into_iter().map(|(id,name)| json!({"id":id,"name":name})).collect::<Vec<_>>()})
        }
        Err(error) => json!({"ok":false,"error":error.to_string()}),
    })
}
/// # Safety
/// text is a valid NUL-terminated UTF-8 JSON request for this call.
/// pixels references length readable straight-alpha RGBA bytes until return.
#[no_mangle]
pub unsafe extern "C" fn recorder_layer_rgba_json(
    text: *const c_char,
    pixels: *const u8,
    length: usize,
) -> *mut c_char {
    guard(|| {
        if text.is_null() || pixels.is_null() || length > 2 * 1024 * 1024 {
            return json!({"ok":false,"error":"Invalid overlay upload"});
        }
        let result = serde_json::from_slice(unsafe { CStr::from_ptr(text) }.to_bytes())
            .map_err(|e| e.to_string())
            .and_then(|meta| {
                layers::upload(meta, unsafe { std::slice::from_raw_parts(pixels, length) })
            });
        match result {
            Ok(()) => json!({"ok":true}),
            Err(error) => json!({"ok":false,"error":error}),
        }
    })
}
/// # Safety
/// text is a valid NUL-terminated UTF-8 JSON request for this call.
#[no_mangle]
pub unsafe extern "C" fn recorder_preview_request_json(text: *const c_char) -> *mut c_char {
    guard(|| {
        #[cfg(windows)]
        {
            if text.is_null() {
                return json!({"ok":false,"error":"Null preview request"});
            }
            let result = serde_json::from_slice(unsafe { CStr::from_ptr(text) }.to_bytes())
                .map_err(|e| e.to_string())
                .and_then(preview::request);
            match result {
                Ok(()) => json!({"ok":true}),
                Err(error) => json!({"ok":false,"error":error}),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = text;
            json!({"ok":false,"error":"Native window preview requires Windows"})
        }
    })
}
/// # Safety
/// length points to writable size_t storage. Non-null results are owned PNG
/// allocations; release with recorder_preview_free using the exact length.
#[no_mangle]
pub unsafe extern "C" fn recorder_preview_png(length: *mut usize) -> *mut u8 {
    if length.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        *length = 0;
    }
    #[cfg(windows)]
    {
        if let Some(bytes) = preview::png() {
            unsafe {
                *length = bytes.len();
            }
            return Box::into_raw(bytes.into_boxed_slice()).cast::<u8>();
        }
    }
    std::ptr::null_mut()
}
/// # Safety
/// pointer/length must be one unfreed allocation returned by recorder_preview_png.
#[no_mangle]
pub unsafe extern "C" fn recorder_preview_free(pointer: *mut u8, length: usize) {
    if !pointer.is_null() {
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                pointer, length,
            )));
        }
    }
}
/// # Safety
/// text must be a valid NUL-terminated UTF-8 C string during this call. The
/// deserialized command is owned by Rust; borrowed input is never retained.
#[no_mangle]
pub unsafe extern "C" fn recorder_command_json(text: *const c_char) -> *mut c_char {
    guard(|| {
        if text.is_null() {
            return json!({"ok":false,"error":"Null command"});
        }
        let command: serde_json::Value =
            match serde_json::from_slice(unsafe { CStr::from_ptr(text) }.to_bytes()) {
                Ok(value) => value,
                Err(e) => return json!({"ok":false,"error":e.to_string()}),
            };
        let mut locked = match WORKER.get_or_init(|| Mutex::new(None)).lock() {
            Ok(locked) => locked,
            Err(_) => return json!({"ok":false,"error":"Native control lock failed"}),
        };
        match command["action"].as_str().unwrap_or("") {
            "webcam_visible" => {
                let Some(worker) = locked.as_ref().filter(|w| !w.finished()) else {
                    return json!({"ok":false,"error":"No active recording"});
                };
                if !worker.webcam_available {
                    return json!({"ok":false,"error":"Enable webcam before starting this recording"});
                }
                let Some(enabled) = command["enabled"].as_bool() else {
                    return json!({"ok":false,"error":"Webcam visibility must be boolean"});
                };
                worker.webcam_visible.store(enabled, Ordering::Release);
                if let Ok(mut status) = worker.status.lock() {
                    status["webcam_visible"] = json!(enabled);
                }
                json!({"ok":true})
            }
            "clear_layers" => match layers::clear() {
                Ok(()) => json!({"ok":true}),
                Err(error) => json!({"ok":false,"error":error}),
            },
            "start" => {
                if locked.as_ref().is_some_and(|w| !w.finished()) {
                    return json!({"ok":false,"error":"A recording is already active"});
                }
                if let Some(mut old) = locked.take() {
                    if let Some(t) = old.thread.take() {
                        let _ = t.join();
                    }
                }
                let request = match serde_json::from_value(command["config"].clone()) {
                    Ok(r) => r,
                    Err(_) => return json!({"ok":false,"error":"Invalid recording configuration"}),
                };
                match worker::Worker::start(request) {
                    Ok(w) => {
                        *locked = Some(w);
                        json!({"ok":true})
                    }
                    Err(e) => json!({"ok":false,"error":e}),
                }
            }
            "reduce_quality" => {
                let Some(worker) = locked.as_ref().filter(|w| !w.finished()) else {
                    return json!({"ok":false,"error":"No active recording"});
                };
                if worker
                    .status
                    .lock()
                    .map(|s| s["adaptive_quality"] != true)
                    .unwrap_or(true)
                {
                    return json!({"ok":false,"error":"Enable adaptive MKV quality before recording"});
                }
                worker.reduce_quality.store(true, Ordering::Release);
                json!({"ok":true})
            }
            "annotations" => {
                let Some(worker) = locked.as_ref().filter(|w| !w.finished()) else {
                    return json!({"ok":false,"error":"No active recording"});
                };
                let marks = match worker::annotations(command["items"].clone()) {
                    Ok(value) => value,
                    Err(error) => return json!({"ok":false,"error":error}),
                };
                match worker.annotations.lock() {
                    Ok(mut value) => {
                        *value = marks;
                        json!({"ok":true})
                    }
                    Err(_) => json!({"ok":false,"error":"Annotation controls unavailable"}),
                }
            }
            "save_replay" => {
                let Some(worker) = locked.as_ref().filter(|w| !w.finished()) else {
                    return json!({"ok":false,"error":"No active recording"});
                };
                let Some(path) = command["path"]
                    .as_str()
                    .filter(|p| p.len() <= 32768 && !p.contains('\0'))
                else {
                    return json!({"ok":false,"error":"Invalid replay output"});
                };
                let file = std::path::Path::new(path);
                if !file.is_absolute()
                    || file
                        .extension()
                        .is_none_or(|e| !e.eq_ignore_ascii_case("mkv"))
                    || file.exists()
                {
                    return json!({"ok":false,"error":"Choose a new absolute MKV replay filename"});
                }
                match worker.replay_request.lock() {
                    Ok(mut pending) if pending.is_none() => {
                        *pending = Some(path.to_owned());
                        json!({"ok":true})
                    }
                    _ => json!({"ok":false,"error":"Another replay save is pending"}),
                }
            }
            "audio" => {
                let Some(worker) = locked.as_ref() else {
                    return json!({"ok":false,"error":"No recording session"});
                };
                let settings: worker::AudioControls =
                    match serde_json::from_value(command["controls"].clone()) {
                        Ok(value) => value,
                        Err(e) => return json!({"ok":false,"error":e.to_string()}),
                    };
                if let Err(e) = settings.validate() {
                    return json!({"ok":false,"error":e});
                }
                match worker.audio_controls.lock() {
                    Ok(mut value) => {
                        *value = settings;
                        json!({"ok":true})
                    }
                    Err(_) => json!({"ok":false,"error":"Audio controls unavailable"}),
                }
            }
            "stop_streaming" => {
                let Some(worker) = locked.as_ref().filter(|w| !w.finished()) else {
                    return json!({"ok":false,"error":"No active recording"});
                };
                worker.streaming_stop.store(true, Ordering::Release);
                json!({"ok":true})
            }
            "pause" | "resume" | "stop" => {
                let Some(worker) = locked.as_ref() else {
                    return json!({"ok":false,"error":"No recording session"});
                };
                if worker.finished() {
                    return json!({"ok":false,"error":"Recording has already ended"});
                }
                if command["action"] == "pause"
                    && worker
                        .status
                        .lock()
                        .map(|s| s["streaming_active"] == true)
                        .unwrap_or(true)
                {
                    return json!({"ok":false,"error":"Stop live streaming before pausing the local recording"});
                }
                match command["action"].as_str() {
                    Some("pause") => worker.pause.store(true, Ordering::Release),
                    Some("resume") => worker.pause.store(false, Ordering::Release),
                    _ => worker.stop.store(true, Ordering::Release),
                }
                json!({"ok":true})
            }
            _ => json!({"ok":false,"error":"Unknown recorder command"}),
        }
    })
}
#[no_mangle]
pub extern "C" fn recorder_status_json() -> *mut c_char {
    guard(|| {
        let Ok(locked) = WORKER.get_or_init(|| Mutex::new(None)).lock() else {
            return json!({"state":"failed","error":"Control lock failed"});
        };
        match locked.as_ref() {
            Some(w) => w
                .status
                .lock()
                .map(|s| s.clone())
                .unwrap_or_else(|_| json!({"state":"failed","error":"Status lock failed"})),
            None => json!({"state":"idle","frames":0,"seconds":0}),
        }
    })
}
/// # Safety
/// pointer must be null or an unreleased string returned by this library.
#[no_mangle]
pub unsafe extern "C" fn recorder_string_free(pointer: *mut c_char) {
    if !pointer.is_null() {
        drop(unsafe { CString::from_raw(pointer) });
    }
}
