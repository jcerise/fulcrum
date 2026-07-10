//! Fulcrum spatial helpers: a deterministic uniform-grid index ([`SpatialGrid`]). Everything here is plain deterministic
//! computation over simulation state — safe anywhere in `FixedUpdate`.

pub mod grid;

use fulcrum_core::{FixedUpdate, Fulcrum, Plugin};

pub use grid::{SpatialGrid, SpatialIndexed};

/// Installs the spatial grid and its per-tick rebuild. Add **before** your game plugin so
/// queries see this tick's positions.
pub struct SpatialPlugin {
    /// Grid cell size in world units (roughly your typical query diameter).
    pub cell_size: f32,
}

impl Default for SpatialPlugin {
    fn default() -> Self {
        Self { cell_size: 64.0 }
    }
}

impl Plugin for SpatialPlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut()
            .insert_resource(SpatialGrid::new(self.cell_size));
        app.add_systems(FixedUpdate, grid::rebuild_spatial_grid);
    }
}
