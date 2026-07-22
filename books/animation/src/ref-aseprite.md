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
- No `--trim` — the importer doesn't read trim offsets, and trimmed frames wobble. See
  [Frame sizes](#frame-sizes-canvases-and-collision) below.

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

## Frame sizes, canvases, and collision

Nothing in the pipeline assumes frames share a size. A sheet's regions are arbitrary pixel
rectangles (`SpriteSheet { regions: Vec<Rect> }`), and a `Sprite` with no `custom_size`
draws at its current region's own size — so a 16×16 critter and a 48×48 boss come through
the same workflow, and two creatures never need to agree on dimensions.

*Within one animation*, though, mixed sizes raise a question the renderer can't answer for
you: where does the bigger frame sit? A sprite is placed by its `anchor` — a *fraction* of
the drawn size, centered by default — so if the spear-thrust frame is 12 px wider than the
rest of the clip, a centered anchor shoves the body 6 px backward for exactly one frame:
the classic frame wobble. The idiomatic fix is to never have mixed sizes in a clip at all:

**Size the canvas for the extremes, and export untrimmed.** In Aseprite the canvas is
fixed per file; make it big enough for the most extended frame — the thrust, the
follow-through — and let the compact frames carry transparent padding. Untrimmed, every
frame exports at canvas size, alignment is automatic under the default centered anchor,
and the padding costs only texture space: at pixel-art scale, the right trade essentially
always.

> **Don't pass `--trim`.** Aseprite can shrink each exported frame to its opaque pixels,
> emitting `spriteSourceSize`/`sourceSize` offsets so an engine can re-align them. The
> importer reads only `frame` and `duration` — a trimmed export loads without error and
> then wobbles, because the offsets that would re-anchor each frame were dropped on the
> floor.

If you *do* mix sizes in a clip — hand-packed sheets from elsewhere, say — `anchor` is
your lever: bottom-center keeps feet planted while height varies. One warning label on
that lever: `flip_x` mirrors the sprite's pixels, not its geometry, so an off-center
anchor stays where it was when the sprite flips. A direction-facing game that leans on
asymmetric anchors must mirror the anchor by hand alongside the flip.

Which leaves the question the padded canvas always raises: **doesn't a canvas sized for
the spear thrust widen the hitbox for every frame?** No — nothing in Fulcrum derives
collision from a sprite. The `Sprite` is presentation; the simulation never reads one
(headless, sprites don't even exist). What things can *hit* is simulation data you
declare: the dojo's strike is `delta.length() <= STRIKE_RANGE`, gated to the extension
frame ([Gameplay on Frames](tut04-frame-keyed.md)); spatial queries take explicit shapes
(`SpatialGrid::query_circle(center, radius)`); grid games compare cells. The spear's
*reach* is `STRIKE_RANGE` on the frames where the spear is out — gameplay data, keyed to
the animation, and entirely indifferent to how much transparent padding the art carries.

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
