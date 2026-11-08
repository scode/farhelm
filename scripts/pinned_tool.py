"""Checksum-pinned installation of a single prebuilt executable, shared by the pinned tool installers.

`install-pinned-nextest.py` and `install-pinned-dx.py` each name one upstream release archive per platform and pin two
digests for it: the archive's and the executable's. This module holds what they have in common: the size limits, the
verification of an archive whose only member is the executable, reuse of an already installed copy, and atomic
publication into a per-version directory. It never executes what it downloads.

The trust boundary is the pins file each front end reads. A download whose archive or executable digest differs from
the pin is refused, whatever the upstream release now serves, so a replaced release asset fails the install instead of
reaching a build.
"""

import hashlib
import io
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile


ARCHIVE_LIMIT = 64 * 1024 * 1024
BINARY_LIMIT = 128 * 1024 * 1024


def digest(data):
    """Return the SHA256 identity used by both archive and executable pins."""
    return hashlib.sha256(data).hexdigest()


def verified_binary(archive, pins, member_name):
    """Accept only the checksummed archive's sole regular executable, without extracting paths.

    Archive verification precedes decompression. Member shape, decompressed size, and executable identity are checked
    independently; links or unexpected extra members cannot redirect installation outside its staging directory.
    """
    if len(archive) > ARCHIVE_LIMIT or digest(archive) != pins["archive_sha256"]:
        raise ValueError(f"{member_name} archive checksum or size mismatch")
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as bundle:
        member = bundle.next()
        if member is None or member.name != member_name or not member.isfile():
            raise ValueError(f"{member_name} archive must contain one regular {member_name} file")
        if not 0 < member.size <= BINARY_LIMIT:
            raise ValueError(f"{member_name} executable exceeds the size limit")
        source = bundle.extractfile(member)
        if source is None:
            raise ValueError(f"{member_name} executable is missing")
        with source:
            binary = source.read(BINARY_LIMIT + 1)
        if len(binary) != member.size or bundle.next() is not None:
            raise ValueError(f"{member_name} archive contains unexpected content")
    if digest(binary) != pins["binary_sha256"]:
        raise ValueError(f"{member_name} executable checksum mismatch")
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
    on both invocations: a user's write-out or output settings must not contaminate the installers' stdout contract.
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


def install(directory, name, url, pins, fetch=download):
    """Publish verified bytes as `directory/name` atomically, allowing concurrent installs of the same immutable pin.

    `directory` should be specific to one version and target, so that a cached executable can only ever be the bytes
    of one pin. Each invocation stages privately beside its destination. Failed downloads or verification leave an
    existing executable untouched. Concurrent publishers write identical verified bytes, so no lock is needed.
    """
    directory.mkdir(parents=True, exist_ok=True)
    executable = directory / name
    if cached_binary_matches(executable, pins["binary_sha256"]):
        return directory
    with tempfile.TemporaryDirectory(prefix=".install-", dir=directory) as temporary:
        stage = Path(temporary)
        archive = stage / "archive.tar.gz"
        fetch(url, archive)
        with archive.open("rb") as source:
            binary = verified_binary(source.read(ARCHIVE_LIMIT + 1), pins, name)
        candidate = stage / name
        candidate.write_bytes(binary)
        candidate.chmod(0o755)
        candidate.replace(executable)
    return directory


INSTALL_ERRORS = (OSError, ValueError, KeyError, tarfile.TarError, subprocess.SubprocessError)
