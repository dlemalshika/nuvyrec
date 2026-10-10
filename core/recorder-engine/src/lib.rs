//! FFmpeg video-session foundation. No process spawning and no UI dependency.
//!
//! The CPU reference path verifies encoder/muxer lifecycle before native GPU
//! capture is integrated. It is not the future zero-CPU-readback capture path.
#![deny(unsafe_op_in_unsafe_fn)]

mod audio;
pub mod audio_processor;
pub mod capture;
mod error;
mod frame;
mod output;
pub mod overlay;
mod replay;
mod session;
#[cfg(windows)]
pub mod wasapi;

pub use error::{Error, Result};
pub use frame::VideoFrame;
pub use recorder_api::{Container, RecordingConfig};
pub use session::RecordingSession;
