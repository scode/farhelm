#!/usr/bin/env python3
"""Self-test for scripts/gui-e2e-smoke.py's check that every step sits directly in its `Smoke` class, under a name and
a number no other step uses.

The smoke's steps are only run when unittest finds them as methods of `Smoke`, and one written anywhere else is skipped
without a word. A jj automatic merge once moved two steps into a module-level function that way, so the smoke refuses
to start while any `test_` function is out of place. Areas merging their steps also meet under one number (they would
run in an order neither chose) or one name (Python keeps only the last), so it refuses those too. These tests pin which
shapes are refused and how each is reported, that the committed script has none, and that `main` refuses in its own
source before it starts anything. Nothing here starts the app.
"""

from __future__ import annotations

import importlib.util
import io
import re
import tempfile
import textwrap
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "gui-e2e-smoke.py"
spec = importlib.util.spec_from_file_location("smoke", SCRIPT)
smoke = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(smoke)

# A well-formed `Smoke` as the cases below start from: two steps, a helper method, a step with a nested helper of its
# own, and a module-level helper after the class, all of which are where they belong.
CLEAN = """
class Smoke(unittest.TestCase):
    def ok(self):
        pass

    def test_01_first(self):
        def inner():
            pass

    async def test_02_second(self):
        pass


def helper():
    pass
"""


def lines_of(text: str, fragment: str) -> list[int]:
    """The 1-based numbers of every line in `text` holding `fragment`, in order: where `step_refusals` must say steps
    are, worked out from the text itself so a case's expected lines cannot drift from its snippet."""
    return [at for at, line in enumerate(text.splitlines(), start=1) if fragment in line]


def line_of(text: str, fragment: str) -> int:
    """`lines_of` for a fragment that must be on exactly one line."""
    found = lines_of(text, fragment)
    assert len(found) == 1, f"{fragment!r} is on {len(found)} lines of the snippet"
    return found[0]


def stray(text: str, name: str) -> str:
    """The refusal `step_refusals` must give for the one stray step `name` in `text`, at its line in that text."""
    return f"{name} (line {line_of(text, f'def {name}(')}) is not a method of Smoke, so it would never run"


class StraySteps(unittest.TestCase):
    """`step_refusals` against small sources: only a direct method of the module-level `Smoke` is in place, and every
    other `test_` function is reported by name and line."""

    def test_a_clean_smoke_class_has_no_stray_steps(self) -> None:
        """The check must not refuse the shape every area writes its steps in, async steps and helpers included."""
        self.assertEqual(smoke.step_refusals(CLEAN), [])

    def test_a_step_inside_a_module_level_function_is_reported(self) -> None:
        """The shape the merge left behind: a helper function after the class swallowing the steps written below it.
        Specified: each swallowed step is reported with its own line, in line order, and the helper is not."""
        text = CLEAN + textwrap.dedent(
            """
            def waiting():
                return []

                def test_52_swallowed(self):
                    pass

                def test_53_swallowed_too(self):
                    pass
            """
        )
        expected = [stray(text, name) for name in ("test_52_swallowed", "test_53_swallowed_too")]
        self.assertEqual(smoke.step_refusals(text), expected)

    def test_a_step_at_module_level_is_reported(self) -> None:
        """A step dedented out of the class, plain or async, is never collected either. Specified: both are reported
        with their own lines, in line order."""
        text = CLEAN + "\ndef test_70_loose():\n    pass\n\nasync def test_71_loose_async():\n    pass\n"
        expected = [stray(text, name) for name in ("test_70_loose", "test_71_loose_async")]
        self.assertEqual(smoke.step_refusals(text), expected)

    def test_a_step_in_another_class_is_reported(self) -> None:
        """Only `Smoke` is loaded, so a step in any other class, a second TestCase included, would never run.
        Specified: it is reported with its line, and the other class's name is not mistaken for `Smoke`."""
        text = CLEAN + "\nclass Other(unittest.TestCase):\n    def test_80_elsewhere(self):\n        pass\n"
        self.assertEqual(smoke.step_refusals(text), [stray(text, "test_80_elsewhere")])

    def test_the_committed_script_passes_the_check(self) -> None:
        """The real smoke must pass its own check, and passing it must mean every step the file spells out runs.
        Specified: no refusals (no stray step, no shared name or number); unittest collects from `Smoke` exactly as
        many steps as the file has `def test_` lines (counted as text, not through the AST the check uses); and steps 52
        and 53, the two the merge hid, are among them."""
        text = SCRIPT.read_text(encoding="utf-8")
        self.assertEqual(smoke.step_refusals(text), [])
        written = len(re.findall(r"^\s*(?:async\s+)?def test_", text, re.MULTILINE))
        self.assertEqual(len(unittest.defaultTestLoader.getTestCaseNames(smoke.Smoke)), written)
        self.assertIn("test_52_welcome_reopens_from_the_help_menu_and_escape_returns_to_the_button", vars(smoke.Smoke))
        self.assertIn("test_53_a_window_start_opens_the_card_only_when_the_run_asks_for_it", vars(smoke.Smoke))


class StepClashes(unittest.TestCase):
    """`step_refusals` against `Smoke` classes whose steps are all in place but share a number or a name: each clash is
    one refusal naming every definition involved, and steps that only look alike pass."""

    def test_distinct_numbers_pass(self) -> None:
        """Numbers that only look alike (06 and 60, 60 and 61) are not a clash, nor is a helper method beside them.
        Specified: no refusals."""
        text = textwrap.dedent(
            """
            class Smoke(unittest.TestCase):
                def test_06_a(self):
                    pass

                def test_60_b(self):
                    pass

                def helper(self):
                    pass

                def test_61_c(self):
                    pass
            """
        )
        self.assertEqual(smoke.step_refusals(text), [])

    def test_two_steps_sharing_a_number_are_reported_with_their_lines(self) -> None:
        """Two areas' steps meeting under one number in a merge both run, but in an order neither area chose.
        Specified: one refusal naming the number and each step with its own line, in class order, and none for the
        step numbered apart."""
        text = textwrap.dedent(
            """
            class Smoke(unittest.TestCase):
                def test_61_settings_area(self):
                    pass

                def test_60_between(self):
                    pass

                def test_61_approvals_area(self):
                    pass
            """
        )
        settings = line_of(text, "def test_61_settings_area(")
        approvals = line_of(text, "def test_61_approvals_area(")
        expected = [
            "step 61 is used by more than one method of Smoke: "
            f"test_61_settings_area (line {settings}), test_61_approvals_area (line {approvals})"
        ]
        self.assertEqual(smoke.step_refusals(text), expected)

    def test_a_step_defined_twice_is_reported_once_as_a_name_clash(self) -> None:
        """Python keeps only the last of two methods with one name, so the first step is gone before unittest looks,
        and nothing the class ends up with shows it. Specified: one refusal naming the step and both lines; the
        shared number is not reported as a second clash, and the async definition counts like the plain one."""
        text = textwrap.dedent(
            """
            class Smoke(unittest.TestCase):
                def test_52_welcome(self):
                    pass

                async def test_52_welcome(self):
                    pass
            """
        )
        first, second = lines_of(text, "def test_52_welcome(")
        expected = [
            f"test_52_welcome is defined in Smoke more than once (lines {first}, {second}), so only the last would run"
        ]
        self.assertEqual(smoke.step_refusals(text), expected)

    def test_a_name_clash_inside_a_number_clash_reports_both(self) -> None:
        """A step defined twice can also share its number with a different step; both problems need fixing. Specified:
        the name clash, then the number clash listing all three definitions with their lines."""
        text = textwrap.dedent(
            """
            class Smoke(unittest.TestCase):
                def test_61_twice(self):
                    pass

                def test_61_other(self):
                    pass

                def test_61_twice(self):
                    pass
            """
        )
        first, second = lines_of(text, "def test_61_twice(")
        other = line_of(text, "def test_61_other(")
        expected = [
            f"test_61_twice is defined in Smoke more than once (lines {first}, {second}), so only the last would run",
            "step 61 is used by more than one method of Smoke: "
            f"test_61_twice (line {first}), test_61_other (line {other}), test_61_twice (line {second})",
        ]
        self.assertEqual(smoke.step_refusals(text), expected)


class MainRefuses(unittest.TestCase):
    """`main` with a stray step in its own source: it says which and exits non-zero before it makes a run directory or
    starts the app."""

    def test_main_refuses_its_own_source_before_starting_anything(self) -> None:
        """The refusal is only worth having if `main` checks the file it is running from, and does so before the app
        run, whose steps would otherwise pass without the stray one. The module's `__file__`, which `main` reads its
        source through, is pointed at a temporary file holding a clean class and a stray step, so a `main` that checked
        anything else would start past it. Any subprocess or run directory made afterwards fails the test rather than
        starting the real app. Specified: exit 1, and one stderr line naming the stray step and its line."""
        text = CLEAN + "\ndef test_90_stray():\n    pass\n"
        forbidden = mock.Mock(side_effect=AssertionError("main went on past a stray step"))
        stderr = io.StringIO()
        with tempfile.TemporaryDirectory() as root:
            fake = Path(root) / "gui-e2e-smoke.py"
            fake.write_text(text, encoding="utf-8")
            with (
                mock.patch.object(smoke, "__file__", str(fake)),
                mock.patch.object(smoke.subprocess, "run", forbidden),
                mock.patch.object(smoke.tempfile, "mkdtemp", forbidden),
                redirect_stderr(stderr),
            ):
                self.assertEqual(smoke.main([]), 1)
        line = line_of(text, "def test_90_stray(")
        self.assertEqual(
            stderr.getvalue().splitlines(),
            [f"gui-e2e-smoke: test_90_stray (line {line}) is not a method of Smoke, so it would never run"],
        )


if __name__ == "__main__":
    unittest.main()
