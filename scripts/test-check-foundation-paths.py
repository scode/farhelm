"""Tests of check-foundation-paths.py against temporary git repositories that each test builds itself.

The check is the landing step's only mechanical guard on the GPUI foundations, and it fails open in every interesting
way: a rule that admits too much lets an area rewrite a foundation unasked, a PR that can edit the rules it is judged
by can grant itself anything, and a table naming a file that no longer exists leaves that file unowned. These cases run
the real script as a subprocess, against real `git diff` output, so path handling, exit codes and the report lines are
what a landing step sees.

Nothing here changes the test process's environment. Each git command gets its identity and the settings a user's
global configuration could otherwise change (signing, hooks, the default branch) as `-c` arguments.
"""
import importlib.util
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).with_name("check-foundation-paths.py")
REPO = SCRIPT.resolve().parent.parent
spec = importlib.util.spec_from_file_location("foundation_paths", SCRIPT)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)

GIT = [
    "git",
    "-c", "user.name=Test",
    "-c", "user.email=test@example.invalid",
    "-c", "commit.gpgsign=false",
    "-c", "core.hooksPath=/dev/null",
    "-c", "init.defaultBranch=main",
]

# The fixture's ownership, as (name, owned paths, extension points). Widgets has a new-files directory with an
# append-only file inside it, so the most specific extension point must decide; Theme has an append-only file; Focus
# owns a file with no extension point and a root-level file (no `/`) to check that the table parser finds it; Rules
# owns the table itself, as Enforcement does in the real one.
FIXTURE = [
    ("Widgets", ["src/widgets/"], [("src/widgets/", "new-files"), ("src/widgets/mod.rs", "append-only")]),
    ("Theme", ["src/theme.rs"], [("src/theme.rs", "append-only")]),
    ("Focus", ["src/focus.rs", "deny.toml"], []),
    ("Rules", ["docs/gpui/foundation-ownership.md"], []),
]
TABLE = "docs/gpui/foundation-ownership.md"


def table_md(foundations) -> str:
    """The markdown form of `foundations`, shaped like the real file: prose, the main table, then the rules table."""
    lines = ["# Ownership", "", "Some prose with a `code/span` that is not a table.", ""]
    lines += ["| Foundation | Owned paths | Extension points | Lint exceptions |", "| --- | --- | --- | --- |"]
    for name, owned, _ in foundations:
        lines.append(f"| {name} | {', '.join(f'`{p}`' for p in owned)} (with a `<U+XXXX>` aside) | prose | none |")
    lines += ["", "## Rules", "", "| Foundation | Extension path | Rule |", "| --- | --- | --- |"]
    for name, _, extensions in foundations:
        lines += [f"| {name} | `{path}` | `{rule}` |" for path, rule in extensions]
    return "\n".join(lines) + "\n"


class Repo:
    """A throwaway git repository whose `base` commit holds the fixture's table and every file it names."""

    def __init__(self, root: pathlib.Path):
        self.root = root
        self.git("init", "-q")
        self.write("README", "unowned\n")
        self.write("src/widgets/mod.rs", "mod button;\n")
        self.write("src/widgets/button.rs", "fn button() {}\n")
        self.write("src/theme.rs", "const A: u8 = 1;\nconst B: u8 = 2;\n")
        self.write("src/focus.rs", "fn focus() {}\n")
        self.write("src/area.rs", "fn area() {}\n")
        self.write("deny.toml", "")
        self.write(TABLE, table_md(FIXTURE))
        self.commit("base")
        self.git("tag", "base")

    def git(self, *args: str) -> str:
        return subprocess.run([*GIT, "-C", str(self.root), *args], check=True, capture_output=True, text=True).stdout

    def write(self, path: str, text: str | bytes):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(text, bytes):
            target.write_bytes(text)
        else:
            target.write_text(text)

    def commit(self, message: str):
        self.git("add", "-A")
        self.git("commit", "-q", "--allow-empty", "--no-verify", "-m", message)

    def check(self, *args: str) -> subprocess.CompletedProcess:
        """Run the script on `base...HEAD` of this repository."""
        return subprocess.run(
            [sys.executable, "-B", str(SCRIPT), "--repo", str(self.root), "--base", "base", "--head", "HEAD", *args],
            capture_output=True,
            text=True,
        )


class FoundationPathsTest(unittest.TestCase):
    """An area PR may change owned paths only in the shapes the extension points admit."""

    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.repo = Repo(pathlib.Path(directory.name))

    def assertVerdict(self, result, *lines, code):
        """The run exited with `code` and printed every one of `lines`."""
        self.assertEqual(result.returncode, code, result.stdout + result.stderr)
        for line in lines:
            self.assertIn(line, result.stdout.splitlines())

    def test_a_new_file_under_a_new_files_directory_is_allowed(self):
        """The component-constructor case: a new control in its own file needs no request."""
        self.repo.write("src/widgets/slider.rs", "fn slider() {}\n")
        self.repo.commit("add a widget")
        self.assertVerdict(
            self.repo.check(), "allowed: Widgets: src/widgets/slider.rs: new-files: adds the file", code=0
        )

    def test_changing_an_existing_file_under_a_new_files_directory_is_refused(self):
        """new-files admits additions only: editing a control that is already there is a foundation change."""
        self.repo.write("src/widgets/button.rs", "fn button() { changed() }\n")
        self.repo.commit("edit a widget")
        self.assertVerdict(
            self.repo.check(), "refused: Widgets: src/widgets/button.rs: new-files: modifies the file", code=1
        )

    def test_a_pure_addition_to_an_append_only_file_is_allowed(self):
        """The declare-a-new-action case: lines added, none removed, anywhere in the file."""
        self.repo.write("src/theme.rs", "const A: u8 = 1;\nconst NEW: u8 = 9;\nconst B: u8 = 2;\n")
        self.repo.commit("add a token")
        self.assertVerdict(
            self.repo.check(), "allowed: Theme: src/theme.rs: append-only: adds 1 lines and removes 0", code=0
        )

    def test_an_append_only_change_that_deletes_a_line_is_refused(self):
        """A changed line is a removal plus an addition, so rewriting an existing token is refused, naming the rule."""
        self.repo.write("src/theme.rs", "const A: u8 = 7;\nconst B: u8 = 2;\n")
        self.repo.commit("change a token")
        self.assertVerdict(
            self.repo.check(), "refused: Theme: src/theme.rs: append-only: adds 1 lines and removes 1", code=1
        )

    def test_the_most_specific_extension_point_decides(self):
        """mod.rs sits in a new-files directory but is append-only itself: a pure addition to it is allowed, and a
        removal from it is refused by append-only, not by new-files."""
        self.repo.write("src/widgets/mod.rs", "mod button;\nmod slider;\n")
        self.repo.write("src/widgets/slider.rs", "fn slider() {}\n")
        self.repo.commit("register a widget")
        self.assertVerdict(
            self.repo.check(), "allowed: Widgets: src/widgets/mod.rs: append-only: adds 1 lines and removes 0", code=0
        )
        self.repo.write("src/widgets/mod.rs", "mod slider;\n")
        self.repo.commit("drop a widget")
        self.assertVerdict(
            self.repo.check(), "refused: Widgets: src/widgets/mod.rs: append-only: adds 1 lines and removes 1", code=1
        )

    def test_an_owned_file_without_an_extension_point_is_refused(self):
        """Any change to a file with no extension point needs a request, even a pure addition."""
        self.repo.write("src/focus.rs", "fn focus() {}\nfn more() {}\n")
        self.repo.commit("extend focus")
        self.assertVerdict(
            self.repo.check(), "refused: Focus: src/focus.rs: no extension point: modifies the file", code=1
        )

    def test_unowned_paths_are_not_judged(self):
        """Area code is the area's own: changing it prints no verdict and passes."""
        self.repo.write("src/area.rs", "fn area() { rewritten() }\n")
        self.repo.commit("area work")
        result = self.repo.check()
        self.assertVerdict(result, code=0)
        self.assertIn(": 0 owned paths touched, 0 allowed, 0 refused", result.stdout)

    def test_owner_reports_without_refusing(self):
        """A foundation PR by its owner sees what an area PR would have been refused, and passes."""
        self.repo.write("src/focus.rs", "fn focus2() {}\n")
        self.repo.commit("owner change")
        line = "would refuse: Focus: src/focus.rs: no extension point: modifies the file"
        self.assertVerdict(self.repo.check("--owner"), line, code=0)

    def test_renames_count_as_a_deletion_and_an_addition(self):
        """Moving a control out of the owned directory deletes an owned file (refused); moving an area file into a
        new-files directory adds one there (allowed)."""
        self.repo.git("mv", "src/widgets/button.rs", "src/button.rs")
        self.repo.git("mv", "src/area.rs", "src/widgets/area.rs")
        self.repo.commit("move files")
        self.assertVerdict(
            self.repo.check(),
            "refused: Widgets: src/widgets/button.rs: new-files: deletes the file",
            "allowed: Widgets: src/widgets/area.rs: new-files: adds the file",
            code=1,
        )

    def test_deleting_an_append_only_file_is_refused(self):
        """Removing every line is still removing lines; the deletion is named as such."""
        self.repo.git("rm", "-q", "src/theme.rs")
        self.repo.commit("delete theme")
        self.assertVerdict(self.repo.check(), "refused: Theme: src/theme.rs: append-only: deletes the file", code=1)

    def test_a_binary_change_to_an_append_only_file_is_refused(self):
        """git counts no lines for a binary file, so append-only cannot be shown and the change is refused."""
        self.repo.write("src/theme.rs", b"const A: u8 = 1;\nconst B: u8 = 2;\n\x00binary\n")
        self.repo.commit("binary theme")
        self.assertVerdict(self.repo.check(), "refused: Theme: src/theme.rs: append-only: binary change", code=1)

    def test_a_pr_not_rebased_onto_the_base_stops_the_check(self):
        """The rules come from the base and the diff from the PR; when main has moved on since the PR branched (here,
        renaming the file the PR edits), the two describe different trees, and an edit to a foundation file that main
        renamed would pass as unowned. So a base that is not an ancestor of the head exits 2, asking for a rebase."""
        self.repo.git("checkout", "-q", "-b", "pr")
        self.repo.write("src/focus.rs", "fn focus_in_pr() {}\n")
        self.repo.commit("pr")
        self.repo.git("checkout", "-q", "main")
        self.repo.write("src/area.rs", "fn area_on_main() {}\n")
        self.repo.commit("main moves on")
        self.repo.git("tag", "-f", "base")
        self.repo.git("checkout", "-q", "pr")
        result = self.repo.check()
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("rebase the PR onto the base first", result.stderr)

    def test_the_bases_copy_runs_from_stdin(self):
        """The landing step pipes the base's copy of the script into Python, so that a PR cannot change the script
        that judges it. Specified: the documented command judges a PR that rewrites the script to always pass, and
        refuses it."""
        self.repo.write("scripts/check.py", "")
        self.repo.write(TABLE, table_md([*FIXTURE[:3], ("Rules", [TABLE, "scripts/check.py"], [])]))
        self.repo.commit("own the script")
        self.repo.git("tag", "-f", "base")
        self.repo.write("scripts/check.py", "import sys\nsys.exit(0)\n")
        self.repo.commit("make the check pass")
        result = subprocess.run(
            [sys.executable, "-I", "-", "--repo", str(self.repo.root), "--base", "base", "--head", "HEAD"],
            input=SCRIPT.read_text(), capture_output=True, text=True,
        )
        self.assertVerdict(result, "refused: Rules: scripts/check.py: no extension point: modifies the file", code=1)

    def test_an_unexpected_failure_exits_2_without_a_traceback(self):
        """Exit 1 means refused, so nothing else may end the run with Python's own exit 1. A file name that is not
        UTF-8 makes reading git's output fail; the check stops with exit 2 and a one-line reason."""
        (self.repo.root / os.fsdecode(b"latin1-\xe9.txt")).write_bytes(b"x\n")
        self.repo.commit("a non-UTF-8 name")
        result = self.repo.check()
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("cannot judge: UnicodeDecodeError", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_a_pr_cannot_widen_the_rules_it_is_judged_by(self):
        """The table is read from the base revision. A PR that adds rows granting itself append-only on an owned file
        and on the table, then appends to both, is judged by the base's rules and refused: every one of its changes is
        a pure addition, so judging it by its own table would let it through."""
        widened = [*FIXTURE[:2], ("Focus", ["src/focus.rs", "deny.toml"], [("src/focus.rs", "append-only")])]
        widened.append(("Rules", [TABLE], [(TABLE, "append-only")]))
        self.repo.write(TABLE, table_md(widened))
        self.repo.write("src/focus.rs", "fn focus() {}\nfn area_was_here() {}\n")
        self.repo.commit("widen the rules")
        self.assertVerdict(
            self.repo.check(),
            "refused: Focus: src/focus.rs: no extension point: modifies the file",
            f"refused: Rules: {TABLE}: no extension point: modifies the file",
            code=1,
        )

    def test_a_mode_change_alone_is_not_an_addition(self):
        """append-only needs at least one added line, so a chmod on an append-only file is refused."""
        (self.repo.root / "src/theme.rs").chmod(0o755)
        self.repo.commit("make theme executable")
        self.assertVerdict(
            self.repo.check(), "refused: Theme: src/theme.rs: append-only: adds 0 lines and removes 0", code=1
        )

    def test_an_owned_path_missing_from_the_base_tree_stops_the_check(self):
        """A foundation file renamed without updating the table would silently lose its owner, so a table naming a
        file the base revision does not have is malformed: exit 2, naming the path, and no traceback."""
        self.repo.git("mv", "src/focus.rs", "src/focus2.rs")
        self.repo.commit("rename focus, forget the table")
        self.repo.git("tag", "-f", "base")
        self.repo.write("src/focus2.rs", "fn rewritten() {}\n")
        self.repo.commit("area rewrites the renamed file")
        result = self.repo.check()
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("Focus: src/focus.rs names no file in the base revision", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_a_malformed_table_stops_the_check(self):
        """A table the check cannot read exits 2, never 1, which would read as a refusal."""
        self.repo.write(TABLE, table_md(FIXTURE).replace("| Theme | `src/theme.rs` | `append-only` |", "| Theme |"))
        self.repo.commit("break the table")
        self.repo.git("tag", "-f", "base")
        result = self.repo.check()
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("has 1 cells, not 3", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_a_git_failure_stops_the_check(self):
        """An unknown revision is not an empty diff: the check cannot run, and says so."""
        result = subprocess.run(
            [sys.executable, "-B", str(SCRIPT), "--repo", str(self.repo.root), "--base", "no-such-rev", "--head", "main"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("no-such-rev", result.stderr)


class TableTest(unittest.TestCase):
    """The table's own validation, which keeps every verdict down to one owner and one rule."""

    def problems(self, foundations, tree) -> str:
        with self.assertRaises(checker.CannotRun) as raised:
            checker.parse_table(table_md(foundations), set(tree))
        return str(raised.exception)

    def test_rules_must_fit_their_path(self):
        """new-files needs a directory, append-only a file, and the rule set is fixed."""
        text = self.problems(
            [("A", ["a/"], [("a/x.rs", "new-files"), ("a/", "append-only"), ("a/y.rs", "anything")])], ["a/x.rs"]
        )
        self.assertIn("a/x.rs: new-files needs a directory", text)
        self.assertIn("a/: append-only needs a file", text)
        self.assertIn("unknown rule 'anything'", text)

    def test_extension_points_lie_inside_their_foundation(self):
        """A foundation cannot open someone else's path as its extension point."""
        text = self.problems([("A", ["a.rs"], [("b.rs", "append-only")]), ("B", ["b.rs"], [])], ["a.rs", "b.rs"])
        self.assertIn("A: b.rs: extension point outside the foundation's owned paths", text)

    def test_owned_paths_never_overlap(self):
        """A file claimed by two foundations, or covered by another's directory, would have two owners."""
        text = self.problems([("A", ["src/"], []), ("B", ["src/b.rs"], [])], ["src/b.rs"])
        self.assertIn("src/ (A) and src/b.rs (B) overlap", text)

    def test_a_foundation_listed_twice_is_malformed(self):
        """A second row with the same name would silently replace the first row's paths."""
        text = self.problems([("A", ["a.rs"], []), ("A", ["b.rs"], [])], ["a.rs", "b.rs"])
        self.assertIn("A: foundation listed twice", text)

    def test_a_rule_for_a_foundation_the_main_table_lacks_is_malformed(self):
        """A rules row whose foundation is missing from the main table means the two tables disagree, often because
        the main table lost rows."""
        markdown = table_md([("A", ["a.rs"], [])]) + "| B | `b.rs` | `append-only` |\n"
        with self.assertRaises(checker.CannotRun) as raised:
            checker.parse_table(markdown, {"a.rs", "b.rs"})
        self.assertIn("B: b.rs: extension point of a foundation the main table does not list", str(raised.exception))

    def test_a_row_that_does_not_read_as_a_row_is_malformed(self):
        """A row split across lines, or written without its leading `|` (valid in GitHub's markdown), would end the
        table there and silently drop every row after it, so the table may end only on a blank line."""
        split = table_md(FIXTURE).replace("| Theme | `src/theme.rs` (with", "| Theme |\n`src/theme.rs` (with")
        unpiped = table_md(FIXTURE).replace("| Theme | `src/theme.rs` (with", "Theme | `src/theme.rs` (with")
        for markdown, problem in [(split, "has 1 cells, not 4"), (unpiped, "ends on a line that is not a row")]:
            with self.assertRaises(checker.CannotRun) as raised:
                checker.parse_table(markdown, set())
            self.assertIn(problem, str(raised.exception))

    def test_unreadable_paths_are_malformed(self):
        """A path with a space inside its backticks, or written without backticks, would silently be unowned."""
        text = self.problems([("A", ["a.rs", "x/y z.rs"], [])], ["a.rs"])
        self.assertIn("A: 'x/y z.rs' is not a path", text)
        markdown = table_md([("A", ["a.rs"], [])]).replace("`a.rs` (with", "`a.rs`, scripts/b.py (with")
        with self.assertRaises(checker.CannotRun) as raised:
            checker.parse_table(markdown, {"a.rs"})
        self.assertIn("A: a path in the Owned paths column is not in backticks", str(raised.exception))

    def test_root_level_files_and_asides_parse(self):
        """`deny.toml` has no `/` but is a path; `<U+XXXX>` in the same column is not."""
        foundations = checker.parse_table(table_md(FIXTURE), {"src/widgets/mod.rs", "src/theme.rs", "src/focus.rs",
                                                              "deny.toml", TABLE})
        self.assertEqual(foundations[2].owned, ["src/focus.rs", "deny.toml"])

    def test_the_checked_in_table_is_valid(self):
        """The repository's own table parses, and every path it names exists in the checkout, so a bad edit fails
        here before a landing step."""
        files = subprocess.run(
            ["git", "-C", str(REPO), "ls-files", "--cached"],
            check=True, capture_output=True, text=True,
        ).stdout.splitlines()
        foundations = checker.parse_table((REPO / TABLE).read_text(), set(files))
        self.assertIn("Operation store", [foundation.name for foundation in foundations])


if __name__ == "__main__":
    unittest.main()
