#!/usr/bin/env python3
"""Generate the dojo game's pixel art: hero.png/json + dummy.png/json (Aseprite-style
JSON-array exports). Deliberately simple 16x16 art — the animation book is about motion,
not rendering. Rerun after edits: python3 tools/gen_dojo_art.py"""

import json
import os
from PIL import Image

# Palette
SKIN = (238, 182, 134, 255)
HAIR = (90, 58, 36, 255)
EYE = (30, 24, 20, 255)
TUNIC = (62, 120, 194, 255)
TUNIC_D = (46, 90, 148, 255)
BELT = (122, 74, 36, 255)
LEGS = (72, 72, 92, 255)
BOOTS = (45, 45, 60, 255)
BLADE = (223, 227, 234, 255)
GLINT = (255, 255, 255, 255)
HILT = (138, 106, 66, 255)
ARC = (255, 233, 160, 255)
FLASH = (255, 255, 255, 255)
HURT = (232, 90, 80, 255)

POST = (138, 98, 66, 255)
POST_D = (94, 66, 44, 255)
SACK = (207, 166, 107, 255)
STITCH = (122, 90, 52, 255)
BASE = (74, 58, 42, 255)

S = 16  # frame size


def blank():
    return Image.new("RGBA", (S, S), (0, 0, 0, 0))


def rect(img, x0, y0, x1, y1, c):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            if 0 <= x < S and 0 <= y < S:
                img.putpixel((x, y), c)


def px(img, x, y, c):
    if 0 <= x < S and 0 <= y < S:
        img.putpixel((x, y), c)


def hero(bob=0, legs="stand", arms="side", sword=None, lean=0, tint=None):
    """One hero frame, facing right. bob: body y-offset. lean: body x-offset."""
    img = blank()
    bx, by = lean, bob

    # legs first (never bob — the body bobs over them)
    if legs == "stand":
        rect(img, 5, 12, 6, 13, LEGS)
        rect(img, 9, 12, 10, 13, LEGS)
        rect(img, 5, 14, 6, 14, BOOTS)
        rect(img, 9, 14, 10, 14, BOOTS)
    else:  # run poses: (front leg dx, back leg dx, feet up?)
        front, back = legs
        rect(img, 9 + front, 12, 10 + front, 13, LEGS)
        rect(img, 5 + back, 12, 6 + back, 13, LEGS)
        rect(img, 9 + front, 14, 10 + front, 14, BOOTS)
        rect(img, 5 + back, 14, 6 + back, 14, BOOTS)

    # torso
    rect(img, 4 + bx, 7 + by, 10 + bx, 10 + by, TUNIC)
    rect(img, 4 + bx, 10 + by, 10 + bx, 10 + by, TUNIC_D)
    rect(img, 4 + bx, 11 + by, 10 + bx, 11 + by, BELT)

    # head
    rect(img, 4 + bx, 2 + by, 10 + bx, 6 + by, SKIN)
    rect(img, 4 + bx, 1 + by, 10 + bx, 2 + by, HAIR)
    px(img, 4 + bx, 3 + by, HAIR)
    px(img, 9 + bx, 4 + by, EYE)

    # arms + sword
    if arms == "side":
        rect(img, 3 + bx, 7 + by, 3 + bx, 10 + by, SKIN)
        rect(img, 11 + bx, 7 + by, 11 + bx, 10 + by, SKIN)
    elif arms == "swing_f":  # run, arms mid-swing forward
        rect(img, 3 + bx, 8 + by, 3 + bx, 10 + by, SKIN)
        rect(img, 11 + bx, 7 + by, 12 + bx, 8 + by, SKIN)
    elif arms == "swing_b":
        rect(img, 3 + bx, 7 + by, 3 + bx, 8 + by, SKIN)
        rect(img, 11 + bx, 8 + by, 11 + bx, 10 + by, SKIN)
    elif arms == "up":  # hit reaction
        rect(img, 3 + bx, 6 + by, 3 + bx, 8 + by, SKIN)
        rect(img, 11 + bx, 6 + by, 11 + bx, 8 + by, SKIN)

    if sword == "raised":  # windup: blade above the head, angled back
        rect(img, 11 + bx, 7 + by, 12 + bx, 8 + by, SKIN)
        px(img, 12 + bx, 6 + by, HILT)
        for i in range(4):
            px(img, 11 + bx - i, 5 + by - i, BLADE)
            px(img, 12 + bx - i, 5 + by - i, BLADE)
        px(img, 8 + bx, 1 + by, GLINT)
    elif sword == "mid":  # blade sweeping overhead, arc trailing
        rect(img, 11 + bx, 6 + by, 12 + bx, 7 + by, SKIN)
        px(img, 13 + bx, 5 + by, HILT)
        for i in range(3):
            px(img, 14 + bx, 4 + by - i, BLADE)
        px(img, 14 + bx, 1 + by, GLINT)
        px(img, 12 + bx, 2 + by, ARC)
        px(img, 10 + bx, 1 + by, ARC)
    elif sword == "out":  # full extension, blade level at torso height
        rect(img, 11 + bx, 8 + by, 12 + bx, 8 + by, SKIN)
        px(img, 12 + bx, 8 + by, HILT)
        rect(img, 13 + bx, 7 + by, 15, 8 + by, BLADE)
        px(img, 15, 7 + by, GLINT)
        px(img, 14 + bx, 5 + by, ARC)
        px(img, 15, 6 + by, ARC)
    elif sword == "low":  # follow-through, blade angled down-forward
        rect(img, 11 + bx, 9 + by, 12 + bx, 10 + by, SKIN)
        px(img, 12 + bx, 11 + by, HILT)
        for i in range(3):
            px(img, 13 + bx + i, 12 + by + i, BLADE)

    if tint == "flash":
        for y in range(S):
            for x in range(S):
                if img.getpixel((x, y))[3]:
                    img.putpixel((x, y), FLASH)
    elif tint == "hurt":
        for y in range(S):
            for x in range(S):
                r, g, b, a = img.getpixel((x, y))
                if a:
                    img.putpixel((x, y), ((r + HURT[0]) // 2, (g + HURT[1]) // 2, (b + HURT[2]) // 2, a))
    return img


def dummy(sway=0, tilt=0):
    """Training dummy: a post with a burlap head. tilt shears the top sideways."""
    img = blank()
    rect(img, 5, 14, 10, 15, BASE)
    rect(img, 7, 6, 8, 14, POST)
    rect(img, 8, 6, 8, 14, POST_D)
    # crossbar and head lean with the tilt
    rect(img, 4 + tilt, 8, 11 + tilt, 9, POST)
    px(img, 11 + tilt, 9, POST_D)
    hx = 5 + tilt + sway
    rect(img, hx, 2, hx + 5, 6, SACK)
    px(img, hx + 2, 4, STITCH)
    px(img, hx + 4, 4, STITCH)
    px(img, hx + 3, 5, STITCH)
    rect(img, hx + 1, 6, hx + 4, 6, STITCH)
    return img


def sheet(frames, cols, path):
    rows = (len(frames) + cols - 1) // cols
    img = Image.new("RGBA", (cols * S, rows * S), (0, 0, 0, 0))
    boxes = []
    for i, frame in enumerate(frames):
        x, y = (i % cols) * S, (i // cols) * S
        img.paste(frame, (x, y))
        boxes.append({"x": x, "y": y, "w": S, "h": S})
    img.save(path)
    return boxes


def export(name, frames, tags, out_dir):
    """frames: list of (label, image, duration_ms). tags: (name, from, to, repeat|None)."""
    boxes = sheet([f[1] for f in frames], 8, os.path.join(out_dir, f"{name}.png"))
    data = {
        "frames": [
            {"filename": f"{label} {i}", "frame": box, "duration": ms}
            for i, ((label, _, ms), box) in enumerate(zip(frames, boxes))
        ],
        "meta": {
            "image": f"{name}.png",
            "frameTags": [
                {"name": tag, "from": a, "to": b, "direction": "forward"}
                | ({"repeat": str(rep)} if rep else {})
                for (tag, a, b, rep) in tags
            ],
        },
    }
    with open(os.path.join(out_dir, f"{name}.json"), "w") as f:
        json.dump(data, f, indent=2)
        f.write("\n")


def main():
    out = os.path.join(os.path.dirname(__file__), "..", "games", "dojo", "assets")
    os.makedirs(out, exist_ok=True)

    hero_frames = (
        # idle: a slow breath — the body settles, the head bobs
        [("hero idle", hero(bob=b, arms="side"), 160) for b in (0, 0, 1, 1)]
        # run: six-frame stride cycle
        + [
            ("hero run", hero(bob=0, legs=(2, -2), arms="swing_f"), 90),
            ("hero run", hero(bob=1, legs=(1, -1), arms="swing_f"), 90),
            ("hero run", hero(bob=0, legs=(0, 0), arms="side"), 90),
            ("hero run", hero(bob=0, legs=(-2, 2), arms="swing_b"), 90),
            ("hero run", hero(bob=1, legs=(-1, 1), arms="swing_b"), 90),
            ("hero run", hero(bob=0, legs=(0, 0), arms="side"), 90),
        ]
        # attack: windup, sweep, full extension, follow-through
        + [
            ("hero attack", hero(lean=-1, sword="raised"), 120),
            ("hero attack", hero(lean=0, sword="mid"), 70),
            ("hero attack", hero(lean=1, sword="out"), 70),
            ("hero attack", hero(lean=0, sword="low"), 120),
        ]
        # hit: one white flash, one red recoil
        + [
            ("hero hit", hero(lean=-1, arms="up", tint="flash"), 70),
            ("hero hit", hero(lean=-1, arms="up", tint="hurt"), 160),
        ]
    )
    export(
        "hero",
        hero_frames,
        [
            ("idle", 0, 3, None),
            ("run", 4, 9, None),
            ("attack", 10, 13, 1),
            ("hit", 14, 15, 1),
        ],
        out,
    )

    dummy_frames = (
        [("dummy idle", dummy(), 400), ("dummy idle", dummy(sway=0, tilt=0), 400)]
        + [
            ("dummy hit", dummy(tilt=2), 60),
            ("dummy hit", dummy(tilt=-1), 90),
            ("dummy hit", dummy(tilt=0), 140),
        ]
    )
    export("dummy", dummy_frames, [("idle", 0, 1, None), ("hit", 2, 4, 1)], out)
    print(f"wrote hero + dummy sheets to {os.path.normpath(out)}")


if __name__ == "__main__":
    main()
