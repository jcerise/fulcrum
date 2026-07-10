//! The uniform spatial grid: fast "who's near me" queries for crowd-scale games.
//!
//! Opt-in: tag entities with [`SpatialIndexed`] and the grid rebuilds from their positions at
//! the start of every tick (clear + reinsert — simpler and faster than incremental updates at
//! the scales 2D games hit). **Results come back in a defined, deterministic order** (cells
//! visited row-major, entities in id order within a cell) — callers may rely on it, which is
//! why this is hand-rolled instead of a dependency.

use bevy_ecs::prelude::{Component, Entity, Query, ResMut, Resource, With};
use fulcrum_core::{FxHashMap, Rect, Transform2D, Vec2};

/// Tag: include this entity in the spatial grid (RTS units yes; UI never).
#[derive(Component, Default)]
pub struct SpatialIndexed;

/// The grid. Query it from any simulation system via `Res<SpatialGrid>`.
#[derive(Resource)]
pub struct SpatialGrid {
    cell_size: f32,
    cells: FxHashMap<(i32, i32), Vec<(Entity, Vec2)>>,
}

impl SpatialGrid {
    /// A grid with the given cell size (roughly the diameter of your typical query).
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size: cell_size.max(1.0),
            cells: FxHashMap::default(),
        }
    }

    fn cell_of(&self, position: Vec2) -> (i32, i32) {
        (
            (position.x / self.cell_size).floor() as i32,
            (position.y / self.cell_size).floor() as i32,
        )
    }

    /// Test/manual-use rebuild (games normally rely on the automatic per-tick system).
    pub fn rebuild_for_test(&mut self, entries: Vec<(Entity, Vec2)>) {
        self.rebuild(entries);
    }

    pub(crate) fn rebuild(&mut self, mut entries: Vec<(Entity, Vec2)>) {
        // Entity-id order in every cell -> deterministic query sequences regardless of
        // archetype layout.
        entries.sort_unstable_by_key(|(entity, _)| *entity);
        self.cells.clear();
        for (entity, position) in entries {
            let cell = self.cell_of(position);
            self.cells.entry(cell).or_default().push((entity, position));
        }
    }

    fn cells_in(&self, min: Vec2, max: Vec2) -> impl Iterator<Item = (i32, i32)> + use<> {
        let (min_x, min_y) = self.cell_of(min);
        let (max_x, max_y) = self.cell_of(max);
        // Row-major visit order: part of the determinism contract.
        (min_y..=max_y).flat_map(move |y| (min_x..=max_x).map(move |x| (x, y)))
    }

    /// Entities within `radius` of `center`, in deterministic order.
    pub fn query_circle(&self, center: Vec2, radius: f32) -> Vec<Entity> {
        let r = radius.max(0.0);
        let radius_squared = r * r;
        let mut out = Vec::new();
        for cell in self.cells_in(center - Vec2::splat(r), center + Vec2::splat(r)) {
            let Some(entries) = self.cells.get(&cell) else {
                continue;
            };
            for (entity, position) in entries {
                if position.distance_squared(center) <= radius_squared {
                    out.push(*entity);
                }
            }
        }
        out
    }

    /// Entities inside `rect` (edges inclusive), in deterministic order.
    pub fn query_rect(&self, rect: Rect) -> Vec<Entity> {
        let mut out = Vec::new();
        for cell in self.cells_in(rect.min, rect.max) {
            let Some(entries) = self.cells.get(&cell) else {
                continue;
            };
            for (entity, position) in entries {
                if rect.contains(*position) {
                    out.push(*entity);
                }
            }
        }
        out
    }

    /// The closest entity to `from` within `max_radius` passing `filter`; distance ties break
    /// by entity id (deterministic).
    pub fn nearest(
        &self,
        from: Vec2,
        max_radius: f32,
        mut filter: impl FnMut(Entity) -> bool,
    ) -> Option<Entity> {
        let mut best: Option<(u32, Entity)> = None;
        for cell in self.cells_in(
            from - Vec2::splat(max_radius),
            from + Vec2::splat(max_radius),
        ) {
            let Some(entries) = self.cells.get(&cell) else {
                continue;
            };
            for (entity, position) in entries {
                let distance = position.distance(from);
                if distance > max_radius || !filter(*entity) {
                    continue;
                }
                let key = distance.to_bits();
                let better = match best {
                    None => true,
                    Some((best_key, best_entity)) => {
                        key < best_key || (key == best_key && *entity < best_entity)
                    }
                };
                if better {
                    best = Some((key, *entity));
                }
            }
        }
        best.map(|(_, entity)| entity)
    }
}

/// `FixedUpdate`-first system: rebuild the grid from tagged entities.
pub(crate) fn rebuild_spatial_grid(
    mut grid: ResMut<SpatialGrid>,
    entities: Query<(Entity, &Transform2D), With<SpatialIndexed>>,
) {
    let entries: Vec<(Entity, Vec2)> = entities
        .iter()
        .map(|(entity, transform)| (entity, transform.translation))
        .collect();
    grid.rebuild(entries);
}

/// A convenience for tests and non-ECS use: read positions straight from a query result.
pub fn brute_force_circle(entries: &[(Entity, Vec2)], center: Vec2, radius: f32) -> Vec<Entity> {
    let mut hits: Vec<Entity> = entries
        .iter()
        .filter(|(_, p)| p.distance(center) <= radius)
        .map(|(e, _)| *e)
        .collect();
    hits.sort_unstable();
    hits
}
