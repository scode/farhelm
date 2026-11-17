#!/usr/bin/env python3
"""Tests for `scenario_json5.py`, the reader of the docs-media scenario files.

Why these matter: the loader accepts a deliberately small subset of JSON5, so two things can go wrong. A real scenario
can start using a feature the subset lacks, which the real-file tests catch, and the translation to strict JSON can
mangle something it should have copied, such as a `//` or a comma inside a string, which the feature tests catch. The
refusal tests pin that JSON5 outside the subset fails with the file and line instead of being half-understood.

Run with `python3 -I scripts/test-scenario-json5.py`. Seconds, no network, nothing written outside a temporary
directory. Whether the hero publisher's import leaves bytecode behind is checked by its own `--self-test`.
"""

import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

# Loading the module must not leave a `__pycache__` in the checkout.
sys.dont_write_bytecode = True

SCRIPTS = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("scenario_json5", SCRIPTS / "scenario_json5.py")
scenario_json5 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(scenario_json5)

SCENARIOS = {
    name: SCRIPTS.parent / "docs" / name / "scenario.json5" for name in ("readme-hero", "readme-video", "docs-shots")
}


class RealScenarios(unittest.TestCase):
    """Every scenario in the repository parses, into the shape the capture flows and the publish script read."""

    def test_the_files_still_use_the_features_the_subset_exists_for(self):
        """Premise for the parse tests below: between them, the files use comments, bare keys and trailing commas.

        If none did any more, the parse tests would pass on plain JSON and prove nothing about the translation.
        """
        text = "\n".join(path.read_text(encoding="utf-8") for path in SCENARIOS.values())
        self.assertIn("\n//", text)
        self.assertIn("  viewport: {", text)
        self.assertRegex(text, r",\n\s*[}\]]")

    def test_each_scenario_parses_with_its_viewport_hosts_and_sessions(self):
        """Each file yields a dict with integer viewport fields and non-empty host and session lists."""
        for name, path in SCENARIOS.items():
            with self.subTest(name):
                scenario = scenario_json5.load(path)
                for field in ("width", "height", "scale"):
                    self.assertIs(type(scenario["viewport"][field]), int)
                self.assertTrue(scenario["hosts"])
                self.assertTrue(scenario["sessions"])
                for session in scenario["sessions"]:
                    self.assertIsInstance(session["title"], str)

    def test_the_hero_values_survive_the_translation(self):
        """Spot checks on the frozen hero design, whose values change only when the maintainer asks.

        They cover a nested object, a string carrying `$` and `@`, a boolean and the last element of a list that ends
        in a trailing comma.
        """
        hero = scenario_json5.load(SCENARIOS["readme-hero"])
        self.assertEqual(hero["viewport"], {"width": 1280, "height": 800, "scale": 2})
        self.assertEqual(hero["hosts"][2], {"key": "gpu-1", "alias": "gpu-1", "kind": "remote", "ssh": "$USER@localhost"})
        self.assertIs(hero["sessions"][0]["yolo"], True)
        self.assertEqual(len(hero["sessions"]), 7)
        self.assertEqual(hero["sessions"][-1]["title"], "port ci to nextest")


class SupportedFeatures(unittest.TestCase):
    """The JSON5 features the subset accepts, each translated without touching anything around it."""

    def test_line_comments_are_dropped(self):
        """Before the document, after a brace, after a value, on a line of their own, and at the end without a newline."""
        text = "// head\n{ // after brace\n  a: 1, // after value\n  // own line\n  b: 2 }\n// tail"
        self.assertEqual(scenario_json5.loads(text), {"a": 1, "b": 2})

    def test_syntax_inside_a_string_is_kept(self):
        """A `//`, a comma before a bracket, a bare key and an escaped quote inside a string are content, not syntax."""
        text = '{ url: "https://x//y", odd: ",]", key: "a: 1", quote: "say \\"hi\\" // ok", "quoted key": 1 }'
        expected = {"url": "https://x//y", "odd": ",]", "key": "a: 1", "quote": 'say "hi" // ok', "quoted key": 1}
        self.assertEqual(scenario_json5.loads(text), expected)

    def test_unquoted_keys(self):
        """Letters, digits, `_` and `$`, a space before the colon, and words that are JSON literals elsewhere."""
        text = "{ shown_ssh: 1, $x: 2, a1: 3, exit_code : 4, null: true, false: null }"
        expected = {"shown_ssh": 1, "$x": 2, "a1": 3, "exit_code": 4, "null": True, "false": None}
        self.assertEqual(scenario_json5.loads(text), expected)

    def test_trailing_commas_in_objects_and_arrays_even_across_comments(self):
        text = "{ a: [1, 2, ], b: { c: 3e2, }, d: [ [], {}, ], e: -4, // last\n}"
        self.assertEqual(scenario_json5.loads(text), {"a": [1, 2], "b": {"c": 300.0}, "d": [[], {}], "e": -4})


class Refusals(unittest.TestCase):
    """JSON5 outside the subset, and broken files, fail with the source and the file's line rather than misparse."""

    def assert_refused(self, text, line):
        with self.assertRaises(scenario_json5.ScenarioSyntaxError) as caught:
            scenario_json5.loads(text, source="t.json5")
        self.assertEqual(caught.exception.line, line)
        prefix = "t.json5: " if line is None else f"t.json5:{line}: "
        self.assertTrue(str(caught.exception).startswith(prefix), str(caught.exception))
        return str(caught.exception)

    def test_json5_features_outside_the_subset(self):
        """Each sits on line 2, after a comment and a bare key, so a drifted line count shows."""
        cases = {
            "single-quoted string": "{ // c\n  a: 'x' }",
            "block comment": "{ // c\n  /* c */ a: 1 }",
            "bare word": "{ // c\n  a: yes }",
            "hex number": "{ // c\n  a: 0x1F }",
            "string escape": '{ // c\n  a: "\\x41" }',
            "line continuation": '{ // c\n  a: "x\\\ny",\n  b: 1 }',
            "comma in an empty array": "{ // c\n  a: [,] }",
            "comma in an empty object": "{ // c\n  a: { , } }",
            "comment between key and colon": "{ // c\n  a // c\n : 1 }",
        }
        for name, text in cases.items():
            with self.subTest(name):
                self.assert_refused(text, 2)

    def test_infinity_and_nan_name_no_line(self):
        for word in ("Infinity", "-Infinity", "NaN"):
            with self.subTest(word):
                self.assertIn(f"{word} is not supported", self.assert_refused(f"[{word}]", None))

    def test_duplicate_keys(self):
        """JSON5 keeps the last of two equal keys; a scenario must not, since one field would vanish silently."""
        self.assertEqual(self.assert_refused('{ status: 1, "status": 2 }', None), "t.json5: duplicate key 'status'")

    def test_a_broken_file_names_its_path_and_line(self):
        """A missing comma after a comment-laden line is reported on the line it is missing from, under the path."""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "scenario.json5"
            path.write_text("// one\n{\n  a: 1, // three\n  b: 2\n  c: 3,\n}\n", encoding="utf-8")
            with self.assertRaises(scenario_json5.ScenarioSyntaxError) as caught:
                scenario_json5.load(path)
            self.assertTrue(str(caught.exception).startswith(f"{path}:5: "), str(caught.exception))


if __name__ == "__main__":
    unittest.main()
