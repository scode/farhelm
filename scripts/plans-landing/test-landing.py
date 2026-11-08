#!/usr/bin/env python3
"""Tests for the landing monitor's round tooling (`scripts/plans-landing/landing.py`).

These cover the parts that can be wrong without any jj or GitHub call failing: the monitor log edits a resume relies
on, the round files every later step reads, the claim check that guards every merge, the landing notes' assembly
rules and the review page's escaping. The jj and gh sequences themselves are exercised only by real landings; a fake
of either would test the fake. Seconds, no network.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import pathlib
import sys
import tempfile
import unittest

_spec = importlib.util.spec_from_file_location("landing", pathlib.Path(__file__).with_name("landing.py"))
landing = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(landing)

LOG = """## Status

running, last checked 2026-01-01T00:00Z

## Monitor state

- Landing claim in progress: none.

## History

- 2026-01-01T00:00Z: started."""


class MonitorLog(unittest.TestCase):
    """The monitor log is the only state a resumed monitor has, so its edits must land on the right lines or refuse."""

    def test_log_sits_beside_the_checkout(self):
        """The same place `queue status` reads the landing claim's quiet time from (the checkout's parent)."""
        self.assertEqual(landing.monitor_log_path(pathlib.Path("/x/checkout")),
                         pathlib.Path("/x/farhelm-plans-monitor-log.md"))

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

    def test_log_command_end_to_end(self):
        """`log` appends stdin and exits 0; on a log without a running Status line it exits 1 and writes nothing."""
        with tempfile.TemporaryDirectory() as tmp:
            checkout = pathlib.Path(tmp, "checkout")
            checkout.mkdir()
            log = pathlib.Path(tmp, "farhelm-plans-monitor-log.md")
            for text, want in ((LOG, 0), ("## Status\n\nstopped x\n", 1)):
                log.write_text(text, encoding="utf-8")
                with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                    stdin, sys.stdin = sys.stdin, io.StringIO("an entry")
                    try:
                        code = landing.main(["--checkout", str(checkout), "--repo", "o/n", "log"])
                    finally:
                        sys.stdin = stdin
                self.assertEqual(code, want)
                self.assertEqual(log.read_text(encoding="utf-8").endswith(": an entry\n"), want == 0)


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
