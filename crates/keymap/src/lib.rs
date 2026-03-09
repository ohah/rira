//! rira-keymap: Key binding parsing, mapping, conflict detection

pub mod action;
pub mod keymap;
pub mod types;

pub use action::Action;
pub use keymap::{Conflict, Keymap};
pub use types::{Key, KeyBinding, Modifiers};

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
        // Create default keymap, merge user overrides, look up binding
        let mut km = Keymap::default_bindings();

        let toml = r#"
[[bindings]]
key = "ctrl+shift+s"
action = "file.save"
"#;
        let (overrides, warnings) = Keymap::from_toml(toml).expect("should parse");
        assert!(warnings.is_empty());

        let conflicts = km.merge(&overrides);
        assert!(conflicts.is_empty());

        let binding = KeyBinding::parse("ctrl+shift+s").expect("should parse");
        assert_eq!(km.lookup(&binding), Some(Action::FileSave));
    }
}
