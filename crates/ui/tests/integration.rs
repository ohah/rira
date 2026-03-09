//! Integration tests: Editor state → ratatui TestBackend rendering
//!
//! These tests verify the full pipeline from editor operations to screen output
//! without requiring a GPU. They use ratatui's TestBackend to capture rendered frames.

#![allow(clippy::unwrap_used)]

use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use rira_editor::{Cursor, Editor, Selection};
use rira_ui::LineNumberGutter;

/// Helper: render the editor state into a TestBackend terminal and return the buffer snapshot.
fn render_editor(editor: &Editor, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal creation should succeed");

    let editor_content = editor.buffer.content();
    let cursor_line = editor.cursor.line;
    let cursor_col = editor.cursor.col;
    let has_selection = !editor.selection.is_empty();
    let (sel_start, sel_end) = editor.selection.ordered();

    let selection_style = Style::default().fg(Color::White).bg(Color::Blue);

    terminal
        .draw(|frame| {
            let area = frame.area();

            let block = Block::default()
                .title(" rira ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));

            let inner = block.inner(area);
            frame.render_widget(block, area);

            let mut lines: Vec<Line<'_>> = Vec::new();

            // split('\n') matches the editor's line model
            let text_lines: Vec<&str> = if editor_content.is_empty() {
                vec![""]
            } else {
                editor_content.split('\n').collect()
            };

            for (line_idx, line_text) in text_lines.iter().enumerate() {
                let chars: Vec<char> = line_text.chars().collect();

                if has_selection && line_idx >= sel_start.line && line_idx <= sel_end.line {
                    // Line overlaps the selection
                    let start_col = if line_idx == sel_start.line {
                        sel_start.col
                    } else {
                        0
                    };
                    let end_col = if line_idx == sel_end.line {
                        sel_end.col
                    } else {
                        chars.len()
                    };
                    let start_col = start_col.min(chars.len());
                    let end_col = end_col.min(chars.len());

                    let mut spans = Vec::new();

                    if line_idx == cursor_line {
                        // Render char-by-char with cursor and selection
                        let cursor_c = cursor_col.min(chars.len());
                        for (i, &ch) in chars.iter().enumerate() {
                            let style = if i == cursor_c {
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(Color::White)
                                    .add_modifier(Modifier::BOLD)
                            } else if i >= start_col && i < end_col {
                                selection_style
                            } else {
                                Style::default().fg(Color::White)
                            };
                            spans.push(Span::styled(ch.to_string(), style));
                        }
                        if cursor_c >= chars.len() {
                            spans.push(Span::styled(
                                " ",
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    } else {
                        // No cursor on this line
                        if start_col > 0 {
                            let before: String = chars[..start_col].iter().collect();
                            spans.push(Span::styled(before, Style::default().fg(Color::White)));
                        }
                        if start_col < end_col {
                            let selected: String = chars[start_col..end_col].iter().collect();
                            spans.push(Span::styled(selected, selection_style));
                        }
                        if end_col < chars.len() {
                            let after: String = chars[end_col..].iter().collect();
                            spans.push(Span::styled(after, Style::default().fg(Color::White)));
                        }
                    }

                    lines.push(Line::from(spans));
                } else if line_idx == cursor_line {
                    let before: String = chars[..cursor_col.min(chars.len())].iter().collect();
                    let cursor_char = if cursor_col < chars.len() {
                        chars[cursor_col].to_string()
                    } else {
                        " ".to_string()
                    };
                    let after: String = if cursor_col + 1 < chars.len() {
                        chars[cursor_col + 1..].iter().collect()
                    } else {
                        String::new()
                    };

                    lines.push(Line::from(vec![
                        Span::styled(before, Style::default().fg(Color::White)),
                        Span::styled(
                            cursor_char,
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(after, Style::default().fg(Color::White)),
                    ]));
                } else {
                    lines.push(Line::styled(
                        line_text.to_string(),
                        Style::default().fg(Color::White),
                    ));
                }
            }

            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph, inner);
        })
        .expect("draw should succeed");

    terminal.backend().buffer().clone()
}

/// Extract visible text from a buffer row (ignoring styling).
fn row_text(buf: &ratatui::buffer::Buffer, y: u16) -> String {
    let width = buf.area.width;
    (0..width)
        .map(|x| {
            buf.cell((x, y))
                .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
        })
        .collect::<String>()
        .trim_end()
        .to_string()
}

// ============================================================================
// Integration tests: Editor → Screen
// ============================================================================

#[test]
fn test_empty_editor_renders_cursor() {
    let editor = Editor::new();
    let buf = render_editor(&editor, 40, 10);

    // Row 0: top border with title
    let top = row_text(&buf, 0);
    assert!(top.contains("rira"), "Title bar should contain 'rira'");

    // Row 1 (first content row): should have the cursor (space with inverted style)
    let cell = buf.cell((1, 1)).unwrap();
    assert_eq!(cell.symbol(), " ");
    // Cursor should have black fg on white bg
    assert_eq!(cell.fg, Color::Black);
    assert_eq!(cell.bg, Color::White);
}

#[test]
fn test_typing_text_appears_on_screen() {
    let mut editor = Editor::new();
    for ch in "Hello".chars() {
        editor.insert_char(ch).unwrap();
    }

    let buf = render_editor(&editor, 40, 10);

    // Content row should contain "Hello"
    let content = row_text(&buf, 1);
    assert!(
        content.contains("Hello"),
        "Screen should show typed text, got: '{content}'"
    );
}

#[test]
fn test_newline_creates_second_line() {
    let mut editor = Editor::new();
    for ch in "Line1".chars() {
        editor.insert_char(ch).unwrap();
    }
    editor.newline().unwrap();
    for ch in "Line2".chars() {
        editor.insert_char(ch).unwrap();
    }

    let buf = render_editor(&editor, 40, 10);

    let line1 = row_text(&buf, 1);
    let line2 = row_text(&buf, 2);
    assert!(
        line1.contains("Line1"),
        "First line should contain 'Line1', got: '{line1}'"
    );
    assert!(
        line2.contains("Line2"),
        "Second line should contain 'Line2', got: '{line2}'"
    );
}

#[test]
fn test_backspace_removes_character_from_screen() {
    let mut editor = Editor::new();
    for ch in "abc".chars() {
        editor.insert_char(ch).unwrap();
    }
    editor.backspace().unwrap();

    let buf = render_editor(&editor, 40, 10);
    let content = row_text(&buf, 1);
    assert!(
        content.contains("ab"),
        "Screen should show 'ab' after backspace, got: '{content}'"
    );
    assert!(
        !content.contains("abc"),
        "Screen should not contain 'abc' after backspace"
    );
}

#[test]
fn test_cursor_position_after_arrow_keys() {
    let mut editor = Editor::new();
    for ch in "abcd".chars() {
        editor.insert_char(ch).unwrap();
    }
    // Cursor is at col 4, move left twice → col 2
    editor.cursor_left();
    editor.cursor_left();

    let buf = render_editor(&editor, 40, 10);

    // The cursor should be on 'c' (index 2)
    // In the render, offset by 1 for the border
    let cell = buf.cell((3, 1)).unwrap(); // x=1(border) + 2(col) = 3
    assert_eq!(cell.fg, Color::Black, "Cursor cell should have Black fg");
    assert_eq!(cell.bg, Color::White, "Cursor cell should have White bg");
    assert_eq!(cell.symbol(), "c");
}

#[test]
fn test_cursor_moves_to_correct_line_on_arrow_down() {
    let mut editor = Editor::new();
    for ch in "abc".chars() {
        editor.insert_char(ch).unwrap();
    }
    editor.newline().unwrap();
    for ch in "def".chars() {
        editor.insert_char(ch).unwrap();
    }
    // Cursor is on line 1, col 3. Move up.
    editor.cursor_up();
    assert_eq!(editor.cursor.line, 0);
    assert_eq!(editor.cursor.col, 3);

    let buf = render_editor(&editor, 40, 10);

    // Cursor should be on first line, col 3 (the position after 'c')
    // That's a space (past end of "abc")
    let cell = buf.cell((4, 1)).unwrap(); // x=1(border)+3(col)=4
    assert_eq!(cell.fg, Color::Black);
    assert_eq!(cell.bg, Color::White);
}

#[test]
fn test_delete_key_removes_char_after_cursor() {
    let mut editor = Editor::from_text("hello");
    editor.cursor = rira_editor::Cursor::new(0, 0);

    editor.delete_char().unwrap();

    let buf = render_editor(&editor, 40, 10);
    let content = row_text(&buf, 1);
    assert!(
        content.contains("ello"),
        "Screen should show 'ello' after delete, got: '{content}'"
    );
}

#[test]
fn test_home_end_keys() {
    let mut editor = Editor::from_text("hello world");
    editor.cursor = rira_editor::Cursor::new(0, 5);

    // Home → col 0
    editor.move_to_line_start();
    assert_eq!(editor.cursor.col, 0);

    let buf = render_editor(&editor, 40, 10);
    let cell = buf.cell((1, 1)).unwrap(); // x=1(border)+0(col)
    assert_eq!(cell.fg, Color::Black);
    assert_eq!(cell.bg, Color::White);
    assert_eq!(cell.symbol(), "h");

    // End → col 11
    editor.move_to_line_end();
    assert_eq!(editor.cursor.col, 11);
}

#[test]
fn test_undo_reflects_on_screen() {
    let mut editor = Editor::new();
    for ch in "abc".chars() {
        editor.insert_char(ch).unwrap();
    }
    editor.undo().unwrap();

    let buf = render_editor(&editor, 40, 10);
    let content = row_text(&buf, 1);
    // After undo, buffer should be empty
    assert!(
        !content.contains("abc"),
        "Screen should not contain 'abc' after undo"
    );
}

#[test]
fn test_gutter_renders_with_editor_lines() {
    let backend = TestBackend::new(30, 10);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let area = frame.area();
            let gutter = LineNumberGutter::new()
                .total_lines(5)
                .current_line(2)
                .scroll_offset(0);
            frame.render_widget(gutter, area);
        })
        .unwrap();

    let buf = terminal.backend().buffer();

    // Line 3 (current, 0-indexed=2) should have the current line style
    let line3_text = row_text(buf, 2);
    assert!(
        line3_text.contains('3'),
        "Gutter should show line number 3, got: '{line3_text}'"
    );
}

#[test]
fn test_multiple_redraws_produce_consistent_output() {
    let mut editor = Editor::new();
    for ch in "test".chars() {
        editor.insert_char(ch).unwrap();
    }

    let buf1 = render_editor(&editor, 40, 10);
    let buf2 = render_editor(&editor, 40, 10);

    // Two renders of the same state should produce identical output
    assert_eq!(buf1, buf2, "Consecutive renders should be identical");
}

#[test]
fn test_multiline_content_renders_correctly() {
    let mut editor = Editor::new();
    let lines_text = ["fn main() {", "    println!(\"hello\");", "}"];
    for (i, line) in lines_text.iter().enumerate() {
        for ch in line.chars() {
            editor.insert_char(ch).unwrap();
        }
        if i < lines_text.len() - 1 {
            editor.newline().unwrap();
        }
    }

    // Use a wide enough terminal
    let buf = render_editor(&editor, 50, 10);

    let row1 = row_text(&buf, 1);
    let row2 = row_text(&buf, 2);
    let row3 = row_text(&buf, 3);

    assert!(
        row1.contains("fn main()"),
        "Row 1 should contain 'fn main()', got: '{row1}'"
    );
    assert!(
        row2.contains("println!"),
        "Row 2 should contain 'println!', got: '{row2}'"
    );
    assert!(
        row3.contains('}'),
        "Row 3 should contain '}}', got: '{row3}'"
    );
}

#[test]
fn test_selection_renders_with_highlight_style() {
    let mut editor = Editor::from_text("hello world");
    editor.cursor = Cursor::new(0, 0);
    editor.selection = Selection::collapsed(Cursor::new(0, 0));
    // Select "hello" (5 chars to the right)
    editor.select_right();
    editor.select_right();
    editor.select_right();
    editor.select_right();
    editor.select_right();

    let buf = render_editor(&editor, 40, 10);

    // The selected characters (h, e, l, l, o at cols 0-4) should have Blue bg
    // The cursor is at col 5 (space after "hello")
    // Cols 0-4 are selected, col 5 is cursor
    // Border is col 0 in terminal, so text starts at col 1
    for col in 0..5 {
        let cell = buf.cell((col + 1, 1)).unwrap();
        assert_eq!(
            cell.bg,
            Color::Blue,
            "Selected char at col {col} should have Blue bg, got: {:?}",
            cell.bg
        );
    }
}

#[test]
fn test_shift_arrow_selection_accumulates() {
    let mut editor = Editor::from_text("abcdef");
    editor.cursor = Cursor::new(0, 1);
    editor.selection = Selection::collapsed(Cursor::new(0, 1));

    // Shift+Right three times
    editor.select_right();
    editor.select_right();
    editor.select_right();

    assert_eq!(editor.selection.anchor, Cursor::new(0, 1));
    assert_eq!(editor.selection.cursor, Cursor::new(0, 4));
    assert_eq!(
        editor.selection.selected_text(&editor.buffer),
        Some("bcd".to_string())
    );

    let buf = render_editor(&editor, 40, 10);

    // Chars at cols 1, 2, 3 should be selected (Blue bg)
    // Col 4 is the cursor (White bg)
    for col in 1..4 {
        let cell = buf.cell((col + 1, 1)).unwrap();
        assert_eq!(
            cell.bg,
            Color::Blue,
            "Selected char at col {col} should have Blue bg, got: {:?}",
            cell.bg
        );
    }
    // Cursor position
    let cursor_cell = buf.cell((5, 1)).unwrap();
    assert_eq!(cursor_cell.bg, Color::White, "Cursor should have White bg");
}

#[test]
fn test_multiline_selection_renders() {
    let mut editor = Editor::from_text("hello\nworld");
    editor.cursor = Cursor::new(0, 3);
    editor.selection = Selection::collapsed(Cursor::new(0, 3));
    // Select to end of first line and into second line
    // col 3 -> col 4 -> col 5 (end of "hello") -> line 1 col 0 -> line 1 col 1
    editor.select_right(); // col 4
    editor.select_right(); // col 5
    editor.select_right(); // line 1, col 0
    editor.select_right(); // line 1, col 1

    assert_eq!(editor.selection.anchor, Cursor::new(0, 3));
    assert_eq!(editor.cursor, Cursor::new(1, 1));

    let buf = render_editor(&editor, 40, 10);

    // First line: cols 3 and 4 should be selected (Blue bg)
    for col in 3..5 {
        let cell = buf.cell((col + 1, 1)).unwrap();
        assert_eq!(
            cell.bg,
            Color::Blue,
            "Line 0 col {col} should have Blue bg, got: {:?}",
            cell.bg
        );
    }

    // Second line: col 0 should be selected, col 1 is cursor
    let cell = buf.cell((1, 2)).unwrap();
    assert_eq!(
        cell.bg,
        Color::Blue,
        "Line 1 col 0 should have Blue bg, got: {:?}",
        cell.bg
    );
}
