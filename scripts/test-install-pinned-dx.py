#!/usr/bin/env python3
"""Exercise the pinned dx installer's own decisions with offline archives.

The archive, cache and download rules it shares with the nextest installer are covered by
`test-install-pinned-nextest.py` through `pinned_tool.py`; these tests cover what is dx's: the platform mapping, the
refusal of a pin that drifted from the workspace's dioxus version, and an install that lands a verified `dx` where the
release job puts it on PATH. Fixtures inject downloads and platform identity and never execute a downloaded binary.
"""

import importlib.util
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest


_path = Path(__file__).with_name("install-pinned-dx.py")
_spec = importlib.util.spec_from_file_location("pinned_dx", _path)
installer = importlib.util.module_from_spec(_spec)
exec(compile(_path.read_bytes(), str(_path), "exec"), installer.__dict__)
pinned_tool = sys.modules["pinned_tool"]


def archive_for(binary, name="dx"):
    """Build a tiny archive shaped like a Dioxus release asset: one executable named `dx`."""
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as bundle:
        member = tarfile.TarInfo(name)
        member.size = len(binary)
        member.mode = 0o755
        bundle.addfile(member, io.BytesIO(binary))
    return output.getvalue()


class DxInstallerTests(unittest.TestCase):
    """Keep the release build's dx exactly the pinned upstream binary for the workspace's dioxus version."""

    def test_platform_selection(self):
        """Every release build host maps to a pinned target; hosts nothing builds on refuse rather than guessing."""
        self.assertEqual(installer.target_for("Darwin", "arm64"), "aarch64-apple-darwin")
        self.assertEqual(installer.target_for("Linux", "x86_64"), "x86_64-unknown-linux-gnu")
        self.assertEqual(installer.target_for("Linux", "aarch64"), "aarch64-unknown-linux-gnu")
        for system, machine in (("Darwin", "x86_64"), ("Windows", "AMD64")):
            with self.subTest(system=system, machine=machine), self.assertRaises(ValueError):
                installer.target_for(system, machine)

    def test_pins_cover_every_target_and_match_the_workspace(self):
        """The checked-in pins name every target `target_for` can return, at the workspace's dioxus version."""
        pins = json.loads((installer.REPO / ".github/dx-pins.json").read_text())
        installer.require_workspace_version(pins["version"], (installer.REPO / "Cargo.toml").read_text())
        self.assertEqual(
            set(pins["targets"]),
            {"aarch64-apple-darwin", "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"},
        )

    def test_version_drift_refuses(self):
        """A dioxus bump without a matching pin bump fails before download, in either dependency spelling."""
        for spelling in ('dioxus = "0.7.11"', 'dioxus = { version = "0.7.11", default-features = false }'):
            with self.subTest(spelling=spelling), self.assertRaises(ValueError):
                installer.require_workspace_version("0.7.10", f"[workspace.dependencies]\n{spelling}\n")
        installer.require_workspace_version("0.7.10", '[workspace.dependencies]\ndioxus = "0.7.10"\n')

    def test_install_publishes_the_verified_dx_from_the_release_url(self):
        """The upstream URL names the version and target, and the result is an executable `dx` in a pin directory."""
        binary = b"fixture dx"
        archive = archive_for(binary)
        pins = {"archive_sha256": pinned_tool.digest(archive), "binary_sha256": pinned_tool.digest(binary)}
        urls = []

        def fetch(url, path):
            urls.append(url)
            path.write_bytes(archive)

        with tempfile.TemporaryDirectory() as temporary:
            directory = installer.install(Path(temporary), "0.7.10", "x86_64-unknown-linux-gnu", pins, fetch)
            self.assertEqual(directory.name, "0.7.10-x86_64-unknown-linux-gnu")
            self.assertEqual((directory / "dx").read_bytes(), binary)
            self.assertEqual(
                urls,
                ["https://github.com/DioxusLabs/dioxus/releases/download/v0.7.10/dx-x86_64-unknown-linux-gnu.tar.gz"],
            )
            with self.assertRaises(ValueError):
                installer.install(Path(temporary), "0.7.10", "other", pins, lambda url, path: path.write_bytes(b"x"))


if __name__ == "__main__":
    unittest.main()
