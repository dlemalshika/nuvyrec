//! Deterministic compositor/muxer fixture; no camera or desktop pixels are used.
use recorder_engine::{
    webcam::{Image, Placement},
    Container, RecordingConfig, RecordingSession, VideoFrame,
};
use std::time::Instant;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Provide a new MKV output path")?;
    let mut config = RecordingConfig::reference(path, Container::Matroska);
    config.width = 640;
    config.height = 360;
    config.fps = 24;
    config.encoder_threads = 1;
    let mut session = RecordingSession::new(&config)?;
    for index in 0..48 {
        let mut frame = VideoFrame::new(640, 360)?;
        frame.copy_yuv420(
            &vec![80; 640 * 360],
            &vec![160; 320 * 180],
            &vec![100; 320 * 180],
        )?;
        let mut bgra = vec![0; 160 * 120 * 4];
        for y in 0_i32..120 {
            for x in 0_i32..160 {
                let cx = 40 + index % 60;
                let color = if (x - cx as i32).pow(2) + (y - 60i32).pow(2) < 28 * 28 {
                    [30, 60, 240, 255]
                } else {
                    [0, 255, 0, 255]
                };
                let offset = (y as usize * 160 + x as usize) * 4;
                bgra[offset..offset + 4].copy_from_slice(&color);
            }
        }
        let image = Image {
            width: 160,
            height: 120,
            bgra,
            captured: Instant::now(),
        };
        let place = Placement {
            x: 1.0,
            y: 1.0,
            width: 0.4,
            mirror: false,
            key: true,
            similarity: 0.15,
            softness: 0.08,
        };
        frame.webcam(&image, &place)?;
        session.write_video(&mut frame, index)?;
    }
    session.finish()?;
    Ok(())
}
