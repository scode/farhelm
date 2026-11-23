#!/usr/bin/env python3
"""Connect owned guests through guest-local keys and verified SSH host identity.

Tart's command channel reads the peer's host key directly; an unauthenticated
keyscan is not the trust source. The generated SSH configuration lives only in
the Mac guest, applies to a run-specific alias, and retains strict verification.
Nothing edits the host Mac's SSH configuration or authentication stores.
"""

import argparse
import base64
import ipaddress
import json
from pathlib import Path
import re
import uuid

from control import Control, Refused


PEER_ALIAS_PREFIX = {'linux': 'fh-linux', 'remote-mac': 'fh-mac'}
MAC_PYTHON = '/opt/homebrew/bin/python3'
HOST_KEY = '/etc/ssh/ssh_host_ed25519_key.pub'


def configuration(run_id, address, username, host_key, *, peer_role='linux'):
    """Build an isolated guest SSH alias without allowing config-line injection.

The key comes from the recorded peer guest, not the network. Discard its
optional comment, which may contain a machine identity irrelevant to pinning.
"""
    if peer_role not in PEER_ALIAS_PREFIX:
        raise Refused('unknown SSH peer role')
    alias = f'{PEER_ALIAS_PREFIX[peer_role]}-{uuid.UUID(run_id)}'
    parsed = ipaddress.ip_address(address)
    # ipaddress preserves arbitrary scope text, including SSH line breaks.
    # Guest addresses need no interface scope; plain normalized IPs are tokens.
    if isinstance(parsed, ipaddress.IPv6Address) and parsed.scope_id is not None:
        raise Refused('guest IPv6 address must not have an interface scope')
    address = str(parsed)
    username_pattern = r'[a-z_][a-z0-9_-]*[$]?' if peer_role == 'linux' else r'[A-Za-z_][A-Za-z0-9_-]*'
    if not re.fullmatch(username_pattern, username):
        raise Refused(f'{peer_role} username must be a plain account name')
    fields = host_key.split()
    if len(fields) < 2 or fields[0] != 'ssh-ed25519':
        raise Refused(f'expected the {peer_role} guest ed25519 host key')
    base64.b64decode(fields[1], validate=True)
    pinned = f'{alias} {fields[0]} {fields[1]}\n'
    config = (f'Host {alias}\n  HostName {address}\n  User {username}\n'
              '  IdentityFile ~/.ssh/farhelm-test\n  IdentitiesOnly yes\n'
              f'  HostKeyAlias {alias}\n'
              '  UserKnownHostsFile ~/.ssh/farhelm-test-known-hosts\n'
              '  StrictHostKeyChecking yes\n  BatchMode yes\n  ConnectTimeout 10\n')
    return alias, config, pinned


def route_mac_peer(control, guest, dest, gateway):
    """Route one Mac guest to its peer through the existing NAT gateway.

A scoped reject route left by unsuccessful peer ARP can shadow the new host
route. Clear only the peer's neighbor cache entry in the owned VM.
"""
    control.mutate(['exec', guest, '/bin/sh', '-c',
                    '/usr/bin/sudo -n /sbin/route -n add -host "$1" "$2" || '
                    '/usr/bin/sudo -n /sbin/route -n change -host "$1" "$2"',
                    'farhelm-peer-route', dest, gateway],
                   'Route only the peer through the existing gateway inside macOS.')
    control.mutate(['exec', guest, '/bin/sh', '-c',
                    'if /usr/sbin/arp -n "$1" >/dev/null 2>&1; then '
                    '/usr/bin/sudo -n /usr/sbin/arp -d "$1"; fi',
                    'farhelm-peer-neighbor', dest],
                   'Remove only an existing failed/scoped guest peer neighbor entry.')


def route_guests(control, mac, peer, peer_address, *, peer_role='linux'):
    """Route peers through the existing gateway without editing the host network.

On the verified macOS 26 host, Apple's NAT bridge marks both guest ports PRIVATE,
so peer ARP never reaches the other guest. The host already forwards IP traffic.
Reciprocal guest host routes work around that port isolation; a failed earlier
ARP lookup on macOS must be removed because its scoped reject route otherwise
shadows the new static gateway route. Never enable host forwarding implicitly.
"""
    gateway_probes = {
        'remote-mac': (('/sbin/route', '-n', 'get', 'default'), r'^\s+gateway:\s+(\S+)'),
        'linux': (('/bin/sh', '-c', 'ip -4 route show default'), r'\bvia\s+(\S+)'),
    }
    if peer_role not in gateway_probes:
        raise Refused('unknown SSH peer role')
    forwarding = control.runner(['/usr/sbin/sysctl', '-n', 'net.inet.ip.forwarding'],
                                check=True, capture_output=True, timeout=5).stdout.strip()
    if forwarding != b'1':
        raise Refused('host IP forwarding is not already enabled; discuss host changes with the operator')
    mac_address = control.command('ip', mac, '--wait', '10', capture_output=True).stdout.decode().strip()
    mac_cmd, mac_pattern = gateway_probes['remote-mac']
    peer_cmd, peer_pattern = gateway_probes[peer_role]
    mac_route = control.command('exec', mac, *mac_cmd, capture_output=True).stdout.decode()
    peer_route = control.command('exec', peer, *peer_cmd, capture_output=True).stdout.decode()
    mac_gateway = re.search(mac_pattern, mac_route, re.MULTILINE)
    peer_gateway = re.search(peer_pattern, peer_route, re.MULTILINE)
    if not mac_gateway or not peer_gateway:
        raise Refused('unable to establish both guest gateways')
    gateway = str(ipaddress.IPv4Address(mac_gateway.group(1)))
    mac_address = str(ipaddress.IPv4Address(mac_address))
    peer_address = str(ipaddress.IPv4Address(peer_address))
    if gateway != str(ipaddress.IPv4Address(peer_gateway.group(1))):
        raise Refused('guest gateways differ; do not guess a routing topology')
    route_mac_peer(control, mac, peer_address, gateway)
    if peer_role == 'remote-mac':
        route_mac_peer(control, peer, mac_address, gateway)
    else:
        control.mutate(['exec', peer, '/usr/bin/sudo', '-n', 'ip', 'route', 'replace',
                        f'{mac_address}/32', 'via', gateway],
                       'Route only replies to the macOS peer through the existing gateway inside Linux.')


def peer_host_key(control, mac, peer, peer_role):
    """Read the peer pin through Tart and break shared host identities on Mac clones.

The default Mac pair can inherit a single host key from its common image.
When that happens, rotate the peer's Ed25519 key inside that owned clone.
Remote Login starts a new sshd per connection, so the following authenticated
connection will use the fresh key. The primary clone and bases stay untouched.
"""
    key = control.command('exec', peer, '/bin/cat', HOST_KEY, capture_output=True).stdout.decode()
    if peer_role != 'remote-mac':
        return key
    primary = control.command('exec', mac, '/bin/sh', '-c',
                              'if test -f "$1"; then /bin/cat "$1"; fi',
                              'farhelm-primary-host-key', HOST_KEY, capture_output=True).stdout.decode()
    # SSH key comments may differ without the actual identities differing.
    if primary.split()[:2] != key.split()[:2]:
        return key
    script = r'''work=$(/usr/bin/mktemp -d /etc/ssh/.farhelm-hostkey.XXXXXX) || exit 1
trap '/bin/rm -f "$work/key" "$work/key.pub"; /bin/rmdir "$work"' EXIT
/usr/bin/ssh-keygen -q -t ed25519 -N '' -f "$work/key" &&
    /bin/mv -f "$work/key" /etc/ssh/ssh_host_ed25519_key &&
    /bin/mv -f "$work/key.pub" /etc/ssh/ssh_host_ed25519_key.pub
'''
    control.mutate(['exec', peer, '/usr/bin/sudo', '-n', '/bin/sh', '-c', script],
                   'Give the disposable remote Mac its own Ed25519 SSH host identity.')
    fresh = control.command('exec', peer, '/bin/cat', HOST_KEY, capture_output=True).stdout.decode()
    if fresh.split()[:2] == key.split()[:2]:
        raise Refused('the remote Mac still shares the GUI Mac SSH host key')
    return fresh


def connect(control, mac, peer, username, route_via_gateway=False, *, peer_role='linux'):
    """Configure a pair whose ownership the caller has already checked.

Bases may retain this private keypair and peer authorization. Each new pair
refreshes the alias, address and pin from its own manifest; DHCP addresses are
never assumed to persist across cloning. All writes occur inside the guests.
    """
    address = control.command('ip', peer, '--wait', '10', capture_output=True).stdout.decode().strip()
    key = peer_host_key(control, mac, peer, peer_role)
    alias, config, pinned = configuration(control.data['run_id'], address, username, key, peer_role=peer_role)
    if route_via_gateway:
        route_guests(control, mac, peer, address, peer_role=peer_role)
    control.mutate(['exec', mac, '/bin/sh', '-c',
                    'umask 077; mkdir -p "$HOME/.ssh" && chmod 700 "$HOME/.ssh" && '
                    'if test ! -f "$HOME/.ssh/farhelm-test"; then '
                    '/usr/bin/ssh-keygen -q -t ed25519 -N "" -f "$HOME/.ssh/farhelm-test"; fi'],
                   'Create/reuse a guest-only SSH keypair; private key is not exported.')
    # Resolve the guest home rather than assuming the Cirrus image's account
    # when reading files. This keeps a prepared base usable with another user.
    public_key = control.command('exec', mac, '/bin/sh', '-c',
                                 'cat "$HOME/.ssh/farhelm-test.pub"', capture_output=True).stdout
    if not public_key.startswith(b'ssh-ed25519 '):
        raise Refused('unexpected Mac guest public key')
    authorize = (
        'from pathlib import Path; import sys; '
        'd=Path.home()/".ssh"; d.mkdir(mode=0o700,exist_ok=True); d.chmod(0o700); '
        'p=d/"authorized_keys"; key=sys.stdin.read().strip(); '
        'old=p.read_text() if p.exists() else ""; '
        'p.write_text(old if key in old.splitlines() else old.rstrip()+"\\n"+key+"\\n"); p.chmod(0o600)'
    )
    peer_python = MAC_PYTHON if peer_role == 'remote-mac' else '/usr/bin/python3'
    control.mutate(['exec', '-i', peer, peer_python, '-c', authorize],
                   'Authorize only the GUI Mac guest public key on the peer; input omitted.', input=public_key)
    write_configuration = (
        'from pathlib import Path; import json,sys; '
        'v=json.load(sys.stdin); d=Path.home()/".ssh"; '
        '[(d/n).write_text(v[k]) for n,k in [("farhelm-test.conf","config"),("farhelm-test-known-hosts","pin")]]; '
        '[(d/n).chmod(0o600) for n in ["farhelm-test.conf","farhelm-test-known-hosts"]]; '
        'p=d/"config"; old=p.read_text() if p.exists() else ""; include="Include ~/.ssh/farhelm-test.conf"; '
        'p.write_text(old if include in old.splitlines() else include+"\\n"+old); p.chmod(0o600)'
    )
    payload = json.dumps({'config': config, 'pin': pinned}).encode()
    control.mutate(['exec', '-i', mac, MAC_PYTHON, '-c', write_configuration],
                   'Pin the peer host identity and configure only the run alias in the Mac guest.', input=payload)
    user_manager = ('/bin/launchctl print "user/$(id -u)"' if peer_role == 'remote-mac' else
                    'systemctl --user show-environment')
    control.mutate(['exec', mac, '/usr/bin/ssh', alias,
                    f'id && {user_manager} >/dev/null && printf "guest-to-guest-ssh-ready\\n"'],
                   'Prove authenticated guest SSH and the service manager for that login.')
    control.data['ssh'] = {'alias': alias, 'address': address, 'user': username,
                           'route_via_gateway': route_via_gateway}
    control.save()
    print(f'Guest SSH alias: {alias}')


def main():
    """Resolve both VM identities from the manifest before generating guest writes."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run-dir', required=True, type=Path)
    parser.add_argument('--journal', required=True, type=Path)
    parser.add_argument('--linux-user', required=True)
    parser.add_argument('--route-via-gateway', action='store_true')
    parser.add_argument('--tart-home', type=Path, default=Path.home() / '.tart')
    parser.add_argument('--tart', default='tart')
    args = parser.parse_args()
    control = Control(args.run_dir, args.journal, args.tart_home, args.tart)
    with control.locked():
        connect(control, control.owned('mac'), control.owned('linux'), args.linux_user,
                route_via_gateway=args.route_via_gateway)


if __name__ == '__main__':
    main()
