"""Pin setup consent and credential destinations without using real operator data."""

import json
from pathlib import Path
import tempfile
import subprocess
import os
import sys
import types
import unittest

from control import Refused
from brick_test import Source
from guest_workflow import copy_credentials, require_setup, source_url, task_for
from test_guest_tester import task


class CheckpointInstructionsTests(unittest.TestCase):
    """Inspect the actual guest envelopes where update-path actions are assigned.

Bonus upgrades combine discovery and relaunch in one checkpoint. P0 gives
discovery its own checkpoint, so duplicating that action can invalidate the
independent updater check. These assertions cover instructions, not live UI.
"""

    def envelope(self, pass_id, check_id, update_path):
        """Supply fictional run context to the production task builder without a VM."""
        run = types.SimpleNamespace(
            selected=lambda name: {'required': {check_id: {'gui': True}}},
            data={'run_id': 'fixture-run'},
            profile=types.SimpleNamespace(values={'guest_tester': {'settings': task()['settings']}}))
        return task_for(run, pass_id, check_id,
                        {'pass_directory': '/fixture/pass', 'update_path': update_path})

    def test_bonus_discovery_precedes_restart_for_native_update_paths(self):
        """Disabled auto updates cannot create a Restart offer until the tester checks and installs."""
        for update_path in ('ordinary', 'override'):
            with self.subTest(path=update_path):
                text = self.envelope('upgrade-v0.24.0', 'update-relaunch', update_path)['instructions']
                self.assertIn('FARHELM_DESKTOP_UPDATE_LATEST=candidate', text)
                self.assertLess(text.index("Click the old app's actual Check for updates"),
                                text.index("wait on the old app's real Restart to update offer"))
                self.assertIn('Require discovery and installation of candidate', text)

    def test_manual_and_p0_relaunch_keep_their_existing_action_boundaries(self):
        """Manual baselines use the installer, while P0 must not repeat its separate discovery."""
        for pass_id, update_path in (('upgrade-v0.24.0', 'manual'),
                                    ('p0-installer-upgrade', 'manual'),
                                    ('p0-updater-upgrade', 'ordinary'),
                                    ('p0-updater-upgrade', 'override')):
            with self.subTest(pass_id=pass_id, path=update_path):
                text = self.envelope(pass_id, 'update-relaunch', update_path)['instructions']
                self.assertIn('For manual path only, run the verified candidate installer', text)
                self.assertNotIn("Click the old app's actual Check for updates", text)
        discovery = self.envelope('p0-updater-upgrade', 'update-discovery', 'ordinary')['instructions']
        self.assertIn("Click the old app's actual Check for updates", discovery)


class SetupTests(unittest.TestCase):
    """Require prepared native fixtures before any actual test can copy authentication."""

    def setUp(self):
        """Create fictional private setup records with independently checked file modes."""
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.config = {'settings': task()['settings'], 'auth_source': str(self.root / 'source.json'),
                       'authorization': str(self.root / 'consent.json'),
                       'fixture_receipt': str(self.root / 'fixture.json')}
        self.profile = types.SimpleNamespace(values={'guest_tester': self.config,
            'mac_base': {'name': 'mac-fixture', 'mac_address': '02:00:00:00:00:01'},
            'linux_base': {'name': 'linux-fixture', 'mac_address': '02:00:00:00:00:02'}})
        self.authorization = {'format': 1, 'approved': True, 'scope': 'disposable-test-clones',
                              'destinations': ['mac', 'linux'], 'auth_source': self.config['auth_source']}
        self.receipt = {'format': 1, 'bases': {role: self.profile.values[role + '_base']
                                             for role in ('mac', 'linux')},
                        'settings': self.config['settings'], 'checks': {key: True for key in (
                            'native-screenshot-click-type', 'native-after-reopen', 'clone-permission-carryover',
                            'auth-not-in-bases', 'farhelm-clean', 'linux-codex-ready')}}
        self.write('consent.json', self.authorization)
        self.write('fixture.json', self.receipt)
        self.write('source.json', {'fixture': 'fictional-login-bytes'})

    def write(self, name, value):
        """Write only owned fixture files, never ambient authentication or environment."""
        path = self.root / name
        path.write_text(json.dumps(value))
        path.chmod(0o600)

    def test_consent_requires_both_disposable_roles_and_exact_source(self):
        """Mac consent alone must not authorize a later remote-agent credential copy."""
        self.assertEqual(require_setup(self.profile), self.config)
        for changed in ({'destinations': ['mac']}, {'approved': False},
                        {'auth_source': '/fictional/other-login'}, {'scope': 'reusable-base'}):
            self.write('consent.json', dict(self.authorization, **changed))
            with self.subTest(changed=changed), self.assertRaisesRegex(Refused, 'standing consent'):
                require_setup(self.profile)

    def test_fixture_identity_runtime_and_missing_capability_refuse(self):
        """An old setup attestation cannot silently certify a replacement base or runtime."""
        for key in self.receipt['checks']:
            value = dict(self.receipt, checks=dict(self.receipt['checks'], **{key: False}))
            self.write('fixture.json', value)
            with self.subTest(capability=key), self.assertRaises(Refused):
                require_setup(self.profile)
        self.write('fixture.json', dict(self.receipt, settings=dict(self.config['settings'], model='other')))
        with self.assertRaises(Refused):
            require_setup(self.profile)
        self.write('fixture.json', self.receipt)
        self.profile.values['mac_base'] = dict(self.profile.values['mac_base'], name='replacement')
        with self.assertRaises(Refused):
            require_setup(self.profile)

    def test_feedback_consent_is_separate_from_credential_destinations(self):
        """Credential injection cannot authorize an external feedback issue.

P0 remains runnable without feedback consent. Full/P1 setup must refuse before
starting unless the operator separately recorded that exact live-send scope.
"""
        self.assertEqual(require_setup(self.profile), self.config)
        with self.assertRaisesRegex(Refused, 'live P1 feedback'):
            require_setup(self.profile, feedback=True)
        self.write('consent.json', dict(self.authorization, feedback_send_approved=True))
        self.assertEqual(require_setup(self.profile, feedback=True), self.config)
        self.write('consent.json', dict(self.authorization, feedback_send_approved='yes'))
        with self.assertRaisesRegex(Refused, 'live P1 feedback'):
            require_setup(self.profile, feedback=True)

    def test_credentials_use_stdin_and_owned_destinations_only(self):
        """Journaled argv must not carry bytes, even when both roles share one immutable read."""
        calls = []
        class Controller:
            """Expose exact owned names and capture the otherwise private stdin boundary."""
            def owned(self, role):
                """Return the fixture-owned name rather than a caller-supplied destination."""
                return 'owned-' + role
            def mutate(self, argv, detail, **kwargs):
                """Retain inputs separately so tests can detect argv or journal disclosure."""
                calls.append((argv, detail, kwargs))
        expected = (self.root / 'source.json').read_bytes()
        copy_credentials(Controller(), self.config)
        self.assertEqual([call[0][2] for call in calls], ['owned-mac', 'owned-linux'])
        for argv, detail, kwargs in calls:
            self.assertEqual(kwargs['input'], expected)
            self.assertNotIn('fictional-login-bytes', json.dumps(argv) + detail)
        (self.root / 'source.json').chmod(0o644)
        with self.assertRaisesRegex(Refused, 'private'):
            copy_credentials(Controller(), self.config)
        self.assertEqual(len(calls), 2)

    def test_records_are_private_and_legacy_profiles_cannot_start(self):
        """Historical cleanup support cannot become a host-UI fallback for new tests."""
        (self.root / 'consent.json').chmod(0o644)
        with self.assertRaisesRegex(Refused, 'private'):
            require_setup(self.profile)
        with self.assertRaisesRegex(Refused, 'format 2'):
            require_setup(types.SimpleNamespace(values={}))

    def test_real_credential_writer_refuses_overwrite_and_restricts_modes(self):
        """Execute the actual stdin writer in child homes without mutating the harness environment."""
        homes = {role: self.root / role for role in ('mac', 'linux')}
        for home in homes.values():
            home.mkdir(mode=0o700)
        class Controller:
            """Map fictional owned guests to explicit isolated subprocess homes."""
            def owned(self, role):
                """Keep role lookup independent from arbitrary caller file paths."""
                return role
            def mutate(self, argv, detail, **kwargs):
                """Run the unchanged production write script with a child-only HOME."""
                return subprocess.run([sys.executable, *argv[4:]], check=True,
                                      env={'HOME': str(homes[argv[2]]), 'PATH': os.defpath}, **kwargs)
        copy_credentials(Controller(), self.config)
        for home in homes.values():
            stored = home / '.codex/auth.json'
            self.assertEqual(stored.read_bytes(), (self.root / 'source.json').read_bytes())
            self.assertEqual(stored.stat().st_mode & 0o777, 0o600)
        with self.assertRaises(subprocess.CalledProcessError):
            copy_credentials(Controller(), self.config)
        for home in homes.values():
            self.assertEqual((home / '.codex/auth.json').read_bytes(), (self.root / 'source.json').read_bytes())

    def test_source_origin_does_not_export_git_authentication(self):
        """Only an ordinary public GitHub URL can be passed to the guest source checkout."""
        def source(url):
            """Supply source metadata without Git, network access or host credentials."""
            return types.SimpleNamespace(git=lambda *argv: url)
        self.assertEqual(source_url(source('git@github.com:fixture/project.git')),
                         'https://github.com/fixture/project.git')
        for url in ('https://token@github.com/fixture/project', 'file:///private/checkout',
                    'https://example.invalid/fixture/project', 'https://github.com/fixture/project?token=x'):
            with self.subTest(origin=url), self.assertRaises(Refused):
                source_url(source(url))

    def test_real_source_contract_supplies_text_origin(self):
        """The actual Git reader must compose with guest staging, not only a shaped mock."""
        checkout = self.root / 'checkout'
        subprocess.run(['git', 'init', '-q', str(checkout)], check=True, capture_output=True)
        subprocess.run(['git', '-C', str(checkout), 'remote', 'add', 'origin',
                        'https://github.com/fixture/project.git'], check=True, capture_output=True)
        self.assertEqual(source_url(Source(checkout)), 'https://github.com/fixture/project.git')


if __name__ == '__main__':
    unittest.main()
