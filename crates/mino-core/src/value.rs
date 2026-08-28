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
    Choice { choices: Vec<String> },
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

    /// Windows stores accent colours as a DWORD in `0x00BBGGRR` order — the
    /// reverse of the `#RRGGBB` people write. Getting this backwards is the
    /// classic bug in every accent-colour tool, so it lives in exactly one place.
    pub fn to_abgr_dword(self) -> u32 {
        u32::from(self.r) | (u32::from(self.g) << 8) | (u32::from(self.b) << 16)
    }

    pub fn from_abgr_dword(value: u32) -> Self {
        Color::new(
            (value & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
        )
    }

    /// Mix towards white (`amount > 0`) or black (`amount < 0`), where `amount`
    /// runs from -1.0 to 1.0. Used to derive the accent shade ramp.
    pub fn shade(self, amount: f32) -> Self {
        let target = if amount >= 0.0 { 255.0 } else { 0.0 };
        let t = amount.abs().clamp(0.0, 1.0);
        let mix = |c: u8| (f32::from(c) + (target - f32::from(c)) * t).round().clamp(0.0, 255.0) as u8;
        Color::new(mix(self.r), mix(self.g), mix(self.b))
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
    fn abgr_is_byte_reversed() {
        let c = Color::new(0x0F, 0x62, 0xC0);
        assert_eq!(c.to_abgr_dword(), 0x00C0620F);
        assert_eq!(Color::from_abgr_dword(0x00C0620F), c);
    }

    #[test]
    fn rejects_bad_hex() {
        assert!(Color::parse("#12345").is_err());
        assert!(Color::parse("#gggggg").is_err());
    }

    #[test]
    fn shading_moves_towards_white_and_black() {
        let c = Color::new(0x80, 0x80, 0x80);
        assert!(c.shade(1.0) == Color::new(255, 255, 255));
        assert!(c.shade(-1.0) == Color::new(0, 0, 0));
        assert_eq!(c.shade(0.0), c);
    }
}
