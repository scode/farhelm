#!/usr/bin/env python3
"""Validate one published release archive, structurally, before it is signed.

## Why this is not `tar tzf` and `grep`

The release gate has to answer "does this archive contain exactly the one
binary and the LICENSE, and nothing else" over bytes that are about to be
covered by Farhelm's permanent signing key. A textual listing answers a weaker
question: it shows member NAMES, so a symlink or hard link carrying the
expected name reads as a match, and the `file`/`readelf` checks that follow
would then happily inspect whatever the link resolved to. Farhelm's own
extractor (`provisioning/payloads.rs`) refuses any matching entry that is not a
regular file; this is that same rule applied one step earlier, on the
published artifact rather than on the downloaded one.

Reading tar HEADERS instead also removes the newline-oriented comparison a
crafted member name could confuse, and lets the check reject the things a name
list cannot see at all: device and fifo entries, absolute paths, `..`
traversal, and duplicate names.

## What it guarantees

For `<archive>` built from package/target `<prefix>`:

- every member sits under `<prefix>/`, with no absolute path, no `..`
  component, and no control characters in its name;
- the regular files are EXACTLY `<prefix>/<binary>` and `<prefix>/LICENSE`;
- the binary carries an executable bit;
- the only directory entry, if any, is `<prefix>` itself;
- nothing else exists — no symlink, hard link, device, fifo, or duplicate.

With `--extract-to`, the binary is then written out through
`tarfile.extractfile`, one member, by hand. Callers get a file this validator
produced rather than one `tar x` produced, so a link entry can never redirect
the architecture and linkage checks that come next.

`--expect-file` / `--reject-file` run `file(1)` over that extracted binary and
require/forbid substrings in its answer. That is how the Apple archives get an
architecture assertion at all: they hold Mach-O executables, which `readelf`
cannot read. Substrings rather than an exact string on purpose — GNU `file`
says `Mach-O 64-bit arm64 ... executable` where BSD `file` says `Mach-O 64-bit
executable arm64`, and this runs on whichever the runner has.

Usage:
  check-release-archive.py --archive PATH --prefix NAME --binary NAME
                           [--extract-to PATH]
                           [--expect-file SUBSTR ...] [--reject-file SUBSTR ...]
  check-release-archive.py --self-test
"""

from __future__ import annotations

import argparse
import io
import os
import posixpath
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile


class Rejected(Exception):
    """An archive that must not be signed, with the reason a human needs."""


def _check_name(name: str, prefix: str) -> None:
    """Reject a member name that could escape, hide, or confuse.

    Ordering matters here: the traversal and absolute-path checks run before
    anything uses the name for a path decision, so no later code has to be
    careful.
    """
    if not name or name != name.strip():
        raise Rejected(f"member name {name!r} has leading or trailing whitespace")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in name):
        raise Rejected(f"member name {name!r} contains control characters")
    if name.startswith("/") or (len(name) > 1 and name[1] == ":"):
        raise Rejected(f"member name {name!r} is an absolute path")
    if "\\" in name:
        raise Rejected(f"member name {name!r} contains a backslash")
    parts = name.split("/")
    if ".." in parts:
        raise Rejected(f"member name {name!r} traverses out of the archive")
    if parts[0] != prefix:
        raise Rejected(f"member name {name!r} is not under {prefix}/")


def validate(
    archive: str,
    prefix: str,
    binary: str,
    extract_to: str | None = None,
    expect_file: list[str] | None = None,
    reject_file: list[str] | None = None,
) -> None:
    """Raise `Rejected` unless `archive` is exactly the expected two files.

    Extracts the binary to `extract_to` (and runs `file(1)` assertions over it)
    only after every structural check has passed, so the caller never gets a
    path to something this function was about to refuse.
    """
    want_binary = posixpath.join(prefix, binary)
    want_license = posixpath.join(prefix, "LICENSE")

    seen: set[str] = set()
    regular: dict[str, tarfile.TarInfo] = {}

    with tarfile.open(archive, "r:gz") as tar:
        for member in tar:
            name = member.name.rstrip("/") if member.isdir() else member.name
            _check_name(name, prefix)
            if name in seen:
                raise Rejected(f"{archive} lists {name!r} more than once")
            seen.add(name)

            if member.isdir():
                # dist emits one directory entry for the archive root. Anything
                # deeper would be a shape no consumer expects, even empty.
                if name != prefix:
                    raise Rejected(f"unexpected directory entry {name!r}")
                continue
            if not member.isreg():
                raise Rejected(
                    f"{name!r} is not a regular file (tar type {member.type!r}); "
                    "links and devices are never published"
                )
            regular[name] = member

        if set(regular) != {want_binary, want_license}:
            raise Rejected(
                f"{archive} holds {sorted(regular)!r}, "
                f"expected exactly {sorted([want_binary, want_license])!r}"
            )
        if not regular[want_binary].mode & 0o111:
            raise Rejected(f"{want_binary} is not executable (mode {regular[want_binary].mode:o})")

        if extract_to is None and not (expect_file or reject_file):
            return

        # One member, read through the archive object, written by hand. Never
        # `extractall`: the point of everything above is that this file is the
        # one the header described.
        with tempfile.TemporaryDirectory() as work:
            staged = os.path.join(work, binary)
            source = tar.extractfile(regular[want_binary])
            if source is None:
                raise Rejected(f"{want_binary} has no readable contents")
            with open(staged, "wb") as out:
                shutil.copyfileobj(source, out)
            os.chmod(staged, 0o755)

            if expect_file or reject_file:
                described = subprocess.run(
                    ["file", "-b", staged], check=True, capture_output=True, text=True
                ).stdout.strip()
                for want in expect_file or []:
                    if want not in described:
                        raise Rejected(
                            f"{want_binary} is {described!r}, which does not contain {want!r}"
                        )
                for unwanted in reject_file or []:
                    if unwanted in described:
                        raise Rejected(
                            f"{want_binary} is {described!r}, which must not contain {unwanted!r}"
                        )

            if extract_to is not None:
                os.makedirs(os.path.dirname(os.path.abspath(extract_to)) or ".", exist_ok=True)
                shutil.copyfile(staged, extract_to)
                os.chmod(extract_to, 0o755)


# --------------------------------------------------------------------------
# Self-test
#
# The gate's silence is only worth something if it can be shown to speak. Each
# fixture below is a way a compromised or broken build could dress an archive
# up as the expected one; the test asserts every one of them is refused, and
# that a legitimate archive still passes.
# --------------------------------------------------------------------------

MACHO_ARM64 = 0x0100000C
MACHO_X86_64 = 0x01000007


def _macho(cputype: int) -> bytes:
    """A minimal 64-bit Mach-O executable header `file(1)` can classify.

    Enough for an architecture assertion to have something real to read; it is
    not a loadable program and does not need to be.
    """
    header = struct.pack(
        "<8I", 0xFEEDFACF, cputype, 3, 2, 0, 0, 0x00200085, 0
    )
    return header + b"\0" * 64


def _fixture(path: str, prefix: str, binary: str, *, kind: str, payload: bytes = b"exe") -> None:
    """Write one archive shaped by `kind` — `good` plus one flaw per name."""
    with tarfile.open(path, "w:gz") as tar:

        def add_file(name: str, data: bytes, mode: int = 0o755) -> None:
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mode = mode
            tar.addfile(info, io.BytesIO(data))

        def add_special(name: str, type_: bytes, linkname: str = "") -> None:
            info = tarfile.TarInfo(name)
            info.type = type_
            info.linkname = linkname
            info.mode = 0o755
            tar.addfile(info)

        root = tarfile.TarInfo(prefix)
        root.type = tarfile.DIRTYPE
        root.mode = 0o755
        tar.addfile(root)

        if kind == "symlink-binary":
            add_file(f"{prefix}/real", payload)
            add_special(f"{prefix}/{binary}", tarfile.SYMTYPE, "real")
        elif kind == "hardlink-binary":
            add_file(f"{prefix}/real", payload)
            add_special(f"{prefix}/{binary}", tarfile.LNKTYPE, f"{prefix}/real")
        elif kind == "fifo":
            add_file(f"{prefix}/{binary}", payload)
            add_special(f"{prefix}/pipe", tarfile.FIFOTYPE)
        elif kind == "absolute":
            add_file(f"{prefix}/{binary}", payload)
            add_file("/etc/passwd", b"root")
        elif kind == "traversal":
            add_file(f"{prefix}/{binary}", payload)
            add_file(f"{prefix}/../escape", b"x")
        elif kind == "extra":
            add_file(f"{prefix}/{binary}", payload)
            add_file(f"{prefix}/surprise.sh", b"#!/bin/sh\n")
        elif kind == "nested-dir":
            add_file(f"{prefix}/{binary}", payload)
            nested = tarfile.TarInfo(f"{prefix}/inner")
            nested.type = tarfile.DIRTYPE
            tar.addfile(nested)
        elif kind == "not-executable":
            add_file(f"{prefix}/{binary}", payload, mode=0o644)
        elif kind == "missing-license":
            add_file(f"{prefix}/{binary}", payload)
            return
        elif kind == "good":
            add_file(f"{prefix}/{binary}", payload)
        else:
            raise AssertionError(f"unknown fixture kind {kind}")

        add_file(f"{prefix}/LICENSE", b"MIT\n", mode=0o644)


def self_test() -> int:
    prefix, binary = "farhelm-aarch64-apple-darwin", "farhelm"
    failures: list[str] = []

    with tempfile.TemporaryDirectory() as work:

        def path(kind: str) -> str:
            p = os.path.join(work, f"{kind}.tar.gz")
            _fixture(p, prefix, binary, kind=kind, payload=_macho(MACHO_ARM64))
            return p

        def expect_reject(kind: str, **kwargs) -> None:
            try:
                validate(path(kind), prefix, binary, **kwargs)
            except Rejected as why:
                print(f"  ok: {kind} rejected — {why}")
                return
            failures.append(f"{kind} was ACCEPTED")

        try:
            validate(path("good"), prefix, binary)
            print("  ok: a well-formed archive is accepted")
        except Rejected as why:
            failures.append(f"the well-formed fixture was rejected: {why}")

        for kind in (
            "symlink-binary",
            "hardlink-binary",
            "fifo",
            "absolute",
            "traversal",
            "extra",
            "nested-dir",
            "not-executable",
            "missing-license",
        ):
            expect_reject(kind)

        # F17's negative case: the same structurally valid archive, holding an
        # x86_64 Mach-O under an arm64 asset name.
        wrong_arch = os.path.join(work, "wrong-arch.tar.gz")
        _fixture(wrong_arch, prefix, binary, kind="good", payload=_macho(MACHO_X86_64))
        try:
            validate(
                wrong_arch,
                prefix,
                binary,
                expect_file=["Mach-O 64-bit", "arm64"],
                reject_file=["x86_64"],
            )
            failures.append("an x86_64 Mach-O under an arm64 name was ACCEPTED")
        except Rejected as why:
            print(f"  ok: wrong-architecture Mach-O rejected — {why}")

        right_arch = os.path.join(work, "right-arch.tar.gz")
        _fixture(right_arch, prefix, binary, kind="good", payload=_macho(MACHO_ARM64))
        try:
            validate(
                right_arch,
                prefix,
                binary,
                expect_file=["Mach-O 64-bit", "arm64"],
                reject_file=["x86_64"],
            )
            print("  ok: an arm64 Mach-O passes the same assertion")
        except Rejected as why:
            failures.append(f"an arm64 Mach-O was rejected: {why}")

    if failures:
        for line in failures:
            print(f"self-test FAILED: {line}", file=sys.stderr)
        return 1
    print("== self-test ok: every malformed archive is refused")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--archive")
    parser.add_argument("--prefix")
    parser.add_argument("--binary")
    parser.add_argument("--extract-to")
    parser.add_argument("--expect-file", action="append", default=[])
    parser.add_argument("--reject-file", action="append", default=[])
    args = parser.parse_args()

    if args.self_test:
        return self_test()
    if not (args.archive and args.prefix and args.binary):
        parser.error("--archive, --prefix and --binary are required")

    try:
        validate(
            args.archive,
            args.prefix,
            args.binary,
            extract_to=args.extract_to,
            expect_file=args.expect_file,
            reject_file=args.reject_file,
        )
    except (Rejected, tarfile.TarError, OSError) as why:
        print(f"{args.archive}: {why}", file=sys.stderr)
        return 1
    print(f"ok: {args.archive} holds exactly {args.prefix}/{{{args.binary},LICENSE}}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
