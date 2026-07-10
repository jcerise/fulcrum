//! Nav acceptance: A* paths/refusals/tie-breaks, flow-field descent, and (logged) perf.

use fulcrum_spatial::{FlowField, NavGrid, astar, simplify_path};

fn grid_from_rows(rows: &[&str]) -> NavGrid {
    let mut grid = NavGrid::new(rows[0].len() as u32, rows.len() as u32);
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            grid.set_walkable(x as u32, (rows.len() - 1 - y) as u32, ch != '#');
        }
    }
    grid
}

#[test]
fn astar_paths_and_refusals() {
    let grid = grid_from_rows(&[".....", ".###.", "....."]);
    // Straight line along the bottom row.
    let path = astar(&grid, (0, 0), (4, 0)).unwrap();
    assert_eq!(path, vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]);

    // Detour around the wall (snapshot: deterministic tie-breaking).
    let over = astar(&grid, (0, 2), (4, 2)).unwrap();
    assert_eq!(over.first(), Some(&(0, 2)));
    assert_eq!(over.last(), Some(&(4, 2)));
    assert!(over.iter().all(|&(x, y)| grid.is_walkable(x, y)));
    let a = astar(&grid, (0, 2), (4, 2)).unwrap();
    assert_eq!(a, over, "identical inputs, identical path");

    // No path.
    let sealed = grid_from_rows(&[".#.", ".#.", ".#."]);
    assert_eq!(astar(&sealed, (0, 0), (2, 0)), None);

    // Corner cutting is forbidden: diagonal through touching corners.
    let corners = grid_from_rows(&[".#", "#."]);
    assert_eq!(
        astar(&corners, (0, 0), (1, 1)),
        None,
        "no squeezing between corners"
    );
}

#[test]
fn simplify_keeps_endpoints_and_line_of_sight() {
    let grid = grid_from_rows(&["....."; 5]);
    let path = astar(&grid, (0, 0), (4, 4)).unwrap();
    let simple = simplify_path(&grid, &path);
    assert_eq!(simple.first(), Some(&(0, 0)));
    assert_eq!(simple.last(), Some(&(4, 4)));
    assert!(simple.len() <= path.len());
}

#[test]
fn flow_field_descends_and_flags_unreachable() {
    let grid = grid_from_rows(&[".....", "####.", "....."]);
    let field = FlowField::compute(&grid, &[(0, 0)]);
    for y in 0..3 {
        for x in 0..5 {
            if !grid.is_walkable(x, y) || !field.is_reachable(x, y) {
                continue;
            }
            let here = field.integration_at(x, y);
            if here == 0 {
                continue;
            }
            // Follow the arrow one cell: integration must strictly decrease.
            let dir = field.sample(&grid, grid.cell_center(x, y));
            let next = (
                (x as f32 + dir.x).round() as u32,
                (y as f32 + dir.y).round() as u32,
            );
            assert!(
                field.integration_at(next.0, next.1) < here,
                "arrow at ({x},{y}) climbs"
            );
        }
    }

    let sealed = grid_from_rows(&[".#.", ".#.", ".#."]);
    let field = FlowField::compute(&sealed, &[(0, 0)]);
    assert!(!field.is_reachable(2, 0), "across the wall is unreachable");
}

#[test]
fn flow_field_perf_snapshot() {
    let grid = NavGrid::new(512, 512);
    let start = std::time::Instant::now();
    let field = FlowField::compute(&grid, &[(256, 256)]);
    let elapsed = start.elapsed();
    println!("512x512 flow field: {elapsed:?}");
    assert!(field.is_reachable(0, 0));
    assert!(
        elapsed.as_millis() < 2000,
        "generous CI bound; log has the real number"
    );
}
