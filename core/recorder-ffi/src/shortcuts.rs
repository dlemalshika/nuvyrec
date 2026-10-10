//! Opt-in tutorial HUD: modifier shortcuts/function keys while recording only.
//! No hooks, typed text, clipboard, persistent keyboard history or log files.
use recorder_engine::layers::{Layer, Raster};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
pub struct Tracker {
    held: Option<String>,
    label: Option<String>,
    last: Instant,
    layer: Option<Layer>,
}
impl Tracker {
    pub fn new() -> Self {
        Self {
            held: None,
            label: None,
            last: Instant::now(),
            layer: None,
        }
    }
    pub fn sample(&mut self, chord: Option<String>, seconds: u32, right: bool) -> Option<Layer> {
        if let Some(value) = &chord {
            if self.held.as_ref() != Some(value) {
                self.layer = Some(render(value, right));
                self.label = Some(value.clone());
            }
            self.last = Instant::now();
        }
        self.held = chord;
        if self.last.elapsed() > Duration::from_secs(seconds as u64) {
            self.layer = None;
            self.label = None;
        }
        self.layer.clone()
    }
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    pub fn layer(&self) -> Option<Layer> {
        self.layer.clone()
    }
}
#[cfg(windows)]
pub fn current() -> Option<String> {
    detect(|key| unsafe {
        windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(key as i32) < 0
    })
}
#[cfg(windows)]
fn detect(down: impl Fn(u16) -> bool) -> Option<String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    let ctrl = down(VK_CONTROL);
    let alt = down(VK_MENU);
    let win = down(VK_LWIN) || down(VK_RWIN);
    let shift = down(VK_SHIFT);
    let key = (VK_F1..=VK_F12)
        .find(|k| down(*k))
        .map(|k| format!("F{}", k - VK_F1 + 1))
        .or_else(|| {
            if ctrl || alt || win {
                (0x30u16..=0x39)
                    .chain(0x41..=0x5a)
                    .find(|k| down(*k))
                    .map(|k| (k as u8 as char).to_string())
                    .or_else(|| {
                        [
                            (VK_TAB, "TAB"),
                            (VK_RETURN, "ENTER"),
                            (VK_ESCAPE, "ESC"),
                            (VK_SPACE, "SPACE"),
                            (VK_LEFT, "LEFT"),
                            (VK_RIGHT, "RIGHT"),
                            (VK_UP, "UP"),
                            (VK_DOWN, "DOWN"),
                            (VK_DELETE, "DEL"),
                        ]
                        .into_iter()
                        .find(|(k, _)| down(*k))
                        .map(|(_, s)| s.to_owned())
                    })
            } else {
                None
            }
        })?;
    let mut parts = Vec::new();
    if ctrl {
        parts.push("CTRL");
    }
    if alt {
        parts.push("ALT");
    }
    if win {
        parts.push("WIN");
    }
    if shift {
        parts.push("SHIFT");
    }
    parts.push(&key);
    Some(parts.join(" + "))
}
fn render(label: &str, right: bool) -> Layer {
    let chars: Vec<_> = label.chars().take(40).collect();
    let width = chars.len() * 12 + 16;
    let height = 30;
    let mut pixels = vec![0u8; width * height * 4];
    for p in pixels.as_chunks_mut::<4>().0 {
        p.copy_from_slice(&[12, 18, 24, 220]);
    }
    for (i, ch) in chars.into_iter().enumerate() {
        for (row, bits) in glyph(ch).into_iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                for dy in 0..2 {
                    for dx in 0..2 {
                        let offset = ((8 + row * 2 + dy) * width + 8 + i * 12 + col * 2 + dx) * 4;
                        pixels[offset..offset + 4].copy_from_slice(&[255; 4]);
                    }
                }
            }
        }
    }
    Layer {
        raster: Arc::new(Raster::rgba(width, height, &pixels).expect("bounded shortcut raster")),
        x: if right { 1.0 } else { 0.0 },
        y: 1.0,
        width: 0.35,
        opacity: 1.0,
    }
}
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '+' => [0, 4, 4, 31, 4, 4, 0],
        _ => [0; 7],
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    fn ordinary_typing_and_shift_typing_are_excluded() {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
        assert_eq!(detect(|key| key == 0x41), None);
        assert_eq!(detect(|key| key == 0x41 || key == VK_SHIFT), None);
        assert_eq!(
            detect(|key| key == 0x41 || key == VK_CONTROL),
            Some("CTRL + A".into())
        );
        assert_eq!(detect(|key| key == VK_F5), Some("F5".into()));
        assert_eq!(detect(|key| key == VK_CONTROL), None);
    }
    #[test]
    fn caches_held_shortcuts_and_expires() {
        let mut tracker = Tracker::new();
        assert!(tracker.sample(None, 2, false).is_none());
        let first = tracker.sample(Some("CTRL + C".into()), 2, false).unwrap();
        let second = tracker.sample(Some("CTRL + C".into()), 2, false).unwrap();
        assert!(Arc::ptr_eq(&first.raster, &second.raster));
        tracker.last = Instant::now() - Duration::from_secs(3);
        assert!(tracker.sample(None, 2, false).is_none());
        assert!(tracker.label().is_none());
    }
}
