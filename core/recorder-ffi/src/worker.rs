use crate::sources;
use recorder_engine::{capture::Capture, Container, RecordingConfig, RecordingSession};
use serde::Deserialize;
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AudioControls {
    pub system_gain: f32,
    pub microphone_gain: f32,
    pub system_muted: bool,
    pub microphone_muted: bool,
}
impl Default for AudioControls {
    fn default() -> Self {
        Self {
            system_gain: 1.0,
            microphone_gain: 1.0,
            system_muted: false,
            microphone_muted: false,
        }
    }
}
impl AudioControls {
    pub fn validate(&self) -> Result<(), String> {
        if !self.system_gain.is_finite()
            || !self.microphone_gain.is_finite()
            || !(0.0..=2.0).contains(&self.system_gain)
            || !(0.0..=2.0).contains(&self.microphone_gain)
        {
            return Err("Audio gain must be between 0% and 200%".into());
        }
        Ok(())
    }
}
fn enabled() -> bool {
    true
}
fn shortcut_seconds() -> u32 {
    2
}
fn manual_mode() -> String {
    "manual".into()
}

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WebcamRequest {
    pub enabled: bool,
    pub device: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub mirror: bool,
    pub key: bool,
    pub similarity: f32,
    pub softness: f32,
}
impl Default for WebcamRequest {
    fn default() -> Self {
        Self {
            enabled: false,
            device: String::new(),
            x: 1.0,
            y: 1.0,
            width: 0.25,
            mirror: true,
            key: false,
            similarity: 0.15,
            softness: 0.08,
        }
    }
}
impl WebcamRequest {
    pub(crate) fn placement(&self) -> recorder_engine::webcam::Placement {
        recorder_engine::webcam::Placement {
            x: self.x,
            y: self.y,
            width: self.width,
            mirror: self.mirror,
            key: self.key,
            similarity: self.similarity,
            softness: self.softness,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnnotationRequest {
    pub kind: String,
    pub points: Vec<[f32; 2]>,
    pub rgb: [u8; 3],
    pub width: u32,
}
pub fn annotations(
    value: serde_json::Value,
) -> Result<Vec<recorder_engine::overlay::Annotation>, String> {
    use recorder_engine::overlay::{Annotation, Shape};
    let requests: Vec<AnnotationRequest> =
        serde_json::from_value(value).map_err(|e| e.to_string())?;
    if requests.len() > 64 || requests.iter().filter(|r| r.kind == "spotlight").count() > 1 {
        return Err("Limit annotations to 64 objects and one spotlight".into());
    }
    requests
        .into_iter()
        .map(|r| {
            if r.points.is_empty()
                || r.points.len() > 256
                || !(1..=16).contains(&r.width)
                || r.points
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            {
                return Err("Invalid annotation geometry".into());
            }
            let shape = match r.kind.as_str() {
                "draw" => Shape::Stroke,
                "arrow" => Shape::Arrow,
                "box" => Shape::Box,
                "spotlight" => Shape::Spotlight,
                _ => return Err("Unknown annotation tool".into()),
            };
            Ok(Annotation {
                shape,
                points: r.points,
                rgb: r.rgb,
                width: r.width,
            })
        })
        .collect()
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamRequest {
    pub platform: String,
    pub server: String,
    pub key: String,
}
impl StreamRequest {
    fn destination(&self) -> recorder_engine::streaming::Destination {
        recorder_engine::streaming::Destination {
            platform: self.platform.clone(),
            server: self.server.clone(),
            key: self.key.clone(),
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    #[serde(default)]
    pub streaming: Vec<StreamRequest>,
    pub output: String,
    pub container: String,
    pub source_id: u64,
    pub source_kind: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    pub encoder: String,
    #[serde(default)]
    pub system_audio: bool,
    #[serde(default)]
    pub microphone: bool,
    #[serde(default)]
    pub region: Option<[u32; 4]>,
    #[serde(default)]
    pub follow_mouse: bool,
    #[serde(default)]
    pub system_device: String,
    #[serde(default)]
    pub microphone_device: String,
    #[serde(default)]
    pub audio_controls: AudioControls,
    #[serde(default)]
    pub mix_audio: bool,
    #[serde(default)]
    pub noise_suppression: bool,
    #[serde(default)]
    pub noise_gate: bool,
    #[serde(default)]
    pub limiter: bool,
    #[serde(default)]
    pub max_seconds: u32,
    #[serde(default)]
    pub max_file_mib: u32,
    #[serde(default = "enabled")]
    pub capture_cursor: bool,
    #[serde(default)]
    pub replay_seconds: u32,
    #[serde(default)]
    pub replay_mib: u32,
    #[serde(default)]
    pub annotations_enabled: bool,
    #[serde(default)]
    pub layers_enabled: bool,
    #[serde(default)]
    pub shortcut_overlay: bool,
    #[serde(default = "shortcut_seconds")]
    pub shortcut_seconds: u32,
    #[serde(default)]
    pub shortcut_right: bool,
    #[serde(default)]
    pub cursor_highlight: bool,
    #[serde(default)]
    pub click_effects: bool,
    #[serde(default)]
    pub segment_seconds: u32,
    #[serde(default = "manual_mode")]
    pub performance_mode: String,
    #[serde(default)]
    pub adaptive_quality: bool,
    #[serde(default)]
    pub webcam: WebcamRequest,
}

pub struct Worker {
    pub streaming_stop: Arc<AtomicBool>,
    pub stop: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
    pub status: Arc<Mutex<serde_json::Value>>,
    pub thread: Option<JoinHandle<()>>,
    pub audio_controls: Arc<Mutex<AudioControls>>,
    pub replay_request: Arc<Mutex<Option<String>>>,
    pub annotations: Arc<Mutex<Vec<recorder_engine::overlay::Annotation>>>,
    pub reduce_quality: Arc<AtomicBool>,
    pub webcam_visible: Arc<AtomicBool>,
    pub webcam_available: bool,
}

impl Worker {
    pub fn start(request: Request) -> Result<Self, String> {
        if request.streaming.len() > 3 {
            return Err("Choose at most three streaming destinations".into());
        }
        for (index, target) in request.streaming.iter().enumerate() {
            target.destination().validate().map_err(|e| e.to_string())?;
            if request.streaming[..index]
                .iter()
                .any(|t| t.platform == target.platform)
            {
                return Err("Duplicate streaming destination".into());
            }
        }
        if !request.streaming.is_empty() {
            if !["mkv", "mp4"].contains(&request.container.as_str())
                || request.segment_seconds != 0
                || request.adaptive_quality
            {
                return Err("Live streaming requires one continuous MKV/MP4 recording; disable recovery parts and adaptive resolution".into());
            }
            if request.system_audio && request.microphone && !request.mix_audio {
                return Err(
                    "Enable combined audio for streaming system sound and microphone".into(),
                );
            }
        }
        if !cfg!(windows) {
            return Err("Native desktop capture is currently available on Windows only".into());
        }
        let source = sources::enumerate()
            .into_iter()
            .find(|s| s.id == request.source_id && s.kind == request.source_kind)
            .ok_or("Capture source no longer exists; refresh the source list")?;
        if source.minimized {
            return Err("Restore the selected window before recording".into());
        }
        if request.shortcut_overlay && !(1..=5).contains(&request.shortcut_seconds) {
            return Err("Shortcut display duration must be 1–5 seconds".into());
        }
        if request.follow_mouse && request.region.is_none() {
            return Err("Follow mouse requires a region on the selected display".into());
        }
        if !["auto", "libx264", "h264_nvenc", "h264_amf", "h264_qsv"]
            .contains(&request.encoder.as_str())
        {
            return Err("Unsupported encoder".into());
        }
        let container = match request.container.as_str() {
            "mp4" => Container::Mp4,
            "mkv" => Container::Matroska,
            "avi" => Container::Avi,
            "gif" => Container::Gif,
            _ => return Err("Unsupported recording container".into()),
        };
        let mut config = RecordingConfig {
            output: request.output.clone().into(),
            container,
            width: request.width,
            height: request.height,
            fps: request.fps,
            bitrate: request.bitrate,
            encoder_threads: 2,
        };
        config.validate().map_err(str::to_owned)?;
        if !["automatic", "eco", "manual"].contains(&request.performance_mode.as_str()) {
            return Err("Unknown performance mode".into());
        }
        if request.adaptive_quality && request.container != "mkv" {
            return Err("Adaptive resolution requires independently finalized MKV parts".into());
        }
        crate::performance::Machine::detect().constrain(
            &mut config,
            &request.performance_mode,
            false,
        );
        request.audio_controls.validate()?;
        if request.webcam.enabled
            && (!request.webcam.placement().valid()
                || request.webcam.device.is_empty()
                || request.webcam.device.len() > 4096)
        {
            return Err("Choose a webcam and valid overlay settings".into());
        }
        if request.segment_seconds != 0
            && (!(2..=3600).contains(&request.segment_seconds) || request.container != "mkv")
        {
            return Err("Recovery segments require MKV and a 2–3600 second interval".into());
        }
        if (request.annotations_enabled
            || request.layers_enabled
            || request.shortcut_overlay
            || request.cursor_highlight
            || request.click_effects
            || request.webcam.enabled)
            && request.container == "gif"
        {
            return Err("Overlays require H.264 recording, not GIF".into());
        }
        if request.replay_seconds > 0
            && (request.container == "gif"
                || !(5..=120).contains(&request.replay_seconds)
                || !(4..=128).contains(&request.replay_mib))
        {
            return Err("Replay requires H.264 video, 5–120 seconds and 4–128 MiB".into());
        }
        if request.max_seconds > 86400 || request.max_file_mib > 1048576 {
            return Err("Recording limits exceed 24 hours or 1 TiB".into());
        }
        if request.container == "gif" && (request.system_audio || request.microphone) {
            return Err("GIF does not support audio tracks".into());
        }
        if request.container == "gif" && (request.fps > 15 || request.height > 720) {
            return Err("GIF recordings are limited to 720p and 15 fps".into());
        }
        if config.output.exists() {
            return Err("Output file already exists; choose a new file".into());
        }
        if let Some([x, y, w, h]) = request.region {
            if source.kind != "display"
                || w < 2
                || h < 2
                || x.checked_add(w).is_none_or(|r| r > source.width)
                || y.checked_add(h).is_none_or(|b| b > source.height)
            {
                return Err("Region must fit inside the selected display".into());
            }
        }
        let stop = Arc::new(AtomicBool::new(false));
        let pause = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(
            json!({"state":"preparing","output":request.output,"frames":0,"seconds":0,
                "streaming_active": !request.streaming.is_empty()}),
        ));
        let worker_stop = stop.clone();
        let worker_pause = pause.clone();
        let worker_status = status.clone();
        let audio_controls = Arc::new(Mutex::new(request.audio_controls.clone()));
        let worker_controls = audio_controls.clone();
        let replay_request = Arc::new(Mutex::new(None));
        let worker_replay = replay_request.clone();
        let annotations = Arc::new(Mutex::new(Vec::new()));
        let worker_annotations = annotations.clone();
        let reduce_quality = Arc::new(AtomicBool::new(false));
        let worker_quality = reduce_quality.clone();
        let webcam_available = request.webcam.enabled;
        let webcam_visible = Arc::new(AtomicBool::new(webcam_available));
        let worker_webcam_visible = webcam_visible.clone();
        let streaming_stop = Arc::new(AtomicBool::new(false));
        let worker_streaming_stop = streaming_stop.clone();
        let thread = thread::Builder::new()
            .name("recorder-media".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(
                        request,
                        config,
                        source,
                        &worker_stop,
                        &worker_pause,
                        &worker_status,
                        &worker_controls,
                        &worker_replay,
                        &worker_annotations,
                        &worker_quality,
                        &worker_webcam_visible,
                        &worker_streaming_stop,
                    )
                }));
                let error = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(e)) => Some(e),
                    Err(_) => Some("Native recording worker panicked".into()),
                };
                if let Some(error) = error {
                    if let Ok(mut value) = worker_status.lock() {
                        value["streaming_active"] = json!(false);
                        if let Some(items) = value["streaming"].as_array_mut() {
                            for item in items {
                                item["state"] = json!("stopped");
                            }
                        }
                    }
                    update(&worker_status, "failed", Some(&error), None, None, None);
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            streaming_stop,
            stop,
            pause,
            status,
            thread: Some(thread),
            audio_controls,
            replay_request,
            annotations,
            reduce_quality,
            webcam_visible,
            webcam_available,
        })
    }
    pub fn finished(&self) -> bool {
        self.thread.as_ref().is_none_or(|t| t.is_finished())
    }
}

fn update(
    status: &Mutex<serde_json::Value>,
    state: &str,
    error: Option<&str>,
    encoder: Option<&str>,
    frames: Option<u64>,
    seconds: Option<f64>,
) {
    if let Ok(mut value) = status.lock() {
        value["state"] = json!(state);
        if let Some(e) = error {
            value["error"] = json!(e);
        }
        if let Some(e) = encoder {
            value["encoder"] = json!(e);
            value["gpu_pipeline"] = json!(matches!(e, "h264_nvenc" | "h264_amf"));
        }
        if let Some(n) = frames {
            value["frames"] = json!(n);
        }
        if let Some(n) = seconds {
            value["seconds"] = json!(n);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run(
    mut request: Request,
    mut config: RecordingConfig,
    source: sources::Source,
    stop: &AtomicBool,
    pause: &AtomicBool,
    status: &Mutex<serde_json::Value>,
    controls: &Mutex<AudioControls>,
    replay_request: &Mutex<Option<String>>,
    annotations: &Mutex<Vec<recorder_engine::overlay::Annotation>>,
    reduce_quality: &AtomicBool,
    webcam_visible: &AtomicBool,
    streaming_stop: &AtomicBool,
) -> Result<(), String> {
    #[cfg(windows)]
    let _dpi = sources::PhysicalCoordinates::enter();
    #[cfg(windows)]
    let _apartment = recorder_engine::wasapi::Apartment::initialize().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let mut audio = {
        let mut tracks = Vec::new();
        if request.system_audio {
            tracks.push(
                recorder_engine::wasapi::AudioCapture::open_device(true, &request.system_device)
                    .map_err(|e| e.to_string())?,
            );
        }
        if request.microphone {
            tracks.push(
                recorder_engine::wasapi::AudioCapture::open_device(
                    false,
                    &request.microphone_device,
                )
                .map_err(|e| e.to_string())?,
            );
        }
        tracks
    };
    #[cfg(windows)]
    let mut processors: Vec<_> = audio
        .iter()
        .map(|a| {
            recorder_engine::audio_processor::AudioProcessor::new(
                a.rate as i32,
                a.title == "Microphone" && request.noise_suppression,
                a.title == "Microphone" && request.noise_gate,
                request.limiter,
            )
        })
        .collect::<recorder_engine::Result<_>>()
        .map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let mix_enabled = request.mix_audio && audio.len() == 2;
    #[cfg(windows)]
    let mut mixer = if mix_enabled {
        Some(recorder_engine::audio_processor::AudioMixer::new(2).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let silent_stream_audio =
        !request.streaming.is_empty() && !request.system_audio && !request.microphone;
    #[cfg(windows)]
    let audio_rates: Vec<(i32, &str)> = if silent_stream_audio {
        vec![(48000, "Silent playback")]
    } else if mix_enabled {
        vec![
            (48000, "Combined audio"),
            (48000, "System audio"),
            (48000, "Microphone"),
        ]
    } else {
        audio.iter().map(|a| (48000, a.title)).collect()
    };
    #[cfg(not(windows))]
    let audio_rates: Vec<(i32, &str)> = Vec::new();
    let handle = if source.kind == "display" {
        format!("hmonitor={}", source.id)
    } else {
        format!("hwnd={}", source.id)
    };
    let crop = match request.region {
        Some([x, y, w, h]) => format!(
            ":crop_left={x}:crop_top={y}:crop_right={}:crop_bottom={}",
            source.width - x - w,
            source.height - y - h
        ),
        None => String::new(),
    };
    let cursor = u8::from(request.capture_cursor || request.follow_mouse);
    let effects = request.annotations_enabled
        || request.layers_enabled
        || request.shortcut_overlay
        || request.cursor_highlight
        || request.click_effects
        || request.webcam.enabled;
    let candidates: Vec<&str> = if request.container == "gif" {
        vec!["gif"]
    } else if request.encoder == "auto" {
        vec!["h264_nvenc", "h264_amf", "h264_qsv", "libx264"]
    } else {
        vec![&request.encoder]
    };
    let mut chosen = None;
    let mut failures = Vec::new();
    for name in candidates {
        if stop.load(Ordering::Acquire) {
            update(status, "stopped", None, None, None, None);
            return Ok(());
        }
        if name == "libx264" {
            crate::performance::Machine::detect().constrain(
                &mut config,
                &request.performance_mode,
                true,
            );
        }
        let graph = capture_graph(&config, name, &handle, &crop, cursor);
        let attempt: Result<_, String> = (|| {
            let mut capture = Capture::open(&graph).map_err(|e| e.to_string())?;
            if effects {
                if let Err(error) = capture.enable_gpu_overlays() {
                    if name != "libx264" {
                        return Err(error.to_string());
                    }
                    failures.push(format!("GPU compositor unavailable; CPU fallback: {error}"));
                }
                #[cfg(windows)]
                if capture.gpu_overlays_enabled() {
                    let layers = if request.layers_enabled {
                        crate::layers::snapshot()?
                    } else {
                        Vec::new()
                    };
                    capture
                        .update_gpu_overlays(&layers, &[], &[], None)
                        .map_err(|e| e.to_string())?;
                }
            }
            let began = Instant::now();
            loop {
                if stop.load(Ordering::Acquire) {
                    return Err("Capture canceled".into());
                }
                if let Some(first) = capture.next_frame().map_err(|e| e.to_string())? {
                    let session =
                        RecordingSession::encoded_with_audio(&config, name, &first, &audio_rates)
                            .map_err(|e| e.to_string())?;
                    return Ok((capture, session, first));
                }
                if began.elapsed() > Duration::from_secs(8) {
                    return Err("Capture source did not produce frames".into());
                }
                thread::sleep(Duration::from_millis(5));
            }
        })();
        match attempt {
            Ok(tuple) => {
                chosen = Some((name.to_owned(), tuple));
                break;
            }
            Err(error) => {
                failures.push(format!("{name}: {error}"));
                if config.output.exists() {
                    break;
                }
            }
        }
    }
    let (encoder, (mut capture, mut session, mut first)) =
        chosen.ok_or_else(|| failures.join("; "))?;
    let camera = if request.webcam.enabled {
        Some(
            recorder_engine::webcam::Camera::start(
                request.webcam.device.clone(),
                if request.performance_mode == "eco" {
                    320
                } else {
                    640
                },
            )
            .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let gpu_overlays = capture.gpu_overlays_enabled();
    let repeat_overlays = !gpu_overlays
        && (camera.is_some()
            || request.layers_enabled
            || request.shortcut_overlay
            || request.annotations_enabled);
    let mut last_screen = if repeat_overlays {
        Some(first.try_clone().map_err(|e| e.to_string())?)
    } else {
        None
    };
    if request.replay_seconds > 0 {
        session
            .enable_replay(request.replay_seconds, request.replay_mib)
            .map_err(|e| e.to_string())?;
    }
    let streaming_enabled =
        !request.streaming.is_empty() && !streaming_stop.load(Ordering::Acquire);
    if streaming_enabled {
        session
            .enable_streaming(
                request
                    .streaming
                    .iter()
                    .map(StreamRequest::destination)
                    .collect(),
            )
            .map_err(|e| e.to_string())?;
        request.streaming.clear(); // only network workers retain credentials
    }
    if let Ok(mut value) = status.lock() {
        value["streaming_active"] = json!(streaming_enabled);
        value["streaming"] = stream_status(&session);
        value["fallback_reasons"] = json!(failures);
        value["annotations_enabled"] = json!(request.annotations_enabled);
        value["layers_enabled"] = json!(request.layers_enabled);
        value["shortcut_overlay"] = json!(request.shortcut_overlay);
        value["shortcut_right"] = json!(request.shortcut_right);
        value["webcam_available"] = json!(request.webcam.enabled);
        value["webcam_visible"] = json!(request.webcam.enabled);
        value["compositor"] = json!(if gpu_overlays {
            "d3d11"
        } else if effects {
            "cpu"
        } else {
            "none"
        });
    }
    if request.layers_enabled && !gpu_overlays {
        first
            .raster_layers(&crate::layers::snapshot()?)
            .map_err(|e| e.to_string())?;
    }
    session
        .write_video(&mut first, 0)
        .map_err(|e| e.to_string())?;
    #[cfg(windows)]
    for track in &mut audio {
        let _ = track.poll().map_err(|e| e.to_string())?;
    }
    let start = Instant::now();
    let mut paused = Duration::ZERO;
    let mut pause_start = None;
    let mut frames = 1;
    let mut last_pts = 0;
    let mut last_event = Instant::now();
    let mut skipped = 0u64;
    let mut encode_ms = 0.0f64;
    let mut segment_start = 0.0f64;
    let mut segment_number = 1u32;
    let mut current_path = config.output.clone();
    let mut finalized_bytes = 0u64;
    let machine = crate::performance::Machine::detect();
    let mut monitor = crate::performance::Monitor::new(machine.logical_cores);
    let mut governor = crate::performance::Governor::new();
    let mut last_load = Instant::now();
    let mut next_quality = None;
    if let Ok(mut value) = status.lock() {
        value["outputs"] = json!([config.output]);
        value["segment_durations"] = json!({});
        value["effective_width"] = json!(config.width);
        value["effective_height"] = json!(config.height);
        value["effective_fps"] = json!(config.fps);
        value["effective_bitrate"] = json!(config.bitrate);
        value["performance_mode"] = json!(request.performance_mode);
        value["adaptive_quality"] = json!(request.adaptive_quality);
    }
    #[cfg(windows)]
    let mut mouse_down = false;
    #[cfg(windows)]
    let mut click: Option<([f32; 2], Instant)> = None;
    update(
        status,
        "recording",
        None,
        Some(&encoder),
        Some(frames),
        Some(0.0),
    );
    let mut current_region = request.region;
    let mut viewport = request
        .region
        .map(|r| crate::follow::Viewport::new(r, [source.width, source.height]));
    let mut last_pointer = Instant::now();
    let mut shortcuts = crate::shortcuts::Tracker::new();
    let mut last_shortcut_poll = Instant::now();
    let mut last_gpu_update = Instant::now() - Duration::from_secs(1);
    while !stop.load(Ordering::Acquire) {
        if streaming_stop.load(Ordering::Acquire) {
            session.stop_streaming();
            if let Ok(mut value) = status.lock() {
                value["streaming_active"] = json!(false);
            }
        }
        let replay_path = replay_request
            .lock()
            .map_err(|_| "Replay control lock failed")?
            .take();
        if let Some(path) = replay_path {
            let result = session.save_replay(std::path::Path::new(&path));
            if let Ok(mut value) = status.lock() {
                match result {
                    Ok(()) => {
                        value["replay_output"] = json!(path);
                        value["replay_error"] = json!(null);
                    }
                    Err(error) => value["replay_error"] = json!(error.to_string()),
                }
            }
        }
        #[cfg(windows)]
        if request.follow_mouse && !pause.load(Ordering::Acquire) {
            let now = Instant::now();
            let region = viewport.as_mut().ok_or("Missing follow viewport")?.sample(
                sources::local_pointer(source.id),
                now.duration_since(last_pointer).as_secs_f64(),
            );
            last_pointer = now;
            if current_region != Some(region) {
                capture
                    .pan_region(region, [source.width, source.height])
                    .map_err(|e| e.to_string())?;
                current_region = Some(region);
            }
            if let Ok(mut value) = status.lock() {
                value["current_region"] = json!(region);
            }
        }
        #[cfg(windows)]
        let cursor =
            if source.kind == "display" && (request.cursor_highlight || request.click_effects) {
                let position = sources::cursor_position(source.id, current_region);
                if request.click_effects {
                    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
                        GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON,
                    };
                    let down = unsafe {
                        GetAsyncKeyState(i32::from(VK_LBUTTON)) < 0
                            || GetAsyncKeyState(i32::from(VK_RBUTTON)) < 0
                    };
                    if down && !mouse_down {
                        click = position.map(|p| (p, Instant::now()));
                    }
                    mouse_down = down;
                }
                position
            } else {
                None
            };
        #[cfg(windows)]
        let shortcut_layer = if request.shortcut_overlay {
            if last_shortcut_poll.elapsed() >= Duration::from_millis(20) {
                shortcuts.sample(
                    crate::shortcuts::current(),
                    request.shortcut_seconds,
                    request.shortcut_right,
                );
                last_shortcut_poll = Instant::now();
            }
            shortcuts.layer()
        } else {
            None
        };
        #[cfg(windows)]
        let mut gpu_cursor = Vec::new();
        #[cfg(windows)]
        if gpu_overlays {
            if request.cursor_highlight {
                if let Some(position) = cursor {
                    gpu_cursor.push(cursor_ring(position, 18.0, &config, [255, 220, 40]));
                }
            }
            if let Some((position, began)) = click {
                let age = began.elapsed().as_secs_f32();
                if age < 0.4 {
                    gpu_cursor.push(cursor_ring(
                        position,
                        10.0 + age * 80.0,
                        &config,
                        [255, 90, 70],
                    ));
                } else {
                    click = None;
                }
            }
            if last_gpu_update.elapsed() >= Duration::from_secs_f64(1.0 / f64::from(config.fps)) {
                refresh_gpu_overlays(
                    &mut capture,
                    &request,
                    camera.as_ref(),
                    webcam_visible,
                    annotations,
                    shortcut_layer.as_ref(),
                    &gpu_cursor,
                )?;
                last_gpu_update = Instant::now();
            }
        }
        let mut frame = capture.next_frame().map_err(|e| e.to_string())?;
        #[cfg(windows)]
        let audio_packets: Vec<Vec<f32>> = audio
            .iter_mut()
            .map(|a| a.poll())
            .collect::<recorder_engine::Result<_>>()
            .map_err(|e| e.to_string())?;
        if pause.load(Ordering::Acquire) {
            if pause_start.is_none() {
                pause_start = Some(Instant::now());
                update(status, "paused", None, None, None, None);
            }
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        if let Some(began) = pause_start.take() {
            paused += began.elapsed();
            update(status, "recording", None, None, None, None);
        }
        let elapsed = start.elapsed().saturating_sub(paused);
        if request.adaptive_quality && reduce_quality.swap(false, Ordering::AcqRel) {
            next_quality = crate::performance::lower_quality(&config);
            if let Ok(mut value) = status.lock() {
                value["performance_notice"] = json!(if next_quality.is_some() {
                    "Lower quality requested: starting a new MKV part"
                } else {
                    "Already at the minimum adaptive profile"
                });
            }
        }
        if next_quality.is_some()
            || (request.segment_seconds > 0
                && elapsed.as_secs_f64() - segment_start >= f64::from(request.segment_seconds))
        {
            if segment_number >= 1024 {
                if let Ok(mut value) = status.lock() {
                    value["stop_reason"] = json!("Maximum 1024 recovery segments reached");
                }
                break;
            }
            if frame.is_some() {
                #[cfg(windows)]
                if let Some(bus) = &mut mixer {
                    drain_mix(&mut session, bus, elapsed.as_secs_f64(), segment_start)?;
                }
                session.finalize().map_err(|e| e.to_string())?;
                if let Ok(mut value) = status.lock() {
                    value["segment_durations"][current_path.to_string_lossy().as_ref()] =
                        json!(elapsed.as_secs_f64() - segment_start);
                }
                finalized_bytes += current_path.metadata().map(|m| m.len()).unwrap_or(0);
                segment_number += 1;
                let mut filename = config
                    .output
                    .file_stem()
                    .ok_or("Invalid recording filename")?
                    .to_os_string();
                filename.push(format!("-part{segment_number:04}.mkv"));
                current_path = config.output.with_file_name(filename);
                let mut next = config.clone();
                if let Some(quality) = next_quality.take() {
                    next = quality;
                    config = next.clone();
                    // Free the old capture pool before opening a smaller one.
                    drop(capture);
                    capture = Capture::open(&capture_graph(
                        &config,
                        &encoder,
                        &handle,
                        &crop,
                        u8::from(request.capture_cursor || request.follow_mouse),
                    ))
                    .map_err(|e| e.to_string())?;
                    #[cfg(windows)]
                    if gpu_overlays {
                        capture.enable_gpu_overlays().map_err(|e| e.to_string())?;
                        refresh_gpu_overlays(
                            &mut capture,
                            &request,
                            camera.as_ref(),
                            webcam_visible,
                            annotations,
                            shortcut_layer.as_ref(),
                            &gpu_cursor,
                        )?;
                    }
                    let began = Instant::now();
                    if request.follow_mouse {
                        capture
                            .pan_region(
                                current_region.ok_or("Missing region")?,
                                [source.width, source.height],
                            )
                            .map_err(|e| e.to_string())?;
                    }
                    loop {
                        frame = capture.next_frame().map_err(|e| e.to_string())?;
                        if frame.is_some() {
                            break;
                        }
                        if stop.load(Ordering::Acquire) || began.elapsed() > Duration::from_secs(8)
                        {
                            return Err(
                                "Adaptive capture reinitialization canceled or timed out".into()
                            );
                        }
                        thread::sleep(Duration::from_millis(5));
                    }
                    last_pts = (elapsed.as_secs_f64() * f64::from(config.fps)).floor() as i64 - 1;
                    encode_ms = 0.0;
                }
                next.output = current_path.clone();
                session = RecordingSession::encoded_with_audio(
                    &next,
                    &encoder,
                    frame.as_ref().ok_or("No adaptive frame")?,
                    &audio_rates,
                )
                .map_err(|e| e.to_string())?;
                if request.replay_seconds > 0 {
                    session
                        .enable_replay(request.replay_seconds, request.replay_mib)
                        .map_err(|e| e.to_string())?;
                }
                segment_start = elapsed.as_secs_f64();
                if let Ok(mut value) = status.lock() {
                    value["current_output"] = json!(current_path);
                    value["effective_width"] = json!(config.width);
                    value["effective_height"] = json!(config.height);
                    value["effective_fps"] = json!(config.fps);
                    value["effective_bitrate"] = json!(config.bitrate);
                    if let Some(outputs) = value["outputs"].as_array_mut() {
                        outputs.push(json!(current_path));
                    }
                }
            }
        }
        let segment_clock = elapsed.as_secs_f64() - segment_start;
        if last_load.elapsed() >= Duration::from_secs(5) {
            let load = monitor.sample();
            if request.adaptive_quality && next_quality.is_none() {
                next_quality = governor.observe(&config, &load, encode_ms);
            }
            if let Ok(mut value) = status.lock() {
                value["load"] = json!(load);
                if next_quality.is_some() {
                    value["performance_notice"] =
                        json!("Sustained overload: lowering quality in a new MKV part");
                }
            }
            last_load = Instant::now();
        }
        #[cfg(windows)]
        for (index, raw_samples) in audio_packets.iter().enumerate() {
            let mut samples = processors[index]
                .process(raw_samples)
                .map_err(|e| e.to_string())?;
            let settings = controls
                .lock()
                .map_err(|_| "Audio controls lock failed")?
                .clone();
            let system = audio[index].title == "System audio";
            let gain = if system {
                if settings.system_muted {
                    0.0
                } else {
                    settings.system_gain
                }
            } else if settings.microphone_muted {
                0.0
            } else {
                settings.microphone_gain
            };
            for sample in &mut samples {
                *sample = (*sample * gain).clamp(-1.0, 1.0);
            }
            let peak = samples.iter().fold(0.0f32, |max, s| max.max(s.abs()));
            let rms = if samples.is_empty() {
                0.0
            } else {
                (samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len() as f64)
                    .sqrt()
            };
            if let Ok(mut value) = status.lock() {
                let key = if system {
                    "system_peak"
                } else {
                    "microphone_peak"
                };
                let previous = value[key].as_f64().unwrap_or(0.0) as f32;
                value[key] = json!(peak.max(previous * 0.96));
                value[if system {
                    "system_rms"
                } else {
                    "microphone_rms"
                }] = json!(rms);
            }
            if let Some(bus) = &mut mixer {
                bus.push(index, &samples).map_err(|e| e.to_string())?;
            }
            session
                .write_audio(index + usize::from(mix_enabled), &samples, segment_clock)
                .map_err(|e| e.to_string())?;
        }
        #[cfg(windows)]
        if let Some(bus) = &mut mixer {
            drain_mix(&mut session, bus, elapsed.as_secs_f64(), segment_start)?;
        }
        if silent_stream_audio {
            session
                .write_audio(0, &[], elapsed.as_secs_f64())
                .map_err(|e| e.to_string())?;
        }
        let pts = (elapsed.as_secs_f64() * f64::from(config.fps)).floor() as i64;
        if repeat_overlays {
            if let Some(base) = &frame {
                last_screen = Some(base.try_clone().map_err(|e| e.to_string())?);
            }
            if frame.is_none() && pts > last_pts {
                frame = last_screen
                    .as_ref()
                    .map(|base| base.try_clone())
                    .transpose()
                    .map_err(|e| e.to_string())?;
            }
        }
        if let Some(mut frame) = frame {
            if pts > last_pts {
                skipped += (pts - last_pts - 1) as u64;
                let encoding_started = Instant::now();
                if !gpu_overlays {
                    if let Some(camera) = camera
                        .as_ref()
                        .filter(|_| webcam_visible.load(Ordering::Acquire))
                    {
                        if let Ok(image) = camera.latest.lock() {
                            if let Some(image) = image
                                .as_ref()
                                .filter(|v| v.captured.elapsed() < Duration::from_secs(2))
                            {
                                frame
                                    .webcam(image, &request.webcam.placement())
                                    .map_err(|e| e.to_string())?;
                            }
                        }
                    }
                    if request.annotations_enabled {
                        let marks = annotations.lock().map_err(|_| "Annotation lock failed")?;
                        frame.annotate(&marks).map_err(|e| e.to_string())?;
                    }
                    if request.layers_enabled {
                        frame
                            .raster_layers(&crate::layers::snapshot()?)
                            .map_err(|e| e.to_string())?;
                    }
                    #[cfg(windows)]
                    if let Some(layer) = &shortcut_layer {
                        frame
                            .raster_layers(std::slice::from_ref(layer))
                            .map_err(|e| e.to_string())?;
                    }
                    #[cfg(windows)]
                    {
                        if request.cursor_highlight {
                            if let Some(position) = cursor {
                                frame
                                    .annotate(&[cursor_ring(
                                        position,
                                        18.0,
                                        &config,
                                        [255, 220, 40],
                                    )])
                                    .map_err(|e| e.to_string())?;
                            }
                        }
                        if let Some((position, began)) = click {
                            let age = began.elapsed().as_secs_f32();
                            if age < 0.4 {
                                frame
                                    .annotate(&[cursor_ring(
                                        position,
                                        10.0 + age * 80.0,
                                        &config,
                                        [255, 90, 70],
                                    )])
                                    .map_err(|e| e.to_string())?;
                            } else {
                                click = None;
                            }
                        }
                    }
                }
                session
                    .write_video(
                        &mut frame,
                        segment_video_pts(pts, segment_start, config.fps),
                    )
                    .map_err(|e| e.to_string())?;
                encode_ms =
                    encode_ms * 0.9 + encoding_started.elapsed().as_secs_f64() * 1000.0 * 0.1;
                frames += 1;
                last_pts = pts;
            }
        } else {
            thread::sleep(Duration::from_millis(2));
        }
        if last_event.elapsed() >= Duration::from_millis(200) {
            if request.shortcut_overlay {
                if let Ok(mut value) = status.lock() {
                    value["shortcut_label"] = json!(shortcuts.label());
                }
            }
            if let Some(camera) = &camera {
                if let Ok(mut value) = status.lock() {
                    value["webcam_error"] = json!(camera.error.lock().ok().and_then(|v| v.clone()));
                    value["webcam_visible"] = json!(webcam_visible.load(Ordering::Acquire));
                    value["webcam_ready"] = json!(camera.latest.lock().ok().is_some_and(|v| v
                        .as_ref()
                        .is_some_and(|image| image.captured.elapsed() < Duration::from_secs(2))));
                }
            }
            let bytes = finalized_bytes + current_path.metadata().map(|m| m.len()).unwrap_or(0);
            if let Ok(mut value) = status.lock() {
                value["file_bytes"] = json!(bytes);
                value["streaming"] = stream_status(&session);
                value["skipped_frame_slots"] = json!(skipped);
                value["encode_ms"] = json!(encode_ms);
                let (seconds, bytes) = session.replay_status();
                value["replay_seconds"] = json!(seconds);
                value["replay_bytes"] = json!(bytes);
                value["replay_enabled"] = json!(request.replay_seconds > 0);
            }
            if (request.max_seconds > 0 && elapsed.as_secs_f64() >= f64::from(request.max_seconds))
                || (request.max_file_mib > 0
                    && bytes >= u64::from(request.max_file_mib) * 1024 * 1024)
            {
                if let Ok(mut value) = status.lock() {
                    value["stop_reason"] = json!("Configured recording limit reached");
                }
                stop.store(true, Ordering::Release);
            }
            update(
                status,
                "recording",
                None,
                None,
                Some(frames),
                Some(elapsed.as_secs_f64()),
            );
            last_event = Instant::now();
        }
    }
    update(status, "stopping", None, None, None, None);
    let final_clock = active_duration(
        start.elapsed(),
        paused,
        pause_start.map(|p| p.elapsed()).unwrap_or_default(),
    )
    .as_secs_f64();
    #[cfg(windows)]
    for (index, processor) in processors.iter_mut().enumerate() {
        let mut tail = processor.finish().map_err(|e| e.to_string())?;
        let settings = controls
            .lock()
            .map_err(|_| "Audio controls lock failed")?
            .clone();
        let (gain, muted) = if audio[index].title == "System audio" {
            (settings.system_gain, settings.system_muted)
        } else {
            (settings.microphone_gain, settings.microphone_muted)
        };
        for sample in &mut tail {
            *sample = if muted {
                0.0
            } else {
                (*sample * gain).clamp(-1.0, 1.0)
            };
        }
        if let Some(bus) = &mut mixer {
            bus.push(index, &tail).map_err(|e| e.to_string())?;
        }
        session
            .write_audio(
                index + usize::from(mix_enabled),
                &tail,
                final_clock - segment_start,
            )
            .map_err(|e| e.to_string())?;
    }
    #[cfg(windows)]
    if let Some(bus) = &mut mixer {
        let clock = final_clock + 0.05;
        drain_mix(&mut session, bus, clock, segment_start)?;
    }
    let mut streams = stream_status(&session);
    if silent_stream_audio {
        session
            .write_audio(0, &[], final_clock)
            .map_err(|e| e.to_string())?;
    }
    session.finish().map_err(|e| e.to_string())?;
    if let Ok(mut value) = status.lock() {
        if let Some(items) = streams.as_array_mut() {
            for item in items {
                item["state"] = json!("stopped");
            }
        }
        value["streaming"] = streams;
        value["streaming_active"] = json!(false);
        value["segment_durations"][current_path.to_string_lossy().as_ref()] =
            json!((final_clock - segment_start).max(0.0));
    }
    update(
        status,
        "stopped",
        None,
        None,
        Some(frames),
        Some(final_clock),
    );
    Ok(())
}

fn stream_status(session: &RecordingSession) -> serde_json::Value {
    json!(session
        .streaming_status()
        .iter()
        .map(|s| json!({
            "platform": s.platform, "state": s.state, "packets": s.packets,
            "bytes": s.bytes, "reconnects": s.reconnects, "dropped_packets": s.dropped_packets,
        "error": s.error, "error_code": s.error_code, "error_operation": s.error_operation,
        }))
        .collect::<Vec<_>>())
}

/// Quantize the segment origin once. Rounding a fractional elapsed delta on
/// every frame can produce duplicate PTS even when absolute ticks increase.
fn segment_video_pts(absolute: i64, origin: f64, fps: u32) -> i64 {
    absolute - (origin * f64::from(fps)).floor() as i64
}

fn active_duration(elapsed: Duration, paused: Duration, current_pause: Duration) -> Duration {
    elapsed.saturating_sub(paused.saturating_add(current_pause))
}

/// Camera bytes are borrowed under a short latest-frame lock only for a small
/// texture upload. No camera read/decode or full-screen CPU blending runs here.
#[cfg(windows)]
fn refresh_gpu_overlays(
    capture: &mut Capture,
    request: &Request,
    camera: Option<&recorder_engine::webcam::Camera>,
    visible: &AtomicBool,
    marks: &Mutex<Vec<recorder_engine::overlay::Annotation>>,
    shortcut: Option<&recorder_engine::layers::Layer>,
    cursor: &[recorder_engine::overlay::Annotation],
) -> Result<(), String> {
    let mut layers = if request.layers_enabled {
        crate::layers::snapshot()?
    } else {
        Vec::new()
    };
    if let Some(layer) = shortcut {
        layers.push(layer.clone());
    }
    let annotations = marks.lock().map_err(|_| "Annotation lock failed")?;
    let annotations = if request.annotations_enabled {
        annotations.as_slice()
    } else {
        &[]
    };
    let camera = camera.filter(|_| visible.load(Ordering::Acquire));
    let image = camera.and_then(|c| c.latest.lock().ok());
    let place = request.webcam.placement();
    let image = image
        .as_ref()
        .and_then(|i| i.as_ref())
        .filter(|i| i.captured.elapsed() < Duration::from_secs(2));
    capture
        .update_gpu_overlays(&layers, annotations, cursor, image.map(|i| (i, &place)))
        .map_err(|e| e.to_string())
}

fn capture_graph(
    config: &RecordingConfig,
    name: &str,
    handle: &str,
    crop: &str,
    cursor: u8,
) -> String {
    // Scale the BGRA GPU canvas before any readback. A 4K source recorded at
    // 720p transfers only the 720p canvas to RAM on software/QSV paths.
    let source=format!("gfxcapture={handle}:capture_border=1:max_framerate={}:width={}:height={}:capture_cursor={cursor}:resize_mode=scale_aspect{crop}",config.fps,config.width,config.height);
    match name {
        "libx264" => format!("{source},hwdownload,format=bgra,format=yuv420p"),
        "h264_qsv" => format!("{source},hwdownload,format=bgra,format=nv12"),
        "gif" => format!("{source},hwdownload,format=bgra,format=rgb8"),
        _ => format!(
            "{source},scale_d3d11=width={}:height={}:format=nv12",
            config.width, config.height
        ),
    }
}

#[cfg(windows)]
fn drain_mix(
    session: &mut RecordingSession,
    bus: &mut recorder_engine::audio_processor::AudioMixer,
    clock: f64,
    segment_start: f64,
) -> Result<(), String> {
    while clock - bus.clock() >= 1.0 / 48000.0 {
        let samples = bus.until(clock).map_err(|e| e.to_string())?;
        if samples.is_empty() {
            break;
        }
        session
            .write_audio(0, &samples, (bus.clock() - segment_start).max(0.0))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(windows)]
fn cursor_ring(
    position: [f32; 2],
    radius: f32,
    config: &RecordingConfig,
    rgb: [u8; 3],
) -> recorder_engine::overlay::Annotation {
    recorder_engine::overlay::Annotation {
        shape: recorder_engine::overlay::Shape::Stroke,
        points: (0..=32)
            .map(|i| {
                let angle = i as f32 * std::f32::consts::TAU / 32.0;
                [
                    (position[0] + angle.cos() * radius / config.width as f32).clamp(0.0, 1.0),
                    (position[1] + angle.sin() * radius / config.height as f32).clamp(0.0, 1.0),
                ]
            })
            .collect(),
        rgb,
        width: 2,
    }
}

#[cfg(test)]
mod tests {
    use super::{active_duration, segment_video_pts};
    use std::time::Duration;
    #[test]
    fn fractional_segment_origin_keeps_strict_frame_order() {
        let times = [2.03f64, 2.04, 2.08];
        let actual: Vec<_> = times
            .into_iter()
            .map(|time| segment_video_pts((time * 30.0).floor() as i64, 2.013, 30))
            .collect();
        assert_eq!(actual, vec![0, 1, 2]);
    }
    #[test]
    fn stopping_during_pause_excludes_the_unfinished_pause() {
        assert_eq!(
            active_duration(
                Duration::from_secs(10),
                Duration::from_secs(2),
                Duration::from_secs(5)
            ),
            Duration::from_secs(3)
        );
    }
}
