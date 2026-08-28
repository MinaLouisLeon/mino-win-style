use crate::error::{Error, Result};
use crate::os::{BuildRange, WIN11_22H2, WIN11_24H2};
use crate::provider::{RegSpec, RegValue, RegistryProvider};
use crate::tweak::{Category, Change, ChangeSet, Refresh, Tier, Tweak};
use crate::tweaks::helpers::{BoolTweak, ChoiceTweak};
use crate::value::{Value, ValueKind};

pub(crate) const ADVANCED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const SEARCH: &str = r"Software\Microsoft\Windows\CurrentVersion\Search";

/// Explorer watches these keys directly and picks most of them up within a
/// moment, but we still broadcast so anything else listening keeps in step.
const TRAY: Refresh = Refresh::Broadcast("TraySettings");

pub fn all() -> Vec<Box<dyn Tweak>> {
    vec![
        Box::new(ChoiceTweak::new(
            "taskbar.alignment",
            Category::Taskbar,
            RegSpec::hkcu(ADVANCED, "TaskbarAl"),
            &[("left", 0), ("center", 1)],
            "center",
            TRAY,
        )),
        Box::new(
            BoolTweak::new(
                "taskbar.widgets",
                Category::Taskbar,
                RegSpec::hkcu(ADVANCED, "TaskbarDa"),
                TRAY,
            )
            .default_on(true)
            .tier(Tier::B),
        ),
        Box::new(
            BoolTweak::new(
                "taskbar.task_view",
                Category::Taskbar,
                RegSpec::hkcu(ADVANCED, "ShowTaskViewButton"),
                TRAY,
            )
            .default_on(true),
        ),
        Box::new(ChoiceTweak::new(
            "taskbar.search",
            Category::Taskbar,
            RegSpec::hkcu(SEARCH, "SearchboxTaskbarMode"),
            &[
                ("hidden", 0),
                ("icon", 1),
                ("box", 2),
                ("icon_and_label", 3),
            ],
            "box",
            TRAY,
        )),
        Box::new(
            ChoiceTweak::new(
                "taskbar.icon_size",
                Category::Taskbar,
                RegSpec::hkcu(ADVANCED, "TaskbarSi"),
                &[("small", 0), ("medium", 1), ("large", 2)],
                "medium",
                TRAY,
            )
            .tier(Tier::B)
            // Reliable up to 22H2; later builds ignore it or handle it
            // inconsistently, so it is gated rather than quietly broken.
            .builds(BuildRange::between(crate::os::WIN11_21H2, WIN11_22H2)),
        ),
        Box::new(
            BoolTweak::new(
                "taskbar.seconds_in_clock",
                Category::Taskbar,
                RegSpec::hkcu(ADVANCED, "ShowSecondsInSystemClock"),
                TRAY,
            )
            .builds(BuildRange::from(WIN11_22H2)),
        ),
        Box::new(
            BoolTweak::new(
                "taskbar.end_task",
                Category::Taskbar,
                RegSpec::hkcu(ADVANCED, "TaskbarEndTask"),
                TRAY,
            )
            .builds(BuildRange::from(WIN11_24H2))
            .note(
                "dev_end_task",
                "Mirrors the End Task switch in Settings for developers.",
            ),
        ),
        Box::new(AutoHideTweak),
    ]
}

/// Auto-hide the taskbar.
///
/// This one is not a value of its own — it is a single bit inside the blob
/// Explorer uses to remember the taskbar's position and size. We flip that bit
/// in a copy and write the copy back; the untouched bytes are carried over
/// verbatim, and the journal holds the whole original blob, so a revert restores
/// the taskbar's geometry exactly along with the flag.
pub struct AutoHideTweak;

impl AutoHideTweak {
    const SETTINGS: RegSpec = RegSpec::hkcu(
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StuckRects3",
        "Settings",
    );
    /// Byte 8 holds the taskbar's option flags; 0x01 is auto-hide.
    const FLAGS_BYTE: usize = 8;
    const AUTO_HIDE_BIT: u8 = 0x01;

    fn blob(reg: &dyn RegistryProvider) -> Result<Vec<u8>> {
        let loc = Self::SETTINGS.loc();
        match reg.read(&loc)? {
            Some(RegValue::Binary(bytes)) if bytes.len() > Self::FLAGS_BYTE => Ok(bytes),
            // Never fabricate this: inventing a taskbar layout would be worse
            // than declining to touch one.
            Some(RegValue::Binary(bytes)) => Err(Error::UnexpectedState {
                loc: loc.to_string(),
                detail: format!("only {} bytes; expected more than 8", bytes.len()),
            }),
            Some(other) => Err(Error::UnexpectedState {
                loc: loc.to_string(),
                detail: format!("expected REG_BINARY, found {}", other.type_name()),
            }),
            None => Err(Error::UnexpectedState {
                loc: loc.to_string(),
                detail: "Explorer has not written its taskbar settings yet".into(),
            }),
        }
    }
}

impl Tweak for AutoHideTweak {
    fn id(&self) -> &'static str {
        "taskbar.auto_hide"
    }
    fn category(&self) -> Category {
        Category::Taskbar
    }
    /// Tier B: an undocumented bit in an undocumented blob.
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
        let bytes = Self::blob(reg)?;
        Ok(Value::Bool(
            bytes[Self::FLAGS_BYTE] & Self::AUTO_HIDE_BIT != 0,
        ))
    }

    fn plan(&self, reg: &dyn RegistryProvider, want: &Value) -> Result<ChangeSet> {
        let want = want.as_bool(self.id())?;
        let bytes = Self::blob(reg)?;
        let is_on = bytes[Self::FLAGS_BYTE] & Self::AUTO_HIDE_BIT != 0;
        if is_on == want {
            return Ok(ChangeSet::nothing(self.id()));
        }

        let mut next = bytes.clone();
        if want {
            next[Self::FLAGS_BYTE] |= Self::AUTO_HIDE_BIT;
        } else {
            next[Self::FLAGS_BYTE] &= !Self::AUTO_HIDE_BIT;
        }

        Ok(ChangeSet {
            tweak: self.id().to_string(),
            changes: vec![Change::Value {
                loc: Self::SETTINGS.loc(),
                from: Some(RegValue::Binary(bytes)),
                to: Some(RegValue::Binary(next)),
            }],
            refresh: self.refresh(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::MemoryRegistry;

    fn machine_with(flags: u8) -> MemoryRegistry {
        let reg = MemoryRegistry::new();
        let mut blob = vec![0u8; 40];
        blob[AutoHideTweak::FLAGS_BYTE] = flags;
        blob[20] = 0xAB; // stand-in for geometry we must not disturb
        reg.seed(&AutoHideTweak::SETTINGS.loc(), RegValue::Binary(blob));
        reg
    }

    #[test]
    fn reads_the_bit() {
        assert_eq!(
            AutoHideTweak.read(&machine_with(0x7A)).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            AutoHideTweak.read(&machine_with(0x7B)).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn flips_one_bit_and_leaves_the_rest_alone() {
        let reg = machine_with(0x7A);
        let set = AutoHideTweak.plan(&reg, &Value::Bool(true)).unwrap();
        match &set.changes[0] {
            Change::Value {
                to: Some(RegValue::Binary(next)),
                from: Some(RegValue::Binary(before)),
                ..
            } => {
                assert_eq!(next[AutoHideTweak::FLAGS_BYTE], 0x7B);
                assert_eq!(next[20], 0xAB, "geometry byte was disturbed");
                assert_eq!(next.len(), before.len());
            }
            other => panic!("unexpected change: {other:?}"),
        }
    }

    #[test]
    fn refuses_to_invent_a_blob_that_is_not_there() {
        let reg = MemoryRegistry::new();
        assert!(AutoHideTweak.read(&reg).is_err());
    }
}
