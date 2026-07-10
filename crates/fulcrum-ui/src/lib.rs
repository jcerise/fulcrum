//! Fulcrum retained-mode game UI: a tree of [`UiNode`]s laid out with anchors, pivots, and
//! stacking; widgets (panel / label / button / image); RON layout files with hot reload; and
//! pointer interaction surfaced as [`UiEvent`]s the simulation can read.

pub mod debug;
pub mod extract;
pub mod interact;
pub mod layout;
pub mod loader;
pub mod node;
pub mod widgets;

use fulcrum_core::{Fulcrum, IntoScheduleConfigs, Plugin, Update};

pub use debug::{DebugUi, DebugUiFocus, DebugUiPlugin};
pub use interact::{UiEvent, UiFocus};
pub use loader::{UiLoader, UiQuery};
pub use node::{Anchor, StackDir, UiId, UiNode, UiRect, UiRootPath, UiSize};
pub use widgets::{ButtonState, ButtonStyle, UiButton, UiImage, UiLabel, UiPanel};

/// Installs the UI systems (interact -> layout -> extract, each frame) and hot reload.
/// Part of `DefaultPlugins`.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(UiFocus::default());
        app.register_event::<UiEvent>();
        app.register_event::<fulcrum_asset::AssetEvent>();
        // Clicks arrive as replayable `ui:click` commands; this rehydrates them into UiEvents.
        // Registered here — before any game system — so games read clicks the same tick.
        app.add_systems(fulcrum_core::FixedUpdate, interact::dispatch_ui_commands);
        // Reload first (despawn/respawn applies before layout queues per-entity work).
        app.add_systems(
            Update,
            (
                loader::reload_ui_layouts,
                interact::interact_ui,
                layout::layout_system,
                extract::extract_ui,
            )
                .chain(),
        );
    }
}
