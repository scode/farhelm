"""Pin a checkout-independent private machine interface without environment mutation."""

import json
from pathlib import Path
import tempfile
import unittest

from control import Refused
from machine_profile import Profile, discover


def fixture(root):
    """Create fictional private bindings, never importing the operator's actual profile.

Relative paths deliberately differ from a source checkout: that separation is
the contract the one-line agent request relies on across machines.
"""
    root = Path(root)
    (root / 'policy.md').write_text('Fixture operating policy.\n')
    return {'format': 1, 'mac_base': {'name': 'mac-base', 'mac_address': '02:00:00:00:00:01'},
            'linux_base': {'name': 'linux-base', 'mac_address': '02:00:00:00:00:02'},
            'linux_user': 'guest', 'tart_home': '~/vm-store', 'journal': 'journal.md',
            'evidence_root': 'runs', 'operator_policy': 'policy.md',
            'resource_limits': {'minimum_free_gib': 1, 'maximum_run_growth_gib': 5},
            'route_via_gateway': False, 'agents': ['codex']}


class ProfileTests(unittest.TestCase):
    """Refuse ambiguous configuration before any VM runner becomes reachable."""

    def setUp(self):
        """Own a private configuration tree and an unrelated simulated home."""
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        # macOS exposes temporary paths through /var -> /private/var. Profile
        # normalization resolves that alias, so fixture expectations must too.
        self.root = Path(self.tmp.name).resolve()
        self.home = self.root / 'different-home'
        self.path = self.root / 'environment.json'
        self.data = fixture(self.root)
        self.write()

    def write(self):
        """Update only this test's private file, with the production permission premise."""
        self.path.write_text(json.dumps(self.data))
        self.path.chmod(0o600)

    def load(self):
        """Inject home resolution rather than editing HOME in the shared process."""
        return Profile.load(self.path, home=self.home)

    def test_discovery_precedence_and_convention(self):
        """A configured override cannot be silently replaced by a fallback profile."""
        env = {'FARHELM_BRICK_TEST_PROFILE': str(self.root / 'override.json')}
        self.assertEqual(discover(environ={}, home=self.home), self.home / '.config/farhelm/brick-test.json')
        self.assertEqual(discover(environ=env, home=self.home), self.root / 'override.json')
        self.assertEqual(discover(self.path, environ=env, home=self.home), self.path)
        with self.assertRaisesRegex(Refused, 'missing'):
            Profile.load(discover(environ=env, home=self.home))

    def test_bindings_are_relative_to_profile_not_checkout(self):
        """Moving the checkout must not retarget journal, policy or VM storage."""
        profile = self.load()
        self.assertEqual(profile.values['journal'], str(self.root / 'journal.md'))
        self.assertEqual(profile.values['tart_home'], str(self.home / 'vm-store'))
        self.assertEqual(profile.values['evidence_root'], str(self.root / 'runs'))

    def test_format_two_runtime_and_consent_paths_are_explicit(self):
        """Native setup references follow the private profile, never the current checkout."""
        self.data.update(format=2, guest_tester={
            'settings': {'codex': '/fixture/codex', 'python': '/fixture/python',
                         'desktop_app': '/fixture/Codex.app', 'model': 'fixture-model', 'effort': 'high'},
            'auth_source': 'fixture-login.json', 'authorization': 'consent.json', 'fixture_receipt': 'native.json'})
        self.write()
        profile = self.load()
        self.assertEqual(profile.values['guest_tester']['authorization'], str(self.root / 'consent.json'))
        self.assertEqual(profile.values['guest_tester']['auth_source'], str(self.root / 'fixture-login.json'))
        self.data['guest_tester']['settings']['codex'] = 'relative/codex'
        self.write()
        with self.assertRaisesRegex(Refused, 'absolute'):
            self.load()

    def test_legacy_format_cannot_smuggle_native_fields_and_references_stay_private(self):
        """Historical cleanup compatibility must not silently acquire new credential capabilities."""
        config = {'settings': {'codex': '/fixture/codex', 'python': '/fixture/python',
                              'desktop_app': '/fixture/Codex.app', 'model': 'fixture-model', 'effort': 'high'},
                  'auth_source': 'public/login.json', 'authorization': 'consent.json', 'fixture_receipt': 'native.json'}
        self.data['guest_tester'] = config
        self.write()
        with self.assertRaisesRegex(Refused, 'format'):
            self.load()
        self.data['format'] = 2
        self.write()
        with self.assertRaisesRegex(Refused, 'outside'):
            Profile.load(self.path, home=self.home, repo=self.root / 'public')

    def test_incomplete_and_secret_like_extra_fields_are_refused(self):
        """Misspellings and credential additions need an explicit schema change."""
        for key in ('journal', 'resource_limits'):
            original = self.data.copy()
            del self.data[key]
            self.write()
            with self.assertRaises(Refused):
                self.load()
            self.data = original
        self.data['token'] = 'fictional-value'
        self.write()
        with self.assertRaises(Refused):
            self.load()

    def test_public_and_readable_profiles_are_refused(self):
        """Private bindings must not be kept in the public source tree or exposed by mode."""
        with self.assertRaisesRegex(Refused, 'outside'):
            Profile.load(self.path, home=self.home, repo=self.root)
        self.path.chmod(0o644)
        with self.assertRaisesRegex(Refused, 'private'):
            self.load()

    def test_unsafe_base_names_and_account_directives_are_refused(self):
        """Profile identifiers must remain data, not filesystem or SSH instructions."""
        self.data['mac_base']['name'] = '../foreign-base'
        self.write()
        with self.assertRaises(Refused):
            self.load()
        self.data = fixture(self.root)
        self.data['linux_user'] = 'guest\nProxyCommand surprise'
        self.write()
        with self.assertRaises(Refused):
            self.load()

    def test_invalid_budgets_and_same_base_are_refused(self):
        """Resource fences require meaningful values and two distinct source guests."""
        for value in (0, -1, True, float('nan'), float('inf')):
            self.data['resource_limits']['minimum_free_gib'] = value
            self.write()
            with self.assertRaises(Refused):
                self.load()
        self.data = fixture(self.root)
        self.data['linux_base']['name'] = self.data['mac_base']['name']
        self.write()
        with self.assertRaises(Refused):
            self.load()

    def test_shell_expansion_and_missing_policy_are_refused(self):
        """The workflow must not acquire meaning from ambient shell variables."""
        self.data['evidence_root'] = '$HOME/runs'
        self.write()
        with self.assertRaises(Refused):
            self.load()

        self.data = fixture(self.root)
        self.data['operator_policy'] = 'missing.md'
        self.write()
        with self.assertRaises(Refused):
            self.load()

    def test_split_vm_and_evidence_volumes_are_refused(self):
        """The single-volume budget cannot silently authorize writes onto another disk."""
        def volume_id(path):
            """Model distinct mounted devices without creating real host mounts."""
            return 'vm-volume' if path.name == 'vms' else 'evidence-volume'
        with self.assertRaisesRegex(Refused, 'share a filesystem'):
            Profile.load(self.path, home=self.home, volume_id=volume_id)

    def test_profile_install_is_private_idempotent_and_does_not_clobber(self):
        """One-time setup may retain identical bindings but cannot replace another machine."""
        profile = self.load()
        output = self.home / '.config/farhelm/brick-test.json'
        self.assertTrue(profile.write(output))
        self.assertFalse(profile.write(output))
        self.assertEqual(output.stat().st_mode & 0o777, 0o600)
        original = output.read_bytes()
        different = dict(profile.values, linux_user='otherguest')
        with self.assertRaises(Refused):
            Profile(self.path, different).write(output)
        self.assertEqual(output.read_bytes(), original)


if __name__ == '__main__':
    unittest.main()
