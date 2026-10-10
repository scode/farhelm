#!/usr/bin/env python3
"""Tests for the plans queue script: its grammar, transitions, invariants, and the compare-and-swap loop.

Several executors rely on this script as the only thing that keeps two of them from building the same plan, and on
its invariants to keep a planning PR or a bad splice from corrupting the queue. These tests run without the network:
`FakeGitHub` is a small in-memory stand-in for the Git Data API that enforces the one property the lock depends on,
that a non-forced ref update succeeds only when the new commit's parent is the branch's current head. Hooks on it
inject the concurrent writers and lost responses that cannot be produced on demand against the real service.

The formatter is stubbed except in one round trip that runs real dprint when it is on PATH, because nothing else
proves that dprint's output still parses the way the script expects.
"""

from __future__ import annotations

import datetime
import hashlib
import importlib.util
import io
import json
import pathlib
import shutil
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout

SCRIPT = pathlib.Path(__file__).with_name("plans-queue.py").resolve()
REPO_ROOT = SCRIPT.parent.parent


def load_queue():
    """Import plans-queue.py, whose hyphenated name rules out a plain import."""
    spec = importlib.util.spec_from_file_location("plans_queue", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    sys.modules["plans_queue"] = module
    spec.loader.exec_module(module)
    return module


pq = load_queue()

HEADER = "# Plans queue\n\nOne line per plan.\n\n"
DPRINT_JSON = '{"markdown": {"lineWidth": 120, "textWrap": "always"}}\n'


def index(*lines: str) -> str:
    return HEADER + "".join(line + "\n" for line in lines)


def plan(title: str = "A plan") -> str:
    return f"# {title}\n\nSome goal.\n\n## Done criterion\n\nDone when done.\n"


def line(state: str, slug: str, summary: str = "does a thing", deps: tuple[str, ...] = ()) -> str:
    text = f"- [{state}] `{slug}.md` — {summary}"
    if deps:
        text += " (after " + ", ".join(f"`{d}.md`" for d in deps) + ")"
    return text


def reports(*lines: str) -> str:
    return pq.REPORTS_HEADER + "".join(line + "\n" for line in lines)


def landed(slug: str, prs: tuple[int, ...] = (1,), summary: str = "does a thing", date: str = "2026-10-04") -> str:
    """A REPORTS.md line, written out by hand so the tests pin the grammar rather than echo the script's renderer."""
    numbers = ", ".join(f"#{n}" for n in prs)
    return f"- [`{slug}`](reports/{slug}.report.md) landed {date} in {numbers}: {summary}"


# --- A fake GitHub -------------------------------------------------------------------------------------------------


class FakeGitHub:
    """An in-memory repository with just enough of the Git Data API for the script.

    Commits hold whole file maps. Tree and blob ids are derived from content so identical files share ids, which is
    what `wake_check` compares. `update_ref` refuses anything but a fast-forward from the current head, exactly the
    property the script's lock rests on.

    Hooks:
      before_update(fake, branch): runs just before each ref update decision, to land a concurrent commit.
      update_failure: an exception to raise from update_ref instead of updating (after applying it, if
        `update_failure_lands` is set, to model a response lost on the way back).
    """

    def __init__(self, files: dict[str, str]):
        self.commits: dict[str, dict] = {}
        self.trees: dict[str, dict[str, str]] = {}
        self.blobs: dict[str, str] = {}
        self.refs: dict[str, str] = {}
        self.pulls: list[dict] = []
        self.closed: dict[int, dict] = {}
        self.stale_heads: list[str] = []
        self.counter = 0
        self.updates = 0
        self.before_update = None
        self.update_failure: Exception | None = None
        self.update_failure_lands = False
        root = self._commit(dict(files), None, "initial")
        self.refs["main"] = root

    # Construction helpers used by tests as well as by the fake itself.

    def _id(self, prefix: str, payload: str) -> str:
        return hashlib.sha1((prefix + payload).encode()).hexdigest()

    def _commit(self, files: dict[str, str], parent: str | None, message: str) -> str:
        self.counter += 1
        sha = self._id("commit", f"{self.counter}")
        self.commits[sha] = {"files": files, "parent": parent, "message": message, "date": "2026-10-02T10:00:00Z"}
        return sha

    def land(self, branch: str, change: dict[str, str | None], message: str = "concurrent") -> str:
        """Commit directly on a branch, as another writer would."""
        files = dict(self.commits[self.refs[branch]]["files"])
        for path, content in change.items():
            if content is None:
                files.pop(path, None)
            else:
                files[path] = content
        sha = self._commit(files, self.refs[branch], message)
        self.refs[branch] = sha
        return sha

    def files(self, branch: str = "main") -> dict[str, str]:
        return self.commits[self.refs[branch]]["files"]

    # The GitHub protocol.

    def head(self, branch: str) -> str:
        # A queued stale value models a read served from a replica that has not seen the latest update yet.
        if self.stale_heads:
            return self.stale_heads.pop(0)
        return self.refs[branch]

    def commit(self, sha: str) -> dict:
        return {"tree": {"sha": "root:" + sha}, "message": self.commits[sha]["message"]}

    def _blob_id(self, content: str) -> str:
        sha = self._id("blob", content)
        self.blobs[sha] = content
        return sha

    def tree(self, sha: str, recursive: bool = False) -> list[dict]:
        kind, commit = sha.split(":", 1)
        files = self.commits[commit]["files"] if commit in self.commits else self.trees[commit]
        if kind == "root":
            entries = []
            plans = {p: c for p, c in files.items() if p.startswith("plans/")}
            if plans:
                entries.append({"path": "plans", "type": "tree", "sha": "plans:" + commit})
            for path, content in files.items():
                if "/" not in path:
                    entries.append({"path": path, "type": "blob", "sha": self._blob_id(content)})
            return entries
        assert kind == "plans" and recursive
        return [
            {"path": path[len("plans/") :], "type": "blob", "sha": self._blob_id(content)}
            for path, content in sorted(files.items())
            if path.startswith("plans/")
        ]

    def blob(self, sha: str) -> str:
        return self.blobs[sha]

    def create_tree(self, base_tree: str, entries: list[dict]) -> str:
        _, commit = base_tree.split(":", 1)
        files = dict(self.commits[commit]["files"])
        for entry in entries:
            if entry.get("sha", "x") is None:
                files.pop(entry["path"], None)
            else:
                files[entry["path"]] = entry["content"]
        tree_id = self._id("tree", json.dumps(files, sort_keys=True) + str(self.counter))
        self.trees[tree_id] = files
        return "root:" + tree_id

    def create_commit(self, message: str, tree: str, parent: str) -> str:
        _, tree_id = tree.split(":", 1)
        return self._commit(dict(self.trees[tree_id]), parent, message)

    def update_ref(self, branch: str, sha: str) -> None:
        self.updates += 1
        if self.before_update:
            self.before_update(self, branch)
        if self.update_failure is not None:
            failure = self.update_failure
            if self.update_failure_lands and self.commits[sha]["parent"] == self.refs[branch]:
                self.refs[branch] = sha
            raise failure
        if self.commits[sha]["parent"] != self.refs[branch]:
            raise pq.NotFastForward("Update is not a fast forward")
        self.refs[branch] = sha

    def is_ancestor_or_same(self, ancestor: str, descendant: str) -> bool:
        node: str | None = descendant
        while node is not None:
            if node == ancestor:
                return True
            node = self.commits[node]["parent"]
        return False

    def open_pulls(self) -> list[dict]:
        return list(self.pulls)

    def pull(self, number: int) -> dict:
        for item in self.pulls:
            if item["number"] == number:
                return {"number": number, "ref": item["ref"], "merged": False}
        return {"number": number, **self.closed[number]}

    def merged_pulls(self, prefix: str) -> list[dict]:
        return [
            {"number": n, **info}
            for n, info in sorted(self.closed.items())
            if info["merged"] and info["ref"].startswith(prefix)
        ]

    def path_history(self, branch: str, path: str, limit: int) -> list[dict]:
        """Commits that changed `path`, newest first. Like GitHub's, a directory path matches any file under it."""

        def under(files: dict[str, str]) -> dict[str, str]:
            return {p: c for p, c in files.items() if p == path or p.startswith(path + "/")}

        out = []
        node = self.refs[branch]
        while node is not None and len(out) < limit:
            commit = self.commits[node]
            parent = commit["parent"]
            before = under(self.commits[parent]["files"]) if parent else {}
            if under(commit["files"]) != before:
                out.append({"commit": {"message": commit["message"], "committer": {"date": commit["date"]}}})
            node = parent
        return out


def base_files(index_text: str, **plans: str) -> dict[str, str]:
    files = {"dprint.json": DPRINT_JSON, "plans/AGENTS.md": "rules\n", pq.INDEX_PATH: index_text}
    for slug, text in plans.items():
        files[pq.plan_path(slug.replace("_", "-"))] = text
    return files


class Harness:
    """Runs the script's command line against one FakeGitHub, capturing output and exit status."""

    def __init__(self, fake: FakeGitHub, scratch: pathlib.Path):
        self.fake = fake
        self.scratch = scratch
        self.sleeps: list[float] = []

    def run(self, *argv: str) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            code = pq.main(
                ["--repo", "owner/name", *argv],
                gh_factory=lambda repo: self.fake,
                formatter=lambda text, path, config: text,
                sleep=self.sleeps.append,
            )
        return code, out.getvalue(), err.getvalue()

    def text_file(self, name: str, text: str) -> str:
        path = self.scratch / name
        path.write_text(text, encoding="utf-8")
        return str(path)

    @property
    def today(self) -> str:
        """The date the script stamps on what it writes: UTC, like the script."""
        return datetime.datetime.now(datetime.timezone.utc).date().isoformat()

    def state(self, slug: str) -> str:
        parsed, problems = pq.parse_index(self.fake.files()[pq.INDEX_PATH])
        assert not problems, problems
        entry = parsed.find(slug)
        return "gone" if entry is None else entry.state_text()


class QueueTestCase(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="plans-queue-test-")
        self.scratch = pathlib.Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()

    def harness(self, index_text: str, **plans: str) -> Harness:
        return Harness(FakeGitHub(base_files(index_text, **plans)), self.scratch)


# --- Grammar ---------------------------------------------------------------------------------------------------------


class GrammarTest(QueueTestCase):
    def test_round_trip_preserves_header_states_and_dependencies(self):
        """Every state the queue uses, and multi-dependency clauses, survive parse then render byte for byte.

        The script rewrites INDEX.md on every transition; a lossy round trip would silently corrupt other plans' lines
        while moving one.
        """
        text = index(
            line("pending", "a"),
            line("in-flight 0a1b2c", "b", deps=("a",)),
            line("blocked", "c", "has (parentheses) inside", deps=("a", "b")),
            line("complete", "d"),
            line("landing 3d4e5f", "e"),
        )
        parsed, problems = pq.parse_index(text)
        self.assertEqual(problems, [])
        self.assertEqual(parsed.render(), text)
        self.assertEqual(parsed.find("b").claim, "0a1b2c")
        self.assertEqual((parsed.find("e").state, parsed.find("e").claim), ("landing", "3d4e5f"))
        self.assertEqual(parsed.find("c").deps, ["a", "b"])
        self.assertEqual(parsed.find("c").summary, "has (parentheses) inside")

    def test_malformed_lines_are_reported_not_carried(self):
        """A stray line, an unknown state, or a bad claim id is a grammar problem `check` reports."""
        text = index(
            line("pending", "a"),
            "stray text",
            line("done", "b"),
            line("in-flight XYZ", "c"),
            line("in review", "d"),
            line("landing", "e"),
        )
        parsed, problems = pq.parse_index(text)
        self.assertEqual([e.slug for e in parsed.entries], ["a"])
        self.assertEqual(len(problems), 5)
        self.assertEqual(parsed.render(), text)

    def test_reports_round_trip(self):
        """REPORTS.md is rewritten on every landing and review; a lossy round trip would corrupt other plans' lines.
        The summary may hold anything a plan summary holds, including colons and backticks."""
        text = reports(landed("a", (12,)), landed("b", (3, 40, 41), "has: colons and `code` (after `x.md`)"))
        parsed, problems = pq.parse_reports(text)
        self.assertEqual(problems, [])
        self.assertEqual(parsed.render(), text)
        self.assertEqual(parsed.find("b").prs, [3, 40, 41])
        self.assertEqual(parsed.find("b").summary, "has: colons and `code` (after `x.md`)")

    def test_malformed_report_lines_are_kept_and_reported(self):
        """A stray line, a missing PR list, or a link to another plan's report is a problem that stays visible."""
        text = reports(
            landed("a"),
            "stray",
            "- [`b`](reports/b.report.md) landed 2026-10-04 in : nothing",
            "- [`c`](reports/a.report.md) landed 2026-10-04 in #1: points at a's report",
        )
        parsed, problems = pq.parse_reports(text)
        self.assertEqual([e.slug for e in parsed.entries], ["a"])
        self.assertEqual(len(problems), 3)
        self.assertIn("links another plan's report", problems[2])
        self.assertEqual(parsed.render(), text)

    def test_a_mistyped_dependency_clause_is_a_problem(self):
        """Read as summary text, a mistyped `(after ...)` would drop the dependency and let the plan start before the
        work it builds on has landed."""
        for text in ("- [pending] `b.md` — thing (after `a.md` and `c.md`)", "- [pending] `d.md` — thing (after a.md)"):
            with self.subTest(text=text):
                parsed, problems = pq.parse_index(index(text))
                self.assertEqual(parsed.entries, [])
                self.assertEqual(len(problems), 1)
                self.assertIn("malformed (after", problems[0])


# --- Invariants ------------------------------------------------------------------------------------------------------


def snapshot(files: dict[str, str]) -> pq.Snapshot:
    tokens = {path: hashlib.sha1(content.encode()).hexdigest() for path, content in files.items()}
    return pq.Snapshot(files=tokens, read=files.__getitem__)


class InvariantTest(QueueTestCase):
    def test_a_consistent_queue_has_no_violations(self):
        files = {
            pq.INDEX_PATH: index(line("pending", "a"), line("complete", "b", deps=("done",))),
            pq.REPORTS_PATH: reports(landed("done")),
            pq.plan_path("a"): plan(),
            pq.plan_path("b"): plan(),
            pq.plan_path("done"): plan(),
            pq.report_path("b"): "# Report\n",
            pq.report_path("done"): "# Report\n",
        }
        self.assertEqual(pq.violations(snapshot(files)), [])

    def test_each_invariant_fires(self):
        """Each state-to-artifact rule from plans/AGENTS.md is enforced: these are what executors trust without
        re-reading every file."""
        files = {
            pq.INDEX_PATH: index(
                line("pending", "a"),
                line("blocked", "b"),
                line("complete", "c"),
                line("pending", "d", deps=("d",)),
                line("pending", "e", deps=("f",)),
                line("pending", "f", deps=("e",)),
                line("pending", "a"),
                line("pending", "missing"),
            ),
            pq.plan_path("a"): plan() + "\n## Blocked\n\nWhy.\n",
            pq.plan_path("b"): plan(),
            pq.plan_path("c"): plan(),
            pq.plan_path("d"): plan(),
            pq.plan_path("e"): plan(),
            pq.plan_path("f"): plan(),
            pq.plan_path("orphan"): plan(),
            pq.report_path("nobody"): "# Report\n",
            f"{pq.QUEUE_DIR}/notes.txt": "x",
        }
        found = "\n".join(pq.violations(snapshot(files)))
        for expected in (
            "a: has a ## Blocked section but is [pending]",
            "b: [blocked] needs exactly one ## Blocked section, found 0",
            "c: [complete] needs plans/reports/c.report.md",
            "d: depends on itself",
            "dependency cycle",
            "a: listed more than once",
            "missing: listed, but plans/queue/missing.md does not exist",
            "plans/queue/orphan.md: plan file without a line in INDEX.md or REPORTS.md",
            "plans/reports/nobody.report.md: report for a plan that is not listed",
            "plans/queue/notes.txt: unexpected file",
        ):
            self.assertIn(expected, found)

    def test_landed_plan_invariants_fire(self):
        """A landed plan keeps its plan file and report until the maintainer approves it, is never also in the queue,
        and never carries a Blocked section, which nothing could ever answer."""
        files = {
            pq.INDEX_PATH: index(line("pending", "a")),
            pq.REPORTS_PATH: reports(landed("a"), landed("b"), landed("c"), landed("d"), landed("d")),
            pq.plan_path("a"): plan(),
            pq.plan_path("b"): plan() + "\n## Blocked\n\nWhy.\n",
            pq.report_path("b"): "# R\n",
            pq.report_path("c"): "# R\n",
            pq.plan_path("d"): plan(),
            pq.report_path("d"): "# R\n",
        }
        found = "\n".join(pq.violations(snapshot(files)))
        for expected in (
            "a: listed in both INDEX.md and REPORTS.md",
            "a: landed, but plans/reports/a.report.md does not exist",
            "b: has a ## Blocked section but has landed",
            "c: landed, but plans/queue/c.md does not exist",
            "d: listed more than once in REPORTS.md",
        ):
            self.assertIn(expected, found)

    def test_base_rules_refuse_what_a_planning_pr_must_not_change(self):
        """A planning PR that reverts a claim, drops a line, edits a claimed plan, or touches a report is refused.

        This is the guard against a planning agent resolving a textual conflict by taking its own side and silently
        undoing a claim, which would let a second executor build the same plan.
        """
        base = {
            pq.INDEX_PATH: index(line("in-flight 0a1b2c", "a"), line("pending", "b"), line("complete", "c")),
            pq.REPORTS_PATH: reports(landed("l")),
            pq.plan_path("a"): plan(),
            pq.plan_path("b"): plan(),
            pq.plan_path("c"): plan(),
            pq.plan_path("l"): plan(),
            pq.report_path("c"): "# Report\n",
            pq.report_path("l"): "# Report\n",
        }
        bad = dict(base)
        bad[pq.INDEX_PATH] = index(line("pending", "a"), line("blocked", "new"))
        bad[pq.REPORTS_PATH] = reports()
        bad[pq.plan_path("a")] = plan("Revised")
        bad[pq.plan_path("l")] = plan("Revised")
        bad[pq.report_path("c")] = "# Edited\n"
        found = "\n".join(pq.base_violations(snapshot(base), snapshot(bad)))
        for expected in (
            "a: state changed from [in-flight 0a1b2c]",
            "b: line removed",
            "c: line removed",
            "new: new line must be [pending]",
            "a: plan file changed while [in-flight] on main",
            "plans/reports/c.report.md: reports are written only by the queue script",
            "plans/REPORTS.md: reports are written only by the queue script",
            "l: plan file changed after it landed",
        ):
            self.assertIn(expected, found)

    def test_base_rules_allow_ordinary_planning(self):
        """New pending plans, reordering, summary edits and revising a pending plan are what planning PRs do."""
        base = {
            pq.INDEX_PATH: index(line("in-flight 0a1b2c", "a"), line("pending", "b")),
            pq.plan_path("a"): plan(),
            pq.plan_path("b"): plan(),
        }
        good = dict(base)
        good[pq.INDEX_PATH] = index(
            line("pending", "b", "reworded"), line("in-flight 0a1b2c", "a"), line("pending", "new", deps=("a",))
        )
        good[pq.plan_path("b")] = plan("Revised")
        good[pq.plan_path("new")] = plan()
        self.assertEqual(pq.base_violations(snapshot(base), snapshot(good)), [])

    def test_local_check_reads_a_checkout(self):
        """`check` without a ref validates the working copy, which is what a planning PR runs before it lands."""
        root = self.scratch / "checkout"
        (root / pq.QUEUE_DIR).mkdir(parents=True)
        (root / pq.INDEX_PATH).write_text(index(line("pending", "a")), encoding="utf-8")
        (root / pq.plan_path("a")).write_text(plan(), encoding="utf-8")
        with redirect_stdout(io.StringIO()):
            self.assertEqual(pq.main(["check", "--tree", str(root)]), pq.EXIT_OK)
        (root / pq.plan_path("b")).write_text(plan(), encoding="utf-8")
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(pq.main(["check", "--tree", str(root)]), pq.EXIT_VIOLATIONS)
        self.assertIn("plan file without a line in INDEX.md or REPORTS.md", out.getvalue())


# --- Transitions -----------------------------------------------------------------------------------------------------


class TransitionTest(QueueTestCase):
    def test_full_lifecycle(self):
        """A plan goes pending, in flight, blocked, answered, in flight again, complete, landing, landed; a follow-up
        sends it back for a second round, which lands again and is approved, with every artifact the state table
        requires along the way. Landing alone is what lets the dependent plan start."""
        h = self.harness(index(line("pending", "a"), line("pending", "b", deps=("a",))), a=plan(), b=plan())
        self.assertEqual(h.run("claim", "a", "--claim", "0a1b2c")[0], 0)
        self.assertEqual(h.state("a"), "in-flight 0a1b2c")

        question = h.text_file("q.md", "Which way?\n\n### Options\n\nLeft or right.\n")
        self.assertEqual(h.run("block", "a", "--claim", "0a1b2c", "--question", question)[0], 0)
        self.assertEqual(h.state("a"), "blocked")
        self.assertEqual(len(pq.blocked_sections(h.fake.files()[pq.plan_path("a")])), 1)

        answer = h.text_file("a.md", "Go left.\n")
        self.assertEqual(h.run("answer", "a", "--decision", answer)[0], 0)
        text = h.fake.files()[pq.plan_path("a")]
        self.assertEqual(pq.blocked_sections(text), [])
        self.assertIn("## Decisions", text)
        self.assertIn("> Which way?", text)
        self.assertIn("Go left.", text)
        self.assertEqual(h.state("a"), "pending")

        self.assertEqual(h.run("claim", "a", "--claim", "dddddd")[0], 0)
        report = h.text_file("r.md", "# Report\n\nDone.\n")
        self.assertEqual(h.run("deliver", "a", "--claim", "dddddd", "--report", report)[0], 0)
        self.assertEqual(h.state("a"), "complete")
        self.assertIn(pq.report_path("a"), h.fake.files())

        self.assertEqual(h.run("start-landing", "a", "--claim", "111111")[0], 0)
        self.assertEqual(h.state("a"), "landing 111111")
        h.fake.closed[5] = {"ref": "plan/a/01-thing", "merged": True}
        notes = h.text_file("n.md", "Landed alone; ran the unit tests again after the rebase.\n")
        self.assertEqual(h.run("landed", "a", "--claim", "111111", "--merged", "5", "--notes", notes)[0], 0)
        self.assertEqual(h.state("a"), "gone")
        files = h.fake.files()
        self.assertEqual(files[pq.REPORTS_PATH], reports(landed("a", (5,), date=h.today)))
        self.assertTrue(
            files[pq.report_path("a")].endswith(
                "### Landing\n\nLanded alone; ran the unit tests again after the rebase.\n"
            )
        )
        self.assertIn(pq.plan_path("a"), files)
        # Taking the landed plan out of the queue is what satisfies its dependent; review has not happened yet.
        self.assertEqual(h.run("claim", "b", "--claim", "222222")[0], 0)

        follow = h.text_file("f.md", "Also handle the empty case.\n")
        self.assertEqual(h.run("follow-up", "a", "--decision", follow)[0], 0)
        self.assertEqual(h.state("a"), "pending")
        parsed, _ = pq.parse_index(h.fake.files()[pq.INDEX_PATH])
        self.assertEqual([e.slug for e in parsed.entries], ["a", "b"])
        self.assertEqual(parsed.find("a").summary, "does a thing")
        self.assertEqual(pq.parse_reports(h.fake.files()[pq.REPORTS_PATH])[0].entries, [])
        text = h.fake.files()[pq.plan_path("a")]
        self.assertEqual(text.count("## Decisions"), 1)
        self.assertLess(text.index("Go left."), text.index("Also handle the empty case."))

        self.assertEqual(h.run("claim", "a", "--claim", "eeeeee")[0], 0)
        self.assertEqual(h.run("deliver", "a", "--claim", "eeeeee", "--report", report)[0], 0)
        self.assertEqual(h.run("start-landing", "a", "--claim", "333333")[0], 0)
        h.fake.closed[6] = {"ref": "plan/a/02-empty", "merged": True}
        self.assertEqual(h.run("landed", "a", "--claim", "333333", "--merged", "6", "--notes", notes)[0], 0)
        self.assertEqual(h.run("approve", "a")[0], 0)
        files = h.fake.files()
        self.assertNotIn(pq.plan_path("a"), files)
        self.assertNotIn(pq.report_path("a"), files)
        self.assertEqual(files[pq.REPORTS_PATH], reports())

    def test_commit_messages_are_the_fixed_templates(self):
        """Bookkeeping commits skip PR review and the commit-message review, so their wording is pinned here."""
        h = self.harness(index(line("pending", "a")), a=plan())
        h.run("claim", "a", "--claim", "0a1b2c")
        self.assertEqual(h.fake.commits[h.fake.refs["main"]]["message"], "chore(plans): claim a\n\nClaim 0a1b2c.\n")

    def test_illegal_transitions_exit_10(self):
        """Every verb refuses a plan in the wrong state, so a stale caller can never move a plan it does not hold."""
        h = self.harness(
            index(
                line("pending", "p"),
                line("in-flight 0a1b2c", "f"),
                line("complete", "c"),
                line("landing 3d4e5f", "l"),
            ),
            p=plan(),
            f=plan(),
            c=plan(),
            l=plan(),
            d=plan(),
        )
        h.fake.land(
            "main",
            {
                pq.report_path("c"): "# C\n",
                pq.report_path("l"): "# L\n",
                pq.report_path("d"): "# D\n",
                pq.REPORTS_PATH: reports(landed("d")),
            },
        )
        text = h.text_file("t.md", "text\n")
        for argv in (
            ("claim", "f", "--claim", "aaaaaa"),
            ("claim", "c", "--claim", "aaaaaa"),
            ("claim", "d", "--claim", "aaaaaa"),
            ("unclaim", "p", "--claim", "aaaaaa"),
            ("unclaim", "l", "--claim", "3d4e5f"),
            ("block", "p", "--claim", "aaaaaa", "--question", text),
            ("deliver", "c", "--claim", "aaaaaa", "--report", text),
            ("start-landing", "f", "--claim", "aaaaaa"),
            ("start-landing", "l", "--claim", "aaaaaa"),
            ("stop-landing", "c", "--claim", "aaaaaa"),
            ("landed", "c", "--claim", "aaaaaa", "--merged", "1", "--notes", text),
            ("release", "p"),
            ("release", "c"),
            ("answer", "p", "--decision", text),
            ("follow-up", "c", "--decision", text),
            ("approve", "c"),
            ("approve", "p"),
            ("abandon", "f"),
            ("abandon", "l"),
            ("abandon", "d"),
            ("claim", "nope", "--claim", "aaaaaa"),
            ("approve", "nope"),
        ):
            with self.subTest(argv=argv):
                self.assertEqual(h.run(*argv)[0], pq.EXIT_PRECONDITION)

    def test_another_claim_id_cannot_act_on_a_claim(self):
        """Claim holders prove ownership by claim id; a released executor's or monitor's late call is refused."""
        h = self.harness(index(line("in-flight 0a1b2c", "a"), line("landing 3d4e5f", "b")), a=plan(), b=plan())
        h.fake.land("main", {pq.report_path("b"): "# R\n"})
        h.fake.closed[1] = {"ref": "plan/b/01-x", "merged": True}
        text = h.text_file("t.md", "text\n")
        for argv in (
            ("unclaim", "a", "--claim", "ffffff"),
            ("block", "a", "--claim", "ffffff", "--question", text),
            ("deliver", "a", "--claim", "ffffff", "--report", text),
            ("block", "b", "--claim", "ffffff", "--question", text),
            ("stop-landing", "b", "--claim", "ffffff"),
            ("landed", "b", "--claim", "ffffff", "--merged", "1", "--notes", text),
        ):
            with self.subTest(argv=argv):
                code, _, err = h.run(*argv)
                self.assertEqual(code, pq.EXIT_PRECONDITION)
                self.assertIn("carries claim", err)
        self.assertEqual(h.run("unclaim", "a", "--claim", "0a1b2c")[0], 0)
        self.assertEqual(h.state("a"), "pending")
        self.assertEqual(h.run("stop-landing", "b", "--claim", "3d4e5f")[0], 0)
        self.assertEqual(h.state("b"), "complete")

    def test_a_landing_can_block(self):
        """A landing that needs a design decision blocks the plan with a question, as an executor would; the plan's
        report stays, since the answer sends it back to an executor that will deliver a new one."""
        h = self.harness(index(line("landing 3d4e5f", "a")), a=plan())
        h.fake.land("main", {pq.report_path("a"): "# R\n"})
        question = h.text_file("q.md", "The rebase conflicts with the new resume path.\n")
        self.assertEqual(h.run("block", "a", "--claim", "3d4e5f", "--question", question)[0], 0)
        self.assertEqual(h.state("a"), "blocked")
        self.assertIn("Blocked while landing on", h.fake.files()[pq.plan_path("a")])
        self.assertIn(pq.report_path("a"), h.fake.files())

    def test_release_returns_each_claim_to_where_it_came_from(self):
        """A stuck executor's plan goes back to pending; a stuck monitor's goes back to complete, because its work is
        finished and only the landing needs redoing."""
        h = self.harness(index(line("in-flight 0a1b2c", "a"), line("landing 3d4e5f", "b")), a=plan(), b=plan())
        h.fake.land("main", {pq.report_path("b"): "# R\n"})
        self.assertEqual(h.run("release", "a")[0], 0)
        self.assertEqual(h.run("release", "b")[0], 0)
        self.assertEqual((h.state("a"), h.state("b")), ("pending", "complete"))

    def test_claim_refuses_an_unlanded_dependency(self):
        """A dependency is satisfied only once its line is gone; a complete or landing, unlanded one does not count."""
        for state in ("complete", "landing 3d4e5f"):
            with self.subTest(state=state):
                h = self.harness(index(line(state, "a"), line("pending", "b", deps=("a",))), a=plan(), b=plan())
                h.fake.land("main", {pq.report_path("a"): "# R\n"})
                code, _, err = h.run("claim", "b", "--claim", "0a1b2c")
                self.assertEqual(code, pq.EXIT_PRECONDITION)
                self.assertIn("waits on a", err)

    def test_landed_and_abandon_refuse_while_plan_prs_are_open(self):
        """Recording a landing or abandoning a plan whose PRs are still open would lose track of unlanded work. A PR
        of a plan whose slug merely starts the same way (`ab` against `a`) does not count."""
        h = self.harness(index(line("landing 3d4e5f", "a"), line("complete", "c")), a=plan(), c=plan())
        h.fake.land("main", {pq.report_path("a"): "# R\n", pq.report_path("c"): "# R\n"})
        notes = h.text_file("n.md", "Landed.\n")
        h.fake.closed[6] = {"ref": "plan/a/01-thing", "merged": True}
        h.fake.pulls = [
            {"number": 7, "ref": "plan/a/02-more"},
            {"number": 8, "ref": "plan/ab/01-other"},
            {"number": 9, "ref": "plan/c/01-x"},
        ]
        code, _, err = h.run("landed", "a", "--claim", "3d4e5f", "--merged", "6", "--notes", notes)
        self.assertEqual(code, pq.EXIT_PRECONDITION)
        self.assertIn("#7", err)
        self.assertNotIn("#8", err)
        self.assertEqual(h.run("abandon", "c")[0], pq.EXIT_PRECONDITION)
        h.fake.pulls = [{"number": 8, "ref": "plan/ab/01-other"}]
        self.assertEqual(h.run("landed", "a", "--claim", "3d4e5f", "--merged", "6", "--notes", notes)[0], 0)
        self.assertEqual(h.run("abandon", "c")[0], 0)
        self.assertNotIn(pq.report_path("c"), h.fake.files())

    def test_landed_requires_a_merged_pr_of_this_plan(self):
        """`landed` records the plan's work as on main, so "no open PRs" is not enough: PRs the maintainer closed
        unmerged look the same. The caller names the PRs that landed and each is verified."""
        h = self.harness(index(line("landing 3d4e5f", "a")), a=plan())
        h.fake.land("main", {pq.report_path("a"): "# R\n"})
        notes = h.text_file("n.md", "Landed.\n")
        h.fake.closed[6] = {"ref": "plan/a/01-thing", "merged": False}
        h.fake.closed[9] = {"ref": "plan/other/01-x", "merged": True}
        base = ("landed", "a", "--claim", "3d4e5f", "--notes", notes, "--merged")
        self.assertEqual(h.run(*base, "6")[0], pq.EXIT_PRECONDITION)
        self.assertEqual(h.run(*base, "9")[0], pq.EXIT_USAGE)
        self.assertEqual(h.run(*base, "")[0], pq.EXIT_USAGE)
        self.assertEqual(h.state("a"), "landing 3d4e5f")
        h.fake.closed[6]["merged"] = True
        self.assertEqual(h.run(*base, "#6")[0], 0)

    def test_landing_notes_are_checked_like_spliced_text(self):
        """The notes land under the report's `### Landing` heading on main without review, so a `##` or `###` heading
        (either would end or split that section) and local paths are refused, while `####` is fine."""
        h = self.harness(index(line("landing 3d4e5f", "a")), a=plan())
        h.fake.land("main", {pq.report_path("a"): "# R\n"})
        h.fake.closed[6] = {"ref": "plan/a/01-thing", "merged": True}
        for body in ("Fine.\n\n## Checks\n\nAll.\n", "Fine.\n\n### Checks\n\nAll.\n", "Built in /tmp/x\n"):
            with self.subTest(body=body):
                notes = h.text_file("n.md", body)
                code = h.run("landed", "a", "--claim", "3d4e5f", "--merged", "6", "--notes", notes)[0]
                self.assertEqual(code, pq.EXIT_USAGE)
        notes = h.text_file("n.md", "Fine.\n\n#### Checks\n\nAll.\n")
        self.assertEqual(h.run("landed", "a", "--claim", "3d4e5f", "--merged", "6", "--notes", notes)[0], 0)

    def test_abandon_settles_dependents_explicitly(self):
        """Abandoning must not silently satisfy a dependent's `(after ...)`: the caller says block or drop-dep."""
        h = self.harness(
            index(line("pending", "a"), line("pending", "b", deps=("a",)), line("pending", "c", deps=("a", "z"))),
            a=plan(),
            b=plan(),
            c=plan(),
        )
        self.assertEqual(h.run("abandon", "a")[0], pq.EXIT_USAGE)
        self.assertEqual(h.run("abandon", "a", "--dependents", "block")[0], 0)
        self.assertEqual(h.state("b"), "blocked")
        self.assertIn("was abandoned", h.fake.files()[pq.plan_path("b")])
        parsed, _ = pq.parse_index(h.fake.files()[pq.INDEX_PATH])
        self.assertEqual(parsed.find("c").deps, ["z"])

        h = self.harness(index(line("pending", "a"), line("pending", "b", deps=("a",))), a=plan(), b=plan())
        self.assertEqual(h.run("abandon", "a", "--dependents", "drop-dep")[0], 0)
        self.assertEqual(h.state("b"), "pending")
        self.assertEqual(pq.parse_index(h.fake.files()[pq.INDEX_PATH])[0].find("b").deps, [])

    def test_spliced_text_rejects_high_headings_and_local_details(self):
        """A `##` heading would split the Blocked section; a home path or temp path must not reach the public main."""
        h = self.harness(index(line("in-flight 0a1b2c", "a")), a=plan())
        for body in ("Why?\n\n## Options\n\nA.\n", "See /home/someone/log.md\n", "Ran in /tmp/x\n"):
            with self.subTest(body=body):
                question = h.text_file("q.md", body)
                self.assertEqual(h.run("block", "a", "--claim", "0a1b2c", "--question", question)[0], pq.EXIT_USAGE)
        fenced = h.text_file("q.md", "Why?\n\n```md\n## Not a heading\n```\n")
        self.assertEqual(h.run("block", "a", "--claim", "0a1b2c", "--question", fenced)[0], 0)
        self.assertEqual(len(pq.blocked_sections(h.fake.files()[pq.plan_path("a")])), 1)

    def test_fences_follow_commonmark_closing_rules(self):
        """A quoted inner fence or an info-string line must not end a fence, or a `##` line inside quoted Markdown
        would be taken for a section heading of the plan file."""
        text = "intro\n\n````markdown\n```bash\n## not a heading\n```\n````\n\n## Real\n\nbody\n"
        self.assertEqual([heading for heading, _, _ in pq._section_spans(text)], ["Real"])
        self.assertEqual(pq.splice_problems("````md\n```bash\n## x\n```\n````\n"), [])

    def test_heading_spellings_cannot_strand_a_blocked_question(self):
        """Refuse formatter-normalized sibling headings before a block can lose its suffix.

        Deeper headings and fenced examples still round-trip through block
        and answer, so the guard cannot solve corruption by refusing Markdown.
        """
        h = self.harness(index(line("in-flight 0a1b2c", "a")), a=plan())
        original = h.fake.files()[pq.plan_path("a")]
        for heading in (" ## Options", "   #\tOptions", "##", "Options\n---", "Options\n  ==="):
            with self.subTest(heading=heading):
                question = h.text_file("q.md", f"Why?\n\n{heading}\n\nKeep this suffix.\n")
                self.assertEqual(h.run("block", "a", "--claim", "0a1b2c", "--question", question)[0], pq.EXIT_USAGE)
                self.assertEqual(h.fake.files()[pq.plan_path("a")], original)
                self.assertEqual(h.state("a"), "in-flight 0a1b2c")
        body = "Why?\n\n### Options\n\n```md\n  ## Example\nOptions\n---\n```\n\nKeep this suffix.\n"
        question = h.text_file("q.md", body)
        self.assertEqual(h.run("block", "a", "--claim", "0a1b2c", "--question", question)[0], 0)
        decision = h.text_file("d.md", "Keep the existing choice.\n")
        self.assertEqual(h.run("answer", "a", "--decision", decision)[0], 0)
        archived = h.fake.files()[pq.plan_path("a")]
        self.assertEqual(pq.blocked_sections(archived), [])
        self.assertIn("> Keep this suffix.", archived)
        self.assertEqual(pq.splice_problems("Paragraph.\n\n---\n"), [])

    def test_unclosed_fence_is_refused_in_spliced_text(self):
        """An unclosed fence in a decision would hide the next `## Blocked` heading, so the next block would fail."""
        h = self.harness(index(line("blocked", "a")), a=plan() + "\n## Blocked\n\nWhy?\n")
        decision = h.text_file("d.md", "Go left.\n\n```sh\nmake\n")
        self.assertEqual(h.run("answer", "a", "--decision", decision)[0], pq.EXIT_USAGE)
        self.assertEqual(h.state("a"), "blocked")

    def test_hostname_is_matched_as_a_word(self):
        """The host name check must catch the name itself without refusing every word that merely contains it."""
        self.assertTrue(pq.hygiene_problems("built on buildbox today", ["buildbox"]))
        self.assertFalse(pq.hygiene_problems("buildboxes are fine", ["buildbox"]))

    def test_removing_a_line_keeps_stray_lines_in_place_and_unblocked(self):
        """Removing a line shifts the line numbers in every parse problem below it. Those must still count as the
        same, already-present problems, or one malformed line would stop every landing; and the stray line must stay
        between the neighbors it had."""
        h = self.harness(
            index(line("landing 3d4e5f", "a"), line("pending", "b"), "stray mid", line("pending", "c"), "stray end"),
            a=plan(),
            b=plan(),
            c=plan(),
        )
        h.fake.land("main", {pq.report_path("a"): "# R\n"})
        h.fake.closed[1] = {"ref": "plan/a/01-x", "merged": True}
        notes = h.text_file("n.md", "Landed.\n")
        code, _, err = h.run("landed", "a", "--claim", "3d4e5f", "--merged", "1", "--notes", notes)
        self.assertEqual(code, 0, err)
        self.assertEqual(
            h.fake.files()[pq.INDEX_PATH],
            index(line("pending", "b"), "stray mid", line("pending", "c"), "stray end"),
        )

    def test_a_follow_up_does_not_hold_back_dependents(self):
        """A plan that waits on another waited for its work to reach main. A follow-up puts that other plan back in
        the queue, but its first round is still on main, so the maintainer chose to let dependents proceed rather than
        wait for the follow-up to land too. Other dependencies are kept."""
        h = self.harness(
            index(line("pending", "y", deps=("x", "z")), line("pending", "z")), x=plan(), y=plan(), z=plan()
        )
        h.fake.land("main", {pq.report_path("x"): "# R\n", pq.REPORTS_PATH: reports(landed("x"))})
        self.assertEqual(h.run("follow-up", "x", "--decision", h.text_file("d.md", "More.\n"))[0], 0)
        parsed, _ = pq.parse_index(h.fake.files()[pq.INDEX_PATH])
        self.assertEqual([e.slug for e in parsed.entries], ["x", "y", "z"])
        self.assertEqual(parsed.find("y").deps, ["z"])

    def test_review_verbs_leave_index_untouched(self):
        """Approving a landed plan changes REPORTS.md and deletes the plan's files, nothing else. Rewriting INDEX.md
        would put every review commit in textual conflict with whatever planning PR is open."""
        h = self.harness(index(line("pending", "b"), "", line("pending", "c")), a=plan(), b=plan(), c=plan())
        h.fake.land("main", {pq.report_path("a"): "# R\n", pq.REPORTS_PATH: reports(landed("a"))})
        before = h.fake.files()[pq.INDEX_PATH]
        self.assertEqual(h.run("approve", "a")[0], 0)
        self.assertEqual(h.fake.files()[pq.INDEX_PATH], before)

    def test_existing_violations_warn_but_new_ones_refuse(self):
        """One malformed line merged by a planning PR must not stop every executor from blocking or delivering."""
        h = self.harness(index("stray first", line("pending", "a"), "stray line", "another"), a=plan())
        code, _, err = h.run("claim", "a", "--claim", "0a1b2c")
        self.assertEqual(code, 0)
        self.assertIn("warning: already on main", err)
        # The malformed lines stay where they were rather than being dropped by the rewrite.
        self.assertEqual(
            h.fake.files()[pq.INDEX_PATH],
            index("stray first", line("in-flight 0a1b2c", "a"), "stray line", "another"),
        )

    def test_writes_outside_the_queue_are_refused(self):
        req = pq.Request(verb="claim", slug="a", claim="0a1b2c")
        fake = FakeGitHub(base_files(index(line("pending", "a")), a=plan()))
        original = pq.plan_transition

        def sneaky(*args, **kwargs):
            change = original(*args, **kwargs)
            change.writes["TODO.md"] = "x\n"
            return change

        pq.plan_transition = sneaky
        try:
            with self.assertRaises(pq.Failure):
                pq.apply(fake, req, "main", lambda t, p, c: t, sleep=lambda s: None, log=lambda s: None)
        finally:
            pq.plan_transition = original


# --- The compare-and-swap loop -------------------------------------------------------------------------------------


class CasTest(QueueTestCase):
    def setUp(self):
        super().setUp()
        self.fake = FakeGitHub(base_files(index(line("pending", "a"), line("pending", "b")), a=plan(), b=plan()))

    def claim(self, slug="a", claim="0a1b2c", attempts=8):
        req = pq.Request(verb="claim", slug=slug, claim=claim)
        return pq.apply(
            self.fake, req, "main", lambda t, p, c: t, attempts=attempts, sleep=lambda s: None, log=lambda s: None
        )

    def state(self, slug):
        return pq.parse_index(self.fake.files()[pq.INDEX_PATH])[0].find(slug).state_text()

    def test_plain_success(self):
        """With main not moving, a transition is one read and one ref update."""
        self.assertEqual(self.claim(), self.fake.refs["main"])
        self.assertEqual(self.state("a"), "in-flight 0a1b2c")

    def test_unrelated_concurrent_commit_retries_and_keeps_both(self):
        """Main moving under the script is the normal case with several executors; the claim retries on the new head
        and the other writer's change survives."""
        fired = []

        def other_claims_b(fake, branch):
            if not fired:
                fired.append(1)
                text = fake.files()[pq.INDEX_PATH].replace("[pending] `b.md`", "[in-flight ffffff] `b.md`")
                fake.land(branch, {pq.INDEX_PATH: text})

        self.fake.before_update = other_claims_b
        self.claim()
        self.assertEqual(self.state("a"), "in-flight 0a1b2c")
        self.assertEqual(self.state("b"), "in-flight ffffff")
        self.assertEqual(self.fake.updates, 2)

    def test_racing_claim_on_the_same_line_loses_with_exit_10(self):
        """Two executors claiming one plan: the one whose update is refused re-reads, sees the line taken, and gets
        Precondition (exit 10), never a second claim over the first."""

        def other_claims_a(fake, branch):
            if "0a1b2c" not in fake.files()[pq.INDEX_PATH] and "ffffff" not in fake.files()[pq.INDEX_PATH]:
                text = fake.files()[pq.INDEX_PATH].replace("[pending] `a.md`", "[in-flight ffffff] `a.md`")
                fake.land(branch, {pq.INDEX_PATH: text})

        self.fake.before_update = other_claims_a
        with self.assertRaises(pq.Precondition):
            self.claim()
        self.assertEqual(self.state("a"), "in-flight ffffff")

    def test_endless_contention_is_an_error(self):
        """A branch that never stops moving must end in an error after the attempt budget, not loop forever."""

        def always_moves(fake, branch):
            fake.land(branch, {"plans/queue/noise.md": None, "dprint.json": DPRINT_JSON + " " * fake.counter})

        self.fake.before_update = always_moves
        with self.assertRaises(pq.Failure) as caught:
            self.claim(attempts=3)
        self.assertIn("contended", str(caught.exception))
        self.assertEqual(self.fake.updates, 3)

    def test_refusal_that_landed_anyway_is_success(self):
        """GitHub can refuse a racing update as "cannot be updated"; if our commit is on the branch regardless, the
        claim is ours, and retrying would misread our own claim as another executor's and report exit 10."""
        self.fake.update_failure = pq.NotFastForward("Reference cannot be updated")
        self.fake.update_failure_lands = True
        commit = self.claim()
        self.assertEqual(self.fake.refs["main"], commit)
        self.assertEqual(self.fake.updates, 1)

    def test_lost_response_after_landing_is_success(self):
        """A ref update whose response never came back but which landed must not be reported as a failure: the
        executor would otherwise believe it holds no claim while main says it does."""
        self.fake.update_failure = pq.Failure("connection reset")
        self.fake.update_failure_lands = True
        commit = self.claim()
        self.assertEqual(self.fake.refs["main"], commit)
        self.assertEqual(self.state("a"), "in-flight 0a1b2c")

    def test_a_stale_read_is_checked_again_before_reporting_lost(self):
        """Exit 10 makes an executor drop its work, so a precondition failure seen on a possibly stale read is
        re-checked once after a pause before it is believed."""
        self.fake.stale_heads = [
            self.fake.land("main", {pq.INDEX_PATH: index(line("in-flight ffffff", "a"), line("pending", "b"))})
        ]
        self.fake.land("main", {pq.INDEX_PATH: index(line("pending", "a"), line("pending", "b"))})
        sleeps = []
        req = pq.Request(verb="claim", slug="a", claim="0a1b2c")
        pq.apply(self.fake, req, "main", lambda t, p, c: t, sleep=sleeps.append, log=lambda s: None)
        self.assertIn(2.0, sleeps)
        self.assertEqual(self.state("a"), "in-flight 0a1b2c")

    def test_cli_exit_codes(self):
        """plans/AGENTS.md tells agents what to do by exit code: 10 for a lost race, 3 for contention or anything
        unexpected. Those are the contract, so they are checked through the command line, not just as exceptions."""
        h = Harness(self.fake, self.scratch)

        def other_claims_a(fake, branch):
            if "ffffff" not in fake.files()[pq.INDEX_PATH]:
                text = fake.files()[pq.INDEX_PATH].replace("[pending] `a.md`", "[in-flight ffffff] `a.md`")
                fake.land(branch, {pq.INDEX_PATH: text})

        self.fake.before_update = other_claims_a
        self.assertEqual(h.run("claim", "a", "--claim", "0a1b2c")[0], pq.EXIT_PRECONDITION)

        def always_moves(fake, branch):
            fake.land(branch, {"dprint.json": DPRINT_JSON + str(fake.counter)})

        self.fake.before_update = always_moves
        code, _, err = h.run("claim", "b", "--claim", "0a1b2c")
        self.assertEqual(code, pq.EXIT_ERROR)
        self.assertIn("contended", err)

        self.fake.before_update = None
        self.fake.head = lambda branch: {}["boom"]
        code, _, err = h.run("claim", "b", "--claim", "0a1b2c")
        self.assertEqual(code, pq.EXIT_ERROR)
        self.assertIn("unexpected KeyError", err)

    def test_other_update_failures_are_not_retried(self):
        """A protection change or missing object is not contention; retrying would only hide it."""
        self.fake.update_failure = pq.Failure("Protected branch update failed (HTTP 422)")
        with self.assertRaises(pq.Failure):
            self.claim()
        self.assertEqual(self.fake.updates, 1)
        self.assertEqual(self.state("a"), "pending")


class GhApiErrorTest(QueueTestCase):
    """The real client's error classification, against a stub `gh` executable passed by path.

    Which refusals count as contention decides whether a losing executor retries and cleanly reports "lost" or stops
    with an error. The texts are what `gh api` prints for GitHub's responses.
    """

    def stub(self, stderr: str) -> str:
        path = self.scratch / "gh"
        path.write_text(f"#!/bin/sh\nprintf '%s\\n' '{stderr}' >&2\nexit 1\n", encoding="utf-8")
        path.chmod(0o755)
        return str(path)

    def test_contention_refusals(self):
        for text in ("gh: Update is not a fast forward (HTTP 422)", "gh: Reference cannot be updated (HTTP 422)"):
            with self.subTest(text=text):
                with self.assertRaises(pq.NotFastForward):
                    pq.GhApi("owner/name", gh=self.stub(text)).update_ref("main", "0" * 40)

    def test_other_refusals_are_failures(self):
        api = pq.GhApi("owner/name", gh=self.stub("gh: Protected branch update failed (HTTP 422)"))
        with self.assertRaises(pq.Failure):
            api.update_ref("main", "0" * 40)


# --- Wake check ------------------------------------------------------------------------------------------------------


class WakeCheckTest(QueueTestCase):
    """The watcher's filter: an idle executor wakes only for work it could pick, the monitor only for finished plans."""

    def setUp(self):
        super().setUp()
        self.fake = FakeGitHub(
            base_files(
                index(line("pending", "a"), line("in-flight 0a1b2c", "b"), line("pending", "w", deps=("b",))),
                a=plan(),
                b=plan(),
                w=plan(),
            )
        )
        self.baseline = self.fake.refs["main"]

    def set_index(self, *lines):
        self.fake.land("main", {pq.INDEX_PATH: index(*lines)})

    def wakes(self, role="executor"):
        return pq.wake_check(self.fake, self.baseline, "main", role)

    def test_claims_are_ignored(self):
        """Claims are the most frequent commits and never create work for an idle executor."""
        self.set_index(line("in-flight 111111", "a"), line("in-flight 0a1b2c", "b"), line("pending", "w", deps=("b",)))
        self.assertFalse(self.wakes())

    def test_a_claim_replaced_by_another_is_ignored(self):
        """A release followed by a new claim nets out to no new work."""
        self.set_index(line("pending", "a"), line("in-flight 222222", "b"), line("pending", "w", deps=("b",)))
        self.assertFalse(self.wakes())

    def test_a_claim_given_back_wakes(self):
        """A claim given back makes a plan pending again, which is work; a digest of one state could not see it."""
        self.set_index(line("pending", "a"), line("pending", "b"), line("pending", "w", deps=("b",)))
        self.assertTrue(self.wakes())

    def test_claim_then_release_nets_to_nothing(self):
        """A plan claimed and given back between two polls is where it started."""
        self.set_index(line("in-flight 111111", "a"), line("in-flight 0a1b2c", "b"), line("pending", "w", deps=("b",)))
        self.set_index(line("pending", "a"), line("in-flight 0a1b2c", "b"), line("pending", "w", deps=("b",)))
        self.assertFalse(self.wakes())

    def test_delivering_or_landing_does_not_wake_executors(self):
        """Deliveries and landing claims leave a plan with open PRs, so it still counts as unlanded work and frees
        nothing an idle executor could pick. Before the monitor existed every state change woke every executor, which
        wasted a model round each time."""
        for changed in (line("complete", "b"), line("landing 3d4e5f", "b")):
            with self.subTest(changed):
                self.fake.refs["main"] = self.baseline
                files = {pq.report_path("b"): "# R\n"}
                files[pq.INDEX_PATH] = index(line("pending", "a"), changed, line("pending", "w", deps=("b",)))
                self.fake.land("main", files)
                self.assertFalse(self.wakes())

    def test_a_block_wakes_executors(self):
        """A plan that blocks before opening any PR stops being unlanded work, so a plan an executor skipped for
        conflicting with it may now be picked. The filter cannot see PRs, so every block wakes. Found in review: the
        first version ignored blocks, and the skipped plan sat until something unrelated changed."""
        self.fake.land(
            "main",
            {
                pq.plan_path("b"): plan() + "\n## Blocked\n\nWhy?\n",
                pq.INDEX_PATH: index(line("pending", "a"), line("blocked", "b"), line("pending", "w", deps=("b",))),
            },
        )
        self.assertTrue(self.wakes())

    def test_work_that_becomes_pickable_wakes(self):
        """A new plan, an answered or followed-up one, and a dropped dependency all make a plan eligible."""
        cases = {
            "new line": (
                line("pending", "a"),
                line("in-flight 0a1b2c", "b"),
                line("pending", "w", deps=("b",)),
                line("pending", "c"),
            ),
            "dependency dropped": (line("pending", "a"), line("in-flight 0a1b2c", "b"), line("pending", "w")),
        }
        for name, lines in cases.items():
            with self.subTest(name):
                self.fake.refs["main"] = self.baseline
                self.fake.land("main", {pq.INDEX_PATH: index(*lines), pq.plan_path("c"): plan()})
                self.assertTrue(self.wakes())

    def test_reordering_eligible_plans_wakes(self):
        """Picking prefers the oldest eligible plan, so swapping two eligible plans can change what an idle executor
        would take. The same plans with the same files in a new order must wake it."""
        self.fake.land(
            "main",
            {pq.INDEX_PATH: index(line("pending", "a"), line("pending", "c")), pq.plan_path("c"): plan()},
        )
        self.baseline = self.fake.refs["main"]
        self.set_index(line("pending", "c"), line("pending", "a"))
        self.assertTrue(self.wakes())

    def test_a_plan_leaving_the_queue_wakes(self):
        """A plan that landed or was abandoned can satisfy a dependency or end a conflict an executor skipped for."""
        self.fake.land(
            "main",
            {
                pq.INDEX_PATH: index(line("pending", "a"), line("pending", "w", deps=("b",))),
                pq.REPORTS_PATH: reports(landed("b")),
                pq.report_path("b"): "# R\n",
            },
        )
        self.assertTrue(self.wakes())

    def test_a_revision_wakes_only_when_the_plan_is_eligible(self):
        """Revising a plan an executor could pick changes what it would do; revising one still waiting does not."""
        self.fake.land("main", {pq.plan_path("w"): plan("Revised")})
        self.assertFalse(self.wakes())
        self.fake.land("main", {pq.plan_path("a"): plan("Revised")})
        self.assertTrue(self.wakes())

    def test_review_bookkeeping_is_ignored(self):
        """Approving a landed plan touches only REPORTS.md and its own files, none of which give an executor work."""
        self.fake.land("main", {pq.REPORTS_PATH: reports(), "plans/AGENTS.md": "new rules\n"})
        self.assertFalse(self.wakes())

    def test_the_lander_wakes_only_for_newly_complete_plans(self):
        """The monitor acts on nothing else, so it sleeps through claims, blocks, answers and its own landing claims."""
        self.set_index(line("pending", "a"), line("blocked", "b"), line("pending", "w"))
        self.assertFalse(self.wakes("lander"))
        self.fake.land(
            "main",
            {
                pq.INDEX_PATH: index(line("pending", "a"), line("complete", "b"), line("pending", "w")),
                pq.report_path("b"): "# R\n",
            },
        )
        self.assertTrue(self.wakes("lander"))
        self.baseline = self.fake.refs["main"]
        self.set_index(line("pending", "a"), line("landing 3d4e5f", "b"), line("pending", "w"))
        self.assertFalse(self.wakes("lander"))
        self.set_index(line("pending", "a"), line("complete", "b"), line("pending", "w"))
        self.assertFalse(self.wakes("lander"), "complete at the baseline already, so nothing new to land")

    def test_the_lander_wakes_for_a_redelivery_complete_at_the_baseline(self):
        """The monitor's baseline can predate the last delivery: a plan complete at the baseline is landed, comes back
        through a follow-up, and is delivered again. It is complete in both snapshots, but with a new report, and
        nothing else would wake the monitor for it. Found in review, reproduced with the real transitions."""
        h = Harness(self.fake, self.scratch)
        self.fake.land(
            "main",
            {
                pq.INDEX_PATH: index(line("complete", "a"), line("in-flight 0a1b2c", "b"), line("pending", "w")),
                pq.report_path("a"): "# Report\n",
            },
        )
        self.baseline = self.fake.refs["main"]
        self.fake.closed[1] = {"ref": "plan/a/01-x", "merged": True}
        notes = h.text_file("n.md", "Landed.\n")
        decision = h.text_file("d.md", "More, please.\n")
        report = h.text_file("r.md", "# Report, second round\n")
        for argv in (
            ("start-landing", "a", "--claim", "3d4e5f"),
            ("landed", "a", "--claim", "3d4e5f", "--merged", "1", "--notes", notes),
            ("follow-up", "a", "--decision", decision),
            ("claim", "a", "--claim", "111111"),
        ):
            self.assertEqual(h.run(*argv)[0], 0, argv)
        self.assertFalse(self.wakes("lander"), "in flight again: nothing to land yet")
        self.assertEqual(h.run("deliver", "a", "--claim", "111111", "--report", report)[0], 0)
        self.assertTrue(self.wakes("lander"))

    def test_the_role_is_passed_through_the_command_line(self):
        """The watcher selects the filter with `--for`; an unknown role is a usage error, not a silent executor."""
        h = Harness(self.fake, self.scratch)
        argv = ("wake-check", "--baseline", self.baseline, "--ref", "main", "--for", "lander")
        self.assertEqual(h.run(*argv)[1], "ignore\n")
        with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            h.run("wake-check", "--baseline", self.baseline, "--ref", "main", "--for", "nobody")


# --- Status ----------------------------------------------------------------------------------------------------------


class StatusTest(QueueTestCase):
    def test_flags(self):
        """status names the situations someone must act on: a landing that stopped before `landed` (with the merged
        PRs to pass it), a landing whose PRs closed without merging, a complete plan with nothing left to land,
        unlanded work from an earlier round, and a pending plan's unmet dependency. Landed plans awaiting review are
        listed after the queue."""
        fake = FakeGitHub(
            base_files(
                index(
                    line("landing 3d4e5f", "a"),
                    line("landing 3d4e5f", "d"),
                    line("complete", "e"),
                    line("pending", "b"),
                    line("pending", "c", deps=("a",)),
                ),
                a=plan(),
                b=plan(),
                c=plan(),
                d=plan(),
                e=plan(),
                r=plan(),
            )
        )
        fake.land(
            "main",
            {
                pq.report_path("a"): "# R\n",
                pq.report_path("d"): "# R\n",
                pq.report_path("e"): "# R\n",
                pq.report_path("r"): "# R\n",
                pq.REPORTS_PATH: reports(landed("r", (2, 3))),
            },
        )
        fake.pulls = [{"number": 3, "ref": "plan/b/01-x"}]
        fake.closed = {4: {"ref": "plan/a/01-x", "merged": True}, 5: {"ref": "plan/d/01-x", "merged": False}}
        data = pq.status(fake, "main", self.scratch, datetime.datetime(2026, 10, 3, tzinfo=datetime.timezone.utc))
        by_slug = {p["slug"]: p for p in data["plans"]}
        self.assertIn("landing, no open PRs, merged 4: finish with landed --merged", by_slug["a"]["flags"])
        self.assertTrue(any("ask the maintainer" in f for f in by_slug["d"]["flags"]))
        self.assertTrue(any("none found merged" in f and "ask the maintainer" in f for f in by_slug["e"]["flags"]))
        self.assertIn("unlanded work from an earlier round", by_slug["b"]["flags"])
        self.assertEqual(by_slug["c"]["deps_waiting"], ["a"])
        self.assertEqual(data["landed"], [{"slug": "r", "landed": "2026-10-04", "prs": [2, 3]}])
        rendered = pq.render_status(data)
        self.assertIn("[landing 3d4e5f] a", rendered)
        self.assertIn("[landed 2026-10-04] r | PRs #2, #3 | report awaiting review", rendered)

    def test_a_released_landing_whose_prs_all_merged_can_be_finished(self):
        """A monitor that merged every PR and died before `landed` leaves, once released, a complete plan with no open
        PRs. That is not the same as PRs closed unmerged: status names the merged PRs so the next landing round can
        record it, instead of leaving it, and every plan waiting on it, for the maintainer. Found in review."""
        fake = FakeGitHub(base_files(index(line("complete", "a")), a=plan()))
        fake.land("main", {pq.report_path("a"): "# R\n"})
        fake.closed = {7: {"ref": "plan/a/01-x", "merged": True}, 8: {"ref": "plan/a/02-y", "merged": True}}
        data = pq.status(fake, "main", self.scratch, datetime.datetime(2026, 10, 3, tzinfo=datetime.timezone.utc))
        self.assertIn(
            "complete, no open PRs, merged 7,8: claim with start-landing, finish with landed --merged",
            data["plans"][0]["flags"],
        )

    def test_claim_missing_from_history_is_flagged(self):
        """A claim older than the history status reads would otherwise get no age and no flag at all."""
        fake = FakeGitHub(base_files(index(line("in-flight 0a1b2c", "a")), a=plan()))
        data = pq.status(fake, "main", self.scratch, datetime.datetime(2026, 10, 3, tzinfo=datetime.timezone.utc))
        self.assertIsNone(data["plans"][0]["claimed_at"])
        # With no working log at all, the claim is as good as abandoned, and gets the flag the monitor notifies on.
        # Found in review: this case used to get a flag nothing acted on.
        self.assertTrue(any(f.startswith("possibly abandoned") for f in data["plans"][0]["flags"]))
        (self.scratch / "farhelm-plan-a-log.md").write_text("working\n", encoding="utf-8")
        data = pq.status(fake, "main", self.scratch, datetime.datetime.now(datetime.timezone.utc))
        self.assertFalse(any(f.startswith("possibly abandoned") for f in data["plans"][0]["flags"]))
        self.assertTrue(any("not in recent history" in f for f in data["plans"][0]["flags"]))

    def test_history_shows_each_claim_holders_commits(self):
        """An executor or monitor that crashed mid-verb uses history to see whether its own commit landed, so every
        commit a claim holder makes must name its claim id. Review commits touch only REPORTS.md and the plan's own
        files, and must show up too."""
        h = self.harness(index(line("pending", "a"), line("pending", "ab")), a=plan(), ab=plan())
        h.run("claim", "a", "--claim", "0a1b2c")
        h.run("claim", "ab", "--claim", "111111")
        h.run("deliver", "a", "--claim", "0a1b2c", "--report", h.text_file("r.md", "# R\n"))
        h.run("start-landing", "a", "--claim", "3d4e5f")
        h.run("stop-landing", "a", "--claim", "3d4e5f")
        h.run("start-landing", "a", "--claim", "444444")
        h.run("block", "a", "--claim", "444444", "--question", h.text_file("q.md", "Which way?\n"))
        h.run("answer", "a", "--decision", h.text_file("d.md", "Left.\n"))
        h.run("claim", "a", "--claim", "555555")
        h.run("deliver", "a", "--claim", "555555", "--report", h.text_file("r.md", "# R2\n"))
        h.run("start-landing", "a", "--claim", "3d4e5f")
        h.fake.closed[1] = {"ref": "plan/a/01-x", "merged": True}
        h.run("landed", "a", "--claim", "3d4e5f", "--merged", "1", "--notes", h.text_file("n.md", "Landed.\n"))
        h.run("approve", "a")
        lines = pq.history(h.fake, "main", "a", 100)
        expected = [
            "chore(plans): approve a",
            "chore(plans): record the landing of a (claim 3d4e5f)",
            "chore(plans): start landing a (claim 3d4e5f)",
            "chore(plans): deliver the report for a (claim 555555)",
            "chore(plans): claim a (claim 555555)",
            "chore(plans): answer a",
            "chore(plans): block a (claim 444444)",
            "chore(plans): start landing a (claim 444444)",
            "chore(plans): stop landing a (claim 3d4e5f)",
            "chore(plans): start landing a (claim 3d4e5f)",
            "chore(plans): deliver the report for a (claim 0a1b2c)",
            "chore(plans): claim a (claim 0a1b2c)",
        ]
        self.assertEqual([entry.split(" ", 1)[1] for entry in lines], expected)

    def test_old_quiet_claim_is_flagged(self):
        """A claim over a day old whose log is missing or quiet is how a dead executor or monitor shows up. A landing
        claim is judged by the monitor's log, since the plan's working log belongs to the executors."""
        fake = FakeGitHub(base_files(index(line("pending", "a"), line("complete", "b")), a=plan(), b=plan()))
        fake.land("main", {pq.report_path("b"): "# R\n"})
        for verb, slug, claim in (("claim", "a", "0a1b2c"), ("start-landing", "b", "3d4e5f")):
            req = pq.Request(verb=verb, slug=slug, claim=claim)
            pq.apply(fake, req, "main", lambda t, p, c: t, sleep=lambda s: None, log=lambda s: None)
        now = datetime.datetime(2026, 10, 5, tzinfo=datetime.timezone.utc)
        data = pq.status(fake, "main", self.scratch, now)
        for item in data["plans"]:
            self.assertEqual(item["claimed_at"], "2026-10-02T10:00:00Z")
            self.assertTrue(any("possibly abandoned" in f for f in item["flags"]), item)
        # A monitor log written recently means the monitor is alive, whatever the plan's working log says.
        (self.scratch / pq.MONITOR_LOG).write_text("alive\n", encoding="utf-8")
        data = pq.status(fake, "main", self.scratch, datetime.datetime.now(datetime.timezone.utc))
        by_slug = {p["slug"]: p for p in data["plans"]}
        self.assertFalse(any("possibly abandoned" in f for f in by_slug["b"]["flags"]))
        self.assertTrue(any("possibly abandoned" in f for f in by_slug["a"]["flags"]))


# --- Real dprint ----------------------------------------------------------------------------------------------------


@unittest.skipUnless(shutil.which("dprint"), "dprint is not on PATH")
class DprintRoundTripTest(QueueTestCase):
    def test_a_blocked_plan_formatted_by_dprint_still_splices(self):
        """dprint rewraps and normalizes Markdown; the Blocked section must still be found and removed afterwards."""
        config = (REPO_ROOT / "dprint.json").read_text(encoding="utf-8")
        long = " ".join(["word"] * 60)
        text = pq.append_section(plan(), "Blocked", f"Blocked on 2026-10-02.\n\n{long}\n\n* one\n* two\n")
        formatted = pq.dprint_formatter(text, pq.plan_path("a"), config)
        self.assertEqual(len(pq.blocked_sections(formatted)), 1)
        remaining, body = pq.remove_blocked(formatted)
        self.assertEqual(pq.blocked_sections(remaining), [])
        self.assertIn("- one", body)
        self.assertEqual(pq.dprint_formatter(remaining, pq.plan_path("a"), config), remaining)


if __name__ == "__main__":
    unittest.main(verbosity=2)
