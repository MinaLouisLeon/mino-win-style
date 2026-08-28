use crate::error::Result;
use crate::os::BuildRange;
use crate::provider::{Hive, RegSpec, RegValue, RegistryProvider};
use crate::tweak::{Category, Change, ChangeSet, Refresh, Tier, Tweak};
use crate::tweaks::helpers::{BoolTweak, ChoiceTweak};
use crate::tweaks::taskbar::ADVANCED;
use crate::value::{Value, ValueKind};

pub fn all() -> Vec<Box<dyn Tweak>> {
    vec![
        Box::new(
            // Inverted on purpose: the registry value hides extensions, the
            // control shows them. `on: 0` keeps that honest in one place.
            BoolTweak::new(
                "explorer.show_file_extensions",
                Category::Explorer,
                RegSpec::hkcu(ADVANCED, "HideFileExt"),
                Refresh::AssocChanged,
            )
            .values(0, 1),
        ),
        Box::new(
            BoolTweak::new(
                "explorer.show_hidden_files",
                Category::Explorer,
                RegSpec::hkcu(ADVANCED, "Hidden"),
                Refresh::AssocChanged,
            )
            .values(1, 2),
        ),
        Box::new(
            BoolTweak::new(
                "explorer.show_protected_os_files",
                Category::Explorer,
                RegSpec::hkcu(ADVANCED, "ShowSuperHidden"),
                Refresh::AssocChanged,
            )
            .note("Shows system files. Useful for troubleshooting, easy to break things with."),
        ),
        Box::new(ChoiceTweak::new(
            "explorer.launch_to",
            Category::Explorer,
            RegSpec::hkcu(ADVANCED, "LaunchTo"),
            &[("home", 2), ("this_pc", 1), ("downloads", 3)],
            "home",
            Refresh::AssocChanged,
        )),
        Box::new(BoolTweak::new(
            "explorer.compact_view",
            Category::Explorer,
            RegSpec::hkcu(ADVANCED, "UseCompactMode"),
            Refresh::AssocChanged,
        )),
        Box::new(ClassicContextMenuTweak),
    ]
}

/// The Windows 10 right-click menu, without the "Show more options" detour.
///
/// This one is a key, not a value: Explorer looks for a specific CLSID with an
/// empty `InprocServer32` default and falls back to the old menu when it finds
/// one. Turning it off has to remove the key again, so the plan records the key
/// *and* the value — inverting a key-creation alone would leave the value
/// behind and the revert would not be exact.
pub struct ClassicContextMenuTweak;

impl ClassicContextMenuTweak {
    const CLSID: &'static str =
        r"Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}";
    const INPROC: &'static str =
        r"Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32";
}

impl Tweak for ClassicContextMenuTweak {
    fn id(&self) -> &'static str {
        "explorer.classic_context_menu"
    }
    fn category(&self) -> Category {
        Category::Explorer
    }
    /// Tier B: undocumented, and Microsoft could stop honouring it at any point.
    fn tier(&self) -> Tier {
        Tier::B
    }
    fn value_kind(&self) -> ValueKind {
        ValueKind::Bool
    }
    fn builds(&self) -> BuildRange {
        BuildRange::any()
    }
    fn refresh(&self) -> Refresh {
        Refresh::RestartShell
    }

    fn read(&self, reg: &dyn RegistryProvider) -> Result<Value> {
        Ok(Value::Bool(reg.key_exists(Hive::CurrentUser, Self::INPROC)?))
    }

    fn plan(&self, reg: &dyn RegistryProvider, want: &Value) -> Result<ChangeSet> {
        let want = want.as_bool(self.id())?;
        let clsid_present = reg.key_exists(Hive::CurrentUser, Self::CLSID)?;
        let inproc_present = reg.key_exists(Hive::CurrentUser, Self::INPROC)?;

        if want == inproc_present {
            return Ok(ChangeSet::nothing(self.id()));
        }

        let default_value = crate::provider::RegLoc {
            hive: Hive::CurrentUser,
            path: Self::INPROC.to_string(),
            name: String::new(),
        };

        let changes = if want {
            vec![
                Change::Key {
                    hive: Hive::CurrentUser,
                    path: Self::CLSID.to_string(),
                    from_present: clsid_present,
                    to_present: true,
                },
                Change::Key {
                    hive: Hive::CurrentUser,
                    path: Self::INPROC.to_string(),
                    from_present: inproc_present,
                    to_present: true,
                },
                Change::Value {
                    loc: default_value,
                    from: None,
                    to: Some(RegValue::Sz(String::new())),
                },
            ]
        } else {
            // Reverse order, so the undo of this batch rebuilds the key before
            // it writes the value back into it.
            vec![
                Change::Value {
                    loc: default_value.clone(),
                    from: reg.read(&default_value)?,
                    to: None,
                },
                Change::Key {
                    hive: Hive::CurrentUser,
                    path: Self::INPROC.to_string(),
                    from_present: inproc_present,
                    to_present: false,
                },
                Change::Key {
                    hive: Hive::CurrentUser,
                    path: Self::CLSID.to_string(),
                    from_present: clsid_present,
                    to_present: false,
                },
            ]
        };

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
    fn enabling_creates_key_then_value() {
        let reg = MemoryRegistry::new();
        assert_eq!(
            ClassicContextMenuTweak.read(&reg).unwrap(),
            Value::Bool(false)
        );

        let set = ClassicContextMenuTweak
            .plan(&reg, &Value::Bool(true))
            .unwrap();
        assert_eq!(set.changes.len(), 3);
        assert!(matches!(set.changes[0], Change::Key { .. }));
        assert!(matches!(set.changes[2], Change::Value { .. }));
    }

    #[test]
    fn no_change_when_already_in_the_wanted_state() {
        let reg = MemoryRegistry::new();
        assert!(ClassicContextMenuTweak
            .plan(&reg, &Value::Bool(false))
            .unwrap()
            .is_empty());
    }
}
