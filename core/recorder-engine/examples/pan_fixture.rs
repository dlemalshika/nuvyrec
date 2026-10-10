//! Native integration probe: the caller owns the synthetic window. No pointer
//! manipulation or desktop capture. Verify live native GPU panning on one graph.
#[cfg(windows)]
use recorder_engine::capture::Capture;
#[cfg(windows)]
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let hwnd: u64 = args[1].parse()?;
    let output = PathBuf::from(&args[2]);
    let _apartment = recorder_engine::wasapi::Apartment::initialize()?;
    let graph=format!("gfxcapture=hwnd={hwnd}:capture_border=0:width=160:height=180:crop_right=320:max_framerate=30:resize_mode=scale,hwdownload,format=bgra,format=rgba");
    let mut capture = Capture::open(&graph)?;
    for (label, region) in [("left", [0, 0, 320, 360]), ("right", [320, 0, 320, 360])] {
        capture.pan_region(region, [640, 360])?;
        let began = Instant::now();
        let mut png = None;
        while began.elapsed() < Duration::from_secs(1) {
            if let Some(frame) = capture.next_frame()? {
                png = Some(frame.thumbnail_png()?);
            }
            thread::sleep(Duration::from_millis(5));
        }
        std::fs::write(
            output.join(format!("pan-{label}.png")),
            png.ok_or("No fixture frame")?,
        )?;
    }
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("The owned-window GPU pan probe requires Windows");
}
