"""Exercise manual reporting without launching tests or modifying retained evidence."""
import argparse
import importlib.util
import io
import json
import pathlib
import signal
import subprocess
import sys
import tempfile
import unittest
import uuid
from unittest import mock

from test_run_inventory import Budget, Inventory


def load(name, filename):
    """Load hyphenated script entrypoints without invoking their command-line main."""
    spec = importlib.util.spec_from_file_location(name, pathlib.Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


cli = load("summary_cli", "summarize-test-runs.py")
fixtures = load("inventory_fixtures", "test-test-run-inventory.py")


class SummaryCommandTest(unittest.TestCase):
    """Run and ledger denominators stay independent, including incomplete discovery."""

    def test_rejected_uuid_entry_makes_cli_discovery_incomplete(self):
        """A linked or non-directory run identity is rejected evidence, not an unrelated root file."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            ledger = root / "FLAKES.md"
            ledger.write_text("")
            entry = root / str(uuid.uuid4())
            for state in ("linked", "regular"):
                if state == "linked":
                    entry.symlink_to(root, target_is_directory=True)
                else:
                    entry.unlink()
                    entry.write_text("damaged run directory")
                output = io.StringIO()
                with mock.patch.object(sys, "stdout", output):
                    status = cli.main(["--root", str(root), "--flakes", str(ledger)])
                report = json.loads(output.getvalue())
                self.assertEqual(status, 125)
                self.assertFalse(report["discovery"]["complete"])
                self.assertEqual(report["discovery"]["issues"]["rejected_uuid_entry_type"], 1)

    def arguments(self, root, **changes):
        """Keep fixture paths explicit so the command never reads the repository ledger."""
        values = dict(root=[root], run=[], batch=[], flakes=[root / "FLAKES.md"], since=None)
        values.update(changes)
        return argparse.Namespace(**values)

    def test_cli_preserves_failure_then_pass_without_exporting_private_text(self):
        """A later success must not erase its batch's earlier failure or publish raw evidence."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            fixtures.write_batch(root)
            (root / "FLAKES.md").write_text("## 2026-09-07 — private incident title\n\nprivate diagnosis\n")
            before = {str(path.relative_to(root)): path.read_bytes() for path in root.rglob("*") if path.is_file()}
            result = subprocess.run(
                [sys.executable, str(pathlib.Path(cli.__file__)), "--root", str(root),
                 "--flakes", str(root / "FLAKES.md")], capture_output=True, text=True, timeout=5,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            report = json.loads(result.stdout)
            group = report["runs"]["kinds"]["repetition"]
            self.assertEqual(group["child_results"], {"passed": 1, "failed": 1})
            self.assertEqual(group["nonzero_child_share"]["observed_child_denominator"], 2)
            self.assertEqual(report["batches"]["batch_denominator"], 1)
            self.assertEqual(report["flake_ledger"]["dated_entries"], 1)
            self.assertNotIn(temporary, result.stdout)
            self.assertNotIn("private incident", result.stdout)
            self.assertNotIn("private diagnosis", result.stdout)
            after = {str(path.relative_to(root)): path.read_bytes() for path in root.rglob("*") if path.is_file()}
            self.assertEqual(before, after)

    def test_since_excludes_old_and_undated_runs_without_filtering_batch_schedule(self):
        """Batch indexes have no start date; filtering their attempts cannot date the schedule."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            batch, _ = fixtures.write_batch(root)
            manifest_path = next((batch / "attempt-0002").iterdir()) / "manifest.json"
            manifest = json.loads(manifest_path.read_text())
            manifest.pop("started_at")
            manifest_path.write_text(json.dumps(manifest))
            (root / "FLAKES.md").write_text("## 2026-09-07 — old\n\n## 2026-09-08 — retained\n")
            report = cli.report(self.arguments(root, since="2026-09-08"), Inventory())
            self.assertEqual(report["runs"]["selected_records"], 0)
            self.assertEqual(report["runs"]["excluded_before_since"], 1)
            self.assertEqual(report["runs"]["excluded_undated"], 1)
            self.assertEqual(report["batches"]["declared_finished"], 2)
            self.assertEqual(report["flake_ledger"]["dated_entries"], 1)

    def test_active_ledger_and_archive_are_counted_together_without_their_preambles(self):
        """The flake history is split between FLAKES.md and FLAKES_ARCHIVE.md, and the summary counts both.

        Each file opens with a preamble that quotes the field names. If the
        files were simply joined, one file's last entry would run on into the
        next file's preamble, so its `Cause:` line would no longer be the
        body's last and would count as invalid. An empty heading ahead of
        the archive entry is malformed but still ends the preceding body.
        A bare heading also ends the final entry in a CRLF archive.
        Both default discovery and repeated --flakes must keep the tags.
        """
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            entry = "## {date} — {title}\n\nobservation\n\nClass: {cls}\n\nCause: {cause}\n"
            preamble = "# Ledger\n\nEntries end with `Class: <class>` and `Cause: <confidence>`.\n\n"
            (root / "FLAKES.md").write_text(
                preamble + entry.format(date="2026-11-09", title="active", cls="substrate", cause="hypothesis"))
            (root / "FLAKES_ARCHIVE.md").write_text(
                preamble + "## \n\nA heading with a space.\n\n"
                + entry.format(date="2026-09-07", title="closed", cls="readiness", cause="established")
                + "\n##\n\nA bare heading.\n",
                newline="\r\n",
            )
            other = root / "other-checkout"
            other.mkdir()
            explicit = []
            # No canonical sibling can stand in for either explicit path.
            for name in ("FLAKES.md", "FLAKES_ARCHIVE.md"):
                copied = other / f"selected-{name}"
                copied.write_text((root / name).read_text(), newline="\n")
                explicit.extend(["--flakes", str(copied)])
            for checkout, arguments in ((root, []), (other, explicit)):
                output = io.StringIO()
                with self.subTest(arguments=arguments), mock.patch.object(
                    cli, "__file__", str(checkout / "scripts/summarize-test-runs.py")
                ), mock.patch.object(sys, "stdout", output):
                    self.assertEqual(cli.main(arguments), 0)
                    ledger = json.loads(output.getvalue())["flake_ledger"]
                    self.assertEqual(ledger["state"], "read")
                    self.assertEqual(ledger["dated_entries"], 2)
                    self.assertEqual(ledger["invalid_date_headings"], 2)
                    self.assertEqual((ledger["first_date"], ledger["last_date"]), ("2026-09-07", "2026-11-09"))
                    self.assertEqual(ledger["cause"]["hypothesis"], 1)
                    self.assertEqual(ledger["cause"]["established"], 1)
                    self.assertEqual(ledger["cause"]["invalid"] + ledger["cause"]["missing"], 0)
                    self.assertEqual(ledger["class"]["substrate"], 1)
                    self.assertEqual(ledger["class"]["readiness"], 1)

    def test_missing_ledger_and_exhausted_discovery_are_explicit(self):
        """A partial inventory remains usable but cannot present its denominator as complete."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            fixtures.write_run(root)
            ledger = root / "FLAKES.md"
            ledger.write_text("")
            missing = cli.report(self.arguments(root, flakes=[ledger, root / "unavailable.md"]), Inventory())
            self.assertFalse(missing["discovery"]["complete"])
            self.assertEqual(missing["flake_ledger"]["state"], "missing_or_invalid")
            self.assertEqual(missing["discovery"]["unique_valid_run_records"], 1)
            self.assertEqual(missing["runs"]["selected_records"], 1)
            bounded = cli.report(self.arguments(root), Inventory(budget=Budget(entries=0)))
            self.assertFalse(bounded["discovery"]["complete"])
            self.assertEqual(bounded["discovery"]["limit_reached"], "entries")

    def test_cancellation_during_publication_preserves_signal_and_handlers(self):
        """Cancellation after discovery still controls exit status, even while stdout flushes."""
        with tempfile.TemporaryDirectory() as temporary:
            ledger = pathlib.Path(temporary) / "FLAKES.md"
            ledger.write_text("")
            prior = {number: signal.getsignal(number) for number in (signal.SIGINT, signal.SIGTERM)}

            class CancelOnFlush(io.StringIO):
                def flush(self):
                    signal.getsignal(signal.SIGTERM)(signal.SIGTERM, None)

            output = CancelOnFlush()
            with mock.patch.object(sys, "stdout", output):
                status = cli.main(["--flakes", str(ledger)])
            self.assertEqual(status, 143)
            self.assertTrue(json.loads(output.getvalue())["discovery"]["complete"])
            for number, handler in prior.items():
                self.assertIs(signal.getsignal(number), handler)


if __name__ == "__main__":
    unittest.main()
