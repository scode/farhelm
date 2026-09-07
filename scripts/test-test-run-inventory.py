"""Exercise retained-evidence discovery against private temporary filesystem fixtures."""
import json
import os
import pathlib
import tempfile
import unittest
import uuid

from test_run_inventory import Budget, Inventory, InventoryLimit, output_observation, DIRECTORY_FLAGS


def write_run(parent, *, identity=None, code=0, kind="development", output=b""):
    """Publish a minimal complete recorder record with optional real retained output."""
    identity = identity or str(uuid.uuid4())
    root = parent / identity
    root.mkdir(parents=True)
    files = []
    if output:
        (root / "output-head.log").write_bytes(output)
        files.append({"name": "output-head.log", "role": "head", "bytes": len(output)})
    manifest = {
        "schema_version": 1, "run_id": identity, "outcome": "completed",
        "started_at": "2026-09-07T00:00:00Z", "labels": {"kind": kind},
        "child_status": {"raw_returncode": code, "exit_code": code, "signal": None},
        "recorder": {"exit_code": code, "forced_cleanup": False, "cleanup_limit": None, "error": None},
        "source": {"complete": True, "fingerprint_sha256": "a" * 64},
        "tmux": {"mode": "none", "uses_tmux": False},
        "console": {"worker_finished": True, "dropped_or_pending_bytes": 0},
        "output": {"eof_observed": True, "truncated": False, "omitted_bytes": 0,
                   "retained_bytes": len(output), "observed_bytes": len(output), "files_in_read_order": files},
        "test_traces": {"collection": {"collection_complete": True}},
    }
    (root / "manifest.json").write_text(json.dumps(manifest))
    return root


def write_batch(root):
    """Create an ordinary failed-then-passed batch with two distinct retained run identities."""
    batch = root / str(uuid.uuid4())
    batch.mkdir()
    attempts = []
    for number, code in ((1, 100), (2, 0)):
        run = write_run(batch / f"attempt-{number:04d}", code=code, kind="repetition")
        attempts.append({"attempt": number, "status": "failed" if code else "passed",
                         "evidence": str(run.relative_to(batch))})
    index = {"schema_version": 1, "state": "failed", "planned": 2, "started": 2,
             "finished": 2, "not_started": 0, "attempts": attempts}
    (batch / "index.json").write_text(json.dumps(index))
    return batch, index


class InventoryTest(unittest.TestCase):
    """Only supported paths and nonconflicting identities contribute to run denominators."""

    def test_release_export_report_is_present_only_when_regular(self):
        """Release retention flattens the JUnit path; neither layout may follow a link."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            run = write_run(root, kind="release")
            manifest_path = run / "manifest.json"
            manifest = json.loads(manifest_path.read_text())
            manifest["runner"] = {"name": "nextest"}
            manifest_path.write_text(json.dumps(manifest))
            exported = run / "nextest-junit.xml"
            for state in ("missing", "regular", "linked"):
                if state == "regular":
                    exported.write_text("<testsuites/>")
                elif state == "linked":
                    exported.rename(root / "outside.xml")
                    exported.symlink_to(root / "outside.xml")
                inventory = Inventory()
                inventory.directory(run, "run")
                expected = "present_unverified" if state == "regular" else "missing_or_invalid"
                self.assertEqual(inventory.runs[run.name][1]["raw_report_retention"], expected)

    def test_invalid_conflicting_copy_excludes_valid_copy_in_either_order(self):
        """A damaged failure record must not leave its conflicting passing copy authoritative."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            first = write_run(root / "one")
            second = write_run(root / "two", identity=first.name, code=1)
            path = second / "manifest.json"
            manifest = json.loads(path.read_text())
            manifest["recorder"]["exit_code"] = "damaged"
            path.write_text(json.dumps(manifest))
            for order in ((first, second), (second, first)):
                inventory = Inventory()
                for run in order:
                    inventory.directory(run, "run")
                self.assertFalse(inventory.runs)
                self.assertIn(first.name, inventory.conflicts)
                self.assertEqual(inventory.issues["unreadable_or_invalid_manifest"], 1)

    def test_duplicate_retention_is_independent_of_input_order(self):
        """A manifests-only copy cannot hide gaps by following a complete copy in the input list."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            first = write_run(root / "one", output=b"SKIPPED\n")
            second = write_run(root / "two", identity=first.name, output=b"SKIPPED\n")
            (second / "output-head.log").unlink()
            values = []
            for order in ((first, second), (second, first)):
                inventory = Inventory(scan_output=True)
                for path in order:
                    inventory.directory(path, "run")
                self.assertEqual(len(inventory.runs), 1)
                values.append(inventory.runs[first.name][1])
            self.assertEqual(values[0], values[1])
            self.assertEqual(values[0]["output_files"]["retention"], "varies_between_copies")
            self.assertTrue(values[0]["output_files"]["marker_observed"])
            self.assertIn("retained_output_files_unavailable", values[0]["gaps"])

    def test_unindexed_attempt_evidence_is_retained_with_a_schedule_gap(self):
        """A stale batch index must not erase an attempt already published to its private directory."""
        with tempfile.TemporaryDirectory() as temporary:
            batch, _ = write_batch(pathlib.Path(temporary))
            write_run(batch / "attempt-0003", code=1, kind="repetition")
            inventory = Inventory()
            inventory.directory(batch, "batch")
            self.assertEqual(len(inventory.runs), 3)
            self.assertEqual(inventory.batches[batch.name][1]["started"], 2)
            self.assertEqual(inventory.issues["batch_attempt_absent_from_index"], 1)

    def test_duplicate_identity_is_counted_once_and_conflicting_copies_are_excluded(self):
        """Selecting a run twice cannot inflate totals; conflicting copies have no chosen winner."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            first = write_run(root / "one")
            second = write_run(root / "two", identity=first.name)
            inventory = Inventory()
            inventory.directory(first, "run")
            inventory.directory(second, "run")
            self.assertEqual(len(inventory.runs), 1)
            self.assertEqual(inventory.issues["run_duplicate_copy"], 1)
            manifest = json.loads((second / "manifest.json").read_text())
            manifest["child_status"].update(raw_returncode=1, exit_code=1)
            manifest["recorder"]["exit_code"] = 1
            (second / "manifest.json").write_text(json.dumps(manifest))
            inventory.directory(second, "run")
            self.assertEqual(inventory.runs, {})
            self.assertEqual(inventory.issues["run_identity_conflict"], 1)

    def test_links_special_files_and_unrelated_json_are_not_followed(self):
        """A manifest name does not authorize reading a linked file or blocking on a FIFO."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            run = write_run(root)
            manifest = run / "manifest.json"
            manifest.rename(root / "unrelated.json")
            manifest.symlink_to(root / "unrelated.json")
            inventory = Inventory()
            inventory.directory(root, "root")
            self.assertFalse(inventory.runs)
            self.assertEqual(inventory.issues["unreadable_or_invalid_manifest"], 1)
            self.assertEqual(inventory.issues["unrecognized_root_entry"], 1)
            manifest.unlink()
            os.mkfifo(manifest)
            inventory.directory(run, "run")
            self.assertEqual(inventory.issues["unreadable_or_invalid_manifest"], 2)

    def test_declared_limits_stop_discovery_instead_of_silently_reporting_a_subset(self):
        """Entry, byte and time exhaustion escape as explicit partial-discovery conditions."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            write_run(root)
            for budget in (Budget(entries=0), Budget(byte_limit=1), Budget(seconds=0)):
                with self.subTest(budget=budget), self.assertRaises(InventoryLimit):
                    Inventory(budget=budget).directory(root, "root")

    def test_batches_keep_failed_and_passed_attempts_and_check_references(self):
        """Index paths are compared as data while actual runs come only from constructed directories."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            batch, index = write_batch(root)
            index["attempts"][0]["evidence"] = "../../private/manifest.json"
            (batch / "index.json").write_text(json.dumps(index))
            inventory = Inventory()
            inventory.directory(root, "root")
            self.assertEqual(len(inventory.batches), 1)
            self.assertEqual(sorted(item[1]["child"] for item in inventory.runs.values()), ["failed", "passed"])
            self.assertEqual(inventory.issues["batch_evidence_reference_mismatch"], 1)
            self.assertEqual(inventory.batches[batch.name][1]["not_started"], 0)

    def test_missing_attempts_and_impossible_batch_counters_remain_gaps(self):
        """A retained index cannot manufacture missing manifests or extra completed attempts."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            batch, index = write_batch(root)
            path = next((batch / "attempt-0002").iterdir()) / "manifest.json"
            path.unlink()
            inventory = Inventory()
            inventory.directory(batch, "batch")
            self.assertEqual(len(inventory.runs), 1)
            self.assertEqual(inventory.issues["missing_manifest"], 1)
            index["finished"] = True
            (batch / "index.json").write_text(json.dumps(index))
            invalid = Inventory()
            invalid.directory(batch, "batch")
            self.assertEqual(invalid.issues["unreadable_or_invalid_batch_index"], 1)
            self.assertFalse(invalid.batches)


class OutputObservationTest(unittest.TestCase):
    """Marker absence requires a complete scan; rolling-output gaps cannot invent a marker."""

    def test_positive_marker_survives_missing_or_invalid_later_chunk(self):
        """Expired tail evidence invalidates absence claims, not a marker already read from the head."""
        with tempfile.TemporaryDirectory() as temporary:
            run = write_run(pathlib.Path(temporary), output=b"SKIPPED\n")
            path = run / "manifest.json"
            manifest = json.loads(path.read_text())
            manifest["output"]["files_in_read_order"].append(
                {"name": "output-tail-000000.log", "role": "tail", "bytes": 1})
            manifest["output"].update(retained_bytes=9, observed_bytes=9)
            path.write_text(json.dumps(manifest))
            for state in ("missing", "wrong_size"):
                if state == "wrong_size":
                    (run / "output-tail-000000.log").write_bytes(b"too large")
                inventory = Inventory(scan_output=True)
                inventory.directory(run, "run")
                output = inventory.runs[run.name][1]["output_files"]
                self.assertEqual(output["scan"], "partial")
                self.assertTrue(output["marker_observed"])
                self.assertEqual(output["retention"], "missing_or_invalid")

    def test_marker_observation_is_optional_and_not_a_test_count(self):
        """Default reporting never interprets raw text; explicit scanning only reports marker presence."""
        with tempfile.TemporaryDirectory() as temporary:
            run = write_run(pathlib.Path(temporary), output=b"SKIPPED fixture substrate unavailable\n")
            for scan, expected in ((False, None), (True, True)):
                inventory = Inventory(scan_output=scan)
                inventory.directory(run, "run")
                summary = inventory.runs[run.name][1]
                self.assertEqual(summary["output_files"]["marker_observed"], expected)
                self.assertIsNone(summary["report"])

    def test_omitted_bytes_do_not_join_unrelated_marker_fragments(self):
        """The retained head and rolling tail are not necessarily adjacent bytes of output."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "output-head.log").write_bytes(b"SKI")
            (root / "output-tail-000123.log").write_bytes(b"PPED")
            output = {"eof_observed": True, "truncated": True, "omitted_bytes": 10,
                      "retained_bytes": 7, "observed_bytes": 17,
                      "files_in_read_order": [{"name": "output-head.log", "role": "head", "bytes": 3},
                                              {"name": "output-tail-000123.log", "role": "tail", "bytes": 4}]}
            fd = os.open(root, DIRECTORY_FLAGS)
            try:
                result = output_observation(Budget(), fd, output, True)
            finally:
                os.close(fd)
            self.assertEqual(result["scan"], "partial")
            self.assertIsNone(result["marker_observed"])


if __name__ == "__main__":
    unittest.main()
