"""Keep historical missing fields and explicit diagnosis confidence distinguishable."""
import unittest
import copy
import uuid

from test_run_summary import BROWSER_COUNTS, flake_summary, manifest_summary, reported_counts


class FlakeSummaryTest(unittest.TestCase):
    """Summaries report supplied metadata without inventing a diagnosis or leaking prose."""

    def test_missing_invalid_and_explicit_unknown_have_distinct_denominators(self):
        """Legacy narrative cause text is not a metadata tag and must not be classified."""
        text = """# Flakes
## 2026-09-01 — private old test
An unknown cause; output not kept. Try on recurrence.
## 2026-09-02 — private new test
An observation.

Class: readiness

Cause: established
## 2026-09-03 — another test
Class: unknown

Cause: unknown
## 2026-09-04 — malformed metadata
Class: invented

Cause: established

A trailing paragraph invalidates the required last-line cause field.
"""
        result = flake_summary(text)
        self.assertEqual(result["dated_entries"], 4)
        self.assertEqual(result["cause"], {"established": 1, "hypothesis": 0, "unknown": 1,
                                           "missing": 1, "invalid": 1})
        self.assertEqual(result["cause_tagged_entries"], 2)
        self.assertEqual(result["class_tagged_entries"], 2)
        self.assertEqual(result["mentions_not_kept"], 1)
        self.assertEqual(result["mentions_on_recurrence"], 1)
        self.assertNotIn("private", str(result))

    def test_date_window_invalid_dates_and_duplicate_tags_are_visible(self):
        """A date filter excludes only established old dates, while malformed dates stay a gap."""
        text = """## 2026-08-31 — old
Cause: established
## 2026-09-01 — selected
Class: readiness
Class: budget
Cause: hypothesis
Cause: established
## 2026-02-30 — invalid date
Cause: established
"""
        result = flake_summary(text, "2026-09-01")
        self.assertEqual(result["dated_entries"], 1)
        self.assertEqual(result["before_since"], 1)
        self.assertEqual(result["invalid_date_headings"], 1)
        self.assertEqual(result["class"]["invalid"], 1)
        self.assertEqual(result["cause"]["invalid"], 1)
        self.assertEqual(result["established_share"]["cause_tagged_denominator"], 0)
        with self.assertRaises(ValueError):
            flake_summary(text, "2026-9-1")


class ReportCountsTest(unittest.TestCase):
    """Reported totals require complete typed evidence, never a generic command's exit code."""

    def test_nextest_counts_require_a_complete_consistent_denominator(self):
        """Missing counts and boolean integers cannot become a zero-failure observation."""
        counts = {"tests": 3, "passed": 1, "failures": 1, "errors": 0, "skipped": 1, "flaky": 0}
        runner = {"name": "nextest", "prepared": True, "report": {"complete": True, "counts": counts}}
        self.assertEqual(reported_counts(runner)["counts"], counts)
        counts["tests"] = True
        self.assertIsNone(reported_counts(runner))
        self.assertIsNone(reported_counts(None))
        self.assertIsNone(reported_counts({"name": "generic", "prepared": True, "report": {"complete": True}}))

    def test_browser_engine_counts_and_expected_failures_remain_distinct(self):
        """A known expected failure is executed coverage, not an unexpected failure or skip."""
        bucket = dict.fromkeys(BROWSER_COUNTS, 0)
        bucket.update(tests=1, failed=1, expected=1, expected_failures=1)
        total = {name: count * 2 for name, count in bucket.items()}
        total["global_errors"] = 0
        runner = {"name": "playwright", "prepared": True, "report": {
            "complete": True, "counts": total, "engines": {"chromium": bucket.copy(), "webkit": bucket.copy()}}}
        self.assertEqual(reported_counts(runner)["counts"]["expected_failures"], 2)
        runner["report"]["engines"]["webkit"]["expected_failures"] = 0
        self.assertIsNone(reported_counts(runner))


class ManifestSummaryTest(unittest.TestCase):
    """Child results remain independent of recorder errors and retained-evidence gaps."""

    def fixture(self):
        """Build a complete generic command record without any private text fields."""
        identity = str(uuid.uuid4())
        return identity, {
            "schema_version": 1, "run_id": identity, "outcome": "completed",
            "started_at": "2026-09-07T00:00:00Z", "labels": {"kind": "development"},
            "child_status": {"raw_returncode": 0, "exit_code": 0, "signal": None},
            "recorder": {"exit_code": 0, "forced_cleanup": False, "cleanup_limit": None, "error": None},
            "source": {"complete": True, "fingerprint_sha256": "a" * 64},
            "tmux": {"mode": "none", "uses_tmux": False},
            "console": {"worker_finished": True, "dropped_or_pending_bytes": 0},
            "output": {"eof_observed": True, "truncated": False, "omitted_bytes": 0},
            "test_traces": {"collection": {"collection_complete": True}},
        }

    def test_generic_success_has_no_inferred_test_count(self):
        """A successful shell command is an attempt, not a measured passing test case."""
        identity, manifest = self.fixture()
        result = manifest_summary(manifest, identity)
        self.assertEqual(result["child"], "passed")
        self.assertEqual(result["report_state"], "unrequested")
        self.assertIsNone(result["report"])
        self.assertEqual(result["gaps"], [])

    def test_failure_and_missing_diagnostics_are_both_preserved(self):
        """Evidence failure must not erase an already observed nonzero child result."""
        identity, manifest = self.fixture()
        manifest["child_status"].update(raw_returncode=100, exit_code=100)
        manifest["recorder"].update(exit_code=100, cleanup_limit="private detail")
        manifest["source"] = None
        manifest["output"]["eof_observed"] = False
        result = manifest_summary(manifest, identity)
        self.assertEqual(result["child"], "failed")
        self.assertIn("source_identity_incomplete", result["gaps"])
        self.assertIn("output_incomplete", result["gaps"])
        self.assertIn("cleanup_or_recorder_error", result["gaps"])
        self.assertNotIn("private detail", str(result))

    def test_running_records_do_not_imply_unobserved_child_results(self):
        """An interrupted writer can leave only an initial record; no outcome is invented."""
        identity, manifest = self.fixture()
        manifest["outcome"] = "running"
        manifest["recorder"]["exit_code"] = None
        manifest["child_status"] = dict.fromkeys(("raw_returncode", "exit_code", "signal"))
        result = manifest_summary(manifest, identity)
        self.assertEqual(result["child"], "unavailable")
        self.assertEqual(result["recorder"], "running")
        self.assertIn("in_progress_record", result["gaps"])

    def test_malformed_status_identity_and_boolean_integers_are_rejected(self):
        """Malformed records cannot smuggle status objects or boolean counts into totals."""
        identity, original = self.fixture()
        for mutate in (
            lambda m: m.update(outcome=[]),
            lambda m: m.update(schema_version=True),
            lambda m: m.update(run_id="foreign"),
            lambda m: m["child_status"].update(raw_returncode=True),
            lambda m: m["child_status"].update(signal=9),
            lambda m: m["recorder"].update(exit_code=False),
        ):
            manifest = copy.deepcopy(original)
            mutate(manifest)
            self.assertIsNone(manifest_summary(manifest, identity))
        original["labels"]["kind"] = ["private"]
        self.assertEqual(manifest_summary(original, identity)["kind"], "unknown")


if __name__ == "__main__":
    unittest.main()
