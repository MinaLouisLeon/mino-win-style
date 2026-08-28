use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::os::{BuildRange, OsBuild, Support};
use crate::provider::{Hive, RegLoc, RegValue, RegistryProvider};
use crate::value::{Value, ValueKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Appearance,
    Taskbar,
    Start,
    Explorer,
}

impl Category {
    pub const ALL: [Category; 4] = [
        Category::Appearance,
        Category::Taskbar,
        Category::Start,
        Category::Explorer,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Category::Appearance => "appearance",
            Category::Taskbar => "taskbar",
            Category::Start => "start",
            Category::Explorer => "explorer",
        }
    }
}

/// See the project plan, section 02. The tier is part of the tweak definition so
/// that "is this safe?" is answered by the code, not by whoever reviews the PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Documented API or per-user registry value. No admin, survives updates.
    A,
    /// Undocumented or admin-level. Works today, gated by build number, shown
    /// with a warning.
    B,
    /// Binary patching or injection. Never implemented in this binary — the
    /// variant exists so a plugin can declare itself honestly.
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Privilege {
    User,
    Elevated,
}

/// What has to happen for a change to become visible.
///
/// Serialize only: the `&'static str` area cannot be deserialised, and nothing
/// needs to read a `Refresh` back — the journal stores [`Change`]s, not these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "area", rename_all = "snake_case")]
pub enum Refresh {
    None,
    /// `WM_SETTINGCHANGE` with the given area string, e.g. "ImmersiveColorSet".
    Broadcast(&'static str),
    /// `SHChangeNotify(SHCNE_ASSOCCHANGED)` — shell icons and context menus.
    AssocChanged,
    Cursors,
    /// Explorer must be restarted. Always confirmed by the user first.
    RestartShell,
    /// Nothing we can do from here; the user must sign out and back in.
    SignOut,
}

impl Refresh {
    /// Ranked so a batch can pick the single strongest refresh it needs.
    pub fn weight(self) -> u8 {
        match self {
            Refresh::None => 0,
            Refresh::Broadcast(_) => 1,
            Refresh::AssocChanged => 2,
            Refresh::Cursors => 2,
            Refresh::RestartShell => 3,
            Refresh::SignOut => 4,
        }
    }
}

/// One reversible operation. `from` is captured at plan time and is what the
/// journal replays to undo the change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Change {
    Value {
        loc: RegLoc,
        from: Option<RegValue>,
        to: Option<RegValue>,
    },
    /// Creating or removing a key itself, for tweaks like the classic context
    /// menu where the *presence* of the key is the setting.
    Key {
        hive: Hive,
        path: String,
        from_present: bool,
        to_present: bool,
    },
}

impl Change {
    /// The same operation in reverse. Undo is just apply, backwards.
    pub fn inverted(&self) -> Change {
        match self {
            Change::Value { loc, from, to } => Change::Value {
                loc: loc.clone(),
                from: to.clone(),
                to: from.clone(),
            },
            Change::Key {
                hive,
                path,
                from_present,
                to_present,
            } => Change::Key {
                hive: *hive,
                path: path.clone(),
                from_present: *to_present,
                to_present: *from_present,
            },
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Change::Value { loc, from, to } => {
                let show = |v: &Option<RegValue>| match v {
                    None => "(not set)".to_string(),
                    Some(RegValue::Dword(d)) => format!("{d}"),
                    Some(RegValue::Sz(s)) | Some(RegValue::ExpandSz(s)) => format!("\"{s}\""),
                    Some(RegValue::Binary(b)) => format!("{} bytes", b.len()),
                };
                format!("{loc}: {} -> {}", show(from), show(to))
            }
            Change::Key {
                hive,
                path,
                from_present,
                to_present,
            } => format!(
                "{}\\{}: {} -> {}",
                hive.short(),
                path,
                if *from_present { "exists" } else { "absent" },
                if *to_present { "exists" } else { "absent" }
            ),
        }
    }
}

/// Everything one tweak wants to change, plus how to make it visible.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChangeSet {
    pub tweak: String,
    pub changes: Vec<Change>,
    pub refresh: Refresh,
}

impl ChangeSet {
    pub fn nothing(tweak: &str) -> Self {
        ChangeSet {
            tweak: tweak.to_string(),
            changes: Vec::new(),
            refresh: Refresh::None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// A single setting.
///
/// The split that matters: a tweak can read the world and *describe* a change,
/// but it can never perform one. Applying happens once, centrally, in
/// [`crate::engine::Engine`] — which is why journalling, ordering and rollback
/// are written once instead of once per setting.
pub trait Tweak: Send + Sync {
    fn id(&self) -> &'static str;
    fn category(&self) -> Category;
    fn tier(&self) -> Tier;
    fn value_kind(&self) -> ValueKind;
    fn builds(&self) -> BuildRange;
    fn refresh(&self) -> Refresh;

    fn privilege(&self) -> Privilege {
        Privilege::User
    }

    /// Overridable for tweaks whose caveat is more subtle than a build range.
    fn support(&self, os: &OsBuild) -> Support {
        if self.builds().contains(os.build) {
            Support::Full
        } else if os.build < self.builds().min {
            Support::Unsupported("Needs a newer Windows 11 build.")
        } else {
            Support::Unsupported("Windows removed or changed this setting in a later build.")
        }
    }

    fn read(&self, reg: &dyn RegistryProvider) -> Result<Value>;

    /// Pure: reads the current state and returns what *would* change. Must not write.
    fn plan(&self, reg: &dyn RegistryProvider, want: &Value) -> Result<ChangeSet>;
}

/// The serialisable description the UI renders. Built from a `Tweak` plus the
/// current OS, so support notes are resolved once on the Rust side.
#[derive(Debug, Clone, Serialize)]
pub struct TweakInfo {
    pub id: &'static str,
    pub category: Category,
    pub tier: Tier,
    pub kind: ValueKind,
    pub privilege: Privilege,
    pub refresh: Refresh,
    pub support: Support,
}

/// A tweak plus its value on this machine right now.
#[derive(Debug, Clone, Serialize)]
pub struct TweakState {
    #[serde(flatten)]
    pub info: TweakInfo,
    pub value: Option<Value>,
    /// Set when reading failed — the UI shows the row disabled with this text
    /// rather than pretending the setting is off.
    pub error: Option<String>,
}
