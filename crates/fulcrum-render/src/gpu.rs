//! wgpu bootstrap: instance, surface, device, and the per-frame clear/present pass.

use std::sync::Arc;

use bevy_ecs::prelude::Resource;
use bevy_ecs::world::World;
use fulcrum_core::FulcrumConfig;
use winit::window::Window;

/// The GPU: device, queue, and the window surface. Created by the window plugin once the window
/// exists.
#[derive(Resource)]
pub struct GpuContext {
    /// The window surface being presented to.
    pub surface: wgpu::Surface<'static>,
    /// The wgpu device.
    pub device: wgpu::Device,
    /// The wgpu queue.
    pub queue: wgpu::Queue,
    /// Current surface configuration (format, size, present mode).
    pub surface_config: wgpu::SurfaceConfiguration,
}

impl GpuContext {
    /// Reconfigure the surface after a window resize. Zero-sized dimensions (minimized window)
    /// are ignored.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }
}

/// Create the wgpu instance/surface/device for `window` and configure the surface.
pub(crate) fn init(window: Arc<Window>, width: u32, height: u32) -> GpuContext {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let surface = instance
        .create_surface(window)
        .expect("failed to create wgpu surface");
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: Some(&surface),
        ..Default::default()
    }))
    .expect("no compatible GPU adapter found");
    log::info!("gpu adapter: {:?}", adapter.get_info().name);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("fulcrum device"),
        ..Default::default()
    }))
    .expect("failed to create wgpu device");

    let mut surface_config = surface
        .get_default_config(&adapter, width.max(1), height.max(1))
        .expect("surface not supported by adapter");
    surface_config.present_mode = if std::env::var_os("FULCRUM_NO_VSYNC").is_some() {
        wgpu::PresentMode::AutoNoVsync
    } else {
        wgpu::PresentMode::AutoVsync
    };
    surface.configure(&device, &surface_config);

    GpuContext {
        surface,
        device,
        queue,
        surface_config,
    }
}

/// Render one frame: acquire the surface texture, clear it to the configured color, draw the
/// sprite batches, present.
pub(crate) fn render(world: &mut World) {
    world.resource_scope(
        |world, mut renderer: bevy_ecs::world::Mut<crate::batch::SpriteRenderer>| {
            render_with(world, &mut renderer);
        },
    );
}

fn render_with(world: &mut World, renderer: &mut crate::batch::SpriteRenderer) {
    let configured_clear = world.resource::<FulcrumConfig>().clear_color;
    let camera_frame = {
        let camera = world.resource::<crate::camera::Camera2D>();
        let config = &world.resource::<GpuContext>().surface_config;
        camera.frame(fulcrum_core::vec2(
            config.width as f32,
            config.height as f32,
        ))
    };
    // With bars showing, the pass clears to black (the bars) and the renderer paints the
    // configured color inside the viewport; without bars, clear straight to the configured
    // color.
    let clear = if camera_frame.letterboxed {
        fulcrum_core::Color::BLACK
    } else {
        configured_clear
    };
    // Take this frame's gizmo lines (hand the allocation back, cleared, afterwards).
    let mut gizmo_vertices =
        std::mem::take(&mut world.resource_mut::<crate::gizmos::Gizmos>().vertices);
    let gpu = world.resource::<GpuContext>();

    let frame = match gpu.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) => frame,
        // Still usable this frame; the next Resized event reconfigures properly.
        wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
        // Nothing to draw to right now; skip the frame.
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            let (width, height) = {
                let config = &world.resource::<GpuContext>().surface_config;
                (config.width, config.height)
            };
            world.resource_mut::<GpuContext>().resize(width, height);
            return;
        }
        wgpu::CurrentSurfaceTexture::Validation => {
            log::error!("surface texture acquisition failed validation; exiting");
            std::process::exit(1);
        }
    };

    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("fulcrum frame"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: clear.r as f64,
                        g: clear.g as f64,
                        b: clear.b as f64,
                        a: clear.a as f64,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if camera_frame.letterboxed {
            let origin = camera_frame.viewport_origin;
            let size = camera_frame.viewport_size;
            pass.set_viewport(origin.x, origin.y, size.x, size.y, 0.0, 1.0);
            pass.set_scissor_rect(
                origin.x as u32,
                origin.y as u32,
                size.x as u32,
                size.y as u32,
            );
        }
        renderer.draw(gpu, &camera_frame, configured_clear, &mut pass);
        renderer.draw_gizmos(gpu, &gizmo_vertices, &mut pass);
    }
    gpu.queue.submit(Some(encoder.finish()));
    gpu.queue.present(frame);
    gizmo_vertices.clear();
    world.resource_mut::<crate::gizmos::Gizmos>().vertices = gizmo_vertices;
}
