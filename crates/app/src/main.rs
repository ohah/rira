use std::sync::Arc;

use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use rira_renderer::WgpuBackend;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

/// Application state holding the renderer and terminal.
struct App {
    window: Option<Arc<Window>>,
    terminal: Option<Terminal<WgpuBackend>>,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            terminal: None,
        }
    }

    fn render(&mut self) {
        let Some(terminal) = self.terminal.as_mut() else {
            return;
        };

        let result = terminal.draw(|frame| {
            let area = frame.area();

            let block = Block::default()
                .title(" rira - Rust Native Code Editor ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));

            let inner = block.inner(area);

            frame.render_widget(block, area);

            let lines = vec![
                Line::styled("Hello, rira!", Style::default().fg(Color::LightGreen)),
                Line::raw(""),
                Line::styled(
                    format!("Window size: {}x{} cells", area.width, area.height),
                    Style::default().fg(Color::LightYellow),
                ),
                Line::raw(""),
                Line::styled(
                    "Press 'q' or Escape to quit.",
                    Style::default().fg(Color::Gray),
                ),
                Line::raw(""),
                Line::styled(
                    "Type any key to see it in the log.",
                    Style::default().fg(Color::Gray),
                ),
            ];

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
                }
                // Re-render after resize
                self.render();
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                log::info!("Scale factor changed to {scale_factor}");
                if let Some(terminal) = self.terminal.as_mut() {
                    terminal.backend_mut().update_scale_factor(scale_factor);
                }
                // Re-render after DPI change
                self.render();
            }

            WindowEvent::RedrawRequested => {
                self.render();
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
                log::info!("Key pressed: {:?}", logical_key);

                match &logical_key {
                    Key::Named(NamedKey::Escape) => {
                        log::info!("Escape pressed, exiting");
                        event_loop.exit();
                    }
                    Key::Character(ch) if ch.as_str() == "q" => {
                        log::info!("'q' pressed, exiting");
                        event_loop.exit();
                    }
                    _ => {
                        // Request redraw to show any visual feedback
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
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

    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new();

    if let Err(e) = event_loop.run_app(&mut app) {
        log::error!("Event loop error: {e}");
    }
}
