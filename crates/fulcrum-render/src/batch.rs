//! The sprite batcher: collects all sprites each frame, sorts by `(z, texture)`, and draws them
//! in as few draw calls as possible.

use bevy_ecs::prelude::{Query, Res, ResMut, Resource};
use bytemuck::{Pod, Zeroable};
use fulcrum_asset::{Assets, Handle};
use fulcrum_core::{Color, PreviousTransform2D, Time, Transform2D, Vec2};
use glam::Mat4;
use rustc_hash::FxHashMap;

use crate::camera::CameraFrame;
use crate::gpu::GpuContext;
use crate::sprite::Sprite;
use crate::texture::Texture;

/// Per-frame render statistics, readable by games and logged for the batching acceptance
/// criteria.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct RenderStats {
    /// Sprites drawn last frame.
    pub sprites: usize,
    /// Draw calls issued for sprites last frame.
    pub batches: usize,
    /// Tilemap chunks that passed culling last frame.
    pub tilemap_chunks: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SpriteVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

const VERTEX_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<SpriteVertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
};

/// One contiguous run of vertices sharing a texture: a single draw call.
struct Batch {
    texture_id: u32,
    vertex_range: std::ops::Range<u32>,
}

/// The sprite rendering pipeline and per-frame vertex data. Created by the window plugin once
/// the GPU exists.
#[derive(Resource)]
pub struct SpriteRenderer {
    pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    texture_bind_groups: FxHashMap<u32, wgpu::BindGroup>,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    vertices: Vec<SpriteVertex>,
    batches: Vec<Batch>,
    /// Built-in 1x1 white texture: backs the letterbox background quad (and future solid fills).
    white_bind_group: wgpu::BindGroup,
    /// Tiny dedicated buffer for the letterbox background quad.
    background_buffer: wgpu::Buffer,
    gizmo_pipeline: wgpu::RenderPipeline,
    gizmo_buffer: wgpu::Buffer,
    gizmo_capacity: u64,
}

const INITIAL_VERTEX_CAPACITY: u64 = 6 * 1024; // vertices, not bytes

impl SpriteRenderer {
    /// Build the pipeline against the surface format.
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite globals layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite texture layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite pipeline layout"),
            bind_group_layouts: &[Some(&globals_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(VERTEX_LAYOUT)],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite globals"),
            size: std::mem::size_of::<Mat4>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite sampler (nearest)"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let vertex_buffer = Self::create_vertex_buffer(device, INITIAL_VERTEX_CAPACITY);

        // Built-in 1x1 white texture for solid-color quads.
        let white = crate::texture::upload_raw(device, queue, "white", &[255u8; 4], 1, 1);
        let white_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite white texture"),
            layout: &texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let background_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("letterbox background"),
            size: 6 * std::mem::size_of::<SpriteVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Gizmo line pipeline: untextured, shares the globals bind group.
        let gizmo_shader = device.create_shader_module(wgpu::include_wgsl!("shader_gizmos.wgsl"));
        let gizmo_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gizmo pipeline layout"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });
        let gizmo_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gizmo pipeline"),
            layout: Some(&gizmo_layout),
            vertex: wgpu::VertexState {
                module: &gizmo_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<crate::gizmos::GizmoVertex>()
                        as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &gizmo_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let gizmo_capacity: u64 = 1024;
        let gizmo_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gizmo vertices"),
            size: gizmo_capacity * std::mem::size_of::<crate::gizmos::GizmoVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            globals_buffer,
            globals_bind_group,
            texture_layout,
            sampler,
            texture_bind_groups: FxHashMap::default(),
            vertex_buffer,
            vertex_capacity: INITIAL_VERTEX_CAPACITY,
            vertices: Vec::new(),
            batches: Vec::new(),
            white_bind_group,
            background_buffer,
            gizmo_pipeline,
            gizmo_buffer,
            gizmo_capacity,
        }
    }

    /// Draw this frame's gizmo lines above everything. Globals were written by
    /// [`draw`](Self::draw); call after it, inside the same pass.
    pub(crate) fn draw_gizmos(
        &mut self,
        gpu: &GpuContext,
        vertices: &[crate::gizmos::GizmoVertex],
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        if vertices.is_empty() {
            return;
        }
        let needed = vertices.len() as u64;
        if needed > self.gizmo_capacity {
            self.gizmo_capacity = needed.next_power_of_two();
            self.gizmo_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gizmo vertices"),
                size: self.gizmo_capacity
                    * std::mem::size_of::<crate::gizmos::GizmoVertex>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        gpu.queue
            .write_buffer(&self.gizmo_buffer, 0, bytemuck::cast_slice(vertices));
        pass.set_pipeline(&self.gizmo_pipeline);
        pass.set_bind_group(0, &self.globals_bind_group, &[]);
        pass.set_vertex_buffer(0, self.gizmo_buffer.slice(..));
        pass.draw(0..vertices.len() as u32, 0..1);
    }

    fn create_vertex_buffer(device: &wgpu::Device, capacity: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite vertices"),
            size: capacity * std::mem::size_of::<SpriteVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn bind_group_for(
        &mut self,
        gpu: &GpuContext,
        texture_id: u32,
        texture: &Texture,
    ) -> &wgpu::BindGroup {
        self.texture_bind_groups
            .entry(texture_id)
            .or_insert_with(|| {
                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("sprite texture"),
                    layout: &self.texture_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                })
            })
    }

    /// Upload this frame's vertices and record the draws into `pass`: the letterbox background
    /// (when bars are showing), then every sprite batch. Called inside the frame's render pass.
    pub(crate) fn draw(
        &mut self,
        gpu: &GpuContext,
        frame: &CameraFrame,
        clear_color: Color,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        // Globals are written unconditionally: the gizmo pass reuses them.
        let view_proj = frame.view_proj.to_cols_array();
        gpu.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::cast_slice(&view_proj));

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.globals_bind_group, &[]);

        if frame.letterboxed {
            // The pass cleared the whole surface to black (the bars); paint the game's clear
            // color across the visible world.
            let color = [clear_color.r, clear_color.g, clear_color.b, clear_color.a];
            let quad = [0usize, 1, 2, 0, 2, 3].map(|i| SpriteVertex {
                position: frame.bg_corners[i].into(),
                uv: [0.0, 0.0],
                color,
            });
            gpu.queue
                .write_buffer(&self.background_buffer, 0, bytemuck::cast_slice(&quad));
            pass.set_vertex_buffer(0, self.background_buffer.slice(..));
            pass.set_bind_group(1, &self.white_bind_group, &[]);
            pass.draw(0..6, 0..1);
        }

        if self.batches.is_empty() {
            return;
        }
        let needed = self.vertices.len() as u64;
        if needed > self.vertex_capacity {
            self.vertex_capacity = needed.next_power_of_two();
            self.vertex_buffer = Self::create_vertex_buffer(&gpu.device, self.vertex_capacity);
        }
        gpu.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));

        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        for batch in &self.batches {
            let Some(bind_group) = self.texture_bind_groups.get(&batch.texture_id) else {
                continue;
            };
            pass.set_bind_group(1, bind_group, &[]);
            pass.draw(batch.vertex_range.clone(), 0..1);
        }
    }
}

/// Collected per sprite before sorting. Also produced by the text extractor (glyph quads).
pub(crate) struct ExtractedSprite {
    pub(crate) z: f32,
    pub(crate) texture: Handle<Texture>,
    pub(crate) corners: [Vec2; 4],
    pub(crate) uv: [[f32; 2]; 4],
    pub(crate) color: [f32; 4],
}

/// Extra quads contributed by other extractors (text glyphs) this frame; merged into the sprite
/// batch before sorting, then cleared.
#[derive(Resource, Default)]
pub(crate) struct ExtraQuads(pub(crate) Vec<ExtractedSprite>);

/// `PreRender` system: gather all visible sprites at their interpolated transforms, sort by
/// `(z, texture)`, and build this frame's vertex list and batches.
#[allow(clippy::too_many_arguments)] // ECS systems legitimately take many resources
pub(crate) fn extract_sprites(
    sprites: Query<(&Sprite, &Transform2D, &PreviousTransform2D)>,
    textures: Res<Assets<Texture>>,
    sheets: Res<Assets<crate::atlas::SpriteSheet>>,
    gpu: Res<GpuContext>,
    time: Res<Time>,
    mut extra: ResMut<ExtraQuads>,
    mut renderer: ResMut<SpriteRenderer>,
    mut stats: ResMut<RenderStats>,
) {
    let alpha = time.alpha;

    let mut extracted: Vec<ExtractedSprite> = Vec::with_capacity(sprites.iter().len());
    for (sprite, transform, previous) in &sprites {
        // Resolve the texture and UV sub-rect: either a sheet region or the whole texture.
        let (texture_handle, region_rect) = match sprite.region {
            Some(region) => {
                let Some(sheet) = sheets.get(region.sheet) else {
                    continue;
                };
                let Some(rect) = sheet.regions.get(region.index as usize) else {
                    continue;
                };
                (sheet.texture, Some(*rect))
            }
            None => (sprite.texture, None),
        };
        let Some(texture) = textures.get(texture_handle) else {
            continue;
        };
        let interpolated = previous.0.lerp(transform, alpha);
        let natural_size = region_rect
            .map(|r| r.size())
            .unwrap_or(Vec2::new(texture.width as f32, texture.height as f32));
        let size = sprite.custom_size.unwrap_or(natural_size);

        // Local corners in +Y-up space, anchor-relative, before rotation/scale.
        let min = -sprite.anchor * size;
        let max = min + size;
        let locals = [
            Vec2::new(min.x, min.y), // bottom-left
            Vec2::new(max.x, min.y), // bottom-right
            Vec2::new(max.x, max.y), // top-right
            Vec2::new(min.x, max.y), // top-left
        ];
        let (sin, cos) = interpolated.rotation.sin_cos();
        let corners = locals.map(|local| {
            let scaled = local * interpolated.scale;
            Vec2::new(
                scaled.x * cos - scaled.y * sin,
                scaled.x * sin + scaled.y * cos,
            ) + interpolated.translation
        });

        // Texture space: v = 0 at the top; our bottom-left corner samples v = 1.
        let (u0, u1) = if sprite.flip_x {
            (1.0, 0.0)
        } else {
            (0.0, 1.0)
        };
        let (v_top, v_bottom) = if sprite.flip_y {
            (1.0, 0.0)
        } else {
            (0.0, 1.0)
        };
        let uv = [[u0, v_bottom], [u1, v_bottom], [u1, v_top], [u0, v_top]];

        extracted.push(ExtractedSprite {
            z: sprite.z,
            texture: texture_handle,
            corners,
            uv,
            color: [
                sprite.color.r,
                sprite.color.g,
                sprite.color.b,
                sprite.color.a,
            ],
        });
    }

    extracted.append(&mut extra.0);

    // Stable sort keeps query order for ties, so batching is deterministic frame to frame.
    extracted.sort_by(|a, b| {
        a.z.total_cmp(&b.z)
            .then_with(|| a.texture.id().cmp(&b.texture.id()))
    });

    let renderer = &mut *renderer;
    renderer.vertices.clear();
    renderer.batches.clear();
    for item in &extracted {
        // Ensure a bind group exists for this texture (created once, cached).
        if let Some(texture) = textures.get(item.texture) {
            renderer.bind_group_for(&gpu, item.texture.id(), texture);
        }

        let start = renderer.vertices.len() as u32;
        let quad = [0usize, 1, 2, 0, 2, 3].map(|i| SpriteVertex {
            position: item.corners[i].into(),
            uv: item.uv[i],
            color: item.color,
        });
        renderer.vertices.extend_from_slice(&quad);

        match renderer.batches.last_mut() {
            Some(batch) if batch.texture_id == item.texture.id() => {
                batch.vertex_range.end = start + 6;
            }
            _ => renderer.batches.push(Batch {
                texture_id: item.texture.id(),
                vertex_range: start..start + 6,
            }),
        }
    }

    stats.sprites = extracted.len();
    stats.batches = renderer.batches.len();
}
