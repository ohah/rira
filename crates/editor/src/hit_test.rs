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
///
/// All pixel values must use the same coordinate space (physical pixels).
/// winit's `CursorMoved` provides `PhysicalPosition`, and the renderer's
/// `cell_width()` / `cell_height()` / `title_bar_height_px()` return physical
/// pixels — pass them directly without dividing by `scale_factor`.
#[derive(Debug, Clone, Copy)]
pub struct HitTestConfig {
    /// Width of each character cell in pixels.
    pub cell_width: f64,
    /// Height of each line in pixels.
    pub line_height: f64,
    /// X offset where content starts (after border + gutter) in pixels.
    pub content_x: f64,
    /// Y offset where content starts (after border/title) in pixels.
    pub content_y: f64,
    /// Current scroll offset (first visible line).
    pub scroll_offset: usize,
}

impl HitTestConfig {
    /// Convert screen coordinates to buffer line/col position.
    ///
    /// The coordinates `x` and `y` must be in the same pixel space as the
    /// config fields (physical pixels). The result is clamped to valid buffer
    /// bounds.
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

    // ── Physical pixel coordinate tests ──
    //
    // These simulate real-world scenarios where all values are in physical
    // pixels (winit CursorMoved + WgpuBackend cell/title_bar dimensions).
    // Verifies the fix for the physical-vs-logical pixel mismatch bug.

    /// Simulate Retina display (2x): physical cell_width = 19.2, line_height = 40.0
    fn retina_config() -> HitTestConfig {
        let scale = 2.0;
        let cell_width = 9.6 * scale; // 19.2 physical pixels
        let line_height = 20.0 * scale; // 40.0 physical pixels
        let title_bar_height = 38.0 * scale; // 76.0 physical pixels
        HitTestConfig {
            cell_width,
            line_height,
            content_x: cell_width,                   // 1 cell left border
            content_y: title_bar_height + line_height, // title bar + 1 cell top border
            scroll_offset: 0,
        }
    }

    #[test]
    fn test_retina_click_first_char() {
        let buf = Buffer::from_text("hello");
        let config = retina_config();
        // Click at content origin in physical pixels
        // x = content_x = 19.2, y = content_y = 116.0
        let result = config.hit_test(19.2, 116.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 0 });
    }

    #[test]
    fn test_retina_click_third_char() {
        let buf = Buffer::from_text("hello");
        let config = retina_config();
        // x = content_x + 3 * cell_width = 19.2 + 57.6 = 76.8
        let result = config.hit_test(76.8, 116.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 3 });
    }

    #[test]
    fn test_retina_click_second_line() {
        let buf = Buffer::from_text("hello\nworld");
        let config = retina_config();
        // y = content_y + 1 * line_height = 116.0 + 40.0 = 156.0
        // x = content_x + 2 * cell_width = 19.2 + 38.4 = 57.6
        let result = config.hit_test(57.6, 156.0, &buf);
        assert_eq!(result, HitTestResult { line: 1, col: 2 });
    }

    #[test]
    fn test_retina_click_wide_char() {
        let buf = Buffer::from_text("한글");
        let config = retina_config();
        // '한' = cells 0-1, '글' = cells 2-3 (in physical: 0..38.4, 38.4..76.8)
        // Click at right half of '한': cell_col 1.5
        // x = content_x + 1.5 * cell_width = 19.2 + 28.8 = 48.0
        let result = config.hit_test(48.0, 116.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 1 });
    }

    /// BUG REGRESSION: if cell_width is divided by scale_factor (logical)
    /// but cursor_position is physical, column calculation is 2x off.
    #[test]
    fn test_physical_logical_mismatch_would_fail() {
        let buf = Buffer::from_text("hello");
        let scale = 2.0;
        let physical_cell_width = 9.6 * scale; // 19.2
        let physical_line_height = 20.0 * scale; // 40.0
        let physical_title_bar = 38.0 * scale; // 76.0

        // Correct: all physical
        let correct_config = HitTestConfig {
            cell_width: physical_cell_width,
            line_height: physical_line_height,
            content_x: physical_cell_width,
            content_y: physical_title_bar + physical_line_height,
            scroll_offset: 0,
        };

        // Bug: mixed logical cell_width with physical cursor position
        let buggy_config = HitTestConfig {
            cell_width: physical_cell_width / scale, // 9.6 (logical)
            line_height: physical_line_height / scale,
            content_x: physical_cell_width / scale,
            content_y: (physical_title_bar + physical_line_height) / scale,
            scroll_offset: 0,
        };

        // Physical cursor at column 3: x = content_x + 3 * cell_width
        let physical_x = physical_cell_width + 3.0 * physical_cell_width; // 19.2 + 57.6 = 76.8

        let correct = correct_config.hit_test(physical_x, 116.0, &buf);
        let buggy = buggy_config.hit_test(physical_x, 116.0, &buf);

        assert_eq!(correct, HitTestResult { line: 0, col: 3 });
        // Buggy config maps physical x=76.8 with logical cell_width=9.6:
        // relative_x = 76.8 - 9.6 = 67.2, cell_col = 67.2 / 9.6 = 7.0 → col 5 (clamped)
        assert_ne!(
            correct, buggy,
            "physical/logical mismatch should produce wrong result"
        );
    }

    /// Simulate 1x (non-Retina) display
    fn standard_config() -> HitTestConfig {
        let cell_width = 9.6;
        let line_height = 20.0;
        let title_bar_height = 38.0;
        HitTestConfig {
            cell_width,
            line_height,
            content_x: cell_width,
            content_y: title_bar_height + line_height,
            scroll_offset: 0,
        }
    }

    #[test]
    fn test_standard_display_click() {
        let buf = Buffer::from_text("hello");
        let config = standard_config();
        // x = content_x + 2 * cell_width = 9.6 + 19.2 = 28.8
        let result = config.hit_test(28.8, 58.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 2 });
    }

    /// Simulate Windows 150% DPI scaling
    fn windows_150_config() -> HitTestConfig {
        let scale = 1.5;
        let cell_width = 9.6 * scale; // 14.4
        let line_height = 20.0 * scale; // 30.0
        let title_bar_height = 38.0 * scale; // 57.0
        HitTestConfig {
            cell_width,
            line_height,
            content_x: cell_width,
            content_y: title_bar_height + line_height,
            scroll_offset: 0,
        }
    }

    #[test]
    fn test_windows_150_dpi_click() {
        let buf = Buffer::from_text("hello");
        let config = windows_150_config();
        // x = content_x + 4 * cell_width = 14.4 + 57.6 = 72.0
        let result = config.hit_test(72.0, 87.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 4 });
    }

    #[test]
    fn test_windows_150_dpi_wide_char() {
        let buf = Buffer::from_text("a한b");
        let config = windows_150_config();
        // 'a'=cell 0, '한'=cells 1-2, 'b'=cell 3
        // Click right half of '한': cell_col ~2.5
        // x = content_x + 2.5 * cell_width = 14.4 + 36.0 = 50.4
        let result = config.hit_test(50.4, 87.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 2 });
    }

    /// All scale factors should produce the same logical result for
    /// equivalent click positions.
    #[test]
    fn test_consistent_result_across_scale_factors() {
        let buf = Buffer::from_text("hello");

        for &scale in &[1.0, 1.25, 1.5, 2.0, 3.0] {
            let cell_width = 9.6 * scale;
            let line_height = 20.0 * scale;
            let title_bar = 38.0 * scale;
            let config = HitTestConfig {
                cell_width,
                line_height,
                content_x: cell_width,
                content_y: title_bar + line_height,
                scroll_offset: 0,
            };

            // Click at logical column 3 → physical x = content_x + 3 * cell_width
            let x = cell_width + 3.0 * cell_width;
            let y = title_bar + line_height; // first content line

            let result = config.hit_test(x, y, &buf);
            assert_eq!(
                result,
                HitTestResult { line: 0, col: 3 },
                "scale_factor={scale}: expected col 3"
            );
        }
    }

    /// All scale factors with wide chars should produce consistent results.
    #[test]
    fn test_consistent_wide_char_across_scale_factors() {
        let buf = Buffer::from_text("한글테스트");

        for &scale in &[1.0, 1.25, 1.5, 2.0, 3.0] {
            let cell_width = 9.6 * scale;
            let line_height = 20.0 * scale;
            let title_bar = 38.0 * scale;
            let config = HitTestConfig {
                cell_width,
                line_height,
                content_x: cell_width,
                content_y: title_bar + line_height,
                scroll_offset: 0,
            };

            // '한'=cells 0-1, '글'=cells 2-3, '테'=cells 4-5
            // Click right half of '글': cell_col ~3.5
            let x = cell_width + 3.5 * cell_width;
            let y = title_bar + line_height;

            let result = config.hit_test(x, y, &buf);
            assert_eq!(
                result,
                HitTestResult { line: 0, col: 2 },
                "scale_factor={scale}: expected col 2 (after '글')"
            );
        }
    }
}
