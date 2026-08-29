//! The shell layer: our own surfaces, drawn on top of Windows.
//!
//! `mino-core` changes settings Windows exposes. This crate exists because that
//! has a ceiling — Windows has no dock and no way to add one — so the dock is
//! ours, drawn in a window of our own. Nothing here patches or injects; it
//! enumerates windows, reads icons out of executables, and asks Windows to bring
//! a window forward, all through the documented API.

use serde::{Deserialize, Serialize};

#[cfg(windows)]
pub mod appbar;
#[cfg(windows)]
mod telemetry;
#[cfg(windows)]
mod windows_impl;

/// A top-level window worth showing on a dock.
#[derive(Debug, Clone, Serialize)]
pub struct AppWindow {
    /// The raw `HWND`, passed back when the user clicks. Only meaningful while
    /// the window still exists, which is why the dock re-reads the list often.
    pub hwnd: isize,
    pub title: String,
    pub exe: String,
    pub minimized: bool,
    pub maximized: bool,
}

/// Straight-alpha RGBA, top row first.
#[derive(Debug, Clone, Serialize)]
pub struct Icon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Which side of the screen a surface sits on.
///
/// Only `Top` is used today — it is what the bar wears. The other three are
/// here because the appbar dance is identical for all four and writing it once
/// is what makes a dock down the left side a placement rather than a rewrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// The rectangle a strip of `thickness` wants along one edge of a monitor.
///
/// Pure, and deliberately not left inline in the appbar code: this is the
/// arithmetic that puts a bar half off the screen when it is wrong, and here it
/// can be checked on any machine.
pub fn bar_rect(monitor: WorkArea, edge: Edge, thickness: i32) -> WorkArea {
    let along_y = thickness.clamp(0, monitor.height.max(0));
    let along_x = thickness.clamp(0, monitor.width.max(0));

    match edge {
        Edge::Top => WorkArea {
            height: along_y,
            ..monitor
        },
        Edge::Bottom => WorkArea {
            y: monitor.y + monitor.height - along_y,
            height: along_y,
            ..monitor
        },
        Edge::Left => WorkArea {
            width: along_x,
            ..monitor
        },
        Edge::Right => WorkArea {
            x: monitor.x + monitor.width - along_x,
            width: along_x,
            ..monitor
        },
    }
}

/// Physical pixels to the logical ones every window placement call wants, as
/// `(x, y, width, height)`.
///
/// The guard on `scale` is not defensive programming for its own sake: a scale
/// factor of zero would divide a placement into infinity, and a window placed
/// at infinity is gone with no error anywhere.
pub fn logical(area: WorkArea, scale: f64) -> (f64, f64, f64, f64) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    (
        f64::from(area.x) / scale,
        f64::from(area.y) / scale,
        f64::from(area.width) / scale,
        f64::from(area.height) / scale,
    )
}

/// What the machine is doing right now, for the HUD to draw.
///
/// Raw units throughout — bytes, seconds, bytes per second — because the page
/// knows how it wants to write them and Rust guessing at "1.2 GB" would put a
/// formatting decision, and an untranslatable one, in the wrong layer.
#[derive(Debug, Clone, Serialize)]
pub struct Telemetry {
    /// 0–100 across all cores, averaged over the interval since the last read.
    pub cpu_percent: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub net_down_bps: f64,
    pub net_up_bps: f64,
    pub uptime_seconds: u64,
    /// `None` on a machine with no battery.
    pub battery: Option<Battery>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
}

#[cfg(windows)]
pub use telemetry::Sampler;
#[cfg(windows)]
pub use windows_impl::{
    activate, close, foreground, icon_rgba, is_maximized, launch, minimize, screen_area,
    toggle_maximize, windows, work_area,
};

/// One entry on the dock: an application, whether or not it is running.
///
/// Several windows of the same program collapse into one entry, the way a dock
/// works — the count is what the running indicator under the icon shows.
#[derive(Debug, Clone, Serialize)]
pub struct DockItem {
    /// The executable path, which is also the identity of the entry.
    pub exe: String,
    /// "Notepad" — the file name without extension, title-cased by Windows'
    /// own convention of naming executables after the program.
    pub name: String,
    pub pinned: bool,
    pub windows: Vec<AppWindow>,
}

impl DockItem {
    pub fn running(&self) -> bool {
        !self.windows.is_empty()
    }
}

/// Builds the dock's contents: the pinned entries first, in the order they were
/// pinned, then anything else that happens to be running.
#[cfg(windows)]
pub fn dock_items(pinned: &[String]) -> Vec<DockItem> {
    use std::collections::BTreeMap;

    let mut by_exe: BTreeMap<String, Vec<AppWindow>> = BTreeMap::new();
    for window in windows() {
        by_exe
            .entry(window.exe.to_lowercase())
            .or_default()
            .push(window);
    }

    let mut items = Vec::new();

    for exe in pinned {
        let key = exe.to_lowercase();
        items.push(DockItem {
            name: display_name(exe),
            windows: by_exe.remove(&key).unwrap_or_default(),
            exe: exe.clone(),
            pinned: true,
        });
    }

    for (_, windows) in by_exe {
        let exe = windows[0].exe.clone();
        items.push(DockItem {
            name: display_name(&exe),
            exe,
            pinned: false,
            windows,
        });
    }

    items
}

/// `C:\Windows\notepad.exe` -> `Notepad`.
pub fn display_name(exe: &str) -> String {
    let stem = std::path::Path::new(exe)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(exe);

    let mut chars = stem.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => stem.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: WorkArea = WorkArea {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    #[test]
    fn a_top_bar_spans_the_width_and_sits_at_the_top() {
        let bar = bar_rect(SCREEN, Edge::Top, 26);
        assert_eq!(
            bar,
            WorkArea {
                x: 0,
                y: 0,
                width: 1920,
                height: 26
            }
        );
    }

    #[test]
    fn the_other_three_edges_are_the_same_arithmetic() {
        assert_eq!(bar_rect(SCREEN, Edge::Bottom, 26).y, 1054);
        assert_eq!(bar_rect(SCREEN, Edge::Left, 64).width, 64);
        assert_eq!(bar_rect(SCREEN, Edge::Right, 64).x, 1856);
        // A left or right bar is as tall as the screen; a top or bottom one is
        // as wide.
        assert_eq!(bar_rect(SCREEN, Edge::Left, 64).height, 1080);
        assert_eq!(bar_rect(SCREEN, Edge::Bottom, 26).width, 1920);
    }

    #[test]
    fn a_monitor_that_does_not_start_at_the_origin_is_still_covered() {
        // A primary monitor to the right of another one has a non-zero x, and
        // a bar that ignored it would be drawn on the wrong screen.
        let secondary = WorkArea {
            x: -1920,
            y: 120,
            width: 1280,
            height: 720,
        };
        let bar = bar_rect(secondary, Edge::Top, 30);
        assert_eq!(
            (bar.x, bar.y, bar.width, bar.height),
            (-1920, 120, 1280, 30)
        );
    }

    #[test]
    fn a_bar_cannot_be_thicker_than_the_screen_it_is_on() {
        // Nonsense in the config file is a strip that covers the desktop and
        // cannot be clicked past, so it is clamped rather than trusted.
        assert_eq!(bar_rect(SCREEN, Edge::Top, 99_999).height, 1080);
        assert_eq!(bar_rect(SCREEN, Edge::Top, -5).height, 0);
    }

    #[test]
    fn placement_divides_by_the_scale_factor() {
        let (x, y, width, height) = logical(bar_rect(SCREEN, Edge::Top, 26), 1.5);
        assert_eq!((x, y), (0.0, 0.0));
        assert_eq!(width, 1280.0);
        assert!((height - 17.333_333).abs() < 0.001);
    }

    #[test]
    fn a_nonsense_scale_factor_places_at_one_rather_than_at_infinity() {
        let area = bar_rect(SCREEN, Edge::Top, 26);
        assert_eq!(logical(area, 0.0), (0.0, 0.0, 1920.0, 26.0));
        assert_eq!(logical(area, f64::NAN), (0.0, 0.0, 1920.0, 26.0));
    }

    #[test]
    fn names_come_from_the_file_stem() {
        assert_eq!(display_name(r"C:\Windows\notepad.exe"), "Notepad");
        assert_eq!(display_name(r"C:\Windows\explorer.exe"), "Explorer");
        assert_eq!(display_name("weird"), "Weird");
    }
}
