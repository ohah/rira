//! rira-theme: Theme parsing, color schemes, VSCode theme import

pub mod color;
pub mod theme;

pub use color::{Color, ColorParseError};
pub use theme::{EditorColors, SyntaxColors, Theme, UiColors};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(version(), "0.1.0");
    }

    #[test]
    fn test_public_api_integration() {
        // Parse a color
        let c = Color::from_hex("#FF79C6").expect("should parse");
        assert_eq!(c.to_string(), "#FF79C6");

        // Create default theme
        let theme = Theme::default();
        assert_eq!(theme.syntax.keyword, c);

        // Parse from TOML with partial overrides
        let toml = r##"
name = "Custom"
[editor]
background = "#000000"
foreground = "#FFFFFF"
cursor = "#FFFFFF"
selection = "#333333"
line_highlight = "#222222"
"##;
        let theme = Theme::from_toml(toml).expect("should parse");
        assert_eq!(theme.name, "Custom");
        assert_eq!(theme.editor.background, Color::rgb(0, 0, 0));
        // Syntax falls back to defaults
        assert_eq!(theme.syntax, SyntaxColors::default());
    }
}
