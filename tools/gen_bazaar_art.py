#!/usr/bin/env python3
"""Generate the bazaar game's art: two nine-slice UI frames (day/night skins), ware icons,
and the market-street backdrop. Deliberately simple pixel art — the UI book is about
panels and skins, not painting. Rerun after edits: python3 tools/gen_bazaar_art.py"""

import os
from PIL import Image

OUT = os.path.join(os.path.dirname(__file__), "..", "games", "bazaar", "assets")

# Day skin: sun-bleached wood and parchment.
DAY_EDGE = (122, 82, 46, 255)
DAY_EDGE_D = (86, 56, 32, 255)
DAY_EDGE_L = (158, 112, 66, 255)
DAY_FILL = (58, 42, 30, 255)  # shaded canvas — the HUD's white text needs a dark ground
DAY_STUD = (196, 154, 60, 255)

# Night skin: lacquer, indigo, and brass.
NIGHT_EDGE = (52, 58, 110, 255)
NIGHT_EDGE_D = (32, 36, 72, 255)
NIGHT_EDGE_L = (84, 92, 158, 255)
NIGHT_FILL = (26, 28, 52, 255)
NIGHT_STUD = (222, 178, 92, 255)

# Wares
FIG = (108, 142, 62, 255)
FIG_D = (74, 102, 44, 255)
SILK = (198, 96, 168, 255)
SILK_D = (150, 64, 126, 255)
SILK_L = (232, 152, 205, 255)
LAMP = (238, 186, 82, 255)
LAMP_D = (176, 128, 48, 255)
FLAME = (255, 240, 170, 255)

# Street & stall
DIRT = (168, 138, 100, 255)
DIRT_D = (140, 112, 80, 255)
WOOD = (138, 98, 66, 255)
WOOD_D = (94, 66, 44, 255)
AWNING_A = (196, 70, 58, 255)
AWNING_B = (232, 220, 196, 255)
CLOTH = (222, 222, 222, 255)
CLOTH_D = (180, 180, 186, 255)
SKIN_C = (238, 182, 134, 255)


def blank(w, h):
    return Image.new("RGBA", (w, h), (0, 0, 0, 0))


def rect(img, x0, y0, x1, y1, c):
    w, h = img.size
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            if 0 <= x < w and 0 <= y < h:
                img.putpixel((x, y), c)


def px(img, x, y, c):
    w, h = img.size
    if 0 <= x < w and 0 <= y < h:
        img.putpixel((x, y), c)


def frame(edge, edge_d, edge_l, fill, stud):
    """A 24x24 nine-slice frame with 8px margins: chunky border, studded corners, plain
    center. Corners survive 1:1; the 8px-wide edge strips and the center stretch."""
    img = blank(24, 24)
    rect(img, 0, 0, 23, 23, edge)
    rect(img, 2, 2, 21, 21, edge_l)
    rect(img, 3, 3, 20, 20, edge_d)
    rect(img, 4, 4, 19, 19, fill)
    # corner studs, one per 8px corner block
    for cx, cy in [(2, 2), (21, 2), (2, 21), (21, 21)]:
        px(img, cx, cy, stud)
        px(img, cx - 1 if cx > 12 else cx + 1, cy, edge_d)
        px(img, cx, cy - 1 if cy > 12 else cy + 1, edge_d)
    # outer corner rounding
    for cx, cy in [(0, 0), (23, 0), (0, 23), (23, 23)]:
        px(img, cx, cy, (0, 0, 0, 0))
    return img


def figs():
    img = blank(16, 16)
    for dx, dy in [(0, 0), (5, 1), (2, 4)]:  # three figs in a pile
        rect(img, 4 + dx, 6 + dy, 8 + dx, 10 + dy, FIG)
        rect(img, 4 + dx, 9 + dy, 8 + dx, 10 + dy, FIG_D)
        px(img, 6 + dx, 5 + dy, FIG_D)  # stem
    return img


def silk():
    img = blank(16, 16)
    # a bolt of cloth, rolled
    rect(img, 3, 4, 12, 11, SILK)
    rect(img, 3, 9, 12, 11, SILK_D)
    rect(img, 3, 4, 12, 5, SILK_L)
    rect(img, 2, 4, 2, 11, SILK_D)  # roll end
    px(img, 13, 6, SILK_L)  # loose corner
    px(img, 14, 8, SILK_L)
    return img


def lamps():
    img = blank(16, 16)
    rect(img, 6, 3, 9, 4, LAMP_D)  # cap
    rect(img, 5, 5, 10, 10, LAMP)  # body
    rect(img, 5, 10, 10, 11, LAMP_D)
    rect(img, 7, 6, 8, 8, FLAME)  # glow window
    rect(img, 6, 12, 9, 12, LAMP_D)  # foot
    px(img, 7, 2, LAMP_D)  # handle
    px(img, 8, 2, LAMP_D)
    return img


def customer():
    """A robed shopper, near-white so the game can tint them by the ware they want."""
    img = blank(16, 24)
    rect(img, 4, 8, 11, 21, CLOTH)  # robe
    rect(img, 4, 18, 11, 21, CLOTH_D)
    rect(img, 5, 2, 10, 7, SKIN_C)  # face
    rect(img, 4, 1, 11, 2, CLOTH)  # hood
    rect(img, 4, 2, 4, 7, CLOTH)
    rect(img, 11, 2, 11, 7, CLOTH)
    px(img, 9, 4, (30, 24, 20, 255))  # eye, facing the stall
    rect(img, 3, 10, 3, 15, CLOTH_D)  # arms
    rect(img, 12, 10, 12, 15, CLOTH_D)
    return img


def stall():
    """The player's market stall: striped awning over a wooden counter."""
    img = blank(120, 88)
    # counter
    rect(img, 8, 56, 111, 80, WOOD)
    rect(img, 8, 56, 111, 60, WOOD_D)
    rect(img, 8, 76, 111, 80, WOOD_D)
    # legs
    rect(img, 10, 80, 16, 87, WOOD_D)
    rect(img, 103, 80, 109, 87, WOOD_D)
    # posts
    rect(img, 4, 8, 8, 79, WOOD_D)
    rect(img, 111, 8, 115, 79, WOOD_D)
    # awning, striped
    for x in range(0, 120, 12):
        color = AWNING_A if (x // 12) % 2 == 0 else AWNING_B
        rect(img, x, 0, min(x + 11, 119), 14, color)
    rect(img, 0, 14, 119, 16, WOOD_D)  # awning rail
    # scalloped hem
    for x in range(0, 120, 12):
        color = AWNING_A if (x // 12) % 2 == 0 else AWNING_B
        rect(img, x + 3, 17, min(x + 8, 119), 18, color)
    # wares on the counter
    rect(img, 20, 48, 34, 55, FIG)  # fig basket
    rect(img, 20, 52, 34, 55, WOOD_D)
    rect(img, 48, 46, 66, 55, SILK)  # silk bolts
    rect(img, 48, 51, 66, 55, SILK_D)
    rect(img, 82, 44, 92, 55, LAMP)  # a lamp
    rect(img, 85, 47, 89, 51, FLAME)
    return img


def ground():
    """The street: packed dirt with a few sweepings."""
    img = blank(320, 48)
    rect(img, 0, 0, 319, 47, DIRT)
    for i in range(90):  # deterministic scatter, no RNG needed
        x = (i * 53) % 320
        y = (i * 29) % 48
        px(img, x, y, DIRT_D)
        if i % 3 == 0:
            px(img, x + 1, y, DIRT_D)
    return img


def save(img, *path):
    target = os.path.join(OUT, *path)
    os.makedirs(os.path.dirname(target), exist_ok=True)
    img.save(target)
    print("wrote", os.path.relpath(target, os.path.join(OUT, "..", "..")))


def main():
    save(frame(DAY_EDGE, DAY_EDGE_D, DAY_EDGE_L, DAY_FILL, DAY_STUD), "ui", "day_frame.png")
    save(frame(NIGHT_EDGE, NIGHT_EDGE_D, NIGHT_EDGE_L, NIGHT_FILL, NIGHT_STUD), "ui", "night_frame.png")
    save(figs(), "ui", "figs.png")
    save(silk(), "ui", "silk.png")
    save(lamps(), "ui", "lamps.png")
    save(customer(), "customer.png")
    save(stall(), "stall.png")
    save(ground(), "ground.png")


if __name__ == "__main__":
    main()
