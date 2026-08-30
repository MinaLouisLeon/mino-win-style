//! Shell Looks: which desktop we are wearing.
//!
//! This was JARVIS mode, and JARVIS is now the first entry in a registry rather
//! than the registry itself. The change that matters is in the type: there is
//! exactly one Look worn at a time, so the config holds a *selection* —
//! `Option<LookId>` — and not a boolean per look that could disagree with the
//! others about which one is on.
//!
//! Three things happen when a Look is switched on, and only the first two are
//! ours to do without asking:
//!
//! 1. The surfaces it draws appear — for JARVIS, the HUD: a full-screen,
//!    transparent, click-through overlay drawn by us, over everything, touching
//!    nothing.
//! 2. The app and the dock re-skin themselves, which is a CSS attribute and a
//!    broadcast event.
//! 3. The UI *offers* the Look's pack. That one writes to the registry, so it
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

use serde::{Deserialize, Deserializer, Serialize};
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

// ---------------------------------------------------------------------------
// The registry
// ---------------------------------------------------------------------------

/// Every Look this build has.
///
/// A variant is only added when its Look is actually built — a `LookId` with no
/// entry in [`LOOKS`] would be selectable and have no CSS behind it, and the
/// table test below fails the build if the two ever drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LookId {
    Jarvis,
    Cupertino,
    Yaru,
}

impl LookId {
    /// The `data-theme` value, which is also how this serialises. One string,
    /// so the CSS block, the config file and the wire format cannot disagree.
    pub fn as_str(self) -> &'static str {
        match self {
            LookId::Jarvis => "jarvis",
            LookId::Cupertino => "cupertino",
            LookId::Yaru => "yaru",
        }
    }

    /// `None` for anything this build does not have — see [`lenient_look`].
    fn parse(raw: &str) -> Option<Self> {
        LOOKS
            .iter()
            .map(|look| look.id)
            .find(|id| id.as_str() == raw)
    }
}

/// One of our own windows. Not a Windows setting — a thing we draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Surface {
    /// The full-screen click-through overlay. Ours to show without asking.
    Overlay,
    /// The dock. It has its own switch and its own config, so a Look that wants
    /// it has to *ask* — see the note on `surfaces` below.
    Dock,
    /// The bar. Asked for the same way, and with more reason: it takes a strip
    /// of the desktop away from every other window.
    TopBar,
}

/// A Look is a description, not code: a theme name, the surfaces it draws, and
/// the pack it offers. Adding one is an entry here, a CSS block, and — if it
/// draws an overlay — a component on the page.
///
/// `surfaces` is a statement of what the Look wants, not permission to take it.
/// The overlay is ours — it changes nothing outside our own process, so
/// [`apply_surfaces`] shows and hides it without asking. The dock and the bar
/// are not: they have their own switches, the dock has the user's pinned list
/// in it, and the bar takes a strip of the desktop away from everything else.
/// So a Look that lists either *offers* it, in the same register as its pack,
/// and the answer is the user's. Nothing here turns them on, and nothing here
/// turns them off again either: a surface someone accepted stays theirs, on the
/// switch it has always had.
/// How a Look wants the dock, if it wants one at all.
///
/// Part of the registry rather than of the UI so that adding a Look stays one
/// entry: the picker reads this and passes it on, and nothing in TypeScript has
/// to know that Cupertino hides its dock and Yaru stands its up.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DockWish {
    pub edge: mino_shell::Edge,
    /// Whether it waits at that edge until the pointer comes for it.
    pub hover: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Look {
    pub id: LookId,
    pub theme: &'static str,
    pub surfaces: &'static [Surface],
    /// The pack *offered* when the Look is switched on. Never applied here.
    pub pack_id: Option<&'static str>,
    /// Only read when the dock is offered *and* accepted. A Look never moves a
    /// dock somebody already had.
    pub dock: Option<DockWish>,
}

impl Look {
    pub fn draws(&self, surface: Surface) -> bool {
        self.surfaces.contains(&surface)
    }
}

pub const LOOKS: &[Look] = &[
    Look {
        id: LookId::Jarvis,
        theme: "jarvis",
        surfaces: &[Surface::Overlay],
        pack_id: Some("com.mino.jarvis"),
        dock: None,
    },
    Look {
        id: LookId::Cupertino,
        theme: "cupertino",
        surfaces: &[Surface::TopBar, Surface::Dock],
        // The Look is called Cupertino and the pack is still called macOS. One
        // pack, already shipped and already with a wallpaper, rather than a
        // second near-identical one to keep in step — the picker says so, so
        // nobody reads it as a setting having gone missing.
        pack_id: Some("com.mino.macos"),
        dock: Some(DockWish {
            edge: mino_shell::Edge::Bottom,
            hover: true,
        }),
    },
    Look {
        id: LookId::Yaru,
        theme: "yaru",
        surfaces: &[Surface::TopBar, Surface::Dock],
        pack_id: Some("com.mino.yaru"),
        // Ubuntu's dock stands down the left and stays there: windows maximize
        // beside it rather than under it, which is what makes it reserve.
        dock: Some(DockWish {
            edge: mino_shell::Edge::Left,
            hover: false,
        }),
    },
];

/// The entry for a Look. Total by construction: `LookId` only has variants that
/// [`LOOKS`] carries, and a test holds the two together.
pub fn look(id: LookId) -> &'static Look {
    LOOKS
        .iter()
        .find(|look| look.id == id)
        .expect("every LookId has a LOOKS entry")
}

// ---------------------------------------------------------------------------
// Preferences
// ---------------------------------------------------------------------------

/// `serde(default)` at the container level, not on the fields: it fills a
/// missing field from this struct's own `Default`, so a config written by an
/// older build keeps every preference it does have instead of the whole file
/// failing to parse and resetting the lot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    /// Which Look is worn. `None` is plain Fluent — the app as it ships.
    #[serde(deserialize_with = "lenient_look")]
    pub active: Option<LookId>,
    /// Spoken greeting and interface sounds. Off by default — a machine that
    /// starts talking on its own in a meeting is a bug, whatever the intent.
    ///
    /// This and the two below outlive a switch: set them under JARVIS, move to
    /// another Look and back, and they are still here.
    pub sound: bool,
    /// The live readouts. Turning them off leaves the overlay's decoration and
    /// stops the polling.
    pub telemetry: bool,
    /// What the greeting calls you. Empty means it says nothing after the
    /// time of day.
    pub address: String,
}

impl Default for ShellConfig {
    fn default() -> Self {
        ShellConfig {
            active: None,
            sound: false,
            telemetry: true,
            address: String::new(),
        }
    }
}

/// Reads `active` without letting an unknown name fail the file.
///
/// A config written by a *newer* build can name a Look this one does not have.
/// Rejecting the file would throw away the user's other preferences over a
/// field we already know how to not have; this falls back to no Look, which is
/// a desktop they can see and change.
fn lenient_look<'de, D>(deserializer: D) -> Result<Option<LookId>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = serde_json::Value::deserialize(deserializer)?;
    Ok(raw.as_str().and_then(LookId::parse))
}

/// JARVIS mode as it was written before the looks existed. Read once, to carry
/// a user's mode across the upgrade.
#[derive(Debug, Deserialize)]
#[serde(default)]
struct LegacyConfig {
    enabled: bool,
    sound: bool,
    telemetry: bool,
    address: String,
}

impl Default for LegacyConfig {
    fn default() -> Self {
        LegacyConfig {
            enabled: false,
            sound: false,
            telemetry: true,
            address: String::new(),
        }
    }
}

impl From<LegacyConfig> for ShellConfig {
    fn from(old: LegacyConfig) -> Self {
        ShellConfig {
            active: old.enabled.then_some(LookId::Jarvis),
            sound: old.sound,
            telemetry: old.telemetry,
            address: old.address,
        }
    }
}

fn local_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mino-win-style")
}

fn config_path() -> PathBuf {
    local_dir().join("shell.json")
}

/// Where JARVIS mode kept its preferences before this module existed.
fn legacy_path() -> PathBuf {
    local_dir().join("jarvis.json")
}

impl ShellConfig {
    /// A missing or malformed file is not an error: refusing to start because a
    /// preferences file is broken would be worse than starting with no Look.
    ///
    /// The one side effect is the upgrade: a machine with `jarvis.json` and no
    /// `shell.json` gets its mode carried across and written out once. The old
    /// file is left where it is, so downgrading gets the mode back rather than
    /// an empty desktop.
    pub fn load() -> Self {
        if let Some(config) = read::<ShellConfig>(&config_path()) {
            return config;
        }
        match read::<LegacyConfig>(&legacy_path()) {
            Some(legacy) => {
                let migrated = ShellConfig::from(legacy);
                let _ = migrated.save();
                migrated
            }
            None => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, text).map_err(|e| e.to_string())
    }

    /// The entry for the Look being worn, if any.
    pub fn look(&self) -> Option<&'static Look> {
        self.active.map(look)
    }

    fn draws(&self, surface: Surface) -> bool {
        self.look().is_some_and(|look| look.draws(surface))
    }
}

fn read<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Option<T> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

// ---------------------------------------------------------------------------
// The overlay window
// ---------------------------------------------------------------------------

/// Creates the overlay window, once, at startup.
///
/// Built whether or not a Look is worn, and left hidden when none is — the
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
        .map_err(|e| format!("could not create the overlay window: {e}"))?;

    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        // Every click, every scroll and every hover goes straight through to
        // whatever is underneath. This is the line that makes a full-screen
        // always-on-top window something you can work in front of.
        window
            .set_ignore_cursor_events(true)
            .map_err(|e| format!("the overlay could not be made click-through: {e}"))?;
        place(&window);
    }

    Ok(())
}

/// Sizes the overlay to the whole primary monitor, taskbar included.
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

/// Brings the overlay up, with the boot sequence.
///
/// The config goes along for the ride because it names the Look: the page has
/// one overlay component per Look and this is what tells it which to draw.
pub fn show(app: &AppHandle, config: &ShellConfig) -> Result<(), String> {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return Err("the overlay window is missing; restart the app".into());
    };

    place(&window);
    // Told before it is shown, so the page starts its boot animation on the
    // first frame anyone sees rather than a few frames in.
    let _ = app.emit_to(WINDOW_LABEL, "shell-boot", config);
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
    let _ = app.emit_to(WINDOW_LABEL, "shell-shutdown", ());

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

/// Brings our own surfaces into line with the Look being worn.
///
/// The overlay only, and deliberately: it is the one surface that is ours to
/// show without asking. A Look that wants the dock or the bar asks for it
/// through the UI, and the answer lands on those surfaces' own switches — which
/// is why switching Looks here can never turn one of them on or off behind the
/// user's back.
///
/// Must be called on the main thread — it shows and hides windows.
pub fn apply_surfaces(app: &AppHandle, config: &ShellConfig) {
    if config.draws(Surface::Overlay) {
        if let Err(err) = show(app, config) {
            crate::dock::trace(&format!("overlay show() failed: {err}"));
        }
    } else {
        hide(app);
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn shell_config() -> ShellConfig {
    ShellConfig::load()
}

/// The registry, for the picker.
///
/// The UI reads the list from here rather than keeping its own copy, so adding
/// a Look does not mean remembering to add it twice.
#[tauri::command]
pub fn shell_looks() -> &'static [Look] {
    LOOKS
}

/// Wears a Look, or takes the current one off with `None`.
///
/// Returns as soon as the config is written; the window work is handed to the
/// main thread for the same reason the dock does it — a webview window cannot
/// be shown from a worker thread without deadlocking on the event loop.
#[tauri::command]
pub fn shell_set_look(app: AppHandle, id: Option<LookId>) -> Result<ShellConfig, String> {
    let mut config = ShellConfig::load();
    config.active = id;
    config.save()?;

    // Every window hears this, which is how the settings window and the dock
    // re-skin themselves without polling.
    let _ = app.emit("shell-look", &config);

    let handle = app.clone();
    let for_thread = config.clone();
    app.run_on_main_thread(move || apply_surfaces(&handle, &for_thread))
        .map_err(|e| e.to_string())?;

    Ok(config)
}

/// Changes one preference without touching which Look is worn.
#[tauri::command]
pub fn shell_set_options(
    app: AppHandle,
    sound: Option<bool>,
    telemetry: Option<bool>,
    address: Option<String>,
) -> Result<ShellConfig, String> {
    let mut config = ShellConfig::load();
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
    let _ = app.emit("shell-look", &config);
    Ok(config)
}

/// One reading of the machine.
///
/// The processor and network figures are rates, so they are measured against
/// the previous call — which makes the sampler state that has to outlive the
/// command, hence its place in the app's managed state.
#[tauri::command]
pub fn shell_telemetry(sampler: tauri::State<'_, mino_shell::Sampler>) -> mino_shell::Telemetry {
    sampler.read()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_worn_and_nothing_talks_until_asked() {
        let config = ShellConfig::default();
        assert!(config.active.is_none());
        assert!(!config.sound, "a machine must not start talking unasked");
        assert!(config.telemetry);
    }

    #[test]
    fn jarvis_mode_carries_across_the_upgrade() {
        // What the previous build wrote to jarvis.json.
        let legacy: LegacyConfig = serde_json::from_str(
            r#"{"enabled":true,"sound":true,"telemetry":false,"address":"Sir"}"#,
        )
        .expect("the old config must still parse");
        let config = ShellConfig::from(legacy);

        assert_eq!(config.active, Some(LookId::Jarvis));
        assert!(config.sound);
        assert!(!config.telemetry);
        assert_eq!(config.address, "Sir");
    }

    #[test]
    fn a_mode_that_was_off_becomes_no_look_rather_than_jarvis() {
        let legacy: LegacyConfig = serde_json::from_str(r#"{"enabled":false}"#).unwrap();
        assert_eq!(ShellConfig::from(legacy).active, None);
    }

    #[test]
    fn a_missing_field_falls_back_rather_than_failing_to_parse() {
        // A config written by an older build has no `address`. Serde fills it
        // from Default rather than rejecting the file, which is what keeps an
        // upgrade from silently resetting the user's preferences.
        let config: ShellConfig =
            serde_json::from_str(r#"{"active":"jarvis","sound":true,"telemetry":false}"#)
                .expect("older configs must still parse");
        assert_eq!(config.active, Some(LookId::Jarvis));
        assert!(config.sound);
        assert!(!config.telemetry);
        assert_eq!(config.address, "");
    }

    #[test]
    fn a_look_this_build_does_not_have_is_no_look_at_all() {
        // Written by a newer build. The name is unknown here, but the rest of
        // the file is the user's and is kept.
        let config: ShellConfig =
            serde_json::from_str(r#"{"active":"phosphor","sound":true,"address":"Sir"}"#)
                .expect("an unknown Look must not fail the file");
        assert_eq!(config.active, None);
        assert!(config.sound);
        assert_eq!(config.address, "Sir");

        // The same goes for a value that is not even a name.
        let odd: ShellConfig = serde_json::from_str(r#"{"active":7}"#).unwrap();
        assert_eq!(odd.active, None);
    }

    /// A `LookId` with no entry would be selectable with no CSS behind it, and
    /// a `theme` that disagrees with the id would be a Look whose stylesheet is
    /// never found. Both are one-line mistakes to make when adding a Look, so
    /// they fail here rather than on screen.
    #[test]
    fn every_look_has_one_entry_and_answers_to_one_name() {
        for look in LOOKS {
            let id = serde_json::to_string(&look.id).unwrap();
            assert_eq!(
                id,
                format!("\"{}\"", look.theme),
                "the theme name and the wire name must match"
            );
            assert_eq!(look.id.as_str(), look.theme);
            assert_eq!(
                LOOKS.iter().filter(|other| other.id == look.id).count(),
                1,
                "{} appears more than once",
                look.theme
            );
            assert_eq!(LookId::parse(look.theme), Some(look.id));
        }
    }

    /// A Look that offers a pack nobody ships is a confirmation screen that
    /// never appears, which looks exactly like a switch doing nothing.
    #[test]
    fn every_offered_pack_is_one_that_ships() {
        let packs = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri has a parent")
            .join("packs");

        for look in LOOKS {
            let Some(pack_id) = look.pack_id else {
                continue;
            };
            let found = fs::read_dir(&packs)
                .expect("the packs directory is part of the repository")
                .flatten()
                .filter_map(|entry| {
                    mino_core::PackManifest::read(entry.path().join("manifest.json")).ok()
                })
                .any(|manifest| manifest.id == pack_id);
            assert!(found, "{} offers a pack that is not in packs/", look.theme);
        }
    }
}
