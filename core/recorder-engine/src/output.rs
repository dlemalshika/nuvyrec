//! Seekable custom AVIO: Rust opens the destination with create_new, preventing
//! overwrite races and supporting native paths (including Windows Unicode).
use crate::{error::check, Error, Result};
use ffmpeg_sys_next as av;
use std::{
    fs::{File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::Path,
    ptr,
};

pub(crate) struct Output {
    pub(crate) raw: *mut av::AVFormatContext,
    io: *mut av::AVIOContext,
    file: Option<Box<File>>,
    pub(crate) replay: Option<crate::replay::Replay>,
}

impl Output {
    pub(crate) fn new(muxer: &std::ffi::CStr) -> Result<Self> {
        let mut output = Self {
            raw: ptr::null_mut(),
            io: ptr::null_mut(),
            file: None,
            replay: None,
        };
        // SAFETY: output pointer and muxer name are valid for this call; no
        // filename is needed because the muxer is chosen explicitly.
        unsafe {
            check(
                av::avformat_alloc_output_context2(
                    &mut output.raw,
                    ptr::null_mut(),
                    muxer.as_ptr(),
                    ptr::null(),
                ),
                "allocate output context",
            )?;
        }
        if output.raw.is_null() {
            return Err(Error::Allocation("output context is null"));
        }
        Ok(output)
    }

    /// Attach IO only after codec initialization succeeds. Existing files are
    /// never replaced. A failed recording may leave a partial file for diagnosis.
    pub(crate) fn open_file(&mut self, path: &Path) -> Result<()> {
        self.file = Some(Box::new(
            OpenOptions::new().write(true).create_new(true).open(path)?,
        ));
        let file = self.file.as_mut().ok_or(Error::State("file is missing"))?;
        unsafe {
            let buffer = av::av_malloc(32 * 1024).cast::<u8>();
            if buffer.is_null() {
                return Err(Error::Allocation("AVIO buffer allocation failed"));
            }
            // SAFETY: Box keeps File at a stable address; callbacks run
            // synchronously on the session worker. File outlives AVIOContext.
            self.io = av::avio_alloc_context(
                buffer,
                32 * 1024,
                1,
                (&mut **file as *mut File).cast(),
                None,
                Some(write_packet),
                Some(seek),
            );
            if self.io.is_null() {
                av::av_free(buffer.cast());
                return Err(Error::Allocation("avio_alloc_context failed"));
            }
            (*self.raw).pb = self.io;
            (*self.raw).flags |= av::AVFMT_FLAG_CUSTOM_IO;
        }
        Ok(())
    }

    pub(crate) fn sync(&mut self) -> Result<()> {
        unsafe {
            av::avio_flush(self.io);
            check((*self.io).error, "flush output IO")?;
        }
        self.file
            .as_ref()
            .ok_or(Error::State("file is missing"))?
            .sync_all()?;
        Ok(())
    }
    pub(crate) unsafe fn mux(&mut self, packet: *mut av::AVPacket) -> Result<()> {
        unsafe {
            if let Some(replay) = &mut self.replay {
                replay.record(self.raw, packet)?;
            }
            check(
                av::av_interleaved_write_frame(self.raw, packet),
                "mux encoded packet",
            )
        }
    }
}

// SAFETY contract: FFmpeg provides the File pointer and a buffer valid for len
// bytes. These private callbacks neither retain buffers nor unwind into C.
unsafe extern "C" fn write_packet(opaque: *mut libc::c_void, buf: *const u8, len: i32) -> i32 {
    if opaque.is_null() || buf.is_null() || len < 0 {
        return -libc::EINVAL;
    }
    let file = unsafe { &mut *opaque.cast::<File>() };
    let bytes = unsafe { std::slice::from_raw_parts(buf, len as usize) };
    match file.write_all(bytes) {
        Ok(()) => len,
        Err(_) => -libc::EIO,
    }
}

unsafe extern "C" fn seek(opaque: *mut libc::c_void, offset: i64, whence: i32) -> i64 {
    if opaque.is_null() {
        return i64::from(-libc::EINVAL);
    }
    let file = unsafe { &mut *opaque.cast::<File>() };
    if whence & av::AVSEEK_SIZE != 0 {
        return match file.metadata() {
            Ok(m) => i64::try_from(m.len()).unwrap_or(i64::from(-libc::EOVERFLOW)),
            Err(_) => i64::from(-libc::EIO),
        };
    }
    let from = match whence & !av::AVSEEK_FORCE {
        0 if offset >= 0 => SeekFrom::Start(offset as u64),
        1 => SeekFrom::Current(offset),
        2 => SeekFrom::End(offset),
        _ => return i64::from(-libc::EINVAL),
    };
    match file.seek(from) {
        Ok(p) => i64::try_from(p).unwrap_or(i64::from(-libc::EOVERFLOW)),
        Err(_) => i64::from(-libc::EIO),
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: the format borrows the custom IO context. Detach it before
            // freeing the format; release FFmpeg's current buffer, which may
            // differ from the original buffer passed to avio_alloc_context.
            if !self.raw.is_null() {
                (*self.raw).pb = ptr::null_mut();
                av::avformat_free_context(self.raw);
            }
            if !self.io.is_null() {
                av::av_free((*self.io).buffer.cast());
                (*self.io).buffer = ptr::null_mut();
                av::avio_context_free(&mut self.io);
            }
        }
    }
}
