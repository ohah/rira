//! Viewport manages the visible area of the editor.
//!
//! Tracks which lines are currently visible and provides methods to scroll
//! and ensure the cursor stays within the visible region.

/// Viewport manages the visible area of the editor.
#[derive(Debug, Clone)]
pub struct Viewport {
    /// First visible line (0-indexed).
    pub scroll_offset: usize,
    /// Number of visible lines.
    pub visible_lines: usize,
}

impl Viewport {
    /// Create a new viewport with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scroll_offset: 0,
            visible_lines: 24,
        }
    }

    /// Scroll up by a given number of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Scroll down by a given number of lines, clamped to `max_lines`.
    ///
    /// `max_lines` is the total number of lines in the buffer.
    pub fn scroll_down(&mut self, lines: usize, max_lines: usize) {
        let max_offset = max_lines.saturating_sub(self.visible_lines);
        self.scroll_offset = (self.scroll_offset + lines).min(max_offset);
    }

    /// Adjust scroll offset so the cursor line is always visible.
    pub fn ensure_cursor_visible(&mut self, cursor_line: usize) {
        if cursor_line < self.scroll_offset {
            self.scroll_offset = cursor_line;
        } else if cursor_line >= self.scroll_offset + self.visible_lines {
            self.scroll_offset = cursor_line.saturating_sub(self.visible_lines.saturating_sub(1));
        }
    }

    /// Check if a given line is currently visible.
    #[must_use]
    pub fn is_line_visible(&self, line: usize) -> bool {
        line >= self.scroll_offset && line < self.scroll_offset + self.visible_lines
    }

    /// Return the first visible line index.
    #[must_use]
    pub fn first_visible_line(&self) -> usize {
        self.scroll_offset
    }

    /// Return the last visible line index (inclusive).
    #[must_use]
    pub fn last_visible_line(&self) -> usize {
        self.scroll_offset + self.visible_lines.saturating_sub(1)
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let vp = Viewport::new();
        assert_eq!(vp.scroll_offset, 0);
        assert_eq!(vp.visible_lines, 24);
    }

    #[test]
    fn test_scroll_up() {
        let mut vp = Viewport::new();
        vp.scroll_offset = 10;
        vp.scroll_up(3);
        assert_eq!(vp.scroll_offset, 7);
    }

    #[test]
    fn test_scroll_up_clamped_to_zero() {
        let mut vp = Viewport::new();
        vp.scroll_offset = 2;
        vp.scroll_up(5);
        assert_eq!(vp.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_up_from_zero() {
        let mut vp = Viewport::new();
        vp.scroll_up(1);
        assert_eq!(vp.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_down() {
        let mut vp = Viewport::new();
        vp.visible_lines = 10;
        vp.scroll_down(3, 100);
        assert_eq!(vp.scroll_offset, 3);
    }

    #[test]
    fn test_scroll_down_clamped_to_max() {
        let mut vp = Viewport::new();
        vp.visible_lines = 10;
        vp.scroll_offset = 85;
        vp.scroll_down(20, 100);
        assert_eq!(vp.scroll_offset, 90); // max_lines(100) - visible_lines(10)
    }

    #[test]
    fn test_scroll_down_when_content_fits() {
        let mut vp = Viewport::new();
        vp.visible_lines = 10;
        vp.scroll_down(5, 5); // content fits entirely
        assert_eq!(vp.scroll_offset, 0);
    }

    #[test]
    fn test_ensure_cursor_visible_cursor_above() {
        let mut vp = Viewport::new();
        vp.scroll_offset = 10;
        vp.visible_lines = 5;
        vp.ensure_cursor_visible(7);
        assert_eq!(vp.scroll_offset, 7);
    }

    #[test]
    fn test_ensure_cursor_visible_cursor_below() {
        let mut vp = Viewport::new();
        vp.scroll_offset = 0;
        vp.visible_lines = 5;
        vp.ensure_cursor_visible(7);
        assert_eq!(vp.scroll_offset, 3); // 7 - (5-1) = 3
    }

    #[test]
    fn test_ensure_cursor_visible_cursor_already_visible() {
        let mut vp = Viewport::new();
        vp.scroll_offset = 5;
        vp.visible_lines = 10;
        vp.ensure_cursor_visible(8);
        assert_eq!(vp.scroll_offset, 5); // no change
    }

    #[test]
    fn test_ensure_cursor_visible_at_boundary() {
        let mut vp = Viewport::new();
        vp.scroll_offset = 5;
        vp.visible_lines = 5;
        // cursor at first visible line
        vp.ensure_cursor_visible(5);
        assert_eq!(vp.scroll_offset, 5);
        // cursor at last visible line
        vp.ensure_cursor_visible(9);
        assert_eq!(vp.scroll_offset, 5);
    }

    #[test]
    fn test_is_line_visible() {
        let mut vp = Viewport::new();
        vp.scroll_offset = 5;
        vp.visible_lines = 10;
        assert!(!vp.is_line_visible(4));
        assert!(vp.is_line_visible(5));
        assert!(vp.is_line_visible(14));
        assert!(!vp.is_line_visible(15));
    }

    #[test]
    fn test_first_and_last_visible_line() {
        let mut vp = Viewport::new();
        vp.scroll_offset = 5;
        vp.visible_lines = 10;
        assert_eq!(vp.first_visible_line(), 5);
        assert_eq!(vp.last_visible_line(), 14);
    }

    #[test]
    fn test_first_and_last_visible_line_at_zero() {
        let vp = Viewport::new();
        assert_eq!(vp.first_visible_line(), 0);
        assert_eq!(vp.last_visible_line(), 23);
    }

    #[test]
    fn test_default_is_same_as_new() {
        let vp1 = Viewport::new();
        let vp2 = Viewport::default();
        assert_eq!(vp1.scroll_offset, vp2.scroll_offset);
        assert_eq!(vp1.visible_lines, vp2.visible_lines);
    }

    #[test]
    fn test_scroll_down_zero_lines() {
        let mut vp = Viewport::new();
        vp.scroll_down(0, 100);
        assert_eq!(vp.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_up_zero_lines() {
        let mut vp = Viewport::new();
        vp.scroll_offset = 5;
        vp.scroll_up(0);
        assert_eq!(vp.scroll_offset, 5);
    }

    #[test]
    fn test_ensure_cursor_visible_at_line_zero() {
        let mut vp = Viewport::new();
        vp.scroll_offset = 10;
        vp.ensure_cursor_visible(0);
        assert_eq!(vp.scroll_offset, 0);
    }

    #[test]
    fn test_visible_lines_one() {
        let mut vp = Viewport::new();
        vp.visible_lines = 1;
        vp.scroll_offset = 0;
        vp.ensure_cursor_visible(5);
        assert_eq!(vp.scroll_offset, 5);
        assert!(vp.is_line_visible(5));
        assert!(!vp.is_line_visible(4));
        assert!(!vp.is_line_visible(6));
    }
}
