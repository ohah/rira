//! rira-editor: Text buffer, cursor, selection, undo/redo

pub mod buffer;
pub mod cursor;
pub mod history;
pub mod hit_test;
pub mod selection;
pub mod viewport;

pub use buffer::{Buffer, BufferError};
pub use cursor::Cursor;
pub use history::{EditOperation, History};
pub use hit_test::{HitTestConfig, HitTestResult};
pub use selection::Selection;
pub use viewport::Viewport;

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
    /// The file path this editor is associated with.
    file_path: Option<std::path::PathBuf>,
    /// Whether the buffer has been modified since last save.
    // TODO: Track save-point to correctly reset modified state after undo to saved state
    modified: bool,
    /// The viewport (scroll offset and visible lines).
    pub viewport: Viewport,
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
            file_path: None,
            modified: false,
            viewport: Viewport::new(),
        }
    }

    /// Create an editor by loading a file.
    ///
    /// # Errors
    /// Returns an `io::Error` if the file cannot be read.
    pub fn from_file(path: &std::path::Path) -> std::io::Result<Self> {
        let buffer = Buffer::from_file(path)?;
        Ok(Self {
            buffer,
            cursor: Cursor::default(),
            selection: Selection::default(),
            history: History::new(),
            file_path: Some(path.to_path_buf()),
            modified: false,
            viewport: Viewport::new(),
        })
    }

    /// Create an editor from a string.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        Self {
            buffer: Buffer::from_text(text),
            cursor: Cursor::default(),
            selection: Selection::default(),
            history: History::new(),
            file_path: None,
            modified: false,
            viewport: Viewport::new(),
        }
    }

    /// Set cursor to the beginning of a specific line (0-indexed), clamping to valid range.
    pub fn set_cursor_line(&mut self, line: usize) {
        let max_line = self.buffer.line_count().saturating_sub(1);
        self.cursor = Cursor::new(line.min(max_line), 0);
        self.selection = Selection::collapsed(self.cursor);
    }

    /// Save the buffer to the current file path.
    ///
    /// # Errors
    /// Returns an `io::Error` if no file path is set or if the file cannot be written.
    pub fn save(&mut self) -> std::io::Result<()> {
        // Clone needed because self.buffer.save() borrows self mutably via modified = false
        let path = self
            .file_path
            .clone()
            .ok_or_else(|| std::io::Error::other("no file path set"))?;
        self.buffer.save(&path)?;
        self.modified = false;
        Ok(())
    }

    /// Save the buffer to a specific path and update the file path.
    ///
    /// # Errors
    /// Returns an `io::Error` if the file cannot be written.
    pub fn save_as(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        self.buffer.save(path)?;
        self.file_path = Some(path.to_path_buf());
        self.modified = false;
        Ok(())
    }

    /// Returns just the filename portion of the file path, if any.
    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
    }

    /// Returns the full file path, if any.
    #[must_use]
    pub fn file_path(&self) -> Option<&std::path::Path> {
        self.file_path.as_deref()
    }

    /// Returns whether the buffer has been modified since last save.
    // TODO: Track save-point to correctly reset modified state after undo to saved state
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.modified
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
        self.modified = true;
        self.viewport.ensure_cursor_visible(self.cursor.line);
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
        self.modified = true;
        self.viewport.ensure_cursor_visible(self.cursor.line);
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
        self.modified = true;
        self.viewport.ensure_cursor_visible(self.cursor.line);
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
        self.modified = true;
        self.viewport.ensure_cursor_visible(self.cursor.line);
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
        self.viewport.ensure_cursor_visible(self.cursor.line);
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
        self.viewport.ensure_cursor_visible(self.cursor.line);
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
        self.modified = true;
        self.viewport.ensure_cursor_visible(self.cursor.line);
        Ok(())
    }

    /// Cut the selected text. Returns the cut text, or `None` if no selection.
    ///
    /// # Errors
    /// Returns a `BufferError` if the delete operation fails.
    pub fn cut(&mut self) -> Result<Option<String>, BufferError> {
        let text = self.copy();
        if text.is_some() {
            self.delete_selection_if_any()?;
        }
        Ok(text)
    }

    /// Move cursor left by one character.
    pub fn cursor_left(&mut self) {
        self.cursor.move_left(&self.buffer);
        self.collapse_selection();
        self.viewport.ensure_cursor_visible(self.cursor.line);
    }

    /// Move cursor right by one character.
    pub fn cursor_right(&mut self) {
        self.cursor.move_right(&self.buffer);
        self.collapse_selection();
        self.viewport.ensure_cursor_visible(self.cursor.line);
    }

    /// Move cursor up by one line.
    pub fn cursor_up(&mut self) {
        self.cursor.move_up(&self.buffer);
        self.collapse_selection();
        self.viewport.ensure_cursor_visible(self.cursor.line);
    }

    /// Move cursor down by one line.
    pub fn cursor_down(&mut self) {
        self.cursor.move_down(&self.buffer);
        self.collapse_selection();
        self.viewport.ensure_cursor_visible(self.cursor.line);
    }

    /// Move cursor to the start of the current line.
    pub fn move_to_line_start(&mut self) {
        self.cursor.move_to_line_start();
        self.collapse_selection();
        self.viewport.ensure_cursor_visible(self.cursor.line);
    }

    /// Move cursor to the end of the current line.
    pub fn move_to_line_end(&mut self) {
        self.cursor.move_to_line_end(&self.buffer);
        self.collapse_selection();
        self.viewport.ensure_cursor_visible(self.cursor.line);
    }

    /// Move cursor to the given line and column, clamping to buffer bounds.
    ///
    /// This collapses any active selection.
    pub fn move_cursor_to(&mut self, line: usize, col: usize) {
        self.cursor = Cursor::new(line, col);
        self.cursor.clamp_to_buffer(&self.buffer);
        self.collapse_selection();
    }

    /// Move cursor left, extending selection.
    pub fn select_left(&mut self) {
        if self.selection.is_empty() {
            self.selection.anchor = self.cursor;
        }
        self.cursor.move_left(&self.buffer);
        self.selection.cursor = self.cursor;
    }

    /// Move cursor right, extending selection.
    pub fn select_right(&mut self) {
        if self.selection.is_empty() {
            self.selection.anchor = self.cursor;
        }
        self.cursor.move_right(&self.buffer);
        self.selection.cursor = self.cursor;
    }

    /// Move cursor up, extending selection.
    pub fn select_up(&mut self) {
        if self.selection.is_empty() {
            self.selection.anchor = self.cursor;
        }
        self.cursor.move_up(&self.buffer);
        self.selection.cursor = self.cursor;
    }

    /// Move cursor down, extending selection.
    pub fn select_down(&mut self) {
        if self.selection.is_empty() {
            self.selection.anchor = self.cursor;
        }
        self.cursor.move_down(&self.buffer);
        self.selection.cursor = self.cursor;
    }

    /// Select from cursor to line start (Shift+Home).
    pub fn select_to_line_start(&mut self) {
        if self.selection.is_empty() {
            self.selection.anchor = self.cursor;
        }
        self.cursor.move_to_line_start();
        self.selection.cursor = self.cursor;
    }

    /// Select from cursor to line end (Shift+End).
    pub fn select_to_line_end(&mut self) {
        if self.selection.is_empty() {
            self.selection.anchor = self.cursor;
        }
        self.cursor.move_to_line_end(&self.buffer);
        self.selection.cursor = self.cursor;
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
            self.modified = true;
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

    #[test]
    fn test_editor_cursor_left() {
        let mut ed = Editor::from_text("hello");
        ed.cursor = Cursor::new(0, 3);
        ed.collapse_selection();
        ed.cursor_left();
        assert_eq!(ed.cursor, Cursor::new(0, 2));
        assert!(ed.selection.is_empty());
    }

    #[test]
    fn test_editor_cursor_right() {
        let mut ed = Editor::from_text("hello");
        ed.cursor = Cursor::new(0, 3);
        ed.collapse_selection();
        ed.cursor_right();
        assert_eq!(ed.cursor, Cursor::new(0, 4));
        assert!(ed.selection.is_empty());
    }

    #[test]
    fn test_editor_cursor_up() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.cursor = Cursor::new(1, 3);
        ed.collapse_selection();
        ed.cursor_up();
        assert_eq!(ed.cursor, Cursor::new(0, 3));
    }

    #[test]
    fn test_editor_cursor_down() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.cursor = Cursor::new(0, 3);
        ed.collapse_selection();
        ed.cursor_down();
        assert_eq!(ed.cursor, Cursor::new(1, 3));
    }

    #[test]
    fn test_editor_move_to_line_start() {
        let mut ed = Editor::from_text("hello");
        ed.cursor = Cursor::new(0, 4);
        ed.collapse_selection();
        ed.move_to_line_start();
        assert_eq!(ed.cursor, Cursor::new(0, 0));
    }

    #[test]
    fn test_editor_move_to_line_end() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.cursor = Cursor::new(0, 0);
        ed.collapse_selection();
        ed.move_to_line_end();
        assert_eq!(ed.cursor, Cursor::new(0, 5));
    }

    #[test]
    fn test_from_file_nonexistent() {
        let result = Editor::from_file(std::path::Path::new("/tmp/rira_nonexistent_test_file.rs"));
        assert!(result.is_err());
    }

    #[test]
    fn test_from_file_roundtrip() {
        let dir = std::env::temp_dir().join("rira_test_from_file");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("test.txt");
        std::fs::write(&path, "hello\nworld\n").expect("write test file");

        let ed = Editor::from_file(&path).expect("should open file");
        assert_eq!(ed.buffer.to_string(), "hello\nworld\n");
        assert_eq!(ed.cursor, Cursor::new(0, 0));
        assert!(ed.selection.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_set_cursor_line() {
        let mut ed = Editor::from_text("line0\nline1\nline2\nline3");
        ed.set_cursor_line(2);
        assert_eq!(ed.cursor, Cursor::new(2, 0));
        assert!(ed.selection.is_empty());
    }

    #[test]
    fn test_move_cursor_to() {
        let mut ed = Editor::from_text("hello\nworld\nfoo");
        ed.move_cursor_to(1, 3);
        assert_eq!(ed.cursor, Cursor::new(1, 3));
        assert!(ed.selection.is_empty());
    }

    #[test]
    fn test_set_cursor_line_clamps() {
        let mut ed = Editor::from_text("line0\nline1");
        ed.set_cursor_line(100);
        assert_eq!(ed.cursor, Cursor::new(1, 0));
        assert!(ed.selection.is_empty());
    }

    #[test]
    fn test_set_cursor_line_zero() {
        let mut ed = Editor::from_text("line0\nline1");
        ed.set_cursor_line(0);
        assert_eq!(ed.cursor, Cursor::new(0, 0));
        assert!(ed.selection.is_empty());
    }

    #[test]
    fn test_set_cursor_line_initializes_selection() {
        let mut ed = Editor::from_text("aaa\nbbb\nccc");
        // Set a non-trivial selection first
        ed.selection = Selection::new(Cursor::new(0, 0), Cursor::new(1, 3));
        ed.set_cursor_line(2);
        // Selection should be collapsed to the new cursor position
        assert_eq!(ed.selection, Selection::collapsed(Cursor::new(2, 0)));
    }

    #[test]
    fn test_cut_with_selection() {
        let mut ed = Editor::from_text("hello world");
        ed.selection = Selection::new(Cursor::new(0, 0), Cursor::new(0, 5));
        ed.cursor = Cursor::new(0, 5);
        let result = ed.cut().expect("cut should succeed");
        assert_eq!(result, Some("hello".to_string()));
        assert_eq!(ed.buffer.to_string(), " world");
        assert!(ed.selection.is_empty());
    }

    #[test]
    fn test_cut_no_selection() {
        let mut ed = Editor::from_text("hello");
        let result = ed.cut().expect("cut should succeed");
        assert_eq!(result, None);
        assert_eq!(ed.buffer.to_string(), "hello");
    }

    #[test]
    fn test_cut_multiline_selection() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.selection = Selection::new(Cursor::new(0, 3), Cursor::new(1, 2));
        ed.cursor = Cursor::new(1, 2);
        let result = ed.cut().expect("cut should succeed");
        assert_eq!(result, Some("lo\nwo".to_string()));
        assert_eq!(ed.buffer.to_string(), "helrld");
    }

    #[test]
    fn test_cut_all_text() {
        let mut ed = Editor::from_text("hello");
        ed.select_all();
        let result = ed.cut().expect("cut should succeed");
        assert_eq!(result, Some("hello".to_string()));
        assert_eq!(ed.buffer.to_string(), "");
        assert_eq!(ed.cursor, Cursor::new(0, 0));
    }

    #[test]
    fn test_move_cursor_to_clamps_line() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.move_cursor_to(100, 0);
        assert_eq!(ed.cursor, Cursor::new(1, 0));
    }

    #[test]
    fn test_move_cursor_to_clamps_col() {
        let mut ed = Editor::from_text("hi\nworld");
        ed.move_cursor_to(0, 100);
        assert_eq!(ed.cursor, Cursor::new(0, 2));
    }

    #[test]
    fn test_move_cursor_to_collapses_selection() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.selection = Selection::new(Cursor::new(0, 0), Cursor::new(0, 5));
        ed.move_cursor_to(1, 2);
        assert!(ed.selection.is_empty());
        assert_eq!(ed.cursor, Cursor::new(1, 2));
    }

    #[test]
    fn test_move_cursor_to_empty_buffer() {
        let mut ed = Editor::new();
        ed.move_cursor_to(5, 5);
        assert_eq!(ed.cursor, Cursor::new(0, 0));
    }

    #[test]
    fn test_editor_cursor_movement_collapses_selection() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.selection = Selection::new(Cursor::new(0, 0), Cursor::new(0, 5));
        ed.cursor = Cursor::new(0, 5);
        ed.cursor_right();
        assert!(ed.selection.is_empty());
    }

    #[test]
    fn test_from_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("rira_test_editor_from_file.txt");
        std::fs::write(&path, "hello from file").expect("write should succeed");
        let ed = Editor::from_file(&path).expect("from_file should succeed");
        assert_eq!(ed.buffer.to_string(), "hello from file");
        assert_eq!(ed.file_path(), Some(path.as_path()));
        assert!(!ed.is_modified());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_and_save_as() {
        let dir = std::env::temp_dir();
        let path1 = dir.join("rira_test_editor_save1.txt");
        let path2 = dir.join("rira_test_editor_save2.txt");

        // save_as sets the path
        let mut ed = Editor::from_text("content");
        ed.insert_char('!').expect("insert should succeed");
        assert!(ed.is_modified());
        ed.save_as(&path1).expect("save_as should succeed");
        assert!(!ed.is_modified());
        assert_eq!(ed.file_path(), Some(path1.as_path()));

        // save uses the existing path
        ed.insert_char('?').expect("insert should succeed");
        assert!(ed.is_modified());
        ed.save().expect("save should succeed");
        assert!(!ed.is_modified());

        // save_as to a different path updates the path
        ed.save_as(&path2).expect("save_as should succeed");
        assert_eq!(ed.file_path(), Some(path2.as_path()));

        let _ = std::fs::remove_file(&path1);
        let _ = std::fs::remove_file(&path2);
    }

    #[test]
    fn test_save_without_path_fails() {
        let mut ed = Editor::from_text("content");
        let result = ed.save();
        assert!(result.is_err());
    }

    #[test]
    fn test_file_name() {
        let dir = std::env::temp_dir();
        let path = dir.join("rira_test_file_name.txt");
        std::fs::write(&path, "hello").expect("write should succeed");
        let ed = Editor::from_file(&path).expect("from_file should succeed");
        assert_eq!(ed.file_name(), Some("rira_test_file_name.txt"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_file_name_none_for_new() {
        let ed = Editor::new();
        assert_eq!(ed.file_name(), None);
    }

    #[test]
    fn test_is_modified_tracking() {
        let mut ed = Editor::new();
        assert!(!ed.is_modified());

        ed.insert_char('a').expect("insert should succeed");
        assert!(ed.is_modified());

        let dir = std::env::temp_dir();
        let path = dir.join("rira_test_modified_tracking.txt");
        ed.save_as(&path).expect("save_as should succeed");
        assert!(!ed.is_modified());

        ed.backspace().expect("backspace should succeed");
        assert!(ed.is_modified());

        ed.save().expect("save should succeed");
        assert!(!ed.is_modified());

        ed.newline().expect("newline should succeed");
        assert!(ed.is_modified());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_is_modified_after_delete() {
        let mut ed = Editor::from_text("hello");
        assert!(!ed.is_modified());

        ed.cursor = Cursor::new(0, 0);
        ed.collapse_selection();
        ed.delete_char().expect("delete should succeed");
        assert!(ed.is_modified());
    }

    #[test]
    fn test_is_modified_after_paste() {
        let mut ed = Editor::new();
        assert!(!ed.is_modified());

        ed.paste("hello").expect("paste should succeed");
        assert!(ed.is_modified());
    }

    #[test]
    fn test_select_left_from_middle() {
        let mut ed = Editor::from_text("hello");
        ed.cursor = Cursor::new(0, 3);
        ed.collapse_selection();
        ed.select_left();
        assert_eq!(ed.cursor, Cursor::new(0, 2));
        assert_eq!(ed.selection.anchor, Cursor::new(0, 3));
        assert_eq!(ed.selection.cursor, Cursor::new(0, 2));
        assert!(!ed.selection.is_empty());
    }

    #[test]
    fn test_select_right_from_middle() {
        let mut ed = Editor::from_text("hello");
        ed.cursor = Cursor::new(0, 2);
        ed.collapse_selection();
        ed.select_right();
        assert_eq!(ed.cursor, Cursor::new(0, 3));
        assert_eq!(ed.selection.anchor, Cursor::new(0, 2));
        assert_eq!(ed.selection.cursor, Cursor::new(0, 3));
    }

    #[test]
    fn test_select_left_wraps_to_previous_line() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.cursor = Cursor::new(1, 0);
        ed.collapse_selection();
        ed.select_left();
        assert_eq!(ed.cursor, Cursor::new(0, 5));
        assert_eq!(ed.selection.anchor, Cursor::new(1, 0));
        assert_eq!(ed.selection.cursor, Cursor::new(0, 5));
    }

    #[test]
    fn test_select_right_wraps_to_next_line() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.cursor = Cursor::new(0, 5);
        ed.collapse_selection();
        ed.select_right();
        assert_eq!(ed.cursor, Cursor::new(1, 0));
        assert_eq!(ed.selection.anchor, Cursor::new(0, 5));
        assert_eq!(ed.selection.cursor, Cursor::new(1, 0));
    }

    #[test]
    fn test_select_up_preserves_column() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.cursor = Cursor::new(1, 3);
        ed.collapse_selection();
        ed.select_up();
        assert_eq!(ed.cursor, Cursor::new(0, 3));
        assert_eq!(ed.selection.anchor, Cursor::new(1, 3));
    }

    #[test]
    fn test_select_down_preserves_column() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.cursor = Cursor::new(0, 3);
        ed.collapse_selection();
        ed.select_down();
        assert_eq!(ed.cursor, Cursor::new(1, 3));
        assert_eq!(ed.selection.anchor, Cursor::new(0, 3));
    }

    #[test]
    fn test_select_to_line_start() {
        let mut ed = Editor::from_text("hello");
        ed.cursor = Cursor::new(0, 3);
        ed.collapse_selection();
        ed.select_to_line_start();
        assert_eq!(ed.cursor, Cursor::new(0, 0));
        assert_eq!(ed.selection.anchor, Cursor::new(0, 3));
        assert_eq!(ed.selection.cursor, Cursor::new(0, 0));
    }

    #[test]
    fn test_select_to_line_end() {
        let mut ed = Editor::from_text("hello\nworld");
        ed.cursor = Cursor::new(0, 2);
        ed.collapse_selection();
        ed.select_to_line_end();
        assert_eq!(ed.cursor, Cursor::new(0, 5));
        assert_eq!(ed.selection.anchor, Cursor::new(0, 2));
        assert_eq!(ed.selection.cursor, Cursor::new(0, 5));
    }

    #[test]
    fn test_multiple_select_operations_accumulate() {
        let mut ed = Editor::from_text("hello");
        ed.cursor = Cursor::new(0, 2);
        ed.collapse_selection();
        ed.select_right();
        ed.select_right();
        ed.select_right();
        // Anchor stays at original position, cursor moves
        assert_eq!(ed.selection.anchor, Cursor::new(0, 2));
        assert_eq!(ed.selection.cursor, Cursor::new(0, 5));
        assert_eq!(ed.cursor, Cursor::new(0, 5));
        assert_eq!(
            ed.selection.selected_text(&ed.buffer),
            Some("llo".to_string())
        );
    }

    #[test]
    fn test_select_then_regular_move_collapses() {
        let mut ed = Editor::from_text("hello");
        ed.cursor = Cursor::new(0, 2);
        ed.collapse_selection();
        ed.select_right();
        ed.select_right();
        assert!(!ed.selection.is_empty());
        // Regular move collapses selection
        ed.cursor_right();
        assert!(ed.selection.is_empty());
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
