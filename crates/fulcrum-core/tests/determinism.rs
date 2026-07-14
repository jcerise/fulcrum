//! The determinism harness (phase-1 seed): identical seeds and inputs must produce
//! bit-identical simulation state. Grows with each phase; becomes a CI gate in phase 4.

use fulcrum_core::{
    Commands, Component, Fulcrum, FulcrumConfig, IntoScheduleConfigs, Query, ResMut, SimRng,
    Transform2D, vec2,
};

#[derive(Component)]
struct Mover(f32); // speed

/// Spawns a mover at an RNG position roughly every third tick, RNG-decided.
fn spawn_movers(mut commands: Commands, mut rng: ResMut<SimRng>) {
    if rng.chance(0.33) {
        let position = vec2(rng.range_f32(-400.0..400.0), rng.range_f32(-300.0..300.0));
        let speed = rng.range_f32(10.0..60.0);
        commands.spawn((Transform2D::from_translation(position), Mover(speed)));
    }
}

/// Jitters every mover by an RNG-scaled step.
fn move_movers(mut movers: Query<(&mut Transform2D, &Mover)>, mut rng: ResMut<SimRng>) {
    for (mut transform, mover) in &mut movers {
        let dir = vec2(rng.range_f32(-1.0..1.0), rng.range_f32(-1.0..1.0));
        transform.translation += dir * mover.0 * (1.0 / 60.0);
        transform.rotation += rng.range_f32(-0.1..0.1);
    }
}

/// Run the RNG-heavy sim for `ticks` and return a bit-exact fingerprint of world state.
fn run_and_fingerprint(seed: u64, ticks: u32) -> Vec<(u32, u32, u32)> {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .add_system((spawn_movers, move_movers).chain());

    app.run_startup();
    for _ in 0..ticks {
        app.tick();
    }

    let world = app.world_mut();
    world
        .query::<&Transform2D>()
        .iter(world)
        .map(|t| {
            (
                t.translation.x.to_bits(),
                t.translation.y.to_bits(),
                t.rotation.to_bits(),
            )
        })
        .collect()
}

#[test]
fn determinism_same_seed_same_state_after_1000_ticks() {
    let a = run_and_fingerprint(42, 1000);
    let b = run_and_fingerprint(42, 1000);
    assert!(!a.is_empty(), "the sim actually spawned entities");
    assert_eq!(a, b, "identical seeds must produce bit-identical state");
}

#[test]
fn determinism_different_seeds_diverge() {
    let a = run_and_fingerprint(1, 200);
    let b = run_and_fingerprint(2, 200);
    assert_ne!(a, b, "different seeds should not coincide");
}

#[test]
fn determinism_forked_streams_are_independent() {
    let mut parent_a = SimRng::seeded(7);
    let mut fork_a = parent_a.fork();
    let mut parent_b = SimRng::seeded(7);
    let mut fork_b = parent_b.fork();

    let seq_a: Vec<u32> = (0..16).map(|_| fork_a.u32()).collect();
    let seq_b: Vec<u32> = (0..16).map(|_| fork_b.u32()).collect();
    assert_eq!(seq_a, seq_b, "forks of equal parents are equal");

    // Consuming the fork doesn't advance the parent beyond the single fork() call.
    assert_eq!(parent_a.u32(), parent_b.u32());
}
