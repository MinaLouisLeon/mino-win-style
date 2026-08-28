//! Everything the dock needs to know about the desktop it sits on.
//!
//! This is window *automation*, not injection: we enumerate top-level windows,
//! read their icons out of the executables they came from, and ask Windows to
//! bring one forward. Nothing is loaded into another process.

use std::ffi::c_void;
use std::path::Path;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, MAX_PATH, POINT, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetMonitorInfoW, GetObjectW, MonitorFromPoint, ReleaseDC,
    BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ, MONITORINFO,
    MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, EnumWindows, GetIconInfo, GetWindow, GetWindowLongPtrW, GetWindowTextLengthW,
    GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, PrivateExtractIconsW,
    SetForegroundWindow, ShowWindow, GWL_EXSTYLE, GW_OWNER, HICON, ICONINFO, SW_RESTORE,
    SW_SHOWNORMAL, WS_EX_TOOLWINDOW,
};

use crate::{AppWindow, Icon, WorkArea};

/// Windows that belong on a dock: visible, top-level, not a tool window, not one
/// of the cloaked ghosts Windows keeps around for suspended Store apps.
///
/// The cloaked check is the one people forget. Without it a dock fills up with
/// invisible `ApplicationFrameHost` windows and a couple of permanent phantoms.
pub fn windows() -> Vec<AppWindow> {
    let mut found: Vec<AppWindow> = Vec::new();
    let ptr = &mut found as *mut Vec<AppWindow> as isize;
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(ptr));
    }
    found
}

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let out = &mut *(lparam.0 as *mut Vec<AppWindow>);

    if !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }

    // Owned windows are dialogs and palettes belonging to something else.
    if GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.0.is_null()) {
        return TRUE;
    }

    let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
    if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
        return TRUE;
    }

    if is_cloaked(hwnd) {
        return TRUE;
    }

    let title = window_text(hwnd);
    if title.trim().is_empty() {
        return TRUE;
    }

    let Some(exe) = process_path(hwnd) else {
        return TRUE;
    };

    // Our own dock and settings window have no business being on the dock.
    if exe.to_lowercase().ends_with("mino-win-style.exe") {
        return TRUE;
    }

    out.push(AppWindow {
        hwnd: hwnd.0 as isize,
        title,
        exe,
        minimized: IsIconic(hwnd).as_bool(),
    });
    TRUE
}

fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked: u32 = 0;
    let ok = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        )
    };
    ok.is_ok() && cloaked != 0
}

fn window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; len as usize + 1];
        let written = GetWindowTextW(hwnd, &mut buffer);
        String::from_utf16_lossy(&buffer[..written as usize])
    }
}

fn process_path(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        // LIMITED_INFORMATION so this works without elevation.
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = vec![0u16; MAX_PATH as usize];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        result.ok()?;
        Some(String::from_utf16_lossy(&buffer[..size as usize]))
    }
}

/// The icon an executable carries, as straight-alpha RGBA.
///
/// `PrivateExtractIconsW` is used rather than `SHGetFileInfo` because it takes
/// the size we want: a dock at 64px wants a 64px icon, not a 32px one stretched.
pub fn icon_rgba(exe: &str, size: u32) -> Option<Icon> {
    if !Path::new(exe).is_file() {
        return None;
    }
    // This one takes the path as a fixed MAX_PATH buffer, not a pointer.
    let mut name = [0u16; MAX_PATH as usize];
    for (slot, ch) in name
        .iter_mut()
        .zip(exe.encode_utf16())
        .take(MAX_PATH as usize - 1)
    {
        *slot = ch;
    }

    unsafe {
        let mut icons = [HICON::default(); 1];
        let count = PrivateExtractIconsW(
            &name,
            0,
            size as i32,
            size as i32,
            Some(&mut icons),
            None,
            0,
        );
        if count == 0 || icons[0].is_invalid() {
            return None;
        }
        let icon = hicon_to_rgba(icons[0]);
        let _ = DestroyIcon(icons[0]);
        icon
    }
}

unsafe fn hicon_to_rgba(hicon: HICON) -> Option<Icon> {
    let mut info = ICONINFO::default();
    GetIconInfo(hicon, &mut info).ok()?;

    let mut bitmap = BITMAP::default();
    let got = GetObjectW(
        HGDIOBJ(info.hbmColor.0),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bitmap as *mut BITMAP as *mut c_void),
    );
    if got == 0 {
        cleanup(&info);
        return None;
    }

    let width = bitmap.bmWidth.max(0) as u32;
    let height = bitmap.bmHeight.max(0) as u32;
    if width == 0 || height == 0 {
        cleanup(&info);
        return None;
    }

    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            // Negative: top-down, so the rows come out in the order everything
            // else in this program expects.
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let hdc = GetDC(None);
    let lines = GetDIBits(
        hdc,
        info.hbmColor,
        0,
        height,
        Some(pixels.as_mut_ptr() as *mut c_void),
        &mut header,
        DIB_RGB_COLORS,
    );
    ReleaseDC(None, hdc);
    cleanup(&info);

    if lines == 0 {
        return None;
    }

    // GetDIBits hands back BGRA.
    let mut any_alpha = false;
    for px in pixels.chunks_exact_mut(4) {
        px.swap(0, 2);
        if px[3] != 0 {
            any_alpha = true;
        }
    }
    // Older icons carry no alpha channel at all; without this they come out
    // fully transparent, which reads as "the icon failed to load".
    if !any_alpha {
        for px in pixels.chunks_exact_mut(4) {
            px[3] = 0xFF;
        }
    }

    Some(Icon {
        width,
        height,
        rgba: pixels,
    })
}

unsafe fn cleanup(info: &ICONINFO) {
    if !info.hbmColor.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
    }
    if !info.hbmMask.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
    }
}

/// Bring a window forward, restoring it first if it was minimised.
pub fn activate(hwnd: isize) -> bool {
    let hwnd = HWND(hwnd as *mut c_void);
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
        }
        SetForegroundWindow(hwnd).as_bool()
    }
}

/// Start something. `ShellExecuteW` so this works for `.exe` files and for URIs
/// such as `ms-settings:` alike.
pub fn launch(target: &str) -> bool {
    let wide: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    unsafe {
        let result = ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
        // ShellExecute returns >32 on success. It is an HINSTANCE for
        // historical reasons and means nothing else.
        result.0 as isize > 32
    }
}

/// The primary monitor's work area — the screen minus the taskbar, so the dock
/// sits above a visible taskbar and at the bottom of the screen when it hides.
pub fn work_area() -> WorkArea {
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let RECT {
                left,
                top,
                right,
                bottom,
            } = info.rcWork;
            WorkArea {
                x: left,
                y: top,
                width: (right - left).max(0),
                height: (bottom - top).max(0),
            }
        } else {
            WorkArea {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }
        }
    }
}
