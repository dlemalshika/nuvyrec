//! AAC track encoder with libswresample conversion and a bounded sample FIFO.
use crate::{error::check, Error, Result};
use ffmpeg_sys_next as av;
use std::ptr;

pub(crate) struct AudioEncoder {
    codec: *mut av::AVCodecContext,
    stream: *mut av::AVStream,
    packet: *mut av::AVPacket,
    frame: *mut av::AVFrame,
    fifo: *mut av::AVAudioFifo,
    resampler: *mut av::SwrContext,
    samples: i64,
    pub(crate) rate: i32,
}

impl AudioEncoder {
    pub(crate) unsafe fn new(
        output: *mut av::AVFormatContext,
        input_rate: i32,
        title: &str,
    ) -> Result<Self> {
        let mut track = Self {
            codec: ptr::null_mut(),
            stream: ptr::null_mut(),
            packet: ptr::null_mut(),
            frame: ptr::null_mut(),
            fifo: ptr::null_mut(),
            resampler: ptr::null_mut(),
            samples: 0,
            rate: input_rate,
        };
        unsafe {
            let codec = av::avcodec_find_encoder(av::AVCodecID::AV_CODEC_ID_AAC);
            if codec.is_null() {
                return Err(Error::State("FFmpeg AAC encoder is unavailable"));
            }
            track.codec = av::avcodec_alloc_context3(codec);
            if track.codec.is_null() {
                return Err(Error::Allocation("audio codec allocation failed"));
            }
            let context = track.codec;
            (*context).sample_rate = 48000;
            (*context).sample_fmt = av::AVSampleFormat::AV_SAMPLE_FMT_FLTP;
            (*context).time_base = av::AVRational { num: 1, den: 48000 };
            (*context).bit_rate = 160000;
            av::av_channel_layout_default(&mut (*context).ch_layout, 2);
            if (*(*output).oformat).flags & av::AVFMT_GLOBALHEADER != 0 {
                (*context).flags |= av::AV_CODEC_FLAG_GLOBAL_HEADER as i32;
            }
            check(
                av::avcodec_open2(track.codec, codec, ptr::null_mut()),
                "open AAC encoder",
            )?;
            track.stream = av::avformat_new_stream(output, ptr::null());
            if track.stream.is_null() {
                return Err(Error::Allocation("audio stream allocation failed"));
            }
            (*track.stream).time_base = (*context).time_base;
            check(
                av::avcodec_parameters_from_context((*track.stream).codecpar, track.codec),
                "copy audio parameters",
            )?;
            let title = std::ffi::CString::new(title)
                .map_err(|_| Error::Invalid("invalid audio track title"))?;
            check(
                av::av_dict_set(
                    &mut (*track.stream).metadata,
                    c"title".as_ptr(),
                    title.as_ptr(),
                    0,
                ),
                "set audio track title",
            )?;
            track.frame = av::av_frame_alloc();
            track.packet = av::av_packet_alloc();
            if track.frame.is_null() || track.packet.is_null() {
                return Err(Error::Allocation("audio frame allocation failed"));
            }
            (*track.frame).format = (*context).sample_fmt as i32;
            (*track.frame).sample_rate = 48000;
            (*track.frame).nb_samples = (*context).frame_size;
            check(
                av::av_channel_layout_copy(&mut (*track.frame).ch_layout, &(*context).ch_layout),
                "copy audio layout",
            )?;
            check(
                av::av_frame_get_buffer(track.frame, 0),
                "allocate audio samples",
            )?;
            track.fifo = av::av_audio_fifo_alloc((*context).sample_fmt, 2, (*context).frame_size);
            if track.fifo.is_null() {
                return Err(Error::Allocation("audio FIFO allocation failed"));
            }
            let mut input_layout: av::AVChannelLayout = std::mem::zeroed();
            av::av_channel_layout_default(&mut input_layout, 2);
            let result = av::swr_alloc_set_opts2(
                &mut track.resampler,
                &(*context).ch_layout,
                (*context).sample_fmt,
                48000,
                &input_layout,
                av::AVSampleFormat::AV_SAMPLE_FMT_FLT,
                input_rate,
                0,
                ptr::null_mut(),
            );
            av::av_channel_layout_uninit(&mut input_layout);
            check(result, "allocate audio resampler")?;
            check(av::swr_init(track.resampler), "initialize audio resampler")?;
        }
        Ok(track)
    }

    pub(crate) unsafe fn append(
        &mut self,
        output: &mut crate::output::Output,
        pcm: &[f32],
    ) -> Result<()> {
        if pcm.is_empty() {
            return Ok(());
        }
        if !pcm.len().is_multiple_of(2) || pcm.len() > self.rate as usize * 2 {
            return Err(Error::Invalid("audio packet exceeds bounded stereo input"));
        }
        unsafe {
            let input_samples = (pcm.len() / 2) as i32;
            let capacity = av::swr_get_out_samples(self.resampler, input_samples);
            check(capacity, "measure audio conversion")?;
            let mut planes: *mut *mut u8 = ptr::null_mut();
            check(
                av::av_samples_alloc_array_and_samples(
                    &mut planes,
                    ptr::null_mut(),
                    2,
                    capacity,
                    av::AVSampleFormat::AV_SAMPLE_FMT_FLTP,
                    0,
                ),
                "allocate conversion buffer",
            )?;
            struct Samples(*mut *mut u8);
            impl Drop for Samples {
                fn drop(&mut self) {
                    unsafe {
                        av::av_free(*self.0 as *mut _);
                        av::av_free(self.0.cast());
                    }
                }
            }
            let converted = Samples(planes);
            let input = pcm.as_ptr().cast::<u8>();
            let count = av::swr_convert(self.resampler, planes, capacity, &input, input_samples);
            check(count, "resample audio packet")?;
            let queued = av::av_audio_fifo_size(self.fifo);
            if queued + count > 96000 {
                return Err(Error::State("audio encoder exceeded sample budget"));
            }
            check(
                av::av_audio_fifo_realloc(self.fifo, queued + count),
                "grow bounded audio FIFO",
            )?;
            if av::av_audio_fifo_write(self.fifo, planes.cast(), count) != count {
                return Err(Error::State("audio FIFO rejected converted samples"));
            }
            drop(converted);
            self.drain_fifo(output, false)
        }
    }

    /// Fill genuine source silence against the recording clock. Small packet
    /// jitter is tolerated; long-session drift correction remains a release gate.
    pub(crate) unsafe fn pad_until(
        &mut self,
        output: &mut crate::output::Output,
        target: i64,
    ) -> Result<()> {
        unsafe {
            let queued = i64::from(av::av_audio_fifo_size(self.fifo));
            let gap = target - self.samples - queued;
            if gap > 2400 {
                let frames = ((gap.min(48000) * i64::from(self.rate) + 47999) / 48000) as usize;
                self.append(output, &vec![0.0; frames * 2])?;
            }
        }
        Ok(())
    }

    unsafe fn drain_fifo(
        &mut self,
        output: &mut crate::output::Output,
        finalizing: bool,
    ) -> Result<()> {
        unsafe {
            let size = (*self.codec).frame_size;
            while av::av_audio_fifo_size(self.fifo) >= size
                || (finalizing && av::av_audio_fifo_size(self.fifo) > 0)
            {
                check(
                    av::av_frame_make_writable(self.frame),
                    "make audio frame writable",
                )?;
                for channel in 0..2 {
                    ptr::write_bytes((*self.frame).data[channel], 0, size as usize * 4);
                }
                let count = av::av_audio_fifo_size(self.fifo).min(size);
                if av::av_audio_fifo_read(self.fifo, (*self.frame).extended_data.cast(), count)
                    != count
                {
                    return Err(Error::State("audio FIFO sample count mismatch"));
                }
                (*self.frame).pts = self.samples;
                self.samples += i64::from(size);
                check(
                    av::avcodec_send_frame(self.codec, self.frame),
                    "send audio frame",
                )?;
                self.receive(output)?;
            }
        }
        Ok(())
    }
    unsafe fn receive(&mut self, output: &mut crate::output::Output) -> Result<()> {
        unsafe {
            loop {
                let code = av::avcodec_receive_packet(self.codec, self.packet);
                if code == -libc::EAGAIN || code == av::AVERROR_EOF {
                    return Ok(());
                }
                check(code, "receive audio packet")?;
                av::av_packet_rescale_ts(
                    self.packet,
                    (*self.codec).time_base,
                    (*self.stream).time_base,
                );
                (*self.packet).stream_index = (*self.stream).index;
                let written = output.mux(self.packet);
                av::av_packet_unref(self.packet);
                written?;
            }
        }
    }
    pub(crate) unsafe fn finish(&mut self, output: &mut crate::output::Output) -> Result<()> {
        unsafe {
            self.drain_fifo(output, true)?;
            check(
                av::avcodec_send_frame(self.codec, ptr::null()),
                "flush audio encoder",
            )?;
            self.receive(output)
        }
    }
}
impl Drop for AudioEncoder {
    fn drop(&mut self) {
        unsafe {
            if !self.fifo.is_null() {
                av::av_audio_fifo_free(self.fifo);
            }
            av::swr_free(&mut self.resampler);
            av::av_frame_free(&mut self.frame);
            av::av_packet_free(&mut self.packet);
            av::avcodec_free_context(&mut self.codec);
        }
    }
}
