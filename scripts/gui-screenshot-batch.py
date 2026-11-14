#!/usr/bin/env python3
"""Capture the GPUI app's scenes from the current tree and lay them out next to the old app's reference screenshots.

The GPUI port copies the old app's look (SPEC_impl.md, "GUI architecture": port, not redesign), so every area checks its
screens side by side with the old app's pictures, which live outside the repository (the migration's reference set, a
directory holding `scenes.json` and `old-app/`). This script makes that comparison one browsable page instead of a
folder of files to open by hand. Its output never enters the repository: captured screenshots are not committed on
main (`.agents/docs-media.md`).

    scripts/gui-screenshot-batch.py capture OUT_DIR [--no-build] [--lock FILE]
    scripts/gui-screenshot-batch.py page OUT_DIR --reference REF_DIR [--touched ID,...] [--copy-references]
    scripts/gui-screenshot-batch.py all OUT_DIR --reference REF_DIR [--touched ID,...] [--copy-references] [...]

`capture` needs the GPUI app's build environment and what `scripts/gui-agent.py` needs to run it (on Linux a headless
Mutter and lavapipe; on a Mac the logged-in GUI session, where the app opens a real window). It writes PNGs and
`captures.json` under OUT_DIR, which must not hold an earlier capture. `page` needs only Python: it reads OUT_DIR's
captures, the committed scene list (`docs/gpui/screenshot-scenes.json`) and the reference set, and writes
OUT_DIR/index.html. Splitting them lets the capture run on a build machine and the page be made wherever the reference
pictures are. `all` runs both.

The scene list names every reference scene (by its id in the reference `scenes.json`) and whether the GPUI app shows it
yet, plus two kinds of new-app capture: app scenes, which drive the running app with bridge commands and screenshot the
window, and the foundation goldens (`crates/gui/farhelm-gui/tests/goldens/`), shown beside the reference scene they
relate to. The goldens are copies, so every platform's are shown wherever the batch runs. App scenes are captured once
per app variant the list names (a theme and a window size): the bridge has no theme or resize command, so each variant
is its own app run, started with `gui-agent.py start --appearance --window-size`. The list's `app_start` names the
start options (the fleet to seed) its scenes share; a scene that needs another fleet (none at all, say) names its own
`start`, and the scenes with the same options share an app run, in list order. On Linux each capture is cropped to
the app's own drawing (`LINUX_FRAME` says why). A reference scene not marked ported is listed on the page as "not yet
ported"; a capture that this batch did not make says "not captured in this batch" instead, so a missing picture is
never read as missing work. An entry marked as a mockup is labelled one and never counts as evidence that something is
built. `--touched` lists scenes changed since the last batch first, so a reviewer sees what moved.

Each app run's directory (its state, logs and the helm credential `gui-agent.py` writes for seeding) lives in a fresh
directory under /tmp, never under OUT_DIR, so the output can be handed on as is. A run that stopped cleanly is deleted;
one that failed is kept for its logs, and the script names it. The script exits non-zero when any capture failed or
any app run did not start or did not stop cleanly.
"""

from __future__ import annotations

import argparse
import html
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SCENE_LIST = REPO / "docs/gpui/screenshot-scenes.json"
GOLDENS = REPO / "crates/gui/farhelm-gui/tests/goldens"
GUI_AGENT = REPO / "scripts/gui-agent.py"

THEMES = ("light", "dark")

# The `gui-agent.py start` options the batch sets itself for every app run (the variant's theme and size, the build
# and the shared lock). A scene's own `start` options may not repeat them: the variant would no longer be what the
# capture's label says. `owned_options` also catches the abbreviations argparse accepts for them (`--appear`).
BATCH_START_OPTIONS = {"--window-size", "--appearance", "--size", "--no-build", "--lock"}

# How long one bridge command (a scene step or the screenshot) may take. The bridge's own `wait` has a shorter default
# timeout, so this only catches a command that hangs outright, which would otherwise hang the whole batch. `start`
# (which may build) and `stop` (which must always finish its cleanup) get no timeout.
STEP_TIMEOUT_SECONDS = 300

# Mutter's virtual monitor must hold the window: the app fits its first window to the display, so a smaller monitor
# would shrink the capture below the size its label claims. The margin leaves room for the window's own decorations.
MONITOR_MARGIN = 200
MIN_MONITOR = (1600, 1200)

# The frame GPUI Kit draws around a Linux window, in pixels: a margin for its drop shadow (`SHADOW_SIZE` in Kit's
# window_border.rs, 20 on Linux and 0 elsewhere) and a 1-pixel border inside it, on every side. Under Mutter GPUI draws
# the window's decorations itself, so the frame is part of the window's surface, and the bridge's screenshot is the
# whole surface: a 900x700 window showed the app in 858x658 pixels, in a black band (the shadow, drawn over nothing).
# The old app's pictures show only its page, so a Linux run asks for a window larger by the frame and crops it off,
# which leaves the app's own drawing at exactly the size the variant names. On a Mac Kit's margin is zero (the system
# draws the frame), so nothing is cropped there; a Mac capture has not been checked for a band of its own.
LINUX_FRAME_MARGIN = 20
LINUX_FRAME_BORDER = 1
LINUX_FRAME = LINUX_FRAME_MARGIN + LINUX_FRAME_BORDER

# Where app runs' directories go. Not the platform's default temporary directory: on a Mac that is under
# /var/folders/, deep enough that a run's deepest socket path is estimated (not measured) to come within a byte or two
# of the 103-byte limit `gui-agent.py start` enforces there.
RUNS_PARENT = Path("/tmp")


# --- scene list -----------------------------------------------------------------------------------------------------


def tree_goldens(goldens: Path) -> set[str]:
    """The golden names committed under any platform's directory (`<name>--<theme>.png`)."""
    return {p.name.split("--")[0] for p in goldens.glob("*/*.png")}


def validate_scenes(scenes: dict, goldens: set[str]) -> list[str]:
    """Every problem in a scene list that would make a batch capture the wrong thing or a page misreport it.

    The page's sections are keyed by scene id, so ids must be unique across app scenes, goldens (`golden-<name>`) and
    reference scenes. A `reference` must name a listed reference scene, or the page would show "unknown reference
    scene" only once someone looks. Every committed golden must be listed and every listed golden committed, so the page
    shows the whole foundation set.
    """
    problems: list[str] = []
    for key in ("reference_commit", "app_variants", "app_scenes", "foundation_goldens", "reference_scenes"):
        if key not in scenes:
            problems.append(f"missing top-level key {key!r}")
    if problems:
        return problems
    reference_ids = [s.get("id") for s in scenes["reference_scenes"]]
    for scene in scenes["reference_scenes"]:
        if not isinstance(scene.get("id"), str) or not isinstance(scene.get("ported"), bool):
            problems.append(f"reference scene needs a string id and a boolean ported: {scene}")
    variants = set()
    for variant in scenes["app_variants"]:
        window = variant.get("window")
        if (
            variant.get("theme") not in THEMES
            or not isinstance(variant.get("size"), str)
            or not (isinstance(window, list) and len(window) == 2 and all(isinstance(n, int) and n > 0 for n in window))
        ):
            problems.append(f"app variant needs theme light|dark, a size name and window [w, h]: {variant}")
            continue
        if (variant["theme"], variant["size"]) in variants:
            problems.append(f"duplicate app variant {variant['theme']} {variant['size']}")
        variants.add((variant["theme"], variant["size"]))
    default_start = scenes.get("app_start", [])
    if not is_start(default_start):
        problems.append("app_start must be a list of strings")
    elif owned := owned_options(default_start):
        problems.append(f"app_start may not set {', '.join(owned)}, which the batch sets")
    for entry in scenes["app_scenes"]:
        if not {"id", "title", "steps"} <= set(entry):
            problems.append(f"app scene needs id, title and steps: {entry}")
            continue
        steps = entry["steps"]
        if not (isinstance(steps, list) and all(
            isinstance(step, list) and step and all(isinstance(word, str) for word in step) for step in steps
        )):
            problems.append(f"app scene {entry['id']}: steps must be a list of non-empty lists of strings")
        start = entry.get("start", [])
        if not is_start(start):
            problems.append(f"app scene {entry['id']}: start must be a list of strings")
        elif owned := owned_options(start):
            problems.append(f"app scene {entry['id']}: start may not set {', '.join(owned)}, which the batch sets")
    for entry in scenes["foundation_goldens"]:
        if not {"golden", "title"} <= set(entry):
            problems.append(f"foundation golden needs golden and title: {entry}")
    for entry in scenes["app_scenes"] + scenes["foundation_goldens"]:
        ref = entry.get("reference")
        if ref is not None and ref not in reference_ids:
            name = entry.get("id") or entry.get("golden")
            problems.append(f"{name}: reference {ref!r} is not a listed reference scene")
    listed_goldens = {e.get("golden") for e in scenes["foundation_goldens"]}
    for name in sorted(goldens - listed_goldens):
        problems.append(f"golden {name!r} is committed but not listed in foundation_goldens")
    for name in sorted(n for n in listed_goldens - goldens if n is not None):
        problems.append(f"golden {name!r} is listed but not committed")
    section_ids = (
        [e.get("id") for e in scenes["app_scenes"]]
        + [f"golden-{e.get('golden')}" for e in scenes["foundation_goldens"]]
        + reference_ids
    )
    for dup in sorted({i for i in section_ids if section_ids.count(i) > 1}, key=str):
        problems.append(f"duplicate scene id {dup!r}")
    return problems


def is_start(start) -> bool:
    """Whether `start` is a list of `gui-agent.py start` arguments (strings), as `app_start` and `start` must be."""
    return isinstance(start, list) and all(isinstance(word, str) for word in start)


def owned_options(start: list[str]) -> list[str]:
    """The BATCH_START_OPTIONS that `start` sets, spelled out in full. A word counts when it is one of them, with or
    without `=value`, or an abbreviation of one, since argparse takes any unambiguous prefix (`--appear dark`)."""
    names = {word.split("=", 1)[0] for word in start if word.startswith("--") and len(word) > 2}
    return sorted(option for option in BATCH_START_OPTIONS if any(option.startswith(name) for name in names))


def load_scenes(path: Path = SCENE_LIST, goldens: Path = GOLDENS) -> dict:
    """The committed scene list, refused with every problem listed when it is not well formed."""
    scenes = json.loads(path.read_text(encoding="utf-8"))
    problems = validate_scenes(scenes, tree_goldens(goldens))
    if problems:
        raise SystemExit("gui-screenshot-batch: the scene list is not well formed:\n  " + "\n  ".join(problems))
    return scenes


# --- pictures -------------------------------------------------------------------------------------------------------


PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def read_rgba(path: Path) -> tuple[int, int, bytearray]:
    """A PNG's width, height and pixels (four bytes each, row after row).

    Only 8-bit RGBA without interlacing, which is what the bridge's screenshot writes; anything else is a ValueError
    rather than a guess, because a misread picture would be cropped in the wrong place without a word.
    """
    data = path.read_bytes()
    if data[:8] != PNG_SIGNATURE:
        raise ValueError(f"{path} is not a PNG")
    header, idat, pos = None, [], 8
    while pos + 8 <= len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            if len(body) != 13:
                raise ValueError(f"{path}: an IHDR chunk of {len(body)} bytes, not 13")
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            idat.append(body)
        elif kind == b"IEND":
            break
    if header is None or header[2:4] != (8, 6) or header[6] != 0:
        raise ValueError(f"{path} is not an 8-bit RGBA PNG without interlacing")
    width, height = header[0], header[1]
    raw = zlib.decompress(b"".join(idat))
    stride = width * 4
    # A header that claims more rows than the data holds would otherwise surface as an IndexError, which `crop_frame`
    # does not expect, and abort the whole batch before its results are written.
    if len(raw) != height * (stride + 1):
        raise ValueError(f"{path}: {len(raw)} bytes of pixel data where a {width}x{height} image needs "
                         f"{height * (stride + 1)}")
    pixels = bytearray(stride * height)
    prev = bytearray(stride)
    # Undo PNG's per-row filters (the PNG specification, section 9): each row starts with its filter type, and each
    # byte was stored as a difference from the byte to its left (Sub), above (Up), their mean (Average) or the Paeth
    # predictor of left, above and upper left.
    for y in range(height):
        start = y * (stride + 1)
        kind, line = raw[start], bytearray(raw[start + 1:start + 1 + stride])
        if kind == 1:
            for i in range(4, stride):
                line[i] = (line[i] + line[i - 4]) & 0xFF
        elif kind == 2:
            line = bytearray((a + b) & 0xFF for a, b in zip(line, prev))
        elif kind == 3:
            for i in range(stride):
                left = line[i - 4] if i >= 4 else 0
                line[i] = (line[i] + ((left + prev[i]) >> 1)) & 0xFF
        elif kind == 4:
            for i in range(stride):
                a = line[i - 4] if i >= 4 else 0
                b, c = prev[i], prev[i - 4] if i >= 4 else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 0xFF
        elif kind != 0:
            raise ValueError(f"{path}: unknown PNG filter type {kind}")
        pixels[y * stride:(y + 1) * stride] = line
        prev = line
    return width, height, pixels


def write_rgba(path: Path, width: int, height: int, pixels: bytes) -> None:
    """Write 8-bit RGBA pixels as a PNG (unfiltered rows; the files are small because the screens are mostly flat)."""
    stride = width * 4
    raw = b"".join(b"\x00" + pixels[y * stride:(y + 1) * stride] for y in range(height))

    def chunk(kind: bytes, body: bytes) -> bytes:
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    path.write_bytes(PNG_SIGNATURE + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


def png_size(path: Path) -> tuple[int, int] | None:
    """A PNG's width and height from its IHDR chunk, or None for anything that is not a readable PNG."""
    try:
        head = path.read_bytes()[:24]
    except OSError:
        return None
    if len(head) < 24 or head[:8] != PNG_SIGNATURE:
        return None
    return struct.unpack(">II", head[16:24])


def crop_frame(path: Path, margin: int, border: int) -> str | None:
    """Crop Kit's Linux window frame (`margin` black pixels, then `border`) off every side of a capture, in place.

    Returns why it did not crop, or None. The frame is checked before anything is cut: along the middle row and column,
    exactly `margin` black pixels from each edge, then a pixel that is not black (the border). A capture that does not
    show that frame (a Kit release that changed its shadow, a compositor that draws decorations itself) is left as it
    was and reported, rather than cropped into the app's drawing or left with a band under a label that claims the
    asked size.
    """
    try:
        width, height, pixels = read_rgba(path)
    except (OSError, ValueError, zlib.error) as problem:
        return f"could not crop the window frame: {problem}"
    cut = margin + border
    if width <= 2 * cut or height <= 2 * cut:
        return f"the capture is {width}x{height}, too small to hold the window frame"

    def black(x: int, y: int) -> bool:
        at = (y * width + x) * 4
        return pixels[at:at + 3] == b"\x00\x00\x00"

    def edge_run(points) -> int:
        run = 0
        for x, y in points:
            if not black(x, y):
                break
            run += 1
        return run

    mid_x, mid_y = width // 2, height // 2
    runs = (
        edge_run((x, mid_y) for x in range(width)),
        edge_run((x, mid_y) for x in reversed(range(width))),
        edge_run((mid_x, y) for y in range(height)),
        edge_run((mid_x, y) for y in reversed(range(height))),
    )
    if runs != (margin,) * 4:
        found = ", ".join(f"{side} {run}" for side, run in zip(("left", "right", "top", "bottom"), runs))
        return f"left uncropped: expected a {margin}-pixel black window frame on every side, found {found}"
    stride = width * 4
    kept = b"".join(pixels[y * stride + cut * 4:(y + 1) * stride - cut * 4] for y in range(cut, height - cut))
    write_rgba(path, width - 2 * cut, height - 2 * cut, kept)
    return None


# --- capture --------------------------------------------------------------------------------------------------------


def host_platform() -> str:
    """The platform the app runs on here, as the batch names it: `macos` or `linux`."""
    return "macos" if sys.platform == "darwin" else "linux"


def capture(
    out: Path,
    scenes: dict,
    build: bool,
    *,
    lock: Path | None = None,
    agent: Path = GUI_AGENT,
    runs: Path | None = None,
    goldens: Path = GOLDENS,
    platform: str | None = None,
    commit: str | None = None,
    step_timeout: float = STEP_TIMEOUT_SECONDS,
) -> dict:
    """Copy the foundation goldens, capture every app scene in every app variant, and record it all in captures.json.

    The goldens are copied from the tree rather than re-rendered: `scripts/check-gui-goldens.py` is what proves them
    current, and a batch is not a gate. Because they are copies, every platform's goldens are shown wherever the batch
    runs, each labelled with its platform; only a golden that no platform has is an error. Each variant is its own app
    run per group of scenes that share `start` options ([`start_groups`]), each in its own fresh directory under `runs`
    (a new directory under /tmp by default); `start` runs inside the `try`,
    so an interrupted or failed start is still followed by `stop` whenever it made the run directory. A start that
    failed is recorded under `start_failures` (also when the list has no app scenes to blame it on) and a `stop` that
    failed under `stop_failures`. `lock` is passed to every `start` (a shared GUI session's lock, as on the test Mac).
    Only the first run builds. On Linux each capture is cropped to the app's own drawing (`LINUX_FRAME` says why).

    `agent`, `runs`, `goldens`, `platform`, `commit` and `step_timeout` exist so the self-test can drive a fake harness
    without touching the real checkout.
    """
    platform = platform or host_platform()
    shots = out / "captures"
    shots.mkdir(parents=True, exist_ok=True)
    record: dict = {
        "commit": commit or tree_commit(),
        "app": {},
        "goldens": {},
        "errors": {},
        "start_failures": [],
        "stop_failures": [],
    }

    def error(scene_id: str, message: str) -> None:
        record["errors"].setdefault(scene_id, []).append(message)

    golden_platforms = sorted(p.name for p in goldens.iterdir() if p.is_dir()) if goldens.is_dir() else []
    for entry in scenes["foundation_goldens"]:
        for theme in THEMES:
            name = f"{entry['golden']}--{theme}.png"
            found = [p for p in golden_platforms if (goldens / p / name).exists()]
            if not found:
                error(f"golden-{entry['golden']}", f"no platform's goldens under {goldens} hold {name}")
            for golden_platform in found:
                target = shots / f"golden-{entry['golden']}--{golden_platform}--{theme}.png"
                shutil.copyfile(goldens / golden_platform / name, target)
                shown = record["goldens"].setdefault(entry["golden"], {}).setdefault(golden_platform, {})
                shown[theme] = str(target.relative_to(out))

    frame = LINUX_FRAME if platform == "linux" else 0
    runs = runs or Path(tempfile.mkdtemp(prefix="gui-batch-", dir=RUNS_PARENT))
    first_run = True
    for variant in scenes["app_variants"]:
        theme, size = variant["theme"], variant["size"]
        width, height = variant["window"]
        # The window is asked larger by the frame so that, once the frame is cropped, the app's drawing is the size the
        # variant names (and the size the old app's pictures of it have).
        window = (width + 2 * frame, height + 2 * frame)
        for group, (extra, group_scenes) in enumerate(start_groups(scenes["app_scenes"], scenes.get("app_start", []))):
            # The first group keeps the plain `<theme>-<size>` name; a later one, its own app run, is numbered.
            run = runs / (f"{theme}-{size}" if group == 0 else f"{theme}-{size}-{group}")
            label = f"{theme} {size}" + (f" ({' '.join(extra)})" if extra else "")
            start = ["start", *extra, "--window-size", f"{window[0]}x{window[1]}", "--appearance", theme]
            if platform == "linux":
                monitor = (max(window[0] + MONITOR_MARGIN, MIN_MONITOR[0]),
                           max(window[1] + MONITOR_MARGIN, MIN_MONITOR[1]))
                start += ["--size", f"{monitor[0]}x{monitor[1]}"]
            if not build or not first_run:
                start.append("--no-build")
            first_run = False
            if lock is not None:
                start += ["--lock", str(lock)]
            clean = False
            try:
                started = clean = run_agent(agent, run, start, timeout=None) == 0
                if not started:
                    kept = f"; the run is kept at {run}" if run.exists() else ""
                    record["start_failures"].append(f"{label}: the app did not start{kept}")
                for entry in group_scenes:
                    if not started:
                        error(entry["id"], f"{label}: the app did not start")
                        continue
                    failed = next(
                        (step for step in entry["steps"] if run_agent(agent, run, step, step_timeout) != 0), None
                    )
                    target = shots / f"{entry['id']}--{theme}--{size}.png"
                    if failed is None and run_agent(agent, run, ["screenshot", str(target)], step_timeout) == 0:
                        shot = str(target.relative_to(out))
                        record["app"].setdefault(entry["id"], {}).setdefault(theme, {})[size] = shot
                        problem = crop_frame(target, LINUX_FRAME_MARGIN, LINUX_FRAME_BORDER) if frame else None
                        if problem:
                            error(entry["id"], f"{label}: {problem}")
                    else:
                        clean = False
                        what = f"step failed: {' '.join(failed)}" if failed else "screenshot failed"
                        error(entry["id"], f"{label}: {what}")
            finally:
                # A start that failed before making the run directory left nothing to stop.
                if run.exists():
                    if run_agent(agent, run, ["stop"], timeout=None) != 0:
                        record["stop_failures"].append(f"{label}: `stop` failed; {run} may still be running")
                    elif clean:
                        shutil.rmtree(run)
    if runs.exists() and not any(runs.iterdir()):
        runs.rmdir()
    (out / "captures.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    return record


def start_groups(app_scenes: list[dict], default: list[str] | None = None) -> list[tuple[list[str], list[dict]]]:
    """The app scenes grouped by their `start` options (`default`, the list's `app_start`, when a scene names none),
    each group one app run per variant: the groups in the order their first scene is listed, the scenes in list order
    within each.

    A scene's steps begin where the previous scene of its group left the app (a menu one scene opened, a session another
    deleted), so a scene that needs a different fleet, an empty one say, cannot share that run. With no app scenes there
    is still one group with none in it: each variant's run then only proves the app starts and stops, and a failure to
    do so still fails the batch.
    """
    groups: dict[tuple[str, ...], list[dict]] = {}
    for entry in app_scenes:
        groups.setdefault(tuple(entry.get("start", default or [])), []).append(entry)
    return [(list(extra), entries) for extra, entries in groups.items()] or [(list(default or []), [])]


def run_agent(agent: Path, run: Path, args: list[str], timeout: float | None = STEP_TIMEOUT_SECONDS) -> int:
    """One `gui-agent.py` command against `run`; a command that outlives `timeout` counts as failed."""
    try:
        return subprocess.run([sys.executable, str(agent), str(run), *args], timeout=timeout).returncode
    except subprocess.TimeoutExpired:
        print(f"gui-screenshot-batch: `{' '.join(args)}` timed out after {timeout} s", file=sys.stderr)
        return 124


def capture_failed(record: dict) -> bool:
    """Whether a capture should fail the script: any scene's capture failed, or an app run did not start or did not
    stop cleanly."""
    return bool(record.get("errors") or record.get("start_failures") or record.get("stop_failures"))


def tree_commit(repo: Path = REPO) -> str:
    """The tree the capture ran on, so the page says what it shows.

    In a jj checkout that is the working-copy commit (`@`), which includes uncommitted work; git's HEAD there is `@-`
    and would name a tree the capture did not run on. Without jj, HEAD is marked when tracked files have uncommitted
    changes. Untracked files are not counted: a build machine's checkout often holds leftovers of other runs (golden
    captures, say) that do not change what the app shows.
    """
    if (repo / ".jj").is_dir():
        result = subprocess.run(
            ["jj", "-R", str(repo), "log", "-r", "@", "--no-graph", "-T", "commit_id"], capture_output=True, text=True
        )
        if result.returncode == 0 and result.stdout.strip():
            return f"{result.stdout.strip()} (jj working copy)"
    head = subprocess.run(["git", "-C", str(repo), "rev-parse", "HEAD"], capture_output=True, text=True)
    if head.returncode != 0:
        return "unknown"
    dirty = subprocess.run(
        ["git", "-C", str(repo), "status", "--porcelain", "--untracked-files=no"], capture_output=True, text=True
    )
    return head.stdout.strip() + (" + uncommitted changes" if dirty.stdout.strip() else "")


# --- page -----------------------------------------------------------------------------------------------------------


def build_page(
    out: Path, scenes: dict, reference: list[dict], ref_dir: Path, captures: dict, touched: list[str], copy_refs: bool
) -> tuple[str, list[str]]:
    """The batch page as HTML, and the `--touched` ids that match no scene (which would otherwise be dropped without a
    word; the page names them, and `page` repeats them on stderr). Pure apart from reading image headers and copying
    references when asked.

    Three labels keep a reviewer from misreading an empty slot: "not yet ported" (a reference scene the scene list does
    not mark ported), "not captured in this batch" (an app scene or golden this batch has no picture of, with no
    recorded failure) and "capture failed: ..." (a recorded failure).
    """
    by_id = {s["id"]: s for s in reference}
    ported = {s["id"] for s in scenes["reference_scenes"] if s.get("ported")}
    listed = {s["id"] for s in scenes["reference_scenes"]}

    def ref_src(rel: str) -> str:
        source = ref_dir / rel
        if copy_refs:
            target = out / "reference" / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            if source.exists():
                shutil.copyfile(source, target)
            return str(Path("reference") / rel)
        return os.path.relpath(source, out)

    def img(src: str, label: str) -> str:
        return (
            f'<figure><a href="{html.escape(src)}"><img src="{html.escape(src)}" alt="{html.escape(label)}" '
            f'loading="lazy"></a><figcaption>{html.escape(label)}</figcaption></figure>'
        )

    def reference_cell(scene_id: str | None) -> str:
        if scene_id is None:
            return '<p class="none">no old-app counterpart</p>'
        scene = by_id.get(scene_id)
        if scene is None:
            return f'<p class="none">unknown reference scene {html.escape(scene_id)}</p>'
        files = scene.get("files", {})
        parts = [img(ref_src(rel), f"{theme} {size}") for theme, sizes in files.items() for size, rel in sizes.items()]
        return "".join(parts) or f'<p class="none">not captured: {html.escape(scene.get("reason", ""))}</p>'

    def capture_cell(paths: list[tuple[str, str]], errors: list[str], mockup: bool) -> str:
        parts = []
        for rel, label in paths:
            size = png_size(out / rel)
            tag = f"{label} ({size[0]}x{size[1]})" if size else label
            parts.append(img(rel, ("MOCKUP " if mockup else "") + tag))
        parts += [f'<p class="missing">capture failed: {html.escape(e)}</p>' for e in errors]
        return "".join(parts) or '<p class="none">not captured in this batch</p>'

    errors = captures.get("errors", {})
    rows: list[tuple[str, str]] = []
    for entry in scenes["app_scenes"]:
        shots = captures.get("app", {}).get(entry["id"], {})
        paths = [(rel, f"{theme} {size}") for theme, sizes in shots.items() for size, rel in sizes.items()]
        cell = capture_cell(paths, errors.get(entry["id"], []), entry.get("mockup", False))
        rows.append((entry["id"], row(entry["id"], entry["title"], cell, reference_cell(entry.get("reference")),
                                      "mockup" if entry.get("mockup") else "app")))
    for entry in scenes["foundation_goldens"]:
        key = f"golden-{entry['golden']}"
        shots = captures.get("goldens", {}).get(entry["golden"], {})
        # Goldens are copies, so every platform's are shown wherever the batch ran; the label says whose each one is.
        paths = [(rel, f"golden, {platform}, {theme}") for platform, themes in sorted(shots.items())
                 for theme, rel in sorted(themes.items())]
        cell = capture_cell(paths, errors.get(key, []), False)
        rows.append((key, row(key, entry["title"], cell, reference_cell(entry.get("reference")), "foundation")))
    for scene in reference:
        state = "ported" if scene["id"] in ported else "not yet ported"
        cell = '<p class="missing">not yet ported</p>' if state != "ported" else "<p>see the app rows above</p>"
        rows.append((scene["id"], row(scene["id"], scene.get("title", scene["id"]), cell, reference_cell(scene["id"]),
                                      state)))

    first = [r for key, r in rows if key in touched]
    rest = [r for key, r in rows if key not in touched]
    warnings = []
    unlisted = sorted(set(by_id) - listed)
    if unlisted:
        warnings.append(f"reference scenes missing from the scene list: {', '.join(unlisted)}")
    keys = {key for key, _ in rows}
    unknown = [t for t in touched if t not in keys]
    if unknown:
        warnings.append(f"--touched ids that match no scene: {', '.join(unknown)}")
    for failure in captures.get("start_failures", []):
        warnings.append(f"an app run did not start: {failure}")
    for failure in captures.get("stop_failures", []):
        warnings.append(f"an app run did not stop cleanly: {failure}")
    counts = (f"{len(ported)} of {plural(len(reference), 'reference scene')} ported; "
              f"{plural(len(scenes['app_scenes']), 'app scene')}; "
              f"{plural(len(scenes['foundation_goldens']), 'foundation golden')}")
    text = PAGE.format(
        commit=html.escape(captures.get("commit", "unknown (no capture)")),
        reference_commit=html.escape(scenes.get("reference_commit", "")),
        counts=html.escape(counts),
        warning="".join(f'<p class="missing">{html.escape(w)}</p>' for w in warnings),
        touched=("<h2>Touched since the last batch</h2>" + "".join(first)) if first else "",
        rows="".join(rest),
    )
    return text, unknown


def plural(count: int, noun: str) -> str:
    """`1 app scene`, `2 app scenes`: the page header's counts."""
    return f"{count} {noun}" if count == 1 else f"{count} {noun}s"


def row(scene_id: str, title: str, new: str, ref: str, state: str) -> str:
    """One scene: its id, title and state, the new capture on the left and the old app's pictures on the right."""
    return (
        f'<section id="{html.escape(scene_id)}" class="{html.escape(state.replace(" ", "-"))}">'
        f"<h3>{html.escape(title)} <code>{html.escape(scene_id)}</code> <span>{html.escape(state)}</span></h3>"
        f'<div class="pair"><div><h4>GPUI app</h4>{new}</div><div><h4>Old app</h4>{ref}</div></div></section>'
    )


PAGE = """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>GPUI screenshot batch</title>
<style>
body {{ font: 14px system-ui, sans-serif; margin: 16px; background: #f4f5f8; color: #1a1d24; }}
section {{ background: #fff; border: 1px solid #d0d4dc; border-radius: 6px; margin: 12px 0; padding: 8px 12px; }}
.pair {{ display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }}
figure {{ display: inline-block; margin: 4px; }} img {{ max-width: 320px; border: 1px solid #ccc; }}
figcaption, h3 span {{ color: #5a6070; font-size: 12px; }} .missing {{ color: #a33; }} .none {{ color: #777; }}
section.mockup h3 span {{ color: #a60; font-weight: bold; }}
</style></head><body>
<h1>GPUI screenshot batch</h1>
<p>Captured from <code>{commit}</code>; old-app reference from <code>{reference_commit}</code>. {counts}.</p>
{warning}
{touched}
<h2>Scenes</h2>
{rows}
</body></html>
"""


def page(out: Path, ref_dir: Path, touched: list[str], copy_refs: bool) -> Path:
    """Write OUT_DIR/index.html from the captures under OUT_DIR and the reference set."""
    scenes = load_scenes()
    reference = json.loads((ref_dir / "scenes.json").read_text(encoding="utf-8"))
    captures_file = out / "captures.json"
    captures = json.loads(captures_file.read_text(encoding="utf-8")) if captures_file.exists() else {}
    text, unknown_ids = build_page(out, scenes, reference, ref_dir, captures, touched, copy_refs)
    for unknown in unknown_ids:
        print(f"gui-screenshot-batch: warning: --touched {unknown} matches no scene", file=sys.stderr)
    target = out / "index.html"
    target.write_text(text, encoding="utf-8")
    return target


# --- command line ---------------------------------------------------------------------------------------------------


def refuse_inside(out: Path, repo: Path) -> str | None:
    """Why `out` may not hold a batch, or None. Captured screenshots are never committed, so nothing goes in the
    checkout; `out` is resolved first, so a relative path or a symlink into the checkout is caught too."""
    out, repo = out.resolve(), repo.resolve()
    if out == repo or repo in out.parents:
        return "the output directory must be outside the repository (captured screenshots are never committed)"
    return None


def refuse_reuse(out: Path) -> str | None:
    """Why `out` may not take a new capture, or None. A second capture into the same directory would mix two batches'
    pictures under one commit."""
    if (out / "captures.json").exists() or (out / "captures").exists():
        return f"{out} already holds a capture; capture into a new directory"
    return None


def main(argv: list[str], repo: Path = REPO) -> int:
    """The command line. `repo` is the checkout the output must stay out of; the self-test passes a scratch one."""
    parser = argparse.ArgumentParser(prog="gui-screenshot-batch.py")
    parser.add_argument("mode", choices=("capture", "page", "all"))
    parser.add_argument("out", type=Path, help="output directory, outside the repository")
    parser.add_argument("--reference", type=Path, help="the reference set (holds scenes.json and old-app/)")
    parser.add_argument("--touched", default="", help="comma-separated scene ids changed since the last batch")
    parser.add_argument("--copy-references", action="store_true", help="copy reference pictures beside the page")
    parser.add_argument("--no-build", action="store_true", help="reuse the app binaries already built")
    parser.add_argument("--lock", type=Path, help="passed to every `gui-agent.py start` (a shared GUI session's lock)")
    args = parser.parse_args(argv)
    out = args.out.resolve()
    refusal = refuse_inside(out, repo)
    if refusal:
        parser.error(refusal)
    if args.mode in ("page", "all") and args.reference is None:
        parser.error("--reference is required to make the page")
    if args.mode in ("capture", "all"):
        refusal = refuse_reuse(out)
        if refusal:
            parser.error(refusal)
    out.mkdir(parents=True, exist_ok=True)
    status = 0
    if args.mode in ("capture", "all"):
        record = capture(out, load_scenes(), not args.no_build, lock=args.lock)
        for failure in record["start_failures"] + record["stop_failures"]:
            print(f"gui-screenshot-batch: {failure}", file=sys.stderr)
        for scene_id, messages in record["errors"].items():
            for message in messages:
                print(f"gui-screenshot-batch: {scene_id}: {message}", file=sys.stderr)
        status = 1 if capture_failed(record) else 0
    if args.mode in ("page", "all"):
        touched = [t for t in args.touched.split(",") if t]
        print(page(out, args.reference.resolve(), touched, args.copy_references))
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
