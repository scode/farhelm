#!/usr/bin/env python3
"""Install the repository's exact nextest binary into a dedicated tool directory.

Only the directory is printed to stdout, for PATH construction. Archive and executable hashes are both pinned:
cached executables must still match the pin before reuse. Release callers should supply a fresh RUNNER_TEMP root;
the default repository cache is for local use. This does not replace tools in the user's Cargo installation.
"""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tarfile
import tempfile


REPO = Path(__file__).resolve().parent.parent
ARCHIVE_LIMIT = 64 * 1024 * 1024
BINARY_LIMIT = 128 * 1024 * 1024


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


def digest(data):
    """Return the SHA256 identity used by both archive and executable pins."""
    return hashlib.sha256(data).hexdigest()


def verified_binary(archive, pins):
    """Accept only the checksummed archive's sole regular executable, without extracting paths.

    Archive verification precedes decompression. Member shape, decompressed size, and executable identity are checked
    independently; links or unexpected extra members cannot redirect installation outside its staging directory.
    """
    if len(archive) > ARCHIVE_LIMIT or digest(archive) != pins["archive_sha256"]:
        raise ValueError("nextest archive checksum or size mismatch")
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as bundle:
        member = bundle.next()
        if member is None or member.name != "cargo-nextest" or not member.isfile():
            raise ValueError("nextest archive must contain one regular cargo-nextest file")
        if not 0 < member.size <= BINARY_LIMIT:
            raise ValueError("nextest executable exceeds the size limit")
        source = bundle.extractfile(member)
        if source is None:
            raise ValueError("nextest executable is missing")
        with source:
            binary = source.read(BINARY_LIMIT + 1)
        if len(binary) != member.size or bundle.next() is not None:
            raise ValueError("nextest archive contains unexpected content")
    if digest(binary) != pins["binary_sha256"]:
        raise ValueError("nextest executable checksum mismatch")
    return binary


def cached_binary_matches(path, expected):
    """Reuse only a regular executable with the pinned bytes; a version string is insufficient.

    The cache root is operator-owned, not a boundary against hostile concurrent writers. Refusing a final symlink
    still prevents accidental reuse of an unrelated tool installed elsewhere.
    """
    try:
        if path.is_symlink() or not path.is_file() or not os.access(path, os.X_OK):
            return False
        if path.stat().st_size > BINARY_LIMIT:
            return False
        with path.open("rb") as source:
            data = source.read(BINARY_LIMIT + 1)
        return len(data) <= BINARY_LIMIT and digest(data) == expected
    except OSError:
        return False


def download(url, destination):
    """Fetch over HTTPS with a curl version that enforces the cap while streaming.

    Before 8.4.0, curl's max-filesize cannot bound a response with unknown content length. Disable the default curlrc
    on both invocations: a user's write-out or output settings must not contaminate this installer's stdout contract.
    """
    probe = subprocess.run(
        ["curl", "-q", "--version"], check=True, capture_output=True, text=True, timeout=10,
    )
    match = re.match(r"curl (\d+)\.(\d+)\.(\d+)\b", probe.stdout)
    if match is None or tuple(map(int, match.groups())) < (8, 4, 0):
        raise ValueError("curl 8.4.0 or newer is required to enforce the download size limit")
    subprocess.run(
        [
            "curl", "-q", "--fail", "--silent", "--show-error", "--location",
            "--proto", "=https", "--proto-redir", "=https", "--max-time", "120",
            "--max-filesize", str(ARCHIVE_LIMIT), "--output", str(destination), url,
        ],
        check=True,
        timeout=130,
    )


def install(root, version, target, pins, fetch=download):
    """Publish verified bytes atomically, allowing concurrent installs of the same immutable pin.

    Each invocation stages privately beside its destination. Failed downloads or verification leave an existing
    executable untouched. Concurrent publishers write identical verified bytes, so no mutable lock state is needed.
    """
    directory = root.resolve() / f"{version}-{target}"
    directory.mkdir(parents=True, exist_ok=True)
    executable = directory / "cargo-nextest"
    if cached_binary_matches(executable, pins["binary_sha256"]):
        return directory
    url = (
        "https://github.com/nextest-rs/nextest/releases/download/"
        f"cargo-nextest-{version}/cargo-nextest-{version}-{target}.tar.gz"
    )
    with tempfile.TemporaryDirectory(prefix=".install-", dir=directory) as temporary:
        stage = Path(temporary)
        archive = stage / "archive.tar.gz"
        fetch(url, archive)
        with archive.open("rb") as source:
            binary = verified_binary(source.read(ARCHIVE_LIMIT + 1), pins)
        candidate = stage / "cargo-nextest"
        candidate.write_bytes(binary)
        candidate.chmod(0o755)
        candidate.replace(executable)
    return directory


def main():
    """Print the installation directory only after cache verification or successful publication."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-root", type=Path, default=REPO / ".ci-nextest")
    args = parser.parse_args()
    try:
        pins = json.loads((REPO / ".github/nextest-pins.json").read_text())
        target = target_for(platform.system(), platform.machine())
        directory = install(args.output_root, pins["version"], target, pins["targets"][target])
    except (OSError, ValueError, KeyError, tarfile.TarError, subprocess.SubprocessError) as error:
        print(f"nextest installation failed: {error}", file=sys.stderr)
        return 1
    print(directory)
    return 0


if __name__ == "__main__":
    sys.exit(main())
