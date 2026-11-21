#!/usr/bin/env python3
"""Exercise the golden runner against a stand-in renderer, without Cargo or a GPU."""

import contextlib
import importlib.util
import io
import json
import signal
import subprocess
import sys
import tempfile
import textwrap
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

        def spawn(command, *, cwd, env, pass_fds, **_):
            result = render(command, cwd=cwd, env=env, pass_fds=pass_fds)
            child = mock.Mock()
            child.wait.return_value = result.returncode
            child.poll.return_value = result.returncode
            return child

        with mock.patch.object(runner, "build_binary", return_value=self.binary), \
                mock.patch.object(runner, "list_scenes", return_value=self.names), \
                mock.patch.object(runner.subprocess, "Popen", side_effect=spawn):
            return runner.run_suite(self.tool, base, update, list(patterns), [], [], 1)

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

    def test_scene_groups_are_stable_disjoint_and_capped(self):
        """A CPU cap changes the number of processes without dropping any catalog entries."""
        with mock.patch.object(runner.os, "sched_getaffinity", return_value={0, 1}, create=True):
            self.assertEqual(runner.scene_groups(self.names, 4), [["first", "launcher-search"], ["launcher-new", "last"]])
            self.assertEqual(runner.scene_groups(self.names, 1), [self.names])
            self.assertEqual(runner.scene_groups(["first"], 4), [["first"]])

    def test_a_killed_worker_leaves_other_outputs_unpublishable(self):
        """Actual concurrent children reach a shared readiness boundary before one is killed.

        All expected PNG names exist. Even that set cannot certify a full update if one of its renderers was killed.
        Each job's captured output is still readable and carries the killed child's signal.
        """
        binary = self.tool.WORK / "fake-renderer"
        binary.write_text(textwrap.dedent("""\
            #!/usr/bin/env python3
            import json
            import os
            import signal
            import sys
            import time
            from pathlib import Path

            captures = Path(os.environ["FARHELM_UPDATE_GOLDENS"])
            names = json.loads(os.environ["FARHELM_GOLDEN_SCENES"])
            print(f"ready {names[0]}", flush=True)
            (captures.parent / (names[0] + ".ready")).touch()
            deadline = time.monotonic() + 10
            while not all((captures.parent / (name + ".ready")).is_file()
                          for name in ("first", "launcher-new")):
                if time.monotonic() > deadline:
                    sys.exit(98)
                time.sleep(0.01)
            platform = "macos" if sys.platform == "darwin" else sys.platform
            dest = captures / platform
            dest.mkdir(exist_ok=True)
            for name in names:
                for appearance in ("light", "dark"):
                    (dest / f"{name}--{appearance}.png").touch()
            if names[0] == "first":
                os.kill(os.getpid(), signal.SIGKILL)
            """), encoding="utf-8")
        binary.chmod(0o700)
        output = io.StringIO()
        with contextlib.redirect_stdout(output), \
                mock.patch.object(runner, "build_binary", return_value=binary), \
                mock.patch.object(runner, "list_scenes", return_value=self.names), \
                mock.patch.object(runner.os, "sched_getaffinity", return_value={0, 1}, create=True):
            status = runner.run_suite(self.tool, {}, True, [], [], [], 2)
        self.assertEqual(status, 137, output.getvalue())
        self.assertIn("ready first", output.getvalue())
        self.assertIn("ready launcher-new", output.getvalue())
        self.assertIn("renderer terminated by signal 9", output.getvalue())
        self.assertIn("[goldens job 2/2]", output.getvalue())
        self.assertEqual(
            {file.name for file in (self.tool.CAPTURES / PLATFORM).iterdir()},
            {f"{name}--{appearance}.png" for name in self.names for appearance in ("light", "dark")},
        )
        self.assertFalse(self.marker.exists())

    def test_term_during_spawn_keeps_the_returned_worker_owned(self):
        """TERM during spawn must leave the returned child tracked and reaped, with no later spawns."""
        child = mock.Mock()
        child.poll.return_value = None

        def spawn(*args, **kwargs):
            signal.raise_signal(signal.SIGTERM)
            return child

        with mock.patch.object(runner.subprocess, "Popen", side_effect=spawn) as popen:
            status = runner.render_scenes(["stand-in"], [["first"], ["second"]], {}, None)
        self.assertEqual(status, 128 + signal.SIGTERM)
        popen.assert_called_once()
        child.kill.assert_called_once_with()
        child.wait.assert_called_once_with()

    def test_term_during_cleanup_overrides_an_already_successful_worker(self):
        """The runner can only certify a completed update after its last cancellation boundary."""
        child = mock.Mock()
        child.poll.return_value = 0

        def wait(*, timeout=None):
            if timeout is None:
                signal.raise_signal(signal.SIGTERM)
            return 0

        child.wait.side_effect = wait
        with mock.patch.object(runner.subprocess, "Popen", return_value=child):
            status = runner.render_scenes(["stand-in"], [["first"]], {}, None)
        self.assertEqual(status, 128 + signal.SIGTERM)


if __name__ == "__main__":
    unittest.main()
