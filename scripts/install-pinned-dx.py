#!/usr/bin/env python3
"""Install the repository's exact Dioxus CLI (`dx`) prebuilt binary into a dedicated tool directory.

The release build jobs need `dx` to build the web bundle and the macOS desktop app. They used to compile it from
crates.io with `cargo install`, which cost six to eleven minutes of every build job, on the slowest job's critical path.
This installs the binary the Dioxus project publishes with its GitHub release instead, accepted only if both the
archive and the executable match `.github/dx-pins.json`. The verification and publication rules are `pinned_tool.py`'s,
shared with the pinned nextest install.

Only the directory is printed to stdout, for PATH construction. The pinned version must equal the workspace's
`dioxus` dependency in the root `Cargo.toml`: dx refuses to build against a dioxus crate of another version, so a pin
left behind by a dioxus bump is refused here, before any download. Release callers should supply a fresh RUNNER_TEMP
root; the default repository cache is for local use. This does not replace a `dx` in the user's Cargo installation.
"""

import argparse
import json
from pathlib import Path
import platform
import sys
import tomllib

# Importing the shared module must not leave a `__pycache__` in the checkout.
_previous_bytecode_setting = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    import pinned_tool
finally:
    sys.dont_write_bytecode = _previous_bytecode_setting


REPO = Path(__file__).resolve().parent.parent


def target_for(system, machine):
    """Select the prebuilt target for the build hosts the release uses; others fail before any download.

    The Linux binaries require glibc, which every hosted runner and development host here has. There is no Intel Mac
    pin because nothing in this repository builds on one. Platform values are inputs so tests never need to alter the
    process environment or impersonate another host.
    """
    if system == "Darwin" and machine in {"arm64", "aarch64"}:
        return "aarch64-apple-darwin"
    if system == "Linux":
        arch = {"x86_64": "x86_64", "aarch64": "aarch64", "arm64": "aarch64"}.get(machine)
        if arch:
            return f"{arch}-unknown-linux-gnu"
    raise ValueError(f"no pinned dx binary for {system}/{machine}")


def require_workspace_version(pinned, cargo_toml):
    """Refuse a pin that is not the workspace's `dioxus` version, which is the only dx the build accepts."""
    workspace = tomllib.loads(cargo_toml)["workspace"]["dependencies"]["dioxus"]
    wanted = workspace if isinstance(workspace, str) else workspace["version"]
    if pinned != wanted:
        raise ValueError(f".github/dx-pins.json pins dx {pinned}, but the workspace depends on dioxus {wanted}")


def install(root, version, target, pins, fetch=pinned_tool.download):
    """Install the pinned `dx` under `root/<version>-<target>` and return that directory."""
    url = f"https://github.com/DioxusLabs/dioxus/releases/download/v{version}/dx-{target}.tar.gz"
    return pinned_tool.install(root.resolve() / f"{version}-{target}", "dx", url, pins, fetch)


def main():
    """Print the installation directory only after cache verification or successful publication."""
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output-root", type=Path, default=REPO / ".ci-dx")
    args = parser.parse_args()
    try:
        pins = json.loads((REPO / ".github/dx-pins.json").read_text())
        require_workspace_version(pins["version"], (REPO / "Cargo.toml").read_text())
        target = target_for(platform.system(), platform.machine())
        directory = install(args.output_root, pins["version"], target, pins["targets"][target])
    except (*pinned_tool.INSTALL_ERRORS, tomllib.TOMLDecodeError) as error:
        print(f"dx installation failed: {error}", file=sys.stderr)
        return 1
    print(directory)
    return 0


if __name__ == "__main__":
    sys.exit(main())
