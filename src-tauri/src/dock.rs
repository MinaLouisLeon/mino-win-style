//! The dock: our own window, drawn by us, sitting above the desktop.
//!
//! Windows has no dock and no way to add one, so this is not a setting — it is
//! a second Tauri window with no decorations, transparent, always on top, and
//! kept out of the taskbar and Alt+Tab. The page inside it measures itself and
//! asks to be placed, which keeps the sizing where the layout is.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder,
};

pub const WINDOW_LABEL: &str = "dock";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockConfig {
    pub enabled: bool,
    /// Executable paths, in the order they appear on the dock.
    pub pinned: Vec<String>,
    /// Height of an icon at rest, in logical pixels.
    pub icon_size: u32,
}

impl Default for DockConfig {
    fn default() -> Self {
        DockConfig {
            enabled: false,
            pinned: default_pins(),
            icon_size: 48,
        }
    }
}

/// Two programs every Windows install has, so the dock is never empty on first
/// run. Anything else the user pins is their business.
fn default_pins() -> Vec<String> {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    [
        format!(r"{root}\explorer.exe"),
        format!(r"{root}\notepad.exe"),
    ]
    .into_iter()
    .filter(|p| std::path::Path::new(p).is_file())
    .collect()
}

fn config_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("mino-win-style").join("dock.json")
}

impl DockConfig {
    /// A missing or unreadable file is not an error: the dock has a default, and
    /// refusing to start because a preferences file is malformed would be worse
    /// than ignoring it.
    pub fn load() -> Self {
        fs::read_to_string(config_path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, text).map_err(|e| e.to_string())
    }
}

/// Creates the dock's window, once, at startup.
///
/// It is built whether or not the dock is switched on, and simply left hidden
/// when it is off. That is not laziness avoided for its own sake: building a
/// webview window anywhere except application setup blocks — the window appears
/// but every line after `build()` is never reached, so it is never placed,
/// never shown properly, and its page never loads. Toggling therefore moves a
/// window that already exists rather than making a new one.
pub fn create(app: &AppHandle) -> Result<(), String> {
    trace("create() entered");
    if app.get_webview_window(WINDOW_LABEL).is_some() {
        trace("create(): already there");
        return Ok(());
    }

    WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::App("dock.html".into()))
        .title("Mino Dock")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        // Keeps it out of the taskbar and, on Windows, out of Alt+Tab.
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        // Never steal focus: clicking the dock should raise the app it points
        // at, not the dock.
        .focused(false)
        .inner_size(600.0, 140.0)
        .build()
        .map_err(|e| format!("could not create the dock window: {e}"))?;

    trace("dock window created");
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = place_window(&window, 600.0, 140.0);
    }

    Ok(())
}

/// Sizes the dock and centres it along the bottom of the primary work area.
///
/// Deliberately does *not* show the window: the page calls this whenever its
/// layout changes, and a hidden dock re-placing itself must stay hidden.
fn place_window(window: &tauri::WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    let area = mino_shell::work_area();
    let scale = window.scale_factor().unwrap_or(1.0);

    let logical_work_width = f64::from(area.width) / scale;
    let x = f64::from(area.x) / scale + (logical_work_width - width) / 2.0;
    let y = f64::from(area.y + area.height) / scale - height;

    window
        .set_size(LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    window
        .set_position(LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Puts the dock on screen. The window already exists; this only reveals it.
pub fn show(app: &AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        trace("show(): no dock window to show");
        return Err("the dock window is missing; restart the app".into());
    };
    trace("show(): revealing the dock");
    window.show().map_err(|e| e.to_string())?;
    // Re-assert after showing: another window going full-screen can take the
    // top spot, and a dock that hides behind things is not a dock.
    let _ = window.set_always_on_top(true);
    let _ = app.emit_to(WINDOW_LABEL, "dock-active", true);
    Ok(())
}

/// Takes the dock off screen, leaving the window in place for next time.
pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        trace("hide(): hiding the dock");
        let _ = window.hide();
        // Tells the page to stop looking at the desktop while nobody can see it.
        let _ = app.emit_to(WINDOW_LABEL, "dock-active", false);
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct DockLayout {
    pub work_x: i32,
    pub work_y: i32,
    pub work_width: i32,
    pub work_height: i32,
    pub icon_size: u32,
}

#[tauri::command]
pub fn dock_config() -> DockConfig {
    DockConfig::load()
}

#[tauri::command]
pub fn dock_set_enabled(app: AppHandle, enabled: bool) -> Result<DockConfig, String> {
    let mut config = DockConfig::load();
    config.enabled = enabled;
    config.save()?;

    // Hand the window work to the main thread rather than doing it here.
    //
    // A command handler runs on a worker thread, and building a webview window
    // from one blocks waiting for the event loop: the window gets created, but
    // every line after `build()` — placing it, showing it — never runs. The
    // symptom is a dock that reappears at the default size in the top-left
    // corner with a page that never loaded, which looks exactly like the
    // toggle doing nothing. At startup this was invisible because `setup()`
    // already runs on the main thread.
    let handle = app.clone();
    app.run_on_main_thread(move || {
        if enabled {
            if let Err(err) = show(&handle) {
                trace(&format!("show() failed: {err}"));
            }
        } else {
            hide(&handle);
        }
    })
    .map_err(|e| e.to_string())?;

    Ok(config)
}

/// Appends a line to `%LOCALAPPDATA%\mino-win-style\dock.log`.
///
/// The dock has no console and its webview has no reachable devtools, so
/// without this there is no way to tell "the page never ran" from "the page ran
/// and did nothing" — which cost an afternoon once.
pub fn trace(line: &str) {
    let path = config_path().with_file_name("dock.log");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    use std::io::Write;
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{} {line}", crate::dock::now());
    }
}

fn now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}.{:03}", d.as_secs(), d.subsec_millis()))
        .unwrap_or_default()
}

/// Lets the dock page report what happened to it. Without this the only signal
/// from a broken page is silence.
#[tauri::command]
pub fn dock_trace(line: String) {
    trace(&format!("page: {line}"));
}

#[tauri::command]
pub fn dock_layout() -> DockLayout {
    trace("dock_layout called");
    let area = mino_shell::work_area();
    let config = DockConfig::load();
    DockLayout {
        work_x: area.x,
        work_y: area.y,
        work_width: area.width,
        work_height: area.height,
        icon_size: config.icon_size,
    }
}

#[tauri::command]
pub fn dock_items() -> Vec<mino_shell::DockItem> {
    mino_shell::dock_items(&DockConfig::load().pinned)
}

#[derive(Debug, Clone, Serialize)]
pub struct IconData {
    pub width: u32,
    pub height: u32,
    /// Base64 of straight-alpha RGBA. The page turns it into an image once and
    /// keeps it; sending it as a JSON array of numbers would be five times the
    /// size for the same bytes.
    pub rgba_base64: String,
}

#[tauri::command]
pub fn dock_icon(exe: String, size: u32) -> Option<IconData> {
    mino_shell::icon_rgba(&exe, size.clamp(16, 256)).map(|icon| IconData {
        width: icon.width,
        height: icon.height,
        rgba_base64: base64(&icon.rgba),
    })
}

#[tauri::command]
pub fn dock_activate(hwnd: isize) -> bool {
    mino_shell::activate(hwnd)
}

#[tauri::command]
pub fn dock_launch(target: String) -> bool {
    mino_shell::launch(&target)
}

#[tauri::command]
pub fn dock_minimize(hwnd: isize) -> bool {
    mino_shell::minimize(hwnd)
}

#[tauri::command]
pub fn dock_toggle_maximize(hwnd: isize) -> bool {
    mino_shell::toggle_maximize(hwnd)
}

/// Asks the window to close. The app may refuse, or prompt about unsaved work —
/// which is the point of asking rather than killing it.
#[tauri::command]
pub fn dock_close(hwnd: isize) -> bool {
    mino_shell::close(hwnd)
}

#[tauri::command]
pub fn dock_pin(exe: String) -> Result<DockConfig, String> {
    let mut config = DockConfig::load();
    if !config.pinned.iter().any(|p| same_path(p, &exe)) {
        config.pinned.push(exe);
        config.save()?;
    }
    Ok(config)
}

#[tauri::command]
pub fn dock_unpin(exe: String) -> Result<DockConfig, String> {
    let mut config = DockConfig::load();
    let before = config.pinned.len();
    config.pinned.retain(|p| !same_path(p, &exe));
    if config.pinned.len() != before {
        config.save()?;
    }
    Ok(config)
}

/// Windows paths are case-insensitive, and the same executable can arrive from
/// a pin (as typed) or from an enumerated window (as Windows reports it).
fn same_path(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Places the dock centred along the bottom of the primary monitor's work area.
///
/// The page knows how wide it needs to be — it has just laid the icons out — so
/// it tells us, rather than Rust trying to predict a CSS layout.
#[tauri::command]
pub fn dock_place(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return Ok(());
    };
    place_window(&window, width, height)
}

/// Minimal base64. A dependency for twenty lines of table lookup would be a
/// poor trade, and this one is exercised by the dock on every icon.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn base64_matches_the_reference_vectors() {
        // RFC 4648 section 10.
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
