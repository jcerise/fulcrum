//! Text rendering: fontdue rasterization into a shelf-packed glyph atlas, drawn as batched
//! quads through the sprite pipeline.
//!
//! Spawn an entity with [`Text`] + `Transform2D` and it renders. The entity's translation is
//! the **baseline-left of the first line** (before [`HAlign`] adjustment). Single lines plus
//! `\n` only — wrapping and rich text arrive with the phase-3 UI.

use bevy_ecs::prelude::{Component, Query, Res, ResMut, Resource};
use fulcrum_asset::{AssetError, AssetServer, Assets, Handle};
use fulcrum_core::{Color, PreviousTransform2D, Rect, Time, Transform2D, Vec2};
use rustc_hash::FxHashMap;

use crate::batch::{ExtraQuads, ExtractedSprite};
use crate::gpu::GpuContext;
use crate::texture::Texture;

/// The engine's built-in font ("Press Start 2P", SIL OFL 1.1 — see `assets/fonts/OFL.txt`).
pub(crate) const DEFAULT_FONT_BYTES: &[u8] =
    include_bytes!("../assets/fonts/PressStart2P-Regular.ttf");

/// A parsed font asset.
pub struct Font(pub(crate) fontdue::Font);

impl Font {
    /// Parse TTF/OTF bytes.
    pub fn from_bytes(path: &str, bytes: &[u8]) -> Result<Self, AssetError> {
        fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map(Font)
            .map_err(|message| AssetError::Decode {
                path: path.to_string(),
                message: message.to_string(),
            })
    }
}

/// Handle to the built-in pixel font, inserted by the window plugin.
#[derive(Resource, Clone, Copy)]
pub struct DefaultFont(pub Handle<Font>);

/// Horizontal alignment of each line relative to the entity's translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum HAlign {
    /// Lines start at the translation (default).
    #[default]
    Left,
    /// Lines center on the translation.
    Center,
    /// Lines end at the translation.
    Right,
}

/// A text label drawn at the entity's `Transform2D`.
#[derive(Component, Clone)]
pub struct Text {
    /// What to display. `\n` starts a new line.
    pub value: String,
    /// Font to use; [`Handle::INVALID`] (the default) means the built-in pixel font.
    pub font: Handle<Font>,
    /// Size in pixels (the built-in pixel font is sharpest at multiples of 8).
    pub size: f32,
    /// Text color.
    pub color: Color,
    /// Per-line horizontal alignment.
    pub h_align: HAlign,
    /// Draw order among sprites: higher draws in front.
    pub z: f32,
}

impl Text {
    /// White, left-aligned, 16 px, built-in font.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            font: Handle::INVALID,
            size: 16.0,
            color: Color::WHITE,
            h_align: HAlign::Left,
            z: 0.0,
        }
    }

    /// Builder-style size (pixels).
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Builder-style color.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Builder-style alignment.
    pub fn with_align(mut self, h_align: HAlign) -> Self {
        self.h_align = h_align;
        self
    }

    /// Builder-style draw order.
    pub fn with_z(mut self, z: f32) -> Self {
        self.z = z;
        self
    }
}

/// One rasterized glyph's home in the atlas.
#[derive(Clone, Copy)]
struct CachedGlyph {
    page: u32,
    /// Pixel rect within the page (image space, top-left origin).
    rect: Rect,
    metrics: fontdue::Metrics,
}

/// A pending CPU->GPU pixel upload.
struct PendingUpload {
    page: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// Shelf-packing state for one atlas page.
struct Shelf {
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

/// The glyph atlas: rasterizes `(font, size, char)` on demand into shelf-packed pages.
/// Packing and caching are CPU-only (unit-testable); [`flush`](Self::flush) does the GPU work.
#[derive(Resource)]
pub(crate) struct GlyphCache {
    page_size: u32,
    shelves: Vec<Shelf>,
    entries: FxHashMap<(u32, u32, char), Option<CachedGlyph>>,
    pending: Vec<PendingUpload>,
    page_textures: Vec<Handle<Texture>>,
    hits: u64,
    misses: u64,
}

impl GlyphCache {
    pub(crate) fn new(page_size: u32) -> Self {
        Self {
            page_size,
            shelves: Vec::new(),
            entries: FxHashMap::default(),
            pending: Vec::new(),
            page_textures: Vec::new(),
            hits: 0,
            misses: 0,
        }
    }

    /// Number of atlas pages allocated so far.
    #[cfg(test)]
    fn page_count(&self) -> usize {
        self.shelves.len()
    }

    /// Forget every cached glyph (font hot reload). Existing atlas pages are reused; stale
    /// pixels get overwritten as glyphs re-rasterize.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.shelves.clear();
        self.pending.clear();
    }

    /// Fetch (or rasterize and pack) a glyph. `None` for whitespace/zero-size glyphs.
    fn glyph(
        &mut self,
        font: &fontdue::Font,
        font_id: u32,
        ch: char,
        px: u32,
    ) -> Option<CachedGlyph> {
        if let Some(cached) = self.entries.get(&(font_id, px, ch)) {
            self.hits += 1;
            return *cached;
        }
        self.misses += 1;
        let (metrics, coverage) = font.rasterize(ch, px as f32);
        let entry = if metrics.width == 0 || metrics.height == 0 {
            None
        } else {
            let (page, x, y) = self.allocate(metrics.width as u32, metrics.height as u32);
            // White text, coverage as alpha; tinting happens per-vertex.
            let mut rgba = Vec::with_capacity(coverage.len() * 4);
            for alpha in coverage {
                rgba.extend_from_slice(&[255, 255, 255, alpha]);
            }
            self.pending.push(PendingUpload {
                page,
                x,
                y,
                width: metrics.width as u32,
                height: metrics.height as u32,
                rgba,
            });
            Some(CachedGlyph {
                page,
                rect: Rect::from_min_size(
                    Vec2::new(x as f32, y as f32),
                    Vec2::new(metrics.width as f32, metrics.height as f32),
                ),
                metrics,
            })
        };
        self.entries.insert((font_id, px, ch), entry);
        entry
    }

    /// Shelf-pack a `w x h` region (1 px padding); grows by whole pages when full.
    fn allocate(&mut self, w: u32, h: u32) -> (u32, u32, u32) {
        let padded_w = w + 1;
        let padded_h = h + 1;
        loop {
            let page = self.shelves.len().wrapping_sub(1) as u32;
            if let Some(shelf) = self.shelves.last_mut() {
                // Fits in the current row?
                if shelf.cursor_x + padded_w <= self.page_size
                    && shelf.cursor_y + padded_h <= self.page_size
                {
                    let (x, y) = (shelf.cursor_x, shelf.cursor_y);
                    shelf.cursor_x += padded_w;
                    shelf.row_height = shelf.row_height.max(padded_h);
                    return (page, x, y);
                }
                // Start a new row?
                if shelf.cursor_y + shelf.row_height + padded_h <= self.page_size
                    && padded_w <= self.page_size
                {
                    shelf.cursor_y += shelf.row_height;
                    shelf.cursor_x = 0;
                    shelf.row_height = 0;
                    continue;
                }
            }
            // New page.
            assert!(
                padded_w <= self.page_size && padded_h <= self.page_size,
                "glyph larger than an atlas page ({}px)",
                self.page_size
            );
            self.shelves.push(Shelf {
                cursor_x: 0,
                cursor_y: 0,
                row_height: 0,
            });
        }
    }

    /// Create any missing page textures and upload pending glyph pixels. GPU side of the cache.
    fn flush(&mut self, gpu: &GpuContext, textures: &mut Assets<Texture>) {
        while self.page_textures.len() < self.shelves.len() {
            let blank = vec![0u8; (self.page_size * self.page_size * 4) as usize];
            let texture = crate::texture::upload_raw(
                &gpu.device,
                &gpu.queue,
                "glyph atlas page",
                &blank,
                self.page_size,
                self.page_size,
            );
            self.page_textures.push(textures.insert(texture));
        }
        for upload in self.pending.drain(..) {
            let handle = self.page_textures[upload.page as usize];
            let Some(texture) = textures.get(handle) else {
                continue;
            };
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: upload.x,
                        y: upload.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &upload.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * upload.width),
                    rows_per_image: Some(upload.height),
                },
                wgpu::Extent3d {
                    width: upload.width,
                    height: upload.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
}

/// A laid-out glyph, before the entity transform is applied: `local` is relative to the
/// baseline-left origin in +Y-up units; `uv_px` is the atlas rect.
struct GlyphQuad {
    page: u32,
    uv_px: Rect,
    local: Rect,
}

/// Lay out `text`, rasterizing glyphs into the cache as needed. Pure CPU; unit-testable.
fn layout(
    text: &Text,
    font: &fontdue::Font,
    font_id: u32,
    cache: &mut GlyphCache,
) -> Vec<GlyphQuad> {
    let px = text.size.round().max(1.0) as u32;
    let line_height = font
        .horizontal_line_metrics(px as f32)
        .map(|m| m.new_line_size)
        .unwrap_or(px as f32);

    let mut quads = Vec::new();
    for (line_index, line) in text.value.split('\n').enumerate() {
        // Measure pass for alignment.
        let mut width = 0.0f32;
        let mut prev: Option<char> = None;
        for ch in line.chars() {
            if let Some(previous) = prev {
                width += font.horizontal_kern(previous, ch, px as f32).unwrap_or(0.0);
            }
            width += font.metrics(ch, px as f32).advance_width;
            prev = Some(ch);
        }
        let mut pen_x = match text.h_align {
            HAlign::Left => 0.0,
            HAlign::Center => -width / 2.0,
            HAlign::Right => -width,
        };
        let pen_y = -(line_index as f32) * line_height;

        let mut prev: Option<char> = None;
        for ch in line.chars() {
            if let Some(previous) = prev {
                pen_x += font.horizontal_kern(previous, ch, px as f32).unwrap_or(0.0);
            }
            if let Some(glyph) = cache.glyph(font, font_id, ch, px) {
                let min = Vec2::new(
                    pen_x + glyph.metrics.xmin as f32,
                    pen_y + glyph.metrics.ymin as f32,
                );
                quads.push(GlyphQuad {
                    page: glyph.page,
                    uv_px: glyph.rect,
                    local: Rect::from_min_size(
                        min,
                        Vec2::new(glyph.metrics.width as f32, glyph.metrics.height as f32),
                    ),
                });
            }
            pen_x += font.metrics(ch, px as f32).advance_width;
            prev = Some(ch);
        }
    }
    quads
}

/// `PreRender` system (before sprite extraction): turn every `Text` entity into glyph quads.
#[allow(clippy::too_many_arguments)] // ECS systems legitimately take many resources
pub(crate) fn extract_text(
    texts: Query<(&Text, &Transform2D, &PreviousTransform2D)>,
    fonts: Res<Assets<Font>>,
    default_font: Res<DefaultFont>,
    gpu: Res<GpuContext>,
    time: Res<Time>,
    mut textures: ResMut<Assets<Texture>>,
    mut cache: ResMut<GlyphCache>,
    mut extra: ResMut<ExtraQuads>,
) {
    let mut placed: Vec<(GlyphQuad, Transform2D, Color, f32)> = Vec::new();
    for (text, transform, previous) in &texts {
        let font_handle = if text.font == Handle::INVALID {
            default_font.0
        } else {
            text.font
        };
        let Some(font) = fonts.get(font_handle) else {
            continue;
        };
        let interpolated = previous.0.lerp(transform, time.alpha);
        for quad in layout(text, &font.0, font_handle.id(), &mut cache) {
            placed.push((quad, interpolated, text.color, text.z));
        }
    }

    // Create/refresh atlas pages before quads reference them.
    cache.flush(&gpu, &mut textures);

    let page_px = cache.page_size as f32;
    for (quad, transform, color, z) in placed {
        let (sin, cos) = transform.rotation.sin_cos();
        let corners = [
            Vec2::new(quad.local.min.x, quad.local.min.y),
            Vec2::new(quad.local.max.x, quad.local.min.y),
            Vec2::new(quad.local.max.x, quad.local.max.y),
            Vec2::new(quad.local.min.x, quad.local.max.y),
        ]
        .map(|local| {
            let scaled = local * transform.scale;
            Vec2::new(
                scaled.x * cos - scaled.y * sin,
                scaled.x * sin + scaled.y * cos,
            ) + transform.translation
        });
        let (u0, u1) = (quad.uv_px.min.x / page_px, quad.uv_px.max.x / page_px);
        let (v_top, v_bottom) = (quad.uv_px.min.y / page_px, quad.uv_px.max.y / page_px);
        extra.0.push(ExtractedSprite {
            z,
            texture: cache.page_textures[quad.page as usize],
            corners,
            uv: [[u0, v_bottom], [u1, v_bottom], [u1, v_top], [u0, v_top]],
            color: [color.r, color.g, color.b, color.a],
        });
    }
}

/// Extension for [`AssetLoader`](crate::texture::AssetLoader)-style font loading; wired as a
/// method there.
pub(crate) fn load_font(
    server: &AssetServer,
    fonts: &mut Assets<Font>,
    path: &str,
) -> Result<Handle<Font>, AssetError> {
    if let Some(handle) = fonts.handle_for_path(path) {
        return Ok(handle);
    }
    let bytes = server.read_bytes(path)?;
    let font = Font::from_bytes(path, &bytes)?;
    Ok(fonts.insert_with_path(path, font))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_font() -> Font {
        Font::from_bytes("<default>", DEFAULT_FONT_BYTES).unwrap()
    }

    #[test]
    fn repeated_layout_hits_cache_and_stops_allocating() {
        let font = test_font();
        let mut cache = GlyphCache::new(512);

        // Simulate a score label changing every tick for 200 "ticks".
        for tick in 0..200u32 {
            let text = Text::new(format!("Score: {}", tick % 100)).with_size(16.0);
            layout(&text, &font.0, 0, &mut cache);
        }
        let pages_after_warmup = cache.page_count();
        let entries_after_warmup = cache.entries.len();
        let misses_after_warmup = cache.misses;

        for tick in 0..200u32 {
            let text = Text::new(format!("Score: {}", tick % 100)).with_size(16.0);
            layout(&text, &font.0, 0, &mut cache);
        }
        assert_eq!(cache.page_count(), pages_after_warmup, "no new pages");
        assert_eq!(cache.entries.len(), entries_after_warmup, "no new glyphs");
        assert_eq!(cache.misses, misses_after_warmup, "all cache hits");
        assert!(cache.hits > 0);
    }

    #[test]
    fn atlas_grows_by_pages_when_full() {
        let font = test_font();
        // Tiny page: a few 32 px glyphs fill it.
        let mut cache = GlyphCache::new(64);
        let text = Text::new("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdefghij").with_size(32.0);
        layout(&text, &font.0, 0, &mut cache);
        assert!(
            cache.page_count() > 1,
            "expected multiple pages, got {}",
            cache.page_count()
        );
    }

    #[test]
    fn alignment_shifts_lines() {
        let font = test_font();
        let mut cache = GlyphCache::new(512);
        let left = layout(&Text::new("MM"), &font.0, 0, &mut cache);
        let center = layout(
            &Text::new("MM").with_align(HAlign::Center),
            &font.0,
            0,
            &mut cache,
        );
        let right = layout(
            &Text::new("MM").with_align(HAlign::Right),
            &font.0,
            0,
            &mut cache,
        );
        assert!(left[0].local.min.x >= 0.0);
        assert!(center[0].local.min.x < 0.0 && center[0].local.max.x < left[0].local.max.x);
        assert!(right.last().unwrap().local.max.x <= 1.0);
    }

    #[test]
    fn newline_stacks_lines_downward() {
        let font = test_font();
        let mut cache = GlyphCache::new(512);
        let quads = layout(&Text::new("A\nB"), &font.0, 0, &mut cache);
        assert_eq!(quads.len(), 2);
        assert!(
            quads[1].local.min.y < quads[0].local.min.y,
            "second line below the first (+Y up)"
        );
    }
}
