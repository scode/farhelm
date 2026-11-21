#!/usr/bin/env python3
"""Exercise the golden runner against a stand-in renderer, without Cargo or a GPU."""

import contextlib
import importlib.util
import io
import json
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
        self.binary = root / "custom-target" / "farhelm_gui"
        self.names = ["first", "launcher-new", "launcher-search", "last"]
        self.enterContext(contextlib.redirect_stdout(io.StringIO()))
        self.enterContext(contextlib.redirect_stderr(io.StringIO()))

    def run_suite(self, render, *, update=True, patterns=(), base=None):
        """Let the real selection and publication logic drive a stand-in catalog and renderer."""
        if base is None:
            base = {} if update else {
                "FARHELM_GOLDENS_DIR": str(self.references),
                "FARHELM_UPDATE_GOLDENS": str(self.tool.CAPTURES),
            }
        with mock.patch.object(runner, "build_binary", return_value=self.binary), \
                mock.patch.object(runner, "list_scenes", return_value=self.names), \
                mock.patch.object(runner.subprocess, "run", side_effect=render):
            return runner.run_suite(self.tool, base, update, list(patterns), [], [])

    @staticmethod
    def write_selected(env):
        """Write both names for each requested scene, as the Rust renderer does in update mode."""
        dest = Path(env["FARHELM_UPDATE_GOLDENS"]) / PLATFORM
        dest.mkdir(exist_ok=True)
        for name in json.loads(env["FARHELM_GOLDEN_SCENES"]):
            (dest / f"{name}--light.png").touch()
            (dest / f"{name}--dark.png").touch()

    def test_cargo_artifact_and_scene_list_come_from_the_same_binary(self):
        """A non-default build directory should work without polling target/debug/deps for stale binaries."""
        def render(command, *, cwd, env, **kwargs):
            if command[0] == "cargo":
                self.assertIn("--target-dir", command)
                artifact = {
                    "reason": "compiler-artifact",
                    "target": {"name": "farhelm_gui", "kind": ["lib"]},
                    "profile": {"test": True},
                    "executable": str(self.binary),
                }
                return subprocess.CompletedProcess(command, 0, json.dumps(artifact))
            self.assertEqual(command[0], str(self.binary))
            self.assertIn("goldens::list_scenes", command)
            listing = f"running 1 test\n{runner.SCENE_INVENTORY}{json.dumps(self.names)}\n"
            return subprocess.CompletedProcess(command, 0, listing)

        with mock.patch.object(runner.subprocess, "run", side_effect=render):
            binary = runner.build_binary({}, ["--target-dir", str(self.binary.parent)])
            self.assertEqual(runner.list_scenes(binary, {}), self.names)

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
            self.write_selected(env)
            return subprocess.CompletedProcess(command, 0)

        self.assertEqual(self.run_suite(render), 0)
        self.assertEqual(self.marker.read_text(encoding="utf-8"), "8\n")

    def test_failed_update_keeps_partial_data_without_certifying_it(self):
        """A renderer can leave usable failure evidence even though publishing it would drop scenes."""
        def render(command, *, cwd, env, pass_fds):
            dest = Path(env["FARHELM_UPDATE_GOLDENS"]) / PLATFORM
            dest.mkdir()
            (dest / "first--light.png").touch()
            return subprocess.CompletedProcess(command, 101)

        self.assertEqual(self.run_suite(render), 101)
        self.assertTrue((self.tool.CAPTURES / PLATFORM / "first--light.png").is_file())
        self.assertFalse(self.marker.exists())

    def test_zero_exit_without_any_pictures_cannot_certify_an_update(self):
        """A libtest command that selects no tests cannot publish an empty golden set."""
        self.assertEqual(self.run_suite(lambda command, **_: subprocess.CompletedProcess(command, 0)), 1)
        self.assertFalse(self.marker.exists())

    def test_matching_the_count_with_wrong_names_cannot_certify_an_update(self):
        """A full update needs each expected appearance. Its count alone can conceal a missing picture."""
        def render(command, *, cwd, env, pass_fds):
            self.write_selected(env)
            dest = Path(env["FARHELM_UPDATE_GOLDENS"]) / PLATFORM
            (dest / "first--light.png").rename(dest / "wrong--light.png")
            return subprocess.CompletedProcess(command, 0)

        self.assertEqual(self.run_suite(render), 1)
        self.assertFalse(self.marker.exists())

    def test_compare_failure_paths_are_isolated_from_other_runs(self):
        """A repeat run must start empty and keep the earlier failure picture available."""
        outputs = []

        def render(command, *, cwd, env, pass_fds):
            self.assertEqual(env["FARHELM_GOLDENS_DIR"], str(self.references))
            self.assertEqual(json.loads(env["FARHELM_GOLDEN_SCENES"]), ["first"])
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

        self.assertEqual(self.run_suite(render, update=False, patterns=["first"]), 101)
        self.assertEqual(self.run_suite(render, update=False, patterns=["first"]), 0)
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
        self.assertEqual(self.run_suite(render, update=False, base={}), 101)
        self.assertEqual(self.run_suite(render, update=False, base={}), 0)
        self.assertNotEqual(*fetched)
        self.assertTrue((fetched[0] / "reference.png").is_file())
        self.assertFalse(fetched[1].parent.exists())

    def test_selected_update_keeps_catalog_order_and_cannot_publish(self):
        """Overlapping patterns pick each scene once, leaving the resulting files available for inspection."""
        def render(command, *, cwd, env, pass_fds):
            self.assertEqual(json.loads(env["FARHELM_GOLDEN_SCENES"]), ["first", "launcher-new", "launcher-search"])
            self.assertIn("goldens::component_goldens", command)
            self.write_selected(env)
            return subprocess.CompletedProcess(command, 0)

        self.assertEqual(self.run_suite(render, patterns=["launcher-*", "first", "*-new"]), 0)
        self.assertTrue((self.tool.CAPTURES / PLATFORM / "launcher-new--light.png").is_file())
        self.assertFalse((self.tool.CAPTURES / PLATFORM / "last--dark.png").exists())
        self.assertFalse(self.marker.exists())

    def test_any_unknown_pattern_prevents_render_even_when_another_matches(self):
        """Treat a typo as a failed selection, including when it accompanies a broad valid match."""
        render = mock.Mock()
        with self.assertRaisesRegex(ValueError, "missing-page"):
            self.run_suite(render, patterns=["*", "missing-page"])
        render.assert_not_called()
        self.assertFalse(self.marker.exists())


if __name__ == "__main__":
    unittest.main()
