//! Fixed-size, physical-pixel viewport. Pointer samples are supplied by the
//! native capture owner; this math never moves the system pointer.
pub struct Viewport {
    position: [f64; 2],
    region: [u32; 4],
    bounds: [u32; 2],
}
impl Viewport {
    pub fn new(region: [u32; 4], bounds: [u32; 2]) -> Self {
        Self {
            position: [region[0] as f64, region[1] as f64],
            region,
            bounds,
        }
    }
    /// Ease toward the pointer, but keep it visible even after a fast move.
    /// Samples outside the selected monitor leave the viewport in place.
    pub fn sample(&mut self, pointer: Option<[f64; 2]>, seconds: f64) -> [u32; 4] {
        if let Some(pointer) = pointer {
            let alpha = 1.0 - (-seconds.clamp(0.0, 0.25) / 0.16).exp();
            for (axis, value) in pointer.into_iter().enumerate() {
                if !value.is_finite() || value < 0.0 || value >= self.bounds[axis] as f64 {
                    continue;
                }
                let size = self.region[axis + 2] as f64;
                let max = (self.bounds[axis] - self.region[axis + 2]) as f64;
                let target = (value - size / 2.0).clamp(0.0, max);
                self.position[axis] += (target - self.position[axis]) * alpha;
                self.position[axis] =
                    self.position[axis].clamp((value - size + 1.0).max(0.0), value.min(max));
                self.region[axis] = self.position[axis].round().clamp(0.0, max) as u32;
            }
        }
        self.region
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn motion_preserves_size_contains_pointer_and_stays_inside_monitor() {
        let mut view = Viewport::new([0, 0, 640, 360], [1920, 1080]);
        for p in [
            [1919.0, 1079.0],
            [0.0, 0.0],
            [960.0, 540.0],
            [1800.0, 100.0],
        ] {
            let [x, y, w, h] = view.sample(Some(p), 1.0 / 60.0);
            assert_eq!([w, h], [640, 360]);
            assert!(x + w <= 1920 && y + h <= 1080);
            assert!(p[0] >= x as f64 && p[0] < (x + w) as f64);
            assert!(p[1] >= y as f64 && p[1] < (y + h) as f64);
        }
    }
    #[test]
    fn absent_pointer_freezes_and_full_monitor_cannot_pan() {
        let mut view = Viewport::new([0, 0, 1920, 1080], [1920, 1080]);
        assert_eq!(view.sample(Some([1919.0, 1079.0]), 0.1), [0, 0, 1920, 1080]);
        assert_eq!(view.sample(None, 0.1), [0, 0, 1920, 1080]);
    }
    #[test]
    fn gentle_motion_is_smoothed() {
        let mut view = Viewport::new([500, 300, 640, 360], [1920, 1080]);
        let result = view.sample(Some([900.0, 540.0]), 1.0 / 60.0);
        assert!(result[0] > 500 && result[0] < 580);
        assert!(result[1] > 300 && result[1] < 360);
    }
}
