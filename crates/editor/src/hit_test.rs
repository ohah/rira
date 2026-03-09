//! Hit testing: convert screen coordinates to buffer positions.

use crate::buffer::Buffer;

/// Result of a hit test - converting screen coordinates to buffer position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitTestResult {
    /// 0-indexed line number in the buffer.
    pub line: usize,
    /// 0-indexed column (character offset within the line).
    pub col: usize,
}

/// Configuration for hit testing.
#[derive(Debug, Clone, Copy)]
pub struct HitTestConfig {
    /// Width of each character cell in logical pixels.
    pub cell_width: f64,
    /// Height of each line in logical pixels.
    pub line_height: f64,
    /// X offset where content starts (after border + gutter) in logical pixels.
    pub content_x: f64,
    /// Y offset where content starts (after border/title) in logical pixels.
    pub content_y: f64,
    /// Current scroll offset (first visible line).
    pub scroll_offset: usize,
}

impl HitTestConfig {
    /// Convert screen coordinates to buffer line/col position.
    ///
    /// The coordinates `x` and `y` are in logical pixels relative to the
    /// window's content area origin (top-left of the window).
    /// The result is clamped to valid buffer bounds.
    #[must_use]
    pub fn hit_test(&self, x: f64, y: f64, buffer: &Buffer) -> HitTestResult {
        let line_count = buffer.line_count();

        // Calculate line from y position relative to content area
        let relative_y = y - self.content_y;
        let visual_line = if relative_y < 0.0 {
            0usize
        } else if self.line_height > 0.0 {
            (relative_y / self.line_height) as usize
        } else {
            0
        };

        let buffer_line = visual_line + self.scroll_offset;

        // Clamp line to valid buffer range
        let line = if line_count == 0 {
            0
        } else {
            buffer_line.min(line_count - 1)
        };

        // Calculate col from x position relative to content area
        let relative_x = x - self.content_x;
        let col = if relative_x < 0.0 || self.cell_width <= 0.0 {
            0
        } else {
            // Use .round() so clicking the left half of a cell places cursor before it,
            // and clicking the right half places cursor after it.
            (relative_x / self.cell_width).round() as usize
        };

        // Clamp col to the content length of the line (excluding trailing newline)
        let max_col = buffer.line_content_len(line);
        let col = col.min(max_col);

        HitTestResult { line, col }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> HitTestConfig {
        HitTestConfig {
            cell_width: 10.0,
            line_height: 20.0,
            content_x: 10.0,
            content_y: 20.0,
            scroll_offset: 0,
        }
    }

    #[test]
    fn test_click_first_char() {
        let buf = Buffer::from_text("hello\nworld");
        let config = default_config();
        // Click at content origin -> line 0, col 0
        let result = config.hit_test(10.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 0 });
    }

    #[test]
    fn test_click_middle_of_line() {
        let buf = Buffer::from_text("hello\nworld");
        let config = default_config();
        // Click at x = content_x + 3 * cell_width = 10 + 30 = 40
        let result = config.hit_test(40.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 3 });
    }

    #[test]
    fn test_click_past_end_of_line() {
        let buf = Buffer::from_text("hi\nworld");
        let config = default_config();
        // Click far to the right on line 0 ("hi" has 2 chars)
        let result = config.hit_test(500.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 2 });
    }

    #[test]
    fn test_click_past_last_line() {
        let buf = Buffer::from_text("hello\nworld");
        let config = default_config();
        // Click far below -> should clamp to last line
        let result = config.hit_test(10.0, 5000.0, &buf);
        assert_eq!(result, HitTestResult { line: 1, col: 0 });
    }

    #[test]
    fn test_click_with_scroll_offset() {
        let buf = Buffer::from_text("line0\nline1\nline2\nline3");
        let config = HitTestConfig {
            scroll_offset: 2,
            ..default_config()
        };
        // Click at content_y -> visual line 0, but buffer line 2
        let result = config.hit_test(10.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 2, col: 0 });
    }

    #[test]
    fn test_click_in_gutter_area() {
        let buf = Buffer::from_text("hello");
        let config = default_config();
        // Click to the left of content area (x < content_x)
        let result = config.hit_test(0.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 0 });
    }

    #[test]
    fn test_click_above_content_area() {
        let buf = Buffer::from_text("hello");
        let config = default_config();
        // Click above content area (y < content_y)
        let result = config.hit_test(40.0, 0.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 3 });
    }

    #[test]
    fn test_click_second_line() {
        let buf = Buffer::from_text("hello\nworld");
        let config = default_config();
        // Click on second line: y = content_y + line_height = 20 + 20 = 40
        let result = config.hit_test(30.0, 40.0, &buf);
        assert_eq!(result, HitTestResult { line: 1, col: 2 });
    }

    #[test]
    fn test_click_empty_buffer() {
        let buf = Buffer::from_text("");
        let config = default_config();
        let result = config.hit_test(50.0, 50.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 0 });
    }

    #[test]
    fn test_click_with_scroll_past_buffer() {
        let buf = Buffer::from_text("hello\nworld");
        let config = HitTestConfig {
            scroll_offset: 10,
            ..default_config()
        };
        // scroll_offset is beyond buffer; should clamp to last line
        let result = config.hit_test(10.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 1, col: 0 });
    }

    #[test]
    fn test_click_single_char_line() {
        let buf = Buffer::from_text("a\nbc");
        let config = default_config();
        // Click on line 0 past the single char
        let result = config.hit_test(100.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 1 });
    }

    #[test]
    fn test_click_negative_coordinates() {
        let buf = Buffer::from_text("hello");
        let config = default_config();
        // Negative x and y
        let result = config.hit_test(-10.0, -10.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 0 });
    }

    #[test]
    fn test_click_rounding_to_nearest_cell() {
        let buf = Buffer::from_text("hello");
        let config = default_config();
        // Click at x = content_x + 2.6 * cell_width = 10 + 26 = 36
        // Rounds to col 3
        let result = config.hit_test(36.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 3 });

        // Click at x = content_x + 2.3 * cell_width = 10 + 23 = 33
        // Rounds to col 2
        let result = config.hit_test(33.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 2 });
    }
}
