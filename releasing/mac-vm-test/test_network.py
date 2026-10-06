"""Keep generated guest SSH configuration isolated and syntactically constrained.

The documentation-only address, guest account, UUID and key payload are fictional
test data. Nothing is copied from a host, guest or private machine profile.
"""

import unittest
import subprocess

from control import Refused
from network import configuration, route_guests


class ConfigurationTests(unittest.TestCase):
    """Check the trusted-channel pin and prevent inputs becoming SSH directives."""

    def test_run_alias_and_pin_share_identity_without_private_comment(self):
        """A run-specific alias must use the exact supplied pin, without machine comments."""
        run_id = 'd2cedd3c-575e-458c-b6b7-03e40b9b4e85'
        alias, config, pin = configuration(run_id, '192.0.2.20', 'guest',
                                           'ssh-ed25519 Zml4dHVyZQ== private-image-comment')
        self.assertEqual(alias, f'fh-linux-{run_id}')
        self.assertIn(f'Host {alias}\n', config)
        self.assertIn(f'HostKeyAlias {alias}\n', config)
        self.assertIn('StrictHostKeyChecking yes', config)
        self.assertIn('IdentitiesOnly yes', config)
        self.assertEqual(pin, f'{alias} ssh-ed25519 Zml4dHVyZQ==\n')
        self.assertNotIn('private-image-comment', pin)

    def test_username_cannot_inject_an_ssh_directive(self):
        """An account input must not broaden access through generated SSH configuration."""
        with self.assertRaises(Refused):
            configuration('d2cedd3c-575e-458c-b6b7-03e40b9b4e85', '192.0.2.20',
                          'guest\n  StrictHostKeyChecking no', 'ssh-ed25519 Zml4dHVyZQ==')

    def test_address_must_be_an_ip_not_ssh_config_text(self):
        """Only an actual discovered address may become the guest HostName value."""
        with self.assertRaises(ValueError):
            configuration('d2cedd3c-575e-458c-b6b7-03e40b9b4e85',
                          'localhost\nProxyCommand surprise', 'guest', 'ssh-ed25519 Zml4dHVyZQ==')

    def test_disabled_host_forwarding_is_refused_without_guest_mutations(self):
        """A guest route workaround must never silently enable a host network setting."""
        class DisabledHost:
            """Permit only the read-only forwarding probe in this fixture."""
            def runner(self, argv, **kwargs):
                """Report a host that has not enabled forwarding."""
                self_probe = ['/usr/sbin/sysctl', '-n', 'net.inet.ip.forwarding']
                if argv != self_probe:
                    raise AssertionError('unexpected host action')
                return subprocess.CompletedProcess(argv, 0, b'0\n')

            def command(self, *args, **kwargs):
                """Any guest command here would precede the required host premise."""
                raise AssertionError('guest command must not run')

        with self.assertRaisesRegex(Refused, 'not already enabled'):
            route_guests(DisabledHost(), 'owned-mac', 'owned-linux', '192.0.2.20')


if __name__ == '__main__':
    unittest.main()
