use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hive {
    CurrentUser,
    LocalMachine,
    ClassesRoot,
}

impl Hive {
    pub fn short(self) -> &'static str {
        match self {
            Hive::CurrentUser => "HKCU",
            Hive::LocalMachine => "HKLM",
            Hive::ClassesRoot => "HKCR",
        }
    }

    /// The form `.reg` files use.
    pub fn long(self) -> &'static str {
        match self {
            Hive::CurrentUser => "HKEY_CURRENT_USER",
            Hive::LocalMachine => "HKEY_LOCAL_MACHINE",
            Hive::ClassesRoot => "HKEY_CLASSES_ROOT",
        }
    }
}

/// A compile-time registry location. Built-in tweaks use this so their key paths
/// are visible in one glance and cannot be built from user input.
#[derive(Debug, Clone, Copy)]
pub struct RegSpec {
    pub hive: Hive,
    pub path: &'static str,
    pub name: &'static str,
}

impl RegSpec {
    pub const fn hkcu(path: &'static str, name: &'static str) -> Self {
        RegSpec {
            hive: Hive::CurrentUser,
            path,
            name,
        }
    }

    pub fn loc(&self) -> RegLoc {
        RegLoc {
            hive: self.hive,
            path: self.path.to_string(),
            name: self.name.to_string(),
        }
    }
}

/// The owned form, used in plans and journal entries.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RegLoc {
    pub hive: Hive,
    pub path: String,
    /// Empty string means the key's unnamed default value.
    pub name: String,
}

impl std::fmt::Display for RegLoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = if self.name.is_empty() {
            "(Default)"
        } else {
            &self.name
        };
        write!(f, "{}\\{}\\\\{}", self.hive.short(), self.path, name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RegValue {
    Dword(u32),
    Sz(String),
    ExpandSz(String),
    Binary(Vec<u8>),
}

impl RegValue {
    pub fn as_dword(&self) -> Option<u32> {
        match self {
            RegValue::Dword(d) => Some(*d),
            _ => None,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            RegValue::Dword(_) => "REG_DWORD",
            RegValue::Sz(_) => "REG_SZ",
            RegValue::ExpandSz(_) => "REG_EXPAND_SZ",
            RegValue::Binary(_) => "REG_BINARY",
        }
    }
}

/// Everything the engine is allowed to do to the registry. Implemented for real
/// in `mino-win`, and faked by [`MemoryRegistry`] in tests.
pub trait RegistryProvider: Send + Sync {
    fn read(&self, loc: &RegLoc) -> Result<Option<RegValue>>;
    fn write(&self, loc: &RegLoc, value: &RegValue) -> Result<()>;
    fn delete_value(&self, loc: &RegLoc) -> Result<()>;
    fn key_exists(&self, hive: Hive, path: &str) -> Result<bool>;
    fn create_key(&self, hive: Hive, path: &str) -> Result<()>;
    /// Deletes the key and everything under it.
    fn delete_key(&self, hive: Hive, path: &str) -> Result<()>;
}

/// How a change is made visible without signing out. Most settings only need a
/// broadcast; restarting Explorer is a last resort and always user-approved.
pub trait ShellRefresher: Send + Sync {
    fn broadcast_setting_change(&self, area: &str) -> Result<()>;
    fn notify_assoc_changed(&self) -> Result<()>;
    fn refresh_cursors(&self) -> Result<()>;
    /// Writing `Control Panel\Desktop\WallPaper` records the choice; the desktop
    /// only repaints once this is called. The engine reads the path back out of
    /// the registry after applying, so the two can never disagree.
    fn apply_wallpaper(&self, path: &str) -> Result<()>;
    fn restart_explorer(&self) -> Result<()>;
}

// ---------------------------------------------------------------------------
// Fakes — used by the test suite and by `mino-cli --dry-run`.
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MemoryRegistry {
    values: Mutex<BTreeMap<String, RegValue>>,
    keys: Mutex<BTreeMap<String, ()>>,
}

impl MemoryRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn value_key(loc: &RegLoc) -> String {
        format!(
            "{}|{}|{}",
            loc.hive.short(),
            loc.path.to_lowercase(),
            loc.name.to_lowercase()
        )
    }

    fn key_key(hive: Hive, path: &str) -> String {
        format!("{}|{}", hive.short(), path.to_lowercase())
    }

    /// Seed a value, so a test can describe the machine it is pretending to be.
    pub fn seed(&self, loc: &RegLoc, value: RegValue) {
        self.keys
            .lock()
            .unwrap()
            .insert(Self::key_key(loc.hive, &loc.path), ());
        self.values
            .lock()
            .unwrap()
            .insert(Self::value_key(loc), value);
    }

    /// Everything currently stored, for asserting that a revert was byte-exact.
    pub fn snapshot(&self) -> BTreeMap<String, RegValue> {
        self.values.lock().unwrap().clone()
    }
}

impl RegistryProvider for MemoryRegistry {
    fn read(&self, loc: &RegLoc) -> Result<Option<RegValue>> {
        Ok(self
            .values
            .lock()
            .unwrap()
            .get(&Self::value_key(loc))
            .cloned())
    }

    fn write(&self, loc: &RegLoc, value: &RegValue) -> Result<()> {
        self.keys
            .lock()
            .unwrap()
            .insert(Self::key_key(loc.hive, &loc.path), ());
        self.values
            .lock()
            .unwrap()
            .insert(Self::value_key(loc), value.clone());
        Ok(())
    }

    fn delete_value(&self, loc: &RegLoc) -> Result<()> {
        self.values.lock().unwrap().remove(&Self::value_key(loc));
        Ok(())
    }

    fn key_exists(&self, hive: Hive, path: &str) -> Result<bool> {
        Ok(self
            .keys
            .lock()
            .unwrap()
            .contains_key(&Self::key_key(hive, path)))
    }

    fn create_key(&self, hive: Hive, path: &str) -> Result<()> {
        self.keys
            .lock()
            .unwrap()
            .insert(Self::key_key(hive, path), ());
        Ok(())
    }

    fn delete_key(&self, hive: Hive, path: &str) -> Result<()> {
        // Deleting a key takes its subkeys and their values with it, the same
        // way `RegDeleteTree` does — otherwise the fake would let a revert look
        // clean while the real thing left values behind.
        let target = path.to_lowercase();
        let covered =
            |candidate: &str| candidate == target || candidate.starts_with(&format!("{target}\\"));

        let hive_prefix = format!("{}|", hive.short());
        self.keys.lock().unwrap().retain(|k, _| {
            k.strip_prefix(&hive_prefix)
                .map_or(true, |rest| !covered(rest))
        });
        self.values.lock().unwrap().retain(|k, _| {
            match k
                .strip_prefix(&hive_prefix)
                .and_then(|rest| rest.rsplit_once('|'))
            {
                Some((key_path, _name)) => !covered(key_path),
                None => true,
            }
        });
        Ok(())
    }
}

/// Records what would have been refreshed, without touching the shell.
#[derive(Default)]
pub struct NoopRefresher {
    pub calls: Mutex<Vec<String>>,
}

impl NoopRefresher {
    pub fn new() -> Self {
        Self::default()
    }

    fn record(&self, what: &str) -> Result<()> {
        self.calls.lock().unwrap().push(what.to_string());
        Ok(())
    }
}

impl ShellRefresher for NoopRefresher {
    fn broadcast_setting_change(&self, area: &str) -> Result<()> {
        self.record(&format!("broadcast:{area}"))
    }
    fn notify_assoc_changed(&self) -> Result<()> {
        self.record("assoc_changed")
    }
    fn refresh_cursors(&self) -> Result<()> {
        self.record("cursors")
    }
    fn apply_wallpaper(&self, path: &str) -> Result<()> {
        self.record(&format!("wallpaper:{path}"))
    }
    fn restart_explorer(&self) -> Result<()> {
        self.record("restart_explorer")
    }
}
