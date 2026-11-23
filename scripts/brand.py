# /// script
# requires-python = ">=3.11"
# dependencies = ["fonttools==4.68.0", "skia-pathops==0.9.2", "resvg-py==0.5.0"]
# ///
"""Draws the logo files in assets/brand/ from code; see docs/brand-guidelines.md.

Every SVG is pure paths, with no fonts or masks, so all renderers agree.
"""
import json
import pathlib
import urllib.parse
import urllib.request

import pathops
import resvg_py
from fontTools.pens.recordingPen import DecomposingRecordingPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "brand"
FONTS = ROOT / "target" / "brand-fonts"
GOOGLE_FONTS = "https://github.com/google/fonts/raw/06bfe0d62a98375cb09c942fa18e68bd32cff057/ofl"

COLOUR = {name: token["$value"]
          for name, token in json.loads((OUT / "tokens.json").read_text())["primitive"]["color"].items()}
RED, RED_NIGHT = COLOUR["postbox-red"], COLOUR["postbox-red-night"]
INK, TEXT_NIGHT = COLOUR["ink"], COLOUR["chalk"]
PAPER, MUTED, BLUE = COLOUR["paper"], COLOUR["ink-soft"], COLOUR["airmail-blue"]
KAPPA = 0.5522847498


def font(path, weight):
    local = FONTS / path.split("/")[-1]
    if not local.exists():
        FONTS.mkdir(parents=True, exist_ok=True)
        urllib.request.urlretrieve(f"{GOOGLE_FONTS}/{urllib.parse.quote(path)}", local)
    face = TTFont(local)
    instantiateVariableFont(face, {"wght": weight}, inplace=True)
    return face


def circle(p, cx, cy, r):
    k = KAPPA * r
    p.moveTo(cx + r, cy)
    p.cubicTo(cx + r, cy + k, cx + k, cy + r, cx, cy + r)
    p.cubicTo(cx - k, cy + r, cx - r, cy + k, cx - r, cy)
    p.cubicTo(cx - r, cy - k, cx - k, cy - r, cx, cy - r)
    p.cubicTo(cx + k, cy - r, cx + r, cy - k, cx + r, cy)
    p.close()


def rounded(x, y, w, h, r):
    p = pathops.Path()
    k = KAPPA * r
    p.moveTo(x + r, y)
    p.lineTo(x + w - r, y)
    p.cubicTo(x + w - r + k, y, x + w, y + r - k, x + w, y + r)
    p.lineTo(x + w, y + h - r)
    p.cubicTo(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h)
    p.lineTo(x + r, y + h)
    p.cubicTo(x + r - k, y + h, x, y + h - r + k, x, y + h - r)
    p.lineTo(x, y + r)
    p.cubicTo(x, y + r - k, x + r - k, y, x + r, y)
    p.close()
    return p


def stroked(points, width):
    p = pathops.Path()
    p.moveTo(*points[0])
    for point in points[1:]:
        p.lineTo(*point)
    p.stroke(width, pathops.LineCap.ROUND_CAP, pathops.LineJoin.ROUND_JOIN, 4)
    # Boolean ops reject the conics a round stroke leaves.
    p.convertConicsToQuads()
    return p


def minus(a, b):
    return pathops.op(a, b, pathops.PathOp.DIFFERENCE)


def stamp(small):
    """The stamp on a 48-unit grid; the small cut keeps one notch per edge."""
    holes = pathops.Path()
    if small:
        body = rounded(5, 5, 38, 38, 2)
        for cx, cy in ((5, 24), (43, 24), (24, 5), (24, 43)):
            circle(holes, cx, cy, 4)
        chevron = stroked([(14, 15), (23.5, 24), (14, 33)], 5.5)
        cursor = rounded(26, 29, 10, 5, 1)
    else:
        body = rounded(7, 7, 34, 34, 0.0001)
        for c in (12, 20, 28, 36):
            for cx, cy in ((7, c), (41, c), (c, 7), (c, 41)):
                circle(holes, cx, cy, 2.2)
        chevron = stroked([(15.5, 17.5), (22.5, 24), (15.5, 30.5)], 3.6)
        cursor = rounded(25, 28.5, 8.5, 3.4, 0.8)
    return minus(minus(minus(body, holes), chevron), cursor)


def path_data(path, transform=(1, 0, 0, 1, 0, 0)):
    pen = SVGPathPen(None, ntos=lambda v: f"{v:.3f}".rstrip("0").rstrip("."))
    path.draw(TransformPen(pen, transform))
    return pen.getCommands()


def outline(face, text, size, tracking=0.0):
    """`text` as one path, baseline at y=0, y down; tracking in em."""
    glyphs = face.getGlyphSet()
    cmap = face.getBestCmap()
    scale = size / face["head"].unitsPerEm
    out = pathops.Path()
    x = 0.0
    for ch in text:
        glyph = glyphs[cmap[ord(ch)]]
        shape = pathops.Path()
        pen = DecomposingRecordingPen(glyphs)
        glyph.draw(pen)
        pen.replay(TransformPen(shape.getPen(), (scale, 0, 0, -scale, x, 0)))
        out = pathops.op(out, shape, pathops.PathOp.UNION)
        x += glyph.width * scale + tracking * size
    return out


def svg(view, body, size=None):
    x, y, w, h = view
    w_px, h_px = size or (round(w, 2), round(h, 2))
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x:g} {y:g} {w:.2f} {h:.2f}" '
            f'width="{w_px}" height="{h_px}">{body}</svg>\n')


def write(name, text):
    (OUT / name).write_text(text)


def png(name, text, width, height):
    (OUT / name).write_bytes(bytes(resvg_py.svg_to_bytes(svg_string=text, width=width, height=height)))


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    full, small = path_data(stamp(False)), path_data(stamp(True))
    full_view, small_view = (4.8, 4.8, 38.4, 38.4), (1, 1, 46, 46)
    colours = {"": RED, "-night": RED_NIGHT, "-ink": INK, "-paper": PAPER}

    for suffix, colour in colours.items():
        write(f"postal-mark{suffix}.svg", svg(full_view, f'<path fill="{colour}" d="{full}"/>'))
        write(f"postal-mark-small{suffix}.svg", svg(small_view, f'<path fill="{colour}" d="{small}"/>'))

    write("favicon.svg", svg(small_view,
        f"<style>path{{fill:{RED}}}@media (prefers-color-scheme:dark){{path{{fill:{RED_NIGHT}}}}}</style>"
        f'<path d="{small}"/>'))

    tile = path_data(rounded(0, 0, 48, 48, 10.75))
    app = svg((0, 0, 48, 48), f'<path fill="{INK}" d="{tile}"/>'
              f'<path fill="{RED_NIGHT}" transform="translate(4.8 4.8) scale(0.8)" d="{full}"/>')
    write("postal-app-icon.svg", app)

    # The wordmark's baseline sits on the cursor's bottom edge, 8 units right of the stamp.
    word = outline(font("schibstedgrotesk/SchibstedGrotesk[wght].ttf", 800), "postal", 31, -0.03)
    wx0, wy0, wx1, wy1 = word.bounds
    dx, dy = 49 - wx0, 31.9
    word_d = path_data(word, (1, 0, 0, 1, dx, dy))
    lock_view = (7, min(7, wy0 + dy), 42 + wx1 - wx0, max(41, wy1 + dy) - min(7, wy0 + dy))
    lock_colours = {"": (RED, INK), "-night": (RED_NIGHT, TEXT_NIGHT), "-ink": (INK, INK), "-paper": (PAPER, PAPER)}
    for suffix, (mark, text) in lock_colours.items():
        write(f"postal-logo{suffix}.svg",
              svg(lock_view, f'<path fill="{mark}" d="{full}"/><path fill="{text}" d="{word_d}"/>'))
    write("postal-wordmark.svg",
          svg((wx0, wy0, wx1 - wx0, wy1 - wy0), f'<path fill="{INK}" d="{path_data(word)}"/>'))

    tagline = outline(font("jetbrainsmono/JetBrainsMono[wght].ttf", 500), "Mail at terminal speed.", 34)
    tx0, _, tx1, _ = tagline.bounds
    lx, ly, lw, lh = lock_view
    scale = 820 / lw
    stripes = "".join(f'<path fill="{RED if i % 2 == 0 else BLUE}" d="M{i * 60 - 40} 0h36l-24 24h-36z"/>'
                      for i in range(24))
    social = svg((0, 0, 1280, 640),
        f'<rect width="1280" height="640" fill="{PAPER}"/><g>{stripes}</g>'
        f'<g transform="translate(0 616)">{stripes}</g>'
        f'<g transform="translate({(1280 - lw * scale) / 2 - lx * scale:.2f} {180 - ly * scale:.2f}) scale({scale:.4f})">'
        f'<path fill="{RED}" d="{full}"/><path fill="{INK}" d="{word_d}"/></g>'
        f'<path fill="{MUTED}" transform="translate({(1280 - (tx1 - tx0)) / 2 - tx0:.2f} {270 + lh * scale:.2f})" '
        f'd="{path_data(tagline)}"/>')
    write("social-preview.svg", social)

    write("postal-mark.txt", "┌┄┄┄┄┄┐\n┆ ›_  ┆\n└┄┄┄┄┄┘\n")
    write("postal-lockup.txt",
          "┌┄┄┄┄┄┐                  █        █\n"
          "┆ ›_  ┆  █▀▀▄ ▄▀▀▄ ▄█▀▀ ▀█▀▀ ▀▀█▄ █\n"
          "└┄┄┄┄┄┘  █▀▀  ▀▄▄▀ ▄▄█▀  ▀▄▄ ▀▄▄█ ▀▄▄")

    small_red = svg(small_view, f'<path fill="{RED}" d="{small}"/>')
    png("favicon-16.png", small_red, 16, 16)
    png("favicon-32.png", small_red, 32, 32)
    png("apple-touch-icon.png", app, 180, 180)
    png("postal-app-icon-512.png", app, 512, 512)
    png("social-preview.png", social, 1280, 640)
    png("postal-logo-1024.png", svg(lock_view, f'<path fill="{RED}" d="{full}"/><path fill="{INK}" d="{word_d}"/>'),
        1024, round(1024 * lock_view[3] / lock_view[2]))


if __name__ == "__main__":
    main()
