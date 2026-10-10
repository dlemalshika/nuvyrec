use ffmpeg_sys_next as av;
use std::{ffi::CStr, fmt};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Native(String),
    Invalid(&'static str),
    Allocation(&'static str),
    Ffmpeg {
        operation: &'static str,
        code: i32,
        message: String,
    },
    Io(std::io::Error),
    State(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native(s) => f.write_str(s),
            Self::Invalid(s) | Self::Allocation(s) | Self::State(s) => f.write_str(s),
            Self::Ffmpeg {
                operation,
                code,
                message,
            } => write!(f, "{operation}: {message} ({code})"),
            Self::Io(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub(crate) fn check(code: i32, operation: &'static str) -> Result<()> {
    if code >= 0 {
        return Ok(());
    }
    let mut buffer = [0 as libc::c_char; 128];
    // SAFETY: buffer is writable for the provided size; av_strerror terminates
    // successful output. The zero-initialized buffer also covers failure.
    unsafe {
        av::av_strerror(code, buffer.as_mut_ptr(), buffer.len());
    }
    let message = unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    Err(Error::Ffmpeg {
        operation,
        code,
        message,
    })
}
