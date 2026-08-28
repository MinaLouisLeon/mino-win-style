//! The two shapes almost every Windows appearance setting takes: a DWORD that
//! means on/off, and a DWORD that means "one of these". Writing them once keeps
//! the individual tweak definitions to a single readable literal each.

use crate::error::{Error, Result};
use crate::os::{BuildRange, OsBuild, Support};
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
    pub note: Option<&'static str>,
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

    pub const fn note(mut self, note: &'static str) -> Self {
        self.note = Some(note);
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
            (false, _) if os.build < self.builds.min => {
                Support::Unsupported("Needs a newer Windows 11 build.")
            }
            (false, _) => Support::Unsupported("Windows changed this setting in a later build."),
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
        let loc = self.spec.loc();
        let from = reg.read(&loc)?;
        let to = RegValue::Dword(if want { self.on } else { self.off });
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

/// A DWORD mapped to a small set of named options.
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
    pub note: Option<&'static str>,
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
        }
    }

    pub const fn tier(mut self, tier: Tier) -> Self {
        self.tier = tier;
        self
    }

    pub const fn builds(mut self, builds: BuildRange) -> Self {
        self.builds = builds;
        self
    }

    pub const fn note(mut self, note: &'static str) -> Self {
        self.note = Some(note);
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
            choices: self.map.iter().map(|(name, _)| (*name).to_string()).collect(),
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
            (false, _) if os.build < self.builds.min => {
                Support::Unsupported("Needs a newer Windows 11 build.")
            }
            (false, _) => Support::Unsupported("Windows changed this setting in a later build."),
        }
    }

    fn read(&self, reg: &dyn RegistryProvider) -> Result<Value> {
        let loc = self.spec.loc();
        let choice = match reg.read(&loc)? {
            None => self.default,
            Some(RegValue::Dword(d)) => self.choice_for(d).ok_or_else(|| Error::UnexpectedState {
                loc: loc.to_string(),
                detail: format!("{d} is not one of the values this app knows"),
            })?,
            Some(other) => {
                return Err(Error::UnexpectedState {
                    loc: loc.to_string(),
                    detail: format!("expected REG_DWORD, found {}", other.type_name()),
                })
            }
        };
        Ok(Value::Choice(choice.to_string()))
    }

    fn plan(&self, reg: &dyn RegistryProvider, want: &Value) -> Result<ChangeSet> {
        let choice = want.as_choice(self.id)?;
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
        let to = RegValue::Dword(number);
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
