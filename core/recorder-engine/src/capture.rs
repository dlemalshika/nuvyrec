//! Library-based Windows.Graphics.Capture and Direct3D11 filter graph.
use crate::{error::check, Error, Result, VideoFrame};
use ffmpeg_sys_next as av;
use std::{ffi::CString, marker::PhantomData, ptr, rc::Rc};

pub struct Capture {
    graph: *mut av::AVFilterGraph,
    sink: *mut av::AVFilterContext,
    _thread: PhantomData<Rc<()>>,
}

impl Capture {
    /// The controller constructs descriptions from validated numeric values.
    pub fn open(description: &str) -> Result<Self> {
        let text =
            CString::new(description).map_err(|_| Error::Invalid("invalid capture graph"))?;
        let mut result = Self {
            graph: unsafe { av::avfilter_graph_alloc() },
            sink: ptr::null_mut(),
            _thread: PhantomData,
        };
        if result.graph.is_null() {
            return Err(Error::Allocation("filter graph allocation failed"));
        }
        unsafe {
            (*result.graph).nb_threads = 2;
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
            let mut outputs = ptr::null_mut();
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
        }
        Ok(result)
    }

    /// EAGAIN means a temporary lack of frames.
    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>> {
        let frame = VideoFrame::empty()?;
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
        }
    }
}
