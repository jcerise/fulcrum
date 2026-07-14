# The Aseprite Pipeline

How art gets in: Fulcrum's native animation interchange is the
[Aseprite](https://www.aseprite.org/) JSON export — the de-facto standard for pixel art,
and plain enough to generate from any tool (or script) when no Aseprite is involved.
Implementation: `crates/fulcrum-anim/src/aseprite.rs`.

## Exporting

```text
aseprite -b hero.ase --sheet hero.png --data hero.json --format json-array --list-tags
```

- `--format json-array` — the importer reads the array form (`"frames": [...]`), not the
  hash form.
- `--list-tags` — tags become clips; without this flag you get a sheet and no animations.
- The PNG is resolved **relative to the JSON's directory** via the JSON's `meta.image`
  field — keep them side by side and paths take care of themselves.

## What the importer reads

```json
{
  "frames": [
    { "filename": "hero idle 0", "frame": {"x":0,"y":0,"w":16,"h":16}, "duration": 100 }
  ],
  "meta": {
    "image": "hero.png",
    "frameTags": [
      { "name": "attack", "from": 10, "to": 13, "direction": "forward", "repeat": "1" }
    ]
  }
}
```

Per **frame**: the rectangle becomes a sheet region; a non-empty `filename` also names it
(usable as `region: "hero idle 0"` in prefabs and `Sprite` lookups); `duration` is
milliseconds, converted to ticks at load (`round(ms/1000 × tick_rate)`, minimum 1 — at
60 Hz, durations under ~8 ms all become 1 tick).

Per **tag**, one `AnimationClip`:

| Tag field | Effect |
| --- | --- |
| `name` | the clip's key in `AsepriteImport::clips`, and the `#tag` in machine references |
| `from`/`to` | inclusive frame range (validated against the frame count — bad ranges are load errors) |
| `direction` | `forward`, `reverse`, or `pingpong` (`0 1 2 3` → `0 1 2 3 2 1` — interior frames only, so the ends don't double) |
| `repeat` | **absent = loop forever.** `"N"` (a string, per Aseprite) = play N times then stop: frames unroll N×, `looping: false`, `finished()` can fire |

`repeat` is how one-shot animations exist: in Aseprite, set the tag's *Repeat* property to
1 (Tag Properties → Repeat). A machine state whose `on_finish` never fires is almost always
a tag missing its `repeat` — the clip loops, so it never finishes.

## Loading

```rust,ignore
fn setup(mut aseprite: AsepriteLoader, ...) {
    let art: AsepriteImport = aseprite.load("hero.json").expect("loads");
    // art.sheet: Handle<SpriteSheet>       — regions, named and indexed
    // art.clips: map of tag name → Handle<AnimationClip>
}
```

Loads are cached by path — call `load` freely; the file is read once. Errors are
descriptive (`AssetError::Decode` naming the path and problem), not panics.

**Headless, the same call works.** `AsepriteLoader`'s GPU handle is optional: regions and
clip timing import normally (they're simulation data); texture upload is skipped and the
sheet's texture handle stays invalid. This is what lets game startup code that loads
animation run identically under `cargo test` — the dojo's whole test suite rests on it.

## Hot reload

Windowed, Aseprite files hot-reload: save a change to a loaded `.json` (or its PNG) and the
importer re-imports **over the same handles** — every sheet region, every clip, every
machine state pointing at them updates in place, mid-game, no restart. Live players whose
clip shrank clamp to the last frame instead of indexing out of bounds. The loop is: run the
game, retune durations in the JSON, save, feel it. (Timing changes *are* gameplay changes
when mechanics are frame-keyed — the dojo's tests will tell you which retunes broke the
game's promises.)

## No Aseprite? Generate the JSON

The format above is the whole contract — `frames` + `meta.image` + `meta.frameTags` — and
anything that writes it is a valid art pipeline. The repository generates all of the dojo's
art programmatically: `tools/gen_dojo_art.py` draws frames with PIL, packs the PNG, and
emits the JSON (tags, durations, `repeat` and all). Fork it for your own placeholder art,
or point your texture packer's custom-export template at the same shape.
