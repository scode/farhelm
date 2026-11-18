#!/usr/bin/env python3
"""Capture docs page regions from one staged GPUI run; only checked pictures reach the local docs preview.

Page modules declare their shots, while gui-agent owns staging, capture checks and process cleanup.
"""

import argparse
from dataclasses import dataclass
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


@dataclass(frozen=True)
class Shot:
    """A page's named picture: UI steps, crop regions and an optional known-safe pointer spot for a modal surface."""

    name: str
    steps: Callable
    regions: list[str]
    park: str | None = None


def load(path: Path):
    """Load a sibling script or page by path, so hyphenated filenames need no import search path."""
    spec = importlib.util.spec_from_file_location(path.stem.replace("-", "_"), path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


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


class Bridge:
    """The page steps' small input facade; a bridge refusal fails the shot with the harness's own message."""

    def __init__(self, gui, run: Path, record: dict):
        self.gui, self.run, self.record = gui, run, record

    def _call(self, *args: str) -> str:
        """Read or act through this run's bridge, preserving its failure diagnostics."""
        result = self.gui.cli(self.run, self.record, list(args), capture=True)
        if result.returncode:
            raise RuntimeError(result.stderr.strip() or result.stdout.strip())
        return result.stdout

    def click(self, target: str):
        """Press the named control as a person would, including its focus handoff."""
        self._call("click", target)

    def fill(self, target: str, text: str):
        """Replace a field's value through its input handler so the real UI reacts to it."""
        self._call("fill", target, text)

    def type_into_focused(self, target: str, text: str):
        """Wait for the UI's focus handoff and type without a pointer click that could move focus again."""
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
        self._call("wait", target)


def reset(bridge: Bridge):
    """Dismiss layers before each shot so one shot's dialogs cannot become the next shot's starting state."""
    for attempt in range(7):
        entries = json.loads(bridge._call("snapshot", "-a", "--json", "--no-source"))["entries"]
        if not any(entry.get("key", {}).get("path", "").split("/")[-1].startswith("layer-") for entry in entries):
            return
        if attempt == 6:
            raise RuntimeError("layers remain after 6 Escape presses")
        bridge.press("escape")


def crop_rect(boxes: list[dict], window: dict, frame: int = 0, title_bar: int = 0) -> tuple[int, int, int, int]:
    """Enclose the padded union in captured pixels. Bridge boxes are relative to the decorated window on Linux."""
    scale = window["scale"]
    content_top = frame + title_bar
    left = max(0, min(box["x"] for box in boxes) - frame - 16)
    top = max(0, min(box["y"] for box in boxes) - content_top - 16)
    right = min(window["width"] - frame, max(box["x"] + box["w"] for box in boxes) + 16) - frame
    bottom = min(window["height"] - frame, max(box["y"] + box["h"] for box in boxes) + 16) - content_top
    return math.floor(left * scale), math.floor(top * scale), math.ceil(right * scale), math.ceil(bottom * scale)


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


def capture_pages(gui, batch, run: Path, pages, out: Path, *, frame: int = 0, title_bar: int = 0) -> dict[str, str]:
    """Attempt every shot independently; failed checks never reach the output directory and do not lose later shots."""
    record = json.loads((run / "run.json").read_text())
    bridge, failures = Bridge(gui, run, record), {}
    for page, shots in pages:
        for shot in shots:
            try:
                reset(bridge)
                shot.steps(bridge)
                full = run / f"{page}-{shot.name}-full.png"
                args = [str(full), "--keep-focus"]
                if shot.park is not None:
                    args += ["--park", shot.park]
                if gui.capture(run, record, args):
                    raise RuntimeError("capture failed")
                boxes = [json.loads(bridge._call("get", "box", region, "--json")) for region in shot.regions]
                window = json.loads(bridge._call("windows", "--json"))[0]
                target = out / page / f"{shot.name}.png"
                target.parent.mkdir(parents=True, exist_ok=True)
                batch.crop_png(full, crop_rect(boxes, window, frame, title_bar), target)
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
    parser.add_argument("--lock", type=Path, help="shared GUI lock, held by gui-agent from build through stop")
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
        # Spare width keeps Mutter from maximizing the window and removing the frame capture checks expect.
        start += ["--size", f"{width + batch.MONITOR_MARGIN}x{max(height, 1200)}",
                  "--scale", str(plan["viewport"].get("scale", 1))]
        frame, title_bar = batch.LINUX_FRAME, batch.LINUX_TITLE_BAR
    if options.no_build:
        start.append("--no-build")
    if options.lock:
        start += ["--lock", str(options.lock)]
    failed = True
    try:
        if gui.main([str(run), *start]) == 0:
            stage_templates(gui, run, plan)
            failed = bool(capture_pages(gui, batch, run, pages, options.out, frame=frame, title_bar=title_bar))
    except Exception as error:
        print(f"docs-shots: {error}", file=sys.stderr)
    finally:
        failed = gui.main([str(run), "stop"]) != 0 or failed
        if failed:
            print(f"docs-shots: run directory: {run}", file=sys.stderr)
    return int(failed)


if __name__ == "__main__":
    sys.exit(main())
