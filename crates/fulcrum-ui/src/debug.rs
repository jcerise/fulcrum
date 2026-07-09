//! The egui debug overlay: F12 toggles a world inspector (entities with editable registered
//! components), performance stats, and loaded-asset lists with force-reload buttons.
//!
//! Dev tooling only — editing state here invalidates determinism/replay guarantees for the run
//! (a notice is logged on the first edit).

use std::sync::Mutex;

use bevy_ecs::prelude::{Entity, Local, Res, ResMut, Resource};
use bevy_ecs::world::World;
use fulcrum_asset::{AssetEvent, Assets};
use fulcrum_core::{Fulcrum, Input, Key, Name, Plugin, Time, Update};
use fulcrum_render::{
    GpuContext, RenderOverlay, RenderStats, Texture, WindowEventHooks, WindowHandle,
};

/// Overlay visibility (toggled with F12).
#[derive(Resource, Default)]
pub struct DebugUi {
    /// Whether the overlay is showing.
    pub open: bool,
}

/// Whether egui wants the pointer/keyboard — game input systems should ignore clicks/typing
/// when set (consumed events already don't reach `Input`).
#[derive(Resource, Default, Clone, Copy)]
pub struct DebugUiFocus {
    /// egui is using the pointer.
    pub wants_pointer: bool,
    /// egui is using the keyboard (e.g. a text field is focused).
    pub wants_keyboard: bool,
}

struct EguiParts {
    ctx: egui::Context,
    winit: Mutex<Option<egui_winit::State>>,
    renderer: Mutex<Option<egui_wgpu::Renderer>>,
    edited_once: Mutex<bool>,
}

#[derive(Resource)]
struct EguiRes(EguiParts);

/// Installs the F12 egui overlay. Part of `DefaultPlugins` in debug builds; add explicitly for
/// release tooling.
pub struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(DebugUi::default());
        app.world_mut().insert_resource(DebugUiFocus::default());
        app.world_mut().insert_resource(EguiRes(EguiParts {
            ctx: egui::Context::default(),
            winit: Mutex::new(None),
            renderer: Mutex::new(None),
            edited_once: Mutex::new(false),
        }));
        if app.world().get_resource::<WindowEventHooks>().is_none() {
            app.world_mut().insert_resource(WindowEventHooks::default());
        }
        app.world_mut()
            .resource_mut::<WindowEventHooks>()
            .0
            .push(Box::new(window_event_hook));
        app.world_mut()
            .insert_resource(RenderOverlay(Box::new(render_overlay)));
        app.add_systems(Update, toggle_overlay);
    }
}

/// F12 toggles the overlay (frame-edge latched: `Input` state spans several frames per tick).
fn toggle_overlay(
    input: Option<Res<Input>>,
    mut debug: ResMut<DebugUi>,
    mut was_down: Local<bool>,
) {
    let Some(input) = input else { return };
    let down = input.pressed(Key::F12);
    if down && !*was_down {
        debug.open = !debug.open;
    }
    *was_down = down;
}

fn window_event_hook(world: &mut World, event: &winit::event::WindowEvent) -> bool {
    let Some(window) = world.get_resource::<WindowHandle>().map(|w| w.0.clone()) else {
        return false;
    };
    let open = world.resource::<DebugUi>().open;
    let parts = &world.resource::<EguiRes>().0;
    let response = {
        let mut guard = parts.winit.lock().unwrap();
        let state = guard.get_or_insert_with(|| {
            egui_winit::State::new(
                parts.ctx.clone(),
                egui::ViewportId::ROOT,
                &window,
                Some(window.scale_factor() as f32),
                None,
                None,
            )
        });
        state.on_window_event(&window, event)
    };
    let focus = DebugUiFocus {
        wants_pointer: open && parts.ctx.egui_wants_pointer_input(),
        wants_keyboard: open && parts.ctx.egui_wants_keyboard_input(),
    };
    world.insert_resource(focus);
    response.consumed && open
}

fn render_overlay(world: &mut World, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
    if !world.resource::<DebugUi>().open {
        return;
    }
    let Some(window) = world.get_resource::<WindowHandle>().map(|w| w.0.clone()) else {
        return;
    };
    let Some(parts) = world.remove_resource::<EguiRes>() else {
        return;
    };
    let parts = parts.0;

    let raw_input = {
        let mut guard = parts.winit.lock().unwrap();
        guard.as_mut().map(|state| state.take_egui_input(&window))
    };
    let Some(raw_input) = raw_input else {
        // No events processed yet (state is created by the first window event).
        world.insert_resource(EguiRes(parts));
        return;
    };
    let ctx = parts.ctx.clone();
    let edited_flag = &parts.edited_once;
    ctx.begin_pass(raw_input);
    draw_windows(&ctx, world, edited_flag);
    let output = ctx.end_pass();
    {
        let mut guard = parts.winit.lock().unwrap();
        if let Some(state) = guard.as_mut() {
            state.handle_platform_output(&window, output.platform_output);
        }
    }

    let pixels_per_point = ctx.pixels_per_point();
    let jobs = ctx.tessellate(output.shapes, pixels_per_point);
    let (device, queue, format, size) = {
        let gpu = world.resource::<GpuContext>();
        (
            gpu.device.clone(),
            gpu.queue.clone(),
            gpu.surface_config.format,
            [gpu.surface_config.width, gpu.surface_config.height],
        )
    };
    {
        let mut guard = parts.renderer.lock().unwrap();
        let renderer = guard.get_or_insert_with(|| {
            egui_wgpu::Renderer::new(&device, format, egui_wgpu::RendererOptions::default())
        });
        for (id, delta) in &output.textures_delta.set {
            renderer.update_texture(&device, &queue, *id, delta);
        }
        let descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point,
        };
        let user_buffers = renderer.update_buffers(&device, &queue, encoder, &jobs, &descriptor);
        if !user_buffers.is_empty() {
            queue.submit(user_buffers);
        }
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui overlay"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            renderer.render(&mut pass, &jobs, &descriptor);
        }
        for id in &output.textures_delta.free {
            renderer.free_texture(id);
        }
    }
    world.insert_resource(EguiRes(parts));
}

fn draw_windows(ctx: &egui::Context, world: &mut World, edited_once: &Mutex<bool>) {
    egui::Window::new("Performance")
        .default_pos([16.0, 16.0])
        .show(ctx, |ui| {
            let time = *world.resource::<Time>();
            let stats = *world.resource::<RenderStats>();
            ui.label(format!(
                "frame: {:.2} ms ({:.0} fps)",
                time.frame_delta * 1000.0,
                if time.frame_delta > 0.0 {
                    1.0 / time.frame_delta
                } else {
                    0.0
                }
            ));
            ui.label(format!("tick: {}", time.tick));
            ui.label(format!(
                "sprites: {} | batches: {} | tilemap chunks: {}",
                stats.sprites, stats.batches, stats.tilemap_chunks
            ));
        });

    egui::Window::new("Entities")
        .default_pos([16.0, 140.0])
        .default_width(340.0)
        .vscroll(true)
        .show(ctx, |ui| {
            entities_ui(ui, world, edited_once);
        });

    egui::Window::new("Assets")
        .default_pos([16.0, 420.0])
        .vscroll(true)
        .show(ctx, |ui| {
            assets_ui(ui, world);
        });
}

fn entities_ui(ui: &mut egui::Ui, world: &mut World, edited_once: &Mutex<bool>) {
    let mut entities: Vec<Entity> = world.query::<Entity>().iter(world).collect();
    entities.sort_unstable();

    world.resource_scope(
        |world, registry: bevy_ecs::world::Mut<fulcrum_scene::ComponentRegistry>| {
            // Only show entities with at least one registered component (bevy_ecs models
            // resources as entities; those are noise here).
            entities.retain(|&entity| {
                registry
                    .names()
                    .iter()
                    .any(|name| registry.extract_from(&world.entity(entity), name).is_some())
            });
            ui.label(format!("{} entities", entities.len()));
            for entity in entities {
                let name = world
                    .entity(entity)
                    .get::<Name>()
                    .map(|n| n.0.clone())
                    .unwrap_or_default();
                let header = format!("{entity:?} {name}");
                egui::CollapsingHeader::new(header)
                    .id_salt(entity)
                    .show(ui, |ui| {
                        for component in registry.names() {
                            let Some(mut value) =
                                registry.extract_from(&world.entity(entity), component)
                            else {
                                continue;
                            };
                            ui.label(egui::RichText::new(component).strong());
                            if value_ui(ui, &mut value, component) {
                                let mut target = world.entity_mut(entity);
                                if let Err(error) =
                                    registry.insert_on(&mut target, component, &value)
                                {
                                    log::error!("inspector edit: {error}");
                                } else {
                                    let mut edited = edited_once.lock().unwrap();
                                    if !*edited {
                                        *edited = true;
                                        log::warn!(
                                            "inspector edited live state: determinism/replay \
                                         guarantees no longer hold for this run"
                                        );
                                    }
                                }
                            }
                        }
                    });
            }
        },
    );
}

/// Generic editor over a RON value: maps recurse, numbers drag, bools check, strings edit,
/// short number sequences edit inline. Returns true if anything changed.
fn value_ui(ui: &mut egui::Ui, value: &mut ron::Value, salt: &str) -> bool {
    let mut changed = false;
    match value {
        ron::Value::Bool(b) => {
            changed |= ui.checkbox(b, "").changed();
        }
        ron::Value::Number(number) => {
            let mut as_f64 = number.into_f64();
            if ui
                .add(egui::DragValue::new(&mut as_f64).speed(0.05))
                .changed()
            {
                *number = ron::value::Number::new(as_f64);
                changed = true;
            }
        }
        ron::Value::String(text) => {
            changed |= ui.text_edit_singleline(text).changed();
        }
        ron::Value::Char(c) => {
            let mut text = c.to_string();
            if ui.text_edit_singleline(&mut text).changed()
                && let Some(first) = text.chars().next()
            {
                *c = first;
                changed = true;
            }
        }
        ron::Value::Option(inner) => {
            if let Some(inner) = inner.as_mut() {
                changed |= value_ui(ui, inner, salt);
            } else {
                ui.label("None");
            }
        }
        ron::Value::Seq(items) => {
            ui.horizontal(|ui| {
                for (i, item) in items.iter_mut().enumerate() {
                    changed |= value_ui(ui, item, &format!("{salt}[{i}]"));
                }
            });
        }
        ron::Value::Map(map) => {
            for (key, entry) in map.iter_mut() {
                let label = match key {
                    ron::Value::String(s) => s.clone(),
                    other => format!("{other:?}"),
                };
                ui.horizontal(|ui| {
                    ui.label(&label);
                    changed |= value_ui(ui, entry, &format!("{salt}.{label}"));
                });
            }
        }
        _ => {
            ui.label("(unsupported)");
        }
    }
    changed
}

fn assets_ui(ui: &mut egui::Ui, world: &mut World) {
    let mut reload: Option<String> = None;
    {
        let textures = world.resource::<Assets<Texture>>();
        ui.label(egui::RichText::new("Textures").strong());
        for (path, _) in textures.paths() {
            ui.horizontal(|ui| {
                ui.label(path);
                if !path.starts_with('<') && ui.small_button("reload").clicked() {
                    reload = Some(path.to_string());
                }
            });
        }
    }
    if let Some(path) = reload {
        world
            .resource_mut::<bevy_ecs::prelude::Messages<AssetEvent>>()
            .write(AssetEvent { path });
    }
}
