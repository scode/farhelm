#!/usr/bin/env python3
"""Exercise installer trust and publication boundaries with offline archives.

Fixtures inject downloads and platform identity. They never execute a downloaded binary or modify the test process's
environment; the real platform smoke belongs on a sandbox before runner adoption.
"""

import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest import mock


_path = Path(__file__).with_name("install-pinned-nextest.py")
_spec = importlib.util.spec_from_file_location("pinned_nextest", _path)
installer = importlib.util.module_from_spec(_spec)
exec(compile(_path.read_bytes(), str(_path), "exec"), installer.__dict__)


def archive_for(binary, *, name="cargo-nextest", extra=False, symlink=False):
    """Build a tiny archive whose hash can be trusted independently of its member shape."""
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as bundle:
        member = tarfile.TarInfo(name)
        if symlink:
            member.type = tarfile.SYMTYPE
            member.linkname = "elsewhere"
            bundle.addfile(member)
        else:
            member.size = len(binary)
            bundle.addfile(member, io.BytesIO(binary))
        if extra:
            bundle.addfile(tarfile.TarInfo("unexpected"))
    return output.getvalue()


class InstallerTests(unittest.TestCase):
    """Ensure corrupted inputs cannot become a reused or newly installed executable."""

    def test_platform_selection(self):
        """Supported host aliases converge on the pins; unknown hosts refuse rather than guessing."""
        self.assertEqual(installer.target_for("Darwin", "arm64"), "universal-apple-darwin")
        self.assertEqual(installer.target_for("Linux", "arm64"), "aarch64-unknown-linux-gnu")
        self.assertEqual(installer.target_for("Linux", "x86_64"), "x86_64-unknown-linux-gnu")
        with self.assertRaises(ValueError):
            installer.target_for("Windows", "AMD64")

    def test_archive_and_binary_identity(self):
        """Both identities and archive shape matter, even when the archive checksum itself matches."""
        binary = b"fixture executable"
        valid = archive_for(binary)
        pins = {"archive_sha256": installer.digest(valid), "binary_sha256": installer.digest(binary)}
        self.assertEqual(installer.verified_binary(valid, pins), binary)
        for archive in (
            valid + b"corruption",
            archive_for(binary, name="../cargo-nextest"),
            archive_for(binary, symlink=True),
            archive_for(binary, extra=True),
            archive_for(b"wrong binary"),
        ):
            with self.subTest(archive=installer.digest(archive)):
                shaped_pins = dict(pins)
                if archive != valid + b"corruption":
                    shaped_pins["archive_sha256"] = installer.digest(archive)
                with self.assertRaises(ValueError):
                    installer.verified_binary(archive, shaped_pins)

    def test_cache_validation_and_failed_publication(self):
        """Valid cached bytes avoid downloads; a failed repair preserves the old file for diagnosis."""
        binary = b"fixture executable"
        archive = archive_for(binary)
        pins = {"archive_sha256": installer.digest(archive), "binary_sha256": installer.digest(binary)}
        calls = []

        def fetch(url, path):
            calls.append(url)
            path.write_bytes(archive)

        def corrupt_fetch(url, path):
            path.write_bytes(b"corrupt")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = installer.install(root, "0.9.143", "fixture", pins, fetch)
            executable = directory / "cargo-nextest"
            self.assertEqual(executable.read_bytes(), binary)
            self.assertTrue(installer.cached_binary_matches(executable, pins["binary_sha256"]))
            installer.install(root, "0.9.143", "fixture", pins, fetch)
            self.assertEqual(len(calls), 1)
            executable.write_bytes(b"old wrong bytes")
            with self.assertRaises(ValueError):
                installer.install(root, "0.9.143", "fixture", pins, corrupt_fetch)
            self.assertEqual(executable.read_bytes(), b"old wrong bytes")
            self.assertEqual(list(directory.iterdir()), [executable])
            installer.install(root, "0.9.143", "fixture", pins, fetch)
            self.assertEqual(executable.read_bytes(), binary)
            executable.chmod(0o644)
            self.assertFalse(installer.cached_binary_matches(executable, pins["binary_sha256"]))
            elsewhere = root / "elsewhere"
            executable.rename(elsewhere)
            elsewhere.chmod(0o755)
            executable.symlink_to(elsewhere)
            self.assertFalse(installer.cached_binary_matches(executable, pins["binary_sha256"]))
            installer.install(root, "0.9.143", "fixture", pins, fetch)
            self.assertFalse(executable.is_symlink())
            self.assertEqual(elsewhere.read_bytes(), binary)

    def test_download_disables_curlrc_and_requires_streaming_limit(self):
        """No network call starts with an old curl; accepted calls keep configuration and byte bounds explicit."""
        for version in ("curl 7.88.1", "curl 8.3.0", "unexpected output"):
            with self.subTest(version=version), mock.patch.object(installer.subprocess, "run") as run:
                run.return_value.stdout = version
                with self.assertRaises(ValueError):
                    installer.download("https://example.invalid/archive", Path("unused"))
                self.assertEqual(run.call_count, 1)
                self.assertEqual(run.call_args.args[0], ["curl", "-q", "--version"])
        with mock.patch.object(installer.subprocess, "run") as run:
            run.return_value.stdout = "curl 8.4.0 (fixture)"
            installer.download("https://example.invalid/archive", Path("unused"))
            probe, transfer = run.call_args_list
            self.assertEqual(probe.args[0][:2], ["curl", "-q"])
            argv = transfer.args[0]
            self.assertEqual(argv[:2], ["curl", "-q"])
            self.assertEqual(argv[argv.index("--max-filesize") + 1], str(installer.ARCHIVE_LIMIT))
            self.assertEqual(argv[argv.index("--proto") + 1], "=https")
            self.assertEqual(argv[argv.index("--proto-redir") + 1], "=https")
            self.assertEqual(transfer.kwargs["timeout"], 130)


if __name__ == "__main__":
    unittest.main()
