//! Captures only the caller-owned synthetic window; camera pixels are generated.
//! Exercises the real D3D11 render path independently of encoder availability.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use recorder_engine::{
        capture::Capture,
        layers::{Layer, Raster},
        overlay::{Annotation, Shape},
        webcam::{Image, Placement},
    };
    use std::{
        path::PathBuf,
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };
    let args: Vec<_> = std::env::args().collect();
    let hwnd: u64 = args[1].parse()?;
    let directory = PathBuf::from(&args[2]);
    let mut capture=Capture::open(&format!("gfxcapture=hwnd={hwnd}:capture_border=0:width=320:height=180:max_framerate=30:resize_mode=scale,hwdownload,format=bgra,format=rgba"))?;
    capture.enable_gpu_overlays()?;
    let raster = Arc::new(Raster::rgba(
        2,
        2,
        &[
            255, 255, 255, 0, 255, 255, 255, 128, 255, 255, 255, 0, 255, 255, 255, 128,
        ],
    )?);
    let layer = Layer {
        raster,
        x: 0.,
        y: 1.,
        width: 0.25,
        opacity: 1.,
    };
    let line = Annotation {
        shape: Shape::Stroke,
        points: vec![[0.1, 0.25], [0.9, 0.25]],
        rgb: [0, 255, 0],
        width: 6,
    };
    let camera = Image {
        width: 2,
        height: 2,
        bgra: vec![
            0, 255, 0, 255, 255, 0, 255, 255, 0, 255, 0, 255, 255, 0, 255, 255,
        ],
        captured: Instant::now(),
    };
    let place = Placement {
        x: 1.,
        y: 1.,
        width: 0.25,
        mirror: true,
        key: true,
        similarity: 0.15,
        softness: 0.08,
    };
    capture.update_gpu_overlays(&[layer], &[line], &[], Some((&camera, &place)))?;
    for phase in ["composite", "clear", "spotlight"] {
        if phase == "clear" {
            capture.update_gpu_overlays(&[], &[], &[], None)?;
        }
        if phase == "spotlight" {
            capture.update_gpu_overlays(
                &[],
                &[Annotation {
                    shape: Shape::Spotlight,
                    points: vec![[0.25, 0.25]],
                    rgb: [0; 3],
                    width: 1,
                }],
                &[],
                None,
            )?;
        }
        let began = Instant::now();
        let mut png = None;
        while began.elapsed() < Duration::from_millis(350) {
            if let Some(frame) = capture.next_frame()? {
                png = Some(frame.thumbnail_png()?);
            }
            thread::sleep(Duration::from_millis(5));
        }
        std::fs::write(
            directory.join(format!("gpu-{phase}.png")),
            png.ok_or("No GPU fixture frame")?,
        )?;
    }
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("GPU fixture requires Windows");
}
