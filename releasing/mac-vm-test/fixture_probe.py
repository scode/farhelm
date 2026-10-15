#!/usr/bin/env python3
"""Inspect clean guest fixtures without model-written path or service probes.

These are the shipped install/state defaults documented in SPEC_impl.md.
Absolute XDG overrides follow the supervisor and unit-directory rules; relative
overrides are ignored. The helper reads filesystem metadata, executable names
and service listings only. It never opens product databases, credentials or
configuration contents, and never repairs a dirty fixture.
"""

import argparse
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess


class ProbeError(RuntimeError):
    """A required observation was unavailable; absence has not been proved."""


def command(argv, runner=subprocess.run):
    """Retain bounded command outcomes without interpreting nonzero as absence."""
    result = runner(argv, capture_output=True, timeout=20)
    if len(result.stdout) + len(result.stderr) > 256 * 1024:
        raise ProbeError('fixture observation exceeded its output bound')
    return {'argv': argv, 'returncode': result.returncode,
            'stdout': result.stdout.decode('utf-8', errors='replace'),
            'stderr': result.stderr.decode('utf-8', errors='replace')}


def required(row):
    """Require a successful independent substrate observation, never silent failure."""
    if row['returncode'] != 0 or row['stderr'].strip():
        raise ProbeError('required observation failed: ' + ' '.join(row['argv']))
    return row['stdout']


def service_listing(row):
    """Accept systemd's empty-list exit 1 only with no output or diagnostics.

Callers first prove the relevant manager responds. This narrow no-match case
must not turn connection failures, missing commands or diagnostic output into
a clean fixture. Nonempty output identifies product residue even when inactive.
"""
    if row['returncode'] == 1 and not row['stdout'].strip() and not row['stderr'].strip():
        return ''
    return required(row)


def paths(home, environ):
    """Check default and absolute-XDG state plus both installer and provisioner layouts."""
    home = Path(home)
    state = home / '.local/state'
    config = home / '.config'
    if environ.get('XDG_STATE_HOME') and Path(environ['XDG_STATE_HOME']).is_absolute():
        state = Path(environ['XDG_STATE_HOME'])
    if environ.get('XDG_CONFIG_HOME') and Path(environ['XDG_CONFIG_HOME']).is_absolute():
        config = Path(environ['XDG_CONFIG_HOME'])
    values = {home / '.local/state/farhelm', state / 'farhelm', home / '.local/lib/farhelm',
              home / '.config/farhelm', home / 'Applications/Farhelm.app', Path('/Applications/Farhelm.app')}
    for directory in (home / '.local/bin', Path('/usr/local/bin'), Path('/usr/bin'), Path('/opt/homebrew/bin')):
        values.update(directory / name for name in ('farhelm', 'farhelm-desktop', 'farhelm-supervisor'))
    values.add(home / '.local/bin/.farhelm-installation')
    values.add(home / 'Applications/.farhelm-app.lock')
    bundle = 'org.scode.farhelm.desktop'
    values.add(home / ('Library/Preferences/' + bundle + '.plist'))
    values.add(home / ('Library/Saved Application State/' + bundle + '.savedState'))
    for directory in ('WebKit', 'Caches', 'HTTPStorages'):
        values.add(home / 'Library' / directory / bundle)
    for directory in (home / '.config/systemd/user', config / 'systemd/user',
                      Path('/etc/systemd/user'), Path('/etc/systemd/system'),
                      Path('/usr/lib/systemd/user'), Path('/usr/lib/systemd/system')):
        values.update(directory / name for name in ('farhelm-supervisor.service', 'farhelm-helm.service'))
    return sorted(values)


def present(path):
    """Distinguish actual absence from inaccessible metadata, retaining broken links as residue."""
    try:
        Path(path).lstat()
        return True
    except FileNotFoundError:
        return False


def probe(role, home, environ, runner=subprocess.run, present=present, which=shutil.which):
    """Require actual filesystem, process and service absence on one guest.

Dependency injection keeps tests independent of the test process environment
and service manager. A broken symlink is residue, not a clean installation.
Process listings use executable names, so this helper's own script path does
not match merely because it lives under a farhelm-brick-test evidence tree.
"""
    receipts = []
    path_rows = [{'path': str(path), 'present': present(path)} for path in paths(home, environ)]
    # An interrupted installer can leave inert staging beside an absent bundle.
    # It is still residue: enumerate names only, without reading their contents.
    for directory in (Path(home) / 'Applications', Path(home) / '.local/bin'):
        if present(directory):
            path_rows.extend({'path': str(path), 'present': True} for path in directory.iterdir()
                             if path.name.startswith(('.farhelm-app-build.', '.farhelm-update.', '.farhelm-install.', '.farhelm-link.')))
    executables = {name: which(name) for name in ('farhelm', 'farhelm-desktop', 'farhelm-supervisor')}
    processes = command(['/bin/ps', '-axo', 'comm='], runner)
    receipts.append(processes)
    names = [line.strip() for line in required(processes).splitlines()]
    # Linux comm is limited to 15 characters; the shipped remote runs as
    # farhelm, but retain the truncated legacy supervisor name as residue too.
    running = [name for name in names if Path(name).name in {*executables, 'farhelm-supervi'}]
    services = []
    if role == 'mac':
        row = command(['/bin/launchctl', 'list'], runner)
        receipts.append(row)
        services.extend(line for line in required(row).splitlines() if 'farhelm' in line.lower())
    elif role == 'linux':
        for scope in ('--user', '--system'):
            manager = command(['/usr/bin/systemctl', scope, 'show', '--property=Version', '--value'], runner)
            receipts.append(manager)
            if not required(manager).strip():
                raise ProbeError('systemd manager did not return its version')
            for action, extra in (('list-unit-files', []), ('list-units', ['--all', '--plain'])):
                row = command(['/usr/bin/systemctl', scope, action, '--no-pager', '--no-legend', *extra, 'farhelm*'], runner)
                receipts.append(row)
                services.extend(line for line in service_listing(row).splitlines() if line.strip())
    else:
        raise ProbeError('unknown guest role')
    return {'role': role, 'home': str(home), 'xdg': {key: environ.get(key) for key in ('XDG_STATE_HOME', 'XDG_CONFIG_HOME')},
            'clean': not any(row['present'] for row in path_rows)
            and not any(executables.values()) and not running and not services,
            'paths': path_rows, 'executables': executables, 'running': running,
            'services': services, 'commands': receipts}


def require_clean_pair(value):
    """Require both guests' concrete absence observations rather than a lone clean label.

This validates the deterministic helper's receipt at host import. It supplements
the operator's review of actual command events; a same-user model is not an
adversarial security boundary. Missing roles or contradictory dirty observations
cannot become a passing fixture checkpoint through its top-level boolean.
"""
    if (not isinstance(value, dict) or type(value.get('format')) is not int or value['format'] != 1 or value.get('clean') is not True
            or value.get('error') or not isinstance(value.get('observations'), list)
            or len(value['observations']) != 2):
        raise ProbeError('fixture receipt needs a clean Mac and Linux pair')
    roles = set()
    for row in value['observations']:
        if not isinstance(row, dict) or row.get('role') not in ('mac', 'linux') or row['role'] in roles:
            raise ProbeError('fixture receipt has missing or duplicate guest roles')
        roles.add(row['role'])
        if not Path(row.get('home', '')).is_absolute() or set(row.get('xdg', {})) != {'XDG_STATE_HOME', 'XDG_CONFIG_HOME'}:
            raise ProbeError('fixture receipt lacks product path inputs')
        expected_paths = {str(path) for path in paths(row['home'], row['xdg'])}
        if not expected_paths.issubset({item['path'] for item in row.get('paths', [])}):
            raise ProbeError('fixture receipt omits required product paths')
        if (row.get('clean') is not True or row.get('running') != [] or row.get('services') != []
                or set(row.get('executables', {})) != {'farhelm', 'farhelm-desktop', 'farhelm-supervisor'}
                or any(row['executables'].values()) or not row.get('paths')
                or any(path.get('present') is not False for path in row['paths'])):
            raise ProbeError('fixture receipt contains residue or missing observations')
        commands = row.get('commands', [])
        expected = [['/bin/ps', '-axo', 'comm=']]
        if row['role'] == 'mac':
            expected.append(['/bin/launchctl', 'list'])
        else:
            for scope in ('--user', '--system'):
                expected.extend([['/usr/bin/systemctl', scope, 'show', '--property=Version', '--value'],
                                 ['/usr/bin/systemctl', scope, 'list-unit-files', '--no-pager', '--no-legend', 'farhelm*'],
                                 ['/usr/bin/systemctl', scope, 'list-units', '--no-pager', '--no-legend', '--all', '--plain', 'farhelm*']])
        if [item.get('argv') for item in commands] != expected:
            raise ProbeError('fixture receipt has incomplete process/service observations')
        for command_row in commands:
            if command_row['argv'][0] == '/usr/bin/systemctl' and command_row['argv'][2] in ('list-units', 'list-unit-files'):
                if service_listing(command_row).strip():
                    raise ProbeError('fixture receipt contains installed or loaded services')
            else:
                required(command_row)
        if any(Path(line.strip()).name in {'farhelm', 'farhelm-desktop', 'farhelm-supervisor', 'farhelm-supervi'}
               for line in commands[0]['stdout'].splitlines()):
            raise ProbeError('fixture process receipt contradicts absence')
        if row['role'] == 'mac' and any('farhelm' in line.lower() for line in commands[1]['stdout'].splitlines()):
            raise ProbeError('fixture launch-agent receipt contradicts absence')
        if row['role'] == 'linux':
            managers = [item for item in commands if item['argv'][0] == '/usr/bin/systemctl' and item['argv'][2] == 'show']
            if len(managers) != 2 or any(not item['stdout'].strip() for item in managers):
                raise ProbeError('fixture receipt lacks positive manager readiness')
    return value


def merge_peer(remote):
    """Require a completed Linux observation without mistaking dirty state for SSH failure."""
    if len(remote.stdout) + len(remote.stderr) > 512 * 1024:
        raise ProbeError('peer fixture observation exceeded its output bound')
    if remote.returncode not in (0, 1) or remote.stderr.strip():
        raise ProbeError('pinned peer fixture observation failed')
    peer = json.loads(remote.stdout)
    if peer.get('format') != 1 or len(peer.get('observations', [])) != 1 or peer['observations'][0].get('role') != 'linux':
        raise ProbeError('pinned peer returned an unexpected fixture receipt')
    if peer.get('error') or type(peer.get('clean')) is not bool or remote.returncode != int(not peer['clean']):
        raise ProbeError('pinned peer cleanliness and completion disagree')
    return peer['observations']


def main():
    """Collect guest-local and pinned-peer observations into one exclusive private receipt."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--role', choices=('mac', 'linux'), required=True)
    parser.add_argument('--ssh-alias')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    expected = 'Darwin' if args.role == 'mac' else 'Linux'
    if platform.system() != expected:
        raise ProbeError('role does not match this guest platform')
    if args.role == 'mac':
        virtual = command(['/usr/sbin/sysctl', '-n', 'kern.hv_vmm_present'])
        if required(virtual).strip() != '1':
            raise ProbeError('Mac fixture inspection requires a virtualized guest')
    value = {'format': 1, 'observations': [], 'clean': False}
    try:
        value['observations'].append(probe(args.role, Path.home(), os.environ))
        if args.ssh_alias:
            if args.role != 'mac' or not re.fullmatch(r'fh-linux-[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}', args.ssh_alias):
                raise ProbeError('peer inspection requires the prepared run-specific Linux alias')
            remote = subprocess.run(['/usr/bin/ssh', '-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=yes',
                                     args.ssh_alias, '/usr/bin/python3', '-', '--role', 'linux'],
                                    input=Path(__file__).read_bytes(), capture_output=True, timeout=180)
            value['observations'].extend(merge_peer(remote))
        value['clean'] = all(row['clean'] for row in value['observations'])
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        value['error'] = str(error)
    encoded = (json.dumps(value, indent=2) + '\n').encode()
    if args.output:
        with args.output.open('xb') as stream:
            os.chmod(args.output, 0o600)
            stream.write(encoded)
    else:
        print(encoded.decode(), end='')
    return 0 if value['clean'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
