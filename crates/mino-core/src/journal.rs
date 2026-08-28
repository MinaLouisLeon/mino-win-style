use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::provider::{Hive, RegValue};
use crate::tweak::Change;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Written before the first write lands. If a crash leaves an entry in this
    /// state, `--safe-restore` still has everything it needs to undo it.
    Pending,
    Applied,
    /// Something failed mid-batch and the already-applied changes were undone.
    RolledBack,
    /// The user reverted it later.
    Reverted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub when: DateTime<Utc>,
    /// What the user did, e.g. "Applied pack: Midnight Cairo" or "Dark mode".
    pub label: String,
    pub status: Status,
    /// The build this was applied on — a revert onto a different build is worth
    /// warning about.
    pub os_build: u32,
    pub tweaks: Vec<String>,
    pub changes: Vec<Change>,
}

impl JournalEntry {
    pub fn new(label: impl Into<String>, os_build: u32, tweaks: Vec<String>, changes: Vec<Change>) -> Self {
        let when = Utc::now();
        JournalEntry {
            id: when.format("%Y%m%dT%H%M%S%3f").to_string(),
            when,
            label: label.into(),
            status: Status::Pending,
            os_build,
            tweaks,
            changes,
        }
    }

    /// The changes needed to put this entry back the way it was, newest first.
    pub fn undo_changes(&self) -> Vec<Change> {
        self.changes.iter().rev().map(Change::inverted).collect()
    }
}

/// Append-only directory of what this app has done to the machine.
pub struct Journal {
    dir: PathBuf,
}

impl Journal {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Journal { dir: dir.into() }
    }

    /// `%LOCALAPPDATA%\mino-win-style\journal`, or a fallback next to the
    /// executable when the variable is missing.
    pub fn default_dir() -> PathBuf {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("mino-win-style").join("journal")
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn entry_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn backup_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.reg"))
    }

    /// Writes the entry, plus a `.reg` file holding the *previous* values as a
    /// human-readable backstop. Both land before any change is applied.
    pub fn write(&self, entry: &JournalEntry) -> Result<()> {
        fs::create_dir_all(&self.dir)?;
        let json = serde_json::to_string_pretty(entry)?;
        fs::write(self.entry_path(&entry.id), json)?;
        if entry.status == Status::Pending {
            fs::write(self.backup_path(&entry.id), reg_backup(&entry.changes))?;
        }
        Ok(())
    }

    pub fn load(&self, id: &str) -> Result<JournalEntry> {
        let path = self.entry_path(id);
        let text = fs::read_to_string(&path)
            .map_err(|e| Error::Journal(format!("cannot read {}: {e}", path.display())))?;
        Ok(serde_json::from_str(&text)?)
    }

    /// Newest first. Unreadable files are skipped rather than failing the list —
    /// one corrupt entry must not hide the rest of the history.
    pub fn list(&self) -> Result<Vec<JournalEntry>> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for item in fs::read_dir(&self.dir)? {
            let path = item?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(entry) = serde_json::from_str::<JournalEntry>(&text) {
                    entries.push(entry);
                }
            }
        }
        entries.sort_by(|a, b| b.when.cmp(&a.when));
        Ok(entries)
    }

    /// The most recent entry that actually changed something and has not been
    /// reverted — what `--safe-restore latest` targets.
    pub fn latest_revertible(&self) -> Result<Option<JournalEntry>> {
        Ok(self
            .list()?
            .into_iter()
            .find(|e| matches!(e.status, Status::Applied | Status::Pending) && !e.changes.is_empty()))
    }
}

/// Renders the previous state of `changes` as a `.reg` file.
///
/// This is the escape hatch: if every other part of this program is broken, a
/// user can still double-click the file Windows itself wrote the format for.
pub fn reg_backup(changes: &[Change]) -> String {
    let mut by_key: BTreeMap<(Hive, String), Vec<(String, Option<RegValue>)>> = BTreeMap::new();
    let mut absent_keys: Vec<(Hive, String)> = Vec::new();

    for change in changes {
        match change {
            Change::Value { loc, from, .. } => {
                by_key
                    .entry((loc.hive, loc.path.clone()))
                    .or_default()
                    .push((loc.name.clone(), from.clone()));
            }
            Change::Key {
                hive,
                path,
                from_present,
                ..
            } => {
                if !from_present {
                    absent_keys.push((*hive, path.clone()));
                }
            }
        }
    }

    let mut out = String::from("Windows Registry Editor Version 5.00\r\n\r\n");
    out.push_str("; Previous values, written by mino-win-style before applying changes.\r\n");
    out.push_str("; Import this file to put things back by hand.\r\n\r\n");

    for (hive, path) in &absent_keys {
        // A leading minus deletes the key on import.
        out.push_str(&format!("[-{}\\{}]\r\n\r\n", hive.long(), path));
    }

    for ((hive, path), values) in by_key {
        out.push_str(&format!("[{}\\{}]\r\n", hive.long(), path));
        for (name, value) in values {
            let name_part = if name.is_empty() {
                "@".to_string()
            } else {
                format!("\"{}\"", escape(&name))
            };
            match value {
                // `=-` means "delete this value" — correct for something that
                // did not exist before we touched it.
                None => out.push_str(&format!("{name_part}=-\r\n")),
                Some(RegValue::Dword(d)) => {
                    out.push_str(&format!("{name_part}=dword:{d:08x}\r\n"));
                }
                Some(RegValue::Sz(s)) => {
                    out.push_str(&format!("{name_part}=\"{}\"\r\n", escape(&s)));
                }
                Some(RegValue::ExpandSz(s)) => {
                    out.push_str(&format!("{name_part}=hex(2):{}\r\n", utf16_hex(&s)));
                }
                Some(RegValue::Binary(bytes)) => {
                    let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
                    out.push_str(&format!("{name_part}=hex:{}\r\n", hex.join(",")));
                }
            }
        }
        out.push_str("\r\n");
    }
    out
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn utf16_hex(text: &str) -> String {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    units.push(0);
    units
        .iter()
        .flat_map(|u| u.to_le_bytes())
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::RegLoc;

    fn loc(name: &str) -> RegLoc {
        RegLoc {
            hive: Hive::CurrentUser,
            path: r"Software\Test".into(),
            name: name.into(),
        }
    }

    #[test]
    fn backup_writes_previous_values_not_new_ones() {
        let changes = vec![Change::Value {
            loc: loc("HideFileExt"),
            from: Some(RegValue::Dword(1)),
            to: Some(RegValue::Dword(0)),
        }];
        let text = reg_backup(&changes);
        assert!(text.contains(r"[HKEY_CURRENT_USER\Software\Test]"));
        assert!(text.contains("\"HideFileExt\"=dword:00000001"));
        assert!(!text.contains("dword:00000000"));
    }

    #[test]
    fn values_that_did_not_exist_are_marked_for_deletion() {
        let changes = vec![Change::Value {
            loc: loc("New"),
            from: None,
            to: Some(RegValue::Dword(1)),
        }];
        assert!(reg_backup(&changes).contains("\"New\"=-"));
    }

    #[test]
    fn keys_we_created_are_marked_for_removal() {
        let changes = vec![Change::Key {
            hive: Hive::CurrentUser,
            path: r"Software\Classes\CLSID\{test}".into(),
            from_present: false,
            to_present: true,
        }];
        assert!(reg_backup(&changes).contains(r"[-HKEY_CURRENT_USER\Software\Classes\CLSID\{test}]"));
    }

    #[test]
    fn undo_is_reversed_and_inverted() {
        let entry = JournalEntry::new(
            "test",
            26200,
            vec!["a".into()],
            vec![
                Change::Value {
                    loc: loc("first"),
                    from: None,
                    to: Some(RegValue::Dword(1)),
                },
                Change::Value {
                    loc: loc("second"),
                    from: Some(RegValue::Dword(0)),
                    to: Some(RegValue::Dword(1)),
                },
            ],
        );
        let undo = entry.undo_changes();
        match &undo[0] {
            Change::Value { loc, from, to } => {
                assert_eq!(loc.name, "second");
                assert_eq!(from, &Some(RegValue::Dword(1)));
                assert_eq!(to, &Some(RegValue::Dword(0)));
            }
            _ => panic!("expected a value change"),
        }
    }
}
