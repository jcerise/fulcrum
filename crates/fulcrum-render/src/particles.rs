//! Particles: data-driven emitters, simulated on the frame schedule.
//!
//! Particles are **cosmetic by construction**: they advance with `frame_delta` in `Update`,
//! use their own throwaway RNG (never `SimRng`), and nothing about them feeds back into the
//! simulation — so effects can be lavish without ever touching determinism or replays.
//!
//! ```ron
//! ParticleEffect(
//!     texture: "fx/spark.png",
//!     mode: Rate(50.0),               // particles/second — or Burst(30) once on spawn
//!     lifetime: (0.3, 0.8),           // seconds, min..max
//!     initial_speed: (20.0, 60.0),
//!     direction_deg: 90.0, spread_deg: 360.0,
//!     gravity: (0.0, -100.0),
//!     size: (start: 4.0, end: 0.0),
//!     color_start: (r: 1.0, g: 0.8, b: 0.5, a: 1.0),
//!     color_end: (r: 1.0, g: 0.3, b: 0.0, a: 0.0),
//!     rotation_speed: (-3.0, 3.0),
//!     additive: true,                 // glow-style blending
//! )
//! ```

use bevy_ecs::prelude::{Commands, Component, Entity, Query, Res, ResMut, Resource};
use bevy_ecs::system::SystemParam;
use fulcrum_asset::{AssetError, AssetServer, Assets, Handle};
use fulcrum_core::{Color, Time, Transform2D, Vec2, vec2};
use serde::Deserialize;

use crate::batch::{ExtraQuads, ExtractedSprite};
use crate::texture::{Texture, load_texture};

/// Hard cap of live particles per emitter (oldest recycled beyond it).
const POOL_CAP: usize = 1024;

/// How an emitter produces particles.
#[derive(Deserialize, Clone, Copy, Debug)]
pub enum EmitMode {
    /// Continuous, particles per second.
    Rate(f32),
    /// One burst of N particles when the emitter appears (pairs with one-shot effects).
    Burst(u32),
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct SizeCurve {
    /// Size at birth (world units, square).
    pub start: f32,
    /// Size at death.
    pub end: f32,
}

/// A particle effect definition (`*.fx.ron`), hot-reloadable.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename = "ParticleEffect")]
pub struct ParticleEffectAsset {
    /// Texture path (resolved at load; a 1x1 white works for pure color).
    pub texture: String,
    /// Emission mode.
    pub mode: EmitMode,
    /// Lifetime range in seconds.
    pub lifetime: (f32, f32),
    /// Launch speed range, units/second.
    pub initial_speed: (f32, f32),
    /// Launch direction, degrees counter-clockwise from +X.
    #[serde(default = "up")]
    pub direction_deg: f32,
    /// Cone width around the direction, degrees.
    #[serde(default = "full_circle")]
    pub spread_deg: f32,
    /// Constant acceleration, units/second^2.
    #[serde(default)]
    pub gravity: Vec2,
    /// Size over life.
    pub size: SizeCurve,
    /// Tint at birth.
    pub color_start: Color,
    /// Tint at death (fade alpha to 0 for smoke/sparks).
    pub color_end: Color,
    /// Angular velocity range, radians/second.
    #[serde(default)]
    pub rotation_speed: (f32, f32),
    /// Additive (glow) blending instead of alpha.
    #[serde(default)]
    pub additive: bool,
    /// Resolved texture (set at load; not part of the file).
    #[serde(skip, default = "invalid_texture")]
    pub(crate) texture_handle: Handle<Texture>,
}

fn invalid_texture() -> Handle<Texture> {
    Handle::INVALID
}

fn up() -> f32 {
    90.0
}
fn full_circle() -> f32 {
    360.0
}

/// Attach to an entity with a `Transform2D` to emit from it.
#[derive(Component)]
pub struct ParticleEmitter {
    /// The effect to emit.
    pub effect: Handle<ParticleEffectAsset>,
    /// Rate-mode emitters can be toggled.
    pub active: bool,
    /// Despawn the entity once every particle has died (burst effects).
    pub one_shot: bool,
    pool: Vec<Particle>,
    spawn_accumulator: f32,
    burst_done: bool,
}

impl ParticleEmitter {
    /// A continuously-emitting attachment.
    pub fn new(effect: Handle<ParticleEffectAsset>) -> Self {
        Self {
            effect,
            active: true,
            one_shot: false,
            pool: Vec::new(),
            spawn_accumulator: 0.0,
            burst_done: false,
        }
    }

    /// Live particle count (diagnostics).
    pub fn live(&self) -> usize {
        self.pool.len()
    }
}

struct Particle {
    position: Vec2,
    velocity: Vec2,
    rotation: f32,
    rotation_speed: f32,
    age: f32,
    lifetime: f32,
}

/// Cosmetic RNG for particles: a bare LCG, deliberately not `SimRng` — particle randomness
/// must never consume simulation rolls.
#[derive(Resource)]
pub struct FxRng(u32);

impl Default for FxRng {
    fn default() -> Self {
        Self(0x9e37_79b9)
    }
}

impl FxRng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / 16_777_216.0
    }
    fn range(&mut self, range: (f32, f32)) -> f32 {
        range.0 + self.next() * (range.1 - range.0)
    }
}

/// `Update` system: age, spawn, integrate, and hand quads to the renderer.
pub(crate) fn simulate_particles(
    mut commands: Commands,
    mut emitters: Query<(Entity, &Transform2D, &mut ParticleEmitter)>,
    effects: Res<Assets<ParticleEffectAsset>>,
    time: Res<Time>,
    mut rng: ResMut<FxRng>,
    mut alpha_quads: ResMut<ExtraQuads>,
    mut additive_quads: ResMut<AdditiveQuads>,
) {
    let dt = time.frame_delta.min(0.1);
    for (entity, transform, mut emitter) in &mut emitters {
        let Some(effect) = effects.get(emitter.effect) else {
            continue;
        };

        // Spawn.
        let mut wanted = 0usize;
        match effect.mode {
            EmitMode::Rate(rate) => {
                if emitter.active {
                    emitter.spawn_accumulator += rate * dt;
                    wanted = emitter.spawn_accumulator as usize;
                    emitter.spawn_accumulator -= wanted as f32;
                }
            }
            EmitMode::Burst(count) => {
                if !emitter.burst_done {
                    emitter.burst_done = true;
                    wanted = count as usize;
                }
            }
        }
        for _ in 0..wanted {
            let angle = (effect.direction_deg
                + rng.range((-effect.spread_deg / 2.0, effect.spread_deg / 2.0)))
            .to_radians();
            let speed = rng.range(effect.initial_speed);
            let particle = Particle {
                position: transform.translation,
                velocity: vec2(angle.cos(), angle.sin()) * speed,
                rotation: rng.range((0.0, std::f32::consts::TAU)),
                rotation_speed: rng.range(effect.rotation_speed),
                age: 0.0,
                lifetime: rng.range(effect.lifetime).max(0.01),
            };
            if emitter.pool.len() >= POOL_CAP {
                emitter.pool.remove(0); // recycle oldest
            }
            emitter.pool.push(particle);
        }

        // Integrate + retire.
        for particle in &mut emitter.pool {
            particle.age += dt;
            particle.velocity += effect.gravity * dt;
            particle.position += particle.velocity * dt;
            particle.rotation += particle.rotation_speed * dt;
        }
        emitter.pool.retain(|p| p.age < p.lifetime);

        // Emit quads.
        let out = if effect.additive {
            &mut additive_quads.0
        } else {
            &mut alpha_quads.0
        };
        for particle in &emitter.pool {
            let t = (particle.age / particle.lifetime).clamp(0.0, 1.0);
            let size = effect.size.start + (effect.size.end - effect.size.start) * t;
            let half = size / 2.0;
            let (sin, cos) = particle.rotation.sin_cos();
            let corners = [
                vec2(-half, -half),
                vec2(half, -half),
                vec2(half, half),
                vec2(-half, half),
            ]
            .map(|c| vec2(c.x * cos - c.y * sin, c.x * sin + c.y * cos) + particle.position);
            let mix = |a: f32, b: f32| a + (b - a) * t;
            out.push(ExtractedSprite {
                z: 100.0, // above sprites; below UI/gizmos by stage order
                texture: effect.texture_handle,
                corners,
                uv: [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                color: [
                    mix(effect.color_start.r, effect.color_end.r),
                    mix(effect.color_start.g, effect.color_end.g),
                    mix(effect.color_start.b, effect.color_end.b),
                    mix(effect.color_start.a, effect.color_end.a),
                ],
            });
        }

        if emitter.one_shot && emitter.burst_done && emitter.pool.is_empty() {
            commands.entity(entity).despawn();
        }
    }
}

/// World-space quads drawn with additive blending, after alpha sprites.
#[derive(Resource, Default)]
pub struct AdditiveQuads(pub Vec<ExtractedSprite>);

/// One-line effect loading: `let boom = effects.load("fx/boom.fx.ron")?;`
#[derive(SystemParam)]
pub struct EffectLoader<'w> {
    server: Res<'w, AssetServer>,
    textures: ResMut<'w, Assets<Texture>>,
    effects: ResMut<'w, Assets<ParticleEffectAsset>>,
    gpu: Res<'w, crate::gpu::GpuContext>,
}

impl EffectLoader<'_> {
    /// Load a `.fx.ron` effect (and its texture), deduplicated by path.
    pub fn load(&mut self, path: &str) -> Result<Handle<ParticleEffectAsset>, AssetError> {
        if let Some(handle) = self.effects.handle_for_path(path) {
            return Ok(handle);
        }
        let bytes = self.server.read_bytes(path)?;
        let asset = parse_effect(path, &bytes)?;
        let asset = resolve_effect(asset, &self.server, &mut self.textures, &self.gpu);
        Ok(self.effects.insert_with_path(path, asset))
    }
}

pub(crate) fn parse_effect(path: &str, bytes: &[u8]) -> Result<ParticleEffectAsset, AssetError> {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(&String::from_utf8_lossy(bytes))
        .map_err(|error| AssetError::Decode {
            path: path.to_string(),
            message: error.to_string(),
        })
}

pub(crate) fn resolve_effect(
    mut asset: ParticleEffectAsset,
    server: &AssetServer,
    textures: &mut Assets<Texture>,
    gpu: &crate::gpu::GpuContext,
) -> ParticleEffectAsset {
    asset.texture_handle = load_texture(server, textures, gpu, &asset.texture);
    asset
}

/// `commands.spawn_effect_at(...)`: fire-and-forget one-shot effects.
pub trait SpawnEffectExt {
    /// Spawn a burst effect at a world position; the entity cleans itself up when the last
    /// particle dies.
    fn spawn_effect_at(&mut self, effect: Handle<ParticleEffectAsset>, position: Vec2) -> Entity;
}

impl SpawnEffectExt for Commands<'_, '_> {
    fn spawn_effect_at(&mut self, effect: Handle<ParticleEffectAsset>, position: Vec2) -> Entity {
        let mut emitter = ParticleEmitter::new(effect);
        emitter.one_shot = true;
        self.spawn((emitter, Transform2D::from_translation(position)))
            .id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch::ExtraQuads;
    use fulcrum_core::{Fulcrum, Update};

    fn burst_effect(count: u32) -> ParticleEffectAsset {
        ParticleEffectAsset {
            texture: String::new(),
            mode: EmitMode::Burst(count),
            lifetime: (0.2, 0.2),
            initial_speed: (10.0, 20.0),
            direction_deg: 90.0,
            spread_deg: 360.0,
            gravity: Vec2::ZERO,
            size: SizeCurve {
                start: 4.0,
                end: 0.0,
            },
            color_start: Color::WHITE,
            color_end: Color::TRANSPARENT,
            rotation_speed: (0.0, 0.0),
            additive: false,
            texture_handle: Handle::INVALID,
        }
    }

    fn app_with_effect(effect: ParticleEffectAsset) -> (Fulcrum, Handle<ParticleEffectAsset>) {
        let mut app = Fulcrum::new("particles test");
        let world = app.world_mut();
        world.insert_resource(ExtraQuads::default());
        world.insert_resource(AdditiveQuads::default());
        world.insert_resource(FxRng::default());
        let mut effects = Assets::<ParticleEffectAsset>::default();
        let handle = effects.insert(effect);
        world.insert_resource(effects);
        app.add_systems(Update, simulate_particles);
        (app, handle)
    }

    fn run_frames(app: &mut Fulcrum, frames: u32, dt: f32) {
        for _ in 0..frames {
            app.world_mut().resource_mut::<Time>().frame_delta = dt;
            app.world_mut().run_schedule(Update);
        }
    }

    #[test]
    fn one_shot_emitters_clean_themselves_up() {
        let (mut app, effect) = app_with_effect(burst_effect(30));
        let world = app.world_mut();
        let mut emitter = ParticleEmitter::new(effect);
        emitter.one_shot = true;
        let entity = world.spawn((emitter, Transform2D::default())).id();
        let baseline = world
            .query::<bevy_ecs::prelude::Entity>()
            .iter(world)
            .count();

        run_frames(&mut app, 2, 0.05); // burst + particles alive
        assert!(app.world_mut().get_entity(entity).is_ok());
        run_frames(&mut app, 10, 0.05); // 0.5s > 0.2s lifetime
        assert!(
            app.world_mut().get_entity(entity).is_err(),
            "one-shot despawned itself"
        );
        let world = app.world_mut();
        let after = world
            .query::<bevy_ecs::prelude::Entity>()
            .iter(world)
            .count();
        assert_eq!(after, baseline - 1, "entity count back to baseline");
    }

    #[test]
    fn the_pool_caps_and_recycles_oldest() {
        let (mut app, effect) = app_with_effect(burst_effect(5000));
        let world = app.world_mut();
        let entity = world
            .spawn((ParticleEmitter::new(effect), Transform2D::default()))
            .id();
        run_frames(&mut app, 1, 0.001);
        let live = app
            .world_mut()
            .entity(entity)
            .get::<ParticleEmitter>()
            .unwrap()
            .live();
        assert!(live <= POOL_CAP, "pool capped, got {live}");
    }
}
