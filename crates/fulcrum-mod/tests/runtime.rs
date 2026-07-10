//! Sandbox and runtime acceptance: capabilities absent, deterministic RNG, budget aborts,
//! require with cycles.

use fulcrum_mod::LuaRuntime;

fn mod_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fulcrum-lua-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    dir
}

fn runtime_with(name: &str, seed: u64, init_lua: &str) -> LuaRuntime {
    let dir = mod_dir(name);
    std::fs::write(dir.join("scripts/init.lua"), init_lua).unwrap();
    let mut runtime = LuaRuntime::new(seed).unwrap();
    runtime.register_mod("testmod", &dir);
    runtime
        .run_entry("testmod", "scripts/init.lua")
        .unwrap_or_else(|e| panic!("entry failed: {e}"));
    runtime
}

#[test]
fn dangerous_capabilities_are_absent() {
    let runtime = runtime_with(
        "sandbox",
        1,
        r#"
        assert(io == nil, "io must be nil")
        assert(package == nil, "package must be nil")
        assert(dofile == nil, "dofile must be nil")
        assert(load == nil, "load must be nil")
        assert(os.execute == nil, "os.execute must be nil")
        assert(type(os.clock) == "function", "os.clock stub exists")
        local ok = pcall(require, "socket")
        assert(not ok, "require of external modules must fail")
        "#,
    );
    drop(runtime); // asserts ran inside the entry script
}

#[test]
fn math_random_is_deterministic_per_seed() {
    let script = r#"
        acc = ""
        fulcrum.on_tick(function(tick)
            acc = acc .. tostring(math.random(1, 1000)) .. ","
        end)
    "#;
    let run = |seed| {
        let mut runtime = runtime_with(&format!("rng-{seed}"), seed, script);
        for tick in 0..50 {
            runtime.run_tick(tick, 60);
        }
        runtime.eval_string("return acc").unwrap()
    };
    let a = run(42);
    let b = run(42);
    let c = run(43);
    assert_eq!(a, b, "same seed, same rolls");
    assert_ne!(a, c, "different seed, different rolls");
    assert!(a.len() > 100, "rolls actually accumulated: {a}");
}

#[test]
fn runaway_scripts_abort_and_the_game_survives() {
    let mut runtime = runtime_with(
        "budget",
        1,
        r#"
        ran_after = 0
        fulcrum.on_tick(function() while true do end end)
        fulcrum.on_tick(function() ran_after = ran_after + 1 end)
        "#,
    );
    for tick in 0..4 {
        runtime.run_tick(tick, 60); // first callback errors (and gets disabled after 3)
    }
    let after: String = runtime.eval_string("return tostring(ran_after)").unwrap();
    assert_eq!(after, "4", "later callbacks kept running every tick");
}

#[test]
fn require_resolves_within_the_mod_and_reports_cycles() {
    let dir = mod_dir("require");
    std::fs::write(
        dir.join("scripts/util.lua"),
        "return { double = function(x) return x * 2 end }",
    )
    .unwrap();
    std::fs::write(
        dir.join("scripts/init.lua"),
        r#"
        local util = require("util")
        result = util.double(21)
    "#,
    )
    .unwrap();
    let mut runtime = LuaRuntime::new(1).unwrap();
    runtime.register_mod("testmod", &dir);
    runtime.run_entry("testmod", "scripts/init.lua").unwrap();
    assert_eq!(
        runtime.eval_string("return tostring(result)").unwrap(),
        "42"
    );

    // Cycle: a -> b -> a.
    let dir = mod_dir("cycle");
    std::fs::write(dir.join("scripts/a.lua"), r#"return require("b")"#).unwrap();
    std::fs::write(dir.join("scripts/b.lua"), r#"return require("a")"#).unwrap();
    std::fs::write(dir.join("scripts/init.lua"), r#"require("a")"#).unwrap();
    let mut runtime = LuaRuntime::new(1).unwrap();
    runtime.register_mod("cyclic", &dir);
    let error = runtime.run_entry("cyclic", "scripts/init.lua").unwrap_err();
    assert!(error.contains("cycle"), "error names the cycle: {error}");
}
