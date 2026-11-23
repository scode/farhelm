"""Keep generated guest SSH configuration isolated and syntactically constrained.

The documentation-only address, guest account, UUID and key payload are fictional
test data. Nothing is copied from a host, guest or private machine profile.
"""

import json
import subprocess
import unittest

from control import Refused
from network import configuration, connect, route_guests


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

    def test_scoped_ipv6_cannot_carry_ssh_configuration(self):
        """Even valid ipaddress scope text must not become an SSH token or directive."""
        for address in ('fe80::1%eth0', 'fe80::1%eth 0', 'fe80::1%eth0\n  ProxyCommand surprise'):
            with self.subTest(address=address), self.assertRaises(Refused):
                configuration('d2cedd3c-575e-458c-b6b7-03e40b9b4e85', address,
                              'guest', 'ssh-ed25519 Zml4dHVyZQ==')

    def test_plain_ip_addresses_remain_usable(self):
        """The scope fence must preserve both supported IP families and normalization."""
        for address, normalized in (('192.0.2.20', '192.0.2.20'), ('2001:db8::0020', '2001:db8::20')):
            with self.subTest(address=address):
                _, config, _ = configuration('d2cedd3c-575e-458c-b6b7-03e40b9b4e85', address,
                                             'guest', 'ssh-ed25519 Zml4dHVyZQ==')
                self.assertIn(f'  HostName {normalized}\n', config)

    def test_remote_mac_connect_pins_the_peer_and_routes_both_mac_guests(self):
        """Clones of the same Mac base must no longer share the peer's pinned identity."""
        mac, peer = 'owned-mac', 'owned-remote-mac'
        run_id = 'd2cedd3c-575e-458c-b6b7-03e40b9b4e85'
        guest_key = b'ssh-ed25519 Zml4dHVyZQ=='
        shared_key = b'ssh-ed25519 c2hhcmVk'
        operator_key = b'ssh-ed25519 cHJpbWFyeQ==\n'
        routes = '/sbin/route', '-n', 'get', 'default'
        gateway = b'    gateway: 192.0.2.1\n'
        key_path = '/etc/ssh/ssh_host_ed25519_key.pub'
        peer_key_read = ('exec', peer, '/bin/cat', key_path)
        replies = {
            ('ip', peer, '--wait', '10'): b'192.0.2.20\n',
            ('ip', mac, '--wait', '10'): b'192.0.2.10\n',
            ('exec', mac, *routes): gateway,
            ('exec', peer, *routes): gateway,
            peer_key_read: shared_key + b' peer-comment\n',
            ('exec', mac, '/bin/sh', '-c', 'if test -f "$1"; then /bin/cat "$1"; fi',
             'farhelm-primary-host-key', key_path): shared_key + b' primary-comment\n',
            ('exec', mac, '/bin/sh', '-c', 'cat "$HOME/.ssh/farhelm-test.pub"'): operator_key,
        }
        writes = []
        reads = []

        class Pair:
            """Model Tart's trusted reads and record guest writes without invoking SSH."""

            data = {'run_id': run_id}
            saved = False

            def runner(self, argv, **kwargs):
                """The host side permits only observation of already enabled forwarding."""
                if argv != ['/usr/sbin/sysctl', '-n', 'net.inet.ip.forwarding']:
                    raise AssertionError(f'unexpected host command: {argv!r}')
                return subprocess.CompletedProcess(argv, 0, b'1\n')

            def command(self, *args, **kwargs):
                """Account for which owned VM supplied each identity and route observation."""
                reads.append(args)
                if args == peer_key_read and writes:
                    return subprocess.CompletedProcess(args, 0, guest_key + b' peer-comment\n')
                return subprocess.CompletedProcess(args, 0, replies[args])

            def mutate(self, argv, detail, **kwargs):
                """Capture each target and payload before the manifest is saved."""
                writes.append((argv, kwargs.get('input')))

            def save(self):
                """Expose when the completed SSH relationship becomes usable by the coordinator."""
                self.saved = True

        pair = Pair()
        connect(pair, mac, peer, 'FixtureUser', route_via_gateway=True, peer_role='remote-mac')
        alias = f'fh-mac-{run_id}'
        self.assertEqual(set(reads), set(replies))
        self.assertEqual(reads.count(peer_key_read), 2, 'pin a second trusted read after rotating a copied key')
        self.assertTrue(pair.saved)
        self.assertEqual(pair.data['ssh'], {'alias': alias, 'address': '192.0.2.20',
                                            'user': 'FixtureUser', 'route_via_gateway': True})
        rotation, *routing, _, authorization, config, proof = writes
        self.assertEqual(rotation[0][:6], ['exec', peer, '/usr/bin/sudo', '-n', '/bin/sh', '-c'])
        self.assertIn('/usr/bin/ssh-keygen -q -t ed25519', rotation[0][6])
        self.assertEqual([(argv[1], argv[5:]) for argv, _ in routing], [
            (mac, ['farhelm-peer-route', '192.0.2.20', '192.0.2.1']),
            (mac, ['farhelm-peer-neighbor', '192.0.2.20']),
            (peer, ['farhelm-peer-route', '192.0.2.10', '192.0.2.1']),
            (peer, ['farhelm-peer-neighbor', '192.0.2.10']),
        ])
        self.assertIn('/sbin/route -n add -host', routing[0][0][4])
        self.assertIn('/usr/sbin/arp -d', routing[3][0][4])
        self.assertEqual(authorization[0][:5], ['exec', '-i', peer, '/opt/homebrew/bin/python3', '-c'])
        self.assertEqual(authorization[1], operator_key)
        payload = json.loads(config[1])
        self.assertEqual(payload['pin'].encode(), alias.encode() + b' ' + guest_key + b'\n')
        self.assertIn(f'  HostKeyAlias {alias}\n', payload['config'])
        self.assertIn('  StrictHostKeyChecking yes\n', payload['config'])
        self.assertEqual(proof[0][:4], ['exec', mac, '/usr/bin/ssh', alias])
        self.assertIn('/bin/launchctl print "user/$(id -u)"', proof[0][4])


if __name__ == '__main__':
    unittest.main()
