//! Asset storage and server tests: path dedup, missing files.

use fulcrum_asset::{AssetError, AssetServer, Assets};

#[test]
fn same_path_dedups_to_same_handle() {
    let mut assets: Assets<String> = Assets::default();
    let first = assets.insert_with_path("sprites/ship.png", "ship".to_string());

    // A loader always checks handle_for_path before inserting; simulate a second load.
    let second = assets
        .handle_for_path("sprites/ship.png")
        .expect("path registered");
    assert_eq!(first, second);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets.get(second).unwrap(), "ship");
}

#[test]
fn distinct_paths_get_distinct_handles() {
    let mut assets: Assets<String> = Assets::default();
    let a = assets.insert_with_path("a.png", "a".to_string());
    let b = assets.insert_with_path("b.png", "b".to_string());
    assert_ne!(a, b);
    assert_eq!(assets.get(a).unwrap(), "a");
    assert_eq!(assets.get(b).unwrap(), "b");
}

#[test]
fn missing_file_is_an_io_error_not_a_panic() {
    let server = AssetServer::new("nonexistent-root");
    let result = server.read_bytes("nope.png");
    match result {
        Err(AssetError::Io { path, .. }) => assert_eq!(path, "nope.png"),
        other => panic!("expected Io error, got {other:?}"),
    }
}

#[test]
fn read_bytes_resolves_against_root() {
    let dir = std::env::temp_dir().join("fulcrum-asset-test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("data.bin"), b"hello").unwrap();

    let server = AssetServer::new(&dir);
    assert_eq!(server.read_bytes("data.bin").unwrap(), b"hello");
}
