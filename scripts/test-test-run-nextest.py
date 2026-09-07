#!/usr/bin/env python3
"""Check runner policy and incomplete-report handling without compiling Rust."""

import pathlib
import runpy
import sys
import tempfile
import unittest
from unittest import mock

# These checks must not create source-tree bytecode that changes the next
# recorder invocation's source fingerprint.
previous_bytecode_setting = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    import test_run_nextest as nextest
finally:
    sys.dont_write_bytecode = previous_bytecode_setting


class NextestEvidenceTest(unittest.TestCase):
    """Keep selection separate from execution policy and count only real report evidence."""

    def test_selection_cannot_override_runner_policy(self):
        """Alternate spelling and passthrough must not bypass fixed concurrency or retries."""

        for options in (
            ["--retries", "3"], ["--test-threads=99"], ["-j99"], ["--profile", "other"],
            ["--config-file=x"], ["--tool-config-file", "x:y"], ["--user-config-file=default"],
            ["--", "--ignored"], ["--config", "runner='other'"], ["--no-tests", "pass"],
            ["-p"], ["--package="], ["--features", "--retries=4"],
        ):
            with self.subTest(options=options), self.assertRaises(ValueError):
                nextest.selection_args(["cargo", "nextest", "run", *options])
        selected = ["--workspace", "--exclude=farhelm-desktop", "--features", "desktop", "-E", "test(=a)"]
        self.assertEqual(nextest.selection_args(["cargo", "nextest", "run", "--locked", *selected]), selected)

    def report(self, content):
        """Inspect one private report fixture through the production bounded reader."""

        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            report = root / "nextest" / "default" / "junit.xml"
            report.parent.mkdir(parents=True)
            if content is not None:
                report.write_bytes(content)
            return nextest.collect(root)

    def test_selected_out_tests_stay_separate_from_passes(self):
        """A narrow green run must retain its unexercised denominator and its failures."""

        report = self.report(b'<testsuites tests="4" skipped="1" failures="1" errors="0" uuid="fixture">'
                             b'<testsuite tests="4" skipped="1" failures="1" errors="0">'
                             b'<testcase name="pass"/><testcase name="skip"><skipped/></testcase>'
                             b'<testcase name="fail"><failure/></testcase>'
                             b'<testcase name="flaky"><flakyFailure/></testcase></testsuite></testsuites>')
        self.assertTrue(report["complete"])
        self.assertEqual(report["counts"], {"tests": 4, "passed": 2, "skipped": 1,
                                            "failures": 1, "errors": 0, "flaky": 1})

    def test_missing_partial_or_contradictory_reports_are_incomplete(self):
        """Absent evidence, invalid XML and forged totals never become zero-test passes."""

        for content in (
            None, b"", b"<testsuites", b"<different/>",
            b'<testsuites tests="2" skipped="0" failures="0" errors="0"><testsuite><testcase/></testsuite></testsuites>',
            b'<testsuites tests="1" skipped="0" failures="0" errors="0">'
            b'<testsuite tests="2" skipped="0" failures="1" errors="0"><testcase/></testsuite></testsuites>',
            b'<testsuites tests="2" skipped="1" failures="1" errors="0"><testsuite>'
            b'<testcase><skipped/><failure/></testcase><testcase/></testsuite></testsuites>',
            b'<!DOCTYPE testsuites [<!ENTITY e "expansion">]><testsuites/>',
            '<!DOCTYPE testsuites [<!ENTITY e "expansion">]><testsuites/>'.encode("utf-16"),
        ):
            with self.subTest(content=content):
                result = self.report(content)
                self.assertFalse(result["complete"])
                self.assertNotIn("counts", result)

    def test_cleanup_verdict_rejects_expired_holders(self):
        """A delayed checker must not call voluntary lease expiry successful forced cleanup."""

        module = runpy.run_path(str(pathlib.Path(__file__).with_name("test-nextest-cleanup.py")))
        check = module["assert_forced_cleanup"]
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            with mock.patch.dict(check.__globals__, same_live_process=lambda _: False):
                check(root, [{}])
                (root / "child.voluntary").touch()
                with self.assertRaisesRegex(AssertionError, "voluntarily"):
                    check(root, [{}])

    def test_reader_refuses_links_and_oversize_files(self):
        """Evidence collection must not follow a replaced report or read beyond its cap."""

        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            real = root / "real"
            real.write_bytes(b"12345")
            link = root / "link"
            link.symlink_to(real)
            with self.assertRaises(OSError):
                nextest.read_regular(link, 5)
            with self.assertRaises(ValueError):
                nextest.read_regular(real, 4)
            self.assertEqual(nextest.read_regular(real, 5), b"12345")


if __name__ == "__main__":
    unittest.main(verbosity=2)
