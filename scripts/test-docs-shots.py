#!/usr/bin/env python3
"""Pin page declarations, crop geometry and per-shot failure isolation without starting the desktop app."""

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("docs_shots", Path(__file__).with_name("docs_shots.py"))
engine = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = engine
spec.loader.exec_module(engine)
batch = engine.load(Path(__file__).with_name("gui-screenshot-batch.py"))


class Harness:
    """A stand-in bridge and capture: persistent layers, a declared window and an optional capture refusal."""

    def __init__(self, fail=False, layers=False):
        self.fail, self.layers, self.calls, self.captures = fail, layers, [], []

    def cli(self, run, record, args, capture=False):
        """Answer the engine's reads and record inputs, so reset and capture ordering are observable."""
        self.calls.append(args)
        out = ""
        if args[0] == "snapshot":
            out = json.dumps({"entries": [{"key": {"path": "root/layer-launcher"}}] if self.layers else []})
        elif args[:2] == ["get", "box"]:
            out = json.dumps({"x": 20, "y": 30, "w": 10, "h": 10})
        elif args[0] == "windows":
            out = json.dumps([{"width": 100, "height": 100, "scale": 1}])
        return subprocess.CompletedProcess(args, 0, stdout=out, stderr="")

    def capture(self, run, record, args):
        """Refuse only the first capture when asked; a later real PNG proves the engine continued."""
        self.captures.append(args)
        if self.fail and len(self.captures) == 1:
            raise RuntimeError("stand-in size check refused")
        batch.write_rgba(Path(args[0]), 100, 100, bytes((30, 40, 50, 255)) * 10000)
        return 0


class DocsShots(unittest.TestCase):
    """A bad declaration or a failed shot must never masquerade as a checked docs picture."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def page(self, declaration):
        """Write one page using the engine's public Shot import, as a real page module does."""
        (self.root / "page.py").write_text("from docs_shots import Shot\n" + declaration)
        return engine.load_pages(self.root, "page")

    def test_missing_shots_is_refused(self):
        """A page without SHOTS would silently capture nothing; it must fail before any app starts."""
        for declaration in ("", "SHOTS = ()"):
            with self.subTest(declaration=declaration), self.assertRaisesRegex(ValueError, "SHOTS must be a list"):
                self.page(declaration)

    def test_bad_names_are_refused(self):
        """Shot names become output paths; only lowercase kebab case may reach the output tree."""
        for name in ("", "Upper", "two_words", "a/b", "-leading", "trailing-", "two--parts"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "bad shot name"):
                self.page(f"SHOTS = [Shot({name!r}, lambda b: None, ['id:one'])]")

    def test_duplicate_names_are_refused(self):
        """Two shots must not overwrite the same page output; repeated names fail declaration loading."""
        with self.assertRaisesRegex(ValueError, "duplicate shot name one"):
            self.page("SHOTS = [Shot('one', lambda b: None, ['id:one'])] * 2")

    def test_empty_regions_are_refused(self):
        """Every shot needs a crop target; an empty union must fail before driving the app."""
        with self.assertRaisesRegex(ValueError, "non-empty list"):
            self.page("SHOTS = [Shot('one', lambda b: None, [])]")

    def test_non_id_regions_are_refused(self):
        """A crop must use stable ids; text selectors and an id with no target are refused."""
        for target in ("text:One", "@e1", "id:", 3):
            with self.subTest(target=target), self.assertRaisesRegex(ValueError, "id: targets"):
                self.page(f"SHOTS = [Shot('one', lambda b: None, [{target!r}])]")

    def test_crop_unions_pads_clamps_and_scales(self):
        """The crop must include both regions and their padding, with clipping before scaling and outward rounding."""
        boxes = [{"x": 4, "y": 35.25, "w": 10, "h": 5}, {"x": 60.25, "y": 80, "w": 35, "h": 19}]
        self.assertEqual(engine.crop_rect(boxes, {"width": 100, "height": 100, "scale": 2}), (0, 38, 200, 200))
        self.assertEqual(engine.crop_rect([boxes[1]], {"width": 200, "height": 200, "scale": 2}),
                         (88, 128, 223, 230))

    def test_crop_uses_content_origin_after_linux_frame_is_removed(self):
        """A framed window's boxes still count the removed title bar and frame. The crop must not shift past the shot."""
        window = {"width": 142, "height": 176, "scale": 2}
        boxes = [{"x": 25, "y": 85, "w": 10, "h": 20}, {"x": 111, "y": 145, "w": 10, "h": 10}]
        self.assertEqual(engine.crop_rect(boxes, window, frame=21, title_bar=34), (0, 28, 200, 200))

    def release_listings(self, version, *names):
        """Write the same names the website generator reads and the publisher writes to its manifest."""
        releases, data = self.root / "release-notes", self.root / "data"
        releases.mkdir(exist_ok=True)
        data.mkdir(exist_ok=True)
        (releases / f"{version}.json").write_text(json.dumps({"screenshots": [{"name": name} for name in names]}))
        return data / "docs-shots.json"

    def test_release_notes_module_is_discovered(self):
        """The full run discovers the release page, even when there are no declarations to drive."""
        pages = dict(engine.load_pages(engine.HERE / "docs-shots", None))
        self.assertIn("release-notes", pages)
        self.assertIsInstance(pages["release-notes"], list)

    def test_only_listed_and_published_release_shots_are_frozen(self):
        """A first capture reaches the publisher's path, while a published image needs an explicit recapture request."""
        frozen, fresh = "release-notes/v1-2-3-old", "release-notes/v1-2-3-new"
        manifest = self.release_listings("v1.2.3", frozen, fresh)
        manifest.write_text(json.dumps({"shots": {frozen: {}, "page/ordinary": {}}}))
        old, new, ordinary = (engine.Shot(name, lambda b: None, ["id:one"])
                              for name in ("v1-2-3-old", "v1-2-3-new", "ordinary"))
        pages = [("release-notes", [old, new]), ("page", [ordinary])]
        selected = engine.select_shots(pages, self.root, [])
        self.assertEqual(selected, [("release-notes", [new]), ("page", [ordinary])])
        run, out = self.root / "run", self.root / "out"
        run.mkdir()
        (run / "run.json").write_text("{}")
        self.assertEqual(engine.capture_pages(Harness(), batch, run, selected, out), {})
        self.assertEqual(list((out / "release-notes").iterdir()), [out / f"{fresh}.png"])
        self.assertEqual(engine.select_shots(pages, self.root, [frozen]), pages)
        with self.assertRaisesRegex(ValueError, "not a published release shot"):
            engine.select_shots(pages, self.root, [fresh])
        with self.assertRaisesRegex(ValueError, "add its shot"):
            engine.select_shots([], self.root, [frozen])

    def test_release_declarations_must_match_the_version_listing(self):
        """A version typo must fail before capture, since the site finds a listing by its release tag."""
        name = "v1-2-3-control"
        self.release_listings("v1.2.4", f"release-notes/{name}")
        shot = engine.Shot(name, lambda b: None, ["id:one"])
        with self.assertRaisesRegex(ValueError, r"v1\.2\.3\.json"):
            engine.select_shots([("release-notes", [shot])], self.root, [])

    def test_park_is_optional_but_must_name_an_id_target(self):
        """A declared park spot bypasses the automatic search, so a malformed target must fail before app startup."""
        for target in ("", "text:Title", "@e1", "id:", 3):
            with self.subTest(target=target), self.assertRaisesRegex(ValueError, "id: targets"):
                self.page(f"SHOTS = [Shot('one', lambda b: None, ['id:one'], park={target!r})]")
        pages = self.page("SHOTS = [Shot('one', lambda b: None, ['id:one'], park='id:title')]")
        self.assertEqual(pages[0][1][0].park, "id:title")
        self.assertIsNone(engine.Shot("one", lambda b: None, ["id:one"]).park)

    def test_failed_capture_writes_nothing_and_next_shot_runs(self):
        """A size refusal leaves no output for that shot, records its message and still attempts the next shot."""
        pages = self.page("SHOTS = [Shot(n, lambda b: None, ['id:one'], park='id:title' if n == 'two' else None) "
                          "for n in ['one', 'two']]")
        run, out = self.root / "run", self.root / "out"
        run.mkdir()
        (run / "run.json").write_text("{}")
        harness = Harness(fail=True)
        failures = engine.capture_pages(harness, batch, run, pages, out)
        self.assertEqual(failures, {"page/one": "stand-in size check refused"})
        self.assertEqual(json.loads((run / "failures.json").read_text()), failures)
        self.assertEqual(list((out / "page").iterdir()), [out / "page/two.png"])
        self.assertEqual(batch.png_size(out / "page/two.png"), (42, 42))
        self.assertEqual(len(harness.captures), 2)
        self.assertEqual(harness.captures[0][-1], "--keep-focus")
        self.assertEqual(harness.captures[1][-2:], ["--park", "id:title"])

    def test_reset_gives_up_after_six_escape_presses(self):
        """A stuck layer must fail the shot after six presses, rather than hang or capture the previous dialog."""
        harness = Harness(layers=True)
        bridge = engine.Bridge(harness, self.root, {})
        with self.assertRaisesRegex(RuntimeError, "layers remain after 6 Escape presses"):
            engine.reset(bridge)
        self.assertEqual(sum(args == ["press", "escape"] for args in harness.calls), 6)


if __name__ == "__main__":
    unittest.main()
