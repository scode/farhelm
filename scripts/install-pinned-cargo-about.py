#!/usr/bin/env python3
"""Install the repository's exact cargo-about prebuilt binary into a dedicated tool directory.

cargo-about collects the license texts of the crates compiled into Farhelm's shipped binaries, which
`scripts/dependency-notices.py` turns into the checked-in notices file that `farhelm licenses` prints. The binary is
the one the cargo-about project publishes with its GitHub release, accepted only if both the archive and the
executable match `.github/cargo-about-pins.json`; the verification and publication rules are `pinned_tool.py`'s, shared
with the pinned cargo-deny and nextest installs. Like cargo-deny's, the archive wraps the executable in a directory
beside its README and license files, so this names the executable's path inside the archive. cargo-about itself is
MIT OR Apache-2.0, both on `deny.toml`'s allowlist.

Only the directory is printed to stdout. CI callers should supply a fresh RUNNER_TEMP root; the default repository
cache is for local use. This does not replace a cargo-about in the user's Cargo installation.
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
    """Select the prebuilt target for the hosts that generate or check the notices; others fail before any download.

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
    raise ValueError(f"no pinned cargo-about binary for {system}/{machine}")


def install(root, version, target, pins, fetch=pinned_tool.download):
    """Install the pinned `cargo-about` under `root/<version>-<target>` and return that directory."""
    stem = f"cargo-about-{version}-{target}"
    url = f"https://github.com/EmbarkStudios/cargo-about/releases/download/{version}/{stem}.tar.gz"
    return pinned_tool.install(
        root.resolve() / f"{version}-{target}", "cargo-about", url, pins, fetch, member_path=f"{stem}/cargo-about",
    )


def main():
    """Print the installation directory only after cache verification or successful publication."""
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output-root", type=Path, default=REPO / ".ci-cargo-about")
    args = parser.parse_args()
    try:
        pins = json.loads((REPO / ".github/cargo-about-pins.json").read_text())
        target = target_for(platform.system(), platform.machine())
        directory = install(args.output_root, pins["version"], target, pins["targets"][target])
    except pinned_tool.INSTALL_ERRORS as error:
        print(f"cargo-about installation failed: {error}", file=sys.stderr)
        return 1
    print(directory)
    return 0


if __name__ == "__main__":
    sys.exit(main())
