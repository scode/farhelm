#!/usr/bin/env python3
"""Run the GPUI app's screenshot goldens under a renderer that gives the same pixels on every machine.

The goldens (`crates/gui/farhelm-gui/src/goldens/`) compare captures pixel for pixel, which only holds when every run
draws with the same rasterizer. On Linux that is Mesa's software Vulkan driver (lavapipe): this script points the
Vulkan and EGL loaders at Mesa's software drivers so a machine's real GPU, if it has one, is never picked. It also gives
the run a private, empty runtime directory, so the headless app cannot find a display server or a compositor's socket
from the session it was started in.

The reference pictures live outside the repository (`scripts/gui-goldens.py` says where and why). When
`FARHELM_GOLDENS_DIR` is already set, the run compares against that directory and fetches nothing: that is how a
machine without access to the storage repository runs the goldens, with references fetched elsewhere and copied over.
Otherwise this script runs `gui-goldens.py fetch` first, which needs read access to the storage repository, and points
the test at what it wrote.

The test is ignored in ordinary runs because it needs this environment; running it here is what un-ignores it. Pass
`--update` after an intended visual change: the run then compares nothing and writes every capture to
`target/gui-goldens/captures`, and `python3 scripts/gui-goldens.py publish` pushes the changed pictures and rewrites the
manifest's lines. The captures are tied to the manifest they were taken against, so publish before rebasing, or run
`--update` again after. Look at every picture its report names as changed or added before committing the manifest: the store
only grows, so a wrong picture that was pushed costs nothing, but a manifest line naming it makes it the reference.
Pass `--scene 'settings-*'` to compare or update only matching scenes. Repeat it to select the union of several
patterns. Any explicit selection is for inspection. Publishing requires a full `--update` run.

Extra Cargo build options go before `--`. Only libtest output options (`--nocapture`, `--show-output`, `--quiet`)
go after it so a second test filter cannot quietly skip the render.

Compare mode keeps only the differing or unreferenced captures in the run's own directory under
`target/gui-goldens/`. Automatically fetched references live with that run so another comparison cannot replace
them mid-render. The test prints each retained capture's path beside its failure.
"""

from __future__ import annotations

import argparse
import fnmatch
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from contextlib import nullcontext
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
GUI_CRATE = REPO / "crates/gui/farhelm-gui"
SCENE_INVENTORY = "FARHELM_GOLDEN_SCENES="

# Mesa's software drivers, at the paths Debian and Ubuntu install them to (`mesa-vulkan-drivers`, `libegl-mesa0`).
LAVAPIPE_ICD = Path("/usr/share/vulkan/icd.d/lvp_icd.json")
MESA_EGL_VENDOR = Path("/usr/share/glvnd/egl_vendor.d/50_mesa.json")


def load_goldens_tool():
    """`scripts/gui-goldens.py` as a module (its name is not importable), for its fetch and default paths."""
    spec = importlib.util.spec_from_file_location("gui_goldens", REPO / "scripts/gui-goldens.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def golden_env(
    base: dict[str, str], runtime_dir: str, references: Path | None, captures: Path
) -> dict[str, str]:
    """Give each child a software renderer, an isolated display environment, and this run's paths."""
    env = {key: value for key, value in base.items() if key not in ("WAYLAND_DISPLAY", "DISPLAY")}
    env["VK_ICD_FILENAMES"] = str(LAVAPIPE_ICD)
    env["__EGL_VENDOR_LIBRARY_FILENAMES"] = str(MESA_EGL_VENDOR)
    env["LIBGL_ALWAYS_SOFTWARE"] = "true"
    env["XDG_RUNTIME_DIR"] = runtime_dir
    env.pop("FARHELM_UPDATE_GOLDENS", None)
    env.pop("FARHELM_GOLDENS_DIR", None)
    env.pop("FARHELM_GOLDEN_CAPTURE_DIR", None)
    env.pop("FARHELM_GOLDEN_SCENES", None)
    if references is None:
        env["FARHELM_UPDATE_GOLDENS"] = str(captures)
    else:
        env["FARHELM_GOLDENS_DIR"] = str(references)
        env["FARHELM_GOLDEN_CAPTURE_DIR"] = str(captures)
    return env


def build_binary(base_env: dict[str, str], cargo_args: list[str]) -> Path:
    """Build once and locate the unit test executable in Cargo's artifact records, including custom targets."""
    command = [
        "cargo", "test", "--locked", "--no-run", "-p", "farhelm-gui", "--lib",
        "--message-format=json-render-diagnostics", *cargo_args,
    ]
    built = subprocess.run(command, cwd=REPO, env=base_env, text=True, stdout=subprocess.PIPE, check=True)
    binaries = set()
    for line in built.stdout.splitlines():
        if not line.startswith("{"):
            continue
        artifact = json.loads(line)
        if (artifact.get("reason") == "compiler-artifact"
            and artifact["target"]["name"] == "farhelm_gui"
            and "lib" in artifact["target"]["kind"]
            and artifact["profile"]["test"]
            and artifact["executable"]):
            binaries.add(Path(artifact["executable"]))
    if len(binaries) != 1:
        raise ValueError("Cargo did not report one farhelm-gui unit test executable")
    return binaries.pop()


def list_scenes(binary: Path, base_env: dict[str, str]) -> list[str]:
    """Ask this exact test binary for its ordered scene names without starting a headless app."""
    listing = subprocess.run(
        [str(binary), "--ignored", "--exact", "goldens::list_scenes", "--nocapture"],
        cwd=GUI_CRATE, env=base_env, text=True, stdout=subprocess.PIPE, check=True,
    )
    inventories = [line.removeprefix(SCENE_INVENTORY) for line in listing.stdout.splitlines()
                   if line.startswith(SCENE_INVENTORY)]
    if len(inventories) != 1:
        raise ValueError("the golden test did not report one scene inventory")
    names = json.loads(inventories[0])
    if (not isinstance(names, list) or not names
        or not all(isinstance(name, str) and re.fullmatch(r"[a-z0-9][a-z0-9-]*", name) for name in names)
        or len(set(names)) != len(names)):
        raise ValueError("the golden test reported an empty, invalid or duplicate scene inventory")
    return names


def select_scenes(names: list[str], patterns: list[str]) -> list[str]:
    """Reject a typo in any pattern, even if another pattern matches, and keep catalog order across overlaps."""
    if not patterns:
        return names
    selected = set()
    for pattern in patterns:
        matches = [name for name in names if fnmatch.fnmatchcase(name, pattern)]
        if not matches:
            raise ValueError(f"no golden scene matches {pattern!r}")
        selected.update(matches)
    return [name for name in names if name in selected]


def complete_update(captures: Path, platform: str, names: list[str]) -> bool:
    """Certify only the full catalog. A test invocation that exits zero without drawing is still incomplete."""
    expected = {f"{name}--{appearance}.png" for name in names for appearance in ("light", "dark")}
    pictures = {image.name for image in (captures / platform).glob("*--*.png") if image.is_file()}
    if expected != pictures:
        print(f"incomplete golden update under {captures}: "
              f"{len(expected - pictures)} missing, {len(pictures - expected)} unexpected", file=sys.stderr)
        return False
    (captures / f"{platform}.complete").write_text(f"{len(pictures)}\n", encoding="utf-8")
    return True


def run_suite(tool, base_env: dict[str, str], update: bool, patterns: list[str],
              cargo_args: list[str], test_args: list[str]) -> int:
    """Run with fresh captures, keeping an incomplete update unpublishable and any comparison failures inspectable."""
    lease = tool.capture_lock(tool.CAPTURES) if update else nullcontext()
    with lease as lock:
        return run_locked(tool, base_env, update, patterns, cargo_args, test_args, lock)


def run_locked(tool, base_env: dict[str, str], update: bool, patterns: list[str],
               cargo_args: list[str], test_args: list[str], lock: int | None) -> int:
    """An update owns the default capture directory until every writer has exited and the marker is decided."""
    references = None
    if update:
        captures = tool.CAPTURES
        # Drop the old marker and pictures before drawing. The digest ties this output to the manifest in this run.
        try:
            shutil.rmtree(captures)
        except FileNotFoundError:
            pass
        captures.mkdir(parents=True)
        tool.record_manifest(captures, tool.MANIFEST)
    binary = build_binary(base_env, cargo_args)
    names = select_scenes(list_scenes(binary, base_env), patterns)
    if not update:
        # A custom Cargo target can be shared by many checkouts. Both comparisons and their automatically fetched
        # references belong to this run in the checkout, so another invocation cannot replace either mid-render.
        tool.WORK.mkdir(parents=True, exist_ok=True)
        run_dir = Path(tempfile.mkdtemp(prefix="compare-", dir=tool.WORK))
        captures = run_dir / "captures"
        if given := base_env.get("FARHELM_GOLDENS_DIR"):
            references = Path(given).resolve()
        else:
            references = tool.fetch(tool.MANIFEST, run_dir / "refs", tool.REMOTE, tool.CACHE)
    with tempfile.TemporaryDirectory(prefix="fh-goldens.") as runtime_dir:
        os.chmod(runtime_dir, 0o700)
        command = [str(binary), "--ignored", "--exact", "goldens::component_goldens", *test_args]
        env = golden_env(base_env, runtime_dir, references, captures)
        env["FARHELM_GOLDEN_SCENES"] = json.dumps(names)
        status = subprocess.run(
            command, cwd=GUI_CRATE, env=env, pass_fds=() if lock is None else (lock,),
        ).returncode
    if not update:
        if status == 0:
            shutil.rmtree(run_dir)
        return status
    if status != 0:
        return status
    if patterns:
        print(f"\nselected captures written to {captures}. Run a full --update before publishing.")
        return 0
    platform = {"darwin": "macos", "win32": "windows"}.get(sys.platform, sys.platform)
    if not complete_update(captures, platform, names):
        return 1
    print(f"\ncaptures written to {captures}. Publish them with\n  python3 scripts/gui-goldens.py publish\n"
          "and look at each picture it names as changed or added before committing the manifest.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0], allow_abbrev=False)
    parser.add_argument("--update", action="store_true", help="write this run's captures for gui-goldens.py publish")
    parser.add_argument("--scene", action="append", default=[], metavar="PATTERN",
                        help="match a scene name (repeatable, quote wildcards, selected updates cannot publish)")
    args, extras = parser.parse_known_args()
    separator = extras.index("--") if "--" in extras else len(extras)
    cargo_args, test_args = extras[:separator], extras[separator + 1:]
    for arg in test_args:
        if arg not in ("--nocapture", "--show-output", "--quiet"):
            parser.error(f"test argument {arg!r} is not an output option. Use --scene to select scenes.")
    if sys.platform.startswith("linux"):
        missing = [str(path) for path in (LAVAPIPE_ICD, MESA_EGL_VENDOR) if not path.exists()]
        if missing:
            print(f"the software renderer is not installed: {', '.join(missing)} missing", file=sys.stderr)
            return 1
    tool = load_goldens_tool()
    try:
        return run_suite(tool, dict(os.environ), args.update, args.scene, cargo_args, test_args)
    except tool.Refused as refusal:
        print(f"gui-goldens: {refusal}", file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as error:
        return error.returncode
    except ValueError as error:
        print(f"gui-goldens: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
