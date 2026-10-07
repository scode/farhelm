#!/usr/bin/env python3
"""Connect owned guests through guest-local keys and verified SSH host identity.

Tart's command channel reads Linux's host key directly; an unauthenticated
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


def configuration(run_id, address, username, host_key):
    """Build an isolated guest SSH alias without allowing config-line injection.

The key comes from the recorded Linux guest, not the network. Discard its
optional comment, which may contain a machine identity irrelevant to pinning.
"""
    alias = f'fh-linux-{uuid.UUID(run_id)}'
    address = str(ipaddress.ip_address(address))
    if not re.fullmatch(r'[a-z_][a-z0-9_-]*[$]?', username):
        raise Refused('Linux username must be a plain account name')
    fields = host_key.split()
    if len(fields) < 2 or fields[0] != 'ssh-ed25519':
        raise Refused('expected the Linux guest ed25519 host key')
    base64.b64decode(fields[1], validate=True)
    pinned = f'{alias} {fields[0]} {fields[1]}\n'
    config = (f'Host {alias}\n  HostName {address}\n  User {username}\n'
              '  IdentityFile ~/.ssh/farhelm-test\n  IdentitiesOnly yes\n'
              f'  HostKeyAlias {alias}\n'
              '  UserKnownHostsFile ~/.ssh/farhelm-test-known-hosts\n'
              '  StrictHostKeyChecking yes\n  BatchMode yes\n  ConnectTimeout 10\n')
    return alias, config, pinned


def route_guests(control, mac, linux, linux_address):
    """Route peers through the existing gateway without editing the host network.

On the verified macOS 26 host, Apple's NAT bridge marks both guest ports PRIVATE,
so peer ARP never reaches the other guest. The host already forwards IP traffic.
Reciprocal guest host routes work around that port isolation; a failed earlier
ARP lookup on macOS must be removed because its scoped reject route otherwise
shadows the new static gateway route. Never enable host forwarding implicitly.
"""
    forwarding = control.runner(['/usr/sbin/sysctl', '-n', 'net.inet.ip.forwarding'],
                                check=True, capture_output=True, timeout=5).stdout.strip()
    if forwarding != b'1':
        raise Refused('host IP forwarding is not already enabled; discuss host changes with the operator')
    mac_address = control.command('ip', mac, '--wait', '10', capture_output=True).stdout.decode().strip()
    mac_route = control.command('exec', mac, '/sbin/route', '-n', 'get', 'default', capture_output=True).stdout.decode()
    match = re.search(r'^\s+gateway:\s+(\S+)', mac_route, re.MULTILINE)
    linux_route = control.command('exec', linux, '/bin/sh', '-c',
                                  'ip -4 route show default', capture_output=True).stdout.decode().split()
    if not match or 'via' not in linux_route:
        raise Refused('unable to establish both guest gateways')
    gateway = str(ipaddress.IPv4Address(match.group(1)))
    linux_gateway = str(ipaddress.IPv4Address(linux_route[linux_route.index('via') + 1]))
    mac_address = str(ipaddress.IPv4Address(mac_address))
    linux_address = str(ipaddress.IPv4Address(linux_address))
    if gateway != linux_gateway:
        raise Refused('guest gateways differ; do not guess a routing topology')
    control.mutate(['exec', mac, '/bin/sh', '-c',
                    '/usr/bin/sudo -n /sbin/route -n add -host "$1" "$2" || '
                    '/usr/bin/sudo -n /sbin/route -n change -host "$1" "$2"',
                    'farhelm-peer-route', linux_address, gateway],
                   'Route only the Linux peer through the existing gateway inside macOS.')
    control.mutate(['exec', mac, '/bin/sh', '-c',
                    'if /usr/sbin/arp -n "$1" >/dev/null 2>&1; then '
                    '/usr/bin/sudo -n /usr/sbin/arp -d "$1"; fi',
                    'farhelm-peer-neighbor', linux_address],
                   'Remove only an existing failed/scoped guest peer neighbor entry.')
    control.mutate(['exec', linux, '/usr/bin/sudo', '-n', 'ip', 'route', 'replace',
                    f'{mac_address}/32', 'via', gateway],
                   'Route only replies to the macOS peer through the existing gateway inside Linux.')


def connect(control, mac, linux, username, route_via_gateway=False):
    """Configure a pair whose ownership the caller has already checked.

Bases may retain this private keypair and Linux authorization. Each new pair
refreshes the alias, address and pin from its own manifest; DHCP addresses are
never assumed to persist across cloning. All writes occur inside the guests.
    """
    address = control.command('ip', linux, '--wait', '10', capture_output=True).stdout.decode().strip()
    if route_via_gateway:
        route_guests(control, mac, linux, address)
    key = control.command('exec', linux, '/bin/cat', '/etc/ssh/ssh_host_ed25519_key.pub', capture_output=True).stdout.decode()
    alias, config, pinned = configuration(control.data['run_id'], address, username, key)
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
    control.mutate(['exec', '-i', linux, '/usr/bin/python3', '-c', authorize],
                   'Authorize only the Mac guest public key on Linux; input omitted.', input=public_key)
    write_configuration = (
        'from pathlib import Path; import json,sys; '
        'v=json.load(sys.stdin); d=Path.home()/".ssh"; '
        '[(d/n).write_text(v[k]) for n,k in [("farhelm-test.conf","config"),("farhelm-test-known-hosts","pin")]]; '
        '[(d/n).chmod(0o600) for n in ["farhelm-test.conf","farhelm-test-known-hosts"]]; '
        'p=d/"config"; old=p.read_text() if p.exists() else ""; include="Include ~/.ssh/farhelm-test.conf"; '
        'p.write_text(old if include in old.splitlines() else include+"\\n"+old); p.chmod(0o600)'
    )
    payload = json.dumps({'config': config, 'pin': pinned}).encode()
    control.mutate(['exec', '-i', mac, '/opt/homebrew/bin/python3', '-c', write_configuration],
                   'Pin Linux host identity and configure only the run alias in the Mac guest.', input=payload)
    control.mutate(['exec', mac, '/usr/bin/ssh', alias,
                    'id && systemctl --user show-environment >/dev/null && printf "guest-to-guest-ssh-ready\\n"'],
                   'Prove authenticated Mac-to-Linux SSH and the SSH-launched user manager.')
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
