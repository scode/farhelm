#!/usr/bin/env python3
"""Focused tests for the finite nextest hunt's scheduling and evidence rules."""

from __future__ import annotations

import importlib.util
import json
import os
import pathlib
import signal
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import uuid


ROOT = pathlib.Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts" / "hunt-rust-tests.py"
RECORDER_SCRIPT = ROOT / "scripts" / "record-test-run.py"


def load(path: pathlib.Path, name: str):
    """Load a script as a module without creating checkout bytecode."""

    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


HUNT = load(SCRIPT, "hunt_fixture")
RECORDER = load(RECORDER_SCRIPT, "recorder_fixture_for_hunt")


class FakeNextest:
    """Provide only the recorder seams needed to exercise batch scheduling."""

    def __init__(self, base: pathlib.Path, exits: list[int], complete: list[bool] | None = None,
                 modes: list[str] | None = None, counts: list[dict[str, int]] | None = None):
        self.base = base
        self.exits = exits
        self.complete = complete or [True] * len(exits)
        self.modes = modes or ["normal"] * len(exits)
        self.counts = counts or [None] * len(exits)
        self.calls = 0

    class _Nextest:
        @staticmethod
        def selection_args(command):
            return RECORDER.test_run_nextest.selection_args(command)

        read_regular = staticmethod(RECORDER.test_run_nextest.read_regular)

    SignalIntent = RECORDER.SignalIntent
    UsageRefusal = RECORDER.UsageRefusal
    child_status = staticmethod(RECORDER.child_status)
    test_run_nextest = _Nextest()

    def default_output_root(self, _environment):
        return self.base

    @staticmethod
    def checkout_marker_ancestor(_cwd):
        return None

    @staticmethod
    def prepare_root(requested, _checkout, _ambient):
        requested.mkdir(mode=0o700, parents=True, exist_ok=True)
        os.chmod(requested, 0o700)
        return requested

    def run(self, argv, *, signal_intent):
        """Write the same fixed-layout evidence shape the real recorder publishes."""

        index = argv.index("--output-root")
        root = pathlib.Path(argv[index + 1])
        run_dir = root / str(uuid.uuid4())
        run_dir.mkdir(mode=0o700)
        passed = self.exits[self.calls] == 0
        report_complete = self.complete[self.calls]
        counts = self.counts[self.calls] or {
            "tests": 1, "passed": int(passed), "skipped": 0,
            "failures": int(not passed), "errors": 0, "flaky": 0,
        }
        manifest = {
            "schema_version": 1,
            "run_id": run_dir.name,
            "outcome": "completed",
            "command": {"argv": ["cargo-nextest-fixture"]},
            "child_status": RECORDER.child_status(self.exits[self.calls]),
            "recorder": {"exit_code": self.exits[self.calls] if self.exits[self.calls] >= 0 else 128 - self.exits[self.calls],
                         "forced_cleanup": False, "error": None, "cleanup_limit": None},
            "output": {"eof_observed": True, "truncated": False, "omitted_bytes": 0},
            "console": {"worker_finished": True, "dropped_or_pending_bytes": 0},
            "test_traces": {"collection": {"status": "complete", "collection_complete": True}},
            "runner": {"name": "nextest", "prepared": True, "report": {
                "complete": report_complete,
                "counts": counts,
            }},
        }
        mode = self.modes[self.calls]
        if mode == "nested-status":
            manifest["recorder"]["forced_cleanup"] = {"unexpected": "object"}
        elif mode == "contradictory-status":
            manifest["child_status"]["signal"] = 15
        elif mode == "cleanup-limit":
            manifest["recorder"]["cleanup_limit"] = "escaped descendant may remain"
        elif mode == "missing-eof":
            manifest["output"]["eof_observed"] = False
        elif mode == "truncated-output":
            manifest["output"].update(truncated=True, omitted_bytes=1)
        elif mode == "schema":
            manifest["schema_version"] = 2
        elif mode == "trace-incomplete":
            manifest["test_traces"]["collection"].update(status="incomplete", collection_complete=False)
        elif mode == "trace-uncollected":
            manifest["test_traces"]["collection"] = {"status": "uncollected"}
        elif mode == "trace-missing":
            del manifest["test_traces"]
        elif mode == "console-missing":
            del manifest["console"]
        elif mode == "console-worker":
            manifest["console"]["worker_finished"] = False
        if mode == "malformed":
            (run_dir / "manifest.json").write_text("{", encoding="utf-8")
        elif mode == "oversized":
            (run_dir / "manifest.json").write_bytes(b"x" * (HUNT.MANIFEST_LIMIT + 1))
        elif mode == "symlink":
            target = root / "manifest-target"
            target.write_text(json.dumps(manifest), encoding="utf-8")
            (run_dir / "manifest.json").symlink_to(target)
        else:
            (run_dir / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
        result = manifest["recorder"]["exit_code"]
        self.calls += 1
        return result


class HuntTest(unittest.TestCase):
    """Exercise the finite scheduler without Rust, tmux, or environment mutation."""

    def command(self):
        return ["cargo", "nextest", "run", "-p", "fixture", "--lib"]

    def args(self, base, repeat=2, execute=True):
        return HUNT.argparse.Namespace(
            repeat=repeat, timeout=0.25, output_root=base, execute=execute, command=self.command()
        )

    def test_plan_has_no_filesystem_side_effect(self):
        """Plan mode validates and describes a batch without creating its root."""

        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary) / "evidence"
            result = subprocess.run(
                [sys.executable, str(SCRIPT), "--repeat", "2", "--timeout", "0.25",
                 "--output-root", str(root), "--", *self.command()],
                cwd=ROOT, capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(root.exists())
            plan = json.loads(result.stdout)
            self.assertEqual(plan["maximum_child_command_seconds"], 0.5)

    def test_scope_policy_rejects_runner_only_flags(self):
        """A feature or lock flag alone cannot silently select the whole workspace."""

        with tempfile.TemporaryDirectory() as temporary:
            args = self.args(pathlib.Path(temporary), execute=False)
            args.command = ["cargo", "nextest", "run", "--locked"]
            with self.assertRaises(ValueError):
                HUNT.validate_request(args, RECORDER)

    def test_plan_does_not_create_import_bytecode(self):
        """Default CLI planning leaves source untouched without relying on caller flags."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            scripts = root / "scripts"
            scripts.mkdir()
            for name in ("hunt-rust-tests.py", "test_hunt.py", "record-test-run.py",
                         "test_run_nextest.py", "test_run_traces.py"):
                shutil.copyfile(ROOT / "scripts" / name, scripts / name)
            environment = dict(os.environ)
            environment.pop("PYTHONDONTWRITEBYTECODE", None)
            result = subprocess.run(
                [sys.executable, str(scripts / "hunt-rust-tests.py"), "--repeat", "1",
                 "--timeout", "1", "--", *self.command()],
                cwd=root, env=environment, capture_output=True, text=True, timeout=10,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse((scripts / "__pycache__").exists())

    def test_allowlist_rejects_policy_override(self):
        """A scope cannot smuggle a runner-policy option through the hunt."""

        with tempfile.TemporaryDirectory() as temporary:
            args = self.args(pathlib.Path(temporary), execute=False)
            args.command = ["cargo", "nextest", "run", "-p", "fixture", "--profile", "custom"]
            with self.assertRaises(ValueError):
                HUNT.validate_request(args, RECORDER)

    def test_all_pass_batch_and_distinct_private_attempt_roots(self):
        """A complete all-pass batch records every distinct attempt evidence path."""

        with tempfile.TemporaryDirectory() as temporary:
            base = pathlib.Path(temporary)
            fake = FakeNextest(base, [0, 0])
            self.assertEqual(HUNT.run_batch(self.args(base), fake), 0)
            index = json.loads(next(base.glob("*/index.json")).read_text())
            self.assertEqual(index["state"], "completed")
            evidence = [item["evidence"] for item in index["attempts"]]
            self.assertEqual(len(set(evidence)), 2)
            self.assertEqual(index["finished"], 2)

    def test_maximum_attempt_index_stays_bounded(self):
        """The largest supported batch can publish its final index."""

        with tempfile.TemporaryDirectory() as temporary:
            base = pathlib.Path(temporary)
            fake = FakeNextest(base, [0] * 1000)
            self.assertEqual(HUNT.run_batch(self.args(base, repeat=1000), fake), 0)
            index_path = next(base.glob("*/index.json"))
            self.assertLessEqual(index_path.stat().st_size, HUNT.INDEX_LIMIT)
            self.assertEqual(len(json.loads(index_path.read_text())["attempts"]), 1000)

    def test_failed_attempt_remains_failed_after_later_pass(self):
        """A later pass cannot erase an earlier ordinary test failure."""

        with tempfile.TemporaryDirectory() as temporary:
            base = pathlib.Path(temporary)
            fake = FakeNextest(base, [100, 0])
            code = HUNT.run_batch(self.args(base), fake)
            self.assertEqual(code, 1)
            index = json.loads(next(base.glob("*/index.json")).read_text())
            self.assertEqual(index["state"], "failed")
            self.assertEqual([item["status"] for item in index["attempts"]], ["failed", "passed"])

    def test_incomplete_report_is_not_zero_passes(self):
        """A recorder success with incomplete report evidence makes the batch incomplete."""

        with tempfile.TemporaryDirectory() as temporary:
            base = pathlib.Path(temporary)
            fake = FakeNextest(base, [0], [False])
            self.assertEqual(HUNT.run_batch(self.args(base, repeat=1), fake), 125)
            index = json.loads(next(base.glob("*/index.json")).read_text())
            self.assertEqual(index["state"], "incomplete")
            self.assertFalse(index["attempts"][0]["report_complete"])
            self.assertNotIn("counts", index["attempts"][0])

    def test_malformed_oversized_and_symlink_manifests_are_incomplete(self):
        """Fixed manifest reads fail closed for malformed, oversized, and linked files."""

        for mode in ("malformed", "oversized", "symlink"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                base = pathlib.Path(temporary)
                fake = FakeNextest(base, [0], modes=[mode])
                self.assertEqual(HUNT.run_batch(self.args(base, repeat=1), fake), 125)

    def test_contradictory_and_all_skipped_counts_are_incomplete(self):
        """Contradictory or all-skipped reports never become passing attempts."""

        cases = [
            {"tests": 2, "passed": 1, "skipped": 0, "failures": 0, "errors": 0, "flaky": 0},
            {"tests": 1, "passed": 0, "skipped": 1, "failures": 0, "errors": 0, "flaky": 0},
        ]
        for counts in cases:
            with self.subTest(counts=counts), tempfile.TemporaryDirectory() as temporary:
                base = pathlib.Path(temporary)
                fake = FakeNextest(base, [0], counts=[counts])
                self.assertEqual(HUNT.run_batch(self.args(base, repeat=1), fake), 125)
                if counts["tests"] != sum(counts[key] for key in ("passed", "skipped", "failures", "errors")):
                    index = json.loads(next(base.glob("*/index.json")).read_text())
                    self.assertNotIn("counts", index["attempts"][0])

    def test_incomplete_lifecycle_and_runner_failures_stop_repetition(self):
        """Complete test reports cannot authorize repetition after lost runner evidence."""

        for mode, child_exit in (("cleanup-limit", 0), ("missing-eof", 0), ("truncated-output", 0),
                                 ("schema", 0), ("normal", -15), ("normal", 1), ("normal", 101)):
            with self.subTest(mode=mode, child_exit=child_exit), tempfile.TemporaryDirectory() as temporary:
                base = pathlib.Path(temporary)
                fake = FakeNextest(base, [child_exit, 0], modes=[mode, "normal"])
                self.assertEqual(HUNT.run_batch(self.args(base), fake), 125)
                self.assertEqual(fake.calls, 1)

    def test_checkout_root_refusal_has_distinct_status(self):
        """A preflight refusal must not look like an ordinary failed test batch."""

        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--repeat", "1", "--timeout", "1", "--execute",
             "--output-root", ".", "--", *self.command()],
            cwd=ROOT, capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(result.returncode, 125)
        self.assertIn("output root is inside the tested checkout", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_incomplete_traces_and_console_stop_even_after_ordinary_failure(self):
        """A valid test report cannot authorize another run with unfinished evidence work."""
        for mode in ("trace-incomplete", "trace-uncollected", "trace-missing",
                     "console-missing", "console-worker"):
            for child_exit in (0, 100):
                with self.subTest(mode=mode, child_exit=child_exit), tempfile.TemporaryDirectory() as temporary:
                    base = pathlib.Path(temporary)
                    fake = FakeNextest(base, [child_exit, 0], modes=[mode, "normal"])
                    self.assertEqual(HUNT.run_batch(self.args(base), fake), 125)
                    self.assertEqual(fake.calls, 1)
                    index = json.loads(next(base.glob("*/index.json")).read_text())
                    self.assertEqual(index["attempts"][0]["child_status"]["raw_returncode"], child_exit)

    def test_nextest_execution_errors_stop_an_otherwise_repeatable_exit(self):
        """Exit 100 also covers execution errors, which cannot authorize another attempt."""
        for failures in (0, 1):
            with self.subTest(failures=failures), tempfile.TemporaryDirectory() as temporary:
                base = pathlib.Path(temporary)
                counts = {"tests": 1 + failures, "passed": 0, "skipped": 0,
                          "failures": failures, "errors": 1, "flaky": 0}
                fake = FakeNextest(base, [100, 0], counts=[counts, None])
                self.assertEqual(HUNT.run_batch(self.args(base), fake), 125)
                self.assertEqual(fake.calls, 1)
                attempt = json.loads(next(base.glob("*/index.json")).read_text())["attempts"][0]
                self.assertEqual(attempt["counts"], counts)
                self.assertEqual(attempt["child_status"]["raw_returncode"], 100)

    def test_blocked_console_worker_prevents_another_attempt(self):
        """An undrained owned pipe must leave at most one recorder worker in a batch."""
        for child_exit in (0, 100):
            with self.subTest(child_exit=child_exit), tempfile.TemporaryDirectory() as temporary:
                base = pathlib.Path(temporary)
                read_fd, write_fd = os.pipe()
                workers = []

                class BlockedConsoleFake(FakeNextest):
                    def run(self, argv, *, signal_intent):
                        result = super().run(argv, signal_intent=signal_intent)
                        worker = RECORDER.ConsoleForwarder(write_fd)
                        workers.append(worker)
                        worker.offer(b"x" * (1024 * 1024))
                        console = worker.finish()
                        attempt = pathlib.Path(argv[argv.index("--output-root") + 1])
                        manifest_path = next(attempt.glob("*/manifest.json"))
                        manifest = json.loads(manifest_path.read_text())
                        manifest["console"] = console
                        manifest_path.write_text(json.dumps(manifest))
                        return result

                try:
                    fake = BlockedConsoleFake(base, [child_exit, 0])
                    self.assertEqual(HUNT.run_batch(self.args(base), fake), 125)
                    self.assertEqual(fake.calls, 1)
                    self.assertEqual(len(workers), 1)
                    self.assertTrue(workers[0]._thread.is_alive())
                finally:
                    # Closing only our read end wakes the blocked write. Join
                    # before releasing its descriptor so no reused fd is touched.
                    os.close(read_fd)
                    for worker in workers:
                        worker._thread.join(timeout=5)
                    os.close(write_fd)
                self.assertTrue(all(not worker._thread.is_alive() for worker in workers))

    def test_invalid_status_fields_are_not_copied_into_index(self):
        """Untrusted status shapes cannot inflate an index or contradict a pass."""

        for mode in ("nested-status", "contradictory-status"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                base = pathlib.Path(temporary)
                fake = FakeNextest(base, [0], modes=[mode])
                self.assertEqual(HUNT.run_batch(self.args(base, repeat=1), fake), 125)
                index = json.loads(next(base.glob("*/index.json")).read_text())
                self.assertNotIn("manifest_recorder", index["attempts"][0])

    def test_cancellation_between_attempts_stops_scheduling(self):
        """A shared signal intent prevents the next attempt from starting."""

        class CancellingFake(FakeNextest):
            def run(self, argv, *, signal_intent):
                result = super().run(argv, signal_intent=signal_intent)
                signal_intent.received = signal.SIGTERM
                return result

        with tempfile.TemporaryDirectory() as temporary:
            base = pathlib.Path(temporary)
            fake = CancellingFake(base, [0, 0])
            self.assertEqual(HUNT.run_batch(self.args(base), fake), 128 + signal.SIGTERM)
            index = json.loads(next(base.glob("*/index.json")).read_text())
            self.assertEqual(index["state"], "cancelled")
            self.assertEqual(fake.calls, 1)

    def test_shared_intent_interrupts_real_harmless_child(self):
        """A recorder subprocess cleans a real child after explicit readiness."""

        with tempfile.TemporaryDirectory() as temporary:
            base = pathlib.Path(temporary)
            ready = base / "child-ready"
            (base / "evidence").mkdir(mode=0o700)
            recorder_argv = [
                "--kind", "repetition", "--selection", "fixture", "--concurrency", "none", "--tmux", "none",
                "--output-root", str(base / "evidence"), "--", sys.executable, "-c",
                f"import pathlib,time; pathlib.Path({str(ready)!r}).touch(); time.sleep(30)",
            ]
            helper = (
                "import importlib.util, json, pathlib, sys\n"
                "sys.dont_write_bytecode = True\n"
                f"script = pathlib.Path({str(RECORDER_SCRIPT)!r})\n"
                "sys.path.insert(0, str(script.parent))\n"
                "spec = importlib.util.spec_from_file_location('recorder_child', script)\n"
                "module = importlib.util.module_from_spec(spec); sys.modules[spec.name] = module; spec.loader.exec_module(module)\n"
                f"intent = module.SignalIntent()\nresult = module.run({recorder_argv!r}, signal_intent=intent)\n"
                f"pathlib.Path({str(base / 'result')!r}).write_text(json.dumps({{'result': result, 'signal': intent.received}}))\n"
                "raise SystemExit(result)\n"
            )
            process = subprocess.Popen([sys.executable, "-c", helper], cwd=ROOT, env=dict(os.environ))
            try:
                deadline = time.monotonic() + 10
                while not ready.exists() and time.monotonic() < deadline:
                    time.sleep(0.02)
                self.assertTrue(ready.exists())
                process.send_signal(signal.SIGTERM)
                process.communicate(timeout=10)
            finally:
                # A failed readiness assertion must still let the recorder
                # terminate and reap its child before temporary state vanishes.
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=15)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
            self.assertFalse(process.poll() is None)
            result = json.loads((base / "result").read_text())
            self.assertEqual(result["result"], 128 + signal.SIGTERM)
            self.assertEqual(result["signal"], signal.SIGTERM)
            run_dirs = list((base / "evidence").iterdir())
            self.assertEqual(len(run_dirs), 1)
            manifest = json.loads((run_dirs[0] / "manifest.json").read_text())
            self.assertEqual(manifest["outcome"], "interrupted")
            self.assertIsNotNone(manifest["child_status"]["raw_returncode"])


if __name__ == "__main__":
    unittest.main()
