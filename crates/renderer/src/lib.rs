//! rira-renderer: wgpu + cosmic-text, ratatui Backend implementation
//!
//! Renders a ratatui cell grid to a GPU-backed window using wgpu.
//! Text shaping is handled by cosmic-text with monospace fonts.

use std::sync::Arc;

use cosmic_text::{
    Attrs, Buffer as CosmicBuffer, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use ratatui::backend::{ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use unicode_width::UnicodeWidthStr;
use winit::window::Window;

/// Default font size in pixels
const DEFAULT_FONT_SIZE: f32 = 16.0;
/// Default line height in pixels
const DEFAULT_LINE_HEIGHT: f32 = 20.0;
/// Minimum zoom level (50%)
const MIN_ZOOM: f32 = 0.5;
/// Maximum zoom level (300%)
const MAX_ZOOM: f32 = 3.0;
/// Zoom step per Cmd+/Cmd- press
const ZOOM_STEP: f32 = 0.1;
/// Default cell width for monospace font (approximate, measured at init)
const DEFAULT_CELL_WIDTH: f32 = 9.6;
/// Height of the custom title bar in logical pixels
const TITLE_BAR_HEIGHT: f32 = 38.0;
/// Title bar background color (slightly lighter than editor background)
const TITLE_BAR_BG: (u8, u8, u8) = (45, 45, 45);
/// Title bar text color
const TITLE_BAR_FG: (u8, u8, u8) = (180, 180, 180);
/// Title bar font size
const TITLE_BAR_FONT_SIZE: f32 = 13.0;
/// Title bar bottom border color (subtle separator)
const TITLE_BAR_BORDER: (u8, u8, u8) = (60, 60, 60);

/// Errors that can occur in the wgpu backend.
#[derive(Debug)]
pub enum RenderError {
    /// wgpu surface error
    Surface(wgpu::SurfaceError),
    /// wgpu request device error
    RequestDevice(wgpu::RequestDeviceError),
    /// wgpu adapter not found
    AdapterNotFound,
    /// Surface configuration error
    SurfaceConfig(String),
    /// General I/O error
    Io(std::io::Error),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Surface(e) => write!(f, "wgpu surface error: {e}"),
            Self::RequestDevice(e) => write!(f, "wgpu request device error: {e}"),
            Self::AdapterNotFound => write!(f, "no suitable wgpu adapter found"),
            Self::SurfaceConfig(msg) => write!(f, "surface config error: {msg}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for RenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Surface(e) => Some(e),
            Self::RequestDevice(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for RenderError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// GPU state created from a winit Window.
struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
}

/// Font rendering state using cosmic-text.
struct FontState {
    font_system: FontSystem,
    swash_cache: SwashCache,
    /// Physical (scaled) cell width in pixels
    cell_width: f32,
    /// Physical (scaled) cell height in pixels
    cell_height: f32,
}

/// A wgpu + cosmic-text backend for ratatui.
///
/// Renders a monospace text grid into a winit window using wgpu for GPU
/// presentation and cosmic-text for font shaping.
pub struct WgpuBackend {
    window: Arc<Window>,
    gpu: GpuState,
    font: FontState,
    /// Current display scale factor (1.0 on standard displays, 2.0 on Retina)
    scale_factor: f64,
    /// Cursor position in grid coordinates
    cursor_pos: Position,
    /// Whether cursor is visible
    cursor_visible: bool,
    /// Grid size in columns/rows
    grid_cols: u16,
    grid_rows: u16,
    /// CPU pixel buffer (RGBA, row-major)
    pixel_buffer: Vec<u8>,
    /// Buffer dimensions in pixels
    buf_width: u32,
    buf_height: u32,
    /// wgpu texture for uploading the pixel buffer
    texture: wgpu::Texture,
    /// Bind group for the fullscreen blit
    bind_group: wgpu::BindGroup,
    /// Render pipeline for fullscreen blit
    render_pipeline: wgpu::RenderPipeline,
    /// Title bar height in physical pixels (accounts for scale factor)
    title_bar_height_px: u32,
    /// Current title string displayed in the title bar
    title: String,
    /// Current zoom level (1.0 = 100%, 0.5 = 50%, 3.0 = 300%)
    zoom_level: f32,
}

impl WgpuBackend {
    /// Create a new `WgpuBackend` from a winit window.
    ///
    /// This will initialize wgpu, create a surface, and set up font rendering.
    ///
    /// # Errors
    ///
    /// Returns `RenderError` if wgpu initialization fails.
    pub fn new(window: Arc<Window>) -> Result<Self, RenderError> {
        let gpu = Self::init_gpu(&window)?;
        let scale_factor = window.scale_factor();
        let zoom_level = 1.0;
        let font = Self::init_font(scale_factor, zoom_level);

        let size = window.inner_size();
        let title_bar_height_px = (TITLE_BAR_HEIGHT * scale_factor as f32) as u32;

        let content_height = size.height.saturating_sub(title_bar_height_px);
        let grid_cols = (size.width as f32 / font.cell_width) as u16;
        let grid_rows = (content_height as f32 / font.cell_height) as u16;

        let buf_width = size.width.max(1);
        let buf_height = size.height.max(1);
        let pixel_buffer = vec![0u8; (buf_width * buf_height * 4) as usize];

        let (texture, bind_group, render_pipeline) =
            Self::create_blit_resources(&gpu.device, buf_width, buf_height, &gpu.surface_config);

        Ok(Self {
            window,
            gpu,
            font,
            scale_factor,
            cursor_pos: Position { x: 0, y: 0 },
            cursor_visible: true,
            grid_cols,
            grid_rows,
            pixel_buffer,
            buf_width,
            buf_height,
            texture,
            bind_group,
            render_pipeline,
            title_bar_height_px,
            title: String::from("rira"),
            zoom_level,
        })
    }

    fn init_gpu(window: &Arc<Window>) -> Result<GpuState, RenderError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(Arc::clone(window))
            .map_err(|e| RenderError::SurfaceConfig(format!("{e}")))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(|_| RenderError::AdapterNotFound)?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rira-device"),
            ..Default::default()
        }))
        .map_err(RenderError::RequestDevice)?;

        let size = window.inner_size();
        let surface_config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or(RenderError::SurfaceConfig(
                "failed to get default surface config".to_string(),
            ))?;

        surface.configure(&device, &surface_config);

        Ok(GpuState {
            device,
            queue,
            surface,
            surface_config,
        })
    }

    fn init_font(scale_factor: f64, zoom_level: f32) -> FontState {
        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let (cell_width, cell_height) =
            Self::measure_cell_size(&mut font_system, scale_factor, zoom_level);

        FontState {
            font_system,
            swash_cache,
            cell_width,
            cell_height,
        }
    }

    /// Measure cell dimensions at the given scale without recreating the font system.
    fn measure_cell_size(
        font_system: &mut FontSystem,
        scale_factor: f64,
        zoom_level: f32,
    ) -> (f32, f32) {
        let scale = scale_factor as f32 * zoom_level;
        let scaled_font_size = DEFAULT_FONT_SIZE * scale;
        let scaled_line_height = DEFAULT_LINE_HEIGHT * scale;

        let metrics = Metrics::new(scaled_font_size, scaled_line_height);
        let mut measure_buf = CosmicBuffer::new(font_system, metrics);
        measure_buf.set_text(
            font_system,
            "M",
            &Attrs::new().family(Family::Monospace),
            Shaping::Advanced,
            None,
        );
        measure_buf.shape_until_scroll(font_system, false);

        let cell_width = measure_buf
            .layout_runs()
            .next()
            .and_then(|run| run.glyphs.first().map(|g| g.w))
            .unwrap_or(DEFAULT_CELL_WIDTH * scale);

        (cell_width, scaled_line_height)
    }

    fn create_blit_resources(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> (wgpu::Texture, wgpu::BindGroup, wgpu::RenderPipeline) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rira-cell-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rira-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rira-blit-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rira-blit-bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rira-blit-pl"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rira-blit-shader"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rira-blit-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        (texture, bind_group, render_pipeline)
    }

    /// Handle a window resize event.
    ///
    /// `width` and `height` are in physical pixels (as returned by `window.inner_size()`).
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.gpu.surface_config.width = width;
        self.gpu.surface_config.height = height;
        self.gpu
            .surface
            .configure(&self.gpu.device, &self.gpu.surface_config);

        self.scale_factor = self.window.scale_factor();
        self.title_bar_height_px = (TITLE_BAR_HEIGHT * self.scale_factor as f32) as u32;

        let content_height = height.saturating_sub(self.title_bar_height_px);
        self.grid_cols = (width as f32 / self.font.cell_width) as u16;
        self.grid_rows = (content_height as f32 / self.font.cell_height) as u16;

        self.buf_width = width;
        self.buf_height = height;
        self.pixel_buffer = vec![0u8; (width * height * 4) as usize];

        let (texture, bind_group, render_pipeline) =
            Self::create_blit_resources(&self.gpu.device, width, height, &self.gpu.surface_config);
        self.texture = texture;
        self.bind_group = bind_group;
        self.render_pipeline = render_pipeline;
    }

    /// Handle a scale factor change (e.g., window moved between displays).
    ///
    /// This reinitializes font metrics at the new scale and triggers a resize.
    pub fn update_scale_factor(&mut self, scale_factor: f64) {
        if (self.scale_factor - scale_factor).abs() < f64::EPSILON {
            return;
        }

        self.scale_factor = scale_factor;
        // Reuse existing FontSystem — avoids expensive system font re-scan
        let (cw, ch) =
            Self::measure_cell_size(&mut self.font.font_system, scale_factor, self.zoom_level);
        self.font.cell_width = cw;
        self.font.cell_height = ch;
        self.font.swash_cache = SwashCache::new();

        // Re-derive grid dimensions and buffers at the current physical size
        let size = self.window.inner_size();
        self.resize(size.width, size.height);
    }

    /// Current display scale factor.
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// Present the current pixel buffer to the window surface.
    ///
    /// # Errors
    ///
    /// Returns `RenderError` if the surface texture cannot be acquired.
    pub fn present(&mut self) -> Result<(), RenderError> {
        // Upload pixel buffer to texture
        self.gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.pixel_buffer,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.buf_width * 4),
                rows_per_image: Some(self.buf_height),
            },
            wgpu::Extent3d {
                width: self.buf_width,
                height: self.buf_height,
                depth_or_array_layers: 1,
            },
        );

        let output = self
            .gpu
            .surface
            .get_current_texture()
            .map_err(RenderError::Surface)?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rira-blit-encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rira-blit-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw(0..6, 0..1);
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Get a reference to the window.
    pub fn window(&self) -> &Window {
        &self.window
    }

    /// Request a redraw of the window.
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Cell width in pixels.
    pub fn cell_width(&self) -> f32 {
        self.font.cell_width
    }

    /// Cell height in pixels.
    pub fn cell_height(&self) -> f32 {
        self.font.cell_height
    }

    /// Title bar height in physical pixels.
    pub fn title_bar_height_px(&self) -> u32 {
        self.title_bar_height_px
    }

    /// Set the title string displayed in the custom title bar.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    /// Current zoom level (1.0 = 100%).
    pub fn zoom_level(&self) -> f32 {
        self.zoom_level
    }

    /// Set zoom level, clamped to [MIN_ZOOM, MAX_ZOOM].
    /// Reinitializes font metrics and triggers a resize.
    pub fn set_zoom(&mut self, level: f32) {
        let new_zoom = level.clamp(MIN_ZOOM, MAX_ZOOM);
        if (self.zoom_level - new_zoom).abs() < f32::EPSILON {
            return;
        }
        self.zoom_level = new_zoom;
        // Reuse existing FontSystem — avoids expensive system font re-scan
        let (cw, ch) =
            Self::measure_cell_size(&mut self.font.font_system, self.scale_factor, new_zoom);
        self.font.cell_width = cw;
        self.font.cell_height = ch;
        self.font.swash_cache = SwashCache::new();
        let size = self.window.inner_size();
        self.resize(size.width, size.height);
    }

    /// Zoom in by one step.
    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom_level + ZOOM_STEP);
    }

    /// Zoom out by one step.
    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom_level - ZOOM_STEP);
    }

    /// Reset zoom to 100%.
    pub fn zoom_reset(&mut self) {
        self.set_zoom(1.0);
    }

    /// Check if a physical pixel coordinate is within the title bar area.
    pub fn is_in_title_bar(&self, _x: f32, y: f32) -> bool {
        y < self.title_bar_height_px as f32
    }

    /// Render the custom title bar directly into the pixel buffer.
    ///
    /// This draws a solid background, a bottom border, and centered title text.
    /// Must be called BEFORE ratatui content is rendered (as part of clear or
    /// at the beginning of each frame).
    pub fn render_title_bar(&mut self) {
        let tb_h = self.title_bar_height_px;
        let w = self.buf_width;

        // Fill title bar background
        for y in 0..tb_h {
            for x in 0..w {
                let idx = ((y * w + x) * 4) as usize;
                if idx + 3 < self.pixel_buffer.len() {
                    self.pixel_buffer[idx] = TITLE_BAR_BG.0;
                    self.pixel_buffer[idx + 1] = TITLE_BAR_BG.1;
                    self.pixel_buffer[idx + 2] = TITLE_BAR_BG.2;
                    self.pixel_buffer[idx + 3] = 255;
                }
            }
        }

        // Draw 1px bottom border
        if tb_h > 0 {
            let border_y = tb_h - 1;
            for x in 0..w {
                let idx = ((border_y * w + x) * 4) as usize;
                if idx + 3 < self.pixel_buffer.len() {
                    self.pixel_buffer[idx] = TITLE_BAR_BORDER.0;
                    self.pixel_buffer[idx + 1] = TITLE_BAR_BORDER.1;
                    self.pixel_buffer[idx + 2] = TITLE_BAR_BORDER.2;
                    self.pixel_buffer[idx + 3] = 255;
                }
            }
        }

        // Render centered title text using cosmic-text
        let scale = self.scale_factor as f32;
        let font_size = TITLE_BAR_FONT_SIZE * scale;
        let line_height = font_size * 1.3;
        let metrics = Metrics::new(font_size, line_height);
        let mut buf = CosmicBuffer::new(&mut self.font.font_system, metrics);
        buf.set_text(
            &mut self.font.font_system,
            &self.title,
            &Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
            None,
        );
        buf.shape_until_scroll(&mut self.font.font_system, false);

        // Calculate text width for centering
        let text_width: f32 = buf
            .layout_runs()
            .next()
            .map(|run| run.glyphs.iter().map(|g| g.w).sum())
            .unwrap_or(0.0);

        let text_x = ((w as f32 - text_width) / 2.0).max(0.0);
        // Vertically center in title bar
        let text_y = ((tb_h as f32 - line_height) / 2.0).max(0.0);

        // Re-create buffer (borrow checker workaround since we need font_system for both)
        let mut buf2 = CosmicBuffer::new(&mut self.font.font_system, metrics);
        buf2.set_text(
            &mut self.font.font_system,
            &self.title,
            &Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
            None,
        );
        buf2.shape_until_scroll(&mut self.font.font_system, false);

        for run in buf2.layout_runs() {
            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                let image = self
                    .font
                    .swash_cache
                    .get_image(&mut self.font.font_system, physical.cache_key)
                    .clone();

                if let Some(ref img) = image {
                    let glyph_x = text_x as i32 + physical.x + img.placement.left;
                    let glyph_y =
                        text_y as i32 + (run.line_y as i32) + physical.y - img.placement.top;

                    for gy in 0..img.placement.height as i32 {
                        for gx in 0..img.placement.width as i32 {
                            let dest_x = glyph_x + gx;
                            let dest_y = glyph_y + gy;

                            if dest_x < 0
                                || dest_y < 0
                                || dest_x >= w as i32
                                || dest_y >= tb_h as i32
                            {
                                continue;
                            }

                            let src_idx = (gy as u32 * img.placement.width + gx as u32) as usize;

                            let alpha = match img.content {
                                cosmic_text::SwashContent::Mask => {
                                    img.data.get(src_idx).copied().unwrap_or(0)
                                }
                                cosmic_text::SwashContent::Color => {
                                    img.data.get(src_idx * 4 + 3).copied().unwrap_or(0)
                                }
                                cosmic_text::SwashContent::SubpixelMask => {
                                    img.data.get(src_idx * 3 + 1).copied().unwrap_or(0)
                                }
                            };

                            if alpha == 0 {
                                continue;
                            }

                            let idx = ((dest_y as u32 * w + dest_x as u32) * 4) as usize;
                            if idx + 3 < self.pixel_buffer.len() {
                                let a = alpha as f32 / 255.0;
                                let inv_a = 1.0 - a;
                                match img.content {
                                    cosmic_text::SwashContent::Color => {
                                        let sr = img.data.get(src_idx * 4).copied().unwrap_or(0);
                                        let sg =
                                            img.data.get(src_idx * 4 + 1).copied().unwrap_or(0);
                                        let sb =
                                            img.data.get(src_idx * 4 + 2).copied().unwrap_or(0);
                                        self.pixel_buffer[idx] =
                                            blend(sr, self.pixel_buffer[idx], a, inv_a);
                                        self.pixel_buffer[idx + 1] =
                                            blend(sg, self.pixel_buffer[idx + 1], a, inv_a);
                                        self.pixel_buffer[idx + 2] =
                                            blend(sb, self.pixel_buffer[idx + 2], a, inv_a);
                                    }
                                    _ => {
                                        self.pixel_buffer[idx] =
                                            blend(TITLE_BAR_FG.0, self.pixel_buffer[idx], a, inv_a);
                                        self.pixel_buffer[idx + 1] = blend(
                                            TITLE_BAR_FG.1,
                                            self.pixel_buffer[idx + 1],
                                            a,
                                            inv_a,
                                        );
                                        self.pixel_buffer[idx + 2] = blend(
                                            TITLE_BAR_FG.2,
                                            self.pixel_buffer[idx + 2],
                                            a,
                                            inv_a,
                                        );
                                    }
                                }
                                self.pixel_buffer[idx + 3] = 255;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Render a single cell at grid position (col, row) into the pixel buffer.
    /// The cell is offset by the title bar height so ratatui content starts below it.
    /// Fill only the background of a cell at the given grid position.
    ///
    /// Used to propagate background color to continuation cells of wide
    /// characters (e.g. Korean/CJK), which ratatui resets to default style.
    fn fill_cell_background(&mut self, col: u16, row: u16, bg: ratatui::style::Color) {
        let px_x = (col as f32 * self.font.cell_width) as u32;
        let px_y = (row as f32 * self.font.cell_height) as u32 + self.title_bar_height_px;
        let cw = self.font.cell_width.ceil() as u32;
        let ch = self.font.cell_height.ceil() as u32;

        let (bg_r, bg_g, bg_b) = color_to_rgb(bg, false);

        for dy in 0..ch {
            for dx in 0..cw {
                let x = px_x + dx;
                let y = px_y + dy;
                if x < self.buf_width && y < self.buf_height {
                    let idx = ((y * self.buf_width + x) * 4) as usize;
                    if idx + 3 < self.pixel_buffer.len() {
                        self.pixel_buffer[idx] = bg_r;
                        self.pixel_buffer[idx + 1] = bg_g;
                        self.pixel_buffer[idx + 2] = bg_b;
                        self.pixel_buffer[idx + 3] = 255;
                    }
                }
            }
        }
    }

    /// Render only the background of a cell at grid position (col, row).
    fn render_cell_bg(&mut self, col: u16, row: u16, cell: &Cell) {
        let px_x = (col as f32 * self.font.cell_width) as u32;
        let px_y = (row as f32 * self.font.cell_height) as u32 + self.title_bar_height_px;
        let cw = self.font.cell_width.ceil() as u32;
        let ch = self.font.cell_height.ceil() as u32;

        let (bg_r, bg_g, bg_b) = color_to_rgb(cell.bg, false);

        for dy in 0..ch {
            for dx in 0..cw {
                let x = px_x + dx;
                let y = px_y + dy;
                if x < self.buf_width && y < self.buf_height {
                    let idx = ((y * self.buf_width + x) * 4) as usize;
                    if idx + 3 < self.pixel_buffer.len() {
                        self.pixel_buffer[idx] = bg_r;
                        self.pixel_buffer[idx + 1] = bg_g;
                        self.pixel_buffer[idx + 2] = bg_b;
                        self.pixel_buffer[idx + 3] = 255;
                    }
                }
            }
        }
    }

    /// Render only the glyph of a cell at grid position (col, row).
    fn render_cell_glyph(&mut self, col: u16, row: u16, cell: &Cell) {
        let px_x = (col as f32 * self.font.cell_width) as u32;
        let px_y = (row as f32 * self.font.cell_height) as u32 + self.title_bar_height_px;

        // Render glyph
        let sym = cell.symbol();
        if sym.is_empty() || sym == " " {
            return;
        }

        let (fg_r, fg_g, fg_b) = color_to_rgb(cell.fg, true);

        let scale = self.scale_factor as f32 * self.zoom_level;
        let metrics = Metrics::new(DEFAULT_FONT_SIZE * scale, DEFAULT_LINE_HEIGHT * scale);
        let mut buf = CosmicBuffer::new(&mut self.font.font_system, metrics);
        buf.set_text(
            &mut self.font.font_system,
            sym,
            &Attrs::new().family(Family::Monospace),
            Shaping::Advanced,
            None,
        );
        buf.shape_until_scroll(&mut self.font.font_system, false);

        for run in buf.layout_runs() {
            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);

                let image = self
                    .font
                    .swash_cache
                    .get_image(&mut self.font.font_system, physical.cache_key)
                    .clone();

                if let Some(ref img) = image {
                    let glyph_x = px_x as i32 + physical.x + img.placement.left;
                    let glyph_y =
                        px_y as i32 + (run.line_y as i32) + physical.y - img.placement.top;

                    for gy in 0..img.placement.height as i32 {
                        for gx in 0..img.placement.width as i32 {
                            let dest_x = glyph_x + gx;
                            let dest_y = glyph_y + gy;

                            if dest_x < 0
                                || dest_y < 0
                                || dest_x >= self.buf_width as i32
                                || dest_y >= self.buf_height as i32
                            {
                                continue;
                            }

                            let src_idx = (gy as u32 * img.placement.width + gx as u32) as usize;

                            let alpha = match img.content {
                                cosmic_text::SwashContent::Mask => {
                                    img.data.get(src_idx).copied().unwrap_or(0)
                                }
                                cosmic_text::SwashContent::Color => {
                                    img.data.get(src_idx * 4 + 3).copied().unwrap_or(0)
                                }
                                cosmic_text::SwashContent::SubpixelMask => {
                                    // Use the green channel as alpha for simplicity
                                    img.data.get(src_idx * 3 + 1).copied().unwrap_or(0)
                                }
                            };

                            if alpha == 0 {
                                continue;
                            }

                            let idx =
                                ((dest_y as u32 * self.buf_width + dest_x as u32) * 4) as usize;
                            if idx + 3 < self.pixel_buffer.len() {
                                let a = alpha as f32 / 255.0;
                                let inv_a = 1.0 - a;
                                match img.content {
                                    cosmic_text::SwashContent::Color => {
                                        let sr = img.data.get(src_idx * 4).copied().unwrap_or(0);
                                        let sg =
                                            img.data.get(src_idx * 4 + 1).copied().unwrap_or(0);
                                        let sb =
                                            img.data.get(src_idx * 4 + 2).copied().unwrap_or(0);
                                        self.pixel_buffer[idx] =
                                            blend(sr, self.pixel_buffer[idx], a, inv_a);
                                        self.pixel_buffer[idx + 1] =
                                            blend(sg, self.pixel_buffer[idx + 1], a, inv_a);
                                        self.pixel_buffer[idx + 2] =
                                            blend(sb, self.pixel_buffer[idx + 2], a, inv_a);
                                    }
                                    _ => {
                                        self.pixel_buffer[idx] =
                                            blend(fg_r, self.pixel_buffer[idx], a, inv_a);
                                        self.pixel_buffer[idx + 1] =
                                            blend(fg_g, self.pixel_buffer[idx + 1], a, inv_a);
                                        self.pixel_buffer[idx + 2] =
                                            blend(fg_b, self.pixel_buffer[idx + 2], a, inv_a);
                                    }
                                }
                                self.pixel_buffer[idx + 3] = 255;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Render a block cursor at the given grid position.
    /// The cursor is offset by the title bar height to match the content area.
    fn render_cursor(&mut self, col: u16, row: u16) {
        let px_x = (col as f32 * self.font.cell_width) as u32;
        let px_y = (row as f32 * self.font.cell_height) as u32 + self.title_bar_height_px;
        let cw = self.font.cell_width.ceil() as u32;
        let ch = self.font.cell_height.ceil() as u32;

        // Semi-transparent white block cursor
        for dy in 0..ch {
            for dx in 0..cw {
                let x = px_x + dx;
                let y = px_y + dy;
                if x < self.buf_width && y < self.buf_height {
                    let idx = ((y * self.buf_width + x) * 4) as usize;
                    if idx + 3 < self.pixel_buffer.len() {
                        // Invert colors for cursor visibility
                        self.pixel_buffer[idx] = 255 - self.pixel_buffer[idx];
                        self.pixel_buffer[idx + 1] = 255 - self.pixel_buffer[idx + 1];
                        self.pixel_buffer[idx + 2] = 255 - self.pixel_buffer[idx + 2];
                        self.pixel_buffer[idx + 3] = 255;
                    }
                }
            }
        }
    }
}

impl ratatui::backend::Backend for WgpuBackend {
    type Error = RenderError;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        // Two-pass rendering: backgrounds first, then glyphs.
        // This prevents continuation cell backgrounds from overwriting
        // the right half of wide character (Korean/CJK) glyphs.
        let cells: Vec<(u16, u16, Cell)> = content.map(|(x, y, c)| (x, y, c.clone())).collect();

        // Pass 1: Fill all backgrounds (including wide char continuation cells)
        for (x, y, cell) in &cells {
            self.render_cell_bg(*x, *y, cell);

            // Wide characters occupy 2+ cells but ratatui resets
            // continuation cells to default style. Propagate the
            // background so selection highlights are continuous.
            let symbol = cell.symbol();
            if !symbol.is_empty() && symbol != " " {
                let char_width = UnicodeWidthStr::width(symbol);
                for dx in 1..char_width as u16 {
                    self.fill_cell_background(x + dx, *y, cell.bg);
                }
            }
        }

        // Pass 2: Render all glyphs on top of backgrounds
        for (x, y, cell) in &cells {
            self.render_cell_glyph(*x, *y, cell);
        }

        // Draw cursor if visible
        if self.cursor_visible {
            self.render_cursor(self.cursor_pos.x, self.cursor_pos.y);
        }

        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.cursor_visible = false;
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.cursor_visible = true;
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        Ok(self.cursor_pos)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        self.cursor_pos = position.into();
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.pixel_buffer.fill(0);
        // Re-render title bar after clearing the pixel buffer
        self.render_title_bar();
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        match clear_type {
            ClearType::All => self.clear(),
            _ => {
                // For non-All clear types, just clear the whole buffer for simplicity
                self.clear()
            }
        }
    }

    fn size(&self) -> Result<Size, Self::Error> {
        Ok(Size::new(self.grid_cols, self.grid_rows))
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        Ok(WindowSize {
            columns_rows: Size::new(self.grid_cols, self.grid_rows),
            pixels: Size::new(self.buf_width as u16, self.buf_height as u16),
        })
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.present()
    }
}

/// Convert a ratatui `Color` to an (r, g, b) tuple.
/// `is_fg` selects defaults: white for foreground, dark for background.
fn color_to_rgb(color: ratatui::style::Color, is_fg: bool) -> (u8, u8, u8) {
    use ratatui::style::Color;
    match color {
        Color::Reset => {
            if is_fg {
                (220, 220, 220) // light gray foreground
            } else {
                (30, 30, 30) // dark background
            }
        }
        Color::Black => (0, 0, 0),
        Color::Red => (204, 0, 0),
        Color::Green => (78, 154, 6),
        Color::Yellow => (196, 160, 0),
        Color::Blue => (52, 101, 164),
        Color::Magenta => (117, 80, 123),
        Color::Cyan => (6, 152, 154),
        Color::Gray => (211, 215, 207),
        Color::DarkGray => (85, 87, 83),
        Color::LightRed => (239, 41, 41),
        Color::LightGreen => (138, 226, 52),
        Color::LightYellow => (252, 233, 79),
        Color::LightBlue => (114, 159, 207),
        Color::LightMagenta => (173, 127, 168),
        Color::LightCyan => (52, 226, 226),
        Color::White => (255, 255, 255),
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(idx) => ansi256_to_rgb(idx),
    }
}

/// Convert an ANSI 256-color index to RGB.
fn ansi256_to_rgb(idx: u8) -> (u8, u8, u8) {
    match idx {
        0 => (0, 0, 0),
        1 => (128, 0, 0),
        2 => (0, 128, 0),
        3 => (128, 128, 0),
        4 => (0, 0, 128),
        5 => (128, 0, 128),
        6 => (0, 128, 128),
        7 => (192, 192, 192),
        8 => (128, 128, 128),
        9 => (255, 0, 0),
        10 => (0, 255, 0),
        11 => (255, 255, 0),
        12 => (0, 0, 255),
        13 => (255, 0, 255),
        14 => (0, 255, 255),
        15 => (255, 255, 255),
        16..=231 => {
            let idx = idx - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;
            let to_val = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            (to_val(r), to_val(g), to_val(b))
        }
        232..=255 => {
            let v = 8 + 10 * (idx - 232);
            (v, v, v)
        }
    }
}

/// Alpha blend: fg * alpha + bg * (1 - alpha)
fn blend(fg: u8, bg: u8, a: f32, inv_a: f32) -> u8 {
    ((fg as f32).mul_add(a, bg as f32 * inv_a)).min(255.0) as u8
}

/// WGSL fullscreen blit shader.
/// Draws a fullscreen quad (2 triangles from 6 vertices) and samples the texture.
const BLIT_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle strip as 2 triangles (6 vertices)
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

@group(0) @binding(0) var t_texture: texture_2d<f32>;
@group(0) @binding(1) var t_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_texture, t_sampler, in.uv);
}
"#;

/// Return the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(version(), "0.1.0");
    }

    #[test]
    fn test_color_to_rgb_reset_fg() {
        let (r, g, b) = color_to_rgb(ratatui::style::Color::Reset, true);
        assert_eq!((r, g, b), (220, 220, 220));
    }

    #[test]
    fn test_color_to_rgb_reset_bg() {
        let (r, g, b) = color_to_rgb(ratatui::style::Color::Reset, false);
        assert_eq!((r, g, b), (30, 30, 30));
    }

    #[test]
    fn test_color_to_rgb_named() {
        assert_eq!(color_to_rgb(ratatui::style::Color::Red, true), (204, 0, 0));
        assert_eq!(
            color_to_rgb(ratatui::style::Color::White, true),
            (255, 255, 255)
        );
        assert_eq!(color_to_rgb(ratatui::style::Color::Black, true), (0, 0, 0));
    }

    #[test]
    fn test_color_to_rgb_direct() {
        assert_eq!(
            color_to_rgb(ratatui::style::Color::Rgb(100, 200, 50), true),
            (100, 200, 50)
        );
    }

    #[test]
    fn test_ansi256_standard_colors() {
        assert_eq!(ansi256_to_rgb(0), (0, 0, 0));
        assert_eq!(ansi256_to_rgb(15), (255, 255, 255));
    }

    #[test]
    fn test_ansi256_cube() {
        // Index 16 = r:0 g:0 b:0
        assert_eq!(ansi256_to_rgb(16), (0, 0, 0));
        // Index 196 = r:5 g:0 b:0 -> (255, 0, 0) ... 196 = 16 + 180 = 16 + 5*36
        assert_eq!(ansi256_to_rgb(196), (255, 0, 0));
    }

    #[test]
    fn test_ansi256_grayscale() {
        assert_eq!(ansi256_to_rgb(232), (8, 8, 8));
        assert_eq!(ansi256_to_rgb(255), (238, 238, 238));
    }

    #[test]
    fn test_blend() {
        assert_eq!(blend(255, 0, 1.0, 0.0), 255);
        assert_eq!(blend(0, 255, 0.0, 1.0), 255);
        // 50% blend
        let result = blend(200, 100, 0.5, 0.5);
        assert!((result as i32 - 150).abs() <= 1);
    }

    // -----------------------------------------------------------------------
    // Resize-redraw regression tests
    //
    // These tests reproduce the blank-screen-after-resize bug without a GPU.
    // The bug: WgpuBackend::resize() recreates the pixel buffer (all zeros),
    // but ratatui's internal diff state is NOT reset. On the next draw(),
    // ratatui compares previous vs current buffer — if the content is
    // identical (same render), the diff is empty, Backend::draw() receives
    // 0 cells, and the pixel buffer stays all zeros → blank screen.
    //
    // Fix: call terminal.clear() after resize, which resets ratatui's
    // "previous" buffer so the next diff sees all cells as changed.
    // -----------------------------------------------------------------------

    use ratatui::backend::{ClearType, WindowSize};
    use ratatui::buffer::Cell;
    use ratatui::layout::{Position, Size};
    use ratatui::style::{Color, Style};
    use ratatui::widgets::Paragraph;
    use ratatui::Terminal;
    use std::io;

    /// A minimal test double for `WgpuBackend` that uses a `Vec<u8>` pixel
    /// buffer instead of GPU resources. It implements the ratatui `Backend`
    /// trait and mimics the resize-clears-pixel-buffer behavior that caused
    /// the blank-screen bug.
    struct PixelBufferBackend {
        /// Grid size in columns
        cols: u16,
        /// Grid size in rows
        rows: u16,
        /// CPU pixel buffer (RGBA, row-major) — analogous to WgpuBackend::pixel_buffer
        pixel_buffer: Vec<u8>,
        /// Buffer width in pixels
        buf_width: u32,
        /// Buffer height in pixels
        buf_height: u32,
        /// Cursor position
        cursor_pos: Position,
        /// Whether the backend clear() was called (for assertions)
        cleared: bool,
        /// Count of cells written via draw() in the most recent call
        cells_drawn: usize,
        /// Track background color per cell for testing wide char bg propagation.
        /// Indexed as [row * cols + col].
        cell_bg: Vec<Color>,
    }

    impl PixelBufferBackend {
        /// Create a new test backend with the given grid dimensions.
        /// Each cell is assumed to be 10x20 pixels for simplicity.
        fn new(cols: u16, rows: u16) -> Self {
            let buf_width = cols as u32 * 10;
            let buf_height = rows as u32 * 20;
            Self {
                cols,
                rows,
                pixel_buffer: vec![0u8; (buf_width * buf_height * 4) as usize],
                buf_width,
                buf_height,
                cursor_pos: Position { x: 0, y: 0 },
                cleared: false,
                cells_drawn: 0,
                cell_bg: vec![Color::Reset; (cols as usize) * (rows as usize)],
            }
        }

        /// Mimic `WgpuBackend::resize()`: recreate the pixel buffer as all
        /// zeros. This is the exact behavior that causes the bug — the pixel
        /// data is gone but ratatui doesn't know about it.
        fn resize(&mut self, cols: u16, rows: u16) {
            self.cols = cols;
            self.rows = rows;
            self.buf_width = cols as u32 * 10;
            self.buf_height = rows as u32 * 20;
            // Recreate pixel buffer — all zeros, previous content is lost
            self.pixel_buffer = vec![0u8; (self.buf_width * self.buf_height * 4) as usize];
            self.cell_bg = vec![Color::Reset; (cols as usize) * (rows as usize)];
        }

        /// Check if the pixel buffer is entirely zeroed (blank screen).
        fn is_pixel_buffer_blank(&self) -> bool {
            self.pixel_buffer.iter().all(|&b| b == 0)
        }

        /// Return the number of cells drawn in the last Backend::draw() call.
        fn last_cells_drawn(&self) -> usize {
            self.cells_drawn
        }
    }

    impl PixelBufferBackend {
        /// Record the background color of a cell (for test assertions).
        fn set_cell_bg(&mut self, col: u16, row: u16, bg: Color) {
            let idx = row as usize * self.cols as usize + col as usize;
            if idx < self.cell_bg.len() {
                self.cell_bg[idx] = bg;
            }
        }

        /// Read the background color of a cell at grid position.
        fn get_cell_bg(&self, col: u16, row: u16) -> Color {
            let idx = row as usize * self.cols as usize + col as usize;
            self.cell_bg.get(idx).copied().unwrap_or(Color::Reset)
        }
    }

    impl ratatui::backend::Backend for PixelBufferBackend {
        type Error = io::Error;

        fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
        where
            I: Iterator<Item = (u16, u16, &'a Cell)>,
        {
            self.cells_drawn = 0;
            for (x, y, cell) in content {
                self.cells_drawn += 1;

                // Record background color for this cell
                self.set_cell_bg(x, y, cell.bg);

                // Write non-zero pixel data for each cell to simulate rendering.
                let (r, g, b) = color_to_rgb(cell.fg, true);
                let px_x = x as u32 * 10;
                let px_y = y as u32 * 20;
                for dy in 0..20u32 {
                    for dx in 0..10u32 {
                        let bx = px_x + dx;
                        let by = px_y + dy;
                        if bx < self.buf_width && by < self.buf_height {
                            let idx = ((by * self.buf_width + bx) * 4) as usize;
                            if idx + 3 < self.pixel_buffer.len() {
                                self.pixel_buffer[idx] = r;
                                self.pixel_buffer[idx + 1] = g;
                                self.pixel_buffer[idx + 2] = b;
                                self.pixel_buffer[idx + 3] = 255;
                            }
                        }
                    }
                }

                // Wide character continuation cell background propagation
                // (mirrors WgpuBackend::draw logic)
                let symbol = cell.symbol();
                if !symbol.is_empty() && symbol != " " {
                    let char_width = UnicodeWidthStr::width(symbol);
                    for dx in 1..char_width as u16 {
                        self.set_cell_bg(x + dx, y, cell.bg);
                    }
                }
            }
            Ok(())
        }

        fn hide_cursor(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn show_cursor(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
            Ok(self.cursor_pos)
        }

        fn set_cursor_position<P: Into<Position>>(
            &mut self,
            position: P,
        ) -> Result<(), Self::Error> {
            self.cursor_pos = position.into();
            Ok(())
        }

        fn clear(&mut self) -> Result<(), Self::Error> {
            self.pixel_buffer.fill(0);
            self.cleared = true;
            Ok(())
        }

        fn clear_region(&mut self, _clear_type: ClearType) -> Result<(), Self::Error> {
            self.clear()
        }

        fn size(&self) -> Result<Size, Self::Error> {
            Ok(Size::new(self.cols, self.rows))
        }

        fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
            Ok(WindowSize {
                columns_rows: Size::new(self.cols, self.rows),
                pixels: Size::new(self.buf_width as u16, self.buf_height as u16),
            })
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    /// Helper: render a simple paragraph that fills the entire frame area.
    fn render_content(terminal: &mut Terminal<PixelBufferBackend>) {
        terminal
            .draw(|frame| {
                let area = frame.area();
                let paragraph =
                    Paragraph::new("Hello rira!").style(Style::default().fg(Color::White));
                frame.render_widget(paragraph, area);
            })
            .expect("draw should succeed");
    }

    /// Verify that resize() zeros the pixel buffer, mimicking WgpuBackend behavior.
    #[test]
    fn test_resize_clears_pixel_buffer() {
        let mut backend = PixelBufferBackend::new(80, 24);

        // Write some non-zero data into the pixel buffer
        for byte in &mut backend.pixel_buffer {
            *byte = 0xFF;
        }
        assert!(!backend.is_pixel_buffer_blank());

        // After resize, pixel buffer should be all zeros
        backend.resize(100, 30);
        assert!(
            backend.is_pixel_buffer_blank(),
            "resize() must recreate the pixel buffer as all zeros"
        );
    }

    /// Regression test: after resize + terminal.clear(), the next draw must
    /// produce a full redraw (all cells sent to Backend::draw), so the pixel
    /// buffer is populated and the screen is not blank.
    #[test]
    fn test_clear_after_resize_enables_full_redraw() {
        let backend = PixelBufferBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("terminal creation should succeed");

        // Initial draw — populates both ratatui buffers and the pixel buffer
        render_content(&mut terminal);
        assert!(
            !terminal.backend().is_pixel_buffer_blank(),
            "pixel buffer should be non-blank after initial draw"
        );

        // Simulate resize: pixel buffer is recreated (all zeros)
        terminal.backend_mut().resize(100, 30);
        assert!(
            terminal.backend().is_pixel_buffer_blank(),
            "pixel buffer should be blank immediately after resize"
        );

        // The fix: call terminal.clear() to reset ratatui's diff state
        terminal.clear().expect("clear should succeed");

        // Now draw again — ratatui should send ALL cells because the
        // "previous" buffer was reset by clear()
        render_content(&mut terminal);

        let cells_drawn = terminal.backend().last_cells_drawn();
        assert!(
            cells_drawn > 0,
            "after resize + clear, draw must produce >0 cells, got {cells_drawn}"
        );
        assert!(
            !terminal.backend().is_pixel_buffer_blank(),
            "pixel buffer must not be blank after resize + clear + draw"
        );
    }

    /// Document the bug behavior: WITHOUT terminal.clear() after resize,
    /// ratatui's diff produces 0 changed cells because the previous and
    /// current buffers contain the same content. The pixel buffer stays
    /// blank → the user sees a blank screen.
    ///
    /// This test exists to document the failure mode. If ratatui ever
    /// changes its diff behavior, this test should be updated accordingly.
    #[test]
    fn test_draw_without_clear_after_resize_loses_content() {
        let backend = PixelBufferBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("terminal creation should succeed");

        // Initial draw — establishes ratatui's diff baseline
        render_content(&mut terminal);
        assert!(!terminal.backend().is_pixel_buffer_blank());

        // Simulate resize to the SAME size (so ratatui's autoresize doesn't
        // trigger its own internal resize, which calls clear). The pixel
        // buffer is recreated as all zeros but ratatui doesn't know.
        terminal.backend_mut().resize(80, 24);
        assert!(
            terminal.backend().is_pixel_buffer_blank(),
            "pixel buffer should be blank after resize"
        );

        // Draw WITHOUT calling terminal.clear() first.
        // Ratatui compares previous vs current — content is identical,
        // so diff yields 0 updates → Backend::draw() receives 0 cells.
        render_content(&mut terminal);

        let cells_drawn = terminal.backend().last_cells_drawn();
        assert_eq!(
            cells_drawn, 0,
            "BUG: without clear(), ratatui diff produces 0 cells \
             because both buffers match — pixel buffer stays blank. \
             Got {cells_drawn} cells instead of 0."
        );
        assert!(
            terminal.backend().is_pixel_buffer_blank(),
            "BUG: pixel buffer remains blank because no cells were drawn"
        );
    }

    // ── Zoom tests ──
    // These test zoom-related logic (constants, clamping, font scaling)
    // without requiring GPU resources.

    #[test]
    fn test_zoom_clamp_within_range() {
        let level = 1.5_f32;
        let clamped = level.clamp(MIN_ZOOM, MAX_ZOOM);
        assert!((clamped - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_zoom_clamp_below_min() {
        let level = 0.1_f32;
        let clamped = level.clamp(MIN_ZOOM, MAX_ZOOM);
        assert!((clamped - MIN_ZOOM).abs() < f32::EPSILON);
    }

    #[test]
    fn test_zoom_clamp_above_max() {
        let level = 5.0_f32;
        let clamped = level.clamp(MIN_ZOOM, MAX_ZOOM);
        assert!((clamped - MAX_ZOOM).abs() < f32::EPSILON);
    }

    #[test]
    fn test_zoom_in_step() {
        let current = 1.0_f32;
        let next = (current + ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
        assert!((next - 1.1).abs() < f32::EPSILON);
    }

    #[test]
    fn test_zoom_out_step() {
        let current = 1.0_f32;
        let next = (current - ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
        assert!((next - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_zoom_out_does_not_go_below_min() {
        // Use a value very close to MIN_ZOOM so subtracting ZOOM_STEP goes below
        let current = MIN_ZOOM + ZOOM_STEP * 0.5;
        let next = (current - ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
        assert!((next - MIN_ZOOM).abs() < f32::EPSILON);
    }

    #[test]
    fn test_zoom_in_does_not_go_above_max() {
        // Use a value very close to MAX_ZOOM so adding ZOOM_STEP goes above
        let current = MAX_ZOOM - ZOOM_STEP * 0.5;
        let next = (current + ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
        assert!((next - MAX_ZOOM).abs() < f32::EPSILON);
    }

    #[test]
    fn test_init_font_zoom_scales_cell_size() {
        let font_1x = WgpuBackend::init_font(1.0, 1.0);
        let font_2x_zoom = WgpuBackend::init_font(1.0, 2.0);

        // At 2x zoom, cell dimensions should be approximately 2x larger
        let width_ratio = font_2x_zoom.cell_width / font_1x.cell_width;
        let height_ratio = font_2x_zoom.cell_height / font_1x.cell_height;

        assert!(
            (width_ratio - 2.0).abs() < 0.1,
            "2x zoom should ~double cell_width, got ratio {width_ratio}"
        );
        assert!(
            (height_ratio - 2.0).abs() < f32::EPSILON,
            "2x zoom should double cell_height, got ratio {height_ratio}"
        );
    }

    #[test]
    fn test_init_font_zoom_half() {
        let font_1x = WgpuBackend::init_font(1.0, 1.0);
        let font_half = WgpuBackend::init_font(1.0, 0.5);

        let width_ratio = font_half.cell_width / font_1x.cell_width;
        let height_ratio = font_half.cell_height / font_1x.cell_height;

        assert!(
            (width_ratio - 0.5).abs() < 0.1,
            "0.5x zoom should ~halve cell_width, got ratio {width_ratio}"
        );
        assert!(
            (height_ratio - 0.5).abs() < f32::EPSILON,
            "0.5x zoom should halve cell_height, got ratio {height_ratio}"
        );
    }

    #[test]
    fn test_init_font_zoom_with_scale_factor() {
        // zoom=1.5 with scale_factor=2.0 should equal zoom=1.0 with scale_factor=3.0
        // because effective scale = scale_factor * zoom_level
        let font_a = WgpuBackend::init_font(2.0, 1.5); // effective 3.0
        let font_b = WgpuBackend::init_font(3.0, 1.0); // effective 3.0

        assert!(
            (font_a.cell_height - font_b.cell_height).abs() < f32::EPSILON,
            "same effective scale should give same cell_height"
        );
        // cell_width may have tiny differences due to font hinting, allow small tolerance
        assert!(
            (font_a.cell_width - font_b.cell_width).abs() < 0.5,
            "same effective scale should give similar cell_width: {} vs {}",
            font_a.cell_width,
            font_b.cell_width
        );
    }

    #[test]
    fn test_zoom_reset_value() {
        // Verify reset zoom gives same font as initial
        let font_initial = WgpuBackend::init_font(1.0, 1.0);
        let font_reset = WgpuBackend::init_font(1.0, 1.0);

        assert!((font_initial.cell_width - font_reset.cell_width).abs() < f32::EPSILON);
        assert!((font_initial.cell_height - font_reset.cell_height).abs() < f32::EPSILON);
    }

    // ── Wide character (CJK) selection background tests ──
    //
    // ratatui resets continuation cells of wide characters (Korean, CJK)
    // to default style via Cell::reset(). This causes selection highlights
    // to appear as individual blocks per character instead of a continuous
    // bar. The fix propagates the parent cell's background color to
    // continuation cells during draw().

    use ratatui::text::{Line, Span};
    use unicode_width::UnicodeWidthStr;

    /// Helper: render a Line with spans to the test backend and return
    /// the terminal so tests can inspect the pixel buffer.
    fn render_line_to_backend(
        cols: u16,
        rows: u16,
        line: Line<'_>,
    ) -> Terminal<PixelBufferBackend> {
        let backend = PixelBufferBackend::new(cols, rows);
        let mut terminal = Terminal::new(backend).expect("terminal creation should succeed");
        terminal.clear().expect("clear should succeed");
        terminal
            .draw(|frame| {
                let area = frame.area();
                let paragraph = Paragraph::new(line);
                frame.render_widget(paragraph, area);
            })
            .expect("draw should succeed");
        terminal
    }

    #[test]
    fn test_wide_char_continuation_cell_gets_selection_bg() {
        // Render a Korean character "한" with blue selection background.
        // "한" is a wide character (width=2), so it occupies cells 0 and 1.
        // Cell 1 is a continuation cell that ratatui resets to default.
        // Our fix should propagate the blue background to cell 1.
        let selection_style = Style::default().fg(Color::White).bg(Color::Blue);
        let line = Line::from(Span::styled("한", selection_style));

        let terminal = render_line_to_backend(10, 1, line);

        // Cell 0: the wide character itself — should have blue bg
        assert_eq!(
            terminal.backend().get_cell_bg(0, 0),
            Color::Blue,
            "cell 0 (wide char) should have selection bg"
        );

        // Cell 1: continuation cell — must also have blue bg (the fix)
        assert_eq!(
            terminal.backend().get_cell_bg(1, 0),
            Color::Blue,
            "cell 1 (continuation) should have selection bg"
        );
    }

    #[test]
    fn test_multiple_wide_chars_continuous_selection_bg() {
        // "한글" = two wide characters, occupying cells 0-1 and 2-3.
        // All four cells should have blue background for continuous selection.
        let selection_style = Style::default().fg(Color::White).bg(Color::Blue);
        let line = Line::from(Span::styled("한글", selection_style));

        let terminal = render_line_to_backend(10, 1, line);

        for col in 0..4u16 {
            assert_eq!(
                terminal.backend().get_cell_bg(col, 0),
                Color::Blue,
                "cell {col} should have selection bg for continuous highlight"
            );
        }
    }

    #[test]
    fn test_wide_char_per_char_spans_continuous_selection() {
        // Simulate the cursor-line rendering path where each character
        // gets its own Span. Both continuation cells should still have
        // the selection background.
        let selection_style = Style::default().fg(Color::White).bg(Color::Blue);
        let line = Line::from(vec![
            Span::styled("한", selection_style),
            Span::styled("글", selection_style),
        ]);

        let terminal = render_line_to_backend(10, 1, line);

        for col in 0..4u16 {
            assert_eq!(
                terminal.backend().get_cell_bg(col, 0),
                Color::Blue,
                "cell {col} (per-char spans) should have selection bg"
            );
        }
    }

    #[test]
    fn test_mixed_ascii_and_wide_char_selection() {
        // "aB한c" where "B한" is selected (blue bg).
        // Layout: a(0) B(1) 한(2,3) c(4)
        // Cells 1-3 should have blue bg, cells 0 and 4 should have default bg.
        let default_style = Style::default().fg(Color::White);
        let selection_style = Style::default().fg(Color::White).bg(Color::Blue);

        let line = Line::from(vec![
            Span::styled("a", default_style),
            Span::styled("B한", selection_style),
            Span::styled("c", default_style),
        ]);

        let terminal = render_line_to_backend(10, 1, line);

        // Cell 0: 'a' — default bg
        assert_eq!(terminal.backend().get_cell_bg(0, 0), Color::Reset);
        // Cell 1: 'B' — selection bg
        assert_eq!(terminal.backend().get_cell_bg(1, 0), Color::Blue);
        // Cell 2: '한' — selection bg
        assert_eq!(terminal.backend().get_cell_bg(2, 0), Color::Blue);
        // Cell 3: continuation of '한' — selection bg (the fix)
        assert_eq!(
            terminal.backend().get_cell_bg(3, 0),
            Color::Blue,
            "continuation cell of '한' in mixed text should have selection bg"
        );
        // Cell 4: 'c' — default bg
        assert_eq!(terminal.backend().get_cell_bg(4, 0), Color::Reset);
    }

    #[test]
    fn test_ascii_selection_unaffected() {
        // Pure ASCII selection should work as before — no continuation cells.
        let selection_style = Style::default().fg(Color::White).bg(Color::Blue);
        let line = Line::from(Span::styled("abc", selection_style));

        let terminal = render_line_to_backend(10, 1, line);

        for col in 0..3u16 {
            assert_eq!(
                terminal.backend().get_cell_bg(col, 0),
                Color::Blue,
                "ASCII cell {col} should have selection bg"
            );
        }
    }

    #[test]
    fn test_wide_char_no_selection_default_bg() {
        // Wide character without selection should have default background.
        let line = Line::from(Span::styled("한", Style::default().fg(Color::White)));

        let terminal = render_line_to_backend(10, 1, line);

        // Both cells (character + continuation) should have default bg
        assert_eq!(terminal.backend().get_cell_bg(0, 0), Color::Reset);
        assert_eq!(terminal.backend().get_cell_bg(1, 0), Color::Reset);
    }
}
