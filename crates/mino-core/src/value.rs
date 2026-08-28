use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// The user-facing value of a tweak. This is what the UI sends and receives —
/// never a registry type. Translating between this and what the registry
/// actually stores is each tweak's job.
///
/// Untagged, so a style pack reads the way a person would write it:
/// `true`, `"#0F62C0"`, `"left"`. Order matters — `Color` is tried before
/// `Choice` and rejects anything that is not six hex digits, so `"left"` can
/// only land on `Choice`. Which of the two a tweak actually wants comes from
/// its [`ValueKind`], not from guessing at the JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Color(Color),
    /// One of the ids listed in the tweak's `ValueKind::Choice`.
    Choice(String),
}

impl Value {
    pub fn as_bool(&self, tweak: &str) -> Result<bool> {
        match self {
            Value::Bool(b) => Ok(*b),
            other => Err(Error::BadValue {
                tweak: tweak.to_string(),
                got: other.describe(),
                expected: "a boolean".into(),
            }),
        }
    }

    pub fn as_color(&self, tweak: &str) -> Result<Color> {
        match self {
            Value::Color(c) => Ok(*c),
            other => Err(Error::BadValue {
                tweak: tweak.to_string(),
                got: other.describe(),
                expected: "a colour such as \"#0F62C0\"".into(),
            }),
        }
    }

    pub fn as_choice(&self, tweak: &str) -> Result<&str> {
        match self {
            Value::Choice(c) => Ok(c.as_str()),
            other => Err(Error::BadValue {
                tweak: tweak.to_string(),
                got: other.describe(),
                expected: "one of the listed choices".into(),
            }),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Value::Bool(b) => b.to_string(),
            Value::Color(c) => c.to_hex(),
            Value::Choice(c) => format!("\"{c}\""),
        }
    }
}

/// What the UI should render for a tweak, and what values are legal.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValueKind {
    Bool,
    Color,
    /// Ids only. The UI looks up labels as `tweak.<id>.choice.<choice>` in its
    /// locale files, so Arabic and English wording never leaks into the core.
    Choice {
        choices: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let hex = text.trim().trim_start_matches('#');
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::BadValue {
                tweak: "color".into(),
                got: format!("\"{text}\""),
                expected: "six hex digits, e.g. \"#0F62C0\"".into(),
            });
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
        Ok(Color::new(byte(0), byte(2), byte(4)))
    }

    /// Windows stores accent colours as a DWORD in `0xAABBGGRR` order — the
    /// reverse of the `#RRGGBB` people write. Getting this backwards is the
    /// classic bug in every accent-colour tool, so it lives in exactly one place.
    ///
    /// The alpha byte is `FF`, not zero: measured on Windows 11 25H2, where
    /// `AccentColor` for the default blue reads `0xFFD77800`.
    pub fn to_abgr_dword(self) -> u32 {
        0xFF00_0000 | u32::from(self.r) | (u32::from(self.g) << 8) | (u32::from(self.b) << 16)
    }

    pub fn from_abgr_dword(value: u32) -> Self {
        Color::new(
            (value & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
        )
    }

    /// Hue (0–360), saturation and lightness (0–1).
    pub fn to_hsl(self) -> (f32, f32, f32) {
        let (r, g, b) = (
            f32::from(self.r) / 255.0,
            f32::from(self.g) / 255.0,
            f32::from(self.b) / 255.0,
        );
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        let d = max - min;
        if d <= f32::EPSILON {
            return (0.0, 0.0, l);
        }
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if max == r {
            (g - b) / d
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        ((h * 60.0).rem_euclid(360.0), s, l)
    }

    pub fn from_hsl(h: f32, s: f32, l: f32) -> Self {
        let l = l.clamp(0.0, 1.0);
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s.clamp(0.0, 1.0);
        let hp = h.rem_euclid(360.0) / 60.0;
        let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
        let (r, g, b) = match hp as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = l - c / 2.0;
        let byte = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        Color::new(byte(r), byte(g), byte(b))
    }

    /// Move lightness by `delta`, keeping hue and saturation.
    ///
    /// Lightening in HSL rather than mixing towards white in RGB, because an RGB
    /// mix washes the hue out: for the default blue, mixing gives `#99C9EE`
    /// where Windows' own light shade is `#99EBFF`.
    pub fn lighten(self, delta: f32) -> Self {
        let (h, s, l) = self.to_hsl();
        Color::from_hsl(h, s, l + delta)
    }

    /// Relative luminance, used to decide whether a colour is light enough that
    /// Windows will draw dark text on it.
    pub fn luminance(self) -> f32 {
        let channel = |c: u8| {
            let c = f32::from(c) / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
}

impl TryFrom<String> for Color {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Color::parse(&value)
    }
}

impl From<Color> for String {
    fn from(value: Color) -> Self {
        value.to_hex()
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips() {
        let c = Color::parse("#0F62C0").unwrap();
        assert_eq!(c, Color::new(0x0F, 0x62, 0xC0));
        assert_eq!(c.to_hex(), "#0F62C0");
        assert_eq!(Color::parse("0f62c0").unwrap(), c);
    }

    #[test]
    fn abgr_is_byte_reversed_with_full_alpha() {
        let c = Color::new(0x0F, 0x62, 0xC0);
        assert_eq!(c.to_abgr_dword(), 0xFFC0620F);
        assert_eq!(Color::from_abgr_dword(0xFFC0620F), c);
    }

    #[test]
    fn matches_what_windows_stores_for_its_default_accent() {
        // Read from a Windows 11 25H2 machine: AccentColor = 0xFFD77800.
        let default_blue = Color::new(0x00, 0x78, 0xD7);
        assert_eq!(default_blue.to_abgr_dword(), 0xFFD77800);
        assert_eq!(Color::from_abgr_dword(0xFFD77800), default_blue);
    }

    #[test]
    fn hsl_round_trips_within_a_step() {
        for c in [
            Color::new(0x00, 0x78, 0xD4),
            Color::new(0xF7, 0x63, 0x0C),
            Color::new(0x80, 0x80, 0x80),
            Color::new(0x00, 0x00, 0x00),
            Color::new(0xFF, 0xFF, 0xFF),
        ] {
            let (h, s, l) = c.to_hsl();
            let back = Color::from_hsl(h, s, l);
            let close = |a: u8, b: u8| (i16::from(a) - i16::from(b)).abs() <= 1;
            assert!(
                close(c.r, back.r) && close(c.g, back.g) && close(c.b, back.b),
                "{c} came back as {back}"
            );
        }
    }

    #[test]
    fn rejects_bad_hex() {
        assert!(Color::parse("#12345").is_err());
        assert!(Color::parse("#gggggg").is_err());
    }

    #[test]
    fn lightening_keeps_the_hue_and_clamps_at_the_ends() {
        let base = Color::new(0x00, 0x78, 0xD4);
        let (h, _, _) = base.to_hsl();
        let (lighter_h, _, lighter_l) = base.lighten(0.25).to_hsl();
        assert!((h - lighter_h).abs() < 1.0, "hue drifted");
        assert!(lighter_l > base.to_hsl().2);

        assert_eq!(base.lighten(1.0), Color::new(255, 255, 255));
        assert_eq!(base.lighten(-1.0), Color::new(0, 0, 0));
        assert_eq!(base.lighten(0.0), base);
    }
}
