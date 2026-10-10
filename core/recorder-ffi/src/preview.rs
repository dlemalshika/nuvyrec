//! Independent, bounded native window thumbnails. The UI polls a PNG cache;
//! capture/encoding never blocks Flutter's platform thread. No raw pixels/JSON.
use crate::sources;
use recorder_engine::capture::Capture;
use serde::Deserialize;
use std::{
    sync::{Arc, Condvar, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: u64,
    pub width: u32,
    pub height: u32,
}
#[derive(Default)]
struct State {
    wanted: Option<Request>,
    pending: Option<Request>,
    png: Option<Vec<u8>>,
    error: Option<String>,
}
struct Service {
    state: Arc<(Mutex<State>, Condvar)>,
}
static SERVICE: OnceLock<Service> = OnceLock::new();
impl Service {
    fn new() -> Self {
        let state = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let shared = state.clone();
        thread::Builder::new()
            .name("frame-window-preview".into())
            .spawn(move || run(shared))
            .expect("create preview worker");
        Self { state }
    }
}
/// All COM, capture graph and image-encoding state belongs to this worker.
fn run(shared: Arc<(Mutex<State>, Condvar)>) {
    let _dpi = sources::PhysicalCoordinates::enter();
    let apartment = recorder_engine::wasapi::Apartment::initialize();
    let mut capture: Option<(Request, Capture)> = None;
    loop {
        let (mutex, wake) = &*shared;
        let mut state = mutex.lock().unwrap_or_else(|e| e.into_inner());
        while state.pending.is_none() {
            let (next, idle) = wake
                .wait_timeout(state, Duration::from_secs(2))
                .unwrap_or_else(|e| e.into_inner());
            state = next;
            if idle.timed_out() {
                capture = None;
            }
        }
        let request = state.pending.take().unwrap();
        drop(state);
        let result = (|| -> Result<Vec<u8>, String> {
            apartment.as_ref().map_err(|e| e.to_string())?;
            let source = sources::enumerate()
                .into_iter()
                .find(|s| s.kind == "window" && s.id == request.id)
                .ok_or("Window closed or unavailable")?;
            if source.minimized {
                return Err("Restore the selected window to preview it".into());
            }
            if capture.as_ref().is_none_or(|(key, _)| key != &request) {
                capture = None; // release old source/pool before opening another
                let graph=format!("gfxcapture=hwnd={}:capture_border=1:capture_cursor=0:width={}:height={}:max_framerate=1:resize_mode=scale_aspect,hwdownload,format=bgra,format=rgba",request.id,request.width,request.height);
                capture = Some((
                    request.clone(),
                    Capture::open(&graph).map_err(|e| e.to_string())?,
                ));
            }
            let began = Instant::now();
            loop {
                if let Some(frame) = capture
                    .as_mut()
                    .unwrap()
                    .1
                    .next_frame()
                    .map_err(|e| e.to_string())?
                {
                    return frame.thumbnail_png().map_err(|e| e.to_string());
                }
                if began.elapsed() > Duration::from_secs(2) {
                    let state = mutex.lock().unwrap_or_else(|e| e.into_inner());
                    if state.wanted.as_ref() == Some(&request) {
                        if let Some(png) = &state.png {
                            return Ok(png.clone());
                        }
                    }
                    return Err("Window did not produce a preview; restore it and check capture permissions".into());
                }
                if mutex
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .wanted
                    .as_ref()
                    != Some(&request)
                {
                    return Err("Preview source changed".into());
                }
                thread::sleep(Duration::from_millis(5));
            }
        })();
        if result.is_err() {
            capture = None;
        }
        let mut state = mutex.lock().unwrap_or_else(|e| e.into_inner());
        if state.wanted.as_ref() == Some(&request) {
            match result {
                Ok(png) => {
                    state.png = Some(png);
                    state.error = None;
                }
                Err(error) => {
                    state.png = None;
                    state.error = Some(error);
                }
            }
        }
    }
}
pub fn request(request: Request) -> Result<(), String> {
    if request.id == 0
        || request.width < 1
        || request.height < 1
        || request.width > 480
        || request.height > 270
    {
        return Err("Invalid thumbnail request".into());
    }
    let service = SERVICE.get_or_init(Service::new);
    let (mutex, wake) = &*service.state;
    let mut state = mutex.lock().map_err(|_| "Preview lock failed")?;
    if state.wanted.as_ref() != Some(&request) {
        state.png = None;
        state.error = None;
    }
    state.wanted = Some(request.clone());
    state.pending = Some(request);
    wake.notify_one();
    state.error.clone().map_or(Ok(()), Err)
}
pub fn png() -> Option<Vec<u8>> {
    SERVICE.get()?.state.0.lock().ok()?.png.clone()
}
