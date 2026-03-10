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
///
/// All pixel values must use a consistent unit (physical pixels).
/// Mixing physical coordinates with logical sizes will produce incorrect results
/// on HiDPI/Retina displays (scale_factor > 1.0).
#[derive(Debug, Clone, Copy)]
pub struct HitTestConfig {
    /// Width of each character cell in physical pixels.
    pub cell_width: f64,
    /// Height of each line in physical pixels.
    pub line_height: f64,
    /// X offset where content starts (after border + gutter) in physical pixels.
    pub content_x: f64,
    /// Y offset where content starts (after border/title) in physical pixels.
    pub content_y: f64,
    /// Current scroll offset (first visible line).
    pub scroll_offset: usize,
}

impl HitTestConfig {
    /// Convert screen coordinates to buffer line/col position.
    ///
    /// The coordinates `x` and `y` must be in the same unit (physical pixels)
    /// as the config fields. The result is clamped to valid buffer bounds.
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

    // ── HiDPI / Retina regression tests ──
    // These tests verify that hit testing produces correct results when all
    // values are consistently in physical pixels, and that mixing
    // physical coordinates with logical sizes (the old bug) gives wrong results.

    /// Simulates physical pixel values on a Retina display (scale_factor = 2.0).
    /// cell_width=20px, cell_height=40px, title_bar=80px (all physical).
    fn retina_config() -> HitTestConfig {
        let scale_factor = 2.0;
        // Logical: cell_width=10, cell_height=20, title_bar=40
        let cell_width = 10.0 * scale_factor;
        let cell_height = 20.0 * scale_factor;
        let title_bar_height = 40.0 * scale_factor;
        HitTestConfig {
            cell_width,
            line_height: cell_height,
            content_x: cell_width,                     // 1 cell left border
            content_y: title_bar_height + cell_height, // title bar + 1 cell top border
            scroll_offset: 0,
        }
    }

    #[test]
    fn test_retina_click_first_char() {
        let buf = Buffer::from_text("hello\nworld");
        let config = retina_config();
        // Physical click at content origin: x=20, y=120 (title_bar 80 + border 40)
        let result = config.hit_test(20.0, 120.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 0 });
    }

    #[test]
    fn test_retina_click_col3_line0() {
        let buf = Buffer::from_text("hello\nworld");
        let config = retina_config();
        // Physical: x = content_x + 3 * cell_width = 20 + 60 = 80
        let result = config.hit_test(80.0, 120.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 3 });
    }

    #[test]
    fn test_retina_click_second_line() {
        let buf = Buffer::from_text("hello\nworld");
        let config = retina_config();
        // Physical: y = content_y + 1 * line_height = 120 + 40 = 160
        // Physical: x = content_x + 2 * cell_width = 20 + 40 = 60
        let result = config.hit_test(60.0, 160.0, &buf);
        assert_eq!(result, HitTestResult { line: 1, col: 2 });
    }

    #[test]
    fn test_retina_click_past_end_of_line() {
        let buf = Buffer::from_text("hi\nworld");
        let config = retina_config();
        // Click far right on line 0 ("hi" = 2 chars), should clamp to col 2
        let result = config.hit_test(1000.0, 120.0, &buf);
        assert_eq!(result, HitTestResult { line: 0, col: 2 });
    }

    #[test]
    fn test_retina_click_with_scroll() {
        let buf = Buffer::from_text("line0\nline1\nline2\nline3");
        let config = HitTestConfig {
            scroll_offset: 2,
            ..retina_config()
        };
        // Click at content_y -> visual line 0, but buffer line 2
        let result = config.hit_test(20.0, 120.0, &buf);
        assert_eq!(result, HitTestResult { line: 2, col: 0 });
    }

    #[test]
    fn test_scale_factor_consistency() {
        // The same logical click position should produce the same buffer position
        // regardless of scale_factor, as long as all values scale together.
        let buf = Buffer::from_text("hello\nworld\nfoo");

        // 1x: click on line 1, col 3
        let config_1x = HitTestConfig {
            cell_width: 10.0,
            line_height: 20.0,
            content_x: 10.0,
            content_y: 60.0, // title_bar(40) + border(20)
            scroll_offset: 0,
        };
        // Physical click: x = 10 + 3*10 = 40, y = 60 + 1*20 = 80
        let result_1x = config_1x.hit_test(40.0, 80.0, &buf);

        // 2x: same logical position scaled to physical
        let config_2x = HitTestConfig {
            cell_width: 20.0,
            line_height: 40.0,
            content_x: 20.0,
            content_y: 120.0,
            scroll_offset: 0,
        };
        // Physical click: x = 20 + 3*20 = 80, y = 120 + 1*40 = 160
        let result_2x = config_2x.hit_test(80.0, 160.0, &buf);

        // 3x: same logical position scaled to physical
        let config_3x = HitTestConfig {
            cell_width: 30.0,
            line_height: 60.0,
            content_x: 30.0,
            content_y: 180.0,
            scroll_offset: 0,
        };
        // Physical click: x = 30 + 3*30 = 120, y = 180 + 1*60 = 240
        let result_3x = config_3x.hit_test(120.0, 240.0, &buf);

        assert_eq!(result_1x, HitTestResult { line: 1, col: 3 });
        assert_eq!(result_1x, result_2x);
        assert_eq!(result_2x, result_3x);
    }

    #[test]
    fn test_mixed_units_gives_wrong_result() {
        // Regression guard: demonstrates that using physical cursor coordinates
        // with logical cell sizes (the old bug) produces incorrect results.
        let buf = Buffer::from_text("hello\nworld");
        let scale_factor = 2.0;

        // CORRECT: all physical pixels
        let physical_config = HitTestConfig {
            cell_width: 10.0 * scale_factor,  // 20px physical
            line_height: 20.0 * scale_factor, // 40px physical
            content_x: 10.0 * scale_factor,   // 20px
            content_y: 60.0 * scale_factor,   // 120px
            scroll_offset: 0,
        };
        // Physical cursor at col 3, line 0: x = 20 + 3*20 = 80, y = 120
        let correct = physical_config.hit_test(80.0, 120.0, &buf);
        assert_eq!(correct, HitTestResult { line: 0, col: 3 });

        // BUG: physical cursor coordinates with logical cell sizes
        let buggy_config = HitTestConfig {
            cell_width: 10.0,  // logical (divided by scale_factor)
            line_height: 20.0, // logical
            content_x: 10.0,   // logical
            content_y: 60.0,   // logical
            scroll_offset: 0,
        };
        // Same physical cursor position (80, 120) but with logical config
        let buggy = buggy_config.hit_test(80.0, 120.0, &buf);
        // Bug: col would be (80-10)/10 = 7, clamped to 5 (len of "hello")
        // Bug: line would be (120-60)/20 = 3, clamped to 1
        assert_ne!(buggy, correct, "mixed units should give wrong result");
        assert_eq!(buggy, HitTestResult { line: 1, col: 5 });
    }

    #[test]
    fn test_title_bar_boundary_physical_pixels() {
        // Verify that title bar boundary detection works with physical pixels.
        // title_bar_height = 40 logical * 2.0 scale = 80 physical
        let buf = Buffer::from_text("hello");
        let config = retina_config(); // content_y = 80 + 40 = 120

        // Click at y=79 (in title bar area, before content)
        let result = config.hit_test(20.0, 79.0, &buf);
        // y < content_y, so relative_y < 0 → line 0
        assert_eq!(result.line, 0);

        // Click at y=120 (exactly at content start)
        let result = config.hit_test(20.0, 120.0, &buf);
        assert_eq!(result.line, 0);

        // Click at y=160 (one line below content start)
        let result = config.hit_test(20.0, 160.0, &buf);
        // Only 1 line in buffer, clamped to line 0
        assert_eq!(result.line, 0);
    }

    #[test]
    fn test_retina_drag_selection_consistency() {
        // Simulate drag: press at col 1, drag to col 4 on same line
        let buf = Buffer::from_text("hello world");
        let config = retina_config();

        // Press at col 1: x = 20 + 1*20 = 40
        let press = config.hit_test(40.0, 120.0, &buf);
        assert_eq!(press, HitTestResult { line: 0, col: 1 });

        // Drag to col 4: x = 20 + 4*20 = 100
        let drag = config.hit_test(100.0, 120.0, &buf);
        assert_eq!(drag, HitTestResult { line: 0, col: 4 });

        // Selection should span exactly 3 characters ("ello" -> col 1..4)
        assert_eq!(drag.col - press.col, 3);
    }

    #[test]
    fn test_retina_multiline_drag() {
        // Simulate drag across multiple lines on Retina
        let buf = Buffer::from_text("hello\nworld\nfoo");
        let config = retina_config();

        // Press at line 0, col 2: x=60, y=120
        let press = config.hit_test(60.0, 120.0, &buf);
        assert_eq!(press, HitTestResult { line: 0, col: 2 });

        // Drag to line 2, col 1: x=40, y=200 (120 + 2*40)
        let drag = config.hit_test(40.0, 200.0, &buf);
        assert_eq!(drag, HitTestResult { line: 2, col: 1 });
    }
}
