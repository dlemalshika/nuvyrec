//! Opt-in DirectShow camera reader. FFmpeg objects stay on one worker thread.
//! Only the latest bounded BGRA image crosses threads; screen capture never
//! waits for a camera frame. No camera microphone is opened.
use crate::{error::check, Error, Result, VideoFrame};
use ffmpeg_sys_next as av;
use std::{
    ffi::{CStr, CString},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock, Weak,
    },
    thread,
    time::{Duration, Instant},
};
static CAMERA_BUSY: AtomicBool = AtomicBool::new(false);
static SHARED_CAMERA: OnceLock<Mutex<Weak<CameraLife>>> = OnceLock::new();
static REGISTER_DEVICES: std::sync::Once = std::sync::Once::new();
struct Lease;
impl Drop for Lease {
    fn drop(&mut self) {
        CAMERA_BUSY.store(false, Ordering::Release);
    }
}

pub fn devices() -> Result<Vec<(String, String)>> {
    REGISTER_DEVICES.call_once(|| unsafe { av::avdevice_register_all() });
    unsafe {
        let format = av::av_find_input_format(c"dshow".as_ptr());
        if format.is_null() {
            return Err(Error::State("DirectShow unavailable"));
        }
        let mut list = ptr::null_mut();
        let code = av::avdevice_list_input_sources(format, ptr::null(), ptr::null_mut(), &mut list);
        let result = (|| {
            check(code, "enumerate cameras")?;
            let mut result = Vec::new();
            if !list.is_null() {
                for index in 0..(*list).nb_devices {
                    let device = *(*list).devices.add(index as usize);
                    if device.is_null() || (*device).device_name.is_null() {
                        continue;
                    }
                    if (*device).nb_media_types <= 0 || (*device).media_types.is_null() {
                        continue;
                    }
                    let types = std::slice::from_raw_parts(
                        (*device).media_types,
                        (*device).nb_media_types as usize,
                    );
                    if !types.contains(&av::AVMediaType::AVMEDIA_TYPE_VIDEO) {
                        continue;
                    }
                    let id = CStr::from_ptr((*device).device_name)
                        .to_string_lossy()
                        .into_owned();
                    let name = if (*device).device_description.is_null() {
                        id.clone()
                    } else {
                        CStr::from_ptr((*device).device_description)
                            .to_string_lossy()
                            .into_owned()
                    };
                    result.push((id, name));
                }
            }
            Ok(result)
        })();
        av::avdevice_free_list_devices(&mut list);
        result
    }
}

pub struct Image {
    pub width: usize,
    pub height: usize,
    pub bgra: Vec<u8>,
    pub captured: Instant,
}
pub struct Camera {
    pub latest: Arc<Mutex<Option<Image>>>,
    pub error: Arc<Mutex<Option<String>>>,
    _life: Arc<CameraLife>,
}
/// Preview and recording lease the same reader, never open a device twice.
/// The last lease requests shutdown; a stuck driver retains only its one worker.
struct CameraLife {
    device: String,
    max_width: usize,
    latest: Arc<Mutex<Option<Image>>>,
    error: Arc<Mutex<Option<String>>>,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}
impl Camera {
    pub fn start(device: String, max_width: usize) -> Result<Self> {
        if !(160..=640).contains(&max_width) {
            return Err(Error::Invalid("invalid camera budget"));
        }
        let mut shared = SHARED_CAMERA
            .get_or_init(|| Mutex::new(Weak::new()))
            .lock()
            .map_err(|_| Error::State("camera lease lock failed"))?;
        if let Some(life) = shared.upgrade() {
            if life.device != device || life.max_width != max_width {
                return Err(Error::State("camera settings are changing; retry shortly"));
            }
            return Ok(Self {
                latest: life.latest.clone(),
                error: life.error.clone(),
                _life: life,
            });
        }
        if CAMERA_BUSY
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(Error::State(
                "previous camera is still closing; retry shortly",
            ));
        }
        let lease = Lease;
        let latest = Arc::new(Mutex::new(None));
        let error = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (image, failure, cancel) = (latest.clone(), error.clone(), stop.clone());
        let reader_device = device.clone();
        let thread = thread::Builder::new()
            .name("frame-camera".into())
            .spawn(move || {
                let _lease = lease;
                if let Err(e) = read_camera(&reader_device, max_width, cancel, image) {
                    if let Ok(mut value) = failure.lock() {
                        *value = Some(e.to_string());
                    }
                }
            })
            .map_err(|_| Error::State("camera worker creation failed"))?;
        let life = Arc::new(CameraLife {
            device,
            max_width,
            latest: latest.clone(),
            error: error.clone(),
            stop,
            thread: Mutex::new(Some(thread)),
        });
        *shared = Arc::downgrade(&life);
        Ok(Self {
            latest,
            error,
            _life: life,
        })
    }
}
impl Drop for CameraLife {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Drivers may ignore FFmpeg's interrupt callback. Never block UI/recording
        // finalization on such a driver; its thread owns all retained resources.
        if let Some(thread) = self
            .thread
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            if thread.is_finished() {
                let _ = thread.join();
            }
        }
    }
}
struct Interrupt {
    stop: Arc<AtomicBool>,
    deadline: Instant,
}
unsafe extern "C" fn interrupt(opaque: *mut std::ffi::c_void) -> i32 {
    let state = unsafe { &*(opaque as *const Interrupt) };
    i32::from(state.stop.load(Ordering::Acquire) || Instant::now() > state.deadline)
}
struct Input {
    format: *mut av::AVFormatContext,
    decoder: *mut av::AVCodecContext,
    packet: *mut av::AVPacket,
    frame: *mut av::AVFrame,
    scale: *mut av::SwsContext,
}
impl Drop for Input {
    fn drop(&mut self) {
        unsafe {
            av::sws_freeContext(self.scale);
            av::av_frame_free(&mut self.frame);
            av::av_packet_free(&mut self.packet);
            av::avcodec_free_context(&mut self.decoder);
            av::avformat_close_input(&mut self.format);
        }
    }
}
fn read_camera(
    device: &str,
    max_width: usize,
    stop: Arc<AtomicBool>,
    latest: Arc<Mutex<Option<Image>>>,
) -> Result<()> {
    if !devices()?.iter().any(|(id, _)| id == device) {
        return Err(Error::Invalid(
            "selected webcam unavailable; refresh camera devices",
        ));
    }
    let mut interrupt_state = Box::new(Interrupt {
        stop: stop.clone(),
        deadline: Instant::now() + Duration::from_secs(8),
    });
    let url = CString::new(format!("video={device}"))
        .map_err(|_| Error::Invalid("invalid camera device"))?;
    let mut input = Input {
        format: unsafe { av::avformat_alloc_context() },
        decoder: ptr::null_mut(),
        packet: unsafe { av::av_packet_alloc() },
        frame: unsafe { av::av_frame_alloc() },
        scale: ptr::null_mut(),
    };
    if input.format.is_null() || input.packet.is_null() || input.frame.is_null() {
        return Err(Error::Allocation("camera allocation"));
    }
    unsafe {
        (*input.format).interrupt_callback = av::AVIOInterruptCB {
            callback: Some(interrupt),
            opaque: (&mut *interrupt_state as *mut Interrupt).cast(),
        };
        let mut options = ptr::null_mut();
        av::av_dict_set(&mut options, c"rtbufsize".as_ptr(), c"8388608".as_ptr(), 0);
        let code = av::avformat_open_input(
            &mut input.format,
            url.as_ptr(),
            av::av_find_input_format(c"dshow".as_ptr()),
            &mut options,
        );
        av::av_dict_free(&mut options);
        check(
            code,
            "open webcam (check camera privacy permissions/device use)",
        )?;
        check(
            av::avformat_find_stream_info(input.format, ptr::null_mut()),
            "inspect camera stream",
        )?;
        let mut decoder = ptr::null();
        let stream = av::av_find_best_stream(
            input.format,
            av::AVMediaType::AVMEDIA_TYPE_VIDEO,
            -1,
            -1,
            &mut decoder,
            0,
        );
        check(stream, "find camera video")?;
        input.decoder = av::avcodec_alloc_context3(decoder);
        if input.decoder.is_null() {
            return Err(Error::Allocation("camera decoder"));
        }
        check(
            av::avcodec_parameters_to_context(
                input.decoder,
                (*(*(*input.format).streams.add(stream as usize))).codecpar,
            ),
            "camera codec parameters",
        )?;
        if (*input.decoder).width > 4096 || (*input.decoder).height > 2160 {
            return Err(Error::Invalid(
                "camera input exceeds 4096 x 2160 decode budget",
            ));
        }
        (*input.decoder).thread_count = 1;
        check(
            av::avcodec_open2(input.decoder, decoder, ptr::null_mut()),
            "open camera decoder",
        )?;
        let mut last = Instant::now() - Duration::from_secs(1);
        let mut reusable = Vec::new();
        while !stop.load(Ordering::Acquire) {
            interrupt_state.deadline = Instant::now() + Duration::from_secs(5);
            let read = av::av_read_frame(input.format, input.packet);
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            check(read, "read webcam (disconnected or stalled)")?;
            if (*input.packet).stream_index == stream {
                let sent = av::avcodec_send_packet(input.decoder, input.packet);
                av::av_packet_unref(input.packet);
                check(sent, "decode camera packet")?;
                loop {
                    let received = av::avcodec_receive_frame(input.decoder, input.frame);
                    if received == -libc::EAGAIN || received == av::AVERROR_EOF {
                        break;
                    }
                    check(received, "receive camera frame")?;
                    let frame = &*input.frame;
                    if frame.width < 2
                        || frame.height < 2
                        || frame.width > 4096
                        || frame.height > 2160
                    {
                        return Err(Error::Invalid(
                            "camera resolution exceeds bounded decoder budget",
                        ));
                    }
                    if last.elapsed() >= Duration::from_millis(67) {
                        let ratio = (max_width as f64 / frame.width as f64)
                            .min(480.0 / frame.height as f64)
                            .min(1.0);
                        let w = ((frame.width as f64 * ratio) as usize / 2 * 2).max(2);
                        let h = ((frame.height as f64 * ratio) as usize / 2 * 2).max(2);
                        if frame.format < 0
                            || frame.format >= av::AVPixelFormat::AV_PIX_FMT_NB as i32
                        {
                            return Err(Error::Invalid("unknown camera pixel format"));
                        }
                        // Normalize legacy JPEG formats: libswscale internally
                        // rewrites YUVJ to YUV, defeating cache reuse otherwise.
                        let source_format =
                            std::mem::transmute::<i32, av::AVPixelFormat>(frame.format);
                        let normalized = match source_format {
                            av::AVPixelFormat::AV_PIX_FMT_YUVJ420P => {
                                av::AVPixelFormat::AV_PIX_FMT_YUV420P
                            }
                            av::AVPixelFormat::AV_PIX_FMT_YUVJ422P => {
                                av::AVPixelFormat::AV_PIX_FMT_YUV422P
                            }
                            av::AVPixelFormat::AV_PIX_FMT_YUVJ444P => {
                                av::AVPixelFormat::AV_PIX_FMT_YUV444P
                            }
                            av::AVPixelFormat::AV_PIX_FMT_YUVJ440P => {
                                av::AVPixelFormat::AV_PIX_FMT_YUV440P
                            }
                            av::AVPixelFormat::AV_PIX_FMT_YUVJ411P => {
                                av::AVPixelFormat::AV_PIX_FMT_YUV411P
                            }
                            other => other,
                        };
                        input.scale = av::sws_getCachedContext(
                            input.scale,
                            frame.width,
                            frame.height,
                            normalized,
                            w as i32,
                            h as i32,
                            av::AVPixelFormat::AV_PIX_FMT_BGRA,
                            av::SwsFlags::SWS_FAST_BILINEAR as i32,
                            ptr::null_mut(),
                            ptr::null_mut(),
                            ptr::null(),
                        );
                        if input.scale.is_null() {
                            return Err(Error::Allocation("camera converter"));
                        }
                        let coefficients = av::sws_getCoefficients(
                            if frame.colorspace == av::AVColorSpace::AVCOL_SPC_BT709 {
                                av::SWS_CS_ITU709
                            } else {
                                av::SWS_CS_ITU601
                            },
                        );
                        let full_range = i32::from(
                            source_format != normalized
                                || frame.color_range == av::AVColorRange::AVCOL_RANGE_JPEG,
                        );
                        check(
                            av::sws_setColorspaceDetails(
                                input.scale,
                                coefficients,
                                full_range,
                                coefficients,
                                1,
                                0,
                                1 << 16,
                                1 << 16,
                            ),
                            "configure camera color range",
                        )?;
                        reusable.resize(w * h * 4, 0);
                        let mut planes = [
                            reusable.as_mut_ptr(),
                            ptr::null_mut(),
                            ptr::null_mut(),
                            ptr::null_mut(),
                        ];
                        let strides = [(w * 4) as i32, 0, 0, 0];
                        check(
                            av::sws_scale(
                                input.scale,
                                frame.data.as_ptr().cast(),
                                frame.linesize.as_ptr(),
                                0,
                                frame.height,
                                planes.as_mut_ptr(),
                                strides.as_ptr(),
                            ),
                            "scale camera frame",
                        )?;
                        if let Ok(mut value) = latest.lock() {
                            let previous = value.replace(Image {
                                width: w,
                                height: h,
                                bgra: std::mem::take(&mut reusable),
                                captured: Instant::now(),
                            });
                            reusable = previous.map(|image| image.bgra).unwrap_or_default();
                        }
                        last = Instant::now();
                    }
                    av::av_frame_unref(input.frame);
                }
            } else {
                av::av_packet_unref(input.packet);
            }
        }
    }
    Ok(())
}

/// Position and width are fractions of the output. Source aspect ratio is kept.
pub struct Placement {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub mirror: bool,
    pub key: bool,
    pub similarity: f32,
    pub softness: f32,
}
impl Placement {
    pub fn valid(&self) -> bool {
        [self.x, self.y, self.width, self.similarity, self.softness]
            .iter()
            .all(|v| v.is_finite())
            && (0.0..=1.0).contains(&self.x)
            && (0.0..=1.0).contains(&self.y)
            && (0.1..=0.5).contains(&self.width)
            && (0.05..=0.6).contains(&self.similarity)
            && (0.01..=0.3).contains(&self.softness)
    }
}
impl Image {
    /// Small premultiplied RGBA thumbnail for Flutter. Key/mirror match the
    /// recorded compositor. Latest-only copying keeps UI queues bounded.
    pub fn preview_rgba(&self, place: &Placement) -> Result<(usize, usize, Vec<u8>)> {
        if !place.valid()
            || self.width == 0
            || self.height == 0
            || self.width > 640
            || self.height > 480
            || self.bgra.len() != self.width * self.height * 4
        {
            return Err(Error::Invalid("invalid webcam preview image"));
        }
        let ratio = (320.0 / self.width as f64)
            .min(240.0 / self.height as f64)
            .min(1.0);
        let w = (self.width as f64 * ratio).floor().max(1.0) as usize;
        let h = (self.height as f64 * ratio).floor().max(1.0) as usize;
        let mut output = vec![0; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let sx = x * self.width / w;
                let sx = if place.mirror {
                    self.width - 1 - sx
                } else {
                    sx
                };
                let p = (y * self.height / h * self.width + sx) * 4;
                let [b, g, r] = [
                    self.bgra[p] as f32,
                    self.bgra[p + 1] as f32,
                    self.bgra[p + 2] as f32,
                ];
                let alpha = if place.key {
                    (((r * r + (g - 255.0).powi(2) + b * b).sqrt() / 441.673 - place.similarity)
                        / place.softness)
                        .clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let dst = (y * w + x) * 4;
                for (index, color) in [r, g, b, 255.0].iter().enumerate() {
                    output[dst + index] = (color * alpha).round() as u8;
                }
            }
        }
        Ok((w, h, output))
    }
}
impl VideoFrame {
    pub fn webcam(&mut self, image: &Image, place: &Placement) -> Result<()> {
        if !place.valid()
            || image.width == 0
            || image.height == 0
            || image.width > 640
            || image.height > 480
            || image.bgra.len() != image.width * image.height * 4
        {
            return Err(Error::Invalid("invalid webcam composition"));
        }
        unsafe {
            if (*self.raw).format != av::AVPixelFormat::AV_PIX_FMT_YUV420P as i32 {
                return Err(Error::Invalid("webcam requires CPU YUV420P"));
            }
            check(
                av::av_frame_make_writable(self.raw),
                "prepare webcam composite",
            )?;
            let fw = (*self.raw).width as usize;
            let fh = (*self.raw).height as usize;
            let w = ((fw as f32 * place.width) as usize / 2 * 2)
                .max(2)
                .min(fw)
                .min((fh * image.width / image.height / 2 * 2).max(2));
            let h = ((w * image.height / image.width) / 2 * 2).max(2).min(fh);
            let x = (((fw - w) as f32 * place.x) as usize / 2) * 2;
            let y = (((fh - h) as f32 * place.y) as usize / 2) * 2;
            for row in (0..h).step_by(2) {
                for col in (0..w).step_by(2) {
                    let mut alpha = 0.0;
                    let mut chroma = [0.0; 2];
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let sx = (col + dx) * image.width / w;
                            let sx = if place.mirror {
                                image.width - 1 - sx
                            } else {
                                sx
                            };
                            let sy = (row + dy) * image.height / h;
                            let p = (sy * image.width + sx) * 4;
                            let [b, g, r] = [
                                image.bgra[p] as f32,
                                image.bgra[p + 1] as f32,
                                image.bgra[p + 2] as f32,
                            ];
                            let a = if place.key {
                                (((r * r + (g - 255.0).powi(2) + b * b).sqrt() / 441.673
                                    - place.similarity)
                                    / place.softness)
                                    .clamp(0.0, 1.0)
                            } else {
                                1.0
                            };
                            let yy = 16.0 + 0.1826 * r + 0.6142 * g + 0.0620 * b;
                            let uv = [
                                128.0 - 0.1006 * r - 0.3386 * g + 0.4392 * b,
                                128.0 + 0.4392 * r - 0.3989 * g - 0.0403 * b,
                            ];
                            let target = (*self.raw).data[0].add(
                                (y + row + dy) * (*self.raw).linesize[0] as usize + x + col + dx,
                            );
                            *target = (*target as f32 * (1.0 - a) + yy * a)
                                .round()
                                .clamp(16.0, 235.0) as u8;
                            alpha += a;
                            for i in 0..2 {
                                chroma[i] += uv[i] * a;
                            }
                        }
                    }
                    for (i, component) in chroma.iter().enumerate() {
                        let target = (*self.raw).data[i + 1].add(
                            ((y + row) / 2) * (*self.raw).linesize[i + 1] as usize + (x + col) / 2,
                        );
                        *target = (*target as f32 * (1.0 - alpha / 4.0) + component / 4.0)
                            .round()
                            .clamp(16.0, 240.0) as u8;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_is_bounded_mirrored_and_premultiplied_with_key() {
        let image = fixture();
        let (w,h,pixels) = image.preview_rgba(&placement(true,false)).unwrap();
        assert_eq!((w,h), (4,2));
        assert_eq!(&pixels[..4], &[0,0,0,0]);
        assert_eq!(&pixels[8..12], &[255,0,0,255]);
        let (_,_,mirrored) = image.preview_rgba(&placement(true,true)).unwrap();
        assert_eq!(&mirrored[..4], &[255,0,0,255]);
        let large = Image { width:640,height:480,bgra:vec![255;640*480*4],captured:Instant::now() };
        let (w,h,rgba) = large.preview_rgba(&placement(false,false)).unwrap();
        assert_eq!((w,h,rgba.len()),(320,240,320*240*4));
    }
    #[test]
    fn preview_and_recording_share_one_reader_until_final_lease() {
        // Inject an owned image, not a physical device. This exercises real
        // lease selection/shutdown without camera permissions or a driver.
        let stop = Arc::new(AtomicBool::new(false));
        let life = Arc::new(CameraLife { device:"fixture-camera".into(),max_width:320,latest:Arc::new(Mutex::new(Some(fixture()))),error:Arc::new(Mutex::new(None)),stop:stop.clone(),thread:Mutex::new(None) });
        *SHARED_CAMERA.get_or_init(|| Mutex::new(Weak::new())).lock().unwrap() = Arc::downgrade(&life);
        let preview = Camera::start("fixture-camera".into(),320).unwrap();
        let recording = Camera::start("fixture-camera".into(),320).unwrap();
        assert!(Arc::ptr_eq(&preview.latest,&recording.latest));
        assert!(Camera::start("different-camera".into(),320).is_err());
        drop(life); drop(preview);
        assert!(!stop.load(Ordering::Acquire));
        drop(recording);
        assert!(stop.load(Ordering::Acquire));
    }
    fn placement(key: bool, mirror: bool) -> Placement {
        Placement {
            x: 1.0,
            y: 1.0,
            width: 0.5,
            mirror,
            key,
            similarity: 0.15,
            softness: 0.08,
        }
    }
    fn fixture() -> Image {
        let mut bgra = Vec::new();
        for _ in 0..2 {
            for x in 0..4 {
                bgra.extend_from_slice(if x < 2 {
                    &[0, 255, 0, 255]
                } else {
                    &[0, 0, 255, 255]
                });
            }
        }
        Image {
            width: 4,
            height: 2,
            bgra,
            captured: Instant::now(),
        }
    }
    fn luma(frame: &VideoFrame, x: usize, y: usize) -> u8 {
        unsafe { *(*frame.raw).data[0].add(y * (*frame.raw).linesize[0] as usize + x) }
    }
    #[test]
    fn key_preserves_background_and_mirror_and_frame_ownership() {
        let image = fixture();
        let base = VideoFrame::new(64, 64).unwrap();
        let mut keyed = base.try_clone().unwrap();
        keyed.webcam(&image, &placement(true, false)).unwrap();
        assert_eq!(luma(&keyed, 32, 48), 16);
        assert!(luma(&keyed, 56, 48) > 40);
        assert_eq!(luma(&base, 56, 48), 16);
        let mut mirrored = base.try_clone().unwrap();
        mirrored.webcam(&image, &placement(false, true)).unwrap();
        assert!(luma(&mirrored, 32, 48) < luma(&mirrored, 56, 48));
        assert_eq!(luma(&mirrored, 31, 48), 16);
        assert_eq!(luma(&mirrored, 63, 47), 16);
    }
    #[test]
    fn invalid_geometry_and_short_buffers_are_rejected() {
        let mut frame = VideoFrame::new(64, 64).unwrap();
        let mut place = placement(false, false);
        place.width = f32::NAN;
        assert!(frame.webcam(&fixture(), &place).is_err());
        let mut image = fixture();
        image.bgra.pop();
        assert!(frame.webcam(&image, &placement(false, false)).is_err());
    }
}
