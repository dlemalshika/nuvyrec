//! Encoded-packet replay history: no raw frames, hard byte and duration limits.
use crate::{error::check, output::Output, Error, Result};
use ffmpeg_sys_next as av;
use std::{collections::VecDeque, path::Path, ptr};

pub(crate) struct Packet {
    pub bytes: Vec<u8>,
    pub stream: i32,
    pub pts: i64,
    pub dts: i64,
    pub duration: i64,
    pub flags: i32,
    pub seconds: f64,
}
pub(crate) struct Replay {
    packets: VecDeque<Packet>,
    bytes: usize,
    limit: usize,
    seconds: f64,
    latest: f64,
}
impl Replay {
    pub fn new(seconds: u32, limit: usize) -> Result<Self> {
        if !(5..=120).contains(&seconds) || !(4 * 1024 * 1024..=128 * 1024 * 1024).contains(&limit)
        {
            return Err(Error::Invalid(
                "replay requires 5–120 seconds and 4–128 MiB",
            ));
        }
        Ok(Self {
            packets: VecDeque::new(),
            bytes: 0,
            limit,
            seconds: f64::from(seconds),
            latest: 0.0,
        })
    }
    pub unsafe fn record(
        &mut self,
        context: *mut av::AVFormatContext,
        packet: *const av::AVPacket,
    ) -> Result<()> {
        unsafe {
            let p = &*packet;
            if p.size == 0 {
                return Ok(());
            }
            if p.size < 0
                || p.size as usize > self.limit
                || p.stream_index < 0
                || p.stream_index as u32 >= (*context).nb_streams
            {
                return Err(Error::Invalid("replay packet exceeds configured bounds"));
            }
            let stream = *(*context).streams.add(p.stream_index as usize);
            let time = (*stream).time_base;
            let pts = if p.pts == av::AV_NOPTS_VALUE {
                p.dts
            } else {
                p.pts
            };
            if pts == av::AV_NOPTS_VALUE {
                return Ok(());
            }
            let seconds = pts as f64 * f64::from(time.num) / f64::from(time.den);
            if p.stream_index == 0 {
                self.latest = seconds;
            }
            while self.bytes + p.size as usize > self.limit
                || self.packets.len() >= 32768
                || self
                    .packets
                    .front()
                    .is_some_and(|old| old.seconds < self.latest - self.seconds - 2.0)
            {
                if let Some(old) = self.packets.pop_front() {
                    self.bytes -= old.bytes.len();
                } else {
                    break;
                }
            }
            let bytes = if p.size == 0 {
                Vec::new()
            } else {
                if p.data.is_null() {
                    return Err(Error::State("null replay packet"));
                }
                std::slice::from_raw_parts(p.data, p.size as usize).to_vec()
            };
            self.bytes += bytes.len();
            self.packets.push_back(Packet {
                bytes,
                stream: p.stream_index,
                pts: p.pts,
                dts: p.dts,
                duration: p.duration,
                flags: p.flags,
                seconds,
            });
        }
        Ok(())
    }
    fn start(&self) -> Option<f64> {
        self.packets
            .iter()
            .find(|p| {
                p.stream == 0
                    && p.flags & av::AV_PKT_FLAG_KEY != 0
                    && p.seconds >= self.latest - self.seconds
            })
            .map(|p| p.seconds)
    }
    pub fn available(&self) -> f64 {
        self.start()
            .map_or(0.0, |start| (self.latest - start).max(0.0))
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub unsafe fn save(&self, source: *mut av::AVFormatContext, path: &Path) -> Result<()> {
        let start = self
            .start()
            .ok_or(Error::State("Replay is waiting for a keyframe"))?;
        if self.available() < 0.2 {
            return Err(Error::State("Replay needs more recorded frames"));
        }
        let mut output = Output::new(c"matroska")?;
        unsafe {
            for index in 0..(*source).nb_streams as usize {
                let original = *(*source).streams.add(index);
                let stream = av::avformat_new_stream(output.raw, ptr::null());
                if stream.is_null() {
                    return Err(Error::Allocation("replay stream"));
                }
                check(
                    av::avcodec_parameters_copy((*stream).codecpar, (*original).codecpar),
                    "copy replay stream",
                )?;
                (*(*stream).codecpar).codec_tag = 0;
                (*stream).time_base = (*original).time_base;
                av::av_dict_copy(&mut (*stream).metadata, (*original).metadata, 0);
            }
            output.open_file(path)?;
            check(
                av::avformat_write_header(output.raw, ptr::null_mut()),
                "write replay header",
            )?;
            for stored in self.packets.iter().filter(|p| p.seconds >= start) {
                let mut packet = av::av_packet_alloc();
                if packet.is_null() {
                    return Err(Error::Allocation("replay packet"));
                }
                let initialized = av::av_new_packet(packet, stored.bytes.len() as i32);
                if initialized < 0 {
                    av::av_packet_free(&mut packet);
                    check(initialized, "allocate replay bytes")?;
                }
                if !stored.bytes.is_empty() {
                    ptr::copy_nonoverlapping(
                        stored.bytes.as_ptr(),
                        (*packet).data,
                        stored.bytes.len(),
                    );
                }
                let original = *(*source).streams.add(stored.stream as usize);
                let destination = *(*output.raw).streams.add(stored.stream as usize);
                let base = (*original).time_base;
                let offset = (start * f64::from(base.den) / f64::from(base.num)).round() as i64;
                (*packet).pts = if stored.pts == av::AV_NOPTS_VALUE {
                    stored.pts
                } else {
                    stored.pts - offset
                };
                (*packet).dts = if stored.dts == av::AV_NOPTS_VALUE {
                    stored.dts
                } else {
                    stored.dts - offset
                };
                (*packet).duration = stored.duration;
                (*packet).stream_index = stored.stream;
                (*packet).flags = stored.flags;
                av::av_packet_rescale_ts(packet, base, (*destination).time_base);
                let written = av::av_interleaved_write_frame(output.raw, packet);
                av::av_packet_free(&mut packet);
                check(written, "mux replay")?;
            }
            check(av::av_write_trailer(output.raw), "finish replay")?;
        }
        output.sync()
    }
}
