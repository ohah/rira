//! Text selection model.

use crate::buffer::Buffer;
use crate::cursor::Cursor;

/// A text selection defined by an anchor and a cursor.
///
/// The anchor is where the selection started, and the cursor is where it ends.
/// The anchor may be after the cursor (reverse selection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// The starting point of the selection.
    pub anchor: Cursor,
    /// The current end of the selection (where the cursor is).
    pub cursor: Cursor,
}

impl Selection {
    /// Create a new selection.
    #[must_use]
    pub fn new(anchor: Cursor, cursor: Cursor) -> Self {
        Self { anchor, cursor }
    }

    /// Create a collapsed selection (no text selected) at the given cursor.
    #[must_use]
    pub fn collapsed(cursor: Cursor) -> Self {
        Self {
            anchor: cursor,
            cursor,
        }
    }

    /// Returns true if no text is selected (anchor == cursor).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }

    /// Returns (start, end) positions regardless of selection direction.
    #[must_use]
    pub fn ordered(&self) -> (Cursor, Cursor) {
        if self.anchor <= self.cursor {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }

    /// Extract the selected text from the buffer.
    /// Returns `None` if the selection is empty or positions are invalid.
    #[must_use]
    pub fn selected_text(&self, buffer: &Buffer) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let (start, end) = self.ordered();
        let start_offset = start.to_char_offset(buffer);
        let end_offset = end.to_char_offset(buffer);
        buffer.slice(start_offset..end_offset).ok()
    }
}

impl Default for Selection {
    fn default() -> Self {
        Self::collapsed(Cursor::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collapsed_selection() {
        let sel = Selection::collapsed(Cursor::new(0, 0));
        assert!(sel.is_empty());
    }

    #[test]
    fn test_non_empty_selection() {
        let sel = Selection::new(Cursor::new(0, 0), Cursor::new(0, 5));
        assert!(!sel.is_empty());
    }

    #[test]
    fn test_ordered_forward() {
        let sel = Selection::new(Cursor::new(0, 0), Cursor::new(0, 5));
        let (start, end) = sel.ordered();
        assert_eq!(start, Cursor::new(0, 0));
        assert_eq!(end, Cursor::new(0, 5));
    }

    #[test]
    fn test_ordered_reverse() {
        let sel = Selection::new(Cursor::new(0, 5), Cursor::new(0, 0));
        let (start, end) = sel.ordered();
        assert_eq!(start, Cursor::new(0, 0));
        assert_eq!(end, Cursor::new(0, 5));
    }

    #[test]
    fn test_selected_text() {
        let buf = Buffer::from_text("hello world");
        let sel = Selection::new(Cursor::new(0, 0), Cursor::new(0, 5));
        assert_eq!(sel.selected_text(&buf), Some("hello".to_string()));
    }

    #[test]
    fn test_selected_text_empty() {
        let buf = Buffer::from_text("hello");
        let sel = Selection::collapsed(Cursor::new(0, 0));
        assert_eq!(sel.selected_text(&buf), None);
    }

    #[test]
    fn test_selected_text_multiline() {
        let buf = Buffer::from_text("hello\nworld");
        let sel = Selection::new(Cursor::new(0, 3), Cursor::new(1, 2));
        // offset 3 to offset 8 ("lo\nwo")
        assert_eq!(sel.selected_text(&buf), Some("lo\nwo".to_string()));
    }

    #[test]
    fn test_default() {
        let sel = Selection::default();
        assert!(sel.is_empty());
        assert_eq!(sel.cursor, Cursor::new(0, 0));
    }
}
