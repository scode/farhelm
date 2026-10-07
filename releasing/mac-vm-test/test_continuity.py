"""Prove that the Linux oracle observes live progress and rejects a dead process."""

from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

from continuity import require_progress, sample


@unittest.skipUnless(sys.platform == 'linux', 'requires the actual Linux /proc substrate')
class ContinuityTests(unittest.TestCase):
    """Use a real owned child so stale output cannot accidentally satisfy the oracle."""

    def setUp(self):
        """Start one workload and wait for its live identity and first complete line."""
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name) / 'counter'
        self.process = subprocess.Popen([sys.executable, str(Path(__file__).with_name('continuity.py')),
                                         'run', str(self.directory)], stdout=subprocess.DEVNULL,
                                        stderr=subprocess.DEVNULL)
        self.addCleanup(self.stop)
        self.before = self.wait_sample(lambda value: value['counter'] >= 0)
        self.assertEqual(self.before['pid'], self.process.pid)
        self.assertIsNone(self.process.poll())

    def stop(self):
        """Reap only the child this fixture owns, preserving numeric-PID ownership."""
        if self.process.poll() is None:
            self.process.terminate()
        self.process.wait(timeout=5)

    def wait_sample(self, predicate):
        """Bound observation polling and distinguish a dead fixture from slow progress."""
        deadline = time.monotonic() + 5
        last = None
        while time.monotonic() < deadline:
            self.assertIsNone(self.process.poll(), 'owned workload exited before observation')
            try:
                last = sample(self.directory)
                if predicate(last):
                    return last
            except (FileNotFoundError, RuntimeError):
                pass
            # Observe file readiness/progress; elapsed delay never establishes it.
            time.sleep(0.02)
        self.fail(f'live workload failed to reach observation boundary: {last}')

    def test_live_original_process_advances(self):
        """Independent samples must show progress from the same original kernel process."""
        after = self.wait_sample(lambda value: value['counter'] > self.before['counter'])
        require_progress(self.before, after)
        self.assertEqual(after['pid'], self.process.pid)

    def test_stopped_process_does_not_pass_from_retained_output(self):
        """Retained counter bytes cannot establish continuity after the process is reaped."""
        self.assertTrue((self.directory / 'counter.log').is_file())
        self.stop()
        self.assertIsNotNone(self.process.returncode)
        with self.assertRaisesRegex(RuntimeError, 'stopped'):
            sample(self.directory)

    def test_advanced_output_with_replacement_identity_is_refused(self):
        """A larger counter alone must not authorize a replacement process as continuity."""
        replacement = dict(self.before, start_ticks=self.before['start_ticks'] + 1,
                           counter=self.before['counter'] + 10)
        with self.assertRaisesRegex(RuntimeError, 'identity changed'):
            require_progress(self.before, replacement)


if __name__ == '__main__':
    unittest.main()
