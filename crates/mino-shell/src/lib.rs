//! The shell layer: our own surfaces, drawn on top of Windows.
//!
//! `mino-core` changes settings Windows exposes. This crate exists because that
//! has a ceiling — Windows has no dock and no way to add one — so the dock is
//! ours, drawn in a window of our own. Nothing here patches or injects; it
//! enumerates windows, reads icons out of executables, and asks Windows to bring
//! a window forward, all through the documented API.

use serde::Serialize;

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
}

/// Straight-alpha RGBA, top row first.
#[derive(Debug, Clone, Serialize)]
pub struct Icon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[cfg(windows)]
pub use windows_impl::{activate, icon_rgba, launch, windows, work_area};

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

    #[test]
    fn names_come_from_the_file_stem() {
        assert_eq!(display_name(r"C:\Windows\notepad.exe"), "Notepad");
        assert_eq!(display_name(r"C:\Windows\explorer.exe"), "Explorer");
        assert_eq!(display_name("weird"), "Weird");
    }
}
