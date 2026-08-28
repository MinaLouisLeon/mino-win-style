use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::os::OsBuild;
use crate::value::Value;

/// A set of settings the user wants. Nothing more: no scripts, no key paths, no
/// executables. A pack is data, and this is the only shape it can take.
pub type Settings = BTreeMap<String, Value>;

/// The manifest at the root of a `.minostyle` pack.
///
/// M2 adds the zip container around this; for now a pack is a folder holding a
/// `manifest.json` and its assets, which is the same thing without the zip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackManifest {
    /// Bumped when the format changes in a way older builds cannot read.
    pub schema: u32,
    pub id: String,
    /// Locale code -> display name, e.g. `{"en": "Midnight Cairo", "ar": "..."}`.
    pub name: BTreeMap<String, String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: BTreeMap<String, String>,
    #[serde(default)]
    pub requires: Requires,
    pub settings: Settings,
}

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Requires {
    #[serde(default)]
    pub min_build: Option<u32>,
    #[serde(default)]
    pub max_build: Option<u32>,
}

impl Requires {
    pub fn allows(&self, os: &OsBuild) -> bool {
        self.min_build.map_or(true, |min| os.build >= min)
            && self.max_build.map_or(true, |max| os.build <= max)
    }
}

impl PackManifest {
    pub fn from_json(text: &str) -> Result<Self> {
        let manifest: PackManifest = serde_json::from_str(text)?;
        if manifest.schema > SCHEMA_VERSION {
            return Err(Error::Journal(format!(
                "this pack needs a newer version of the app (schema {} > {SCHEMA_VERSION})",
                manifest.schema
            )));
        }
        Ok(manifest)
    }

    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path.as_ref())?;
        Self::from_json(&text)
    }

    /// Falls back to English, then to the id, so a missing translation shows
    /// something usable rather than an empty row.
    pub fn display_name(&self, locale: &str) -> String {
        self.name
            .get(locale)
            .or_else(|| self.name.get("en"))
            .cloned()
            .unwrap_or_else(|| self.id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Color;

    // Doubled hashes: the sample contains `"#0F62C0"`, and `"#` would close a
    // plain `r#"…"#` string.
    const SAMPLE: &str = r##"{
      "schema": 1,
      "id": "com.mino.test",
      "name": { "en": "Test", "ar": "تجربة" },
      "requires": { "min_build": 22621 },
      "settings": {
        "appearance.dark_mode": true,
        "appearance.accent_color": "#0F62C0",
        "taskbar.alignment": "left"
      }
    }"##;

    #[test]
    fn reads_natural_json() {
        let pack = PackManifest::from_json(SAMPLE).unwrap();
        assert_eq!(pack.display_name("ar"), "تجربة");
        assert_eq!(pack.settings["appearance.dark_mode"], Value::Bool(true));
        assert_eq!(
            pack.settings["appearance.accent_color"],
            Value::Color(Color::new(0x0F, 0x62, 0xC0))
        );
        assert_eq!(
            pack.settings["taskbar.alignment"],
            Value::Str("left".into())
        );
    }

    #[test]
    fn round_trips() {
        let pack = PackManifest::from_json(SAMPLE).unwrap();
        let text = serde_json::to_string(&pack).unwrap();
        let again = PackManifest::from_json(&text).unwrap();
        assert_eq!(again.settings, pack.settings);
    }

    #[test]
    fn build_requirements_are_checked() {
        let pack = PackManifest::from_json(SAMPLE).unwrap();
        assert!(pack.requires.allows(&OsBuild::fake(26200)));
        assert!(!pack.requires.allows(&OsBuild::fake(22000)));
    }

    #[test]
    fn a_pack_from_the_future_is_refused() {
        let text = SAMPLE.replace("\"schema\": 1", "\"schema\": 99");
        assert!(PackManifest::from_json(&text).is_err());
    }
}
