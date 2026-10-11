//! Small, cached raster overlays. Unicode shaping belongs to the native Flutter
//! UI; the media worker receives an owned straight-alpha RGBA image once and
//! converts it to limited-range BT.709 YUV once, never once per video frame.
use crate::{error::check, Error, Result, VideoFrame};
use ffmpeg_sys_next as av;
use std::sync::Arc;

pub const MAX_LAYERS: usize = 4;
// Reserve both the straight-alpha GPU upload and cached CPU fallback colors.
pub const MAX_BANK_BYTES: usize = 8 * 1024 * 1024;
pub struct Raster {
    pub width: usize,
    pub height: usize,
    yuva: Vec<[u8; 4]>,
    rgba: Vec<u8>,
}
impl Raster {
    pub fn rgba(width: usize, height: usize, bytes: &[u8]) -> Result<Self> {
        if width == 0
            || height == 0
            || width > 1024
            || height > 512
            || bytes.len() != width * height * 4
        {
            return Err(Error::Invalid(
                "overlay must be a tightly packed RGBA image up to 1024 x 512",
            ));
        }
        let yuva = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                let [r, g, b] = [p[0], p[1], p[2]].map(f32::from);
                [
                    (16.0 + 0.1826 * r + 0.6142 * g + 0.0620 * b)
                        .round()
                        .clamp(16.0, 235.0) as u8,
                    (128.0 - 0.1006 * r - 0.3386 * g + 0.4392 * b)
                        .round()
                        .clamp(16.0, 240.0) as u8,
                    (128.0 + 0.4392 * r - 0.3989 * g - 0.0403 * b)
                        .round()
                        .clamp(16.0, 240.0) as u8,
                    p[3],
                ]
            })
            .collect();
        Ok(Self {
            width,
            height,
            yuva,
            rgba: bytes.to_vec(),
        })
    }
    pub fn bytes(&self) -> usize {
        self.yuva.len() * 4 + self.rgba.len()
    }
    pub(crate) fn gpu_rgba(&self) -> &[u8] {
        &self.rgba
    }
}
/// x/y are anchors in remaining free space (0 = left/top, 1 = right/bottom).
#[derive(Clone)]
pub struct Layer {
    pub raster: Arc<Raster>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub opacity: f32,
}
impl Layer {
    pub fn valid(&self) -> bool {
        [self.x, self.y, self.width, self.opacity]
            .iter()
            .all(|v| v.is_finite())
            && (0.0..=1.0).contains(&self.x)
            && (0.0..=1.0).contains(&self.y)
            && (0.02..=1.0).contains(&self.width)
            && (0.0..=1.0).contains(&self.opacity)
    }
}
impl VideoFrame {
    /// Blend only each layer's bounding rectangle. Alpha for a chroma sample
    /// averages its four luma pixels, preserving transparent edges on YUV420P.
    pub fn raster_layers(&mut self, layers: &[Layer]) -> Result<()> {
        if layers.len() > MAX_LAYERS || layers.iter().any(|l| !l.valid()) {
            return Err(Error::Invalid("invalid overlay layers"));
        }
        unsafe {
            if (*self.raw).format != av::AVPixelFormat::AV_PIX_FMT_YUV420P as i32 {
                return Err(Error::Invalid("raster overlays require CPU YUV420P"));
            }
            if layers.is_empty() {
                return Ok(());
            }
            check(
                av::av_frame_make_writable(self.raw),
                "prepare raster overlay frame",
            )?;
            let fw = (*self.raw).width as usize;
            let fh = (*self.raw).height as usize;
            for layer in layers {
                let image = &layer.raster;
                let scale = (fw as f32 * layer.width / image.width as f32)
                    .min(fh as f32 / image.height as f32);
                let w = (image.width as f32 * scale).round().clamp(1.0, fw as f32) as usize;
                let h = (image.height as f32 * scale).round().clamp(1.0, fh as f32) as usize;
                // Even anchors make each 2x2 chroma group local to this layer.
                let left = ((fw - w) as f32 * layer.x).round() as usize & !1;
                let top = ((fh - h) as f32 * layer.y).round() as usize & !1;
                for cy in (0..h).step_by(2) {
                    for cx in (0..w).step_by(2) {
                        let mut sum_alpha = 0.0;
                        let mut chroma = [0.0; 2];
                        for dy in 0..2 {
                            for dx in 0..2 {
                                let x = cx + dx;
                                let y = cy + dy;
                                if x >= w || y >= h {
                                    continue;
                                }
                                let p = image.yuva
                                    [(y * image.height / h) * image.width + x * image.width / w];
                                let a = p[3] as f32 / 255.0 * layer.opacity;
                                if a == 0.0 {
                                    continue;
                                }
                                sum_alpha += a;
                                chroma[0] += p[1] as f32 * a;
                                chroma[1] += p[2] as f32 * a;
                                let dst = (*self.raw).data[0]
                                    .add((top + y) * (*self.raw).linesize[0] as usize + left + x);
                                *dst = (*dst as f32 * (1.0 - a) + p[0] as f32 * a).round() as u8;
                            }
                        }
                        if sum_alpha == 0.0 {
                            continue;
                        }
                        for plane in 1..=2 {
                            let dst = (*self.raw).data[plane].add(
                                ((top + cy) / 2) * (*self.raw).linesize[plane] as usize
                                    + (left + cx) / 2,
                            );
                            *dst = (*dst as f32 * (1.0 - sum_alpha / 4.0) + chroma[plane - 1] / 4.0)
                                .round() as u8;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transparent_pixels_and_encoder_retained_frames_are_preserved() {
        let mut frame = VideoFrame::new(8, 8).unwrap();
        let original = frame.try_clone().unwrap();
        let raster = Arc::new(
            Raster::rgba(
                2,
                2,
                &[
                    255, 0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 128, 255, 255, 255, 0,
                ],
            )
            .unwrap(),
        );
        let layer = Layer {
            raster,
            x: 1.0,
            y: 1.0,
            width: 0.25,
            opacity: 1.0,
        };
        frame.raster_layers(&[layer]).unwrap();
        unsafe {
            assert_eq!(
                *(*original.raw).data[0].add(6 * (*original.raw).linesize[0] as usize + 6),
                16
            );
            assert!(*(*frame.raw).data[0].add(6 * (*frame.raw).linesize[0] as usize + 6) > 16);
            assert_eq!(
                *(*frame.raw).data[0].add(6 * (*frame.raw).linesize[0] as usize + 7),
                16
            );
        }
    }
    #[test]
    fn rejects_unbounded_assets_and_invalid_placement() {
        assert!(Raster::rgba(1025, 1, &[]).is_err());
        assert!(Raster::rgba(2, 2, &[0; 15]).is_err());
        let mut layer = Layer {
            raster: Arc::new(Raster::rgba(1, 1, &[255; 4]).unwrap()),
            x: 0.0,
            y: 0.0,
            width: 1.0,
            opacity: 1.0,
        };
        layer.opacity = f32::NAN;
        assert!(VideoFrame::new(8, 8)
            .unwrap()
            .raster_layers(&[layer])
            .is_err());
    }
}
