//! Native encoded-packet fan-out. Capture/encoding never performs network IO.
//!
//! Each destination owns its FFmpeg muxer, codec parameter copies, bounded
//! queue, interrupt deadline and reconnect state. No URL/key is exposed by
//! status, Debug, errors or logs. One slow receiver cannot stall another.
use crate::{error::check, Error, Result};
use ffmpeg_sys_next as av;
use std::{
    collections::VecDeque,
    ffi::CString,
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const QUEUE_BYTES: usize = 4 * 1024 * 1024;
const QUEUE_PACKETS: usize = 256;
const IO_TIMEOUT_MS: u64 = 5000;
static NETWORK_INIT: OnceLock<i32> = OnceLock::new();

/// Credentials stay in memory. Deliberately implements neither Debug nor Serialize.
pub struct Destination {
    pub platform: String,
    pub server: String,
    pub key: String,
}
impl Destination {
    pub fn validate(&self) -> Result<()> {
        if !["youtube", "facebook", "tiktok"].contains(&self.platform.as_str()) {
            return Err(Error::Invalid("unknown streaming platform"));
        }
        let rest = self
            .server
            .strip_prefix("rtmps://")
            .or_else(|| self.server.strip_prefix("rtmp://"))
            .ok_or(Error::Invalid("stream server must use RTMP or RTMPS"))?;
        let host = rest.split('/').next().unwrap_or_default();
        if host.is_empty()
            || host.contains('@')
            || self.server.contains(['?', '#'])
            || self.server.len() > 4096
            || self.key.is_empty()
            || self.key.len() > 2048
            || self
                .server
                .chars()
                .chain(self.key.chars())
                .any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(Error::Invalid("invalid stream server or key"));
        }
        Ok(())
    }
    fn url(&self) -> CString {
        // Validation rejects NUL, spaces and FFmpeg's URL-option injection syntax.
        CString::new(format!(
            "{}/{}",
            self.server.trim_end_matches('/'),
            self.key
        ))
        .expect("validated streaming URL")
    }
}

#[derive(Clone)]
pub struct DestinationStatus {
    pub platform: String,
    /// connecting / waiting_keyframe / sending / reconnecting / stopped
    pub state: &'static str,
    pub packets: u64,
    pub bytes: u64,
    pub reconnects: u64,
    pub dropped_packets: u64,
    pub error: Option<&'static str>,
    pub error_code: Option<i32>,
    pub error_operation: Option<&'static str>,
}

// SAFETY: one exclusively owned deep copy moves to one network thread, never
// shared. FFmpeg parameter allocation/copy/free has no capture-device affinity.
struct CodecParameters(*mut av::AVCodecParameters);
unsafe impl Send for CodecParameters {}
impl Drop for CodecParameters {
    fn drop(&mut self) {
        unsafe {
            av::avcodec_parameters_free(&mut self.0);
        }
    }
}
struct Track {
    parameters: CodecParameters,
    time: av::AVRational,
    source: i32,
}
impl Track {
    unsafe fn copy(source: *mut av::AVStream, index: i32) -> Result<Self> {
        unsafe {
            let parameters = CodecParameters(av::avcodec_parameters_alloc());
            if parameters.0.is_null() {
                return Err(Error::Allocation("stream parameters"));
            }
            check(
                av::avcodec_parameters_copy(parameters.0, (*source).codecpar),
                "copy stream parameters",
            )?;
            Ok(Self {
                parameters,
                time: (*source).time_base,
                source: index,
            })
        }
    }
}
struct Packet {
    bytes: Arc<[u8]>,
    source: i32,
    pts: i64,
    dts: i64,
    duration: i64,
    flags: i32,
}
impl Packet {
    fn keyframe(&self) -> bool {
        self.source == 0 && self.flags & av::AV_PKT_FLAG_KEY != 0
    }
}
struct Queue {
    packets: VecDeque<Arc<Packet>>,
    bytes: usize,
    generation: u64,
    waiting: bool,
    status: DestinationStatus,
}
struct Endpoint {
    queue: Mutex<Queue>,
    ready: Condvar,
    stop: Arc<AtomicBool>,
}
impl Endpoint {
    fn failure_code(&self, error: &Error) {
        if let Ok(mut q) = self.queue.lock() {
            if let Error::Ffmpeg {
                operation, code, ..
            } = error
            {
                q.status.error_code = Some(*code);
                q.status.error_operation = Some(operation);
            }
        }
    }
    fn reset(&self, error: &'static str) {
        if let Ok(mut q) = self.queue.lock() {
            q.status.dropped_packets += q.packets.len() as u64;
            q.packets.clear();
            q.bytes = 0;
            q.waiting = true;
            q.generation += 1;
            q.status.state = "reconnecting";
            q.status.error = Some(error);
            q.status.reconnects += 1;
        }
    }
}

pub(crate) struct Streaming {
    endpoints: Vec<Arc<Endpoint>>,
    threads: Vec<JoinHandle<()>>,
    audio_source: Option<i32>,
}
impl Streaming {
    pub unsafe fn new(source: *mut av::AVFormatContext, targets: Vec<Destination>) -> Result<Self> {
        if targets.is_empty() || targets.len() > 3 {
            return Err(Error::Invalid("choose one to three stream destinations"));
        }
        for (index, target) in targets.iter().enumerate() {
            target.validate()?;
            if targets[..index]
                .iter()
                .any(|t| t.platform == target.platform)
            {
                return Err(Error::Invalid("duplicate stream destination"));
            }
        }
        unsafe {
            let video = *(*source).streams;
            if (*(*video).codecpar).codec_id != av::AVCodecID::AV_CODEC_ID_H264 {
                return Err(Error::Invalid("streaming requires H.264"));
            }
            let audio_source = (1..(*source).nb_streams)
                .find(|i| {
                    let s = *(*source).streams.add(*i as usize);
                    (*(*s).codecpar).codec_id == av::AVCodecID::AV_CODEC_ID_AAC
                })
                .map(|i| i as i32);
            // FFmpeg may print credential-bearing URLs from protocol code. Mute
            // its process-wide logger before any network IO. Our own numeric,
            // credential-free status replaces it; no temporary global toggles.
            av::av_log_set_level(av::AV_LOG_QUIET);
            check(
                *NETWORK_INIT.get_or_init(|| av::avformat_network_init()),
                "initialize network",
            )?;
            let mut result = Self {
                endpoints: Vec::new(),
                threads: Vec::new(),
                audio_source,
            };
            for target in targets {
                let mut tracks = vec![Track::copy(video, 0)?];
                if let Some(index) = audio_source {
                    tracks.push(Track::copy(*(*source).streams.add(index as usize), index)?);
                }
                let endpoint = Arc::new(Endpoint {
                    queue: Mutex::new(Queue {
                        packets: VecDeque::new(),
                        bytes: 0,
                        generation: 0,
                        waiting: true,
                        status: DestinationStatus {
                            platform: target.platform.clone(),
                            state: "connecting",
                            packets: 0,
                            bytes: 0,
                            reconnects: 0,
                            dropped_packets: 0,
                            error: None,
                            error_code: None,
                            error_operation: None,
                        },
                    }),
                    ready: Condvar::new(),
                    stop: Arc::new(AtomicBool::new(false)),
                });
                let worker = endpoint.clone();
                let handle = thread::Builder::new()
                    .name(format!("stream-{}", target.platform))
                    .spawn(move || run(target, tracks, worker))?;
                result.endpoints.push(endpoint);
                result.threads.push(handle);
            }
            Ok(result)
        }
    }
    /// Copies encoded bytes once, then shares immutable storage among targets.
    /// At most 4 MiB + one in-flight packet per target; never holds raw frames.
    pub unsafe fn submit(&self, raw: *const av::AVPacket) {
        unsafe {
            let p = &*raw;
            if p.stream_index != 0 && Some(p.stream_index) != self.audio_source {
                return;
            }
            if p.size > 0 && p.size as usize > QUEUE_BYTES {
                for endpoint in &self.endpoints {
                    endpoint.reset("Encoded packet exceeds the upload queue budget");
                }
                return;
            }
            if p.size <= 0 || p.data.is_null() {
                return;
            }
            if self
                .endpoints
                .iter()
                .all(|e| e.stop.load(Ordering::Acquire))
            {
                return;
            }
            let packet = Arc::new(Packet {
                bytes: Arc::from(std::slice::from_raw_parts(p.data, p.size as usize)),
                source: p.stream_index,
                pts: p.pts,
                dts: p.dts,
                duration: p.duration,
                flags: p.flags,
            });
            for endpoint in &self.endpoints {
                if endpoint.stop.load(Ordering::Acquire) {
                    continue;
                }
                // This lock only protects a tiny deque operation, never IO or
                // codec work. FIFO ordering cannot skip a random P-frame.
                if let Ok(mut q) = endpoint.queue.lock() {
                    if q.bytes + packet.bytes.len() > QUEUE_BYTES
                        || q.packets.len() >= QUEUE_PACKETS
                    {
                        q.status.dropped_packets += q.packets.len() as u64;
                        q.packets.clear();
                        q.bytes = 0;
                        q.waiting = true;
                        q.generation += 1;
                    }
                    if q.waiting && !packet.keyframe() {
                        q.status.dropped_packets += 1;
                        continue;
                    }
                    q.waiting = false;
                    q.bytes += packet.bytes.len();
                    q.packets.push_back(packet.clone());
                    endpoint.ready.notify_one();
                }
            }
        }
    }
    pub fn status(&self) -> Vec<DestinationStatus> {
        self.endpoints
            .iter()
            .filter_map(|e| e.queue.lock().ok().map(|q| q.status.clone()))
            .collect()
    }
    pub fn stop(&mut self) {
        // Signal ALL workers before joining any: stalled connections stop in
        // parallel through AVIOInterruptCB, rather than three serial timeouts.
        self.request_stop();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
    pub fn request_stop(&self) {
        for e in &self.endpoints {
            e.stop.store(true, Ordering::Release);
            e.ready.notify_all();
        }
    }
}
impl Drop for Streaming {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Interrupt {
    stop: Arc<AtomicBool>,
    origin: Instant,
    deadline: AtomicU64,
}
impl Interrupt {
    fn arm(&self) {
        self.deadline.store(
            self.origin.elapsed().as_millis() as u64 + IO_TIMEOUT_MS,
            Ordering::Release,
        );
    }
}
unsafe extern "C" fn interrupt(opaque: *mut libc::c_void) -> i32 {
    let context = unsafe { &*opaque.cast::<Interrupt>() };
    i32::from(
        context.stop.load(Ordering::Acquire)
            || context.origin.elapsed().as_millis() as u64
                >= context.deadline.load(Ordering::Acquire),
    )
}
struct Connection {
    raw: *mut av::AVFormatContext,
    interrupt: Box<Interrupt>,
    header: bool,
}
impl Connection {
    fn open(target: &Destination, tracks: &[Track], stop: Arc<AtomicBool>) -> Result<Self> {
        let mut connection = Self {
            raw: ptr::null_mut(),
            interrupt: Box::new(Interrupt {
                stop,
                origin: Instant::now(),
                deadline: AtomicU64::new(IO_TIMEOUT_MS),
            }),
            header: false,
        };
        unsafe {
            check(
                av::avformat_alloc_output_context2(
                    &mut connection.raw,
                    ptr::null_mut(),
                    c"flv".as_ptr(),
                    ptr::null(),
                ),
                "stream muxer",
            )?;
            if connection.raw.is_null() {
                return Err(Error::Allocation("stream muxer"));
            }
            (*connection.raw).interrupt_callback = av::AVIOInterruptCB {
                callback: Some(interrupt),
                opaque: (&mut *connection.interrupt as *mut Interrupt).cast(),
            };
            (*connection.raw).max_interleave_delta = 200_000;
            (*connection.raw).flags |= av::AVFMT_FLAG_FLUSH_PACKETS;
            for track in tracks {
                let stream = av::avformat_new_stream(connection.raw, ptr::null());
                if stream.is_null() {
                    return Err(Error::Allocation("stream track"));
                }
                check(
                    av::avcodec_parameters_copy((*stream).codecpar, track.parameters.0),
                    "stream parameters",
                )?;
                (*(*stream).codecpar).codec_tag = 0;
                (*stream).time_base = track.time;
            }
            let mut options = ptr::null_mut();
            av::av_dict_set(&mut options, c"rw_timeout".as_ptr(), c"5000000".as_ptr(), 0);
            av::av_dict_set(&mut options, c"tls_verify".as_ptr(), c"1".as_ptr(), 0);
            av::av_dict_set(
                &mut options,
                c"protocol_whitelist".as_ptr(),
                c"rtmp,rtmps,tcp,tls".as_ptr(),
                0,
            );
            // The URL may contain a key; never pass it to Error/Debug/status.
            let opened = av::avio_open2(
                &mut (*connection.raw).pb,
                target.url().as_ptr(),
                av::AVIO_FLAG_WRITE,
                &(*connection.raw).interrupt_callback,
                &mut options,
            );
            av::av_dict_free(&mut options);
            check(opened, "connect stream")?;
            connection.interrupt.arm();
            let mut options = ptr::null_mut();
            av::av_dict_set(
                &mut options,
                c"flvflags".as_ptr(),
                c"no_duration_filesize".as_ptr(),
                0,
            );
            let written = av::avformat_write_header(connection.raw, &mut options);
            av::av_dict_free(&mut options);
            check(written, "stream header")?;
            connection.header = true;
        }
        Ok(connection)
    }
    fn write(&mut self, stored: &Packet, tracks: &[Track], origin_us: i64) -> Result<()> {
        let Some(index) = tracks.iter().position(|t| t.source == stored.source) else {
            return Ok(());
        };
        unsafe {
            let mut packet = av::av_packet_alloc();
            if packet.is_null() {
                return Err(Error::Allocation("stream packet"));
            }
            let result = (|| {
                check(
                    av::av_new_packet(packet, stored.bytes.len() as i32),
                    "stream bytes",
                )?;
                ptr::copy_nonoverlapping(stored.bytes.as_ptr(), (*packet).data, stored.bytes.len());
                let offset = av::av_rescale_q(
                    origin_us,
                    av::AVRational {
                        num: 1,
                        den: 1_000_000,
                    },
                    tracks[index].time,
                );
                if stored.dts == av::AV_NOPTS_VALUE
                    || stored.pts == av::AV_NOPTS_VALUE
                    || stored.dts < offset
                {
                    return Ok(()); // delayed audio before the reconnection keyframe
                }
                (*packet).pts = stored.pts - offset;
                (*packet).dts = stored.dts - offset;
                (*packet).duration = stored.duration;
                (*packet).flags = stored.flags;
                (*packet).stream_index = index as i32;
                av::av_packet_rescale_ts(
                    packet,
                    tracks[index].time,
                    (**(*self.raw).streams.add(index)).time_base,
                );
                self.interrupt.arm();
                check(
                    av::av_interleaved_write_frame(self.raw, packet),
                    "send stream packet",
                )?;
                av::avio_flush((*self.raw).pb);
                check((*(*self.raw).pb).error, "flush stream packet")
            })();
            av::av_packet_free(&mut packet);
            result
        }
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        if self.raw.is_null() {
            return;
        }
        unsafe {
            self.interrupt.arm();
            if self.header {
                av::av_write_trailer(self.raw);
            }
            av::avio_closep(&mut (*self.raw).pb);
            av::avformat_free_context(self.raw);
        }
    }
}
fn run(target: Destination, tracks: Vec<Track>, endpoint: Arc<Endpoint>) {
    let mut backoff = 1u64;
    while !endpoint.stop.load(Ordering::Acquire) {
        let connection = Connection::open(&target, &tracks, endpoint.stop.clone());
        if let Ok(mut connection) = connection {
            let mut origin_us = None;
            let mut generation = None;
            if let Ok(mut q) = endpoint.queue.lock() {
                q.status.state = "waiting_keyframe";
            }
            loop {
                if endpoint.stop.load(Ordering::Acquire) {
                    break;
                }
                let (packet, current) = {
                    let Ok(mut q) = endpoint.queue.lock() else {
                        break;
                    };
                    if q.packets.is_empty() {
                        let Ok((next, _)) =
                            endpoint.ready.wait_timeout(q, Duration::from_millis(100))
                        else {
                            break;
                        };
                        q = next;
                    }
                    let packet = q.packets.pop_front();
                    if let Some(p) = &packet {
                        q.bytes -= p.bytes.len();
                    }
                    (packet, q.generation)
                };
                if generation.is_some_and(|old| old != current) {
                    endpoint.reset("Upload queue filled; restarting at a keyframe");
                    break;
                }
                let Some(packet) = packet else {
                    continue;
                };
                generation = Some(current);
                if origin_us.is_none() {
                    if !packet.keyframe() {
                        continue;
                    }
                    origin_us = Some(unsafe {
                        av::av_rescale_q(
                            packet.dts,
                            tracks[0].time,
                            av::AVRational {
                                num: 1,
                                den: 1_000_000,
                            },
                        )
                    });
                }
                if let Err(error) = connection.write(&packet, &tracks, origin_us.unwrap_or(0)) {
                    endpoint
                        .reset("Connection interrupted; check server/key and upload connection");
                    endpoint.failure_code(&error);
                    break;
                }
                backoff = 1;
                if let Ok(mut q) = endpoint.queue.lock() {
                    q.status.state = "sending";
                    q.status.error = None;
                    q.status.error_code = None;
                    q.status.error_operation = None;
                    q.status.packets += 1;
                    q.status.bytes += packet.bytes.len() as u64;
                }
            }
        } else {
            endpoint.reset("Could not connect; check server/key and platform LIVE access");
            if let Err(error) = connection {
                endpoint.failure_code(&error);
            }
        }
        // Interruptible retry delay; no capture thread sleep or IO.
        if let Ok(q) = endpoint.queue.lock() {
            let _ = endpoint
                .ready
                .wait_timeout_while(q, Duration::from_secs(backoff), |_| {
                    !endpoint.stop.load(Ordering::Acquire)
                });
        }
        backoff = (backoff * 2).min(10);
    }
    if let Ok(mut q) = endpoint.queue.lock() {
        q.packets.clear();
        q.bytes = 0;
        q.status.state = "stopped";
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn endpoint() -> Arc<Endpoint> {
        Arc::new(Endpoint {
            queue: Mutex::new(Queue {
                packets: VecDeque::new(),
                bytes: 0,
                generation: 0,
                waiting: true,
                status: DestinationStatus {
                    platform: "youtube".into(),
                    state: "connecting",
                    packets: 0,
                    bytes: 0,
                    reconnects: 0,
                    dropped_packets: 0,
                    error: None,
                    error_code: None,
                    error_operation: None,
                },
            }),
            ready: Condvar::new(),
            stop: Arc::new(AtomicBool::new(false)),
        })
    }
    #[test]
    fn overflow_waits_for_a_keyframe_and_preserves_other_destinations() {
        let slow = endpoint();
        let fast = endpoint();
        let streaming = Streaming {
            endpoints: vec![slow.clone(), fast.clone()],
            threads: Vec::new(),
            audio_source: Some(1),
        };
        let mut bytes = vec![1u8; 1024 * 1024];
        let mut packet: av::AVPacket = unsafe { std::mem::zeroed() };
        packet.data = bytes.as_mut_ptr();
        packet.size = bytes.len() as i32;
        packet.flags = av::AV_PKT_FLAG_KEY;
        unsafe {
            streaming.submit(&packet);
        }
        packet.flags = 0;
        for _ in 0..4 {
            // Simulate a healthy destination consuming all queued packets.
            {
                let mut q = fast.queue.lock().unwrap();
                q.packets.clear();
                q.bytes = 0;
            }
            unsafe {
                streaming.submit(&packet);
            }
        }
        {
            let q = slow.queue.lock().unwrap();
            assert!(q.waiting);
            assert_eq!(q.bytes, 0);
            assert_eq!(q.generation, 1);
        }
        assert_eq!(fast.queue.lock().unwrap().generation, 0);
        packet.stream_index = 1;
        unsafe {
            streaming.submit(&packet);
        } // audio cannot restart a broken GOP
        assert_eq!(slow.queue.lock().unwrap().bytes, 0);
        packet.stream_index = 0;
        packet.flags = av::AV_PKT_FLAG_KEY;
        unsafe {
            streaming.submit(&packet);
        }
        assert_eq!(slow.queue.lock().unwrap().bytes, bytes.len());
        packet.stream_index = 2; // local independent microphone never gets published
        unsafe {
            streaming.submit(&packet);
        }
        assert_eq!(slow.queue.lock().unwrap().bytes, bytes.len());
        streaming.request_stop();
        assert!(slow.stop.load(Ordering::Acquire));
        assert!(fast.stop.load(Ordering::Acquire));
    }
    #[test]
    fn network_callback_observes_deadlines_and_stop_without_locking() {
        let mut context = Interrupt {
            stop: Arc::new(AtomicBool::new(false)),
            origin: Instant::now(),
            deadline: AtomicU64::new(0),
        };
        let pointer = (&mut context as *mut Interrupt).cast();
        assert_eq!(unsafe { interrupt(pointer) }, 1);
        context.arm();
        assert_eq!(unsafe { interrupt(pointer) }, 0);
        context.stop.store(true, Ordering::Release);
        assert_eq!(unsafe { interrupt(pointer) }, 1);
    }
    #[test]
    fn validates_without_echoing_credentials() {
        let mut d = Destination {
            platform: "youtube".into(),
            server: "rtmps://example.invalid/live".into(),
            key: "secret?token=123".into(),
        };
        assert!(d.validate().is_ok());
        for server in [
            "file:///tmp/stream",
            "https://example.invalid",
            "rtmp://",
            "rtmp://user:secret@host/live",
            "rtmp://host/live timeout=1",
            "rtmp://host/live?secret",
        ] {
            d.server = server.into();
            let error = d.validate().unwrap_err().to_string();
            assert!(!error.contains("secret"));
        }
        d.server = "rtmp://localhost/live".into();
        d.key = "key\0bad".into();
        assert!(d.validate().is_err());
    }
}
