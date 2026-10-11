//! Loopback-only multistream fixture: generated pixels/audio, no desktop/camera.
//! Usage: stream_fixture <new.mkv> <port1> <port2> <port3>
use recorder_engine::{
    streaming::Destination, Container, RecordingConfig, RecordingSession, VideoFrame,
};
use std::{
    error::Error,
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 && !(args.len() == 6 && args[5] == "--tls-last") {
        return Err("usage: stream_fixture <new.mkv> <port1> <port2> <port3>".into());
    }
    let mut config = RecordingConfig::reference(&args[1], Container::Matroska);
    config.width = 320;
    config.height = 180;
    config.fps = 25;
    config.bitrate = 400_000;
    let mut frame = VideoFrame::new(config.width, config.height)?;
    let mut session = RecordingSession::encoded_with_audio(
        &config,
        "libx264",
        &frame,
        &[(48000, "Combined audio"), (48000, "Independent microphone")],
    )?;
    let platforms = ["youtube", "facebook", "tiktok"];
    let mut targets = Vec::new();
    for (platform, port) in platforms.iter().zip(&args[2..5]) {
        let port: u16 = port.parse()?;
        targets.push(Destination {
            platform: (*platform).into(),
            server: format!(
                "{}://127.0.0.1:{port}/live",
                if args.len() == 6 && *platform == "tiktok" {
                    "rtmps"
                } else {
                    "rtmp"
                }
            ),
            key: "fixture".into(),
        });
    }
    session.enable_streaming(targets)?;
    let pixels = config.width as usize * config.height as usize;
    let mut y = vec![16; pixels];
    let u = vec![128; pixels / 4];
    let v = vec![128; pixels / 4];
    let start = Instant::now();
    for pts in 0..250i64 {
        y.fill(32 + (pts % 160) as u8);
        frame.copy_yuv420(&y, &u, &v)?;
        session.write_video(&mut frame, pts)?;
        let pcm: Vec<f32> = (0..1920)
            .flat_map(|i| {
                let t = (pts * 1920 + i) as f32 / 48000.0;
                let sample = (t * 440.0 * std::f32::consts::TAU).sin() * 0.08;
                [sample, sample]
            })
            .collect();
        session.write_audio(0, &pcm, (pts + 1) as f64 / 25.0)?;
        session.write_audio(1, &pcm, (pts + 1) as f64 / 25.0)?;
        if pts == 199 {
            session.stop_streaming();
        }
        let due = Duration::from_millis((pts as u64 + 1) * 40);
        if let Some(delay) = due.checked_sub(start.elapsed()) {
            thread::sleep(delay);
        }
    }
    for status in session.streaming_status() {
        println!(
            "{} {} packets={} reconnects={} dropped={} error_code={:?} operation={:?}",
            status.platform,
            status.state,
            status.packets,
            status.reconnects,
            status.dropped_packets,
            status.error_code,
            status.error_operation
        );
    }
    let end = Instant::now();
    session.finish()?;
    println!("finalize_ms={}", end.elapsed().as_millis());
    Ok(())
}
