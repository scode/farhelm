#!/usr/bin/env python3
"""Install the repository's exact cargo-deny prebuilt binary into a dedicated tool directory.

cargo-deny enforces `deny.toml`, the dependency license and source policy. CI's `deny` job and local checks run the
binary the cargo-deny project publishes with its GitHub release, accepted only if both the archive and the executable
match `.github/cargo-deny-pins.json`; that is seconds, where `cargo install` compiles it for minutes. The verification
and publication rules are `pinned_tool.py`'s, shared with the pinned nextest and dx installs. Unlike theirs, cargo-deny's
archive wraps the executable in a directory beside its README and license files, so this names the executable's path
inside the archive.

Only the directory is printed to stdout. The executable runs on its own as `<directory>/cargo-deny --locked check
licenses sources` from the checkout root, so it does not need to be on PATH. CI callers should supply a fresh RUNNER_TEMP root;
the default repository cache is for local use. This does not replace a cargo-deny in the user's Cargo installation.
"""

import argparse
import json
from pathlib import Path
import platform
import sys

# Importing the shared module must not leave a `__pycache__` in the checkout.
_previous_bytecode_setting = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    import pinned_tool
finally:
    sys.dont_write_bytecode = _previous_bytecode_setting


REPO = Path(__file__).resolve().parent.parent


def target_for(system, machine):
    """Select the prebuilt target for the hosts that run the policy check; others fail before any download.

    The Linux binaries are static musl builds, so they run on any Linux host of their architecture. There is no Intel
    Mac pin because nothing in this repository runs checks on one. Platform values are inputs so tests never need to
    alter the process environment or impersonate another host.
    """
    if system == "Darwin" and machine in {"arm64", "aarch64"}:
        return "aarch64-apple-darwin"
    if system == "Linux":
        arch = {"x86_64": "x86_64", "aarch64": "aarch64", "arm64": "aarch64"}.get(machine)
        if arch:
            return f"{arch}-unknown-linux-musl"
    raise ValueError(f"no pinned cargo-deny binary for {system}/{machine}")


def install(root, version, target, pins, fetch=pinned_tool.download):
    """Install the pinned `cargo-deny` under `root/<version>-<target>` and return that directory."""
    stem = f"cargo-deny-{version}-{target}"
    url = f"https://github.com/EmbarkStudios/cargo-deny/releases/download/{version}/{stem}.tar.gz"
    return pinned_tool.install(
        root.resolve() / f"{version}-{target}", "cargo-deny", url, pins, fetch, member_path=f"{stem}/cargo-deny",
    )


def main():
    """Print the installation directory only after cache verification or successful publication."""
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output-root", type=Path, default=REPO / ".ci-cargo-deny")
    args = parser.parse_args()
    try:
        pins = json.loads((REPO / ".github/cargo-deny-pins.json").read_text())
        target = target_for(platform.system(), platform.machine())
        directory = install(args.output_root, pins["version"], target, pins["targets"][target])
    except pinned_tool.INSTALL_ERRORS as error:
        print(f"cargo-deny installation failed: {error}", file=sys.stderr)
        return 1
    print(directory)
    return 0


if __name__ == "__main__":
    sys.exit(main())
