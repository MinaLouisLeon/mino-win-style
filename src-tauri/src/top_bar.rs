//! The bar: a strip across the top of the primary monitor, in a window of our
//! own.
//!
//! It is the third surface, and the first that does not merely sit on top of
//! the desktop but takes a piece of it. The difference from the other two is
//! worth having in one place:
//!
//! |            | Dock          | Overlay              | Bar                    |
//! |------------|---------------|----------------------|------------------------|
//! | Clicks     | takes them    | passes them through  | takes them             |
//! | Work area  | floats over   | covers the screen    | **reserves a slice**   |
//! | Focus      | never takes   | never takes          | takes, when clicked    |
//!
//! The reservation is the whole difficulty. A dock at the bottom can float,
//! because a maximized window going underneath it costs nothing — the taskbar
//! there is auto-hidden anyway. A bar across the top cannot: a maximized window
//! would put its own title bar and close button under ours, where they cannot
//! be reached. So Windows is asked to keep the strip, through the appbar
//! protocol in `mino_shell::appbar`, and the hazard that comes with asking —
//! dead space left behind by a process that died before giving it back — is
//! answered there and by `mino shell-reset`.
//!
//! What the bar shows is deliberately only what it can act on. There is no
//! supported way to read another application's File/Edit/View from outside its
//! process, and this project does not go inside one, so the bar carries the
//! focused application's *name*, the three window commands we genuinely
//! implement, and our own menu. Greyed-out menus that did nothing would be the
//! one dishonest thing in this app.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder,
};

pub const WINDOW_LABEL: &str = "topbar";

/// Tall enough for a name and a clock at the app's smallest readable size, and
/// no taller: every pixel here is one the desktop does not get back.
const DEFAULT_HEIGHT: u32 = 28;

/// Above this the bar stops being a bar. The clamp is not politeness — the
/// height goes straight into a reservation, and a config file saying 4000
/// would hand the whole screen to a strip nobody can click past.
const MAX_HEIGHT: u32 = 96;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TopBarConfig {
    pub enabled: bool,
    /// Logical pixels. Physical ones are what the reservation is made in, which
    /// is why every use of this goes through the scale factor first.
    pub height: u32,
}

impl Default for TopBarConfig {
    fn default() -> Self {
        TopBarConfig {
            enabled: false,
            height: DEFAULT_HEIGHT,
        }
    }
}

impl TopBarConfig {
    /// A missing or malformed file is not an error: the bar has a default, and
    /// refusing to start because a preferences file is broken would be worse
    /// than starting with the bar switched off.
    pub fn load() -> Self {
        let mut config: TopBarConfig = fs::read_to_string(config_path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        config.height = config.height.clamp(16, MAX_HEIGHT);
        config
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

fn config_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mino-win-style")
        .join("topbar.json")
}

/// Built at startup on the main thread whether or not the bar is switched on,
/// and simply left hidden when it is off.
///
/// The reason is the one `dock::create` gives in full: building a webview
/// window anywhere except application setup blocks on the event loop, so the
/// window appears but every line after `build()` is never reached — never
/// placed, never registered, page never loaded. Toggling therefore moves a
/// window that already exists rather than making a new one.
pub fn create(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(WINDOW_LABEL).is_some() {
        return Ok(());
    }

    let area = mino_shell::screen_area();
    let config = TopBarConfig::load();

    WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::App("topbar.html".into()))
        .title("Mino Bar")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        // Out of the taskbar and out of Alt+Tab: the bar is furniture, not a
        // window someone switches to.
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        // Not focused when it appears. Unlike the dock and the overlay it *can*
        // take focus — it has buttons and menus — but it must never take it
        // just by existing.
        .focused(false)
        .visible(false)
        .inner_size(f64::from(area.width), f64::from(config.height))
        .build()
        .map_err(|e| format!("could not create the bar window: {e}"))?;

    Ok(())
}

/// Reserves the strip and puts the bar in it.
///
/// The rectangle the window is placed to is the one Windows *granted*, not the
/// one that was asked for: another appbar may already hold part of that edge,
/// and placing to the request rather than the answer is how a bar ends up
/// overlapping the taskbar it was supposed to sit beside.
pub fn show(app: &AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return Err("the bar window is missing; restart the app".into());
    };

    let config = TopBarConfig::load();
    let scale = window.scale_factor().unwrap_or(1.0);

    // The HWND crosses a crate boundary here as a plain integer on purpose:
    // Tauri and `mino-shell` are built against different versions of the
    // `windows` crate, so their `HWND` types are not the same type at all.
    let hwnd = window
        .hwnd()
        .map_err(|e| format!("the bar has no window handle yet: {e}"))?
        .0 as isize;

    // The reservation is made in physical pixels; the height in the config is
    // logical, which on a 150% display is a third as many again.
    let thickness = (f64::from(config.height) * scale).round() as i32;

    let granted = mino_shell::appbar::register(hwnd, mino_shell::Edge::Top, thickness)
        .ok_or("Windows refused to reserve the strip for the bar")?;

    place(&window, granted, scale)?;
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_always_on_top(true);
    let _ = app.emit_to(WINDOW_LABEL, "top-bar-active", true);
    Ok(())
}

/// Takes the bar off screen **and gives the strip back**.
///
/// The order matters: hiding a window that still holds a reservation leaves the
/// desktop a band short with nothing visible to explain it, which is the exact
/// failure this whole module is careful about.
pub fn hide(app: &AppHandle) {
    mino_shell::appbar::unregister();
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.hide();
        let _ = app.emit_to(WINDOW_LABEL, "top-bar-active", false);
    }
}

fn place(
    window: &tauri::WebviewWindow,
    area: mino_shell::WorkArea,
    scale: f64,
) -> Result<(), String> {
    let (x, y, width, height) = mino_shell::logical(area, scale);
    window
        .set_size(LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    window
        .set_position(LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn top_bar_config() -> TopBarConfig {
    TopBarConfig::load()
}

#[tauri::command]
pub fn top_bar_set_enabled(app: AppHandle, enabled: bool) -> Result<TopBarConfig, String> {
    let mut config = TopBarConfig::load();
    config.enabled = enabled;
    config.save()?;

    // The window work goes to the main thread for the same reason the dock's
    // does: a webview window cannot be shown from a worker thread without
    // deadlocking on the event loop.
    let handle = app.clone();
    app.run_on_main_thread(move || {
        if enabled {
            if let Err(err) = show(&handle) {
                crate::dock::trace(&format!("bar show() failed: {err}"));
            }
        } else {
            hide(&handle);
        }
    })
    .map_err(|e| e.to_string())?;

    Ok(config)
}

/// Whatever the user is working in, or `None` when that is one of ours.
///
/// The page keeps the last answer that was not `None`, which is what stops the
/// bar renaming itself to *Mino* at the exact moment someone clicks it.
#[tauri::command]
pub fn top_bar_foreground() -> Option<mino_shell::AppWindow> {
    mino_shell::foreground()
}

/// Brings the settings window back, from the bar's own menu.
///
/// Everything the bar could offer as a menu — the Look picker, the dock switch,
/// this one — already exists there, and a second copy of a control is a second
/// thing to keep true.
#[tauri::command]
pub fn top_bar_open_settings(app: AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Err("the settings window is missing".into());
    };
    window.show().map_err(|e| e.to_string())?;
    let _ = window.unminimize();
    window.set_focus().map_err(|e| e.to_string())
}

/// Quits, giving the strip back on the way out.
///
/// `AppHandle::exit` runs the exit handler in `lib.rs`, which unregisters — but
/// this asks directly as well, because the one failure that must never happen
/// is leaving the reservation behind, and belt and braces is cheap.
#[tauri::command]
pub fn top_bar_quit(app: AppHandle) {
    mino_shell::appbar::unregister();
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_starts_switched_off() {
        assert!(!TopBarConfig::default().enabled);
    }

    #[test]
    fn a_height_from_a_hand_edited_file_is_clamped_not_trusted() {
        // The height becomes a reservation, so the nonsense case is a strip
        // that covers the screen and cannot be clicked past.
        let config: TopBarConfig =
            serde_json::from_str(r#"{"enabled":true,"height":4000}"#).expect("must parse");
        let clamped = config.height.clamp(16, MAX_HEIGHT);
        assert_eq!(clamped, MAX_HEIGHT);
    }

    #[test]
    fn a_missing_field_falls_back_rather_than_failing_to_parse() {
        let config: TopBarConfig = serde_json::from_str(r#"{"enabled":true}"#).expect("must parse");
        assert!(config.enabled);
        assert_eq!(config.height, DEFAULT_HEIGHT);
    }
}
