"""Prove workflow safety and evidence gates without borrowing real GUI outcomes.

The fake Tart substrate has independently observable identities, states and
mutation calls. Tiny image fixtures prove format/linkage gates only; they are
never evidence that Farhelm itself rendered or that a conversation occurred.
"""

import base64
from collections import namedtuple
import io
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest
import uuid

from brick_test import Run, artifact, metadata, plan, smoke, version
from control import Control, Refused, atomic_json
from machine_profile import Profile
from test_machine_profile import fixture


PNG = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j9AAAAABJRU5ErkJggg==')


class FakeSource:
    """Describe fictional release capabilities without accessing Git or the site."""

    def __init__(self, supported=()):
        self.supported = set(supported)

    def tags(self):
        """Mix eligible stable tags with an RC and an excluded pre-contract release."""
        return ['v0.22.0', 'v0.23.0', 'v0.24.0', 'v0.25.0-rc.1', 'v0.25.0', 'unrelated']

    def commit(self, tag):
        """Give each source tag a distinct observable identity."""
        return 'fixture-source-' + tag

    def override(self, tag):
        """The capability is explicitly supplied by this fixture, not a historical claim."""
        return tag in self.supported


class PlanningTests(unittest.TestCase):
    """Pin candidate selection and the full supported-old-version coverage."""

    def test_previous_stable_leads_and_older_supported_pairs_remain_required(self):
        """Testing only the latest old stable must not silently waive older upgrade contracts."""
        result = plan('v0.25.0', 'v0.24.0', FakeSource())
        self.assertEqual([v['id'] for v in result['passes']],
                         ['fresh', 'upgrade-v0.24.0', 'upgrade-v0.23.0'])
        self.assertEqual(result['passes'][1]['update_path'], 'manual')

    def test_override_uses_old_source_and_never_selects_an_rc(self):
        """A candidate's support cannot make an unsupported old app use its override."""
        source = FakeSource(['v0.24.0'])
        stable = plan('v0.25.0', 'v0.24.0', source)
        self.assertEqual(stable['passes'][1]['update_path'], 'override')
        self.assertEqual(stable['passes'][2]['update_path'], 'manual')
        rc = plan('v0.25.0-rc.1', 'v0.24.0', source)
        self.assertTrue(all(v['update_path'] == 'manual' for v in rc['passes'][1:]))

    def test_promoted_candidate_is_an_ordinary_rehearsal(self):
        """After-publication testing still uses older versions, never candidate-to-itself."""
        result = plan('v0.24.0', 'v0.24.0', FakeSource())
        self.assertEqual([v['previous'] for v in result['passes']], ['v0.23.0', 'v0.23.0'])
        self.assertEqual(result['passes'][1]['update_path'], 'ordinary')

    def test_downgrades_missing_old_source_and_unsafe_tags_are_refused(self):
        """Invalid gate inputs must fail before any VM can be created."""
        with self.assertRaises(Refused):
            plan('v0.23.0', 'v0.24.0', FakeSource())
        class MissingStable(FakeSource):
            """Omit the promoted release even though an older eligible tag exists."""
            def tags(self):
                """Expose the incomplete source prerequisite independently of planning."""
                return ['v0.23.0']
        with self.assertRaisesRegex(Refused, 'promoted stable is missing'):
            plan('v0.25.0', 'v0.24.0', MissingStable())
        for value in ('--help', '../release', 'v01.2.3', 'v1.2.3-rc.01'):
            with self.subTest(tag=value), self.assertRaises(Refused):
                version(value)
        self.assertLess(version('v1.2.3-rc.2'), version('v1.2.3-rc.10'))
        self.assertLess(version('v1.2.3-rc.10'), version('v1.2.3'))

    def test_availability_receipt_does_not_claim_signature_verification(self):
        """Unverified public metadata cannot satisfy the old-ring verification milestone."""
        import hashlib
        installer = b'fixture installer bytes'
        inputs = {'install.sh': installer,
                  'SHA256SUMS': (hashlib.sha256(installer).hexdigest() + '  install.sh\n').encode(),
                  'SHA256SUMS.minisig': b'trusted comment: farhelm v0.25.0\n'}
        def fetch(url):
            """Return only the known candidate paths, without a network request."""
            self.assertTrue(url.startswith('https://get.farhelm.io/v0.25.0/'))
            return inputs[url.rsplit('/', 1)[1]]
        _, receipt = metadata('v0.25.0', fetch)
        self.assertEqual(receipt['signature_verification'], 'pending')
        inputs['install.sh'] = b'different bytes'
        with self.assertRaises(Refused):
            metadata('v0.25.0', fetch)


class FakeTart:
    """Model Tart independently from workflow state and retain every mutation call."""

    def __init__(self, profile):
        self.profile = profile
        self.states = {}
        self.calls = []
        self.children = []
        self.account = 'guest'
        self.fail_clone = False
        self.fail_start_probe = False
        self.markers = {}
        home = Path(profile.values['tart_home'])
        for role in ('mac', 'linux'):
            base = profile.values[role + '_base']
            directory = home / 'vms' / base['name']
            directory.mkdir(parents=True)
            atomic_json(directory / 'config.json', {'macAddress': base['mac_address']})
            self.states[base['name']] = 'stopped'

    def runner(self, argv, **kwargs):
        """Make each modeled state transition visible through independent subsequent probes."""
        self.calls.append(argv)
        action = argv[1]
        result = b''
        if action == 'list':
            result = json.dumps([{'Name': name} for name in self.states]).encode()
        elif action == 'get':
            if self.fail_start_probe and self.states[argv[2]] == 'running':
                raise Refused('injected post-spawn probe failure')
            result = json.dumps({'State': self.states[argv[2]]}).encode()
        elif action == 'clone':
            if self.fail_clone:
                raise subprocess.CalledProcessError(1, argv)
            name = argv[3]
            directory = Path(self.profile.values['tart_home']) / 'vms' / name
            directory.mkdir()
            atomic_json(directory / 'config.json', {'macAddress': '02:00:00:00:00:10'})
            self.states[name] = 'stopped'
        elif action == 'exec':
            name = argv[3] if argv[2] == '-i' else argv[2]
            command = argv[4:] if argv[2] == '-i' else argv[3:]
            if '/sbin/shutdown' in command:
                self.states[name] = 'stopped'
            elif command == ['/usr/bin/id', '-un']:
                result = (self.account + '\n').encode()
            elif any('commands.txt' in item for item in command):
                self.markers[name] = b'fixture-smoke-ready\n'
            elif any('printf %s' in item for item in command):
                result = b'/fictional/guest/evidence'
        elif action == 'delete':
            name = argv[2]
            self.states.pop(name)
            shutil.rmtree(Path(self.profile.values['tart_home']) / 'vms' / name)
        return subprocess.CompletedProcess(argv, 0, result)

    def spawn(self, argv, **kwargs):
        """Retain an owned synthetic child whose liveness follows the modeled VM."""
        owner, name = self, argv[-1]
        owner.states[name] = 'running'
        class Child:
            """No OS process is spawned by this fixture; no real PID may be signaled."""
            pid = 1000 + len(owner.children)
            returncode = None

            def poll(self):
                """Report completion only after a separate shutdown changes VM state."""
                if owner.states.get(name) != 'running':
                    self.returncode = 0
                return self.returncode
        child = Child()
        owner.children.append(child)
        return child

    def factory(self, *args):
        """Use the real ownership/journal implementation with fake command and export substrates."""
        model = self
        class FixtureControl(Control):
            """Keep workflow calls on production fences while avoiding real Tart execution."""

            def start(self, role, headless=False):
                """Route spawning to synthetic children rather than an installed Tart binary."""
                return super().start(role, headless, spawner=model.spawn)

            def collect(self, role, guest_directory, label):
                """Retain a real tar fixture so smoke export validation reads actual bytes."""
                name = self.owned(role)
                path = self.directory / (label + '.tar')
                with tarfile.open(path, 'w') as archive:
                    content = model.markers[name]
                    info = tarfile.TarInfo('./commands.txt')
                    info.size = len(content)
                    archive.addfile(info, io.BytesIO(content))
        return FixtureControl(*args, runner=self.runner)

    def connect(self, control, mac, linux, user, route):
        """Assert named owned peers, then record a fictional already-proved SSH handle."""
        if self.states[mac] != 'running' or self.states[linux] != 'running':
            raise AssertionError('both owned guest processes must be live before connection')
        if user != self.account:
            raise AssertionError('account premise must already be checked')
        control.data['ssh'] = {'alias': 'fixture-peer', 'user': user}
        control.save()


class WorkflowTests(unittest.TestCase):
    """Demonstrate failure, interruption and evidence boundaries on isolated private state."""

    def setUp(self):
        """Create a valid unrelated machine profile and two independently stopped source bases."""
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        data = fixture(self.root)
        data['tart_home'] = 'tart'
        path = self.root / 'profile.json'
        path.write_text(json.dumps(data)); path.chmod(0o600)
        self.profile = Profile.load(path, home=self.root / 'home')
        self.model = FakeTart(self.profile)
        self.evidence = Path(self.profile.values['evidence_root'])
        self.evidence.mkdir()
        usage = namedtuple('Usage', 'free')
        self.free = 20 * 1024 ** 3
        self.run = Run(self.evidence / ('brick-' + str(uuid.uuid4())), self.profile,
                       factory=self.model.factory, disk_usage=lambda path: usage(self.free), connector=self.model.connect)
        self.plan = plan('v0.25.0', 'v0.24.0', FakeSource())
        self.plan['sha256sums_sha256'] = 'fixture-checksum'
        self.run.create(self.plan)
        self.assertTrue(all(state == 'stopped' for state in self.model.states.values()))

    def evidence_file(self, name, value):
        """Write an owned bounded artifact and return only its run-relative reference."""
        p = self.run.directory / name
        p.write_bytes(value)
        return name

    def prepare(self, pass_id='fresh'):
        """Establish live owned guests before testing any result or cleanup behavior."""
        children = []
        with self.run.locked():
            self.run.prepare(pass_id, children)
        self.assertEqual(len(children), 2)
        self.assertTrue(all(child.poll() is None for child in children))
        return children

    def test_replaced_or_running_base_prevents_all_cloning(self):
        """Source identity and stopped-state premises must precede any working VM mutation."""
        base = self.profile.values['mac_base']
        self.model.states[base['name']] = 'running'
        with self.run.locked(), self.assertRaisesRegex(Refused, 'remain stopped'):
            self.run.prepare('fresh', [])
        self.assertFalse(any(c[1] == 'clone' for c in self.model.calls))
        self.model.states[base['name']] = 'stopped'
        p = Path(self.profile.values['tart_home']) / 'vms' / base['name'] / 'config.json'
        atomic_json(p, {'macAddress': '02:00:00:00:00:99'})
        with self.run.locked(), self.assertRaisesRegex(Refused, 'identity changed'):
            self.run.prepare('fresh', [])
        self.assertFalse(any(c[1] == 'clone' for c in self.model.calls))

    def test_resume_profile_edit_cannot_redirect_existing_run(self):
        """A changed journal or VM binding is a different machine, not a cleanup override."""
        values = dict(self.profile.values, journal=str(self.root / 'different.md'))
        other = Run(self.run.directory, Profile(self.profile.source, values))
        with self.assertRaisesRegex(Refused, 'mismatch'):
            with other.locked():
                pass
        self.assertEqual(self.model.calls, [])

    def test_creation_intent_and_result_share_an_event_even_on_failure(self):
        """An unmatched intent must mean interrupted work, not unrelated result IDs."""
        journal = Path(self.profile.values['journal'])
        headings = [line for line in journal.read_text().splitlines() if line.startswith('## ')]
        self.assertIn('| intent |', headings[0])
        self.assertIn('| result |', headings[1])
        self.assertEqual(headings[0].rsplit(' | ', 1)[1], headings[1].rsplit(' | ', 1)[1])
        with self.assertRaises(FileExistsError):
            self.run.create(self.plan)
        headings = [line for line in journal.read_text().splitlines() if line.startswith('## ')]
        self.assertEqual(headings[-2].rsplit(' | ', 1)[1], headings[-1].rsplit(' | ', 1)[1])
        self.assertIn('creation failed', journal.read_text())

    def test_changed_evidence_invalidates_accepted_observation_and_cleanup(self):
        """Reusing a path with different bytes cannot inherit the old pass attestation."""
        self.prepare()
        reference = self.evidence_file('observation.txt', b'original observation')
        with self.run.locked():
            entry = self.run.selected('fresh')
            self.run.record('fresh', 'fixture-clean', 'pass', [reference], 'fictional clean fixture')
            # Narrow this synthetic pass so the later refusal distinguishes a
            # changed artifact from unrelated, deliberately unperformed checks.
            entry['required'] = {'fixture-clean': entry['required']['fixture-clean']}
            self.run.save()
            self.assertEqual(self.run.verdict(entry), 'pass')
        self.evidence_file('observation.txt', b'replacement observation')
        with self.run.locked():
            self.assertEqual(self.run.verdict(self.run.selected('fresh')), 'could not run')
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup('fresh')
        self.assertFalse(any(c[1] == 'delete' for c in self.model.calls))

    def test_budget_refuses_new_pair_without_mutation(self):
        """Observed space limits fence cloning even when the profile was valid at init."""
        self.free = 512 * 1024 ** 2
        with self.run.locked(), self.assertRaisesRegex(Refused, 'floor'):
            self.run.prepare('fresh', [])
        self.assertFalse(self.model.calls)

    def test_storage_volume_change_is_refused_before_cloning(self):
        """A new mount cannot turn the evidence-volume floor into the wrong-disk check."""
        observed = []
        def volume_id(path):
            """Expose distinct VM/evidence devices without changing real mounts."""
            observed.append(path)
            return 'vm-volume' if path.name == 'vms' else 'evidence-volume'
        self.run.volume_id = volume_id
        with self.run.locked(), self.assertRaisesRegex(Refused, 'share a filesystem'):
            self.run.prepare('fresh', [])
        self.assertIn(Path(self.profile.values['tart_home']) / 'vms', observed)
        self.assertIn(self.evidence, observed)
        self.assertEqual(self.model.calls, [])

    def test_failed_startup_transfers_live_child_to_workflow_supervision(self):
        """Preparation must retain a newly spawned child even when start never returns."""
        self.model.fail_start_probe = True
        children = []
        with self.run.locked(), self.assertRaisesRegex(Refused, 'post-spawn probe failure'):
            self.run.prepare('fresh', children)
        self.assertEqual(len(children), 1)
        self.assertEqual(children, self.model.children)
        self.assertTrue(all(child.poll() is None for child in children))
        with self.run.locked():
            self.assertEqual(self.run.selected('fresh')['phase'], 'failed')
            self.model.fail_start_probe = False
            self.run.cleanup('fresh', discard_failed=True)
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_interrupted_preparation_is_not_replayed_or_claimed_clean(self):
        """A failed clone must retain partial ownership state for actual reconciliation."""
        self.model.fail_clone = True
        with self.run.locked(), self.assertRaises(subprocess.CalledProcessError):
            self.run.prepare('fresh', [])
        with self.run.locked():
            self.assertEqual(self.run.data['passes']['fresh']['phase'], 'failed')
            with self.assertRaises(Refused):
                self.run.prepare('fresh', [])
            with self.assertRaises(Refused):
                self.run.cleanup('fresh')
        self.assertEqual(len([c for c in self.model.calls if c[1] == 'clone']), 1)

    def test_wrong_guest_account_retains_failure_and_live_children(self):
        """A mismatched SSH account is a fixture failure before authorizing remote key setup."""
        self.model.account = 'differentguest'
        children = []
        with self.run.locked(), self.assertRaisesRegex(Refused, 'account differs'):
            self.run.prepare('fresh', children)
        self.assertEqual(len(children), 2)
        self.assertTrue(all(child.poll() is None for child in children))
        with self.run.locked():
            self.assertEqual(self.run.verdict(self.run.selected('fresh')), 'could not run')
            self.run.cleanup('fresh', discard_failed=True)
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_one_active_pair_fences_the_next_upgrade(self):
        """A standard workflow cannot start another Mac pair while the first is owned and live."""
        self.prepare()
        before = len([c for c in self.model.calls if c[1] == 'clone'])
        with self.run.locked(), self.assertRaisesRegex(Refused, 'existing working pair'):
            self.run.prepare('upgrade-v0.24.0', [])
        self.assertEqual(len([c for c in self.model.calls if c[1] == 'clone']), before)

    def test_gui_pass_requires_image_and_hybrid_checks_also_require_commands(self):
        """Text-only logs and screenshots alone cannot satisfy different recipe evidence contracts."""
        self.prepare()
        command = self.evidence_file('command.txt', b'fixture observation')
        image = self.evidence_file('frame.png', PNG)
        empty = self.evidence_file('empty.txt', b'')
        with self.run.locked():
            for check_id in ('app-start', 'remote-terminal'):
                for refs in ([command], [empty]):
                    with self.subTest(check=check_id, refs=refs), self.assertRaises(Refused):
                        self.run.record('fresh', check_id, 'pass', refs, 'Fixture evidence gate only.')
            for check_id in ('codex-conversation', 'remote-terminal'):
                with self.subTest(check=check_id), self.assertRaises(Refused):
                    self.run.record('fresh', check_id, 'pass', [image], 'Fixture evidence gate only.')
            self.run.record('fresh', 'app-start', 'pass', [image], 'Format/linkage fixture; no product GUI was tested.')
            self.assertEqual(self.run.verdict(self.run.selected('fresh')), 'could not run')
            with self.assertRaises(Refused):
                self.run.cleanup('fresh')

    def test_evidence_cannot_escape_or_be_overwritten_as_an_observation(self):
        """Private run references and append-only observations survive later mistakes."""
        self.prepare()
        outside = self.root / 'outside.txt'; outside.write_bytes(b'owned outside fixture')
        link = self.run.directory / 'escape'; link.symlink_to(outside)
        with self.assertRaises(Refused):
            artifact(self.run.directory, 'escape')
        image = self.evidence_file('frame.png', PNG)
        with self.run.locked():
            self.run.record('fresh', 'app-start', 'pass', [image], 'Fixture linkage only.')
            with self.assertRaises(Refused):
                self.run.record('fresh', 'app-start', 'fail', [], 'Cannot erase the earlier result.')

    def test_failed_or_skipped_checks_never_become_a_pass(self):
        """An explicit product failure and an unavailable fixture remain different verdicts."""
        self.prepare()
        with self.run.locked():
            self.run.record('fresh', 'app-start', 'could-not-run', [], 'Fixture has no GUI substrate.')
            self.assertEqual(self.run.verdict(self.run.selected('fresh')), 'could not run')
            self.run.record('fresh', 'remote-terminal', 'fail', [], 'Synthetic failure scenario.')
            self.assertEqual(self.run.verdict(self.run.selected('fresh')), 'fail')

    def test_continuity_rejects_replacement_stale_or_malformed_receipts(self):
        """A larger counter cannot substitute a replacement process or an untyped identity."""
        self.prepare('upgrade-v0.24.0')
        before = {'pid': 42, 'start_ticks': 100, 'boot_id': '00000000-0000-0000-0000-000000000001', 'counter': 5}
        a = self.evidence_file('before.json', json.dumps(before).encode())
        for delta in ({'pid': 43, 'counter': 6}, {'counter': 5}, {'counter': '6'}):
            b = self.evidence_file('after.json', json.dumps(dict(before, **delta)).encode())
            with self.run.locked(), self.assertRaises((Refused, RuntimeError)):
                self.run.record('upgrade-v0.24.0', 'continuity-before', 'pass', [a, b],
                                'Synthetic continuity fixture.', a, b)
        b = self.evidence_file('after.json', json.dumps(dict(before, counter=6)).encode())
        with self.run.locked():
            self.run.record('upgrade-v0.24.0', 'continuity-before', 'pass', [a, b],
                            'Original fixture identity advanced.', a, b)

    def test_verification_must_use_old_ring_and_match_candidate_checksum(self):
        """Trust from the candidate itself must never waive the old app's compatibility check."""
        self.prepare()
        value = {'version': 'v0.25.0', 'previous_trust_ring': 'v0.25.0',
                 'sha256sums_sha256': 'fixture-checksum', 'signature': 'verified',
                 'trusted_comment': 'verified', 'installer_checksum': 'verified'}
        ref = self.evidence_file('verification.json', json.dumps(value).encode())
        with self.run.locked(), self.assertRaises(Refused):
            self.run.record('fresh', 'artifact-verification', 'pass', [ref], 'Verification fixture.')
        value['previous_trust_ring'] = 'v0.24.0'
        self.evidence_file('verification.json', json.dumps(value).encode())
        with self.run.locked():
            self.run.record('fresh', 'artifact-verification', 'pass', [ref], 'Old-ring receipt fixture accepted.')

    def test_continuity_milestones_reject_replacements_replay_and_wrong_order(self):
        """Internally valid pairs must still identify one advancing original workload."""
        self.prepare('upgrade-v0.24.0')
        image = self.evidence_file('frame.png', PNG)
        identity = {'pid': 42, 'start_ticks': 100, 'boot_id': '00000000-0000-0000-0000-000000000001'}
        def pair(label, low, high, **changes):
            """Create distinct synthetic receipts so rejected cases overwrite no observation."""
            return [self.evidence_file(label + '-before.json', json.dumps(dict(identity, counter=low, **changes)).encode()),
                    self.evidence_file(label + '-after.json', json.dumps(dict(identity, counter=high, **changes)).encode())]
        baseline = pair('baseline', 1, 2)
        remote = pair('remote', 2, 3)
        replacement = pair('replacement', 2, 3, pid=99)
        with self.run.locked():
            def record(check, refs):
                """Drive the production evidence gates with GUI and typed command receipts."""
                self.run.record('upgrade-v0.24.0', check, 'pass', [image, *refs],
                                'Synthetic continuity contract only.', *refs)
            with self.assertRaisesRegex(Refused, 'preceding continuity'):
                record('remote-update', remote)
            record('continuity-before', baseline)
            with self.assertRaisesRegex(Refused, 'original process'):
                record('remote-update', replacement)
            with self.assertRaisesRegex(Refused, 'replays'):
                record('remote-update', baseline)
            record('remote-update', remote)
            with self.assertRaisesRegex(Refused, 'replays'):
                record('reopen', remote)
            record('reopen', pair('reopen', 3, 4))

    def test_saved_verdict_rechecks_cross_milestone_identity_with_all_checks_present(self):
        """Legacy accepted replacements must not pass or authorize cleanup after upgrading the harness.

Start with a complete valid synthetic pass, then model the old coordinator's
otherwise well-formed replacement receipts. This excludes missing checks and
artifact corruption as competing causes of the new refusal.
"""
        self.prepare('upgrade-v0.24.0')
        image = self.evidence_file('frame.png', PNG)
        command = self.evidence_file('command.txt', b'fictional command evidence')
        verification = self.evidence_file('verification.json', json.dumps({
            'version': 'v0.25.0', 'previous_trust_ring': 'v0.24.0',
            'sha256sums_sha256': 'fixture-checksum', 'signature': 'verified',
            'trusted_comment': 'verified', 'installer_checksum': 'verified'}).encode())
        identity = {'pid': 42, 'start_ticks': 100, 'boot_id': '00000000-0000-0000-0000-000000000001'}
        order = ['continuity-before', 'remote-update', 'reopen']
        with self.run.locked():
            self.run.select_update('upgrade-v0.24.0', 'v0.24.0')
            entry = self.run.selected('upgrade-v0.24.0')
            for check_id, spec in entry['required'].items():
                refs = [verification] if check_id == 'artifact-verification' else [image, command]
                before = after = None
                if spec['continuity']:
                    low = 1 + order.index(check_id)
                    before = self.evidence_file(check_id + '-before.json', json.dumps(dict(identity, counter=low)).encode())
                    after = self.evidence_file(check_id + '-after.json', json.dumps(dict(identity, counter=low + 1)).encode())
                    refs.extend([before, after])
                self.run.record('upgrade-v0.24.0', check_id, 'pass', refs,
                                'Complete synthetic acceptance fixture; no product behavior was tested.', before, after)
            self.assertEqual(len(entry['results']), 10)
            self.assertEqual(self.run.verdict(entry), 'pass')
            result = entry['results']['reopen']
            for role, counter in (('before', 3), ('after', 4)):
                ref = result['continuity'][role]
                self.evidence_file(ref, json.dumps(dict(identity, pid=99, counter=counter)).encode())
                result['sha256'][ref] = hashlib.sha256(artifact(self.run.directory, ref)).hexdigest()
            self.assertEqual(self.run.verdict(entry), 'could not run')
            self.run.save()
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup('upgrade-v0.24.0')
        self.assertFalse(any(call[1] == 'delete' for call in self.model.calls))

    def test_latest_is_reobserved_before_install_and_promotion_is_reported(self):
        """Promotion during old-state seeding must change path and invalidate pre-promotion coverage."""
        self.prepare('upgrade-v0.24.0')
        image = self.evidence_file('frame.png', PNG)
        command = self.evidence_file('command.txt', b'fixture installed version')
        with self.run.locked():
            with self.assertRaises(Refused):
                self.run.record('upgrade-v0.24.0', 'update-relaunch', 'pass', [image, command], 'Fixture check.')
            self.run.select_update('upgrade-v0.24.0', 'v0.25.0')
            self.assertEqual(self.run.selected('upgrade-v0.24.0')['update_path'], 'ordinary')
            self.assertEqual(self.run.report()['promotion_coverage'], 'after-publication')

    def test_required_addendum_checks_cannot_replace_baseline(self):
        """Release-specific coverage adds obligations instead of renaming away missing checks."""
        with self.run.locked():
            baseline = set(self.run.selected('fresh')['required'])
            self.run.add_check('fresh', 'extra-new-feature', gui=True)
            self.assertTrue(baseline < set(self.run.selected('fresh')['required']))
            with self.assertRaises(Refused):
                self.run.add_check('fresh', 'app-start')

    def test_fixture_smoke_exports_stops_and_deletes_only_working_guests(self):
        """The short smoke validates orchestration without manufacturing a release verdict."""
        second = Run(self.evidence / ('brick-' + str(uuid.uuid4())), self.profile,
                     factory=self.model.factory, connector=self.model.connect)
        second.create(smoke=True)
        smoke(second)
        with second.locked():
            self.assertEqual(second.report()['verdict'], 'fixture smoke: pass')
            self.assertEqual(second.selected('smoke')['phase'], 'deleted')
        self.assertEqual(set(self.model.states), {'mac-base', 'linux-base'})
        self.assertTrue(all(state == 'stopped' for state in self.model.states.values()))
        self.assertTrue(all(child.poll() == 0 for child in self.model.children))


if __name__ == '__main__':
    unittest.main()
