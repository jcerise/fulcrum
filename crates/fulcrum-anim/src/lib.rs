//! Fulcrum animation: frame [`AnimationClip`]s driven by [`AnimationPlayer`] components, and
//! Aseprite import. Animation is simulation state — it advances in `FixedUpdate` (durations are
//! in ticks), so gameplay can key off frames deterministically.

pub mod aseprite;
pub mod clip;
pub mod player;
pub mod state_machine;

use bevy_ecs::prelude::Local;
use fulcrum_asset::{AssetEvent, Assets};
use fulcrum_core::{EventReader, FixedUpdate, Fulcrum, IntoScheduleConfigs, Plugin, Update};

pub use aseprite::{AsepriteImport, AsepriteLoader};
pub use clip::AnimationClip;
pub use player::AnimationPlayer;
pub use state_machine::{Animator, AnimatorLoader, Condition, StateMachineAsset};

/// Installs clip storage and the per-tick animation advance system. Part of `DefaultPlugins`.
pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut()
            .insert_resource(Assets::<AnimationClip>::default());
        app.world_mut()
            .insert_resource(Assets::<StateMachineAsset>::default());
        // The loaders also touch texture/sheet storage. Windowed, the render plugin has
        // already inserted these; headless (tests, servers) nobody else will, so make sure
        // they exist — Aseprite imports work without a GPU, they just skip the pixels.
        app.world_mut()
            .init_resource::<Assets<fulcrum_render::Texture>>();
        app.world_mut()
            .init_resource::<Assets<fulcrum_render::SpriteSheet>>();
        app.add_systems(
            FixedUpdate,
            (state_machine::drive_animators, player::advance_animations).chain(),
        );
        app.register_event::<AssetEvent>();
        app.add_systems(Update, reload_aseprite_files);
    }
}

/// Hot reload: when a loaded Aseprite JSON changes, re-import it in place — the sheet and every
/// tagged clip are replaced behind their existing handles. Live `AnimationPlayer`s clamp if a
/// clip shrank.
fn reload_aseprite_files(
    mut events: EventReader<AssetEvent>,
    mut loader: AsepriteLoader,
    mut pending: Local<Vec<String>>,
) {
    pending.extend(
        events
            .read()
            .filter(|event| loader.sheets().handle_for_path(&event.path).is_some())
            .map(|event| event.path.clone()),
    );
    for path in pending.drain(..) {
        match loader.reload(&path) {
            Ok(()) => log::info!("reloaded aseprite {path}"),
            Err(error) => log::error!("hot reload: {error}"),
        }
    }
}
