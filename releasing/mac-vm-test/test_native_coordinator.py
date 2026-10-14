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

from brick_test import Run, smoke
from control import Refused
from guest_workflow import CleanupPending
import test_brick_test as fixtures
from test_brick_test import FakeSource, PNG
from test_guest_tester import events, result, task
from test_fixture_probe import Commands
from fixture_probe import probe, ProbeError


def after_capture():
    """Provide a distinct valid synthetic PNG for the two-observation P1 receipt contract.

This pixel is deliberately unrelated to Farhelm. Its role is to prove capture
linkage and distinguish two retained observations, never a real dialog outcome.
"""
    def chunk(kind, value):
        """Encode a PNG chunk with its independent wire checksum."""
        return struct.pack('>I', len(value)) + kind + value + struct.pack('>I', zlib.crc32(kind + value))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(b'\x00\xff\x00\x00\xff')) + chunk(b'IEND', b''))


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
        f.enable_p0(p0_only=True)
        self.run = f.run
        self.seen = []
        source = FakeSource(['v0.24.0'])
        source.git = lambda *args: 'https://github.com/fixture/project.git\n'
        self.source = source

    def archive(self, controller, envelope, status='pass', unverified=None, two_captures=True, image_only=False,
                fixture_defect=None, nested_receipt=False, observation_identity=None):
        """Return a real archive with separate execution, observation and capture identities."""
        label = 'tester-' + envelope['attempt']
        observation = result(envelope, unverified or status)
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
        if envelope['check_id'] == 'p1-feedback-send' and two_captures:
            observation['evidence'].append('native-after.png')
            native_rows[0]['item']['result']['content'].append(
                {'type': 'image', 'mimeType': 'image/png', 'data': base64.b64encode(after_capture()).decode()})
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
                for role in ('mac', 'linux')]}
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
        if envelope['check_id'] == 'p1-feedback-send' and two_captures:
            files['native-after.png'] = after_capture()
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
        """Retain valid native P0 evidence before exercising the final pair's P1 boundary."""
        for pass_id in ('p0-fresh', 'p0-installer-upgrade'):
            self.drive_pair(pass_id)
            with self.run.locked():
                self.run.cleanup(pass_id)

    def approve_feedback(self):
        """Record fictional send consent separately from the fixture's credential consent."""
        path = self.fixture.root / 'consent.json'
        value = json.loads(path.read_text())
        value['feedback_send_approved'] = True
        path.write_text(json.dumps(value))

    def test_p0_only_never_dispatches_feedback_and_cleans_final_pair(self):
        """The action fence must precede guest execution, since record-time refusal is too late."""
        self.finish_preceding_pairs()
        self.drive_pair('p0-updater-upgrade')
        self.assertNotIn('p1-feedback-send', [value['check_id'] for value in self.seen])
        self.assertTrue(all('feedback_authorized' not in value['context'] for value in self.seen))
        with self.run.locked():
            self.assertEqual(self.run.report()['p0']['verdict'], 'pass')
            self.assertEqual(self.run.report()['requested_verdict'], 'pass')
            self.run.cleanup('p0-updater-upgrade')

    def test_full_and_p1_setup_refuse_without_send_consent_before_mutation(self):
        """Known missing send authority must not first pause an otherwise completed P0 run."""
        self.prepare()
        self.assertEqual(self.run.data['passes']['p0-fresh']['phase'], 'ready')
        with self.run.locked():
            controller = self.run.pair('p0-fresh')
        with controller.locked():
            self.assertTrue(all(controller.state(role)['State'] == 'running' for role in ('mac', 'linux')))
        for scope in ('full', 'p1-only'):
            with self.subTest(scope=scope), self.run.locked():
                self.run.data['scope'] = scope
                self.run.save()
            calls = len(self.fixture.model.calls)
            with self.assertRaisesRegex(Refused, 'live P1 feedback'):
                self.run.test('p0-fresh', source=self.source)
            self.assertEqual(len(self.fixture.model.calls), calls)
            self.assertEqual(self.seen, [])
            with self.run.locked():
                self.assertNotIn('credentials_injected', self.run.selected('p0-fresh'))

    def test_valid_failed_p1_keeps_p0_and_allows_cleanup_and_bonus(self):
        """A completed feedback failure cannot erase valid upgrades or become an automation fault."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        with self.run.locked():
            self.run.data['scope'] = 'full'
            self.run.save()
        def failed_send(controller, envelope, files):
            """Return a valid product failure only at the separately authorized send boundary."""
            self.seen.append(envelope)
            return self.archive(controller, envelope, 'fail' if envelope['check_id'] == 'p1-feedback-send' else 'pass')
        self.drive_pair('p0-updater-upgrade', failed_send)
        feedback = [value for value in self.seen if value['check_id'] == 'p1-feedback-send']
        self.assertEqual(len(feedback), 1)
        self.assertIs(feedback[0]['context']['feedback_authorized'], True)
        self.assertNotIn('addendum.md', feedback[0]['instructions'])
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'pass')
            self.assertEqual(report['p1']['verdict'], 'fail')
            self.assertEqual(report['automation']['verdict'], 'no failure observed')
            self.assertNotIn('automation_failure', self.run.selected('p0-updater-upgrade'))
            self.run.cleanup('p0-updater-upgrade')
            self.run.require_priority('fresh')

    def test_removed_feedback_consent_stops_before_p1_dispatch(self):
        """Setup approval must still exist at the action boundary, after the final P0 observation."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        with self.run.locked():
            self.run.data['scope'] = 'full'
            self.run.save()
        def removed_consent(controller, envelope, files):
            """Withdraw only fictional send consent as the modeled last P0 task completes."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'remote-supervisor-agent':
                path = self.fixture.root / 'consent.json'
                value = json.loads(path.read_text())
                value.pop('feedback_send_approved')
                path.write_text(json.dumps(value))
            return self.archive(controller, envelope)
        with self.assertRaisesRegex(Refused, 'live P1 feedback'):
            self.drive_pair('p0-updater-upgrade', removed_consent)
        self.assertNotIn('p1-feedback-send', [value['check_id'] for value in self.seen])
        with self.run.locked():
            self.assertEqual(self.run.report()['p0']['verdict'], 'pass')
            self.assertEqual(self.run.report()['p1']['verdict'], 'could not run')
            self.assertEqual(self.run.selected('p0-updater-upgrade')['automation_failure']['kind'], 'unavailable')
            self.assertEqual(self.run.report()['automation']['verdict'], 'no failure observed')

    def test_mid_turn_p0_stop_fences_the_not_yet_dispatched_feedback(self):
        """A late P0-only request must be honored before the queued P1 task can send anything."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        with self.run.locked():
            self.run.data['scope'] = 'full'
            self.run.save()
        def narrowed(controller, envelope, files):
            """Persist the user's scope change while the modeled last P0 task is outside the workflow lock."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'remote-supervisor-agent':
                with self.run.locked():
                    self.run.limit_to_p0()
            return self.archive(controller, envelope)
        self.drive_pair('p0-updater-upgrade', narrowed)
        self.assertNotIn('p1-feedback-send', [value['check_id'] for value in self.seen])
        with self.run.locked():
            self.assertEqual(self.run.report()['requested_verdict'], 'pass')
            self.run.cleanup('p0-updater-upgrade')

    def test_changed_p0_evidence_fences_feedback_before_guest_dispatch(self):
        """Record-time refusal cannot undo a live send after an earlier P0 premise was invalidated."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        with self.run.locked():
            self.run.data['scope'] = 'full'
            archive = self.run.selected('p0-fresh')['results']['app-start']['evidence'][0].split('::', 1)[0]
            self.run.save()
        def changed_prior(controller, envelope, files):
            """Damage independently retained earlier evidence as the final P0 task completes."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'remote-supervisor-agent':
                (self.run.directory / archive).write_bytes(b'changed earlier archive')
            return self.archive(controller, envelope)
        with self.assertRaisesRegex(Refused, 'valid evidence before P1 dispatch'):
            self.drive_pair('p0-updater-upgrade', changed_prior)
        self.assertNotIn('p1-feedback-send', [value['check_id'] for value in self.seen])
        with self.run.locked():
            self.assertEqual(self.run.report()['p0']['verdict'], 'could not run')

    def test_native_p1_pass_satisfies_its_requested_scope(self):
        """A genuine native receipt path must preserve the successful P1-only verdict and ordinary cleanup."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        with self.run.locked():
            self.run.data['scope'] = 'p1-only'
            self.run.save()
        self.drive_pair('p0-updater-upgrade')
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'pass')
            self.assertEqual(report['p1']['verdict'], 'pass')
            self.assertEqual(report['requested_verdict'], 'pass')
            self.run.cleanup('p0-updater-upgrade')

    def test_in_flight_p1_outcome_survives_late_scope_narrowing(self):
        """A scope request cannot recall a dispatched send or erase its actual retained result."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        with self.run.locked():
            self.run.data['scope'] = 'full'
            self.run.save()
        def in_flight(controller, envelope, files):
            """Narrow scope only after the modeled P1 task is already dispatched."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'p1-feedback-send':
                with self.run.locked():
                    self.run.limit_to_p0()
            return self.archive(controller, envelope)
        self.drive_pair('p0-updater-upgrade', in_flight)
        with self.run.locked():
            entry = self.run.selected('p0-updater-upgrade')
            self.assertTrue(entry['results']['p1-feedback-send']['dispatched_before_scope_stop'])
            self.assertNotIn('automation_failure', entry)
            self.assertEqual(self.run.report()['p1']['verdict'], 'pass')
            self.assertEqual(self.run.report()['requested_verdict'], 'pass')
            self.run.cleanup('p0-updater-upgrade')

    def test_native_p1_fault_keeps_p0_but_fences_bonus_and_cleanup(self):
        """P1's nonblocking product exception must never hide an aborted native tester or an unverified break."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        with self.run.locked():
            self.run.data['scope'] = 'full'
            self.run.save()
        def fault(controller, envelope, files):
            """Supply an aborted-turn observation only at P1, after independently passed P0."""
            self.seen.append(envelope)
            if envelope['check_id'] == 'p1-feedback-send':
                return self.archive(controller, envelope, 'automation-failure', unverified='fail')
            return self.archive(controller, envelope)
        with self.assertRaises(Refused):
            self.drive_pair('p0-updater-upgrade', fault)
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'pass')
            self.assertEqual(report['p1']['verdict'], 'could not run')
            self.assertEqual(report['automation']['verdict'], 'fail')
            self.assertEqual(report['p0']['unverified_product_failures'], [])
            self.assertEqual(len(report['p1']['unverified_product_failures']), 1)
            with self.assertRaisesRegex(Refused, 'first failed'):
                self.run.require_priority('fresh')
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup('p0-updater-upgrade')

    def test_p1_pass_requires_two_distinct_native_captures(self):
        """The structural gate requires two captures; their before/after meaning still needs independent review."""
        self.finish_preceding_pairs()
        self.approve_feedback()
        with self.run.locked():
            self.run.data['scope'] = 'p1-only'
            self.run.save()
        def one_capture(controller, envelope, files):
            """Keep an otherwise valid P1 receipt with only one real modeled native capture."""
            self.seen.append(envelope)
            return self.archive(controller, envelope, two_captures=False)
        with self.assertRaisesRegex(Refused, 'two distinct native'):
            self.drive_pair('p0-updater-upgrade', one_capture)

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
