//! Theme structure and TOML parsing with defaults.

use serde::{Deserialize, Serialize};

use crate::color::Color;

/// Syntax highlighting colors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxColors {
    #[serde(with = "crate::color::serde_color")]
    pub keyword: Color,
    #[serde(with = "crate::color::serde_color")]
    pub string: Color,
    #[serde(with = "crate::color::serde_color")]
    pub comment: Color,
    #[serde(with = "crate::color::serde_color")]
    pub function: Color,
    #[serde(with = "crate::color::serde_color")]
    pub variable: Color,
    #[serde(with = "crate::color::serde_color")]
    pub number: Color,
    #[serde(with = "crate::color::serde_color")]
    pub type_name: Color,
    #[serde(with = "crate::color::serde_color")]
    pub operator: Color,
}

impl Default for SyntaxColors {
    fn default() -> Self {
        // Dracula-inspired dark theme defaults
        SyntaxColors {
            keyword: Color::rgb(0xFF, 0x79, 0xC6),   // pink
            string: Color::rgb(0xF1, 0xFA, 0x8C),    // yellow
            comment: Color::rgb(0x62, 0x72, 0xA4),   // muted blue
            function: Color::rgb(0x50, 0xFA, 0x7B),  // green
            variable: Color::rgb(0xF8, 0xF8, 0xF2),  // foreground
            number: Color::rgb(0xBD, 0x93, 0xF9),    // purple
            type_name: Color::rgb(0x8B, 0xE9, 0xFD), // cyan
            operator: Color::rgb(0xFF, 0x79, 0xC6),  // pink
        }
    }
}

/// UI element colors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiColors {
    #[serde(with = "crate::color::serde_color")]
    pub sidebar_bg: Color,
    #[serde(with = "crate::color::serde_color")]
    pub tab_active_bg: Color,
    #[serde(with = "crate::color::serde_color")]
    pub tab_inactive_bg: Color,
    #[serde(with = "crate::color::serde_color")]
    pub terminal_bg: Color,
    #[serde(with = "crate::color::serde_color")]
    pub gutter: Color,
    #[serde(with = "crate::color::serde_color")]
    pub diff_added: Color,
    #[serde(with = "crate::color::serde_color")]
    pub diff_removed: Color,
}

impl Default for UiColors {
    fn default() -> Self {
        UiColors {
            sidebar_bg: Color::rgb(0x21, 0x22, 0x2C),
            tab_active_bg: Color::rgb(0x28, 0x2A, 0x36),
            tab_inactive_bg: Color::rgb(0x21, 0x22, 0x2C),
            terminal_bg: Color::rgb(0x1E, 0x1F, 0x29),
            gutter: Color::rgb(0x62, 0x72, 0xA4),
            diff_added: Color::rgba(0x50, 0xFA, 0x7B, 0x33),
            diff_removed: Color::rgba(0xFF, 0x55, 0x55, 0x33),
        }
    }
}

/// Editor area colors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorColors {
    #[serde(with = "crate::color::serde_color")]
    pub background: Color,
    #[serde(with = "crate::color::serde_color")]
    pub foreground: Color,
    #[serde(with = "crate::color::serde_color")]
    pub cursor: Color,
    #[serde(with = "crate::color::serde_color")]
    pub selection: Color,
    #[serde(with = "crate::color::serde_color")]
    pub line_highlight: Color,
}

impl Default for EditorColors {
    fn default() -> Self {
        EditorColors {
            background: Color::rgb(0x28, 0x2A, 0x36),
            foreground: Color::rgb(0xF8, 0xF8, 0xF2),
            cursor: Color::rgb(0xF8, 0xF8, 0xF2),
            selection: Color::rgba(0x44, 0x47, 0x5A, 0xCC),
            line_highlight: Color::rgba(0x44, 0x47, 0x5A, 0x66),
        }
    }
}

/// A complete theme combining syntax, UI, and editor colors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub syntax: SyntaxColors,
    #[serde(default)]
    pub ui: UiColors,
    #[serde(default)]
    pub editor: EditorColors,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            name: "Rira Dark".to_string(),
            syntax: SyntaxColors::default(),
            ui: UiColors::default(),
            editor: EditorColors::default(),
        }
    }
}

impl Theme {
    /// Parse a theme from TOML content. Missing fields fall back to defaults.
    ///
    /// # Errors
    /// Returns an error if the TOML is malformed.
    pub fn from_toml(content: &str) -> Result<Self, String> {
        // For empty content, return defaults
        if content.trim().is_empty() {
            return Ok(Theme::default());
        }

        // We use a two-pass approach: first parse to a raw Value to check structure,
        // then deserialize with defaults for missing fields.
        let theme: Theme =
            toml::from_str(content).map_err(|e| format!("failed to parse theme TOML: {e}"))?;
        Ok(theme)
    }

    /// Serialize this theme to TOML.
    ///
    /// # Errors
    /// Returns an error if serialization fails.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| format!("failed to serialize theme: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme() {
        let theme = Theme::default();
        assert_eq!(theme.name, "Rira Dark");
        assert_eq!(theme.editor.background, Color::rgb(0x28, 0x2A, 0x36));
        assert_eq!(theme.editor.foreground, Color::rgb(0xF8, 0xF8, 0xF2));
        assert_eq!(theme.syntax.keyword, Color::rgb(0xFF, 0x79, 0xC6));
    }

    #[test]
    fn test_from_toml_full() {
        let toml = r##"
name = "My Theme"

[syntax]
keyword = "#FF0000"
string = "#00FF00"
comment = "#0000FF"
function = "#FFFF00"
variable = "#FF00FF"
number = "#00FFFF"
type_name = "#FFFFFF"
operator = "#808080"

[ui]
sidebar_bg = "#111111"
tab_active_bg = "#222222"
tab_inactive_bg = "#333333"
terminal_bg = "#444444"
gutter = "#555555"
diff_added = "#66FF66"
diff_removed = "#FF6666"

[editor]
background = "#1A1A1A"
foreground = "#FAFAFA"
cursor = "#FFFFFF"
selection = "#44475ACC"
line_highlight = "#33333366"
"##;
        let theme = Theme::from_toml(toml).expect("should parse");
        assert_eq!(theme.name, "My Theme");
        assert_eq!(theme.syntax.keyword, Color::rgb(0xFF, 0x00, 0x00));
        assert_eq!(theme.editor.background, Color::rgb(0x1A, 0x1A, 0x1A));
        assert_eq!(theme.ui.sidebar_bg, Color::rgb(0x11, 0x11, 0x11));
    }

    #[test]
    fn test_from_toml_partial_uses_defaults() {
        let toml = r##"
name = "Partial"

[editor]
background = "#000000"
foreground = "#FFFFFF"
cursor = "#FFFFFF"
selection = "#333333"
line_highlight = "#222222"
"##;
        let theme = Theme::from_toml(toml).expect("should parse");
        assert_eq!(theme.name, "Partial");
        assert_eq!(theme.editor.background, Color::rgb(0, 0, 0));
        // syntax and ui should be defaults
        assert_eq!(theme.syntax, SyntaxColors::default());
        assert_eq!(theme.ui, UiColors::default());
    }

    #[test]
    fn test_from_toml_empty() {
        let theme = Theme::from_toml("").expect("should parse empty");
        assert_eq!(theme, Theme::default());
    }

    #[test]
    fn test_from_toml_only_name() {
        let toml = r##"name = "Just Name""##;
        let theme = Theme::from_toml(toml).expect("should parse");
        assert_eq!(theme.name, "Just Name");
        assert_eq!(theme.syntax, SyntaxColors::default());
        assert_eq!(theme.ui, UiColors::default());
        assert_eq!(theme.editor, EditorColors::default());
    }

    #[test]
    fn test_from_toml_malformed() {
        assert!(Theme::from_toml("this is not valid {{{").is_err());
    }

    #[test]
    fn test_from_toml_invalid_color() {
        let toml = r##"
[editor]
background = "not-a-color"
foreground = "#FFFFFF"
cursor = "#FFFFFF"
selection = "#333333"
line_highlight = "#222222"
"##;
        assert!(Theme::from_toml(toml).is_err());
    }

    #[test]
    fn test_roundtrip_toml() {
        let original = Theme::default();
        let toml_str = original.to_toml().expect("should serialize");
        let parsed = Theme::from_toml(&toml_str).expect("should parse back");
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_syntax_colors_default() {
        let sc = SyntaxColors::default();
        // All colors should be non-default (not white)
        assert_ne!(sc.keyword, Color::white());
        assert_ne!(sc.comment, Color::white());
    }

    #[test]
    fn test_ui_colors_default() {
        let uc = UiColors::default();
        // diff colors should have alpha
        assert_ne!(uc.diff_added.a, 255);
        assert_ne!(uc.diff_removed.a, 255);
    }

    #[test]
    fn test_editor_colors_default() {
        let ec = EditorColors::default();
        assert_ne!(ec.background, ec.foreground);
        assert_ne!(ec.selection.a, 255); // selection has alpha
    }
}
