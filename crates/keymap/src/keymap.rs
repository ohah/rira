//! Keymap loading, merging, and lookup.

use std::collections::HashMap;

use serde::Deserialize;

use crate::action::Action;
use crate::types::KeyBinding;

/// A mapping from key bindings to editor actions.
#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: HashMap<KeyBinding, Action>,
}

/// A single binding entry in a TOML keymap file.
#[derive(Debug, Deserialize)]
struct TomlBinding {
    key: String,
    action: String,
}

/// The top-level structure of a TOML keymap file.
#[derive(Debug, Deserialize)]
struct TomlKeymap {
    #[serde(default)]
    bindings: Vec<TomlBinding>,
}

/// A conflict detected when merging keymaps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub binding: KeyBinding,
    pub old_action: Action,
    pub new_action: Action,
}

impl Keymap {
    /// Create an empty keymap.
    #[must_use]
    pub fn empty() -> Self {
        Keymap {
            bindings: HashMap::new(),
        }
    }

    /// Create a keymap with VSCode-like default bindings.
    /// Uses platform-aware modifier parsing (cmd = Cmd on macOS, Ctrl elsewhere).
    #[must_use]
    pub fn default_bindings() -> Self {
        let mut keymap = Keymap::empty();
        let defaults = [
            ("cmd+s", Action::FileSave),
            ("cmd+o", Action::FileOpen),
            ("cmd+n", Action::FileNew),
            ("cmd+w", Action::FileClose),
            ("cmd+z", Action::Undo),
            ("cmd+shift+z", Action::Redo),
            ("cmd+c", Action::Copy),
            ("cmd+v", Action::Paste),
            ("cmd+x", Action::Cut),
            ("cmd+a", Action::SelectAll),
            ("cmd+f", Action::Find),
            ("cmd+h", Action::FindReplace),
            ("cmd+d", Action::AddCursorToNextMatch),
            ("cmd+b", Action::ToggleSidebar),
            ("cmd+`", Action::ToggleTerminal),
            ("cmd+shift+p", Action::OpenCommandPalette),
            ("cmd+p", Action::OpenFileFinder),
            ("ctrl+g", Action::GoToLine),
            ("cmd+shift+=", Action::ZoomIn),
            ("cmd+-", Action::ZoomOut),
            ("cmd+shift+/", Action::ToggleComment),
        ];

        for (key_str, action) in defaults {
            if let Some(binding) = KeyBinding::parse(key_str) {
                keymap.bindings.insert(binding, action);
            }
        }

        keymap
    }

    /// Parse a keymap from TOML content. Returns the keymap and any parse warnings.
    ///
    /// # Errors
    /// Returns an error if the TOML is malformed.
    pub fn from_toml(content: &str) -> Result<(Self, Vec<String>), String> {
        let parsed: TomlKeymap =
            toml::from_str(content).map_err(|e| format!("failed to parse keymap TOML: {e}"))?;

        let mut keymap = Keymap::empty();
        let mut warnings = Vec::new();

        for entry in &parsed.bindings {
            let binding = match KeyBinding::parse(&entry.key) {
                Some(b) => b,
                None => {
                    warnings.push(format!("invalid key binding: '{}'", entry.key));
                    continue;
                }
            };

            let action: Action = match entry.action.parse() {
                Ok(a) => a,
                Err(_) => {
                    warnings.push(format!("unknown action: '{}'", entry.action));
                    continue;
                }
            };

            if let Some(old) = keymap.bindings.insert(binding, action) {
                if old != action {
                    warnings.push(format!(
                        "duplicate binding '{}': '{}' replaced by '{}'",
                        entry.key, old, action
                    ));
                }
            }
        }

        Ok((keymap, warnings))
    }

    /// Merge overrides on top of this keymap. Returns any conflicts detected.
    pub fn merge(&mut self, overrides: &Keymap) -> Vec<Conflict> {
        let mut conflicts = Vec::new();

        for (binding, new_action) in &overrides.bindings {
            if let Some(old_action) = self.bindings.get(binding) {
                if old_action != new_action {
                    conflicts.push(Conflict {
                        binding: *binding,
                        old_action: *old_action,
                        new_action: *new_action,
                    });
                }
            }
            self.bindings.insert(*binding, *new_action);
        }

        conflicts
    }

    /// Look up the action for a key binding.
    #[must_use]
    pub fn lookup(&self, binding: &KeyBinding) -> Option<Action> {
        self.bindings.get(binding).copied()
    }

    /// Insert a single binding.
    pub fn insert(&mut self, binding: KeyBinding, action: Action) -> Option<Action> {
        self.bindings.insert(binding, action)
    }

    /// Number of bindings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Whether the keymap is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Iterate over all bindings.
    pub fn iter(&self) -> impl Iterator<Item = (&KeyBinding, &Action)> {
        self.bindings.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Key, Modifiers};

    #[test]
    fn test_default_bindings_not_empty() {
        let km = Keymap::default_bindings();
        assert!(!km.is_empty());
    }

    #[test]
    fn test_default_bindings_lookup() {
        let km = Keymap::default_bindings();

        // cmd+s should map to FileSave
        // On macOS cmd = Cmd, on Linux cmd = Ctrl, but the binding was parsed via cmd
        let binding = KeyBinding::parse("cmd+s").expect("should parse cmd+s");
        assert_eq!(km.lookup(&binding), Some(Action::FileSave));
    }

    #[test]
    fn test_default_bindings_cmd_shift() {
        let km = Keymap::default_bindings();
        let binding = KeyBinding::parse("cmd+shift+p").expect("should parse");
        assert_eq!(km.lookup(&binding), Some(Action::OpenCommandPalette));
    }

    #[test]
    fn test_from_toml_valid() {
        let toml = r#"
[[bindings]]
key = "ctrl+s"
action = "file.save"

[[bindings]]
key = "ctrl+z"
action = "edit.undo"
"#;
        let (km, warnings) = Keymap::from_toml(toml).expect("should parse");
        assert!(warnings.is_empty(), "warnings: {:?}", warnings);
        assert_eq!(km.len(), 2);

        let binding = KeyBinding::new(Modifiers::CTRL, Key::Char('s'));
        assert_eq!(km.lookup(&binding), Some(Action::FileSave));
    }

    #[test]
    fn test_from_toml_with_warnings() {
        let toml = r#"
[[bindings]]
key = "ctrl+s"
action = "file.save"

[[bindings]]
key = "invalid_key_here"
action = "file.save"

[[bindings]]
key = "ctrl+z"
action = "bogus.action"
"#;
        let (km, warnings) = Keymap::from_toml(toml).expect("should parse");
        assert_eq!(km.len(), 1);
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn test_from_toml_empty() {
        let toml = r#"
bindings = []
"#;
        let (km, warnings) = Keymap::from_toml(toml).expect("should parse");
        assert!(km.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_from_toml_no_bindings_key() {
        let toml = "";
        let (km, warnings) = Keymap::from_toml(toml).expect("should parse");
        assert!(km.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_from_toml_malformed() {
        let toml = "this is not valid toml {{{";
        assert!(Keymap::from_toml(toml).is_err());
    }

    #[test]
    fn test_from_toml_duplicate_binding_warns() {
        let toml = r#"
[[bindings]]
key = "ctrl+s"
action = "file.save"

[[bindings]]
key = "ctrl+s"
action = "file.open"
"#;
        let (km, warnings) = Keymap::from_toml(toml).expect("should parse");
        assert_eq!(km.len(), 1);
        // Should have overwritten and warned
        let binding = KeyBinding::new(Modifiers::CTRL, Key::Char('s'));
        assert_eq!(km.lookup(&binding), Some(Action::FileOpen));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("duplicate"));
    }

    #[test]
    fn test_merge_no_conflicts() {
        let mut base = Keymap::empty();
        base.insert(
            KeyBinding::new(Modifiers::CTRL, Key::Char('s')),
            Action::FileSave,
        );

        let mut overrides = Keymap::empty();
        overrides.insert(
            KeyBinding::new(Modifiers::CTRL, Key::Char('z')),
            Action::Undo,
        );

        let conflicts = base.merge(&overrides);
        assert!(conflicts.is_empty());
        assert_eq!(base.len(), 2);
    }

    #[test]
    fn test_merge_with_conflicts() {
        let mut base = Keymap::empty();
        base.insert(
            KeyBinding::new(Modifiers::CTRL, Key::Char('s')),
            Action::FileSave,
        );

        let mut overrides = Keymap::empty();
        overrides.insert(
            KeyBinding::new(Modifiers::CTRL, Key::Char('s')),
            Action::FileOpen,
        );

        let conflicts = base.merge(&overrides);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].old_action, Action::FileSave);
        assert_eq!(conflicts[0].new_action, Action::FileOpen);

        // Override should win
        let binding = KeyBinding::new(Modifiers::CTRL, Key::Char('s'));
        assert_eq!(base.lookup(&binding), Some(Action::FileOpen));
    }

    #[test]
    fn test_merge_same_action_no_conflict() {
        let mut base = Keymap::empty();
        base.insert(
            KeyBinding::new(Modifiers::CTRL, Key::Char('s')),
            Action::FileSave,
        );

        let mut overrides = Keymap::empty();
        overrides.insert(
            KeyBinding::new(Modifiers::CTRL, Key::Char('s')),
            Action::FileSave,
        );

        let conflicts = base.merge(&overrides);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_lookup_missing() {
        let km = Keymap::empty();
        let binding = KeyBinding::new(Modifiers::CTRL, Key::Char('q'));
        assert_eq!(km.lookup(&binding), None);
    }

    #[test]
    fn test_iter() {
        let mut km = Keymap::empty();
        km.insert(
            KeyBinding::new(Modifiers::CTRL, Key::Char('s')),
            Action::FileSave,
        );
        km.insert(
            KeyBinding::new(Modifiers::CTRL, Key::Char('z')),
            Action::Undo,
        );
        assert_eq!(km.iter().count(), 2);
    }
}
