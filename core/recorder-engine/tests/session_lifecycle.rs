use recorder_engine::{Container, RecordingConfig, RecordingSession, VideoFrame};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "recorder-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn rejects_bad_timestamps_without_poisoning_session_and_finishes() {
    let directory = TestDirectory::new();
    let path = directory.0.join("recording.mp4");
    let mut config = RecordingConfig::reference(&path, Container::Mp4);
    config.width = 64;
    config.height = 64;
    let mut session = RecordingSession::new(&config).unwrap();
    let mut frame = VideoFrame::new(64, 64).unwrap();
    assert!(frame
        .copy_yuv420(&[16; 4095], &[128; 1024], &[128; 1024])
        .is_err());
    frame
        .copy_yuv420(&[16; 4096], &[128; 1024], &[128; 1024])
        .unwrap();
    assert!(session.write_video(&mut frame, -1).is_err());
    session.write_video(&mut frame, 0).unwrap();
    assert!(session.write_video(&mut frame, 0).is_err());
    session.write_video(&mut frame, 1).unwrap();
    session.finish().unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    // Removal verifies the native IO handle was released on Windows as well.
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn initialization_never_overwrites_existing_recordings() {
    let directory = TestDirectory::new();
    let path = directory.0.join("existing.mp4");
    std::fs::write(&path, b"preserve this recording").unwrap();
    let config = RecordingConfig::reference(&path, Container::Mp4);
    assert!(RecordingSession::new(&config).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"preserve this recording");
}

#[test]
fn encodes_independent_audio_tracks_with_different_device_rates() {
    let directory = TestDirectory::new();
    let path = directory.0.join("multitrack.mkv");
    let mut config = RecordingConfig::reference(&path, Container::Matroska);
    config.width = 64;
    config.height = 64;
    let mut frame = VideoFrame::new(64, 64).unwrap();
    let mut session = RecordingSession::encoded_with_audio(
        &config,
        "mpeg4",
        &frame,
        &[(44100, "System audio"), (48000, "Microphone")],
    )
    .unwrap();
    for pts in 0..30 {
        session.write_video(&mut frame, pts).unwrap();
        for (track, rate) in [(0, 44100), (1, 48000)] {
            let samples: Vec<f32> = (0..rate / 30)
                .flat_map(|i| {
                    let sample = ((i + pts * rate / 30) as f32 * 440.0 * std::f32::consts::TAU
                        / rate as f32)
                        .sin()
                        * 0.1;
                    [sample, sample]
                })
                .collect();
            session
                .write_audio(track, &samples, (pts + 1) as f64 / 30.0)
                .unwrap();
        }
    }
    session.finish().unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() > 1000);
}

#[test]
fn replay_starts_on_keyframe_preserves_audio_and_keeps_recording() {
    let directory = TestDirectory::new();
    let mut config = RecordingConfig::reference(directory.0.join("main.mkv"), Container::Matroska);
    config.width = 64;
    config.height = 64;
    let mut frame = VideoFrame::new(64, 64).unwrap();
    frame
        .copy_yuv420(&[32; 4096], &[128; 1024], &[128; 1024])
        .unwrap();
    let mut session = RecordingSession::encoded_with_audio(
        &config,
        "libx264",
        &frame,
        &[(48000, "Combined audio")],
    )
    .unwrap();
    session.enable_replay(5, 4).unwrap();
    let clip = directory.0.join("replay.mkv");
    assert!(session.save_replay(&clip).is_err());
    for pts in 0..210 {
        session.write_video(&mut frame, pts).unwrap();
        session
            .write_audio(0, &[0.0; 3200], (pts + 1) as f64 / 30.0)
            .unwrap();
        if pts == 180 {
            session.save_replay(&clip).unwrap();
            let original = std::fs::read(&clip).unwrap();
            assert!(session.save_replay(&clip).is_err());
            assert_eq!(std::fs::read(&clip).unwrap(), original);
        }
        assert!(session.replay_status().1 <= 4 * 1024 * 1024);
    }
    session.finish().unwrap();
    // Read the independently finalized replay through FFmpeg, checking codec
    // metadata and timestamp ordering rather than merely checking file size.
    unsafe {
        use ffmpeg_sys_next as av;
        let name = std::ffi::CString::new(clip.to_str().unwrap()).unwrap();
        let mut context = std::ptr::null_mut();
        assert_eq!(
            av::avformat_open_input(
                &mut context,
                name.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut()
            ),
            0
        );
        assert!(av::avformat_find_stream_info(context, std::ptr::null_mut()) >= 0);
        assert_eq!((*context).nb_streams, 2);
        let mut packet = av::av_packet_alloc();
        let mut previous = [i64::MIN; 2];
        let mut counts = [0; 2];
        while av::av_read_frame(context, packet) >= 0 {
            let index = (*packet).stream_index as usize;
            if index == 0 && counts[0] == 0 {
                assert!((*packet).flags & av::AV_PKT_FLAG_KEY != 0);
            }
            if (*packet).dts != av::AV_NOPTS_VALUE {
                assert!((*packet).dts >= previous[index]);
                previous[index] = (*packet).dts;
            }
            counts[index] += 1;
            av::av_packet_unref(packet);
        }
        av::av_packet_free(&mut packet);
        av::avformat_close_input(&mut context);
        assert!(counts[0] >= 60 && counts[1] >= 60);
    }
}
