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
            "192.168.001.001",
            "169.254.169.254",
            "100.64.0.1",
            "100.127.255.254",
            "[::ffff:169.254.1.1]",
            "[::ffff:100.100.100.100]",
        ):
            for userinfo in ("", "user@", "user:pass@", "user:p@ss@", "user:p,ass@"):
                for suffix in ("/backend-api/codex", "?mode=proxy", "#proxy"):
                    with self.subTest(host=host, userinfo=userinfo, suffix=suffix):
                        text = f"base URL: http://{userinfo}{host}:8080{suffix}"
                        self.assertIsNotNone(scrubber.check(text))
                        clean = scrubber.scrub(text)
                        self.assertEqual(clean, "base URL: http://proxy.invalid" + suffix)
                        self.assertIsNone(scrubber.check(clean))

    def test_link_local_and_tailnet_ranges_apply_only_to_url_authorities(self):
        """Keep addresses adjacent to the private ranges and bare addresses in captured output."""
        scrubber = capture.Scrubber()
        for host in ("100.63.255.255", "100.128.0.0", "169.253.0.1", "169.255.0.1", "192.0.2.1"):
            for literal in (host, f"[::ffff:{host}]"):
                with self.subTest(literal=literal):
                    text = f"https://{literal}/guide"
                    self.assertEqual(scrubber.scrub(text), text)
                    self.assertIsNone(scrubber.check(text))
        for text in ("connected to 169.254.169.254 and 100.64.0.1", "address ://100.64.0.1"):
            with self.subTest(text=text):
                self.assertEqual(scrubber.scrub(text), text)
                self.assertIsNone(scrubber.check(text))

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

    def test_stop_uses_the_owned_fallback_when_socket_creation_fails(self):
        """Control socket exhaustion still needs a host stop before captured evidence can be published."""
        (self.run_dir / "farhelm").write_text(str(self.farhelm))
        run = backend.HostRun(self.run_dir)
        self.enterContext(mock.patch.object(backend.socket, "socket", side_effect=OSError("too many open files")))
        fallback = self.enterContext(mock.patch.object(
            backend.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "")
        ))

        run.stop()

        self.assertEqual(fallback.call_args.args[0][-2:], ["--state-dir", str(self.run_dir)])
        self.assertFalse(self.run_dir.exists())


class CaptureHarnessTests(unittest.TestCase):
    """Keep scrubbed frames when the terminal host fails during a capture or during shutdown."""

    def test_unconfirmed_stop_still_writes_the_captured_screens_and_fails_the_harness(self):
        """A capture spends real vendor turns, so a stop the host cannot confirm must not discard what it captured.

        Specified: the screens staged before the stop are written to the fixture tree, the unconfirmed stop is reported
        among the misses with the host's own message (which names the kept run directory), and the harness is marked
        failed so the whole run exits nonzero."""
        root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        fixtures, work = root / "fixtures", root / "work"
        work.mkdir()

        def drive(term, rec, _work):
            rec.save("idle", "fresh-prompt", "screen text", "title")

        class StandIn:
            def __init__(self, _farhelm):
                pass

            def close(self):
                raise backend.HostError("stop unconfirmed; kept /tmp/fh-run for retry")

        self.enterContext(mock.patch.dict(capture.HARNESSES, {"stub": (["stub", "--version"], drive)}))
        self.enterContext(mock.patch.object(capture, "harness_version", return_value="1.2.3"))
        self.enterContext(mock.patch.object(capture, "Terminal", StandIn))
        self.enterContext(mock.patch.object(capture, "FIXTURES_ROOT", fixtures))
        self.enterContext(mock.patch.object(capture.sys, "stderr", io.StringIO()))
        scrubber = mock.Mock(scrub=lambda text: text, check=lambda _text: None)
        rec = capture.capture_harness("stub", Path("farhelm"), work, scrubber, ui_only=False)
        self.assertEqual(rec.captured, ["idle-fresh-prompt"], "premise: the stand-in drive captured one screen")
        self.assertEqual((fixtures / "stub" / "1.2.3" / "idle-fresh-prompt.txt").read_text(), "screen text")
        self.assertTrue(rec.stop_failed)
        self.assertTrue(any("kept /tmp/fh-run for retry" in miss for miss in rec.missed), rec.missed)

    def test_drive_failure_keeps_frames_and_continues_with_the_next_harness(self):
        """A transport error after the first saved frame fails the command and still publishes both harnesses."""
        root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        fixtures = root / "fixtures"
        farhelm = root / "farhelm"
        farhelm.touch()
        scrubber = mock.Mock(scrub=lambda text: text, check=lambda _text: None)

        def interrupted(_term, rec, _work):
            rec.save("idle", "fresh-prompt", "first screen", "first title")
            self.assertEqual((rec.staging / "idle-fresh-prompt.txt").read_text(), "first screen")
            raise backend.HostError("controller disconnected")

        def complete(_term, rec, _work):
            rec.save("idle", "fresh-prompt", "second screen", "second title")

        harnesses = {"first": ([], interrupted), "second": ([], complete)}
        self.enterContext(mock.patch.object(capture, "HARNESSES", harnesses))
        self.enterContext(mock.patch.object(capture, "harness_version", return_value="1.2.3"))
        terminal = self.enterContext(mock.patch.object(capture, "Terminal"))
        self.enterContext(mock.patch.object(capture, "FIXTURES_ROOT", fixtures))
        self.enterContext(mock.patch.object(capture.Scrubber, "for_host", return_value=scrubber))
        self.enterContext(mock.patch.object(capture.sys, "stderr", io.StringIO()))
        stdout = self.enterContext(mock.patch.object(capture.sys, "stdout", io.StringIO()))

        self.assertEqual(capture.main(["--farhelm", str(farhelm), "--no-test"]), 1)

        self.assertEqual(terminal.return_value.close.call_count, 2)
        self.assertIn("terminal host drive failed: controller disconnected", stdout.getvalue())
        for harness in harnesses:
            pair = fixtures / harness / "1.2.3"
            self.assertEqual((pair / "idle-fresh-prompt.txt").read_text(), f"{harness} screen")
            self.assertEqual((pair / "idle-fresh-prompt.title").read_text(), f"{harness} title\n")


if __name__ == "__main__":
    unittest.main()
