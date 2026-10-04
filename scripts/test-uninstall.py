#!/usr/bin/env python3
"""Exercise the standalone installer and its installed uninstall command.

The release server serves a real compiled CLI and a small desktop packaging
fixture. The installer puts them in ~/Applications/Farhelm.app, which is the
whole installation, with ~/.local/bin/farhelm a link to the app's forwarder;
`farhelm uninstall` therefore runs as the app's versioned CLI. All mutations
belong to fresh temporary homes. This suite runs only on macOS because the
installer refuses other platforms. A uname shim cannot substitute for native
coverage: a Linux CLI plans a different (flat) kind of installation.
"""

import argparse
import functools
import hashlib
import http.server
import os
from pathlib import Path
import platform
import pty
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import threading
import unittest


def snapshot(root):
    """Capture contents, links and modes without following fixture symlinks.

    Access times are excluded because inspection may update them. Content and
    directory-entry equality catches receipt, lock and state-file mutations.
    """
    result = {}
    for directory, dirs, files in os.walk(root, followlinks=False):
        for name in dirs + files:
            path = Path(directory) / name
            metadata = path.lstat()
            mode = metadata.st_mode
            if stat.S_ISLNK(mode):
                data = os.readlink(path)
            elif stat.S_ISREG(mode):
                data = hashlib.sha256(path.read_bytes()).hexdigest()
            else:
                data = None
            result[str(path.relative_to(root))] = (mode, data)
    return result


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    """Keep expected fixture requests out of the recorder's test diagnostics."""

    def log_message(self, _format, *_args):
        pass


@unittest.skipUnless(sys.platform == "darwin", "installed uninstall acceptance requires macOS")
class InstalledUninstall(unittest.TestCase):
    """Each case owns a complete install and observes it through child CLIs."""

    binary = None
    installer = None

    @classmethod
    def setUpClass(cls):
        """Serve the immutable current release fixture for all cases.

        The desktop fixture carries the text every desktop build since the
        side-by-side layout contains, which the installer checks for.
        """
        cls.release_dir = tempfile.TemporaryDirectory(prefix="farhelm-releases-")
        cls.addClassCleanup(cls.release_dir.cleanup)
        root = Path(cls.release_dir.name)
        version = subprocess.check_output([str(cls.binary), "--version"], text=True).strip()
        if not version.startswith("farhelm "):
            raise AssertionError(f"unexpected compiled CLI version: {version!r}")
        cls.version = version.removeprefix("farhelm ")
        if platform.machine() != "arm64":
            raise RuntimeError("installer supports native arm64 macOS only")
        cls.target = "aarch64-apple-darwin"
        for name, release_version in (("current", cls.version),):
            release = root / name
            stage = root / (name + "-stage")
            release.mkdir()
            cli_dir = stage / ("farhelm-" + cls.target)
            cli_dir.mkdir(parents=True)
            cli = cli_dir / "farhelm"
            shutil.copy2(cls.binary, cli)
            cli.chmod(0o755)
            with tarfile.open(release / (cli_dir.name + ".tar.gz"), "w:gz") as archive:
                archive.add(cli_dir, arcname=cli_dir.name)
            desktop_dir = stage / ("farhelm-desktop-" + cls.target)
            desktop_dir.mkdir()
            desktop = desktop_dir / "farhelm-desktop"
            desktop.write_text(
                f"#!/bin/sh\nprintf 'desktop fixture {release_version}\\n'\n"
                "# needs its own version of the farhelm binary at its folder\n")
            desktop.chmod(0o755)
            (desktop_dir / "Farhelm.icns").write_bytes(b"packaging fixture icon")
            with tarfile.open(release / (desktop_dir.name + ".tar.gz"), "w:gz") as archive:
                archive.add(desktop_dir, arcname=desktop_dir.name)
            checksums = [f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n"
                         for p in sorted(release.glob("*.tar.gz"))]
            (release / "SHA256SUMS").write_text("".join(checksums))
        handler = functools.partial(QuietHandler, directory=str(root))
        cls.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
        cls.server_thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.server_thread.start()
        cls.addClassCleanup(cls.close_server)
        cls.url = f"http://127.0.0.1:{cls.server.server_port}"

    @classmethod
    def close_server(cls):
        """Join the owned request loop before deleting its release files."""
        cls.server.shutdown()
        cls.server.server_close()
        cls.server_thread.join(timeout=5)
        if cls.server_thread.is_alive():
            raise AssertionError("fixture HTTP server did not terminate")

    def setUp(self):
        """Separate install artifacts, retained data and command shims per case."""
        self.directory = tempfile.TemporaryDirectory(prefix="farhelm-uninstall-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        self.home = self.root / "home 'with quotes'"
        self.home.mkdir(mode=0o700)
        self.env = {
            "HOME": str(self.home),
            "PATH": os.defpath,
            "XDG_CONFIG_HOME": str(self.home / ".config"),
            "XDG_STATE_HOME": str(self.home / ".local/state"),
            "LC_ALL": "C",
        }
        self.install_dir = self.home / ".local/bin"
        self.bundle = self.home / "Applications/Farhelm.app"
        self.data = self.home / ".local/state/farhelm/credentials"
        self.data.parent.mkdir(parents=True)
        self.data.write_bytes(b"retained private fixture data")

    def run_child(self, argv, *, env=None, terminal_answer=None):
        """Capture child diagnostics before the owned fixture is cleaned up."""
        kwargs = dict(env=env or self.env, cwd=self.home, stdout=subprocess.PIPE,
                      stderr=subprocess.PIPE)
        if terminal_answer is None:
            return subprocess.run(argv, stdin=subprocess.DEVNULL, timeout=30, **kwargs)
        master, slave = pty.openpty()
        try:
            with subprocess.Popen(argv, stdin=slave, **kwargs) as child:
                os.close(slave)
                slave = None
                os.write(master, (terminal_answer + "\n").encode())
                try:
                    stdout, stderr = child.communicate(timeout=30)
                except subprocess.TimeoutExpired:
                    child.kill()
                    stdout, stderr = child.communicate()
                    self.fail(f"confirmation child timed out: {stdout!r} {stderr!r}")
                return subprocess.CompletedProcess(argv, child.returncode, stdout, stderr)
        finally:
            os.close(master)
            if slave is not None:
                os.close(slave)

    def install(self):
        """Run the actual shell installer and independently verify its CLI bytes.

        Returns the Terminal link, which is how a user runs uninstall.
        """
        env = dict(self.env, FARHELM_INSTALL_TEST_BASE_URL=self.url + "/current",
                   FARHELM_VERSION=self.version)
        result = self.run_child(["/bin/sh", str(self.installer)], env=env)
        self.assert_success(result)
        cli = self.install_dir / "farhelm"
        self.assertTrue(cli.is_symlink())
        self.assertEqual(Path(os.readlink(cli)), self.bundle / "Contents/MacOS/farhelm")
        versioned = self.bundle / "Contents/Versions" / self.version / "farhelm"
        self.assertEqual(hashlib.sha256(versioned.read_bytes()).digest(),
                         hashlib.sha256(self.binary.read_bytes()).digest())
        self.assertTrue((self.bundle / "Contents/.farhelm-installation").is_file())
        return cli

    def seed_old_layout(self):
        """The installation the layout before this one left: both programs
        copied into ~/.local/bin with a checksum record, and Farhelm.app
        holding copies of both with its own record, written from their
        documented formats."""
        self.install_dir.mkdir(parents=True, exist_ok=True)
        canonical = os.fsencode(self.install_dir.resolve())
        contents = self.bundle / "Contents"
        (contents / "MacOS").mkdir(parents=True)
        (contents / "Resources").mkdir()
        def write(path, data, mode):
            path.write_bytes(data)
            path.chmod(mode)
        def digest(path):
            return hashlib.sha256(path.read_bytes()).hexdigest().encode()
        cli = b"#!/bin/sh\necho 'farhelm 0.0.1'\n"
        desktop = b"#!/bin/sh\necho 'desktop fixture 0.0.1'\n"
        for directory in (self.install_dir, contents / "MacOS"):
            write(directory / "farhelm", cli, 0o755)
            write(directory / "farhelm-desktop", desktop, 0o755)
        write(self.install_dir / ".farhelm-installation", b"\0".join([
            b"farhelm-standalone", canonical, digest(self.install_dir / "farhelm"),
            digest(self.install_dir / "farhelm-desktop")]) + b"\0", 0o600)
        write(contents / "Info.plist", b"<plist/>\n", 0o644)
        write(contents / "Resources/Farhelm.icns", b"old icon", 0o644)
        write(contents / ".farhelm-installation", b"\0".join([
            b"farhelm-app", canonical, digest(contents / "MacOS/farhelm"),
            digest(contents / "MacOS/farhelm-desktop"), digest(contents / "Info.plist"),
            digest(contents / "Resources/Farhelm.icns")]) + b"\0", 0o600)

    def assert_success(self, result):
        """Include both child streams while the failed fixture still exists."""
        self.assertEqual(result.returncode, 0, (result.stdout, result.stderr))

    def assert_removed(self):
        """Removal must preserve user state and the shared executable directory."""
        self.assertFalse((self.install_dir / "farhelm").is_symlink())
        self.assertFalse((self.install_dir / "farhelm").exists())
        self.assertFalse(self.bundle.exists())
        self.assertTrue(self.install_dir.is_dir())
        # Uninstall's app lock never outlives it (see the lock interplay test).
        self.assertFalse((self.home / "Applications/.farhelm-app.lock").exists())
        self.assertEqual(self.data.read_bytes(), b"retained private fixture data")

    def test_fresh_preview_and_remove(self):
        """Dry-run is read-only; confirmed removal retains data and unrelated
        files, and removes the Running record a supervisor of this app left."""
        cli = self.install()
        sentinel = self.install_dir / "unrelated"
        sentinel.write_bytes(b"leave me alone")
        running = self.data.parent / "running-version"
        running.write_text(self.version + "\n")
        before = snapshot(self.home)
        preview = self.run_child([str(cli), "uninstall", "--dry-run"])
        self.assert_success(preview)
        self.assertIn(str(self.bundle).encode(), preview.stdout + preview.stderr)
        self.assertEqual(snapshot(self.home), before)
        self.assert_success(self.run_child([str(cli), "uninstall", "--yes"]))
        self.assert_removed()
        self.assertFalse(running.exists())
        self.assertEqual(sentinel.read_bytes(), b"leave me alone")

    def test_app_lock_interplay(self):
        """Uninstall and the installer exclude each other through the lock
        beside the app.

        Why: SPEC.md ("Concurrent and interrupted runs") requires a correct
        outcome when uninstall overlaps an install or update. Both take
        ~/Applications/.farhelm-app.lock without waiting. Spec: with the lock
        held, the real installer refuses naming it, uninstall refuses naming
        it and removes nothing, and with it gone uninstall succeeds.
        """
        cli = self.install()
        lock = self.home / "Applications/.farhelm-app.lock"
        lock.mkdir(mode=0o700)
        refused_install = self.run_child(["/bin/sh", str(self.installer)], env=dict(
            self.env, FARHELM_INSTALL_TEST_BASE_URL=self.url + "/current", FARHELM_VERSION=self.version))
        self.assertNotEqual(refused_install.returncode, 0)
        self.assertIn(b".farhelm-app.lock", refused_install.stderr)
        refused_uninstall = self.run_child([str(cli), "uninstall", "--yes"])
        self.assertNotEqual(refused_uninstall.returncode, 0)
        self.assertIn(b".farhelm-app.lock", refused_uninstall.stdout + refused_uninstall.stderr)
        self.assertTrue(self.bundle.is_dir(), "a refused uninstall removes nothing")
        lock.rmdir()
        self.assert_success(self.run_child([str(cli), "uninstall", "--yes"]))
        self.assert_removed()

    def test_update_from_the_old_layout_then_remove(self):
        """The installer turns the old layout into the app-only one, and the
        result uninstalls through a link to its Terminal link."""
        self.seed_old_layout()
        cli = self.install()
        self.assertFalse((self.install_dir / "farhelm-desktop").exists())
        self.assertFalse((self.install_dir / ".farhelm-installation").exists())
        link = self.home / "cli-link"
        link.symlink_to(cli)
        self.assert_success(self.run_child([str(link), "uninstall", "--yes"]))
        self.assert_removed()
        self.assertTrue(link.is_symlink())

    def test_confirmation(self):
        """Redirected stdin needs --yes; a terminal gets one cancelable prompt."""
        cli = self.install()
        before = snapshot(self.home)
        result = self.run_child([str(cli), "uninstall"])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"--yes", result.stdout + result.stderr)
        self.assertEqual(snapshot(self.home), before)
        cancelled = self.run_child([str(cli), "uninstall"], terminal_answer="no")
        self.assert_success(cancelled)
        self.assertIn(b"Cancelled", cancelled.stdout)
        self.assertEqual(snapshot(self.home), before)
        oversized = self.run_child([str(cli), "uninstall"], terminal_answer="yes" + " " * 64)
        self.assert_success(oversized)
        self.assertIn(b"Cancelled", oversized.stdout)
        self.assertEqual(snapshot(self.home), before)
        self.assert_success(self.run_child([str(cli), "uninstall"], terminal_answer="yes"))
        self.assert_removed()

    def test_foreign_record_link_refuses(self):
        """A record symlink never expands the uninstaller's deletion authority."""
        cli = self.install()
        record = self.bundle / "Contents/.farhelm-installation"
        target = self.home / "foreign-record"
        record.rename(target)
        record.symlink_to(target)
        before = snapshot(self.home)
        result = self.run_child([str(cli), "uninstall", "--yes"])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(str(record).encode(), result.stdout + result.stderr)
        self.assertIn(b"regular file", result.stdout + result.stderr)
        self.assertEqual(snapshot(self.home), before)

    def test_missing_payload_is_retryable(self):
        """An already-removed payload must not prevent finishing the removal."""
        cli = self.install()
        desktop = self.bundle / "Contents/MacOS/farhelm-desktop"
        self.assertTrue(desktop.is_file())
        desktop.unlink()
        self.assert_success(self.run_child([str(cli), "uninstall", "--yes"]))
        self.assert_removed()

    def test_cli_outside_the_app_names_the_installed_command(self):
        """A farhelm that is not the app's own refuses and names the command
        to use, removing nothing."""
        self.install()
        before = snapshot(self.home)
        result = self.run_child([str(self.binary), "uninstall", "--yes"])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"~/.local/bin/farhelm uninstall", result.stdout + result.stderr)
        self.assertEqual(snapshot(self.home), before)

    def test_foreign_bundle_entry_refuses(self):
        """Unexpected bundle contents survive a refusal before any file removal."""
        cli = self.install()
        sentinel = self.bundle / "Contents/operator-file"
        sentinel.write_bytes(b"operator content")
        before = snapshot(self.home)
        result = self.run_child([str(cli), "uninstall", "--yes"])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"operator-file", result.stdout + result.stderr)
        self.assertEqual(snapshot(self.home), before)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--installer", type=Path, required=True)
    args, remaining = parser.parse_known_args()
    InstalledUninstall.binary = args.binary.resolve(strict=True)
    InstalledUninstall.installer = args.installer.resolve(strict=True)
    unittest.main(argv=[sys.argv[0], *remaining], verbosity=2)
