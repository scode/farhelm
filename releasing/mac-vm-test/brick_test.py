#!/usr/bin/env python3
"""Coordinate an agent-driven release test through private, resumable run state.

The host agent performs the recipe's genuine GUI checks. This driver supplies
the machine bindings, release plan, owned guest pair and evidence gates; it
never invents GUI results or promotes a release. A separate fixture smoke path
exercises command wiring without claiming any release acceptance checks passed.
"""

import argparse
import contextlib
import fcntl
import hashlib
import json
from pathlib import Path
import re
import shlex
import shutil
import struct
import subprocess
import sys
import tarfile
import time
from urllib.request import urlopen
import uuid

from control import Control, Refused, atomic_json
from continuity import require_progress
from machine_profile import Profile, discover, filesystem_id, require_shared_storage
from network import connect
from verify_release import require_comment


REPO = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
ORIGIN = 'https://get.farhelm.io'


# Release discovery is read-only; the old source describes the update mechanism.
# Public metadata is bounded here, while verification and installation run in guests.
def version(tag):
    """Order the repository's stable, RC and dev tags by SemVer precedence.

The limited release grammar prevents tags from becoming arbitrary Git options
or URL paths. Numeric components have no ambiguous leading-zero spelling.
"""
    number = r'(0|[1-9][0-9]*)'
    match = re.fullmatch(r'v' + number + r'\.' + number + r'\.' + number
                         + r'(?:-(rc|dev)\.' + number + r')?', tag)
    if not match:
        raise Refused('use an exact stable, rc.N or dev.N release tag')
    major, minor, patch, kind, sequence = match.groups()
    return (int(major), int(minor), int(patch), int(kind is None), kind or '', int(sequence or 0))


class Source:
    """Read immutable release-tag source; the installed old tag is the authority."""

    def __init__(self, directory=REPO):
        self.directory = Path(directory)

    def git(self, *args):
        """Bound source lookup and diagnose a missing tag before creating guests."""
        try:
            return subprocess.check_output(['git', '-C', str(self.directory), *args],
                                           text=True, stderr=subprocess.PIPE, timeout=15)
        except subprocess.SubprocessError as error:
            raise Refused('release source lookup failed; fetch the release tags and inspect the checkout') from error

    def tags(self):
        """Offer source-known tags; published inputs are checked separately."""
        return self.git('tag', '--list', 'v*').splitlines()

    def commit(self, tag):
        """Peel the validated tag to the commit whose source describes its release."""
        version(tag)
        return self.git('rev-parse', '--verify', tag + '^{commit}').strip()

    def override(self, tag):
        """Select the recipe branch from old source, not the candidate's support.

This is source support only. Native launch forwarding and override removal still
require the recipe's actual startup-log and GUI observations.
"""
        version(tag)
        return 'FARHELM_DESKTOP_UPDATE_LATEST' in self.git(
            'show', tag + ':crates/farhelm-ui/src/desktop/updater.rs')


def download(url):
    """Read bounded public metadata; archives and installation belong in guests."""
    with urlopen(url, timeout=30) as response:
        value = response.read(1024 * 1024 + 1)
    if len(value) > 1024 * 1024:
        raise Refused('release metadata exceeds 1 MiB')
    return value


def update_path(candidate, latest, supports_override):
    """Choose from the observed public default and the actual old app's capability."""
    target, promoted = version(candidate), version(latest)
    if promoted[3] != 1 or target < promoted:
        raise Refused('public default changed beyond this candidate; inspect and replan')
    return ('ordinary' if latest == candidate else
            'override' if target[3] == 1 and supports_override else 'manual')


def plan(candidate, latest, source):
    """Cover every source-known eligible stable through the promoted stable.

A standard gate cannot silently test a downgrade or choose the candidate as its
own previous release. The current stable pair leads, followed by older pairs.
The filled release addendum must identify any historical availability gap.
"""
    target = version(candidate)
    promoted = version(latest)
    if promoted[3] != 1 or target < promoted:
        raise Refused('latest must be stable and candidate must not precede it')
    eligible = []
    for tag in source.tags():
        try:
            parsed = version(tag)
        except Refused:
            continue
        if parsed[3] == 1 and version('v0.23.0') <= parsed < target and parsed <= promoted:
            eligible.append(tag)
    eligible = sorted(set(eligible), key=version, reverse=True)
    if not eligible:
        raise Refused('no eligible previous stable from v0.23.0 onward is available in source')
    if latest != candidate and latest not in eligible:
        raise Refused('the promoted stable is missing from the eligible source tags')
    passes = [{'id': 'fresh', 'previous': eligible[0], 'update_path': None}]
    for previous in eligible:
        supported = source.override(previous)
        path = update_path(candidate, latest, supported)
        passes.append({'id': 'upgrade-' + previous, 'previous': previous,
                       'previous_commit': source.commit(previous), 'old_supports_override': supported,
                       'update_path': path})
    return {'candidate': candidate, 'candidate_commit': source.commit(candidate),
            'latest_at_plan': latest, 'passes': passes}


def metadata(candidate, fetch=download):
    """Bind availability to exact candidate inputs without claiming trust verification.

The signature's comment and the installer hash are checked here, but only the
old key ring's actual minisign verification can satisfy that required checkpoint.
Signed archive availability/provisioning is proved by the genuine guest flow.
"""
    version(candidate)
    inputs = {name: fetch(f'{ORIGIN}/{candidate}/{name}') for name in
              ('SHA256SUMS', 'SHA256SUMS.minisig', 'install.sh')}
    require_comment(inputs['SHA256SUMS.minisig'].decode(), candidate)
    entries = [line.split() for line in inputs['SHA256SUMS'].decode().splitlines()]
    installers = [row for row in entries if len(row) == 2 and row[1] == 'install.sh']
    checksum = hashlib.sha256(inputs['install.sh']).hexdigest()
    if len(installers) != 1 or installers[0][0].lower() != checksum:
        raise Refused('candidate installer does not match the advertised checksum')
    return inputs, {'sha256sums_sha256': hashlib.sha256(inputs['SHA256SUMS']).hexdigest(),
                    'installer_sha256': checksum, 'signature_verification': 'pending'}


# Evidence gates establish what an observation must retain. They can validate
# linkage, identity and formats; interpreting actual GUI behavior stays with the agent.
def checks(pass_id, smoke=False):
    """Declare acceptance boundaries before evidence is recorded.

GUI checks require actual image evidence and explicit operator observations.
Continuity checkpoints require independent before/after receipts as well.
"""
    if smoke:
        return {'commands': {'gui': False, 'continuity': False}}
    items = ({'artifact-verification': False, 'fixture-clean': False, 'install': False,
              'app-start': True, 'remote-provision': True, 'codex-conversation': True,
              'remote-terminal': True, 'addendum': False} if pass_id == 'fresh' else
             {'artifact-verification': False, 'old-fixture': True, 'seed-state': True,
              'continuity-before': False, 'update-relaunch': True, 'preserved-state': True,
              'codex-resume': True, 'remote-update': True, 'reopen': True, 'addendum': False})
    hybrid = {'remote-provision', 'codex-conversation', 'remote-terminal', 'old-fixture', 'seed-state',
              'update-relaunch', 'preserved-state', 'codex-resume', 'remote-update', 'reopen'}
    return {key: {'gui': gui, 'command': key in hybrid,
                  'continuity': key in ('remote-update', 'reopen', 'continuity-before')}
            for key, gui in items.items()}


def artifact(root, reference):
    """Read a bounded private evidence file or tar member without extracting it.

References are relative to this run. Symlinks cannot escape it, and the member
must be a regular file. This validates linkage/format, not image semantics.
"""
    if not isinstance(reference, str) or reference.startswith('/'):
        raise Refused('evidence references must be relative to this run')
    filename, separator, member = reference.partition('::')
    path = (root / filename).resolve()
    if not path.is_relative_to(root.resolve()) or not path.is_file() or path.stat().st_size > 128 * 1024 * 1024:
        raise Refused('evidence is missing, outside the run, or over its size bound')
    if separator:
        with tarfile.open(path) as archive:
            info = archive.getmember(member)
            if not info.isfile() or info.size > 16 * 1024 * 1024:
                raise Refused('evidence member must be a regular file <=16 MiB')
            return archive.extractfile(info).read()
    if path.stat().st_size > 16 * 1024 * 1024:
        raise Refused('direct evidence file exceeds 16 MiB')
    return path.read_bytes()


def is_image(value):
    """Recognize a nonempty PNG frame; interpreting the frame remains agent work."""
    return (len(value) >= 33 and value.startswith(b'\x89PNG\r\n\x1a\n')
            and value[12:16] == b'IHDR' and all(struct.unpack('>II', value[16:24])))


def progress(root, before, after):
    """Reject malformed receipts as well as replayed or replaced process observations."""
    values = [json.loads(artifact(root, ref)) for ref in (before, after)]
    for value in values:
        if not isinstance(value, dict) or any(type(value.get(k)) is not int or value[k] < 0
                                             for k in ('pid', 'start_ticks', 'counter')) or value['pid'] == 0:
            raise Refused('continuity receipts need actual numeric process/counter identities')
        uuid.UUID(value['boot_id'])
    require_progress(*values)
    return values


def continuity_chain(root, entry, check_id, before=None, after=None):
    """Bind each ordered milestone to the same original, advancing Linux process.

Each pair must be live and advancing, but pairwise checks alone could accept a
replacement or replay old evidence at the next milestone. Later before samples
must be at least the preceding after sample, and later after samples must advance.
Recheck the same relation when reporting retained results, not only at recording.
"""
    order = ('continuity-before', 'remote-update', 'reopen')
    index = order.index(check_id)
    if before is None or after is None:
        references = entry['results'][check_id]['continuity']
        before, after = references['before'], references['after']
    current_before, current_after = progress(root, before, after)
    if index:
        predecessor = entry['results'].get(order[index - 1])
        if not predecessor or predecessor['status'] != 'pass':
            raise Refused('record the preceding continuity milestone as pass first')
        previous = predecessor['continuity']
        continuity_chain(root, entry, order[index - 1])
        _, previous_after = progress(root, previous['before'], previous['after'])
        if any(current_before[key] != previous_after[key] for key in ('pid', 'start_ticks', 'boot_id')):
            raise Refused('continuity milestone changed the original process identity')
        if current_before['counter'] < previous_after['counter']:
            raise Refused('continuity milestone replays an earlier observation')
        require_progress(previous_after, current_after)


# Persisted workflow state outlives the process supervising Tart. That separation
# lets a later invocation inspect an interruption without guessing clone ownership.
class Run:
    """Persist a workflow independently of the foreground guest supervisor.

Every modifying invocation reloads state under a lock. Profile edits cannot
redirect an existing run's cleanup, and incomplete preparation is retained for
reconciliation rather than guessed safe to retry.
"""

    def __init__(self, directory, profile, factory=Control, disk_usage=shutil.disk_usage, connector=connect,
                 volume_id=filesystem_id):
        self.directory = Path(directory).resolve()
        if not self.directory.is_relative_to(Path(profile.values['evidence_root'])):
            raise Refused('workflow must stay under the configured private evidence root')
        self.profile = profile
        self.factory = factory
        self.disk_usage = disk_usage
        self.connector = connector
        self.volume_id = volume_id
        self.path = self.directory / 'workflow.json'
        self.data = None

    def control(self, directory=None):
        """Use one profile's Tart home and external journal for all owned passes."""
        v = self.profile.values
        return self.factory(directory or self.directory, Path(v['journal']), Path(v['tart_home']))

    def note(self, kind, detail, event=None):
        """Persist decisions or a matched intent/result without logging raw evidence.

Return the event identity so an attempted transition can close the same event
on success or failure. An unmatched intent then has its ordinary forensic meaning.
"""
        c = self.control()
        c.data = {'run_id': self.data['run_id']}
        event = event or str(uuid.uuid4())
        c.record(kind, event, 'brick-test workflow', detail)
        return event

    def budget(self):
        """Check the volume floor and observed growth before starting another pair.

VM storage and evidence must share a filesystem, checked again in case a mount
changed. Measure the actual vms directory's volume. Other host activity can
consume space too, so growth is conservative, and no continuous quota is claimed.
"""
        require_shared_storage(self.profile.values, self.volume_id)
        path = Path(self.profile.values['tart_home']) / 'vms'
        while not path.exists():
            path = path.parent
        free = self.disk_usage(path).free
        limits = self.profile.values['resource_limits']
        if free < limits['minimum_free_gib'] * 1024 ** 3:
            raise Refused('volume free space is below the configured floor')
        initial = self.data.get('initial_free_bytes', free) if self.data else free
        if initial - free > limits['maximum_run_growth_gib'] * 1024 ** 3:
            raise Refused('observed run disk growth exceeds the configured budget')
        return free

    def create(self, release_plan=None, inputs=None, smoke=False):
        """Mint private state and requirements before any clone can be claimed."""
        self.data = {'format': 1, 'run_id': str(uuid.uuid4()), 'kind': 'fixture-smoke' if smoke else 'release',
                     'profile': self.profile.values, 'plan': release_plan, 'passes': {}}
        self.data['initial_free_bytes'] = self.budget()
        specifications = [{'id': 'smoke', 'previous': None, 'update_path': None}] if smoke else release_plan['passes']
        for spec in specifications:
            self.data['passes'][spec['id']] = dict(spec, phase='pending',
                                                   required=checks(spec['id'], smoke), results={})
        event = self.note('intent', f'Create private workflow directory `{self.directory}`; GUI results are never inferred.')
        try:
            self.directory.mkdir(mode=0o700, parents=False)
            atomic_json(self.path, self.data)
            if inputs:
                directory = self.directory / 'candidate-inputs'
                directory.mkdir(mode=0o700)
                for name, value in inputs.items():
                    target = directory / name
                    target.write_bytes(value)
                    target.chmod(0o600)
        except BaseException:
            self.note('result', 'Workflow creation failed; inspect partial state before retry.', event)
            raise
        self.note('result', 'Workflow state created; no VM has been started.', event)

    @contextlib.contextmanager
    def locked(self):
        """Keep transitions atomic and bind resumed operations to the original profile."""
        if not self.path.is_file():
            raise Refused('workflow state is missing')
        with (self.directory / '.workflow.lock').open('a') as stream:
            fcntl.flock(stream, fcntl.LOCK_EX)
            self.data = json.loads(self.path.read_text())
            if self.data.get('format') != 1 or self.data['profile'] != self.profile.values:
                raise Refused('workflow/profile mismatch; inspect the original bindings')
            uuid.UUID(self.data['run_id'])
            yield

    def save(self):
        """Leave the last complete state intact if writing an update fails."""
        atomic_json(self.path, self.data)

    def selected(self, pass_id):
        """Reject arbitrary path-like pass names before resolving a guest manifest."""
        if pass_id not in self.data['passes']:
            raise Refused('unknown workflow pass')
        return self.data['passes'][pass_id]

    def pair(self, pass_id):
        """Open only the manifest belonging to a planned, already prepared pass."""
        entry = self.selected(pass_id)
        if entry['phase'] not in ('ready', 'stopped', 'failed'):
            raise Refused('pass is not prepared; interrupted preparation needs reconciliation')
        return self.control(self.directory / pass_id)

    def prepare(self, pass_id, processes, headless=False):
        """Verify both stopped bases before creating a single owned working pair.

The caller keeps the returned Tart children supervised. Preparation failure is
recorded with partial state; neither cleanup authority nor a retry is invented.
"""
        entry = self.selected(pass_id)
        if entry['phase'] != 'pending':
            raise Refused('pass already attempted; inspect its state instead of replaying preparation')
        if any(v['phase'] in ('preparing', 'ready', 'failed') for v in self.data['passes'].values()):
            raise Refused('finish or reconcile the existing working pair first')
        self.budget()
        inspector = self.control()
        for role in ('mac', 'linux'):
            base = self.profile.values[role + '_base']
            if inspector.identity(base['name']).lower() != base['mac_address']:
                raise Refused('prepared base hardware identity changed')
            value = json.loads(inspector.command('get', base['name'], '--format', 'json', capture_output=True).stdout)
            if value['State'] != 'stopped':
                raise Refused('prepared base must remain stopped')
        entry['phase'] = 'preparing'
        self.save()
        c = self.control(self.directory / pass_id)
        try:
            c.directory.mkdir(mode=0o700)
            c.data = {'format': 1, 'run_id': str(uuid.uuid4()), 'vms': {}}
            c.save()
            with c.locked():
                for role in ('mac', 'linux'):
                    c.clone(role, self.profile.values[role + '_base']['name'])
                for role in ('mac', 'linux'):
                    c.start(role, headless=headless or role == 'linux')
                    c.ready(role, 120)
                account = c.command('exec', c.owned('linux'), '/usr/bin/id', '-un', capture_output=True).stdout.decode().strip()
                if account != self.profile.values['linux_user']:
                    raise Refused('Linux guest-execution account differs from configured SSH account')
                self.connector(c, c.owned('mac'), c.owned('linux'), self.profile.values['linux_user'],
                               self.profile.values['route_via_gateway'])
                stage = ('from pathlib import Path; import sys; '
                         'd=Path.home()/"farhelm-brick-test"; d.mkdir(mode=0o700,exist_ok=True); '
                         'p=d/"continuity.py"; p.write_bytes(sys.stdin.buffer.read()); p.chmod(0o600)')
                c.mutate(['exec', '-i', c.owned('linux'), '/usr/bin/python3', '-c', stage],
                         'Stage the portable continuity workload only inside the owned Linux guest.',
                         input=(HERE / 'continuity.py').read_bytes())
            entry['phase'] = 'ready'
            self.save()
            self.note('decision', f'Pass `{pass_id}` has command readiness and verified guest SSH; GUI checks remain required.')
        except BaseException as error:
            entry['phase'] = 'failed'
            entry['preparation_error'] = type(error).__name__
            self.save()
            self.note('decision', f'Preparation failed for `{pass_id}`; preserve partial manifests and reconcile before retry.')
            raise
        finally:
            # start() retains handles at spawn, including failed startup probes
            # and writes. Transfer them before the foreground caller unwinds.
            processes.extend(c.processes)

    def record(self, pass_id, check_id, status, references, detail, before=None, after=None):
        """Accept explicit observations only when required evidence is present.

Image presence does not interpret its contents: the agent's detail is the GUI
attestation. Verification and continuity receipts receive additional structural
checks, and later report generation revalidates retained artifact references.
"""
        entry = self.selected(pass_id)
        if entry['phase'] != 'ready' or check_id not in entry['required']:
            raise Refused('record results only for a prepared pass and declared check')
        if check_id in entry['results']:
            raise Refused('a result already exists; preserve it rather than overwriting history')
        if status not in ('pass', 'fail', 'could-not-run') or not detail.strip():
            raise Refused('provide a supported result and an actual observation')
        if len(detail) > 4000 or len(references) > 20:
            raise Refused('observation or evidence list exceeds its bound')
        contents = [artifact(self.directory, ref) for ref in references]
        spec = entry['required'][check_id]
        if status == 'pass':
            if not any(contents) or (spec['gui'] and not any(is_image(v) for v in contents)):
                raise Refused('a passing check requires evidence, including an actual PNG for GUI checks')
            if spec.get('command') and not any(v and not is_image(v) for v in contents):
                raise Refused('this check also requires retained command/identity evidence')
            if check_id == 'update-relaunch' and 'latest_at_selection' not in entry:
                raise Refused('select the update path from a fresh latest observation first')
            if spec['continuity']:
                if before is None or after is None:
                    raise Refused('continuity requires independent before/after receipts')
                continuity_chain(self.directory, entry, check_id, before, after)
            if check_id == 'artifact-verification':
                target = self.data['plan']['candidate']
                values = []
                for raw in contents:
                    try:
                        values.append(json.loads(raw))
                    except (ValueError, UnicodeError):
                        continue
                valid = any(isinstance(v, dict) and v.get('version') == target
                            and v.get('previous_trust_ring') == entry['previous']
                            and all(v.get(k) == 'verified' for k in
                                    ('signature', 'trusted_comment', 'installer_checksum'))
                            and v.get('sha256sums_sha256') == self.data['plan']['sha256sums_sha256']
                            for v in values)
                if not valid:
                    raise Refused('old-ring verification receipt does not match this pass and candidate')
        value = {'status': status, 'evidence': references, 'detail': detail}
        if before is not None or after is not None:
            value['continuity'] = {'before': before, 'after': after}
        # Bind each accepted observation to retained bytes, so a later accidental
        # overwrite cannot inherit the old attestation just by keeping its name.
        bound = set(references) | {ref for ref in (before, after) if ref is not None}
        value['sha256'] = {ref: hashlib.sha256(artifact(self.directory, ref)).hexdigest() for ref in bound}
        entry['results'][check_id] = value
        self.save()
        self.note('decision', f'Pass `{pass_id}`, check `{check_id}` recorded `{status}`; private evidence retained.')

    def select_update(self, pass_id, latest):
        """Reobserve /latest at the action boundary instead of trusting initial planning.

If promotion happened while state was seeded, the ordinary path is selected and
reported as after-publication coverage. A default beyond the candidate refuses
continuation. No endpoint is modified by this operation.
"""
        entry = self.selected(pass_id)
        if entry['phase'] != 'ready' or not pass_id.startswith('upgrade-'):
            raise Refused('select an update only for a prepared upgrade pass')
        if 'latest_at_selection' in entry:
            raise Refused('update selection already recorded; inspect the original observation')
        selected = update_path(self.data['plan']['candidate'], latest, entry['old_supports_override'])
        receipt = self.directory / pass_id / 'latest-at-selection.json'
        atomic_json(receipt, {'latest': latest, 'update_path': selected})
        entry.update(latest_at_selection=latest, update_path=selected)
        self.save()
        self.note('decision', f'Pass `{pass_id}` selected `{selected}` from observed public default `{latest}`.')

    def add_check(self, pass_id, check_id, gui=False):
        """Make release-specific addendum checks mandatory without replacing baseline checks."""
        entry = self.selected(pass_id)
        if entry['phase'] not in ('pending', 'ready'):
            raise Refused('add obligations before completing or failing a pass')
        if not re.fullmatch(r'extra-[a-z0-9-]+', check_id) or check_id in entry['required']:
            raise Refused('use a new extra-... check identifier')
        entry['required'][check_id] = {'gui': gui, 'command': False, 'continuity': False}
        self.save()
        self.note('decision', f'Added required addendum check `{check_id}` to `{pass_id}`.')

    def verdict(self, entry):
        """Missing, invalidated or skipped checks cannot become a release pass."""
        results = entry['results']
        if any(v['status'] == 'fail' for v in results.values()):
            return 'fail'
        if entry.get('preparation_error') or set(results) != set(entry['required']):
            return 'could not run'
        for check_id, result in results.items():
            if result['status'] != 'pass':
                return 'could not run'
            try:
                for ref, digest in result['sha256'].items():
                    if hashlib.sha256(artifact(self.directory, ref)).hexdigest() != digest:
                        return 'could not run'
                if 'continuity' in result:
                    continuity_chain(self.directory, entry, check_id)
            except (OSError, ValueError, RuntimeError, KeyError, tarfile.TarError):
                return 'could not run'
        return 'pass'

    def report(self):
        """Return private progress and handles, explicitly separating smoke from acceptance."""
        output = {'kind': self.data['kind'], 'run_dir': str(self.directory),
                  'operator_policy': self.profile.values['operator_policy'], 'plan': self.data['plan'], 'passes': {}}
        for pass_id, entry in self.data['passes'].items():
            value = dict(entry, verdict=self.verdict(entry))
            if entry['phase'] in ('ready', 'stopped', 'failed'):
                c = self.control(self.directory / pass_id)
                if c.manifest_path.is_file():
                    manifest = json.loads(c.manifest_path.read_text())
                    value['guests'] = manifest.get('vms', {})
                    value['ssh'] = manifest.get('ssh')
                    value['control_prefix'] = shlex.join([
                        sys.executable, '-B', str(HERE / 'control.py'), '--run-dir', str(c.directory),
                        '--journal', self.profile.values['journal'], '--tart-home', self.profile.values['tart_home']])
            output['passes'][pass_id] = value
        verdicts = [v['verdict'] for v in output['passes'].values()]
        outcome = 'fail' if 'fail' in verdicts else 'pass' if all(v == 'pass' for v in verdicts) else 'could not run'
        output['verdict'] = outcome if self.data['kind'] == 'release' else 'fixture smoke: ' + outcome
        if self.data['kind'] == 'release':
            candidate = self.data['plan']['candidate']
            observations = [self.data['plan']['latest_at_plan']] + [v.get('latest_at_selection')
                            for key, v in self.data['passes'].items() if key.startswith('upgrade-')]
            output['promotion_coverage'] = ('after-publication' if candidate in observations else
                                            'pending' if None in observations else 'before-public-default')
        return output

    def cleanup(self, pass_id, discard_failed=False):
        """Delete only a tested successful pair, or an explicitly approved failed pair.

Evidence is rechecked first. Clean guest shutdown remains mandatory, and each
role's low-level manifest must prove unchanged ownership and observed absence.
"""
        entry = self.selected(pass_id)
        if self.verdict(entry) != 'pass' and not discard_failed:
            raise Refused('pass is incomplete or failed; ask about retention before --discard-failed')
        c = self.pair(pass_id)
        with c.locked():
            for role in ('mac', 'linux'):
                if c.data['vms'].get(role, {}).get('phase') == 'deleted':
                    continue
                if c.state(role)['State'] != 'stopped':
                    c.shutdown(role)
                c.delete(role)
        entry['phase'] = 'deleted'
        self.save()
        self.note('decision', f'Pass `{pass_id}` working clones deleted after checked evidence; bases remain unchanged.')


def supervise(processes):
    """Keep the foreground owner alive until every started Tart child ends.

The interval polls actual owned children, not process-name matches. No failure
path force-kills guests; another agent invocation can inspect and cleanly stop
the recorded pair while this caller keeps its supervision session open.
"""
    while any(p.poll() is None for p in processes):
        time.sleep(0.5)
    if any(p.returncode != 0 for p in processes):
        raise Refused('a Tart child exited unsuccessfully; inspect guest state and retained evidence')


def smoke(run):
    """Exercise real profile-to-guest command wiring without any product/GUI verdict.

Both guests write and export a small marker, and Mac-to-Linux SSH readiness was
already independently proved during preparation. Cleanup uses the same gates
as release runs; a failing smoke is retained for inspection.
"""
    processes = []
    try:
        with run.locked():
            run.prepare('smoke', processes, headless=True)
            c = run.pair('smoke')
            references = []
            with c.locked():
                for role in ('mac', 'linux'):
                    c.mutate(['exec', c.owned(role), '/bin/sh', '-c',
                              'umask 077; mkdir -p "$HOME/farhelm-brick-test-evidence" && '
                              'printf "fixture-smoke-ready\\n" > "$HOME/farhelm-brick-test-evidence/commands.txt"'],
                             'Write a fixture-only command marker inside the owned guest.')
                    directory = c.command('exec', c.owned(role), '/bin/sh', '-c',
                                          'printf %s "$HOME/farhelm-brick-test-evidence"', capture_output=True).stdout.decode()
                    c.collect(role, directory, role + '-smoke')
                    ref = 'smoke/' + role + '-smoke.tar::./commands.txt'
                    if artifact(run.directory, ref).strip() != b'fixture-smoke-ready':
                        raise Refused('exported guest marker does not match')
                    references.append(ref)
            run.record('smoke', 'commands', 'pass', references,
                       'Both guests ready; pinned guest SSH and exported command markers verified. No product or GUI checks ran.')
            run.cleanup('smoke')
            print(json.dumps(run.report(), indent=2), flush=True)
    except BaseException as error:
        print(f'Fixture smoke failed: {error}; inspect {run.directory}. Started guests remain supervised.',
              file=sys.stderr, flush=True)
        raise
    finally:
        supervise(processes)


def main():
    """Expose bounded transitions while retaining one foreground owner for guest runs."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', type=Path)
    sub = parser.add_subparsers(dest='action', required=True)
    for name in ('plan', 'init'):
        sub.add_parser(name).add_argument('candidate')
    sub.add_parser('smoke')
    for name in ('serve', 'status', 'select-update', 'record', 'add-check', 'cleanup'):
        child = sub.add_parser(name)
        child.add_argument('run_dir', type=Path)
        if name != 'status':
            child.add_argument('pass_id')
        if name == 'serve':
            child.add_argument('--headless', action='store_true')
        elif name == 'record':
            child.add_argument('check_id')
            child.add_argument('--status', required=True, choices=('pass', 'fail', 'could-not-run'))
            child.add_argument('--evidence', action='append', default=[])
            child.add_argument('--detail', required=True)
            child.add_argument('--before')
            child.add_argument('--after')
        elif name == 'add-check':
            child.add_argument('check_id')
            child.add_argument('--gui', action='store_true')
        elif name == 'cleanup':
            child.add_argument('--discard-failed', action='store_true')
    args = parser.parse_args()
    profile = Profile.load(discover(args.profile), repo=REPO)
    if args.action in ('plan', 'init'):
        latest = download(ORIGIN + '/latest').decode().strip()
        release = plan(args.candidate, latest, Source())
        inputs, info = metadata(args.candidate)
        release.update(info)
        release['operator_policy'] = profile.values['operator_policy']
        if args.action == 'plan':
            print(json.dumps(release, indent=2))
            return
    if args.action in ('init', 'smoke'):
        root = Path(profile.values['evidence_root'])
        root.mkdir(mode=0o700, parents=True, exist_ok=True)
        run = Run(root / ('brick-' + str(uuid.uuid4())), profile)
        run.create(release if args.action == 'init' else None,
                   inputs if args.action == 'init' else None, smoke=args.action == 'smoke')
        if args.action == 'smoke':
            smoke(run)
        else:
            with run.locked():
                print(json.dumps(run.report(), indent=2))
        return
    run = Run(args.run_dir, profile)
    if args.action == 'serve':
        processes = []
        try:
            with run.locked():
                run.prepare(args.pass_id, processes, args.headless)
                print(json.dumps(run.report(), indent=2), flush=True)
        except BaseException as error:
            print(f'Preparation failed: {error}; inspect {run.directory}. Started guests remain supervised.',
                  file=sys.stderr, flush=True)
            raise
        finally:
            supervise(processes)
        return
    with run.locked():
        if args.action == 'record':
            run.record(args.pass_id, args.check_id, args.status, args.evidence,
                       args.detail, args.before, args.after)
        elif args.action == 'add-check':
            run.add_check(args.pass_id, args.check_id, args.gui)
        elif args.action == 'cleanup':
            run.cleanup(args.pass_id, args.discard_failed)
        elif args.action == 'select-update':
            run.select_update(args.pass_id, download(ORIGIN + '/latest').decode().strip())
        report = run.report()
        if args.action == 'status':
            atomic_json(run.directory / 'report.json', report)
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, KeyError, tarfile.TarError, subprocess.SubprocessError) as error:
        print(f'brick test refused/failed: {error}', file=sys.stderr)
        sys.exit(1)
