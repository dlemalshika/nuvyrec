//! Dependency-free configuration shared by the engine and future control bridge.
#![forbid(unsafe_code)]

use std::path::PathBuf;

/// Video containers and a low-resolution animated GIF output profile.
#[derive(Debug, Clone, Copy)]
pub enum Container {
    Mp4,
    Matroska,
    Avi,
    Gif,
}

impl Container {
    pub fn muxer_name(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Matroska => "matroska",
            Self::Avi => "avi",
            Self::Gif => "gif",
        }
    }
}

/// Explicit CPU reference configuration. Hardware sessions will use a separate
/// negotiated configuration containing device, surface format and pool limits.
#[derive(Debug, Clone)]
pub struct RecordingConfig {
    pub output: PathBuf,
    pub container: Container,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    /// Bound software-encoding parallelism instead of consuming every CPU core.
    pub encoder_threads: u8,
}

impl RecordingConfig {
    /// Conservative reference preset; this is not an automatic device benchmark.
    pub fn reference(output: impl Into<PathBuf>, container: Container) -> Self {
        Self {
            output: output.into(),
            container,
            width: 1280,
            height: 720,
            fps: 30,
            bitrate: 3_000_000,
            encoder_threads: 2,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.output.as_os_str().is_empty() {
            return Err("output path must not be empty");
        }
        if self.width < 2 || self.height < 2 || self.width > 7680 || self.height > 4320 {
            return Err("dimensions must be within 2..=7680 by 2..=4320");
        }
        if !self.width.is_multiple_of(2) || !self.height.is_multiple_of(2) {
            return Err("YUV420P requires even dimensions");
        }
        if !(1..=120).contains(&self.fps) {
            return Err("frame rate must be within 1..=120");
        }
        if !(100_000..=250_000_000).contains(&self.bitrate) {
            return Err("bitrate must be within 100,000..=250,000,000 bits/s");
        }
        if !(1..=16).contains(&self.encoder_threads) {
            return Err("encoder thread count must be within 1..=16");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_4k120_configuration_but_does_not_claim_device_support() {
        let mut config = RecordingConfig::reference("test.mkv", Container::Matroska);
        config.width = 3840;
        config.height = 2160;
        config.fps = 120;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_yuv_dimensions_and_unbounded_frame_rate() {
        let mut config = RecordingConfig::reference("test.mp4", Container::Mp4);
        config.width = 1279;
        assert!(config.validate().is_err());
        config.width = 1280;
        config.fps = 0;
        assert!(config.validate().is_err());
    }
}
