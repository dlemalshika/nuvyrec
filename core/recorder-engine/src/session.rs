use crate::audio::AudioEncoder;
use crate::{error::check, output::Output, Error, RecordingConfig, Result, VideoFrame};
use ffmpeg_sys_next as av;
use std::{ffi::CString, marker::PhantomData, ptr, rc::Rc};

#[derive(Clone, Copy, PartialEq)]
enum State {
    Ready,
    Failed,
    Finished,
}

/// A single-owner, video-only recording session. Create and use it on the same
/// native worker thread. No unsafe Send/Sync implementations are provided.
///
/// Call finish to drain delayed packets and write the trailer. Drop only frees
/// resources: it cannot report IO errors and does not promise a playable file.
pub struct RecordingSession {
    output: Output,
    encoder: *mut av::AVCodecContext,
    packet: *mut av::AVPacket,
    stream: *mut av::AVStream,
    state: State,
    last_pts: Option<i64>,
    audio: Vec<AudioEncoder>,
    _thread_confined: PhantomData<Rc<()>>,
}

impl RecordingSession {
    /// Start independent, bounded RTMP outputs using this session's encoded
    /// H.264 and first AAC track. Call before submitting the first video frame.
    /// Network failures never propagate into the local recording muxer.
    pub fn enable_streaming(&mut self, targets: Vec<crate::streaming::Destination>) -> Result<()> {
        if self.state != State::Ready || self.output.streaming.is_some() || self.last_pts.is_some()
        {
            return Err(Error::State("start streaming before the first frame"));
        }
        self.output.streaming =
            Some(unsafe { crate::streaming::Streaming::new(self.output.raw, targets)? });
        Ok(())
    }

    pub fn streaming_status(&self) -> Vec<crate::streaming::DestinationStatus> {
        self.output
            .streaming
            .as_ref()
            .map_or_else(Vec::new, |s| s.status())
    }

    /// Stop publishing while keeping the local recording session alive.
    pub fn stop_streaming(&mut self) {
        if let Some(streaming) = &mut self.output.streaming {
            streaming.request_stop();
        }
    }

    pub fn enable_replay(&mut self, seconds: u32, limit_mib: u32) -> Result<()> {
        self.output.replay = Some(crate::replay::Replay::new(
            seconds,
            limit_mib as usize * 1024 * 1024,
        )?);
        Ok(())
    }
    pub fn replay_status(&self) -> (f64, usize) {
        self.output
            .replay
            .as_ref()
            .map_or((0.0, 0), |r| (r.available(), r.bytes()))
    }
    pub fn save_replay(&self, path: &std::path::Path) -> Result<()> {
        let replay = self
            .output
            .replay
            .as_ref()
            .ok_or(Error::State("Replay buffer is disabled"))?;
        unsafe { replay.save(self.output.raw, path) }
    }
    /// Initialize FFmpeg's MPEG-4 Part 2 encoder and the selected muxer.
    /// This widely available reference encoder requires no optional libx264.
    /// It is not the planned production H.264/hardware encoder policy.
    pub fn new(config: &RecordingConfig) -> Result<Self> {
        Self::initialize(config, None, None, &[])
    }

    /// Initialize an encoder from a capture frame's negotiated format and
    /// hardware frame context. No pixel data crosses the UI boundary.
    pub fn encoded(config: &RecordingConfig, name: &str, first: &VideoFrame) -> Result<Self> {
        let name = CString::new(name).map_err(|_| Error::Invalid("invalid encoder name"))?;
        Self::initialize(config, Some(&name), Some(first), &[])
    }

    /// Attach separate AAC tracks before the muxer header is committed.
    pub fn encoded_with_audio(
        config: &RecordingConfig,
        name: &str,
        first: &VideoFrame,
        tracks: &[(i32, &str)],
    ) -> Result<Self> {
        if !tracks.is_empty() && matches!(config.container, crate::Container::Gif) {
            return Err(Error::Invalid("GIF cannot contain audio tracks"));
        }
        if tracks.len() > 3
            || tracks
                .iter()
                .any(|(rate, _)| !(8000..=192000).contains(rate))
        {
            return Err(Error::Invalid("invalid audio track configuration"));
        }
        let name = CString::new(name).map_err(|_| Error::Invalid("invalid encoder name"))?;
        Self::initialize(config, Some(&name), Some(first), tracks)
    }

    fn initialize(
        config: &RecordingConfig,
        name: Option<&std::ffi::CStr>,
        first: Option<&VideoFrame>,
        tracks: &[(i32, &str)],
    ) -> Result<Self> {
        config.validate().map_err(Error::Invalid)?;
        let muxer = CString::new(config.container.muxer_name())
            .map_err(|_| Error::Invalid("invalid muxer name"))?;
        let output = Output::new(&muxer)?;
        let mut session = Self {
            output,
            encoder: ptr::null_mut(),
            packet: ptr::null_mut(),
            stream: ptr::null_mut(),
            state: State::Ready,
            last_pts: None,
            audio: Vec::new(),
            _thread_confined: PhantomData,
        };
        // SAFETY: every allocation is checked before dereference. Ownership is
        // immediately stored in session so failures run the appropriate Drop.
        unsafe {
            let codec = match name {
                Some(name) => av::avcodec_find_encoder_by_name(name.as_ptr()),
                None => av::avcodec_find_encoder(av::AVCodecID::AV_CODEC_ID_MPEG4),
            };
            if codec.is_null() {
                return Err(Error::State("FFmpeg build lacks the mpeg4 encoder"));
            }
            if av::avformat_query_codec((*session.output.raw).oformat, (*codec).id, 0) <= 0 {
                return Err(Error::Invalid("muxer does not support the reference codec"));
            }
            session.encoder = av::avcodec_alloc_context3(codec);
            if session.encoder.is_null() {
                return Err(Error::Allocation("codec allocation failed"));
            }
            let encoder = &mut *session.encoder;
            encoder.width = config.width as i32;
            encoder.height = config.height as i32;
            encoder.pix_fmt = av::AVPixelFormat::AV_PIX_FMT_YUV420P;
            encoder.time_base = av::AVRational {
                num: 1,
                den: config.fps as i32,
            };
            encoder.framerate = av::AVRational {
                num: config.fps as i32,
                den: 1,
            };
            encoder.bit_rate = i64::from(config.bitrate);
            encoder.thread_count = i32::from(config.encoder_threads);
            encoder.gop_size = config.fps as i32 * 2;
            encoder.max_b_frames = 0;
            encoder.color_range = av::AVColorRange::AVCOL_RANGE_MPEG;
            encoder.colorspace = av::AVColorSpace::AVCOL_SPC_BT709;
            encoder.color_primaries = av::AVColorPrimaries::AVCOL_PRI_BT709;
            encoder.color_trc = av::AVColorTransferCharacteristic::AVCOL_TRC_BT709;
            if let Some(frame) = first {
                if (*frame.raw).width != encoder.width || (*frame.raw).height != encoder.height {
                    return Err(Error::Invalid(
                        "capture dimensions differ from requested output",
                    ));
                }
                encoder.pix_fmt =
                    std::mem::transmute::<i32, av::AVPixelFormat>((*frame.raw).format);
                if !(*frame.raw).hw_frames_ctx.is_null() {
                    encoder.hw_frames_ctx = av::av_buffer_ref((*frame.raw).hw_frames_ctx);
                    if encoder.hw_frames_ctx.is_null() {
                        return Err(Error::Allocation("hardware context reference failed"));
                    }
                }
            }
            if (*(*session.output.raw).oformat).flags & av::AVFMT_GLOBALHEADER != 0 {
                encoder.flags |= av::AV_CODEC_FLAG_GLOBAL_HEADER as i32;
            }
            let mut options = ptr::null_mut();
            if name.is_some_and(|n| n.to_bytes() == b"libx264") {
                av::av_dict_set(&mut options, c"preset".as_ptr(), c"ultrafast".as_ptr(), 0);
                av::av_dict_set(&mut options, c"tune".as_ptr(), c"zerolatency".as_ptr(), 0);
            }
            let opened = av::avcodec_open2(session.encoder, codec, &mut options);
            av::av_dict_free(&mut options);
            check(opened, "open video encoder")?;
            session.stream = av::avformat_new_stream(session.output.raw, ptr::null());
            if session.stream.is_null() {
                return Err(Error::Allocation("stream allocation failed"));
            }
            (*session.stream).time_base = (*session.encoder).time_base;
            (*session.stream).avg_frame_rate = (*session.encoder).framerate;
            check(
                av::avcodec_parameters_from_context((*session.stream).codecpar, session.encoder),
                "copy stream parameters",
            )?;
            session.packet = av::av_packet_alloc();
            if session.packet.is_null() {
                return Err(Error::Allocation("packet allocation failed"));
            }
            for (rate, title) in tracks {
                session
                    .audio
                    .push(AudioEncoder::new(session.output.raw, *rate, title)?);
            }
            session.output.open_file(&config.output)?;
            check(
                av::avformat_write_header(session.output.raw, ptr::null_mut()),
                "write container header",
            )?;
        }
        Ok(session)
    }

    /// Submit a frame with nonnegative, strictly increasing PTS measured in
    /// 1/fps seconds. Missing timestamps represent skipped capture intervals.
    pub fn write_video(&mut self, frame: &mut VideoFrame, pts: i64) -> Result<()> {
        if self.state != State::Ready {
            return Err(Error::State("session is not recording"));
        }
        if pts < 0 || self.last_pts.is_some_and(|previous| pts <= previous) {
            return Err(Error::Invalid(
                "PTS must be nonnegative and strictly increasing",
            ));
        }
        unsafe {
            if (*frame.raw).width != (*self.encoder).width
                || (*frame.raw).height != (*self.encoder).height
                || (*frame.raw).format != (*self.encoder).pix_fmt as i32
            {
                return Err(Error::Invalid(
                    "frame format or dimensions do not match the session",
                ));
            }
            (*frame.raw).pts = pts;
        }
        let result = self.send(frame.raw, false);
        if result.is_err() {
            self.state = State::Failed;
        } else {
            self.last_pts = Some(pts);
        }
        result
    }

    /// Append interleaved stereo float PCM at a track's input sample rate.
    pub fn write_audio(&mut self, index: usize, pcm: &[f32], clock_seconds: f64) -> Result<()> {
        if self.state != State::Ready {
            return Err(Error::State("session is not recording"));
        }
        if !clock_seconds.is_finite() || clock_seconds < 0.0 {
            return Err(Error::Invalid("invalid audio clock"));
        }
        let track = self
            .audio
            .get_mut(index)
            .ok_or(Error::Invalid("unknown audio track"))?;
        let result = unsafe {
            track
                .append(&mut self.output, pcm)
                .and_then(|_| track.pad_until(&mut self.output, (clock_seconds * 48000.0) as i64))
        };
        if result.is_err() {
            self.state = State::Failed;
        }
        result
    }

    /// Consume the session so it can never accept frames after finalization.
    pub fn finish(mut self) -> Result<()> {
        self.finalize()
    }

    /// Finish a segment while retaining a handle that rejects further writes.
    /// This permits the worker to rotate to an independently playable file.
    pub fn finalize(&mut self) -> Result<()> {
        if self.state != State::Ready {
            return Err(Error::State("cannot finalize a failed session"));
        }
        self.state = State::Failed;
        self.send(ptr::null(), true)?;
        for track in &mut self.audio {
            unsafe {
                track.finish(&mut self.output)?;
            }
        }
        unsafe {
            check(
                av::av_write_trailer(self.output.raw),
                "write container trailer",
            )?;
        }
        self.output.sync()?;
        self.state = State::Finished;
        Ok(())
    }

    fn send(&mut self, frame: *const av::AVFrame, flushing: bool) -> Result<()> {
        unsafe {
            let mut code = av::avcodec_send_frame(self.encoder, frame);
            if code == -libc::EAGAIN {
                self.drain(false)?;
                code = av::avcodec_send_frame(self.encoder, frame);
            }
            check(code, "send frame")?;
        }
        self.drain(flushing)
    }

    fn drain(&mut self, flushing: bool) -> Result<()> {
        unsafe {
            loop {
                let code = av::avcodec_receive_packet(self.encoder, self.packet);
                if code == av::AVERROR_EOF {
                    return Ok(());
                }
                if code == -libc::EAGAIN {
                    if flushing {
                        return Err(Error::State("encoder requested frames after flush"));
                    }
                    return Ok(());
                }
                check(code, "receive encoded packet")?;
                // Header writing may have changed the stream time base.
                av::av_packet_rescale_ts(
                    self.packet,
                    (*self.encoder).time_base,
                    (*self.stream).time_base,
                );
                (*self.packet).stream_index = (*self.stream).index;
                let written = self.output.mux(self.packet);
                av::av_packet_unref(self.packet);
                written?;
            }
        }
    }
}

impl Drop for RecordingSession {
    fn drop(&mut self) {
        unsafe {
            av::av_packet_free(&mut self.packet);
            av::avcodec_free_context(&mut self.encoder);
        }
    }
}
