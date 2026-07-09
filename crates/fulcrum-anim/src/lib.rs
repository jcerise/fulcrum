//! Fulcrum animation: frame [`AnimationClip`]s driven by [`AnimationPlayer`] components, and
//! Aseprite import. Animation is simulation state — it advances in `FixedUpdate` (durations are
//! in ticks), so gameplay can key off frames deterministically.

pub mod aseprite;
pub mod clip;
pub mod player;

use fulcrum_asset::Assets;
use fulcrum_core::{FixedUpdate, Fulcrum, Plugin};

pub use aseprite::{AsepriteImport, AsepriteLoader};
pub use clip::AnimationClip;
pub use player::AnimationPlayer;

/// Installs clip storage and the per-tick animation advance system. Part of `DefaultPlugins`.
pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut()
            .insert_resource(Assets::<AnimationClip>::default());
        app.add_systems(FixedUpdate, player::advance_animations);
    }
}
