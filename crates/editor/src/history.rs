//! Undo/redo history for edit operations.

use crate::buffer::{Buffer, BufferError};

/// An edit operation that can be undone/redone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOperation {
    /// Insert text at a character position.
    Insert {
        /// Character position where text was inserted.
        pos: usize,
        /// The text that was inserted.
        text: String,
    },
    /// Delete text from a character range.
    Delete {
        /// Character position where deletion started.
        pos: usize,
        /// The text that was deleted.
        text: String,
    },
}

/// A group of operations that should be undone/redone together.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EditGroup {
    ops: Vec<EditOperation>,
}

/// Undo/redo history.
#[derive(Debug, Clone)]
pub struct History {
    undo_stack: Vec<EditGroup>,
    redo_stack: Vec<EditGroup>,
    /// When true, consecutive single-char inserts/deletes are grouped.
    grouping: bool,
    /// When true, force the next push to start a new group.
    force_new_group: bool,
}

impl History {
    /// Create a new empty history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            grouping: true,
            force_new_group: false,
        }
    }

    /// Push an operation onto the undo stack.
    /// Clears the redo stack. Groups consecutive character inserts/deletes.
    pub fn push(&mut self, op: EditOperation) {
        self.redo_stack.clear();

        if self.grouping && !self.force_new_group {
            if let Some(group) = self.undo_stack.last_mut() {
                if Self::can_merge(&group.ops, &op) {
                    group.ops.push(op);
                    return;
                }
            }
        }

        self.force_new_group = false;
        self.undo_stack.push(EditGroup { ops: vec![op] });
    }

    /// Undo the last group of operations.
    ///
    /// # Errors
    /// Returns a `BufferError` if the undo operation fails.
    pub fn undo(&mut self, buffer: &mut Buffer) -> Result<bool, BufferError> {
        let group = match self.undo_stack.pop() {
            Some(g) => g,
            None => return Ok(false),
        };

        let mut redo_ops = Vec::new();

        // Apply operations in reverse order
        for op in group.ops.iter().rev() {
            match op {
                EditOperation::Insert { pos, text } => {
                    buffer.delete(*pos..*pos + text.len())?;
                    redo_ops.push(op.clone());
                }
                EditOperation::Delete { pos, text } => {
                    buffer.insert(*pos, text)?;
                    redo_ops.push(op.clone());
                }
            }
        }

        // The redo group stores the original ops so redo replays them
        self.redo_stack.push(group);
        Ok(true)
    }

    /// Redo the last undone group of operations.
    ///
    /// # Errors
    /// Returns a `BufferError` if the redo operation fails.
    pub fn redo(&mut self, buffer: &mut Buffer) -> Result<bool, BufferError> {
        let group = match self.redo_stack.pop() {
            Some(g) => g,
            None => return Ok(false),
        };

        // Replay operations in forward order
        for op in &group.ops {
            match op {
                EditOperation::Insert { pos, text } => {
                    buffer.insert(*pos, text)?;
                }
                EditOperation::Delete { pos, text } => {
                    buffer.delete(*pos..*pos + text.len())?;
                }
            }
        }

        self.undo_stack.push(group);
        Ok(true)
    }

    /// Start a new group boundary (prevents merging with previous ops).
    pub fn break_group(&mut self) {
        self.force_new_group = true;
    }

    /// Re-enable grouping after a break.
    pub fn enable_grouping(&mut self) {
        self.grouping = true;
    }

    /// Check if the undo stack is empty.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if the redo stack is empty.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Determine if a new operation can be merged with the current group.
    fn can_merge(existing: &[EditOperation], new_op: &EditOperation) -> bool {
        let last = match existing.last() {
            Some(op) => op,
            None => return false,
        };

        match (last, new_op) {
            // Consecutive single-char inserts at adjacent positions
            (
                EditOperation::Insert {
                    pos: prev_pos,
                    text: prev_text,
                },
                EditOperation::Insert { pos, text },
            ) => {
                prev_text.len() == 1
                    && text.len() == 1
                    && *pos == *prev_pos + prev_text.len()
                    && !prev_text.ends_with('\n')
                    && !text.ends_with('\n')
            }
            // Consecutive single-char deletes (backspace) at adjacent positions
            (
                EditOperation::Delete {
                    pos: prev_pos,
                    text: prev_text,
                },
                EditOperation::Delete { pos, text },
            ) => {
                prev_text.len() == 1
                    && text.len() == 1
                    && (*pos == prev_pos.saturating_sub(1) || *pos == *prev_pos)
            }
            _ => false,
        }
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_insert() {
        let mut buf = Buffer::from_text("hello");
        let mut history = History::new();

        buf.insert(5, " world").expect("insert should succeed");
        history.push(EditOperation::Insert {
            pos: 5,
            text: " world".to_string(),
        });

        assert_eq!(buf.to_string(), "hello world");
        history.undo(&mut buf).expect("undo should succeed");
        assert_eq!(buf.to_string(), "hello");
    }

    #[test]
    fn test_undo_delete() {
        let mut buf = Buffer::from_text("hello world");
        let mut history = History::new();

        buf.delete(5..11).expect("delete should succeed");
        history.push(EditOperation::Delete {
            pos: 5,
            text: " world".to_string(),
        });

        assert_eq!(buf.to_string(), "hello");
        history.undo(&mut buf).expect("undo should succeed");
        assert_eq!(buf.to_string(), "hello world");
    }

    #[test]
    fn test_redo() {
        let mut buf = Buffer::from_text("hello");
        let mut history = History::new();

        buf.insert(5, " world").expect("insert should succeed");
        history.push(EditOperation::Insert {
            pos: 5,
            text: " world".to_string(),
        });

        history.undo(&mut buf).expect("undo should succeed");
        assert_eq!(buf.to_string(), "hello");

        history.redo(&mut buf).expect("redo should succeed");
        assert_eq!(buf.to_string(), "hello world");
    }

    #[test]
    fn test_undo_clears_on_new_edit() {
        let mut buf = Buffer::from_text("hello");
        let mut history = History::new();

        buf.insert(5, " world").expect("insert should succeed");
        history.push(EditOperation::Insert {
            pos: 5,
            text: " world".to_string(),
        });

        history.undo(&mut buf).expect("undo should succeed");

        // New edit should clear redo
        buf.insert(5, "!").expect("insert should succeed");
        history.push(EditOperation::Insert {
            pos: 5,
            text: "!".to_string(),
        });

        assert!(!history.can_redo());
    }

    #[test]
    fn test_grouping_consecutive_inserts() {
        let mut buf = Buffer::from_text("");
        let mut history = History::new();

        // Simulate typing "abc"
        for (i, ch) in "abc".chars().enumerate() {
            buf.insert(i, &ch.to_string())
                .expect("insert should succeed");
            history.push(EditOperation::Insert {
                pos: i,
                text: ch.to_string(),
            });
        }

        assert_eq!(buf.to_string(), "abc");
        // All three should be in one group
        history.undo(&mut buf).expect("undo should succeed");
        assert_eq!(buf.to_string(), "");
    }

    #[test]
    fn test_break_group() {
        let mut buf = Buffer::from_text("");
        let mut history = History::new();

        buf.insert(0, "a").expect("insert should succeed");
        history.push(EditOperation::Insert {
            pos: 0,
            text: "a".to_string(),
        });

        history.break_group();
        history.enable_grouping();

        buf.insert(1, "b").expect("insert should succeed");
        history.push(EditOperation::Insert {
            pos: 1,
            text: "b".to_string(),
        });

        assert_eq!(buf.to_string(), "ab");
        // Undo should only remove "b"
        history.undo(&mut buf).expect("undo should succeed");
        assert_eq!(buf.to_string(), "a");
    }

    #[test]
    fn test_undo_empty() {
        let mut buf = Buffer::new();
        let mut history = History::new();
        let result = history.undo(&mut buf).expect("undo should succeed");
        assert!(!result);
    }

    #[test]
    fn test_redo_empty() {
        let mut buf = Buffer::new();
        let mut history = History::new();
        let result = history.redo(&mut buf).expect("redo should succeed");
        assert!(!result);
    }

    #[test]
    fn test_can_undo_redo() {
        let history = History::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn test_clear() {
        let mut history = History::new();
        history.push(EditOperation::Insert {
            pos: 0,
            text: "x".to_string(),
        });
        history.clear();
        assert!(!history.can_undo());
    }
}
