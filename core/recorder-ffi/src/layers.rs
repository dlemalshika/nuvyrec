//! Four-slot cache shared by the UI uploader and recording owner. Replacement
//! is atomic: already encoded frames and current readers retain their Arc.
use recorder_engine::layers::{Layer, Raster, MAX_BANK_BYTES, MAX_LAYERS};
use serde::Deserialize;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
static BANK: OnceLock<Mutex<Vec<Option<Layer>>>> = OnceLock::new();
fn bank() -> &'static Mutex<Vec<Option<Layer>>> {
    BANK.get_or_init(|| Mutex::new(vec![None; MAX_LAYERS]))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Upload {
    #[serde(default)]
    pub transaction: Option<u64>,
    pub id: usize,
    pub pixel_width: usize,
    pub pixel_height: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub opacity: f32,
}
pub fn upload(meta: Upload, rgba: &[u8]) -> Result<(), String> {
    if meta.id >= MAX_LAYERS {
        return Err("Overlay slot out of range".into());
    }
    let layer = Layer {
        raster: Arc::new(
            Raster::rgba(meta.pixel_width, meta.pixel_height, rgba).map_err(|e| e.to_string())?,
        ),
        x: meta.x,
        y: meta.y,
        width: meta.width,
        opacity: meta.opacity,
    };
    if !layer.valid() {
        return Err("Invalid overlay placement".into());
    }
    let mut staging = scenes().lock().map_err(|_| "Scene lock failed")?;
    let mut current = bank().lock().map_err(|_| "Overlay cache lock failed")?;
    let value = match meta.transaction {
        Some(token) => {
            &mut staging
                .stage
                .as_mut()
                .filter(|s| s.0 == token)
                .ok_or("Expired scene transaction")?
                .1
        }
        None => &mut *current,
    };
    let bytes = value
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != meta.id)
        .filter_map(|(_, l)| l.as_ref())
        .map(|l| l.raster.bytes())
        .sum::<usize>()
        + layer.raster.bytes();
    if bytes > MAX_BANK_BYTES {
        return Err("Overlay cache exceeds 8 MiB (RGBA and fallback colors)".into());
    }
    value[meta.id] = Some(layer);
    Ok(())
}
pub fn clear() -> Result<(), String> {
    let mut s = scenes().lock().map_err(|_| "Scene lock failed")?;
    let serial = s.serial;
    *s = Scenes {
        serial,
        ..Scenes::default()
    };
    bank()
        .lock()
        .map_err(|_| "Overlay cache lock failed")?
        .fill(None);
    Ok(())
}
pub fn snapshot() -> Result<Vec<Layer>, String> {
    tick(Instant::now())?;
    Ok(bank()
        .lock()
        .map_err(|_| "Overlay cache lock failed")?
        .iter()
        .flatten()
        .cloned()
        .collect())
}

/// At most one staged scene and one transition. RGBA/CPU caches stay bounded
/// to two 8 MiB banks; a fade uses the existing output canvas, never two videos.
#[derive(Default)]
struct Scenes {
    serial: u64,
    stage: Option<(u64, Vec<Option<Layer>>)>,
    name: String,
    webcam: Option<crate::worker::WebcamRequest>,
    transition: Option<Transition>,
}
struct Transition {
    began: Instant,
    duration: Duration,
    next: Option<(Vec<Option<Layer>>, SceneCommit)>,
}
static SCENES: OnceLock<Mutex<Scenes>> = OnceLock::new();
fn scenes() -> &'static Mutex<Scenes> {
    SCENES.get_or_init(|| Mutex::new(Scenes::default()))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneCommit {
    pub transaction: u64,
    pub count: usize,
    pub name: String,
    pub duration_ms: u64,
    pub webcam: Option<crate::worker::WebcamRequest>,
}
pub fn begin() -> Result<u64, String> {
    tick(Instant::now())?;
    let mut s = scenes().lock().map_err(|_| "Scene lock failed")?;
    if s.transition.is_some() {
        return Err("Wait for the current scene transition".into());
    }
    s.serial = s
        .serial
        .checked_add(1)
        .ok_or("Scene transaction exhausted")?;
    let token = s.serial;
    s.stage = Some((token, vec![None; MAX_LAYERS]));
    Ok(token)
}
pub fn abort(token: u64) -> Result<(), String> {
    let mut s = scenes().lock().map_err(|_| "Scene lock failed")?;
    if s.stage.as_ref().is_some_and(|v| v.0 == token) {
        s.stage = None;
    }
    Ok(())
}
pub fn commit(value: SceneCommit) -> Result<(), String> {
    if value.name.is_empty()
        || value.name.chars().count() > 48
        || value.name.contains('\0')
        || value.duration_ms > 1000
        || value.count > MAX_LAYERS
        || value
            .webcam
            .as_ref()
            .is_some_and(|c| !c.placement().valid())
    {
        return Err("Invalid scene settings".into());
    }
    tick(Instant::now())?;
    let mut s = scenes().lock().map_err(|_| "Scene lock failed")?;
    if s.transition.is_some() {
        return Err("Wait for the current scene transition".into());
    }
    let stage = s
        .stage
        .as_ref()
        .filter(|v| v.0 == value.transaction)
        .ok_or("Expired scene transaction")?;
    if stage
        .1
        .iter()
        .enumerate()
        .any(|(i, v)| v.is_some() != (i < value.count))
    {
        return Err("Scene assets are incomplete".into());
    }
    let (_, layers) = s.stage.take().ok_or("Scene assets unavailable")?;
    if value.duration_ms == 0 {
        *bank().lock().map_err(|_| "Overlay cache lock failed")? = layers;
        s.name = value.name;
        s.webcam = value.webcam;
    } else {
        s.transition = Some(Transition {
            began: Instant::now(),
            duration: Duration::from_millis(value.duration_ms),
            next: Some((layers, value)),
        });
    }
    Ok(())
}
fn fade_at(age: Duration, duration: Duration) -> f32 {
    let progress = (age.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
    1.0 - (progress * 2.0 - 1.0).abs()
}
fn tick(now: Instant) -> Result<f32, String> {
    let mut s = scenes().lock().map_err(|_| "Scene lock failed")?;
    let Some(t) = &mut s.transition else {
        return Ok(0.0);
    };
    let age = now.saturating_duration_since(t.began);
    let fade = fade_at(age, t.duration);
    let next = if age >= t.duration / 2 {
        t.next.take()
    } else {
        None
    };
    let done = age >= t.duration;
    if let Some((layers, value)) = next {
        *bank().lock().map_err(|_| "Overlay cache lock failed")? = layers;
        s.name = value.name;
        s.webcam = value.webcam;
    }
    if done {
        s.transition = None;
    }
    Ok(fade)
}
pub fn visual() -> Result<(f32, Option<crate::worker::WebcamRequest>, String), String> {
    let fade = tick(Instant::now())?;
    let s = scenes().lock().map_err(|_| "Scene lock failed")?;
    Ok((fade, s.webcam.clone(), s.name.clone()))
}
pub fn camera_visible(visible: bool) -> Result<(), String> {
    let mut s = scenes().lock().map_err(|_| "Scene lock failed")?;
    if let Some(camera) = &mut s.webcam {
        camera.enabled = visible;
    }
    Ok(())
}
pub fn transitioning() -> Result<bool, String> {
    tick(Instant::now())?;
    Ok(scenes()
        .lock()
        .map_err(|_| "Scene lock failed")?
        .transition
        .is_some())
}
pub struct View {
    pub layers: Vec<Layer>,
    pub fade: f32,
    pub webcam: Option<crate::worker::WebcamRequest>,
}
pub fn view() -> Result<View, String> {
    let fade = tick(Instant::now())?;
    let s = scenes().lock().map_err(|_| "Scene lock failed")?;
    let layers = bank()
        .lock()
        .map_err(|_| "Overlay cache lock failed")?
        .iter()
        .flatten()
        .cloned()
        .collect();
    Ok(View {
        layers,
        fade,
        webcam: s.webcam.clone(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_over_budget_upload_atomically_and_clear_releases_cache() {
        clear().unwrap();
        let bytes = vec![255; 1024 * 512 * 4];
        let meta = |id| Upload {
            transaction: None,
            id,
            pixel_width: 1024,
            pixel_height: 512,
            x: 0.0,
            y: 0.0,
            width: 0.2,
            opacity: 1.0,
        };
        upload(meta(0), &bytes).unwrap();
        upload(meta(1), &bytes).unwrap();
        assert!(upload(meta(2), &bytes).is_err());
        assert_eq!(snapshot().unwrap().len(), 2);
        assert!(upload(meta(4), &bytes).is_err());
        let token = begin().unwrap();
        let commit_value = |count| SceneCommit {
            transaction: token,
            count,
            name: "Test scene".into(),
            duration_ms: 0,
            webcam: None,
        };
        assert!(commit(commit_value(1)).is_err());
        assert_eq!(snapshot().unwrap().len(), 2); // partial upload never clears on-air assets
        let mut staged = meta(0);
        staged.transaction = Some(token);
        upload(staged, &bytes).unwrap();
        assert_eq!(snapshot().unwrap().len(), 2);
        commit(commit_value(1)).unwrap();
        assert_eq!(snapshot().unwrap().len(), 1);
        assert_eq!(visual().unwrap().2, "Test scene");
        assert!(commit(commit_value(1)).is_err()); // tokens are single use
        let newer = begin().unwrap();
        abort(token).unwrap(); // stale abort cannot discard another transaction
        commit(SceneCommit {
            transaction: newer,
            count: 0,
            name: "Clean scene".into(),
            duration_ms: 0,
            webcam: None,
        })
        .unwrap();
        assert!(snapshot().unwrap().is_empty());
        clear().unwrap();
        assert!(snapshot().unwrap().is_empty());
    }
    #[test]
    fn dip_to_black_has_one_midpoint_and_no_extra_canvas() {
        let duration = Duration::from_millis(600);
        for (ms, expected) in [
            (0, 0.0),
            (150, 0.5),
            (300, 1.0),
            (450, 0.5),
            (600, 0.0),
            (900, 0.0),
        ] {
            assert!((fade_at(Duration::from_millis(ms), duration) - expected).abs() < 0.001);
        }
    }
}
