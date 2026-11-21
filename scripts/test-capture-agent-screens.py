#!/usr/bin/env python3
"""Exercise capture privacy and terminal host ownership using synthetic screens and stand-ins."""

import importlib.util
import io
import subprocess
import sys
import tempfile
import unittest
from contextlib import chdir
from pathlib import Path
from unittest import mock

import agent_screen_host as backend

_spec = importlib.util.spec_from_file_location(
    "capture_agent_screens", Path(__file__).with_name("capture-agent-screens.py")
)
capture = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = capture
_spec.loader.exec_module(capture)


class ScrubberTests(unittest.TestCase):
    """A local URL is private even when userinfo precedes its host; public links still belong in a fixture."""

    def test_local_urls_with_userinfo_are_replaced_and_refused(self):
        """The replacement removes credentials, and the backstop refuses each unsanitized authority independently."""
        scrubber = capture.Scrubber()
        for host in (
            "localhost",
            "[::1]",
            "[::]",
            "[fd00::1]",
            "[fe80::1%25eth0]",
            "[::ffff:192.168.1.4]",
            "127.0.0.1",
            "10.0.0.2",
        ):
            for userinfo in ("", "user@", "user:pass@", "user:p@ss@", "user:p,ass@"):
                for suffix in ("/backend-api/codex", "?mode=proxy", "#proxy"):
                    with self.subTest(host=host, userinfo=userinfo, suffix=suffix):
                        text = f"base URL: http://{userinfo}{host}:8080{suffix}"
                        self.assertIsNotNone(scrubber.check(text))
                        clean = scrubber.scrub(text)
                        self.assertEqual(clean, "base URL: http://proxy.invalid" + suffix)
                        self.assertIsNone(scrubber.check(clean))

    def test_public_hosts_are_not_changed_by_at_signs_outside_the_authority(self):
        """A path or query naming a local host must not make an ordinary public documentation link private."""
        scrubber = capture.Scrubber()
        for text in (
            "https://docs.example.com/user@localhost",
            "https://docs.example.com/?next=user@localhost",
            "https://[2001:db8::1]/user@localhost",
            "https://localhost.example.com/help",
            "https://localhost:8080,user@example.com/guide",
            "https://localhost:8080,user@example.com?view=guide",
        ):
            with self.subTest(text=text):
                self.assertEqual(scrubber.scrub(text), text)
                self.assertIsNone(scrubber.check(text))

    def test_comma_can_separate_a_local_url_from_prose(self):
        """The adjacent address stays readable even though the proxy authority has to go."""
        scrubber = capture.Scrubber()
        for suffix in ("", "?", "#"):
            text = "http://localhost:8080,user@example.com" + suffix
            self.assertIsNotNone(scrubber.check(text))
            self.assertEqual(scrubber.scrub(text), "http://proxy.invalid,user@example.com" + suffix)

    def test_quoted_local_authority_is_redacted_inside_the_quotes(self):
        """Configuration syntax can surround a local URL with quotes and terminate it before any path."""
        scrubber = capture.Scrubber()
        for host in ("127.0.0.1.", "[fd00::1]"):
            text = f'"http://user:p@ss@{host}:8080"'
            self.assertIsNotNone(scrubber.check(text))
            self.assertEqual(scrubber.scrub(text), '"http://proxy.invalid"')

    def test_complete_private_authority_wins_over_punctuation_prefix(self):
        """A privacy scrub removes all of the credentials even when their prefix resembles an adjacent URL."""
        scrubber = capture.Scrubber()
        for suffix in ("/guide", ",user@example.com"):
            text = "http://localhost:123,p@ss@localhost:8080" + suffix
            self.assertIsNotNone(scrubber.check(text))
            clean = scrubber.scrub(text)
            self.assertEqual(clean, "http://proxy.invalid" + suffix)
            self.assertIsNone(scrubber.check(clean))


class HostRunTests(unittest.TestCase):
    """Pin the binary and controller owned by a capture across startup and shutdown failures."""

    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.run_dir = self.root / "run"
        self.run_dir.mkdir()
        self.farhelm = self.root / "farhelm"
        self.farhelm.touch()

    def stand_in_for_controller(self):
        """Supply a detached child that has reached the real readiness boundary without running Farhelm."""
        child = mock.Mock(spec=subprocess.Popen)
        child.stdout = io.BytesIO(b"ready\n")
        child.pid = 123
        child.wait.return_value = 0
        self.enterContext(mock.patch.object(backend.tempfile, "mkdtemp", return_value=str(self.run_dir)))
        self.enterContext(mock.patch.object(backend.select, "select", return_value=([child.stdout], [], [])))
        launch = self.enterContext(mock.patch.object(backend.subprocess, "Popen", return_value=child))
        return child, launch

    def test_the_checked_binary_is_launched_even_for_a_bare_relative_path(self):
        """The executable marker preserves the path exactly, including a trailing space in its file name."""
        child, launch = self.stand_in_for_controller()
        farhelm = self.farhelm.rename(self.root / "farhelm ")
        with chdir(self.root):
            run = backend.HostRun.start(Path("./farhelm "), self.root, 80, 24, ["fake-agent"])
        launched = launch.call_args.args[0][0]
        self.assertEqual(launched, str(farhelm.resolve()))
        self.assertEqual(run.farhelm, launched)
        self.assertIs(run.process, child)
        self.assertEqual((self.run_dir / "pid").read_text().strip(), str(child.pid))

    def test_pid_write_failure_stops_the_child_before_start_returns(self):
        """The caller has no run handle if metadata fails after readiness. Startup still owns that child."""
        child, _ = self.stand_in_for_controller()
        write_text = Path.write_text

        def write_except_pid(path, *args, **kwargs):
            if path == self.run_dir / "pid":
                raise PermissionError("pid metadata is unwritable")
            return write_text(path, *args, **kwargs)

        self.enterContext(mock.patch.object(Path, "write_text", autospec=True, side_effect=write_except_pid))
        request = self.enterContext(mock.patch.object(backend.HostRun, "request"))
        fallback = self.enterContext(mock.patch.object(
            backend.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "")
        ))
        with self.assertRaisesRegex(PermissionError, "pid metadata is unwritable"):
            backend.HostRun.start(self.farhelm, self.root, 80, 24, ["fake-agent"])
        self.assertTrue(child.stdout.closed)
        request.assert_called_once_with({"op": "stop"})
        child.wait.assert_called_once_with(timeout=backend.STOP_SECONDS)
        self.assertEqual(fallback.call_args.args[0][-2:], ["--state-dir", str(self.run_dir)])
        self.assertFalse(self.run_dir.exists())

    def test_interrupted_spawn_keeps_the_run_when_the_handle_is_unknown(self):
        """A Ctrl+C during Popen leaves no child handle, but the detached process might already exist."""
        _, launch = self.stand_in_for_controller()
        launch.side_effect = KeyboardInterrupt
        with self.assertRaisesRegex(backend.HostError, "launch interrupted") as failure:
            backend.HostRun.start(self.farhelm, self.root, 80, 24, ["fake-agent"])
        self.assertIn(str(self.run_dir), str(failure.exception))
        self.assertEqual(backend.HostRun(self.run_dir).farhelm, str(self.farhelm))

    def test_unconfirmed_stop_keeps_the_run_for_a_retry(self):
        """Only a successful stop of the terminal hosts permits removing their sockets and diagnostics."""
        (self.run_dir / "farhelm").write_text(str(self.farhelm))
        log = self.run_dir / "log"
        log.write_text("retained controller log")
        run = backend.HostRun(self.run_dir)
        self.enterContext(mock.patch.object(run, "request", side_effect=backend.HostError("controller exited")))
        self.enterContext(mock.patch.object(backend.subprocess, "run", side_effect=[
            subprocess.TimeoutExpired("fake-stop", 60, output=b"controller stop is still pending\n"),
            subprocess.CompletedProcess([], 1, "host still reachable"),
            subprocess.CompletedProcess([], 0, ""),
        ]))
        for reason in ("controller stop is still pending", "host still reachable"):
            with self.assertRaisesRegex(backend.HostError, reason) as failure:
                run.stop()
            self.assertIn(str(self.run_dir), str(failure.exception))
            self.assertEqual(log.read_text(), "retained controller log")
        run.stop()
        self.assertFalse(self.run_dir.exists())


if __name__ == "__main__":
    unittest.main()
