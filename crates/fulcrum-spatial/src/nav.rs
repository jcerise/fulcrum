//! Grid pathfinding: [`NavGrid`] walkability/cost, A* for single paths, and [`FlowField`]s for
//! crowds (one field steers any number of units toward the same goal — the RTS move-command
//! primitive). All plain deterministic functions over deterministic inputs.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use fulcrum_core::{Vec2, vec2};
use fulcrum_render::TilemapAsset;

/// Walkability and terrain cost per cell. Cost `0` = blocked; `1` = normal; higher = slower
/// terrain (mud). Kept in sync by the game as the world changes (buildings rise, walls fall).
pub struct NavGrid {
    width: u32,
    height: u32,
    cost: Vec<u32>,
    /// World size of one cell (for world<->cell mapping).
    pub cell_size: f32,
    /// World position of cell (0,0)'s min corner.
    pub origin: Vec2,
}

impl NavGrid {
    /// An all-walkable grid.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            cost: vec![1; (width * height) as usize],
            cell_size: 1.0,
            origin: Vec2::ZERO,
        }
    }

    /// Build from a tilemap layer: `classify` maps each tile value to `Some(cost)` or `None`
    /// (blocked). Cell size and origin come from the map (origin = the map entity's
    /// translation).
    pub fn from_tilemap(
        map: &TilemapAsset,
        layer: &str,
        origin: Vec2,
        classify: impl Fn(u32) -> Option<u32>,
    ) -> Option<Self> {
        let layer = map.layers.iter().find(|l| l.name == layer)?;
        let mut grid = Self::new(layer.width, layer.height);
        grid.cell_size = map.tile_size.x;
        grid.origin = origin;
        for y in 0..layer.height {
            for x in 0..layer.width {
                let value = layer.tiles[(y * layer.width + x) as usize];
                grid.set_cost(x, y, classify(value).unwrap_or(0));
            }
        }
        Some(grid)
    }

    /// Grid dimensions in cells.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn index(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }

    /// Terrain cost, or `None` when blocked/out of bounds.
    pub fn cost(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        match self.cost[self.index(x, y)] {
            0 => None,
            cost => Some(cost),
        }
    }

    /// Is the cell walkable?
    pub fn is_walkable(&self, x: u32, y: u32) -> bool {
        self.cost(x, y).is_some()
    }

    /// Set a cell's cost (`0` = blocked).
    pub fn set_cost(&mut self, x: u32, y: u32, cost: u32) {
        if x < self.width && y < self.height {
            let index = self.index(x, y);
            self.cost[index] = cost;
        }
    }

    /// Convenience: block or unblock a cell.
    pub fn set_walkable(&mut self, x: u32, y: u32, walkable: bool) {
        self.set_cost(x, y, u32::from(walkable));
    }

    /// Which cell a world position lands in.
    pub fn world_to_cell(&self, world: Vec2) -> Option<(u32, u32)> {
        let local = (world - self.origin) / self.cell_size;
        if local.x < 0.0 || local.y < 0.0 {
            return None;
        }
        let (x, y) = (local.x as u32, local.y as u32);
        (x < self.width && y < self.height).then_some((x, y))
    }

    /// A cell's center in world space.
    pub fn cell_center(&self, x: u32, y: u32) -> Vec2 {
        self.origin
            + vec2(
                (x as f32 + 0.5) * self.cell_size,
                (y as f32 + 0.5) * self.cell_size,
            )
    }

    /// 8-directional neighbors with the no-corner-cutting rule: a diagonal step requires both
    /// adjacent orthogonal cells to be walkable.
    fn neighbors(&self, x: u32, y: u32) -> impl Iterator<Item = (u32, u32, u32)> + '_ {
        const DIRECTIONS: [(i32, i32); 8] = [
            (0, 1),
            (1, 0),
            (0, -1),
            (-1, 0), // orthogonal first
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];
        DIRECTIONS.iter().filter_map(move |&(dx, dy)| {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 {
                return None;
            }
            let (nx, ny) = (nx as u32, ny as u32);
            let terrain = self.cost(nx, ny)?;
            let diagonal = dx != 0 && dy != 0;
            if diagonal
                && !(self.is_walkable((x as i32 + dx) as u32, y)
                    && self.is_walkable(x, (y as i32 + dy) as u32))
            {
                return None; // corner cutting forbidden
            }
            let step = if diagonal { 14 } else { 10 }; // octile, x10 fixed-point
            Some((nx, ny, step * terrain))
        })
    }
}

fn octile(a: (u32, u32), b: (u32, u32)) -> u32 {
    let dx = a.0.abs_diff(b.0);
    let dy = a.1.abs_diff(b.1);
    let (long, short) = (dx.max(dy), dx.min(dy));
    10 * (long - short) + 14 * short
}

/// A* over the grid, 8-directional, deterministic tie-breaking (f, then h, then cell index).
/// Returns the path including both endpoints, or `None` when unreachable.
pub fn astar(grid: &NavGrid, from: (u32, u32), to: (u32, u32)) -> Option<Vec<(u32, u32)>> {
    if !grid.is_walkable(from.0, from.1) || !grid.is_walkable(to.0, to.1) {
        return None;
    }
    if from == to {
        return Some(vec![from]);
    }
    let cells = (grid.width * grid.height) as usize;
    let mut g = vec![u32::MAX; cells];
    let mut parent: Vec<u32> = vec![u32::MAX; cells];
    let mut open: BinaryHeap<Reverse<(u32, u32, u32)>> = BinaryHeap::new(); // (f, h, index)

    let start_index = grid.index(from.0, from.1);
    g[start_index] = 0;
    open.push(Reverse((
        octile(from, to),
        octile(from, to),
        start_index as u32,
    )));

    while let Some(Reverse((_, _, index))) = open.pop() {
        let (x, y) = (index % grid.width, index / grid.width);
        if (x, y) == to {
            // Reconstruct.
            let mut path = vec![to];
            let mut cursor = index;
            while parent[cursor as usize] != u32::MAX {
                cursor = parent[cursor as usize];
                path.push((cursor % grid.width, cursor / grid.width));
            }
            path.reverse();
            return Some(path);
        }
        let current_g = g[index as usize];
        for (nx, ny, step) in grid.neighbors(x, y) {
            let neighbor = grid.index(nx, ny);
            let candidate = current_g.saturating_add(step);
            if candidate < g[neighbor] {
                g[neighbor] = candidate;
                parent[neighbor] = index;
                let h = octile((nx, ny), to);
                open.push(Reverse((candidate + h, h, neighbor as u32)));
            }
        }
    }
    None
}

/// Grid line-of-sight (supercover walk): true if every cell on the segment is walkable.
fn line_of_sight(grid: &NavGrid, a: (u32, u32), b: (u32, u32)) -> bool {
    let (mut x, mut y) = (a.0 as i64, a.1 as i64);
    let (x1, y1) = (b.0 as i64, b.1 as i64);
    let dx = (x1 - x).abs();
    let dy = (y1 - y).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let sy = if y < y1 { 1 } else { -1 };
    let mut error = dx - dy;
    loop {
        if !grid.is_walkable(x as u32, y as u32) {
            return false;
        }
        if x == x1 && y == y1 {
            return true;
        }
        let doubled = error * 2;
        // Supercover: when the line crosses a corner, check both adjacent cells.
        if doubled == 0
            && dx != 0
            && dy != 0
            && (!grid.is_walkable((x + sx) as u32, y as u32)
                || !grid.is_walkable(x as u32, (y + sy) as u32))
        {
            return false;
        }
        if doubled > -dy {
            error -= dy;
            x += sx;
        }
        if doubled < dx {
            error += dx;
            y += sy;
        }
    }
}

/// Drop intermediate waypoints that a straight walk can skip (line-of-sight smoothing).
pub fn simplify_path(grid: &NavGrid, path: &[(u32, u32)]) -> Vec<(u32, u32)> {
    if path.len() <= 2 {
        return path.to_vec();
    }
    let mut out = vec![path[0]];
    let mut anchor = 0;
    for i in 1..path.len() - 1 {
        if !line_of_sight(grid, path[anchor], path[i + 1]) {
            out.push(path[i]);
            anchor = i;
        }
    }
    out.push(*path.last().unwrap());
    out
}

/// A crowd-steering field: Dijkstra integration from the goal cells, plus a per-cell direction
/// toward the cheapest neighbor. Compute once per move command; any number of units sample it.
pub struct FlowField {
    width: u32,
    integration: Vec<u32>,
    direction: Vec<Vec2>,
}

impl FlowField {
    /// Build the field toward `goals` (multi-goal supported: nearest wins).
    pub fn compute(grid: &NavGrid, goals: &[(u32, u32)]) -> Self {
        let cells = (grid.width * grid.height) as usize;
        let mut integration = vec![u32::MAX; cells];
        let mut open: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new(); // (cost, index)
        for &(x, y) in goals {
            if grid.is_walkable(x, y) {
                let index = grid.index(x, y);
                integration[index] = 0;
                open.push(Reverse((0, index as u32)));
            }
        }
        while let Some(Reverse((cost, index))) = open.pop() {
            if cost > integration[index as usize] {
                continue;
            }
            let (x, y) = (index % grid.width, index / grid.width);
            for (nx, ny, step) in grid.neighbors(x, y) {
                let neighbor = grid.index(nx, ny);
                let candidate = cost.saturating_add(step);
                if candidate < integration[neighbor] {
                    integration[neighbor] = candidate;
                    open.push(Reverse((candidate, neighbor as u32)));
                }
            }
        }

        // Direction: toward the lowest-integration neighbor (ties by neighbor order, which is
        // fixed — deterministic).
        let mut direction = vec![Vec2::ZERO; cells];
        for y in 0..grid.height {
            for x in 0..grid.width {
                let index = grid.index(x, y);
                if integration[index] == u32::MAX || integration[index] == 0 {
                    continue; // unreachable or goal
                }
                let mut best = integration[index];
                let mut best_dir = Vec2::ZERO;
                for (nx, ny, _) in grid.neighbors(x, y) {
                    let value = integration[grid.index(nx, ny)];
                    if value < best {
                        best = value;
                        best_dir = vec2(nx as f32 - x as f32, ny as f32 - y as f32);
                    }
                }
                direction[index] = best_dir.normalize_or_zero();
            }
        }
        Self {
            width: grid.width,
            integration,
            direction,
        }
    }

    /// The steering direction at a world position (`Vec2::ZERO` at goals and unreachable
    /// cells).
    pub fn sample(&self, grid: &NavGrid, world: Vec2) -> Vec2 {
        match grid.world_to_cell(world) {
            Some((x, y)) => self.direction[(y * self.width + x) as usize],
            None => Vec2::ZERO,
        }
    }

    /// Can this cell reach a goal?
    pub fn is_reachable(&self, x: u32, y: u32) -> bool {
        let index = (y * self.width + x) as usize;
        self.integration.get(index).is_some_and(|&v| v != u32::MAX)
    }

    /// Integration cost at a cell (`u32::MAX` = unreachable).
    pub fn integration_at(&self, x: u32, y: u32) -> u32 {
        self.integration[(y * self.width + x) as usize]
    }
}
