#!/usr/bin/env python3
"""Check browser batch decisions against real bounded report collection.

Synthetic reports isolate scheduling policy without launching a browser. The
recorder's browser lifecycle is covered separately by focused sandbox tests.
"""
import importlib.util
import contextlib
import copy
import io
import json
import pathlib
import signal
import tempfile
import unittest
import uuid
from types import SimpleNamespace
from unittest import mock


def load(name, filename):
    """Load a CLI or fixture module without running its main entry point."""
    spec = importlib.util.spec_from_file_location(name, pathlib.Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


hunt = load("browser_hunt", "hunt-browser-tests.py")
fixtures = load("browser_fixtures", "test-playwright-report.py")
recorder = hunt.test_hunt.load_recorder()


class BrowserHuntTest(unittest.TestCase):
    """A retry needs complete evidence; ordinary failure remains visible."""

    def write_attempt(self, attempt, *, actual="passed", expected="passed", outcome="expected",
                      terminal="passed", code=0, mutate=None, mutate_report=None, mutate_data=None):
        """Publish complete synthetic evidence at the scheduler's assigned attempt root."""
        run = attempt / str(uuid.uuid4())
        run.mkdir(parents=True)
        output = run / fixtures.adapter.ARTIFACTS
        data = fixtures.report(output, status=outcome, expected=expected,
                               results=[{"status": actual, "retry": 0}])
        if mutate_data:
            mutate_data(data)
        policy = fixtures.policy_report(output)
        policy["status"] = terminal
        (run / fixtures.adapter.REPORT).write_text(json.dumps(data))
        (run / fixtures.adapter.POLICY).write_text(json.dumps(policy))
        manifest = {
            "schema_version": 1, "run_id": run.name, "outcome": "completed",
            "child_status": recorder.child_status(code),
            "recorder": {"exit_code": code, "forced_cleanup": False,
                         "cleanup_limit": None, "error": None},
            "runner": {"name": "playwright", "prepared": True,
                       "report": fixtures.adapter.collect(run)},
            "console": {"worker_finished": True, "dropped_or_pending_bytes": 0},
            "test_traces": {"collection": {"collection_complete": True}},
            "output": {"eof_observed": True, "truncated": False, "omitted_bytes": 0},
        }
        if mutate:
            mutate(manifest)
        (run / "manifest.json").write_text(json.dumps(manifest))
        if mutate_report:
            mutate_report(run)

    def summarize(self, **options):
        """Classify a real retained report with only the requested contract altered."""
        with tempfile.TemporaryDirectory() as tmp:
            attempt = pathlib.Path(tmp) / "attempt-0001"
            self.write_attempt(attempt, **options)
            return hunt.summarize_attempt(1, attempt, options.get("code", 0),
                                          SimpleNamespace(received=None), recorder)

    def test_pass_failure_expected_failure_and_case_timeout(self):
        """Completed assertions may repeat, including expected failures and case timeouts."""
        self.assertEqual(self.summarize()["status"], "passed")
        self.assertEqual(self.summarize(actual="failed", expected="failed")["status"], "passed")
        for actual in ("failed", "timedOut"):
            with self.subTest(actual=actual):
                summary = self.summarize(actual=actual, outcome="unexpected", terminal="failed", code=1)
                self.assertEqual(summary["status"], "failed", summary)

    def test_unfinished_capture_and_status_contradictions_stop_repetition(self):
        """A complete browser report alone cannot authorize another child process."""
        mutations = (
            lambda m: m.update(schema_version=True),
            lambda m: m.update(run_id="foreign"),
            lambda m: m.update(outcome="timeout"),
            lambda m: m["recorder"].update(exit_code=True),
            lambda m: m["child_status"].update(raw_returncode=True),
            lambda m: m["child_status"].update(exit_code=False),
            lambda m: m["console"].update(worker_finished=False),
            lambda m: m["console"].update(dropped_or_pending_bytes=1),
            lambda m: m["test_traces"]["collection"].update(collection_complete=False),
            lambda m: m["output"].update(eof_observed=False),
            lambda m: m["output"].update(truncated=True),
            lambda m: m["recorder"].update(cleanup_limit="descendant survived"),
            lambda m: m["runner"].update(prepared=False),
        )
        for index, mutate in enumerate(mutations):
            with self.subTest(index=index):
                summary = self.summarize(mutate=mutate)
                self.assertEqual(summary["status"], "incomplete", summary)

    def test_missing_or_modified_reports_have_no_inferred_counts(self):
        """The index must not repeat stale counts after raw reports disappear or change."""
        mutations = (
            lambda run: (run / fixtures.adapter.REPORT).unlink(),
            lambda run: (run / fixtures.adapter.POLICY).write_text("{}"),
        )
        for mutate in mutations:
            summary = self.summarize(mutate_report=mutate)
            self.assertEqual(summary["status"], "incomplete", summary)
            self.assertNotIn("counts", summary)

    def test_skip_and_interruption_are_not_executed_success(self):
        """Both engine names being present does not establish actual test execution."""
        for actual, expected, terminal, code in (
            ("skipped", "skipped", "passed", 0),
            ("interrupted", "passed", "interrupted", 130),
        ):
            summary = self.summarize(actual=actual, expected=expected, outcome="skipped",
                                     terminal=terminal, code=code)
            self.assertEqual(summary["status"], "incomplete", summary)

    def test_batch_retains_failure_and_stops_on_incomplete_or_cancellation(self):
        """The browser classifier drives real scheduling and preserves every finished attempt."""
        for mode, expected_exit, expected_states in (
            ("failure-then-pass", 1, ["failed", "passed"]),
            ("incomplete", 125, ["incomplete"]),
            ("cancelled", 143, ["cancelled"]),
        ):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as tmp:
                args = hunt.parse_args(["--repeat", "2", "--timeout", "60", "--output-root", tmp,
                                        "--", "npx", "playwright", "test", "fixture.spec.ts"])
                calls = []

                def record(argv, *, signal_intent):
                    """Stand in for execution while preserving real per-attempt report files."""
                    calls.append(argv)
                    self.assertEqual(argv[argv.index("--runner") + 1], "playwright")
                    self.assertEqual(argv[argv.index("--tmux") + 1], "required")
                    self.assertEqual(argv[argv.index("--kind") + 1], "repetition")
                    attempt = pathlib.Path(argv[argv.index("--output-root") + 1])
                    if mode == "failure-then-pass" and len(calls) == 1:
                        self.write_attempt(attempt, actual="failed", outcome="unexpected", terminal="failed", code=1)
                        return 1
                    self.write_attempt(attempt, mutate=(
                        lambda m: m["output"].update(eof_observed=False)) if mode == "incomplete" else None)
                    if mode == "cancelled":
                        signal_intent.handle(signal.SIGTERM, None)
                    return 0

                with mock.patch.object(recorder, "run", record), contextlib.redirect_stderr(io.StringIO()):
                    code = hunt.test_hunt.run_batch(args, recorder, runner_name="playwright",
                                                   concurrency=hunt.CONCURRENCY, summarize=hunt.summarize_attempt)
                self.assertEqual(code, expected_exit)
                indexes = list(pathlib.Path(tmp).glob("*/index.json"))
                self.assertEqual(len(indexes), 1)
                index = json.loads(indexes[0].read_text())
                self.assertEqual([a["status"] for a in index["attempts"]], expected_states)
                self.assertEqual(len(calls), len(expected_states))
                self.assertEqual(index["not_started"], 2 - len(calls))
                self.assertEqual(len({a["evidence"] for a in index["attempts"]}), len(calls))

    def test_mixed_passes_and_unstarted_cases_stop_repetition(self):
        """Executed cases in both engines cannot conceal a third case which never started."""
        def add_unstarted(data):
            """Preserve both passing cases while adding a distinct blocked Chromium case."""
            blocked = copy.deepcopy(data["suites"][0]["specs"][0])
            blocked["id"] = "blocked-case"
            blocked["tests"][0].update(status="skipped", results=[])
            data["suites"][0]["specs"].append(blocked)
            data["stats"]["skipped"] = 1

        summary = self.summarize(mutate_data=add_unstarted)
        self.assertTrue(summary["report_complete"], summary)
        self.assertEqual(summary["counts"]["passed"], 2)
        self.assertEqual(summary["counts"]["not_run"], 1)
        self.assertEqual(summary["status"], "incomplete", summary)

    def test_selection_and_budget_are_explicit(self):
        """Dry planning rejects implicit full-suite work and fixed-policy overrides."""
        for selected in ([], ["--project", "chromium"], ["--repeat-each=20"]):
            args = hunt.parse_args(["--repeat", "2", "--timeout", "60", "--",
                                    "npx", "playwright", "test", *selected])
            with self.assertRaises(ValueError):
                hunt.validate_request(args, recorder)
        args = hunt.parse_args(["--repeat", "2", "--timeout", "60", "--",
                                "npx", "playwright", "test", "fixture.spec.ts"])
        hunt.validate_request(args, recorder)
        self.assertFalse(hunt.plan(args)["execute"])
        self.assertEqual(hunt.plan(args)["maximum_child_command_seconds"], 120)


if __name__ == "__main__":
    unittest.main()
