#!/usr/bin/env python3
"""Exercise the manual drift runner using stand-ins. No test starts a vendor CLI."""

import contextlib
import io
import json
import os
from pathlib import Path
import re
import runpy
import signal
import subprocess
import tempfile
import threading
import time
import unittest
from unittest.mock import patch
import urllib.error
import urllib.request

from agent_screen_host import OUTER_SESSION_MARKERS
import harness_upgrades

runner = runpy.run_path(str(Path(__file__).with_name("run-harness-drift.py")))
# runpy returns a copy of the globals. Patch the actual function namespace so main uses the stand-ins.
globals_of_runner = runner["main"].__globals__
DRIFT_SOURCE = Path(__file__).resolve().parents[1] / "crates/farhelm/tests/e2e/real_agent_capture/drift"


def no_upgrades(names, env, upgrade):
    """Stand in for the upgrade phase in runner tests that are about verdicts, so no vendor updater ever runs."""
    return {name: harness_upgrades.Upgrade("current", "1.0", "1.0") for name in names}


class DriftRunnerTests(unittest.TestCase):
    def test_build_and_discovery_use_the_single_cargo_artifact_and_only_ignored_drift_tests(self):
        """A custom target directory must work and unrelated ignored live tests must never run."""
        artifact = {
            "reason": "compiler-artifact",
            "target": {"name": "e2e", "kind": ["test"]},
            "executable": "/synthetic/custom/target/debug/deps/e2e-test",
        }
        listing = (
            "real_agent_capture::real_claude_session_reports_its_identity_across_clear: test\n"
            "real_agent_capture::drift::tests::a_drift_unit_test: test\n"
            "real_agent_capture::drift::base::drift_codex: test\n"
            "real_agent_capture::drift::base::drift_antigravity: test\n"
            "real_agent_capture::drift::model_selection::drift_codex: test\n\n"
            "5 tests, 0 benchmarks\n"
        )
        outputs = [
            subprocess.CompletedProcess([], 0, json.dumps(artifact)),
            subprocess.CompletedProcess([], 0, listing),
        ]
        env = {"CARGO_TARGET_DIR": "/synthetic/custom/target"}
        with patch.object(subprocess, "run", side_effect=outputs) as run:
            binary = runner["build_binary"](env)
            cases = runner["discover"](binary, env)
        self.assertEqual(list(cases), [("base", "antigravity"), ("base", "codex"), ("model_selection", "codex")])
        self.assertEqual(cases["base", "codex"], "real_agent_capture::drift::base::drift_codex")
        self.assertEqual(cases["model_selection", "codex"], "real_agent_capture::drift::model_selection::drift_codex")
        self.assertIn("--no-run", run.call_args_list[0].args[0])
        self.assertEqual(run.call_args_list[1].args[0], [str(binary), "--ignored", "--list"])
        self.assertEqual(run.call_count, 2)

    def test_child_environment_keeps_login_without_nested_agent_and_farhelm_state(self):
        """Keep credentials in child env while dropping every outer-session marker."""
        outer = dict.fromkeys((*OUTER_SESSION_MARKERS, "TMUX", "TMUX_PANE", "FARHELM_AGENT_ID"), "outer")
        auth = {"CODEX_HOME": "vendor-home", "OPENROUTER_API_KEY": "stand-in", "PATH": "bin"}
        environment = {**outer, **auth}
        self.assertEqual(runner["child_environment"](environment), auth)
        self.assertEqual(environment, {**outer, **auth})

    def test_missing_invalid_and_inconsistent_verdicts_fail(self):
        """A passing libtest or a stale success record alone cannot establish coverage."""
        read = runner["read_verdict"]
        with tempfile.TemporaryDirectory() as directory:
            verdict = Path(directory) / "verdict.json"
            self.assertEqual(read(verdict, 0).verdict, "failed")
            for payload in [[], {"verdict": "other", "detail": "text"}, {"verdict": "passed", "detail": ""}]:
                verdict.write_text(json.dumps(payload))
                self.assertEqual(read(verdict, 0).verdict, "failed")
            verdict.write_text(json.dumps({"verdict": "could_not_run", "detail": "no login"}))
            self.assertEqual(read(verdict, 0).verdict, "could_not_run")
            self.assertEqual(read(verdict, 1).verdict, "failed")

    def test_missing_verdict_keeps_the_recorders_known_failure_reason(self):
        """The recorder can know why a case ended before Rust could write its result."""
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            manifest = evidence / "manifest.json"
            verdict = evidence / "missing.json"
            recorded = runner["recorder_manifest"]
            manifest.write_text(json.dumps({"schema_version": 1, "outcome": "timed_out", "recorder": {"error": None}}))
            result = runner["read_verdict"](verdict, 124, recorded(evidence))
            self.assertEqual(result.verdict, "failed")
            self.assertIn("timed out", result.detail)
            manifest.write_text(json.dumps({
                "schema_version": 1, "outcome": "recorder-error", "recorder": {"error": "fixture refused"},
            }))
            result = runner["read_verdict"](verdict, 125, recorded(evidence))
            self.assertEqual(result.verdict, "failed")
            self.assertIn("fixture refused", result.detail)
            for version in [2, True, 1.0]:
                manifest.write_text(json.dumps({"schema_version": version, "outcome": "timed_out"}))
                result = runner["read_verdict"](verdict, 124, recorded(evidence))
                self.assertIn("without a readable drift verdict", result.detail)

    def test_every_selected_case_runs_and_only_an_explicit_skip_fails(self):
        """Missing prerequisites are distinguishable and do not prevent another harness from running."""
        cases = {
            ("base", "claude"): "case-claude",
            ("base", "codex"): "case-codex",
            ("base", "grok"): "case-grok",
            ("model_selection", "codex"): "case-model",
        }
        result = runner["Result"]
        for requested, verdicts, code in [
            ([], [result("could_not_run", "no login"), result("passed", "ok"), result("passed", "ok")], 0),
            ([], [result("failed", "no report"), result("passed", "ok"), result("could_not_run", "no hook")], 1),
            (["--harness", "grok", "--harness", "codex"], [result("could_not_run", "no hook"), result("passed", "ok")], 1),
        ]:
            called = []

            def recorded(binary, test, name, env):
                called.append(name)
                return verdicts[len(called) - 1]

            with (
                patch.dict(globals_of_runner, {
                    "build_binary": lambda env: Path("/synthetic/e2e"),
                    "discover": lambda binary, env: cases,
                    "recorded_case": recorded,
                    "prepare": no_upgrades,
                }),
                contextlib.redirect_stdout(io.StringIO()) as output,
            ):
                self.assertEqual(runner["main"](requested), code)
            self.assertEqual(called, ["base/grok", "base/codex"] if requested else ["base/claude", "base/codex", "base/grok"])
            self.assertIn("COULD NOT RUN", output.getvalue())
            self.assertIn("PASSED", output.getvalue())

    def test_list_discovers_but_never_launches(self):
        """Listing the suite must spend zero vendor turns."""
        def unexpected(*args):
            self.fail("listing must never run a case")

        with patch.dict(globals_of_runner, {
            "build_binary": lambda env: Path("/synthetic/e2e"),
            "discover": lambda binary, env: {("base", "claude"): "case-claude", ("model_selection", "codex"): "case-model"},
            "recorded_case": unexpected,
            "prepare": unexpected,
        }), contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(runner["main"](["--list", "--category", "model_selection", "--harness", "codex"]), 0)
        self.assertEqual(output.getvalue().strip(), "model_selection/codex")

    def test_category_selection_deduplicates_upgrades_but_retains_every_verdict(self):
        """A pass in one category cannot overwrite the same harness's failure in another."""
        cases = {("base", "codex"): "base-case", ("model_selection", "codex"): "model-case"}
        for filters in [
            ["--category", "all"],
            ["--category", "base", "--category", "model_selection", "--category", "base", "--harness", "codex"],
        ]:
            calls = []

            def prepare(names, env, upgrade):
                calls.append(("upgrade", names))
                return no_upgrades(names, env, upgrade)

            def recorded(binary, test, name, env):
                calls.append(("case", test))
                if test == "base-case":
                    return runner["Result"]("failed", "base failure")
                return runner["Result"]("passed", "model success")

            with patch.dict(globals_of_runner, {
                "build_binary": lambda env: Path("/synthetic/e2e"),
                "discover": lambda binary, env: cases,
                "prepare": prepare,
                "recorded_case": recorded,
            }), contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(runner["main"](filters), 1)
            self.assertEqual(calls, [("upgrade", ["codex"]), ("case", "base-case"), ("case", "model-case")])
            self.assertRegex(output.getvalue(), r"FAILED +base +codex +1\.0 +base failure")
            self.assertRegex(output.getvalue(), r"PASSED +model_selection +codex +1\.0 +model success")

    def test_invalid_category_combinations_run_no_upgrades_or_cases(self):
        """Upgrade-only needs no build, and a category without a selected harness cannot report success."""
        def unexpected(*args, **kwargs):
            self.fail("the rejected selection must not start work")

        cases = {("base", "grok"): "base-grok", ("model_selection", "codex"): "model-codex"}
        for argv, diagnostic, builds in [
            (["--upgrade-only", "--category", "base"], "--category cannot be used with --upgrade-only", False),
            (["--list", "--upgrade-only"], "--list cannot be used with --upgrade-only", False),
            (["--category", "model_selection", "--harness", "grok"], "available harnesses: codex", True),
            (
                ["--category", "model_selection", "--harness", "codex", "--harness", "grok"],
                "no case for grok in the selected categories",
                True,
            ),
            (["--category", "missing"], "unknown category: missing", True),
        ]:
            with patch.dict(globals_of_runner, {
                "build_binary": (lambda env: Path("/synthetic/e2e")) if builds else unexpected,
                "discover": lambda binary, env: cases,
                "prepare": unexpected,
                "recorded_case": unexpected,
            }), contextlib.redirect_stderr(io.StringIO()) as errors, self.assertRaises(SystemExit) as exited:
                runner["main"](argv)
            self.assertEqual(exited.exception.code, 2)
            self.assertIn(diagnostic, errors.getvalue())

    def test_retains_the_structured_verdict_with_the_recorder_files(self):
        """Give the recorder time for both turns and retain the verdict beside its manifest."""
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            (evidence / "manifest.json").write_text("{}")

            class Recorder:
                """A recorder that announces its directory before the child's own output."""
                def __init__(self, command, **kwargs):
                    self.stderr = io.StringIO(f"test-run evidence: {evidence}\n")
                    self.command = command
                    verdict_file = Path(kwargs["env"]["FARHELM_DRIFT_VERDICT_FILE"])
                    if verdict_file.exists():
                        raise AssertionError("the caller's verdict must not be reused")
                    verdict_file.write_text(json.dumps({"verdict": "could_not_run", "detail": "login missing"}))

                def __enter__(self):
                    return self

                def __exit__(self, *_):
                    self.stderr.close()

                def wait(self):
                    return 0

            with patch.object(subprocess, "Popen", side_effect=Recorder) as spawn, contextlib.redirect_stderr(io.StringIO()):
                result = runner["recorded_case"](Path("/synthetic/e2e"), "exact-case", "grok", {"PATH": "bin"})
            command = spawn.call_args.args[0]
            self.assertEqual(command[-7:], ["/synthetic/e2e", "--ignored", "--exact", "exact-case", "--test-threads", "1", "--nocapture"])
            self.assertEqual(command[command.index("--timeout") + 1], "720")
            self.assertIn("FARHELM_DRIFT_VERDICT_FILE", command)
            self.assertEqual(result.verdict, "could_not_run")
            self.assertEqual(result.evidence, evidence)
            self.assertEqual(json.loads((evidence / "drift-verdict.json").read_text())["detail"], "login missing")

    def test_runner_only_termination_reaches_the_recorder(self):
        """Wait for recorder cleanup and retain its interruption without adding a failed drift verdict."""
        prior_handler = signal.getsignal(signal.SIGTERM)
        with (tempfile.TemporaryDirectory() as directory, patch.object(subprocess, "Popen") as spawn,
              contextlib.redirect_stderr(io.StringIO())):
            evidence = Path(directory).resolve()
            (evidence / "manifest.json").write_text(json.dumps({"schema_version": 1, "outcome": "interrupted"}))
            recorder = spawn.return_value.__enter__.return_value
            recorder.stderr = [f"test-run evidence: {evidence}\n"]

            def cancel(*_args, **_kwargs):
                signal.raise_signal(signal.SIGTERM)
                return spawn.return_value

            spawn.side_effect = cancel
            recorder.wait.return_value = 128 + signal.SIGTERM
            with self.assertRaises(SystemExit) as stopped:
                runner["recorded_case"](Path("/synthetic/e2e"), "exact-case", "grok", {"PATH": "bin"})
            self.assertFalse((evidence / "drift-verdict.json").exists())
        recorder.send_signal.assert_called_once_with(signal.SIGTERM)
        recorder.wait.assert_called_once()
        self.assertEqual(stopped.exception.code, 128 + signal.SIGTERM)
        self.assertEqual(signal.getsignal(signal.SIGTERM), prior_handler)

    def test_finalized_recorder_outcome_distinguishes_interruption_from_a_child_exit(self):
        """One exit code can describe either event. Keep the case's own verdict on recorder-only cancellation."""
        status = 128 + signal.SIGTERM
        payload = {"verdict": "passed", "detail": "case wrote this before recorder exit"}
        for outcome in ("interrupted", "completed"):
            with (self.subTest(outcome=outcome), tempfile.TemporaryDirectory() as directory,
                  patch.object(subprocess, "Popen") as spawn, contextlib.redirect_stderr(io.StringIO())):
                evidence = Path(directory).resolve()
                (evidence / "manifest.json").write_text(json.dumps({"schema_version": 1, "outcome": outcome}))
                recorder = spawn.return_value.__enter__.return_value
                recorder.stderr = [f"test-run evidence: {evidence}\n"]
                recorder.wait.return_value = status

                def finish(*_args, env, **_kwargs):
                    Path(env["FARHELM_DRIFT_VERDICT_FILE"]).write_text(json.dumps(payload))
                    return spawn.return_value

                spawn.side_effect = finish
                args = (Path("/synthetic/e2e"), "exact-case", "grok", {"PATH": "bin"})
                if outcome == "interrupted":
                    with self.assertRaises(SystemExit) as stopped:
                        runner["recorded_case"](*args)
                    self.assertEqual(stopped.exception.code, status)
                    self.assertEqual(json.loads((evidence / "drift-verdict.json").read_text()), payload)
                else:
                    result = runner["recorded_case"](*args)
                    self.assertEqual(result.verdict, "failed")
                    self.assertIn(f"recorder exited {status}", result.detail)

    def test_termination_during_a_failed_spawn_stops_the_suite(self):
        """A spawn error may fail one case, but a simultaneous cancellation ends the run."""
        def failed_spawn(*_args, **_kwargs):
            signal.raise_signal(signal.SIGTERM)
            raise OSError("fixture spawn failed")

        with patch.object(subprocess, "Popen", side_effect=failed_spawn):
            with self.assertRaises(SystemExit) as stopped:
                runner["recorded_case"](Path("/synthetic/e2e"), "exact-case", "grok", {"PATH": "bin"})
        self.assertEqual(stopped.exception.code, 128 + signal.SIGTERM)


def require_user_manager(test: unittest.TestCase) -> None:
    """Skip `test` when no systemd user manager answers here: missing coverage, said so, never a quiet pass."""
    problem = harness_upgrades.user_manager_problem()
    if problem:
        print(f"SKIPPED {test.id()}: needs a systemd user manager ({problem})", flush=True)
        test.skipTest("no systemd user manager")


def process_gone(pid: int) -> bool:
    """Whether `pid` no longer runs; a zombie counts as gone, since it can write nothing."""
    try:
        state = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[0]
    except (FileNotFoundError, ProcessLookupError):
        return True
    return state in ("Z", "X")


def detached_and_running(pid: int) -> bool:
    """Whether `pid` runs as the leader of a session of its own, as a `setsid` worker does, out of its updater's group."""
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
    except (FileNotFoundError, ProcessLookupError):
        return False
    return fields[0] not in ("Z", "X") and int(fields[3]) == pid


def watch_premise(path: Path) -> tuple[threading.Thread, list[bool]]:
    """Start a thread that waits for the worker's pid in `path` and records whether it then runs detached, so a test
    can check its premise while the code under test is still blocking the main thread."""
    seen: list[bool] = []

    def look():
        seen.append(detached_and_running(int(wait_for_file(path))))

    thread = threading.Thread(target=look, daemon=True)
    thread.start()
    return thread, seen


def wait_for_file(path: Path, timeout: float = 10) -> str:
    """The contents of `path` once a stand-in has written it, failing the test when it never appears."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists() and path.read_text().strip():
            return path.read_text().strip()
        time.sleep(0.05)
    raise AssertionError(f"{path} never appeared")


class ScopedUpdaterTests(unittest.TestCase):
    """Every updater runs in its own transient user scope, and nothing it started outlives the upgrade step.

    Why: an updater's background worker (npm's, a self-updating launcher, a detached helper) that swapped the CLI's
    binary after the runner probed its version would make a drift verdict name the wrong release. Each test starts a
    stand-in whose worker leaves the updater's session with `setsid`, which a process-group kill would miss, and
    checks the worker by its own pid, never through the runner. They need a systemd user manager and are skipped,
    loudly, without one; no vendor CLI runs.
    """

    def setUp(self):
        require_user_manager(self)
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.dir = Path(directory.name)
        self.env = {"PATH": "/usr/bin:/bin"}

    def worker(self, body: str) -> str:
        """Shell that starts a detached worker in a session of its own, which records its pid and then runs `body`."""
        return (
            f"setsid sh -c 'echo $$ > {self.dir}/worker.pid; {body}' </dev/null >/dev/null 2>&1 &"
            f" until [ -s {self.dir}/worker.pid ]; do sleep 0.05; done"
        )

    def test_an_ordinary_exit_waits_for_a_detached_worker_to_finish(self):
        """Spec: an updater that exits 0 while its detached worker still runs succeeds only after the worker has
        finished its write (half a second later), and the worker is gone when the upgrade step returns."""
        script = self.worker(f"sleep 0.5; echo swapped > {self.dir}/result") + "; exit 0"
        ok, detail = harness_upgrades.run_updater(["sh", "-c", script], self.env, timeout=30)
        self.assertEqual((ok, detail), (True, ""))
        self.assertEqual((self.dir / "result").read_text(), "swapped\n")
        self.assertTrue(process_gone(int((self.dir / "worker.pid").read_text())))

    def test_a_worker_still_running_at_the_deadline_is_stopped_and_the_upgrade_fails(self):
        """Spec: an updater that exits 0 but leaves a detached worker running past the deadline fails, saying its
        background processes did not finish, and the worker is gone when the upgrade step returns."""
        script = self.worker("exec sleep 1000") + "; exit 0"
        watcher, premise = watch_premise(self.dir / "worker.pid")
        ok, detail = harness_upgrades.run_updater(["sh", "-c", script], self.env, timeout=2)
        watcher.join(timeout=10)
        self.assertEqual(premise, [True], "premise: the worker ran detached before the deadline")
        self.assertFalse(ok)
        self.assertIn("background processes did not finish within 2 seconds", detail)
        self.assertTrue(process_gone(int((self.dir / "worker.pid").read_text())))

    def test_an_updater_past_its_deadline_is_stopped_with_its_detached_worker(self):
        """Spec: an updater still running at the deadline fails, saying it did not finish, and both it and its
        detached worker are gone when the upgrade step returns."""
        script = self.worker("exec sleep 1000") + f"; echo $$ > {self.dir}/updater.pid; exec sleep 1000"
        watcher, premise = watch_premise(self.dir / "worker.pid")
        ok, detail = harness_upgrades.run_updater(["sh", "-c", script], self.env, timeout=2)
        watcher.join(timeout=10)
        self.assertEqual(premise, [True], "premise: the worker ran detached before the deadline")
        self.assertFalse(ok)
        self.assertIn("did not finish within 2 seconds", detail)
        self.assertTrue(process_gone(int((self.dir / "worker.pid").read_text())))
        self.assertTrue(process_gone(int((self.dir / "updater.pid").read_text())))

    def test_sigterm_stops_the_scope_before_the_cancellation_is_raised(self):
        """Spec: SIGTERM while the updater and its detached worker run raises `Cancelled` carrying SIGTERM, only once
        both are gone and the scope has no processes, and the signal handlers are the ones in place before."""
        script = self.worker("exec sleep 1000") + f"; echo $$ > {self.dir}/updater.pid; exec sleep 1000"
        before = signal.getsignal(signal.SIGTERM)
        units = []
        real_popen = subprocess.Popen

        def recording_popen(command, **kwargs):
            if command[0] == "systemd-run":
                units.extend(arg.removeprefix("--unit=") for arg in command if arg.startswith("--unit="))
            return real_popen(command, **kwargs)

        premise = []

        def send_once_running():
            wait_for_file(self.dir / "updater.pid")
            premise.append(detached_and_running(int(wait_for_file(self.dir / "worker.pid"))))
            os.kill(os.getpid(), signal.SIGTERM)

        sender = threading.Thread(target=send_once_running, daemon=True)
        sender.start()
        with patch.object(harness_upgrades.subprocess, "Popen", recording_popen):
            with self.assertRaises(harness_upgrades.Cancelled) as cancelled:
                harness_upgrades.run_updater(["sh", "-c", script], self.env, timeout=60)
        sender.join(timeout=10)
        self.assertEqual(premise, [True], "premise: the worker ran detached when SIGTERM was sent")
        self.assertEqual(cancelled.exception.signum, signal.SIGTERM)
        self.assertTrue(process_gone(int((self.dir / "worker.pid").read_text())))
        self.assertTrue(process_gone(int((self.dir / "updater.pid").read_text())))
        self.assertEqual(len(units), 1)
        self.assertRegex(units[0], r"^farhelm-drift-upgrade-[0-9a-f-]{36}\.scope$")
        self.assertFalse(harness_upgrades.scope_running(units[0]))
        self.assertEqual(signal.getsignal(signal.SIGTERM), before)

    def test_a_scope_that_cannot_be_confirmed_stopped_raises_instead_of_failing(self):
        """Why: after a failed stop a worker may still swap the binary, so nothing may run after the updater; a plain
        failed upgrade would let the fallback installer, the version probe and the cases go ahead. Spec: an updater
        past its deadline whose scope stop reports a problem (the real stop runs, then a seam adds the problem) raises
        `ContainmentLost` naming both the overrun and the problem."""
        real_stop = harness_upgrades.stop_scope

        def stop_then_fail(unit, child, name):
            real_stop(unit, child, name)
            return "injected stop failure"

        with patch.object(harness_upgrades, "stop_scope", stop_then_fail):
            with self.assertRaises(harness_upgrades.ContainmentLost) as lost:
                harness_upgrades.run_updater(["sh", "-c", "exec sleep 1000"], self.env, timeout=2)
        self.assertIn("did not finish within 2 seconds", str(lost.exception))
        self.assertIn("injected stop failure", str(lost.exception))

    def test_a_signal_arriving_as_the_updater_starts_still_stops_what_it_started(self):
        """Spec: SIGTERM delivered right as the updater's process is created (before any wait) raises `Cancelled`, and
        neither the updater's scope nor a worker it may have started is left running.

        What it proves is that the handler is in place before the start (the default SIGTERM would end the test
        process) and that an early cancellation ends cleanly. The worker has usually not started by then, so this
        test alone does not show a running scope being stopped; the SIGTERM test above does.
        """
        units = []
        real_popen = subprocess.Popen

        # Patching `Popen` reaches every subprocess call, the systemctl ones included; only the updater's start signals.
        def popen_then_signal(command, **kwargs):
            child = real_popen(command, **kwargs)
            if command[0] == "systemd-run":
                units.extend(arg.removeprefix("--unit=") for arg in command if arg.startswith("--unit="))
                signal.raise_signal(signal.SIGTERM)
            return child

        script = self.worker("exec sleep 1000") + "; exec sleep 1000"
        with patch.object(harness_upgrades.subprocess, "Popen", popen_then_signal):
            with self.assertRaises(harness_upgrades.Cancelled):
                harness_upgrades.run_updater(["sh", "-c", script], self.env, timeout=60)
        self.assertFalse(harness_upgrades.scope_running(units[0]))
        pid_file = self.dir / "worker.pid"
        if pid_file.exists() and pid_file.read_text().strip():
            self.assertTrue(process_gone(int(pid_file.read_text())))


class UpgradeContainmentTests(unittest.TestCase):
    """What the runner does when an updater cannot be contained or is cancelled; no updater or user manager needed."""

    def test_no_user_manager_fails_the_upgrade_without_running_the_updater(self):
        """Why: an upgrade with no scope could leave a worker swapping the binary under a drift case. Spec: with no
        user manager answering, the updater is never started and the upgrade fails naming the missing manager."""
        with patch.object(harness_upgrades, "user_manager_problem", return_value="no bus"), patch.object(
            harness_upgrades.subprocess, "Popen", side_effect=AssertionError("the updater must not start")
        ):
            ok, detail = harness_upgrades.run_updater(["fake", "update"], {"PATH": "/usr/bin:/bin"})
        self.assertFalse(ok)
        self.assertIn("systemd user manager", detail)
        self.assertIn("no bus", detail)

    def test_lost_containment_skips_the_fallback_installer_and_the_version_probe(self):
        """Why: the installer or the probe could run beside a worker that survived. Spec: when the updater raises
        `ContainmentLost`, `upgrade` lets it through without running the installer or probing the version again."""
        recipe = harness_upgrades.Recipe("fake", ("fake", "update"), installer_url="https://example.invalid/install")
        with tempfile.TemporaryDirectory() as directory:
            fake = Path(directory) / "fake"
            fake.write_text("#!/bin/sh\necho fake 1.0\n")
            fake.chmod(0o755)
            env = {"PATH": f"{directory}:/usr/bin:/bin"}
            probes = []
            real_version = harness_upgrades.version
            with patch.object(
                harness_upgrades, "run_updater", side_effect=harness_upgrades.ContainmentLost("scope still running")
            ), patch.object(
                harness_upgrades, "run_installer", side_effect=AssertionError("no installer after lost containment")
            ), patch.object(
                harness_upgrades, "version", lambda r, e: probes.append(1) or real_version(r, e)
            ):
                with self.assertRaises(harness_upgrades.ContainmentLost):
                    harness_upgrades.upgrade(recipe, env)
        self.assertEqual(len(probes), 1, "only the probe before the updater ran")

    def test_lost_containment_ends_the_run_before_anything_else_runs(self):
        """Why: a worker that survived a failed stop could swap a binary under the next harness's upgrade or a case.
        Spec: `prepare` turns `ContainmentLost` from the first harness's upgrade into `SystemExit(1)` and never upgrades
        the second."""
        calls = []

        def upgrade(recipe, env):
            calls.append(recipe.binary)
            raise harness_upgrades.ContainmentLost("scope still running")

        with patch.object(harness_upgrades, "upgrade", upgrade):
            with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(SystemExit) as stopped:
                runner["prepare"](["codex", "claude"], {}, True)
        self.assertEqual(stopped.exception.code, 1)
        self.assertEqual(calls, ["codex"])

    def test_a_cancelled_upgrade_ends_the_run_with_128_plus_the_signal(self):
        """Why: a cancelled upgrade must stop the run like a cancelled case, not test a half-upgraded CLI. Spec:
        `prepare` turns `Cancelled(SIGINT)` from an upgrade into `SystemExit(128 + SIGINT)`."""
        with patch.object(harness_upgrades, "upgrade", side_effect=harness_upgrades.Cancelled(signal.SIGINT)):
            with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(SystemExit) as stopped:
                runner["prepare"](["codex"], {}, True)
        self.assertEqual(stopped.exception.code, 128 + signal.SIGINT)


class UpgradePhaseTests(unittest.TestCase):
    """The runner brings each harness to its latest release before testing it, and says so when it could not."""

    def test_every_drift_case_has_a_recipe_for_the_binary_it_launches(self):
        """A new drift case without a recipe would quietly test a stale CLI; a renamed binary would upgrade nothing."""
        source = (DRIFT_SOURCE / "mod.rs").read_text()
        for name, recipe in harness_upgrades.RECIPES.items():
            self.assertIn(f'"{recipe.binary}"', source, name)
        categories = {
            path.name: re.findall(r"^\s+drift_([a-z][a-z0-9_]*) =>", path.read_text(), re.MULTILINE)
            for path in DRIFT_SOURCE.glob("*.rs")
        }
        self.assertEqual(sorted(categories["base.rs"]), sorted(harness_upgrades.RECIPES))
        for category, registered in categories.items():
            self.assertLessEqual(set(registered), harness_upgrades.RECIPES.keys(), category)

    def stand_in(self, directory, body):
        """Write an executable `fake` CLI whose `--version` prints the file `version` beside it.

        Its updater runs in a transient user scope like a real one (`harness_upgrades.run_updater`), so every test
        built on it needs a systemd user manager and is skipped, loudly, without one.
        """
        require_user_manager(self)
        path = Path(directory) / "fake"
        path.write_text(
            "#!/bin/sh\n"
            f'here="{directory}"\n'
            'if [ "$1" = --version ]; then echo "$QUIET" >> "$here/quiet"; cat "$here/version"; exit 0; fi\n'
            f"{body}\n"
        )
        path.chmod(0o755)
        (Path(directory) / "version").write_text("fake 1.0\n")
        return {"PATH": f"{directory}:/usr/bin:/bin"}

    def test_upgrade_reports_versions_and_names_the_updaters_reason_for_failing(self):
        """Upgraded, already current and failed are told apart by the updater's exit and the versions around it."""
        recipe = harness_upgrades.Recipe("fake", ("fake", "update"))
        with tempfile.TemporaryDirectory() as directory:
            env = self.stand_in(directory, f'echo "fake 2.0" > "{directory}/version"; echo done')
            self.assertEqual(harness_upgrades.upgrade(recipe, env), harness_upgrades.Upgrade("upgraded", "fake 1.0", "fake 2.0"))
            self.assertEqual(harness_upgrades.upgrade(recipe, env).status, "current")
        with tempfile.TemporaryDirectory() as directory:
            env = self.stand_in(directory, "echo checking; echo 'Error: [unauthenticated]'; exit 1")
            result = harness_upgrades.upgrade(recipe, env)
            self.assertEqual(result, harness_upgrades.Upgrade("failed", "fake 1.0", "fake 1.0", "Error: [unauthenticated]"))
        self.assertEqual(harness_upgrades.upgrade(recipe, {"PATH": "/nonexistent"}).status, "not_installed")

    def test_updaters_get_no_terminal_input_and_the_quiet_environment_reaches_every_probe(self):
        """An updater that asks a question must fail, not hang, and a launcher must not update during a probe."""
        recipe = harness_upgrades.Recipe(
            "fake", ("fake", "update"), quiet_env={"QUIET": "yes"}, upgrade_env={"QUIET": "upgrading"},
        )
        with tempfile.TemporaryDirectory() as directory:
            env = self.stand_in(directory, 'echo "$QUIET" > "$here/updater"; read answer || { echo no input; exit 1; }')
            result = harness_upgrades.upgrade(recipe, env)
            self.assertEqual((result.status, result.detail), ("failed", "no input"))
            self.assertEqual((Path(directory) / "updater").read_text(), "upgrading\n")
            self.assertEqual((Path(directory) / "quiet").read_text(), "yes\nyes\n")

    def test_the_installer_runs_only_after_the_updater_fails(self):
        """Cursor's and goose's updaters broke while their installers worked; a working updater is never bypassed."""
        recipe = harness_upgrades.Recipe(
            "fake", ("fake", "update"), installer_url="https://example.invalid/install", installer_env={"CONFIGURE": "false"},
        )
        installs = []

        def installer(url, env):
            installs.append((url, env.get("CONFIGURE")))
            return True, ""

        with tempfile.TemporaryDirectory() as directory, patch.object(harness_upgrades, "run_installer", installer):
            env = self.stand_in(directory, "exit 0")
            self.assertEqual(harness_upgrades.upgrade(recipe, env).status, "current")
            self.assertEqual(installs, [])
            env = self.stand_in(directory, "echo updater broke; exit 1")
            result = harness_upgrades.upgrade(recipe, env)
            self.assertEqual(result.status, "current")
            self.assertIn("updater broke", harness_upgrades.describe("fake", result))
            self.assertEqual(installs, [("https://example.invalid/install", "false")])

    def test_installer_redirects_require_https_at_each_hop(self):
        """Refuse a non-TLS redirect before a downloader can connect to its target."""
        source = urllib.request.Request("https://example.invalid/install")
        handler = harness_upgrades.InstallerRedirects()
        target = "https://download.invalid/install"
        self.assertEqual(handler.redirect_request(source, None, 302, "Found", {}, target).full_url, target)
        for scheme in ("http", "ftp"):
            with self.subTest(scheme=scheme), self.assertRaisesRegex(urllib.error.URLError, "must use HTTPS"):
                handler.redirect_request(source, None, 302, "Found", {}, f"{scheme}://download.invalid/install")

    def test_installer_checks_the_final_url_before_reading_bytes(self):
        """A response outside HTTPS must never supply bytes to the fallback shell."""
        with patch.object(urllib.request, "build_opener") as build, patch.object(harness_upgrades, "run_updater") as run:
            response = build.return_value.open.return_value.__enter__.return_value
            response.geturl.return_value = "http://download.invalid/install"
            ok, detail = harness_upgrades.run_installer("https://example.invalid/install", {})
            self.assertFalse(ok)
            self.assertIn("must use HTTPS", detail)
            self.assertIsInstance(build.call_args.args[0], harness_upgrades.InstallerRedirects)
            response.read.assert_not_called()
            run.assert_not_called()

    def run_main(self, argv, upgrade_result=None):
        """Run the runner's main against stand-ins, returning its exit code, output and what it upgraded or probed."""
        calls = []
        upgrade_result = upgrade_result or harness_upgrades.Upgrade("current", "1.0", "1.0")

        def upgrade(recipe, env):
            calls.append(("upgrade", recipe.binary))
            return upgrade_result

        def probe(recipe, env):
            calls.append(("probe", recipe.binary))
            return harness_upgrades.Upgrade("skipped", "1.0", "1.0")

        with (
            patch.object(harness_upgrades, "upgrade", upgrade),
            patch.object(harness_upgrades, "probe", probe),
            patch.dict(globals_of_runner, {
                "build_binary": lambda env: calls.append(("build",)) or Path("/synthetic/e2e"),
                "discover": lambda binary, env: {("base", "claude"): "case-claude", ("base", "newcomer"): "case-newcomer"},
                "recorded_case": lambda binary, test, name, env: calls.append(("case", name)) or runner["Result"]("passed", "ok"),
            }),
            contextlib.redirect_stdout(io.StringIO()) as output,
        ):
            code = runner["main"](argv)
        return code, output.getvalue(), calls

    def test_cases_run_after_their_upgrade_and_a_failed_upgrade_fails_the_run(self):
        """A pass on a CLI that could not be upgraded is not the evidence the suite exists to give."""
        code, output, calls = self.run_main(["--harness", "claude"])
        self.assertEqual((code, calls), (0, [("build",), ("upgrade", "claude"), ("case", "base/claude")]))
        self.assertRegex(output, r"PASSED +base +claude +1\.0")
        failed = harness_upgrades.Upgrade("failed", "1.0", "1.0", "offline")
        code, output, calls = self.run_main(["--harness", "claude"], failed)
        self.assertEqual((code, calls[-1]), (1, ("case", "base/claude")))
        self.assertIn("UPGRADE FAILED claude", output)
        self.assertIn("UPGRADE FAILED for claude", output)

    def test_a_case_without_a_recipe_is_reported_as_a_failed_upgrade(self):
        """Adding a harness to the suite must come with a decision about how it gets upgraded."""
        code, output, calls = self.run_main(["--harness", "newcomer"])
        self.assertEqual((code, calls), (1, [("build",), ("case", "base/newcomer")]))
        self.assertIn("no recipe in scripts/harness_upgrades.py", output)

    def test_no_upgrade_probes_versions_and_upgrade_only_builds_nothing(self):
        """Reproducing a failure needs the same version; upgrading alone needs neither the build nor a vendor turn."""
        code, _, calls = self.run_main(["--harness", "claude", "--no-upgrade"])
        self.assertEqual((code, calls), (0, [("build",), ("probe", "claude"), ("case", "base/claude")]))
        code, _, calls = self.run_main(["--upgrade-only", "--harness", "codex"])
        self.assertEqual((code, calls), (0, [("upgrade", "codex")]))
        code, _, calls = self.run_main(["--upgrade-only"])
        self.assertEqual(calls, [("upgrade", recipe.binary) for recipe in harness_upgrades.RECIPES.values()])


if __name__ == "__main__":
    unittest.main()
