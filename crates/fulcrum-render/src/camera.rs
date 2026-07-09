//! [`Camera2D`]: what part of the world is on screen, and how the window is filled.

use bevy_ecs::prelude::Resource;
use fulcrum_core::Vec2;
use glam::Mat4;

/// How the world is fitted into the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScalingMode {
    /// 1 world unit = 1 physical pixel (at `zoom` 1); the visible world grows with the window.
    Stretch,
    /// A fixed number of world units vertically; width follows the window's aspect ratio.
    FixedHeight(f32),
    /// A fixed virtual resolution, scaled to fit the window; unused space becomes black bars.
    Letterbox {
        /// Virtual width in world units.
        width: f32,
        /// Virtual height in world units.
        height: f32,
    },
    /// Like `Letterbox`, but only whole-number scale factors — pixel-art stays crisp. Windows
    /// smaller than the virtual resolution fall back to fractional letterbox scaling.
    IntegerScale {
        /// Virtual width in world units.
        width: f32,
        /// Virtual height in world units.
        height: f32,
    },
}

/// The one camera. A resource, not a component — split-screen is deliberately out of scope.
///
/// Move it freely from `Update` systems (camera position is cosmetic, e.g. a lerped follow);
/// the renderer reads it each frame.
#[derive(Resource, Debug, Clone)]
pub struct Camera2D {
    /// World position at the center of the screen.
    pub center: Vec2,
    /// Magnification: 2.0 shows half as much world, twice as large.
    pub zoom: f32,
    /// View rotation in radians (counter-clockwise world spin).
    pub rotation: f32,
    /// How the world fits the window.
    pub scaling: ScalingMode,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 1.0,
            rotation: 0.0,
            scaling: ScalingMode::Stretch,
        }
    }
}

/// Per-frame camera math handed to the renderer.
pub(crate) struct CameraFrame {
    /// World -> clip transform for the sprite/gizmo shaders.
    pub view_proj: Mat4,
    /// Viewport origin in physical pixels (top-left).
    pub viewport_origin: Vec2,
    /// Viewport size in physical pixels.
    pub viewport_size: Vec2,
    /// True when the viewport doesn't cover the whole window (bars are showing).
    pub letterboxed: bool,
    /// World-space quad exactly covering the visible area (bl, br, tr, tl) — used to paint the
    /// clear color inside the viewport when bars are showing.
    pub bg_corners: [Vec2; 4],
}

impl Camera2D {
    /// The world-unit size visible on screen for a window of `window` physical pixels.
    pub fn visible_world(&self, window: Vec2) -> Vec2 {
        let base = match self.scaling {
            ScalingMode::Stretch => window,
            ScalingMode::FixedHeight(height) => Vec2::new(height * window.x / window.y, height),
            ScalingMode::Letterbox { width, height }
            | ScalingMode::IntegerScale { width, height } => Vec2::new(width, height),
        };
        base / self.zoom
    }

    /// Viewport rect in physical pixels: `(origin_top_left, size)`.
    fn viewport(&self, window: Vec2) -> (Vec2, Vec2) {
        match self.scaling {
            ScalingMode::Stretch | ScalingMode::FixedHeight(_) => (Vec2::ZERO, window),
            ScalingMode::Letterbox { width, height } => {
                let scale = (window.x / width).min(window.y / height);
                let size = Vec2::new(width, height) * scale;
                ((window - size) / 2.0, size)
            }
            ScalingMode::IntegerScale { width, height } => {
                let scale = (window.x / width).min(window.y / height);
                // Whole-number scaling for crisp pixels; fractional fallback when the window is
                // smaller than the virtual resolution.
                let scale = if scale >= 1.0 { scale.floor() } else { scale };
                let size = Vec2::new(width, height) * scale;
                ((window - size) / 2.0, size)
            }
        }
    }

    /// Map a world position to physical-pixel screen coordinates (top-left origin, +Y down).
    pub fn world_to_screen(&self, world: Vec2, window: Vec2) -> Vec2 {
        let visible = self.visible_world(window);
        let (origin, size) = self.viewport(window);
        // View space: camera-relative, un-rotated.
        let view = rotate(world - self.center, -self.rotation);
        let ndc = Vec2::new(view.x / (visible.x / 2.0), view.y / (visible.y / 2.0));
        Vec2::new(
            origin.x + (ndc.x + 1.0) / 2.0 * size.x,
            origin.y + (1.0 - ndc.y) / 2.0 * size.y,
        )
    }

    /// Map physical-pixel screen coordinates (top-left origin, +Y down) to a world position.
    pub fn screen_to_world(&self, screen: Vec2, window: Vec2) -> Vec2 {
        let visible = self.visible_world(window);
        let (origin, size) = self.viewport(window);
        let ndc = Vec2::new(
            (screen.x - origin.x) / size.x * 2.0 - 1.0,
            1.0 - (screen.y - origin.y) / size.y * 2.0,
        );
        let view = Vec2::new(ndc.x * visible.x / 2.0, ndc.y * visible.y / 2.0);
        self.center + rotate(view, self.rotation)
    }

    /// Everything the renderer needs this frame.
    pub(crate) fn frame(&self, window: Vec2) -> CameraFrame {
        let visible = self.visible_world(window);
        let (viewport_origin, viewport_size) = self.viewport(window);
        let letterboxed = viewport_size != window;

        let projection = glam::camera::rh::proj::directx::orthographic(
            -visible.x / 2.0,
            visible.x / 2.0,
            -visible.y / 2.0,
            visible.y / 2.0,
            -1.0,
            1.0,
        );
        let view = Mat4::from_rotation_z(-self.rotation)
            * Mat4::from_translation((-self.center).extend(0.0));

        let half = visible / 2.0;
        let bg_corners = [
            Vec2::new(-half.x, -half.y),
            Vec2::new(half.x, -half.y),
            Vec2::new(half.x, half.y),
            Vec2::new(-half.x, half.y),
        ]
        .map(|corner| self.center + rotate(corner, self.rotation));

        CameraFrame {
            view_proj: projection * view,
            viewport_origin,
            viewport_size,
            letterboxed,
            bg_corners,
        }
    }
}

fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOWS: [Vec2; 3] = [
        Vec2::new(800.0, 600.0),
        Vec2::new(1280.0, 720.0),
        Vec2::new(500.0, 900.0),
    ];

    fn modes() -> Vec<ScalingMode> {
        vec![
            ScalingMode::Stretch,
            ScalingMode::FixedHeight(480.0),
            ScalingMode::Letterbox {
                width: 640.0,
                height: 360.0,
            },
            ScalingMode::IntegerScale {
                width: 320.0,
                height: 180.0,
            },
        ]
    }

    #[test]
    fn round_trips_in_all_modes_windows_zooms_rotations() {
        for scaling in modes() {
            for window in WINDOWS {
                for (zoom, rotation) in [(1.0, 0.0), (2.5, 0.0), (0.75, 0.7), (1.0, -2.1)] {
                    let camera = Camera2D {
                        center: Vec2::new(123.0, -45.0),
                        zoom,
                        rotation,
                        scaling,
                    };
                    for point in [Vec2::ZERO, Vec2::new(100.0, 50.0), Vec2::new(-321.0, 77.5)] {
                        let round =
                            camera.screen_to_world(camera.world_to_screen(point, window), window);
                        assert!(
                            (round - point).length() < 1e-2,
                            "{scaling:?} {window:?} zoom={zoom} rot={rotation}: \
                             {point:?} -> {round:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn camera_center_maps_to_viewport_center() {
        for scaling in modes() {
            let camera = Camera2D {
                center: Vec2::new(50.0, 60.0),
                zoom: 2.0,
                rotation: 0.4,
                scaling,
            };
            let window = Vec2::new(1000.0, 700.0);
            let screen = camera.world_to_screen(camera.center, window);
            assert!(
                (screen - window / 2.0).length() < 1e-3,
                "{scaling:?}: center -> {screen:?}"
            );
        }
    }

    #[test]
    fn letterbox_adds_bars_on_the_short_axis() {
        let camera = Camera2D {
            scaling: ScalingMode::Letterbox {
                width: 640.0,
                height: 360.0,
            },
            ..Default::default()
        };
        // A 4:3 window with 16:9 content: bars top and bottom.
        let frame = camera.frame(Vec2::new(800.0, 600.0));
        assert!(frame.letterboxed);
        assert_eq!(frame.viewport_size, Vec2::new(800.0, 450.0));
        assert_eq!(frame.viewport_origin, Vec2::new(0.0, 75.0));
    }

    #[test]
    fn integer_scale_snaps_to_whole_factors() {
        let camera = Camera2D {
            scaling: ScalingMode::IntegerScale {
                width: 320.0,
                height: 180.0,
            },
            ..Default::default()
        };
        // 1280x720 fits 320x180 exactly 4x: no bars.
        let frame = camera.frame(Vec2::new(1280.0, 720.0));
        assert_eq!(frame.viewport_size, Vec2::new(1280.0, 720.0));
        // 1000x700 -> floor(min(3.125, 3.888)) = 3x = 960x540, centered.
        let frame = camera.frame(Vec2::new(1000.0, 700.0));
        assert_eq!(frame.viewport_size, Vec2::new(960.0, 540.0));
        assert_eq!(frame.viewport_origin, Vec2::new(20.0, 80.0));
        // Window smaller than virtual resolution: fractional fallback, no crash.
        let frame = camera.frame(Vec2::new(200.0, 200.0));
        assert!(frame.viewport_size.x <= 200.0 && frame.viewport_size.y <= 200.0);
    }

    #[test]
    fn zoom_shrinks_the_visible_world() {
        let camera = Camera2D {
            zoom: 2.0,
            ..Default::default()
        };
        assert_eq!(
            camera.visible_world(Vec2::new(800.0, 600.0)),
            Vec2::new(400.0, 300.0)
        );
    }
}
