//! Watcher integration: real filesystem events arrive as asset-relative paths.

use std::time::{Duration, Instant};

use fulcrum_asset::AssetWatcher;

#[test]
fn file_change_produces_relative_path_event() {
    let root = std::env::temp_dir().join("fulcrum-watch-test");
    let sub = root.join("sprites");
    std::fs::create_dir_all(&sub).unwrap();
    let file = sub.join("hero.png");
    std::fs::write(&file, b"v1").unwrap();

    let watcher = AssetWatcher::start(&root).expect("watcher starts");
    // Give the backend a moment to arm before mutating.
    std::thread::sleep(Duration::from_millis(200));
    std::fs::write(&file, b"v2").unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        seen.extend(watcher.drain());
        if seen.iter().any(|p| p == "sprites/hero.png") {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("no event for sprites/hero.png; saw {seen:?}");
}
