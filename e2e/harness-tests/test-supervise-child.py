#!/usr/bin/env python3
"""Focused contracts for the browser child supervisor, independent of browser startup."""

import importlib.util
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock


SCRIPT = Path(__file__).with_name("supervise-child.py")
SPEC = importlib.util.spec_from_file_location("browser_child_supervisor", SCRIPT)
SUPERVISOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SUPERVISOR)


class SupervisionContracts(unittest.TestCase):
    """Exercise ownership and termination through real children with independently bounded lives."""

    def launch(self, code, *args, timeout=3):
        """Hold the cancellation lease open until the test deliberately releases it."""
        return subprocess.Popen(
            [sys.executable, str(SCRIPT), f"--timeout={timeout}", "--", sys.executable, "-c", code, *args],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def finish(self, child):
        """Reap the supervisor while leaving stdin open, so a normal exit stays normal.

        Fixtures emit only tiny fixed records, below pipe capacity. Their own finite
        lifetimes provide fallback cleanup even if the supervisor under test breaks;
        this fixture never signals a remembered PID after reaping it.
        """
        try:
            child.wait(timeout=12)
            return child.returncode, child.stdout.read(4096), child.stderr.read(4096)
        finally:
            child.stdin.close()
            child.stdout.close()
            child.stderr.close()

    def test_normal_exit_is_preserved(self):
        """A nonzero CLI result remains distinguishable from supervisor cancellation."""
        code, output, _ = self.finish(self.launch("print('result'); raise SystemExit(7)"))
        self.assertEqual(code, 7)
        self.assertEqual(output, b"result\n")

    def test_deadline_escalates_past_ignored_term(self):
        """An uncooperative CLI cannot turn a supervisor timeout into a normal test failure."""
        child = self.launch("import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); print('term ignored', flush=True); time.sleep(8)", timeout=2)
        code, output, _ = self.finish(child)
        self.assertEqual(code, 124)
        self.assertEqual(output, b"term ignored\n")

    def test_parent_eof_cancels(self):
        """The parent's death/closed lease reaches cleanup without a numeric signal from Node."""
        child = self.launch("import time; time.sleep(8)")
        child.stdin.close()
        code, _, _ = self.finish(child)
        self.assertEqual(code, 124)

    def test_signal_exit_is_not_playwright_failure(self):
        """An abort after output must not pass the parent's exact exit-code-1 contract."""
        child = self.launch("import os,signal; print('valid-looking report', flush=True); os.kill(os.getpid(), signal.SIGKILL)")
        code, output, _ = self.finish(child)
        self.assertEqual(code, 128 + signal.SIGKILL)
        self.assertIn(b"valid-looking report", output)

    def test_lost_wait_ownership_refuses_signals(self):
        """A stale child-list PID cannot authorize either a group or a direct signal."""
        with mock.patch.object(SUPERVISOR.os, "waitid", side_effect=ChildProcessError):
            with mock.patch.object(SUPERVISOR.os, "kill") as kill, mock.patch.object(SUPERVISOR.os, "killpg") as killpg:
                for group in (False, True):
                    with self.assertRaises(ChildProcessError):
                        SUPERVISOR.signal_owned(123, signal.SIGKILL, group=group)
                kill.assert_not_called()
                killpg.assert_not_called()

    @unittest.skipUnless(sys.platform == "linux", "detached orphan ownership requires Linux subreaping")
    def test_detached_descendant_is_dead_and_reaped_before_return(self):
        """A browser-shaped detached process cannot survive its CLI's normal exit.

        A pidfd is opened while the fixture is held at a release-file handshake.
        Death is observed through that stable descriptor, and procfs absence after
        supervisor completion separately checks reaping. No numeric fallback signal
        is used: the fixture's eight-second lifetime bounds a broken implementation.
        """
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            code = """
import os, pathlib, signal, sys, time
root = pathlib.Path(sys.argv[1])
pid = os.fork()
if pid == 0:
    os.setsid()
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    (root / 'pid.tmp').write_text(str(os.getpid()))
    (root / 'pid.tmp').replace(root / 'pid')
    time.sleep(8)
    os._exit(0)
deadline = time.monotonic() + 8
while not (root / 'release').exists() and time.monotonic() < deadline:
    time.sleep(0.01)
raise SystemExit(7)
"""
            child = self.launch(code, directory)
            pidfd = None
            try:
                deadline = time.monotonic() + 2
                while not (root / "pid").exists():
                    self.assertLess(time.monotonic(), deadline, "descendant did not reach handshake")
                    time.sleep(0.01)
                pid = int((root / "pid").read_text())
                pidfd = os.pidfd_open(pid)
                self.assertEqual(select.select([pidfd], [], [], 0)[0], [])
                (root / "release").touch()
                status, _, stderr = self.finish(child)
                self.assertEqual(status, 7, stderr)
                self.assertEqual(select.select([pidfd], [], [], 0)[0], [pidfd])
                self.assertFalse(Path(f"/proc/{pid}").exists(), "owned orphan was not reaped")
            finally:
                if child.returncode is None:
                    child.stdin.close()
                    self.finish(child)
                if pidfd is not None:
                    os.close(pidfd)


if __name__ == "__main__":
    unittest.main()
