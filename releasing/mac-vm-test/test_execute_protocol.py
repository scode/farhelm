"""Exercise transport, stop acknowledgement and collection as distinct host boundaries."""

import json
import subprocess
import unittest

from guest_workflow import CleanupPending, execute
from test_guest_tester import task


class Controller:
    """Model transport independently from the executor's observable lifetime state.

A failed Tart call can leave the executor running. Collection is an independent
operation and cannot manufacture the missing stopped acknowledgement.
"""

    def __init__(self, failure=None, cancel_ok=True):
        self.failure = failure
        self.cancel_ok = cancel_ok
        self.phase = 'running'
        self.calls = []

    def owned(self, role):
        """Return an explicit owned role, with no real guest or private binding."""
        return 'owned-' + role

    def mutate(self, argv, detail, **kwargs):
        """Distinguish staging, executor launch and cancellation transport outcomes."""
        if '-i' in argv:
            self.calls.append('stage')
            return subprocess.CompletedProcess(argv, 0, b'/fixture/attempt\n')
        if argv[-2] == '-c' and 'cancel.json' in argv[-1]:
            self.calls.append('cancel')
            if not self.cancel_ok:
                raise subprocess.CalledProcessError(1, argv)
            self.phase = 'stopped'
        else:
            self.calls.append('execute')
            if self.failure == 'transport':
                raise subprocess.CalledProcessError(1, argv)
            if self.failure == 'timeout':
                raise subprocess.TimeoutExpired(argv, 30)
            self.phase = 'stopped'
        return subprocess.CompletedProcess(argv, 0, b'')

    def command(self, *argv, **kwargs):
        """Report the independent state oracle rather than the preceding exit code."""
        self.calls.append('probe')
        return subprocess.CompletedProcess(argv, 0, json.dumps({'attempt': 'attempt', 'phase': self.phase}).encode())

    def collect(self, role, directory, label):
        """Make collection order observable; these tests export no files or credentials."""
        self.calls.append('collect')


class ExecuteProtocolTests(unittest.TestCase):
    """Require actual termination observation before treating transport loss as cleanup."""

    def test_stopped_probe_precedes_collection(self):
        """A clean CLI exit alone cannot grant collection or cleanup authority."""
        controller = Controller()
        self.assertEqual(execute(controller, task(), {'guest_tester.py': '# public fixture'}), 'tester-attempt')
        self.assertEqual(controller.calls, ['stage', 'execute', 'probe', 'collect'])

    def test_transport_failure_with_live_executor_does_not_collect(self):
        """Nonzero Tart transport is not proof that the independent guest driver exited."""
        controller = Controller(failure='transport')
        self.assertEqual(controller.phase, 'running')
        with self.assertRaises(CleanupPending):
            execute(controller, task(), {'guest_tester.py': '# public fixture'})
        self.assertEqual(controller.calls, ['stage', 'execute', 'probe'])

    def test_timeout_requires_cancel_and_stopped_probe_before_collection(self):
        """A deadline may export diagnostics only after its owned cancellation is acknowledged."""
        controller = Controller(failure='timeout')
        execute(controller, task(), {'guest_tester.py': '# public fixture'})
        self.assertEqual(controller.calls, ['stage', 'execute', 'cancel', 'probe', 'collect'])

    def test_missing_cancel_acknowledgement_stays_cleanup_pending(self):
        """A cancellation request cannot release the fence when its peer gives no acknowledgement."""
        controller = Controller(failure='timeout', cancel_ok=False)
        with self.assertRaises(CleanupPending):
            execute(controller, task(), {'guest_tester.py': '# public fixture'})
        self.assertEqual(controller.calls, ['stage', 'execute', 'cancel'])
        self.assertEqual(controller.phase, 'running')


if __name__ == '__main__':
    unittest.main()
