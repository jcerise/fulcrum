//! Grid acceptance: property tests vs brute force, deterministic ordering, and (logged) perf.

use bevy_ecs::entity::Entity;
use fulcrum_core::{Rect, SimRng, Vec2, vec2};
use fulcrum_spatial::SpatialGrid;
use fulcrum_spatial::grid::brute_force_circle;

fn random_entries(count: u32, seed: u64) -> Vec<(Entity, Vec2)> {
    let mut rng = SimRng::seeded(seed);
    (0..count)
        .map(|i| {
            (
                Entity::from_bits(1u64 << 32 | (i + 1) as u64), // gen 1; index bits must be nonzero
                vec2(rng.range_f32(-500.0..500.0), rng.range_f32(-500.0..500.0)),
            )
        })
        .collect()
}

#[test]
fn grid_matches_brute_force_for_circles_and_rects() {
    let entries = random_entries(1000, 7);
    let mut grid = SpatialGrid::new(64.0);
    grid.rebuild_for_test(entries.clone());
    let mut rng = SimRng::seeded(99);
    for _ in 0..100 {
        let center = vec2(rng.range_f32(-550.0..550.0), rng.range_f32(-550.0..550.0));
        let radius = rng.range_f32(0.0..200.0);
        let mut from_grid = grid.query_circle(center, radius);
        from_grid.sort_unstable();
        assert_eq!(from_grid, brute_force_circle(&entries, center, radius));

        let size = vec2(rng.range_f32(1.0..300.0), rng.range_f32(1.0..300.0));
        let rect = Rect::from_center_size(center, size);
        let mut from_grid = grid.query_rect(rect);
        from_grid.sort_unstable();
        let mut brute: Vec<Entity> = entries
            .iter()
            .filter(|(_, p)| rect.contains(*p))
            .map(|(e, _)| *e)
            .collect();
        brute.sort_unstable();
        assert_eq!(from_grid, brute);
    }
}

#[test]
fn query_order_is_deterministic() {
    let entries = random_entries(500, 3);
    let mut a = SpatialGrid::new(64.0);
    let mut b = SpatialGrid::new(64.0);
    a.rebuild_for_test(entries.clone());
    let mut reversed = entries.clone();
    reversed.reverse(); // different insertion order must not matter
    b.rebuild_for_test(reversed);
    for radius in [10.0, 80.0, 300.0] {
        assert_eq!(
            a.query_circle(Vec2::ZERO, radius),
            b.query_circle(Vec2::ZERO, radius),
            "same result *sequence*, not just set"
        );
    }
    // nearest ties break by entity id.
    let mut grid = SpatialGrid::new(64.0);
    grid.rebuild_for_test(vec![
        (Entity::from_bits(2 << 32 | 2), vec2(10.0, 0.0)),
        (Entity::from_bits(1 << 32 | 1), vec2(10.0, 0.0)),
    ]);
    assert_eq!(
        grid.nearest(Vec2::ZERO, 100.0, |_| true),
        Some(Entity::from_bits(1 << 32 | 1))
    );
}

#[test]
fn spatial_perf_snapshot() {
    // 10k entities across a 4000x4000 world (game-like density, not a stress pile-up).
    let mut rng = SimRng::seeded(5);
    let entries: Vec<(Entity, Vec2)> = (0..10_000u32)
        .map(|i| {
            (
                Entity::from_bits(1u64 << 32 | (i + 1) as u64),
                vec2(
                    rng.range_f32(-2000.0..2000.0),
                    rng.range_f32(-2000.0..2000.0),
                ),
            )
        })
        .collect();
    let mut grid = SpatialGrid::new(64.0);
    let start = std::time::Instant::now();
    grid.rebuild_for_test(entries);
    for i in 0..500 {
        let _ = grid.query_circle(vec2((i % 100) as f32 * 40.0 - 2000.0, 0.0), 64.0);
    }
    let elapsed = start.elapsed();
    println!("10k rebuild + 500 circle queries: {elapsed:?}");
    assert!(
        elapsed.as_millis() < 200,
        "generous CI bound; log has the real number"
    );
}
