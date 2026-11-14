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
ban_exceptions, stray_exceptions = checker.ban_exceptions, checker.stray_exceptions
manifest_switches, stray_configs = checker.manifest_switches, checker.stray_configs
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



class BanExceptionTest(unittest.TestCase):
    """A ban switched off outside its owners would quietly undo the rule it protects, in a file the ownership check
    only reports, so the check must find every such attribute and accept only the owners' own."""

    OWNER = """\
#[allow(
    clippy::disallowed_methods,
    reason = "the owner"
)]
/// The one exemption: a doc comment between the attribute and its item.
#[rustfmt::skip]
mod raw {
    pub fn color() {}
}
"""

    @staticmethod
    def owners_but(path: str, source: str) -> list[tuple[str, str]]:
        """Every owner's file holding just its owner's exception, except `path`, which holds `source`."""
        files = {}
        for owner_path, item in checker.EXCEPTION_OWNERS:
            files.setdefault(owner_path, "")
            files[owner_path] += f"#[allow(clippy::disallowed_methods)]\nfn {item}() {{}}\n"
        files[path] = source
        return sorted(files.items())

    def test_an_owners_exception_is_accepted_with_its_item(self):
        """Specified: a multi-line `allow` naming a disallowed lint binds to the next item, skipping a doc comment and
        another attribute between them; with every owner holding its one exception, nothing is reported."""
        self.assertEqual(ban_exceptions(self.OWNER), [(1, "raw")])
        self.assertEqual(stray_exceptions(self.owners_but("farhelm-gui/src/theme.rs", self.OWNER)), [])

    def test_the_same_exception_elsewhere_is_reported(self):
        """Specified: the owner's attribute in another file, or on another item, is a problem naming file and line."""
        self.assertEqual(
            stray_exceptions(self.owners_but("farhelm-gui/src/session_list/row.rs", self.OWNER)),
            [
                "farhelm-gui/src/session_list/row.rs:1: a lint ban is switched off outside its owners "
                "(docs/gpui/foundation-ownership.md, lint exceptions)"
            ],
        )
        renamed = self.OWNER.replace("mod raw", "mod other")
        self.assertEqual(
            stray_exceptions(self.owners_but("farhelm-gui/src/theme.rs", renamed)),
            [
                "farhelm-gui/src/theme.rs:1: a lint ban is switched off outside its owners "
                "(docs/gpui/foundation-ownership.md, lint exceptions)",
                "farhelm-gui/src/theme.rs: owner raw holds 0 ban exceptions, not exactly one",
            ],
        )

    def test_an_owners_name_used_twice_is_not_the_owner(self):
        """Specified: a second item of an owner's name in its file carrying an exception (a method or nested function
        called `timer`) makes the owner hold two, which is reported."""
        twice = "#[allow(clippy::disallowed_methods)]\nfn timer() {}\nimpl X {\n    #[allow(clippy::disallowed_methods)]\n    fn timer() {}\n}\n"
        self.assertEqual(
            stray_exceptions(self.owners_but("farhelm-gui/src/ticker.rs", twice)),
            ["farhelm-gui/src/ticker.rs: owner timer holds 2 ban exceptions, not exactly one"],
        )

    def test_groups_and_spacing_that_switch_a_ban_off_are_found(self):
        """Specified: allowing clippy's `style` or `all` group, or every warning, switches the bans off too and is found,
        as is an attribute written with spaces around `#`, `[` and `::`; an unrelated `allow` is not."""
        source = """\
#[allow(clippy::style)]
fn a() {}
#[allow(clippy::all)]
fn b() {}
#[allow(warnings)]
fn c() {}
# [allow(clippy :: disallowed_types)]
fn d() {}
#[allow(dead_code)]
fn e() {}
"""
        self.assertEqual(ban_exceptions(source), [(1, "a"), (3, "b"), (5, "c"), (7, "d")])

    def test_inner_and_statement_exceptions_have_no_owner(self):
        """Specified: an inner attribute and one on a statement bind to no item, so they are always reported, as is an
        `expect` or a `cfg_attr` form; an `allow` of an ordinary lint is not a ban exception."""
        source = """\
#![allow(clippy::disallowed_types)]
fn timer() {
    #[expect(clippy::disallowed_methods)]
    let _ = now();
}
#[cfg_attr(test, allow(clippy::disallowed_methods))]
fn apply() {}
#[allow(clippy::too_many_arguments)]
fn wide() {}
"""
        self.assertEqual(ban_exceptions(source), [(1, None), (3, None), (6, "apply")])
        problems = stray_exceptions([("farhelm-gui/src/ticker.rs", source)])
        self.assertEqual(sum("outside its owners" in problem for problem in problems), 3)

    def test_a_manifest_switching_a_ban_off_for_the_whole_crate_is_reported(self):
        """A crate-level switch acts on every item at once and sits in a manifest the ownership check only reports, so
        it must fail the check like an item's. Specified: `[lints.clippy]` allowing or expecting a `disallowed_*` ban
        (dashed or not, plain or as a table with a level), clippy's `style` or `all` group, or `[lints.rust]` allowing
        `warnings`, is reported; denying them, or allowing an unrelated lint, is not."""
        manifest = tomllib.loads("""\
[lints.clippy]
unwrap_used = "deny"
disallowed_methods = "allow"
disallowed-types = { level = "expect", priority = 1 }
style = "allow"
too_many_arguments = "allow"
all = "allow"

[lints.rust]
warnings = "allow"
""")
        problems = manifest_switches("farhelm-gui/Cargo.toml", manifest, {})
        self.assertEqual(len(problems), 5, problems)
        for key in ("disallowed_methods", "disallowed-types", "style", "all", "warnings"):
            self.assertTrue(any(f"] {key} = " in problem for problem in problems), key)
        self.assertEqual(manifest_switches("x/Cargo.toml", tomllib.loads('[lints.clippy]\nall = "deny"\n'), {}), [])

    def test_a_crate_inheriting_the_workspaces_lints_is_checked_against_them(self):
        """A crate that says `lints.workspace = true` takes the root manifest's table, which could switch a ban off for
        every GUI crate at once. Specified: such a crate is reported for a switch in `[workspace.lints.clippy]`, and
        not for one that only a crate without the inheritance would see."""
        workspace = tomllib.loads('[workspace.lints.clippy]\ndisallowed_methods = "allow"\n')
        inheriting = tomllib.loads("[lints]\nworkspace = true\n")
        problems = manifest_switches("farhelm-gui/Cargo.toml", inheriting, workspace)
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("through the workspace's lints", problems[0])
        self.assertEqual(manifest_switches("farhelm-gui/Cargo.toml", {}, workspace), [])

    def test_a_second_clippy_configuration_under_the_gui_crates_is_reported(self):
        """Clippy reads the clippy.toml nearest a crate, so a second one under crates/gui would drop every shared ban for
        the crates under it, without any lint attribute. Specified: only the shared `clippy.toml` at crates/gui passes;
        a crate-local `clippy.toml` or `.clippy.toml` is reported."""
        self.assertEqual(stray_configs(["clippy.toml"]), [])
        problems = stray_configs(["clippy.toml", "farhelm-gui/clippy.toml", "farhelm-grid-model/.clippy.toml"])
        self.assertEqual(len(problems), 2, problems)


if __name__ == "__main__":
    unittest.main()
