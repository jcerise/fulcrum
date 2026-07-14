# Switching Clips by Hand

This chapter makes the hero playable — walk with WASD, swing with Space — by switching clips
the way your instincts suggest: with `if` statements. Everything you write here is correct,
idiomatic, and *doomed*: chapter 3 deletes most of it. Building the doomed version first is
the point. State machines are unmotivated machinery until you've hand-rolled the thing they
replace, and the three player-control tools you'll meet (`restart`, `play`, `finished`)
survive into every future chapter.

## Step 1 — stash the clips

The control system will need to switch between three clips, so keep their handles in a
resource. Add above `setup`:

```rust,ignore
/// Clip handles, stashed at startup so the control system can switch between them.
#[derive(Resource)]
struct Art {
    idle: Handle<AnimationClip>,
    run: Handle<AnimationClip>,
    attack: Handle<AnimationClip>,
}

#[derive(Component)]
struct HeroTag;
```

And in `setup`, drop the dummy for now (it returns in chapter 4), tag the hero, and stash:

```rust,ignore
    let hero = aseprite.load("hero.json").expect("hero sheet loads");
    commands.insert_resource(Art {
        idle: hero.clips["idle"],
        run: hero.clips["run"],
        attack: hero.clips["attack"],
    });
    commands.spawn((
        HeroTag,
        Sprite::from_sheet(hero.sheet, 0).with_z(1.0),
        Transform2D::from_xy(160.0, 90.0),
        AnimationPlayer::play(hero.clips["idle"]),
    ));
```

## Step 2 — the control system, rule by rule

Type this slowly — each block is one *rule of animation logic*, and counting them is this
chapter's homework:

```rust,ignore
fn control(
    mut heroes: Query<(&mut Transform2D, &mut AnimationPlayer, &mut Sprite), With<HeroTag>>,
    art: Option<Res<Art>>,
    input: Res<Input>,
    time: Res<Time>,
) {
    let (Some(art), Ok((mut transform, mut player, mut sprite))) = (art, heroes.single_mut())
    else {
        return;
    };

    // Rule: an attack in progress locks everything until its clip finishes.
    let attacking = player.clip == art.attack;
    if attacking && !player.finished() {
        return;
    }
    // Rule: a finished attack hands back to idle.
    if attacking && player.finished() {
        *player = AnimationPlayer::play(art.idle);
    }

    let mut dir = Vec2::ZERO;
    if input.pressed(Key::A) {
        dir.x -= 1.0;
    }
    if input.pressed(Key::D) {
        dir.x += 1.0;
    }
    if input.pressed(Key::S) {
        dir.y -= 1.0;
    }
    if input.pressed(Key::W) {
        dir.y += 1.0;
    }

    // Rule: Space starts the attack clip from frame zero, unconditionally.
    if input.just_pressed(Key::Space) {
        *player = AnimationPlayer::play(art.attack);
        return;
    }

    if dir != Vec2::ZERO {
        transform.translation += dir.normalize() * 90.0 * time.fixed_delta;
        if dir.x != 0.0 {
            sprite.flip_x = dir.x < 0.0;
        }
        // Rule: moving means the run clip — restart(), not play(), so holding a key
        // doesn't restart the clip every tick.
        player.restart(art.run);
    } else {
        // Rule: still means idle.
        player.restart(art.idle);
    }
}
```

Register it — animation logic is simulation logic, so it's a tick system:

```rust,ignore
        .add_startup(setup)
        .add_system(control)      // <-- new
        .run();
```

Run it. Walk around; the run cycle plays and the hero faces where he's going (`flip_x` —
one sheet, both directions, free). Swing; the hero commits to the whole attack, then returns
to idle. It *feels* right, and three tool choices are why:

> **Toolbox — `restart(clip)` vs `play(clip)`:** `restart` is a no-op if that clip is
> already the active one; `play` unconditionally starts from frame zero. The movement rules
> must use `restart` — `control` runs sixty times a second, and `play(run)` every tick
> would pin the run cycle to its first frame forever (a classic first bug; try it). The
> attack must use `play` — a second swing should restart the swing even if the clip
> matches. Wrong choice in either place feels broken in a way players can't articulate.
>
> **Toolbox — `finished()`:** true once a **non-looping** clip has fully elapsed. The
> attack tag carried `"repeat": "1"`, so its clip imported as play-once; `finished()` is
> how gameplay notices the swing is over. A looping clip never finishes — if a wait on
> `finished()` hangs forever, check the tag.
>
> **Toolbox — clip identity:** `player.clip == art.attack` — handles compare cheaply, so
> "which animation is this entity in?" is an equality test. This is the poor man's state
> query, and its clumsiness is foreshadowing.

## Step 3 — feel the doom

The system works. Now audit it. Count the rules encoded in `control`: lock-while-attacking,
attack-hands-to-idle, space-starts-attack, moving-means-run, still-means-idle, plus the
subtle `restart`/`play` distinction holding two of them together. Six animation rules,
braided *through* the movement logic — the `return`s are load-bearing, and the order of the
blocks matters.

Now run the thought experiment that kills every hand-rolled version. The designer asks for:
a hit reaction that interrupts attacks (new rule, and it must beat the attack lock), a
charge attack on held Space (now `attacking` isn't binary), and footstep dust only during
run (now other systems need to know the state too — do they all compare clip handles?).
Every addition multiplies against every existing `if`. This function is where animation
bugs will live for the rest of the project.

The diagnosis, precisely: **gameplay and animation-flow logic are the same code here.**
Gameplay's job is facts — "moving at 90 units/sec", "attack requested". Which clip that
implies, what interrupts what, where a finished clip goes next: that's a separate concern,
and it's *data-shaped*. Chapter 3 moves it into a file.

## Checkpoint

Your program should match `games/dojo/examples/an02_switching.rs`:

```text
cargo run -p dojo --example an02_switching
```

## Exercises

1. **The classic bug, on purpose.** Change the movement rule's `restart(art.run)` to
   `play(art.run)` and walk around. Diagnose what you see before reading the Toolbox note
   again. Every engine's forum has this bug reported weekly with different words.
2. **Backpedal.** Make S-while-facing-right (and W/E equivalents) keep the current facing
   instead of flipping — moving away from the dummy while watching it. One condition — but
   decide *which* rule owns it, and notice how crowded this function is getting.
3. **A fourth clip (harder).** Wire up the `hit` clip: on pressing H, play it, lock movement
   until it finishes, return to idle — and it must interrupt an in-progress attack. Do it
   honestly in `control`. Count how many existing lines you had to *read and reason about*
   to add one state safely. Keep your version; chapter 3 does the same feature in two lines
   of RON, and you'll want the comparison.

Next: the file that deletes most of this function.
