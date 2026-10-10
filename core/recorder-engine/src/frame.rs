use crate::{error::check, Error, Result};
use ffmpeg_sys_next as av;
use std::{marker::PhantomData, ptr, rc::Rc};

/// Owned, reference-counted FFmpeg YUV420P frame for the CPU reference path.
/// Thread confinement avoids accidental unsynchronized mutation of AVFrame.
pub struct VideoFrame {
    pub(crate) raw: *mut av::AVFrame,
    _thread_confined: PhantomData<Rc<()>>,
}

impl VideoFrame {
    /// Reference-counted clone; compositors call make_writable before mutation.
    pub fn try_clone(&self) -> Result<Self> {
        let raw = unsafe { av::av_frame_clone(self.raw) };
        if raw.is_null() {
            return Err(Error::Allocation("clone video frame"));
        }
        Ok(Self {
            raw,
            _thread_confined: PhantomData,
        })
    }
    pub(crate) fn empty() -> Result<Self> {
        let raw = unsafe { av::av_frame_alloc() };
        if raw.is_null() {
            return Err(Error::Allocation("frame allocation failed"));
        }
        Ok(Self {
            raw,
            _thread_confined: PhantomData,
        })
    }
    /// Allocate a reusable, aligned CPU frame initialized to limited-range black
    /// so safe callers cannot expose uninitialized bytes. Use copy_yuv420 to
    /// replace its pixels before submitting captured content.
    pub fn new(width: u32, height: u32) -> Result<Self> {
        if width < 2
            || height < 2
            || width > 7680
            || height > 4320
            || !width.is_multiple_of(2)
            || !height.is_multiple_of(2)
        {
            return Err(Error::Invalid("invalid YUV420P frame dimensions"));
        }
        // SAFETY: allocation is checked before dereferencing; this wrapper owns
        // the frame and releases it exactly once in Drop, including failures.
        unsafe {
            let mut frame = Self {
                raw: av::av_frame_alloc(),
                _thread_confined: PhantomData,
            };
            if frame.raw.is_null() {
                return Err(Error::Allocation("av_frame_alloc failed"));
            }
            (*frame.raw).width = width as i32;
            (*frame.raw).height = height as i32;
            (*frame.raw).format = av::AVPixelFormat::AV_PIX_FMT_YUV420P as i32;
            (*frame.raw).color_range = av::AVColorRange::AVCOL_RANGE_MPEG;
            (*frame.raw).colorspace = av::AVColorSpace::AVCOL_SPC_BT709;
            (*frame.raw).color_primaries = av::AVColorPrimaries::AVCOL_PRI_BT709;
            (*frame.raw).color_trc = av::AVColorTransferCharacteristic::AVCOL_TRC_BT709;
            check(
                av::av_frame_get_buffer(frame.raw, 32),
                "allocate frame planes",
            )?;
            frame.fill_black();
            Ok(frame)
        }
    }

    fn fill_black(&mut self) {
        // SAFETY: called only after allocating writable planes. Initialize the
        // entire padded row so neither visible pixels nor row padding leak old
        // allocator contents to native code.
        unsafe {
            for plane in 0..3 {
                let rows = if plane == 0 {
                    (*self.raw).height
                } else {
                    (*self.raw).height / 2
                };
                let bytes = rows as usize * (*self.raw).linesize[plane] as usize;
                ptr::write_bytes(
                    (*self.raw).data[plane],
                    if plane == 0 { 16 } else { 128 },
                    bytes,
                );
            }
        }
    }

    /// Copy tightly packed planes into an FFmpeg frame, respecting its padded
    /// row stride. Borrowed source slices never outlive this call.
    pub fn copy_yuv420(&mut self, y: &[u8], u: &[u8], v: &[u8]) -> Result<()> {
        unsafe {
            let w = (*self.raw).width as usize;
            let h = (*self.raw).height as usize;
            if y.len() != w * h || u.len() != w * h / 4 || v.len() != w * h / 4 {
                return Err(Error::Invalid(
                    "YUV plane lengths do not match frame dimensions",
                ));
            }
            // SAFETY: the frame is uniquely accessed; writable ensures the
            // encoder's retained references are not overwritten.
            check(av::av_frame_make_writable(self.raw), "make frame writable")?;
            for (plane, data) in [y, u, v].iter().enumerate() {
                let (pw, ph) = if plane == 0 { (w, h) } else { (w / 2, h / 2) };
                for row in 0..ph {
                    ptr::copy_nonoverlapping(
                        data.as_ptr().add(row * pw),
                        (*self.raw).data[plane].add(row * (*self.raw).linesize[plane] as usize),
                        pw,
                    );
                }
            }
        }
        Ok(())
    }
}

impl Drop for VideoFrame {
    fn drop(&mut self) {
        // SAFETY: FFmpeg accepts null and clears the owned pointer.
        unsafe {
            av::av_frame_free(&mut self.raw);
        }
    }
}
