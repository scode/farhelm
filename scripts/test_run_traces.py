#!/usr/bin/env python3
"""Collect only the fixed, bounded files written by farhelm-testtrace.

The recorder owns a private trace root for each command. This module opens
only 128 fixed slot names and five fixed file names per slot; it never walks
arbitrary test state. Collection preserves bytes, including incomplete JSON,
without interpreting a trace as proof that a test passed or finished.
"""

from __future__ import annotations

import argparse
import errno
import hashlib
import io
import json
import os
import pathlib
import stat
import tarfile
import time
from typing import BinaryIO


TRACE_ENV = "FARHELM_TEST_TRACE_DIR"
SLOT_COUNT = 128
FILES = (("metadata.json", 4096), ("head.jsonl", 262144),
         ("tail-0.jsonl", 262144), ("tail-1.jsonl", 262144), ("tail-2.jsonl", 262144))
COLLECTION_SECONDS = 10.0
ERROR_LIMIT = 32
READ_CHUNK = 64 * 1024


class _ArchiveOutput:
    """Give tarfile complete writes over an unbuffered owned descriptor.

    FileIO may return a short write without raising. Tarfile assumes its sink
    consumes each supplied buffer, so forwarding that result would silently
    corrupt the archive while its member hashes still described full data.
    A close never retries data after an earlier output error.
    """

    def __init__(self, raw: BinaryIO, deadline: float):
        self.raw = raw
        self.deadline = deadline

    def tell(self) -> int:
        return self.raw.tell()

    def write(self, data: bytes) -> int:
        view = memoryview(data)
        total = len(view)
        while view:
            if time.monotonic() >= self.deadline:
                raise TimeoutError("trace archive deadline")
            try:
                count = self.raw.write(view[:READ_CHUNK])
            except InterruptedError:
                continue
            if count is None or count <= 0:
                raise OSError(errno.EIO, "trace archive write made no progress")
            view = view[count:]
        return total

    def close(self) -> None:
        self.raw.close()


def open_private_directory(path: pathlib.Path, *, parent_fd: int | None = None) -> int:
    """Hold a private effective-user-owned directory without following its final link.

    Component normalization removes a trailing slash/dot that would otherwise
    force directory traversal before O_NOFOLLOW is applied. Parent directories
    are supplied by the recorder or trusted operator; this is not a filesystem
    sandbox against concurrent mutation by another process of the same user.
    """

    fd = os.open(os.fspath(path), os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                 dir_fd=parent_fd)
    try:
        info = os.fstat(fd)
        if info.st_uid != os.geteuid() or stat.S_IMODE(info.st_mode) & 0o077:
            raise PermissionError("trace directory must be private and owned by the effective user")
        return fd
    except BaseException:
        os.close(fd)
        raise


def create_run_root(run_dir: pathlib.Path) -> tuple[pathlib.Path, int]:
    """Create an exclusive trace directory before a command can inherit its location."""

    path = run_dir / "test-traces"
    path.mkdir(mode=0o700)
    return path, open_private_directory(path)


def _read_prefix(fd: int, limit: int, deadline: float) -> tuple[bytes, str | None, int | None]:
    """Read at most the file cap plus one byte to disclose a clipped or growing file.

    Deadline checks bound user-space retries and reads. A filesystem syscall
    blocked in the kernel remains outside that wall-clock guarantee.
    """

    content = bytearray()
    while len(content) <= limit:
        if time.monotonic() >= deadline:
            return bytes(content[:limit]), "deadline", None
        try:
            chunk = os.read(fd, min(READ_CHUNK, limit + 1 - len(content)))
        except InterruptedError:
            continue
        except OSError as error:
            return bytes(content[:limit]), "read_error", error.errno
        if not chunk:
            return bytes(content), None, None
        content.extend(chunk)
    return bytes(content[:limit]), "file_limit", None


def collect(root_fd: int, archive_path: pathlib.Path) -> dict[str, object]:
    """Archive fixed-name regular-file prefixes through held directory descriptors.

    Missing files in an occupied slot, rejected types, oversized files and I/O
    errors remain explicit. Unknown names are not traversed or uploaded. The
    reported completeness covers this fixed-layout collection, not the trace's
    own counters, outcome or semantic completeness. An empty successful run may
    have no retained slots and therefore no archive.
    """

    deadline = time.monotonic() + COLLECTION_SECONDS
    result: dict[str, object] = {
        "status": "collected",
        "collection_complete": True,
        "archive": None,
        "slots_observed": 0,
        "files": [],
        "errors": [],
        "errors_omitted": 0,
        "limits": {
            "slots": SLOT_COUNT,
            "files_per_slot": dict(FILES),
            "seconds": COLLECTION_SECONDS,
            "unknown_names": "not inspected or collected",
            "snapshot": "observed files; not an atomic filesystem snapshot",
        },
    }
    archive: tarfile.TarFile | None = None
    output: _ArchiveOutput | None = None
    archive_failed = False

    def problem(slot: str | None, name: str | None, kind: str, errno: int | None = None) -> None:
        """Retain bounded error categories without paths or arbitrary exception text."""

        result["collection_complete"] = False
        result["status"] = "incomplete"
        errors = result["errors"]
        if len(errors) < ERROR_LIMIT:
            errors.append({"slot": slot, "file": name, "kind": kind, "errno": errno})
        else:
            result["errors_omitted"] += 1

    try:
        for index in range(SLOT_COUNT):
            if time.monotonic() >= deadline:
                problem(None, None, "deadline")
                break
            slot = f"slot-{index:03d}"
            try:
                slot_fd = open_private_directory(pathlib.Path(slot), parent_fd=root_fd)
            except FileNotFoundError:
                continue
            except OSError as error:
                problem(slot, None, type(error).__name__, error.errno)
                continue
            result["slots_observed"] += 1
            try:
                for name, limit in FILES:
                    fd: int | None = None
                    try:
                        if time.monotonic() >= deadline:
                            raise TimeoutError("trace collection deadline")
                        fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                                     dir_fd=slot_fd)
                        info = os.fstat(fd)
                        if not stat.S_ISREG(info.st_mode):
                            problem(slot, name, "not_regular")
                            continue
                        if info.st_uid != os.geteuid():
                            problem(slot, name, "foreign_owner")
                            continue
                        content, read_problem, read_errno = _read_prefix(fd, limit, deadline)
                        if read_problem is not None:
                            problem(slot, name, read_problem, read_errno)
                    except OSError as error:
                        problem(slot, name, type(error).__name__, error.errno)
                        continue
                    finally:
                        if fd is not None:
                            try:
                                os.close(fd)
                            except OSError as error:
                                problem(slot, name, "file_close", error.errno)

                    # Read failures preserve their prefix. Archive failures instead
                    # stop collection: appending after a partial member write would
                    # make later evidence appear present inside a corrupt archive.
                    if archive is None:
                        output_fd = os.open(archive_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                                            0o600)
                        try:
                            output = _ArchiveOutput(os.fdopen(output_fd, "wb", buffering=0), deadline)
                        except BaseException:
                            os.close(output_fd)
                            raise
                        result["archive"] = archive_path.name
                        archive = tarfile.open(fileobj=output, mode="w", format=tarfile.USTAR_FORMAT)
                    member = tarfile.TarInfo(f"{slot}/{name}")
                    member.size = len(content)
                    member.mode = 0o600
                    archive.addfile(member, io.BytesIO(content))
                    result["files"].append({
                        "path": member.name,
                        "bytes": len(content),
                        "sha256": hashlib.sha256(content).hexdigest(),
                        "truncated": read_problem is not None,
                    })
            finally:
                os.close(slot_fd)
    except (OSError, tarfile.TarError) as error:
        archive_failed = True
        problem(None, None, type(error).__name__, getattr(error, "errno", None))
    finally:
        if archive_failed and archive is not None:
            # TarFile.close writes terminators. After an output fault even that
            # write is forbidden; the unbuffered descriptor can still be closed.
            archive.closed = True
        for resource in (archive, output):
            if resource is not None:
                try:
                    resource.close()
                except OSError as error:
                    problem(None, None, "archive_close", error.errno)
    return result


def main() -> None:
    """Export bounded trace evidence independently after recorder interruption or death."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace_root", type=pathlib.Path)
    parser.add_argument("archive", type=pathlib.Path)
    args = parser.parse_args()
    root_fd: int | None = None
    root_identity: dict[str, int] | None = None
    try:
        root_fd = open_private_directory(args.trace_root)
        info = os.fstat(root_fd)
        root_identity = {"device": info.st_dev, "inode": info.st_ino}
        result = collect(root_fd, args.archive)
    except OSError as error:
        result = {"status": "incomplete", "collection_complete": False,
                  "errors": [{"kind": type(error).__name__, "errno": error.errno}]}
    finally:
        if root_fd is not None:
            os.close(root_fd)
    result["provenance"] = {
        "observation": "standalone export of raw trace files",
        "root_identity": root_identity,
        "earlier_archive_exported": False,
        "earlier_archive_policy": "this export does not copy or describe an earlier recorder archive",
    }
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
