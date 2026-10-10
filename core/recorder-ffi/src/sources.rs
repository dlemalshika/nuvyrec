use serde::Serialize;

/// Coordinates stay native and are sampled only for an explicitly enabled
/// display-capture cursor effect. No keyboard events are collected.
#[cfg(windows)]
pub fn cursor_position(id: u64, region: Option<[u32; 4]>) -> Option<[f32; 2]> {
    use windows_sys::Win32::{
        Foundation::POINT, Graphics::Gdi::*, UI::WindowsAndMessaging::GetCursorPos,
    };
    unsafe {
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        let mut cursor = POINT::default();
        if GetMonitorInfoW(id as usize as HMONITOR, &mut info) == 0
            || GetCursorPos(&mut cursor) == 0
        {
            return None;
        }
        let [left, top, width, height] = region.unwrap_or([
            0,
            0,
            (info.rcMonitor.right - info.rcMonitor.left) as u32,
            (info.rcMonitor.bottom - info.rcMonitor.top) as u32,
        ]);
        let x = cursor.x - info.rcMonitor.left - left as i32;
        let y = cursor.y - info.rcMonitor.top - top as i32;
        if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
            return None;
        }
        Some([
            x as f32 / (width - 1) as f32,
            y as f32 / (height - 1) as f32,
        ])
    }
}

#[derive(Clone, Serialize)]
pub struct Source {
    pub id: u64,
    pub kind: &'static str,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub minimized: bool,
}

#[cfg(windows)]
pub struct PhysicalCoordinates(windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT);
#[cfg(windows)]
impl PhysicalCoordinates {
    pub fn enter() -> Self {
        Self(unsafe {
            windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
                windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            )
        })
    }
}
#[cfg(windows)]
impl Drop for PhysicalCoordinates {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(self.0);
            }
        }
    }
}

#[cfg(windows)]
pub fn local_pointer(id: u64) -> Option<[f64; 2]> {
    use windows_sys::Win32::{
        Foundation::POINT, Graphics::Gdi::*, UI::WindowsAndMessaging::GetCursorPos,
    };
    let _dpi = PhysicalCoordinates::enter();
    unsafe {
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        let mut point = POINT::default();
        if GetMonitorInfoW(id as usize as HMONITOR, &mut info) == 0 || GetCursorPos(&mut point) == 0
        {
            return None;
        }
        if point.x < info.rcMonitor.left
            || point.y < info.rcMonitor.top
            || point.x >= info.rcMonitor.right
            || point.y >= info.rcMonitor.bottom
        {
            return None;
        }
        Some([
            (point.x - info.rcMonitor.left) as f64,
            (point.y - info.rcMonitor.top) as f64,
        ])
    }
}

#[cfg(windows)]
pub fn enumerate() -> Vec<Source> {
    let _dpi = PhysicalCoordinates::enter();
    use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};
    unsafe extern "system" fn monitor(
        handle: HMONITOR,
        _: HDC,
        rect: *mut RECT,
        data: LPARAM,
    ) -> i32 {
        let list = unsafe { &mut *(data as *mut Vec<Source>) };
        let rect = unsafe { &*rect };
        list.push(Source {
            id: handle as usize as u64,
            kind: "display",
            name: format!(
                "Display {} · {} × {}",
                list.len() + 1,
                rect.right - rect.left,
                rect.bottom - rect.top
            ),
            width: (rect.right - rect.left) as u32,
            height: (rect.bottom - rect.top) as u32,
            minimized: false,
        });
        1
    }
    unsafe extern "system" fn window(handle: HWND, data: LPARAM) -> i32 {
        unsafe {
            if IsWindowVisible(handle) == 0 {
                return 1;
            }
            use windows_sys::Win32::Graphics::Dwm::*;
            let mut cloaked = 0u32;
            if DwmGetWindowAttribute(
                handle,
                DWMWA_CLOAKED as u32,
                (&mut cloaked as *mut u32).cast(),
                4,
            ) >= 0
                && cloaked != 0
            {
                return 1;
            }
            let mut title = [0u16; 512];
            let len = GetWindowTextW(handle, title.as_mut_ptr(), title.len() as i32);
            let mut rect = RECT::default();
            if len <= 0 || GetWindowRect(handle, &mut rect) == 0 {
                return 1;
            }
            let minimized = IsIconic(handle) != 0;
            if minimized {
                let mut placement: WINDOWPLACEMENT = std::mem::zeroed();
                placement.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
                if GetWindowPlacement(handle, &mut placement) != 0 {
                    rect = placement.rcNormalPosition;
                }
            } else {
                let mut visible = RECT::default();
                if DwmGetWindowAttribute(
                    handle,
                    DWMWA_EXTENDED_FRAME_BOUNDS as u32,
                    (&mut visible as *mut RECT).cast(),
                    std::mem::size_of::<RECT>() as u32,
                ) >= 0
                {
                    rect = visible;
                }
            }
            if rect.right - rect.left < 2 || rect.bottom - rect.top < 2 {
                return 1;
            }
            let list = &mut *(data as *mut Vec<Source>);
            list.push(Source {
                id: handle as usize as u64,
                kind: "window",
                name: String::from_utf16_lossy(&title[..len as usize]),
                width: (rect.right - rect.left) as u32,
                height: (rect.bottom - rect.top) as u32,
                minimized,
            });
            1
        }
    }
    let mut list = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(monitor),
            &mut list as *mut _ as isize,
        );
        EnumWindows(Some(window), &mut list as *mut _ as isize);
    }
    list
}

#[cfg(not(windows))]
pub fn enumerate() -> Vec<Source> {
    Vec::new()
}
