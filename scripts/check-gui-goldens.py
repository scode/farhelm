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
Other arguments go to `cargo test` unchanged (a `--` and test-binary arguments included).

Compare mode keeps only the differing or unreferenced captures in the run's own directory under
`target/gui-goldens/`. Automatically fetched references live with that run so another comparison cannot replace
them mid-render. The test prints each retained capture's path beside its failure.
"""

from __future__ import annotations

import argparse
import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
from contextlib import nullcontext
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

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
    if references is None:
        env["FARHELM_UPDATE_GOLDENS"] = str(captures)
    else:
        env["FARHELM_GOLDENS_DIR"] = str(references)
        env["FARHELM_GOLDEN_CAPTURE_DIR"] = str(captures)
    return env


def complete_update(captures: Path, platform: str) -> bool:
    """Certify a successful full render for publication. A filtered-out libtest invocation can exit zero."""
    pictures = sum(1 for image in (captures / platform).glob("*--*.png") if image.is_file())
    if pictures == 0:
        print("the golden update wrote no pictures", file=sys.stderr)
        return False
    (captures / f"{platform}.complete").write_text(f"{pictures}\n", encoding="utf-8")
    return True


def run_suite(tool, base_env: dict[str, str], update: bool, cargo_args: list[str]) -> int:
    """Run with fresh captures, keeping an incomplete update unpublishable and any comparison failures inspectable."""
    lease = tool.capture_lock(tool.CAPTURES) if update else nullcontext()
    with lease as lock:
        return run_locked(tool, base_env, update, cargo_args, lock)


def run_locked(tool, base_env: dict[str, str], update: bool, cargo_args: list[str], lock: int | None) -> int:
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
    else:
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
        command = ["cargo", "test", "--locked", "-p", "farhelm-gui", "--lib", *cargo_args]
        if "--" not in command:
            command.append("--")
        command += ["goldens::", "--ignored"]
        env = golden_env(base_env, runtime_dir, references, captures)
        status = subprocess.run(
            command, cwd=REPO, env=env, pass_fds=() if lock is None else (lock,),
        ).returncode
    if not update:
        if status == 0:
            shutil.rmtree(run_dir)
        return status
    if status != 0:
        return status
    platform = {"darwin": "macos", "win32": "windows"}.get(sys.platform, sys.platform)
    if not complete_update(captures, platform):
        return 1
    print(f"\ncaptures written to {captures}. Publish them with\n  python3 scripts/gui-goldens.py publish\n"
          "and look at each picture it names as changed or added before committing the manifest.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--update", action="store_true", help="write this run's captures for gui-goldens.py publish")
    args, cargo_args = parser.parse_known_args()
    if sys.platform.startswith("linux"):
        missing = [str(path) for path in (LAVAPIPE_ICD, MESA_EGL_VENDOR) if not path.exists()]
        if missing:
            print(f"the software renderer is not installed: {', '.join(missing)} missing", file=sys.stderr)
            return 1
    tool = load_goldens_tool()
    try:
        return run_suite(tool, dict(os.environ), args.update, cargo_args)
    except tool.Refused as refusal:
        print(f"gui-goldens: {refusal}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
