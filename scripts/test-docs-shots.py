#!/usr/bin/env python3
"""Pin page declarations, crop geometry and per-shot failure isolation without starting the desktop app."""

import importlib.util
import json
from pathlib import Path
import shutil
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

    def __init__(self, fail=False, layers=False, size=100, nodes=()):
        self.fail, self.layers, self.size, self.calls, self.captures = fail, layers, size, [], []
        self.nodes = list(nodes)

    def cli(self, run, record, args, capture=False):
        """Answer the engine's reads and record inputs, so reset and capture ordering are observable."""
        self.calls.append(args)
        out = ""
        if args[0] == "snapshot":
            out = json.dumps({"entries": [{"key": {"path": "root/layer-launcher"}}] if self.layers else []})
        elif args[:2] == ["get", "box"]:
            out = json.dumps({"x": 20, "y": 30, "w": 10, "h": 10})
        elif args[0] == "windows":
            out = json.dumps([{"width": self.size, "height": self.size, "scale": 1}])
        elif args[0] == "tree":
            out = json.dumps({"nodes": self.nodes})
        return subprocess.CompletedProcess(args, 0, stdout=out, stderr="")

    def capture(self, run, record, args):
        """Refuse only the first capture when asked; a later real PNG proves the engine continued."""
        self.captures.append(args)
        if self.fail and len(self.captures) == 1:
            raise RuntimeError("stand-in size check refused")
        batch.write_rgba(Path(args[0]), self.size, self.size, bytes((30, 40, 50, 255)) * self.size * self.size)
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

    def test_annotations_must_be_well_formed(self):
        """A callout with a bad side, empty text or a non-id target would fail mid-run or draw nothing useful, so the page
        is refused before the app starts; well-formed callouts and rings load."""
        bad = {
            "Callout('text:Title', 'x')": "id: targets",
            "Callout('id:a', '  ')": "text must be non-empty",
            "Callout('id:a', 'x', side='under')": "side must be one of",
            "'id:a'": "Callout or Highlight",
        }
        for declaration, message in bad.items():
            with self.subTest(declaration=declaration), self.assertRaisesRegex(ValueError, message):
                self.page("from docs_shots import Callout, Highlight\n"
                          f"SHOTS = [Shot('one', lambda b: None, ['id:one'], annotations=[{declaration}])]")
        with self.assertRaisesRegex(ValueError, "max_width"):
            self.page("SHOTS = [Shot('one', lambda b: None, ['id:one'], max_width=0)]")
        pages = self.page("from docs_shots import Callout, Highlight\n"
                          "SHOTS = [Shot('one', lambda b: None, ['id:one'], max_width=930, "
                          "annotations=[Callout('id:a', 'Hi', side='right', dy=20), Highlight('id:b')])]")
        self.assertEqual(len(pages[0][1][0].annotations), 2)

    def test_callout_text_wraps_like_the_old_boxes(self):
        """The old CSS boxes held 26 characters of 15px JetBrains Mono a line; ported wording must wrap the same way, or
        a callout placed by its old dx/dy would change size and land somewhere else."""
        width, height, lines = engine.callouts.box_size(
            "The list's order. Click it for Recently active, Newest created or Title A–Z, or for Compact: one line "
            "per session.")
        self.assertEqual(lines[0], "The list's order. Click it")
        self.assertTrue(all(len(line) <= 26 for line in lines))
        self.assertEqual(width, engine.callouts.BOX_MAX)
        self.assertAlmostEqual(height, len(lines) * 21 + 2 * (10.5 + 2))
        short_width, _, short = engine.callouts.box_size("Hi there")
        self.assertEqual(short, ["Hi there"])
        self.assertAlmostEqual(short_width, 8 * 9 + 2 * (13.5 + 2))
        self.assertEqual(engine.callouts.wrap("a" * 30, 26), ["a" * 26, "a" * 4])

    def test_callout_placement_matches_the_old_overlay(self):
        """`place` is a port of the old overlay's: the box sits the gap away on the asked side, shifted by dx/dy, kept
        inside the view, and the arrow runs from the box's facing edge to just outside the target."""
        rect = engine.callouts.Rect
        x, y, start, end = engine.callouts.place(rect(100, 100, 50, 20), 200, 60, "right", 0, 0, 1000, 800)
        self.assertEqual((x, y), (190, 80))
        self.assertEqual(start, (190, 110))
        self.assertEqual(end, (156, 110))
        x, y, _, _ = engine.callouts.place(rect(100, 100, 50, 20), 200, 60, "right", 900, -500, 1000, 800)
        self.assertEqual((x, y), (784, 16))
        x, y, start, end = engine.callouts.place(rect(400, 700, 40, 20), 100, 50, "auto", 0, 0, 1000, 800)
        self.assertEqual((x, y), (370, 610))
        self.assertEqual(end, (420, 694))

    def test_annotations_are_drawn_and_kept_inside_the_crop(self):
        """A callout far from the shot's region must still be whole in the picture, and the pixels it does not cover
        must be the captured ones: the drawing is an overlay, never a resample of the capture."""
        if shutil.which("rsvg-convert") is None:
            self.skipTest("SKIPPED: rsvg-convert (librsvg) is not installed; annotation rendering is not covered")
        pages = self.page("from docs_shots import Callout\n"
                          "SHOTS = [Shot('one', lambda b: None, ['id:one'], "
                          "annotations=[Callout('id:one', 'Here', side='bottom', dy=10)])]")
        run, out = self.root / "run", self.root / "out"
        run.mkdir()
        (run / "run.json").write_text("{}")
        harness = Harness(size=200)
        self.assertEqual(engine.capture_pages(harness, batch, run, pages, out), {})
        width, height = batch.png_size(out / "page/one.png")
        # Region 20,30 10x10 alone crops to 42x42; the callout below it (40 below, 46 high) and its arrow grow the crop.
        self.assertGreater(height, 120)
        _, _, pixels = batch.read_rgba(out / "page/one.png")
        self.assertEqual(pixels[:4], bytes((30, 40, 50, 255)))
        self.assertIn(bytes((239, 68, 68, 255)), {bytes(pixels[i:i + 4]) for i in range(0, len(pixels), 4)})

    def test_placeholders_name_this_runs_sessions_and_hosts(self):
        """Row and rail ids carry keys a fresh run mints, so pages name them by scenario title and host key; resolving
        must use this run's record and the helm's listing, and a name the run lacks must fail the shot, not guess."""

        class Gui:
            def helm_request(self, run, method, path):
                return {"hosts": [{"id": 7, "kind": "local", "state": {"phase": "connected"}}]}

            def scenario_host_ids(self, plan, rows):
                return {"this-machine": rows[0]["id"]}, []

        record = {"scenario": {"sessions": {"fix flaky attach test": "s3"}}}
        bridge = engine.Bridge(Gui(), self.root, record, plan={"hosts": []})
        self.assertEqual(bridge.resolve("id:session-row-{session:fix flaky attach test}"), "id:session-row-s3")
        self.assertEqual(bridge.resolve("id:host-rail-host-{host:this-machine}"), "id:host-rail-host-7")
        self.assertEqual(bridge.resolve("id:plain"), "id:plain")
        with self.assertRaisesRegex(RuntimeError, "no staged session"):
            bridge.resolve("id:session-row-{session:nope}")
        with self.assertRaisesRegex(RuntimeError, "no connected scenario host"):
            bridge.resolve("id:host-rail-host-{host:gpu-1}")

    def test_a_callout_never_covers_an_earlier_one(self):
        """Offsets ported from the browser UI assume its row order and sizes; when two callouts would land on each
        other in the desktop app, the later one must move clear rather than hide the first one's text."""
        rect, callout = engine.callouts.Rect, engine.callouts.Callout
        targets = {"id:a": rect(100, 100, 50, 20), "id:b": rect(100, 130, 50, 20)}
        drawn = engine.callouts.layout([callout("id:a", "First"), callout("id:b", "Second")], targets, 1000, 800)
        first, second = (item.bounds for item in drawn)
        self.assertFalse(engine.callouts.overlaps(first, second))
        self.assertGreaterEqual(second.y, first.bottom + engine.callouts.SPACING)
        self.assertEqual(drawn[1].start, (second.x, second.y + second.h / 2))

    def test_max_height_keeps_the_top(self):
        """A region running to the window's foot, such as the terminal, is capped from below and keeps its top."""
        boxes = [{"x": 10, "y": 10, "w": 100, "h": 900}]
        window = {"width": 200, "height": 1000, "scale": 2}
        self.assertEqual(engine.crop_rect(boxes, window, max_height=300), (0, 0, 252, 600))

    def test_a_shot_that_changes_sessions_fails(self):
        """Steps photograph the launcher and questions but must stop short of acting; a shot whose steps started a
        session must fail instead of shipping a picture of a fleet that no longer matches the scenario."""

        class Changing(Harness):
            def __init__(self):
                super().__init__()
                self.listings = 0

            def helm_request(self, run, method, path):
                self.listings += 1
                return {"sessions": [{"id": "a"}] + ([{"id": "b"}] if self.listings > 1 else [])}

        pages = self.page("SHOTS = [Shot('one', lambda b: None, ['id:one'])]")
        run, out = self.root / "run", self.root / "out"
        run.mkdir()
        (run / "run.json").write_text("{}")
        failures = engine.capture_pages(Changing(), batch, run, pages, out)
        self.assertEqual(failures, {"page/one": "the shot's steps started, replaced or deleted a session"})
        self.assertFalse((out / "page/one.png").exists())

    def test_reset_hides_the_hosts_panel(self):
        """The Hosts panel is not a layer, so Escape leaves it; a shot that showed it must not leave it up for the next."""

        class Panel(Harness):
            def cli(self, run, record, args, capture=False):
                if args[0] == "snapshot":
                    self.calls.append(args)
                    out = json.dumps({"entries": [{"key": {"path": "root/hosts-panel"}}]})
                    return subprocess.CompletedProcess(args, 0, stdout=out, stderr="")
                return super().cli(run, record, args, capture)

        harness = Panel()
        engine.reset(engine.Bridge(harness, self.root, {}))
        self.assertIn(["click", "id:hosts-panel-toggle"], harness.calls)

    def test_a_container_id_takes_its_whole_box_from_the_tree(self):
        """A launcher form row has no snapshot entry, so `get box` would crop to its first label and cut the field off;
        an id one element holds in the raw tree gives that element's whole box, and anything else falls back."""
        row = {"id": "launcher-folder-row", "visible": {"x": 5, "y": 6, "w": 910, "h": 32}}
        twins = [{"id": "twin", "visible": {"x": 1, "y": 1, "w": 1, "h": 1}}] * 2
        harness = Harness(nodes=[row, *twins])
        bridge = engine.Bridge(harness, self.root, {})
        self.assertEqual(bridge.box("id:launcher-folder-row"), {"x": 5, "y": 6, "w": 910, "h": 32})
        self.assertEqual(bridge.box("id:twin"), {"x": 20, "y": 30, "w": 10, "h": 10})
        self.assertEqual(bridge.box("id:absent"), {"x": 20, "y": 30, "w": 10, "h": 10})
        self.assertEqual(sum(args == ["tree"] for args in harness.calls), 1)

    def test_a_cap_never_cuts_an_annotation(self):
        """A cap is for a region that runs to the window's foot; a callout placed below the capped part must still be
        whole in the picture, as the spec promises, or the shot ships half a sentence."""
        boxes = [{"x": 10, "y": 10, "w": 100, "h": 900}]
        callout = [{"x": 20, "y": 400, "w": 50, "h": 40}]
        window = {"width": 200, "height": 1000, "scale": 1}
        self.assertEqual(engine.crop_rect(boxes, window, max_height=300, keep=callout), (0, 0, 126, 456))

    def test_max_width_keeps_the_left_edge(self):
        """A wide region capped by max_width keeps its left edge and loses the right, as the old maxWidth did."""
        boxes = [{"x": 10, "y": 10, "w": 900, "h": 20}]
        window = {"width": 1000, "height": 100, "scale": 2}
        self.assertEqual(engine.crop_rect(boxes, window, max_width=300), (0, 0, 600, 92))

    def test_reset_gives_up_after_six_escape_presses(self):
        """A stuck layer must fail the shot after six presses, rather than hang or capture the previous dialog."""
        harness = Harness(layers=True)
        bridge = engine.Bridge(harness, self.root, {})
        with self.assertRaisesRegex(RuntimeError, "layers remain after 6 Escape presses"):
            engine.reset(bridge)
        self.assertEqual(sum(args == ["press", "escape"] for args in harness.calls), 6)


if __name__ == "__main__":
    unittest.main()
