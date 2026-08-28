use serde::{Deserialize, Serialize};

/// Windows 11 21H2. Anything below this is out of scope for 1.0 — see the
/// project plan, section 12. The gating machinery below is what makes adding
/// Windows 10 later a data change rather than a rewrite.
pub const MIN_SUPPORTED_BUILD: u32 = 22000;

/// Named builds, so tweak definitions read as intent rather than as magic numbers.
pub const WIN11_21H2: u32 = 22000;
pub const WIN11_22H2: u32 = 22621;
pub const WIN11_23H2: u32 = 22631;
pub const WIN11_24H2: u32 = 26100;
pub const WIN11_25H2: u32 = 26200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsBuild {
    /// e.g. 26200
    pub build: u32,
    /// Update Build Revision, e.g. 8106
    pub ubr: u32,
    /// e.g. "25H2"
    pub display_version: String,
    pub product_name: String,
}

impl OsBuild {
    pub fn is_supported(&self) -> bool {
        self.build >= MIN_SUPPORTED_BUILD
    }

    /// Used by tests and by the CLI's `--pretend-build` flag, which lets us
    /// exercise the compatibility matrix without a fleet of VMs.
    pub fn fake(build: u32) -> Self {
        OsBuild {
            build,
            ubr: 0,
            display_version: "test".into(),
            product_name: "Windows 11 (simulated)".into(),
        }
    }
}

impl std::fmt::Display for OsBuild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} (build {}.{})",
            self.product_name, self.display_version, self.build, self.ubr
        )
    }
}

/// Inclusive on both ends. `max: None` means "still current as far as we know".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BuildRange {
    pub min: u32,
    pub max: Option<u32>,
}

impl BuildRange {
    pub const fn from(min: u32) -> Self {
        BuildRange { min, max: None }
    }

    pub const fn between(min: u32, max: u32) -> Self {
        BuildRange {
            min,
            max: Some(max),
        }
    }

    pub const fn any() -> Self {
        BuildRange::from(MIN_SUPPORTED_BUILD)
    }

    pub fn contains(&self, build: u32) -> bool {
        build >= self.min && self.max.map_or(true, |max| build <= max)
    }
}

/// A caveat about a setting, carried as a translation key *and* an English
/// sentence.
///
/// The key is what the UI looks up (`support.note.<key>`), so Arabic wording
/// lives in the locale files where the rest of it does. The English text rides
/// along so the CLI — which is English-only and has no locale files — can print
/// something useful, and so the UI has a fallback for a note nobody has
/// translated yet. Neither side has to keep a copy of the other's strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SupportNote {
    pub key: &'static str,
    pub en: &'static str,
}

impl SupportNote {
    pub const fn new(key: &'static str, en: &'static str) -> Self {
        SupportNote { key, en }
    }
}

pub const NEEDS_NEWER_BUILD: SupportNote =
    SupportNote::new("needs_newer_build", "Needs a newer Windows 11 build.");
pub const CHANGED_IN_LATER_BUILD: SupportNote = SupportNote::new(
    "changed_in_later_build",
    "Windows changed this setting in a later build.",
);

/// Whether a tweak can be offered on the current build, and why not when it can't.
///
/// `Unsupported` is deliberately loud: a setting that silently does nothing is
/// worse than one the UI greys out with a reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "level", content = "note", rename_all = "snake_case")]
pub enum Support {
    Full,
    /// Works, but with a caveat worth showing the user.
    Partial(SupportNote),
    Unsupported(SupportNote),
}

impl Support {
    pub fn is_usable(&self) -> bool {
        !matches!(self, Support::Unsupported(_))
    }

    pub fn note(&self) -> Option<SupportNote> {
        match self {
            Support::Full => None,
            Support::Partial(note) | Support::Unsupported(note) => Some(*note),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_inclusive() {
        let r = BuildRange::between(WIN11_21H2, WIN11_22H2);
        assert!(r.contains(WIN11_21H2));
        assert!(r.contains(WIN11_22H2));
        assert!(!r.contains(WIN11_23H2));

        let open = BuildRange::from(WIN11_24H2);
        assert!(open.contains(WIN11_25H2));
        assert!(!open.contains(WIN11_23H2));
    }
}
