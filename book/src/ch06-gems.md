# Gems, Events, and Sound

Time to make it a game: gems to collect, a count, and a chime.

## Sim decides, presentation reacts

The collecting itself is simulation — two queries and a distance check:

```rust,ignore
#[derive(Event)]
struct GemCollected;

fn collect(
    mut commands: Commands,
    gems: Query<(Entity, &Transform2D), With<Gem>>,
    players: Query<&Transform2D, (With<Player>, Without<Gem>)>,
    mut collected: ResMut<Collected>,
    mut events: EventWriter<GemCollected>,
) {
    let Ok(player) = players.single() else { return };
    for (gem, at) in &gems {
        if at.translation.distance(player.translation) < 14.0 {
            commands.entity(gem).despawn();
            collected.0 += 1;
            events.write(GemCollected);
        }
    }
}
```

The new piece is the **event**. `#[derive(Event)]` + `add_event::<GemCollected>()` gives you a
buffered channel: any system can `EventWriter::write`, any system can `EventReader::read`, and
each reader sees each event exactly once. Events are how the simulation talks to the
presentation without knowing it exists — the sim announces *what happened*; a frame system
decides it should sound like something:

```rust,ignore
fn present(mut events: EventReader<GemCollected>, mut audio: ResMut<Audio>,
           ding: Res<Ding>, sounds: Res<Assets<Sound>>) {
    for _ in events.read() {
        audio.play(&sounds, ding.0);
    }
}
```

This split is a habit the whole engine rewards: when you write the headless test in chapter
10, the sim runs alone and the events are still there to assert on.

## Audio in three lines

```rust,ignore
let ding = sounds.load("ding.wav");            // SoundLoader: wav/ogg/mp3/flac
audio.play(&sounds, ding);                     // fire and forget
audio.play_with(&sounds, ding, PlayParams { volume: 0.8, pitch: 1.1, pan: 0.0 });
audio.play_music(sounds.assets(), music, true); // one looping music slot
```

Playback is cosmetic by definition — nothing about it feeds back into the simulation, so it's
exempt from the determinism rules. A machine with no audio device plays silence instead of
crashing.

## A score you can see

For now the count is a world-space `Text` entity — spawn `Text::new("Gems: 0")` with a
`Transform2D` and update its `value` from a frame system. Text renders through the same
sprite batcher (the built-in pixel font is embedded; sizes that are multiples of 8 are
crispest). It works, but it lives *in the world*, which is why chapter 8 replaces it with
real screen-space UI.

```text
cargo run -p grove --example ch06_gems
```

Six gems in a ring, a chime per pickup, a live count. It's nearly a game — it's just that all
of it is hard-coded. Let's fix that properly.
