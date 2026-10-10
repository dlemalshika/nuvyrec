//! Four-slot cache shared by the UI uploader and recording owner. Replacement
//! is atomic: already encoded frames and current readers retain their Arc.
use recorder_engine::layers::{Layer, Raster, MAX_BANK_BYTES, MAX_LAYERS};
use serde::Deserialize;
use std::sync::{Arc, Mutex, OnceLock};
static BANK: OnceLock<Mutex<Vec<Option<Layer>>>> = OnceLock::new();
fn bank() -> &'static Mutex<Vec<Option<Layer>>> {
    BANK.get_or_init(|| Mutex::new(vec![None; MAX_LAYERS]))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Upload {
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
    let mut value = bank().lock().map_err(|_| "Overlay cache lock failed")?;
    let bytes = value
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != meta.id)
        .filter_map(|(_, l)| l.as_ref())
        .map(|l| l.raster.bytes())
        .sum::<usize>()
        + layer.raster.bytes();
    if bytes > MAX_BANK_BYTES {
        return Err("Overlay cache exceeds 4 MiB".into());
    }
    value[meta.id] = Some(layer);
    Ok(())
}
pub fn clear() -> Result<(), String> {
    bank()
        .lock()
        .map_err(|_| "Overlay cache lock failed")?
        .fill(None);
    Ok(())
}
pub fn snapshot() -> Result<Vec<Layer>, String> {
    Ok(bank()
        .lock()
        .map_err(|_| "Overlay cache lock failed")?
        .iter()
        .flatten()
        .cloned()
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_over_budget_upload_atomically_and_clear_releases_cache() {
        clear().unwrap();
        let bytes = vec![255; 1024 * 512 * 4];
        let meta = |id| Upload {
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
        clear().unwrap();
        assert!(snapshot().unwrap().is_empty());
    }
}
