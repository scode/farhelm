#!/usr/bin/env python3
"""Unit tests for the deflake driver's pure parts: parsing, exclusions, events.

The daemon's process-driving path (jj workspace, recorder invocations, hours of
tests) is exercised by `deflake start --only js-unit` by hand, not here. What
this file pins down is the contract between the checked-in known-flakes.txt,
the recorder's reports, and the event files the agent reads, because a silent
mistake in any of those either re-runs a known flake or hides a real failure.

Run: `python3 deflake/test-deflake.py`
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent


def load_driver():
    """Import the extensionless `deflake` script as a module without running its CLI."""
    previous = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec = importlib.util.spec_from_loader(
            "deflake_driver",
            importlib.machinery.SourceFileLoader("deflake_driver", str(HERE / "bin" / "deflake")),
        )
        module = importlib.util.module_from_spec(spec)
        # dataclasses resolves annotations through sys.modules, so the module
        # must be registered before its body runs.
        sys.modules[spec.name] = module
        spec.loader.exec_module(module)
        return module
    finally:
        sys.dont_write_bytecode = previous


D = load_driver()
TestId = D.TestId


class KnownFlakesFormat(unittest.TestCase):
    """known-flakes.txt is the one place a known flake is machine-readable.

    Each accepted line must round-trip through `TestId.line`, comments and blank
    lines must be ignored, and anything else must be an error rather than a
    silently dropped entry (dropping one would put a known flake back in the
    sweep without anyone noticing).
    """

    def test_round_trip_of_every_kind(self):
        text = "\n".join([
            "# header comment",
            "",
            "nextest farhelm::e2e agent_relay::a_helm_that_dies  # TODO: relay-helm-death",
            "playwright terminal-keys.spec.ts plain Enter sends bare CR  # TODO: enter-cr",
            "phase desktop-smoke",
        ])
        ids = D.parse_known_flakes(text)
        self.assertEqual(ids, [
            TestId("nextest", ("farhelm::e2e", "agent_relay::a_helm_that_dies")),
            TestId("playwright", ("terminal-keys.spec.ts", "plain Enter sends bare CR")),
            TestId("phase", ("desktop-smoke",)),
        ])
        self.assertEqual([TestId.parse(i.line()) for i in ids], ids)

    def test_hash_inside_a_title_is_not_a_comment(self):
        ids = D.parse_known_flakes("playwright feed.spec.ts row #3 stays put  # slug\n")
        self.assertEqual(ids, [TestId("playwright", ("feed.spec.ts", "row #3 stays put"))])

    def test_malformed_lines_are_errors(self):
        for bad in ["nextest onlybinary", "playwright spec.ts", "phase two words", "bogus x y", "nextest"]:
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                D.parse_known_flakes(bad)


class Exclusions(unittest.TestCase):
    """Exclusion argv must name both binary and test, and must vanish when empty.

    An empty `-E` or `--grep-invert` would either be refused by the recorder or,
    worse, select nothing, so "no known flakes" has to mean "no extra argv".
    """

    def test_nextest_filterset_names_binary_and_test(self):
        ids = [TestId("nextest", ("farhelm::e2e", "a::b")), TestId("nextest", ("farhelm-helm", "c::d")),
               TestId("playwright", ("x.spec.ts", "t"))]
        self.assertEqual(D.nextest_exclusion(ids), [
            "-E", "not ((binary_id(=farhelm::e2e) & test(=a::b)) | (binary_id(=farhelm-helm) & test(=c::d)))",
        ])
        self.assertEqual(D.nextest_exclusion([TestId("phase", ("p",))]), [])

    def test_playwright_grep_invert_escapes_titles(self):
        ids = [TestId("playwright", ("feed.spec.ts", "a (b) rename")), TestId("nextest", ("x", "y"))]
        argv = D.playwright_exclusion(ids)
        self.assertEqual(argv[0], "--grep-invert")
        self.assertIn(r"feed\.spec\.ts .*a\ \(b\)\ rename$", argv[1])
        self.assertEqual(D.playwright_exclusion([]), [])


class ReportParsing(unittest.TestCase):
    """Failed cases come from the runners' reports, never from console heuristics.

    Nextest's JUnit names suites by binary id and cases by full path, which is
    exactly the pair the exclusion filterset needs. Playwright's JSON reports
    one entry per project for the same case; the id must be engine-agnostic so
    a flake is excluded from both engines, with the engine kept on the side.
    """

    def test_nextest_junit_failures(self):
        junit = """<?xml version="1.0"?>
<testsuites name="run" tests="3" failures="1" errors="0">
  <testsuite name="farhelm::e2e" tests="2" failures="1" errors="0">
    <testcase name="a::passes" classname="farhelm::e2e"/>
    <testcase name="a::fails" classname="farhelm::e2e"><failure message="boom"/></testcase>
  </testsuite>
  <testsuite name="farhelm-helm" tests="1" failures="0" errors="0">
    <testcase name="b::skipped" classname="farhelm-helm"><skipped/></testcase>
  </testsuite>
</testsuites>"""
        self.assertEqual(D.nextest_failures(junit), [TestId("nextest", ("farhelm::e2e", "a::fails"))])

    def test_playwright_json_failures_merge_engines(self):
        report = {"suites": [{"file": "feed.spec.ts", "suites": [{"title": "d", "specs": [
            {"title": "t1", "file": "feed.spec.ts", "tests": [
                {"projectName": "chromium-feed", "status": "expected", "results": [{"status": "passed"}]},
                {"projectName": "webkit-feed", "status": "unexpected",
                 "results": [{"status": "failed", "error": {"message": "expect failed"}}]},
            ]},
        ]}]}]}
        self.assertEqual(D.playwright_failures(report), [
            (TestId("playwright", ("feed.spec.ts", "t1")), "webkit-feed", "expect failed"),
        ])

    def test_nextest_excerpt_takes_final_reprint(self):
        output = "\n".join([
            "--- STDERR:              farhelm::e2e a::fails ---", "first attempt output",
            "--- STDERR:              farhelm::e2e xa::fails ---", "a different test whose name ends the same",
            "--- STDOUT + STDERR:     farhelm::e2e a::fails ---", "final reprint", "panic here",
            "--- STDOUT:              farhelm::e2e a::other ---", "tail",
        ])
        excerpt = D.nextest_excerpt(output, "a::fails")
        self.assertIn("final reprint", excerpt)
        self.assertIn("panic here", excerpt)
        self.assertNotIn("first attempt", excerpt)
        self.assertNotIn("different test", excerpt)
        self.assertNotIn("tail", excerpt)

    def test_nextest_excerpt_reads_the_status_line_format(self):
        """The pinned nextest (0.9.143) delimits failure output with status lines.

        The first successful eval sweep showed the excerpt falling back to a
        line grep, which repeated the FAIL line and lost the panic message.
        The block must run from the test's own FAIL line to the next status
        line and include its stderr section.
        """
        output = "\n".join([
            "    Starting 103 tests across 1 binary",
            "        FAIL [   0.015s] (  1/103) farhelm-proto deflake_eval::tests::flaky",
            "  stdout ───", "", "    running 1 test", "",
            "  stderr ───", "", "    thread 'deflake_eval::tests::flaky' panicked at src/x.rs:16:9:",
            "    first run fails on purpose", "",
            "        PASS [   0.019s] (  2/103) farhelm-proto io::tests::other",
            "  stdout ───", "    running 1 test",
        ])
        excerpt = D.nextest_excerpt(output, "deflake_eval::tests::flaky")
        self.assertTrue(excerpt.startswith("        FAIL ["))
        self.assertIn("first run fails on purpose", excerpt)
        self.assertNotIn("io::tests::other", excerpt)

    def test_evidence_output_follows_the_manifest_chunk_list(self):
        """The recorder's manifest lists chunks as objects, not names.

        The first eval sweep crashed the daemon on exactly this: treating each
        `files_in_read_order` entry as a path. Objects, bare names, and a
        missing manifest must all read the retained chunks in order.
        """
        with tempfile.TemporaryDirectory() as tmp:
            run = pathlib.Path(tmp)
            (run / "output-head.log").write_text("head ")
            (run / "output-tail-000001.log").write_text("tail")
            (run / "manifest.json").write_text(json.dumps({"output": {"files_in_read_order": [
                {"name": "output-head.log", "bytes": 5, "role": "head"}, "output-tail-000001.log",
            ]}}))
            self.assertEqual(D.read_evidence_output(run), "head tail")
            (run / "manifest.json").unlink()
            self.assertEqual(D.read_evidence_output(run), "head tail")

    def test_evidence_dir_is_the_last_announcement(self):
        text = "noise\ntest-run evidence: /a/b\nmore\ntest-run evidence: /c/d\n"
        self.assertEqual(D.evidence_dir_from_log(text), pathlib.Path("/c/d"))
        self.assertIsNone(D.evidence_dir_from_log("nothing"))


class EventLifecycle(unittest.TestCase):
    """Events are numbered monotonically and stay pending until acked.

    `wait` returns the FIRST pending event, so ordering and the ack marker are
    the whole protocol between daemon and agent; found flakes accumulate in a
    file that later phases read back as exclusions.
    """

    def test_emit_ack_and_found(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = D.RunDir(pathlib.Path(tmp) / "run")
            run.create()
            first = run.emit("failure", phase="p", test="nextest b t")
            second = run.emit("stalled", phase="p")
            self.assertEqual((first, second), ("0001", "0002"))
            self.assertEqual([e["id"] for e in run.unacked()], ["0001", "0002"])
            (run.events / "0001.acked").write_text("x")
            self.assertEqual([e["id"] for e in run.unacked()], ["0002"])
            run.record_found(TestId("nextest", ("b", "t")))
            self.assertEqual(run.found_flakes(), [TestId("nextest", ("b", "t"))])
            self.assertFalse(run.daemon_alive())
            self.assertTrue(run.has_event("stalled"))
            self.assertFalse(run.has_event("daemon-died"))
            # A stale id claim (another writer got there first) moves on to the
            # next number instead of overwriting.
            (run.events / "0003.json").write_text("{}")
            self.assertEqual(run.emit("failure"), "0004")
            data = json.loads((run.events / "0001.json").read_text())
            self.assertEqual(data["kind"], "failure")
            self.assertIn("event 0001: failure", D.format_event(data))


if __name__ == "__main__":
    unittest.main()
