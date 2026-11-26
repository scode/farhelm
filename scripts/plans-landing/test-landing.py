#!/usr/bin/env python3
"""Tests for the landing monitor's round tooling (`scripts/plans-landing/landing.py`).

These cover the parts that can be wrong without any jj or GitHub call failing: the monitor log edits a resume relies
on, the round files every later step reads, where both live in the agent records, the records pushes that must come
before each GitHub-changing step (and stop the command when they fail), the claim check that guards every merge, the
landing notes' assembly rules and the review page's escaping. The jj and gh sequences themselves are exercised only by
real landings; a fake of either would test the fake. Seconds, no network.

The records tool and the queue script are replaced by small stand-ins written into a temporary directory: the records
command is injected through `landing.main`'s `records` argument, and the queue stand-in sits at the temporary
checkout's `scripts/plans-queue.py`, where `landing.py` runs the queue from. Both append one line per call to a shared
calls file, which is how the tests see the order of pushes and queue verbs. No test runs cargo, jj or gh against real
state, and none changes the test process's environment.
"""

from __future__ import annotations

import argparse
import contextlib
import importlib.util
import io
import pathlib
import re
import sys
import tempfile
import unittest

_spec = importlib.util.spec_from_file_location("landing", pathlib.Path(__file__).with_name("landing.py"))
landing = importlib.util.module_from_spec(_spec)
# Dataclass processing can resolve postponed annotations through the module registry during import.
sys.modules[_spec.name] = landing
_spec.loader.exec_module(landing)

LOG = """## Status

running, last checked 2026-01-01T00:00Z

## Monitor state

- Landing claim in progress: none.

## History

- 2026-01-01T00:00Z: started."""


# A stand-in for `cargo xtask records`: `path <name>` prints `<root>/<name>` (created), `push` prints a commit line and
# copies the monitor log as it is at that moment to `<root>/pushed-log-<k>.md` (k counts pushes from 1), which is what
# the push would have carried. A `fail-path` or `fail-push` file in the root makes that command exit 1, as the real
# tool does on a refusal.
RECORDS_STUB = """
import pathlib, sys
root = pathlib.Path(sys.argv[1])
args = sys.argv[2:]
with (root.parent / "calls.txt").open("a", encoding="utf-8") as f:
    f.write("records " + " ".join(args) + "\\n")
if args[0] == "path":
    if (root / "fail-path").exists():
        print("refusing: local edits overlap remote changes", file=sys.stderr)
        sys.exit(1)
    d = root / args[1]
    d.mkdir(parents=True, exist_ok=True)
    print(d)
elif args[0] == "push":
    if (root / "fail-push").exists():
        print("push rejected", file=sys.stderr)
        sys.exit(1)
    k = len(list(root.glob("pushed-log-*.md"))) + 1
    log = root / "landing" / "log.md"
    if log.exists():
        (root / f"pushed-log-{k}.md").write_text(log.read_text(encoding="utf-8"), encoding="utf-8")
    print("pushed 0123abcd")
"""

# A stand-in for the checkout's queue script: records the verb and its arguments (after `--repo o/n`), prints a commit
# id, and exits with the code in `queue-exit` (default 0).
QUEUE_STUB = """
import pathlib, sys
top = pathlib.Path(__file__).resolve().parents[2]
with (top / "calls.txt").open("a", encoding="utf-8") as f:
    f.write("queue " + " ".join(sys.argv[3:]) + "\\n")
exit_file = top / "queue-exit"
print("c0ffee0011223344")
sys.exit(int(exit_file.read_text()) if exit_file.exists() else 0)
"""


class RecordsCase(unittest.TestCase):
    """A temporary checkout with the queue stand-in, a records stand-in, and the monitor log in its `landing` record.

    Layout under the temporary directory: `checkout/` (with `scripts/plans-queue.py`), `records/<name>/` for each
    record the stand-in has been asked for, `calls.txt` (one line per records or queue call, in order) and
    `records-stub.py`.
    """

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.top = pathlib.Path(self._tmp.name)
        self.checkout = self.top / "checkout"
        (self.checkout / "scripts").mkdir(parents=True)
        (self.checkout / "scripts" / "plans-queue.py").write_text(QUEUE_STUB, encoding="utf-8")
        self.records_root = self.top / "records"
        self.records_root.mkdir()
        stub = self.top / "records-stub.py"
        stub.write_text(RECORDS_STUB, encoding="utf-8")
        self.records = [sys.executable, str(stub), str(self.records_root)]
        self.log = self.records_root / "landing" / "log.md"
        self.log.parent.mkdir()
        self.log.write_text(LOG + "\n", encoding="utf-8")

    def tearDown(self):
        self._tmp.cleanup()

    def round_dir(self, number: int = 1) -> pathlib.Path:
        d = self.records_root / "round" / str(number)
        d.mkdir(parents=True, exist_ok=True)
        return d

    def run_landing(self, *argv: str, stdin: str = "") -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            saved, sys.stdin = sys.stdin, io.StringIO(stdin)
            try:
                code = landing.main(["--checkout", str(self.checkout), "--repo", "o/n", *argv], records=self.records)
            finally:
                sys.stdin = saved
        return code, out.getvalue(), err.getvalue()

    def calls(self) -> list[str]:
        path = self.top / "calls.txt"
        return path.read_text(encoding="utf-8").splitlines() if path.exists() else []

    def make_fail(self, what: str) -> None:
        """Make every later records `path` or `push` (`what`) exit 1. Not named `fail`: that is unittest's own."""
        (self.records_root / f"fail-{what}").write_text("", encoding="utf-8")


class MonitorLog(RecordsCase):
    """The monitor log is the only state a resumed monitor has, so its edits must land on the right lines or refuse."""

    def test_status_and_entry_share_one_stamp(self):
        text = landing.with_history_entry(landing.with_status_refreshed(LOG, "T"), "two\n  lines", "T")
        self.assertIn("running, last checked T\n", text)
        self.assertTrue(text.endswith("- 2026-01-01T00:00Z: started.\n- T: two lines\n"))

    def test_stopped_monitor_and_empty_entry_are_refused(self):
        """A stopped monitor's Status must not be silently flipped back to running by a log entry."""
        with self.assertRaises(landing.Refusal):
            landing.with_status_refreshed(LOG.replace("running, last checked", "stopped"), "T")
        with self.assertRaises(landing.Refusal):
            landing.with_history_entry(LOG, " \n", "T")

    def test_claim_line_is_replaced_once_or_refused(self):
        text = landing.with_claim_line(LOG, "slug abc123 starting.")
        self.assertIn("- Landing claim in progress: slug abc123 starting.\n", text)
        self.assertNotIn("none.", text)
        with self.assertRaises(landing.Refusal):
            landing.with_claim_line("## Status\n", "none.")

    def test_log_command_writes_the_landing_record_and_pushes_nothing(self):
        """The log is `log.md` in the directory `records path landing` prints, never the old file beside the checkout,
        and `log` only writes it locally: a push belongs before a GitHub change, and a log entry is not one.
        Specified: `log` appends stdin and exits 0, leaves a file with the old name beside the checkout untouched and
        calls the records tool for `path landing` alone; on a log without a running Status line it exits 1 and writes
        nothing."""
        decoy = self.top / "farhelm-plans-monitor-log.md"
        decoy.write_text(LOG, encoding="utf-8")
        code, out, _ = self.run_landing("log", stdin="an entry")
        self.assertEqual(code, 0)
        self.assertTrue(self.log.read_text(encoding="utf-8").endswith(": an entry\n"))
        self.assertEqual(decoy.read_text(encoding="utf-8"), LOG)
        self.assertEqual(self.calls(), ["records path landing"])
        self.log.write_text("## Status\n\nstopped x\n", encoding="utf-8")
        code, _, _ = self.run_landing("log", stdin="another")
        self.assertEqual(code, 1)
        self.assertEqual(self.log.read_text(encoding="utf-8"), "## Status\n\nstopped x\n")

    def test_missing_log_and_failed_path_are_refusals(self):
        """A records `path` that fails (local edits overlapping remote ones, say) or a landing record with no log must
        stop the command with exit 1 and the tool's message, not a traceback, and never create a log from nothing."""
        self.log.unlink()
        code, _, err = self.run_landing("log", stdin="x")
        self.assertEqual(code, 1)
        self.assertIn("does not exist", err)
        self.assertFalse(self.log.exists())
        self.make_fail("path")
        code, _, err = self.run_landing("log", stdin="x")
        self.assertEqual(code, 1)
        self.assertIn("records path landing failed", err)
        self.assertIn("local edits overlap", err)


class PushesBeforeGitHub(RecordsCase):
    """The records rule: push right before every GitHub-changing step, and stop the command if the push fails.

    Pushing first is what keeps the remote copy of the monitor's reasoning from ever lagging what GitHub shows, and a
    failed push must stop the flow before the step it guards (the design's "a failed push stops the flow"). These
    tests check the order of records and queue calls, and that a failing push leaves the queue untouched and the log's
    claim line where a resume can settle it.
    """

    def setUp(self):
        super().setUp()
        rd = self.round_dir(1)
        (rd / "prs.txt").write_text("a 11:plan/a/01-x 12:plan/a/02-y\n", encoding="utf-8")
        (rd / "notes").mkdir()
        (rd / "notes" / "a.final.md").write_text("Landed.\n", encoding="utf-8")

    def claim_line(self) -> str:
        return next(line for line in self.log.read_text(encoding="utf-8").splitlines()
                    if line.startswith("- Landing claim in progress:"))

    def test_claim_pushes_the_log_and_round_before_start_landing(self):
        """Specified: the `starting` line is written, then `landing` and `round/1` are pushed together (the pushed log
        already holds that line), then `queue start-landing` runs, and only then is the line rewritten to `landing`."""
        code, out, err = self.run_landing("claim", "--round", "1", "a")
        self.assertEqual(code, 0, err)
        calls = self.calls()
        push = next(i for i, c in enumerate(calls) if c.startswith("records push"))
        start = next(i for i, c in enumerate(calls) if c.startswith("queue start-landing a --claim"))
        self.assertLess(push, start)
        self.assertTrue(calls[push].startswith("records push landing round/1 -m "), calls[push])
        claim = out.split("CLAIM a ")[1].split()[0]
        # What the push carried: the log as it was before start-landing, already naming the claim as `starting`.
        pushed = (self.records_root / "pushed-log-1.md").read_text(encoding="utf-8")
        self.assertIn(f"- Landing claim in progress: a {claim} starting.\n", pushed)
        self.assertIn(f"a {claim} (landing; claim commit c0ffee00)", self.claim_line())

    def test_a_failed_push_stops_claim_before_the_queue(self):
        """A failed push must not be followed by `start-landing`. The line keeps `starting` with a claim id main never
        saw, which the resume rule clears (landing.md, "Starting and resuming")."""
        self.make_fail("push")
        code, _, err = self.run_landing("claim", "--round", "1", "a")
        self.assertEqual(code, 1)
        self.assertIn("records push landing round/1 failed", err)
        self.assertFalse(any(c.startswith("queue") for c in self.calls()), self.calls())
        self.assertTrue(self.claim_line().endswith(" starting."), self.claim_line())

    def test_a_plan_no_longer_complete_is_skipped_after_the_push(self):
        """Exit 10 from `start-landing` clears the claim line and exits 10. The push still came first: the claim verb is
        a GitHub change whatever its outcome, so its reasoning must reach the records before it runs."""
        (self.top / "queue-exit").write_text("10", encoding="utf-8")
        code, _, _ = self.run_landing("claim", "--round", "1", "a")
        self.assertEqual(code, landing.EXIT_SKIP)
        calls = self.calls()
        push = next(i for i, c in enumerate(calls) if c.startswith("records push landing round/1"))
        start = next(i for i, c in enumerate(calls) if c.startswith("queue start-landing a --claim"))
        self.assertLess(push, start, calls)
        self.assertEqual(self.claim_line(), "- Landing claim in progress: none.")

    def test_record_pushes_before_queue_landed(self):
        """Specified: `recording` is written and pushed before `queue landed`, and a failed push leaves `recording` on
        the line and the queue uncalled, so a resume settles it from `queue history`."""
        code, _, err = self.run_landing("record", "--round", "1", "a", "abc123", "11,12")
        self.assertEqual(code, 0, err)
        calls = self.calls()
        push = next(i for i, c in enumerate(calls) if c.startswith("records push landing round/1"))
        landed = next(i for i, c in enumerate(calls) if c.startswith("queue landed a --claim abc123"))
        self.assertLess(push, landed)
        self.assertEqual(self.claim_line(), "- Landing claim in progress: none.")
        (self.top / "calls.txt").unlink()
        self.make_fail("push")
        code, _, err = self.run_landing("record", "--round", "1", "a", "abc123", "11,12")
        self.assertEqual(code, 1)
        self.assertFalse(any(c.startswith("queue") for c in self.calls()), self.calls())
        self.assertIn("abc123 recording.", self.claim_line())

    def test_land_pushes_before_touching_the_first_pr(self):
        """`land` opens each PR's turn with a push. With the push failing, the command stops naming the records push
        (not a jj or gh failure, which is what a later step failing would show) and says the claim is still held."""
        self.make_fail("push")
        code, _, err = self.run_landing("land", "--round", "1", "a", "abc123")
        self.assertEqual(code, 1)
        self.assertIn("records push landing round/1 failed", err)
        self.assertIn("claim abc123 still held", err)
        self.assertEqual([c for c in self.calls() if c.startswith("records push")][0].split(" -m ")[1],
                         "round 1: land a #11")

    def test_land_docs_prs_pushes_the_landing_record_first(self):
        """`land-docs-prs` changes GitHub from its first step on, so it pushes the `landing` record (no round applies)
        before anything else, and a failed push stops it there."""
        msg = self.top / "msg.txt"
        msg.write_text("docs: x\n", encoding="utf-8")
        self.make_fail("push")
        code, _, err = self.run_landing("land-docs-prs", "--jjstack", str(self.top / "jjstack"), f"todo-x={msg}")
        self.assertEqual(code, 1)
        self.assertIn("records push landing failed", err)
        pushes = [c for c in self.calls() if c.startswith("records push")]
        self.assertEqual(pushes, ["records push landing -m landing: open and land todo-x"])


class RoundRecord(RecordsCase):
    """The round's text files live in its `round/<n>` record; bulk stays out of it."""

    def test_round_start_refuses_a_round_already_started(self):
        """A record that already holds `plans.txt` is a started round; starting it again would replace the plans a
        resumed round carries on from. The refusal comes before the working copy is even looked at, says to resume the
        round from its record, and writes no `r3 started` entry (which would make the log claim a second start)."""
        (self.round_dir(3) / "plans.txt").write_text("a plan/a/01-x\n", encoding="utf-8")
        before = self.log.read_text(encoding="utf-8")
        code, _, err = self.run_landing("round-start", "--round", "3")
        self.assertEqual(code, 1)
        self.assertIn("round 3 was already started", err)
        self.assertIn("Resume that round from its record", err)
        self.assertEqual(self.calls(), ["records path round/3"])
        self.assertEqual(self.log.read_text(encoding="utf-8"), before)

    def test_round_start_writes_its_entry_then_the_files(self):
        """The `r<n> started, head <short sha>` entry is the round number's source (next round = newest rN + 1), so it
        must exist whenever the round's `plans.txt` does. Specified: after `write_round_start` the log ends with that
        entry (eight characters of the head), `base.txt` holds the full head, `plans.txt` the plan lines, `notes/`
        exists; and with a log that cannot take the entry (missing), nothing is written to the record at all."""
        ctx = landing.Ctx(self.checkout, "o/n", self.records)
        head = "0123456789abcdef0123456789abcdef01234567"
        landing.write_round_start(ctx, 4, head, ["a plan/a/01-x"])
        self.assertRegex(self.log.read_text(encoding="utf-8"),
                         r"\n- \d{4}-\d\d-\d\dT\d\d:\d\dZ: r4 started, head 01234567\n$")
        rd = self.records_root / "round" / "4"
        self.assertEqual((rd / "base.txt").read_text(encoding="utf-8"), head + "\n")
        self.assertEqual((rd / "plans.txt").read_text(encoding="utf-8"), "a plan/a/01-x\n")
        self.assertTrue((rd / "notes").is_dir())
        self.log.unlink()
        with self.assertRaises(landing.Refusal):
            landing.write_round_start(ctx, 5, head, [])
        self.assertEqual(sorted(p.name for p in (self.records_root / "round" / "5").iterdir()), [])

    def test_round_entries_keep_the_shape_readers_parse(self):
        """The plan-status skill finds the round and whether it is under way with
        `^- (\\S+): (?:- )?r(\\d+) (start|started|end|ended|done)\\b`; both entries must match it once written into
        the log (the start as under way, the end as finished), and both keep their exact text, which the landing
        instructions name."""
        parser = re.compile(r"^- (\S+): (?:- )?r(\d+) (start|started|end|ended|done)\b")
        start = landing.with_history_entry("", landing.round_entry(12, "started", "abcdef0123"), "T").strip()
        self.assertEqual(start, "- T: r12 started, head abcdef01")
        self.assertEqual(parser.match(start).group(2, 3), ("12", "started"))
        self.assertEqual(landing.round_entry(12, "ended"), "r12 ended")
        end = landing.with_history_entry("", landing.round_entry(12, "ended"), "T").strip()
        self.assertEqual(parser.match(end).group(2, 3), ("12", "ended"))
        with self.assertRaises(landing.Refusal):
            landing.round_entry(12, "started")

    def test_diffs_refuse_a_scratch_directory_inside_the_record(self):
        """Diffs are bulk that every push would carry; `--scratch` inside the record is refused before any diff."""
        rd = self.round_dir(2)
        (rd / "plans.txt").write_text("a plan/a/01-x\n", encoding="utf-8")
        for scratch in (rd, rd / "sub"):
            code, _, err = self.run_landing("diffs", "--round", "2", "--oldest-base", "x", "--scratch", str(scratch))
            self.assertEqual(code, 1)
            self.assertIn("never go into a record", err)
        self.assertFalse((rd / "diffs").exists())

    def test_round_numbers_are_what_the_records_tool_accepts(self):
        """`round/<n>` takes a positive decimal without leading zeros; anything else is a usage error (exit 2)."""
        for bad in ("0", "01", "-1", "x"):
            with self.assertRaises(argparse.ArgumentTypeError):
                landing.round_number(bad)
        self.assertEqual(landing.round_number("412"), 412)
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as caught:
            landing.build_parser().parse_args(["verify", "--round", "01", "top"])
        self.assertEqual(caught.exception.code, 2)


class RoundFiles(unittest.TestCase):
    """Every step after `round-start` reads these files back, often in a resumed session."""

    def write(self, name: str, text: str) -> pathlib.Path:
        path = pathlib.Path(self.tmp.name, name)
        path.write_text(text, encoding="utf-8")
        return path

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()

    def tearDown(self):
        self.tmp.cleanup()

    def test_plans_keep_queue_and_stack_order(self):
        path = self.write("plans.txt", "b plan/b/01-x plan/b/02-y\n\na plan/a/01-z\n")
        self.assertEqual(landing.read_plans(path), [("b", ["plan/b/01-x", "plan/b/02-y"]), ("a", ["plan/a/01-z"])])

    def test_plan_without_bookmarks_is_refused(self):
        with self.assertRaises(landing.Refusal):
            landing.read_plans(self.write("plans.txt", "a\n"))

    def test_pairs_and_tested_commits(self):
        pairs = landing.read_pairs(self.write("prs.txt", "a 11:plan/a/01-x 12:plan/a/02-y\n"))
        self.assertEqual(pairs, {"a": [("11", "plan/a/01-x"), ("12", "plan/a/02-y")]})
        tested = self.write("tested_commits.txt", "c1 plan/a/01-x*\nc2 plan/a/02-y\n")
        self.assertEqual(landing.tested_commit(tested, "plan/a/01-x"), "c1")
        self.assertEqual(landing.tested_commit(tested, "plan/a/02-y"), "c2")
        with self.assertRaises(landing.Refusal):
            landing.tested_commit(tested, "plan/a/01")


class ClaimCheck(unittest.TestCase):
    """The guard before every push and merge: a released or foreign claim must never pass."""

    STATUS = {"plans": [
        {"slug": "foo-bar", "state": "landing", "claim": "abc123"},
        {"slug": "foo", "state": "complete", "claim": None},
    ]}

    def test_only_this_plan_and_claim_in_landing(self):
        self.assertTrue(landing.claim_held(self.STATUS, "foo-bar", "abc123"))
        self.assertFalse(landing.claim_held(self.STATUS, "foo-bar", "abc124"))
        # A slug that is a prefix of the claimed plan's must not match, as a line-prefix grep would.
        self.assertFalse(landing.claim_held(self.STATUS, "foo", "abc123"))


class Notes(unittest.TestCase):
    """The queue script inserts landing notes as written, so these rules are the last check before main."""

    def test_one_placeholder_exactly(self):
        self.assertEqual(landing.fill_checks("a\nCHECKS_PLACEHOLDER\nb\n", "checks\n"), "a\nchecks\nb\n")
        for body in ("no placeholder", "CHECKS_PLACEHOLDER CHECKS_PLACEHOLDER"):
            with self.assertRaises(landing.Refusal):
                landing.fill_checks(body, "checks")

    def test_long_lines_except_lone_code_spans(self):
        long_prose = "x " * 61
        long_span = "`" + "y" * 130 + "`."
        self.assertEqual(landing.long_lines(f"short\n{long_prose}\n{long_span}\n"), [2])


class ReviewPage(unittest.TestCase):
    def test_report_and_summary_text_is_escaped(self):
        """Reports are executor-written text shown on a page the maintainer acts on; none of it may become markup."""
        plans = [{"slug": "a", "prs": [7], "summary": "<b>s</b>", "done": "d&d",
                  "leftovers": [{"text": "<i>x</i>", "decision_needed": True}]}]
        page = landing.review_page(plans, {"a": "<script>r</script>"}, "o/n", "running", "3", "")
        self.assertNotIn("<script>", page)
        self.assertNotIn("<b>s</b>", page)
        self.assertIn("&lt;script&gt;", page)
        self.assertIn("<strong>Decision needed:</strong> &lt;i&gt;x&lt;/i&gt;", page)
        self.assertIn('href="https://github.com/o/n/pull/7/changes"', page)
        self.assertIn('data-pick="a"', page)


class Repo(unittest.TestCase):
    def test_github_remotes(self):
        self.assertEqual(landing.parse_github_repo("https://github.com/o/n.git"), "o/n")
        self.assertEqual(landing.parse_github_repo("git@github.com:o/n.x.git"), "o/n.x")
        self.assertIsNone(landing.parse_github_repo("https://example.com/o/n"))


if __name__ == "__main__":
    unittest.main()
