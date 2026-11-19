#!/usr/bin/env python3
"""Capture docs page regions from one staged GPUI run; only checked pictures reach the local docs preview.

Page modules declare their shots, while gui-agent owns staging, capture checks and process cleanup. A shot's
annotations (red callouts with arrows, and highlight rings) are drawn onto the capture by `scripts/docs_callouts.py`.
"""

import argparse
from dataclasses import dataclass, field
import importlib.util
import json
import math
from pathlib import Path
import pwd
import re
import signal
import subprocess
import sys
from typing import Callable

HERE = Path(__file__).resolve().parent
SCENARIO = HERE.parent / "docs/docs-shots/scenario.json5"
WEBSITE_SRC = HERE.parent / "website/src"
RELEASE_PAGE = "release-notes"
sys.modules["docs_shots"] = sys.modules[__name__]


def load(path: Path):
    """Load a sibling script or page by path, so hyphenated filenames need no import search path."""
    spec = importlib.util.spec_from_file_location(path.stem.replace("-", "_"), path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


callouts = load(HERE / "docs_callouts.py")
# Page modules import these from docs_shots, beside Shot, so a page names one module for its whole declaration.
Callout, Highlight = callouts.Callout, callouts.Highlight


@dataclass(frozen=True)
class Shot:
    """A page's named picture: UI steps, crop regions, annotations, and an optional known-safe pointer spot.

    `annotations` are Callout and Highlight values drawn onto the picture after capture; the crop always keeps them
    whole, so a callout placed beside its target is never cut off. `max_width` caps the crop's width in logical pixels,
    keeping its left edge, for a region much wider than what matters in it (a full-width list whose rows are all
    left-aligned text), so the image is not shrunk to fit the docs' text column. `max_height` does the same downward,
    keeping the top, for a region that runs to the window's foot (the terminal, the session list).
    """

    name: str
    steps: Callable
    regions: list[str]
    park: str | None = None
    annotations: list = field(default_factory=list)
    max_width: int | None = None
    max_height: int | None = None


def load_pages(directory: Path, only: str | None) -> list[tuple[str, list[Shot]]]:
    """Refuse malformed shot declarations before an app starts; names must be unique within their page."""
    pages = []
    for path in [directory / f"{only}.py"] if only else sorted(directory.glob("*.py")):
        module = load(path)
        if not hasattr(module, "SHOTS") or not isinstance(module.SHOTS, list):
            raise ValueError(f"{path}: SHOTS must be a list of Shot values")
        names = set()
        for shot in module.SHOTS:
            if not isinstance(shot, Shot) or not isinstance(shot.name, str) or not re.fullmatch(
                r"[a-z0-9]+(?:-[a-z0-9]+)*", shot.name
            ):
                raise ValueError(f"{path}: bad shot name (expected lowercase kebab case)")
            if shot.name in names:
                raise ValueError(f"{path}: duplicate shot name {shot.name}")
            names.add(shot.name)
            if not isinstance(shot.regions, list) or not shot.regions:
                raise ValueError(f"{path}: {shot.name}: regions must be a non-empty list")
            targets = shot.regions + ([] if shot.park is None else [shot.park])
            if any(not isinstance(target, str) or not target.startswith("id:") or len(target) == 3 for target in targets):
                raise ValueError(f"{path}: {shot.name}: regions and park must be id: targets")
            if not isinstance(shot.annotations, (list, tuple)):
                raise ValueError(f"{path}: {shot.name}: annotations must be a list")
            for annotation in shot.annotations:
                problem = callouts.check(annotation)
                if problem:
                    raise ValueError(f"{path}: {shot.name}: {problem}")
            for limit in ("max_width", "max_height"):
                value = getattr(shot, limit)
                if value is not None and (not isinstance(value, int) or value <= 0):
                    raise ValueError(f"{path}: {shot.name}: {limit} must be a positive number of logical pixels")
        pages.append((path.stem, module.SHOTS))
    return pages


def select_shots(
    pages: list[tuple[str, list[Shot]]], website_src: Path, recapture: list[str]
) -> list[tuple[str, list[Shot]]]:
    """Resolve release declarations to their listings and leave published shots for the publisher to carry over.

    The publisher freezes the intersection of the release listings and manifest. Older releases can have images
    in both without a GPUI declaration. Replacing one requires both an explicit request and a current declaration.
    """
    listings = {}
    for path in sorted((website_src / RELEASE_PAGE).glob("*.json")):
        for shot in json.loads(path.read_text(encoding="utf-8")).get("screenshots", []):
            listings.setdefault(shot.get("name"), []).append(path.stem)
    manifest = website_src / "data/docs-shots.json"
    published = json.loads(manifest.read_text(encoding="utf-8")).get("shots", {}) if manifest.is_file() else {}
    frozen = listings.keys() & published.keys()
    requested = set(recapture)
    for name in sorted(requested):
        if not name.startswith(f"{RELEASE_PAGE}/") or name not in frozen:
            raise ValueError(f"--recapture-release-shot {name}: not a published release shot "
                             "(listed in website/src/release-notes and the docs shots manifest)")

    selected = []
    declared = set()
    for page, shots in pages:
        page_shots = []
        for shot in shots:
            name = f"{page}/{shot.name}"
            if page == RELEASE_PAGE:
                match = re.fullmatch(r"v(\d+)-(\d+)-(\d+)-.+", shot.name)
                if not match:
                    raise ValueError(f"{name}: release shot name must be release-notes/v<X-Y-Z>-<what>")
                version = "v" + ".".join(match.groups())
                if listings.get(name) != [version]:
                    raise ValueError(f"{name}: list this shot exactly once, in website/src/release-notes/{version}.json")
            declared.add(name)
            if name in frozen and name not in requested:
                print(f"docs-shots: skipping frozen {name}", flush=True)
            else:
                page_shots.append(shot)
        selected.append((page, page_shots))
    for name in sorted(requested - declared):
        raise ValueError(f"--recapture-release-shot {name}: add its shot to scripts/docs-shots/release-notes.py first")
    return selected


# A target may name a staged session or host by its scenario name, `{session:<title>}` or `{host:<key>}`: the elements
# the app draws for them carry the helm's session key or host id (`session-row-<key>`, `host-rail-host-<id>`), which a
# fresh run mints anew, so a page module could not spell them out.
PLACEHOLDER = re.compile(r"\{(session|host):([^{}]+)\}")


class Bridge:
    """The page steps' small input facade; a bridge refusal fails the shot with the harness's own message.

    Every target it takes may hold `{session:<title>}` and `{host:<key>}` placeholders (`resolve`).
    """

    def __init__(self, gui, run: Path, record: dict, plan: dict | None = None):
        self.gui, self.run, self.record, self.plan = gui, run, record, plan
        self._host_ids: dict | None = None

    def session_key(self, title: str) -> str:
        """The helm's key of the staged session the scenario titles `title`, as `start --scenario` recorded it."""
        sessions = self.record.get("scenario", {}).get("sessions", {})
        if title not in sessions:
            raise RuntimeError(f"no staged session is titled {title!r}")
        return sessions[title]

    def host_id(self, key: str) -> str:
        """The helm's id of the scenario host `key`, read once per run from the helm's own host listing."""
        if self._host_ids is None:
            rows = self.gui.helm_request(self.run, "GET", "/api/hosts")["hosts"]
            self._host_ids, _ = self.gui.scenario_host_ids(self.plan, rows)
        if key not in self._host_ids:
            raise RuntimeError(f"no connected scenario host has the key {key!r}")
        return str(self._host_ids[key])

    def resolve(self, target: str) -> str:
        """`target` with its placeholders replaced by this run's session keys and host ids."""
        return PLACEHOLDER.sub(
            lambda m: self.session_key(m[2]) if m[1] == "session" else self.host_id(m[2]), target
        )

    def _call(self, *args: str) -> str:
        """Read or act through this run's bridge, preserving its failure diagnostics."""
        result = self.gui.cli(self.run, self.record, list(args), capture=True)
        if result.returncode:
            raise RuntimeError(result.stderr.strip() or result.stdout.strip())
        return result.stdout

    def click(self, target: str):
        """Press the named control as a person would, including its focus handoff."""
        self._call("click", self.resolve(target))

    def fill(self, target: str, text: str):
        """Replace a field's value through its input handler so the real UI reacts to it."""
        self._call("fill", self.resolve(target), text)

    def hover(self, target: str):
        """Rest the pointer on the named element, for a shot of what hovering shows; pair it with the shot's `park` on
        the same target, since the capture moves the pointer to its park spot before the picture."""
        self._call("hover", self.resolve(target))

    def scroll_into_view(self, target: str):
        """Scroll the named element's scrollable ancestors until it is on screen, for a control below a long dialog's
        fold."""
        self._call("scrollintoview", self.resolve(target))

    def type_into_focused(self, target: str, text: str):
        """Wait for the UI's focus handoff and type without a pointer click that could move focus again."""
        target = self.resolve(target)

        def focus():
            entries = json.loads(self._call("snapshot", "-i", "--json", "--no-source"))["entries"]
            focused = [entry["key"]["path"] for entry in entries if "focused" in entry.get("states", [])]
            # Kit's input-state child holds focus under the stable field id.
            matched = any(target.removeprefix("id:") in path.split("/") for path in focused)
            return (True if matched else None), f"focused elements: {focused}"

        self.gui.poll(f"{target} to take focus", focus, 10, 0.25)
        self._call("type", text)

    def press(self, key: str):
        """Send a key to the focused control, including Escape through the layer manager."""
        self._call("press", key)

    def wait_text(self, text: str):
        """Wait for visible text as the shot's readiness oracle, using the bridge's bounded wait."""
        self._call("wait", "--text", text)

    def wait(self, target: str):
        """Wait for an element whose stable id establishes that the shot's control is visible."""
        self._call("wait", self.resolve(target))


def reset(bridge: Bridge):
    """Dismiss layers before each shot so one shot's dialogs cannot become the next shot's starting state.

    The Hosts panel is not a layer: it takes the session list's place until its rail button is pressed again, so a shot
    that showed it would otherwise leave it up for every shot after it.
    """
    for attempt in range(7):
        entries = json.loads(bridge._call("snapshot", "-a", "--json", "--no-source"))["entries"]
        names = [entry.get("key", {}).get("path", "").split("/")[-1] for entry in entries]
        if not any(name.startswith("layer-") for name in names):
            if "hosts-panel" in names:
                bridge.click("id:hosts-panel-toggle")
            return
        if attempt == 6:
            raise RuntimeError("layers remain after 6 Escape presses")
        bridge.press("escape")


def session_ids(gui, run: Path) -> set:
    """The helm's sessions, by id: the guard that a shot's steps started, replaced or deleted none (a shot of the
    launcher or of a YOLO question must stop short of Launch; the old browser capture failed any create)."""
    return {row["id"] for row in gui.helm_request(run, "GET", "/api/sessions")["sessions"]}


def stage_add_host(gui, run: Path, plan: dict):
    """Script the run's fake hosts so the scenario's invented `add_host` destination checks as a reachable machine with
    nothing set up and the scenario's home, so the Add host dialog shows the helm's real setup plan for it. Staged runs
    force the scripted provisioning backend on (docs/gui-agent.md, "Scripted hosts"), so no ssh is dialed; no shot
    confirms the setup."""
    add_host = plan.get("add_host")
    if add_host:
        # The unit directory too: the backend's default names its own test account's home
        # (`/home/farhelm-e2e/.config/systemd/user`), which the plan's steps would show instead of the scenario's `~/`.
        home = add_host["home"]
        target = {"probe": "absent", "home": home, "user_unit_dir": f"{home}/.config/systemd/user"}
        gui.script_hosts(run, {"targets": {f"ssh:{add_host['ssh']}": target}})


def crop_rect(boxes: list[dict], window: dict, frame: int = 0, title_bar: int = 0,
              max_width: int | None = None, max_height: int | None = None) -> tuple[int, int, int, int]:
    """Enclose the padded union in captured pixels. Bridge boxes are relative to the decorated window on Linux.

    `max_width` and `max_height` (logical pixels) narrow the result from the right and the bottom, keeping its top left.
    """
    scale = window["scale"]
    content_top = frame + title_bar
    left = max(0, min(box["x"] for box in boxes) - frame - 16)
    top = max(0, min(box["y"] for box in boxes) - content_top - 16)
    right = min(window["width"] - frame, max(box["x"] + box["w"] for box in boxes) + 16) - frame
    if max_width is not None:
        right = min(right, left + max_width)
    bottom = min(window["height"] - frame, max(box["y"] + box["h"] for box in boxes) + 16) - content_top
    if max_height is not None:
        bottom = min(bottom, top + max_height)
    return math.floor(left * scale), math.floor(top * scale), math.ceil(right * scale), math.ceil(bottom * scale)


def annotate(bridge, batch, shot: Shot, full: Path, window: dict, frame: int, title_bar: int) -> list[dict]:
    """Draw `shot`'s annotations onto the capture `full`, in place, and return their extents as bridge-style boxes.

    The capture has already lost the Linux frame and title bar (`gui-agent.py capture`), so it shows exactly the
    content area, the view the old browser overlay placed its callouts in. Bridge boxes still count the decorations, so
    targets move to content coordinates for placement, and the returned boxes move back, so the crop treats them like
    the shot's regions.
    """
    if not shot.annotations:
        return []
    origin = (frame, frame + title_bar)
    targets = {}
    for annotation in shot.annotations:
        box = json.loads(bridge._call("get", "box", bridge.resolve(annotation.target), "--json"))
        targets[annotation.target] = callouts.Rect(box["x"] - origin[0], box["y"] - origin[1], box["w"], box["h"])
    view_w = window["width"] - 2 * frame
    view_h = window["height"] - 2 * frame - title_bar
    drawn = callouts.layout(shot.annotations, targets, view_w, view_h)
    width_px, height_px = batch.png_size(full)
    annotated = full.with_name(full.stem + "-annotated.png")
    callouts.render(full, width_px, height_px, window["scale"], drawn, annotated)
    annotated.replace(full)
    extents = [callouts.extent(item) for item in drawn]
    return [{"x": r.x + origin[0], "y": r.y + origin[1], "w": r.w, "h": r.h} for r in extents]


def stage_templates(gui, run: Path, plan: dict):
    """Store real templates, replacing scenario host keys with the connected hosts' install identities."""
    rows = gui.helm_request(run, "GET", "/api/hosts")["hosts"]
    ids, _ = gui.scenario_host_ids(plan, rows)
    identities = {key: next(row["identity"] for row in rows if row["id"] == host) for key, host in ids.items()}
    for template in plan.get("templates", []):
        fields = dict(template["fields"])
        if "host" in fields:
            fields["host"] = identities[fields["host"]]
            if not fields["host"]:
                raise RuntimeError(f"template {template['name']}: host has no identity yet")
        gui.helm_request(run, "PUT", f"/api/templates/{gui.urllib.parse.quote(template['name'], safe='')}", fields)


def capture_pages(gui, batch, run: Path, pages, out: Path, *, frame: int = 0, title_bar: int = 0,
                  plan: dict | None = None) -> dict[str, str]:
    """Attempt every shot independently; failed checks never reach the output directory and do not lose later shots."""
    record = json.loads((run / "run.json").read_text())
    bridge, failures = Bridge(gui, run, record, plan), {}
    guard = hasattr(gui, "helm_request")
    for page, shots in pages:
        for shot in shots:
            try:
                reset(bridge)
                before = session_ids(gui, run) if guard else None
                shot.steps(bridge)
                full = run / f"{page}-{shot.name}-full.png"
                args = [str(full), "--keep-focus"]
                if shot.park is not None:
                    args += ["--park", bridge.resolve(shot.park)]
                if gui.capture(run, record, args):
                    raise RuntimeError("capture failed")
                boxes = [json.loads(bridge._call("get", "box", bridge.resolve(region), "--json"))
                         for region in shot.regions]
                window = json.loads(bridge._call("windows", "--json"))[0]
                boxes += annotate(bridge, batch, shot, full, window, frame, title_bar)
                if guard and session_ids(gui, run) != before:
                    raise RuntimeError("the shot's steps started, replaced or deleted a session")
                target = out / page / f"{shot.name}.png"
                target.parent.mkdir(parents=True, exist_ok=True)
                rect = crop_rect(boxes, window, frame, title_bar, shot.max_width, shot.max_height)
                batch.crop_png(full, rect, target)
            except Exception as error:
                failures[f"{page}/{shot.name}"] = str(error)
                print(f"docs-shots: {page}/{shot.name}: {error}", file=sys.stderr)
    (run / "failures.json").write_text(json.dumps(failures, indent=2) + "\n")
    return failures


def main() -> int:
    """Own one scenario run for the selected pages, always stopping it before reporting success or retained evidence."""
    # SIGTERM takes the Ctrl-C path so recorder or worker termination reaches the finally that stops the app.
    signal.signal(signal.SIGTERM, signal.default_int_handler)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--check-selection", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--only")
    parser.add_argument("--no-build", action="store_true")
    parser.add_argument("--recapture-release-shot", action="append", default=[], metavar="NAME")
    options = parser.parse_args()
    if options.out is None and not options.check_selection:
        parser.error("--out is required")
    if options.recapture_release_shot and options.only not in (None, RELEASE_PAGE):
        parser.error("--recapture-release-shot works only on a full run or with --only release-notes")
    try:
        pages = select_shots(load_pages(HERE / "docs-shots", options.only), WEBSITE_SRC, options.recapture_release_shot)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    if options.check_selection or not any(shots for _, shots in pages):
        return 0
    gui, batch = load(HERE / "gui-agent.py"), load(HERE / "gui-screenshot-batch.py")
    plan = gui.load_scenario(SCENARIO, pwd.getpwuid(gui.os.getuid()).pw_name)
    run = Path(subprocess.check_output(["mktemp", "-d", "/tmp/fh-docs.XXXXXX"], text=True).strip()) / "run"
    start = ["start", "--scenario", str(SCENARIO), "--appearance", "dark"]
    frame = title_bar = 0
    if sys.platform.startswith("linux"):
        width, height = gui.parse_size(gui.scenario_window_size(plan))
        # Spare width and height keep Mutter from maximizing the window and removing the frame capture checks expect;
        # a window as tall as the monitor is maximized as surely as one as wide.
        start += ["--size", f"{width + batch.MONITOR_MARGIN}x{max(height + batch.MONITOR_MARGIN, 1200)}",
                  "--scale", str(plan["viewport"].get("scale", 1))]
        frame, title_bar = batch.LINUX_FRAME, batch.LINUX_TITLE_BAR
    if options.no_build:
        start.append("--no-build")
    failed = True
    try:
        if gui.main([str(run), *start]) == 0:
            stage_templates(gui, run, plan)
            stage_add_host(gui, run, plan)
            failed = bool(capture_pages(gui, batch, run, pages, options.out, frame=frame, title_bar=title_bar,
                                        plan=plan))
    except Exception as error:
        print(f"docs-shots: {error}", file=sys.stderr)
    finally:
        failed = gui.main([str(run), "stop"]) != 0 or failed
        if failed:
            print(f"docs-shots: run directory: {run}", file=sys.stderr)
    return int(failed)


if __name__ == "__main__":
    sys.exit(main())
