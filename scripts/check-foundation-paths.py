#!/usr/bin/env python3
"""Refuse an area PR that changes a GPUI foundation's rule-carrying paths in a way no extension point allows.

The GPUI app's app-wide concerns each have one owner (SPEC_impl.md, "GUI architecture"), and
`docs/gpui/foundation-ownership.md` says which paths each owner holds. An area may add to a few of those paths without
asking, in the shapes the table's extension points describe, and may change the shared wiring among them (the main
view, the components, the bindings and the like) when the change follows the pattern already there; every other change
to an owned path is a foundation-change request to the orchestrator. This script is the landing step's mechanical half
of that rule. It reads the changed paths between two revisions and, for each one inside an owned path, decides whether
an extension point admits the change, and which admitted changes the review gate must look at.

The markdown file is the only source of the rules: the main table's second column gives each foundation's owned paths,
and the "Extension point rules" table gives each extension point's path and rule. Here's the TLDR of the three rules:

- `new-files` (on a directory, written with a trailing `/`): admits only files that the diff adds under it.
- `append-only` (on a file): admits a change to an existing file that adds at least one line and removes none, as git's
  numstat counts them. A changed line is a removal plus an addition, so it is refused; so are a binary change, a mode
  change on its own, and the file's deletion.
- `reported` (on a file or a directory): admits any change except a deletion, and prints it as `reported` rather than
  `allowed`. The script cannot tell wiring from a rule change in these files, so the review gate judges each reported
  touch: one that follows the pattern lands, one that changes a rule is still a request. A deletion is refused because
  the table may name the file (the next check would stop on it) and other areas build on it.

A rule admits a shape of change, not its content, so review still checks that an admitted change follows the pattern
the table describes. Renames are not detected (`--no-renames`): a rename is the deletion of its old path and the
addition of its new one, which is what the rules care about.

The rules are read from the base revision, never from the PR, so a PR cannot widen the rules it is judged by. For the
same reason the landing step runs the base's copy of this script, from stdin, which is why `--repo` has no default:

    git show <base>:scripts/check-foundation-paths.py | python3 -I - --repo <checkout> --base <base> --head <PR commit>

`--head` is required because in a jj workspace git's `HEAD` is the working copy's parent, which need not be the PR's
tip. Both revisions are resolved to commit ids once, and the report names them. The base must be an ancestor of the
head (rebase first): judging a stale PR by newer rules against its older tree would miss an edit to a foundation file
that main renamed since. Every owned and extension path must cover a file in the base tree, so a foundation file
renamed without updating the table stops the check instead of silently losing its owner.

`--owner` is for a foundation PR made by the owner: it reports what would have been refused and exits 0.

Exit status: 0 when nothing is refused (or under `--owner`), whatever was reported, 1 when something is refused, 2
when the check cannot judge (a git failure, a malformed table, or anything unexpected).
`scripts/test-check-foundation-paths.py` covers it against temporary git repositories.
"""
from __future__ import annotations

import argparse
import dataclasses
import pathlib
import re
import subprocess
import sys

TABLE = "docs/gpui/foundation-ownership.md"
RULE_NAMES = ("new-files", "append-only", "reported")
PREFIX = "check-foundation-paths"


class CannotRun(Exception):
    """The check cannot reach a verdict: git failed, or the table is malformed."""


# ---------------------------------------------------------------------------------------------------------------------
# The table
# ---------------------------------------------------------------------------------------------------------------------


@dataclasses.dataclass(frozen=True)
class Extension:
    """One extension point: a path inside its foundation's owned paths and the rule that admits changes to it."""

    path: str
    rule: str


@dataclasses.dataclass
class Foundation:
    """One row of the main table, with the extension points the rules table gives it."""

    name: str
    owned: list[str]
    extensions: list[Extension] = dataclasses.field(default_factory=list)


def covers(owned: str, path: str) -> bool:
    """Whether the owned path `owned` covers the repository path `path`: a directory (trailing `/`) covers everything
    under it, a file only itself."""
    return path.startswith(owned) if owned.endswith("/") else path == owned


def table_rows(text: str, header: tuple[str, str], width: int) -> list[list[str]]:
    """The body rows of the first markdown table whose header row's first two cells are `header`, as stripped cells.

    dprint keeps the tables one row per line with `|` at both ends, and no cell in the file contains a `|`, so a split
    on `|` is enough. Raises CannotRun when no such table exists, a row does not have `width` cells, or the table ends
    on anything but a blank line or the end of the file: a row split across lines, or written without its leading `|`,
    would otherwise end the table there and silently drop every row after it.
    """
    lines = text.splitlines()
    for index, line in enumerate(lines):
        if line.startswith("|") and [c.strip() for c in line.strip().strip("|").split("|")][:2] == list(header):
            rows = []
            for body in lines[index + 2 :]:
                if not body.startswith("|"):
                    if body.strip():
                        raise CannotRun(f"{TABLE}: the {header[1]} table ends on a line that is not a row: {body!r}")
                    break
                cells = [cell.strip() for cell in body.strip().strip("|").split("|")]
                if len(cells) != width:
                    raise CannotRun(f"{TABLE}: a row of the {header[1]} table has {len(cells)} cells, not {width}")
                rows.append(cells)
            return rows
    raise CannotRun(f"{TABLE} has no table headed | {' | '.join(header)} |")


CODE = re.compile(r"`([^`]+)`")
# How the main table writes an owned path: a backticked repository path. Other code spans in that column (`<U+XXXX>`)
# do not match; one that has a `/` but does not match is a malformed path, not an aside, and stops the check.
PATH_TOKEN = re.compile(r"^[\w.-]+(/[\w.-]+)*/?$")


def parse_table(markdown: str, tree: set[str]) -> list[Foundation]:
    """The foundations the markdown declares, validated against `tree`, the base revision's files. Raises CannotRun
    naming every problem found.

    Validation is what keeps a verdict unambiguous and current: every rule is one of the fixed set, a `new-files` rule
    sits on a directory and an `append-only` rule on a file, every extension point lies inside its own foundation's
    owned paths, no repository path is covered by two owned paths (so each changed path has at most one owner), and
    every owned or extension path covers at least one file that exists.
    """
    foundations = {}
    problems = []
    for name, owned_cell, *_ in table_rows(markdown, ("Foundation", "Owned paths"), 4):
        spans = CODE.findall(owned_cell)
        owned = [span for span in spans if PATH_TOKEN.match(span)]
        # A path the parser cannot read would otherwise be silently unowned.
        problems += [f"{name}: {span!r} is not a path" for span in spans if "/" in span and span not in owned]
        if "/" in CODE.sub("", owned_cell):
            problems.append(f"{name}: a path in the Owned paths column is not in backticks")
        if name in foundations:
            problems.append(f"{name}: foundation listed twice")
        elif not owned:
            problems.append(f"{name}: no owned paths")
        foundations[name] = Foundation(name, owned)
    for name, path_cell, rule_cell in table_rows(markdown, ("Foundation", "Extension path"), 3):
        path, rule = CODE.sub(r"\1", path_cell), CODE.sub(r"\1", rule_cell)
        foundation = foundations.get(name)
        if foundation is None:
            problems.append(f"{name}: {path}: extension point of a foundation the main table does not list")
        elif rule not in RULE_NAMES:
            problems.append(f"{name}: {path}: unknown rule {rule!r} (rules: {', '.join(RULE_NAMES)})")
        elif rule == "new-files" and not path.endswith("/"):
            problems.append(f"{name}: {path}: new-files needs a directory, written with a trailing /")
        elif rule == "append-only" and path.endswith("/"):
            problems.append(f"{name}: {path}: append-only needs a file, not a directory")
        elif not any(covers(owned, path) for owned in foundation.owned):
            problems.append(f"{name}: {path}: extension point outside the foundation's owned paths")
        else:
            foundation.extensions.append(Extension(path, rule))
    claims = [(path, f.name) for f in foundations.values() for path in f.owned]
    for i, (first, first_owner) in enumerate(claims):
        for second, second_owner in claims[i + 1 :]:
            if covers(first, second) or covers(second, first):
                problems.append(f"{first} ({first_owner}) and {second} ({second_owner}) overlap")
    paths = {(path, f.name) for f in foundations.values() for path in f.owned + [e.path for e in f.extensions]}
    for path, name in sorted(paths):
        if not any(covers(path, file) for file in tree):
            problems.append(f"{name}: {path} names no file in the base revision")
    if problems:
        raise CannotRun(f"{TABLE} is malformed:\n" + "\n".join(f"  {problem}" for problem in problems))
    return list(foundations.values())


# ---------------------------------------------------------------------------------------------------------------------
# The diff
# ---------------------------------------------------------------------------------------------------------------------


@dataclasses.dataclass(frozen=True)
class Change:
    """One changed path: git's status letter (A, M, D, T) and its line counts, `None` for a binary change."""

    path: str
    status: str
    added: int | None
    deleted: int | None


def git(repo: pathlib.Path, *args: str) -> str:
    """Run git in `repo` with an explicit argument list and return its stdout. Raises CannotRun on failure."""
    result = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)
    if result.returncode != 0:
        raise CannotRun(f"git {' '.join(args)} failed (exit {result.returncode}): {result.stderr.strip()}")
    return result.stdout


def resolve(repo: pathlib.Path, base: str, head: str) -> tuple[str, str]:
    """Both revisions as commit ids, resolved once, so that a ref moving during the run (another worktree's fetch)
    cannot make the rules, the tree and the diff come from different commits. Raises CannotRun unless the base is an
    ancestor of the head."""
    base, head = (git(repo, "rev-parse", "--verify", f"{rev}^{{commit}}").strip() for rev in (base, head))
    if subprocess.run(["git", "-C", str(repo), "merge-base", "--is-ancestor", base, head]).returncode != 0:
        raise CannotRun(f"the base {base} is not an ancestor of the head {head}; rebase the PR onto the base first")
    return base, head


def load(repo: pathlib.Path, base: str) -> list[Foundation]:
    """The foundations as the base revision's table declares them, checked against the base revision's tree."""
    tree = set(git(repo, "ls-tree", "-r", "--name-only", "-z", base).split("\0")) - {""}
    return parse_table(git(repo, "show", f"{base}:{TABLE}"), tree)


def changes(repo: pathlib.Path, base: str, head: str) -> list[Change]:
    """Every path the PR changes, from `git diff base..head`, in path order.

    NUL-separated output (`-z`) keeps every path byte-exact, whatever it contains; `--no-ext-diff` and `--no-textconv`
    keep a user's diff configuration from changing the counts.
    """
    options = ["-z", "--no-renames", "--no-ext-diff", "--no-textconv", f"{base}..{head}", "--"]
    fields = git(repo, "diff", "--name-status", *options).split("\0")
    statuses = dict(zip(fields[1::2], fields[0::2]))
    counts = {}
    for record in git(repo, "diff", "--numstat", *options).split("\0"):
        if record:
            added, deleted, path = record.split("\t", 2)
            counts[path] = (None, None) if added == "-" else (int(added), int(deleted))
    return [Change(path, status, *counts.get(path, (0, 0))) for path, status in sorted(statuses.items()) if path]


# ---------------------------------------------------------------------------------------------------------------------
# The verdict
# ---------------------------------------------------------------------------------------------------------------------

VERBS = {"A": "adds the file", "M": "modifies the file", "D": "deletes the file", "T": "changes the file's type"}


@dataclasses.dataclass(frozen=True)
class Verdict:
    """What the check says about one changed owned path: allowed or not, whether the review gate must judge it, and
    which rule decided.

    `reported` implies `allowed`: a reported touch never changes the exit status, it only puts the change in front of
    the review gate.
    """

    foundation: str
    path: str
    allowed: bool
    rule: str
    reason: str
    reported: bool = False

    def line(self, owner: bool) -> str:
        """The report line: `allowed`, `reported`, `refused`, or under `--owner` `would refuse`."""
        if self.allowed:
            word = "reported" if self.reported else "allowed"
        else:
            word = "would refuse" if owner else "refused"
        return f"{word}: {self.foundation}: {self.path}: {self.rule}: {self.reason}"


def judge(change: Change, foundations: list[Foundation]) -> Verdict | None:
    """The verdict on one changed path, or `None` when no foundation owns it (area code, docs, goldens and the like)."""
    # The first owner is the only one: `parse_table` refuses owned paths that overlap.
    owner = next((f for f in foundations if any(covers(owned, change.path) for owned in f.owned)), None)
    if owner is None:
        return None
    verb = VERBS.get(change.status, f"changes the file (git status {change.status})")
    # The most specific extension point decides: in the tests' fixture `src/widgets/mod.rs` is append-only although the
    # `src/widgets/` directory around it admits only new files, and a file path is always longer than a directory
    # covering it.
    matching = [extension for extension in owner.extensions if covers(extension.path, change.path)]
    if not matching:
        return Verdict(owner.name, change.path, False, "no extension point", verb)
    extension = max(matching, key=lambda extension: len(extension.path))
    if extension.rule == "reported":
        # A deletion (or a rename, which is a deletion here since renames are not detected) is a request: a file the
        # table names would stop the next check as a malformed table, and one under a reported directory (an existing
        # component, say) is wiring other areas build on.
        if change.status == "D":
            return Verdict(owner.name, change.path, False, extension.rule, verb)
        if change.status != "M":
            return Verdict(owner.name, change.path, True, extension.rule, verb, reported=True)
        counts = f"adds {change.added} lines and removes {change.deleted}"
        reason = "binary change" if change.deleted is None else counts
        return Verdict(owner.name, change.path, True, extension.rule, reason, reported=True)
    if extension.rule == "new-files":
        return Verdict(owner.name, change.path, change.status == "A", extension.rule, verb)
    # The table check guarantees an append-only file exists at the base, so the only whole-file change is a deletion.
    if change.status == "D":
        return Verdict(owner.name, change.path, False, extension.rule, verb)
    if change.deleted is None:
        return Verdict(owner.name, change.path, False, extension.rule, "binary change")
    reason = f"adds {change.added} lines and removes {change.deleted}"
    allowed = change.deleted == 0 and change.added > 0
    return Verdict(owner.name, change.path, allowed, extension.rule, reason)


def run(argv: list[str] | None = None) -> int:
    """Judge the diff between `--base` and `--head` and print one line per owned path it touches, then a summary.

    Every owned path touched gets a line, allowed ones included, so a landing log shows which touches an extension
    point covered, which the review gate must judge, and which need a request. Paths no foundation owns are not listed.
    """
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--base", default="origin/main", help="the revision the PR lands on (default: origin/main)")
    parser.add_argument("--head", required=True, help="the PR's commit")
    parser.add_argument("--owner", action="store_true", help="a foundation PR by its owner: report, never refuse")
    parser.add_argument("--repo", type=pathlib.Path, required=True, help="the checkout whose git holds both revisions")
    args = parser.parse_args(argv)
    try:
        base, head = resolve(args.repo, args.base, args.head)
        foundations = load(args.repo, base)
        verdicts = [v for change in changes(args.repo, base, head) if (v := judge(change, foundations))]
    except CannotRun as error:
        print(f"{PREFIX}: {error}", file=sys.stderr)
        return 2
    # Exit 1 means "refused", so nothing unexpected may end the run with Python's own exit 1 and a traceback.
    except Exception as error:  # noqa: BLE001
        print(f"{PREFIX}: cannot judge: {type(error).__name__}: {error}", file=sys.stderr)
        return 2
    for verdict in verdicts:
        print(verdict.line(args.owner))
    refused = sum(not verdict.allowed for verdict in verdicts)
    reported = sum(verdict.reported for verdict in verdicts)
    allowed = len(verdicts) - refused - reported
    summary = f"{head} against {base}: {len(verdicts)} owned paths touched, {allowed} allowed, {reported} reported"
    if args.owner:
        print(f"{PREFIX}: {summary}, {refused} would be refused without --owner")
        return 0
    print(f"{PREFIX}: {summary}, {refused} refused")
    if reported:
        print(
            f"{PREFIX}: the review gate checks that each reported change follows the pattern already there; one that "
            f"changes a rule is a foundation-change request (see {TABLE})"
        )
    if refused:
        print(
            f"{PREFIX}: request each refused change from the orchestrator (a foundation-change request in the area's "
            f"outbox); see {TABLE}"
        )
    return 1 if refused else 0


if __name__ == "__main__":
    sys.exit(run())
