//! Line number gutter widget for the editor.
//!
//! Displays right-aligned line numbers in a fixed-width column alongside the editor content.
//! The current line is highlighted with a distinct style.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

/// A widget that renders line numbers in a gutter alongside the editor.
///
/// # Example
/// ```
/// use rira_ui::LineNumberGutter;
///
/// let gutter = LineNumberGutter::new()
///     .total_lines(150)
///     .current_line(10)
///     .scroll_offset(0);
/// ```
#[derive(Debug, Clone)]
pub struct LineNumberGutter {
    total_lines: usize,
    current_line: usize,
    scroll_offset: usize,
    line_number_style: Style,
    current_line_style: Style,
}

impl Default for LineNumberGutter {
    fn default() -> Self {
        Self {
            total_lines: 1,
            current_line: 0,
            scroll_offset: 0,
            line_number_style: Style::default().fg(Color::DarkGray),
            current_line_style: Style::default().fg(Color::Yellow),
        }
    }
}

impl LineNumberGutter {
    /// Creates a new `LineNumberGutter` with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the total number of lines in the buffer.
    #[must_use]
    pub fn total_lines(mut self, total: usize) -> Self {
        self.total_lines = total.max(1);
        self
    }

    /// Sets the current (cursor) line index (0-based).
    #[must_use]
    pub fn current_line(mut self, line: usize) -> Self {
        self.current_line = line;
        self
    }

    /// Sets the scroll offset (0-based index of the first visible line).
    #[must_use]
    pub fn scroll_offset(mut self, offset: usize) -> Self {
        self.scroll_offset = offset;
        self
    }

    /// Sets the style for regular (non-current) line numbers.
    #[must_use]
    pub fn line_number_style(mut self, style: Style) -> Self {
        self.line_number_style = style;
        self
    }

    /// Sets the style for the current line number.
    #[must_use]
    pub fn current_line_style(mut self, style: Style) -> Self {
        self.current_line_style = style;
        self
    }

    /// Calculates the minimum width needed to display line numbers.
    ///
    /// Returns the number of digits needed for the largest line number,
    /// plus 1 for padding on the right.
    #[must_use]
    pub fn required_width(&self) -> u16 {
        let digits = digit_count(self.total_lines);
        // digits + 1 right-padding space
        (digits + 1) as u16
    }
}

/// Returns the number of decimal digits needed to represent `n`.
fn digit_count(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut count = 0;
    let mut val = n;
    while val > 0 {
        count += 1;
        val /= 10;
    }
    count
}

impl Widget for LineNumberGutter {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = area.width as usize;
        if width == 0 || area.height == 0 {
            return;
        }

        let digits = digit_count(self.total_lines);

        for row in 0..area.height {
            let line_index = self.scroll_offset + row as usize;

            if line_index >= self.total_lines {
                // Past end of file — render blank
                break;
            }

            let line_number = line_index + 1; // 1-based display
            let is_current = line_index == self.current_line;
            let style = if is_current {
                self.current_line_style
            } else {
                self.line_number_style
            };

            // Right-align the number within `digits` columns, then a trailing space
            let text = format!("{:>width$} ", line_number, width = digits);

            // Write characters into the buffer
            let y = area.y + row;
            for (i, ch) in text.chars().enumerate() {
                if i >= width {
                    break;
                }
                buf[(area.x + i as u16, y)].set_char(ch).set_style(style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    /// Helper: render a gutter into a test terminal and return the buffer.
    fn render_gutter(gutter: LineNumberGutter, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("failed to create terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(gutter, area);
            })
            .expect("failed to draw");
        terminal.backend().buffer().clone()
    }

    /// Extract text content from a buffer row (trimming trailing spaces from the overall line).
    fn row_text(buf: &Buffer, row: u16) -> String {
        let area = buf.area();
        let mut s = String::new();
        for col in 0..area.width {
            s.push(buf[(col, row)].symbol().chars().next().unwrap_or(' '));
        }
        s.trim_end().to_string()
    }

    #[test]
    fn test_gutter_renders_line_numbers() {
        let gutter = LineNumberGutter::new()
            .total_lines(5)
            .current_line(0)
            .scroll_offset(0);

        // digits=1, width needed = 2 (digit + space)
        let buf = render_gutter(gutter, 2, 5);

        assert_eq!(row_text(&buf, 0), "1");
        assert_eq!(row_text(&buf, 1), "2");
        assert_eq!(row_text(&buf, 2), "3");
        assert_eq!(row_text(&buf, 3), "4");
        assert_eq!(row_text(&buf, 4), "5");
    }

    #[test]
    fn test_gutter_highlights_current_line() {
        let gutter = LineNumberGutter::new()
            .total_lines(5)
            .current_line(2)
            .scroll_offset(0);

        let buf = render_gutter(gutter, 2, 5);

        // Row 0 (line 1) — normal: foreground should be DarkGray
        assert_eq!(buf[(0, 0)].fg, Color::DarkGray);
        // Row 2 (line 3) — current: foreground should be Yellow
        assert_eq!(buf[(0, 2)].fg, Color::Yellow);
        // Row 4 (line 5) — normal
        assert_eq!(buf[(0, 4)].fg, Color::DarkGray);
    }

    #[test]
    fn test_gutter_width_auto_calculates() {
        // <10 lines → 1 digit + 1 space = 2
        let g = LineNumberGutter::new().total_lines(9);
        assert_eq!(g.required_width(), 2);

        // 10..99 lines → 2 digits + 1 space = 3
        let g = LineNumberGutter::new().total_lines(50);
        assert_eq!(g.required_width(), 3);

        // 100..999 lines → 3 digits + 1 space = 4
        let g = LineNumberGutter::new().total_lines(500);
        assert_eq!(g.required_width(), 4);

        // 1000..9999 lines → 4 digits + 1 space = 5
        let g = LineNumberGutter::new().total_lines(5000);
        assert_eq!(g.required_width(), 5);
    }

    #[test]
    fn test_gutter_with_scroll_offset() {
        let gutter = LineNumberGutter::new()
            .total_lines(20)
            .current_line(7)
            .scroll_offset(5);

        // 2 digits + 1 space = 3
        let buf = render_gutter(gutter, 3, 5);

        // First visible line is 6 (index 5), right-aligned in 2-digit column
        assert_eq!(row_text(&buf, 0), " 6");
        assert_eq!(row_text(&buf, 1), " 7");
        assert_eq!(row_text(&buf, 2), " 8");
        assert_eq!(row_text(&buf, 3), " 9");
        assert_eq!(row_text(&buf, 4), "10");
    }

    #[test]
    fn test_gutter_empty_buffer() {
        // An "empty" buffer still has 1 line
        let gutter = LineNumberGutter::new()
            .total_lines(1)
            .current_line(0)
            .scroll_offset(0);

        let buf = render_gutter(gutter, 2, 3);

        assert_eq!(row_text(&buf, 0), "1");
        // Rows beyond total_lines should be empty
        assert_eq!(row_text(&buf, 1), "");
        assert_eq!(row_text(&buf, 2), "");
    }

    #[test]
    fn test_gutter_large_line_count() {
        let gutter = LineNumberGutter::new()
            .total_lines(1500)
            .current_line(999)
            .scroll_offset(998);

        // 4 digits + 1 space = 5
        let buf = render_gutter(gutter, 5, 4);

        assert_eq!(row_text(&buf, 0), " 999");
        assert_eq!(row_text(&buf, 1), "1000");
        assert_eq!(row_text(&buf, 2), "1001");
        assert_eq!(row_text(&buf, 3), "1002");

        // Line 1000 (index 999) is current
        assert_eq!(buf[(3, 1)].fg, Color::Yellow);
    }
}
