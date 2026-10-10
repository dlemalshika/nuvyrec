//! Conservative initial budgets and measured overload decisions. Hardware
//! discovery is a hint, never a claim that a GPU supports a particular mode.
use recorder_engine::RecordingConfig;
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct Machine {
    pub logical_cores: usize,
    pub total_memory_mib: u64,
    pub available_memory_mib: u64,
    pub recommended_height: u32,
    pub recommended_fps: u32,
    pub recommended_bitrate: u32,
}
impl Machine {
    pub fn detect() -> Self {
        let (total, available) = memory();
        Self::from_resources(
            std::thread::available_parallelism().map_or(2, usize::from),
            total,
            available,
        )
    }
    fn from_resources(cores: usize, total: u64, available: u64) -> Self {
        let low = cores <= 2 || (total > 0 && total <= 4096) || (available > 0 && available < 1024);
        Self {
            logical_cores: cores,
            total_memory_mib: total,
            available_memory_mib: available,
            recommended_height: if low { 720 } else { 1080 },
            recommended_fps: if low { 24 } else { 30 },
            recommended_bitrate: if low { 3_000_000 } else { 8_000_000 },
        }
    }
    pub fn constrain(&self, config: &mut RecordingConfig, mode: &str, software: bool) {
        if mode == "manual" {
            return;
        }
        let eco = mode == "eco";
        let height = if eco {
            720
        } else if software {
            self.recommended_height.min(1080)
        } else {
            self.recommended_height
        };
        resize(config, height);
        config.fps = config.fps.min(if eco { 24 } else { self.recommended_fps });
        config.bitrate = config.bitrate.min(if eco {
            3_000_000
        } else {
            self.recommended_bitrate
        });
        config.encoder_threads = if eco || self.logical_cores <= 2 { 1 } else { 2 };
    }
}
fn resize(config: &mut RecordingConfig, height: u32) {
    if config.height <= height {
        return;
    }
    let ratio = f64::from(height) / f64::from(config.height);
    config.width = ((f64::from(config.width) * ratio / 2.0).floor() as u32 * 2).max(2);
    config.height = height;
}

#[derive(Default, Clone, Serialize)]
pub struct Load {
    pub system_cpu: Option<f64>,
    pub process_cpu: Option<f64>,
    pub available_memory_mib: u64,
    pub process_memory_mib: u64,
}
pub struct Monitor {
    previous: Option<[u64; 5]>,
    previous_time: std::time::Instant,
    cores: usize,
}
impl Monitor {
    pub fn new(cores: usize) -> Self {
        Self {
            previous: counters(),
            previous_time: std::time::Instant::now(),
            cores,
        }
    }
    pub fn sample(&mut self) -> Load {
        let current = counters();
        let wall = self.previous_time.elapsed().as_secs_f64();
        let mut result = Load {
            available_memory_mib: memory().1,
            process_memory_mib: working_set(),
            ..Load::default()
        };
        if let (Some(before), Some(now)) = (self.previous, current) {
            let total = now[1].saturating_sub(before[1]) + now[2].saturating_sub(before[2]);
            if total > 0 {
                result.system_cpu = Some(
                    (1.0 - now[0].saturating_sub(before[0]) as f64 / total as f64).clamp(0.0, 1.0),
                );
            }
            if wall > 0.0 {
                result.process_cpu = Some(
                    ((now[3].saturating_sub(before[3]) + now[4].saturating_sub(before[4])) as f64
                        / 10_000_000.0
                        / wall
                        / self.cores.max(1) as f64)
                        .clamp(0.0, 1.0),
                );
            }
        }
        self.previous = current;
        self.previous_time = std::time::Instant::now();
        result
    }
}

/// Three consecutive five-second windows, then a 15-second cooldown. Static
/// screens with few capture frames are not treated as encoder overload.
pub struct Governor {
    overloaded: u8,
    cooldown: u8,
}
impl Governor {
    pub fn new() -> Self {
        Self {
            overloaded: 0,
            cooldown: 0,
        }
    }
    pub fn observe(
        &mut self,
        config: &RecordingConfig,
        load: &Load,
        encode_ms: f64,
    ) -> Option<RecordingConfig> {
        if self.cooldown > 0 {
            self.cooldown -= 1;
            return None;
        }
        let busy = encode_ms > 800.0 / f64::from(config.fps)
            || (load.system_cpu.is_some_and(|v| v > 0.9)
                && load.process_cpu.is_some_and(|v| v > 0.25))
            || (load.available_memory_mib > 0 && load.available_memory_mib < 384);
        self.overloaded = if busy { self.overloaded + 1 } else { 0 };
        if self.overloaded < 3 {
            return None;
        }
        self.overloaded = 0;
        self.cooldown = 3;
        lower_quality(config)
    }
}

pub fn lower_quality(config: &RecordingConfig) -> Option<RecordingConfig> {
    let mut next = config.clone();
    if next.height > 1080 {
        resize(&mut next, 1080);
    } else if next.fps > 30 {
        next.fps = 30;
    } else if next.height > 720 {
        resize(&mut next, 720);
    } else if next.fps > 24 {
        next.fps = 24;
    } else if next.height > 480 {
        resize(&mut next, 480);
    } else if next.fps > 15 {
        next.fps = 15;
    } else {
        return None;
    }
    let ratio = (u64::from(next.width) * u64::from(next.height) * u64::from(next.fps)) as f64
        / (u64::from(config.width) * u64::from(config.height) * u64::from(config.fps)) as f64;
    next.bitrate =
        ((f64::from(config.bitrate) * ratio).round().max(500_000.0) as u32).min(config.bitrate);
    Some(next)
}

#[cfg(windows)]
fn memory() -> (u64, u64) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    unsafe {
        let mut value: MEMORYSTATUSEX = std::mem::zeroed();
        value.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut value) == 0 {
            (0, 0)
        } else {
            (value.ullTotalPhys / 1048576, value.ullAvailPhys / 1048576)
        }
    }
}
#[cfg(not(windows))]
fn memory() -> (u64, u64) {
    (0, 0)
}
#[cfg(windows)]
fn counters() -> Option<[u64; 5]> {
    use windows_sys::Win32::{
        Foundation::FILETIME,
        System::Threading::{GetCurrentProcess, GetProcessTimes, GetSystemTimes},
    };
    fn ticks(v: FILETIME) -> u64 {
        (u64::from(v.dwHighDateTime) << 32) | u64::from(v.dwLowDateTime)
    }
    unsafe {
        let mut idle = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let mut created = FILETIME::default();
        let mut ended = FILETIME::default();
        let mut pk = FILETIME::default();
        let mut pu = FILETIME::default();
        if GetSystemTimes(&mut idle, &mut kernel, &mut user) == 0
            || GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut ended,
                &mut pk,
                &mut pu,
            ) == 0
        {
            None
        } else {
            Some([
                ticks(idle),
                ticks(kernel),
                ticks(user),
                ticks(pk),
                ticks(pu),
            ])
        }
    }
}
#[cfg(not(windows))]
fn counters() -> Option<[u64; 5]> {
    None
}
#[cfg(windows)]
fn working_set() -> u64 {
    use windows_sys::Win32::System::{
        ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        Threading::GetCurrentProcess,
    };
    unsafe {
        let mut value: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        value.cb = size;
        if K32GetProcessMemoryInfo(GetCurrentProcess(), &mut value, size) == 0 {
            0
        } else {
            value.WorkingSetSize as u64 / 1048576
        }
    }
}
#[cfg(not(windows))]
fn working_set() -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use recorder_engine::Container;
    #[test]
    fn low_memory_budget_preserves_aspect_and_manual_choice() {
        let machine = Machine::from_resources(2, 4096, 800);
        let mut config = RecordingConfig::reference("test.mkv", Container::Matroska);
        config.width = 3840;
        config.height = 2160;
        config.fps = 120;
        let manual = config.clone();
        machine.constrain(&mut config, "automatic", true);
        assert_eq!(
            (
                config.width,
                config.height,
                config.fps,
                config.encoder_threads
            ),
            (1280, 720, 24, 1)
        );
        let mut config = manual;
        machine.constrain(&mut config, "manual", true);
        assert_eq!(config.fps, 120);
    }
    #[test]
    fn overload_requires_sustained_pressure_and_cooldown() {
        let config = RecordingConfig::reference("test.mkv", Container::Matroska);
        let mut governor = Governor::new();
        let load = Load::default();
        assert!(governor.observe(&config, &load, 40.0).is_none());
        assert!(governor.observe(&config, &load, 40.0).is_none());
        let next = governor.observe(&config, &load, 40.0).unwrap();
        assert_eq!(next.height, 720);
        assert!(governor.observe(&next, &load, 40.0).is_none());
        for _ in 0..8 {
            assert!(governor.observe(&next, &load, 1.0).is_none());
        }
    }
    #[test]
    fn quality_ladder_never_raises_resources_and_reaches_a_floor() {
        let mut config = RecordingConfig::reference("test.mkv", Container::Matroska);
        config.width = 3840;
        config.height = 2160;
        config.fps = 120;
        config.bitrate = 100_000;
        let mut steps = 0;
        while let Some(next) = lower_quality(&config) {
            assert!(next.width <= config.width && next.height <= config.height);
            assert!(next.fps <= config.fps && next.bitrate <= config.bitrate);
            assert_eq!(next.width % 2, 0);
            assert_eq!(next.height % 2, 0);
            config = next;
            steps += 1;
            assert!(steps < 10);
        }
        assert_eq!((config.height, config.fps), (480, 15));
    }
    #[test]
    fn unknown_memory_and_transient_pressure_do_not_force_a_reduction() {
        let machine = Machine::from_resources(8, 0, 0);
        assert_eq!(machine.recommended_height, 1080);
        let config = RecordingConfig::reference("test.mkv", Container::Matroska);
        let mut governor = Governor::new();
        let load = Load::default();
        for _ in 0..12 {
            assert!(governor.observe(&config, &load, 40.0).is_none());
            assert!(governor.observe(&config, &load, 1.0).is_none());
        }
    }
}
