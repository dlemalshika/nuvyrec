//! Thread-confined WASAPI loopback/microphone capture. Packets are copied into
//! bounded CPU audio buffers; video frames remain independent of this path.
use crate::{Error, Result};
use windows::Win32::{Media::Audio::*, System::Com::*};

fn native(error: windows::core::Error) -> Error {
    Error::Native(format!("Audio device: {error}"))
}

/// Must outlive every COM audio interface created on this recording worker.
pub struct Apartment {
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl Apartment {
    pub fn initialize() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(native)?;
        }
        Ok(Self {
            _thread: std::marker::PhantomData,
        })
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

pub struct AudioCapture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    pub rate: u32,
    channels: usize,
    bits: usize,
    float: bool,
    pub title: &'static str,
}

impl AudioCapture {
    pub fn open(system: bool) -> Result<Self> {
        Self::open_device(system, "")
    }
    pub fn open_device(system: bool, id: &str) -> Result<Self> {
        if id.len() > 1024 || id.contains('\0') {
            return Err(Error::Invalid("invalid audio device ID"));
        }
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(native)?;
            let device = if id.is_empty() {
                enumerator
                    .GetDefaultAudioEndpoint(if system { eRender } else { eCapture }, eConsole)
                    .map_err(native)?
            } else {
                let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
                enumerator
                    .GetDevice(windows::core::PCWSTR(wide.as_ptr()))
                    .map_err(native)?
            };
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(native)?;
            let raw = client.GetMixFormat().map_err(native)?;
            // SAFETY: WASAPI allocates a complete WAVEFORMATEX including its
            // cbSize extension; it stays live through Initialize and is then freed.
            let format = std::ptr::read_unaligned(raw);
            let float = if format.wFormatTag == 0xfffe && format.cbSize >= 22 {
                (*raw.cast::<WAVEFORMATEXTENSIBLE>()).SubFormat.data1 == 3
            } else {
                format.wFormatTag == 3
            };
            let channels = format.nChannels as usize;
            let bits = format.wBitsPerSample as usize;
            let sample_rate = format.nSamplesPerSec;
            let valid = (1..=2).contains(&channels)
                && (8000..=192000).contains(&sample_rate)
                && ((!float && [16, 24, 32].contains(&bits)) || (float && bits == 32));
            if !valid {
                CoTaskMemFree(Some(raw.cast()));
                return Err(Error::Invalid(
                    "Audio requires a mono/stereo PCM or float default device",
                ));
            }
            let initialized = client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                if system {
                    AUDCLNT_STREAMFLAGS_LOOPBACK
                } else {
                    0
                },
                5_000_000,
                0,
                raw,
                None,
            );
            CoTaskMemFree(Some(raw.cast()));
            initialized.map_err(native)?;
            let capture: IAudioCaptureClient = client.GetService().map_err(native)?;
            client.Start().map_err(native)?;
            Ok(Self {
                client,
                capture,
                rate: format.nSamplesPerSec,
                channels,
                bits,
                float,
                title: if system { "System audio" } else { "Microphone" },
            })
        }
    }

    /// Read at most one second of queued samples, always releasing native
    /// packets. Output is stereo float PCM at the device's native sample rate.
    pub fn poll(&mut self) -> Result<Vec<f32>> {
        let mut stereo = Vec::new();
        for _ in 0..64 {
            unsafe {
                if self.capture.GetNextPacketSize().map_err(native)? == 0 {
                    break;
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0;
                let mut flags = 0;
                self.capture
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, None)
                    .map_err(native)?;
                struct Packet<'a>(&'a IAudioCaptureClient, u32);
                impl Drop for Packet<'_> {
                    fn drop(&mut self) {
                        let _ = unsafe { self.0.ReleaseBuffer(self.1) };
                    }
                }
                let _packet = Packet(&self.capture, frames);
                if frames > self.rate || stereo.len() + frames as usize * 2 > self.rate as usize * 2
                {
                    return Err(Error::State(
                        "Audio capture backlog exceeded its bounded buffer",
                    ));
                }
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
                if !silent && data.is_null() {
                    return Err(Error::State("Audio device returned a null sample buffer"));
                }
                let step = self.bits / 8;
                for frame in 0..frames as usize {
                    for channel in 0..2 {
                        let value = if silent {
                            0.0
                        } else {
                            let offset =
                                (frame * self.channels + channel.min(self.channels - 1)) * step;
                            let bytes = std::slice::from_raw_parts(data.add(offset), step);
                            match (self.float, self.bits) {
                                (true, 32) => f32::from_le_bytes(
                                    bytes
                                        .try_into()
                                        .map_err(|_| Error::State("Invalid float sample"))?,
                                ),
                                (_, 16) => {
                                    i16::from_le_bytes([bytes[0], bytes[1]]) as f32 / 32768.0
                                }
                                (_, 24) => {
                                    i32::from_le_bytes([
                                        bytes[0],
                                        bytes[1],
                                        bytes[2],
                                        if bytes[2] & 128 != 0 { 255 } else { 0 },
                                    ]) as f32
                                        / 8388608.0
                                }
                                (_, 32) => {
                                    i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
                                        as f32
                                        / 2147483648.0
                                }
                                _ => 0.0,
                            }
                        };
                        stereo.push(if value.is_finite() {
                            value.clamp(-1.0, 1.0)
                        } else {
                            0.0
                        });
                    }
                }
            }
        }
        Ok(stereo)
    }
}
impl Drop for AudioCapture {
    fn drop(&mut self) {
        let _ = unsafe { self.client.Stop() };
    }
}
