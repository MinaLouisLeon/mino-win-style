//! The desktop itself: wallpaper and how it is fitted.
//!
//! Windows splits this across three values that have to agree, and none of them
//! do anything until `SystemParametersInfo(SPI_SETDESKWALLPAPER)` is called —
//! hence [`Refresh::Wallpaper`]. Writing the registry and repainting are kept
//! separate so the write is still journalled and revertible like everything else.

use crate::error::{Error, Result};
use crate::os::BuildRange;
use crate::provider::{RegSpec, RegValue, RegistryProvider};
use crate::tweak::{Category, Change, ChangeSet, Refresh, Tier, Tweak};
use crate::tweaks::helpers::ChoiceTweak;
use crate::value::{Value, ValueKind};

const DESKTOP: &str = r"Control Panel\Desktop";

pub fn all() -> Vec<Box<dyn Tweak>> {
    vec![
        Box::new(WallpaperTweak),
        Box::new(
            ChoiceTweak::new(
                "desktop.wallpaper_fit",
                Category::Desktop,
                RegSpec::hkcu(DESKTOP, "WallpaperStyle"),
                // Windows' own numbering. `fill` and `fit` arrived with 7;
                // `span` needs more than one monitor to mean anything.
                &[
                    ("fill", 10),
                    ("fit", 6),
                    ("stretch", 2),
                    ("center", 0),
                    ("span", 22),
                ],
                "fill",
                Refresh::Wallpaper,
            )
            // Verified on a live machine: Windows keeps this as REG_SZ "10".
            .stored_as_text(),
        ),
    ]
}

/// The wallpaper image.
///
/// The path is validated before it is planned: pointing Windows at a file that
/// is not there paints the desktop black, and finding that out after the fact is
/// exactly the kind of surprise this app exists to avoid.
pub struct WallpaperTweak;

impl WallpaperTweak {
    const PATH: RegSpec = RegSpec::hkcu(DESKTOP, "WallPaper");
    /// `WallpaperStyle` covers the rest, but tiling has its own flag and must be
    /// switched off or Windows ignores the style entirely.
    const TILE: RegSpec = RegSpec::hkcu(DESKTOP, "TileWallpaper");

    const EXTENSIONS: [&'static str; 5] = ["jpg", "jpeg", "png", "bmp", "dib"];
}

impl Tweak for WallpaperTweak {
    fn id(&self) -> &'static str {
        "desktop.wallpaper"
    }
    fn category(&self) -> Category {
        Category::Desktop
    }
    fn tier(&self) -> Tier {
        Tier::A
    }
    fn value_kind(&self) -> ValueKind {
        ValueKind::Path {
            extensions: Self::EXTENSIONS.iter().map(|e| (*e).to_string()).collect(),
        }
    }
    fn builds(&self) -> BuildRange {
        BuildRange::any()
    }
    fn refresh(&self) -> Refresh {
        Refresh::Wallpaper
    }

    fn read(&self, reg: &dyn RegistryProvider) -> Result<Value> {
        let path = match reg.read(&Self::PATH.loc())? {
            Some(RegValue::Sz(p)) | Some(RegValue::ExpandSz(p)) => p,
            _ => String::new(),
        };
        Ok(Value::Str(path))
    }

    fn plan(&self, reg: &dyn RegistryProvider, want: &Value) -> Result<ChangeSet> {
        let path = want.as_path(self.id())?;

        if !path.is_empty() {
            let file = std::path::Path::new(path);
            if !file.is_absolute() {
                return Err(Error::BadValue {
                    tweak: self.id().to_string(),
                    got: format!("\"{path}\""),
                    // Windows resolves this at sign-in, from a directory that
                    // has nothing to do with wherever we were run from.
                    expected: "an absolute path".into(),
                });
            }
            if !file.is_file() {
                return Err(Error::BadValue {
                    tweak: self.id().to_string(),
                    got: format!("\"{path}\""),
                    expected: "a file that exists".into(),
                });
            }
            let ok = file
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| Self::EXTENSIONS.contains(&e.to_lowercase().as_str()))
                .unwrap_or(false);
            if !ok {
                return Err(Error::BadValue {
                    tweak: self.id().to_string(),
                    got: format!("\"{path}\""),
                    expected: format!("one of: {}", Self::EXTENSIONS.join(", ")),
                });
            }
        }

        let mut changes = Vec::new();

        let loc = Self::PATH.loc();
        let from = reg.read(&loc)?;
        let to = RegValue::Sz(path.to_string());
        if from.as_ref() != Some(&to) {
            changes.push(Change::Value {
                loc,
                from,
                to: Some(to),
            });
        }

        let tile = Self::TILE.loc();
        let from_tile = reg.read(&tile)?;
        let to_tile = RegValue::Sz("0".into());
        if from_tile.as_ref() != Some(&to_tile) {
            changes.push(Change::Value {
                loc: tile,
                from: from_tile,
                to: Some(to_tile),
            });
        }

        Ok(ChangeSet {
            tweak: self.id().to_string(),
            changes,
            refresh: self.refresh(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::MemoryRegistry;

    #[test]
    fn refuses_a_path_that_is_not_there() {
        let reg = MemoryRegistry::new();
        let err = WallpaperTweak
            .plan(&reg, &Value::Str(r"C:\nope\missing.jpg".into()))
            .unwrap_err();
        assert!(err.to_string().contains("a file that exists"), "{err}");
    }

    /// A relative path would be resolved by Windows at sign-in, against a
    /// directory that has nothing to do with this process.
    #[test]
    fn refuses_a_relative_path() {
        let reg = MemoryRegistry::new();
        let err = WallpaperTweak
            .plan(
                &reg,
                &Value::Str(r".\packs\macos\assets\wallpaper.png".into()),
            )
            .unwrap_err();
        assert!(err.to_string().contains("an absolute path"), "{err}");
    }

    #[test]
    fn refuses_a_file_windows_cannot_use_as_a_wallpaper() {
        let reg = MemoryRegistry::new();
        // A real file, wrong kind: this crate's own manifest.
        let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        let err = WallpaperTweak
            .plan(&reg, &Value::Str(manifest.into()))
            .unwrap_err();
        assert!(err.to_string().contains("one of:"), "{err}");
    }

    /// The bug this guards against was found on a real machine: `WallpaperStyle`
    /// is `REG_SZ "10"`, and reading it as a DWORD failed the whole row.
    #[test]
    fn the_fit_setting_reads_text_and_writes_text() {
        let fit = &all()[1];
        let loc = RegSpec::hkcu(DESKTOP, "WallpaperStyle").loc();

        let reg = MemoryRegistry::new();
        reg.seed(&loc, RegValue::Sz("10".into()));
        assert_eq!(fit.read(&reg).unwrap(), Value::Str("fill".into()));

        // A DWORD left by some other tool is still understood.
        let reg = MemoryRegistry::new();
        reg.seed(&loc, RegValue::Dword(6));
        assert_eq!(fit.read(&reg).unwrap(), Value::Str("fit".into()));

        let set = fit.plan(&reg, &Value::Str("center".into())).unwrap();
        match &set.changes[0] {
            Change::Value { to: Some(v), .. } => assert_eq!(v, &RegValue::Sz("0".into())),
            other => panic!("unexpected change: {other:?}"),
        }
    }

    #[test]
    fn clearing_the_wallpaper_is_allowed() {
        let reg = MemoryRegistry::new();
        let set = WallpaperTweak
            .plan(&reg, &Value::Str(String::new()))
            .unwrap();
        assert_eq!(set.refresh, Refresh::Wallpaper);
        assert!(!set.changes.is_empty());
    }
}
