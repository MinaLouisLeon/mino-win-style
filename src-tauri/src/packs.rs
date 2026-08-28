//! Finding the Looks that ship with the app.
//!
//! A "Look" is a pack: a folder with a `manifest.json` and its assets. Where
//! that folder lives depends on how the app is running — from the repository
//! during development, from the install directory afterwards — so the search is
//! explicit rather than relying on the working directory, which for an app
//! launched from a Start menu shortcut is anyone's guess.

use std::path::{Path, PathBuf};

use mino_core::{Engine, PackManifest};
use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
pub struct PackSummary {
    pub id: String,
    /// Absolute path to the pack folder; the UI passes it straight back.
    pub dir: String,
    pub name: std::collections::BTreeMap<String, String>,
    pub description: std::collections::BTreeMap<String, String>,
    pub author: Option<String>,
    /// How many settings it touches that this build of Windows supports.
    pub settings: usize,
    /// False when the pack declares a build range this machine is outside of.
    pub applicable: bool,
}

/// Every directory that might hold packs, best first.
fn candidates(app: &AppHandle) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Installed: bundled next to the executable as a Tauri resource.
    if let Ok(resources) = app.path().resource_dir() {
        dirs.push(resources.join("packs"));
        dirs.push(resources);
    }

    // Running from `cargo tauri dev`, or a portable exe sitting in the repo.
    if let Ok(exe) = std::env::current_exe() {
        let mut here = exe.parent().map(Path::to_path_buf);
        for _ in 0..4 {
            match here {
                Some(dir) => {
                    dirs.push(dir.join("packs"));
                    here = dir.parent().map(Path::to_path_buf);
                }
                None => break,
            }
        }
    }

    dirs
}

pub fn find_all(app: &AppHandle, engine: &Engine) -> Vec<PackSummary> {
    let mut found: Vec<PackSummary> = Vec::new();

    for dir in candidates(app) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let manifest_path = entry.path().join("manifest.json");
            if !manifest_path.is_file() {
                continue;
            }
            let Ok(manifest) = PackManifest::read(&manifest_path) else {
                continue; // a broken pack is skipped, not fatal
            };
            if found.iter().any(|p| p.id == manifest.id) {
                continue; // an earlier directory already provided this one
            }
            found.push(PackSummary {
                id: manifest.id.clone(),
                dir: entry.path().to_string_lossy().into_owned(),
                name: manifest.name.clone(),
                description: manifest.description.clone(),
                author: manifest.author.clone(),
                settings: manifest.settings.len(),
                applicable: manifest.requires.allows(engine.os()),
            });
        }
        if !found.is_empty() {
            break;
        }
    }

    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

/// Loads one pack and turns it into settings with absolute asset paths.
pub fn settings_for(
    engine: &Engine,
    dir: &str,
) -> mino_core::Result<(PackManifest, mino_core::Settings)> {
    let base = PathBuf::from(dir);
    let manifest = PackManifest::read(base.join("manifest.json"))?;
    let settings = engine.resolve_pack(&manifest, &base);
    Ok((manifest, settings))
}
