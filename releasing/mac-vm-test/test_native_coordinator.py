"""Compose format-two orchestration with real archives and independently modeled VM state.

These tests establish scheduling, import and cleanup authority. The tiny native
capture is synthetic; no result here claims real CUA or a product agent ran.
"""

import io
import base64
import json
import os
import subprocess
import sys
import tarfile
import struct
import unittest
import zlib
from unittest import mock

from brick_test import Run, pass_roles, rate_limited_without_actions, smoke
from checkpoint_contract import REMOTE_MAC_PASS
from control import Refused
from guest_workflow import CleanupPending
import test_brick_test as fixtures
from test_brick_test import FakeSource, PNG
from test_guest_tester import events, result, task
from test_fixture_probe import Commands
from fixture_probe import probe, ProbeError


def after_capture(pixel=b'\xff\x00\x00\xff'):
    """Provide a distinct valid synthetic PNG for a multi-observation receipt contract.

This pixel is deliberately unrelated to Farhelm. Its role is to prove capture
linkage and distinguish two retained observations, never a real dialog outcome.
"""
    def chunk(kind, value):
        """Encode a PNG chunk with its independent wire checksum."""
        return struct.pack('>I', len(value)) + kind + value + struct.pack('>I', zlib.crc32(kind + value))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(b'\x00' + pixel)) + chunk(b'IEND', b''))


def provider_refusal():
    """Model the observed provider 429 before any command, native action or approval."""
    message = 'exceeded retry limit, last status: 429 Too Many Requests'
    execution = {'format': 1, 'status': 'automation-failure', 'exit_code': 1,
                 'cleanup_error': None, 'error': 'guest tester process failed'}
    rows = [{'type': 'thread.started'}, {'type': 'turn.started'},
            {'type': 'error', 'message': message}, {'type': 'turn.failed', 'error': {'message': message}}]
    protocol = [{'direction': 'from-client', 'method': 'initialize', 'id': 0, 'error': False},
                {'direction': 'from-client', 'method': 'tools/list', 'id': 1, 'error': False}]
    return execution, rows, protocol


class NativeCoordinatorTests(unittest.TestCase):
    """Exercise the new orchestration path rather than only its helper-shaped mocks."""

    def setUp(self):
        """Reuse the owned fake-Tart substrate, with explicit format-two setup records."""
        self.fixture = fixtures.WorkflowTests()
        self.addCleanup(self.fixture.doCleanups)
        self.fixture.setUp()
        f = self.fixture
        config = {'settings': task()['settings'], 'auth_source': str(f.root / 'fictional-login.json'),
                  'authorization': str(f.root / 'consent.json'), 'fixture_receipt': str(f.root / 'native-fixture.json')}
        f.profile.values.update(format=2, guest_tester=config)
        values = {'fictional-login.json': {'fixture': 'fake-login'},
                  'consent.json': {'format': 1, 'approved': True, 'scope': 'disposable-test-clones',
                                   'destinations': ['mac', 'linux'], 'auth_source': config['auth_source']},
                  'native-fixture.json': {'format': 1,
                      'bases': {role: f.profile.values[role + '_base'] for role in ('mac', 'linux')},
                      'settings': config['settings'], 'checks': {key: True for key in (
                          'native-screenshot-click-type', 'native-after-reopen', 'clone-permission-carryover',
                          'auth-not-in-bases', 'farhelm-clean', 'linux-codex-ready')}}}
        for name, value in values.items():
            path = f.root / name
            path.write_text(json.dumps(value))
            path.chmod(0o600)
        original = f.model.runner
        def runner(argv, **kwargs):
            """Expose the home probe as actual text bytes from this modeled command channel."""
            if argv[1] == 'exec' and argv[-1] == 'from pathlib import Path; print(Path.home())':
                return subprocess.CompletedProcess(argv, 0, b'/Users/fixture\n')
            return original(argv, **kwargs)
        f.model.runner = runner
        f.enable_p0(scope='p0-only')
        self.run = f.run
        self.seen = []
        source = FakeSource(['v0.24.0'])
        source.git = lambda *args: 'https://github.com/fixture/project.git\n'
        self.source = source

    def archive(self, controller, envelope, status='pass', unverified=None, two_captures=True, image_only=False,
                fixture_defect=None, nested_receipt=False, observation_identity=None, detail=None):
        """Return a real archive with separate execution, observation and capture identities."""
        label = 'tester-' + envelope['attempt']
        observation = result(envelope, unverified or status)
        if detail is not None:
            observation['detail'] = detail
        if observation_identity is not None:
            observation['identity'] = observation_identity
        observation['evidence'] = ['native.png', 'commands.txt', 'verification.json']
        if nested_receipt:
            observation['evidence'][-1] = 'signed-inputs/verification.json'
        native_rows = events(PNG)
        if image_only:
            observation['evidence'] = ['native.png', 'native.bin']
            native_rows[0]['item']['result']['content'].append(
                {'type': 'image', 'data': base64.b64encode(b'unknown-native-image-format').decode()})
        extra_captures = {}
        if envelope['check_id'] in ('p2-feedback-send', 'remote-host-remove', 'remote-host-uninstall') and two_captures:
            extra_captures['native-after.png'] = after_capture()
        if envelope['check_id'] == 'p2-announcements':
            extra_captures['native-dialog.png'] = after_capture()
            if two_captures:
                extra_captures['native-history.png'] = after_capture(b'\x00\x00\xff\xff')
        for name, capture in extra_captures.items():
            observation['evidence'].append(name)
            native_rows[0]['item']['result']['content'].append(
                {'type': 'image', 'mimeType': 'image/png', 'data': base64.b64encode(capture).decode()})
        receipt = {'version': envelope['context']['candidate'], 'previous_trust_ring': envelope['context']['previous'],
                   'sha256sums_sha256': 'fixture-checksum', 'signature': 'verified',
                   'trusted_comment': 'verified', 'installer_checksum': 'verified'}
        summary = {'format': 1, 'status': status, 'result': observation}
        if unverified:
            summary = {'format': 1, 'status': status, 'unverified_result': observation,
                       'error': 'fixture tool turn aborted after observation'}
        files = {'task.json': json.dumps(envelope).encode(), 'events.jsonl':
                 ('\n'.join(json.dumps(row) for row in native_rows) + '\n').encode(),
                 'execution.json': json.dumps(summary).encode(),
                 'native.png': PNG, 'commands.txt': b'fictional independent command receipt',
                 ('signed-inputs/verification.json' if nested_receipt else 'verification.json'): json.dumps(receipt).encode()}
        if envelope['check_id'] == 'fixture-clean':
            clean = {'format': 1, 'clean': True, 'observations': [
                probe(role, '/fictional/home', {}, runner=Commands(), present=lambda path: False, which=lambda name: None)
                for role in pass_roles(envelope['pass_id'])]}
            if fixture_defect == 'missing-peer':
                clean['observations'].pop()
            elif fixture_defect == 'service':
                clean['observations'][1]['services'] = ['farhelm-supervisor.service inactive']
            elif fixture_defect == 'path':
                clean['observations'][0]['paths'][0]['present'] = True
            files['fixture-clean.json'] = json.dumps(clean).encode()
            observation['evidence'].append('fixture-clean.json')
            if fixture_defect == 'missing-receipt':
                observation['evidence'].remove('fixture-clean.json')
                del files['fixture-clean.json']
            files['execution.json'] = json.dumps(summary).encode()
        if image_only:
            files['native.bin'] = b'unknown-native-image-format'
        files.update(extra_captures)
        with tarfile.open(controller.directory / (label + '.tar'), 'w') as archive:
            for name, content in files.items():
                member = tarfile.TarInfo('./' + name)
                member.size = len(content)
                archive.addfile(member, io.BytesIO(content))
        return label

    def execute_fixture(self, controller, envelope, files):
        """Record actual scheduled envelopes and inspect their staged public contract."""
        self.seen.append(envelope)
        self.assertIn('guest_tester.py', files)
        self.assertIn('fixture_probe.py', files)
        self.assertEqual(envelope['settings'], self.fixture.profile.values['guest_tester']['settings'])
        return self.archive(controller, envelope)

    def provider_archive(self, controller, envelope, action=False):
        """Retain a failed attempt separately, including its exact task and side-effect boundary."""
        execution, rows, protocol = provider_refusal()
        if action:
            protocol.append({'direction': 'from-client', 'method': 'tools/call', 'id': 2, 'error': False})
        contents = {'task.json': json.dumps(envelope), 'execution.json': json.dumps(execution),
                    'events.jsonl': '\n'.join(map(json.dumps, rows)),
                    'managed-protocol.jsonl': '\n'.join(map(json.dumps, protocol))}
        label = 'tester-' + envelope['attempt']
        with tarfile.open(controller.directory / (label + '.tar'), 'w') as archive:
            for name, value in contents.items():
                raw = value.encode()
                member = tarfile.TarInfo('./' + name)
                member.size = len(raw)
                archive.addfile(member, io.BytesIO(raw))
        return label

    def prepare(self):
        """Require independently running owned peers before driving host import behavior."""
        self.fixture.prepare('p0-fresh')
        self.assertEqual(self.run.data['gui_backend'], 'guest-native')
        self.run.tester = self.execute_fixture

    def test_fresh_sequence_imports_native_receipts_and_revalidates_preflight(self):
        """Native readiness is an evidence dependency, not a sticky boolean pass label."""
        self.prepare()
        self.run.test('p0-fresh', source=self.source)
        self.assertEqual([value['check_id'] for value in self.seen],
                         ['native-preflight', 'artifact-verification', 'fixture-clean', 'install', 'app-start'])
        with self.run.locked():
            entry = self.run.selected('p0-fresh')
            self.assertEqual(self.run.verdict(entry), 'pass')
            preflight = next(iter(entry['native_preflight']['sha256']))
            archive = self.run.directory / preflight.split('::', 1)[0]
            archive.write_bytes(b'changed capture archive')
            self.assertEqual(self.run.verdict(entry), 'could not run')

    def test_provider_retry_classifier_refuses_actions_uncertainty_and_other_failures(self):
        """A status code alone must never authorize replay of an uncertain test action."""
        execution, rows, protocol = provider_refusal()
        self.assertTrue(rate_limited_without_actions(execution, rows, protocol))
        for changed in (dict(execution, cleanup_error='unknown group state'),
                        dict(execution, unverified_result={'status': 'fail'}), dict(execution, exit_code=None)):
            self.assertFalse(rate_limited_without_actions(changed, rows, protocol))
        for method in ('tools/call', 'elicitation/create', 'unknown'):
            self.assertFalse(rate_limited_without_actions(execution, rows,
                protocol + [{'direction': 'from-client', 'method': method, 'error': False}]))
        for changed in (rows[:-1], rows + [{'type': 'item.started'}],
                        rows[:-1] + [{'type': 'turn.failed', 'error': {'message': 'permission refused'}}]):
            self.assertFalse(rate_limited_without_actions(execution, changed, protocol))
        self.assertFalse(rate_limited_without_actions(execution, rows, []))

    def test_pre_action_provider_retry_preserves_attempt_and_its_evidence(self):
        """A fresh attempt can recover a proven pre-action refusal without erasing its predecessor."""
        self.prepare()
        delays, refused = [], []
        self.run.pause = delays.append

        def execute(controller, envelope, files):
            """Refuse the first native request before any action, then use ordinary accepted fixtures."""
            if not refused:
                refused.append(envelope['attempt'])
                return self.provider_archive(controller, envelope)
            return self.execute_fixture(controller, envelope, files)

        self.run.tester = execute
        self.run.test('p0-fresh', source=self.source)
        self.assertEqual(delays, [60])
        self.assertNotEqual(refused[0], self.seen[0]['attempt'])
        with self.run.locked():
            entry = self.run.selected('p0-fresh')
            self.assertEqual(self.run.verdict(entry), 'pass')
            history = entry['provider_retries']
            self.assertEqual(history[0]['attempt'], refused[0])
            self.assertEqual(self.run.report()['automation']['verdict'], 'recovered provider interruption')
            reference = next(iter(history[0]['sha256']))
            (self.run.directory / reference.split('::', 1)[0]).write_bytes(b'changed failed attempt')
            self.assertEqual(self.run.verdict(entry), 'could not run')

    def test_provider_retries_are_bounded_and_never_replay_an_action(self):
        """Persistent quota refusal stops after the bound; a possibly dispatched tool gets no retry at all."""
        for action, timeout, expected in ((False, 1200, 4), (True, 1200, 1), (False, 30, 1)):
            with self.subTest(action=action, timeout=timeout):
                case = NativeCoordinatorTests()
                case.setUp()
                self.addCleanup(case.doCleanups)
                case.prepare()
                attempts, delays = [], []
                case.run.pause = delays.append

                def execute(controller, envelope, files):
                    """Make every attempt fail, optionally after the protocol could have dispatched an action."""
                    attempts.append(envelope['attempt'])
                    return case.provider_archive(controller, envelope, action=action)

                case.run.tester = execute
                with self.assertRaisesRegex(Refused, 'guest-native automation failed'):
                    case.run.test('p0-fresh', source=case.source, timeout=timeout)
                self.assertEqual(len(attempts), expected)
                self.assertEqual(len(set(attempts)), expected)
                self.assertEqual(delays, [60, 120, 240] if expected == 4 else [])

    def test_p2_provider_failure_never_schedules_an_automatic_send_retry(self):
        """The live feedback boundary stays conservative even if a provider refusal looks pre-action."""
        self.finish_preceding_pairs()
        with self.run.locked():
            with self.assertRaisesRegex(Refused, 'feedback'):
                self.run.continue_full()
            self.assertEqual(self.run.data['scope'], 'p0-only')
        self.approve_feedback()
        with self.run.locked():
            self.run.continue_full()
        self.fixture.prepare('p0-installer-upgrade')
        delays = []
        self.run.pause = delays.append

        def execute(controller, envelope, files):
            """Complete P0 and P1, then fail before the first P2 action without adding another send attempt."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'p2-feedback-send':
                return self.provider_archive(controller, envelope)
            return self.archive(controller, envelope)

        self.run.tester = execute
        with mock.patch('brick_test.download', return_value=b'v0.24.0'), self.assertRaises(Refused):
            self.run.test('p0-installer-upgrade', source=self.source)
        self.assertEqual(delays, [])
        self.assertEqual(sum(row['check_id'] == 'p2-feedback-send' for row in self.seen), 1)

    def test_provider_retry_keeps_the_effective_old_sessions_budget(self):
        """A safe retry uses the checkpoint floor minus elapsed time and backoff."""
        self.finish_preceding_pairs()
        with self.run.locked():
            self.run.limit_scope('p1-only')
        self.fixture.prepare('p0-installer-upgrade')
        now = 100.0
        self.run.clock = lambda: now
        budgets = []

        def pause(delay):
            """Spend the retry delay in the same clock that measures the checkpoint."""
            nonlocal now
            now += delay

        self.run.pause = pause

        def execute(controller, envelope, files):
            """Refuse the old-sessions turn before it acts, after consuming ten seconds."""
            nonlocal now
            if envelope['check_id'] == 'old-sessions':
                budgets.append(envelope['timeout_seconds'])
                if len(budgets) == 1:
                    now += 10
                    return self.provider_archive(controller, envelope)
            return self.archive(controller, envelope)

        self.run.tester = execute
        with mock.patch('brick_test.download', return_value=b'v0.24.0'):
            self.run.test('p0-installer-upgrade', source=self.source, timeout=30)
        self.assertEqual(budgets, [1800, 1730])

    def test_unknown_native_image_cannot_supply_command_evidence(self):
        """Native bytes in an unfamiliar format cannot impersonate an independent command receipt."""
        self.prepare()
        def images_only(controller, envelope, files):
            """Keep valid native execution while withholding the app-start command observable."""
            self.seen.append(envelope)
            return self.archive(controller, envelope, image_only=envelope['check_id'] == 'app-start')
        self.run.tester = images_only
        with self.assertRaisesRegex(Refused, 'command/identity evidence'):
            self.run.test('p0-fresh', source=self.source)

    def test_nested_receipt_is_imported_and_revalidated_from_its_exact_tar_member(self):
        """Guest-relative evidence must survive host archive import without being flattened or silently omitted.

The synthetic pair proves only the transport and retention contract. Corrupting
the exported archive must invalidate that same pass, even after the nested
signature receipt was accepted.
"""
        self.prepare()
        def nested(controller, envelope, files):
            """Return a real tar member under the signed-input directory used by release verification."""
            return self.archive(controller, envelope, nested_receipt=True)
        self.run.tester = nested
        self.run.test('p0-fresh', source=self.source)
        with self.run.locked():
            entry = self.run.selected('p0-fresh')
            self.assertEqual(self.run.verdict(entry), 'pass')
            refs = entry['results']['artifact-verification']['sha256']
            receipt = next(ref for ref in refs if ref.endswith('::./signed-inputs/verification.json'))
            archive = self.run.directory / receipt.split('::', 1)[0]
            archive.write_bytes(b'changed nested receipt archive')
            self.assertEqual(self.run.verdict(entry), 'could not run')

    def test_malformed_clean_fixture_pass_is_a_harness_failure(self):
        """A model's pass label cannot replace a complete clean pair receipt at production import."""
        for defect in ('missing-peer', 'service', 'path', 'missing-receipt'):
            with self.subTest(defect=defect):
                case = NativeCoordinatorTests()
                case.setUp()
                self.addCleanup(case.doCleanups)
                case.prepare()
                def invalid(controller, envelope, files):
                    """Complete prerequisites but contradict the fixture claim in its independently retained receipt."""
                    return case.archive(controller, envelope, fixture_defect=defect)
                case.run.tester = invalid
                with self.assertRaisesRegex(Refused, 'fixture-clean') as failure:
                    case.run.test('p0-fresh', source=case.source)
                if defect == 'missing-receipt':
                    self.assertIsNone(failure.exception.__cause__)
                    self.assertIn('needs the deterministic pair receipt', str(failure.exception))
                else:
                    self.assertIsInstance(failure.exception.__cause__, ProbeError)
                    reason = ('needs a clean Mac and Linux pair' if defect == 'missing-peer'
                              else 'contains residue or missing observations')
                    self.assertIn(reason, str(failure.exception.__cause__))
                with case.run.locked():
                    report = case.run.report()
                    self.assertEqual(report['p0']['verdict'], 'could not run')
                    self.assertEqual(report['automation']['verdict'], 'fail')

    def drive_pair(self, pass_id, tester=None):
        """Drive one owned modeled pair through native import with a controlled public default.

This composes the real scheduler and receipt gates across all P0 fixtures;
neither the update endpoint nor the native capture is a live product oracle.
"""
        self.fixture.prepare(pass_id)
        self.run.tester = tester or self.execute_fixture
        with mock.patch('brick_test.download', return_value=b'v0.24.0'):
            self.run.test(pass_id, source=self.source)

    def finish_preceding_pairs(self):
        """Retain valid native P0 evidence before the installer pair, the last one, runs its P0, P1 and P2."""
        for pass_id in self.run.data['plan']['p0_sequence'][:-1]:
            self.drive_pair(pass_id)
            with self.run.locked():
                self.run.cleanup(pass_id)

    def approve_feedback(self):
        """Record fictional send consent separately from the fixture's credential consent."""
        path = self.fixture.root / 'consent.json'
        value = json.loads(path.read_text())
        value['feedback_send_approved'] = True
        path.write_text(json.dumps(value))

    def widen(self, scope):
        """Set a run scope directly, standing in for an `init` that already resolved its consent."""
        with self.run.locked():
            self.run.data['scope'] = scope
            self.run.save()

    def test_p0_only_never_dispatches_continuity_or_feedback_and_cleans_final_pair(self):
        """The action fence must precede guest execution, since record-time refusal is too late."""
        self.finish_preceding_pairs()
        self.drive_pair('p0-installer-upgrade')
        dispatched = [value['check_id'] for value in self.seen]
        for check in ('old-sessions', 'local-old-session', 'remote-host-remove', 'p2-feedback-send'):
            self.assertNotIn(check, dispatched)
        self.assertTrue(all('feedback_authorized' not in value['context'] for value in self.seen))
        with self.run.locked():
            self.assertEqual(self.run.report()['p0']['verdict'], 'pass')
            self.assertEqual(self.run.report()['requested_verdict'], 'pass')
            self.run.cleanup('p0-installer-upgrade')

    def test_installer_pair_dispatches_supervisor_upgrades_before_agents_and_p1_after_p0(self):
        """The dispatched guest turns, not only the declared manifest, follow the priority order.

The old sessions start before the relaunch, both supervisors' upgrades are
observed before either new agent, and the old-session and remote menu turns
follow every P0 turn. The long-turn floors apply to the agent conversations.
"""
        self.finish_preceding_pairs()
        self.widen('p1-only')
        self.seen.clear()
        self.drive_pair('p0-installer-upgrade')
        self.assertEqual([value['check_id'] for value in self.seen],
                         ['native-preflight', 'artifact-verification', 'old-fixture', 'old-sessions', 'update-relaunch',
                          'local-supervisor-upgrade', 'remote-supervisor-upgrade', 'local-supervisor-agent',
                          'remote-supervisor-agent', 'local-old-session', 'remote-old-session', 'remote-host-remove',
                          'remote-host-uninstall'])
        remote = next(value for value in self.seen if value['check_id'] == 'remote-old-session')
        self.assertIn('remote_host_survival', remote['instructions'])
        budgets = {value['check_id']: value['timeout_seconds'] for value in self.seen}
        self.assertEqual((budgets['old-sessions'], budgets['local-old-session'], budgets['update-relaunch']),
                         (1800, 1800, 1200))
        with self.run.locked():
            self.assertEqual(self.run.report()['p0']['verdict'], 'pass')
            self.assertEqual(self.run.report()['p1']['verdict'], 'pass')

    def test_remote_mac_pass_imports_native_evidence_and_only_authenticates_the_gui_mac(self):
        """The P1 peer needs a plain command, with no copied Codex login or bonus readiness."""
        f = self.fixture
        f.enable_p0(scope='p1-only', remote_mac=True)
        self.run = f.run
        self.finish_preceding_pairs()
        self.drive_pair('p0-installer-upgrade')
        with self.run.locked():
            self.run.cleanup('p0-installer-upgrade')
        first_call = len(f.model.calls)
        self.seen.clear()
        self.drive_pair(REMOTE_MAC_PASS)
        self.assertEqual([value['check_id'] for value in self.seen],
                         ['native-preflight', 'artifact-verification', 'fixture-clean', 'install',
                          'app-start', 'remote-provision', 'remote-host-uninstall'])
        self.assertIn('This preflight does not require drag or double-click probes', self.seen[0]['instructions'])
        self.assertTrue(all('addendum' not in value['context'] for value in self.seen))
        self.assertIn('agent type none, /bin/sh', self.seen[-1]['instructions'])
        authorized = [argv[3] for argv in f.model.calls[first_call:]
                      if argv[1:3] == ['exec', '-i'] and '"fixture contains authentication"' in argv[-1]]
        self.assertEqual(len(authorized), 1)
        self.assertTrue(authorized[0].endswith('-mac'))
        self.assertFalse(authorized[0].endswith('-remote-mac'))
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p1']['checks']['remote-mac-uninstall'], 'pass')
            self.assertEqual(report['p1']['verdict'], 'pass')
            self.assertEqual(report['requested_verdict'], 'pass')
            self.assertEqual(report['p2']['verdict'], 'could not run')
            entry = self.run.selected(REMOTE_MAC_PASS)
            preflight = entry.pop('native_preflight')
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup(REMOTE_MAC_PASS)
            self.assertEqual(entry['phase'], 'ready')
            entry['native_preflight'] = preflight
            # Narrowed scope leaves this pair with no required P0 results to guard preparation.
            entry['preparation_error'] = 'fictional incomplete setup'
            self.run.data['scope'] = 'p0-only'
            calls_before = len(f.model.calls)
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup(REMOTE_MAC_PASS)
            self.assertEqual(len(f.model.calls), calls_before)
            del entry['preparation_error']
            self.run.data['scope'] = 'p1-only'
            self.run.cleanup(REMOTE_MAC_PASS)

    def test_a_failed_p1_turn_skips_the_later_p1_turns_and_still_sends_feedback(self):
        """A lost session stops the checks that share it, but not the independent feedback send.

The skipped turns are never dispatched, the run ends without an automation
failure, and the pair cleans up normally with the failure's evidence retained.
"""
        self.finish_preceding_pairs()
        self.approve_feedback()
        self.widen('full')
        self.seen.clear()

        def lost_session(controller, envelope, files):
            """Fail only the local old session; every other turn passes."""
            self.seen.append(envelope)
            return self.archive(controller, envelope, 'fail' if envelope['check_id'] == 'local-old-session' else 'pass')

        self.drive_pair('p0-installer-upgrade', lost_session)
        dispatched = [value['check_id'] for value in self.seen]
        self.assertEqual(dispatched[-3:], ['local-old-session', 'p2-feedback-send', 'p2-announcements'])
        for check in ('remote-old-session', 'remote-host-remove', 'remote-host-uninstall'):
            self.assertNotIn(check, dispatched)
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'pass')
            self.assertEqual(report['p1']['verdict'], 'fail')
            self.assertEqual(report['p2']['verdict'], 'pass')
            self.assertEqual(report['automation']['verdict'], 'no failure observed')
            self.assertEqual(self.run.selected('p0-installer-upgrade')['tester']['phase'], 'complete')
            self.run.cleanup('p0-installer-upgrade')
            self.run.require_priority('fresh')

    def test_unavailable_old_sessions_still_dispatch_every_p0_turn(self):
        """The previous release failing to start its sessions must not cost the supervisor checks."""
        self.finish_preceding_pairs()
        self.widen('p1-only')
        self.seen.clear()

        def no_baseline(controller, envelope, files):
            """Report the old sessions unavailable; every other turn passes."""
            self.seen.append(envelope)
            return self.archive(controller, envelope,
                                'could-not-run' if envelope['check_id'] == 'old-sessions' else 'pass')

        self.drive_pair('p0-installer-upgrade', no_baseline)
        dispatched = [value['check_id'] for value in self.seen]
        self.assertEqual(dispatched[-1], 'remote-supervisor-agent')
        self.assertIn('local-supervisor-upgrade', dispatched)
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'pass')
            self.assertEqual(report['p1']['verdict'], 'could not run')
            self.run.cleanup('p0-installer-upgrade')

    def test_full_and_p2_setup_refuse_without_send_consent_before_mutation(self):
        """Known missing send authority must not first pause an otherwise completed P0 run.

A stop after P1 includes no send, so it needs no such consent.
"""
        self.prepare()
        self.assertEqual(self.run.data['passes']['p0-fresh']['phase'], 'ready')
        with self.run.locked():
            controller = self.run.pair('p0-fresh')
        with controller.locked():
            self.assertTrue(all(controller.state(role)['State'] == 'running' for role in ('mac', 'linux')))
        for scope in ('full', 'p2-only'):
            with self.subTest(scope=scope):
                self.widen(scope)
                calls = len(self.fixture.model.calls)
                with self.assertRaisesRegex(Refused, 'live P2 feedback'):
                    self.run.test('p0-fresh', source=self.source)
                self.assertEqual(len(self.fixture.model.calls), calls)
                self.assertEqual(self.seen, [])
                with self.run.locked():
                    self.assertNotIn('credentials_injected', self.run.selected('p0-fresh'))
        self.widen('p1-only')
        self.run.test('p0-fresh', source=self.source)
        with self.run.locked():
            self.assertEqual(self.run.verdict(self.run.selected('p0-fresh')), 'pass')

    def test_valid_failed_p2_keeps_p0_and_p1_and_allows_cleanup_and_bonus(self):
        """A completed feedback failure cannot erase valid upgrades or become an automation fault."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        self.widen('full')
        def failed_send(controller, envelope, files):
            """Return a valid product failure only at the separately authorized send boundary."""
            self.seen.append(envelope)
            return self.archive(controller, envelope, 'fail' if envelope['check_id'] == 'p2-feedback-send' else 'pass')
        self.drive_pair('p0-installer-upgrade', failed_send)
        feedback = [value for value in self.seen if value['check_id'] == 'p2-feedback-send']
        self.assertEqual(len(feedback), 1)
        self.assertIs(feedback[0]['context']['feedback_authorized'], True)
        self.assertNotIn('addendum.md', feedback[0]['instructions'])
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'pass')
            self.assertEqual(report['p1']['verdict'], 'pass')
            self.assertEqual(report['p2']['verdict'], 'fail')
            self.assertEqual(report['p2']['checks'], {'feedback-send': 'fail', 'announcements': 'pass'})
            self.assertEqual(report['automation']['verdict'], 'no failure observed')
            self.assertNotIn('automation_failure', self.run.selected('p0-installer-upgrade'))
            self.run.cleanup('p0-installer-upgrade')
            self.run.require_priority('fresh')

    def test_removed_feedback_consent_stops_before_p2_dispatch(self):
        """Setup approval must still exist at the action boundary, after the final P0 and P1 observations."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        self.widen('full')
        def removed_consent(controller, envelope, files):
            """Withdraw only fictional send consent while an earlier task of the pair runs."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'remote-old-session':
                path = self.fixture.root / 'consent.json'
                value = json.loads(path.read_text())
                value.pop('feedback_send_approved')
                path.write_text(json.dumps(value))
            return self.archive(controller, envelope)
        with self.assertRaisesRegex(Refused, 'live P2 feedback'):
            self.drive_pair('p0-installer-upgrade', removed_consent)
        self.assertNotIn('p2-feedback-send', [value['check_id'] for value in self.seen])
        with self.run.locked():
            self.assertEqual(self.run.report()['p0']['verdict'], 'pass')
            self.assertEqual(self.run.report()['p2']['verdict'], 'could not run')
            self.assertEqual(self.run.selected('p0-installer-upgrade')['automation_failure']['kind'], 'unavailable')
            self.assertEqual(self.run.report()['automation']['verdict'], 'no failure observed')

    def test_mid_turn_stop_fences_the_tiers_not_yet_dispatched(self):
        """A late stop request must be honored before any queued task of a narrower-excluded tier runs.

A stop after P0 made during the last P0 turn leaves every P1 and P2 turn
undispatched; a stop after P1 made during the last P1 turn leaves the send.
"""
        for scope, during, excluded in (('p0-only', 'remote-supervisor-agent', ('local-old-session', 'p2-feedback-send')),
                                        ('p1-only', 'remote-host-uninstall', ('p2-feedback-send',))):
            with self.subTest(scope=scope):
                case = NativeCoordinatorTests()
                case.setUp()
                self.addCleanup(case.doCleanups)
                case.finish_preceding_pairs()
                case.approve_feedback()
                case.widen('full')

                def narrowed(controller, envelope, files):
                    """Persist the user's scope change while the modeled task is outside the workflow lock."""
                    case.seen.append(envelope)
                    if envelope['check_id'] == during:
                        with case.run.locked():
                            case.run.limit_scope(scope)
                    return case.archive(controller, envelope)

                case.drive_pair('p0-installer-upgrade', narrowed)
                dispatched = [value['check_id'] for value in case.seen]
                self.assertEqual(dispatched[-1], during)
                for check in excluded:
                    self.assertNotIn(check, dispatched)
                with case.run.locked():
                    self.assertEqual(case.run.report()['requested_verdict'], 'pass')
                    case.run.cleanup('p0-installer-upgrade')

    def test_changed_p0_evidence_fences_feedback_before_guest_dispatch(self):
        """Record-time refusal cannot undo a live send after an earlier P0 premise was invalidated.

Lost P0 evidence is not a dead end that a skip could settle: it stops the
scheduler before the send and leaves the pair live for the maintainer.
"""
        self.finish_preceding_pairs()
        self.approve_feedback()
        self.widen('full')
        with self.run.locked():
            archive = self.run.selected('p0-fresh')['results']['app-start']['evidence'][0].split('::', 1)[0]
        def changed_prior(controller, envelope, files):
            """Damage independently retained earlier evidence while a later task of the pair runs."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'remote-host-uninstall':
                (self.run.directory / archive).write_bytes(b'changed earlier archive')
            return self.archive(controller, envelope)
        with self.assertRaisesRegex(Refused, 'valid evidence before P2'):
            self.drive_pair('p0-installer-upgrade', changed_prior)
        self.assertNotIn('p2-feedback-send', [value['check_id'] for value in self.seen])
        with self.run.locked():
            self.assertEqual(self.run.report()['p0']['verdict'], 'could not run')
            self.assertNotIn('p2-feedback-send', self.run.selected('p0-installer-upgrade')['results'])
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup('p0-installer-upgrade')

    def test_native_p2_pass_satisfies_its_requested_scope(self):
        """A genuine native receipt path must preserve the successful stop-after-P2 verdict and ordinary cleanup."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        self.widen('p2-only')
        self.drive_pair('p0-installer-upgrade')
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'pass')
            self.assertEqual(report['p1']['verdict'], 'pass')
            self.assertEqual(report['p2']['verdict'], 'pass')
            self.assertEqual(report['p2']['checks'], {'feedback-send': 'pass', 'announcements': 'pass'})
            self.assertEqual(report['requested_verdict'], 'pass')
            self.run.cleanup('p0-installer-upgrade')

    def test_missing_announcements_feed_preserves_all_earlier_results(self):
        """Import the absent feed as a single terminal P2 result and leave bonus coverage available."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        self.widen('full')
        detail = 'announcements-test.json is not published yet (HTTP 404)'

        def missing_feed(controller, envelope, files):
            self.seen.append(envelope)
            if envelope['check_id'] == 'p2-announcements':
                self.assertNotIn('feedback_authorized', envelope['context'])
                return self.archive(controller, envelope, 'fail', detail=detail)
            return self.archive(controller, envelope)

        self.drive_pair('p0-installer-upgrade', missing_feed)
        with self.run.locked():
            report = self.run.report()
            self.assertEqual((report['p0']['verdict'], report['p1']['verdict']), ('pass', 'pass'))
            self.assertEqual(report['p2']['checks'], {'feedback-send': 'pass', 'announcements': 'fail'})
            entry = self.run.selected('p0-installer-upgrade')
            self.assertEqual(entry['results']['p2-announcements']['detail'], detail)
            self.assertNotIn('automation_failure', entry)
            self.run.cleanup('p0-installer-upgrade')
            self.run.require_priority('fresh')

    def test_rc_native_dispatch_runs_every_planned_check_and_reports_discovery_skipped(self):
        """Exercise the real serial dispatcher on an RC, whose plan has no updater pair.

The installer pair runs the manual relaunch, both supervisor upgrades, the
agents, both old-session checks, the remote menu and the approved send. P0
stays formally incomplete, with a summary saying every runnable check passed.
"""
        self.fixture.enable_p0(candidate='v0.25.0-rc.1', scope='p0-only')
        self.run = self.fixture.run
        self.finish_preceding_pairs()
        self.approve_feedback()
        with self.run.locked():
            self.run.continue_full()
        self.seen.clear()
        self.drive_pair('p0-installer-upgrade')
        dispatched = [value['check_id'] for value in self.seen]
        self.assertNotIn('update-discovery', dispatched)
        for check in ('remote-supervisor-upgrade', 'local-old-session', 'remote-old-session', 'remote-host-uninstall'):
            self.assertIn(check, dispatched)
        self.assertEqual(dispatched[-2:], ['p2-feedback-send', 'p2-announcements'])
        relaunch = next(value for value in self.seen if value['check_id'] == 'update-relaunch')
        self.assertEqual(relaunch['context']['update_path'], 'manual')
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'could not run')
            self.assertIn('All runnable P0 checks passed.', report['p0']['summary'])
            self.assertEqual(report['p2']['verdict'], 'pass')
            self.run.cleanup('p0-installer-upgrade')

    def test_stable_manual_path_fails_required_discovery_without_hiding_the_supervisors(self):
        """A stable release requires discovery even when its old app lacks the override.

The failed updater pair keeps its guests until the maintainer decides; once
released, the installer pair still checks the supervisors and P1, and skips
the send, which can no longer follow a passing P0.
"""
        self.fixture.enable_p0(candidate='v0.25.0', supported=False, scope='p0-only')
        self.run = self.fixture.run
        self.drive_pair('p0-fresh')
        with self.run.locked():
            self.run.cleanup('p0-fresh')
        self.approve_feedback()
        with self.run.locked():
            self.run.continue_full()
        self.seen.clear()
        with self.assertRaisesRegex(Refused, 'discovery is required for stable release v0.25.0'):
            self.drive_pair('p0-updater-upgrade')
        self.assertEqual([value['check_id'] for value in self.seen],
                         ['native-preflight', 'artifact-verification', 'old-fixture'])
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'fail')
            self.assertEqual(report['p0']['checks']['click-to-upgrade'], 'fail')
            self.assertEqual(report['verdict'], 'fail')
            self.assertNotIn('unavailable_checks', report['p0'])
            self.assertNotIn('summary', report['p0'])
            result = self.run.selected('p0-updater-upgrade')['results']['update-discovery']
            self.assertEqual(result['evidence'], ['p0-updater-upgrade/latest-at-selection.json'])
            self.assertTrue(self.run.result_evidence_retained(result))
            with self.assertRaises(Refused):
                self.run.cleanup('p0-updater-upgrade')
            self.run.cleanup('p0-updater-upgrade', discard_failed=True)
        self.seen.clear()
        self.drive_pair('p0-installer-upgrade')
        dispatched = [value['check_id'] for value in self.seen]
        self.assertIn('remote-supervisor-upgrade', dispatched)
        self.assertEqual(dispatched[-1], 'remote-host-uninstall')
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['checks']['remote-supervisor-upgrade'], 'pass')
            self.assertEqual(report['p1']['verdict'], 'pass')
            self.assertEqual(report['p2']['verdict'], 'could not run')
            self.run.cleanup('p0-installer-upgrade')

    def test_in_flight_p2_outcome_survives_late_scope_narrowing(self):
        """A scope request cannot recall a dispatched send or erase its actual retained result."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        self.widen('full')
        def in_flight(controller, envelope, files):
            """Narrow scope only after the modeled P2 task is already dispatched."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'p2-feedback-send':
                with self.run.locked():
                    self.run.limit_scope('p0-only')
            return self.archive(controller, envelope)
        self.drive_pair('p0-installer-upgrade', in_flight)
        with self.run.locked():
            entry = self.run.selected('p0-installer-upgrade')
            self.assertTrue(entry['results']['p2-feedback-send']['dispatched_before_scope_stop'])
            self.assertNotIn('dispatched_before_scope_stop', entry['results']['remote-host-uninstall'])
            self.assertNotIn('automation_failure', entry)
            self.assertNotIn('p2-announcements', entry['results'])
            report = self.run.report()
            self.assertEqual(report['p2']['checks'], {'feedback-send': 'pass', 'announcements': 'could not run'})
            self.assertEqual(report['requested_verdict'], 'pass')
            self.run.cleanup('p0-installer-upgrade')

    def test_native_p2_fault_keeps_p0_but_fences_bonus_and_cleanup(self):
        """P2's nonblocking product exception must never hide an aborted native tester or an unverified break."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        self.widen('full')
        def fault(controller, envelope, files):
            """Supply an aborted-turn observation only at P2, after independently passed P0 and P1."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'p2-feedback-send':
                return self.archive(controller, envelope, 'automation-failure', unverified='fail')
            return self.archive(controller, envelope)
        with self.assertRaises(Refused):
            self.drive_pair('p0-installer-upgrade', fault)
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'pass')
            self.assertEqual(report['p2']['verdict'], 'could not run')
            self.assertEqual(report['automation']['verdict'], 'fail')
            self.assertEqual(report['p0']['unverified_product_failures'], [])
            self.assertEqual(report['p1']['unverified_product_failures'], [])
            self.assertEqual(len(report['p2']['unverified_product_failures']), 1)
            with self.assertRaisesRegex(Refused, 'first failed'):
                self.run.require_priority('fresh')
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup('p0-installer-upgrade')

    def test_p2_pass_requires_two_distinct_native_captures(self):
        """The structural gate requires two captures; their before/after meaning still needs independent review."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        self.widen('p2-only')
        def one_capture(controller, envelope, files):
            """Keep an otherwise valid P2 receipt with only one real modeled native capture."""
            self.seen.append(envelope)
            return self.archive(controller, envelope, two_captures=envelope['check_id'] != 'p2-feedback-send')
        with self.assertRaisesRegex(Refused, 'two distinct native'):
            self.drive_pair('p0-installer-upgrade', one_capture)

    def test_announcements_pass_requires_the_history_capture(self):
        """Two native images can show the action but leave persistence into history unobserved."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        self.widen('p2-only')
        def missing_history(controller, envelope, files):
            self.seen.append(envelope)
            return self.archive(controller, envelope, two_captures=envelope['check_id'] != 'p2-announcements')
        with self.assertRaisesRegex(Refused, 'three distinct native'):
            self.drive_pair('p0-installer-upgrade', missing_history)
        self.assertEqual(self.seen[-1]['check_id'], 'p2-announcements')

    def test_remote_menu_pass_requires_action_and_result_native_captures(self):
        """Neither a menu-only image nor a result-only image establishes the requested remote action.

Exercise each checkpoint through the real import gate. Earlier checkpoints
retain their valid captures so a refusal cannot accidentally come from P0
setup, the agent reply or a different menu check.
"""
        for check in ('remote-host-remove', 'remote-host-uninstall'):
            with self.subTest(check=check):
                case = NativeCoordinatorTests()
                case.setUp()
                self.addCleanup(case.doCleanups)
                case.finish_preceding_pairs()
                case.widen('p1-only')
                def one_capture(controller, envelope, files):
                    """Withhold only the selected menu result's second native capture."""
                    case.seen.append(envelope)
                    return case.archive(controller, envelope, two_captures=envelope['check_id'] != check)
                with case.assertRaisesRegex(Refused, check + ' needs two distinct native'):
                    case.drive_pair('p0-installer-upgrade', one_capture)
                self.assertEqual(case.seen[-1]['check_id'], check)

    def harness_failure_archive(self, controller, envelope, observation=None):
        """Retain a stopped automation-failure attempt whose one tool call completed with an error."""
        call = {'id': 'read', 'type': 'mcp_tool_call', 'server': 'cua_repl', 'tool': 'brick_action',
                'arguments': {'operation': 'remote_inspect_state'}, 'status': 'failed'}
        rows = [{'type': 'thread.started'}, {'type': 'turn.started'}, {'type': 'item.started', 'item': call},
                {'type': 'item.completed', 'item': call},
                {'type': 'turn.failed', 'error': {'message': 'managed action failed: fixture read error'}}]
        execution = {'format': 1, 'status': 'automation-failure', 'exit_code': 1, 'cleanup_error': None,
                     'error': 'managed action failed: OperationalError: database is locked'}
        if observation is not None:
            execution['unverified_result'] = {
                'identity': {key: envelope[key] for key in ('attempt', 'run_id', 'pass_id', 'check_id')},
                'status': observation, 'detail': 'Retained native observation.', 'evidence': [], 'continuity': None}
        label = 'tester-' + envelope['attempt']
        with tarfile.open(controller.directory / (label + '.tar'), 'w') as archive:
            for name, value in {'task.json': json.dumps(envelope), 'execution.json': json.dumps(execution),
                                'events.jsonl': '\n'.join(map(json.dumps, rows))}.items():
                raw = value.encode()
                member = tarfile.TarInfo('./' + name)
                member.size = len(raw)
                archive.addfile(member, io.BytesIO(raw))
        return label

    def test_a_harness_failure_at_p1_makes_it_unavailable_and_the_pair_goes_on(self):
        """A harness failure at an optional P1 checkpoint must not keep the pair's P0 checks or the send from running.

`old-sessions` runs before the upgrade, so stopping the pair there would hide
whether the supervisors brick. The failed attempt is retained and the check
imports as could-not-run, which stops the later P1 checks (recorded as
skipped) while the P0 checks after it and P2 still run.
"""
        self.finish_preceding_pairs()
        self.approve_feedback()
        self.widen('full')
        self.fixture.prepare('p0-installer-upgrade')

        def execute(controller, envelope, files):
            """Fail only the baseline turn in the harness; pass everything else."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'old-sessions':
                return self.harness_failure_archive(controller, envelope)
            return self.archive(controller, envelope)

        self.run.tester = execute
        self.seen.clear()
        with mock.patch('brick_test.download', return_value=b'v0.24.0'):
            self.run.test('p0-installer-upgrade', source=self.source)
        dispatched = [value['check_id'] for value in self.seen]
        self.assertIn('remote-supervisor-agent', dispatched)
        self.assertEqual(dispatched[-2:], ['p2-feedback-send', 'p2-announcements'])
        self.assertNotIn('local-old-session', dispatched)
        with self.run.locked():
            entry = self.run.selected('p0-installer-upgrade')
            self.assertNotIn('automation_failure', entry)
            self.assertEqual(entry['results']['old-sessions']['status'], 'could-not-run')
            self.assertIn('Guest automation failed', entry['results']['old-sessions']['detail'])
            self.assertEqual(entry['results']['local-old-session']['status'], 'skipped')
            report = self.run.report()
            self.assertEqual((report['p0']['verdict'], report['p1']['verdict']), ('pass', 'could not run'))

    def test_p1_permission_refusal_survives_an_automation_failure(self):
        """A denied OS permission stops dispatch even if the executor could only save its observation."""
        self.finish_preceding_pairs()
        self.widen('p1-only')

        def execute(controller, envelope, files):
            """Finish the earlier fixture checkpoints before the baseline encounters the refusal."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'old-sessions':
                return self.harness_failure_archive(controller, envelope, 'permission-block')
            return self.archive(controller, envelope)

        with self.assertRaisesRegex(Refused, 'guest-native automation failed'):
            self.drive_pair('p0-installer-upgrade', execute)
        self.assertEqual(self.seen[-1]['check_id'], 'old-sessions')
        with self.run.locked():
            entry = self.run.selected('p0-installer-upgrade')
            self.assertNotIn('old-sessions', entry['results'])
            self.assertEqual(entry['tester']['guest_status'], 'permission-block')
            self.assertEqual(entry['tester']['phase'], 'failed')

    def test_p1_failure_claim_is_saved_before_importing_its_evidence(self):
        """A failure to import the archive must leave the guest's product claim available for review."""
        self.finish_preceding_pairs()
        self.widen('p1-only')
        record = self.run.record

        def execute(controller, envelope, files):
            self.seen.append(envelope)
            if envelope['check_id'] == 'local-old-session':
                return self.harness_failure_archive(controller, envelope, 'fail')
            return self.archive(controller, envelope)

        def reject_import(pass_id, check_id, *args, **kwargs):
            if check_id == 'local-old-session':
                entry = json.loads(self.run.path.read_text())['passes'][pass_id]
                self.assertEqual(entry['unverified_failures'][0]['identity']['check_id'], check_id)
                raise Refused('fictional evidence import failure')
            return record(pass_id, check_id, *args, **kwargs)

        with mock.patch.object(self.run, 'record', side_effect=reject_import):
            with self.assertRaisesRegex(Refused, 'fictional evidence import failure'):
                self.drive_pair('p0-installer-upgrade', execute)
        self.assertTrue(self.run.report()['promotion_blocked_pending_review'])

    def test_a_stop_during_bonus_work_fences_the_remaining_bonus_turns(self):
        """Bonus checkpoints obey a mid-run stop like every tier: only the in-flight one finishes."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        self.widen('full')
        self.drive_pair('p0-installer-upgrade')
        with self.run.locked():
            self.run.cleanup('p0-installer-upgrade')
        (self.run.directory / 'addendum.md').write_text('Fictional addendum for scheduling only.\n')

        def narrowed(controller, envelope, files):
            """Ask to stop after P2 while the first bonus product checkpoint runs."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'artifact-verification':
                with self.run.locked():
                    self.run.limit_scope('p2-only')
            return self.archive(controller, envelope)

        self.seen.clear()
        self.drive_pair('fresh', narrowed)
        self.assertEqual([value['check_id'] for value in self.seen], ['native-preflight', 'artifact-verification'])
        with self.run.locked():
            result = self.run.selected('fresh')['results']['artifact-verification']
            self.assertTrue(result['dispatched_before_scope_stop'])

    def assert_stopped_outcome(self, status, expected):
        """Drive a distinct owned run through readiness to one nonpass observation."""
        self.prepare()
        def stopped(controller, envelope, files):
            """Complete readiness, then supply the exact nonpass observation under test."""
            self.seen.append(envelope)
            return self.archive(controller, envelope, status if envelope['check_id'] != 'native-preflight' else 'pass')
        self.run.tester = stopped
        with self.assertRaises(Refused):
            self.run.test('p0-fresh', source=self.source)
        with self.run.locked():
            entry = self.run.selected('p0-fresh')
            self.assertEqual(entry['results']['artifact-verification']['status'], status)
            self.assertEqual(self.run.verdict(entry), expected)
            self.assertNotIn('automation_failure', entry)
        self.assertEqual([v['check_id'] for v in self.seen], ['native-preflight', 'artifact-verification'])

    def test_unavailable_observation_stays_could_not_run(self):
        """Missing product prerequisites retain their observation without becoming a failure."""
        self.assert_stopped_outcome('could-not-run', 'could not run')
        with self.assertRaisesRegex(Refused, 'supported retry transition'):
            self.run.test('p0-fresh', source=self.source)
        with self.run.locked():
            entry = self.run.selected('p0-fresh')
            self.assertEqual(entry['tester']['phase'], 'failed')
            self.assertNotIn('automation_failure', entry)

    def test_failed_observation_stays_failure(self):
        """An actual product failure stops without a later successful check erasing it."""
        self.assert_stopped_outcome('fail', 'fail')

    def test_unacknowledged_cleanup_keeps_active_fence(self):
        """A failed verdict cannot authorize cleanup while guest termination is uncertain."""
        self.prepare()
        self.run.tester = mock.Mock(side_effect=CleanupPending('missing stopped acknowledgement'))
        with self.assertRaises(CleanupPending):
            self.run.test('p0-fresh', source=self.source)
        with self.run.locked():
            self.assertEqual(self.run.selected('p0-fresh')['tester']['phase'], 'reconciliation-required')
            with self.assertRaisesRegex(Refused, 'active'):
                self.run.cleanup('p0-fresh', discard_failed=True)
        with self.assertRaisesRegex(Refused, 'active tester'):
            self.run.test('p0-fresh', source=self.source)

    def readiness_failure(self, unsafe=False, stopped=True):
        """Retain the observed legacy early-sample error on an owned, ready fixture.

Scheduling is deliberately reduced to the one unfinished checkpoint here;
the test concerns retry authority, not the already covered P0 fixture order.
No native product action occurs unless the caller asks for the unsafe case.
"""
        self.prepare()
        entry = self.run.selected('p0-fresh')
        check = 'local-supervisor-agent'
        entry['required'] = {check: {'gui': True, 'command': True, 'continuity': False, 'priority': 'P0'}}
        envelope = task()
        envelope.update(run_id=self.run.data['run_id'], pass_id='p0-fresh', check_id=check)
        identity = {key: envelope[key] for key in ('run_id', 'pass_id', 'attempt', 'check_id')}
        error = 'RuntimeError: sample the new session terminal hosts only after the new agent session exists'
        call = {'id': 'sample', 'type': 'mcp_tool_call', 'server': 'cua_repl', 'tool': 'brick_action',
                'arguments': {'operation': 'terminal_hosts'}, 'status': 'failed', 'error': None,
                'result': {'content': [{'type': 'text', 'text': error}]}}
        rows = [{'type': 'item.started', 'item': call}, {'type': 'item.completed', 'item': call},
                {'type': 'turn.completed'}]
        if unsafe:
            click = dict(call, id='click', tool='brick_ui', arguments={'target': 'farhelm', 'method': 'click', 'args': [4]},
                         result={'content': []})
            rows.insert(0, {'type': 'item.completed', 'item': click})
        summary = {'status': 'automation-failure', 'cleanup_error': None,
                   'result': {'identity': identity, 'status': 'automation-failure', 'detail': error}}
        archive_path = self.run.directory / 'p0-fresh' / ('tester-' + envelope['attempt'] + '.tar')
        with tarfile.open(archive_path, 'w') as archive:
            for name, data in {'task.json': envelope, 'execution.json': summary,
                               'managed-failure.json': {'error': error}}.items():
                raw = json.dumps(data).encode()
                info = tarfile.TarInfo('./' + name)
                info.size = len(raw)
                archive.addfile(info, io.BytesIO(raw))
            raw = '\n'.join(map(json.dumps, rows)).encode()
            info = tarfile.TarInfo('./events.jsonl')
            info.size = len(raw)
            archive.addfile(info, io.BytesIO(raw))
        entry['tester'] = dict(identity, phase='failed', termination_observed=stopped)
        entry['automation_failure'] = {'check_id': check, 'kind': 'harness-failure',
                                       'status': 'could-not-run', 'detail': error}
        self.run.save()
        return archive_path

    def test_readiness_retry_preserves_failed_bytes_and_starts_a_fresh_checkpoint(self):
        """A proven pre-action failure can recover without relabeling or replaying its attempt."""
        path = self.readiness_failure()
        original = path.read_bytes()
        attempt = self.run.selected('p0-fresh')['tester']['attempt']
        self.run.retry_readiness('p0-fresh')
        entry = self.run.selected('p0-fresh')
        self.assertNotIn('automation_failure', entry)
        self.assertEqual(entry['readiness_retries'][0]['tester']['attempt'], attempt)
        self.assertEqual(path.read_bytes(), original)
        self.assertEqual(self.run.report()['automation']['verdict'], 'recovered readiness failure')
        self.run.tester = self.execute_fixture
        self.run.test('p0-fresh', source=self.source)
        entry = self.run.selected('p0-fresh')
        self.assertTrue(all(item['attempt'] != attempt for item in self.seen))
        self.assertEqual(entry['results']['local-supervisor-agent']['status'], 'pass')
        self.assertEqual(path.read_bytes(), original)
        path.unlink()
        self.assertEqual(self.run.verdict(entry), 'could not run', 'lost failure evidence must invalidate recovery')

    def test_readiness_retry_ignores_an_unavailable_p1_result_before_it(self):
        """An unavailable old-session baseline earlier in the pair must not hold back a supported P0 recovery."""
        self.readiness_failure()
        entry = self.run.selected('p0-fresh')
        check = next(iter(entry['required']))
        entry['required'] = {'old-sessions': {'gui': True, 'command': True, 'continuity': False, 'priority': 'P1'},
                             check: entry['required'][check]}
        entry['results']['old-sessions'] = {'status': 'could-not-run', 'evidence': [], 'sha256': {},
                                            'detail': 'Synthetic unavailable baseline.'}
        self.run.save()
        self.run.retry_readiness('p0-fresh')
        self.assertEqual(self.run.selected('p0-fresh')['tester']['phase'], 'retry-ready')

    def test_readiness_retry_refuses_actions_or_uncertain_termination(self):
        """A click or missing stopped acknowledgement cannot be waved through as a harmless read."""
        for unsafe, stopped in ((True, True), (False, False)):
            with self.subTest(unsafe=unsafe, stopped=stopped):
                case = NativeCoordinatorTests()
                case.setUp()
                try:
                    case.readiness_failure(unsafe=unsafe, stopped=stopped)
                    with self.assertRaises(Refused):
                        case.run.retry_readiness('p0-fresh')
                    self.assertIn('automation_failure', case.run.selected('p0-fresh'))
                    self.assertNotIn('readiness_retries', case.run.selected('p0-fresh'))
                finally:
                    case.doCleanups()

    def unavailable_app(self, status='could-not-run'):
        """Finish earlier fixture checks, then retain one unavailable native checkpoint."""
        self.prepare()
        def execute(controller, envelope, files):
            """The completed command/setup prefix is distinct from the unavailable app observation."""
            self.seen.append(envelope)
            return self.archive(controller, envelope, status if envelope['check_id'] == 'app-start' else 'pass')
        self.run.tester = execute
        with self.assertRaises(Refused):
            self.run.test('p0-fresh', source=self.source)

    def test_repaired_fixture_retry_preserves_observation_and_skips_accepted_checks(self):
        """A diagnosed fixture gap can be retried without hiding it or rerunning accepted setup."""
        self.unavailable_app()
        original = self.run.selected('p0-fresh')['results']['app-start']
        self.run.retry_unavailable('p0-fresh', 'Repaired the owned fixture; inspected events show no uncertain action.')
        entry = self.run.selected('p0-fresh')
        self.assertNotIn('app-start', entry['results'])
        self.assertEqual(entry['checkpoint_retries'][0]['result'], original)
        count = len(self.seen)
        self.run.tester = self.execute_fixture
        self.run.test('p0-fresh', source=self.source)
        self.assertEqual([row['check_id'] for row in self.seen[count:]], ['app-start'])
        entry = self.run.selected('p0-fresh')
        self.assertEqual(self.run.verdict(entry), 'pass')
        archive = original['native']['events'].split('::')[0]
        (self.run.directory / archive).unlink()
        self.assertEqual(self.run.verdict(entry), 'could not run')

    def test_unavailable_retry_cannot_clear_product_failure_or_retry_feedback(self):
        """Operator reasoning does not authorize suppressing a product fail or an external-send outcome."""
        self.unavailable_app(status='fail')
        with self.assertRaises(Refused):
            self.run.retry_unavailable('p0-fresh', 'Inspected fixture and repaired a prerequisite before retrying.')
        entry = self.run.selected('p0-fresh')
        self.assertEqual(entry['results']['app-start']['status'], 'fail')
        entry['results']['app-start']['status'] = 'could-not-run'
        # Neither a continuity check, whose sequence went on past it, nor the
        # feedback send can be replayed through this transition.
        for priority in ('P1', 'P2'):
            with self.subTest(priority=priority):
                entry['required']['app-start']['priority'] = priority
                self.run.save()
                with self.assertRaises(Refused):
                    self.run.retry_unavailable('p0-fresh', 'Inspected fixture and repaired a prerequisite before retrying.')
                self.assertNotIn('checkpoint_retries', self.run.selected('p0-fresh'))

    def test_harness_failure_is_separate_from_product_failure(self):
        """Permission bugs block the gate without condemning the published release bytes."""
        self.prepare()
        def blocked(controller, envelope, files):
            """Produce an actual retained permission observation for the host importer."""
            self.seen.append(envelope)
            return self.archive(controller, envelope, 'permission-block')
        self.run.tester = blocked
        with self.assertRaises(Refused):
            self.run.test('p0-fresh', source=self.source)
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'could not run')
            self.assertEqual(report['automation']['verdict'], 'fail')
            self.assertEqual(self.run.selected('p0-fresh')['tester']['guest_status'], 'permission-block')
            self.assertEqual(report['unverified_product_failures'], [])
            self.assertFalse(report['promotion_blocked_pending_review'])
        self.assertEqual(len(self.seen), 1)

    def test_preflight_failure_is_harness_only_and_does_not_claim_product_break(self):
        """A failed Finder/native prerequisite is distinct from an observed Farhelm failure."""
        self.prepare()
        self.run.tester = lambda controller, envelope, files: self.archive(controller, envelope, 'fail')
        with self.assertRaises(Refused):
            self.run.test('p0-fresh', source=self.source)
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(self.run.selected('p0-fresh')['automation_failure']['kind'], 'harness-failure')
            self.assertEqual(report['automation']['verdict'], 'fail')
            self.assertEqual(report['p0']['unverified_product_failures'], [])
            self.assertFalse(report['promotion_blocked_pending_review'])

    def test_partial_injection_failure_cannot_resume_or_start_a_tester(self):
        """A copied Mac credential does not certify the Linux destination after its write fails."""
        self.prepare()
        home = self.fixture.root / 'mac-copy'
        home.mkdir(mode=0o700)
        original = self.fixture.model.runner
        destinations = []
        def injection(argv, **kwargs):
            """Execute the real Mac stdin write, then independently refuse only Linux."""
            if argv[1:3] == ['exec', '-i'] and 'fixture contains authentication' in argv[-1]:
                destinations.append(argv[3])
                if argv[3].endswith('-linux'):
                    raise subprocess.CalledProcessError(1, argv)
                return subprocess.run([sys.executable, *argv[5:]], check=True,
                                      input=kwargs['input'], capture_output=True,
                                      env={'HOME': str(home), 'PATH': os.defpath})
            return original(argv, **kwargs)
        self.fixture.model.runner = injection
        with self.assertRaises(subprocess.CalledProcessError):
            self.run.test('p0-fresh', source=self.source)
        expected = (self.fixture.root / 'fictional-login.json').read_bytes()
        self.assertEqual((home / '.codex/auth.json').read_bytes(), expected)
        with self.assertRaisesRegex(Refused, 'failed automation is retained'):
            self.run.test('p0-fresh', source=self.source)
        self.assertEqual(len(destinations), 2)
        self.assertTrue(destinations[0].endswith('-mac'))
        self.assertTrue(destinations[1].endswith('-linux'))
        self.assertEqual(self.seen, [])
        with self.run.locked():
            entry = self.run.selected('p0-fresh')
            self.assertFalse(entry.get('credentials_injected', False))
            self.assertEqual(entry['tester']['phase'], 'failed')
            self.assertEqual(self.run.verdict(entry), 'could not run')

    def test_aborted_turn_surfaces_its_unverified_product_failure(self):
        """Incomplete tool provenance must not hide a claimed product break as routine setup."""
        self.prepare()
        def aborted(controller, envelope, files):
            """Complete readiness, then retain a failure claim from an aborted product turn."""
            if envelope['check_id'] == 'native-preflight':
                return self.archive(controller, envelope)
            return self.archive(controller, envelope, 'automation-failure', unverified='fail')
        self.run.tester = aborted
        with self.assertRaises(Refused):
            self.run.test('p0-fresh', source=self.source)
        with self.run.locked():
            report = self.run.report()
            self.assertTrue(report['promotion_blocked_pending_review'])
            self.assertEqual(report['p0']['unverified_product_failures'][0]['check_id'], 'artifact-verification')
            self.assertEqual(report['p0']['verdict'], 'could not run')
            self.assertEqual(report['automation']['verdict'], 'fail')

    def test_command_checkpoint_rejects_stale_execution_identity(self):
        """Matching task bytes cannot let a command check borrow another attempt's observation."""
        self.prepare()
        def stale(controller, envelope, files):
            """Retain the exact current task, but substitute an older result identity."""
            label = self.archive(controller, envelope)
            if envelope['check_id'] == 'artifact-verification':
                path = controller.directory / (label + '.tar')
                with tarfile.open(path) as original:
                    contents = {member.name: original.extractfile(member).read() for member in original.getmembers()}
                value = json.loads(contents['./execution.json'])
                value['result']['identity']['attempt'] = 'old-unrelated-attempt'
                contents['./execution.json'] = json.dumps(value).encode()
                with tarfile.open(path, 'w') as replaced:
                    for name, content in contents.items():
                        member = tarfile.TarInfo(name)
                        member.size = len(content)
                        replaced.addfile(member, io.BytesIO(content))
            return label
        self.run.tester = stale
        with self.assertRaisesRegex(Refused, 'identity'):
            self.run.test('p0-fresh', source=self.source)
        with self.run.locked():
            self.assertNotIn('artifact-verification', self.run.selected('p0-fresh')['results'])
            self.assertEqual(self.run.report()['p0']['verdict'], 'could not run')

    def test_a_failure_claim_survives_a_failed_evidence_import(self):
        """A product failure the guest reported must stay visible for review when its evidence cannot be imported.

The import fails as a harness error, so nothing is accepted, but the claim is
kept as an unverified product failure that blocks promotion pending review.
"""
        self.prepare()
        def broken_evidence(controller, envelope, files):
            """Report a fail for app-start whose command receipt is missing from the export."""
            self.seen.append(envelope)
            label = self.archive(controller, envelope, 'fail' if envelope['check_id'] == 'app-start' else 'pass')
            if envelope['check_id'] == 'app-start':
                path = controller.directory / (label + '.tar')
                with tarfile.open(path) as original:
                    contents = {member.name: original.extractfile(member).read() for member in original.getmembers()
                                if member.name != './commands.txt'}
                with tarfile.open(path, 'w') as replaced:
                    for name, content in contents.items():
                        member = tarfile.TarInfo(name)
                        member.size = len(content)
                        replaced.addfile(member, io.BytesIO(content))
            return label
        self.run.tester = broken_evidence
        with self.assertRaises(KeyError):
            self.run.test('p0-fresh', source=self.source)
        with self.run.locked():
            report = self.run.report()
            self.assertNotIn('app-start', self.run.selected('p0-fresh')['results'])
            self.assertEqual([claim['check_id'] for claim in report['p0']['unverified_product_failures']], ['app-start'])
            self.assertTrue(report['promotion_blocked_pending_review'])

    def test_reconciliation_cannot_take_a_live_coordinator_lease(self):
        """An operator inspecting stale-looking state must not interfere with a live tester."""
        self.prepare()
        with self.run.tester_owner(), self.assertRaisesRegex(Refused, 'live coordinator'):
            self.run.reconcile_tester('p0-fresh')

    def test_raw_interruption_without_stopped_receipt_keeps_cleanup_fenced(self):
        """Any cancellation path stays uncertain until the executor's return contract establishes exit."""
        self.prepare()
        self.run.tester = mock.Mock(side_effect=KeyboardInterrupt)
        with self.assertRaises(KeyboardInterrupt):
            self.run.test('p0-fresh', source=self.source)
        with self.run.locked():
            claim = self.run.selected('p0-fresh')['tester']
            self.assertFalse(claim['termination_observed'])
            self.assertEqual(claim['phase'], 'reconciliation-required')
            with self.assertRaises(Refused):
                self.run.cleanup('p0-fresh', discard_failed=True)

    def test_stopped_mac_reconciles_abandoned_claim_without_pid_signals(self):
        """An independently stopped VM establishes termination without trusting a stale claim."""
        self.prepare()
        self.run.tester = mock.Mock(side_effect=CleanupPending('missing stopped acknowledgement'))
        with self.assertRaises(CleanupPending):
            self.run.test('p0-fresh', source=self.source)
        mac = next(name for name in self.fixture.model.states if name.endswith('-mac'))
        self.fixture.model.states[mac] = 'stopped'
        self.run.reconcile_tester('p0-fresh')
        with self.run.locked():
            self.assertEqual(self.run.selected('p0-fresh')['tester']['phase'], 'failed')
            self.assertTrue(self.run.selected('p0-fresh')['tester']['reconciled'])

    def reconcile_interrupted_observation(self, status='automation-failure', observed='fail', stale=False,
                                         stale_task=False):
        """Leave a real archive before host import, then model acknowledged guest cancellation.

The interruption occurs after native readiness and after the guest has retained
its product observation. Collection copies those exact bytes under the actual
reconciliation label; no accepted checkpoint is synthesized by the transport.
"""
        self.prepare()
        retained = {}
        def interrupted(controller, envelope, files):
            """Interrupt the host only after a product attempt has exported its observation."""
            if envelope['check_id'] == 'native-preflight':
                return self.archive(controller, envelope)
            identity = dict(result(envelope)['identity'], attempt='stale-attempt') if stale else None
            retained_envelope = dict(envelope, run_id='foreign-run') if stale_task else envelope
            label = self.archive(controller, retained_envelope, status, unverified=observed,
                                 observation_identity=identity)
            retained['path'] = controller.directory / (label + '.tar')
            raise KeyboardInterrupt
        self.run.tester = interrupted
        with self.assertRaises(KeyboardInterrupt):
            self.run.test('p0-fresh', source=self.source)
        with self.run.locked():
            entry = self.run.selected('p0-fresh')
            self.assertEqual(entry['tester']['phase'], 'reconciliation-required')
            self.assertNotIn('artifact-verification', entry['results'])
            controller = self.run.pair('p0-fresh')
        with controller.locked():
            self.assertTrue(all(controller.state(role)['State'] == 'running' for role in ('mac', 'linux')))
        def collect(role, directory, label):
            """Retain the exact interrupted archive at the host's independently chosen label."""
            self.assertEqual(role, 'mac')
            (controller.directory / (label + '.tar')).write_bytes(retained['path'].read_bytes())
        with mock.patch('brick_test.cancel_guest') as cancel, \
                mock.patch.object(self.run, 'control', return_value=controller), \
                mock.patch.object(controller, 'collect', side_effect=collect):
            self.run.reconcile_tester('p0-fresh')
        cancel.assert_called_once()

    def test_reconciliation_surfaces_interrupted_product_failure_without_accepting_it(self):
        """Recovering diagnostics must retain a product-break warning while the gate stays unavailable."""
        self.reconcile_interrupted_observation()
        with self.run.locked():
            entry = self.run.selected('p0-fresh')
            report = self.run.report()
            self.assertEqual(entry['tester']['phase'], 'failed')
            self.assertTrue(entry['tester']['reconciled'])
            self.assertNotIn('artifact-verification', entry['results'])
            self.assertTrue(report['promotion_blocked_pending_review'])
            self.assertEqual(report['p0']['unverified_product_failures'][0]['check_id'], 'artifact-verification')
            self.assertEqual(report['p0']['verdict'], 'could not run')

    def test_reconciliation_cannot_accept_a_pass_from_an_abandoned_attempt(self):
        """Even a guest pass cannot turn interrupted host import into accepted release evidence."""
        self.reconcile_interrupted_observation(status='pass', observed='pass')
        with self.run.locked():
            entry = self.run.selected('p0-fresh')
            self.assertEqual(entry['tester']['phase'], 'failed')
            self.assertNotIn('artifact-verification', entry['results'])
            self.assertEqual(self.run.report()['p0']['verdict'], 'could not run')
            self.assertFalse(self.run.report()['promotion_blocked_pending_review'])

    def test_reconciliation_surfaces_a_completed_failure_not_yet_imported_by_host(self):
        """Host interruption can also lose an otherwise completed guest product-failure result."""
        self.reconcile_interrupted_observation(status='fail', observed=None)
        with self.run.locked():
            self.assertTrue(self.run.report()['promotion_blocked_pending_review'])
            self.assertEqual(self.run.report()['p0']['verdict'], 'could not run')
            self.assertNotIn('artifact-verification', self.run.selected('p0-fresh')['results'])

    def test_reconciliation_rejects_an_archive_task_from_another_run(self):
        """A shaped result cannot bypass the independent retained task's run/runtime identity fence."""
        self.reconcile_interrupted_observation(stale_task=True)
        with self.run.locked():
            claim = self.run.selected('p0-fresh')['tester']
            self.assertEqual(claim['phase'], 'failed')
            self.assertIn('task identity', claim['reconciliation_evidence_error'])
            self.assertNotIn('observation', claim)
            self.assertFalse(self.run.report()['promotion_blocked_pending_review'])

    def test_reconciliation_rejects_a_stale_observation_identity(self):
        """Another attempt's failure cannot become this run's promotion warning or release result."""
        self.reconcile_interrupted_observation(stale=True)
        with self.run.locked():
            entry = self.run.selected('p0-fresh')
            self.assertEqual(entry['tester']['phase'], 'failed')
            self.assertIn('identity', entry['tester']['reconciliation_evidence_error'])
            self.assertNotIn('observation', entry['tester'])
            self.assertFalse(self.run.report()['promotion_blocked_pending_review'])

    def test_format_two_command_smoke_remains_headless_and_not_a_release_pass(self):
        """Native product prerequisites must not disable the documented setup smoke path."""
        f = self.fixture
        run = Run(f.evidence / 'native-command-smoke', f.profile,
                  factory=f.model.factory, connector=f.model.connect, disk_usage=f.run.disk_usage)
        run.create(smoke=True)
        smoke(run)
        with run.locked():
            result = run.report()
        self.assertEqual(result['verdict'], 'fixture smoke: pass')
        self.assertEqual(set(f.model.states), {'mac-base', 'linux-base'})
        self.assertTrue(all(child.poll() == 0 for child in f.model.children))


if __name__ == '__main__':
    unittest.main()
