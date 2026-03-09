//! Cursor position model.

use crate::buffer::Buffer;

/// A cursor position in the buffer, represented by line and column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    /// 0-indexed line number.
    pub line: usize,
    /// 0-indexed column (character offset within the line).
    pub col: usize,
}

impl Cursor {
    /// Create a new cursor at the given position.
    #[must_use]
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    /// Move cursor left by one character. Wraps to end of previous line if at column 0.
    pub fn move_left(&mut self, buffer: &Buffer) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.line > 0 {
            self.line -= 1;
            self.col = buffer.line_content_len(self.line);
        }
    }

    /// Move cursor right by one character. Wraps to start of next line if at end.
    pub fn move_right(&mut self, buffer: &Buffer) {
        let line_len = buffer.line_content_len(self.line);
        if self.col < line_len {
            self.col += 1;
        } else if self.line + 1 < buffer.line_count() {
            self.line += 1;
            self.col = 0;
        }
    }

    /// Move cursor up by one line.
    pub fn move_up(&mut self, buffer: &Buffer) {
        if self.line > 0 {
            self.line -= 1;
            let line_len = buffer.line_content_len(self.line);
            if self.col > line_len {
                self.col = line_len;
            }
        }
    }

    /// Move cursor down by one line.
    pub fn move_down(&mut self, buffer: &Buffer) {
        if self.line + 1 < buffer.line_count() {
            self.line += 1;
            let line_len = buffer.line_content_len(self.line);
            if self.col > line_len {
                self.col = line_len;
            }
        }
    }

    /// Move cursor to the start of the current line.
    pub fn move_to_line_start(&mut self) {
        self.col = 0;
    }

    /// Move cursor to the end of the current line.
    pub fn move_to_line_end(&mut self, buffer: &Buffer) {
        self.col = buffer.line_content_len(self.line);
    }

    /// Clamp cursor to valid buffer bounds.
    pub fn clamp_to_buffer(&mut self, buffer: &Buffer) {
        let max_line = if buffer.line_count() == 0 {
            0
        } else {
            buffer.line_count() - 1
        };
        if self.line > max_line {
            self.line = max_line;
        }
        let line_len = buffer.line_content_len(self.line);
        if self.col > line_len {
            self.col = line_len;
        }
    }

    /// Convert this cursor position to a character offset in the buffer.
    #[must_use]
    pub fn to_char_offset(&self, buffer: &Buffer) -> usize {
        let line_start = buffer.line_to_char(self.line);
        line_start + self.col
    }
}

impl PartialOrd for Cursor {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Cursor {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line.cmp(&other.line).then(self.col.cmp(&other.col))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_buffer() -> Buffer {
        Buffer::from_text("hello\nworld\nfoo")
    }

    #[test]
    fn test_new_cursor() {
        let c = Cursor::new(0, 0);
        assert_eq!(c.line, 0);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_move_right() {
        let buf = test_buffer();
        let mut c = Cursor::new(0, 0);
        c.move_right(&buf);
        assert_eq!(c.col, 1);
    }

    #[test]
    fn test_move_right_wraps() {
        let buf = test_buffer();
        let mut c = Cursor::new(0, 5); // end of "hello"
        c.move_right(&buf);
        assert_eq!(c.line, 1);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_move_left() {
        let buf = test_buffer();
        let mut c = Cursor::new(0, 3);
        c.move_left(&buf);
        assert_eq!(c.col, 2);
    }

    #[test]
    fn test_move_left_wraps() {
        let buf = test_buffer();
        let mut c = Cursor::new(1, 0);
        c.move_left(&buf);
        assert_eq!(c.line, 0);
        assert_eq!(c.col, 5); // end of "hello" (not counting \n)
    }

    #[test]
    fn test_move_up() {
        let buf = test_buffer();
        let mut c = Cursor::new(1, 3);
        c.move_up(&buf);
        assert_eq!(c.line, 0);
        assert_eq!(c.col, 3);
    }

    #[test]
    fn test_move_up_clamps_col() {
        let buf = Buffer::from_text("hi\nhello world");
        let mut c = Cursor::new(1, 10);
        c.move_up(&buf);
        assert_eq!(c.line, 0);
        assert_eq!(c.col, 2); // "hi" has length 2
    }

    #[test]
    fn test_move_down() {
        let buf = test_buffer();
        let mut c = Cursor::new(0, 2);
        c.move_down(&buf);
        assert_eq!(c.line, 1);
        assert_eq!(c.col, 2);
    }

    #[test]
    fn test_move_down_clamps_col() {
        let buf = Buffer::from_text("hello world\nhi");
        let mut c = Cursor::new(0, 10);
        c.move_down(&buf);
        assert_eq!(c.line, 1);
        assert_eq!(c.col, 2); // "hi" has length 2
    }

    #[test]
    fn test_move_to_line_start() {
        let mut c = Cursor::new(0, 5);
        c.move_to_line_start();
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_move_to_line_end() {
        let buf = test_buffer();
        let mut c = Cursor::new(0, 0);
        c.move_to_line_end(&buf);
        assert_eq!(c.col, 5); // "hello" has 5 chars
    }

    #[test]
    fn test_clamp() {
        let buf = Buffer::from_text("hi");
        let mut c = Cursor::new(100, 100);
        c.clamp_to_buffer(&buf);
        assert_eq!(c.line, 0);
        assert_eq!(c.col, 2);
    }

    #[test]
    fn test_to_char_offset() {
        let buf = test_buffer();
        let c = Cursor::new(1, 2);
        // "hello\n" = 6 chars, so line 1 starts at 6, col 2 => offset 8
        assert_eq!(c.to_char_offset(&buf), 8);
    }

    #[test]
    fn test_cursor_ordering() {
        let a = Cursor::new(0, 5);
        let b = Cursor::new(1, 0);
        assert!(a < b);
    }

    #[test]
    fn test_move_left_at_start() {
        let buf = test_buffer();
        let mut c = Cursor::new(0, 0);
        c.move_left(&buf);
        assert_eq!(c.line, 0);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_move_down_at_last_line() {
        let buf = test_buffer();
        let mut c = Cursor::new(2, 0);
        c.move_down(&buf);
        assert_eq!(c.line, 2);
    }

    #[test]
    fn test_cursor_move_left_stops_at_start() {
        let buf = test_buffer();
        let mut c = Cursor::new(0, 0);
        c.move_left(&buf);
        assert_eq!(c.line, 0);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_cursor_move_right_stops_at_end() {
        // "foo" is the last line (no trailing newline), length 3
        let buf = test_buffer();
        let mut c = Cursor::new(2, 3);
        c.move_right(&buf);
        assert_eq!(c.line, 2);
        assert_eq!(c.col, 3);
    }

    #[test]
    fn test_cursor_move_up_clamps_column() {
        // "hi\nhello world" — moving up from col 10 in line 1 to line 0 ("hi", len 2)
        let buf = Buffer::from_text("hi\nhello world");
        let mut c = Cursor::new(1, 10);
        c.move_up(&buf);
        assert_eq!(c.line, 0);
        assert_eq!(c.col, 2);
    }

    #[test]
    fn test_cursor_move_down_clamps_column() {
        let buf = Buffer::from_text("hello world\nhi");
        let mut c = Cursor::new(0, 10);
        c.move_down(&buf);
        assert_eq!(c.line, 1);
        assert_eq!(c.col, 2);
    }

    #[test]
    fn test_cursor_home() {
        let mut c = Cursor::new(1, 5);
        c.move_to_line_start();
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_cursor_end() {
        let buf = test_buffer();
        let mut c = Cursor::new(1, 0);
        c.move_to_line_end(&buf);
        assert_eq!(c.col, 5); // "world" has 5 chars
    }

    #[test]
    fn test_cursor_movement_clamps_to_buffer() {
        let buf = Buffer::from_text("hi");
        let mut c = Cursor::new(999, 999);
        c.clamp_to_buffer(&buf);
        assert_eq!(c.line, 0);
        assert_eq!(c.col, 2);
    }

    #[test]
    fn test_cursor_empty_buffer() {
        let buf = Buffer::from_text("");
        let mut c = Cursor::new(0, 0);
        c.move_left(&buf);
        assert_eq!(c, Cursor::new(0, 0));
        c.move_right(&buf);
        assert_eq!(c, Cursor::new(0, 0));
        c.move_up(&buf);
        assert_eq!(c, Cursor::new(0, 0));
        c.move_down(&buf);
        assert_eq!(c, Cursor::new(0, 0));
        c.move_to_line_start();
        assert_eq!(c.col, 0);
        c.move_to_line_end(&buf);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_cursor_single_line() {
        let buf = Buffer::from_text("abc");
        let mut c = Cursor::new(0, 1);
        c.move_up(&buf);
        assert_eq!(c, Cursor::new(0, 1)); // stays on line 0
        c.move_down(&buf);
        assert_eq!(c, Cursor::new(0, 1)); // stays on line 0
    }

    #[test]
    fn test_cursor_at_beginning_and_end() {
        let buf = Buffer::from_text("ab\ncd");
        // At very beginning
        let mut c = Cursor::new(0, 0);
        c.move_left(&buf);
        assert_eq!(c, Cursor::new(0, 0));
        // At very end
        let mut c = Cursor::new(1, 2);
        c.move_right(&buf);
        assert_eq!(c, Cursor::new(1, 2));
    }
}
