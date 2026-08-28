//! The two shapes almost every Windows appearance setting takes: a DWORD that
//! means on/off, and a DWORD that means "one of these". Writing them once keeps
//! the individual tweak definitions to a single readable literal each.

use crate::error::{Error, Result};
use crate::os::{
    BuildRange, OsBuild, Support, SupportNote, CHANGED_IN_LATER_BUILD, NEEDS_NEWER_BUILD,
};
use crate::provider::{RegSpec, RegValue, RegistryProvider};
use crate::tweak::{Category, Change, ChangeSet, Privilege, Refresh, Tier, Tweak};
use crate::value::{Value, ValueKind};

/// A DWORD with two meaningful values.
///
/// `on`/`off` are the raw numbers, so inverted settings need no special case:
/// `HideFileExt` becomes `on: 0, off: 1` and the UI honestly reads
/// "show file extensions".
pub struct BoolTweak {
    pub id: &'static str,
    pub category: Category,
    pub tier: Tier,
    pub spec: RegSpec,
    pub on: u32,
    pub off: u32,
    /// What Windows does when the value is absent.
    pub default_on: bool,
    pub refresh: Refresh,
    pub builds: BuildRange,
    pub privilege: Privilege,
    /// Turns `Support::Full` into `Support::Partial`, for settings that work but
    /// come with a caveat the user should see.
    pub note: Option<SupportNote>,
}

impl BoolTweak {
    pub const fn new(
        id: &'static str,
        category: Category,
        spec: RegSpec,
        refresh: Refresh,
    ) -> Self {
        BoolTweak {
            id,
            category,
            tier: Tier::A,
            spec,
            on: 1,
            off: 0,
            default_on: false,
            refresh,
            builds: BuildRange::any(),
            privilege: Privilege::User,
            note: None,
        }
    }

    pub const fn values(mut self, on: u32, off: u32) -> Self {
        self.on = on;
        self.off = off;
        self
    }

    pub const fn default_on(mut self, default_on: bool) -> Self {
        self.default_on = default_on;
        self
    }

    pub const fn tier(mut self, tier: Tier) -> Self {
        self.tier = tier;
        self
    }

    pub const fn builds(mut self, builds: BuildRange) -> Self {
        self.builds = builds;
        self
    }

    pub const fn note(mut self, key: &'static str, en: &'static str) -> Self {
        self.note = Some(SupportNote::new(key, en));
        self
    }
}

impl Tweak for BoolTweak {
    fn id(&self) -> &'static str {
        self.id
    }
    fn category(&self) -> Category {
        self.category
    }
    fn tier(&self) -> Tier {
        self.tier
    }
    fn value_kind(&self) -> ValueKind {
        ValueKind::Bool
    }
    fn builds(&self) -> BuildRange {
        self.builds
    }
    fn refresh(&self) -> Refresh {
        self.refresh
    }
    fn privilege(&self) -> Privilege {
        self.privilege
    }

    fn support(&self, os: &OsBuild) -> Support {
        match (self.builds.contains(os.build), self.note) {
            (true, Some(note)) => Support::Partial(note),
            (true, None) => Support::Full,
            (false, _) if os.build < self.builds.min => Support::Unsupported(NEEDS_NEWER_BUILD),
            (false, _) => Support::Unsupported(CHANGED_IN_LATER_BUILD),
        }
    }

    fn read(&self, reg: &dyn RegistryProvider) -> Result<Value> {
        let loc = self.spec.loc();
        let on = match reg.read(&loc)? {
            None => self.default_on,
            Some(RegValue::Dword(d)) if d == self.on => true,
            Some(RegValue::Dword(d)) if d == self.off => false,
            Some(RegValue::Dword(d)) => {
                return Err(Error::UnexpectedState {
                    loc: loc.to_string(),
                    detail: format!("expected {} or {}, found {d}", self.on, self.off),
                })
            }
            Some(other) => {
                return Err(Error::UnexpectedState {
                    loc: loc.to_string(),
                    detail: format!("expected REG_DWORD, found {}", other.type_name()),
                })
            }
        };
        Ok(Value::Bool(on))
    }

    fn plan(&self, reg: &dyn RegistryProvider, want: &Value) -> Result<ChangeSet> {
        let want = want.as_bool(self.id)?;

        // Compare what the user sees, not what the registry holds. An absent
        // value already *means* the default, so writing that default changes
        // nothing except the journal — and a confirmation screen listing
        // "off -> off" teaches people to stop reading it.
        if self.read(reg)? == Value::Bool(want) {
            return Ok(ChangeSet::nothing(self.id));
        }

        let loc = self.spec.loc();
        let from = reg.read(&loc)?;
        let to = RegValue::Dword(if want { self.on } else { self.off });
        Ok(ChangeSet {
            tweak: self.id.to_string(),
            changes: vec![Change::Value {
                loc,
                from,
                to: Some(to),
            }],
            refresh: self.refresh,
        })
    }
}

/// How Windows spells a number in the registry.
///
/// Most of `Explorer\Advanced` uses `REG_DWORD`, but the older
/// `Control Panel\Desktop` values are `REG_SZ` holding digits — `WallpaperStyle`
/// is `"10"`, not `10`. Reading accepts either; writing uses whichever the
/// tweak declares, so we hand a value back in the shape Windows expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    Dword,
    Sz,
}

/// A number mapped to a small set of named options.
pub struct ChoiceTweak {
    pub id: &'static str,
    pub category: Category,
    pub tier: Tier,
    pub spec: RegSpec,
    /// Order here is the order the UI shows.
    pub map: &'static [(&'static str, u32)],
    pub default: &'static str,
    pub refresh: Refresh,
    pub builds: BuildRange,
    pub privilege: Privilege,
    pub note: Option<SupportNote>,
    pub stored: Stored,
}

impl ChoiceTweak {
    pub const fn new(
        id: &'static str,
        category: Category,
        spec: RegSpec,
        map: &'static [(&'static str, u32)],
        default: &'static str,
        refresh: Refresh,
    ) -> Self {
        ChoiceTweak {
            id,
            category,
            tier: Tier::A,
            spec,
            map,
            default,
            refresh,
            builds: BuildRange::any(),
            privilege: Privilege::User,
            note: None,
            stored: Stored::Dword,
        }
    }

    /// For the `Control Panel\Desktop` values, which keep their numbers as text.
    pub const fn stored_as_text(mut self) -> Self {
        self.stored = Stored::Sz;
        self
    }

    pub const fn tier(mut self, tier: Tier) -> Self {
        self.tier = tier;
        self
    }

    pub const fn builds(mut self, builds: BuildRange) -> Self {
        self.builds = builds;
        self
    }

    pub const fn note(mut self, key: &'static str, en: &'static str) -> Self {
        self.note = Some(SupportNote::new(key, en));
        self
    }

    fn number_for(&self, choice: &str) -> Option<u32> {
        self.map
            .iter()
            .find(|(name, _)| *name == choice)
            .map(|(_, n)| *n)
    }

    fn choice_for(&self, number: u32) -> Option<&'static str> {
        self.map
            .iter()
            .find(|(_, n)| *n == number)
            .map(|(name, _)| *name)
    }
}

impl Tweak for ChoiceTweak {
    fn id(&self) -> &'static str {
        self.id
    }
    fn category(&self) -> Category {
        self.category
    }
    fn tier(&self) -> Tier {
        self.tier
    }
    fn value_kind(&self) -> ValueKind {
        ValueKind::Choice {
            choices: self
                .map
                .iter()
                .map(|(name, _)| (*name).to_string())
                .collect(),
        }
    }
    fn builds(&self) -> BuildRange {
        self.builds
    }
    fn refresh(&self) -> Refresh {
        self.refresh
    }
    fn privilege(&self) -> Privilege {
        self.privilege
    }

    fn support(&self, os: &OsBuild) -> Support {
        match (self.builds.contains(os.build), self.note) {
            (true, Some(note)) => Support::Partial(note),
            (true, None) => Support::Full,
            (false, _) if os.build < self.builds.min => Support::Unsupported(NEEDS_NEWER_BUILD),
            (false, _) => Support::Unsupported(CHANGED_IN_LATER_BUILD),
        }
    }

    fn read(&self, reg: &dyn RegistryProvider) -> Result<Value> {
        let loc = self.spec.loc();
        // Liberal in what we accept: a value we declare as text may already be a
        // DWORD because some other tool wrote it that way, and vice versa.
        let number = match reg.read(&loc)? {
            None => return Ok(Value::Str(self.default.to_string())),
            Some(RegValue::Dword(d)) => d,
            Some(RegValue::Sz(text)) | Some(RegValue::ExpandSz(text)) => text
                .trim()
                .parse::<u32>()
                .map_err(|_| Error::UnexpectedState {
                    loc: loc.to_string(),
                    detail: format!("`{text}` is not a number"),
                })?,
            Some(other) => {
                return Err(Error::UnexpectedState {
                    loc: loc.to_string(),
                    detail: format!("expected a number, found {}", other.type_name()),
                })
            }
        };

        let choice = self
            .choice_for(number)
            .ok_or_else(|| Error::UnexpectedState {
                loc: loc.to_string(),
                detail: format!("{number} is not one of the values this app knows"),
            })?;
        Ok(Value::Str(choice.to_string()))
    }

    fn plan(&self, reg: &dyn RegistryProvider, want: &Value) -> Result<ChangeSet> {
        let choice = want.as_choice(self.id)?;

        // As with BoolTweak: an absent value already means its default.
        if self.read(reg)? == Value::Str(choice.to_string()) {
            return Ok(ChangeSet::nothing(self.id));
        }

        let number = self.number_for(choice).ok_or_else(|| Error::BadValue {
            tweak: self.id.to_string(),
            got: format!("\"{choice}\""),
            expected: self
                .map
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
                .join(", "),
        })?;

        let loc = self.spec.loc();
        let from = reg.read(&loc)?;
        let to = match self.stored {
            Stored::Dword => RegValue::Dword(number),
            Stored::Sz => RegValue::Sz(number.to_string()),
        };
        if from.as_ref() == Some(&to) {
            return Ok(ChangeSet::nothing(self.id));
        }
        Ok(ChangeSet {
            tweak: self.id.to_string(),
            changes: vec![Change::Value {
                loc,
                from,
                to: Some(to),
            }],
            refresh: self.refresh,
        })
    }
}
