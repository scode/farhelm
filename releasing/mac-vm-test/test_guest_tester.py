"""Exercise unattended execution failures without claiming simulated UI is product proof.

The model subprocess fixtures write known event streams; real process ownership
and archive selection remain under test. Native-image fixtures only establish
provenance linkage, never a Farhelm rendering or agent-conversation result.
"""

import base64
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

from brick_test import plan
from test_brick_test import FakeSource
from guest_tester import (TesterError, child_exited, group_live, native_events, request, retain_native_captures,
                         run, stop_owned_group, validate_result)


PNG = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j9AAAAABJRU5ErkJggg==')


def task(gui=True):
    """Supply explicit fictional guest paths rather than reading the host configuration."""
    return {'format': 1, 'attempt': 'attempt', 'run_id': 'run', 'pass_id': 'pass',
            'check_id': 'check', 'gui': gui, 'timeout_seconds': 30,
            'settings': {'codex': '/fixture/codex', 'python': '/fixture/python',
                         'desktop_app': '/fixture/Codex.app', 'model': 'fixture-model', 'effort': 'low'},
            'context': {}, 'instructions': 'Fixture checkpoint.'}


def events(image=b'fixture-native-image'):
    """Make the capture bytes observable independently from the retained evidence file."""
    return [{'type': 'item.completed', 'item': {'type': 'mcp_tool_call', 'server': 'cua_repl',
            'tool': 'js', 'status': 'completed', 'result': {'content': [
                {'type': 'image', 'data': base64.b64encode(image).decode()}]}}}, {'type': 'turn.completed'}]


def result(value, status='pass'):
    """Use task identity and a separate observation; a process exit alone cannot pass."""
    return {'identity': {key: value[key] for key in ('attempt', 'run_id', 'pass_id', 'check_id')},
            'status': status, 'detail': 'Observed the declared fixture action.',
            'evidence': ['native.jpg'], 'continuity': None}


class GuestTesterTests(unittest.TestCase):
    """Refuse stale, forged and blocked observations while retaining private diagnostics."""

    def setUp(self):
        """Own all fixture artifacts without changing the test process environment."""
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def test_planned_stable_upgrade_names_cross_the_guest_envelope_boundary(self):
        """Public planner names must reach the tester without weakening path ownership.

P0 uses plain identifiers, but each bonus baseline embeds a dotted stable tag.
This composes the real planner and validator, so a hand-shaped task cannot hide
that cross-phase naming mismatch.
"""
        coverage = plan('v0.25.0', 'v0.24.0', FakeSource(['v0.23.0', 'v0.24.0']))
        for entry in coverage['passes']:
            value = dict(task(), pass_id=entry['id'])
            with self.subTest(pass_id=entry['id']):
                self.assertEqual(request(value)['pass_id'], entry['id'])

    def test_dotted_pass_names_cannot_escape_or_weaken_other_identifiers(self):
        """Only canonical stable upgrade tags need dots; other task names retain their grammar."""
        for name in ('../upgrade-v0.24.0', 'upgrade-v0.24.0/other', 'upgrade-v0.24.0..',
                     'upgrade-v00.24.0', 'upgrade-v0.24.0-rc.1', 'other.name',
                     'upgrade-v' + '1' * 101 + '.0.0'):
            with self.subTest(name=name), self.assertRaises(TesterError):
                request(dict(task(), pass_id=name))
        for key in ('attempt', 'run_id', 'check_id'):
            with self.subTest(key=key), self.assertRaises(TesterError):
                request(dict(task(), **{key: 'upgrade-v0.24.0'}))

    def test_native_capture_is_required_and_cannot_be_replaced(self):
        """Matching prose and a completed turn must not admit an unrelated image."""
        value = task()
        (self.root / 'native.jpg').write_bytes(b'fixture-native-image')
        self.assertEqual(validate_result(value, result(value), self.root, events())['status'], 'pass')
        (self.root / 'native.jpg').write_bytes(b'fabricated-other-image')
        with self.assertRaisesRegex(TesterError, 'actual native'):
            validate_result(value, result(value), self.root, events())
        with self.assertRaisesRegex(TesterError, 'no actual'):
            validate_result(value, result(value), self.root, [{'type': 'turn.completed'}])

    def test_failed_tool_and_failed_turn_cannot_be_hidden_by_result(self):
        """Vendor refusal and model errors are execution failures even with a pass file."""
        for stream in ([{'type': 'turn.failed'}], [{'type': 'error'}], events()[:-1]):
            with self.subTest(stream=stream), self.assertRaises(TesterError):
                native_events(stream)
        stream = events()
        stream[0]['item']['result']['isError'] = True
        with self.assertRaisesRegex(TesterError, 'refused'):
            native_events(stream)

    def test_identity_auth_names_and_symlinks_are_rejected(self):
        """Evidence belongs to this attempt and cannot export an authentication store."""
        value = task()
        (self.root / 'native.jpg').write_bytes(b'fixture-native-image')
        stale = result(value)
        stale['identity']['attempt'] = 'old-attempt'
        with self.assertRaisesRegex(TesterError, 'identity'):
            validate_result(value, stale, self.root, events())
        forbidden = result(value)
        forbidden['evidence'] = ['auth.json']
        with self.assertRaisesRegex(TesterError, 'authentication'):
            validate_result(value, forbidden, self.root, events())
        (self.root / 'native.jpg').unlink()
        (self.root / 'native.jpg').symlink_to(self.root / 'elsewhere')
        with self.assertRaisesRegex(TesterError, 'escapes'):
            validate_result(value, result(value), self.root, events())
        value['format'] = True
        with self.assertRaises(TesterError):
            request(value)

    def test_native_capture_persistence_needs_no_model_image_file(self):
        """The executor must preserve exact tool bytes when the model supplies no encoding script or image reference."""
        value = task()
        observed = result(value)
        observed['evidence'] = []
        script = ('import json;from pathlib import Path; '
                  'p=Path(' + repr(str(self.root / 'attempt/work/result.json')) + '); '
                  'p.write_text(' + repr(json.dumps(observed)) + '); '
                  '[print(json.dumps(row),flush=True) for row in ' + repr(events(PNG)) + ']')
        def spawn(argv, **kwargs):
            """Use a real owned child whose only image output is the controlled native event stream."""
            return subprocess.Popen([sys.executable, '-c', script], **kwargs)
        summary = run(value, self.root / 'attempt', spawner=spawn)
        export = self.root / 'attempt/export'
        self.assertEqual(summary['status'], 'pass')
        self.assertEqual(summary['result']['evidence'], ['native-capture-001.png'])
        self.assertEqual((export / 'native-capture-001.png').read_bytes(), PNG)
        self.assertEqual(json.loads((export / 'model-result.json').read_text()), observed)
        self.assertEqual(json.loads((export / 'result.json').read_text()), summary['result'])
        self.assertEqual((export / 'native-capture-001.png').stat().st_mode & 0o777, 0o600)
        self.assertEqual((self.root / 'attempt/work/native-capture-001.png').stat().st_mode & 0o777, 0o600)

    def test_selected_nested_receipt_is_exported_without_its_neighbors(self):
        """A signed-input receipt may keep its relative path without exporting the scratch tree.

The old-fixture tester naturally places verification beside downloaded inputs.
This exercises the owned child and real export path, including private parent
permissions, rather than merely weakening the observation validator.
"""
        value = task()
        observed = result(value)
        observed['evidence'] = ['old-signed-inputs/verification.json']
        work = self.root / 'attempt/work'
        script = ('import json;from pathlib import Path; '
                  'w=Path(' + repr(str(work)) + '); '
                  '(w/"old-signed-inputs").mkdir(); '
                  '(w/"old-signed-inputs/verification.json").write_text("verified public installer"); '
                  '(w/"old-signed-inputs/auth.json").write_text("fictional unselected fixture"); '
                  '(w/"old-signed-inputs/scratch.txt").write_text("unselected scratch"); '
                  '(w/"result.json").write_text(' + repr(json.dumps(observed)) + '); '
                  '[print(json.dumps(row),flush=True) for row in ' + repr(events(PNG)) + ']')
        def spawn(argv, **kwargs):
            """Produce the exact nested reference that the previous real old-fixture attempt rejected."""
            return subprocess.Popen([sys.executable, '-c', script], **kwargs)
        summary = run(value, self.root / 'attempt', spawner=spawn)
        self.assertEqual(summary['status'], 'pass')
        self.assertTrue((work / 'old-signed-inputs/auth.json').is_file())
        self.assertTrue((work / 'old-signed-inputs/scratch.txt').is_file())
        exported = self.root / 'attempt/export/old-signed-inputs'
        self.assertEqual({path.name for path in exported.iterdir()}, {'verification.json'})
        self.assertEqual((exported / 'verification.json').read_text(), 'verified public installer')
        self.assertEqual(exported.stat().st_mode & 0o777, 0o700)
        self.assertEqual((exported / 'verification.json').stat().st_mode & 0o777, 0o600)
        self.assertIn('old-signed-inputs/verification.json', summary['result']['evidence'])
        self.assertEqual((self.root / 'attempt/export/native-capture-001.png').read_bytes(), PNG)

    def test_nested_paths_still_refuse_traversal_credentials_and_executor_records(self):
        """Directory support cannot admit an absolute path, unsafe component or renamed executor record."""
        value = task(False)
        cases = {'../receipt.txt': 'attempt only', 'inputs/../receipt.txt': 'attempt only',
                 './receipt.txt': 'attempt only', 'inputs//receipt.txt': 'attempt only',
                 '/receipt.txt': 'attempt only', 'inputs\\receipt.txt': 'attempt only',
                 'inputs/auth.json': 'authentication', 'credentials/receipt.txt': 'authentication',
                 'inputs/task.json': 'executor records', 'inputs/events.jsonl': 'executor records',
                 'inputs/stderr.txt': 'executor records',
                 'task.json/receipt.txt': 'executor records', 'Task.json/receipt.txt': 'executor records'}
        for reference, reason in cases.items():
            with self.subTest(reference=reference):
                observed = result(value)
                observed['evidence'] = [reference]
                with self.assertRaisesRegex(TesterError, reason):
                    validate_result(value, observed, self.root, events())

    def test_nested_receipt_cannot_cross_even_an_internal_symlink_directory(self):
        """Every referenced directory must be owned directly, even when its symlink resolves inside the attempt."""
        actual = self.root / 'actual'
        actual.mkdir()
        (actual / 'receipt.txt').write_text('owned receipt')
        link = self.root / 'redirected'
        link.symlink_to(actual, target_is_directory=True)
        self.assertTrue(link.is_symlink())
        self.assertEqual((link / 'receipt.txt').read_text(), 'owned receipt')
        observed = result(task(False))
        observed['evidence'] = ['redirected/receipt.txt']
        with self.assertRaisesRegex(TesterError, 'escapes'):
            validate_result(task(False), observed, self.root, events())

    def test_case_only_reference_aliases_are_rejected_before_export(self):
        """Two spellings of one Mac file cannot collide during exclusive evidence copying.

Both paths exist on either case-sensitive or case-insensitive test volumes.
The refusal spans evidence and continuity; an identical repeated spelling is
still safe because selected export names are deduplicated exactly.
"""
        for name in ('Receipt.txt', 'receipt.txt'):
            (self.root / name).write_text('controlled command receipt')
            self.assertTrue((self.root / name).is_file())
        value = task(False)
        for continuity in (False, True):
            with self.subTest(continuity=continuity):
                observed = result(value)
                observed['evidence'] = ['Receipt.txt', 'receipt.txt']
                if continuity:
                    observed['evidence'] = ['Receipt.txt']
                    observed['continuity'] = {'before': 'Receipt.txt', 'after': 'receipt.txt'}
                with self.assertRaisesRegex(TesterError, 'case aliases'):
                    validate_result(value, observed, self.root, events())
        observed = result(value)
        observed['evidence'] = ['Receipt.txt', 'Receipt.txt']
        self.assertEqual(validate_result(value, observed, self.root, events())['status'], 'pass')

    def test_nested_native_capture_name_remains_executor_owned(self):
        """A subdirectory cannot hide a model-created file under a reserved automatic-capture basename."""
        nested = self.root / 'images'
        nested.mkdir()
        (nested / 'native-capture-001.png').write_bytes(PNG)
        observed = result(task())
        observed['evidence'] = ['images/native-capture-001.png']
        with self.assertRaisesRegex(TesterError, 'belong to the executor'):
            retain_native_captures(task(), observed, self.root, events(PNG))
        self.assertFalse((self.root / 'native-capture-001.png').exists())
        (nested / 'Native-Capture-002.png').write_bytes(PNG)
        observed['evidence'] = ['images/Native-Capture-002.png']
        with self.assertRaisesRegex(TesterError, 'belong to the executor'):
            retain_native_captures(task(), observed, self.root, events(PNG))
        self.assertFalse((self.root / 'native-capture-001.png').exists())

    def test_latest_distinct_captures_obey_the_reference_bound(self):
        """Long native interaction cannot turn diagnostics into an unbounded image archive."""
        value = task()
        observed = result(value, 'could-not-run')
        observed['evidence'] = []
        rows = []
        for index in range(12):
            rows.extend(events(f'synthetic-image-{index}'.encode())[:-1])
        rows.extend(events(b'synthetic-image-0')[:-1])
        enriched = retain_native_captures(value, observed, self.root, rows)
        self.assertEqual(len(enriched['evidence']), 10)
        self.assertEqual([(self.root / ref).read_bytes() for ref in enriched['evidence']],
                         [f'synthetic-image-{index}'.encode() for index in range(3, 12)] + [b'synthetic-image-0'])
        self.assertEqual(observed['evidence'], [])

    def test_capture_collision_does_not_overwrite_an_existing_path(self):
        """A stale or redirected executor filename must stop rather than replace someone else's bytes."""
        (self.root / 'commands.txt').write_text('controlled command receipt')
        observed = result(task())
        observed['evidence'] = ['commands.txt']
        foreign = self.root / 'foreign.txt'
        foreign.write_bytes(b'preserve')
        (self.root / 'native-capture-001.png').symlink_to(foreign)
        with self.assertRaises(FileExistsError):
            retain_native_captures(task(), observed, self.root, events(PNG))
        self.assertEqual(foreign.read_bytes(), b'preserve')

    def test_capture_bytes_share_the_command_evidence_budget(self):
        """Image retention must not evade the aggregate bound already reserved for command receipts."""
        (self.root / 'commands.txt').write_bytes(b'x' * (16 * 1024 * 1024))
        observed = result(task())
        observed['evidence'] = ['commands.txt']
        enriched = retain_native_captures(task(), observed, self.root, events(PNG))
        self.assertEqual(enriched['evidence'], ['commands.txt'])
        self.assertFalse((self.root / 'native-capture-001.png').exists())

    def test_capture_budget_keeps_recent_images_that_fit(self):
        """When two screenshots each fit but cannot fit together, the later acceptance state takes priority."""
        (self.root / 'commands.txt').write_bytes(b'x' * (16 * 1024 * 1024 - 64))
        observed = result(task(), 'fail')
        observed['evidence'] = ['commands.txt']
        older, newer = b'older-state' * 4, b'latest-state' * 4
        self.assertLessEqual(len(older), 64)
        self.assertLessEqual(len(newer), 64)
        self.assertGreater(len(older) + len(newer), 64)
        rows = events(older)[:-1] + events(newer)[:-1]
        enriched = retain_native_captures(task(), observed, self.root, rows)
        self.assertEqual(enriched['evidence'], ['commands.txt', 'native-capture-001.bin'])
        self.assertEqual((self.root / 'native-capture-001.bin').read_bytes(), newer)

    def test_explicit_tester_fault_remains_automation_failure(self):
        """An evidence-script fault after useful UI output cannot condemn the product as failed."""
        value = task()
        with self.assertRaises(TesterError):
            run(value, self.root / 'attempt', spawner=self.spawn_writer(value, 'automation-failure'))
        summary = json.loads((self.root / 'attempt/export/execution.json').read_text())
        self.assertEqual(summary['status'], 'automation-failure')
        self.assertEqual(summary['unverified_result']['status'], 'automation-failure')
        self.assertEqual(summary['exit_code'], 0)
        self.assertIsNone(summary['cleanup_error'])

    def test_truncated_events_keep_failure_claim_receipt_and_complete_capture(self):
        """An interrupted final write must not erase earlier diagnostic proof or turn a failed child into a pass."""
        value = task()
        observed = result(value, 'fail')
        observed['evidence'] = ['commands.txt']
        script = ('import json,sys;from pathlib import Path; '
                  'root=Path(' + repr(str(self.root / 'attempt/work')) + '); '
                  '(root/"commands.txt").write_text("controlled failure receipt"); '
                  '(root/"result.json").write_text(' + repr(json.dumps(observed)) + '); '
                  '[print(json.dumps(row),flush=True) for row in ' + repr(events(PNG)[:-1]) + ']; '
                  'sys.stdout.write("{\\\"type\\\":");sys.stdout.flush();sys.exit(1)')
        def spawn(argv, **kwargs):
            """Leave a genuinely truncated child log after a complete native row and safe observation."""
            return subprocess.Popen([sys.executable, '-c', script], **kwargs)
        with self.assertRaisesRegex(TesterError, 'process failed'):
            run(value, self.root / 'attempt', spawner=spawn)
        export = self.root / 'attempt/export'
        summary = json.loads((export / 'execution.json').read_text())
        self.assertEqual(summary['status'], 'automation-failure')
        self.assertEqual(summary['unverified_result']['status'], 'fail')
        self.assertEqual((export / 'commands.txt').read_text(), 'controlled failure receipt')
        self.assertEqual((export / 'native-capture-001.png').read_bytes(), PNG)
        self.assertEqual(json.loads((export / 'model-result.json').read_text()), observed)

    def test_capture_failure_keeps_safe_raw_failure_receipt(self):
        """Image retention failure cannot erase the product claim or replace a foreign executor path."""
        value = task()
        observed = result(value, 'fail')
        observed['evidence'] = ['commands.txt']
        script = ('import json,sys;from pathlib import Path; '
                  'root=Path(' + repr(str(self.root / 'attempt/work')) + '); '
                  '(root/"commands.txt").write_text("controlled failure receipt"); '
                  '(root/"native-capture-001.png").write_text("preserve"); '
                  '(root/"result.json").write_text(' + repr(json.dumps(observed)) + '); '
                  '[print(json.dumps(row),flush=True) for row in ' + repr(events(PNG)) + '];sys.exit(1)')
        def spawn(argv, **kwargs):
            """Make exclusive capture creation fail after establishing a safe independent command receipt."""
            return subprocess.Popen([sys.executable, '-c', script], **kwargs)
        with self.assertRaises(TesterError):
            run(value, self.root / 'attempt', spawner=spawn)
        export = self.root / 'attempt/export'
        summary = json.loads((export / 'execution.json').read_text())
        self.assertEqual(summary['unverified_result'], observed)
        self.assertEqual((export / 'commands.txt').read_text(), 'controlled failure receipt')
        self.assertFalse((export / 'native-capture-001.png').exists())
        self.assertEqual((self.root / 'attempt/work/native-capture-001.png').read_text(), 'preserve')

    def test_partial_utf8_tail_keeps_safe_failure_observation(self):
        """Killing a writer mid-character must not discard complete earlier native rows or command receipts."""
        value = task()
        observed = result(value, 'fail')
        observed['evidence'] = ['commands.txt']
        script = ('import json,sys;from pathlib import Path; '
                  'root=Path(' + repr(str(self.root / 'attempt/work')) + '); '
                  '(root/"commands.txt").write_text("controlled failure receipt"); '
                  '(root/"result.json").write_text(' + repr(json.dumps(observed)) + '); '
                  '[print(json.dumps(row),flush=True) for row in ' + repr(events(PNG)[:-1]) + ']; '
                  'sys.stdout.buffer.write(b\'{"text":"\\xc3\');sys.stdout.buffer.flush();sys.exit(1)')
        def spawn(argv, **kwargs):
            """Write an actual invalid UTF-8 suffix through the real child's byte stream."""
            return subprocess.Popen([sys.executable, '-c', script], **kwargs)
        with self.assertRaises(TesterError):
            run(value, self.root / 'attempt', spawner=spawn)
        export = self.root / 'attempt/export'
        summary = json.loads((export / 'execution.json').read_text())
        self.assertEqual(summary['unverified_result']['status'], 'fail')
        self.assertEqual((export / 'commands.txt').read_text(), 'controlled failure receipt')
        self.assertEqual((export / 'native-capture-001.png').read_bytes(), PNG)

    def test_raw_failure_cannot_export_model_images_under_executor_names(self):
        """Diagnostic salvage must preserve the claim without making model-made captures look executor-owned."""
        value = task()
        observed = result(value, 'fail')
        observed['evidence'] = ['commands.txt', 'native-capture-001.png', 'images/native-capture-002.png']
        script = ('import json,sys;from pathlib import Path; '
                  'root=Path(' + repr(str(self.root / 'attempt/work')) + '); '
                  '(root/"commands.txt").write_text("controlled failure receipt"); '
                  '(root/"native-capture-001.png").write_text("model-made image"); '
                  '(root/"images").mkdir(); '
                  '(root/"images/native-capture-002.png").write_text("nested model-made image"); '
                  '(root/"result.json").write_text(' + repr(json.dumps(observed)) + ');sys.exit(1)')
        def spawn(argv, **kwargs):
            """Have a real failed child explicitly claim an executor-reserved image filename."""
            return subprocess.Popen([sys.executable, '-c', script], **kwargs)
        with self.assertRaises(TesterError):
            run(value, self.root / 'attempt', spawner=spawn)
        export = self.root / 'attempt/export'
        summary = json.loads((export / 'execution.json').read_text())
        self.assertEqual(summary['unverified_result']['status'], 'fail')
        self.assertEqual(summary['unverified_result']['evidence'], ['commands.txt'])
        self.assertFalse((export / 'native-capture-001.png').exists())
        self.assertFalse((export / 'images').exists())
        self.assertEqual(json.loads((export / 'model-result.json').read_text()), observed)

    def spawn_writer(self, value, status='pass', extra=''):
        """Launch a real isolated child that produces a controlled model outcome.

Ignoring model argv is intentional: this fixture tests executor ownership and
result import, with no credentials, model service or UI permissions involved.
"""
        script = ('import json;from pathlib import Path;'
                  'root=Path(' + repr(str(self.root / 'attempt/work')) + ');'
                  '(root/"native.jpg").write_bytes(b"fixture-native-image");'
                  '(root/"result.json").write_text(' + repr(json.dumps(result(value, status))) + ');'
                  '(root/"auth.json").write_text("unreferenced-private-fixture");'
                  '(root/"scratch").mkdir();'
                  '[print(json.dumps(row),flush=True) for row in ' + repr(events()) + '];' + extra)
        def spawn(argv, **kwargs):
            """Preserve the executor's real stdio, descriptor and process-group contract."""
            self.assertTrue(kwargs['close_fds'])
            self.assertTrue(kwargs['start_new_session'])
            return subprocess.Popen([sys.executable, '-c', script], **kwargs)
        return spawn

    def test_permission_result_stops_and_exports_only_validated_artifacts(self):
        """A successful child exit cannot convert a permission block into a pass."""
        value = task()
        summary = run(value, self.root / 'attempt', spawner=self.spawn_writer(value, 'permission-block'))
        self.assertEqual(summary['exit_code'], 0)
        self.assertEqual(summary['status'], 'permission-block')
        exported = self.root / 'attempt/export'
        self.assertTrue((exported / 'native.jpg').is_file())
        self.assertFalse((exported / 'auth.json').exists())
        self.assertFalse((exported / 'scratch').exists())
        self.assertTrue(all(p.stat().st_mode & 0o077 == 0 for p in exported.iterdir()))
        with self.assertRaises(FileExistsError):
            run(value, self.root / 'attempt', spawner=self.spawn_writer(value))

    def test_deadline_failure_retains_summary_and_reaps_child(self):
        """A silent live child is terminated, rather than mistaken for absent evidence."""
        process = []
        def spawn(argv, **kwargs):
            """Retain the actual child handle so teardown can be checked independently."""
            child = subprocess.Popen([sys.executable, '-c', 'import time;time.sleep(60)'], **kwargs)
            process.append(child)
            return child
        ticks = iter([0, 31])
        with self.assertRaisesRegex(TesterError, 'deadline'):
            run(task(), self.root / 'attempt', spawner=spawn, clock=lambda: next(ticks))
        self.assertIsNotNone(process[0].returncode)
        summary = json.loads((self.root / 'attempt/export/execution.json').read_text())
        self.assertEqual(summary['status'], 'automation-failure')
        self.assertFalse((self.root / 'attempt/export/native.jpg').exists())

    def test_completed_leader_retains_identity_until_group_cleanup(self):
        """WNOWAIT must keep the exited leader owned until cleanup sends its signals."""
        child = subprocess.Popen([sys.executable, '-c', 'pass'], start_new_session=True)
        self.addCleanup(lambda: child.wait(timeout=5))
        os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOWAIT)
        self.assertTrue(child_exited(child))
        self.assertIsNone(child.returncode)
        stop_owned_group(child)
        self.assertEqual(child.returncode, 0)

    def test_surviving_mcp_child_is_terminated_after_leader_exits(self):
        """An exited leader must not conceal a live MCP descendant that ignores TERM."""
        receipt = self.root / 'peer.json'
        peer = ('import signal,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);'
                'print("peer-ready",flush=True);time.sleep(60)')
        script = ('import json,subprocess,sys;from pathlib import Path;'
                  'peer=subprocess.Popen([sys.executable,"-c",' + repr(peer) + '],stdout=subprocess.PIPE);'
                  'assert peer.stdout.readline()==b"peer-ready\\n";'
                  'Path(' + repr(str(receipt)) + ').write_text(json.dumps({"pid":peer.pid,"ready":True}))')
        leader = subprocess.Popen([sys.executable, '-c', script], start_new_session=True)
        self.addCleanup(lambda: stop_owned_group(leader) if leader.returncode is None else None)
        os.waitid(os.P_PID, leader.pid, os.WEXITED | os.WNOWAIT)
        self.assertTrue(json.loads(receipt.read_text())['ready'])
        self.assertTrue(group_live(leader.pid))
        stop_owned_group(leader)
        peer_pid = json.loads(receipt.read_text())['pid']
        observed = subprocess.run(['/bin/ps', '-o', 'stat=', '-p', str(peer_pid)], capture_output=True, timeout=5)
        self.assertEqual(observed.stderr, b'')
        state = observed.stdout.strip()
        # ps uses exit 1 for an absent PID. Other failures or diagnostics are
        # unavailable evidence, not proof that the owned descendant is gone.
        self.assertEqual(observed.returncode, 0 if state else 1)
        self.assertTrue(not state or state.startswith(b'Z'))

    def test_cancel_marker_acknowledges_only_after_child_cleanup(self):
        """Coordinator cancellation remains failed evidence, with an observed stopped executor."""
        (self.root / 'cancel.json').write_text(json.dumps({'attempt': 'attempt'}))
        owned = []
        def spawn(argv, **kwargs):
            """Retain the real handle to verify the acknowledgement's lifetime claim."""
            child = subprocess.Popen([sys.executable, '-c', 'import time;time.sleep(60)'], **kwargs)
            owned.append(child)
            self.assertFalse(child_exited(child))
            return child
        with self.assertRaisesRegex(TesterError, 'cancelled'):
            run(task(), self.root / 'attempt', spawner=spawn)
        self.assertIsNotNone(owned[0].returncode)
        self.assertEqual(json.loads((self.root / 'executor-state.json').read_text())['phase'], 'stopped')

    def test_cleanup_error_cannot_publish_stopped_after_leader_reaped(self):
        """A cleanup exception retains its fence even when the leader already returned zero."""
        def failed_cleanup(process):
            """Model the exact late-failure boundary: reaped leader, unverified descendants."""
            process.wait(timeout=5)
            raise TesterError('unobserved group cleanup')
        value = task()
        with mock.patch('guest_tester.stop_owned_group', side_effect=failed_cleanup), self.assertRaises(TesterError):
            run(value, self.root / 'attempt', spawner=self.spawn_writer(value))
        summary = json.loads((self.root / 'attempt/export/execution.json').read_text())
        self.assertIsNotNone(summary['cleanup_error'])
        self.assertEqual(json.loads((self.root / 'executor-state.json').read_text())['phase'], 'cleanup-failed')
        self.assertTrue((self.root / 'attempt/export/native.jpg').is_file())
        self.assertEqual(summary['status'], 'automation-failure')


if __name__ == '__main__':
    unittest.main()
