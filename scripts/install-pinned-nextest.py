#!/usr/bin/env python3
"""Install the repository's exact nextest binary into a dedicated tool directory.

Only the directory is printed to stdout, for PATH construction. Archive and executable hashes are both pinned
(`.github/nextest-pins.json`): cached executables must still match the pin before reuse. Release callers should supply
a fresh RUNNER_TEMP root; the default repository cache is for local use. This does not replace tools in the user's
Cargo installation. The verification and publication rules are `pinned_tool.py`'s, shared with the pinned dx install.
"""

import argparse
import json
from pathlib import Path
import platform
import subprocess  # noqa: F401 -- tests patch the download's subprocess calls through this module
import sys

# Importing the shared module must not leave a `__pycache__` in the checkout.
_previous_bytecode_setting = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    import pinned_tool
finally:
    sys.dont_write_bytecode = _previous_bytecode_setting

from pinned_tool import ARCHIVE_LIMIT, cached_binary_matches, digest, download  # noqa: E402,F401


REPO = Path(__file__).resolve().parent.parent


def target_for(system, machine):
    """Select a maintained prebuilt target; unsupported hosts fail before any download.

    The Apple archive is universal. Linux builds require glibc, as do the maintained release and sandbox hosts.
    Platform values are inputs so tests never need to alter the process environment or impersonate another host.
    """
    if system == "Darwin" and machine in {"arm64", "aarch64", "x86_64"}:
        return "universal-apple-darwin"
    if system == "Linux":
        arch = {"x86_64": "x86_64", "aarch64": "aarch64", "arm64": "aarch64"}.get(machine)
        if arch:
            return f"{arch}-unknown-linux-gnu"
    raise ValueError(f"no pinned nextest binary for {system}/{machine}")


def verified_binary(archive, pins):
    """Accept only the checksummed archive's sole regular `cargo-nextest` executable."""
    return pinned_tool.verified_binary(archive, pins, "cargo-nextest")


def install(root, version, target, pins, fetch=download):
    """Install the pinned `cargo-nextest` under `root/<version>-<target>` and return that directory."""
    url = (
        "https://github.com/nextest-rs/nextest/releases/download/"
        f"cargo-nextest-{version}/cargo-nextest-{version}-{target}.tar.gz"
    )
    return pinned_tool.install(root.resolve() / f"{version}-{target}", "cargo-nextest", url, pins, fetch)


def main():
    """Print the installation directory only after cache verification or successful publication."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-root", type=Path, default=REPO / ".ci-nextest")
    args = parser.parse_args()
    try:
        pins = json.loads((REPO / ".github/nextest-pins.json").read_text())
        target = target_for(platform.system(), platform.machine())
        directory = install(args.output_root, pins["version"], target, pins["targets"][target])
    except pinned_tool.INSTALL_ERRORS as error:
        print(f"nextest installation failed: {error}", file=sys.stderr)
        return 1
    print(directory)
    return 0


if __name__ == "__main__":
    sys.exit(main())
