//! Text buffer backed by `ropey::Rope`.

use ropey::Rope;
use std::fmt;
use std::fs;
use std::io;
use std::ops::Range;
use std::path::Path;

/// A text buffer wrapping a `ropey::Rope` for efficient text manipulation.
#[derive(Debug, Clone)]
pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    /// Create an empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    /// Create a buffer from a string.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    /// Create a buffer by reading a file.
    ///
    /// # Errors
    /// Returns an `io::Error` if the file cannot be read.
    pub fn from_file(path: &Path) -> io::Result<Self> {
        let text = fs::read_to_string(path)?;
        Ok(Self::from_text(&text))
    }

    /// Insert text at a character position.
    ///
    /// # Errors
    /// Returns an error if `pos` is out of bounds.
    pub fn insert(&mut self, pos: usize, text: &str) -> Result<(), BufferError> {
        let len = self.rope.len_chars();
        if pos > len {
            return Err(BufferError::OutOfBounds { pos, len });
        }
        self.rope.insert(pos, text);
        Ok(())
    }

    /// Delete a character range.
    ///
    /// # Errors
    /// Returns an error if the range is out of bounds.
    pub fn delete(&mut self, range: Range<usize>) -> Result<String, BufferError> {
        let len = self.rope.len_chars();
        if range.end > len {
            return Err(BufferError::OutOfBounds {
                pos: range.end,
                len,
            });
        }
        if range.start > range.end {
            return Err(BufferError::InvalidRange {
                start: range.start,
                end: range.end,
            });
        }
        let deleted = self.rope.slice(range.start..range.end).to_string();
        self.rope.remove(range);
        Ok(deleted)
    }

    /// Get the nth line (0-indexed). Returns `None` if out of bounds.
    #[must_use]
    pub fn line(&self, n: usize) -> Option<String> {
        if n >= self.rope.len_lines() {
            return None;
        }
        Some(self.rope.line(n).to_string())
    }

    /// Total number of lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Total number of characters.
    #[must_use]
    pub fn char_count(&self) -> usize {
        self.rope.len_chars()
    }

    /// Total number of bytes.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Get the character offset of the start of a line.
    #[must_use]
    pub fn line_to_char(&self, line: usize) -> usize {
        self.rope.line_to_char(line)
    }

    /// Get the length of a line in characters (including trailing newline if any).
    #[must_use]
    pub fn line_len_chars(&self, line: usize) -> usize {
        if line >= self.line_count() {
            return 0;
        }
        self.rope.line(line).len_chars()
    }

    /// Get a slice of text as a String.
    ///
    /// # Errors
    /// Returns an error if the range is out of bounds.
    pub fn slice(&self, range: Range<usize>) -> Result<String, BufferError> {
        let len = self.rope.len_chars();
        if range.end > len {
            return Err(BufferError::OutOfBounds {
                pos: range.end,
                len,
            });
        }
        Ok(self.rope.slice(range).to_string())
    }

    /// Save the buffer to a file.
    ///
    /// # Errors
    /// Returns an `io::Error` if the file cannot be written.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        fs::write(path, self.to_string())
    }

    /// Get the full content as a String.
    #[must_use]
    pub fn content(&self) -> String {
        self.rope.to_string()
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for chunk in self.rope.chunks() {
            write!(f, "{chunk}")?;
        }
        Ok(())
    }
}

/// Errors that can occur during buffer operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferError {
    /// A position was out of bounds.
    OutOfBounds {
        /// The position that was out of bounds.
        pos: usize,
        /// The length of the buffer.
        len: usize,
    },
    /// An invalid range was specified.
    InvalidRange {
        /// Start of the range.
        start: usize,
        /// End of the range.
        end: usize,
    },
}

impl fmt::Display for BufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds { pos, len } => {
                write!(f, "position {pos} out of bounds (buffer length: {len})")
            }
            Self::InvalidRange { start, end } => {
                write!(f, "invalid range {start}..{end}")
            }
        }
    }
}

impl std::error::Error for BufferError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer_is_empty() {
        let buf = Buffer::new();
        assert_eq!(buf.char_count(), 0);
        assert_eq!(buf.to_string(), "");
    }

    #[test]
    fn test_from_str() {
        let buf = Buffer::from_text("hello\nworld");
        assert_eq!(buf.char_count(), 11);
        assert_eq!(buf.line_count(), 2);
    }

    #[test]
    fn test_insert() {
        let mut buf = Buffer::from_text("hllo");
        buf.insert(1, "e").expect("insert should succeed");
        assert_eq!(buf.to_string(), "hello");
    }

    #[test]
    fn test_insert_out_of_bounds() {
        let mut buf = Buffer::from_text("hi");
        let result = buf.insert(100, "x");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete() {
        let mut buf = Buffer::from_text("hello world");
        let deleted = buf.delete(5..11).expect("delete should succeed");
        assert_eq!(deleted, " world");
        assert_eq!(buf.to_string(), "hello");
    }

    #[test]
    fn test_delete_out_of_bounds() {
        let mut buf = Buffer::from_text("hi");
        let result = buf.delete(0..100);
        assert!(result.is_err());
    }

    #[test]
    fn test_line() {
        let buf = Buffer::from_text("line1\nline2\nline3");
        assert_eq!(buf.line(0).expect("line 0 should exist"), "line1\n");
        assert_eq!(buf.line(2).expect("line 2 should exist"), "line3");
        assert!(buf.line(3).is_none());
    }

    #[test]
    fn test_line_count() {
        let buf = Buffer::from_text("a\nb\nc");
        assert_eq!(buf.line_count(), 3);
    }

    #[test]
    fn test_byte_len() {
        let buf = Buffer::from_text("hello");
        assert_eq!(buf.byte_len(), 5);
    }

    #[test]
    fn test_unicode() {
        let buf = Buffer::from_text("한글");
        assert_eq!(buf.char_count(), 2);
        assert_eq!(buf.byte_len(), 6); // each Korean char is 3 bytes
    }

    #[test]
    fn test_save_and_load() {
        let dir = std::env::temp_dir();
        let path = dir.join("rira_test_buffer.txt");
        let buf = Buffer::from_text("test content");
        buf.save(&path).expect("save should succeed");
        let loaded = Buffer::from_file(&path).expect("load should succeed");
        assert_eq!(loaded.to_string(), "test content");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_default() {
        let buf = Buffer::default();
        assert_eq!(buf.char_count(), 0);
    }

    #[test]
    fn test_slice() {
        let buf = Buffer::from_text("hello world");
        assert_eq!(buf.slice(0..5).expect("slice should succeed"), "hello");
    }
}
