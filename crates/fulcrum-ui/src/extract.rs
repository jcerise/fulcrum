//! Turn laid-out UI nodes into screen-space quads for the renderer. Draw order = tree order.

use bevy_ecs::prelude::{Query, Res, ResMut};
use fulcrum_asset::Assets;
use fulcrum_core::{Rect, Vec2};
use fulcrum_render::texture::WhitePixel;
use fulcrum_render::{
    DefaultFont, ExtractedSprite, Font, GlyphCache, GpuContext, HAlign, Texture, UiQuads,
};

use crate::node::UiRect;
use crate::widgets::{ButtonState, UiButton, UiImage, UiLabel, UiPanel};

fn color_array(color: fulcrum_core::Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

/// Corners for a UI rect (top-left origin, +Y down); index 2 pairs with the texture top, same
/// convention as the world sprite path.
fn rect_corners(rect: Rect) -> [Vec2; 4] {
    [
        Vec2::new(rect.min.x, rect.max.y),
        Vec2::new(rect.max.x, rect.max.y),
        Vec2::new(rect.max.x, rect.min.y),
        Vec2::new(rect.min.x, rect.min.y),
    ]
}

const FULL_UV: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

fn push_rect(
    quads: &mut Vec<ExtractedSprite>,
    rect: Rect,
    texture: fulcrum_asset::Handle<Texture>,
    uv: [[f32; 2]; 4],
    color: [f32; 4],
) {
    quads.push(ExtractedSprite {
        z: 0.0, // UI draws in push order; z unused
        texture,
        corners: rect_corners(rect),
        uv,
        color,
    });
}

/// Nine-slice: split `rect` into 9 regions from `margins` = [left, top, right, bottom] texture
/// pixels, mapping matching UV regions.
fn push_nine_slice(
    quads: &mut Vec<ExtractedSprite>,
    rect: Rect,
    texture: fulcrum_asset::Handle<Texture>,
    texture_size: Vec2,
    margins: [f32; 4],
    color: [f32; 4],
) {
    let [left, top, right, bottom] = margins;
    let xs = [
        rect.min.x,
        rect.min.x + left,
        rect.max.x - right,
        rect.max.x,
    ];
    let ys = [
        rect.min.y,
        rect.min.y + top,
        rect.max.y - bottom,
        rect.max.y,
    ];
    let us = [
        0.0,
        left / texture_size.x,
        1.0 - right / texture_size.x,
        1.0,
    ];
    let vs = [
        0.0,
        top / texture_size.y,
        1.0 - bottom / texture_size.y,
        1.0,
    ];
    for row in 0..3 {
        for col in 0..3 {
            let slice = Rect::new(
                Vec2::new(xs[col], ys[row]),
                Vec2::new(xs[col + 1], ys[row + 1]),
            );
            if slice.size().x <= 0.0 || slice.size().y <= 0.0 {
                continue;
            }
            let uv = [
                [us[col], vs[row + 1]],
                [us[col + 1], vs[row + 1]],
                [us[col + 1], vs[row]],
                [us[col], vs[row]],
            ];
            push_rect(quads, slice, texture, uv, color);
        }
    }
}

/// `Update` system (after layout): emit quads for every visible widget, tree order.
#[allow(clippy::too_many_arguments, clippy::type_complexity)] // ECS systems legitimately take many resources
pub(crate) fn extract_ui(
    nodes: Query<(
        &UiRect,
        Option<&UiPanel>,
        Option<&UiLabel>,
        Option<&UiButton>,
        Option<&ButtonState>,
        Option<&UiImage>,
    )>,
    ui_quads: Option<ResMut<UiQuads>>,
    white: Option<Res<WhitePixel>>,
    gpu: Option<Res<GpuContext>>,
    textures: Option<ResMut<Assets<Texture>>>,
    fonts: Option<Res<Assets<Font>>>,
    default_font: Option<Res<DefaultFont>>,
    cache: Option<ResMut<GlyphCache>>,
) {
    let (
        Some(mut ui_quads),
        Some(white),
        Some(gpu),
        Some(mut textures),
        Some(fonts),
        Some(default_font),
        Some(mut cache),
    ) = (ui_quads, white, gpu, textures, fonts, default_font, cache)
    else {
        return;
    };
    let Some(font) = fonts.get(default_font.0) else {
        return;
    };

    let mut ordered: Vec<_> = nodes.iter().collect();
    ordered.sort_by_key(|(placed, ..)| placed.order);

    // Text is laid out first (glyph rects + colors recorded), pages flushed once, then handles
    // resolved — layout_ui/resolve_pages must pair in call order.
    let mut text_blocks: Vec<(Vec<fulcrum_render::UiGlyph>, Vec2, [f32; 4], u32)> = Vec::new();
    let mut solid_work: Vec<(u32, ExtractedSprite)> = Vec::new();

    for (placed, panel, label, button, state, image) in &ordered {
        let rect = placed.rect;
        if let Some(panel) = panel {
            let color = color_array(panel.color);
            match (panel.image, panel.nine_slice) {
                (Some(texture), Some(margins)) => {
                    let size = textures
                        .get(texture)
                        .map(|t| Vec2::new(t.width as f32, t.height as f32))
                        .unwrap_or(Vec2::ONE);
                    let mut quads = Vec::new();
                    push_nine_slice(&mut quads, rect, texture, size, margins, color);
                    for quad in quads {
                        solid_work.push((placed.order, quad));
                    }
                }
                (Some(texture), None) => {
                    let mut quads = Vec::new();
                    push_rect(&mut quads, rect, texture, FULL_UV, color);
                    solid_work.push((placed.order, quads.pop().unwrap()));
                }
                _ => {
                    let mut quads = Vec::new();
                    push_rect(&mut quads, rect, white.0, FULL_UV, color);
                    solid_work.push((placed.order, quads.pop().unwrap()));
                }
            }
        }
        if let Some(image) = image {
            let mut quads = Vec::new();
            push_rect(
                &mut quads,
                rect,
                image.image,
                FULL_UV,
                color_array(image.color),
            );
            solid_work.push((placed.order, quads.pop().unwrap()));
        }
        if let Some(button) = button {
            let style = button.style;
            let color = match state {
                Some(s) if s.pressed => style.pressed,
                Some(s) if s.hovered => style.hover,
                _ => style.normal,
            };
            let mut quads = Vec::new();
            push_rect(&mut quads, rect, white.0, FULL_UV, color_array(color));
            solid_work.push((placed.order, quads.pop().unwrap()));
            if !button.text.is_empty() {
                let glyphs = cache.layout_ui(
                    font,
                    default_font.0.id(),
                    &button.text,
                    button.text_size,
                    HAlign::Center,
                    rect.size().x,
                );
                let text_height = GlyphCache::measure(font, &button.text, button.text_size).y;
                let origin = Vec2::new(rect.min.x, rect.center().y - text_height / 2.0);
                text_blocks.push((
                    glyphs,
                    origin,
                    color_array(fulcrum_core::Color::WHITE),
                    placed.order,
                ));
            }
        }
        if let Some(label) = label {
            let glyphs = cache.layout_ui(
                font,
                default_font.0.id(),
                &label.text,
                label.size,
                label.h_align,
                rect.size().x,
            );
            text_blocks.push((glyphs, rect.min, color_array(label.color), placed.order));
        }
    }

    cache.flush_now(&gpu, &mut textures);
    let page_px = cache.page_px();

    // Merge solids and text by tree order (text above its own node's background).
    let mut text_quads: Vec<(u32, ExtractedSprite)> = Vec::new();
    for (mut glyphs, origin, color, order) in text_blocks {
        cache.resolve_pages(&mut glyphs);
        for glyph in glyphs {
            let rect = Rect::from_min_size(origin + glyph.rect.min, glyph.rect.size());
            let (u0, u1) = (glyph.uv_px.min.x / page_px, glyph.uv_px.max.x / page_px);
            let (v0, v1) = (glyph.uv_px.min.y / page_px, glyph.uv_px.max.y / page_px);
            text_quads.push((
                order,
                ExtractedSprite {
                    z: 0.0,
                    texture: glyph.page,
                    corners: rect_corners(rect),
                    uv: [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
                    color,
                },
            ));
        }
    }
    let mut merged: Vec<(u32, u8, ExtractedSprite)> = solid_work
        .into_iter()
        .map(|(order, quad)| (order, 0u8, quad))
        .chain(
            text_quads
                .into_iter()
                .map(|(order, quad)| (order, 1u8, quad)),
        )
        .collect();
    merged.sort_by_key(|(order, kind, _)| (*order, *kind));
    ui_quads
        .0
        .extend(merged.into_iter().map(|(_, _, quad)| quad));
}
