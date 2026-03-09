//! rira-editor: Text buffer, cursor, selection, undo/redo

pub mod buffer;
pub mod cursor;
pub mod history;
pub mod selection;

pub use buffer::{Buffer, BufferError};
pub use cursor::Cursor;
pub use history::{EditOperation, History};
pub use selection::Selection;

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The main editor struct tying buffer, cursor, selection, and history together.
#[derive(Debug, Clone)]
pub struct Editor {
    /// The text buffer.
    pub buffer: Buffer,
    /// The cursor position.
    pub cursor: Cursor,
    /// The current selection.
    pub selection: Selection,
    /// The undo/redo history.
    pub history: History,
}

impl Editor {
    /// Create a new editor with an empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            cursor: Cursor::default(),
            selection: Selection::default(),
            history: History::new(),
        }
    }

    /// Create an editor from a string.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        Self {
            buffer: Buffer::from_text(text),
            cursor: Cursor::default(),
            selection: Selection::default(),
            history: History::new(),
        }
    }

    /// Insert a character at the cursor position.
    ///
    /// # Errors
    /// Returns a `BufferError` if the position is invalid.
    pub fn insert_char(&mut self, ch: char) -> Result<(), BufferError> {
        self.delete_selection_if_any()?;
        let pos = self.cursor.to_char_offset(&self.buffer);
        let text = ch.to_string();
        self.buffer.insert(pos, &text)?;
        self.history.push(EditOperation::Insert { pos, text });
        self.cursor.move_right(&self.buffer);
        self.collapse_selection();
        Ok(())
    }

    /// Delete the character after the cursor (like the Delete key).
    ///
    /// # Errors
    /// Returns a `BufferError` if the operation fails.
    pub fn delete_char(&mut self) -> Result<(), BufferError> {
        if !self.selection.is_empty() {
            return self.delete_selection_if_any();
        }
        let pos = self.cursor.to_char_offset(&self.buffer);
        if pos >= self.buffer.char_count() {
            return Ok(());
        }
        let deleted = self.buffer.delete(pos..pos + 1)?;
        self.history
            .push(EditOperation::Delete { pos, text: deleted });
        self.cursor.clamp_to_buffer(&self.buffer);
        self.collapse_selection();
        Ok(())
    }

    /// Delete the character before the cursor (backspace).
    ///
    /// # Errors
    /// Returns a `BufferError` if the operation fails.
    pub fn backspace(&mut self) -> Result<(), BufferError> {
        if !self.selection.is_empty() {
            return self.delete_selection_if_any();
        }
        let pos = self.cursor.to_char_offset(&self.buffer);
        if pos == 0 {
            return Ok(());
        }
        self.cursor.move_left(&self.buffer);
        let new_pos = self.cursor.to_char_offset(&self.buffer);
        let deleted = self.buffer.delete(new_pos..new_pos + 1)?;
        self.history.push(EditOperation::Delete {
            pos: new_pos,
            text: deleted,
        });
        self.cursor.clamp_to_buffer(&self.buffer);
        self.collapse_selection();
        Ok(())
    }

    /// Insert a newline at the cursor position.
    ///
    /// # Errors
    /// Returns a `BufferError` if the position is invalid.
    pub fn newline(&mut self) -> Result<(), BufferError> {
        self.delete_selection_if_any()?;
        let pos = self.cursor.to_char_offset(&self.buffer);
        self.buffer.insert(pos, "\n")?;
        self.history.push(EditOperation::Insert {
            pos,
            text: "\n".to_string(),
        });
        self.history.break_group();
        self.history.enable_grouping();
        // Move cursor to start of next line
        self.cursor.line += 1;
        self.cursor.col = 0;
        self.collapse_selection();
        Ok(())
    }

    /// Undo the last operation.
    ///
    /// # Errors
    /// Returns a `BufferError` if undo fails.
    pub fn undo(&mut self) -> Result<bool, BufferError> {
        let result = self.history.undo(&mut self.buffer)?;
        self.cursor.clamp_to_buffer(&self.buffer);
        self.collapse_selection();
        Ok(result)
    }

    /// Redo the last undone operation.
    ///
    /// # Errors
    /// Returns a `BufferError` if redo fails.
    pub fn redo(&mut self) -> Result<bool, BufferError> {
        let result = self.history.redo(&mut self.buffer)?;
        self.cursor.clamp_to_buffer(&self.buffer);
        self.collapse_selection();
        Ok(result)
    }

    /// Select all text in the buffer.
    pub fn select_all(&mut self) {
        self.selection.anchor = Cursor::new(0, 0);
        let last_line = self.buffer.line_count().saturating_sub(1);
        let last_col = self.buffer.line_len_chars(last_line);
        // For the last line, there's no trailing newline so line_len_chars is the content length
        self.selection.cursor = Cursor::new(last_line, last_col);
        self.cursor = self.selection.cursor;
    }

    /// Copy the selected text. Returns `None` if no selection.
    #[must_use]
    pub fn copy(&self) -> Option<String> {
        self.selection.selected_text(&self.buffer)
    }

    /// Paste text at the cursor position, replacing selection if any.
    ///
    /// # Errors
    /// Returns a `BufferError` if the operation fails.
    pub fn paste(&mut self, text: &str) -> Result<(), BufferError> {
        self.delete_selection_if_any()?;
        let pos = self.cursor.to_char_offset(&self.buffer);
        self.buffer.insert(pos, text)?;
        self.history.break_group();
        self.history.push(EditOperation::Insert {
            pos,
            text: text.to_string(),
        });
        self.history.break_group();
        self.history.enable_grouping();

        // Move cursor to end of pasted text
        // Calculate new position by advancing through the pasted text
        let new_offset = pos + text.chars().count();
        self.set_cursor_from_offset(new_offset);
        self.collapse_selection();
        Ok(())
    }

    /// Helper: delete the current selection if any.
    fn delete_selection_if_any(&mut self) -> Result<(), BufferError> {
        if self.selection.is_empty() {
            return Ok(());
        }
        let (start, end) = self.selection.ordered();
        let start_offset = start.to_char_offset(&self.buffer);
        let end_offset = end.to_char_offset(&self.buffer);
        if start_offset < end_offset {
            let deleted = self.buffer.delete(start_offset..end_offset)?;
            self.history.break_group();
            self.history.push(EditOperation::Delete {
                pos: start_offset,
                text: deleted,
            });
            self.history.break_group();
            self.history.enable_grouping();
        }
        self.cursor = start;
        self.cursor.clamp_to_buffer(&self.buffer);
        self.collapse_selection();
        Ok(())
    }

    /// Collapse the selection to the cursor position.
    fn collapse_selection(&mut self) {
        self.selection = Selection::collapsed(self.cursor);
    }

    /// Set the cursor position from a character offset.
    fn set_cursor_from_offset(&mut self, offset: usize) {
        let offset = offset.min(self.buffer.char_count());
        // Find line/col from offset
        let mut remaining = offset;
        let line_count = self.buffer.line_count();
        for line_idx in 0..line_count {
            let line_len = self.buffer.line_len_chars(line_idx);
            if remaining <= line_len.saturating_sub(1) || line_idx == line_count - 1 {
                // Check if this is the last line (no trailing newline)
                let content_len = if line_idx < line_count - 1 {
                    line_len.saturating_sub(1) // exclude \n
                } else {
                    line_len
                };
                self.cursor.line = line_idx;
                self.cursor.col = remaining.min(content_len);
                return;
            }
            remaining = remaining.saturating_sub(line_len);
        }
        // Fallback: end of buffer
        self.cursor.clamp_to_buffer(&self.buffer);
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(version(), "0.1.0");
    }

    #[test]
    fn test_editor_new() {
        let ed = Editor::new();
        assert_eq!(ed.buffer.char_count(), 0);
        assert_eq!(ed.cursor, Cursor::new(0, 0));
    }

    #[test]
    fn test_insert_char() {
        let mut ed = Editor::new();
        ed.insert_char('h').expect("insert should succeed");
        ed.insert_char('i').expect("insert should succeed");
        assert_eq!(ed.buffer.to_string(), "hi");
        assert_eq!(ed.cursor, Cursor::new(0, 2));
    }

    #[test]
    fn test_delete_char() {
        let mut ed = Editor::from_text("hello");
        ed.cursor = Cursor::new(0, 0);
        ed.delete_char().expect("delete should succeed");
        assert_eq!(ed.buffer.to_string(), "ello");
    }

    #[test]
    fn test_backspace() {
        let mut ed = Editor::from_text("hello");
        ed.cursor = Cursor::new(0, 5);
        ed.collapse_selection();
        ed.backspace().expect("backspace should succeed");
        assert_eq!(ed.buffer.to_string(), "hell");
        assert_eq!(ed.cursor, Cursor::new(0, 4));
    }

    #[test]
    fn test_backspace_at_start() {
        let mut ed = Editor::from_text("hello");
        ed.cursor = Cursor::new(0, 0);
        ed.collapse_selection();
        ed.backspace().expect("backspace should succeed");
        assert_eq!(ed.buffer.to_string(), "hello");
    }

    #[test]
    fn test_newline() {
        let mut ed = Editor::from_text("hello");
        ed.cursor = Cursor::new(0, 5);
        ed.collapse_selection();
        ed.newline().expect("newline should succeed");
        assert_eq!(ed.buffer.to_string(), "hello\n");
        assert_eq!(ed.cursor, Cursor::new(1, 0));
    }

    #[test]
    fn test_undo_redo() {
        let mut ed = Editor::new();
        ed.insert_char('a').expect("insert should succeed");
        ed.insert_char('b').expect("insert should succeed");
        ed.insert_char('c').expect("insert should succeed");
        assert_eq!(ed.buffer.to_string(), "abc");

        ed.undo().expect("undo should succeed");
        assert_eq!(ed.buffer.to_string(), "");

        ed.redo().expect("redo should succeed");
        assert_eq!(ed.buffer.to_string(), "abc");
    }

    #[test]
    fn test_select_all() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.select_all();
        assert_eq!(ed.copy(), Some("hello\nworld".to_string()));
    }

    #[test]
    fn test_copy_no_selection() {
        let ed = Editor::from_text("hello");
        assert_eq!(ed.copy(), None);
    }

    #[test]
    fn test_paste() {
        let mut ed = Editor::from_text("hd");
        ed.cursor = Cursor::new(0, 1);
        ed.collapse_selection();
        ed.paste("ello worl").expect("paste should succeed");
        assert_eq!(ed.buffer.to_string(), "hello world");
    }

    #[test]
    fn test_paste_replaces_selection() {
        let mut ed = Editor::from_text("hello world");
        ed.selection = Selection::new(Cursor::new(0, 5), Cursor::new(0, 11));
        ed.cursor = Cursor::new(0, 11);
        ed.paste("!").expect("paste should succeed");
        assert_eq!(ed.buffer.to_string(), "hello!");
    }

    #[test]
    fn test_delete_with_selection() {
        let mut ed = Editor::from_text("hello world");
        ed.selection = Selection::new(Cursor::new(0, 5), Cursor::new(0, 11));
        ed.cursor = Cursor::new(0, 11);
        ed.delete_char().expect("delete should succeed");
        assert_eq!(ed.buffer.to_string(), "hello");
    }

    #[test]
    fn test_backspace_with_selection() {
        let mut ed = Editor::from_text("hello world");
        ed.selection = Selection::new(Cursor::new(0, 0), Cursor::new(0, 5));
        ed.cursor = Cursor::new(0, 5);
        ed.backspace().expect("backspace should succeed");
        assert_eq!(ed.buffer.to_string(), " world");
    }

    #[test]
    fn test_insert_char_replaces_selection() {
        let mut ed = Editor::from_text("hello");
        ed.selection = Selection::new(Cursor::new(0, 0), Cursor::new(0, 5));
        ed.cursor = Cursor::new(0, 5);
        ed.insert_char('!').expect("insert should succeed");
        assert_eq!(ed.buffer.to_string(), "!");
    }

    #[test]
    fn test_newline_in_middle() {
        let mut ed = Editor::from_text("helloworld");
        ed.cursor = Cursor::new(0, 5);
        ed.collapse_selection();
        ed.newline().expect("newline should succeed");
        assert_eq!(ed.buffer.to_string(), "hello\nworld");
        assert_eq!(ed.cursor, Cursor::new(1, 0));
    }

    #[test]
    fn test_undo_paste() {
        let mut ed = Editor::new();
        ed.paste("hello world").expect("paste should succeed");
        assert_eq!(ed.buffer.to_string(), "hello world");
        ed.undo().expect("undo should succeed");
        assert_eq!(ed.buffer.to_string(), "");
    }

    #[test]
    fn test_delete_at_end() {
        let mut ed = Editor::from_text("hi");
        ed.cursor = Cursor::new(0, 2);
        ed.collapse_selection();
        ed.delete_char().expect("delete should succeed");
        assert_eq!(ed.buffer.to_string(), "hi");
    }

    #[test]
    fn test_editor_default() {
        let ed = Editor::default();
        assert_eq!(ed.buffer.char_count(), 0);
    }

    #[test]
    fn test_insert_char_updates_buffer() {
        let mut ed = Editor::new();
        ed.insert_char('a').expect("insert should succeed");
        assert_eq!(ed.buffer.to_string(), "a");
        ed.insert_char('b').expect("insert should succeed");
        assert_eq!(ed.buffer.to_string(), "ab");
        ed.insert_char('c').expect("insert should succeed");
        assert_eq!(ed.buffer.to_string(), "abc");
        assert_eq!(ed.buffer.char_count(), 3);
    }

    #[test]
    fn test_backspace_removes_char() {
        let mut ed = Editor::new();
        ed.insert_char('a').expect("insert should succeed");
        ed.insert_char('b').expect("insert should succeed");
        ed.insert_char('c').expect("insert should succeed");
        assert_eq!(ed.buffer.to_string(), "abc");

        ed.backspace().expect("backspace should succeed");
        assert_eq!(ed.buffer.to_string(), "ab");

        ed.backspace().expect("backspace should succeed");
        assert_eq!(ed.buffer.to_string(), "a");
    }

    #[test]
    fn test_enter_inserts_newline() {
        let mut ed = Editor::new();
        ed.insert_char('a').expect("insert should succeed");
        ed.newline().expect("newline should succeed");
        ed.insert_char('b').expect("insert should succeed");
        assert_eq!(ed.buffer.to_string(), "a\nb");
        assert_eq!(ed.buffer.line_count(), 2);
        assert_eq!(ed.cursor.line, 1);
        assert_eq!(ed.cursor.col, 1);
    }

    #[test]
    fn test_delete_removes_char_after_cursor() {
        let mut ed = Editor::from_text("abc");
        ed.cursor = Cursor::new(0, 0);
        ed.collapse_selection();
        ed.delete_char().expect("delete should succeed");
        assert_eq!(ed.buffer.to_string(), "bc");

        ed.delete_char().expect("delete should succeed");
        assert_eq!(ed.buffer.to_string(), "c");

        ed.delete_char().expect("delete should succeed");
        assert_eq!(ed.buffer.to_string(), "");
    }

    #[test]
    fn test_sequential_typing() {
        let mut ed = Editor::new();
        let text = "Hello, world!";
        for ch in text.chars() {
            ed.insert_char(ch).expect("insert should succeed");
        }
        assert_eq!(ed.buffer.to_string(), "Hello, world!");
        assert_eq!(ed.cursor, Cursor::new(0, 13));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy to generate a random ASCII string for testing.
    fn ascii_text() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 \n]{0,100}"
    }

    proptest! {
        #[test]
        fn buffer_insert_delete_consistency(text in ascii_text()) {
            let mut buf = Buffer::new();
            // Insert text at position 0
            buf.insert(0, &text).expect("insert should succeed");
            assert_eq!(buf.to_string(), text);
            assert_eq!(buf.char_count(), text.chars().count());

            // Delete all text
            if buf.char_count() > 0 {
                buf.delete(0..buf.char_count()).expect("delete should succeed");
                assert_eq!(buf.to_string(), "");
                assert_eq!(buf.char_count(), 0);
            }
        }

        #[test]
        fn buffer_random_edits_stay_consistent(
            initial in ascii_text(),
            insertions in proptest::collection::vec(
                (0..100usize, "[a-z]{1,5}"),
                0..10
            )
        ) {
            let mut buf = Buffer::from_text(&initial);

            for (pos_raw, text) in &insertions {
                let len = buf.char_count();
                if len == 0 {
                    let _ = buf.insert(0, text);
                } else {
                    let pos = pos_raw % (len + 1);
                    let _ = buf.insert(pos, text);
                }

                // Invariant: char_count matches the string length
                let s = buf.to_string();
                assert_eq!(buf.char_count(), s.chars().count());
            }
        }

        #[test]
        fn undo_redo_roundtrip(text in "[a-zA-Z]{1,20}") {
            let mut ed = Editor::new();

            // Type each character
            for ch in text.chars() {
                ed.insert_char(ch).expect("insert should succeed");
            }
            assert_eq!(ed.buffer.to_string(), text);

            // Undo all
            ed.undo().expect("undo should succeed");
            assert_eq!(ed.buffer.to_string(), "");

            // Redo all
            ed.redo().expect("redo should succeed");
            assert_eq!(ed.buffer.to_string(), text);
        }

        #[test]
        fn undo_redo_with_paste(text in "[a-zA-Z ]{1,30}") {
            let mut ed = Editor::new();
            ed.paste(&text).expect("paste should succeed");
            assert_eq!(ed.buffer.to_string(), text);

            ed.undo().expect("undo should succeed");
            assert_eq!(ed.buffer.to_string(), "");

            ed.redo().expect("redo should succeed");
            assert_eq!(ed.buffer.to_string(), text);
        }

        #[test]
        fn cursor_always_valid_after_operations(
            initial in "[a-zA-Z\n]{0,50}",
            ops in proptest::collection::vec(
                prop_oneof![
                    Just("left"),
                    Just("right"),
                    Just("up"),
                    Just("down"),
                    Just("home"),
                    Just("end"),
                ],
                0..20
            )
        ) {
            let mut ed = Editor::from_text(&initial);

            for op in &ops {
                match *op {
                    "left" => ed.cursor.move_left(&ed.buffer),
                    "right" => ed.cursor.move_right(&ed.buffer),
                    "up" => ed.cursor.move_up(&ed.buffer),
                    "down" => ed.cursor.move_down(&ed.buffer),
                    "home" => ed.cursor.move_to_line_start(),
                    "end" => ed.cursor.move_to_line_end(&ed.buffer),
                    _ => {}
                }

                // Cursor must always be within bounds
                assert!(ed.cursor.line < ed.buffer.line_count() || ed.buffer.line_count() == 0);
                let offset = ed.cursor.to_char_offset(&ed.buffer);
                assert!(offset <= ed.buffer.char_count());
            }
        }
    }
}
