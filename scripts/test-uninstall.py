#!/usr/bin/env python3
"""Exercise the standalone installer and its installed uninstall command.

The release server serves a real compiled CLI and a small desktop packaging
fixture. All mutations belong to fresh temporary homes; Linux children see a
fixture systemctl so this suite cannot reach the operator's user manager.
Native macOS uses its actual installer platform and filesystem behavior.
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


class InstalledUninstall(unittest.TestCase):
    """Each case owns a complete install and observes it through child CLIs."""

    binary = None
    installer = None

    @classmethod
    def setUpClass(cls):
        """Serve immutable current/older release fixtures for all cases.

        The older CLI intentionally has no uninstall command. Updating it
        with the real CLI exercises acquisition of the new functionality.
        """
        cls.release_dir = tempfile.TemporaryDirectory(prefix="farhelm-releases-")
        cls.addClassCleanup(cls.release_dir.cleanup)
        root = Path(cls.release_dir.name)
        version = subprocess.check_output([str(cls.binary), "--version"], text=True).strip()
        if not version.startswith("farhelm "):
            raise AssertionError(f"unexpected compiled CLI version: {version!r}")
        cls.version = version.removeprefix("farhelm ")
        arch = platform.machine()
        if sys.platform == "darwin":
            if arch != "arm64":
                raise RuntimeError("installer supports native arm64 macOS only")
            cls.target = "aarch64-apple-darwin"
        else:
            cls.target = {"x86_64": "x86_64", "aarch64": "aarch64"}[arch] + "-unknown-linux-musl"
        for name, release_version in (("current", cls.version), ("old", "0.0.1")):
            release = root / name
            stage = root / (name + "-stage")
            release.mkdir()
            cli_dir = stage / ("farhelm-" + cls.target)
            cli_dir.mkdir(parents=True)
            cli = cli_dir / "farhelm"
            if name == "current":
                shutil.copy2(cls.binary, cli)
            else:
                cli.write_text("#!/bin/sh\nprintf 'farhelm 0.0.1\\n'\n")
            cli.chmod(0o755)
            with tarfile.open(release / (cli_dir.name + ".tar.gz"), "w:gz") as archive:
                archive.add(cli_dir, arcname=cli_dir.name)
            if sys.platform == "darwin":
                desktop_dir = stage / ("farhelm-desktop-" + cls.target)
                desktop_dir.mkdir()
                desktop = desktop_dir / "farhelm-desktop"
                desktop.write_text(f"#!/bin/sh\nprintf 'desktop fixture {release_version}\\n'\n")
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
        self.home = self.root / "home"
        self.home.mkdir(mode=0o700)
        self.tools = self.root / "tools"
        self.tools.mkdir()
        systemctl = self.tools / "systemctl"
        systemctl.write_text("#!/bin/sh\nprintf 'fixture has no user manager\\n' >&2\nexit 1\n")
        systemctl.chmod(0o755)
        self.env = {
            "HOME": str(self.home),
            "PATH": str(self.tools) + os.pathsep + os.defpath,
            "XDG_CONFIG_HOME": str(self.home / ".config"),
            "XDG_STATE_HOME": str(self.home / ".local/state"),
            "XDG_RUNTIME_DIR": str(self.root / "runtime"),
            "DBUS_SESSION_BUS_ADDRESS": "unix:path=" + str(self.root / "absent-bus"),
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

    def install(self, *, old=False, custom=False, no_bundle=False):
        """Run the actual shell installer and independently verify its CLI bytes."""
        env = dict(self.env, FARHELM_RELEASE_BASE_URL=self.url + ("/old" if old else "/current"),
                   FARHELM_VERSION="0.0.1" if old else self.version)
        if custom:
            self.install_dir = self.home / "custom path 'with quotes'" / "bin"
            env["FARHELM_INSTALL_DIR"] = str(self.install_dir)
        if no_bundle:
            env["FARHELM_NO_APP_BUNDLE"] = "1"
        result = self.run_child(["/bin/sh", str(self.installer)], env=env)
        self.assert_success(result)
        cli = self.install_dir / "farhelm"
        self.assertTrue(cli.is_file())
        self.assertTrue((self.install_dir / ".farhelm-installation").is_file())
        if not old:
            self.assertEqual(hashlib.sha256(cli.read_bytes()).digest(),
                             hashlib.sha256(self.binary.read_bytes()).digest())
        return cli

    def assert_success(self, result):
        """Include both child streams while the failed fixture still exists."""
        self.assertEqual(result.returncode, 0, (result.stdout, result.stderr))

    def assert_removed(self):
        """Removal must preserve user state and the shared executable directory."""
        self.assertFalse((self.install_dir / "farhelm").exists())
        self.assertFalse((self.install_dir / "farhelm-desktop").exists())
        self.assertFalse(self.bundle.exists())
        self.assertTrue(self.install_dir.is_dir())
        self.assertEqual(self.data.read_bytes(), b"retained private fixture data")

    def test_fresh_preview_and_remove(self):
        """Dry-run is read-only; confirmed removal retains data and unrelated files."""
        cli = self.install()
        sentinel = self.install_dir / "unrelated"
        sentinel.write_bytes(b"leave me alone")
        before = snapshot(self.home)
        preview = self.run_child([str(cli), "uninstall", "--dry-run"])
        self.assert_success(preview)
        self.assertIn(str(cli).encode(), preview.stdout + preview.stderr)
        self.assertEqual(snapshot(self.home), before)
        self.assert_success(self.run_child([str(cli), "uninstall", "--yes"]))
        self.assert_removed()
        self.assertEqual(sentinel.read_bytes(), b"leave me alone")

    def test_upgrade_custom_install_then_remove(self):
        """One upgrade supplies uninstall even when the old CLI never supported it."""
        cli = self.install(old=True, custom=True)
        self.assertIn(b"0.0.1", self.run_child([str(cli), "--version"]).stdout)
        cli = self.install(custom=True)
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

    def test_foreign_receipt_link_refuses(self):
        """A receipt symlink never expands the uninstaller's deletion authority."""
        cli = self.install()
        receipt = self.install_dir / ".farhelm-installation"
        target = self.home / "foreign-receipt"
        receipt.rename(target)
        receipt.symlink_to(target)
        before = snapshot(self.home)
        result = self.run_child([str(cli), "uninstall", "--yes"])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(str(receipt).encode(), result.stdout + result.stderr)
        self.assertIn(b"regular file", result.stdout + result.stderr)
        self.assertEqual(snapshot(self.home), before)

    @unittest.skipUnless(sys.platform == "linux", "Linux service fixture")
    def test_partial_service_failure_keeps_cli_for_retry(self):
        """A manager failure after one deletion must stop before removing the CLI.

        This drives the composition through the installed command; helper tests
        alone cannot prove that a service error prevents subsequent file removal.
        """
        cli = self.install()
        units = self.home / ".config/systemd/user"
        units.mkdir(parents=True)
        supervisor = units / "farhelm-supervisor.service"
        helm = units / "farhelm-helm.service"
        for service in [supervisor, helm]:
            service.write_text(f"# managed-by: farhelm helm setup\n[Service]\nExecStart={cli} --version\n")
        target = self.home / "custom-drop-ins"
        target.mkdir()
        sentinel = target / "custom.conf"
        sentinel.write_bytes(b"[Service]\nEnvironment=KEEP=yes\n")
        dropin = units / "farhelm-supervisor.service.d"
        dropin.symlink_to(target, target_is_directory=True)
        failure = self.home / "fail-disable"
        failure.touch()
        manager = self.tools / "systemctl"
        manager.write_text(
            "#!/bin/sh\n"
            "case \"$2\" in\n"
            "show-environment) printf 'HOME=%s\\nXDG_CONFIG_HOME=%s\\n' \"$HOME\" \"$XDG_CONFIG_HOME\";;\n"
            "disable) if [ \"$4\" = farhelm-helm.service ] && [ -f \"$HOME/fail-disable\" ]; then\n"
            "  printf 'fixture refuses helm disable\\n' >&2; exit 1; fi;;\n"
            "daemon-reload) :;;\n"
            "*) printf 'unexpected fixture command\\n' >&2; exit 2;;\n"
            "esac\n")
        self.assertTrue(cli.is_file())
        self.assertTrue(supervisor.is_file())
        self.assertTrue(helm.is_file())
        result = self.run_child([str(cli), "uninstall", "--yes"])
        diagnostic = result.stdout + result.stderr
        self.assertNotEqual(result.returncode, 0, diagnostic)
        self.assertIn(b"Stop local sessions", result.stdout)
        self.assertIn(b"fixture refuses helm disable", diagnostic)
        self.assertIn(b"rerun uninstall", diagnostic)
        self.assertIn(b"removed " + str(supervisor).encode(), diagnostic)
        self.assertIn(str(dropin).encode(), diagnostic)
        self.assertFalse(supervisor.exists())
        self.assertTrue(helm.is_file())
        self.assertTrue(cli.is_file())
        self.assertTrue((self.install_dir / ".farhelm-installation").is_file())
        failure.unlink()
        self.assert_success(self.run_child([str(cli), "uninstall", "--yes"]))
        self.assert_removed()
        self.assertFalse(helm.exists())
        self.assertTrue(dropin.is_symlink())
        self.assertEqual(sentinel.read_bytes(), b"[Service]\nEnvironment=KEEP=yes\n")

    @unittest.skipUnless(sys.platform == "darwin", "native macOS bundle case")
    def test_opt_out(self):
        """An absent opt-out bundle does not prevent flat executable removal."""
        cli = self.install(no_bundle=True)
        self.assertFalse(self.bundle.exists())
        self.assert_success(self.run_child([str(cli), "uninstall", "--yes"]))
        self.assert_removed()

    @unittest.skipUnless(sys.platform == "darwin", "native macOS bundle case")
    def test_missing_flat_desktop(self):
        """An already-removed flat payload must not prevent retrying bundle cleanup."""
        cli = self.install()
        desktop = self.install_dir / "farhelm-desktop"
        self.assertTrue(desktop.is_file())
        self.assertTrue((self.bundle / "Contents/MacOS/farhelm-desktop").is_file())
        desktop.unlink()
        self.assert_success(self.run_child([str(cli), "uninstall", "--yes"]))
        self.assert_removed()

    @unittest.skipUnless(sys.platform == "darwin", "native macOS bundle case")
    def test_bundle_cli_directs_to_flat_retry_command(self):
        """An app-local invocation must not remove the binary needed for retry."""
        cli = self.install()
        app_cli = self.bundle / "Contents/MacOS/farhelm"
        self.assertEqual(hashlib.sha256(cli.read_bytes()).digest(),
                         hashlib.sha256(app_cli.read_bytes()).digest())
        before = snapshot(self.home)
        result = self.run_child([str(app_cli), "uninstall", "--yes"])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(str(cli).encode(), result.stdout + result.stderr)
        self.assertEqual(snapshot(self.home), before)

    @unittest.skipUnless(sys.platform == "darwin", "native macOS bundle case")
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
