#!/usr/bin/env python3
"""Draw the docs screenshots' annotations, red callout boxes with arrows and highlight rings, onto a GPUI capture.

The docs pages explain the UI with annotated screenshots (`website/EDITORIAL_RULES.md`, "Rules from feedback"). Before
the GPUI migration the browser UI drew these annotations as an overlay inside the page (`e2e/readme-video/overlay.ts`
with the docs theme from `e2e/docs-shots/stage-docs.ts`, both deleted with the browser UI). A native window has no page
to draw into, so the docs engine (`scripts/docs_shots.py`) now draws them after capture instead, onto the picture,
from the bridge's boxes of the elements they point at. This module keeps the old overlay's look and placement rules so
that a shot ported from the old spec places its callouts where the old one did, given the same `side`, `dx` and `dy`.

Everything here works in the window's content coordinates (logical pixels, origin at the top left of the app's own
drawing, after the Linux frame and title bar), and scales to image pixels only when it writes the SVG. Rendering goes
through `rsvg-convert` (librsvg), which draws the capture underneath at exactly its own pixel size, so the pixels no
annotation covers stay the captured ones. The callout text uses the JetBrains Mono the app bundles, through a private
fontconfig file, so the look does not depend on the fonts the capturing machine has installed; being monospaced, it
lets lines be wrapped here by character count with the same result the browser's wrapping gave.
"""

from dataclasses import dataclass
import html
import math
import os
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
FONT_DIR = HERE.parent / "crates/gui/farhelm-gui/assets/fonts"
FONT_FAMILY = "JetBrainsMono Nerd Font"


# --- The look ---------------------------------------------------------------------------------------------------------
#
# The old docs theme's numbers (`DOCS_THEME`), in logical pixels: sized for images cropped and shown at half their pixel
# size inside a column of text.

ACCENT = "#ef4444"
GLOW = "rgba(239, 68, 68, 0.55)"
TEXT_PX = 15
#: The widest a callout box may be, border and padding included, before its text wraps (CSS `max-width`).
BOX_MAX = 270
STROKE = 3
HEAD = 12
#: The distance between a callout box and the edge of its target.
GAP = 40
RING = 3
PAD_Y = TEXT_PX * 0.7
PAD_X = TEXT_PX * 0.9
BORDER = 2
RADIUS = 10
LINE = TEXT_PX * 1.4
#: JetBrains Mono's advance width is 600 units of its 1000-unit em, for every character a callout uses.
ADVANCE = TEXT_PX * 0.6
#: How close to the window's edge a box may sit.
MARGIN = 16


@dataclass(frozen=True)
class Callout:
    """A text box beside `target`, with an arrow from the box to it.

    `side` picks where the box sits (`auto` takes the side with the most room); `dx` and `dy` then shift it, which is
    how a shot moves a box off something the reader should still see while the arrow keeps pointing at the target.
    """

    target: str
    text: str
    side: str = "auto"
    dx: float = 0
    dy: float = 0


@dataclass(frozen=True)
class Highlight:
    """A red ring around `target`, `pad` logical pixels outside it, for a control the reader should find at a glance."""

    target: str
    pad: float = 3


SIDES = ("auto", "left", "right", "top", "bottom")


def check(annotation) -> str | None:
    """Why a declared annotation cannot be drawn, or None; the engine refuses a page with one before the app starts."""
    if isinstance(annotation, Callout):
        if annotation.side not in SIDES:
            return f"callout side must be one of {', '.join(SIDES)}"
        if not isinstance(annotation.text, str) or not annotation.text.strip():
            return "callout text must be non-empty"
    elif not isinstance(annotation, Highlight):
        return "annotations must be Callout or Highlight values"
    target = annotation.target
    if not isinstance(target, str) or not target.startswith("id:") or len(target) == 3:
        return "annotation targets must be id: targets"
    return None


# --- Layout -----------------------------------------------------------------------------------------------------------


@dataclass(frozen=True)
class Rect:
    """A rectangle in content coordinates."""

    x: float
    y: float
    w: float
    h: float

    @property
    def right(self) -> float:
        return self.x + self.w

    @property
    def bottom(self) -> float:
        return self.y + self.h


def wrap(text: str, width_chars: int) -> list[str]:
    """Break `text` at spaces into lines of at most `width_chars`, as the browser wrapped the old callouts.

    A word longer than a line is cut at the line's width rather than left to overflow the box.
    """
    lines: list[str] = []
    line = ""
    for word in text.split():
        while len(word) > width_chars:
            if line:
                lines.append(line)
                line = ""
            lines.append(word[:width_chars])
            word = word[width_chars:]
        if not line:
            line = word
        elif len(line) + 1 + len(word) <= width_chars:
            line += " " + word
        else:
            lines.append(line)
            line = word
    if line:
        lines.append(line)
    return lines


def box_size(text: str) -> tuple[float, float, list[str]]:
    """The callout box's width and height and its text's lines: shrink-to-fit up to `BOX_MAX`, like the old CSS box."""
    chrome = 2 * (PAD_X + BORDER)
    lines = wrap(text, int((BOX_MAX - chrome) // ADVANCE))
    # CSS shrink-to-fit: text that has to wrap makes the box take its whole max-width, even when no line fills it.
    width = BOX_MAX if len(lines) > 1 else min(BOX_MAX, len(lines[0]) * ADVANCE + chrome)
    height = len(lines) * LINE + 2 * (PAD_Y + BORDER)
    return width, height, lines


def place(target: Rect, bw: float, bh: float, wanted: str, dx: float, dy: float, view_w: float, view_h: float):
    """Place a `bw` x `bh` box beside `target` on `wanted`'s side, shifted and kept inside the view.

    Returns the box's origin and the arrow's two ends: from the middle of the box edge facing the target to the nearest
    point on the target's facing edge, a few pixels outside it. A port of the old overlay's `place`.
    """
    room = {
        "right": view_w - target.right - bw,
        "left": target.x - bw,
        "bottom": view_h - target.bottom - bh,
        "top": target.y - bh,
    }
    side = max(room, key=lambda key: room[key]) if wanted == "auto" else wanted

    def clamp(value, low, high):
        return min(max(value, low), max(low, high))

    if side in ("right", "left"):
        x = target.right + GAP if side == "right" else target.x - GAP - bw
        y = target.y + target.h / 2 - bh / 2
    else:
        x = target.x + target.w / 2 - bw / 2
        y = target.bottom + GAP if side == "bottom" else target.y - GAP - bh
    x = clamp(x + dx, MARGIN, view_w - bw - MARGIN)
    y = clamp(y + dy, MARGIN, view_h - bh - MARGIN)
    inset = 6
    if side == "right":
        start = (x, y + bh / 2)
        end = (target.right + inset, clamp(start[1], target.y + 4, target.bottom - 4))
    elif side == "left":
        start = (x + bw, y + bh / 2)
        end = (target.x - inset, clamp(start[1], target.y + 4, target.bottom - 4))
    elif side == "bottom":
        start = (x + bw / 2, y)
        end = (clamp(start[0], target.x + 4, target.right - 4), target.bottom + inset)
    else:
        start = (x + bw / 2, y + bh)
        end = (clamp(start[0], target.x + 4, target.right - 4), target.y - inset)
    return x, y, start, end


def arrow(start, end):
    """The arrow's curve and open head: a gentle quadratic bend, the head along its final tangent (the old `arrowPath`).

    Returns the control point and the head's two outer points, so callers can both draw the arrow and bound it.
    """
    (x1, y1), (x2, y2) = start, end
    dx, dy = x2 - x1, y2 - y1
    length = math.hypot(dx, dy) or 1
    cx = (x1 + x2) / 2 - (dy / length) * length * 0.18
    cy = (y1 + y2) / 2 + (dx / length) * length * 0.18
    tx, ty = x2 - cx, y2 - cy
    tl = math.hypot(tx, ty) or 1
    ux, uy = tx / tl, ty / tl
    head1 = (x2 - ux * HEAD - uy * HEAD * 0.6, y2 - uy * HEAD + ux * HEAD * 0.6)
    head2 = (x2 - ux * HEAD + uy * HEAD * 0.6, y2 - uy * HEAD - ux * HEAD * 0.6)
    return (cx, cy), head1, head2


@dataclass(frozen=True)
class Drawn:
    """One laid-out annotation: what to draw and the rectangle the crop must keep in the picture."""

    kind: str
    bounds: Rect
    lines: tuple[str, ...] = ()
    start: tuple[float, float] = (0, 0)
    end: tuple[float, float] = (0, 0)


#: The least space kept between two callout boxes when one has to make way for another.
SPACING = 12


def overlaps(a: Rect, b: Rect) -> bool:
    """Whether `a` and `b` come closer than `SPACING`."""
    return (a.x < b.right + SPACING and b.x < a.right + SPACING and a.y < b.bottom + SPACING
            and b.y < a.bottom + SPACING)


def layout(annotations, targets: dict[str, Rect], view_w: float, view_h: float) -> list[Drawn]:
    """Lay out every annotation against its target's box, in declaration order, within a `view_w` x `view_h` view.

    A callout box that would cover an earlier one moves down below it, or up above it when there is no room below, so a
    shot's offsets stay hints rather than exact positions: the desktop app's row order and sizes differ from the old
    browser UI's, and an offset tuned for one layout would otherwise stack two boxes on top of each other. The arrow
    follows the box, since it starts at the box's edge facing the target.
    """
    drawn: list[Drawn] = []
    for annotation in annotations:
        target = targets[annotation.target]
        if isinstance(annotation, Highlight):
            pad = annotation.pad
            drawn.append(Drawn("ring", Rect(target.x - pad, target.y - pad, target.w + 2 * pad, target.h + 2 * pad)))
            continue
        bw, bh, lines = box_size(annotation.text)
        dy = annotation.dy
        x, y, start, end = place(target, bw, bh, annotation.side, annotation.dx, dy, view_w, view_h)
        boxes = [item.bounds for item in drawn if item.kind == "callout"]
        for direction in (1, -1):
            shift = dy
            while (clash := next((b for b in boxes if overlaps(Rect(x, y, bw, bh), b)), None)) is not None:
                shift += (clash.bottom + SPACING - y) if direction > 0 else (clash.y - SPACING - bh - y)
                moved = place(target, bw, bh, annotation.side, annotation.dx, shift, view_w, view_h)
                if moved[1] == y:
                    break  # clamped at the view's edge: try the other way
                x, y, start, end = moved
            if not any(overlaps(Rect(x, y, bw, bh), b) for b in boxes):
                break
            x, y, start, end = place(target, bw, bh, annotation.side, annotation.dx, dy, view_w, view_h)
        drawn.append(Drawn("callout", Rect(x, y, bw, bh), tuple(lines), start, end))
    return drawn


def extent(drawn: Drawn) -> Rect:
    """What the crop must keep for one annotation: the box or ring, and for a callout the whole arrow with its head.

    The arrow bends outside the line between its ends, so its bound includes the curve's control point: a quadratic
    curve stays inside the triangle of its ends and control point.
    """
    if drawn.kind == "ring":
        grow = RING + 4
        r = drawn.bounds
        return Rect(r.x - grow, r.y - grow, r.w + 2 * grow, r.h + 2 * grow)
    control, head1, head2 = arrow(drawn.start, drawn.end)
    xs = [drawn.bounds.x, drawn.bounds.right, drawn.start[0], drawn.end[0], control[0], head1[0], head2[0]]
    ys = [drawn.bounds.y, drawn.bounds.bottom, drawn.start[1], drawn.end[1], control[1], head1[1], head2[1]]
    return Rect(min(xs) - STROKE, min(ys) - STROKE, max(xs) - min(xs) + 2 * STROKE, max(ys) - min(ys) + 2 * STROKE)


# --- Rendering --------------------------------------------------------------------------------------------------------


def svg(image_name: str, width_px: int, height_px: int, scale: float, drawn: list[Drawn]) -> str:
    """An SVG of the capture (`image_name`, beside the SVG) under every annotation, in image pixels.

    The capture shows exactly the content area, so content coordinates times `scale` are image pixels. The image is
    placed at its own pixel size with smoothing off, so rsvg copies its pixels rather than resampling them. Arrows are
    drawn before boxes, as the old overlay's arrow layer sat under its callouts.
    """
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{width_px}" '
        f'height="{height_px}" viewBox="0 0 {width_px} {height_px}">',
        '<defs><filter id="glow" x="-50%" y="-50%" width="200%" height="200%">'
        f'<feGaussianBlur stdDeviation="{11 * scale / 2}"/></filter>'
        '<filter id="shadow" x="-20%" y="-20%" width="140%" height="160%">'
        f'<feDropShadow dx="0" dy="{10 * scale}" stdDeviation="{15 * scale / 2}" flood-color="#000" '
        'flood-opacity="0.55"/></filter></defs>',
        f'<image x="0" y="0" width="{width_px}" height="{height_px}" xlink:href="{html.escape(image_name)}" '
        'image-rendering="optimizeSpeed" preserveAspectRatio="none"/>',
    ]

    def s(value: float) -> str:
        return f"{value * scale:.2f}"

    for item in drawn:
        if item.kind != "ring":
            continue
        r = item.bounds
        geometry = (f'x="{s(r.x + RING / 2)}" y="{s(r.y + RING / 2)}" width="{s(r.w - RING)}" '
                    f'height="{s(r.h - RING)}" rx="{s(12)}"')
        parts.append(f'<rect {geometry} fill="none" stroke="{GLOW}" stroke-width="{s(RING * 3)}" filter="url(#glow)"/>')
        parts.append(f'<rect {geometry} fill="none" stroke="{ACCENT}" stroke-width="{s(RING)}"/>')
    for item in drawn:
        if item.kind != "callout":
            continue
        control, head1, head2 = arrow(item.start, item.end)
        path = (f"M{s(item.start[0])},{s(item.start[1])} Q{s(control[0])},{s(control[1])} {s(item.end[0])},"
                f"{s(item.end[1])} M{s(head1[0])},{s(head1[1])} L{s(item.end[0])},{s(item.end[1])} "
                f"L{s(head2[0])},{s(head2[1])}")
        parts.append(f'<path d="{path}" fill="none" stroke="{ACCENT}" stroke-width="{s(STROKE)}" '
                     'stroke-linecap="round" stroke-linejoin="round"/>')
    for item in drawn:
        if item.kind != "callout":
            continue
        r = item.bounds
        # A CSS border sits inside the box; an SVG stroke straddles its path, so the path runs half a border inside.
        parts.append(f'<rect x="{s(r.x + BORDER / 2)}" y="{s(r.y + BORDER / 2)}" width="{s(r.w - BORDER)}" '
                     f'height="{s(r.h - BORDER)}" rx="{s(RADIUS)}" fill="rgb(18,12,20)" fill-opacity="0.94" '
                     f'stroke="{ACCENT}" stroke-width="{s(BORDER)}" filter="url(#shadow)"/>')
        # The first baseline sits where CSS puts it in a 1.4 line box: half the leading, then the font's ascent
        # (about 1.02 em for JetBrains Mono), below the content box's top.
        top = r.y + BORDER + PAD_Y + (LINE - TEXT_PX * 1.32) / 2 + TEXT_PX * 1.02
        spans = "".join(
            f'<tspan x="{s(r.x + BORDER + PAD_X)}" y="{s(top + index * LINE)}">{html.escape(line)}</tspan>'
            for index, line in enumerate(item.lines)
        )
        parts.append(f'<text font-family="{FONT_FAMILY}" font-size="{s(TEXT_PX)}" fill="#fff" '
                     f'xml:space="preserve">{spans}</text>')
    parts.append("</svg>")
    return "\n".join(parts)


def fontconfig(directory: Path) -> Path:
    """A fontconfig file that offers rsvg the app's bundled fonts ahead of the machine's own, written in `directory`."""
    path = directory / "fonts.conf"
    path.write_text(
        '<?xml version="1.0"?>\n<!DOCTYPE fontconfig SYSTEM "fonts.dtd">\n<fontconfig>\n'
        f"  <dir>{html.escape(str(FONT_DIR))}</dir>\n  <cachedir>{html.escape(str(directory / 'fc-cache'))}</cachedir>\n"
        "  <include ignore_missing=\"yes\">/etc/fonts/fonts.conf</include>\n</fontconfig>\n",
        encoding="utf-8",
    )
    return path


def render(image: Path, width_px: int, height_px: int, scale: float, drawn: list[Drawn], out: Path) -> None:
    """Draw `drawn` over the PNG `image` (`width_px` x `height_px`) and write the result to `out`.

    The SVG and the fontconfig file go beside `image`, which is where rsvg resolves the image's relative reference from.
    A failed render raises with rsvg's own message, so the shot fails rather than going out unannotated.
    """
    source = image.with_suffix(".svg")
    source.write_text(svg(image.name, width_px, height_px, scale, drawn), encoding="utf-8")
    environment = dict(os.environ, FONTCONFIG_FILE=str(fontconfig(image.parent)))
    result = subprocess.run(
        ["rsvg-convert", "--format=png", "--output", str(out), str(source)],
        cwd=image.parent, env=environment, capture_output=True, text=True,
    )
    if result.returncode:
        raise RuntimeError(f"rsvg-convert failed: {result.stderr.strip() or result.returncode}")
