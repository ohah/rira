//! Key types, modifiers, and key binding parsing.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A keyboard key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    Char(char),
    Backspace,
    Delete,
    Enter,
    Tab,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Char(c) => write!(f, "{c}"),
            Key::Backspace => write!(f, "backspace"),
            Key::Delete => write!(f, "delete"),
            Key::Enter => write!(f, "enter"),
            Key::Tab => write!(f, "tab"),
            Key::Escape => write!(f, "escape"),
            Key::Up => write!(f, "up"),
            Key::Down => write!(f, "down"),
            Key::Left => write!(f, "left"),
            Key::Right => write!(f, "right"),
            Key::Home => write!(f, "home"),
            Key::End => write!(f, "end"),
            Key::PageUp => write!(f, "pageup"),
            Key::PageDown => write!(f, "pagedown"),
            Key::F(n) => write!(f, "f{n}"),
        }
    }
}

impl Key {
    /// Parse a key name string into a `Key`.
    pub fn parse(s: &str) -> Option<Key> {
        let lower = s.to_lowercase();
        match lower.as_str() {
            "backspace" => Some(Key::Backspace),
            "delete" | "del" => Some(Key::Delete),
            "enter" | "return" => Some(Key::Enter),
            "tab" => Some(Key::Tab),
            "escape" | "esc" => Some(Key::Escape),
            "up" => Some(Key::Up),
            "down" => Some(Key::Down),
            "left" => Some(Key::Left),
            "right" => Some(Key::Right),
            "home" => Some(Key::Home),
            "end" => Some(Key::End),
            "pageup" => Some(Key::PageUp),
            "pagedown" => Some(Key::PageDown),
            other => {
                if let Some(num) = other.strip_prefix('f') {
                    if let Ok(n) = num.parse::<u8>() {
                        if (1..=24).contains(&n) {
                            return Some(Key::F(n));
                        }
                    }
                    return None;
                }
                let mut chars = other.chars();
                let c = chars.next()?;
                if chars.next().is_none() {
                    Some(Key::Char(c))
                } else {
                    None
                }
            }
        }
    }
}

/// Modifier flags for key bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Modifiers {
    bits: u8,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers { bits: 0 };
    pub const CMD: Modifiers = Modifiers { bits: 0b0001 };
    pub const CTRL: Modifiers = Modifiers { bits: 0b0010 };
    pub const SHIFT: Modifiers = Modifiers { bits: 0b0100 };
    pub const ALT: Modifiers = Modifiers { bits: 0b1000 };

    /// Create empty modifiers.
    #[must_use]
    pub const fn empty() -> Self {
        Modifiers { bits: 0 }
    }

    /// Check if no modifiers are set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    /// Check if a modifier flag is set.
    #[must_use]
    pub const fn contains(self, other: Modifiers) -> bool {
        (self.bits & other.bits) == other.bits
    }

    /// Combine two modifier sets.
    #[must_use]
    pub const fn union(self, other: Modifiers) -> Modifiers {
        Modifiers {
            bits: self.bits | other.bits,
        }
    }

    /// Parse a modifier name. Platform-aware: on macOS "cmd" = Cmd, elsewhere "cmd" = Ctrl.
    pub fn parse(s: &str) -> Option<Modifiers> {
        let lower = s.to_lowercase();
        match lower.as_str() {
            "cmd" | "command" | "meta" | "super" => {
                if cfg!(target_os = "macos") {
                    Some(Modifiers::CMD)
                } else {
                    Some(Modifiers::CTRL)
                }
            }
            "ctrl" | "control" => Some(Modifiers::CTRL),
            "shift" => Some(Modifiers::SHIFT),
            "alt" | "option" | "opt" => Some(Modifiers::ALT),
            _ => None,
        }
    }
}

impl fmt::Display for Modifiers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        let mut write_mod = |name: &str, f: &mut fmt::Formatter<'_>| -> fmt::Result {
            if !first {
                write!(f, "+")?;
            }
            first = false;
            write!(f, "{name}")
        };
        if self.contains(Modifiers::CMD) {
            write_mod("cmd", f)?;
        }
        if self.contains(Modifiers::CTRL) {
            write_mod("ctrl", f)?;
        }
        if self.contains(Modifiers::SHIFT) {
            write_mod("shift", f)?;
        }
        if self.contains(Modifiers::ALT) {
            write_mod("alt", f)?;
        }
        if first {
            // no modifiers written
        }
        Ok(())
    }
}

/// A key binding: modifiers + key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyBinding {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl KeyBinding {
    /// Create a new key binding.
    #[must_use]
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        KeyBinding { modifiers, key }
    }

    /// Parse a string like "cmd+s", "ctrl+shift+p", "escape".
    /// Returns None if parsing fails.
    pub fn parse(s: &str) -> Option<KeyBinding> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let parts: Vec<&str> = s.split('+').collect();
        if parts.is_empty() {
            return None;
        }

        let mut modifiers = Modifiers::empty();
        let key_part = parts.last()?;

        // Try to parse all parts except the last as modifiers.
        // If the last part is also a modifier (and there's no key), treat it as a key parse failure.
        for part in &parts[..parts.len() - 1] {
            let part = part.trim();
            match Modifiers::parse(part) {
                Some(m) => modifiers = modifiers.union(m),
                None => return None,
            }
        }

        let key = Key::parse(key_part.trim())?;
        Some(KeyBinding::new(modifiers, key))
    }
}

impl fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.modifiers.is_empty() {
            write!(f, "{}+{}", self.modifiers, self.key)
        } else {
            write!(f, "{}", self.key)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_parse_char() {
        assert_eq!(Key::parse("s"), Some(Key::Char('s')));
        assert_eq!(Key::parse("S"), Some(Key::Char('s')));
        assert_eq!(Key::parse("a"), Some(Key::Char('a')));
    }

    #[test]
    fn test_key_parse_special() {
        assert_eq!(Key::parse("enter"), Some(Key::Enter));
        assert_eq!(Key::parse("Enter"), Some(Key::Enter));
        assert_eq!(Key::parse("return"), Some(Key::Enter));
        assert_eq!(Key::parse("escape"), Some(Key::Escape));
        assert_eq!(Key::parse("esc"), Some(Key::Escape));
        assert_eq!(Key::parse("tab"), Some(Key::Tab));
        assert_eq!(Key::parse("backspace"), Some(Key::Backspace));
        assert_eq!(Key::parse("delete"), Some(Key::Delete));
        assert_eq!(Key::parse("del"), Some(Key::Delete));
    }

    #[test]
    fn test_key_parse_arrows() {
        assert_eq!(Key::parse("up"), Some(Key::Up));
        assert_eq!(Key::parse("down"), Some(Key::Down));
        assert_eq!(Key::parse("left"), Some(Key::Left));
        assert_eq!(Key::parse("right"), Some(Key::Right));
    }

    #[test]
    fn test_key_parse_function() {
        assert_eq!(Key::parse("f1"), Some(Key::F(1)));
        assert_eq!(Key::parse("F12"), Some(Key::F(12)));
        assert_eq!(Key::parse("f24"), Some(Key::F(24)));
        assert_eq!(Key::parse("f0"), None);
        assert_eq!(Key::parse("f25"), None);
    }

    #[test]
    fn test_key_parse_invalid() {
        assert_eq!(Key::parse(""), None);
        assert_eq!(Key::parse("abc"), None);
    }

    #[test]
    fn test_modifiers_basic() {
        assert!(Modifiers::NONE.is_empty());
        assert!(!Modifiers::CMD.is_empty());
        assert!(Modifiers::CMD.contains(Modifiers::CMD));
        assert!(!Modifiers::CMD.contains(Modifiers::CTRL));
    }

    #[test]
    fn test_modifiers_union() {
        let mods = Modifiers::CMD.union(Modifiers::SHIFT);
        assert!(mods.contains(Modifiers::CMD));
        assert!(mods.contains(Modifiers::SHIFT));
        assert!(!mods.contains(Modifiers::ALT));
    }

    #[test]
    fn test_modifiers_parse() {
        assert_eq!(Modifiers::parse("ctrl"), Some(Modifiers::CTRL));
        assert_eq!(Modifiers::parse("Ctrl"), Some(Modifiers::CTRL));
        assert_eq!(Modifiers::parse("shift"), Some(Modifiers::SHIFT));
        assert_eq!(Modifiers::parse("alt"), Some(Modifiers::ALT));
        assert_eq!(Modifiers::parse("option"), Some(Modifiers::ALT));
        assert_eq!(Modifiers::parse("bogus"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_cmd_maps_to_cmd_on_macos() {
        assert_eq!(Modifiers::parse("cmd"), Some(Modifiers::CMD));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_cmd_maps_to_ctrl_on_other() {
        assert_eq!(Modifiers::parse("cmd"), Some(Modifiers::CTRL));
    }

    #[test]
    fn test_keybinding_parse_simple() {
        let kb = KeyBinding::parse("escape").expect("should parse");
        assert_eq!(kb.modifiers, Modifiers::NONE);
        assert_eq!(kb.key, Key::Escape);
    }

    #[test]
    fn test_keybinding_parse_with_modifier() {
        let kb = KeyBinding::parse("ctrl+s").expect("should parse");
        assert!(kb.modifiers.contains(Modifiers::CTRL));
        assert_eq!(kb.key, Key::Char('s'));
    }

    #[test]
    fn test_keybinding_parse_multiple_modifiers() {
        let kb = KeyBinding::parse("ctrl+shift+p").expect("should parse");
        assert!(kb.modifiers.contains(Modifiers::CTRL));
        assert!(kb.modifiers.contains(Modifiers::SHIFT));
        assert_eq!(kb.key, Key::Char('p'));
    }

    #[test]
    fn test_keybinding_parse_invalid() {
        assert!(KeyBinding::parse("").is_none());
        assert!(KeyBinding::parse("bogus+bogus").is_none());
    }

    #[test]
    fn test_keybinding_display() {
        let kb = KeyBinding::new(Modifiers::CTRL.union(Modifiers::SHIFT), Key::Char('p'));
        let s = kb.to_string();
        assert!(s.contains("ctrl"));
        assert!(s.contains("shift"));
        assert!(s.contains('p'));
    }

    #[test]
    fn test_keybinding_display_no_modifiers() {
        let kb = KeyBinding::new(Modifiers::NONE, Key::Escape);
        assert_eq!(kb.to_string(), "escape");
    }

    #[test]
    fn test_key_display() {
        assert_eq!(Key::F(5).to_string(), "f5");
        assert_eq!(Key::Char('a').to_string(), "a");
        assert_eq!(Key::Enter.to_string(), "enter");
    }
}
