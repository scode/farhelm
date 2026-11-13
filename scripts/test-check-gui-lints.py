"""Fixtures for check-gui-lints.py's parsing: expect lines, cargo's JSON, and ban coverage. Runs no cargo.

The real check needs cargo and the demo crate; these cases pin the parts that decide pass or fail, so a regression in
the parser cannot turn every ban into a silent pass.
"""
import importlib.util
import pathlib
import tomllib
import unittest

spec = importlib.util.spec_from_file_location("gui_lints", pathlib.Path(__file__).with_name("check-gui-lints.py"))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)
expectations, unmet, uncovered, banned_paths, diagnostics, cargo_failure = (
    checker.expectations,
    checker.unmet,
    checker.uncovered,
    checker.banned_paths,
    checker.diagnostics,
    checker.cargo_failure,
)


class GuiLintCheckTest(unittest.TestCase):
    """A ban that stops matching must fail the check, never pass it silently."""

    SOURCE = """\
use x;

fn first() {
    // expect: disallowed_methods gpui::rgb
    let _ = gpui::rgb(0);
}

fn second(v: Option<u8>) -> u8 {
    // expect: unwrap_used
    v.unwrap()
}
"""

    def test_expectations_bind_to_their_function_range(self):
        """Each expect line is attributed to its own function with that function's line range."""
        self.assertEqual(
            expectations(self.SOURCE),
            [("first", "disallowed_methods", "gpui::rgb", 3, 7), ("second", "unwrap_used", None, 8, 11)],
        )

    def test_a_diagnostic_in_another_function_does_not_count(self):
        """A diagnostic outside the demo function (e.g. a renamed path firing elsewhere) leaves the expectation unmet."""
        expected = expectations(self.SOURCE)
        found = [("disallowed_methods", 10, "use of a disallowed method `gpui::rgb`"), ("unwrap_used", 10, "used `unwrap()`")]
        self.assertEqual(unmet(expected, found), ["first: no `disallowed_methods` diagnostic for gpui::rgb"])

    def test_every_expectation_met(self):
        """Matching lint, line range and path satisfy every expectation."""
        expected = expectations(self.SOURCE)
        found = [("disallowed_methods", 5, "use of a disallowed method `gpui::rgb`"), ("unwrap_used", 10, "used `unwrap()`")]
        self.assertEqual(unmet(expected, found), [])

    def test_a_diagnostic_naming_a_longer_path_does_not_count(self):
        """`gpui::rgba` firing does not satisfy the `gpui::rgb` demo: only the exact backticked path matches."""
        expected = expectations(self.SOURCE)
        found = [("disallowed_methods", 5, "use of a disallowed method `gpui::rgba`"), ("unwrap_used", 10, "x")]
        self.assertEqual(unmet(expected, found), ["first: no `disallowed_methods` diagnostic for gpui::rgb"])

    def test_bans_without_a_demo_are_reported(self):
        """A new ban with no demo fails, whether written as a table or as a bare path."""
        config = tomllib.loads(
            'disallowed-methods = [{ path = "gpui::rgb" }, { path = "gpui::rgba" }]\n'
            'disallowed-types = ["gpui_base::Dialog"]\n'
        )
        self.assertEqual(
            uncovered(banned_paths(config), expectations(self.SOURCE)),
            [
                "gpui::rgba: banned in crates/gui/clippy.toml but has no demo in crates/gui/lint-demo",
                "gpui_base::Dialog: banned in crates/gui/clippy.toml but has no demo in crates/gui/lint-demo",
            ],
        )

    def test_cargo_json_keeps_only_clippy_codes_and_primary_spans(self):
        """Rustc warnings and secondary spans are ignored."""
        messages = [
            {"reason": "compiler-artifact"},
            {"reason": "compiler-message", "message": {"code": {"code": "unused_imports"}, "message": "x", "spans": [{"is_primary": True, "line_start": 1}]}},
            {"reason": "compiler-message", "message": {"code": {"code": "clippy::expect_used"}, "message": "used `expect()`", "spans": [{"is_primary": False, "line_start": 2}, {"is_primary": True, "line_start": 3}]}},
        ]
        self.assertEqual(diagnostics(messages), [("expect_used", 3, "used `expect()`")])

    def test_cargo_stopping_before_compiling_reports_its_stderr(self):
        """No compiler messages at all (a stale lockfile under --locked, say) fails with cargo's own words."""
        failure = cargo_failure(101, [{"reason": "build-finished", "success": False}], "error: the lock file needs updating")
        self.assertIn("the lock file needs updating", failure)

    def test_a_demo_that_no_longer_type_checks_fails(self):
        """A rustc error (the demo broke after an upgrade) fails; clippy's own deny errors do not."""
        rustc_error = {"reason": "compiler-message", "message": {"level": "error", "code": {"code": "E0433"}, "rendered": "cannot find `Corner`\n"}}
        clippy_error = {"reason": "compiler-message", "message": {"level": "error", "code": {"code": "clippy::unwrap_used"}, "rendered": "x"}}
        self.assertIn("does not compile", cargo_failure(101, [rustc_error, clippy_error], ""))
        self.assertIsNone(cargo_failure(101, [clippy_error], ""))



if __name__ == "__main__":
    unittest.main()
