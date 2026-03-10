//! Hit testing: convert screen coordinates to buffer positions.

use unicode_width::UnicodeWidthChar;

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
    ///
    /// For wide characters (Korean/CJK, 2 cell width), the cell column
    /// is mapped to the correct character index by walking the line's
    /// characters and accumulating their display widths.
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

        // Calculate cell column from x position
        let relative_x = x - self.content_x;
        let cell_col = if relative_x < 0.0 || self.cell_width <= 0.0 {
            0.0
        } else {
            relative_x / self.cell_width
        };

        // Convert cell column to character index by walking the line's
        // characters and accumulating their display widths.
        // Wide characters (Korean/CJK) occupy 2 cells, ASCII occupies 1.
        let col = if let Some(line_text) = buffer.line(line) {
            let text = line_text.trim_end_matches('\n');
            cell_col_to_char_index(text, cell_col)
        } else {
            0
        };

        HitTestResult { line, col }
    }
}

/// Convert a fractional cell column to a character index, accounting for
/// wide characters (Korean/CJK = 2 cells, ASCII = 1 cell).
///
/// Uses rounding: clicking the first half of a character's cells places the
/// cursor before it, clicking the second half places cursor after it.
fn cell_col_to_char_index(text: &str, cell_col: f64) -> usize {
    let mut accumulated_cells: usize = 0;
    for (char_idx, ch) in text.chars().enumerate() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(1);
        let mid = accumulated_cells as f64 + char_width as f64 / 2.0;
        if cell_col < mid {
            return char_idx;
        }
        accumulated_cells += char_width;
    }
    // Past end of line: return character count
    text.chars().count()
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

    // ── Wide character (Korean/CJK) hit test tests ──

    #[test]
    fn test_click_wide_char_first_cell() {
        // "한글" — '한' occupies cells 0-1, '글' occupies cells 2-3
        let buf = Buffer::from_text("한글");
        let config = default_config();
        // Click at cell 0 (left half of '한') -> char index 0
        let result = config.hit_test(10.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 0 });
    }

    #[test]
    fn test_click_wide_char_second_cell() {
        // "한글" — '한' occupies cells 0-1, '글' occupies cells 2-3
        let buf = Buffer::from_text("한글");
        let config = default_config();
        // Click at cell 1.5 (right half of '한') -> char index 1 (after '한')
        // x = content_x + 1.5 * cell_width = 10 + 15 = 25
        let result = config.hit_test(25.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 1 });
    }

    #[test]
    fn test_click_second_wide_char() {
        // "한글" — '한' occupies cells 0-1, '글' occupies cells 2-3
        let buf = Buffer::from_text("한글");
        let config = default_config();
        // Click at cell 2.5 (left half of '글') -> char index 1
        // x = content_x + 2.5 * cell_width = 10 + 25 = 35
        let result = config.hit_test(35.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 1 });
    }

    #[test]
    fn test_click_after_second_wide_char() {
        // "한글" — '한' occupies cells 0-1, '글' occupies cells 2-3
        let buf = Buffer::from_text("한글");
        let config = default_config();
        // Click at cell 3.5 (right half of '글') -> char index 2 (after '글')
        // x = content_x + 3.5 * cell_width = 10 + 35 = 45
        let result = config.hit_test(45.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 2 });
    }

    #[test]
    fn test_click_past_wide_chars() {
        let buf = Buffer::from_text("한글");
        let config = default_config();
        // Click far right -> clamp to end (2 chars)
        let result = config.hit_test(500.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 2 });
    }

    #[test]
    fn test_click_mixed_ascii_and_wide() {
        // "a한b" — 'a' at cell 0, '한' at cells 1-2, 'b' at cell 3
        let buf = Buffer::from_text("a한b");
        let config = default_config();

        // Click cell 0 (left half of 'a') -> col 0
        let result = config.hit_test(10.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 0 });

        // Click cell 1 (left half of '한') -> col 1
        // x = content_x + 1.0 * cell_width = 10 + 10 = 20
        let result = config.hit_test(20.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 1 });

        // Click cell 2.5 (right half of '한') -> col 2 (after '한')
        // x = content_x + 2.5 * cell_width = 10 + 25 = 35
        let result = config.hit_test(35.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 2 });

        // Click cell 3 ('b') -> col 2
        // x = content_x + 3.0 * cell_width = 10 + 30 = 40
        let result = config.hit_test(40.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 2 });

        // Click cell 3.6 (right half of 'b') -> col 3
        // x = content_x + 3.6 * cell_width = 10 + 36 = 46
        let result = config.hit_test(46.0, 20.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 3 });
    }

    #[test]
    fn test_cell_col_to_char_index_ascii() {
        // Pure ASCII: each char = 1 cell
        assert_eq!(cell_col_to_char_index("hello", 0.0), 0);
        assert_eq!(cell_col_to_char_index("hello", 0.6), 1);
        assert_eq!(cell_col_to_char_index("hello", 2.5), 3);
        assert_eq!(cell_col_to_char_index("hello", 10.0), 5);
    }

    #[test]
    fn test_cell_col_to_char_index_wide() {
        // "한글": '한'=cells 0-1, '글'=cells 2-3
        assert_eq!(cell_col_to_char_index("한글", 0.0), 0); // before '한'
        assert_eq!(cell_col_to_char_index("한글", 0.9), 0); // left half of '한'
        assert_eq!(cell_col_to_char_index("한글", 1.1), 1); // right half of '한'
        assert_eq!(cell_col_to_char_index("한글", 2.0), 1); // start of '글'
        assert_eq!(cell_col_to_char_index("한글", 3.1), 2); // right half of '글'
        assert_eq!(cell_col_to_char_index("한글", 5.0), 2); // past end
    }

    #[test]
    fn test_cell_col_to_char_index_mixed() {
        // "a한b": 'a'=cell 0, '한'=cells 1-2, 'b'=cell 3
        assert_eq!(cell_col_to_char_index("a한b", 0.0), 0); // before 'a'
        assert_eq!(cell_col_to_char_index("a한b", 0.6), 1); // after 'a'
        assert_eq!(cell_col_to_char_index("a한b", 1.5), 1); // left half of '한'
        assert_eq!(cell_col_to_char_index("a한b", 2.1), 2); // right half of '한'
        assert_eq!(cell_col_to_char_index("a한b", 3.0), 2); // before 'b'
        assert_eq!(cell_col_to_char_index("a한b", 3.6), 3); // after 'b'
    }

    #[test]
    fn test_cell_col_to_char_index_empty() {
        assert_eq!(cell_col_to_char_index("", 0.0), 0);
        assert_eq!(cell_col_to_char_index("", 5.0), 0);
    }
}
