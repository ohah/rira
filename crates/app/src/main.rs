mod cli;

use std::sync::Arc;

use clap::Parser;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use rira_editor::{Editor, HitTestConfig};
use rira_renderer::WgpuBackend;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::cli::{parse_file_arg, CliArgs};

/// Returns true if the platform command key is pressed (Cmd on macOS, Ctrl elsewhere).
fn is_cmd_pressed(modifiers: &ModifiersState) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.super_key()
    } else {
        modifiers.control_key()
    }
}

/// Application state holding the renderer and terminal.
struct App {
    window: Option<Arc<Window>>,
    terminal: Option<Terminal<WgpuBackend>>,
    /// Current cursor position in physical pixels for title bar hit testing
    cursor_position: (f64, f64),
    /// The text editor state.
    editor: Editor,
    /// IME preedit text (composing state, e.g. Korean 자모 조합 중)
    ime_preedit: String,
    /// Whether IME is currently active (composing)
    ime_composing: bool,
    /// Current modifier key state.
    modifiers: ModifiersState,
    /// System clipboard for copy/paste/cut
    clipboard: Option<arboard::Clipboard>,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            terminal: None,
            cursor_position: (0.0, 0.0),
            editor: Editor::new(),
            ime_preedit: String::new(),
            ime_composing: false,
            modifiers: ModifiersState::empty(),
            clipboard: arboard::Clipboard::new().ok(),
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn update_window_title(&self) {
        if let Some(window) = &self.window {
            let name = self.editor.file_name().unwrap_or("untitled");
            let modified = if self.editor.is_modified() { " *" } else { "" };
            window.set_title(&format!("{name}{modified} — rira"));
        }
    }

    fn handle_save(&mut self) {
        // Note: rfd dialogs block the main thread. Consider async file dialogs for
        // non-blocking UX in the future.
        if self.editor.file_path().is_some() {
            if let Err(e) = self.editor.save() {
                log::error!("Failed to save file: {e}");
            }
        } else if let Some(path) = rfd::FileDialog::new().save_file() {
            if let Err(e) = self.editor.save_as(&path) {
                log::error!("Failed to save file: {e}");
            }
        }
        self.update_window_title();
        self.render();
    }

    fn handle_open(&mut self) {
        // Note: rfd dialogs block the main thread. Consider async file dialogs for
        // non-blocking UX in the future.
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            match Editor::from_file(&path) {
                Ok(new_editor) => {
                    self.editor = new_editor;
                    self.update_window_title();
                    self.render();
                }
                Err(e) => {
                    log::error!("Failed to open file: {e}");
                }
            }
        }
    }

    fn render(&mut self) {
        let Some(terminal) = self.terminal.as_mut() else {
            return;
        };

        let editor_content = self.editor.buffer.content();
        let cursor_line = self.editor.cursor.line;
        let cursor_col = self.editor.cursor.col;
        let ime_preedit = self.ime_preedit.clone();
        let ime_composing = self.ime_composing;
        let file_name = self.editor.file_name().unwrap_or("untitled").to_string();
        let modified_indicator = if self.editor.is_modified() { " *" } else { "" };
        let has_selection = !self.editor.selection.is_empty();
        let (sel_start, sel_end) = self.editor.selection.ordered();

        // Update visible_lines based on current terminal size.
        // Content height = terminal height - 2 (borders) - 2 (status lines).
        let term_height = terminal.size().unwrap_or_default().height;
        let content_height = term_height.saturating_sub(4) as usize;
        if content_height > 0 {
            self.editor.viewport.visible_lines = content_height;
        }
        let scroll_offset = self.editor.viewport.scroll_offset;

        let result = terminal.draw(|frame| {
            let area = frame.area();

            let block = Block::default()
                .title(" rira - Rust Native Code Editor ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));

            let inner = block.inner(area);

            frame.render_widget(block, area);

            // Reserve 2 rows for the status line (blank + status text)
            let content_height = inner.height.saturating_sub(2) as usize;

            let mut lines: Vec<Line<'_>> = Vec::new();

            // split('\n') matches the editor's line model:
            // "hello\n" → ["hello", ""] which correctly represents 2 lines.
            let text_lines: Vec<&str> = if editor_content.is_empty() {
                vec![""]
            } else {
                editor_content.split('\n').collect()
            };

            let selection_style = Style::default().fg(Color::White).bg(Color::Blue);

            // Only render visible lines based on viewport
            let visible_end = (scroll_offset + content_height).min(text_lines.len());
            for (line_idx, line_text) in text_lines
                .iter()
                .enumerate()
                .take(visible_end)
                .skip(scroll_offset)
            {
                let chars: Vec<char> = line_text.chars().collect();

                if line_idx == cursor_line && ime_composing && !ime_preedit.is_empty() {
                    // IME preedit rendering (no selection highlight during composition)
                    let before: String = chars[..cursor_col.min(chars.len())].iter().collect();
                    let after: String = if cursor_col < chars.len() {
                        chars[cursor_col..].iter().collect()
                    } else {
                        String::new()
                    };

                    lines.push(Line::from(vec![
                        Span::styled(before, Style::default().fg(Color::White)),
                        Span::styled(
                            ime_preedit.clone(),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled(after, Style::default().fg(Color::White)),
                    ]));
                } else if has_selection && line_idx >= sel_start.line && line_idx <= sel_end.line {
                    // TODO: Extract selection rendering logic to crates/ui as a reusable widget
                    // to eliminate duplication between main.rs and integration tests.

                    // This line overlaps the selection
                    let sel_start_col = if line_idx == sel_start.line {
                        sel_start.col
                    } else {
                        0
                    };
                    let sel_end_col = if line_idx == sel_end.line {
                        sel_end.col
                    } else {
                        chars.len()
                    };

                    let sel_start_col = sel_start_col.min(chars.len());
                    let sel_end_col = sel_end_col.min(chars.len());

                    let mut spans = Vec::new();

                    // Text before selection
                    if sel_start_col > 0 {
                        let before: String = chars[..sel_start_col].iter().collect();
                        spans.push(Span::styled(before, Style::default().fg(Color::White)));
                    }

                    // Selected text
                    if sel_start_col < sel_end_col {
                        let selected: String = chars[sel_start_col..sel_end_col].iter().collect();
                        spans.push(Span::styled(selected, selection_style));
                    }

                    // Text after selection
                    if sel_end_col < chars.len() {
                        let after: String = chars[sel_end_col..].iter().collect();
                        spans.push(Span::styled(after, Style::default().fg(Color::White)));
                    }

                    // If the line is empty and within selection, show a selected space
                    if chars.is_empty() && sel_start_col == 0 && line_idx < sel_end.line {
                        spans.push(Span::styled(" ", selection_style));
                    }

                    // Render the cursor on this line if applicable
                    if line_idx == cursor_line {
                        // Cursor is shown within the selection line;
                        // we rebuild spans to include cursor highlight
                        spans.clear();
                        let cursor_c = cursor_col.min(chars.len());

                        // Before selection start
                        let pre_sel = sel_start_col.min(chars.len());
                        let post_sel = sel_end_col.min(chars.len());

                        // Note: char-by-char span generation for cursor line with selection.
                        // This may have performance implications for very long lines.
                        // Consider batch rendering optimization in the future.
                        for (i, &ch) in chars.iter().enumerate() {
                            let style = if i == cursor_c {
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(Color::White)
                                    .add_modifier(Modifier::BOLD)
                            } else if i >= pre_sel && i < post_sel {
                                selection_style
                            } else {
                                Style::default().fg(Color::White)
                            };
                            spans.push(Span::styled(ch.to_string(), style));
                        }

                        // Cursor past end of line
                        if cursor_c >= chars.len() {
                            spans.push(Span::styled(
                                " ",
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    }

                    lines.push(Line::from(spans));
                } else if line_idx == cursor_line {
                    // Cursor line without selection
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

            // Status line at the bottom
            lines.push(Line::raw(""));
            let total_lines = text_lines.len();
            lines.push(Line::styled(
                format!(
                    "{file_name}{modified_indicator} | Ln {}, Col {} | Scroll {}/{} | ESC to quit",
                    cursor_line + 1,
                    cursor_col + 1,
                    scroll_offset + 1,
                    total_lines,
                ),
                Style::default().fg(Color::DarkGray),
            ));

            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph, inner);
        });

        if let Err(e) = result {
            log::error!("Failed to draw terminal frame: {e}");
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("rira")
            .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 768.0));

        // On macOS, use transparent titlebar with fullsize content view
        // to keep native traffic light buttons while rendering our own title bar
        #[cfg(target_os = "macos")]
        let attrs = {
            use winit::platform::macos::WindowAttributesExtMacOS;
            attrs
                .with_titlebar_transparent(true)
                .with_title_hidden(true)
                .with_fullsize_content_view(true)
        };

        // On non-macOS platforms, remove decorations entirely
        #[cfg(not(target_os = "macos"))]
        let attrs = attrs.with_decorations(false);

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("Failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };

        let backend = match WgpuBackend::new(Arc::clone(&window)) {
            Ok(b) => b,
            Err(e) => {
                log::error!("Failed to initialize wgpu backend: {e}");
                event_loop.exit();
                return;
            }
        };

        let terminal = match Terminal::new(backend) {
            Ok(t) => t,
            Err(e) => {
                log::error!("Failed to create ratatui terminal: {e}");
                event_loop.exit();
                return;
            }
        };

        // Enable IME for CJK input (Korean, Japanese, Chinese)
        window.set_ime_allowed(true);

        self.window = Some(window);
        self.terminal = Some(terminal);

        // Do initial render
        self.render();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                log::info!("Window close requested");
                event_loop.exit();
            }

            WindowEvent::Resized(new_size) => {
                log::info!("Window resized to {}x{}", new_size.width, new_size.height);
                if let Some(terminal) = self.terminal.as_mut() {
                    terminal
                        .backend_mut()
                        .resize(new_size.width, new_size.height);
                    // INVARIANT: terminal.clear() MUST be called after backend resize.
                    //
                    // WgpuBackend::resize() recreates the pixel buffer as all zeros,
                    // but ratatui's internal diff state still holds the previous frame.
                    // Without clear(), the next draw() compares identical buffers,
                    // produces 0 diff updates, and the pixel buffer stays blank.
                    // clear() resets the "previous" buffer so the diff sees every
                    // cell as changed, forcing a full redraw into the new pixel buffer.
                    //
                    // See: test_draw_without_clear_after_resize_loses_content in
                    // crates/renderer/src/lib.rs for the regression test.
                    if let Err(e) = terminal.clear() {
                        log::error!("Failed to clear terminal after resize: {e}");
                    }
                }
                // Re-render after resize
                self.render();
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                log::info!("Scale factor changed to {scale_factor}");
                if let Some(terminal) = self.terminal.as_mut() {
                    terminal.backend_mut().update_scale_factor(scale_factor);
                    // INVARIANT: terminal.clear() MUST be called after scale factor
                    // change for the same reason as Resized — see comment above.
                    // update_scale_factor() calls resize() internally, which
                    // recreates the pixel buffer as all zeros.
                    if let Err(e) = terminal.clear() {
                        log::error!("Failed to clear terminal after scale change: {e}");
                    }
                }
                // Re-render after DPI change
                self.render();
            }

            WindowEvent::RedrawRequested => {
                self.render();
            }

            // IME events for CJK input (Korean, Japanese, Chinese)
            WindowEvent::Ime(ime) => match ime {
                Ime::Enabled => {
                    log::info!("IME enabled");
                }
                Ime::Preedit(text, cursor) => {
                    log::info!("IME preedit: {:?}, cursor: {:?}", text, cursor);
                    self.ime_preedit = text;
                    self.ime_composing = !self.ime_preedit.is_empty();
                    self.render();
                }
                Ime::Commit(text) => {
                    log::info!("IME commit: {:?}", text);
                    for c in text.chars() {
                        if let Err(e) = self.editor.insert_char(c) {
                            log::error!("IME commit insert failed: {e}");
                        }
                    }
                    self.ime_preedit.clear();
                    self.ime_composing = false;
                    self.render();
                }
                Ime::Disabled => {
                    log::info!("IME disabled");
                    self.ime_preedit.clear();
                    self.ime_composing = false;
                }
            },

            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                // Skip character input when IME is composing to avoid double input
                if self.ime_composing {
                    if let Key::Character(_) = &logical_key {
                        return;
                    }
                }

                log::info!("Key pressed: {:?}", logical_key);

                // Handle Cmd+key shortcuts before regular key handling
                if is_cmd_pressed(&self.modifiers) {
                    if let Key::Character(ch) = &logical_key {
                        match ch.as_str() {
                            "s" => {
                                self.handle_save();
                                return;
                            }
                            "o" => {
                                self.handle_open();
                                return;
                            }
                            "c" => {
                                if let Some(text) = self.editor.copy() {
                                    if let Some(cb) = self.clipboard.as_mut() {
                                        if let Err(e) = cb.set_text(&text) {
                                            log::error!("Failed to set clipboard: {e}");
                                        }
                                    }
                                }
                                return;
                            }
                            "v" => {
                                if let Some(cb) = self.clipboard.as_mut() {
                                    match cb.get_text() {
                                        Ok(text) => {
                                            if let Err(e) = self.editor.paste(&text) {
                                                log::error!("Paste failed: {e}");
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("Failed to get clipboard: {e}");
                                        }
                                    }
                                }
                                self.render();
                                return;
                            }
                            "x" => {
                                match self.editor.cut() {
                                    Ok(Some(text)) => {
                                        if let Some(cb) = self.clipboard.as_mut() {
                                            if let Err(e) = cb.set_text(&text) {
                                                log::error!("Failed to set clipboard: {e}");
                                            }
                                        }
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        log::error!("Cut failed: {e}");
                                    }
                                }
                                self.render();
                                return;
                            }
                            "a" => {
                                self.editor.select_all();
                                self.request_redraw();
                                return;
                            }
                            "z" => {
                                if self.modifiers.shift_key() {
                                    if let Err(e) = self.editor.redo() {
                                        log::error!("Redo failed: {e}");
                                    }
                                } else if let Err(e) = self.editor.undo() {
                                    log::error!("Undo failed: {e}");
                                }
                                self.render();
                                return;
                            }
                            _ => {}
                        }
                    }
                }

                match &logical_key {
                    Key::Named(NamedKey::Escape) => {
                        log::info!("Escape pressed, exiting");
                        event_loop.exit();
                    }
                    Key::Named(NamedKey::Backspace) => {
                        if let Err(e) = self.editor.backspace() {
                            log::error!("Backspace failed: {e}");
                        }
                        self.render();
                    }
                    Key::Named(NamedKey::Delete) => {
                        if let Err(e) = self.editor.delete_char() {
                            log::error!("Delete failed: {e}");
                        }
                        self.render();
                    }
                    Key::Named(NamedKey::Enter) => {
                        if let Err(e) = self.editor.newline() {
                            log::error!("Enter/newline failed: {e}");
                        }
                        self.render();
                    }
                    Key::Character(ch) => {
                        // Don't insert characters when Cmd is held (shortcuts)
                        if !is_cmd_pressed(&self.modifiers) {
                            for c in ch.chars() {
                                if let Err(e) = self.editor.insert_char(c) {
                                    log::error!("Insert char failed: {e}");
                                }
                            }
                        }
                        self.render();
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if self.modifiers.shift_key() {
                            self.editor.select_left();
                        } else {
                            self.editor.cursor_left();
                        }
                        self.request_redraw();
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if self.modifiers.shift_key() {
                            self.editor.select_right();
                        } else {
                            self.editor.cursor_right();
                        }
                        self.request_redraw();
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if self.modifiers.shift_key() {
                            self.editor.select_up();
                        } else {
                            self.editor.cursor_up();
                        }
                        self.request_redraw();
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if self.modifiers.shift_key() {
                            self.editor.select_down();
                        } else {
                            self.editor.cursor_down();
                        }
                        self.request_redraw();
                    }
                    Key::Named(NamedKey::Home) => {
                        if self.modifiers.shift_key() {
                            self.editor.select_to_line_start();
                        } else {
                            self.editor.move_to_line_start();
                        }
                        self.request_redraw();
                    }
                    Key::Named(NamedKey::End) => {
                        if self.modifiers.shift_key() {
                            self.editor.select_to_line_end();
                        } else {
                            self.editor.move_to_line_end();
                        }
                        self.request_redraw();
                    }
                    _ => {
                        self.request_redraw();
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let total_lines = self.editor.buffer.line_count();
                match delta {
                    MouseScrollDelta::LineDelta(_, y) => {
                        if y > 0.0 {
                            self.editor.viewport.scroll_up(y.abs().round() as usize);
                        } else if y < 0.0 {
                            self.editor
                                .viewport
                                .scroll_down(y.abs().round() as usize, total_lines);
                        }
                    }
                    MouseScrollDelta::PixelDelta(pos) => {
                        let line_height = 20.0_f64;
                        let lines = (pos.y.abs() / line_height).round() as usize;
                        if lines > 0 {
                            if pos.y > 0.0 {
                                self.editor.viewport.scroll_up(lines);
                            } else {
                                self.editor.viewport.scroll_down(lines, total_lines);
                            }
                        }
                    }
                }
                self.render();
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = (position.x, position.y);
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(terminal) = self.terminal.as_ref() {
                    let backend = terminal.backend();
                    let scale_factor = backend.window().scale_factor();
                    let physical_y = self.cursor_position.1 * scale_factor;

                    if backend.is_in_title_bar(0.0, physical_y as f32) {
                        // Click is in the title bar area, initiate window drag
                        if let Some(window) = &self.window {
                            if let Err(e) = window.drag_window() {
                                log::warn!("Failed to drag window: {e}");
                            }
                        }
                    } else {
                        // Click is in the content area, move cursor
                        let cell_width = backend.cell_width() as f64 / scale_factor;
                        let cell_height = backend.cell_height() as f64 / scale_factor;
                        let title_bar_height = backend.title_bar_height_px() as f64 / scale_factor;

                        // Content starts after title bar + 1 cell border (ratatui Block)
                        let content_x = cell_width; // 1 cell for left border
                        let content_y = title_bar_height + cell_height; // title bar + 1 cell for top border

                        let config = HitTestConfig {
                            cell_width,
                            line_height: cell_height,
                            content_x,
                            content_y,
                            scroll_offset: 0,
                        };

                        let result = config.hit_test(
                            self.cursor_position.0,
                            self.cursor_position.1,
                            &self.editor.buffer,
                        );

                        self.editor.move_cursor_to(result.line, result.col);
                        self.render();
                    }
                }
            }

            _ => {}
        }
    }
}

fn main() {
    env_logger::init();

    log::info!("rira v{}", rira_renderer::version());

    let args = CliArgs::parse();

    let editor = if let Some(ref file_arg) = args.file {
        let (path, line) = parse_file_arg(file_arg);

        match Editor::from_file(&path) {
            Ok(mut ed) => {
                let target_line = line
                    .map(|l| l.saturating_sub(1)) // 1-based to 0-based, 0 becomes 0
                    .unwrap_or(0);
                ed.set_cursor_line(target_line);
                log::info!(
                    "Opened file: {} at line {}",
                    path.display(),
                    target_line + 1
                );
                ed
            }
            Err(e) => {
                log::error!("Failed to open file '{}': {e}", path.display());
                Editor::new()
            }
        }
    } else {
        Editor::new()
    };

    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new();
    app.editor = editor;

    if let Err(e) = event_loop.run_app(&mut app) {
        log::error!("Event loop error: {e}");
    }
}
