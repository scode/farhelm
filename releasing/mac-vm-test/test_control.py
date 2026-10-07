"""Pin the cleanup fences before this harness can touch a maintainer's VM.

These fixtures inject a fake Tart runner and use private temporary files. They
must never change the test process's environment or invoke a real VM mutation.
The bring-up run separately exercises actual Tart and guest readiness.
"""

import json
from pathlib import Path
import subprocess
import tempfile
import unittest
import uuid

from control import Control, Refused, atomic_json


class OwnershipTests(unittest.TestCase):
    """Demonstrate that refused operations cannot reach the Tart command runner."""

    def setUp(self):
        """Create an isolated recorded VM with an observable hardware identity."""
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.calls = []
        self.run_id = str(uuid.uuid4())
        self.name = f'fh-{self.run_id}-mac'
        self.home = self.root / 'tart'
        self.vm = self.home / 'vms' / self.name
        self.vm.mkdir(parents=True)
        atomic_json(self.vm / 'config.json', {'macAddress': '02:00:00:00:00:01'})
        self.control = Control(self.root, self.root / 'journal.md', self.home, runner=self.fake)
        self.control.data = {'format': 1, 'run_id': self.run_id, 'vms': {
            'mac': {'name': self.name, 'phase': 'ready', 'mac_address': '02:00:00:00:00:01'}
        }}
        self.control.save()
        self.listed = [{'Name': self.name}]
        self.state = 'stopped'
        self.fail_delete = False
        self.assertEqual(self.control.owned('mac'), self.name)

    def fake(self, argv, **kwargs):
        """Model observable Tart state, keeping mutation attempts independently visible."""
        self.calls.append(argv)
        action = argv[1]
        if action == 'list':
            return subprocess.CompletedProcess(argv, 0, json.dumps(self.listed).encode())
        if action == 'get':
            return subprocess.CompletedProcess(argv, 0, json.dumps({'State': self.state}).encode())
        if action == 'delete':
            if self.fail_delete:
                raise subprocess.CalledProcessError(1, argv)
            self.listed = []
        return subprocess.CompletedProcess(argv, 0, b'')

    def test_replaced_vm_is_not_deleted(self):
        """A familiar name must not authorize deletion after hardware identity changes."""
        atomic_json(self.vm / 'config.json', {'macAddress': '02:00:00:00:00:02'})
        with self.assertRaisesRegex(Refused, 'identity changed'):
            self.control.delete('mac')
        self.assertEqual(self.calls, [])

    def test_manifest_cannot_claim_preexisting_name(self):
        """Even a matching MAC cannot let an edited entry claim a foreign VM name."""
        self.control.data['vms']['mac']['name'] = 'preexisting-vm'
        with self.assertRaisesRegex(Refused, 'owned clone'):
            self.control.delete('mac')
        self.assertEqual(self.calls, [])

    def test_running_vm_is_not_deleted(self):
        """Cleanup must require observed stopped state rather than force a shutdown."""
        self.state = 'running'
        with self.assertRaisesRegex(Refused, 'shut down'):
            self.control.delete('mac')
        self.assertFalse(any(call[1] == 'delete' for call in self.calls))

    def test_journal_failure_prevents_mutation(self):
        """An unwritable intent must stop the destructive command before it launches."""
        self.control.journal = self.root / 'missing-parent' / 'journal.md'
        with self.assertRaises(FileNotFoundError):
            self.control.delete('mac')
        self.assertFalse(any(call[1] == 'delete' for call in self.calls))

    def test_clone_collision_does_not_claim_ownership(self):
        """A live VM collision is refused before creating a manifest entry or clone."""
        self.control.data['vms'] = {}
        with self.assertRaisesRegex(Refused, 'already exists'):
            self.control.clone('mac', 'source-base')
        self.assertEqual(self.control.data['vms'], {})
        self.assertFalse(any(call[1] == 'clone' for call in self.calls))

    def test_failed_delete_preserves_manifest_and_records_failure(self):
        """A child failure leaves ownership intact and cannot become a cleanup success."""
        self.fail_delete = True
        with self.assertRaises(subprocess.CalledProcessError):
            self.control.delete('mac')
        self.assertEqual(self.control.data['vms']['mac']['phase'], 'ready')
        journal = self.control.journal.read_text()
        self.assertIn('| intent |', journal)
        self.assertIn('| result |', journal)
        self.assertIn('Failed: CalledProcessError', journal)

    def test_successful_delete_requires_absence_then_marks_deleted(self):
        """Observed absence, rather than exit code alone, establishes cleanup completion."""
        self.control.delete('mac')
        stored = json.loads(self.control.manifest_path.read_text())
        self.assertEqual(stored['vms']['mac']['phase'], 'deleted')
        self.assertEqual(self.listed, [])

    def test_pending_clone_cannot_be_deleted(self):
        """Interrupted clone attempts require reconciliation instead of guessed authority."""
        self.control.data['vms']['mac']['phase'] = 'creating'
        with self.assertRaisesRegex(Refused, 'owned clone'):
            self.control.delete('mac')
        self.assertEqual(self.calls, [])

    def test_redirected_vm_directory_is_refused(self):
        """A symbolic link must not redirect the identity check into a different VM."""
        other = self.vm.with_name('other')
        self.vm.rename(other)
        self.vm.symlink_to(other, target_is_directory=True)
        with self.assertRaisesRegex(Refused, 'symbolic link'):
            self.control.delete('mac')
        self.assertEqual(self.calls, [])

    def test_export_label_cannot_escape_private_directory(self):
        """Evidence labels must not turn a guest export into an arbitrary host write."""
        # Keep the hypothetical escaped path inside this test's owned parent;
        # a fixed sibling under the system temp root might belong to another run.
        collection = self.root / 'collection'
        collection.mkdir()
        self.control.directory = collection
        escaped = self.root / 'outside.tar'
        self.assertFalse(escaped.exists())
        with self.assertRaises(Refused):
            self.control.collect('mac', '/guest/evidence', '../outside')
        self.assertEqual(self.calls, [])
        self.assertFalse(escaped.exists())

    def test_export_launch_failure_records_result_and_keeps_partial(self):
        """A failed launch must close its intent without claiming complete evidence."""
        def fail_spawn(*args, **kwargs):
            """Model an unavailable Tart binary without starting any process."""
            raise FileNotFoundError('fixture Tart unavailable')
        with self.assertRaises(FileNotFoundError):
            self.control.collect('mac', '/guest/evidence', 'failed', spawner=fail_spawn)
        self.assertTrue((self.root / 'failed.partial').exists())
        self.assertFalse((self.root / 'failed.tar').exists())
        self.assertIn('Export launch failed', self.control.journal.read_text())

    def test_export_does_not_overwrite_retained_artifact(self):
        """A later collection cannot silently replace evidence from an earlier check."""
        retained = self.root / 'earlier.tar'
        retained.write_bytes(b'earlier evidence')
        with self.assertRaises(Refused):
            self.control.collect('mac', '/guest/evidence', 'earlier')
        self.assertEqual(retained.read_bytes(), b'earlier evidence')
        self.assertEqual(self.calls, [])

    def test_start_records_observed_running_state_before_returning(self):
        """A long-lived VM start must have a result before its eventual shutdown."""
        class Process:
            """Represent the still-owned foreground Tart child, without spawning it."""
            pid = 12345

            def poll(self):
                """A None status is the fixture's explicit live-process premise."""
                return None

        def spawn(argv, **kwargs):
            """Model startup becoming visible through the independent Tart state probe."""
            self.assertEqual(argv[-1], self.name)
            self.state = 'running'
            return Process()

        process = self.control.start('mac', spawner=spawn)
        self.assertEqual(process.pid, 12345)
        self.assertEqual(self.control.data['vms']['mac']['run_pid'], 12345)
        journal = self.control.journal.read_text()
        self.assertIn('| result |', journal)
        self.assertIn('Live VM state is running', journal)

    def test_shutdown_transport_failure_requires_stopped_poststate(self):
        """Losing the guest agent during real shutdown must retain both observations.

The command failure stays visible for forensics; independent stopped state,
rather than suppressing an exception alone, establishes successful shutdown.
"""
        base_runner = self.control.runner

        def shutting_down(argv, **kwargs):
            """Model shutdown terminating the guest transport before its reply."""
            if argv[1] == 'exec':
                self.state = 'stopped'
                raise subprocess.CalledProcessError(1, argv)
            return base_runner(argv, **kwargs)

        self.state = 'running'
        self.control.runner = shutting_down
        self.control.shutdown('mac')
        journal = self.control.journal.read_text()
        self.assertIn('Failed: CalledProcessError', journal)
        self.assertIn('Live Tart state is stopped', journal)

    def test_failed_shutdown_cannot_pass_with_running_guest(self):
        """A rejected shutdown never authorizes a force stop or a success claim."""
        base_runner = self.control.runner

        def rejected(argv, **kwargs):
            """Keep the guest running after a failed shutdown request."""
            if argv[1] == 'exec':
                raise subprocess.CalledProcessError(1, argv)
            return base_runner(argv, **kwargs)

        self.state = 'running'
        self.control.runner = rejected
        with self.assertRaisesRegex(Refused, 'did not stop'):
            self.control.shutdown('mac', timeout=0)
        self.assertNotIn('Live Tart state is stopped', self.control.journal.read_text())
        self.assertFalse(any(call[1] == 'stop' for call in self.calls))

    def test_start_retains_live_child_when_save_probe_or_result_fails(self):
        """Every post-spawn failure must leave a handle for the foreground owner.

These synthetic children have no OS PID or resource lifetime. Independent state
and failure hooks distinguish spawning from successfully validating startup.
"""
        class Process:
            """Represent a live child without starting or signaling a host process."""
            pid = 12345

            def poll(self):
                """Keep the startup-failure fixture alive for ownership assertions."""
                return None

        save, state, record = self.control.save, self.control.state, self.control.record
        for boundary in ('save', 'probe', 'result'):
            with self.subTest(boundary=boundary):
                self.state = 'stopped'
                self.control.processes.clear()
                process = Process()

                def spawn(*args, **kwargs):
                    """Make a child live before installing the post-spawn failure."""
                    self.assertEqual(self.state, 'stopped')
                    self.state = 'running'
                    def fail(*args, **kwargs):
                        """Fail only the chosen persistence or observation boundary."""
                        raise OSError('injected ' + boundary)
                    if boundary == 'save':
                        self.control.save = fail
                    elif boundary == 'probe':
                        self.control.state = fail
                    else:
                        def failed_result(kind, *args):
                            """Permit the intent, then fail its success result append."""
                            if kind == 'result':
                                fail()
                            return record(kind, *args)
                        self.control.record = failed_result
                    return process

                try:
                    with self.assertRaisesRegex(OSError, 'injected ' + boundary):
                        self.control.start('mac', spawner=spawn)
                    self.assertIsNone(process.poll())
                    self.assertEqual(self.control.processes, [process])
                    self.assertFalse(any(call[1] == 'stop' for call in self.calls))
                finally:
                    self.control.save, self.control.state, self.control.record = save, state, record

    def test_foreground_owner_waits_even_when_startup_probe_fails(self):
        """The standalone CLI must keep supervision after a live failed startup."""
        owner = self
        class Process:
            """Observe waiting directly; no OS process is spawned by this fixture."""
            pid = 12345
            waited = False

            def poll(self):
                """Remain live until the foreground owner waits for guest shutdown."""
                return None if not self.waited else 0

            def wait(self):
                """Simulate independent guest shutdown during an actual owner wait."""
                owner.assertIsNone(self.poll())
                self.waited = True
                owner.state = 'stopped'
                return 0

        process = Process()
        def spawn(*args, **kwargs):
            """Fail the first post-spawn state observation after registering the child."""
            self.assertEqual(self.state, 'stopped')
            self.state = 'running'
            def failed_probe(*args):
                """Model a transient Tart inspection error, not a stopped child."""
                raise Refused('injected live-state failure')
            self.control.state = failed_probe
            return process
        with self.assertRaisesRegex(Refused, 'injected live-state failure'):
            self.control.run_foreground('mac', spawner=spawn)
        self.assertTrue(process.waited)
        self.assertEqual(self.control.processes, [process])


if __name__ == '__main__':
    unittest.main()
