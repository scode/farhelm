#!/usr/bin/env python3
"""Subprocess tests for the run recorder's evidence and lifecycle contracts."""

from __future__ import annotations

import base64
import hashlib
import importlib.util
import json
import os
import pathlib
import select
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from concurrent.futures import ThreadPoolExecutor
from unittest import mock


SCRIPT = pathlib.Path(__file__).with_name("record-test-run.py").resolve()
PYTHON = sys.executable
POLL_SECONDS = 0.05
READY_TIMEOUT = 10.0


def load_recorder():
    """Load injectable seams for races that cannot be forced reliably through the CLI.

    The CLI fixtures below still drive real recording runs. These additional
    tests control only process readiness or first-directory creation, leaving
    the recorder's lifecycle and filesystem operations under test.
    """

    spec = importlib.util.spec_from_file_location("farhelm_recorder_fixture", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    # Executing compiled source avoids writing __pycache__ into the checkout
    # whose source identity these tests are meant to leave alone.
    exec(compile(SCRIPT.read_bytes(), str(SCRIPT), "exec"), module.__dict__)
    return module


RECORDER = load_recorder()


def run_checked(argv: list[str], cwd: pathlib.Path) -> None:
    """Run fixture setup commands and fail with their diagnostics."""

    subprocess.run(argv, cwd=cwd, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def wait_for(predicate, timeout: float = READY_TIMEOUT) -> None:
    """Wait for an explicit fixture condition instead of relying on a timing race."""

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(POLL_SECONDS)
    raise AssertionError("fixture readiness condition was not met before the deadline")


def process_is_alive(pid: int) -> bool:
    """Inspect status without signaling a remembered PID; zombies hold no live pipes.

    This is a status observation, not authority to signal or an instance identity
    guarantee. Fixture cleanup uses its independent private lifetime file.
    """

    proc_stat = pathlib.Path(f"/proc/{pid}/stat")
    if pathlib.Path("/proc/self/stat").exists():
        try:
            # comm may itself contain spaces or parentheses; state follows its
            # last closing parenthesis rather than the third whitespace token.
            return proc_stat.read_text().rsplit(")", 1)[1].split()[0] != "Z"
        except FileNotFoundError:
            return False
        except (OSError, IndexError):
            return True
    status = subprocess.run(
        ["ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True, check=False
    ).stdout.strip()
    return bool(status) and not status.startswith("Z")


def release_fixture_processes(pid_path: pathlib.Path) -> None:
    """Release the fixture's private lifetime file without signaling a remembered PID.

    The recorder may already have reaped a leader by the time fallback cleanup
    runs. A PID file cannot keep that group's identity reserved. Surviving fixture
    processes instead wait on this separate lease, which also covers deliberately
    escaped descendants that the recorder never promised to own.
    """

    (pid_path.parent / "fixture-stop").touch()
    if not pid_path.exists():
        return
    try:
        pid = int(pid_path.read_text())
    except (OSError, ValueError):
        return
    wait_for(lambda: not process_is_alive(pid))


def cleanup_direct_child(process: subprocess.Popen) -> None:
    """Use an unreaped direct child as the only authority for fallback group signals."""

    if process.returncode is not None:
        return
    try:
        os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    except ChildProcessError:
        return
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait(timeout=READY_TIMEOUT)


class RecorderTest(unittest.TestCase):
    """Drive the real CLI inside isolated Git and process fixtures."""

    def setUp(self) -> None:
        """Build a small committed repository and a private sibling evidence root."""

        self.temporary = tempfile.TemporaryDirectory(prefix="record-test-run-")
        self.base = pathlib.Path(self.temporary.name)
        self.stop_path = self.base / "fixture-stop"
        self.waiter = self.base / "wait-for-cleanup.py"
        self.waiter.write_text(
            "import pathlib, time\n"
            f"stop = pathlib.Path({str(self.stop_path)!r})\n"
            "deadline = time.monotonic() + 60\n"
            "while stop.parent.exists() and not stop.exists() and time.monotonic() < deadline:\n"
            "    time.sleep(0.05)\n",
            encoding="utf-8",
        )
        self.wait_code = f"__import__('runpy').run_path({str(self.waiter)!r})"
        self.repo = self.base / "repo"
        self.repo.mkdir()
        self.evidence = self.base / "evidence"
        self.evidence.mkdir(mode=0o700)
        os.chmod(self.evidence, 0o700)
        run_checked(["git", "init", "--quiet"], self.repo)
        run_checked(["git", "config", "user.email", "fixture@example.invalid"], self.repo)
        run_checked(["git", "config", "user.name", "Recorder Fixture"], self.repo)
        (self.repo / "tracked.txt").write_text("original\n", encoding="utf-8")
        run_checked(["git", "add", "tracked.txt"], self.repo)
        run_checked(["git", "commit", "--quiet", "-m", "fixture"], self.repo)

    def tearDown(self) -> None:
        """Remove the fixture only after every owned background job is gone."""

        self.stop_path.touch()
        for pid_path in self.base.glob("*.pid"):
            release_fixture_processes(pid_path)
        self.temporary.cleanup()

    def environment(self, **updates: str) -> dict[str, str]:
        """Return an explicit child environment without mutating this test process."""

        environment = {key: value for key, value in os.environ.items() if not key.startswith("FARHELM_")}
        environment.update(updates)
        return environment

    def test_exit_observation_keeps_group_owned_until_final_wait(self) -> None:
        """A dead leader must stay waitable while a descendant still holds its pipe.

        Repeated observation proves the leader was not reaped. Killing its
        pinned group must release the inherited pipe, preserve the leader's
        original status, and disarm every later signal after the explicit wait.
        """

        descendant_pid = self.base / "owned-pipe-descendant.pid"
        process = subprocess.Popen(
            [PYTHON, "-c", "import pathlib, subprocess, sys; "
             f"descendant = subprocess.Popen([sys.executable, {str(self.waiter)!r}]); "
             f"pathlib.Path({str(descendant_pid)!r}).write_text(str(descendant.pid)); "
             "sys.exit(7)"],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        try:
            wait_for(lambda: RECORDER.observe_exit(process) is not None)
            self.assertEqual(RECORDER.observe_exit(process), 7)
            self.assertIsNone(process.returncode)
            status = os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
            self.assertIsNotNone(status)
            self.assertEqual(status.si_status, 7)
            self.assertFalse(select.select([process.stdout], [], [], 0)[0])
            self.assertTrue(RECORDER.terminate_group(process, signal.SIGKILL))
            self.assertEqual(process.wait(timeout=READY_TIMEOUT), 7)
            self.assertTrue(select.select([process.stdout], [], [], READY_TIMEOUT)[0])
            self.assertEqual(os.read(process.stdout.fileno(), 1), b"")
            with mock.patch.object(RECORDER.os, "killpg") as kill_group:
                self.assertFalse(RECORDER.terminate_group(process, signal.SIGKILL))
                self.assertFalse(RECORDER.process_group_exists(process))
                kill_group.assert_not_called()
        finally:
            # The helper itself is under test; use the still-waitable child as
            # the fallback ownership proof rather than an already reaped ID.
            cleanup_direct_child(process)
            release_fixture_processes(descendant_pid)
            process.stdout.close()

    def test_lost_wait_ownership_refuses_group_signals(self) -> None:
        """Even signal zero must refuse an ID after another waiter consumed the child."""

        process = mock.Mock(pid=12345, returncode=None)
        with mock.patch.object(RECORDER.os, "waitid", side_effect=ChildProcessError), \
                mock.patch.object(RECORDER.os, "killpg") as kill_group:
            for signum in (0, signal.SIGTERM, signal.SIGKILL):
                with self.subTest(signum=signum), self.assertRaises(RECORDER.RecorderFailure):
                    RECORDER.terminate_group(process, signum)
            kill_group.assert_not_called()

    def test_exit_observation_leaves_interrupts_to_outer_deadline(self) -> None:
        """Interrupted waitid calls must not create an unbounded retry loop inside observation."""

        process = mock.Mock(pid=12345, returncode=None)
        with mock.patch.object(RECORDER.os, "waitid", side_effect=InterruptedError) as waitid:
            self.assertIsNone(RECORDER.observe_exit(process))
            waitid.assert_called_once()

    def test_lost_wait_status_remains_unknown_in_command_evidence(self) -> None:
        """A different waiter must not turn a real exit seven into Popen's synthetic zero."""

        process = subprocess.Popen(
            [PYTHON, "-c", "raise SystemExit(7)"], stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, start_new_session=True,
        )
        try:
            wait_for(lambda: RECORDER.observe_exit(process) is not None)
            waited_pid, status = os.waitpid(process.pid, 0)
            self.assertEqual(waited_pid, process.pid)
            self.assertEqual(os.waitstatus_to_exitcode(status), 7)
            self.assertIsNone(process.returncode)
            with mock.patch.object(RECORDER.subprocess, "Popen", return_value=process), \
                    mock.patch.object(RECORDER.os, "killpg") as kill_group, \
                    mock.patch.object(process, "wait", side_effect=AssertionError("status was consumed")):
                result = RECORDER.run_command(
                    ["fixture"], self.repo, {}, 1, RECORDER.SignalIntent(), mock.Mock(), mock.Mock(),
                )
                self.assertEqual(result.outcome, "recorder-error")
                self.assertEqual(result.recorder_exit, 125)
                self.assertIsNone(result.child_returncode)
                self.assertIsNone(RECORDER.child_status(result.child_returncode)["exit_code"])
                self.assertIn("wait ownership was lost", result.error)
                kill_group.assert_not_called()
        finally:
            # This test, acting as the competing waiter, knows the consumed status.
            # Suppress Popen's later destructor bookkeeping without another wait.
            process.returncode = 7
            process.stdout.close()

    def test_missing_wait_api_refuses_before_any_spawn(self) -> None:
        """Unsupported Python builds fail before launching a probe they cannot safely own."""

        waitid = RECORDER.os.waitid
        with mock.patch.object(RECORDER.subprocess, "Popen") as spawn:
            try:
                del RECORDER.os.waitid
                code = RECORDER.run([
                    "--kind", "development", "--selection", "fixture", "--concurrency", "one",
                    "--tmux", "none", "--output-root", str(self.evidence), "--", PYTHON, "-c", "pass",
                ])
            finally:
                RECORDER.os.waitid = waitid
            self.assertEqual(code, 125)
            spawn.assert_not_called()

    def test_final_reap_cannot_replace_an_observed_failure_with_zero(self) -> None:
        """Lost status at the final boundary stays an error in both lifecycle paths.

        A competing waiter consumes exit seven only after the recorder has seen
        that real status. Popen.wait would silently return zero at this boundary;
        the owned reap must expose the loss without signaling an unowned group.
        """

        reap = RECORDER.reap_owned_child
        for phase in ("command", "probe"):
            with self.subTest(phase=phase):
                process = subprocess.Popen(
                    [PYTHON, "-c", "print('retained probe output'); raise SystemExit(7)"], stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE if phase == "probe" else subprocess.STDOUT,
                    start_new_session=True,
                )
                consumed = False

                def consume_before_reap(child, timeout):
                    nonlocal consumed
                    self.assertEqual(RECORDER.observe_exit(child), 7)
                    pid, status = os.waitpid(child.pid, 0)
                    self.assertEqual(pid, child.pid)
                    self.assertEqual(os.waitstatus_to_exitcode(status), 7)
                    consumed = True
                    return reap(child, timeout)

                try:
                    wait_for(lambda: RECORDER.observe_exit(process) == 7)
                    with mock.patch.object(RECORDER.subprocess, "Popen", return_value=process), \
                            mock.patch.object(RECORDER, "reap_owned_child", side_effect=consume_before_reap), \
                            mock.patch.object(RECORDER.os, "killpg") as kill_group:
                        if phase == "probe":
                            result = RECORDER.bounded_probe(
                                ["fixture"], cwd=self.repo, env={}, intent=RECORDER.SignalIntent(),
                            )
                            self.assertEqual(result.returncode, 7)
                            self.assertEqual(result.stdout_sample, b"retained probe output\n")
                            self.assertEqual(result.stdout_sha256, hashlib.sha256(result.stdout_sample).hexdigest())
                            self.assertIn("ownership", result.lifecycle_error)
                            self.assertFalse(result.complete)
                        else:
                            result = RECORDER.run_command(
                                ["fixture"], self.repo, {}, 1, RECORDER.SignalIntent(),
                                mock.Mock(), mock.Mock(),
                            )
                            self.assertEqual(result.recorder_exit, 125)
                            self.assertEqual(result.outcome, "recorder-error")
                            self.assertEqual(result.child_returncode, 7)
                        self.assertTrue(consumed)
                        kill_group.assert_not_called()
                finally:
                    if consumed:
                        process.returncode = 7
                    else:
                        cleanup_direct_child(process)
                    process.stdout.close()
                    if process.stderr is not None:
                        process.stderr.close()

    def test_later_probe_ownership_loss_retains_source_before_refusing_command(self) -> None:
        """A failed later metadata lifecycle cannot erase earlier source identity or run the command."""

        (self.repo / "tracked.txt").write_text("changed fixture\n", encoding="utf-8")
        marker = self.base / "command-must-not-run"
        reap = RECORDER.reap_owned_child
        consumed = []

        def consume_status_before_reap(process, timeout):
            if process.args[:2] == ["git", "status"]:
                self.assertEqual(RECORDER.observe_exit(process), 0)
                pid, status = os.waitpid(process.pid, 0)
                self.assertEqual(pid, process.pid)
                self.assertEqual(os.waitstatus_to_exitcode(status), 0)
                consumed.append(process)
            return reap(process, timeout)

        try:
            with mock.patch.object(RECORDER.pathlib.Path, "cwd", return_value=self.repo), \
                    mock.patch.object(RECORDER, "reap_owned_child", side_effect=consume_status_before_reap):
                code = RECORDER.run([
                    "--kind", "development", "--selection", "fixture", "--concurrency", "one",
                    "--tmux", "none", "--output-root", str(self.evidence), "--", PYTHON, "-c",
                    f"import pathlib; pathlib.Path({str(marker)!r}).touch()",
                ])
            self.assertEqual(code, 125)
            self.assertEqual(len(consumed), 1)
            self.assertFalse(marker.exists())
            manifest = json.loads(next(self.evidence.glob("*/manifest.json")).read_text())
            self.assertEqual(manifest["outcome"], "recorder-error")
            source = manifest["source"]
            self.assertFalse(source["complete"])
            self.assertFalse(source["lifecycle_complete"])
            self.assertEqual(len(source["head"]), 40)
            self.assertTrue(source["tracked_diff"]["complete"])
            self.assertGreater(source["tracked_diff"]["bytes"], 0)
            probe = source["porcelain"]["probe"]
            self.assertIn("ownership", probe["lifecycle_error"])
            self.assertGreater(source["porcelain"]["bytes"], 0)
            self.assertTrue(source["untracked_tree"]["complete"])
        finally:
            for process in consumed:
                process.returncode = 0

    def test_interrupted_metadata_ownership_loss_retains_probe_and_error_status(self) -> None:
        """Interruption cannot hide an incomplete lifecycle, even during checkout discovery.

        Consume the real exit at the final reap seam and deliver interruption
        there. The manifest must retain both observations before refusing the
        command; no later probe should start just to fill out metadata.
        """

        (self.repo / "tracked.txt").write_text("changed fixture\n", encoding="utf-8")
        probe = RECORDER.bounded_probe
        reap = RECORDER.reap_owned_child
        popen = RECORDER.subprocess.Popen
        for phase in ("initial", "later"):
            with self.subTest(phase=phase):
                consumed = []
                active_intent = None
                spawns_after_interrupt = []
                root = self.base / f"interrupted-{phase}"

                def remember_intent(argv, **kwargs):
                    nonlocal active_intent
                    active_intent = kwargs["intent"]
                    return probe(argv, **kwargs)

                def record_spawn(argv, **kwargs):
                    if active_intent is not None and active_intent.received is not None:
                        spawns_after_interrupt.append(argv)
                    return popen(argv, **kwargs)

                def consume_and_interrupt(process, timeout):
                    selected = (
                        process.args == ["git", "rev-parse", "--show-toplevel"]
                        if phase == "initial" else process.args[:2] == ["git", "status"]
                    )
                    if selected:
                        self.assertEqual(RECORDER.observe_exit(process), 0)
                        pid, status = os.waitpid(process.pid, 0)
                        self.assertEqual(pid, process.pid)
                        self.assertEqual(os.waitstatus_to_exitcode(status), 0)
                        consumed.append(process)
                        active_intent.handle(signal.SIGINT, None)
                    return reap(process, timeout)

                try:
                    with mock.patch.object(RECORDER.pathlib.Path, "cwd", return_value=self.repo), \
                            mock.patch.object(RECORDER, "bounded_probe", side_effect=remember_intent), \
                            mock.patch.object(RECORDER.subprocess, "Popen", side_effect=record_spawn), \
                            mock.patch.object(RECORDER, "reap_owned_child", side_effect=consume_and_interrupt), \
                            mock.patch.object(RECORDER, "run_command") as command:
                        code = RECORDER.run([
                            "--kind", "development", "--selection", "fixture", "--concurrency", "one",
                            "--tmux", "none", "--output-root", str(root), "--", PYTHON, "-c", "pass",
                        ])
                    self.assertEqual(code, 125)
                    self.assertEqual(len(consumed), 1)
                    self.assertEqual(spawns_after_interrupt, [])
                    command.assert_not_called()
                    manifest = json.loads(next(root.glob("*/manifest.json")).read_text())
                    self.assertEqual(manifest["outcome"], "recorder-error")
                    source = manifest["source"]
                    self.assertFalse(source["lifecycle_complete"])
                    retained = source["git_top_level_probe"] if phase == "initial" else source["porcelain"]["probe"]
                    self.assertTrue(retained["interrupted"])
                    self.assertIn("ownership", retained["lifecycle_error"])
                    self.assertEqual(retained["returncode"], 0)
                    self.assertGreater(retained["stdout"]["bytes"], 0)
                    if phase == "later":
                        self.assertEqual(len(source["head"]), 40)
                        self.assertTrue(source["tracked_diff"]["complete"])
                finally:
                    for process in consumed:
                        process.returncode = 0

    def test_probe_consumer_failure_preserves_output_and_still_reaps_after_signal_error(self) -> None:
        """An output consumer and cleanup signal can fail without erasing status or evidence.

        The real child is already waitable, so a failed signal does not prevent
        reaping it. Captured bytes precede consumer delivery and must survive
        both errors; no retry or fallback cleanup is needed on the passing path.
        """

        process = subprocess.Popen(
            [PYTHON, "-c", "print('partial source'); raise SystemExit(7)"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
        )
        try:
            wait_for(lambda: RECORDER.observe_exit(process) == 7)
            with mock.patch.object(RECORDER.subprocess, "Popen", return_value=process), \
                    mock.patch.object(RECORDER, "terminate_group", side_effect=OSError("signal fixture")) as terminate:
                result = RECORDER.bounded_probe(
                    ["fixture"], cwd=self.repo, env={}, intent=RECORDER.SignalIntent(),
                    stdout_consumer=mock.Mock(side_effect=ValueError("consumer fixture")),
                )
            self.assertEqual(result.returncode, 7)
            self.assertEqual(process.returncode, 7)
            self.assertEqual(result.stdout_sample, b"partial source\n")
            self.assertEqual(result.stdout_sha256, hashlib.sha256(result.stdout_sample).hexdigest())
            self.assertIn("consumer fixture", result.lifecycle_error)
            self.assertIn("signal fixture", result.lifecycle_error)
            self.assertFalse(result.complete)
            terminate.assert_called_once_with(process, signal.SIGKILL)
            self.assertTrue(process.stdout.closed)
            self.assertTrue(process.stderr.closed)
        finally:
            cleanup_direct_child(process)
            process.stdout.close()
            process.stderr.close()

    def test_probe_close_failures_preserve_evidence_and_attempt_every_resource(self) -> None:
        """One failing finalizer cannot erase output or prevent the other owned closes.

        Wrappers close their actual descriptors before raising, so the fixture
        exercises error accumulation without relying on leaked resources. The
        consumer fails before normal EOF closure reaches either pipe.
        """

        closed = []

        class FailingClose:
            """Delegate real I/O while making each close report an observable failure."""

            def __init__(self, resource, name):
                self.resource = resource
                self.name = name

            def __getattr__(self, name):
                return getattr(self.resource, name)

            def close(self):
                closed.append(self.name)
                self.resource.close()
                raise OSError(f"{self.name} close fixture")

        process = subprocess.Popen(
            [PYTHON, "-c", "print('retained before close'); raise SystemExit(7)"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
        )
        stdout, stderr = process.stdout, process.stderr
        selector = RECORDER.selectors.DefaultSelector()
        process.stdout = FailingClose(stdout, "stdout")
        process.stderr = FailingClose(stderr, "stderr")
        try:
            wait_for(lambda: RECORDER.observe_exit(process) == 7)
            with mock.patch.object(RECORDER.subprocess, "Popen", return_value=process), \
                    mock.patch.object(RECORDER.selectors, "DefaultSelector", return_value=FailingClose(selector, "selector")):
                result = RECORDER.bounded_probe(
                    ["fixture"], cwd=self.repo, env={}, intent=RECORDER.SignalIntent(),
                    stdout_consumer=mock.Mock(side_effect=ValueError("consumer fixture")),
                )
            self.assertEqual(closed, ["selector", "stdout", "stderr"])
            self.assertEqual(result.returncode, 7)
            self.assertEqual(process.returncode, 7)
            self.assertEqual(result.stdout_sample, b"retained before close\n")
            self.assertEqual(result.stdout_sha256, hashlib.sha256(result.stdout_sample).hexdigest())
            for fragment in ("consumer fixture", "selector close fixture", "stdout close fixture", "stderr close fixture"):
                self.assertIn(fragment, result.lifecycle_error)
            self.assertFalse(result.complete)
            self.assertTrue(stdout.closed)
            self.assertTrue(stderr.closed)
        finally:
            cleanup_direct_child(process)
            selector.close()
            stdout.close()
            stderr.close()

    def test_probe_pipe_truncation_survives_final_ownership_loss(self) -> None:
        """Closing inherited pipes is still evidence loss when a later reap also fails.

        Held writers model an escaped descendant without spawning one. A finite
        clock drives the post-exit drain boundary before ownership disappears.
        """

        readers = []
        writers = []
        for _ in range(2):
            read_fd, write_fd = os.pipe()
            readers.append(os.fdopen(read_fd, "rb", buffering=0))
            writers.append(write_fd)
        process = mock.Mock(pid=12345, returncode=None, stdout=readers[0], stderr=readers[1])
        try:
            with mock.patch.object(RECORDER.subprocess, "Popen", return_value=process), \
                    mock.patch.object(RECORDER, "observe_exit", return_value=7), \
                    mock.patch.object(RECORDER, "terminate_group", return_value=True), \
                    mock.patch.object(RECORDER, "reap_owned_child", side_effect=RECORDER.WaitOwnershipLost("lost ownership")), \
                    mock.patch.object(RECORDER.time, "monotonic", side_effect=range(30)):
                result = RECORDER.bounded_probe(
                    ["fixture"], cwd=self.repo, env={}, intent=RECORDER.SignalIntent(), timeout=30,
                )
            self.assertTrue(result.pipe_drain_truncated)
            self.assertEqual(result.returncode, 7)
            self.assertIn("ownership", result.lifecycle_error)
            self.assertTrue(all(reader.closed for reader in readers))
        finally:
            for reader in readers:
                reader.close()
            for writer in writers:
                os.close(writer)

    def test_post_kill_deadlines_do_not_require_a_dead_leader(self) -> None:
        """A pending SIGKILL cannot keep a runnable recorder polling indefinitely.

        The fake leader never exits and both pipe writers stay open. Advancing
        the injected clock forces the real lifecycle loops through escalation,
        pipe closure and bounded wait failure without making a kernel-stuck
        fixture. Exhausting the finite clock fails an accidentally endless loop.
        """

        for phase in ("command", "probe"):
            with self.subTest(phase=phase):
                readers = []
                writers = []
                for _ in range(2 if phase == "probe" else 1):
                    read_fd, write_fd = os.pipe()
                    readers.append(os.fdopen(read_fd, "rb", buffering=0))
                    writers.append(write_fd)
                process = mock.Mock(
                    pid=12345, returncode=None, stdout=readers[0],
                    stderr=readers[1] if phase == "probe" else None,
                )
                try:
                    with mock.patch.object(RECORDER.subprocess, "Popen", return_value=process), \
                            mock.patch.object(RECORDER, "observe_exit", return_value=None), \
                            mock.patch.object(RECORDER, "terminate_group", return_value=True) as terminate, \
                            mock.patch.object(RECORDER, "reap_owned_child",
                                              side_effect=subprocess.TimeoutExpired(["fixture"], 0.5)) as reap, \
                            mock.patch.object(RECORDER.time, "monotonic", side_effect=range(30)):
                        if phase == "probe":
                            result = RECORDER.bounded_probe(
                                ["fixture"], cwd=self.repo, env={},
                                intent=RECORDER.SignalIntent(), timeout=0.1,
                            )
                            self.assertFalse(result.complete)
                            self.assertTrue(result.pipe_drain_truncated)
                            self.assertIn("leader remained alive", result.lifecycle_error)
                            self.assertIn("lifecycle_error", result.evidence())
                            self.assertIsNone(result.returncode)
                        else:
                            result = RECORDER.run_command(
                                ["fixture"], self.repo, {}, 0.1, RECORDER.SignalIntent(),
                                mock.Mock(), mock.Mock(),
                            )
                            self.assertEqual(result.outcome, "recorder-error")
                            self.assertEqual(result.recorder_exit, 125)
                            self.assertIn("leader remained alive", result.error)
                            self.assertIsNone(result.child_returncode)
                        self.assertIn(mock.call(process, signal.SIGKILL), terminate.call_args_list)
                        self.assertGreaterEqual(reap.call_count, 1)
                        self.assertLessEqual(reap.call_count, 2)
                        for call in reap.call_args_list:
                            self.assertGreater(call.args[1], 0)
                        self.assertTrue(all(reader.closed for reader in readers))
                finally:
                    for reader in readers:
                        reader.close()
                    for writer in writers:
                        os.close(writer)

    def test_partial_selector_setup_cleans_the_spawned_child(self) -> None:
        """Every setup stage owes cleanup once spawn succeeds, even with no registered pipe.

        The child has an independent finite lease. Assertions precede fallback
        cleanup and require the recorder to have killed/reaped it and closed both
        pipes; an injected setup error cannot pass by leaking a quiet child.
        """

        selector_factory = RECORDER.selectors.DefaultSelector
        for phase in ("command", "probe"):
            for stage in ("allocation", "nonblocking", "registration"):
                with self.subTest(phase=phase, stage=stage):
                    process = subprocess.Popen(
                        [PYTHON, "-c", self.wait_code], stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE if phase == "probe" else subprocess.STDOUT,
                        start_new_session=True,
                    )
                    selector = None
                    try:
                        selector = selector_factory() if stage != "allocation" else None
                        error = OSError("injected selector setup failure")
                        with mock.patch.object(RECORDER.subprocess, "Popen", return_value=process), \
                                mock.patch.object(
                                    RECORDER.selectors, "DefaultSelector", return_value=selector,
                                    side_effect=error if stage == "allocation" else None,
                                ), \
                                mock.patch.object(
                                    RECORDER.os, "set_blocking", wraps=os.set_blocking,
                                    side_effect=error if stage == "nonblocking" else None,
                                ):
                            if stage == "registration":
                                register = selector.register
                                registrations = 0

                                def register_or_fail(fileobj, events):
                                    nonlocal registrations
                                    registrations += 1
                                    if phase == "probe" and registrations == 1:
                                        return register(fileobj, events)
                                    raise error

                                # Fail the second probe registration so its first
                                # successfully registered pipe also needs cleanup.
                                selector.register = register_or_fail
                            if phase == "probe":
                                result = RECORDER.bounded_probe(
                                    ["fixture"], cwd=self.repo, env={}, intent=RECORDER.SignalIntent(),
                                )
                                self.assertIn("injected selector", result.lifecycle_error)
                                self.assertEqual(result.returncode, -signal.SIGKILL)
                                self.assertFalse(result.complete)
                            else:
                                result = RECORDER.run_command(
                                    ["fixture"], self.repo, {}, 1, RECORDER.SignalIntent(),
                                    mock.Mock(), mock.Mock(),
                                )
                                self.assertEqual(result.outcome, "recorder-error")
                                self.assertIn("injected selector", result.error)
                        self.assertEqual(process.returncode, -signal.SIGKILL)
                        self.assertTrue(process.stdout.closed)
                        if process.stderr is not None:
                            self.assertTrue(process.stderr.closed)
                    finally:
                        cleanup_direct_child(process)
                        process.stdout.close()
                        if process.stderr is not None:
                            process.stderr.close()
                        if selector is not None:
                            selector.close()

    def test_fixture_lease_cleanup_never_signals_a_reaped_pid(self) -> None:
        """Fallback release uses its private file even after wait ownership has ended."""

        process = subprocess.Popen([PYTHON, "-c", "pass"])
        self.assertEqual(process.wait(timeout=READY_TIMEOUT), 0)
        pid_path = self.base / "reaped.pid"
        pid_path.write_text(str(process.pid))
        with mock.patch.object(os, "kill", side_effect=AssertionError("numeric signal")), \
                mock.patch.object(os, "killpg", side_effect=AssertionError("numeric group signal")):
            self.assertTrue(process_is_alive(os.getpid()))
            release_fixture_processes(pid_path)
        self.assertTrue(self.stop_path.exists())

    def cli(self, command: list[str], *options: str, tmux: str = "none") -> list[str]:
        """Construct the common required recorder interface around one argv."""

        return [
            PYTHON,
            os.fspath(SCRIPT),
            "--kind",
            "development",
            "--selection",
            "fixture selection",
            "--concurrency",
            "one process",
            "--tmux",
            tmux,
            "--output-root",
            os.fspath(self.evidence),
            *options,
            "--",
            *command,
        ]

    def invoke(
        self,
        command: list[str],
        *options: str,
        tmux: str = "none",
        env: dict[str, str] | None = None,
        stdout: int = subprocess.PIPE,
    ) -> subprocess.CompletedProcess[bytes]:
        """Run the recorder synchronously with bounded capture chosen by each test."""

        return subprocess.run(
            self.cli(command, *options, tmux=tmux),
            cwd=self.repo,
            env=env or self.environment(),
            stdout=stdout,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )

    def run_directories(self) -> list[pathlib.Path]:
        """Return recorder-owned UUID directories in stable creation order."""

        return sorted(
            (path for path in self.evidence.iterdir() if path.is_dir()),
            key=lambda path: path.stat().st_mtime_ns,
        )

    def latest_manifest(self) -> tuple[pathlib.Path, dict[str, object]]:
        """Load the newest record after a synchronous recorder invocation."""

        run_dir = self.run_directories()[-1]
        return run_dir, json.loads((run_dir / "manifest.json").read_text(encoding="utf-8"))

    def install_tmux_fixture(
        self, version: str = "9.9", matching: bool = True, reject_environment_name: str | None = None
    ) -> pathlib.Path:
        """Install a controlled tmux and repository binary without touching the real pin."""

        bin_dir = self.base / "bin"
        bin_dir.mkdir(exist_ok=True)
        tmux = bin_dir / "tmux"
        environment_guard = ""
        if reject_environment_name is not None:
            environment_guard = (
                f"if test \"${{{reject_environment_name}+present}}\" = present; then\n"
                "  printf 'tmux contaminated\\n'\n"
                "  exit 0\n"
                "fi\n"
            )
        tmux.write_text(
            f"#!/bin/sh\n{environment_guard}printf 'tmux {version}\\n'\n", encoding="utf-8"
        )
        tmux.chmod(0o700)
        pin_dir = self.repo / ".github" / "release"
        pin_dir.mkdir(parents=True, exist_ok=True)
        (pin_dir / "source-pins.env").write_text("TMUX_VERSION=9.9\n", encoding="utf-8")
        ci_dir = self.repo / ".ci-tmux"
        ci_dir.mkdir(exist_ok=True)
        if matching:
            shutil.copyfile(tmux, ci_dir / "tmux")
        else:
            (ci_dir / "tmux").write_text("different binary\n", encoding="utf-8")
        (ci_dir / "tmux").chmod(0o700)
        return bin_dir

    def test_failed_retry_and_passing_retry_keep_distinct_records(self) -> None:
        """A later pass must not erase the failed evidence that motivated the retry."""

        failed = self.invoke([PYTHON, "-c", "raise SystemExit(7)"])
        passed = self.invoke([PYTHON, "-c", "print('pass')"])
        self.assertEqual(failed.returncode, 7)
        self.assertEqual(passed.returncode, 0)
        records = self.run_directories()
        self.assertEqual(len(records), 2)
        manifests = [json.loads((path / "manifest.json").read_text()) for path in records]
        self.assertEqual([manifest["child_status"]["exit_code"] for manifest in manifests], [7, 0])
        self.assertNotEqual(manifests[0]["run_id"], manifests[1]["run_id"])
        self.assertEqual(len(manifests[0]["run_id"]), 36)

    def test_recorder_help_does_not_steal_child_help(self) -> None:
        """Help works without required options while post-boundary help belongs to the child."""

        for flag in ("--help", "-h"):
            with self.subTest(flag=flag):
                help_result = subprocess.run(
                    [PYTHON, os.fspath(SCRIPT), flag],
                    cwd=self.repo,
                    env=self.environment(),
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    timeout=10,
                    check=False,
                )
                self.assertEqual(help_result.returncode, 0)
                self.assertIn(b"usage:", help_result.stdout)
        self.assertEqual(self.run_directories(), [])

        child_result = self.invoke(
            [PYTHON, "-c", "import sys; print(sys.argv[1])", "--help"]
        )
        self.assertEqual(child_result.returncode, 0)
        self.assertIn(b"--help", child_result.stdout)

    def test_required_tmux_accepts_matching_version_and_binary(self) -> None:
        """Required mode permits execution only when both pin identities match."""

        bin_dir = self.install_tmux_fixture()
        environment = self.environment(PATH=f"{bin_dir}{os.pathsep}{os.environ.get('PATH', '')}")
        result = self.invoke([PYTHON, "-c", "print('ran')"], tmux="required", env=environment)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        _run_dir, manifest = self.latest_manifest()
        self.assertTrue(manifest["tmux"]["matches_required_substrate"])
        self.assertEqual(manifest["tmux"]["actual"]["version_output"], "tmux 9.9")

    def test_required_tmux_refuses_missing_and_wrong_substrates_before_spawn(self) -> None:
        """Required-mode mismatch is a refusal and cannot run even a marker command."""

        marker = self.base / "spawned"
        command = [PYTHON, "-c", f"from pathlib import Path; Path({str(marker)!r}).touch()"]
        missing_env = self.environment(PATH=os.fspath(self.base / "empty-bin"))
        (self.base / "empty-bin").mkdir()
        missing = self.invoke(command, tmux="required", env=missing_env)
        self.assertEqual(missing.returncode, 125)
        self.assertFalse(marker.exists())
        self.assertEqual(self.latest_manifest()[1]["outcome"], "refused")

        bin_dir = self.install_tmux_fixture(version="9.8", matching=False)
        wrong_env = self.environment(PATH=f"{bin_dir}{os.pathsep}{os.environ.get('PATH', '')}")
        wrong = self.invoke(command, tmux="required", env=wrong_env)
        self.assertEqual(wrong.returncode, 125)
        self.assertFalse(marker.exists())
        problems = self.latest_manifest()[1]["tmux"]["problems"]
        self.assertTrue(any("version mismatch" in problem for problem in problems))
        self.assertTrue(any("hash" in problem for problem in problems))

    def test_warn_mode_reports_mismatch_and_runs(self) -> None:
        """Warn mode makes substrate drift visible without changing command execution."""

        bin_dir = self.install_tmux_fixture(version="8.0", matching=False)
        environment = self.environment(PATH=f"{bin_dir}{os.pathsep}{os.environ.get('PATH', '')}")
        result = self.invoke([PYTHON, "-c", "print('still ran')"], tmux="warn", env=environment)
        self.assertEqual(result.returncode, 0)
        self.assertIn(b"tmux substrate mismatch", result.stderr)
        self.assertIn(b"still ran", result.stdout)
        self.assertFalse(self.latest_manifest()[1]["tmux"]["matches_required_substrate"])

    def test_farhelm_environment_is_scrubbed_for_child_and_probe(self) -> None:
        """Ambient FARHELM values stay private while explicit names control retention."""

        secret = "value-that-must-not-enter-metadata"
        bin_dir = self.install_tmux_fixture(reject_environment_name="FARHELM_SECRET")
        environment = self.environment(
            FARHELM_SECRET=secret,
            FARHELM_KEEP="retained value",
            PATH=f"{bin_dir}{os.pathsep}{os.environ.get('PATH', '')}",
        )
        command = [
            PYTHON,
            "-c",
            (
                "import os; "
                "print('secret=' + str('FARHELM_SECRET' in os.environ)); "
                "print('keep=' + str('FARHELM_KEEP' in os.environ))"
            ),
        ]
        result = self.invoke(
            command,
            "--keep-farhelm-env",
            "FARHELM_KEEP",
            "--keep-farhelm-env",
            "FARHELM_ABSENT",
            tmux="required",
            env=environment,
        )
        self.assertEqual(result.returncode, 0)
        self.assertIn(b"secret=False", result.stdout)
        self.assertIn(b"keep=True", result.stdout)
        run_dir, manifest = self.latest_manifest()
        encoded = (run_dir / "manifest.json").read_text(encoding="utf-8")
        self.assertNotIn(secret, encoded)
        names = manifest["environment"]["farhelm"]
        self.assertEqual(names["retained_names"], ["FARHELM_KEEP"])
        self.assertEqual(names["removed_names"], ["FARHELM_SECRET"])
        self.assertEqual(names["requested_but_absent_names"], ["FARHELM_ABSENT"])

    @unittest.skipUnless(hasattr(os, "symlink"), "symlinks are required")
    def test_source_identity_covers_whole_checkout_and_byte_paths(self) -> None:
        """Invocation below the root still fingerprints tracked, untracked, and symlink state."""

        (self.repo / "binary.dat").write_bytes(b"before\x00\xff")
        run_checked(["git", "add", "binary.dat"], self.repo)
        run_checked(["git", "commit", "--quiet", "-m", "binary fixture"], self.repo)
        (self.repo / "tracked.txt").write_text("dirty\n", encoding="utf-8")
        (self.repo / "binary.dat").write_bytes(b"after\x00\xfe")
        (self.repo / "untracked.bin").write_bytes(b"\x00\xffpayload")
        os.symlink("untracked.bin", self.repo / "untracked-link")
        invalid_relative = b"invalid-\xff-name"
        invalid_path = os.path.join(os.fsencode(self.repo), invalid_relative)
        fd = os.open(invalid_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        os.write(fd, b"raw path")
        os.close(fd)
        nested = self.repo / "nested" / "deeper"
        nested.mkdir(parents=True)

        result = subprocess.run(
            self.cli([PYTHON, "-c", "pass"]),
            cwd=nested,
            env=self.environment(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        _run_dir, manifest = self.latest_manifest()
        source = manifest["source"]
        self.assertTrue(source["complete"])
        self.assertEqual(pathlib.Path(source["git_top_level"]), self.repo.resolve())
        self.assertGreater(source["tracked_diff"]["bytes"], 0)
        raw_diff = subprocess.run(
            ["git", "diff", "--no-ext-diff", "--no-textconv", "--binary", "HEAD", "--"],
            cwd=self.repo,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        ).stdout
        self.assertEqual(source["tracked_diff"]["sha256"], hashlib.sha256(raw_diff).hexdigest())
        self.assertEqual(source["untracked_tree"]["entry_count"], 3)
        self.assertIsNotNone(source["fingerprint_sha256"])
        porcelain_sample = base64.b64decode(source["porcelain"]["sample_base64"])
        self.assertIn(invalid_relative, porcelain_sample)
        self.assertNotIn("raw path", json.dumps(manifest))

        first_fingerprint = source["fingerprint_sha256"]
        (self.repo / "binary.dat").write_bytes(b"second tracked state\x00")
        second = subprocess.run(
            self.cli([PYTHON, "-c", "pass"]),
            cwd=nested,
            env=self.environment(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )
        self.assertEqual(second.returncode, 0, second.stderr.decode())
        second_fingerprint = self.latest_manifest()[1]["source"]["fingerprint_sha256"]

        (self.repo / "untracked.bin").write_bytes(b"second untracked state")
        third = subprocess.run(
            self.cli([PYTHON, "-c", "pass"]),
            cwd=nested,
            env=self.environment(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )
        self.assertEqual(third.returncode, 0, third.stderr.decode())
        third_fingerprint = self.latest_manifest()[1]["source"]["fingerprint_sha256"]

        (self.repo / "untracked-link").unlink()
        os.symlink("tracked.txt", self.repo / "untracked-link")
        fourth = subprocess.run(
            self.cli([PYTHON, "-c", "pass"]),
            cwd=nested,
            env=self.environment(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )
        self.assertEqual(fourth.returncode, 0, fourth.stderr.decode())
        fourth_fingerprint = self.latest_manifest()[1]["source"]["fingerprint_sha256"]
        self.assertEqual(len({first_fingerprint, second_fingerprint, third_fingerprint, fourth_fingerprint}), 4)

        # File contents alone cannot distinguish a runnable local helper from
        # the same helper refused by exec because its mode lacks permission.
        untracked = self.repo / "untracked.bin"
        untracked.chmod(stat.S_IMODE(untracked.stat().st_mode) ^ stat.S_IXUSR)
        fifth = self.invoke([PYTHON, "-c", "pass"])
        self.assertEqual(fifth.returncode, 0, fifth.stderr.decode())
        fifth_source = self.latest_manifest()[1]["source"]
        self.assertTrue(fifth_source["complete"])
        self.assertNotEqual(fourth_fingerprint, fifth_source["fingerprint_sha256"])

    def test_concurrent_first_runs_share_a_private_root(self) -> None:
        """Another recorder winning initial mkdir must not prevent this run from starting."""

        root = self.base / "new-evidence"
        creators = threading.Barrier(2)
        original_mkdir = pathlib.Path.mkdir

        def simultaneous_mkdir(path, *args, **kwargs):
            if path == root:
                creators.wait(timeout=READY_TIMEOUT)
            return original_mkdir(path, *args, **kwargs)

        with mock.patch.object(pathlib.Path, "mkdir", simultaneous_mkdir):
            with ThreadPoolExecutor(max_workers=2) as pool:
                futures = [
                    pool.submit(RECORDER.prepare_root, root, self.repo, self.environment())
                    for _ in range(2)
                ]
                self.assertEqual([future.result() for future in futures], [root, root])
        self.assertEqual(stat.S_IMODE(root.stat().st_mode), 0o700)

        # An existing private root is accepted without broadening its owner
        # permissions; an existing shared root is refused without repairing it.
        root.chmod(0o500)
        RECORDER.prepare_root(root, self.repo, self.environment())
        self.assertEqual(stat.S_IMODE(root.stat().st_mode), 0o500)
        root.chmod(0o755)
        with self.assertRaises(RECORDER.UsageRefusal):
            RECORDER.prepare_root(root, self.repo, self.environment())
        self.assertEqual(stat.S_IMODE(root.stat().st_mode), 0o755)

    def test_cancellation_cleans_signal_ignoring_descendants_after_eof(self) -> None:
        """Both lifecycle loops owe group cleanup after the leader and its pipes end.

        Supply an already-ready real process at the spawn seam so startup
        speed cannot decide whether the descendant installed its signal
        handlers before timeout. The descendant redirects both output pipes;
        the recorder must therefore keep its cancellation obligation after EOF.
        """

        for phase in ("command", "probe"):
            for cancellation in ("timeout", "interrupt"):
                with self.subTest(phase=phase, cancellation=cancellation):
                    # Each prior subcase releases and observes its descendants
                    # before the shared private lease is armed for the next one.
                    self.stop_path.unlink(missing_ok=True)
                    ready = self.base / f"{phase}-{cancellation}.ready"
                    descendant_pid = self.base / f"{phase}-{cancellation}.pid"
                    descendant_code = (
                        "import os, pathlib, signal, time; "
                        "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                        "signal.signal(signal.SIGINT, signal.SIG_IGN); "
                        f"pathlib.Path({str(descendant_pid)!r}).write_text(str(os.getpid())); "
                        f"pathlib.Path({str(ready)!r}).touch(); {self.wait_code}"
                    )
                    argv = [PYTHON, "-c", (
                        "import signal, subprocess, sys, time; "
                        "signal.signal(signal.SIGINT, lambda *_: sys.exit(130)); "
                        f"subprocess.Popen([sys.executable, '-c', {descendant_code!r}], "
                        f"stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL); {self.wait_code}"
                    )]
                    process = subprocess.Popen(
                        argv, cwd=self.repo, env=self.environment(), stdin=subprocess.DEVNULL,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE if phase == "probe" else subprocess.STDOUT,
                        start_new_session=True,
                    )
                    output = None
                    console = None
                    try:
                        wait_for(ready.exists)
                        pid = int(descendant_pid.read_text())
                        intent = RECORDER.SignalIntent()
                        if cancellation == "interrupt" and phase == "command":
                            intent.handle(signal.SIGINT, None)

                        def ready_child(*_args, **_kwargs):
                            # Probe interruption after spawn must clean its child;
                            # interruption before spawn intentionally starts none.
                            if cancellation == "interrupt" and phase == "probe":
                                intent.handle(signal.SIGINT, None)
                            return process

                        with mock.patch.object(RECORDER.subprocess, "Popen", side_effect=ready_child):
                            if phase == "probe":
                                result = RECORDER.bounded_probe(
                                    argv, cwd=self.repo, env=self.environment(), intent=intent,
                                    timeout=0.05 if cancellation == "timeout" else 10,
                                )
                                self.assertEqual(result.timed_out, cancellation == "timeout")
                                self.assertEqual(result.interrupted, cancellation == "interrupt")
                                self.assertEqual(result.returncode, -signal.SIGTERM if cancellation == "timeout" else 130)
                            else:
                                run_dir = self.evidence / f"{phase}-{cancellation}"
                                run_dir.mkdir(mode=0o700)
                                output = RECORDER.OutputStore(run_dir)
                                console = RECORDER.ConsoleForwarder()
                                result = RECORDER.run_command(
                                    argv, self.repo, self.environment(),
                                    0.05 if cancellation == "timeout" else None,
                                    intent, output, console,
                                )
                                self.assertEqual(result.outcome, "timed_out" if cancellation == "timeout" else "interrupted")
                                self.assertTrue(result.forced_cleanup)
                                self.assertEqual(result.child_returncode, -signal.SIGTERM if cancellation == "timeout" else 130)
                                self.assertEqual(result.recorder_exit, 124 if cancellation == "timeout" else 130)
                        wait_for(lambda: not process_is_alive(pid))
                    finally:
                        # The direct child may already be reaped. Release the
                        # descendant's independent lease after any owned signal.
                        cleanup_direct_child(process)
                        release_fixture_processes(descendant_pid)
                        if ready.exists():
                            pid = int(descendant_pid.read_text())
                            wait_for(lambda: not process_is_alive(pid))
                        for stream in (process.stdout, process.stderr):
                            if stream is not None:
                                stream.close()
                        if output is not None:
                            output.close()
                        if console is not None:
                            console.finish()

    def test_output_is_bounded_to_head_and_rolling_tail(self) -> None:
        """Large output keeps the first MiB and newest seven MiB with exact counters."""

        command = [
            PYTHON,
            "-c",
            "import os; os.write(1, b'A' * 1048576); os.write(1, b'B' * 8388608)",
        ]
        result = self.invoke(command, stdout=subprocess.DEVNULL)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        run_dir, manifest = self.latest_manifest()
        output = manifest["output"]
        self.assertEqual(output["observed_bytes"], 9 * 1024 * 1024)
        self.assertEqual(output["retained_bytes"], 8 * 1024 * 1024)
        self.assertEqual(output["omitted_bytes"], 1024 * 1024)
        files = output["files_in_read_order"]
        self.assertEqual(sum(item["bytes"] for item in files), 8 * 1024 * 1024)
        self.assertEqual((run_dir / files[0]["name"]).read_bytes()[:1], b"A")
        self.assertEqual((run_dir / files[-1]["name"]).read_bytes()[-1:], b"B")
        for path in run_dir.iterdir():
            self.assertEqual(stat.S_IMODE(path.stat().st_mode) & 0o077, 0)

    def test_blocked_console_does_not_disable_interruption_or_retention(self) -> None:
        """An unread recorder stdout pipe cannot stall interruption or evidence retention."""

        group_pid = self.base / "blocked-console-group.pid"
        ready = self.base / "blocked-console-ready"
        command = [
            PYTHON,
            "-c",
            (
                "import os, pathlib, time\n"
                f"pathlib.Path({str(group_pid)!r}).write_text(str(os.getpid()))\n"
                "payload = b'X' * 6291456\n"
                "while payload:\n"
                "    payload = payload[os.write(1, payload):]\n"
                f"pathlib.Path({str(ready)!r}).touch()\n"
                f"{self.wait_code}\n"
            ),
        ]
        recorder = subprocess.Popen(
            self.cli(command),
            cwd=self.repo,
            env=self.environment(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            wait_for(ready.exists)
            recorder.send_signal(signal.SIGTERM)
            recorder.wait(timeout=8)
            self.assertEqual(recorder.returncode, 128 + signal.SIGTERM)
            _run_dir, manifest = self.latest_manifest()
            self.assertEqual(manifest["outcome"], "interrupted")
            self.assertGreaterEqual(manifest["output"]["observed_bytes"], 6 * 1024 * 1024)
            self.assertGreater(manifest["output"]["retained_bytes"], 0)
            self.assertGreater(manifest["console"]["dropped_or_pending_bytes"], 0)
        finally:
            if recorder.poll() is None:
                recorder.kill()
                recorder.wait()
            release_fixture_processes(group_pid)
            if recorder.stdout is not None:
                recorder.stdout.close()
            if recorder.stderr is not None:
                recorder.stderr.close()

    def test_child_signal_uses_conventional_exit_without_calling_it_interruption(self) -> None:
        """A child-originated signal remains distinct from a signal sent to the recorder."""

        result = self.invoke(
            [PYTHON, "-c", "import os, signal; os.kill(os.getpid(), signal.SIGTERM)"]
        )
        self.assertEqual(result.returncode, 128 + signal.SIGTERM)
        manifest = self.latest_manifest()[1]
        self.assertEqual(manifest["outcome"], "completed")
        self.assertEqual(manifest["child_status"]["signal"], signal.SIGTERM)

    def test_empty_output_does_not_end_a_live_child(self) -> None:
        """Pipe silence and EOF are not substitutes for observing process exit."""

        started = time.monotonic()
        result = self.invoke(
            [PYTHON, "-c", "import os, time; os.close(1); os.close(2); time.sleep(5)"],
            "--timeout",
            "0.25",
        )
        elapsed = time.monotonic() - started
        self.assertEqual(result.returncode, 124)
        self.assertGreaterEqual(elapsed, 0.20)
        self.assertLess(elapsed, 4.0)
        self.assertEqual(self.latest_manifest()[1]["outcome"], "timed_out")

    def test_timeout_terminates_owned_process_group(self) -> None:
        """Timeout covers descendants in the recorder-owned process group."""

        child_pid = self.base / "timeout-child.pid"
        group_pid = self.base / "timeout-group.pid"
        command = [
            PYTHON,
            "-c",
            (
                "import os, pathlib, subprocess, sys, time; "
                f"pathlib.Path({str(group_pid)!r}).write_text(str(os.getpid())); "
                f"p=subprocess.Popen([sys.executable, {str(self.waiter)!r}]); "
                f"pathlib.Path({str(child_pid)!r}).write_text(str(p.pid)); "
                f"{self.wait_code}"
            ),
        ]
        try:
            result = self.invoke(command, "--timeout", "1.5")
            self.assertEqual(result.returncode, 124, result.stderr.decode())
            pid = int(child_pid.read_text())
            wait_for(lambda: not process_is_alive(pid))
            manifest = self.latest_manifest()[1]
            self.assertEqual(manifest["outcome"], "timed_out")
        finally:
            release_fixture_processes(group_pid)

    def test_interrupt_is_forwarded_and_cleans_owned_group(self) -> None:
        """SIGINT to the recorder becomes an interrupted record and reaches descendants."""

        descendant_pid = self.base / "interrupt-child.pid"
        group_pid = self.base / "interrupt-group.pid"
        ready = self.base / "interrupt-ready"
        command = [
            PYTHON,
            "-c",
            (
                "import os, pathlib, subprocess, sys, time; "
                f"pathlib.Path({str(group_pid)!r}).write_text(str(os.getpid())); "
                f"p=subprocess.Popen([sys.executable, {str(self.waiter)!r}]); "
                f"pathlib.Path({str(descendant_pid)!r}).write_text(str(p.pid)); "
                f"pathlib.Path({str(ready)!r}).touch(); "
                f"{self.wait_code}"
            ),
        ]
        recorder = subprocess.Popen(
            self.cli(command),
            cwd=self.repo,
            env=self.environment(),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
        )
        try:
            wait_for(ready.exists)
            recorder.send_signal(signal.SIGINT)
            _stdout, stderr = recorder.communicate(timeout=10)
            self.assertEqual(recorder.returncode, 128 + signal.SIGINT, stderr.decode())
            pid = int(descendant_pid.read_text())
            wait_for(lambda: not process_is_alive(pid))
            self.assertEqual(self.latest_manifest()[1]["outcome"], "interrupted")
        finally:
            if recorder.poll() is None:
                recorder.kill()
                recorder.wait()
            release_fixture_processes(group_pid)

    def test_inherited_pipe_after_leader_exit_has_bounded_cleanup(self) -> None:
        """A descendant holding the output pipe cannot hang finalization forever."""

        descendant_pid = self.base / "pipe-child.pid"
        command = [
            PYTHON,
            "-c",
            (
                "import pathlib, subprocess, sys; "
                f"p=subprocess.Popen([sys.executable, {str(self.waiter)!r}]); "
                f"pathlib.Path({str(descendant_pid)!r}).write_text(str(p.pid))"
            ),
        ]
        pid: int | None = None
        try:
            started = time.monotonic()
            result = self.invoke(command)
            elapsed = time.monotonic() - started
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertLess(elapsed, 6.0)
            pid = int(descendant_pid.read_text())
            wait_for(lambda: not process_is_alive(pid))
            manifest = self.latest_manifest()[1]
            self.assertTrue(manifest["recorder"]["forced_cleanup"])
        finally:
            release_fixture_processes(descendant_pid)

    def test_escaped_descendant_pipe_has_an_independent_drain_deadline(self) -> None:
        """A new-session descendant is disclosed and cannot hold finalization open."""

        descendant_pid = self.base / "escaped-pipe-child.pid"
        command = [
            PYTHON,
            "-c",
            (
                "import pathlib, subprocess, sys; "
                f"p=subprocess.Popen([sys.executable, {str(self.waiter)!r}], start_new_session=True); "
                f"pathlib.Path({str(descendant_pid)!r}).write_text(str(p.pid))"
            ),
        ]
        pid: int | None = None
        try:
            started = time.monotonic()
            result = self.invoke(command)
            elapsed = time.monotonic() - started
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertLess(elapsed, 6.0)
            pid = int(descendant_pid.read_text())
            self.assertTrue(process_is_alive(pid))
            limit = self.latest_manifest()[1]["recorder"]["cleanup_limit"]
            self.assertIn("escaped descendants may remain", limit)
        finally:
            release_fixture_processes(descendant_pid)

    def test_probe_escaped_pipe_has_an_independent_drain_deadline(self) -> None:
        """A malformed Git helper cannot retain probe pipes after its leader exits."""

        bin_dir = self.base / "probe-bin"
        bin_dir.mkdir()
        descendant_pid = self.base / "escaped-probe-child.pid"
        fake_git = bin_dir / "git"
        fake_git.write_text(
            "#!/usr/bin/env python3\n"
            "import pathlib, subprocess, sys\n"
            f"child = subprocess.Popen([sys.executable, {str(self.waiter)!r}], start_new_session=True)\n"
            f"pathlib.Path({str(descendant_pid)!r}).write_text(str(child.pid))\n"
            f"print({str(self.repo)!r})\n",
            encoding="utf-8",
        )
        fake_git.chmod(0o700)
        environment = self.environment(PATH=f"{bin_dir}{os.pathsep}{os.environ.get('PATH', '')}")
        pid: int | None = None
        try:
            started = time.monotonic()
            result = self.invoke([PYTHON, "-c", "print('ran')"], env=environment)
            elapsed = time.monotonic() - started
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertLess(elapsed, 4.0)
            pid = int(descendant_pid.read_text())
            self.assertTrue(process_is_alive(pid))
            source = self.latest_manifest()[1]["source"]
            self.assertFalse(source["complete"])
            self.assertTrue(source["git_top_level_probe"]["pipe_drain_truncated"])
        finally:
            release_fixture_processes(descendant_pid)

    def test_spawn_failure_finalizes_recorder_error(self) -> None:
        """An unspawnable argv must replace the initial running state with an error."""

        result = self.invoke([os.fspath(self.base / "does-not-exist")])
        self.assertEqual(result.returncode, 125)
        _run_dir, manifest = self.latest_manifest()
        self.assertEqual(manifest["outcome"], "recorder-error")
        self.assertIn("spawn failed", manifest["recorder"]["error"])
        self.assertIn(b"test-run evidence:", result.stderr)

    def test_sigkill_leaves_running_manifest_and_incremental_output(self) -> None:
        """Uncatchable recorder death still leaves its last valid state and written output."""

        ready = self.base / "kill-ready"
        child_pid = self.base / "kill-child.pid"
        marker = b"late-durable-before-recorder-sigkill\n"
        command = [
            PYTHON,
            "-c",
            (
                "import os, pathlib, time\n"
                f"pathlib.Path({str(child_pid)!r}).write_text(str(os.getpid()))\n"
                "os.write(1, b'A' * 1048576)\n"
                f"os.write(1, {marker!r})\n"
                f"pathlib.Path({str(ready)!r}).touch()\n"
                f"while not pathlib.Path({str(self.stop_path)!r}).exists():\n"
                "    os.write(1, b'late-output\\n')\n"
                "    time.sleep(.02)\n"
            ),
        ]
        recorder = subprocess.Popen(
            self.cli(command),
            cwd=self.repo,
            env=self.environment(),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
        pid: int | None = None
        try:
            wait_for(ready.exists)
            pid = int(child_pid.read_text())
            wait_for(
                lambda: bool(self.run_directories())
                and any(
                    marker in path.read_bytes()
                    for path in self.run_directories()[-1].glob("output-tail-*.log")
                )
            )
            os.kill(recorder.pid, signal.SIGKILL)
            recorder.wait(timeout=5)
            run_dir, manifest = self.latest_manifest()
            self.assertEqual(manifest["outcome"], "running")
            self.assertTrue(any(marker in path.read_bytes() for path in run_dir.glob("output-tail-*.log")))
        finally:
            if recorder.poll() is None:
                recorder.kill()
                recorder.wait()
            if recorder.stderr is not None:
                recorder.stderr.close()
            release_fixture_processes(child_pid)

    def test_output_root_inside_checkout_is_refused_without_writing(self) -> None:
        """Evidence must never enter the checkout it is trying to fingerprint."""

        unsafe = self.repo / "evidence"
        argv = self.cli([PYTHON, "-c", "pass"])
        root_index = argv.index(os.fspath(self.evidence))
        argv[root_index] = os.fspath(unsafe)
        result = subprocess.run(
            argv,
            cwd=self.repo,
            env=self.environment(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )
        self.assertEqual(result.returncode, 125)
        self.assertFalse(unsafe.exists())
        self.assertIn(b"inside the tested checkout", result.stderr)
        self.assertIn(b"test-run evidence unavailable", result.stderr)

    def test_xdg_override_does_not_unfence_conventional_live_state(self) -> None:
        """A separate XDG state home cannot authorize the conventional live-state tree."""

        fixture_home = self.base / "fixture-home"
        fixture_xdg = self.base / "fixture-xdg"
        fixture_home.mkdir()
        fixture_xdg.mkdir()
        unsafe = fixture_home / ".local" / "state" / "farhelm" / "evidence"
        argv = self.cli([PYTHON, "-c", "pass"])
        root_index = argv.index(os.fspath(self.evidence))
        argv[root_index] = os.fspath(unsafe)
        result = subprocess.run(
            argv,
            cwd=self.repo,
            env=self.environment(HOME=os.fspath(fixture_home), XDG_STATE_HOME=os.fspath(fixture_xdg)),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )
        self.assertEqual(result.returncode, 125)
        self.assertFalse(unsafe.exists())
        self.assertIn(b"inside live Farhelm state", result.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)
