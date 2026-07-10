# Sparks and Fireflies: Particles

Grove works. This part of the book is about making it *rich* — and the cheapest richness per
line of code is particles. In this chapter gems burst into sparks and fireflies drift through
the hedges, and you'll learn the one architectural fact that makes particles simple: **they are
not simulation**.

## An effect is a data file

Like everything else in Fulcrum, a particle effect is a RON file, hot-reloadable while the game
runs. Here's the gem spark, `assets/fx/spark.fx.ron`:

```ron
ParticleEffect(
    texture: "white.png",
    mode: Burst(14),                 // 14 particles, once
    lifetime: (0.2, 0.5),            // seconds, min..max per particle
    initial_speed: (30.0, 110.0),
    direction_deg: 90.0,
    spread_deg: 360.0,               // full circle
    gravity: (0.0, -140.0),
    size: (start: 3.0, end: 0.5),    // shrink over life
    color_start: (r: 1.0, g: 0.9, b: 0.5, a: 1.0),
    color_end:   (r: 1.0, g: 0.4, b: 0.7, a: 0.0),  // fade to nothing
    additive: true,                  // colors add up where particles overlap: glow
)
```

`mode` is the only structural choice: `Burst(n)` fires everything at once (impacts, pickups,
explosions), `Rate(per_second)` emits continuously (smoke, fire, fireflies). Everything else is
ranges the engine randomizes per particle. `additive: true` draws in a separate blending pass
where overlapping particles brighten instead of occlude — the difference between "some dots"
and "a glow."

## Spawning: one component, or one line

An emitter is an entity like any other — a `ParticleEmitter` plus a `Transform2D`:

```rust,ignore
fn setup_fx(mut commands: Commands, mut effects: EffectLoader) {
    let fireflies = effects.load("fx/fireflies.fx.ron").expect("effect loads");
    commands.spawn((ParticleEmitter::new(fireflies), Transform2D::from_xy(0.0, 0.0)));
}
```

That's the ambient case: a `Rate` emitter that lives until you despawn it (it follows its
transform, so you can parent one to a torch or a rocket). For the fire-and-forget case there's
a `Commands` extension:

```rust,ignore
fn sparkle(mut events: EventReader<GemCollected>, mut effects: EffectLoader,
           mut commands: Commands) {
    for GemCollected(at) in events.read() {
        if let Ok(spark) = effects.load("fx/spark.fx.ron") {
            commands.spawn_effect_at(spark, *at);   // one_shot: cleans itself up
        }
    }
}
```

`spawn_effect_at` spawns a one-shot emitter that despawns itself when its last particle dies.
Loading in the handler is fine — `EffectLoader` caches by path, so it's a hash lookup after the
first call.

## Particles are presentation

Notice *where* that system runs: `add_frame_system`, reading the same `GemCollected` event the
chime does. That's not style, it's the rule from chapter 3 wearing new clothes. Particles
simulate on the frame clock with their own throwaway random source — deliberately outside
`SimRng`, so a thousand sparks never consume a roll the gameplay was counting on. Your headless
tests never see a particle; your replays (chapter 14) reproduce battles exactly even though the
smoke curls differently every time. The sim announces *what happened*, and particles are just
another opinion about what it should look like.

The practical consequence: emit particles from **frame systems reading events**, never from
simulation systems. If you find yourself wanting a particle to deal damage, that's not a
particle — that's a projectile entity, and it belongs in chapter 2's world.

```text
cargo run -p grove --example ch11_particles
```

Walk into a gem: a golden burst, fading pink. Stand still: fireflies. Edit `spark.fx.ron` while
it runs — count, colors, gravity — and watch it change on save.
