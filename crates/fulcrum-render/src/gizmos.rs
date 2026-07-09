//! [`Gizmos`]: immediate-mode debug drawing — the one blessed immediate API, for overlays only.
//!
//! Call the methods from any system (typically `Update`); shapes are drawn above all sprites
//! this frame, then forgotten. World-space coordinates, through the camera. Disabled via
//! [`FulcrumConfig::gizmos_enabled`](fulcrum_core::FulcrumConfig) (default: debug builds only),
//! in which case every call is a no-op and no GPU work happens.

use bevy_ecs::prelude::Resource;
use bytemuck::{Pod, Zeroable};
use fulcrum_core::{Color, Rect, Vec2};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GizmoVertex {
    pub(crate) position: [f32; 2],
    pub(crate) color: [f32; 4],
}

/// Immediate-mode debug shapes. Cleared every frame after rendering.
#[derive(Resource)]
pub struct Gizmos {
    enabled: bool,
    pub(crate) vertices: Vec<GizmoVertex>,
}

impl Gizmos {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            vertices: Vec::new(),
        }
    }

    /// Are gizmos being drawn this run?
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// A line segment from `a` to `b` (world units).
    pub fn line(&mut self, a: Vec2, b: Vec2, color: Color) {
        if !self.enabled {
            return;
        }
        let c = [color.r, color.g, color.b, color.a];
        self.vertices.push(GizmoVertex {
            position: a.into(),
            color: c,
        });
        self.vertices.push(GizmoVertex {
            position: b.into(),
            color: c,
        });
    }

    /// A rectangle outline.
    pub fn rect(&mut self, rect: Rect, color: Color) {
        let bl = rect.min;
        let br = Vec2::new(rect.max.x, rect.min.y);
        let tr = rect.max;
        let tl = Vec2::new(rect.min.x, rect.max.y);
        self.line(bl, br, color);
        self.line(br, tr, color);
        self.line(tr, tl, color);
        self.line(tl, bl, color);
    }

    /// A circle outline (32 segments).
    pub fn circle(&mut self, center: Vec2, radius: f32, color: Color) {
        const SEGMENTS: u32 = 32;
        let mut previous = center + Vec2::new(radius, 0.0);
        for i in 1..=SEGMENTS {
            let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let next = center + Vec2::new(angle.cos(), angle.sin()) * radius;
            self.line(previous, next, color);
            previous = next;
        }
    }

    /// A small cross marking a point (4 world units across).
    pub fn point(&mut self, p: Vec2, color: Color) {
        self.line(p - Vec2::new(2.0, 0.0), p + Vec2::new(2.0, 0.0), color);
        self.line(p - Vec2::new(0.0, 2.0), p + Vec2::new(0.0, 2.0), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fulcrum_core::vec2;

    #[test]
    fn disabled_gizmos_record_nothing() {
        let mut gizmos = Gizmos::new(false);
        gizmos.line(vec2(0.0, 0.0), vec2(1.0, 1.0), Color::WHITE);
        gizmos.rect(
            Rect::from_center_size(Vec2::ZERO, vec2(2.0, 2.0)),
            Color::WHITE,
        );
        gizmos.circle(Vec2::ZERO, 5.0, Color::WHITE);
        gizmos.point(Vec2::ZERO, Color::WHITE);
        assert!(gizmos.vertices.is_empty(), "no vertices -> zero GPU work");
    }

    #[test]
    fn primitives_emit_expected_vertex_counts() {
        let mut gizmos = Gizmos::new(true);
        gizmos.line(vec2(0.0, 0.0), vec2(1.0, 1.0), Color::WHITE);
        assert_eq!(gizmos.vertices.len(), 2);
        gizmos.rect(
            Rect::from_center_size(Vec2::ZERO, vec2(2.0, 2.0)),
            Color::WHITE,
        );
        assert_eq!(gizmos.vertices.len(), 2 + 8);
        gizmos.circle(Vec2::ZERO, 5.0, Color::WHITE);
        assert_eq!(gizmos.vertices.len(), 2 + 8 + 64);
        gizmos.point(Vec2::ZERO, Color::WHITE);
        assert_eq!(gizmos.vertices.len(), 2 + 8 + 64 + 4);
    }
}
