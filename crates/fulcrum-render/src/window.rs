//! The winit window and the deterministic fixed-timestep game loop.

use std::sync::Arc;
use std::time::Instant;

use bevy_ecs::prelude::Resource;
use fulcrum_asset::{AssetServer, Assets};
use fulcrum_core::{
    Fulcrum, FulcrumConfig, Input, IntoScheduleConfigs, Key, MouseButton, Plugin, Time, vec2,
};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::gpu::{self, GpuContext};

/// Longest frame the accumulator will absorb, in seconds. A stall longer than this (breakpoint,
/// laptop lid, ...) slows the simulation down instead of firing a catch-up burst of ticks.
const MAX_FRAME_TIME: f32 = 0.25;

/// Current window dimensions and scale factor. Updated on resize.
#[derive(Resource, Debug, Clone, Copy)]
pub struct WindowInfo {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// OS scale factor (HiDPI).
    pub scale_factor: f32,
}

/// Opens the window, owns the wgpu surface, and installs the winit runner that drives the
/// fixed-timestep loop: accumulate frame time, run zero or more simulation ticks, then render
/// with an interpolation factor.
pub struct WindowPlugin;

impl Plugin for WindowPlugin {
    fn build(&self, app: &mut Fulcrum) {
        let (width, height) = app.config().window_size;
        app.world_mut().insert_resource(WindowInfo {
            width,
            height,
            scale_factor: 1.0,
        });
        if app.world().get_resource::<AssetServer>().is_none() {
            app.world_mut().insert_resource(AssetServer::default());
        }
        app.world_mut()
            .insert_resource(Assets::<crate::texture::Texture>::default());
        app.world_mut()
            .insert_resource(Assets::<crate::atlas::SpriteSheet>::default());
        app.world_mut()
            .insert_resource(crate::batch::RenderStats::default());
        app.world_mut()
            .insert_resource(crate::camera::Camera2D::default());
        let gizmos_enabled = app.config().gizmos_enabled;
        app.world_mut()
            .insert_resource(crate::gizmos::Gizmos::new(gizmos_enabled));

        // Text: font storage, the embedded default font, and the glyph atlas.
        let mut fonts = Assets::<crate::text::Font>::default();
        let default_font =
            crate::text::Font::from_bytes("<default>", crate::text::DEFAULT_FONT_BYTES)
                .expect("embedded default font parses");
        let default_handle = fonts.insert_with_path("<default>", default_font);
        app.world_mut().insert_resource(fonts);
        app.world_mut()
            .insert_resource(crate::text::DefaultFont(default_handle));
        app.world_mut()
            .insert_resource(crate::text::GlyphCache::new(1024));
        app.world_mut()
            .insert_resource(crate::batch::ExtraQuads::default());

        app.add_systems(
            fulcrum_core::PreRender,
            (crate::text::extract_text, crate::batch::extract_sprites).chain(),
        );
        app.set_runner(winit_runner);
    }
}

fn winit_runner(app: Fulcrum) {
    let event_loop = EventLoop::new().expect("failed to create winit event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut handler = WinitApp {
        app,
        window: None,
        last_frame: None,
        accumulator: 0.0,
    };
    event_loop
        .run_app(&mut handler)
        .expect("winit event loop error");
}

struct WinitApp {
    app: Fulcrum,
    window: Option<Arc<Window>>,
    last_frame: Option<Instant>,
    accumulator: f32,
}

impl WinitApp {
    /// One rendered frame: absorb elapsed wall time, run due fixed ticks, then draw.
    fn frame(&mut self) {
        let now = Instant::now();
        let frame_delta = self
            .last_frame
            .map(|last| (now - last).as_secs_f32())
            .unwrap_or(0.0);
        self.last_frame = Some(now);

        let fixed_delta = self.app.world().resource::<Time>().fixed_delta;
        let window_size = {
            let info = self.app.world().resource::<WindowInfo>();
            vec2(info.width as f32, info.height as f32)
        };
        let camera = self
            .app
            .world()
            .resource::<crate::camera::Camera2D>()
            .clone();
        self.accumulator += frame_delta.min(MAX_FRAME_TIME);
        while self.accumulator >= fixed_delta {
            self.app
                .world_mut()
                .resource_mut::<Input>()
                .sample(|screen| camera.screen_to_world(screen, window_size));
            self.app.tick();
            self.accumulator -= fixed_delta;
        }

        {
            let mut time = self.app.world_mut().resource_mut::<Time>();
            time.frame_delta = frame_delta;
            time.alpha = self.accumulator / fixed_delta;
        }

        self.app.world_mut().run_schedule(fulcrum_core::Update);
        self.app.world_mut().run_schedule(fulcrum_core::PreRender);
        gpu::render(self.app.world_mut());
    }
}

impl ApplicationHandler for WinitApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let config = self.app.world().resource::<FulcrumConfig>();
        let attributes = Window::default_attributes()
            .with_title(config.title.clone())
            .with_inner_size(PhysicalSize::new(
                config.window_size.0,
                config.window_size.1,
            ));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("failed to create window"),
        );

        let size = window.inner_size();
        self.app.world_mut().insert_resource(WindowInfo {
            width: size.width,
            height: size.height,
            scale_factor: window.scale_factor() as f32,
        });
        let context = gpu::init(window.clone(), size.width, size.height);
        let renderer = crate::batch::SpriteRenderer::new(
            &context.device,
            &context.queue,
            context.surface_config.format,
        );
        self.app.world_mut().insert_resource(context);
        self.app.world_mut().insert_resource(renderer);
        self.window = Some(window);

        self.app.run_startup();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                {
                    let mut info = self.app.world_mut().resource_mut::<WindowInfo>();
                    info.width = size.width;
                    info.height = size.height;
                }
                self.app
                    .world_mut()
                    .resource_mut::<GpuContext>()
                    .resize(size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.app
                    .world_mut()
                    .resource_mut::<WindowInfo>()
                    .scale_factor = scale_factor as f32;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.repeat
                    && let PhysicalKey::Code(code) = event.physical_key
                    && let Some(key) = map_key(code)
                {
                    self.app
                        .world_mut()
                        .resource_mut::<Input>()
                        .push_key(key, event.state == ElementState::Pressed);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.app
                    .world_mut()
                    .resource_mut::<Input>()
                    .push_cursor(vec2(position.x as f32, position.y as f32));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(button) = map_mouse_button(button) {
                    self.app
                        .world_mut()
                        .resource_mut::<Input>()
                        .push_mouse_button(button, state == ElementState::Pressed);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    // Approximate pixels-per-line for touchpads/high-res wheels.
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
                };
                self.app
                    .world_mut()
                    .resource_mut::<Input>()
                    .push_scroll(lines);
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

/// Map winit physical key codes to Fulcrum keys (layout-independent).
fn map_key(code: KeyCode) -> Option<Key> {
    use KeyCode as C;
    Some(match code {
        C::KeyA => Key::A,
        C::KeyB => Key::B,
        C::KeyC => Key::C,
        C::KeyD => Key::D,
        C::KeyE => Key::E,
        C::KeyF => Key::F,
        C::KeyG => Key::G,
        C::KeyH => Key::H,
        C::KeyI => Key::I,
        C::KeyJ => Key::J,
        C::KeyK => Key::K,
        C::KeyL => Key::L,
        C::KeyM => Key::M,
        C::KeyN => Key::N,
        C::KeyO => Key::O,
        C::KeyP => Key::P,
        C::KeyQ => Key::Q,
        C::KeyR => Key::R,
        C::KeyS => Key::S,
        C::KeyT => Key::T,
        C::KeyU => Key::U,
        C::KeyV => Key::V,
        C::KeyW => Key::W,
        C::KeyX => Key::X,
        C::KeyY => Key::Y,
        C::KeyZ => Key::Z,
        C::Digit0 => Key::Digit0,
        C::Digit1 => Key::Digit1,
        C::Digit2 => Key::Digit2,
        C::Digit3 => Key::Digit3,
        C::Digit4 => Key::Digit4,
        C::Digit5 => Key::Digit5,
        C::Digit6 => Key::Digit6,
        C::Digit7 => Key::Digit7,
        C::Digit8 => Key::Digit8,
        C::Digit9 => Key::Digit9,
        C::ArrowUp => Key::Up,
        C::ArrowDown => Key::Down,
        C::ArrowLeft => Key::Left,
        C::ArrowRight => Key::Right,
        C::Space => Key::Space,
        C::Enter => Key::Enter,
        C::Escape => Key::Escape,
        C::Tab => Key::Tab,
        C::Backspace => Key::Backspace,
        C::ShiftLeft | C::ShiftRight => Key::Shift,
        C::ControlLeft | C::ControlRight => Key::Ctrl,
        C::AltLeft | C::AltRight => Key::Alt,
        C::F1 => Key::F1,
        C::F2 => Key::F2,
        C::F3 => Key::F3,
        C::F4 => Key::F4,
        C::F5 => Key::F5,
        C::F6 => Key::F6,
        C::F7 => Key::F7,
        C::F8 => Key::F8,
        C::F9 => Key::F9,
        C::F10 => Key::F10,
        C::F11 => Key::F11,
        C::F12 => Key::F12,
        _ => return None,
    })
}

/// Map winit mouse buttons to Fulcrum buttons.
fn map_mouse_button(button: winit::event::MouseButton) -> Option<MouseButton> {
    Some(match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        _ => return None,
    })
}
