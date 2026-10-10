//! Bounded, thread-confined audio DSP. Optional denoising runs locally in FFmpeg;
//! all tracks are resampled to stereo 48 kHz before mixing.
use crate::{error::check, Error, Result, VideoFrame};
use ffmpeg_sys_next as av;
use std::{collections::VecDeque, ffi::CString, marker::PhantomData, ptr, rc::Rc};

pub struct AudioProcessor {
    graph: *mut av::AVFilterGraph,
    source: *mut av::AVFilterContext,
    sink: *mut av::AVFilterContext,
    rate: i32,
    pts: i64,
    _thread: PhantomData<Rc<()>>,
}
impl AudioProcessor {
    pub fn new(rate: i32, denoise: bool, gate: bool, limiter: bool) -> Result<Self> {
        if !(8000..=192000).contains(&rate) {
            return Err(Error::Invalid("invalid DSP sample rate"));
        }
        let mut result = Self {
            graph: unsafe { av::avfilter_graph_alloc() },
            source: ptr::null_mut(),
            sink: ptr::null_mut(),
            rate,
            pts: 0,
            _thread: PhantomData,
        };
        if result.graph.is_null() {
            return Err(Error::Allocation("audio DSP graph"));
        }
        let args = CString::new(format!(
            "time_base=1/{rate}:sample_rate={rate}:sample_fmt=flt:channel_layout=stereo"
        ))
        .map_err(|_| Error::Invalid("audio DSP arguments"))?;
        let mut filters = String::from("aresample=48000");
        if denoise {
            filters.push_str(",afftdn=nf=-25:tn=1");
        }
        if gate {
            filters.push_str(",agate=threshold=0.01:ratio=4:attack=10:release=120");
        }
        if limiter {
            filters.push_str(",alimiter=limit=0.95:level=0:latency=1");
        }
        filters.push_str(",aformat=sample_fmts=flt:sample_rates=48000:channel_layouts=stereo");
        let description =
            CString::new(filters).map_err(|_| Error::Invalid("audio DSP description"))?;
        unsafe {
            (*result.graph).nb_threads = 1;
            let input = av::avfilter_get_by_name(c"abuffer".as_ptr());
            let output = av::avfilter_get_by_name(c"abuffersink".as_ptr());
            if input.is_null() || output.is_null() {
                return Err(Error::State("audio DSP filters unavailable"));
            }
            check(
                av::avfilter_graph_create_filter(
                    &mut result.source,
                    input,
                    c"source".as_ptr(),
                    args.as_ptr(),
                    ptr::null_mut(),
                    result.graph,
                ),
                "create audio DSP input",
            )?;
            check(
                av::avfilter_graph_create_filter(
                    &mut result.sink,
                    output,
                    c"sink".as_ptr(),
                    ptr::null(),
                    ptr::null_mut(),
                    result.graph,
                ),
                "create audio DSP output",
            )?;
            let mut inputs = av::avfilter_inout_alloc();
            let mut outputs = av::avfilter_inout_alloc();
            if inputs.is_null() || outputs.is_null() {
                av::avfilter_inout_free(&mut inputs);
                av::avfilter_inout_free(&mut outputs);
                return Err(Error::Allocation("audio DSP endpoints"));
            }
            (*inputs).name = av::av_strdup(c"out".as_ptr());
            (*inputs).filter_ctx = result.sink;
            (*outputs).name = av::av_strdup(c"in".as_ptr());
            (*outputs).filter_ctx = result.source;
            if (*inputs).name.is_null() || (*outputs).name.is_null() {
                av::avfilter_inout_free(&mut inputs);
                av::avfilter_inout_free(&mut outputs);
                return Err(Error::Allocation("audio DSP names"));
            }
            let parsed = av::avfilter_graph_parse_ptr(
                result.graph,
                description.as_ptr(),
                &mut inputs,
                &mut outputs,
                ptr::null_mut(),
            );
            let disconnected = !inputs.is_null() || !outputs.is_null();
            av::avfilter_inout_free(&mut inputs);
            av::avfilter_inout_free(&mut outputs);
            check(parsed, "parse audio DSP")?;
            if disconnected {
                return Err(Error::State("audio DSP disconnected"));
            }
            check(
                av::avfilter_graph_config(result.graph, ptr::null_mut()),
                "configure audio DSP",
            )?;
        }
        Ok(result)
    }
    pub fn process(&mut self, samples: &[f32]) -> Result<Vec<f32>> {
        if !samples.len().is_multiple_of(2) || samples.len() > self.rate as usize * 2 {
            return Err(Error::Invalid("audio DSP packet too large"));
        }
        if !samples.is_empty() {
            let frame = VideoFrame::empty()?;
            unsafe {
                (*frame.raw).format = av::AVSampleFormat::AV_SAMPLE_FMT_FLT as i32;
                (*frame.raw).sample_rate = self.rate;
                (*frame.raw).nb_samples = (samples.len() / 2) as i32;
                (*frame.raw).pts = self.pts;
                av::av_channel_layout_default(&mut (*frame.raw).ch_layout, 2);
                check(av::av_frame_get_buffer(frame.raw, 0), "allocate DSP PCM")?;
                ptr::copy_nonoverlapping(
                    samples.as_ptr(),
                    (*frame.raw).data[0].cast::<f32>(),
                    samples.len(),
                );
                check(
                    av::av_buffersrc_add_frame_flags(
                        self.source,
                        frame.raw,
                        av::AV_BUFFERSRC_FLAG_KEEP_REF as i32,
                    ),
                    "feed audio DSP",
                )?;
            }
            self.pts += (samples.len() / 2) as i64;
        }
        self.drain()
    }
    pub fn finish(&mut self) -> Result<Vec<f32>> {
        unsafe {
            check(
                av::av_buffersrc_add_frame_flags(self.source, ptr::null_mut(), 0),
                "flush audio DSP",
            )?;
        }
        self.drain()
    }
    fn drain(&mut self) -> Result<Vec<f32>> {
        let mut result = Vec::new();
        loop {
            let frame = VideoFrame::empty()?;
            let code = unsafe { av::av_buffersink_get_frame(self.sink, frame.raw) };
            if code == -libc::EAGAIN || code == av::AVERROR_EOF {
                break;
            }
            check(code, "read audio DSP")?;
            unsafe {
                if (*frame.raw).format != av::AVSampleFormat::AV_SAMPLE_FMT_FLT as i32
                    || (*frame.raw).ch_layout.nb_channels != 2
                    || (*frame.raw).nb_samples < 0
                {
                    return Err(Error::State("unexpected audio DSP output"));
                }
                let count = (*frame.raw).nb_samples as usize * 2;
                if result.len() + count > 96000 {
                    return Err(Error::State("audio DSP exceeded bounded output"));
                }
                result.extend_from_slice(std::slice::from_raw_parts(
                    (*frame.raw).data[0].cast::<f32>(),
                    count,
                ));
            }
        }
        Ok(result)
    }
}
impl Drop for AudioProcessor {
    fn drop(&mut self) {
        unsafe {
            av::avfilter_graph_free(&mut self.graph);
        }
    }
}

/// A shared-clock mix bus. Missing source samples become silence; queues and
/// individual output ticks are capped to one second to prevent runaway memory.
pub struct AudioMixer {
    queues: Vec<VecDeque<f32>>,
    emitted: u64,
}
impl AudioMixer {
    pub fn new(tracks: usize) -> Result<Self> {
        if !(1..=2).contains(&tracks) {
            return Err(Error::Invalid("invalid mixer track count"));
        }
        Ok(Self {
            queues: vec![VecDeque::new(); tracks],
            emitted: 0,
        })
    }
    pub fn push(&mut self, index: usize, samples: &[f32]) -> Result<()> {
        let queue = self
            .queues
            .get_mut(index)
            .ok_or(Error::Invalid("invalid mixer source"))?;
        if !samples.len().is_multiple_of(2) || queue.len() + samples.len() > 96000 {
            return Err(Error::State("mixer backlog exceeded one second"));
        }
        queue.extend(samples.iter().copied());
        Ok(())
    }
    pub fn until(&mut self, clock: f64) -> Result<Vec<f32>> {
        if !clock.is_finite() || clock < 0.0 {
            return Err(Error::Invalid("invalid mixer clock"));
        }
        let target = (clock * 48000.0).floor() as u64;
        let frames = target.saturating_sub(self.emitted);
        if frames > 48000 {
            return Err(Error::State("mixer clock jumped more than one second"));
        }
        let mut mixed = Vec::with_capacity(frames as usize * 2);
        for _ in 0..frames * 2 {
            let sum: f32 = self
                .queues
                .iter_mut()
                .map(|q| q.pop_front().unwrap_or(0.0))
                .sum();
            mixed.push(sum.clamp(-0.95, 0.95));
        }
        self.emitted = target;
        Ok(mixed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dsp_resamples_and_flushes_without_losing_duration() {
        let mut processor = AudioProcessor::new(44100, false, false, false).unwrap();
        let mut output = Vec::new();
        for _ in 0..100 {
            output.extend(processor.process(&vec![0.1; 882]).unwrap());
        }
        output.extend(processor.finish().unwrap());
        assert!((output.len() as i64 - 96000).abs() <= 4);
        assert!(output.iter().all(|v| v.is_finite()));
    }
    #[test]
    fn mix_bus_fills_missing_device_and_limits_sum() {
        let mut mix = AudioMixer::new(2).unwrap();
        mix.push(0, &[0.8, 0.8, 0.5, 0.5]).unwrap();
        mix.push(1, &[0.8, 0.8]).unwrap();
        let samples = mix.until(2.0 / 48000.0).unwrap();
        assert_eq!(samples, vec![0.95, 0.95, 0.5, 0.5]);
        assert!(mix.push(0, &vec![0.0; 96002]).is_err());
    }
}
