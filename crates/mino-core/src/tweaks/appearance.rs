use crate::error::Result;
use crate::os::BuildRange;
use crate::provider::{Hive, RegLoc, RegSpec, RegValue, RegistryProvider};
use crate::tweak::{Category, Change, ChangeSet, Privilege, Refresh, Tier, Tweak};
use crate::tweaks::helpers::BoolTweak;
use crate::value::{Color, Value, ValueKind};

const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const DWM: &str = r"Software\Microsoft\Windows\DWM";
const ACCENT: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent";
const DESKTOP: &str = r"Control Panel\Desktop";

/// Windows' own default accent, used when nothing has been set yet.
const DEFAULT_ACCENT: Color = Color::new(0x00, 0x78, 0xD4);

pub fn all() -> Vec<Box<dyn Tweak>> {
    vec![
        Box::new(DarkModeTweak),
        Box::new(AccentColorTweak),
        Box::new(
            BoolTweak::new(
                "appearance.transparency",
                Category::Appearance,
                RegSpec::hkcu(PERSONALIZE, "EnableTransparency"),
                Refresh::Broadcast("ImmersiveColorSet"),
            )
            .default_on(true),
        ),
        Box::new(BoolTweak::new(
            "appearance.accent_on_titlebars",
            Category::Appearance,
            RegSpec::hkcu(DWM, "ColorPrevalence"),
            Refresh::Broadcast("ImmersiveColorSet"),
        )),
        Box::new(
            BoolTweak::new(
                "appearance.accent_on_start_taskbar",
                Category::Appearance,
                RegSpec::hkcu(PERSONALIZE, "ColorPrevalence"),
                Refresh::Broadcast("ImmersiveColorSet"),
            )
            .note("Windows only shows this while dark mode is on."),
        ),
    ]
}

/// Light and dark mode.
///
/// Windows keeps two switches — one for apps, one for the shell — and offers
/// mixed states in Settings. We treat them as one control because that is what
/// people mean by "dark mode"; both are written together and both are journalled,
/// so a mixed setup is restored exactly as it was.
pub struct DarkModeTweak;

impl DarkModeTweak {
    const APPS: RegSpec = RegSpec::hkcu(PERSONALIZE, "AppsUseLightTheme");
    const SYSTEM: RegSpec = RegSpec::hkcu(PERSONALIZE, "SystemUsesLightTheme");
}

impl Tweak for DarkModeTweak {
    fn id(&self) -> &'static str {
        "appearance.dark_mode"
    }
    fn category(&self) -> Category {
        Category::Appearance
    }
    fn tier(&self) -> Tier {
        Tier::A
    }
    fn value_kind(&self) -> ValueKind {
        ValueKind::Bool
    }
    fn builds(&self) -> BuildRange {
        BuildRange::any()
    }
    fn refresh(&self) -> Refresh {
        Refresh::Broadcast("ImmersiveColorSet")
    }

    fn read(&self, reg: &dyn RegistryProvider) -> Result<Value> {
        // Absent means light: that is Windows' own default on a fresh install.
        let apps_light = reg
            .read(&Self::APPS.loc())?
            .and_then(|v| v.as_dword())
            .unwrap_or(1);
        Ok(Value::Bool(apps_light == 0))
    }

    fn plan(&self, reg: &dyn RegistryProvider, want: &Value) -> Result<ChangeSet> {
        let dark = want.as_bool(self.id())?;
        let target = RegValue::Dword(if dark { 0 } else { 1 });

        let mut changes = Vec::new();
        for spec in [Self::APPS, Self::SYSTEM] {
            let loc = spec.loc();
            let from = reg.read(&loc)?;
            if from.as_ref() == Some(&target) {
                continue;
            }
            changes.push(Change::Value {
                loc,
                from,
                to: Some(target.clone()),
            });
        }

        Ok(ChangeSet {
            tweak: self.id().to_string(),
            changes,
            refresh: self.refresh(),
        })
    }
}

/// The accent colour.
///
/// One user-visible colour, six registry values: DWM keeps the colour the window
/// manager draws with, Explorer keeps a shade ramp that Start and the taskbar
/// read, and `AutoColorization` has to be turned off or Windows overwrites the
/// lot the next time the wallpaper changes.
pub struct AccentColorTweak;

impl AccentColorTweak {
    const ACCENT_COLOR: RegSpec = RegSpec::hkcu(DWM, "AccentColor");
    const COLORIZATION: RegSpec = RegSpec::hkcu(DWM, "ColorizationColor");
    const AFTERGLOW: RegSpec = RegSpec::hkcu(DWM, "ColorizationAfterglow");
    const MENU: RegSpec = RegSpec::hkcu(ACCENT, "AccentColorMenu");
    const START: RegSpec = RegSpec::hkcu(ACCENT, "StartColorMenu");
    const PALETTE: RegSpec = RegSpec::hkcu(ACCENT, "AccentPalette");
    const AUTO: RegSpec = RegSpec::hkcu(DESKTOP, "AutoColorization");

    /// The eight-shade ramp Windows stores in `AccentPalette`, lightest first.
    ///
    /// VERIFY ON HARDWARE (M1, VM matrix): the layout below — eight 4-byte
    /// entries of `R, G, B, 00` with the base accent at index 4 — matches what
    /// Windows writes on the machines this was developed against, but it is not
    /// a documented format. The compatibility test for this tweak is to set an
    /// accent through Settings, dump the value, and diff it against this
    /// function's output.
    fn palette(base: Color) -> Vec<u8> {
        const SHADES: [f32; 8] = [0.8, 0.6, 0.4, 0.2, 0.0, -0.2, -0.4, -0.6];
        let mut bytes = Vec::with_capacity(32);
        for shade in SHADES {
            let c = base.shade(shade);
            bytes.extend_from_slice(&[c.r, c.g, c.b, 0x00]);
        }
        bytes
    }

    /// `ColorizationColor` carries an alpha byte; Windows itself writes 0xC4.
    fn colorization(base: Color) -> u32 {
        0xC400_0000
            | (u32::from(base.r) << 16)
            | (u32::from(base.g) << 8)
            | u32::from(base.b)
    }

    fn set(
        reg: &dyn RegistryProvider,
        changes: &mut Vec<Change>,
        loc: RegLoc,
        to: RegValue,
    ) -> Result<()> {
        let from = reg.read(&loc)?;
        if from.as_ref() == Some(&to) {
            return Ok(());
        }
        changes.push(Change::Value {
            loc,
            from,
            to: Some(to),
        });
        Ok(())
    }
}

impl Tweak for AccentColorTweak {
    fn id(&self) -> &'static str {
        "appearance.accent_color"
    }
    fn category(&self) -> Category {
        Category::Appearance
    }
    /// Tier B: `AccentPalette` is not a documented format.
    fn tier(&self) -> Tier {
        Tier::B
    }
    fn value_kind(&self) -> ValueKind {
        ValueKind::Color
    }
    fn builds(&self) -> BuildRange {
        BuildRange::any()
    }
    fn refresh(&self) -> Refresh {
        Refresh::Broadcast("ImmersiveColorSet")
    }
    fn privilege(&self) -> Privilege {
        Privilege::User
    }

    fn read(&self, reg: &dyn RegistryProvider) -> Result<Value> {
        let color = reg
            .read(&Self::ACCENT_COLOR.loc())?
            .and_then(|v| v.as_dword())
            .map(Color::from_abgr_dword)
            .unwrap_or(DEFAULT_ACCENT);
        Ok(Value::Color(color))
    }

    fn plan(&self, reg: &dyn RegistryProvider, want: &Value) -> Result<ChangeSet> {
        let base = want.as_color(self.id())?;
        let darker = base.shade(-0.2);
        let mut changes = Vec::new();

        Self::set(
            reg,
            &mut changes,
            Self::ACCENT_COLOR.loc(),
            RegValue::Dword(base.to_abgr_dword()),
        )?;
        Self::set(
            reg,
            &mut changes,
            Self::COLORIZATION.loc(),
            RegValue::Dword(Self::colorization(base)),
        )?;
        Self::set(
            reg,
            &mut changes,
            Self::AFTERGLOW.loc(),
            RegValue::Dword(Self::colorization(base)),
        )?;
        Self::set(
            reg,
            &mut changes,
            Self::MENU.loc(),
            RegValue::Dword(base.to_abgr_dword()),
        )?;
        Self::set(
            reg,
            &mut changes,
            Self::START.loc(),
            RegValue::Dword(darker.to_abgr_dword()),
        )?;
        Self::set(
            reg,
            &mut changes,
            Self::PALETTE.loc(),
            RegValue::Binary(Self::palette(base)),
        )?;
        // Without this, Windows recolours everything from the wallpaper again.
        Self::set(reg, &mut changes, Self::AUTO.loc(), RegValue::Dword(0))?;

        Ok(ChangeSet {
            tweak: self.id().to_string(),
            changes,
            refresh: self.refresh(),
        })
    }
}

/// `HKCU\Control Panel\Desktop` is not under `Software`, so it gets its own
/// helper rather than being mistyped somewhere.
pub const fn desktop_spec(name: &'static str) -> RegSpec {
    RegSpec {
        hive: Hive::CurrentUser,
        path: DESKTOP,
        name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::MemoryRegistry;

    #[test]
    fn palette_is_thirty_two_bytes_with_base_in_the_middle() {
        let base = Color::new(0x0F, 0x62, 0xC0);
        let bytes = AccentColorTweak::palette(base);
        assert_eq!(bytes.len(), 32);
        assert_eq!(&bytes[16..20], &[base.r, base.g, base.b, 0x00]);
        // Lightest first, darkest last.
        assert!(bytes[0] > bytes[28]);
    }

    #[test]
    fn dark_mode_defaults_to_light_when_unset() {
        let reg = MemoryRegistry::new();
        assert_eq!(DarkModeTweak.read(&reg).unwrap(), Value::Bool(false));
    }

    #[test]
    fn dark_mode_writes_both_switches() {
        let reg = MemoryRegistry::new();
        let set = DarkModeTweak.plan(&reg, &Value::Bool(true)).unwrap();
        assert_eq!(set.changes.len(), 2);
    }

    #[test]
    fn accent_plan_is_empty_when_already_applied() {
        let reg = MemoryRegistry::new();
        let want = Value::Color(Color::new(0x0F, 0x62, 0xC0));
        for change in AccentColorTweak.plan(&reg, &want).unwrap().changes {
            if let Change::Value { loc, to: Some(v), .. } = change {
                reg.seed(&loc, v);
            }
        }
        assert!(AccentColorTweak.plan(&reg, &want).unwrap().is_empty());
    }
}
