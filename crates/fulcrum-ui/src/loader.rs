//! RON layout files, spawning, dynamic content lookup, and hot reload.
//!
//! ```ron
//! Ui(root: Node(
//!     anchor: TopLeft, offset: (16, 16), size: Fit, stack: Vertical(4),
//!     kind: Panel(),
//!     children: [
//!         Node(id: "score", kind: Label(text: "Score: 0", size: 16)),
//!         Node(id: "menu",  size: Px((120, 28)), kind: Button(text: "Menu")),
//!     ],
//! ))
//! ```
//!
//! Layouts are **stateless**: hot reload despawns and respawns the tree; dynamic text is
//! re-driven by game systems through [`UiQuery`] every frame.

use bevy_ecs::prelude::{Commands, Entity, Query, Res};
use bevy_ecs::system::SystemParam;
use fulcrum_asset::{AssetError, AssetEvent, AssetServer, Handle};
use fulcrum_core::{Children, Color, EventReader, Parent, Vec2};
use fulcrum_render::{AssetLoader, HAlign, Texture};
use serde::Deserialize;

use crate::node::{Anchor, StackDir, UiId, UiNode, UiRootPath, UiSize};
use crate::widgets::{ButtonState, ButtonStyle, UiButton, UiImage, UiLabel, UiPanel};

#[derive(Deserialize)]
#[serde(rename = "Ui")]
struct UiDef {
    root: NodeDef,
}

#[derive(Deserialize)]
#[serde(rename = "Node", default)]
struct NodeDef {
    id: Option<String>,
    anchor: Anchor,
    pivot: Vec2,
    offset: Vec2,
    size: UiSize,
    stack: StackDir,
    visible: bool,
    kind: KindDef,
    children: Vec<NodeDef>,
}

impl Default for NodeDef {
    fn default() -> Self {
        Self {
            id: None,
            anchor: Anchor::TopLeft,
            pivot: Vec2::ZERO,
            offset: Vec2::ZERO,
            size: UiSize::Fit,
            stack: StackDir::None,
            visible: true,
            kind: KindDef::None,
            children: Vec::new(),
        }
    }
}

#[derive(Deserialize, Default)]
enum KindDef {
    /// Pure layout node.
    #[default]
    None,
    Panel {
        #[serde(default = "panel_color")]
        color: Color,
        #[serde(default)]
        image: Option<String>,
        #[serde(default)]
        nine_slice: Option<[f32; 4]>,
    },
    Label {
        #[serde(default)]
        text: String,
        #[serde(default = "label_size")]
        size: f32,
        #[serde(default)]
        color: Color,
        #[serde(default)]
        h_align: HAlign,
    },
    Button {
        #[serde(default)]
        text: String,
        #[serde(default = "label_size")]
        text_size: f32,
        #[serde(default)]
        style: ButtonStyle,
    },
    Image {
        image: String,
        #[serde(default)]
        color: Color,
    },
}

fn panel_color() -> Color {
    Color::rgba(0.0, 0.0, 0.0, 0.6)
}
fn label_size() -> f32 {
    16.0
}

fn parse_ui(path: &str, source: &str) -> Result<UiDef, AssetError> {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(source)
        .map_err(|error| AssetError::Decode {
            path: path.to_string(),
            message: error.to_string(),
        })
}

/// Loads `.ui.ron` layouts and manages their trees.
#[derive(SystemParam)]
pub struct UiLoader<'w, 's> {
    commands: Commands<'w, 's>,
    server: Res<'w, AssetServer>,
    assets: AssetLoader<'w>,
    children: Query<'w, 's, &'static Children>,
}

impl UiLoader<'_, '_> {
    /// Spawn a layout file's tree; returns the root entity.
    pub fn load(&mut self, path: &str) -> Result<Entity, AssetError> {
        let bytes = self.server.read_bytes(path)?;
        let def = parse_ui(path, &String::from_utf8_lossy(&bytes))?;
        let root = self.spawn_node(&def.root, None);
        self.commands
            .entity(root)
            .insert(UiRootPath(path.to_string()));
        Ok(root)
    }

    /// Despawn a tree spawned by [`load`](Self::load) (root + all descendants). Tolerates
    /// already-despawned entities (hot reload can race duplicate events).
    pub fn unload(&mut self, root: Entity) {
        let mut stack = vec![root];
        while let Some(entity) = stack.pop() {
            if let Ok(children) = self.children.get(entity) {
                stack.extend(children.0.iter().copied());
            }
            self.commands
                .queue(move |world: &mut bevy_ecs::world::World| {
                    let _ = world.try_despawn(entity);
                });
        }
    }

    fn resolve_image(&mut self, path: &str) -> Handle<Texture> {
        self.assets.load(path)
    }

    fn spawn_node(&mut self, def: &NodeDef, parent: Option<Entity>) -> Entity {
        let node = UiNode {
            anchor: def.anchor,
            pivot: def.pivot,
            offset: def.offset,
            size: def.size,
            stack: def.stack,
            visible: def.visible,
        };
        let entity = self.commands.spawn(node).id();
        if let Some(id) = &def.id {
            self.commands.entity(entity).insert(UiId(id.clone()));
        }
        match &def.kind {
            KindDef::None => {}
            KindDef::Panel {
                color,
                image,
                nine_slice,
            } => {
                let image = image.as_ref().map(|path| self.resolve_image(path));
                self.commands.entity(entity).insert(UiPanel {
                    color: *color,
                    image,
                    nine_slice: *nine_slice,
                });
            }
            KindDef::Label {
                text,
                size,
                color,
                h_align,
            } => {
                self.commands.entity(entity).insert(UiLabel {
                    text: text.clone(),
                    size: *size,
                    color: *color,
                    h_align: *h_align,
                });
            }
            KindDef::Button {
                text,
                text_size,
                style,
            } => {
                if def.id.is_none() {
                    log::error!("ui: button `{text}` has no id; its clicks can't be handled");
                }
                self.commands.entity(entity).insert((
                    UiButton {
                        text: text.clone(),
                        text_size: *text_size,
                        style: *style,
                    },
                    ButtonState::default(),
                ));
            }
            KindDef::Image { image, color } => {
                let image = self.resolve_image(image);
                self.commands.entity(entity).insert(UiImage {
                    image,
                    color: *color,
                });
            }
        }
        if let Some(parent) = parent {
            self.commands.entity(entity).insert(Parent(parent));
        }
        let child_entities: Vec<Entity> = def
            .children
            .iter()
            .map(|child| self.spawn_node(child, Some(entity)))
            .collect();
        if !child_entities.is_empty() {
            self.commands
                .entity(entity)
                .insert(Children(child_entities));
        }
        entity
    }
}

/// Dynamic UI content by [`UiId`]: `ui.set_label("score", format!("Score: {n}"))`.
#[derive(SystemParam)]
pub struct UiQuery<'w, 's> {
    labels: Query<'w, 's, (&'static UiId, &'static mut UiLabel)>,
    nodes: Query<'w, 's, (&'static UiId, &'static mut UiNode)>,
}

impl UiQuery<'_, '_> {
    /// Set a label's text (no-op with a log if the id doesn't exist).
    pub fn set_label(&mut self, id: &str, text: impl Into<String>) {
        let text = text.into();
        for (ui_id, mut label) in &mut self.labels {
            if ui_id.0 == id {
                if label.text != text {
                    label.text = text;
                }
                return;
            }
        }
        log::warn!("UiQuery::set_label: no label with id `{id}`");
    }

    /// Show or hide a node (and its subtree).
    pub fn set_visible(&mut self, id: &str, visible: bool) {
        for (ui_id, mut node) in &mut self.nodes {
            if ui_id.0 == id {
                node.visible = visible;
                return;
            }
        }
        log::warn!("UiQuery::set_visible: no node with id `{id}`");
    }
}

/// Hot reload: when a loaded layout file changes, despawn and respawn its tree in place.
pub(crate) fn reload_ui_layouts(
    mut events: EventReader<AssetEvent>,
    roots: Query<(Entity, &UiRootPath)>,
    mut loader: UiLoader,
) {
    // Dedupe: multiple events for one path in a frame must reload once (commands from the
    // first pass haven't applied yet, so the stale root would double-despawn).
    let mut reloaded: Vec<String> = Vec::new();
    for event in events.read() {
        if reloaded.contains(&event.path) {
            continue;
        }
        let mut any = false;
        for (root, path) in &roots {
            if path.0 == event.path {
                loader.unload(root);
                any = true;
            }
        }
        if any {
            reloaded.push(event.path.clone());
            match loader.load(&event.path) {
                Ok(_) => log::info!("reloaded ui layout {}", event.path),
                Err(error) => log::error!("hot reload: {error}"),
            }
        }
    }
}
