//! JARVIS mode: a heads-up display over the desktop, and a skin for our own
//! windows.
//!
//! Three things happen when it is switched on, and only the first two are ours
//! to do without asking:
//!
//! 1. The HUD window appears — a full-screen, transparent, click-through
//!    overlay drawn by us, over everything, touching nothing.
//! 2. The app and the dock re-skin themselves, which is a CSS attribute and a
//!    broadcast event.
//! 3. The UI *offers* the JARVIS Look. That one writes to the registry, so it
//!    goes through the same confirmation screen as every other change and is
//!    the user's decision, not the toggle's. Nothing here applies it.
//!
//! The click-through is what makes the overlay liveable. Without
//! `WS_EX_TRANSPARENT` — which Tauri sets for us through
//! `set_ignore_cursor_events` — a full-screen always-on-top window swallows
//! every click on the desktop, and the machine becomes unusable in a way that
//! looks exactly like a crash.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder,
};

pub const WINDOW_LABEL: &str = "hud";

/// How long the page's shutdown animation runs before the window is taken away.
///
/// The window has to outlive the animation or the overlay vanishes mid-fade,
/// which reads as a crash rather than as a power-down. Keep this in step with
/// the `--shutdown` duration in `hud.css`.
const SHUTDOWN_MS: u64 = 1_400;

/// `serde(default)` at the container level, not on the fields: it fills a
/// missing field from this struct's own `Default`, so a config written by an
/// older build keeps every preference it does have instead of the whole file
/// failing to parse and resetting the lot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct JarvisConfig {
    /// The whole mode: HUD, skin and all.
    pub enabled: bool,
    /// Spoken greeting and interface sounds. Off by default — a machine that
    /// starts talking on its own in a meeting is a bug, whatever the intent.
    pub sound: bool,
    /// The live readouts. Turning them off leaves the HUD's decoration, arc and
    /// clock, and stops the polling.
    pub telemetry: bool,
    /// What the greeting calls you. Empty means it says nothing after the
    /// time of day.
    pub address: String,
}

impl Default for JarvisConfig {
    fn default() -> Self {
        JarvisConfig {
            enabled: false,
            sound: false,
            telemetry: true,
            address: String::new(),
        }
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("mino-win-style").join("jarvis.json")
}

impl JarvisConfig {
    /// A missing or malformed file is not an error: refusing to start because a
    /// preferences file is broken would be worse than starting switched off.
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

/// Creates the HUD window, once, at startup.
///
/// Built whether or not the mode is on, and left hidden when it is off — the
/// same reason the dock does it this way: building a webview window from a
/// command handler blocks on the event loop, so every line after `build()`
/// silently never runs. See `dock::create`.
pub fn create(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(WINDOW_LABEL).is_some() {
        return Ok(());
    }

    let area = mino_shell::screen_area();

    WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::App("hud.html".into()))
        .title("Mino HUD")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        // Never take focus. The overlay is scenery; whatever the user was
        // typing into stays where it was.
        .focused(false)
        .visible(false)
        .inner_size(f64::from(area.width), f64::from(area.height))
        .build()
        .map_err(|e| format!("could not create the HUD window: {e}"))?;

    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        // Every click, every scroll and every hover goes straight through to
        // whatever is underneath. This is the line that makes a full-screen
        // always-on-top window something you can work in front of.
        window
            .set_ignore_cursor_events(true)
            .map_err(|e| format!("the HUD could not be made click-through: {e}"))?;
        place(&window);
    }

    Ok(())
}

/// Sizes the HUD to the whole primary monitor, taskbar included.
fn place(window: &tauri::WebviewWindow) {
    let area = mino_shell::screen_area();
    let scale = window.scale_factor().unwrap_or(1.0);

    let _ = window.set_size(LogicalSize::new(
        f64::from(area.width) / scale,
        f64::from(area.height) / scale,
    ));
    let _ = window.set_position(LogicalPosition::new(
        f64::from(area.x) / scale,
        f64::from(area.y) / scale,
    ));
}

/// Brings the HUD up, with the boot sequence.
pub fn show(app: &AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return Err("the HUD window is missing; restart the app".into());
    };

    place(&window);
    // Told before it is shown, so the page starts its boot animation on the
    // first frame anyone sees rather than a few frames in.
    let _ = app.emit_to(WINDOW_LABEL, "jarvis-boot", JarvisConfig::load());
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_always_on_top(true);
    let _ = window.set_ignore_cursor_events(true);
    Ok(())
}

/// Plays the power-down, then takes the window away.
///
/// The wait runs on a spawned thread rather than blocking: this is called from
/// a command handler, and sleeping there would freeze the toggle the user just
/// clicked for the length of the animation.
pub fn hide(app: &AppHandle) {
    if app.get_webview_window(WINDOW_LABEL).is_none() {
        return;
    }
    let _ = app.emit_to(WINDOW_LABEL, "jarvis-shutdown", ());

    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(SHUTDOWN_MS));
        let _ = handle.clone().run_on_main_thread(move || {
            if let Some(window) = handle.get_webview_window(WINDOW_LABEL) {
                let _ = window.hide();
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn jarvis_config() -> JarvisConfig {
    JarvisConfig::load()
}

/// Turns the whole mode on or off.
///
/// Returns as soon as the config is written; the window work is handed to the
/// main thread for the same reason the dock does it — a webview window cannot
/// be shown from a worker thread without deadlocking on the event loop.
#[tauri::command]
pub fn jarvis_set_enabled(app: AppHandle, enabled: bool) -> Result<JarvisConfig, String> {
    let mut config = JarvisConfig::load();
    config.enabled = enabled;
    config.save()?;

    // Every window hears this, which is how the settings window and the dock
    // re-skin themselves without polling.
    let _ = app.emit("jarvis-mode", &config);

    let handle = app.clone();
    app.run_on_main_thread(move || {
        if enabled {
            if let Err(err) = show(&handle) {
                crate::dock::trace(&format!("jarvis show() failed: {err}"));
            }
        } else {
            hide(&handle);
        }
    })
    .map_err(|e| e.to_string())?;

    Ok(config)
}

/// Changes one preference without touching whether the mode is on.
#[tauri::command]
pub fn jarvis_set_options(
    app: AppHandle,
    sound: Option<bool>,
    telemetry: Option<bool>,
    address: Option<String>,
) -> Result<JarvisConfig, String> {
    let mut config = JarvisConfig::load();
    if let Some(sound) = sound {
        config.sound = sound;
    }
    if let Some(telemetry) = telemetry {
        config.telemetry = telemetry;
    }
    if let Some(address) = address {
        // A name is written into a spoken sentence, so it is trimmed and capped
        // rather than passed through: the length is what stops a paragraph
        // pasted in here from being read aloud.
        config.address = address.trim().chars().take(40).collect();
    }
    config.save()?;
    let _ = app.emit("jarvis-mode", &config);
    Ok(config)
}

/// One reading of the machine.
///
/// The processor and network figures are rates, so they are measured against
/// the previous call — which makes the sampler state that has to outlive the
/// command, hence its place in the app's managed state.
#[tauri::command]
pub fn jarvis_telemetry(sampler: tauri::State<'_, mino_shell::Sampler>) -> mino_shell::Telemetry {
    sampler.read()
}

#[cfg(test)]
mod tests {
    use super::JarvisConfig;

    #[test]
    fn the_mode_and_its_sound_both_start_off() {
        let config = JarvisConfig::default();
        assert!(!config.enabled);
        assert!(!config.sound, "a machine must not start talking unasked");
        assert!(config.telemetry);
    }

    #[test]
    fn a_missing_field_falls_back_rather_than_failing_to_parse() {
        // A config written by an older build has no `address`. Serde fills it
        // from Default rather than rejecting the file, which is what keeps an
        // upgrade from silently resetting the user's preferences.
        let config: JarvisConfig =
            serde_json::from_str(r#"{"enabled":true,"sound":true,"telemetry":false}"#)
                .expect("older configs must still parse");
        assert!(config.enabled);
        assert!(config.sound);
        assert!(!config.telemetry);
        assert_eq!(config.address, "");
    }
}
