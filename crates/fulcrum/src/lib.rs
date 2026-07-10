//! Fulcrum — an opinionated, deterministic 2D game engine.
//!
//! This is the facade crate: games depend on `fulcrum` alone and import everything through
//! [`prelude`]. A complete window-on-screen program:
//!
//! ```no_run
//! use fulcrum::prelude::*;
//!
//! fn setup(mut commands: Commands, mut assets: AssetLoader) {
//!     commands.spawn((
//!         Sprite::new(assets.load("ship.png")),
//!         Transform2D::from_xy(0.0, 0.0),
//!     ));
//! }
//!
//! fn main() {
//!     Fulcrum::new("my game")
//!         .with_plugin(DefaultPlugins)
//!         .add_startup(setup)
//!         .run();
//! }
//! ```
//!
//! Game logic goes in `FixedUpdate` systems (added with
//! [`add_system`](prelude::Fulcrum::add_system)) — a deterministic, fixed-rate simulation the
//! renderer interpolates. See `docs/determinism.md` for the rules that keep games replayable.

use fulcrum_core::{Fulcrum, Plugin};

/// Everything needed to put a game on screen: window + wgpu renderer, sprite batching, assets,
/// and input. Add once, right after [`Fulcrum::new`](prelude::Fulcrum::new).
///
/// Grows in later phases (audio, animation, UI); adding it today keeps games source-stable.
pub struct DefaultPlugins;

impl Plugin for DefaultPlugins {
    fn build(&self, app: &mut Fulcrum) {
        fulcrum_render::WindowPlugin.build(app);
        fulcrum_audio::AudioPlugin.build(app);
        fulcrum_anim::AnimPlugin.build(app);
        fulcrum_scene::ScenePlugin.build(app);
        fulcrum_ui::UiPlugin.build(app);
        if cfg!(debug_assertions) {
            fulcrum_ui::DebugUiPlugin.build(app);
        }
    }
}

/// The single import surface for games: `use fulcrum::prelude::*;`.
pub mod prelude {
    pub use crate::DefaultPlugins;
    pub use fulcrum_anim::{
        AnimPlugin, AnimationClip, AnimationPlayer, Animator, AnimatorLoader, AsepriteImport,
        AsepriteLoader, StateMachineAsset,
    };
    pub use fulcrum_asset::{AssetServer, Assets, Handle};
    pub use fulcrum_audio::{Audio, AudioPlugin, PlayParams, Sound, SoundLoader};
    pub use fulcrum_core::{
        Added, Bundle, Changed, Color, Commands, Component, DEFAULT_SEED, Entity, Event,
        EventReader, EventWriter, FixedUpdate, Fulcrum, FulcrumConfig, FxHashMap, FxHashSet, Input,
        IntoScheduleConfigs, Key, Local, MouseButton, Or, ParamSet, Plugin, PreRender, Query, Rect,
        Res, ResMut, Resource, SimRng, Startup, Time, Transform2D, Update, Vec2, With, Without,
        World, vec2,
    };
    pub use fulcrum_core::{Children, Name, Parent};
    pub use fulcrum_mod::{LoadedMod, ModEvent, ModPlugin, ModRegistry};
    pub use fulcrum_render::{
        AssetLoader, Camera2D, DefaultFont, EffectLoader, EmitMode, Font, Gizmos, HAlign,
        ParticleEffectAsset, ParticleEmitter, RenderStats, ScalingMode, SpawnEffectExt, Sprite,
        SpriteRegion, SpriteSheet, Text, Texture, TileLayer, Tilemap, TilemapAsset, TilemapLoader,
        WindowInfo, WindowPlugin,
    };
    pub use fulcrum_scene::{
        AnimationPlayerDef, AnimatorDef, ComponentRegistry, PrefabAsset, PrefabLoader,
        RegisterComponentExt, SceneAsset, SceneError, SceneLoader, SceneMember, ScenePlugin,
        SceneSpawner, SpawnPrefabExt, SpriteDef, TextDef, TilemapDef, save_world,
    };
    pub use fulcrum_ui::{
        Anchor, ButtonStyle, DebugUi, DebugUiFocus, DebugUiPlugin, StackDir, UiButton, UiEvent,
        UiFocus, UiId, UiImage, UiLabel, UiLoader, UiNode, UiPanel, UiPlugin, UiQuery, UiRect,
        UiSize,
    };
}
