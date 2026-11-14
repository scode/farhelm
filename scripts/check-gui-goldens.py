#!/usr/bin/env python3
"""Run the GPUI app's screenshot goldens under a renderer that gives the same pixels on every machine.

The goldens (`crates/gui/farhelm-gui/src/goldens.rs`) compare captures pixel for pixel, which only holds when every run
draws with the same rasterizer. On Linux that is Mesa's software Vulkan driver (lavapipe): this script points the
Vulkan and EGL loaders at Mesa's software drivers so a machine's real GPU, if it has one, is never picked. It also gives
the run a private, empty runtime directory, so the headless app cannot find a display server or a compositor's socket
from the session it was started in.

The test is ignored in ordinary runs because it needs this environment; running it here is what un-ignores it. Pass
`--update` after an intended visual change to rewrite the references, then look at every changed PNG before keeping it.
Other arguments go to `cargo test` unchanged (a `--` and test-binary arguments included).
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Mesa's software drivers, at the paths Debian and Ubuntu install them to (`mesa-vulkan-drivers`, `libegl-mesa0`).
LAVAPIPE_ICD = Path("/usr/share/vulkan/icd.d/lvp_icd.json")
MESA_EGL_VENDOR = Path("/usr/share/glvnd/egl_vendor.d/50_mesa.json")


def golden_env(base: dict[str, str], runtime_dir: str, update: bool) -> dict[str, str]:
    """The child's environment: `base` pinned to the software renderer, with no display, and the update switch."""
    env = {key: value for key, value in base.items() if key not in ("WAYLAND_DISPLAY", "DISPLAY")}
    env["VK_ICD_FILENAMES"] = str(LAVAPIPE_ICD)
    env["__EGL_VENDOR_LIBRARY_FILENAMES"] = str(MESA_EGL_VENDOR)
    env["LIBGL_ALWAYS_SOFTWARE"] = "true"
    env["XDG_RUNTIME_DIR"] = runtime_dir
    env.pop("FARHELM_UPDATE_GOLDENS", None)
    if update:
        env["FARHELM_UPDATE_GOLDENS"] = "1"
    return env


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--update", action="store_true", help="rewrite the references from this run's captures")
    args, cargo_args = parser.parse_known_args()
    if sys.platform.startswith("linux"):
        missing = [str(path) for path in (LAVAPIPE_ICD, MESA_EGL_VENDOR) if not path.exists()]
        if missing:
            print(f"the software renderer is not installed: {', '.join(missing)} missing", file=sys.stderr)
            return 1
    with tempfile.TemporaryDirectory(prefix="fh-goldens.") as runtime_dir:
        os.chmod(runtime_dir, 0o700)
        command = ["cargo", "test", "--locked", "-p", "farhelm-gui", "--lib", *cargo_args]
        if "--" not in command:
            command.append("--")
        command += ["goldens::", "--ignored"]
        return subprocess.run(command, cwd=REPO, env=golden_env(dict(os.environ), runtime_dir, args.update)).returncode


if __name__ == "__main__":
    sys.exit(main())
