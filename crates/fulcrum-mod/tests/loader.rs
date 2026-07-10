//! Mod loader acceptance: ordering, data-only mods, and hard failures with clear messages.

use fulcrum_core::Fulcrum;
use fulcrum_mod::{LuaRuntime, ModPlugin, ModRegistry};
use fulcrum_scene::ScenePlugin;

fn make_mod(base: &std::path::Path, id: &str, manifest_extra: &str, init: Option<&str>) {
    let root = base.join(id);
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    let scripts = if init.is_some() {
        r#"scripts: ["scripts/init.lua"],"#
    } else {
        ""
    };
    std::fs::write(
        root.join("mod.ron"),
        format!(r#"Mod(id: "{id}", name: "{id}", {manifest_extra} {scripts})"#),
    )
    .unwrap();
    if let Some(source) = init {
        std::fs::write(root.join("scripts/init.lua"), source).unwrap();
    }
}

fn fresh_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fulcrum-loader-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    dir
}

#[test]
fn load_order_is_lexicographic_with_load_after_constraints() {
    let dir = fresh_dir("order");
    // alpha declares it loads after zeta -> zeta first despite the alphabet.
    make_mod(
        &dir.join("mods"),
        "alpha",
        r#"load_after: ["zeta"],"#,
        Some(&appender("alpha")),
    );
    make_mod(&dir.join("mods"), "midway", "", Some(&appender("midway")));
    make_mod(&dir.join("mods"), "zeta", "", Some(&appender("zeta")));

    let mut app = Fulcrum::new("loader test")
        .insert_resource(fulcrum_asset::AssetServer::new(dir.join("assets")))
        .with_plugin(ScenePlugin)
        .with_plugin(ModPlugin::from_dir(dir.join("mods")));
    app.run_startup();

    let order: Vec<String> = app
        .world_mut()
        .resource::<ModRegistry>()
        .mods
        .iter()
        .map(|m| m.id.clone())
        .collect();
    assert_eq!(
        order,
        ["midway", "zeta", "alpha"],
        "ties by id; constraints honored"
    );

    let lua_order = app
        .world_mut()
        .resource::<LuaRuntime>()
        .eval_string("return order")
        .unwrap();
    assert_eq!(
        lua_order, "midway,zeta,alpha,",
        "init callbacks ran in load order"
    );
}

fn appender(id: &str) -> String {
    format!(
        r#"order = order or ""
fulcrum.on_init(function() order = order .. "{id}," end)"#
    )
}

#[test]
fn data_only_mods_override_assets_with_zero_lua() {
    let dir = fresh_dir("dataonly");
    std::fs::write(dir.join("assets/greeting.txt"), b"base").unwrap();
    let mod_root = dir.join("mods/retexture");
    std::fs::create_dir_all(&mod_root).unwrap();
    std::fs::write(
        mod_root.join("mod.ron"),
        r#"Mod(id: "retexture", name: "Retexture")"#,
    )
    .unwrap();
    std::fs::write(mod_root.join("greeting.txt"), b"modded").unwrap();

    let mut app = Fulcrum::new("data-only")
        .insert_resource(fulcrum_asset::AssetServer::new(dir.join("assets")))
        .with_plugin(ScenePlugin)
        .with_plugin(ModPlugin::from_dir(dir.join("mods")));
    app.run_startup();

    let server = app.world_mut().resource::<fulcrum_asset::AssetServer>();
    assert_eq!(server.read_bytes("greeting.txt").unwrap(), b"modded");
    assert_eq!(server.source_of("greeting.txt"), Some("retexture"));
}

#[test]
#[should_panic(expected = "duplicate mod id")]
fn duplicate_ids_fail_at_startup() {
    let dir = fresh_dir("dup");
    make_mod(&dir.join("mods"), "same_a", "", None);
    make_mod(&dir.join("mods"), "same_b", "", None);
    // Rewrite both manifests to the same id.
    for sub in ["same_a", "same_b"] {
        std::fs::write(
            dir.join("mods").join(sub).join("mod.ron"),
            r#"Mod(id: "same", name: "Same")"#,
        )
        .unwrap();
    }
    let _ = Fulcrum::new("dup")
        .insert_resource(fulcrum_asset::AssetServer::new(dir.join("assets")))
        .with_plugin(ModPlugin::from_dir(dir.join("mods")));
}

#[test]
#[should_panic(expected = "cycle")]
fn dependency_cycles_fail_at_startup() {
    let dir = fresh_dir("cycle");
    make_mod(&dir.join("mods"), "aaa", r#"load_after: ["bbb"],"#, None);
    make_mod(&dir.join("mods"), "bbb", r#"load_after: ["aaa"],"#, None);
    let _ = Fulcrum::new("cycle")
        .insert_resource(fulcrum_asset::AssetServer::new(dir.join("assets")))
        .with_plugin(ModPlugin::from_dir(dir.join("mods")));
}
