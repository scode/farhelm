#!/usr/bin/env python3
"""Turns the wordmark's letters into outlines so no renderer ever picks a font for it.

Writes `wordmark-outline.json` next to this script: one SVG path per letter of
"farhelm" in JetBrains Mono Nerd Font Bold, the font the app itself ships, plus
the metrics the renderer needs to place them. `render-svgs.mjs` inlines those
paths wherever the wordmark appears. Rerun this only when the word or the font
changes; the JSON is checked in so the renderer needs no font tooling.

Why outlines and not an embedded font: GitHub shows a README's SVG through an
image tag, which cannot load an external font, so the letters would fall back
to whatever monospace the reader's platform has. Embedding the font as a data
URI would work but costs a font copy in every file that uses the wordmark.
Paths are the normal shape for a logo anyway: the icon is already geometry,
and this makes the wordmark the same kind of thing.

Needs `fonttools[woff]` (the woff2 decoder pulls in brotli); nothing in the
ordinary build does, which is why this is a separate step from the renderer.
Run from anywhere: paths are resolved relative to this file.

The output space is font units (upem, 1000 for this font) with y pointing
down, which is what SVG wants, so the renderer can place the whole word with
a single translate+scale and does not have to know about the font's ascender
or the y flip.
"""

import json
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

HERE = Path(__file__).resolve().parent
FONT = HERE / ".." / ".." / "crates" / "farhelm-ui" / "assets" / "fonts" / "JetBrainsMonoNerdFont-Bold.woff2"
OUT = HERE / "wordmark-outline.json"
WORD = "farhelm"


def main() -> None:
    font = TTFont(FONT)
    upem = font["head"].unitsPerEm
    cmap = font.getBestCmap()
    glyphs = font.getGlyphSet()
    hmtx = font["hmtx"]
    os2 = font["OS/2"]

    letters = []
    x = 0
    for ch in WORD:
        name = cmap[ord(ch)]
        pen = SVGPathPen(glyphs)
        # Flip y (font units point up, SVG points down) and advance x per
        # letter, so each path is already in its final place within the word.
        glyphs[name].draw(TransformPen(pen, (1, 0, 0, -1, x, 0)))
        letters.append({"char": ch, "d": pen.getCommands()})
        x += hmtx[name][0]

    OUT.write_text(
        json.dumps(
            {
                "font": FONT.name,
                "word": WORD,
                "unitsPerEm": upem,
                # Cap height and x-height let the renderer size the block
                # cursor to the letters rather than to a guessed number.
                "capHeight": os2.sCapHeight,
                "xHeight": os2.sxHeight,
                "advance": x,
                "letters": letters,
            },
            indent=2,
        )
        + "\n"
    )
    print(f"wrote {OUT.relative_to(HERE.parent.parent)}: {len(letters)} glyphs, advance {x}/{upem}")


if __name__ == "__main__":
    main()
