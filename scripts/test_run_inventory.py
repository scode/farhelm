"""Read bounded recorder inventories without following artifact-selected paths.

Only canonical run directories, fixed manifests and constructed batch attempt
paths are interpreted. Raw text remains private; readers return portable status
categories and counts. Missing files are gaps rather than empty observations.
"""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import re
import stat
import time

from test_run_nextest import read_regular
from test_run_summary import canonical_uuid, manifest_summary

DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
MANIFEST_LIMIT = 2 * 1024 * 1024
CHUNK_LIMIT = 1024 * 1024


class InventoryLimit(Exception):
    """Discovery stopped at a declared resource boundary; its denominator is partial."""


class Budget:
    """Bound cumulative discovery, bytes read and runnable elapsed time."""

    def __init__(self, *, entries=10000, byte_limit=64 * 1024 * 1024, seconds=30):
        self.entries_left = entries
        self.bytes_left = byte_limit
        self.deadline = time.monotonic() + seconds
        self.signal = None

    def cancel(self, number, _frame):
        """Record cancellation without doing filesystem work from the signal handler."""
        if self.signal is None:
            self.signal = number

    def check(self):
        """Stop between filesystem operations; kernel-blocked I/O is outside this deadline."""
        if self.signal is not None:
            raise InventoryLimit("signal")
        if time.monotonic() >= self.deadline:
            raise InventoryLimit("time")

    def entries(self, fd):
        """Enumerate through a held directory descriptor with a global entry budget."""
        with os.scandir(fd) as stream:
            for entry in stream:
                self.check()
                if self.entries_left <= 0:
                    raise InventoryLimit("entries")
                self.entries_left -= 1
                yield entry

    def read(self, fd, name, limit):
        """Read one regular fixed-name file, charging actual bytes against the global cap."""
        self.check()
        info = os.stat(name, dir_fd=fd, follow_symlinks=False)
        if not stat.S_ISREG(info.st_mode) or info.st_size > limit:
            raise ValueError("invalid file type or size")
        if info.st_size > self.bytes_left:
            raise InventoryLimit("bytes")
        allowance = min(limit, self.bytes_left)
        if allowance <= 0:
            raise InventoryLimit("bytes")
        # Charge failures conservatively: a file can grow after stat and the
        # bounded reader may consume its allowance before rejecting it.
        self.bytes_left -= allowance
        data = read_regular(pathlib.Path(name), allowance, parent_fd=fd)
        self.bytes_left += allowance - len(data)
        self.check()
        return data


def read_json(budget, fd, name):
    """Parse bounded JSON while retaining its digest for duplicate-identity checks."""
    data = budget.read(fd, name, MANIFEST_LIMIT)
    value = json.loads(data)
    if not isinstance(value, dict):
        raise ValueError("JSON object required")
    return value, hashlib.sha256(data).hexdigest()


def regular_beneath(fd, parts, limit):
    """Check presence of a fixed artifact through no-follow descriptors, without claiming content validation."""
    held = []
    try:
        parent = fd
        for part in parts[:-1]:
            parent = os.open(part, DIRECTORY_FLAGS, dir_fd=parent)
            held.append(parent)
        leaf = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=parent)
        held.append(leaf)
        info = os.fstat(leaf)
        return stat.S_ISREG(info.st_mode) and 0 <= info.st_size <= limit
    except OSError:
        return False
    finally:
        for opened in reversed(held):
            os.close(opened)


def output_observation(budget, fd, output, scan):
    """Inspect only declared recorder chunks; SKIPPED is a marker, never a test count.

    Even a complete scan cannot prove platform-specific coverage ran. Missing,
    truncated or unscanned output must not be reported as absence of markers.
    """
    result = {"retention": "missing_or_invalid", "scan": "partial" if scan else "not_requested", "marker_observed": None}
    if not isinstance(output, dict):
        return result
    files = output.get("files_in_read_order")
    if not isinstance(files, list) or len(files) > 8:
        return result
    names = set()
    retained = 0
    marker = False
    carry = b""
    previous_tail = None
    for position, item in enumerate(files):
        if not isinstance(item, dict):
            return result
        name, size = item.get("name"), item.get("bytes")
        if (not isinstance(name, str) or not re.fullmatch(r"output-(?:head|tail-[0-9]{6,12})\.log", name)
                or name in names or type(size) is not int or not 0 <= size <= CHUNK_LIMIT):
            return result
        names.add(name)
        if name == "output-head.log":
            if position != 0 or item.get("role") != "head":
                return result
        else:
            sequence = int(name.removeprefix("output-tail-").removesuffix(".log"))
            if item.get("role") != "tail" or previous_tail is not None and sequence != previous_tail + 1:
                return result
            if previous_tail is None and sequence != 0:
                carry = b""  # The rolling tail can have a gap after the retained head.
            previous_tail = sequence
        budget.check()
        try:
            info = os.stat(name, dir_fd=fd, follow_symlinks=False)
            if not stat.S_ISREG(info.st_mode) or info.st_size != size:
                return result
            if scan:
                data = budget.read(fd, name, CHUNK_LIMIT)
                marker = marker or b"SKIPPED" in carry + data
                if marker:
                    result["marker_observed"] = True
                carry = data[-6:]
        except (OSError, ValueError):
            return result
        retained += size
    if type(output.get("retained_bytes")) is not int or output["retained_bytes"] != retained:
        return result
    result["retention"] = "present_unverified"
    if scan:
        complete = (output.get("eof_observed") is True and output.get("truncated") is False
                    and type(output.get("omitted_bytes")) is int and output["omitted_bytes"] == 0
                    and type(output.get("observed_bytes")) is int and output["observed_bytes"] == retained)
        result.update(scan="complete" if complete else "partial", marker_observed=marker if complete or marker else None)
    return result


def batch_summary(index):
    """Validate declared scheduling separately from actual retained run evidence."""
    if type(index.get("schema_version")) is not int or index["schema_version"] != 1:
        return None
    state = index.get("state")
    if not isinstance(state, str) or state not in {"running", "completed", "failed", "incomplete", "cancelled"}:
        return None
    numbers = [index.get(key) for key in ("planned", "started", "finished", "not_started")]
    if any(type(value) is not int or not 0 <= value <= 1000 for value in numbers):
        return None
    planned, started, finished, not_started = numbers
    attempts = index.get("attempts")
    if (planned < 1 or not 0 <= finished <= started <= planned or started - finished > 1
            or not_started != planned - started or not isinstance(attempts, list) or len(attempts) != finished
            or state in {"completed", "failed"} and finished != planned):
        return None
    for number, attempt in enumerate(attempts, 1):
        if not isinstance(attempt, dict) or type(attempt.get("attempt")) is not int or attempt["attempt"] != number:
            return None
    return {"state": state, "planned": planned, "started": started, "finished": finished, "not_started": not_started}


class Inventory:
    """Collect unique portable run records and disclose discovery and identity conflicts."""

    def __init__(self, *, budget=None, scan_output=False):
        self.budget = budget or Budget()
        self.scan_output = scan_output
        self.runs = {}
        self.run_digests = {}
        self.conflicts = set()
        self.batches = {}
        self.batch_conflicts = set()
        self.issues = {}

    def issue(self, name):
        """Count a fixed diagnostic category without exporting private paths or errors."""
        self.issues[name] = self.issues.get(name, 0) + 1

    def unique(self, records, conflicts, identity, digest, value, prefix):
        """Exclude every conflicting copy rather than selecting a convenient authoritative one."""
        if identity in conflicts:
            self.issue(prefix + "_conflicting_copy")
        elif identity in records:
            if records[identity][0] == digest:
                self.issue(prefix + "_duplicate_copy")
                if prefix == "run":
                    self.merge_retention(records[identity][1], value)
            else:
                conflicts.add(identity)
                del records[identity]
                self.issue(prefix + "_identity_conflict")
        else:
            records[identity] = (digest, value)

    def merge_retention(self, retained, copy):
        """Keep copy-dependent artifact observations explicit without counting a run twice.

        A manifest may survive in both a full archive and a manifests-only
        export. Input order must not select which copy's gaps are disclosed.
        Marker presence in either copy remains an observation; differing scan
        results cannot establish marker absence across the selected copies.
        """
        retained["gaps"] = sorted(set(retained["gaps"]) | set(copy["gaps"]))
        if retained["raw_report_retention"] != copy["raw_report_retention"]:
            retained["raw_report_retention"] = "varies_between_copies"
        first, second = retained["output_files"], copy["output_files"]
        if first != second:
            retained["gaps"] = sorted(set(retained["gaps"]) | {"retained_copies_differ"})
            retained["output_files"] = {
                "retention": "varies_between_copies",
                "scan": "partial",
                "marker_observed": True if first["marker_observed"] is True or second["marker_observed"] is True else None,
            }

    def run(self, fd, identity):
        """Read one fixed manifest and inspect only known raw artifact locations."""
        try:
            manifest, digest = read_json(self.budget, fd, "manifest.json")
            # Identity belongs to the retained directory, even when a readable
            # copy has damaged status fields. Validate conflicts before those
            # fields can disqualify only the inconvenient copy.
            previous = self.run_digests.setdefault(identity, digest)
            if previous != digest:
                self.conflicts.add(identity)
                self.runs.pop(identity, None)
                self.issue("run_identity_conflict")
            summary = manifest_summary(manifest, identity)
            if summary is None:
                raise ValueError("invalid manifest")
            summary["output_files"] = output_observation(self.budget, fd, manifest.get("output"), self.scan_output)
            if summary["output_files"]["retention"] == "missing_or_invalid":
                summary["gaps"].append("retained_output_files_unavailable")
            runner = manifest.get("runner")
            name = runner.get("name") if isinstance(runner, dict) else None
            if name == "nextest":
                # Release collection flattens the fixed nextest report name;
                # both layouts are producer-owned, never manifest-selected.
                present = (regular_beneath(fd, ("nextest", "default", "junit.xml"), 16 * 1024 * 1024)
                           or regular_beneath(fd, ("nextest-junit.xml",), 16 * 1024 * 1024))
            elif name == "playwright":
                present = (regular_beneath(fd, ("playwright.json",), 16 * 1024 * 1024)
                           and regular_beneath(fd, ("playwright-policy.json",), 64 * 1024))
            else:
                present = None
            summary["raw_report_retention"] = "unrequested" if present is None else "present_unverified" if present else "missing_or_invalid"
            if present is False:
                summary["gaps"].append("raw_report_files_unavailable")
            self.unique(self.runs, self.conflicts, identity, digest, summary, "run")
            return identity
        except FileNotFoundError:
            self.issue("missing_manifest")
        except (OSError, ValueError, RecursionError):
            self.issue("unreadable_or_invalid_manifest")
        return None

    def batch(self, fd, identity):
        """Discover constructed attempt paths, checking index references without following them."""
        try:
            index, digest = read_json(self.budget, fd, "index.json")
            summary = batch_summary(index)
            if summary is None:
                raise ValueError("invalid index")
        except FileNotFoundError:
            self.issue("missing_batch_index")
            return
        except (OSError, ValueError, RecursionError):
            self.issue("unreadable_or_invalid_batch_index")
            return
        self.unique(self.batches, self.batch_conflicts, identity, digest, summary, "batch")
        numbers = set(range(1, summary["started"] + 1))
        # A recorder can die after publishing evidence but before updating its
        # index. Discover bounded canonical attempt names as well as declared
        # ones, so that stale scheduling metadata cannot erase an actual run.
        for entry in self.budget.entries(fd):
            if entry.name == "index.json":
                continue
            match = re.fullmatch(r"attempt-([0-9]{4})", entry.name)
            number = int(match.group(1)) if match else 0
            if not 1 <= number <= 1000:
                self.issue("unrecognized_batch_entry")
                continue
            if number not in numbers:
                self.issue("batch_attempt_absent_from_index")
                numbers.add(number)
        for number in sorted(numbers):
            name = f"attempt-{number:04d}"
            attempt_fd = None
            try:
                self.budget.check()
                attempt_fd = os.open(name, DIRECTORY_FLAGS, dir_fd=fd)
                entries = []
                for entry in self.budget.entries(attempt_fd):
                    if entries or not canonical_uuid(entry.name) or not entry.is_dir(follow_symlinks=False):
                        raise ValueError("ambiguous attempt evidence")
                    entries.append(entry.name)
                if not entries:
                    raise ValueError("missing attempt evidence")
                child = entries[0]
                if number <= summary["finished"]:
                    if index["attempts"][number - 1].get("evidence") != f"{name}/{child}":
                        self.issue("batch_evidence_reference_mismatch")
                child_fd = os.open(child, DIRECTORY_FLAGS, dir_fd=attempt_fd)
                try:
                    observed = self.run(child_fd, child)
                    if observed in self.runs and number <= summary["finished"]:
                        actual = self.runs[observed][1]
                        declared = index["attempts"][number - 1].get("status")
                        if (not isinstance(declared, str) or declared not in {"passed", "failed", "incomplete", "cancelled"}
                                or actual["kind"] != "repetition"
                                or declared == "passed" and (actual["child"] != "passed" or actual["recorder"] != "passed")
                                or declared == "failed" and actual["child"] != "failed"):
                            self.issue("batch_attempt_status_mismatch")
                finally:
                    os.close(child_fd)
            except (OSError, ValueError):
                self.issue("missing_or_invalid_batch_attempt")
            finally:
                if attempt_fd is not None:
                    os.close(attempt_fd)

    def directory(self, path, kind):
        """Inspect an explicit run/batch, or canonical UUID children of one root directory."""
        fd = None
        try:
            self.budget.check()
            fd = os.open(path, DIRECTORY_FLAGS)
            if kind != "root":
                identity = pathlib.Path(path).name
                if not canonical_uuid(identity):
                    raise ValueError("canonical UUID directory required")
                (self.run if kind == "run" else self.batch)(fd, identity)
                return
            for entry in self.budget.entries(fd):
                if not canonical_uuid(entry.name):
                    self.issue("unrecognized_root_entry")
                    continue
                if not entry.is_dir(follow_symlinks=False):
                    self.issue("rejected_uuid_entry_type")
                    continue
                child_fd = os.open(entry.name, DIRECTORY_FLAGS, dir_fd=fd)
                try:
                    # The fixed index name distinguishes batches. A linked or
                    # malformed index is still a batch error, never permission
                    # to reinterpret a directory as an ordinary run.
                    try:
                        os.stat("index.json", dir_fd=child_fd, follow_symlinks=False)
                    except FileNotFoundError:
                        self.run(child_fd, entry.name)
                    else:
                        self.batch(child_fd, entry.name)
                finally:
                    os.close(child_fd)
        except (OSError, ValueError):
            self.issue("unreadable_or_invalid_input_directory")
        finally:
            if fd is not None:
                os.close(fd)
