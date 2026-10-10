//! Deterministic video-only smoke test. This does not capture the screen.
use recorder_engine::{Container, RecordingConfig, RecordingSession, VideoFrame};
use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: synthetic <new-output.mp4|mkv|avi>")?,
    );
    let container = match path.extension().and_then(|s| s.to_str()) {
        Some("mp4") => Container::Mp4,
        Some("mkv") => Container::Matroska,
        Some("avi") => Container::Avi,
        _ => return Err("supported extensions: mp4, mkv, avi".into()),
    };
    let mut config = RecordingConfig::reference(&path, container);
    config.width = 640;
    config.height = 360;
    let mut session = RecordingSession::new(&config)?;
    let mut frame = VideoFrame::new(config.width, config.height)?;
    let pixels = config.width as usize * config.height as usize;
    let mut y = vec![16; pixels];
    let u = vec![128; pixels / 4];
    let v = vec![128; pixels / 4];
    for pts in 0..i64::from(config.fps) * 3 {
        // Limited-range grayscale ramp makes dropped/empty frames observable.
        y.fill(16 + (pts % 220) as u8);
        frame.copy_yuv420(&y, &u, &v)?;
        session.write_video(&mut frame, pts)?;
    }
    session.finish()?;
    println!("Wrote 3 seconds of synthetic video to {}", path.display());
    Ok(())
}
