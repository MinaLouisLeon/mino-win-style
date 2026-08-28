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
            .note(
                "dark_mode_only",
                "Windows only shows this while dark mode is on.",
            ),
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

    /// Lightness offsets for the seven accent shades, lightest first, with the
    /// base at index 3.
    ///
    /// Fitted against what Windows 11 25H2 actually stores for its default blue
    /// (`#0078D4`): the ramp there is `#99EBFF #4CC2FF #0091F8 #0078D4 #0067C0
    /// #003E92 #001A68`, whose HSL lightnesses sit at these offsets from the
    /// base. Microsoft also drifts the hue slightly across the ramp; we do not,
    /// so our shades are close rather than identical. They are our own colours
    /// for a colour the user chose, which is the part that matters.
    const RAMP: [f32; 7] = [0.384, 0.233, 0.070, 0.0, -0.040, -0.130, -0.212];

    /// Windows measures `StartColorMenu` at `#005A9E` for that same base — a
    /// touch darker than Dark1 and lighter than Dark2.
    const START_OFFSET: f32 = -0.106;

    /// Eight 4-byte entries of `R, G, B, 00` (byte order verified against a live
    /// machine). The last entry is *not* part of the ramp — on a default install
    /// it holds an unrelated colour, `#F7630C` — so an existing one is carried
    /// over untouched rather than overwritten with a guess.
    fn palette(base: Color, existing: Option<&RegValue>) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32);
        for offset in Self::RAMP {
            let c = base.lighten(offset);
            bytes.extend_from_slice(&[c.r, c.g, c.b, 0x00]);
        }
        match existing {
            Some(RegValue::Binary(previous)) if previous.len() == 32 => {
                bytes.extend_from_slice(&previous[28..32]);
            }
            _ => bytes.extend_from_slice(&[0xF7, 0x63, 0x0C, 0x00]),
        }
        bytes
    }

    /// `ColorizationColor` carries an alpha byte; Windows itself writes 0xC4.
    fn colorization(base: Color) -> u32 {
        0xC400_0000 | (u32::from(base.r) << 16) | (u32::from(base.g) << 8) | u32::from(base.b)
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
        let darker = base.lighten(Self::START_OFFSET);
        let existing_palette = reg.read(&Self::PALETTE.loc())?;
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
            RegValue::Binary(Self::palette(base, existing_palette.as_ref())),
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
    fn palette_puts_the_base_at_index_three() {
        let base = Color::new(0x0F, 0x62, 0xC0);
        let bytes = AccentColorTweak::palette(base, None);
        assert_eq!(bytes.len(), 32);
        assert_eq!(&bytes[12..16], &[base.r, base.g, base.b, 0x00]);
    }

    #[test]
    fn palette_runs_light_to_dark() {
        let bytes = AccentColorTweak::palette(Color::new(0x00, 0x78, 0xD4), None);
        let lightness = |i: usize| {
            Color::new(bytes[i * 4], bytes[i * 4 + 1], bytes[i * 4 + 2])
                .to_hsl()
                .2
        };
        for i in 0..6 {
            assert!(
                lightness(i) > lightness(i + 1),
                "entry {i} is not lighter than {}",
                i + 1
            );
        }
    }

    /// Our ramp should land near what Windows itself writes. Not identical —
    /// Microsoft drifts the hue too — so this asserts "close", and would catch
    /// the ramp being reversed, flattened or wrongly centred.
    #[test]
    fn palette_is_close_to_the_one_windows_writes() {
        let windows_ramp = [
            Color::new(0x99, 0xEB, 0xFF),
            Color::new(0x4C, 0xC2, 0xFF),
            Color::new(0x00, 0x91, 0xF8),
            Color::new(0x00, 0x78, 0xD4),
            Color::new(0x00, 0x67, 0xC0),
            Color::new(0x00, 0x3E, 0x92),
            Color::new(0x00, 0x1A, 0x68),
        ];
        let bytes = AccentColorTweak::palette(Color::new(0x00, 0x78, 0xD4), None);
        for (i, expected) in windows_ramp.iter().enumerate() {
            let got = Color::new(bytes[i * 4], bytes[i * 4 + 1], bytes[i * 4 + 2]);
            let delta = (got.to_hsl().2 - expected.to_hsl().2).abs();
            assert!(
                delta < 0.02,
                "entry {i}: {got} vs {expected} (ΔL {delta:.3})"
            );
        }
    }

    #[test]
    fn the_eighth_palette_entry_is_preserved() {
        let previous = RegValue::Binary({
            let mut bytes = vec![0u8; 28];
            bytes.extend_from_slice(&[0x11, 0x22, 0x33, 0x00]);
            bytes
        });
        let bytes = AccentColorTweak::palette(Color::new(0x0F, 0x62, 0xC0), Some(&previous));
        assert_eq!(&bytes[28..32], &[0x11, 0x22, 0x33, 0x00]);
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
            if let Change::Value {
                loc, to: Some(v), ..
            } = change
            {
                reg.seed(&loc, v);
            }
        }
        assert!(AccentColorTweak.plan(&reg, &want).unwrap().is_empty());
    }
}
