//! Native control plane. JSON carries configuration/status only, never pixels.
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
    0x0001_0100
}
#[no_mangle]
pub extern "C" fn recorder_sources_json() -> *mut c_char {
    guard(|| json!({"ok":true,"sources":sources::enumerate(),"platform":std::env::consts::OS}))
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
                    Err(e) => return json!({"ok":false,"error":e.to_string()}),
                };
                match worker::Worker::start(request) {
                    Ok(w) => {
                        *locked = Some(w);
                        json!({"ok":true})
                    }
                    Err(e) => json!({"ok":false,"error":e}),
                }
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
            "pause" | "resume" | "stop" => {
                let Some(worker) = locked.as_ref() else {
                    return json!({"ok":false,"error":"No recording session"});
                };
                if worker.finished() {
                    return json!({"ok":false,"error":"Recording has already ended"});
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
