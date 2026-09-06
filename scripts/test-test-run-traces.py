#!/usr/bin/env python3
"""Contracts for fixed-layout trace export, independent of test verdicts."""

from __future__ import annotations

import errno
import hashlib
import io
import json
import os
import pathlib
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

# These tests inspect source evidence too; leave the checkout free of an
# interpreter-dependent cache while preserving the invoking process's setting.
_previous_bytecode_setting = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    import test_run_traces as traces
finally:
    sys.dont_write_bytecode = _previous_bytecode_setting
    del _previous_bytecode_setting


class TraceCollectionTest(unittest.TestCase):
    """Keep every fixture in a private directory with explicit descriptor ownership."""

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="trace-collection-")
        self.base = pathlib.Path(self.temporary.name)
        self.root, self.root_fd = traces.create_run_root(self.base)
        self.archive = self.base / "traces.tar"

    def tearDown(self):
        os.close(self.root_fd)
        self.temporary.cleanup()

    def slot(self, index=0):
        """Create a complete retained slot whose JSON may intentionally be partial."""

        slot = self.root / f"slot-{index:03d}"
        slot.mkdir(mode=0o700)
        for name, _limit in traces.FILES:
            (slot / name).write_bytes(b'{"partial":')
        return slot

    def members(self):
        """Read the produced archive without extracting paths onto the filesystem."""

        with tarfile.open(self.archive) as archive:
            return {member.name: archive.extractfile(member).read() for member in archive}

    def test_empty_success_has_no_archive(self):
        """Cleaned successful captures leave no payload and no inferred test outcome."""

        result = traces.collect(self.root_fd, self.archive)
        self.assertTrue(result["collection_complete"])
        self.assertEqual(result["slots_observed"], 0)
        self.assertIsNone(result["archive"])
        self.assertFalse(self.archive.exists())
        self.assertNotIn("test_outcome", result)

    def test_fixed_names_keep_partial_json_and_ignore_unknown_state(self):
        """An incomplete trace stays useful while unrelated names never enter the archive."""

        slot = self.slot(127)
        (slot / "credentials").write_bytes(b"never upload")
        self.slot(128)
        (self.root / "unrelated").mkdir()
        result = traces.collect(self.root_fd, self.archive)
        expected = {f"slot-127/{name}": b'{"partial":' for name, _ in traces.FILES}
        self.assertEqual(self.members(), expected)
        self.assertEqual(result["slots_observed"], 1)
        self.assertTrue(result["collection_complete"])
        self.assertEqual(stat.S_IMODE(self.archive.stat().st_mode), 0o600)
        for entry in result["files"]:
            self.assertEqual(entry["sha256"], hashlib.sha256(expected[entry["path"]]).hexdigest())

    def test_file_budget_and_missing_file_are_explicit(self):
        """Oversized files retain only the allowed prefix; missing files remain visible loss."""

        slot = self.slot()
        (slot / "metadata.json").write_bytes(b"x" * 5000)
        (slot / "tail-2.jsonl").unlink()
        result = traces.collect(self.root_fd, self.archive)
        self.assertFalse(result["collection_complete"])
        self.assertEqual(self.members()["slot-000/metadata.json"], b"x" * 4096)
        kinds = {entry["kind"] for entry in result["errors"]}
        self.assertIn("file_limit", kinds)
        self.assertIn("FileNotFoundError", kinds)

    def test_links_fifo_and_directory_are_never_read(self):
        """Fixed names do not authorize following links or blocking on a FIFO."""

        slot = self.slot()
        outside = self.base / "outside"
        outside.write_bytes(b"private unrelated content")
        (slot / "head.jsonl").unlink()
        (slot / "head.jsonl").symlink_to(outside)
        (slot / "tail-0.jsonl").unlink()
        os.mkfifo(slot / "tail-0.jsonl")
        (slot / "tail-1.jsonl").unlink()
        (slot / "tail-1.jsonl").mkdir()
        (self.root / "slot-001").symlink_to(slot, target_is_directory=True)
        result = traces.collect(self.root_fd, self.archive)
        self.assertFalse(result["collection_complete"])
        self.assertEqual(set(self.members()), {"slot-000/metadata.json", "slot-000/tail-2.jsonl"})

    def test_held_root_survives_path_replacement(self):
        """Collection remains on the original directory if its visible name is replaced."""

        self.slot()
        self.root.rename(self.base / "original")
        self.root.mkdir(mode=0o700)
        self.slot().joinpath("metadata.json").write_bytes(b"replacement")
        traces.collect(self.root_fd, self.archive)
        self.assertEqual(self.members()["slot-000/metadata.json"], b'{"partial":')

    def test_final_root_link_with_slash_or_dot_is_rejected(self):
        """Path normalization must not let a final directory link bypass nofollow."""

        link = self.base / "link"
        link.symlink_to(self.root, target_is_directory=True)
        for suffix in ("", "/", "/."):
            with self.subTest(suffix=suffix), self.assertRaises(OSError):
                traces.open_private_directory(pathlib.Path(str(link) + suffix))

    def test_existing_archive_is_not_overwritten(self):
        """Recovery cannot silently replace an earlier export, even with a fresh source."""

        self.slot()
        self.archive.write_bytes(b"earlier evidence")
        result = traces.collect(self.root_fd, self.archive)
        self.assertFalse(result["collection_complete"])
        self.assertIsNone(result["archive"])
        self.assertEqual(self.archive.read_bytes(), b"earlier evidence")

    def test_read_fault_retains_prefix_while_archive_output_is_available(self):
        """Read faults preserve bytes when output remains possible under the collection budget."""

        with mock.patch.object(traces.os, "read", side_effect=[b"part", OSError(errno.EIO, "fixture")]):
            content, kind, code = traces._read_prefix(-1, 64, float("inf"))
        self.assertEqual((content, kind, code), (b"part", "read_error", errno.EIO))
        with mock.patch.object(traces.os, "read", return_value=b"part"), \
                mock.patch.object(traces.time, "monotonic", side_effect=[0, 2]):
            content, kind, code = traces._read_prefix(-1, 64, 1)
        self.assertEqual((content, kind, code), (b"part", "deadline", None))

        self.slot()
        with mock.patch.object(traces, "_read_prefix", return_value=(b"part", "read_error", errno.EIO)):
            result = traces.collect(self.root_fd, self.archive)
        self.assertFalse(result["collection_complete"])
        self.assertEqual(set(self.members().values()), {b"part"})

    def test_deadline_after_read_reports_unarchived_prefix(self):
        """A read prefix cannot be promised in the archive after its shared deadline expires."""

        self.slot()
        with mock.patch.object(traces.time, "monotonic", side_effect=[0, 0, 0, 11]), \
                mock.patch.object(traces, "_read_prefix", return_value=(b"part", "deadline", None)):
            result = traces.collect(self.root_fd, self.archive)
        self.assertFalse(result["collection_complete"])
        self.assertEqual(result["files"], [])
        self.assertEqual(self.archive.stat().st_size, 0)
        self.assertEqual((self.root / "slot-000/head.jsonl").read_bytes(), b'{"partial":')

    def test_recovery_observation_cannot_replace_original_archive_claims(self):
        """A changed raw trace exports under a distinct name with independent root provenance."""

        slot = self.slot()
        original = traces.collect(self.root_fd, self.archive)
        original_bytes = self.archive.read_bytes()
        (slot / "head.jsonl").write_bytes(b"later observation")
        recovered = self.base / "recovered-traces.tar"
        process = subprocess.run(
            [sys.executable, str(pathlib.Path(traces.__file__)), str(self.root), str(recovered)],
            capture_output=True, check=True, timeout=15,
        )
        recovery = json.loads(process.stdout)
        self.assertEqual(original["archive"], "traces.tar")
        self.assertEqual(recovery["archive"], "recovered-traces.tar")
        self.assertEqual(self.archive.read_bytes(), original_bytes)
        self.assertFalse(recovery["provenance"]["earlier_archive_exported"])
        info = os.fstat(self.root_fd)
        self.assertEqual(recovery["provenance"]["root_identity"], {"device": info.st_dev, "inode": info.st_ino})
        with tarfile.open(recovered) as archive:
            self.assertEqual(archive.extractfile("slot-000/head.jsonl").read(), b"later observation")

    def test_archive_write_failure_latches_before_later_members_or_close_writes(self):
        """A partial member write ends export; closing must not append misleading terminators."""

        self.slot()
        real_open = traces.tarfile.open
        writes = []

        def open_faulting_archive(*args, **kwargs):
            archive = real_open(*args, **kwargs)
            real_write = archive.fileobj.write

            def write(data):
                writes.append(len(data))
                if len(writes) == 2:
                    real_write(data[:3])
                    raise OSError(errno.ENOSPC, "fixture archive full")
                if len(writes) > 2:
                    raise AssertionError("write retried after archive failure")
                return real_write(data)

            archive.fileobj.write = write
            return archive

        with mock.patch.object(traces.tarfile, "open", side_effect=open_faulting_archive):
            result = traces.collect(self.root_fd, self.archive)
        self.assertFalse(result["collection_complete"])
        self.assertEqual(len(writes), 2)
        self.assertEqual(result["files"], [])
        self.assertEqual(self.archive.stat().st_size, 515)

    def test_short_writes_are_completed_and_zero_progress_is_an_error(self):
        """Tarfile's full-buffer assumption must survive legal short descriptor writes."""

        class ShortWriter(io.BytesIO):
            def write(self, data):
                return super().write(data[:3])

        raw = ShortWriter()
        output = traces._ArchiveOutput(raw, float("inf"))
        self.assertEqual(output.write(b"whole record"), 12)
        self.assertEqual(raw.getvalue(), b"whole record")
        with mock.patch.object(raw, "write", return_value=0), self.assertRaises(OSError):
            output.write(b"lost")
        with mock.patch.object(raw, "write", side_effect=InterruptedError), \
                mock.patch.object(traces.time, "monotonic", side_effect=[0, 2]), \
                self.assertRaises(TimeoutError):
            traces._ArchiveOutput(raw, 1).write(b"interrupted")

    def test_error_records_and_deadline_are_bounded(self):
        """A malformed root cannot grow the manifest or keep scanning after its deadline."""

        for index in range(traces.SLOT_COUNT):
            (self.root / f"slot-{index:03d}").mkdir(mode=0o700)
        result = traces.collect(self.root_fd, self.archive)
        self.assertEqual(len(result["errors"]), traces.ERROR_LIMIT)
        self.assertEqual(result["errors_omitted"], traces.SLOT_COUNT * len(traces.FILES) - traces.ERROR_LIMIT)
        with mock.patch.object(traces.time, "monotonic", side_effect=[0, 11]):
            expired = traces.collect(self.root_fd, self.archive)
        self.assertFalse(expired["collection_complete"])
        self.assertEqual(expired["slots_observed"], 0)


if __name__ == "__main__":
    unittest.main()
