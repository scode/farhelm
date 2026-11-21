#!/usr/bin/env python3
"""Exercise the golden runner against a stand-in renderer, without Cargo or a GPU."""

import contextlib
import importlib.util
import io
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

SPEC = importlib.util.spec_from_file_location("check_gui_goldens", Path(__file__).with_name("check-gui-goldens.py"))
assert SPEC is not None and SPEC.loader is not None
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)

PLATFORM = {"darwin": "macos", "win32": "windows"}.get(sys.platform, sys.platform)


class GoldenRunnerTests(unittest.TestCase):
    def setUp(self):
        """Keep the real checkout's reference store and capture directory outside the fixture."""
        root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        manifest = root / "goldens.txt"
        manifest.write_text("reference manifest\n", encoding="utf-8")
        store = runner.load_goldens_tool()
        self.tool = SimpleNamespace(
            WORK=root,
            CAPTURES=root / "captures",
            MANIFEST=manifest,
            REMOTE="unused",
            CACHE=root / "cache",
            Refused=store.Refused,
            record_manifest=store.record_manifest,
            capture_lock=store.capture_lock,
        )
        self.marker = self.tool.CAPTURES / f"{PLATFORM}.complete"
        self.references = root / "fetched"
        self.references.mkdir()
        self.enterContext(contextlib.redirect_stdout(io.StringIO()))
        self.enterContext(contextlib.redirect_stderr(io.StringIO()))

    def run_update(self, render):
        """Let the real runner decide whether the stand-in's output may be published."""
        with mock.patch.object(runner.subprocess, "run", side_effect=render):
            return runner.run_suite(self.tool, {}, True, [])

    def test_successful_full_update_records_count_after_rendering(self):
        """An old completion marker and old images must be gone before any new pixels are drawn."""
        self.tool.CAPTURES.mkdir()
        self.marker.write_text("99\n", encoding="utf-8")
        old = self.tool.CAPTURES / PLATFORM / "old--light.png"
        old.parent.mkdir()
        old.touch()

        def render(command, *, cwd, env, pass_fds):
            self.assertFalse(self.marker.exists())
            self.assertFalse(old.exists())
            self.assertTrue((self.tool.CAPTURES / "manifest.sha256").is_file())
            self.assertEqual(len(pass_fds), 1)
            with self.assertRaisesRegex(self.tool.Refused, "another golden update or publish"):
                with self.tool.capture_lock(self.tool.CAPTURES):
                    self.fail("a second writer acquired the capture directory")
            dest = Path(env["FARHELM_UPDATE_GOLDENS"]) / PLATFORM
            dest.mkdir()
            (dest / "first--light.png").touch()
            (dest / "first--dark.png").touch()
            return subprocess.CompletedProcess(command, 0)

        self.assertEqual(self.run_update(render), 0)
        self.assertEqual(self.marker.read_text(encoding="utf-8"), "2\n")

    def test_failed_update_keeps_partial_data_without_certifying_it(self):
        """A renderer can leave usable failure evidence even though publishing it would drop scenes."""
        def render(command, *, cwd, env, pass_fds):
            dest = Path(env["FARHELM_UPDATE_GOLDENS"]) / PLATFORM
            dest.mkdir()
            (dest / "first--light.png").touch()
            return subprocess.CompletedProcess(command, 101)

        self.assertEqual(self.run_update(render), 101)
        self.assertTrue((self.tool.CAPTURES / PLATFORM / "first--light.png").is_file())
        self.assertFalse(self.marker.exists())

    def test_zero_exit_without_any_pictures_cannot_certify_an_update(self):
        """A libtest command that selects no tests cannot publish an empty golden set."""
        self.assertEqual(self.run_update(lambda command, **_: subprocess.CompletedProcess(command, 0)), 1)
        self.assertFalse(self.marker.exists())

    def test_compare_failure_paths_are_isolated_from_other_runs(self):
        """A repeat run must start empty and keep the earlier failure picture available."""
        base = {
            "FARHELM_GOLDENS_DIR": str(self.references),
            "FARHELM_UPDATE_GOLDENS": str(self.tool.CAPTURES),
        }
        outputs = []

        def render(command, *, cwd, env, pass_fds):
            self.assertEqual(env["FARHELM_GOLDENS_DIR"], str(self.references))
            self.assertNotIn("FARHELM_UPDATE_GOLDENS", env)
            self.assertEqual(pass_fds, ())
            dest = Path(env["FARHELM_GOLDEN_CAPTURE_DIR"])
            self.assertEqual(dest.parent.parent, self.tool.WORK)
            self.assertFalse(dest.exists())
            outputs.append(dest)
            if len(outputs) == 1:
                (dest / PLATFORM).mkdir(parents=True)
                (dest / PLATFORM / "first--light.png").touch()
                return subprocess.CompletedProcess(command, 101)
            return subprocess.CompletedProcess(command, 0)

        with mock.patch.object(runner.subprocess, "run", side_effect=render):
            self.assertEqual(runner.run_suite(self.tool, base, False, []), 101)
            self.assertEqual(runner.run_suite(self.tool, base, False, []), 0)
        self.assertNotEqual(*outputs)
        self.assertTrue((outputs[0] / PLATFORM / "first--light.png").is_file())
        self.assertFalse(outputs[1].parent.exists())

    def test_automatic_references_are_private_to_the_comparison(self):
        """A second comparison must leave the matching references for an earlier failure available."""
        fetched = []

        def fetch(manifest, out, remote, cache):
            out.mkdir()
            (out / "reference.png").touch()
            fetched.append(out)
            return out

        def render(command, *, cwd, env, pass_fds):
            self.assertEqual(Path(env["FARHELM_GOLDENS_DIR"]), fetched[-1])
            self.assertEqual(Path(env["FARHELM_GOLDEN_CAPTURE_DIR"]).parent, fetched[-1].parent)
            return subprocess.CompletedProcess(command, 101 if len(fetched) == 1 else 0)

        self.tool.fetch = fetch
        with mock.patch.object(runner.subprocess, "run", side_effect=render):
            self.assertEqual(runner.run_suite(self.tool, {}, False, []), 101)
            self.assertEqual(runner.run_suite(self.tool, {}, False, []), 0)
        self.assertNotEqual(*fetched)
        self.assertTrue((fetched[0] / "reference.png").is_file())
        self.assertFalse(fetched[1].parent.exists())


if __name__ == "__main__":
    unittest.main()
