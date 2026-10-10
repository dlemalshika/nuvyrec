use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct Source {
    pub id: u64,
    pub kind: &'static str,
    pub name: String,
    pub width: u32,
    pub height: u32,
}

#[cfg(windows)]
pub fn enumerate() -> Vec<Source> {
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
        });
        1
    }
    unsafe extern "system" fn window(handle: HWND, data: LPARAM) -> i32 {
        unsafe {
            if IsWindowVisible(handle) == 0 || IsIconic(handle) != 0 {
                return 1;
            }
            let mut title = [0u16; 512];
            let len = GetWindowTextW(handle, title.as_mut_ptr(), title.len() as i32);
            let mut rect = RECT::default();
            if len <= 0
                || GetClientRect(handle, &mut rect) == 0
                || rect.right < 2
                || rect.bottom < 2
            {
                return 1;
            }
            let list = &mut *(data as *mut Vec<Source>);
            list.push(Source {
                id: handle as usize as u64,
                kind: "window",
                name: String::from_utf16_lossy(&title[..len as usize]),
                width: rect.right as u32,
                height: rect.bottom as u32,
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
