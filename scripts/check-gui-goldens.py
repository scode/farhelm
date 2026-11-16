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
"""

from __future__ import annotations

import argparse
import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
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
    base: dict[str, str], runtime_dir: str, references: Path | None, captures: Path | None
) -> dict[str, str]:
    """The child's environment: `base` pinned to the software renderer, with no display, and either the references to
    compare with (`FARHELM_GOLDENS_DIR`) or the directory an update run writes its captures to
    (`FARHELM_UPDATE_GOLDENS`), never a stale value of the other."""
    env = {key: value for key, value in base.items() if key not in ("WAYLAND_DISPLAY", "DISPLAY")}
    env["VK_ICD_FILENAMES"] = str(LAVAPIPE_ICD)
    env["__EGL_VENDOR_LIBRARY_FILENAMES"] = str(MESA_EGL_VENDOR)
    env["LIBGL_ALWAYS_SOFTWARE"] = "true"
    env["XDG_RUNTIME_DIR"] = runtime_dir
    env.pop("FARHELM_UPDATE_GOLDENS", None)
    env.pop("FARHELM_GOLDENS_DIR", None)
    if captures is not None:
        env["FARHELM_UPDATE_GOLDENS"] = str(captures)
    if references is not None:
        env["FARHELM_GOLDENS_DIR"] = str(references)
    return env


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
    references = captures = None
    if args.update:
        # The directory is this script's own, under the build directory; clearing it keeps one run's captures apart
        # from the last one's. The manifest's digest beside them lets `publish` refuse them after a rebase moved it.
        captures = tool.CAPTURES
        shutil.rmtree(captures, ignore_errors=True)
        captures.mkdir(parents=True)
        tool.record_manifest(captures, tool.MANIFEST)
    elif given := os.environ.get("FARHELM_GOLDENS_DIR"):
        references = Path(given)
    else:
        try:
            references = tool.fetch(tool.MANIFEST, tool.FETCHED, tool.REMOTE, tool.CACHE)
        except tool.Refused as refusal:
            print(f"gui-goldens: {refusal}", file=sys.stderr)
            return 1
    with tempfile.TemporaryDirectory(prefix="fh-goldens.") as runtime_dir:
        os.chmod(runtime_dir, 0o700)
        command = ["cargo", "test", "--locked", "-p", "farhelm-gui", "--lib", *cargo_args]
        if "--" not in command:
            command.append("--")
        command += ["goldens::", "--ignored"]
        env = golden_env(dict(os.environ), runtime_dir, references, captures)
        status = subprocess.run(command, cwd=REPO, env=env).returncode
    if captures is not None and status == 0:
        print(f"\ncaptures written to {captures}. Publish them with\n  python3 scripts/gui-goldens.py publish\n"
              "and look at each picture it names as changed or added before committing the manifest.")
    return status


if __name__ == "__main__":
    sys.exit(main())
