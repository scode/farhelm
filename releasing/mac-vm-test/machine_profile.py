"""Load private machine bindings without coupling Farhelm to a local workspace.

Discovery is conventional, while every resource path comes from the profile.
The profile holds identifiers and policy references, never credential material.
Dependency injection keeps tests independent of the running process environment.
"""

from dataclasses import dataclass
import json
import os
from pathlib import Path
import re

from control import Refused


PROFILE_ENV = 'FARHELM_BRICK_TEST_PROFILE'


def filesystem_id(path):
    """Identify the storage volume even before a configured directory exists.

An existing VM-storage directory may be a mount or symlink onto another volume;
measure its resolved filesystem rather than assuming Tart's home is that volume.
"""
    path = Path(path)
    while not path.exists():
        path = path.parent
    return path.stat().st_dev


def require_shared_storage(values, volume_id=filesystem_id):
    """Fence the single-volume budget before creation and again before cloning.

Evidence and actual VM storage must share a filesystem. This intentionally
refuses unsupported split-volume accounting instead of measuring the wrong disk.
The identity probe is injected for tests without mounts or environment mutation.
"""
    if volume_id(Path(values['tart_home']) / 'vms') != volume_id(Path(values['evidence_root'])):
        raise Refused('VM storage and evidence must share a filesystem for the configured disk budget')


def expand_path(value, home, relative_to):
    """Resolve profile paths against their owner, with an explicit home for tests.

Relative bindings belong to the configuration file, not the current checkout.
Only the current user's tilde syntax is supported; shell expansion is absent.
"""
    if not isinstance(value, str) or not value.strip() or '\x00' in value:
        raise Refused('profile paths must be nonempty strings')
    if value == '~' or value.startswith('~/'):
        path = home / value[2:] if value != '~' else home
    elif value.startswith('~') or '$' in value:
        raise Refused('use a plain path or ~/; shell variable expansion is unsupported')
    else:
        path = Path(value)
        if not path.is_absolute():
            path = relative_to / path
    return path.resolve()


def discover(explicit=None, environ=None, home=None):
    """Prefer an explicit profile, then the environment override, then convention.

A named but missing override must fail rather than silently selecting a different
machine. No discovery path depends on a Farhelm or Tart project checkout.
"""
    home = Path.home() if home is None else Path(home)
    environ = os.environ if environ is None else environ
    value = explicit if explicit is not None else environ.get(PROFILE_ENV)
    if value is None:
        return home / '.config/farhelm/brick-test.json'
    return expand_path(str(value), home, Path.cwd())


def base_binding(value):
    """Pin a stopped source by both Tart name and hardware identity.

The operator supplies these values privately. Path-like names cannot redirect
inspection outside Tart's VM directory; an actual replacement is refused later.
"""
    if not isinstance(value, dict) or set(value) != {'name', 'mac_address'}:
        raise Refused('each base requires exactly name and mac_address')
    name, mac = value['name'], value['mac_address']
    if not isinstance(name, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*', name):
        raise Refused('base name must be a plain local Tart name')
    if not isinstance(mac, str) or not re.fullmatch(r'(?:[0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}', mac):
        raise Refused('base hardware identity must be a MAC address')
    return {'name': name, 'mac_address': mac.lower()}


@dataclass(frozen=True)
class Profile:
    """Validated private bindings used by every operation in a recorded workflow.

The normalized snapshot is persisted with the run. Later commands must use the
same bindings rather than accidentally adopting an edited global profile.
"""

    source: Path
    values: dict

    @classmethod
    def load(cls, path, home=None, repo=None, volume_id=filesystem_id):
        """Reject incomplete, public or ambiguous configuration before VM mutation.

Strict fields catch typo-driven defaults and discourage putting tokens into the
profile. Policy and journal remain external files; their contents are not copied
into the public checkout or interpreted as additional script capabilities.
"""
        home = Path.home() if home is None else Path(home)
        path = Path(path).resolve()
        if not path.is_file():
            raise Refused(f'brick-test profile missing: {path}; configure this machine first')
        if path.stat().st_size > 65536 or path.stat().st_mode & 0o077:
            raise Refused('profile must be <=64 KiB and private (mode 0600)')
        data = json.loads(path.read_text())
        fields = {'format', 'mac_base', 'linux_base', 'linux_user', 'tart_home',
                  'journal', 'evidence_root', 'operator_policy', 'resource_limits',
                  'route_via_gateway', 'agents'}
        if not isinstance(data, dict) or set(data) != fields or type(data['format']) is not int or data['format'] != 1:
            raise Refused('unsupported or incomplete brick-test profile format')
        values = dict(data)
        for role in ('mac', 'linux'):
            values[role + '_base'] = base_binding(data[role + '_base'])
        if values['mac_base']['name'] == values['linux_base']['name']:
            raise Refused('macOS and Linux bases must be different VMs')
        if not isinstance(data['linux_user'], str) or not re.fullmatch(r'[a-z_][a-z0-9_-]*[$]?', data['linux_user']):
            raise Refused('Linux username must be a plain account name')
        if data['agents'] != ['codex'] or not isinstance(data['route_via_gateway'], bool):
            raise Refused('this workflow supports Codex only and an explicit boolean route choice')
        limits = data['resource_limits']
        if not isinstance(limits, dict) or set(limits) != {'minimum_free_gib', 'maximum_run_growth_gib'}:
            raise Refused('resource_limits requires free-space floor and run-growth limit')
        if any(type(v) not in (int, float) or not 0 < v < 100000 for v in limits.values()):
            raise Refused('resource limits must be positive finite GiB values')
        for key in ('tart_home', 'journal', 'evidence_root', 'operator_policy'):
            values[key] = str(expand_path(data[key], home, path.parent))
        if not Path(values['operator_policy']).is_file():
            raise Refused('operator policy file is missing')
        if not Path(values['journal']).parent.is_dir():
            raise Refused('journal parent must already exist')
        if Path(values['journal']).resolve() == Path(values['operator_policy']).resolve():
            raise Refused('journal and policy must be separate files')
        require_shared_storage(values, volume_id)
        if repo is not None:
            repo = Path(repo).resolve()
            for private in (path, Path(values['tart_home']), Path(values['journal']),
                            Path(values['evidence_root']), Path(values['operator_policy'])):
                if private.is_relative_to(repo):
                    raise Refused('profile, VM storage, policy, journal and evidence must stay outside the public checkout')
        return cls(path, values)

    def write(self, destination):
        """Install a normalized private profile without overwriting local configuration.

An identical existing profile is an idempotent setup result. A different one is
an explicit operator decision, so this helper refuses to replace it silently.
"""
        destination = Path(destination)
        content = json.dumps(self.values, indent=2) + '\n'
        if destination.exists():
            if destination.stat().st_mode & 0o077 or json.loads(destination.read_text()) != self.values:
                raise Refused('destination profile differs or is not private; inspect before replacing it')
            return False
        destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        # Exclusive creation fences a competing writer between inspection and
        # opening. Opening with the final mode avoids a temporary readable file.
        fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, 'w') as stream:
            stream.write(content)
            stream.flush()
            os.fsync(stream.fileno())
        return True
