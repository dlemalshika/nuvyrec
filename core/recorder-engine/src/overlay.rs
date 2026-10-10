//! Bounded CPU annotation compositor, used only when explicitly enabled.
//! Coordinates are normalized to the encoded frame, independent of UI size.
use crate::{error::check, Error, Result, VideoFrame};
use ffmpeg_sys_next as av;

#[derive(Clone)]
pub enum Shape {
    Stroke,
    Arrow,
    Box,
    Spotlight,
}
#[derive(Clone)]
pub struct Annotation {
    pub shape: Shape,
    pub points: Vec<[f32; 2]>,
    pub rgb: [u8; 3],
    pub width: u32,
}
impl VideoFrame {
    /// GPU surfaces are deliberately rejected. The worker chooses a CPU
    /// YUV420P path before enabling this compositor; no implicit GPU readback.
    pub fn annotate(&mut self, annotations: &[Annotation]) -> Result<()> {
        if annotations.len() > 64 {
            return Err(Error::Invalid("too many annotations"));
        }
        unsafe {
            if (*self.raw).format != av::AVPixelFormat::AV_PIX_FMT_YUV420P as i32 {
                return Err(Error::Invalid("annotations require CPU YUV420P"));
            }
            check(
                av::av_frame_make_writable(self.raw),
                "prepare annotation frame",
            )?;
        }
        let w = unsafe { (*self.raw).width };
        let h = unsafe { (*self.raw).height };
        for annotation in annotations {
            if annotation.points.is_empty()
                || annotation.points.len() > 256
                || !(1..=16).contains(&annotation.width)
                || annotation
                    .points
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            {
                return Err(Error::Invalid("invalid annotation geometry"));
            }
            let points: Vec<_> = annotation
                .points
                .iter()
                .map(|p| {
                    (
                        (p[0] * (w - 1) as f32) as i32,
                        (p[1] * (h - 1) as f32) as i32,
                    )
                })
                .collect();
            let [r, g, b] = annotation.rgb.map(f32::from);
            // Limited-range BT.709 matches the recording's color metadata.
            let color = [
                (16.0 + 0.1826 * r + 0.6142 * g + 0.0620 * b)
                    .round()
                    .clamp(16.0, 235.0) as u8,
                (128.0 - 0.1006 * r - 0.3386 * g + 0.4392 * b)
                    .round()
                    .clamp(16.0, 240.0) as u8,
                (128.0 + 0.4392 * r - 0.3989 * g - 0.0403 * b)
                    .round()
                    .clamp(16.0, 240.0) as u8,
            ];
            match annotation.shape {
                Shape::Spotlight => {
                    let (cx, cy) = points[0];
                    let radius = w.min(h) / 7;
                    for y in 0..h {
                        for x in 0..w {
                            if (x - cx).pow(2) + (y - cy).pow(2) > radius.pow(2) {
                                unsafe {
                                    let pixel = (*self.raw).data[0]
                                        .offset((y * (*self.raw).linesize[0] + x) as isize);
                                    *pixel = 16 + ((*pixel).saturating_sub(16) as u16 / 3) as u8;
                                }
                            }
                        }
                    }
                }
                Shape::Box if points.len() >= 2 => {
                    let a = points[0];
                    let b = *points.last().unwrap();
                    for (from, to) in [
                        (a, (b.0, a.1)),
                        ((b.0, a.1), b),
                        (b, (a.0, b.1)),
                        ((a.0, b.1), a),
                    ] {
                        self.overlay_line(from, to, color, annotation.width as i32);
                    }
                }
                Shape::Arrow if points.len() >= 2 => {
                    let a = points[0];
                    let b = *points.last().unwrap();
                    self.overlay_line(a, b, color, annotation.width as i32);
                    let angle = ((b.1 - a.1) as f32).atan2((b.0 - a.0) as f32);
                    let length = (w.min(h) as f32 * 0.04).clamp(8.0, 60.0);
                    for delta in [-0.5f32, 0.5] {
                        let tip = (
                            b.0 - (length * (angle + delta).cos()) as i32,
                            b.1 - (length * (angle + delta).sin()) as i32,
                        );
                        self.overlay_line(b, tip, color, annotation.width as i32);
                    }
                }
                _ => {
                    if points.len() == 1 {
                        self.overlay_line(points[0], points[0], color, annotation.width as i32);
                    }
                    for pair in points.windows(2) {
                        self.overlay_line(pair[0], pair[1], color, annotation.width as i32);
                    }
                }
            }
        }
        Ok(())
    }
    fn overlay_line(&mut self, from: (i32, i32), to: (i32, i32), color: [u8; 3], width: i32) {
        let (mut x, mut y) = from;
        let dx = (to.0 - x).abs();
        let sx = if x < to.0 { 1 } else { -1 };
        let dy = -(to.1 - y).abs();
        let sy = if y < to.1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            unsafe {
                for oy in -(width / 2)..=(width / 2) {
                    for ox in -(width / 2)..=(width / 2) {
                        let px = x + ox;
                        let py = y + oy;
                        if px < 0 || py < 0 || px >= (*self.raw).width || py >= (*self.raw).height {
                            continue;
                        }
                        for (plane, value) in color.iter().enumerate() {
                            let (cx, cy) = if plane == 0 {
                                (px, py)
                            } else {
                                (px / 2, py / 2)
                            };
                            *(*self.raw).data[plane]
                                .offset((cy * (*self.raw).linesize[plane] + cx) as isize) = *value;
                        }
                    }
                }
            }
            if (x, y) == to {
                break;
            }
            let twice = 2 * error;
            if twice >= dy {
                error += dy;
                x += sx;
            }
            if twice <= dx {
                error += dx;
                y += sy;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_geometry_and_does_not_modify_encoder_retained_frame() {
        let mut frame = VideoFrame::new(64, 64).unwrap();
        let original = unsafe { av::av_frame_clone(frame.raw) };
        let stroke = Annotation {
            shape: Shape::Stroke,
            points: vec![[0.0, 0.0], [1.0, 1.0]],
            rgb: [255, 0, 0],
            width: 3,
        };
        frame.annotate(std::slice::from_ref(&stroke)).unwrap();
        unsafe {
            assert_ne!(*(*frame.raw).data[0], 16);
            assert_eq!(*(*original).data[0], 16);
            let mut original = original;
            av::av_frame_free(&mut original);
        }
        let mut bad = stroke;
        bad.points[0][0] = f32::NAN;
        assert!(frame.annotate(&[bad]).is_err());
    }
}
