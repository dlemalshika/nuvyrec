//! A latest-only webcam preview lease. No second camera reader or PNG encoding.
use crate::worker::WebcamRequest;
use recorder_engine::webcam::Camera;
use serde::Deserialize;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub webcam: WebcamRequest,
    pub low_resource: bool,
}
struct Preview {
    camera: Camera,
    request: Request,
}
static PREVIEW: OnceLock<Mutex<Option<Preview>>> = OnceLock::new();
pub fn request(request: Request) -> Result<(), String> {
    let mut state = PREVIEW
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "camera preview lock failed")?;
    if !request.webcam.enabled {
        *state = None;
        return Ok(());
    }
    if !request.webcam.placement().valid()
        || request.webcam.device.is_empty()
        || request.webcam.device.len() > 4096
    {
        return Err("Choose a webcam in Settings before enabling it".into());
    }
    if state.as_ref().is_some_and(|v| {
        v.request.webcam.device == request.webcam.device
            && v.request.low_resource == request.low_resource
    }) {
        state.as_mut().unwrap().request = request;
        return Ok(());
    }
    *state = None;
    let camera = Camera::start(
        request.webcam.device.clone(),
        if request.low_resource { 320 } else { 640 },
    )
    .map_err(|e| e.to_string())?;
    *state = Some(Preview { camera, request });
    Ok(())
}
pub fn frame() -> Option<(u32, u32, Vec<u8>)> {
    let state = PREVIEW.get()?.lock().ok()?;
    let preview = state.as_ref()?;
    let image = preview.camera.latest.lock().ok()?;
    let image = image
        .as_ref()
        .filter(|v| v.captured.elapsed() < Duration::from_secs(2))?;
    let (w, h, rgba) = image
        .preview_rgba(&preview.request.webcam.placement())
        .ok()?;
    Some((w as u32, h as u32, rgba))
}
pub fn status() -> serde_json::Value {
    let state = PREVIEW
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let Some(preview) = state.as_ref() else {
        return serde_json::json!({"ok":true,"enabled":false});
    };
    let error = preview.camera.error.lock().ok().and_then(|v| v.clone());
    serde_json::json!({"ok":true,"enabled":true,"error":error})
}
