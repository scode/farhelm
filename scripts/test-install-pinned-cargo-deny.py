#!/usr/bin/env python3
"""Exercise the pinned cargo-deny installer's own decisions with offline archives.

The download, cache and publication rules it shares with the other pinned tools are covered by
`test-install-pinned-nextest.py` through `pinned_tool.py`. These tests cover what is cargo-deny's: the platform
mapping, the pins file, the release URL, and the nested archive shape (a directory holding the executable beside a
README and license files), which is the one `pinned_tool.py` path no other installer uses. Fixtures inject downloads
and platform identity and never execute a downloaded binary.
"""

import importlib.util
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest


_path = Path(__file__).with_name("install-pinned-cargo-deny.py")
_spec = importlib.util.spec_from_file_location("pinned_cargo_deny", _path)
installer = importlib.util.module_from_spec(_spec)
exec(compile(_path.read_bytes(), str(_path), "exec"), installer.__dict__)
pinned_tool = sys.modules["pinned_tool"]

STEM = "cargo-deny-0.20.2-x86_64-unknown-linux-musl"
MEMBER = f"{STEM}/cargo-deny"


def release_archive(binary, *, binaries=1, symlink_extra=False, members=None, link_at_executable=False):
    """Build an archive shaped like a cargo-deny release asset, with knobs for the shapes that must be refused.

    The real asset holds a directory named after the release, then README.md, LICENSE-APACHE, the executable and
    LICENSE-MIT inside it. `binaries` repeats the executable's member, `symlink_extra` adds a link beside it,
    `link_at_executable` puts a link where the executable belongs instead, and `members` replaces the whole member list
    with that many small files plus the executable.
    """
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as bundle:
        directory = tarfile.TarInfo(STEM)
        directory.type = tarfile.DIRTYPE
        bundle.addfile(directory)
        names = ["README.md", "LICENSE-APACHE", "LICENSE-MIT"] if members is None else [f"f{i}" for i in range(members)]
        for name in names:
            text = f"fixture {name}".encode()
            info = tarfile.TarInfo(f"{STEM}/{name}")
            info.size = len(text)
            bundle.addfile(info, io.BytesIO(text))
        for _ in range(binaries):
            info = tarfile.TarInfo(MEMBER)
            info.size = len(binary)
            info.mode = 0o755
            bundle.addfile(info, io.BytesIO(binary))
        if link_at_executable:
            link = tarfile.TarInfo(MEMBER)
            link.type = tarfile.SYMTYPE
            link.linkname = "/bin/sh"
            bundle.addfile(link)
        if symlink_extra:
            link = tarfile.TarInfo(f"{STEM}/link")
            link.type = tarfile.SYMTYPE
            link.linkname = "/etc/passwd"
            bundle.addfile(link)
    return output.getvalue()


def pins_for(archive, binary):
    """Pins that trust exactly this archive and executable, so only the archive's shape can make it fail."""
    return {"archive_sha256": pinned_tool.digest(archive), "binary_sha256": pinned_tool.digest(binary)}


class CargoDenyInstallerTests(unittest.TestCase):
    """Keep the license policy check running exactly the pinned upstream cargo-deny binary."""

    def test_platform_selection(self):
        """Linux hosts of either architecture and Apple Silicon map to pins; hosts nothing checks on refuse."""
        self.assertEqual(installer.target_for("Darwin", "arm64"), "aarch64-apple-darwin")
        self.assertEqual(installer.target_for("Linux", "x86_64"), "x86_64-unknown-linux-musl")
        self.assertEqual(installer.target_for("Linux", "arm64"), "aarch64-unknown-linux-musl")
        for system, machine in (("Darwin", "x86_64"), ("Windows", "AMD64")):
            with self.subTest(system=system, machine=machine), self.assertRaises(ValueError):
                installer.target_for(system, machine)

    def test_pins_cover_every_target(self):
        """The checked-in pins name every target `target_for` can return, with both digests for each."""
        pins = json.loads((installer.REPO / ".github/cargo-deny-pins.json").read_text())
        supported = {("Darwin", "arm64"), ("Linux", "x86_64"), ("Linux", "arm64"), ("Linux", "aarch64")}
        self.assertEqual(set(pins["targets"]), {installer.target_for(system, machine) for system, machine in supported})
        for target, digests in pins["targets"].items():
            with self.subTest(target=target):
                self.assertEqual(set(digests), {"archive_sha256", "binary_sha256"})

    def test_install_publishes_the_nested_executable_from_the_release_url(self):
        """The URL names the version and target, and only the executable lands in the pin directory."""
        binary = b"fixture cargo-deny"
        archive = release_archive(binary)
        urls = []

        def fetch(url, path):
            urls.append(url)
            path.write_bytes(archive)

        with tempfile.TemporaryDirectory() as temporary:
            directory = installer.install(
                Path(temporary), "0.20.2", "x86_64-unknown-linux-musl", pins_for(archive, binary), fetch,
            )
            self.assertEqual(directory.name, "0.20.2-x86_64-unknown-linux-musl")
            self.assertEqual([path.name for path in directory.iterdir()], ["cargo-deny"])
            self.assertEqual((directory / "cargo-deny").read_bytes(), binary)
            self.assertEqual(
                urls,
                [f"https://github.com/EmbarkStudios/cargo-deny/releases/download/0.20.2/{STEM}.tar.gz"],
            )

    def test_nested_shapes_the_pin_was_not_taken_from_refuse(self):
        """A link beside or at the executable, a missing or repeated executable, too many members, or wrong bytes refuse.

        The archive digest is set to match each fixture, so every refusal here comes from the member walk or the
        executable digest, which are what keep a nested archive as strict as a single-member one.
        """
        binary = b"fixture cargo-deny"
        valid = release_archive(binary)
        self.assertEqual(pinned_tool.verified_binary(valid, pins_for(valid, binary), "cargo-deny", MEMBER), binary)
        # Each fixture differs from `valid` in one way; the expected message pins which check refused it.
        cases = {
            "symlink beside the executable": (release_archive(binary, symlink_extra=True), "not a file or directory"),
            "link at the executable path": (
                release_archive(binary, binaries=0, link_at_executable=True),
                "must contain one regular",
            ),
            "no executable": (release_archive(binary, binaries=0), "must contain one regular"),
            "two executables": (release_archive(binary, binaries=2), "must contain one regular"),
            "too many members": (release_archive(binary, members=pinned_tool.MEMBER_LIMIT), "too many members"),
        }
        for label, (archive, message) in cases.items():
            with self.subTest(label), self.assertRaisesRegex(ValueError, message):
                pinned_tool.verified_binary(archive, pins_for(archive, binary), "cargo-deny", MEMBER)
        at_limit = release_archive(binary, members=pinned_tool.MEMBER_LIMIT - 2)
        self.assertEqual(pinned_tool.verified_binary(at_limit, pins_for(at_limit, binary), "cargo-deny", MEMBER), binary)
        with self.assertRaisesRegex(ValueError, "executable checksum mismatch"):
            pinned_tool.verified_binary(valid, pins_for(valid, b"other bytes"), "cargo-deny", MEMBER)
        with self.assertRaises(ValueError):
            pinned_tool.verified_binary(valid, pins_for(valid, binary), "cargo-deny")


if __name__ == "__main__":
    unittest.main()
