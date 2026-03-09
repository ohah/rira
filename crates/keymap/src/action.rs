//! Editor actions that can be triggered by key bindings.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// An editor action that can be triggered by a key binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    FileSave,
    FileOpen,
    FileNew,
    FileClose,
    Undo,
    Redo,
    Copy,
    Paste,
    Cut,
    SelectAll,
    Find,
    FindReplace,
    AddCursorToNextMatch,
    ToggleSidebar,
    ToggleTerminal,
    OpenCommandPalette,
    OpenFileFinder,
    GoToLine,
    GoToDefinition,
    GoToReferences,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    SplitRight,
    SplitDown,
    NextTab,
    PreviousTab,
    ToggleComment,
    IndentLine,
    OutdentLine,
}

impl Action {
    /// Get the string representation of this action (dotted notation).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Action::FileSave => "file.save",
            Action::FileOpen => "file.open",
            Action::FileNew => "file.new",
            Action::FileClose => "file.close",
            Action::Undo => "edit.undo",
            Action::Redo => "edit.redo",
            Action::Copy => "edit.copy",
            Action::Paste => "edit.paste",
            Action::Cut => "edit.cut",
            Action::SelectAll => "edit.select_all",
            Action::Find => "edit.find",
            Action::FindReplace => "edit.find_replace",
            Action::AddCursorToNextMatch => "edit.add_cursor_to_next_match",
            Action::ToggleSidebar => "view.toggle_sidebar",
            Action::ToggleTerminal => "view.toggle_terminal",
            Action::OpenCommandPalette => "view.open_command_palette",
            Action::OpenFileFinder => "view.open_file_finder",
            Action::GoToLine => "navigate.go_to_line",
            Action::GoToDefinition => "navigate.go_to_definition",
            Action::GoToReferences => "navigate.go_to_references",
            Action::ZoomIn => "view.zoom_in",
            Action::ZoomOut => "view.zoom_out",
            Action::ZoomReset => "view.zoom_reset",
            Action::SplitRight => "view.split_right",
            Action::SplitDown => "view.split_down",
            Action::NextTab => "view.next_tab",
            Action::PreviousTab => "view.previous_tab",
            Action::ToggleComment => "edit.toggle_comment",
            Action::IndentLine => "edit.indent_line",
            Action::OutdentLine => "edit.outdent_line",
        }
    }

    /// All known actions.
    #[must_use]
    pub fn all() -> &'static [Action] {
        &[
            Action::FileSave,
            Action::FileOpen,
            Action::FileNew,
            Action::FileClose,
            Action::Undo,
            Action::Redo,
            Action::Copy,
            Action::Paste,
            Action::Cut,
            Action::SelectAll,
            Action::Find,
            Action::FindReplace,
            Action::AddCursorToNextMatch,
            Action::ToggleSidebar,
            Action::ToggleTerminal,
            Action::OpenCommandPalette,
            Action::OpenFileFinder,
            Action::GoToLine,
            Action::GoToDefinition,
            Action::GoToReferences,
            Action::ZoomIn,
            Action::ZoomOut,
            Action::ZoomReset,
            Action::SplitRight,
            Action::SplitDown,
            Action::NextTab,
            Action::PreviousTab,
            Action::ToggleComment,
            Action::IndentLine,
            Action::OutdentLine,
        ]
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error type for parsing an action from a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseActionError {
    pub input: String,
}

impl fmt::Display for ParseActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown action: '{}'", self.input)
    }
}

impl std::error::Error for ParseActionError {}

impl FromStr for Action {
    type Err = ParseActionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for action in Action::all() {
            if action.as_str() == s {
                return Ok(*action);
            }
        }
        Err(ParseActionError {
            input: s.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_from_str() {
        assert_eq!("file.save".parse::<Action>(), Ok(Action::FileSave));
        assert_eq!("file.open".parse::<Action>(), Ok(Action::FileOpen));
        assert_eq!("edit.undo".parse::<Action>(), Ok(Action::Undo));
        assert_eq!("edit.redo".parse::<Action>(), Ok(Action::Redo));
        assert_eq!("edit.copy".parse::<Action>(), Ok(Action::Copy));
        assert_eq!("edit.paste".parse::<Action>(), Ok(Action::Paste));
        assert_eq!("edit.cut".parse::<Action>(), Ok(Action::Cut));
        assert_eq!("edit.select_all".parse::<Action>(), Ok(Action::SelectAll));
        assert_eq!(
            "edit.add_cursor_to_next_match".parse::<Action>(),
            Ok(Action::AddCursorToNextMatch)
        );
        assert_eq!(
            "view.toggle_sidebar".parse::<Action>(),
            Ok(Action::ToggleSidebar)
        );
        assert_eq!(
            "view.toggle_terminal".parse::<Action>(),
            Ok(Action::ToggleTerminal)
        );
        assert_eq!(
            "view.open_command_palette".parse::<Action>(),
            Ok(Action::OpenCommandPalette)
        );
        assert_eq!(
            "view.open_file_finder".parse::<Action>(),
            Ok(Action::OpenFileFinder)
        );
    }

    #[test]
    fn test_action_from_str_invalid() {
        let err = "bogus.action".parse::<Action>().expect_err("should fail");
        assert_eq!(err.input, "bogus.action");
    }

    #[test]
    fn test_action_display() {
        assert_eq!(Action::FileSave.to_string(), "file.save");
        assert_eq!(
            Action::OpenCommandPalette.to_string(),
            "view.open_command_palette"
        );
    }

    #[test]
    fn test_all_actions_roundtrip() {
        for action in Action::all() {
            let s = action.as_str();
            let parsed: Action = s.parse().expect("all actions should roundtrip");
            assert_eq!(*action, parsed);
        }
    }
}
