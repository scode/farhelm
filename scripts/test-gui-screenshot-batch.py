#!/usr/bin/env python3
"""Self-test for scripts/gui-screenshot-batch.py: what a reviewer sees must not misstate what is built, and a batch must
never leave the app running.

The page is where the maintainer compares the GPUI app with the old app, so its labels are the claims that matter: a
reference scene the app does not show yet must say "not yet ported", a picture this batch did not take must say "not
captured in this batch" rather than "not yet ported", a mockup must say so, a capture must sit next to the reference
scene it belongs to, and scenes touched since the last batch must come first. The capture step starts and stops real
app runs, so its tests drive a fake `gui-agent.py` that records every call: `stop` must follow every `start`, also a
failed or interrupted one, and a failed `start` or `stop` must fail the batch. On Linux a capture is cropped to the
app's own drawing, so the PNG reading and cropping are tested against every PNG row filter and against a picture
without the expected frame. Nothing here needs the app, the reference pictures or the real checkout.
"""

from __future__ import annotations

import importlib.util
import json
import random
import struct
import sys
import tempfile
import unittest
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("batch", HERE / "gui-screenshot-batch.py")
batch = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(batch)


def png_chunk(kind: bytes, data: bytes) -> bytes:
    """One PNG chunk: length, type, data and CRC."""
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def tiny_png(path: Path, width: int, height: int) -> None:
    """A valid one-colour PNG of the given size, enough for the page's size label."""
    raw = b"".join(b"\x00" + b"\x00\x00\x00" * width for _ in range(height))
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    path.parent.mkdir(parents=True, exist_ok=True)
    chunks = png_chunk(b"IHDR", ihdr) + png_chunk(b"IDAT", zlib.compress(raw)) + png_chunk(b"IEND", b"")
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunks)


# The app's ground in the fixtures' pictures (the dark palette's, as RGBA).
GROUND = b"\x18\x1c\x24\xff"


def framed_pixels(width: int, height: int, margin: int, border: int) -> bytes:
    """RGBA pixels shaped like a Linux capture: a black margin, a light border inside it, and a dark ground."""
    black, edge, ground = b"\x00\x00\x00\xff", b"\xb8\xc2\xd3\xff", GROUND
    inner = width - 2 * (margin + border)
    rows = []
    for y in range(height):
        if y < margin or y >= height - margin:
            rows.append(black * width)
        elif y < margin + border or y >= height - margin - border:
            rows.append(black * margin + edge * (width - 2 * margin) + black * margin)
        else:
            rows.append(black * margin + edge * border + ground * inner + edge * border + black * margin)
    return b"".join(rows)


def filtered_png(path: Path, width: int, height: int, pixels: bytes) -> None:
    """An RGBA PNG whose rows cycle through all five PNG filter types, as an encoder like the bridge's may choose."""

    def paeth(a: int, b: int, c: int) -> int:
        p = a + b - c
        pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
        return a if pa <= pb and pa <= pc else b if pb <= pc else c

    stride, raw, prev = width * 4, bytearray(), bytes(width * 4)
    for y in range(height):
        line, kind = pixels[y * stride:(y + 1) * stride], y % 5
        out = bytearray()
        for i, x in enumerate(line):
            left, up, upleft = (line[i - 4] if i >= 4 else 0), prev[i], (prev[i - 4] if i >= 4 else 0)
            predictor = (0, left, up, (left + up) >> 1, paeth(left, up, upleft))[kind]
            out.append((x - predictor) & 0xFF)
        raw += bytes([kind]) + out
        prev = line
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    path.parent.mkdir(parents=True, exist_ok=True)
    chunks = png_chunk(b"IHDR", ihdr) + png_chunk(b"IDAT", zlib.compress(bytes(raw))) + png_chunk(b"IEND", b"")
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunks)


REFERENCE = [
    {"id": "main-running-session", "title": "Main window", "files": {"dark": {"wide": "old-app/main--dark--wide.png"}}},
    {"id": "rename-dialog", "title": "Rename", "files": {"light": {"narrow": "old-app/rename--light--narrow.png"}}},
    {"id": "native-app-menu", "title": "App menu", "files": {}, "reason": "the browser cannot show it"},
]


def scenes(ported: bool = False, mockup: bool = False) -> dict:
    """A minimal well-formed scene list: one app scene in two variants, one golden, three reference scenes."""
    return {
        "reference_commit": "pin",
        "app_variants": [
            {"theme": "light", "size": "wide", "window": [1600, 1000]},
            {"theme": "dark", "size": "narrow", "window": [900, 700]},
        ],
        "app_scenes": [{"id": "app-main-window", "title": "App window", "reference": "main-running-session",
                        "mockup": mockup, "steps": [["wait", "--idle"]]}],
        "foundation_goldens": [{"golden": "dialog", "title": "Dialog frame", "reference": "rename-dialog"}],
        "reference_scenes": [{"id": "main-running-session", "ported": ported}, {"id": "rename-dialog", "ported": False},
                             {"id": "native-app-menu", "ported": False}],
    }


class Page(unittest.TestCase):
    """The page's labels, built from fixture captures."""

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.out, self.ref = root / "out", root / "ref"
        tiny_png(self.out / "captures/app-main-window--light--wide.png", 1600, 1000)
        tiny_png(self.out / "captures/golden-dialog--linux--light.png", 400, 300)
        self.captures = {
            "commit": "abc123",
            "app": {"app-main-window": {"light": {"wide": "captures/app-main-window--light--wide.png"}}},
            "goldens": {"dialog": {"linux": {"light": "captures/golden-dialog--linux--light.png"}}},
            "errors": {},
            "start_failures": [],
            "stop_failures": [],
        }

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def build(self, **kw) -> str:
        """The page for this test's fixture, with any of scenes, captures, touched and copy_refs replaced."""
        return self.build_and_unknown(**kw)[0]

    def build_and_unknown(self, **kw) -> tuple[str, list[str]]:
        """The page and the unknown `--touched` ids `build_page` returns beside it."""
        captures = kw.pop("captures", self.captures)
        return batch.build_page(self.out, kw.pop("scenes", scenes()), REFERENCE, self.ref, captures,
                                kw.pop("touched", []), kw.pop("copy_refs", False))

    def section(self, text: str, scene_id: str) -> str:
        """One scene's whole section, header included."""
        start = text.index(f'<section id="{scene_id}"')
        return text[start:text.index("</section>", start)]

    def gpui_cell(self, text: str, scene_id: str) -> str:
        """The GPUI-app half of a scene's section, so a label in the header cannot satisfy an assertion."""
        sec = self.section(text, scene_id)
        return sec[sec.index("<h4>GPUI app</h4>"):sec.index("<h4>Old app</h4>")]

    def test_an_unported_reference_scene_says_so_and_still_shows_the_old_app(self) -> None:
        """A reviewer must never read an empty slot as 'done': unported scenes carry the label in the GPUI-app cell,
        beside the old picture."""
        text = self.build()
        self.assertIn('<p class="missing">not yet ported</p>', self.gpui_cell(text, "rename-dialog"))
        self.assertIn("rename--light--narrow.png", self.section(text, "rename-dialog"))

    def test_a_scene_the_old_app_could_not_show_names_the_reason(self) -> None:
        """Some old-app scenes could not be captured (native menus); the page says why instead of showing a hole."""
        sec = self.section(self.build(), "native-app-menu")
        self.assertIn("not captured: the browser cannot show it", sec)

    def test_an_app_capture_sits_next_to_its_reference_with_its_variant_and_real_size(self) -> None:
        """The label carries the variant the batch asked for and the measured pixels, so a window that opened at
        another size than asked is visible on the page."""
        sec = self.section(self.build(), "app-main-window")
        self.assertIn("captures/app-main-window--light--wide.png", sec)
        self.assertIn("light wide (1600x1000)", sec)
        self.assertIn('alt="light wide (1600x1000)"', sec)
        self.assertIn("main--dark--wide.png", sec)

    def test_a_picture_this_batch_did_not_take_is_not_called_unported(self) -> None:
        """A page made without a capture (or on a platform with no goldens) must not claim the work is missing: the app
        scene and the golden say "not captured in this batch"."""
        text = self.build(captures={})
        for scene_id in ("app-main-window", "golden-dialog"):
            cell = self.gpui_cell(text, scene_id)
            self.assertIn("not captured in this batch", cell)
            self.assertNotIn("not yet ported", cell)
        self.assertIn("unknown (no capture)", text)

    def test_a_golden_is_labelled_with_its_platform(self) -> None:
        """Goldens are copies of every platform's, shown wherever the batch ran, so each says whose it is."""
        self.assertIn("golden, linux, light (400x300)", self.gpui_cell(self.build(), "golden-dialog"))

    def test_the_header_counts_are_in_proper_english(self) -> None:
        """The fixture has one app scene and one golden: "1 app scene", not "1 app scenes"."""
        text = self.build()
        self.assertIn("0 of 3 reference scenes ported; 1 app scene; 1 foundation golden.", text)

    def test_a_mockup_is_labelled_a_mockup(self) -> None:
        """Mockups never count as evidence that something is built."""
        sec = self.section(self.build(scenes=scenes(mockup=True)), "app-main-window")
        self.assertIn("MOCKUP", sec)
        self.assertIn('class="mockup"', sec)

    def test_a_ported_scene_is_not_labelled_unported(self) -> None:
        """Marking a reference scene ported is how an area records progress; the page must reflect it."""
        sec = self.section(self.build(scenes=scenes(ported=True)), "main-running-session")
        self.assertNotIn("not yet ported", sec)

    def test_touched_scenes_come_first_and_only_once(self) -> None:
        """A reviewer looks at what moved first; a touched scene repeated below would be counted twice."""
        text = self.build(touched=["rename-dialog"])
        self.assertLess(text.index('<section id="rename-dialog"'), text.index('<section id="app-main-window"'))
        self.assertEqual(text.count('<section id="rename-dialog"'), 1)
        self.assertIn("Touched since the last batch", text)

    def test_an_unknown_touched_id_is_flagged(self) -> None:
        """A mistyped `--touched` id would silently drop a scene from the top of the page; the page names it."""
        text, unknown = self.build_and_unknown(touched=["golden-nope", "golden-dialog"])
        self.assertIn("--touched ids that match no scene: golden-nope", text)
        self.assertEqual(unknown, ["golden-nope"], "the ids `page` repeats on stderr")
        self.assertNotIn("match no scene", self.build(touched=["golden-dialog"]))

    def test_a_failed_capture_is_shown_as_a_failure(self) -> None:
        """A failure is evidence too: it shows in place of the picture, with what failed."""
        self.captures["app"] = {}
        self.captures["errors"] = {"app-main-window": ["dark narrow: the app did not start"]}
        cell = self.gpui_cell(self.build(), "app-main-window")
        self.assertIn("capture failed: dark narrow: the app did not start", cell)
        self.assertNotIn("not captured in this batch", cell)

    def test_an_app_run_that_did_not_start_or_stop_is_flagged(self) -> None:
        """A batch that left processes behind, or whose app never started, must say so where the maintainer looks, not
        only in a log (a list with no app scenes has no cell to show a start failure in)."""
        self.captures["start_failures"] = ["dark narrow: the app did not start"]
        self.captures["stop_failures"] = ["light wide: `stop` failed"]
        text = self.build()
        self.assertIn("an app run did not start: dark narrow: the app did not start", text)
        self.assertIn("an app run did not stop cleanly: light wide: `stop` failed", text)

    def test_a_reference_scene_missing_from_the_list_is_flagged(self) -> None:
        """The committed list must keep up with the reference set; a scene it forgot is called out, not dropped."""
        listed = scenes()
        listed["reference_scenes"] = listed["reference_scenes"][:2]
        self.assertIn("missing from the scene list: native-app-menu", self.build(scenes=listed))

    def test_copied_references_land_beside_the_page(self) -> None:
        """`--copy-references` makes the output directory self-contained, so it can be handed on as is."""
        tiny_png(self.ref / "old-app/main--dark--wide.png", 1600, 1000)
        text = self.build(copy_refs=True)
        self.assertIn('src="reference/old-app/main--dark--wide.png"', text)
        self.assertTrue((self.out / "reference/old-app/main--dark--wide.png").exists())

    def test_text_from_the_scene_list_is_escaped(self) -> None:
        """Scene titles are data, never markup."""
        listed = scenes()
        listed["app_scenes"][0]["title"] = "<script>"
        self.assertNotIn("<script>", self.build(scenes=listed).split("<h1>")[1])


class SceneList(unittest.TestCase):
    """The scene list's validation, on the committed list and on broken fixtures."""

    def test_the_committed_scene_list_is_well_formed(self) -> None:
        """The capture and the page trust the committed list; every rule `validate_scenes` checks holds for it."""
        listed = json.loads(batch.SCENE_LIST.read_text(encoding="utf-8"))
        self.assertEqual(batch.validate_scenes(listed, batch.tree_goldens(batch.GOLDENS)), [])

    def test_the_fixture_is_well_formed(self) -> None:
        """The baseline for the broken cases below: each one breaks exactly one rule."""
        self.assertEqual(batch.validate_scenes(scenes(), {"dialog"}), [])

    def test_each_broken_rule_is_reported(self) -> None:
        """A broken list would capture the wrong thing or misreport it only when someone reads the page; each rule
        catches one such break up front."""
        cases = {
            "duplicate scene id 'rename-dialog'": lambda s: s["reference_scenes"].append(
                {"id": "rename-dialog", "ported": False}),
            "duplicate scene id 'app-main-window'": lambda s: s["app_scenes"].append(dict(s["app_scenes"][0])),
            "reference 'nope' is not a listed reference scene": lambda s: s["app_scenes"][0].update(reference="nope"),
            "steps must be a list of non-empty lists of strings": lambda s: s["app_scenes"][0].update(steps=["wait"]),
            "foundation golden needs golden and title": lambda s: s["foundation_goldens"].append({"title": "x"}),
            "golden 'tooltip' is listed but not committed": lambda s: s["foundation_goldens"].append(
                {"golden": "tooltip", "title": "t", "reference": None}),
            "duplicate app variant light wide": lambda s: s["app_variants"].append(dict(s["app_variants"][0])),
            "app variant needs theme light|dark": lambda s: s["app_variants"].append(
                {"theme": "system", "size": "wide", "window": [1, 1]}),
            "missing top-level key 'app_variants'": lambda s: s.pop("app_variants"),
        }
        for expected, breaks in cases.items():
            listed = scenes()
            breaks(listed)
            problems = batch.validate_scenes(listed, {"dialog"})
            self.assertTrue(any(expected in p for p in problems), (expected, problems))
        problems = batch.validate_scenes(scenes(), {"dialog", "buttons"})
        self.assertIn("golden 'buttons' is committed but not listed in foundation_goldens", problems)


class Pictures(unittest.TestCase):
    """Reading and cropping captures: the crop changes what the page shows, so a misread picture must never be cut."""

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_every_png_row_filter_reads_back_exactly(self) -> None:
        """The bridge's encoder picks a filter per row; a mistake in undoing any one of them would shift pixels without
        an error, so each of the five must give back the pixels that were written."""
        # Seeded noise rather than a smooth gradient: Paeth's tie-breaking between its three predictors only matters
        # where they differ in uneven ways, which a gradient never makes them do.
        pixels = random.Random(7).randbytes(16 * 10 * 4)
        filtered_png(self.root / "f.png", 16, 10, pixels)
        self.assertEqual(batch.read_rgba(self.root / "f.png"), (16, 10, bytearray(pixels)))

    def test_a_malformed_png_is_reported_not_raised(self) -> None:
        """A capture whose header claims more rows than its data holds, or whose header chunk has the wrong length,
        must come back from `crop_frame` as a reason, not escape as an exception: an escaping error aborts the whole
        batch before its results are written."""
        path = self.root / "short.png"
        batch.write_rgba(path, 4, 4, GROUND * 16)
        data = bytearray(path.read_bytes())
        # IHDR's height field sits at bytes 20..24; claim 8 rows for 4 rows of data (the CRC is not checked).
        data[20:24] = (8).to_bytes(4, "big")
        path.write_bytes(bytes(data))
        self.assertIn("bytes of pixel data", batch.crop_frame(path, 1, 0))
        # An IHDR chunk one byte short.
        bad = bytearray(batch.PNG_SIGNATURE) + (12).to_bytes(4, "big") + b"IHDR" + bytes(12) + bytes(4)
        path.write_bytes(bytes(bad))
        self.assertIn("IHDR chunk of 12 bytes", batch.crop_frame(path, 1, 0))

    def test_the_linux_frame_is_cropped_to_the_apps_drawing(self) -> None:
        """A Linux capture of a window asked for 42 pixels larger than the variant comes out at the variant's size,
        holding only the ground: no black margin and no border."""
        path = self.root / "shot.png"
        filtered_png(path, 50, 46, framed_pixels(50, 46, 20, 1))
        self.assertIsNone(batch.crop_frame(path, 20, 1))
        width, height, pixels = batch.read_rgba(path)
        self.assertEqual((width, height), (8, 4))
        self.assertEqual(set(bytes(pixels[i:i + 4]) for i in range(0, len(pixels), 4)), {GROUND})

    def test_a_capture_without_the_expected_frame_is_left_alone(self) -> None:
        """If Kit's frame changes (or a compositor draws decorations itself), cropping a fixed width would cut into the
        app or leave a band under a label that claims the asked size; the capture is kept as it was and the reason
        names what was found."""
        path = self.root / "shot.png"
        batch.write_rgba(path, 50, 46, GROUND * (50 * 46))
        before = path.read_bytes()
        problem = batch.crop_frame(path, 20, 1)
        self.assertIn("expected a 20-pixel black window frame", problem)
        self.assertEqual(path.read_bytes(), before)
        filtered_png(path, 50, 46, framed_pixels(50, 46, 19, 1))
        self.assertIn("left 19, right 19, top 19, bottom 19", batch.crop_frame(path, 20, 1))
        tiny_png(path, 50, 46)
        self.assertIn("not an 8-bit RGBA PNG", batch.crop_frame(path, 20, 1) or "")


# A stand-in for scripts/gui-agent.py. It appends every call to `calls.jsonl` in the runs directory (the run
# directory's parent), makes the run directory on `start` like the real one, and on `screenshot` writes a PNG of the
# window size `start` was given, framed the way a Linux capture is. `fail.json` there lists what goes wrong: a verb
# named there exits 1; "start-before-mkdir" fails `start` before it makes the run directory; "hang-<verb>" never
# returns from that verb; "no-frame" writes screenshots without the frame; "interrupt" makes `start` send SIGINT to the
# batch, as a Ctrl-C during a build would. The pictures come from this self-test's own helpers, loaded by path.
FAKE_AGENT = '''
import importlib.util, json, os, signal, sys, time
from pathlib import Path
run, verb, rest = Path(sys.argv[1]), sys.argv[2], sys.argv[3:]
runs = run.parent
with open(runs / "calls.jsonl", "a") as log:
    log.write(json.dumps([run.name, verb, *rest]) + "\\n")
fail = json.loads((runs / "fail.json").read_text()) if (runs / "fail.json").exists() else []
if f"hang-{verb}" in fail:
    time.sleep(3600)
if verb == "start":
    if "start-before-mkdir" in fail:
        sys.exit(1)
    run.mkdir()
    (run / "window-size").write_text(rest[rest.index("--window-size") + 1])
    if "interrupt" in fail:
        os.kill(os.getppid(), signal.SIGINT)
        sys.exit(130)
if verb in fail:
    sys.exit(1)
if verb == "screenshot":
    spec = importlib.util.spec_from_file_location("selftest", SELFTEST)
    selftest = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(selftest)
    width, height = (int(n) for n in (run / "window-size").read_text().split("x"))
    if "no-frame" in fail:
        selftest.batch.write_rgba(Path(rest[0]), width, height, selftest.GROUND * (width * height))
    else:
        selftest.batch.write_rgba(Path(rest[0]), width, height, selftest.framed_pixels(width, height, 20, 1))
'''.replace("SELFTEST", repr(str(Path(__file__).resolve())))


class Capture(unittest.TestCase):
    """The capture step against a fake harness: every start is stopped, and failures are recorded, not dropped."""

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.out, self.runs, self.goldens = root / "out", root / "runs", root / "goldens"
        self.runs.mkdir()
        self.agent = root / "fake-agent.py"
        self.agent.write_text(FAKE_AGENT, encoding="utf-8")
        for theme in ("light", "dark"):
            tiny_png(self.goldens / "linux" / f"dialog--{theme}.png", 4, 3)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def make_fail(self, *verbs: str) -> None:
        """Make the fake harness fail these verbs (named `make_fail` because `fail` is unittest's own)."""
        (self.runs / "fail.json").write_text(json.dumps(list(verbs)), encoding="utf-8")

    def capture(self, **kw) -> dict:
        """Run the capture step on the fixture scene list against the fake harness, a scratch goldens tree and a fixed
        commit, so nothing reads the real checkout."""
        return batch.capture(self.out, kw.pop("scenes", scenes()), kw.pop("build", True), agent=self.agent,
                             runs=self.runs, goldens=self.goldens, platform=kw.pop("platform", "linux"), commit="test",
                             **kw)

    def calls(self) -> list[list[str]]:
        """Every call the fake harness received, in order, as [run name, verb, args...]."""
        return [json.loads(line) for line in (self.runs / "calls.jsonl").read_text().splitlines()]

    def verbs(self, run: str) -> list[str]:
        """The verbs one run (a variant, `<theme>-<size>`) received, in order."""
        return [call[1] for call in self.calls() if call[0] == run]

    def test_each_variant_is_its_own_run_with_its_theme_and_size(self) -> None:
        """The bridge cannot change theme or size, so each variant is a fresh app run started with them; only the first
        run builds, and the shared lock reaches every start. On Linux the window is asked 42 pixels larger than the
        variant (Kit's frame) on a monitor that holds it, and the cropped capture is the variant's size."""
        record = self.capture(lock=Path("/locks/gui"))
        self.assertEqual(self.verbs("light-wide"), ["start", "wait", "screenshot", "stop"])
        self.assertEqual(self.verbs("dark-narrow"), ["start", "wait", "screenshot", "stop"])
        starts = {call[0]: call[2:] for call in self.calls() if call[1] == "start"}
        self.assertEqual(starts["light-wide"], ["--window-size", "1642x1042", "--appearance", "light",
                                                "--size", "1842x1242", "--lock", "/locks/gui"])
        self.assertEqual(starts["dark-narrow"], ["--window-size", "942x742", "--appearance", "dark",
                                                 "--size", "1600x1200", "--no-build", "--lock", "/locks/gui"])
        self.assertEqual(record["app"]["app-main-window"]["dark"]["narrow"],
                         "captures/app-main-window--dark--narrow.png")
        self.assertEqual(batch.png_size(self.out / "captures/app-main-window--dark--narrow.png"), (900, 700))
        self.assertFalse(batch.capture_failed(record))
        self.assertEqual(json.loads((self.out / "captures.json").read_text())["commit"], "test")

    def test_a_mac_run_asks_for_the_variants_size_and_crops_nothing(self) -> None:
        """Kit draws no frame on a Mac and the monitor is the real screen, so the start carries the variant's size and
        no `--size`."""
        self.capture(platform="macos")
        starts = {call[0]: call[2:] for call in self.calls() if call[1] == "start"}
        self.assertEqual(starts["dark-narrow"], ["--window-size", "900x700", "--appearance", "dark", "--no-build"])

    def test_a_capture_without_the_frame_is_shown_uncropped_with_the_reason(self) -> None:
        """The picture is still evidence, but its label would claim a size it does not have; the batch says so and
        fails."""
        self.make_fail("no-frame")
        record = self.capture()
        self.assertIn("app-main-window", record["app"])
        self.assertIn("left uncropped", record["errors"]["app-main-window"][0])
        self.assertTrue(batch.capture_failed(record))

    def test_clean_runs_are_removed_and_never_under_the_output(self) -> None:
        """A run directory holds the helm credential the harness writes; it must not be handed on with the page."""
        self.capture()
        self.assertFalse((self.runs / "light-wide").exists())
        self.assertEqual(sorted(p.name for p in self.out.iterdir()), ["captures", "captures.json"])

    def test_a_failed_start_is_still_stopped_and_recorded(self) -> None:
        """`start` can fail after taking things (a compositor, the lock); `stop` must still run, every scene says the
        app did not start, and the run is kept for its logs."""
        self.make_fail("start")
        record = self.capture()
        self.assertEqual(self.verbs("light-wide"), ["start", "stop"])
        self.assertIn("light wide: the app did not start", record["errors"]["app-main-window"][0])
        self.assertIn(f"kept at {self.runs / 'light-wide'}", record["start_failures"][0])
        self.assertTrue((self.runs / "light-wide").exists(), "a failed run is kept for its logs")
        self.assertTrue(batch.capture_failed(record))

    def test_a_failed_start_fails_the_batch_without_app_scenes(self) -> None:
        """A list with no app scenes has no scene to record the failure under; the start failure itself must still
        fail the batch and keep the run, or a broken app would pass as a clean batch with nothing recorded."""
        self.make_fail("start")
        listed = scenes()
        listed["app_scenes"] = []
        record = self.capture(scenes=listed)
        self.assertEqual(record["errors"], {})
        self.assertEqual(len(record["start_failures"]), 2)
        self.assertTrue((self.runs / "light-wide").exists())
        self.assertTrue(batch.capture_failed(record))

    def test_stop_is_skipped_when_start_made_no_run_directory(self) -> None:
        """`stop` against a directory that was never made would fail and report a run "that may still be running"
        when nothing ever ran."""
        self.make_fail("start-before-mkdir")
        record = self.capture()
        self.assertEqual(self.verbs("light-wide"), ["start"])
        self.assertEqual(record["stop_failures"], [])
        self.assertEqual(record["start_failures"][0], "light wide: the app did not start")
        self.assertTrue(batch.capture_failed(record))

    def test_an_interrupted_start_is_still_stopped(self) -> None:
        """Ctrl-C during a multi-minute build must not leave the compositor and app running."""
        self.make_fail("interrupt")
        with self.assertRaises(KeyboardInterrupt):
            self.capture()
        self.assertEqual(self.verbs("light-wide"), ["start", "stop"])

    def test_a_failed_step_is_recorded_and_the_run_kept_and_stopped(self) -> None:
        """A scene that cannot be reached is a recorded failure, not a missing picture, and its run's logs are what
        show why."""
        self.make_fail("wait")
        record = self.capture()
        self.assertEqual(self.verbs("light-wide"), ["start", "wait", "stop"])
        self.assertEqual(record["errors"]["app-main-window"][0], "light wide: step failed: wait --idle")
        self.assertNotIn("app-main-window", record["app"])
        self.assertTrue((self.runs / "light-wide").exists(), "a failed run is kept for its logs")

    def test_a_hung_step_times_out_as_a_failure(self) -> None:
        """A bridge command that never returns would hang the whole batch; past the step timeout it is a failed step,
        and the run still stops."""
        self.make_fail("hang-wait")
        record = self.capture(step_timeout=0.5)
        self.assertEqual(self.verbs("light-wide"), ["start", "wait", "stop"])
        self.assertEqual(record["errors"]["app-main-window"][0], "light wide: step failed: wait --idle")
        self.assertTrue(batch.capture_failed(record))

    def test_a_failed_stop_fails_the_batch(self) -> None:
        """A run that may still be running (and, with `--lock`, may still hold the shared lock) must not end in a
        quiet success."""
        self.make_fail("stop")
        record = self.capture()
        self.assertEqual(len(record["stop_failures"]), 2)
        self.assertTrue(batch.capture_failed(record))

    def test_goldens_of_every_platform_are_copied_wherever_the_batch_runs(self) -> None:
        """Goldens are files, not renders, so a Mac batch shows the Linux goldens (labelled as such) and does not fail
        for lacking its own; the batch's exit status then means something on a Mac too."""
        tiny_png(self.goldens / "macos" / "dialog--dark.png", 4, 3)
        record = self.capture(platform="macos")
        self.assertEqual(record["goldens"]["dialog"], {
            "linux": {"light": "captures/golden-dialog--linux--light.png",
                      "dark": "captures/golden-dialog--linux--dark.png"},
            "macos": {"dark": "captures/golden-dialog--macos--dark.png"},
        })
        self.assertTrue((self.out / "captures/golden-dialog--macos--dark.png").exists())
        self.assertFalse(batch.capture_failed(record))

    def test_a_golden_no_platform_has_is_an_error(self) -> None:
        """A listed golden with no picture anywhere must show as a failure, not as an empty cell."""
        (self.goldens / "linux" / "dialog--dark.png").unlink()
        record = self.capture()
        self.assertIn("dialog--dark.png", record["errors"]["golden-dialog"][0])
        self.assertEqual(list(record["goldens"]["dialog"]["linux"]), ["light"])
        self.assertTrue(batch.capture_failed(record))


class CommandLine(unittest.TestCase):
    """The command line's refusals, checked against a scratch checkout so a regression cannot write into the real
    one."""

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_the_output_must_be_outside_the_repository(self) -> None:
        """Captured screenshots are never committed on main, so the script refuses a path inside the checkout before
        it creates anything there."""
        repo = self.root / "repo"
        repo.mkdir()
        with self.assertRaises(SystemExit):
            batch.main(["page", str(repo / "batch-out"), "--reference", str(self.root / "ref")], repo=repo)
        self.assertFalse((repo / "batch-out").exists())
        self.assertIsNone(batch.refuse_inside(self.root / "elsewhere", repo))
        self.assertIsNotNone(batch.refuse_inside(repo, repo))

    def test_an_output_directory_with_a_capture_is_refused(self) -> None:
        """A second capture into the same directory would mix two batches under one commit."""
        out = self.root / "out"
        (out / "captures").mkdir(parents=True)
        with self.assertRaises(SystemExit):
            batch.main(["capture", str(out)], repo=self.root / "repo")
        self.assertIsNone(batch.refuse_reuse(self.root / "fresh"))


if __name__ == "__main__":
    unittest.main()
