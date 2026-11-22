#!/usr/bin/env python3
"""Exercise the manual drift runner using stand-ins. No test starts a vendor CLI."""

import contextlib
import io
import json
from pathlib import Path
import runpy
import signal
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from agent_screen_host import OUTER_SESSION_MARKERS

runner = runpy.run_path(str(Path(__file__).with_name("run-harness-drift.py")))
# runpy returns a copy of the globals. Patch the actual function namespace so main uses the stand-ins.
globals_of_runner = runner["main"].__globals__


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
            "real_agent_capture::drift::drift_codex: test\n"
            "real_agent_capture::drift::drift_antigravity: test\n\n"
            "3 tests, 0 benchmarks\n"
        )
        outputs = [
            subprocess.CompletedProcess([], 0, json.dumps(artifact)),
            subprocess.CompletedProcess([], 0, listing),
        ]
        env = {"CARGO_TARGET_DIR": "/synthetic/custom/target"}
        with patch.object(subprocess, "run", side_effect=outputs) as run:
            binary = runner["build_binary"](env)
            cases = runner["discover"](binary, env)
        self.assertEqual(list(cases), ["antigravity", "codex"])
        self.assertEqual(cases["codex"], "real_agent_capture::drift::drift_codex")
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
            manifest.write_text(json.dumps({"schema_version": 1, "outcome": "timed_out", "recorder": {"error": None}}))
            result = runner["read_verdict"](verdict, 124, evidence)
            self.assertEqual(result.verdict, "failed")
            self.assertIn("timed out", result.detail)
            manifest.write_text(json.dumps({
                "schema_version": 1, "outcome": "recorder-error", "recorder": {"error": "fixture refused"},
            }))
            result = runner["read_verdict"](verdict, 125, evidence)
            self.assertEqual(result.verdict, "failed")
            self.assertIn("fixture refused", result.detail)
            for version in [2, True, 1.0]:
                manifest.write_text(json.dumps({"schema_version": version, "outcome": "timed_out"}))
                result = runner["read_verdict"](verdict, 124, evidence)
                self.assertIn("without a readable drift verdict", result.detail)

    def test_every_selected_case_runs_and_only_an_explicit_skip_fails(self):
        """Missing prerequisites are distinguishable and do not prevent another harness from running."""
        cases = {"claude": "case-claude", "codex": "case-codex", "grok": "case-grok"}
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
                }),
                contextlib.redirect_stdout(io.StringIO()) as output,
            ):
                self.assertEqual(runner["main"](requested), code)
            self.assertEqual(called, ["grok", "codex"] if requested else list(cases))
            self.assertIn("COULD NOT RUN", output.getvalue())
            self.assertIn("PASSED", output.getvalue())

    def test_list_discovers_but_never_launches(self):
        """Listing the suite must spend zero vendor turns."""
        def unexpected(*args):
            self.fail("listing must never run a case")

        with patch.dict(globals_of_runner, {
            "build_binary": lambda env: Path("/synthetic/e2e"),
            "discover": lambda binary, env: {"claude": "case-claude"},
            "recorded_case": unexpected,
        }), contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(runner["main"](["--list"]), 0)
        self.assertEqual(output.getvalue().strip(), "claude")

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
        """The recorder owns the test process group and must finish cleanup before the runner exits."""
        prior_handler = signal.getsignal(signal.SIGTERM)
        with patch.object(subprocess, "Popen") as spawn:
            recorder = spawn.return_value.__enter__.return_value

            def cancel(*_args, **_kwargs):
                signal.raise_signal(signal.SIGTERM)
                return spawn.return_value

            spawn.side_effect = cancel
            recorder.wait.return_value = 128 + signal.SIGTERM
            with self.assertRaises(SystemExit) as stopped:
                runner["recorded_case"](Path("/synthetic/e2e"), "exact-case", "grok", {"PATH": "bin"})
        recorder.send_signal.assert_called_once_with(signal.SIGTERM)
        recorder.wait.assert_called_once()
        self.assertEqual(stopped.exception.code, 128 + signal.SIGTERM)
        self.assertEqual(signal.getsignal(signal.SIGTERM), prior_handler)

    def test_termination_during_a_failed_spawn_stops_the_suite(self):
        """A spawn error may fail one case, but a simultaneous cancellation ends the run."""
        def failed_spawn(*_args, **_kwargs):
            signal.raise_signal(signal.SIGTERM)
            raise OSError("fixture spawn failed")

        with patch.object(subprocess, "Popen", side_effect=failed_spawn):
            with self.assertRaises(SystemExit) as stopped:
                runner["recorded_case"](Path("/synthetic/e2e"), "exact-case", "grok", {"PATH": "bin"})
        self.assertEqual(stopped.exception.code, 128 + signal.SIGTERM)


if __name__ == "__main__":
    unittest.main()
