//! Non-blocking native WGC source feeding an FFmpeg Direct3D11 filter graph.
use crate::{error::check, Error, Result, VideoFrame};
use ffmpeg_sys_next as av;
use std::{ffi::CString, marker::PhantomData, ptr, rc::Rc};

pub struct Capture {
    graph: *mut av::AVFilterGraph,
    sink: *mut av::AVFilterContext,
    gfx: *mut av::AVFilterContext,
    #[cfg(windows)]
    native: *mut std::ffi::c_void,
    #[cfg(windows)]
    source: *mut av::AVFilterContext,
    gpu_enabled: bool,
    #[cfg(windows)]
    gpu_size: [u32; 2],
    #[cfg(windows)]
    gpu_rasters: Vec<std::sync::Arc<crate::layers::Raster>>,
    #[cfg(windows)]
    camera_stamp: Option<std::time::Instant>,
    #[cfg(windows)]
    camera_identity: u64,
    _thread: PhantomData<Rc<()>>,
}

impl Capture {
    /// The controller constructs descriptions from validated numeric values.
    pub fn open(description: &str) -> Result<Self> {
        #[cfg(windows)]
        let (options, filters) = native::parse(description)?;
        #[cfg(not(windows))]
        let filters = description;
        let text = CString::new(filters).map_err(|_| Error::Invalid("invalid capture graph"))?;
        let mut result = Self {
            graph: unsafe { av::avfilter_graph_alloc() },
            sink: ptr::null_mut(),
            gfx: ptr::null_mut(),
            #[cfg(windows)]
            native: ptr::null_mut(),
            #[cfg(windows)]
            source: ptr::null_mut(),
            gpu_enabled: false,
            #[cfg(windows)]
            gpu_size: [options.width as u32, options.height as u32],
            #[cfg(windows)]
            gpu_rasters: Vec::new(),
            #[cfg(windows)]
            camera_stamp: None,
            #[cfg(windows)]
            camera_identity: 0,
            _thread: PhantomData,
        };
        if result.graph.is_null() {
            return Err(Error::Allocation("filter graph allocation failed"));
        }
        unsafe {
            (*result.graph).nb_threads = 2;
            #[cfg(windows)]
            {
                let mut frames = ptr::null_mut();
                let mut error = [0 as libc::c_char; 512];
                result.native = native::frame_capture_open(
                    &options,
                    &mut frames,
                    error.as_mut_ptr(),
                    error.len(),
                );
                if result.native.is_null() {
                    return Err(native::error(&error));
                }
                let buffer = av::avfilter_get_by_name(c"buffer".as_ptr());
                result.source =
                    av::avfilter_graph_alloc_filter(result.graph, buffer, c"source".as_ptr());
                let parameters = av::av_buffersrc_parameters_alloc();
                if parameters.is_null() || result.source.is_null() {
                    av::av_buffer_unref(&mut frames);
                    av::av_free(parameters.cast());
                    return Err(Error::Allocation("GPU source allocation failed"));
                }
                (*parameters).format = av::AVPixelFormat::AV_PIX_FMT_D3D11 as i32;
                (*parameters).width = options.width;
                (*parameters).height = options.height;
                (*parameters).color_space = av::AVColorSpace::AVCOL_SPC_RGB;
                (*parameters).color_range = av::AVColorRange::AVCOL_RANGE_JPEG;
                (*parameters).time_base = av::AVRational {
                    num: 1,
                    den: 10_000_000,
                };
                (*parameters).frame_rate = av::AVRational {
                    num: options.fps as i32,
                    den: 1,
                };
                (*parameters).hw_frames_ctx = frames;
                let code = av::av_buffersrc_parameters_set(result.source, parameters);
                av::av_buffer_unref(&mut frames);
                av::av_free(parameters.cast());
                check(code, "configure GPU frame source")?;
                check(
                    av::avfilter_init_str(result.source, ptr::null()),
                    "initialize GPU frame source",
                )?;
            }
            let sink_filter = av::avfilter_get_by_name(c"buffersink".as_ptr());
            if sink_filter.is_null() {
                return Err(Error::State("FFmpeg buffersink is unavailable"));
            }
            check(
                av::avfilter_graph_create_filter(
                    &mut result.sink,
                    sink_filter,
                    c"sink".as_ptr(),
                    ptr::null(),
                    ptr::null_mut(),
                    result.graph,
                ),
                "create capture sink",
            )?;
            let mut inputs = av::avfilter_inout_alloc();
            if inputs.is_null() {
                return Err(Error::Allocation("filter endpoint allocation failed"));
            }
            (*inputs).name = av::av_strdup(c"out".as_ptr());
            if (*inputs).name.is_null() {
                av::avfilter_inout_free(&mut inputs);
                return Err(Error::Allocation("filter endpoint name allocation failed"));
            }
            (*inputs).filter_ctx = result.sink;
            (*inputs).pad_idx = 0;
            #[cfg(not(windows))]
            let mut outputs = ptr::null_mut();
            #[cfg(windows)]
            let mut outputs = av::avfilter_inout_alloc();
            #[cfg(windows)]
            {
                if outputs.is_null() {
                    av::avfilter_inout_free(&mut inputs);
                    return Err(Error::Allocation("GPU source endpoint allocation failed"));
                }
                (*outputs).name = av::av_strdup(c"in".as_ptr());
                if (*outputs).name.is_null() {
                    av::avfilter_inout_free(&mut inputs);
                    av::avfilter_inout_free(&mut outputs);
                    return Err(Error::Allocation(
                        "GPU source endpoint name allocation failed",
                    ));
                }
                (*outputs).filter_ctx = result.source;
                (*outputs).pad_idx = 0;
            }
            let parsed = av::avfilter_graph_parse_ptr(
                result.graph,
                text.as_ptr(),
                &mut inputs,
                &mut outputs,
                ptr::null_mut(),
            );
            let disconnected = !inputs.is_null() || !outputs.is_null();
            av::avfilter_inout_free(&mut inputs);
            av::avfilter_inout_free(&mut outputs);
            check(parsed, "initialize native capture graph")?;
            if disconnected {
                return Err(Error::Invalid("capture graph has disconnected endpoints"));
            }
            check(
                av::avfilter_graph_config(result.graph, ptr::null_mut()),
                "configure native capture",
            )?;
            for index in 0..(*result.graph).nb_filters {
                let context = *(*result.graph).filters.add(index as usize);
                if std::ffi::CStr::from_ptr((*(*context).filter).name).to_bytes() == b"gfxcapture" {
                    result.gfx = context;
                }
            }
        }
        Ok(result)
    }

    /// Capability is checked before an encoder/output is opened. A caller may
    /// fall back to the CPU compositor only with a YUV420P software pipeline.
    pub fn enable_gpu_overlays(&mut self) -> Result<()> {
        #[cfg(windows)]
        {
            let mut error = [0 as libc::c_char; 512];
            if unsafe {
                native::frame_capture_gpu_enable(self.native, error.as_mut_ptr(), error.len())
            } < 0
            {
                return Err(native::error(&error));
            }
            self.gpu_enabled = true;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err(Error::State("D3D11 compositing requires Windows"))
        }
    }
    pub fn gpu_overlays_enabled(&self) -> bool {
        self.gpu_enabled
    }

    /// Synchronous upload on the capture owner thread. Native textures retain
    /// pixels after return; Rust retains raster identities to prevent ABA reuse.
    #[cfg(windows)]
    pub fn update_gpu_overlays(
        &mut self,
        layers: &[crate::layers::Layer],
        marks: &[crate::overlay::Annotation],
        cursor: &[crate::overlay::Annotation],
        camera: Option<(&crate::webcam::Image, &crate::webcam::Placement)>,
    ) -> Result<()> {
        self.update_gpu_scene(layers, marks, cursor, camera, 0.0)
    }

    /// Fade the composed scene on the same GPU target (no second capture or
    /// full-frame readback). A value of one covers the canvas with black.
    #[cfg(windows)]
    pub fn update_gpu_scene(
        &mut self,
        layers: &[crate::layers::Layer],
        marks: &[crate::overlay::Annotation],
        cursor: &[crate::overlay::Annotation],
        camera: Option<(&crate::webcam::Image, &crate::webcam::Placement)>,
        fade: f32,
    ) -> Result<()> {
        if !self.gpu_enabled {
            return Err(Error::State("GPU compositor disabled"));
        }
        if let Some((image, _)) = camera {
            if self.camera_stamp != Some(image.captured) {
                self.camera_identity = self.camera_identity.wrapping_add(1).max(1);
                self.camera_stamp = Some(image.captured);
            }
        }
        let mut packet = crate::gpu_overlay::Packet::new(
            self.gpu_size,
            layers,
            marks,
            cursor,
            camera,
            self.camera_identity,
        )?;
        packet.fade(fade)?;
        let mut error = [0 as libc::c_char; 512];
        if unsafe {
            native::frame_capture_gpu_update(
                self.native,
                packet.textures.as_ptr(),
                packet.textures.len(),
                packet.vertices.as_ptr(),
                packet.vertices.len(),
                packet.draws.as_ptr(),
                packet.draws.len(),
                error.as_mut_ptr(),
                error.len(),
            )
        } < 0
        {
            return Err(native::error(&error));
        }
        self.gpu_rasters = packet.retained;
        Ok(())
    }

    /// Update the native GPU shader crop on the owning media thread. Output
    /// dimensions and pools remain fixed; panning never reopens capture.
    pub fn pan_region(&mut self, region: [u32; 4], bounds: [u32; 2]) -> Result<()> {
        let [x, y, w, h] = region;
        if w < 2
            || h < 2
            || w > bounds[0]
            || h > bounds[1]
            || x > bounds[0] - w
            || y > bounds[1] - h
        {
            return Err(Error::Invalid("invalid native panning region"));
        }
        #[cfg(windows)]
        {
            unsafe {
                native::frame_capture_pan(self.native, x, y, bounds[0] - x - w, bounds[1] - y - h);
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            if self.gfx.is_null() {
                return Err(Error::State("capture cannot pan"));
            }
            unsafe {
                for (key, value) in [
                    (c"crop_left", x),
                    (c"crop_top", y),
                    (c"crop_right", bounds[0] - x - w),
                    (c"crop_bottom", bounds[1] - y - h),
                ] {
                    check(
                        av::av_opt_set_int((*self.gfx).priv_, key.as_ptr(), i64::from(value), 0),
                        "update GPU capture region",
                    )?;
                }
            }
            Ok(())
        }
    }

    /// EAGAIN means a temporary lack of frames.
    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>> {
        let frame = VideoFrame::empty()?;
        #[cfg(windows)]
        {
            let input = VideoFrame::empty()?;
            let mut error = [0 as libc::c_char; 512];
            let code = unsafe {
                native::frame_capture_next(self.native, input.raw, error.as_mut_ptr(), error.len())
            };
            if code == 0 {
                return Ok(None);
            }
            if code < 0 {
                return Err(native::error(&error));
            }
            check(
                unsafe {
                    av::av_buffersrc_add_frame_flags(
                        self.source,
                        input.raw,
                        av::AV_BUFFERSRC_FLAG_KEEP_REF as i32,
                    )
                },
                "submit GPU capture frame",
            )?;
        }
        let code = unsafe { av::av_buffersink_get_frame(self.sink, frame.raw) };
        if code == -libc::EAGAIN {
            return Ok(None);
        }
        check(code, "capture next frame")?;
        Ok(Some(frame))
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        unsafe {
            av::avfilter_graph_free(&mut self.graph);
            #[cfg(windows)]
            if !self.native.is_null() {
                native::frame_capture_close(self.native);
            }
        }
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    #[repr(C)]
    #[derive(Default)]
    pub struct Options {
        pub handle: u64,
        pub window: i32,
        pub width: i32,
        pub height: i32,
        pub fps: u32,
        pub cursor: i32,
        pub border: i32,
        pub aspect: i32,
        pub left: u32,
        pub top: u32,
        pub right: u32,
        pub bottom: u32,
    }
    extern "C" {
        pub fn frame_capture_open(
            options: *const Options,
            frames: *mut *mut av::AVBufferRef,
            error: *mut libc::c_char,
            length: usize,
        ) -> *mut std::ffi::c_void;
        pub fn frame_capture_next(
            pointer: *mut std::ffi::c_void,
            output: *mut av::AVFrame,
            error: *mut libc::c_char,
            length: usize,
        ) -> i32;
        pub fn frame_capture_pan(
            pointer: *mut std::ffi::c_void,
            left: u32,
            top: u32,
            right: u32,
            bottom: u32,
        );
        pub fn frame_capture_close(pointer: *mut std::ffi::c_void);
        pub fn frame_capture_gpu_enable(
            pointer: *mut std::ffi::c_void,
            error: *mut libc::c_char,
            length: usize,
        ) -> i32;
        pub fn frame_capture_gpu_update(
            pointer: *mut std::ffi::c_void,
            images: *const crate::gpu_overlay::Texture,
            image_count: usize,
            vertices: *const crate::gpu_overlay::Vertex,
            vertex_count: usize,
            commands: *const crate::gpu_overlay::Draw,
            command_count: usize,
            error: *mut libc::c_char,
            length: usize,
        ) -> i32;
    }
    pub fn error(buffer: &[libc::c_char]) -> Error {
        Error::Native(
            unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
                .to_string_lossy()
                .into_owned(),
        )
    }
    /// Descriptions are generated internally from validated source selections.
    /// The historical gfxcapture spelling remains compatible with callers;
    /// FFmpeg now receives a real buffer source and cannot wait for WGC events.
    pub fn parse(description: &str) -> Result<(Options, &str)> {
        let (source, filters) = description
            .split_once(',')
            .ok_or(Error::Invalid("missing capture filters"))?;
        let parameters = source
            .strip_prefix("gfxcapture=")
            .ok_or(Error::Invalid("unsupported native capture source"))?;
        let mut result = Options {
            border: 1,
            cursor: 1,
            ..Options::default()
        };
        for field in parameters.split(':') {
            let (key, value) = field
                .split_once('=')
                .ok_or(Error::Invalid("invalid capture option"))?;
            if key == "resize_mode" {
                result.aspect = match value {
                    "scale_aspect" => 1,
                    "scale" => 0,
                    _ => return Err(Error::Invalid("invalid capture scaling")),
                };
                continue;
            }
            let number = value
                .parse::<u64>()
                .map_err(|_| Error::Invalid("invalid numeric capture option"))?;
            if key == "hwnd" || key == "hmonitor" {
                result.handle = number;
                result.window = i32::from(key == "hwnd");
                continue;
            }
            let value =
                u32::try_from(number).map_err(|_| Error::Invalid("capture option overflow"))?;
            match key {
                "width" => {
                    result.width = i32::try_from(value)
                        .map_err(|_| Error::Invalid("capture width overflow"))?
                }
                "height" => {
                    result.height = i32::try_from(value)
                        .map_err(|_| Error::Invalid("capture height overflow"))?
                }
                "max_framerate" => result.fps = value,
                "capture_cursor" if value <= 1 => result.cursor = value as i32,
                "capture_border" if value <= 1 => result.border = value as i32,
                "crop_left" => result.left = value,
                "crop_top" => result.top = value,
                "crop_right" => result.right = value,
                "crop_bottom" => result.bottom = value,
                _ => return Err(Error::Invalid("unknown native capture option")),
            }
        }
        if result.handle == 0
            || result.width < 2
            || result.height < 2
            || result.fps == 0
            || result.fps > 240
        {
            return Err(Error::Invalid("invalid native capture configuration"));
        }
        Ok((result, filters))
    }
}
