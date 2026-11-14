"""Checksum-pinned installation of a single prebuilt executable, shared by the pinned tool installers.

`install-pinned-nextest.py` and `install-pinned-cargo-deny.py` each name one upstream release
archive per platform and pin two digests for it: the archive's and the executable's. This module holds what they have in
common: the size limits, the verification of the archive and of the one executable taken from it, reuse of an already
installed copy, and atomic publication into a per-version directory. It never executes what it downloads.

Most archives hold nothing but the executable, and that shape is required by default. Some upstreams (cargo-deny) wrap
the executable in a directory beside their README and license files; a front end names the executable's path inside the
archive for those, and the other members are then tolerated as long as they are plain files or directories.

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
# Bounds the member walk of an archive with a named member path. Release archives that wrap their executable hold a
# handful of files; anything near this many is not the archive the pin was taken from.
MEMBER_LIMIT = 64


def digest(data):
    """Return the SHA256 identity used by both archive and executable pins."""
    return hashlib.sha256(data).hexdigest()


def verified_binary(archive, pins, member_name, member_path=None):
    """Accept only the checksummed archive's one regular executable, without extracting paths.

    Archive verification precedes decompression. Member shape, decompressed size, and executable identity are checked
    independently; links or unexpected extra members cannot redirect installation outside its staging directory.
    Without `member_path` the executable must be the archive's sole member, named `member_name`. With it, the
    executable is the one member at that path, and every other member must be a plain file or directory, whose
    contents are never read.
    """
    if len(archive) > ARCHIVE_LIMIT or digest(archive) != pins["archive_sha256"]:
        raise ValueError(f"{member_name} archive checksum or size mismatch")
    if member_path is not None:
        return _verified_nested_binary(archive, pins, member_name, member_path)
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


def _verified_nested_binary(archive, pins, member_name, member_path):
    """Take the executable at `member_path` from an archive that also holds other files, already digest-checked.

    The walk is bounded by `MEMBER_LIMIT`. A link, device or other special member anywhere refuses the archive, as does
    a missing executable or a second member at its path: none of those is the shape the pin was taken from.
    """
    binary = None
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as bundle:
        for count, member in enumerate(bundle, start=1):
            if count > MEMBER_LIMIT:
                raise ValueError(f"{member_name} archive has too many members")
            if member.name != member_path:
                if not (member.isfile() or member.isdir()):
                    raise ValueError(f"{member_name} archive contains a member that is not a file or directory")
                continue
            if binary is not None or not member.isfile():
                raise ValueError(f"{member_name} archive must contain one regular {member_path} file")
            if not 0 < member.size <= BINARY_LIMIT:
                raise ValueError(f"{member_name} executable exceeds the size limit")
            source = bundle.extractfile(member)
            if source is None:
                raise ValueError(f"{member_name} executable is missing")
            with source:
                binary = source.read(BINARY_LIMIT + 1)
            if len(binary) != member.size:
                raise ValueError(f"{member_name} archive contains unexpected content")
    if binary is None:
        raise ValueError(f"{member_name} archive must contain one regular {member_path} file")
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


def install(directory, name, url, pins, fetch=download, member_path=None):
    """Publish verified bytes as `directory/name` atomically, allowing concurrent installs of the same immutable pin.

    `directory` should be specific to one version and target, so that a cached executable can only ever be the bytes
    of one pin. Each invocation stages privately beside its destination. Failed downloads or verification leave an
    existing executable untouched. Concurrent publishers write identical verified bytes, so no lock is needed.
    `member_path` is `verified_binary`'s: where the executable sits in an archive that holds more than it.
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
            binary = verified_binary(source.read(ARCHIVE_LIMIT + 1), pins, name, member_path)
        candidate = stage / name
        candidate.write_bytes(binary)
        candidate.chmod(0o755)
        candidate.replace(executable)
    return directory


INSTALL_ERRORS = (OSError, ValueError, KeyError, tarfile.TarError, subprocess.SubprocessError)
