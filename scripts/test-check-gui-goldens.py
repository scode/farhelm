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

PNG_SPEC = importlib.util.spec_from_file_location("gui_screenshot_batch",
                                                Path(__file__).with_name("gui-screenshot-batch.py"))
assert PNG_SPEC is not None and PNG_SPEC.loader is not None
png = importlib.util.module_from_spec(PNG_SPEC)
PNG_SPEC.loader.exec_module(png)

PLATFORM = {"darwin": "macos", "win32": "windows"}.get(sys.platform, sys.platform)


class GoldenRunnerTests(unittest.TestCase):
    def setUp(self):
        """Keep the real checkout's reference store and capture directory outside the fixture."""
        root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        manifest = root / "goldens.txt"
        manifest.write_text("# reference manifest\n", encoding="utf-8")
        store = runner.load_goldens_tool()
        self.tool = SimpleNamespace(
            WORK=root,
            CAPTURES=root / "captures",
            MANIFEST=manifest,
            REMOTE="unused",
            CACHE=root / "cache",
            Refused=store.Refused,
            record_manifest=store.record_manifest,
            manifest_digest=store.manifest_digest,
            capture_lock=store.capture_lock,
            read_manifest=store.read_manifest,
            write_manifest=store.write_manifest,
            blob_id=store.blob_id,
            fetch=mock.Mock(side_effect=store.Refused("storage is unreachable")),
        )
        self.marker = self.tool.CAPTURES / f"{PLATFORM}.complete"
        self.focused = self.tool.WORK / "selected-captures"
        self.page = root / "changes" / "index.html"
        self.references = root / "fetched"
        self.references.mkdir()
        self.binary = root / "custom-target" / "farhelm_gui"
        self.names = ["first", "launcher-new", "launcher-search", "last"]
        self.enterContext(contextlib.redirect_stdout(io.StringIO()))
        self.enterContext(contextlib.redirect_stderr(io.StringIO()))

    def run_suite(self, render, *, update=True, patterns=(), base=None, jobs=1):
        """Let the real selection and publication logic drive a stand-in catalog and test binary."""
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
            return runner.run_suite(self.tool, base, update, list(patterns), [], [], jobs)

    @staticmethod
    def write_selected(env):
        """Write both names for each requested scene, as the Rust renderer does in update mode."""
        dest = Path(env["FARHELM_UPDATE_GOLDENS"]) / PLATFORM
        dest.mkdir(exist_ok=True)
        for name in json.loads(env["FARHELM_GOLDEN_SCENES"]):
            (dest / f"{name}--light.png").touch()
            (dest / f"{name}--dark.png").touch()

    @staticmethod
    def picture(root, name, pixels, width=2):
        """Use real PNGs so the page's copies and highlighted pixels can be read back without the app."""
        path = root / PLATFORM / name
        path.parent.mkdir(parents=True, exist_ok=True)
        png.write_rgba(path, width, 1, pixels)
        return path

    @staticmethod
    def review(directory, stem, reference_size, capture_size, pixels):
        """Stand in for metadata from the focused Rust test, whose own tests check the pixel calculation."""
        directory.mkdir(parents=True, exist_ok=True)
        (directory / f"{stem}.json").write_text(json.dumps({
            "reference_size": reference_size, "capture_size": capture_size, "pixels": pixels,
        }), encoding="utf-8")
        if pixels is not None:
            unchanged = bytes((30, 40, 50, 255))
            changed = bytes((255, 104, 42, 255))
            png.write_rgba(directory / f"{stem}.png", *capture_size, unchanged * (2 - pixels) + changed * pixels)

    def manifest_for(self, *references):
        """Record the same byte identities the real store uses, without contacting it."""
        self.tool.write_manifest(self.tool.MANIFEST, {
            f"{PLATFORM}/{ref.name}": self.tool.blob_id(ref.read_bytes()) for ref in references
        })

    def test_comparison_packages_the_changed_pictures_and_removes_a_stale_page(self):
        """A failing comparison packages only retained pictures and uses the renderer's precomputed highlights."""
        before = bytes((90, 120, 150, 255, 15, 30, 45, 200))
        after = bytes((90, 120, 150, 255, 15, 30, 45, 100))
        original = self.picture(self.references, "first--light.png", before)
        second = self.picture(self.references, "launcher-search--dark.png", before)
        unchanged = self.picture(self.references, "last--dark.png", before)
        self.manifest_for(original, second, unchanged)

        def render(command, *, env, **_):
            self.assertIn("goldens::component_goldens", command)
            dest = Path(env["FARHELM_GOLDEN_CAPTURE_DIR"])
            self.picture(dest, original.name, after)
            self.picture(dest, second.name, after)
            self.picture(dest, "launcher-new--dark.png", after)
            for reference in (original, second):
                self.review(dest / PLATFORM / "review", reference.stem, (2, 1), (2, 1), 1)
            return subprocess.CompletedProcess(command, 101)

        self.assertEqual(self.run_suite(render, update=False), 101)
        self.assertEqual(sys.stdout.getvalue().splitlines()[-1], f"golden changes page: {self.page}")
        page = self.page.read_text(encoding="utf-8")
        self.assertIn("<code>first</code> <span>· light</span>", page)
        self.assertIn("<code>launcher-new</code> <span>· dark</span>", page)
        self.assertIn("New scene", page)
        self.assertIn("1 differing pixel<", page)
        self.assertNotIn("<code>last</code>", page)
        self.assertEqual({p.name for p in self.page.parent.iterdir()}, {
            "index.html", "first--light-capture.png", "first--light-reference.png", "first--light-diff.png",
            "launcher-new--dark-capture.png", "launcher-search--dark-capture.png", "launcher-search--dark-reference.png",
            "launcher-search--dark-diff.png",
        })
        self.assertEqual((self.page.parent / "first--light-reference.png").read_bytes(), original.read_bytes())
        read_rgba = png.read_rgba
        self.assertEqual(read_rgba(self.page.parent / "first--light-capture.png"), (2, 1, after))
        self.assertEqual(read_rgba(self.page.parent / "first--light-diff.png"),
                         (2, 1, bytes((30, 40, 50, 255, 255, 104, 42, 255))))
        self.assertEqual(read_rgba(self.page.parent / "launcher-search--dark-diff.png"),
                         (2, 1, bytes((30, 40, 50, 255, 255, 104, 42, 255))))
        self.assertEqual(self.run_suite(lambda command, **_: subprocess.CompletedProcess(command, 0), update=False), 0)
        self.assertFalse(self.page.parent.exists())

    def test_update_filters_identical_pictures_and_keeps_working_without_the_store(self):
        """An offline update can distinguish changed references from new scenes and still certify a full capture."""
        original = bytes((15, 30, 45, 255) * 2)
        changed = bytes((45, 30, 15, 255) * 2)
        references = [self.picture(self.references, f"{name}--{appearance}.png", original)
                      for name in self.names for appearance in ("light", "dark")
                      if name != "launcher-new"]
        self.manifest_for(*references)

        def render(command, *, env, **_):
            dest = Path(env["FARHELM_UPDATE_GOLDENS"])
            for name in json.loads(env["FARHELM_GOLDEN_SCENES"]):
                self.picture(dest, f"{name}--light.png", changed if name == "first" else original)
                self.picture(dest, f"{name}--dark.png", original)
            return subprocess.CompletedProcess(command, 0)

        self.assertEqual(self.run_suite(render, patterns=["last"]), 0)
        self.tool.fetch.assert_not_called()
        self.assertFalse(self.page.parent.exists())

        def offline(manifest, out, remote, cache):
            self.assertEqual(self.tool.read_manifest(manifest), {
                f"{PLATFORM}/first--light.png": self.tool.blob_id(references[0].read_bytes()),
            })
            raise self.tool.Refused("storage is unreachable")

        self.tool.fetch.side_effect = offline
        self.assertEqual(self.run_suite(render), 0)
        self.tool.fetch.assert_called_once()
        self.assertTrue(self.marker.is_file())
        page = self.page.read_text(encoding="utf-8")
        self.assertIn("Some reference pictures are unavailable.", page)
        self.assertIn("Reference unavailable", page)
        self.assertEqual(page.count("New scene"), 2)
        self.assertNotIn("<code>last</code>", page)
        self.assertEqual({p.name for p in self.page.parent.iterdir()}, {
            "index.html", "first--light-capture.png", "launcher-new--dark-capture.png",
            "launcher-new--light-capture.png",
        })
        self.assertEqual(sys.stdout.getvalue().splitlines()[-1], f"golden changes page: {self.page}")

    def test_update_with_local_reference_reports_size_change_without_fetching(self):
        """Send only the staged changed pairs to the test binary and keep native review files out of publishing."""
        light = self.picture(self.references, "first--light.png", bytes((1, 2, 3, 255)), width=1)
        dark = self.picture(self.references, "first--dark.png", bytes((4, 5, 6, 255) * 2))
        self.manifest_for(light, dark)
        jobs_seen = set()

        def render(command, *, env, pass_fds, **_):
            self.assertEqual(command[0], str(self.binary))
            self.assertEqual(len(pass_fds), 1)
            if "goldens::component_goldens" in command:
                for before in (light, dark):
                    self.picture(Path(env["FARHELM_UPDATE_GOLDENS"]), before.name, bytes((1, 2, 3, 255) * 2))
                return subprocess.CompletedProcess(command, 0)
            self.assertIn("goldens::changes::review_pairs", command)
            request = Path(env["FARHELM_GOLDEN_DIFF_BATCH"])
            task = json.loads(request.read_text(encoding="utf-8"))
            self.assertEqual(len(task["pairs"]), 1)
            reference, capture = map(Path, task["pairs"][0])
            self.assertEqual(capture.parent, self.focused / PLATFORM)
            before = self.references / PLATFORM / capture.name
            self.assertEqual(reference.read_bytes(), before.read_bytes())
            jobs_seen.add(capture.name)
            size = (1, 1) if capture.name == light.name else (2, 1)
            self.review(Path(task["directory"]), capture.stem, size, (2, 1), None if size == (1, 1) else 2)
            return subprocess.CompletedProcess(command, 0)

        with mock.patch.object(runner, "available_cpus", return_value=2):
            self.assertEqual(self.run_suite(render, patterns=["first"], jobs=2,
                                           base={"FARHELM_GOLDENS_DIR": str(self.references)}), 0)
        self.tool.fetch.assert_not_called()
        self.assertEqual(jobs_seen, {light.name, dark.name})
        self.assertEqual({p.name for p in (self.focused / PLATFORM).iterdir()}, jobs_seen)
        page = self.page.read_text(encoding="utf-8")
        self.assertIn("Reference (1 × 1)", page)
        self.assertIn("New capture (2 × 1)", page)
        self.assertIn("Image size changed.", page)
        self.assertNotIn("first--light-diff.png", page)
        self.assertIn("2 differing pixels", page)
        self.assertTrue((self.page.parent / "first--dark-diff.png").is_file())

    def test_update_disregards_stale_local_references(self):
        """A local copy from a different revision cannot supply the old picture or make a new scene look changed."""
        before = self.picture(self.references, "first--light.png", bytes((1, 2, 3, 255) * 2))
        self.manifest_for(before)
        self.picture(self.references, before.name, bytes((3, 2, 1, 255) * 2))
        self.picture(self.references, "first--dark.png", bytes((1, 2, 3, 255) * 2))

        def render(command, *, env, **_):
            dest = Path(env["FARHELM_UPDATE_GOLDENS"])
            for appearance in ("light", "dark"):
                self.picture(dest, f"first--{appearance}.png", bytes((4, 5, 6, 255) * 2))
            return subprocess.CompletedProcess(command, 0)

        self.assertEqual(self.run_suite(render, patterns=["first"], base={"FARHELM_GOLDENS_DIR": str(self.references)}),
                         0)
        self.tool.fetch.assert_not_called()
        self.assertEqual({p.name for p in self.page.parent.iterdir()}, {
            "index.html", "first--dark-capture.png", "first--light-capture.png",
        })
        page = self.page.read_text(encoding="utf-8")
        self.assertIn("<span>· dark</span></h2><div class=\"pictures\"><p class=\"missing\">New scene", page)
        self.assertIn("<span>· light</span></h2><div class=\"pictures\"><p class=\"missing\">Reference unavailable",
                      page)

    def test_update_keeps_the_manifest_baseline_when_the_live_file_moves(self):
        """A capture matching a newer checkout manifest was still a change against the run's baseline."""
        original = self.picture(self.references, "first--light.png", bytes((1, 2, 3, 255) * 2))
        self.manifest_for(original)
        digest = self.tool.manifest_digest(self.tool.MANIFEST)

        def render(command, *, env, **_):
            current = self.picture(Path(env["FARHELM_UPDATE_GOLDENS"]), original.name, bytes((4, 5, 6, 255) * 2))
            self.manifest_for(current)
            return subprocess.CompletedProcess(command, 0)

        self.assertEqual(self.run_suite(render, patterns=["first"]), 0)
        self.assertNotEqual(self.tool.manifest_digest(self.tool.MANIFEST), digest)
        self.assertEqual((self.focused / "manifest.sha256").read_text().strip(), digest)
        self.tool.fetch.assert_called_once()
        self.assertIn("Reference unavailable", self.page.read_text(encoding="utf-8"))
        self.assertTrue((self.page.parent / "first--light-capture.png").is_file())

    def test_cancelled_render_does_not_start_the_page(self):
        """An operator stopping the render must not start a separate review of the partial captures."""
        def render(command, *, env, **_):
            self.write_selected(env)
            return subprocess.CompletedProcess(command, 128 + signal.SIGTERM)

        with mock.patch.object(runner.golden_changes_page, "report") as report:
            self.assertEqual(self.run_suite(render), 128 + signal.SIGTERM)
        report.assert_not_called()
        self.assertFalse(self.marker.exists())

    def test_cancelling_the_review_keeps_a_full_update_unpublishable(self):
        """An operator stopping the review gets a cancellation even if the renderer wrote all expected files."""
        original = self.picture(self.references, "first--light.png", bytes((1, 2, 3, 255) * 2))
        self.manifest_for(original)

        def render(command, *, env, **_):
            self.write_selected(env)
            return subprocess.CompletedProcess(command, 0)

        supervisor = runner.run_workers

        def stopped_review(command, requests, env, lock, label):
            if label == "golden review":
                self.assertEqual(len(requests), 1)
                return 128 + signal.SIGTERM
            return supervisor(command, requests, env, lock, label)

        with mock.patch.object(runner, "run_workers", side_effect=stopped_review) as workers:
            status = self.run_suite(render, base={"FARHELM_GOLDENS_DIR": str(self.references)})
        self.assertEqual(workers.call_count, 2)
        self.assertEqual(status, 128 + signal.SIGTERM)
        self.assertEqual(len(list((self.tool.CAPTURES / PLATFORM).glob("*.png"))), 8)
        self.assertFalse(self.marker.exists())
        self.assertFalse(self.page.parent.exists())

    def test_comparison_keeps_a_capture_when_the_reference_cannot_be_decoded(self):
        """A reference that can be read but cannot be decoded belongs on the page only as a missing image."""
        ref = self.picture(self.references, "first--light.png", bytes((1, 2, 3, 255) * 2))
        self.manifest_for(ref)
        ref.write_bytes(b"truncated PNG")

        def render(command, *, env, **_):
            captures = Path(env["FARHELM_GOLDEN_CAPTURE_DIR"])
            self.picture(captures, ref.name, bytes((4, 5, 6, 255) * 2))
            review = captures / PLATFORM / "review"
            review.mkdir()
            (review / f"{ref.stem}.json").write_text("null", encoding="utf-8")
            return subprocess.CompletedProcess(command, 101)

        self.assertEqual(self.run_suite(render, update=False), 101)
        page = self.page.read_text(encoding="utf-8")
        self.assertIn("Reference unavailable", page)
        self.assertIn("Some reference pictures are unavailable.", page)
        self.assertTrue((self.page.parent / "first--light-capture.png").is_file())
        self.assertFalse((self.page.parent / "first--light-reference.png").exists())

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
        """A focused update puts its pictures aside while a previous full update stays ready to publish."""
        earlier = self.tool.CAPTURES / PLATFORM / "launcher-new--light.png"
        earlier.parent.mkdir(parents=True)
        earlier.write_bytes(b"earlier full update")
        self.marker.write_text("8\n", encoding="utf-8")

        def render(command, *, cwd, env, pass_fds):
            self.assertEqual(json.loads(env["FARHELM_GOLDEN_SCENES"]), ["first", "launcher-new", "launcher-search"])
            self.assertIn("goldens::component_goldens", command)
            self.write_selected(env)
            return subprocess.CompletedProcess(command, 0)

        self.assertEqual(self.run_suite(render, patterns=["launcher-*", "first", "*-new"]), 0)
        self.assertTrue((self.focused / PLATFORM / "launcher-new--light.png").is_file())
        self.assertFalse((self.focused / PLATFORM / "last--dark.png").exists())
        self.assertFalse((self.focused / self.marker.name).exists())
        self.assertEqual(earlier.read_bytes(), b"earlier full update")
        self.assertEqual(self.marker.read_text(encoding="utf-8"), "8\n")

    def test_any_unknown_pattern_prevents_render_even_when_another_matches(self):
        """Treat a typo as a failed selection, including when it accompanies a broad valid match. The refusal comes
        before the update clears anything, so an earlier full update's captures and marker, not yet published,
        survive it."""
        earlier = self.tool.CAPTURES / PLATFORM / "first--light.png"
        earlier.parent.mkdir(parents=True)
        earlier.write_bytes(b"earlier capture")
        self.marker.write_text("8\n", encoding="utf-8")
        render = mock.Mock()
        with self.assertRaisesRegex(ValueError, "missing-page"):
            self.run_suite(render, patterns=["*", "missing-page"])
        render.assert_not_called()
        self.assertEqual(earlier.read_bytes(), b"earlier capture")
        self.assertEqual(self.marker.read_text(encoding="utf-8"), "8\n")

    def test_scene_groups_are_stable_disjoint_and_capped(self):
        """A CPU cap changes the number of processes without dropping any catalog entries."""
        with mock.patch.object(runner.os, "sched_getaffinity", return_value={0, 1}, create=True):
            self.assertEqual(runner.worker_groups(self.names, 4), [["first", "launcher-search"], ["launcher-new", "last"]])
            self.assertEqual(runner.worker_groups(self.names, 1), [self.names])
            self.assertEqual(runner.worker_groups(["first"], 4), [["first"]])

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
