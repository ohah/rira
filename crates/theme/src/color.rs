//! Color type with hex parsing and display.

use serde::{Deserialize, Serialize};
use std::fmt;

/// An RGBA color with 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create a new color with full opacity.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    /// Create a new color with explicit alpha.
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    /// Parse a hex color string.
    ///
    /// Supported formats:
    /// - `#RGB` (4-bit per channel, expanded to 8-bit)
    /// - `#RGBA` (4-bit per channel with alpha)
    /// - `#RRGGBB` (8-bit per channel)
    /// - `#RRGGBBAA` (8-bit per channel with alpha)
    ///
    /// The `#` prefix is optional.
    pub fn from_hex(s: &str) -> Result<Self, ColorParseError> {
        let s = s.trim();
        let hex = s.strip_prefix('#').unwrap_or(s);

        if hex.is_empty() {
            return Err(ColorParseError::Empty);
        }

        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ColorParseError::InvalidChar);
        }

        match hex.len() {
            3 => {
                let r = parse_hex_digit(hex.as_bytes()[0])?;
                let g = parse_hex_digit(hex.as_bytes()[1])?;
                let b = parse_hex_digit(hex.as_bytes()[2])?;
                Ok(Color::rgb(r << 4 | r, g << 4 | g, b << 4 | b))
            }
            4 => {
                let r = parse_hex_digit(hex.as_bytes()[0])?;
                let g = parse_hex_digit(hex.as_bytes()[1])?;
                let b = parse_hex_digit(hex.as_bytes()[2])?;
                let a = parse_hex_digit(hex.as_bytes()[3])?;
                Ok(Color::rgba(r << 4 | r, g << 4 | g, b << 4 | b, a << 4 | a))
            }
            6 => {
                let r = parse_hex_byte(&hex[0..2])?;
                let g = parse_hex_byte(&hex[2..4])?;
                let b = parse_hex_byte(&hex[4..6])?;
                Ok(Color::rgb(r, g, b))
            }
            8 => {
                let r = parse_hex_byte(&hex[0..2])?;
                let g = parse_hex_byte(&hex[2..4])?;
                let b = parse_hex_byte(&hex[4..6])?;
                let a = parse_hex_byte(&hex[6..8])?;
                Ok(Color::rgba(r, g, b, a))
            }
            _ => Err(ColorParseError::InvalidLength(hex.len())),
        }
    }

    /// White color (default).
    #[must_use]
    pub const fn white() -> Self {
        Color::rgb(255, 255, 255)
    }

    /// Black color.
    #[must_use]
    pub const fn black() -> Self {
        Color::rgb(0, 0, 0)
    }

    /// Transparent color.
    #[must_use]
    pub const fn transparent() -> Self {
        Color::rgba(0, 0, 0, 0)
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::white()
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a == 255 {
            write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            write!(
                f,
                "#{:02X}{:02X}{:02X}{:02X}",
                self.r, self.g, self.b, self.a
            )
        }
    }
}

/// Error type for color parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorParseError {
    Empty,
    InvalidChar,
    InvalidLength(usize),
    InvalidHex,
}

impl fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColorParseError::Empty => write!(f, "empty color string"),
            ColorParseError::InvalidChar => write!(f, "invalid hex character"),
            ColorParseError::InvalidLength(len) => {
                write!(
                    f,
                    "invalid hex color length: {len} (expected 3, 4, 6, or 8)"
                )
            }
            ColorParseError::InvalidHex => write!(f, "invalid hex value"),
        }
    }
}

impl std::error::Error for ColorParseError {}

fn parse_hex_digit(byte: u8) -> Result<u8, ColorParseError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(ColorParseError::InvalidHex),
    }
}

fn parse_hex_byte(s: &str) -> Result<u8, ColorParseError> {
    u8::from_str_radix(s, 16).map_err(|_| ColorParseError::InvalidHex)
}

/// Custom deserializer that supports hex color strings in TOML.
pub mod serde_color {
    use super::Color;
    use serde::{self, Deserialize, Deserializer, Serializer};

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S>(color: &Color, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&color.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Color, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Color::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_hex_6_digit() {
        let c = Color::from_hex("#FF79C6").expect("should parse");
        assert_eq!(c.r, 0xFF);
        assert_eq!(c.g, 0x79);
        assert_eq!(c.b, 0xC6);
        assert_eq!(c.a, 255);
    }

    #[test]
    fn test_from_hex_8_digit() {
        let c = Color::from_hex("#FF79C680").expect("should parse");
        assert_eq!(c.r, 0xFF);
        assert_eq!(c.g, 0x79);
        assert_eq!(c.b, 0xC6);
        assert_eq!(c.a, 0x80);
    }

    #[test]
    fn test_from_hex_3_digit() {
        let c = Color::from_hex("#FFF").expect("should parse");
        assert_eq!(c, Color::rgb(255, 255, 255));

        let c = Color::from_hex("#000").expect("should parse");
        assert_eq!(c, Color::rgb(0, 0, 0));

        let c = Color::from_hex("#F0A").expect("should parse");
        assert_eq!(c.r, 0xFF);
        assert_eq!(c.g, 0x00);
        assert_eq!(c.b, 0xAA);
    }

    #[test]
    fn test_from_hex_4_digit() {
        let c = Color::from_hex("#FFFA").expect("should parse");
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 255);
        assert_eq!(c.b, 255);
        assert_eq!(c.a, 0xAA);
    }

    #[test]
    fn test_from_hex_no_hash() {
        let c = Color::from_hex("FF79C6").expect("should parse");
        assert_eq!(c.r, 0xFF);
        assert_eq!(c.g, 0x79);
        assert_eq!(c.b, 0xC6);
    }

    #[test]
    fn test_from_hex_lowercase() {
        let c = Color::from_hex("#ff79c6").expect("should parse");
        assert_eq!(c.r, 0xFF);
        assert_eq!(c.g, 0x79);
        assert_eq!(c.b, 0xC6);
    }

    #[test]
    fn test_from_hex_invalid_empty() {
        assert_eq!(Color::from_hex(""), Err(ColorParseError::Empty));
        assert_eq!(Color::from_hex("#"), Err(ColorParseError::Empty));
    }

    #[test]
    fn test_from_hex_invalid_chars() {
        assert_eq!(
            Color::from_hex("#GGHHII"),
            Err(ColorParseError::InvalidChar)
        );
        assert_eq!(Color::from_hex("#XYZ"), Err(ColorParseError::InvalidChar));
    }

    #[test]
    fn test_from_hex_invalid_length() {
        assert_eq!(
            Color::from_hex("#FF"),
            Err(ColorParseError::InvalidLength(2))
        );
        assert_eq!(
            Color::from_hex("#FF79C6FF0"),
            Err(ColorParseError::InvalidLength(9))
        );
    }

    #[test]
    fn test_default_is_white() {
        assert_eq!(Color::default(), Color::rgb(255, 255, 255));
    }

    #[test]
    fn test_display_rgb() {
        let c = Color::rgb(255, 121, 198);
        assert_eq!(c.to_string(), "#FF79C6");
    }

    #[test]
    fn test_display_rgba() {
        let c = Color::rgba(255, 121, 198, 128);
        assert_eq!(c.to_string(), "#FF79C680");
    }

    #[test]
    fn test_display_roundtrip() {
        let original = Color::rgb(0x28, 0x2A, 0x36);
        let hex = original.to_string();
        let parsed = Color::from_hex(&hex).expect("roundtrip should work");
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_display_roundtrip_with_alpha() {
        let original = Color::rgba(0x28, 0x2A, 0x36, 0x80);
        let hex = original.to_string();
        let parsed = Color::from_hex(&hex).expect("roundtrip should work");
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_constants() {
        assert_eq!(Color::white(), Color::rgb(255, 255, 255));
        assert_eq!(Color::black(), Color::rgb(0, 0, 0));
        assert_eq!(Color::transparent(), Color::rgba(0, 0, 0, 0));
    }
}
