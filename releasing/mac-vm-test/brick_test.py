#!/usr/bin/env python3
"""Coordinate an agent-driven release test through private, resumable run state.

An independent Codex tester inside the Mac guest performs genuine GUI checks.
The host owns setup, lifecycle, credentials and evidence collection. This driver supplies
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
from fixture_probe import require_clean_pair
from verify_release import require_comment
from guest_tester import execution_calls, native_events, native_image_digests, request as tester_request, RESULT_STATES
from guest_workflow import (copy_credentials, execute as execute_guest, cancel as cancel_guest, require_setup,
                            source_url, task_for, Unavailable, CheckStopped, CleanupPending)
from checkpoint_contract import (REMOTE_MAC_PASS, prerequisite_checks, reopen_quit_call, reopen_transition_ref,
                                 require_reopen_transition, turn_timeout)


REPO = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
ORIGIN = 'https://get.farhelm.io'
# Keep this standalone coordinator's baseline equal to farhelm-proto's floor.
# test_upgrade_floor_agrees_with_the_installer_and_helm catches a one-sided raise.
UPGRADE_FLOOR = 'v0.23.0'
PROVIDER_BACKOFF = (60, 120, 240)
# Bonus upgrade pairs start from at most this many of the newest eligible
# stables, whatever the upgrade floor still admits. The maintainer set this cap;
# each older source would cost another whole pair of guests.
HISTORICAL_UPGRADE_LIMIT = 3

# Priority tiers, most important first. P0 asks whether an install or upgrade
# bricks: installing, upgrading through the installer and the in-app updater,
# both supervisors upgrading, and a new agent working on each. P1 asks whether
# the user keeps what they had: old sessions surviving and resuming, and the
# remote host's Remove/re-add and Uninstall. P2 covers feedback and signed announcements.
TIERS = ('P0', 'P1', 'P2')
# What each requested scope runs. Only `full` also runs bonus fixtures.
SCOPE_TIERS = {'p0-only': ('P0',), 'p1-only': ('P0', 'P1'), 'p2-only': TIERS, 'full': TIERS}
# Plans carry this marker from the P0/P1/P2 layout on. Runs planned earlier
# (P0 included old sessions and the remote menu, P1 was feedback) keep their
# saved reports, but this coordinator refuses to continue them: their
# checkpoints and fences follow the earlier contract.
PRIORITY_LAYOUT = 2


def rate_limited_without_actions(execution, events, protocol):
    """Recognize only a provider refusal that provably preceded every test action.

An uncertain command, GUI request, approval, product observation or failed
cleanup must stop normally. Strict event/protocol whitelists deliberately
reject partial or unfamiliar traces rather than guessing that replay is safe.
The executor's separate stopped acknowledgement remains a caller prerequisite.
"""
    if (execution.get('format') != 1 or execution.get('status') != 'automation-failure' or execution.get('exit_code') != 1
            or 'cleanup_error' not in execution
            or execution.get('cleanup_error') is not None or execution.get('result') is not None
            or execution.get('unverified_result') is not None or not events or not protocol):
        return False
    if events[-1].get('type') != 'turn.failed':
        return False
    for row in events:
        kind = row.get('type')
        if kind not in ('thread.started', 'turn.started', 'error', 'turn.failed'):
            return False
        if kind in ('error', 'turn.failed'):
            message = row.get('message') if kind == 'error' else row.get('error', {}).get('message')
            if not isinstance(message, str) or '429' not in message or 'Too Many Requests' not in message:
                return False
    return all(row.get('direction') in ('from-client', 'to-client', 'from-native', 'to-native')
               and row.get('method') in (None, 'initialize', 'notifications/initialized', 'tools/list')
               and row.get('error') is False and 'approval_tool' not in row for row in protocol)


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

    # Where the desktop updater's source has lived, newest layout first. The GPUI
    # cutover moved it from the webview shell's UI crate into farhelm-desktop in one
    # commit, so a real release tag carries one of these, depending on when it was
    # cut; were a tag to carry both, the first (newest) would be read.
    UPDATER_SOURCES = ('crates/farhelm-desktop/src/updater.rs',
                       'crates/farhelm-ui/src/desktop/updater.rs')

    def override(self, tag):
        """Select the recipe branch from old source, not the candidate's support.

This is source support only. Native launch forwarding and override removal still
require the recipe's actual startup-log and GUI observations.

The tag's updater source is read at the first of UPDATER_SOURCES it contains. A
tag with none of them is refused rather than reported as lacking the override:
every release from the upgrade floor onward has an updater, so a missing file
means the source moved again, and silently choosing the manual branch would
turn P0's updater check into an unavailable one for the wrong reason.
"""
        version(tag)
        # --full-tree: ls-tree paths are otherwise relative to where Git runs, unlike
        # the `tag:path` read below, which is always from the repository root.
        present = set(self.git('ls-tree', '--full-tree', '--name-only', tag + '^{commit}', '--',
                               *self.UPDATER_SOURCES).splitlines())
        for path in self.UPDATER_SOURCES:
            if path in present:
                return 'FARHELM_DESKTOP_UPDATE_LATEST' in self.git('show', tag + ':' + path)
        raise Refused(f'{tag} has no desktop updater source at a known path; '
                      'add its location to Source.UPDATER_SOURCES')


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


def plan(candidate, latest, source, floor=UPGRADE_FLOOR):
    """Plan the P0 pairs, remote Mac P1, then bonus coverage.

A standard gate cannot silently test a downgrade or choose the candidate as its
own previous release. Independent P0 fixtures exercise fresh install, in-app
updater discovery (stable candidates only: the updater never offers an RC or
dev build, so a prerelease plans no such pair) and installer upgrade, whose
pair also carries the supervisor upgrades, Linux P1 and P2. The remote Mac
needs an independent fresh pair because the GUI Mac runs the desktop app.
Bonus upgrade pairs cover
at most HISTORICAL_UPGRADE_LIMIT previous stables, newest first; older sources
inside the floor are deliberately omitted, not a coverage gap.
The caller must establish that the supplied floor matches the candidate's
source before planning; BRICK-TEST.md requires this manual comparison for real
runs. Tests supply a fixed floor so later policy raises cannot change their
fictional releases.
"""
    target = version(candidate)
    promoted = version(latest)
    minimum = version(floor)
    if promoted[3] != 1 or target < promoted:
        raise Refused('latest must be stable and candidate must not precede it')
    eligible = []
    for tag in source.tags():
        try:
            parsed = version(tag)
        except Refused:
            continue
        if parsed[3] == 1 and minimum <= parsed < target and parsed <= promoted:
            eligible.append(tag)
    eligible = sorted(set(eligible), key=version, reverse=True)
    if not eligible:
        raise Refused(f'no eligible previous stable from {floor} onward is available in source')
    if latest != candidate and latest not in eligible:
        raise Refused('the promoted stable is missing from the eligible source tags')
    previous = eligible[0]
    supported = source.override(previous)
    old = {'previous': previous, 'previous_commit': source.commit(previous),
           'old_supports_override': supported, 'priority': 'P0'}
    # Discovery runs before the installer pair so that pair, the last P0 one,
    # can go on to P1 and P2 with its candidate still running.
    p0 = [dict(old, id='p0-fresh', update_path=None)]
    if target[3] == 1:
        p0.append(dict(old, id='p0-updater-upgrade', update_path=update_path(candidate, latest, supported)))
    p0.append(dict(old, id='p0-installer-upgrade', update_path='manual'))
    # The local candidate on the installer pair still sends feedback before
    # that pair is cleaned up. The next pair uses two Macs and covers P1 only.
    passes = p0 + [dict(old, id=REMOTE_MAC_PASS, update_path=None, priority='P1')]
    for previous in eligible[:HISTORICAL_UPGRADE_LIMIT]:
        supported = source.override(previous)
        path = update_path(candidate, latest, supported)
        passes.append({'id': 'upgrade-' + previous, 'previous': previous,
                       'previous_commit': source.commit(previous), 'old_supports_override': supported,
                       'update_path': path, 'priority': 'bonus'})
    # Window gestures are only used on the fresh pair. Run independent
    # historical coverage first so a native input problem cannot block it.
    passes.append({'id': 'fresh', 'previous': old['previous'], 'update_path': None, 'priority': 'bonus'})
    return {'candidate': candidate, 'candidate_commit': source.commit(candidate),
            'latest_at_plan': latest, 'p0_sequence': [v['id'] for v in p0], 'passes': passes,
            'priority_layout': PRIORITY_LAYOUT}


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
def pass_roles(pass_id):
    """Keep the GUI Mac in every pair and choose its one disposable SSH peer."""
    peer = 'remote-mac' if pass_id == REMOTE_MAC_PASS else 'linux'
    return 'mac', peer


def checks(pass_id, smoke=False):
    """Declare acceptance boundaries before evidence is recorded.

GUI checks require actual image evidence and explicit operator observations.
Continuity checkpoints require independent before/after receipts as well.
"""
    if smoke:
        return {'commands': {'gui': False, 'continuity': False}}
    if pass_id == REMOTE_MAC_PASS:
        # The first three checks use fixed operations. The app and remote menu
        # checks also require native observations on the GUI Mac.
        return {key: {'gui': gui, 'command': gui, 'continuity': False, 'priority': 'P1'}
                for key, gui in (('artifact-verification', False), ('fixture-clean', False),
                                 ('install', False), ('app-start', True), ('remote-provision', True),
                                 ('remote-host-uninstall', True))}
    if pass_id.startswith('p0-'):
        # Separate upgrade fixtures keep installer success from standing in for
        # discovery by the old updater, and keep a failed discovery from hiding
        # whether the supervisors upgrade: the installer pair carries those.
        # Each supervisor's upgrade is its own checkpoint, before any new agent
        # runs on it, so a flaky agent turn cannot obscure a bricked supervisor.
        # P0 deliberately has no template, settings or continuity workload
        # prerequisites; those remain bonus coverage.
        #
        # The installer pair then goes on to P1 and P2 while its candidate still
        # runs. The old sessions P1 compares against can only be started on the
        # previous release, so `old-sessions` is a P1 checkpoint placed before
        # the upgrade: its failure (an old-version agent turn, say) makes the
        # old-session checks unavailable without touching P0.
        if pass_id == 'p0-fresh':
            items = {'artifact-verification': ('P0', False), 'fixture-clean': ('P0', False),
                     'install': ('P0', False), 'app-start': ('P0', True)}
        elif pass_id == 'p0-updater-upgrade':
            items = {'artifact-verification': ('P0', False), 'old-fixture': ('P0', True),
                     'update-discovery': ('P0', True), 'update-relaunch': ('P0', True)}
        else:
            items = {'artifact-verification': ('P0', False), 'old-fixture': ('P0', True),
                     'old-sessions': ('P1', True), 'update-relaunch': ('P0', True),
                     'local-supervisor-upgrade': ('P0', True), 'remote-supervisor-upgrade': ('P0', True),
                     'local-supervisor-agent': ('P0', True), 'remote-supervisor-agent': ('P0', True),
                     'local-old-session': ('P1', True), 'remote-old-session': ('P1', True),
                     'remote-host-remove': ('P1', True), 'remote-host-uninstall': ('P1', True)}
        required = {key: {'gui': gui, 'command': gui, 'continuity': False, 'priority': priority}
                    for key, (priority, gui) in items.items()}
        if pass_id == 'p0-installer-upgrade':
            # Feedback uses the candidate already running on this pair. Its
            # result is separate from P0 and P1 and must not prevent cleanup.
            required['p2-feedback-send'] = {'gui': True, 'command': False,
                                             'continuity': False, 'priority': 'P2'}
            required['p2-announcements'] = {'gui': True, 'command': True,
                                             'continuity': False, 'priority': 'P2'}
        return required
    # The main pane's title bar needs a selected session, so window-drag
    # follows the session checks. Keep it after the addendum too, since a
    # missing gesture ends the pass and those earlier checks use different input.
    # Manifests store their own required set, so older runs keep theirs.
    items = ({'artifact-verification': False, 'fixture-clean': False, 'install': False,
              'app-start': True, 'remote-provision': True, 'codex-conversation': True,
              'remote-terminal': True, 'addendum': False, 'window-drag': True} if pass_id == 'fresh' else
             {'artifact-verification': False, 'old-fixture': True, 'seed-state': True,
              'continuity-before': False, 'update-relaunch': True, 'preserved-state': True,
              'codex-resume': True, 'remote-update': True, 'reopen': True, 'addendum': False})
    hybrid = {'remote-provision', 'codex-conversation', 'remote-terminal', 'old-fixture', 'seed-state',
              'update-relaunch', 'preserved-state', 'codex-resume', 'remote-update', 'reopen'}
    return {key: {'gui': gui, 'command': key in hybrid,
                  'continuity': key in ('remote-update', 'reopen', 'continuity-before')}
            for key, gui in items.items()}


def priority_entry(entry, priority):
    """Project one priority's evidence without changing the saved observations.

Older manifests declare priority only on the pass, so their existing checks
keep that classification. An empty projection is not proof that the priority
was planned; report callers must check its required set before calling verdict.
"""
    return subset(entry, [key for key in entry['required'] if tier(entry, key) == priority])


def tier(entry, check_id):
    """Name a declared checkpoint's priority: its own label, else its pass's, else bonus."""
    return entry['required'][check_id].get('priority', entry.get('priority', 'bonus'))


def skip_origin(result):
    """Read the structured cause of an automatic skip. Older skips have no replay authority."""
    cause = result.get('derived_from')
    if (isinstance(cause, dict) and set(cause) == {'pass_id', 'check_id'}
            and all(isinstance(value, str) and value for value in cause.values())):
        return cause['pass_id'], cause['check_id']
    return None


def subset(entry, keys):
    """Project a pass onto some of its declared checks, keeping pass-level facts.

Retries, preflight and automation failures stay on the projection, so a
verdict over it still sees everything that can invalidate those checks.
"""
    return dict(entry, required={key: entry['required'][key] for key in keys},
                results={key: entry['results'][key] for key in keys if key in entry['results']})


def combine(verdicts):
    """Fold several verdicts: any failure fails, all passes pass, anything else could not run."""
    return 'fail' if 'fail' in verdicts else 'pass' if all(v == 'pass' for v in verdicts) else 'could not run'


def artifact(root, reference, limit=16 * 1024 * 1024):
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
            if not info.isfile() or info.size > limit:
                raise Refused('evidence member must be a regular file within its declared bound')
            return archive.extractfile(info).read()
    if path.stat().st_size > limit:
        raise Refused('direct evidence file exceeds 16 MiB')
    return path.read_bytes()


def is_image(value):
    """Recognize PNG or native CUA JPEG framing without interpreting the pixels.

Native CUA returns JPEGs on the exercised Mac runtime. Requiring conversion to
PNG would add an unnecessary capture/serialization step and obscure provenance.
GUI results separately bind the retained bytes to the actual native tool stream.
"""
    if (len(value) >= 33 and value.startswith(b'\x89PNG\r\n\x1a\n')
            and value[12:16] == b'IHDR' and all(struct.unpack('>II', value[16:24]))):
        return True
    if not value.startswith(b'\xff\xd8') or not value.endswith(b'\xff\xd9'):
        return False
    offset = 2
    while offset + 4 <= min(len(value), 65536):
        if value[offset] != 0xff:
            return False
        marker = value[offset + 1]
        if marker in (0xd8, 0xd9):
            offset += 2
            continue
        size = int.from_bytes(value[offset + 2:offset + 4], 'big')
        if size < 2 or offset + 2 + size > len(value):
            return False
        if marker in (0xc0, 0xc1, 0xc2):
            return size >= 8 and all(struct.unpack('>HH', value[offset + 5:offset + 9]))
        offset += size + 2
    return False


def progress(root, before, after, contents=None):
    """Reject malformed receipts as well as replayed or replaced process observations.

An observation's optional byte cache is shared with its digest binding, so the
receipt checked for progress cannot change between validation and recording.
"""
    # One observation binds one read of each reference. Repeated references in
    # a continuity chain must be validated and hashed from those same bytes.
    if contents is None:
        contents = {}
    for ref in (before, after):
        if ref not in contents:
            contents[ref] = artifact(root, ref)
    values = [json.loads(contents[ref]) for ref in (before, after)]
    for value in values:
        if not isinstance(value, dict) or any(type(value.get(k)) is not int or value[k] < 0
                                             for k in ('pid', 'start_ticks', 'counter')) or value['pid'] == 0:
            raise Refused('continuity receipts need actual numeric process/counter identities')
        uuid.UUID(value['boot_id'])
    require_progress(*values)
    return values


def continuity_chain(root, entry, check_id, before=None, after=None, contents=None):
    """Bind each ordered milestone to the same original, advancing Linux process.

Each pair must be live and advancing, but pairwise checks alone could accept a
replacement or replay old evidence at the next milestone. Later before samples
must be at least the preceding after sample, and later after samples must advance.
Recheck the same relation when reporting retained results, not only at recording.
An optional byte cache ties recursive checks to the observation being recorded;
callers that omit it still reread evidence when revalidating a retained result.
"""
    if contents is None:
        contents = {}
    order = ('continuity-before', 'remote-update', 'reopen')
    index = order.index(check_id)
    if before is None or after is None:
        references = entry['results'][check_id]['continuity']
        before, after = references['before'], references['after']
    current_before, current_after = progress(root, before, after, contents)
    if index:
        predecessor = entry['results'].get(order[index - 1])
        if not predecessor or predecessor['status'] != 'pass':
            raise Refused('record the preceding continuity milestone as pass first')
        previous = predecessor['continuity']
        continuity_chain(root, entry, order[index - 1], contents=contents)
        _, previous_after = progress(root, previous['before'], previous['after'], contents)
        if any(current_before[key] != previous_after[key] for key in ('pid', 'start_ticks', 'boot_id')):
            raise Refused('continuity milestone changed the original process identity')
        if current_before['counter'] < previous_after['counter']:
            raise Refused('continuity milestone replays an earlier observation')
        require_progress(previous_after, current_after)
    return current_before, current_after


# Persisted workflow state outlives the process supervising Tart. That separation
# lets a later invocation inspect an interruption without guessing clone ownership.
class Run:
    """Persist a workflow independently of the foreground guest supervisor.

Every modifying invocation reloads state under a lock. Profile edits cannot
redirect an existing run's cleanup, and incomplete preparation is retained for
reconciliation rather than guessed safe to retry.
"""

    def __init__(self, directory, profile, factory=Control, disk_usage=shutil.disk_usage, connector=connect,
                 volume_id=filesystem_id, tester=execute_guest, pause=time.sleep, clock=time.monotonic):
        """Bind lifecycle and tester execution to one profile's private evidence root.

Injected services let orchestration tests observe the scheduling and evidence
contracts without a real model, VM or credential store. Construction itself
claims no guests and does not read or alter the persisted workflow.
"""
        self.directory = Path(directory).resolve()
        if not self.directory.is_relative_to(Path(profile.values['evidence_root'])):
            raise Refused('workflow must stay under the configured private evidence root')
        self.profile = profile
        self.factory = factory
        self.disk_usage = disk_usage
        self.connector = connector
        self.volume_id = volume_id
        self.tester = tester
        self.pause = pause
        self.clock = clock
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

    def create(self, release_plan=None, inputs=None, smoke=False, scope='full'):
        """Bind scope and evidence obligations before any clone can be claimed.

Limited scope retains the bonus plan for honest coverage reporting, while
preventing its fixtures from starting. It cannot be attached to a legacy plan
that never declared the independent P0 installation paths.
"""
        if scope not in SCOPE_TIERS:
            raise Refused('unknown run scope')
        if scope != 'full' and (smoke or not release_plan or not release_plan.get('p0_sequence')):
            raise Refused('limited scope requires a plan with explicit P0 fixtures')
        self.data = {'format': 1, 'run_id': str(uuid.uuid4()), 'kind': 'fixture-smoke' if smoke else 'release',
                     'profile': self.profile.values, 'plan': release_plan, 'passes': {},
                     'gui_backend': ('fixture-command' if smoke else
                                     'guest-native' if 'guest_tester' in self.profile.values else 'historical-host'),
                     'scope': scope}
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
        """Keep transitions atomic and bind resumed operations to the original profile.

A run planned under the earlier P0/P1 layout (P0 held the old sessions and
the remote menu, P1 the feedback send) is refused here, for every command:
its checkpoints and fences follow that contract, so the checkout that started
it finishes it. Its own saved `report.json` stays as it was written.
"""
        if not self.path.is_file():
            raise Refused('workflow state is missing')
        with (self.directory / '.workflow.lock').open('a') as stream:
            fcntl.flock(stream, fcntl.LOCK_EX)
            self.data = json.loads(self.path.read_text())
            if self.data.get('format') != 1 or self.data['profile'] != self.profile.values:
                raise Refused('workflow/profile mismatch; inspect the original bindings')
            uuid.UUID(self.data['run_id'])
            plan = self.data['plan']
            if plan and plan.get('p0_sequence') and plan.get('priority_layout') != PRIORITY_LAYOUT:
                raise Refused('this run was planned under the earlier P0/P1 layout; read its saved report.json or '
                              'use the coordinator checkout that started it')
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
P0 scheduling and scope are checked before budget probes or clone mutations.
"""
        entry = self.selected(pass_id)
        if headless and self.data.get('gui_backend') == 'guest-native':
            raise Refused('guest-native product fixtures require the normal Mac desktop start')
        if self.data['plan'] and self.data['plan'].get('p0_sequence'):
            self.require_priority(pass_id)
        if entry['phase'] != 'pending':
            raise Refused('pass already attempted; inspect its state instead of replaying preparation')
        if any(v['phase'] in ('preparing', 'ready', 'failed') for v in self.data['passes'].values()):
            raise Refused('finish or reconcile the existing working pair first')
        self.budget()
        inspector = self.control()
        roles = pass_roles(pass_id)
        bases = {
            'mac': self.profile.values['mac_base'],
            'linux': self.profile.values['linux_base'],
            'remote-mac': self.profile.values.get('remote_mac_base', self.profile.values['mac_base']),
        }
        for role in roles:
            base = bases[role]
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
                for role in roles:
                    c.clone(role, bases[role]['name'])
                for role in roles:
                    c.start(role, headless=headless or role != 'mac')
                    c.ready(role, 120)
                peer_role = roles[1]
                peer = c.owned(peer_role)
                account = c.command('exec', peer, '/usr/bin/id', '-un', capture_output=True).stdout.decode().strip()
                if peer_role == 'remote-mac':
                    self.connector(c, c.owned('mac'), peer, account, self.profile.values['route_via_gateway'],
                                   peer_role=peer_role)
                else:
                    if account != self.profile.values['linux_user']:
                        raise Refused('Linux guest-execution account differs from configured SSH account')
                    self.connector(c, c.owned('mac'), peer, account, self.profile.values['route_via_gateway'])
                    stage = ('from pathlib import Path; import sys; '
                             'd=Path.home()/"farhelm-brick-test"; d.mkdir(mode=0o700,exist_ok=True); '
                             'p=d/"continuity.py"; p.write_bytes(sys.stdin.buffer.read()); p.chmod(0o600)')
                    c.mutate(['exec', '-i', peer, '/usr/bin/python3', '-c', stage],
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

    def record(self, pass_id, check_id, status, references, detail, before=None, after=None, native=None,
               in_flight=False):
        """Accept explicit observations only when required evidence is present.

Image presence does not interpret its contents: the agent's detail is the GUI
attestation. Verification and continuity receipts receive additional structural
checks, and later report generation revalidates retained artifact references.
New P0 plans additionally fence a hand-made record on its own chain
(`chain_block`); the owning guest import was fenced the same way when its
checkpoint was dispatched. Historical manifests without P0 fixtures retain
their declared rules.
Every guest-native pass binds its observation to the current checkpoint and
runtime, including command-only checks. GUI passes additionally require actual
native capture bytes; a matching task file cannot lend a stale result its identity.
Only the owning guest import (`in_flight`) can retain an observation whose
checkpoint was dispatched before the scope narrowed past it. A stop request
fences future actions; it cannot recall a live send or erase its independently
retained outcome.
"""
        entry = self.selected(pass_id)
        if entry['phase'] != 'ready' or check_id not in entry['required']:
            raise Refused('record results only for a prepared pass and declared check')
        if check_id in entry['results']:
            raise Refused('a result already exists; preserve it rather than overwriting history')
        if status not in ('pass', 'fail', 'could-not-run') or not detail.strip():
            raise Refused('provide a supported result and an actual observation')
        if check_id == 'old-sessions' and status == 'fail':
            # It runs on the previous release before any candidate code, so
            # whatever broke there is no candidate failure.
            status = 'could-not-run'
            detail = ('Reported as fail; old-sessions runs on the previous release, so it could not run: '
                      + detail)[:4000]
        if len(detail) > 4000 or len(references) > 20:
            raise Refused('observation or evidence list exceeds its bound')
        spec = entry['required'][check_id]
        priority = tier(entry, check_id)
        # While a guest tester holds the pass, only its own import of the
        # claimed checkpoint may record: a hand-made observation could take the
        # slot of a result already on its way back, or stop a chain under it.
        claim = entry.get('tester', {})
        owning = bool(in_flight and claim.get('phase') == 'running' and claim.get('check_id') == check_id
                      and claim.get('termination_observed') is True)
        if claim.get('phase') in ('running', 'reconciliation-required') and not owning:
            raise Refused('a guest tester owns this pass; wait for it or reconcile it before recording by hand')
        dispatched = False
        if self.data['plan'] and self.data['plan'].get('p0_sequence'):
            self.require_no_failure()
            dispatched = owning and self.data.get('gui_backend') == 'guest-native' and bool(native)
            if not self.in_scope(priority) and not dispatched:
                raise Refused(f'{self.scope_label()}: {priority} is outside this run scope')
            # Hand-made observations must prove their ordering here; the
            # owning import's checkpoint proved it before dispatch.
            if not owning:
                reason = self.chain_block(pass_id, check_id, status)
                if reason:
                    raise Refused(reason)
        native = {} if native is None else native
        if status == 'pass' and self.data.get('gui_backend') == 'guest-native' and set(native) != {'execution', 'events', 'task'}:
            raise Refused('guest-native passes require the independent executor; manual record is for failures/history')
        bound = set(references) | {ref for ref in (before, after) if ref is not None} | set(native.values())
        validated = {ref: artifact(self.directory, ref, limit=96 * 1024 * 1024)
                     if ref == native.get('events') else artifact(self.directory, ref) for ref in bound}
        if owning and native and json.loads(validated[native['task']]).get('attempt') != claim.get('attempt'):
            raise Refused('imported observation belongs to another attempt than the claimed one')
        contents = [validated[ref] for ref in references]
        if status == 'pass':
            image_digests = set()
            if not any(contents) or (spec['gui'] and not any(is_image(v) for v in contents)):
                raise Refused('a passing check requires evidence, including an actual image for GUI checks')
            if self.data.get('gui_backend') == 'guest-native':
                task = tester_request(json.loads(validated[native['task']]))
                execution = json.loads(validated[native['execution']])
                events = [json.loads(line) for line in validated[native['events']].splitlines() if line]
                identity = {key: task[key] for key in ('attempt', 'run_id', 'pass_id', 'check_id')}
                if (task['run_id'] != self.data['run_id'] or task['pass_id'] != pass_id or task['check_id'] != check_id
                        or task['gui'] != spec['gui'] or task['settings'] != self.profile.values['guest_tester']['settings']
                        or execution.get('status') != 'pass' or execution.get('result', {}).get('status') != 'pass'
                        or execution.get('result', {}).get('identity') != identity):
                    raise Refused('native tester receipt does not match this checkpoint')
                calls = execution_calls(task, events)
                if check_id == 'reopen' and not any(reopen_quit_call(call) for call in calls):
                    raise Refused('reopen did not use the structured native Quit action')
                image_digests = native_image_digests(events) if calls else set()
                if spec['gui'] and (not calls or not any(hashlib.sha256(v).hexdigest() in native_image_digests(events)
                                                        for v in contents if is_image(v))):
                    raise Refused('GUI image was not returned by actual guest-native CUA')
                if check_id in ('p2-feedback-send', 'remote-host-remove', 'remote-host-uninstall'):
                    captures = {hashlib.sha256(v).hexdigest() for v in contents if is_image(v)}
                    if len(captures & native_image_digests(events)) < 2:
                        raise Refused(f'{check_id} needs two distinct native images; review their action/result meaning')
                if check_id == 'p2-announcements':
                    captures = {hashlib.sha256(v).hexdigest() for v in contents if is_image(v)}
                    if len(captures & image_digests) < 3:
                        raise Refused('p2-announcements needs three distinct native images of the bell, dialog and history')
            if spec.get('command') and not any(v and not is_image(v)
                                               and hashlib.sha256(v).hexdigest() not in image_digests
                                               for v in contents):
                raise Refused('this check also requires retained command/identity evidence')
            if check_id == 'fixture-clean' and self.data.get('gui_backend') == 'guest-native':
                receipts = [json.loads(validated[ref]) for ref in references
                            if ref.rsplit('/', 1)[-1] == 'fixture-clean.json']
                if len(receipts) != 1:
                    raise Refused('fixture-clean needs the deterministic pair receipt')
                try:
                    require_clean_pair(receipts[0], pass_roles(pass_id)[1])
                except (ValueError, RuntimeError, KeyError, TypeError) as error:
                    raise Refused('fixture-clean receipt is not complete and clean') from error
            if check_id in ('update-relaunch', 'update-discovery') and 'latest_at_selection' not in entry:
                raise Refused('select the update path from a fresh latest observation first')
            if check_id == 'update-discovery' and entry['update_path'] not in ('ordinary', 'override'):
                raise Refused('a manual installer cannot prove click-to-upgrade discovery')
            if spec['continuity'] or check_id == 'reopen':
                if before is None or after is None:
                    raise Refused('continuity requires independent before/after receipts')
                samples = continuity_chain(self.directory, entry, check_id, before, after, contents=validated)
                if check_id == 'reopen':
                    self.check_reopen_transition(pass_id, references, native, samples, validated)
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
        if dispatched and not self.in_scope(priority):
            value['dispatched_before_scope_stop'] = True
        if native:
            value['native'] = native
        if before is not None or after is not None:
            value['continuity'] = {'before': before, 'after': after}
        # Bind each accepted observation to retained bytes, so a later accidental
        # overwrite cannot inherit the old attestation just by keeping its name.
        value['sha256'] = {ref: hashlib.sha256(validated[ref]).hexdigest() for ref in bound}
        entry['results'][check_id] = value
        self.save()
        self.note('decision', f'Pass `{pass_id}`, check `{check_id}` recorded `{status}`; private evidence retained.')
        self.settle()

    def test(self, pass_id, source=None, timeout=1200):
        """Own the host tester lease through execution and guest termination import.

Status uses a different lock. This lease prevents abandoned-claim repair from
racing a live coordinator without guessing numeric PID ownership.
"""
        with self.tester_owner():
            return self.test_owned(pass_id, source, timeout)

    @contextlib.contextmanager
    def tester_owner(self):
        """Reserve the tester independently from brief, queryable state transitions."""
        with (self.directory / '.tester-owner.lock').open('a') as stream:
            try:
                fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as error:
                raise Refused('another live coordinator owns this guest tester') from error
            yield

    def test_owned(self, pass_id, source=None, timeout=1200):
        """Run one prepared pass without host GUI input or interactive approval.

The workflow lock protects the attempt claim and each imported result, but is
released while the guest works so status remains queryable. Cleanup and another
tester refuse the active claim. Every checkpoint gets a fresh bounded turn and
retains its artifacts. A failed or unavailable P0 or bonus checkpoint stops the
sequence. A P1 one, a harness failure at P1 included, stops only the later P1
checkpoints, which are recorded as skipped while P2 still runs. A P2 product
failure leaves the other P2 check and ordinary cleanup available. Only a proven pre-action provider 429 can back off
into a separately retained fresh attempt; it never retries feedback, uncertain
actions or failed cleanup.
"""
        source = Source() if source is None else source
        if type(timeout) is not int or not 30 <= timeout <= 3600:
            raise Refused('guest tester deadline must be 30..3600 seconds')
        check_id = 'native-preflight'
        with self.locked():
            feedback = 'P2' in self.scope_tiers() and any(
                'p2-feedback-send' in value['required'] for value in self.data['passes'].values())
            config = require_setup(self.profile, feedback=feedback)
            entry = self.selected(pass_id)
            if entry['phase'] != 'ready' or entry.get('tester', {}).get('phase') in ('running', 'reconciliation-required'):
                raise Refused('test needs a ready pass with no active tester')
            if self.data.get('gui_backend') != 'guest-native':
                raise Refused('historical host-GUI workflow cannot acquire guest-native test results')
            if entry.get('automation_failure'):
                raise Refused('failed automation is retained; fix the fixture and use a new run')
            if entry.get('tester', {}).get('phase') == 'failed':
                raise Refused('failed checkpoint is retained. Use a supported retry transition or a new run.')
            self.require_priority(pass_id)
            # Resolve read-only source inputs before claiming the test. Once the
            # claim is saved, every mutation failure must leave a terminal state.
            context = {'candidate': self.data['plan']['candidate'], 'previous': entry['previous'],
                       'previous_commit': source.commit(entry['previous']), 'source_url': source_url(source),
                       'pass_id': pass_id}
            c = self.pair(pass_id)
            entry['tester'] = {'phase': 'running', 'check_id': check_id}
            self.save()
        try:
            with c.locked():
                if any(c.state(role)['State'] != 'running' for role in pass_roles(pass_id)):
                    raise Refused('both owned guests must be running before testing')
                if not entry.get('credentials_injected'):
                    roles = ('mac',) if pass_id == REMOTE_MAC_PASS else ('mac', 'linux')
                    copy_credentials(c, config, roles)
                    entry['credentials_injected'] = True
                guest_home = c.command('exec', c.owned('mac'), config['settings']['python'], '-c',
                                       'from pathlib import Path; print(Path.home())', capture_output=True).stdout.decode().strip()
                ssh = dict(c.data['ssh'])
            with self.locked():
                self.selected(pass_id)['credentials_injected'] = True
                self.save()
            context.update(ssh=ssh, pass_directory=guest_home + '/farhelm-brick-test/' + self.data['run_id'] + '/' + pass_id)
            context.update(source_checkout=context['pass_directory'] + '/source',
                           signed_inputs=context['pass_directory'] + '/signed-inputs',
                           shared_state=context['pass_directory'] + '/shared_state.json',
                           agents=self.profile.values['agents'],
                           managed_operations=True)
            source_files = {name: (HERE / name).read_text() for name in
                            ('guest_tester.py', 'guest_operations.py', 'guest_ui.py', 'checkpoint_contract.py',
                             'window_geometry.py', 'window_geometry.swift', 'fixture_probe.py', 'verify_release.py',
                             'continuity.py', 'RECIPE.md', 'OPERATIONS.md')}
            with self.locked():
                entry = self.selected(pass_id)
                context['check_contracts'] = entry['required']
                if entry.get('priority', 'bonus') == 'bonus':
                    addendum = self.directory / 'addendum.md'
                    if addendum.is_symlink() or not addendum.is_file() or not 0 < addendum.stat().st_size <= 32768:
                        raise Unavailable('bonus checks require a bounded filled addendum.md in the private run')
                    source_files['addendum.md'] = addendum.read_text()
                    context['addendum'] = addendum.read_text()
            with self.locked():
                entry = self.selected(pass_id)
                sequence = ([] if entry.get('native_preflight') else ['native-preflight']) + [
                    key for key in entry['required'] if key not in entry['results']]
            for check_id in sequence:
                with self.locked():
                    entry = self.selected(pass_id)
                    priority = (entry.get('priority', 'bonus') if check_id == 'native-preflight'
                                else tier(entry, check_id))
                    # Scope is reread before every checkpoint: a stop request
                    # made while an earlier turn ran fences the later ones.
                    if not self.in_scope(priority):
                        continue
                    is_feedback = check_id == 'p2-feedback-send'
                    if check_id != 'native-preflight':
                        # A checkpoint that can never run is already recorded
                        # as skipped; the independent ones after it still run.
                        self.settle()
                        if check_id in entry['results']:
                            print(json.dumps({'pass': pass_id, 'check': check_id, 'status': 'skipped',
                                              'reason': entry['results'][check_id]['detail']}), flush=True)
                            continue
                        reason = self.chain_block(pass_id, check_id, 'pass')
                        if reason:
                            raise Refused(reason)
                    context.pop('feedback_authorized', None)
                    if is_feedback:
                        # Recheck the separate live-send consent at its action
                        # boundary; earlier credential consent cannot stand in.
                        require_setup(self.profile, feedback=True)
                        context['feedback_authorized'] = True
                        report = self.report()
                        print(json.dumps({'p0': report['p0'], 'p1': report['p1']}), flush=True)
                    if check_id in ('update-discovery', 'update-relaunch') and 'latest_at_selection' not in entry:
                        self.select_update(pass_id, download(ORIGIN + '/latest').decode().strip())
                    context['update_path'] = entry['update_path']
                    if check_id == 'update-discovery' and entry['update_path'] == 'manual':
                        latest = self.manual_discovery_selection(entry)
                        if latest is None:
                            raise Refused('manual discovery path requires a valid update selection receipt')
                        detail = (f'In-app update discovery is required for stable release {context["candidate"]}. '
                                  f'The public update channel pointed to {latest}, and the {entry["previous"]} app '
                                  'cannot select a stable release before it becomes the public default.')
                        # This failure is established by the selection receipt before a guest turn starts.
                        entry['tester'] = {'phase': 'running', 'check_id': check_id, 'termination_observed': True}
                        self.record(pass_id, check_id, 'fail', [f'{pass_id}/latest-at-selection.json'], detail,
                                    in_flight=True)
                        raise CheckStopped(detail)
                    context['latest_at_selection'] = entry.get('latest_at_selection')
                    context['prior_results'] = entry['results']
                    task = task_for(self, pass_id, check_id, context, turn_timeout(check_id, pass_id, timeout))
                    entry['tester'].update(check_id=check_id, attempt=task['attempt'])
                    entry['tester']['guest_directory'] = context['pass_directory'] + '/' + task['attempt']
                    entry['tester']['termination_observed'] = False
                    self.save()
                # The first turn's floor also applies to the budget shared by its retries.
                checkpoint_deadline = self.clock() + task['timeout_seconds']
                for provider_attempt in range(len(PROVIDER_BACKOFF) + 1):
                    with c.locked():
                        label = self.tester(c, task, source_files)
                    with self.locked():
                        self.selected(pass_id)['tester']['termination_observed'] = True
                        self.save()
                    prefix = pass_id + '/' + label + '.tar::./'
                    native = {key: prefix + filename for key, filename in
                              (('execution', 'execution.json'), ('events', 'events.jsonl'), ('task', 'task.json'))}
                    execution = json.loads(artifact(self.directory, native['execution']))
                    retained_task = json.loads(artifact(self.directory, native['task']))
                    if retained_task != task:
                        raise Refused('retained guest envelope differs from the claimed attempt')
                    if (execution.get('status') != 'automation-failure' or is_feedback
                            or provider_attempt == len(PROVIDER_BACKOFF)):
                        break
                    try:
                        retry_refs = dict(native, protocol=prefix + 'managed-protocol.jsonl')
                        retry_bytes = {key: artifact(self.directory, ref) for key, ref in retry_refs.items()}
                        event_rows = [json.loads(row) for row in retry_bytes['events'].splitlines() if row]
                        protocol_rows = [json.loads(row) for row in retry_bytes['protocol'].splitlines() if row]
                        safe = rate_limited_without_actions(execution, event_rows, protocol_rows)
                    except (OSError, ValueError, RuntimeError, KeyError, TypeError, AttributeError, tarfile.TarError):
                        safe = False
                    delay = PROVIDER_BACKOFF[provider_attempt]
                    if not safe or self.clock() + delay + 30 >= checkpoint_deadline:
                        break
                    with self.locked():
                        entry = self.selected(pass_id)
                        entry.setdefault('provider_retries', []).append({
                            'check_id': check_id, 'attempt': task['attempt'], 'delay_seconds': delay,
                            'detail': 'Provider 429 before any command, UI or approval; stopped acknowledgement retained.',
                            'sha256': {retry_refs[key]: hashlib.sha256(raw).hexdigest()
                                       for key, raw in retry_bytes.items()}})
                        self.save()
                        self.note('decision', f'Provider 429 before actions at `{pass_id}/{check_id}`; '
                                  f'retain attempt `{task["attempt"]}`, back off {delay}s before a fresh attempt.')
                    print(json.dumps({'pass': pass_id, 'check': check_id, 'provider_backoff_seconds': delay}), flush=True)
                    self.pause(delay)
                    with self.locked():
                        entry = self.selected(pass_id)
                        if check_id in ('update-discovery', 'update-relaunch') and (
                                download(ORIGIN + '/latest').decode().strip() != entry['latest_at_selection']):
                            raise Unavailable('public default changed during provider backoff; replan before update actions')
                        remaining = int(checkpoint_deadline - self.clock())
                        if remaining < 30:
                            raise Unavailable('provider backoff exhausted the checkpoint deadline')
                        task = task_for(self, pass_id, check_id, context, remaining)
                        entry['tester'].update(attempt=task['attempt'], termination_observed=False,
                            guest_directory=context['pass_directory'] + '/' + task['attempt'])
                        self.save()
                observation = execution.get('result') or execution.get('unverified_result')
                identity = {key: task[key] for key in ('attempt', 'run_id', 'pass_id', 'check_id')}
                if observation is not None and observation.get('identity') != identity:
                    raise Refused('guest observation identity differs from the claimed attempt')
                permission_block = (observation or {}).get('status') == 'permission-block'
                if (priority == 'P1' and check_id != 'native-preflight'
                        and execution.get('status') == 'automation-failure' and not permission_block):
                    # A harness failure at an optional P1 check makes that check
                    # unavailable and stops the P1 chain like any P1 nonpass; it
                    # must not keep the pair's P0 checks (after `old-sessions`)
                    # or the send from running. The guest stopped (termination
                    # was acknowledged to get here) and nothing is replayed. A
                    # permission block still stops the pair below, and a failure
                    # the tester claimed stays visible for review.
                    with self.locked():
                        entry = self.selected(pass_id)
                        if (observation or {}).get('status') == 'fail' and check_id != 'old-sessions':
                            entry.setdefault('unverified_failures', []).append(
                                {key: observation[key] for key in ('identity', 'status', 'detail')})
                            self.save()
                        self.record(pass_id, check_id, 'could-not-run', [],
                                    ('Guest automation failed at this P1 check: '
                                     + str(execution.get('error') or (observation or {}).get('detail', '')))[:4000],
                                    native=native, in_flight=True)
                    print(json.dumps({'pass': pass_id, 'check': check_id, 'status': 'could-not-run'}), flush=True)
                    continue
                if execution.get('status') not in ('pass', 'fail', 'could-not-run'):
                    with self.locked():
                        claim = self.selected(pass_id)['tester']
                        claim['guest_status'] = 'permission-block' if permission_block else execution.get('status')
                        claim['observation'] = execution.get('result') or execution.get('unverified_result')
                        self.save()
                    raise Refused('guest-native automation failed; retained attempt ' + task['attempt'] + ': '
                                  + str(execution.get('error', execution.get('result', {}).get('detail', '')))[:1000])
                result = execution['result']
                if execution['status'] != result['status']:
                    raise Refused('guest execution summary and observation disagree')
                with self.locked():
                    entry = self.selected(pass_id)
                    if check_id == 'native-preflight':
                        if result['status'] != 'pass':
                            entry['tester']['observation'] = result
                            self.save()
                            raise Refused(result['detail'])
                        events = [json.loads(line) for line in artifact(self.directory, native['events'],
                                                                         limit=96 * 1024 * 1024).splitlines() if line]
                        if not native_image_digests(events):
                            raise Refused('native readiness has no actual guest CUA screenshot')
                        bound = list(native.values()) + [prefix + ref for ref in result['evidence']]
                        images = native_image_digests(events)
                        if not any(hashlib.sha256(artifact(self.directory, prefix + ref)).hexdigest() in images
                                   for ref in result['evidence']):
                            raise Refused('native readiness image was not retained')
                        entry['native_preflight'] = {'native': native, 'sha256': {
                            ref: hashlib.sha256(artifact(self.directory, ref, limit=96 * 1024 * 1024)
                                                if ref == native['events'] else artifact(self.directory, ref)).hexdigest()
                            for ref in bound}}
                        self.save()
                    else:
                        continuity = result['continuity'] or {}
                        if result['status'] == 'fail':
                            # Saved before the import, so a failure claim the
                            # guest reported stays visible for review even if
                            # its evidence then fails import (an unverified
                            # product failure); a successful import clears it.
                            entry['tester']['observation'] = {key: result[key]
                                                              for key in ('identity', 'status', 'detail')}
                            self.save()
                        self.record(pass_id, check_id, result['status'], [prefix + ref for ref in result['evidence']],
                                    result['detail'], prefix + continuity['before'] if continuity else None,
                                    prefix + continuity['after'] if continuity else None, native=native,
                                    in_flight=True)
                        entry['tester'].pop('observation', None)
                        self.save()
                        if result['status'] != 'pass' and priority not in ('P1', 'P2'):
                            raise CheckStopped(result['detail'])
                print(json.dumps({'pass': pass_id, 'check': check_id, 'status': result['status']}), flush=True)
        except BaseException as error:
            with self.locked():
                entry = self.selected(pass_id)
                if not isinstance(error, CheckStopped):
                    entry['automation_failure'] = {'check_id': check_id, 'error': type(error).__name__,
                                                   'status': 'could-not-run',
                                                   'kind': 'unavailable' if isinstance(error, Unavailable) else 'harness-failure',
                                                   'detail': str(error)[:1000]}
                claim = entry['tester']
                uncertain = claim.get('attempt') and not claim.get('termination_observed')
                claim['phase'] = 'reconciliation-required' if uncertain or isinstance(error, CleanupPending) else 'failed'
                self.save()
                self.note('decision', f'Guest-native test failed at `{pass_id}/{check_id}`; no host/manual rescue is allowed.')
            raise
        with self.locked():
            self.selected(pass_id)['tester']['phase'] = 'complete'
            self.save()

    def retry_unavailable(self, pass_id, reason):
        """Retain an unavailable observation before retrying its repaired fixture.

The host must inspect retained native actions and explain why repeating this
checkpoint is safe after repair. This transition never clears a product fail,
an automation error, a pending action, a P1 continuity check (its sequence
already went on past it) or any feedback attempt. It records the operator's
judgment beside the unchanged original result and its byte hashes.
Later automatic skips whose cause is this checkpoint are archived with it.
Earlier slots have already passed. A later legacy skip may conceal a different cause.
"""
        if not isinstance(reason, str) or not 20 <= len(reason.strip()) <= 2000:
            raise Refused('explain the fixture repair and why this checkpoint can be repeated safely')
        with self.tester_owner(), self.locked():
            entry = self.selected(pass_id)
            claim = entry.get('tester', {})
            check = claim.get('check_id')
            result = entry['results'].get(check, {})
            if (entry['phase'] != 'ready' or claim.get('phase') != 'failed'
                    or claim.get('termination_observed') is not True or entry.get('automation_failure')
                    or result.get('status') != 'could-not-run'
                    or tier(entry, check) in ('P1', 'P2')
                    or len(entry.get('checkpoint_retries', [])) >= 3
                    or not self.result_evidence_retained(result)):
                raise Refused('retry only a stopped unavailable P0 or bonus checkpoint with intact evidence, never feedback')
            ordered = list(entry['required'])
            discarded_skips = {}
            for key in ordered[ordered.index(check) + 1:]:
                later = entry['results'].get(key)
                if later is not None and (later['status'] != 'skipped' or skip_origin(later) is None):
                    raise Refused('later checkpoint observations prevent replaying this fixture boundary')
                if later is not None and skip_origin(later) == (pass_id, check):
                    discarded_skips[key] = later
            native = result.get('native', {})
            rows = [json.loads(line) for line in artifact(self.directory, native['events'],
                                                        limit=96 * 1024 * 1024).splitlines() if line]
            native_events(rows)
            started = {row['item']['id'] for row in rows if row.get('type') == 'item.started'
                       and row.get('item', {}).get('type') == 'mcp_tool_call'}
            completed = {row['item'].get('id') for row in rows if row.get('type') == 'item.completed'
                         and row.get('item', {}).get('type') == 'mcp_tool_call'}
            if started - completed:
                raise Refused('unavailable checkpoint has an uncertain tool call')
            c = self.pair(pass_id)
            with c.locked():
                if any(c.state(role)['State'] != 'running' for role in ('mac', 'linux')):
                    raise Refused('retry requires both unchanged owned guests running')
            entry.setdefault('checkpoint_retries', []).append({'reason': reason.strip(), 'tester': dict(claim),
                                                               'result': result, 'discarded_skips': discarded_skips})
            for key in discarded_skips:
                del entry['results'][key]
            del entry['results'][check]
            entry['tester'] = {'phase': 'retry-ready', 'check_id': check}
            self.save()
            self.note('decision', f'Retained unavailable `{pass_id}/{check}` before a repaired-fixture retry: {reason}')

    def retry_readiness(self, pass_id):
        """Release only a proven pre-action early-agent-sample failure for a fresh attempt.

Older bridges made that read-only prerequisite fatal. Retained task, execution
and strict event records must prove that the tester stopped before any product
action. Successful checkpoints stay accepted; the failed attempt and its byte
hashes stay visible in recovery history. No permission, transport, product or
uncertain-action failure is cleared by this transition.
"""
        with self.tester_owner(), self.locked():
            entry = self.selected(pass_id)
            claim = entry.get('tester', {})
            check = claim.get('check_id')
            if (entry['phase'] != 'ready' or claim.get('phase') != 'failed'
                    or claim.get('termination_observed') is not True
                    or check not in ('local-supervisor-agent', 'remote-supervisor-agent')
                    or check in entry['results'] or not entry.get('automation_failure')
                    or len(entry.get('readiness_retries', [])) >= 3):
                raise Refused('readiness retry needs a stopped, unfinished early-agent-sample failure')
            prefix = pass_id + '/tester-' + claim['attempt'] + '.tar::./'
            refs = {key: prefix + filename for key, filename in (
                ('task', 'task.json'), ('execution', 'execution.json'), ('events', 'events.jsonl'),
                ('failure', 'managed-failure.json'))}
            raw = {key: artifact(self.directory, ref, limit=96 * 1024 * 1024) for key, ref in refs.items()}
            envelope, execution = json.loads(raw['task']), json.loads(raw['execution'])
            expected = {'run_id': self.data['run_id'], 'pass_id': pass_id,
                        'attempt': claim['attempt'], 'check_id': check}
            observation = execution.get('result') or execution.get('unverified_result') or {}
            if (any(envelope.get(key) != value for key, value in expected.items())
                    or envelope.get('settings') != self.profile.values['guest_tester']['settings']
                    or execution.get('status') != 'automation-failure' or execution.get('cleanup_error')
                    or observation.get('identity') != expected or observation.get('status') != 'automation-failure'
                    or json.loads(raw['failure']).get('error') !=
                    'RuntimeError: sample the new session terminal hosts only after the new agent session exists'):
                raise Refused('retained failure does not prove the supported read-only readiness mistake')
            rows = [json.loads(line) for line in raw['events'].splitlines() if line]
            started = set()
            completed = set()
            refused = 0
            operation = 'remote_terminal_hosts' if check.startswith('remote-') else 'terminal_hosts'
            for row in rows:
                item = row.get('item', {})
                if item.get('type') != 'mcp_tool_call':
                    continue
                if row.get('type') == 'item.started':
                    started.add(item['id'])
                    continue
                if row.get('type') != 'item.completed':
                    continue
                completed.add(item['id'])
                args, result = item.get('arguments', {}), item.get('result') or {}
                # Codex can lift MCP isError into the event's failed status,
                # leaving only text in result. Both spellings still need the
                # exact fixed-operation failure record checked above.
                error = (item.get('status') == 'failed' or item.get('error')
                         or result.get('isError') or result.get('is_error'))
                allowed = (item.get('server') == 'cua_repl' and item.get('status') in ('completed', 'failed')
                           and ((item.get('tool') == 'brick_ui' and args.get('method') in
                                 ('getState', 'bind', 'getAXState', 'getAXStateAndScreenshot', 'getScreenshot')
                                 and not error)
                                or (item.get('tool') == 'brick_action' and args == {'operation': 'prior_receipts'}
                                    and not error)
                                or (item.get('tool') == 'brick_action' and args == {'operation': operation}
                                    and error and not refused)))
                if not allowed or refused:
                    raise Refused('readiness retry refuses product actions, other errors or work after refusal')
                refused += bool(error)
            if refused != 1 or started - completed:
                raise Refused('readiness retry needs one completed sampling refusal and no uncertain call')
            # Only the retried check's own prerequisites count: an unavailable
            # P1 result before it (`old-sessions`) does not hold back P0.
            if any(entry['results'][key]['status'] != 'pass' or not self.result_evidence_retained(entry['results'][key])
                   for key in self.prerequisites(entry, check) if key in entry['results']):
                raise Refused('earlier checkpoint evidence is not intact')
            c = self.pair(pass_id)
            with c.locked():
                if any(c.state(role)['State'] != 'running' for role in ('mac', 'linux')):
                    raise Refused('retry requires both unchanged owned guests running')
            entry.setdefault('readiness_retries', []).append({
                'tester': dict(claim), 'failure': entry['automation_failure'], 'native': refs,
                'sha256': {ref: hashlib.sha256(raw[key]).hexdigest() for key, ref in refs.items()}})
            del entry['automation_failure']
            entry['tester'] = {'phase': 'retry-ready', 'check_id': check}
            self.save()
            self.note('decision', f'Retained early sampling failure for `{pass_id}/{check}`; '
                      'proven no product action, fresh checkpoint attempt authorized after repair.')

    def reconcile_tester(self, pass_id):
        """Release an abandoned claim only after independently observed guest termination.

The live host lease refuses interference with another operator. The exclusive
guest cancellation marker asks its executor to terminate its owned child group;
no persisted PID is signaled. A stopped VM independently proves its tester gone.
Missing acknowledgement preserves the cleanup fence and the failed observation.
Recovered observations remain diagnostics, never accepted checkpoints. A host
interruption must not hide a product-failure claim that the guest exported,
but unreadable or mismatched diagnostics cannot undo acknowledged termination.
"""
        with self.tester_owner(), self.locked():
            entry = self.selected(pass_id)
            claim = entry.get('tester', {})
            if claim.get('phase') not in ('running', 'reconciliation-required'):
                raise Refused('no abandoned active tester claim to reconcile')
            c = self.pair(pass_id)
            with c.locked():
                state = c.state('mac')['State']
                if state == 'running':
                    if not claim.get('guest_directory') or not claim.get('attempt'):
                        # The claim precedes authentication preparation. No task
                        # identifier means this coordinator never staged a tester.
                        if claim.get('attempt'):
                            raise CleanupPending('missing guest task path; preserve the cleanup fence')
                    else:
                        task = {'attempt': claim['attempt'], 'settings': self.profile.values['guest_tester']['settings']}
                        cancel_guest(c, task, claim['guest_directory'])
                        label = 'reconciled-' + str(uuid.uuid4())
                        c.collect('mac', claim['guest_directory'] + '/output/export', label)
                        claim['reconciliation_archive'] = pass_id + '/' + label + '.tar'
                        prefix = claim['reconciliation_archive'] + '::./'
                        try:
                            retained = tester_request(json.loads(artifact(self.directory, prefix + 'task.json')))
                            identity = {'attempt': claim['attempt'], 'run_id': self.data['run_id'],
                                        'pass_id': pass_id, 'check_id': claim['check_id']}
                            if ({key: retained[key] for key in identity} != identity
                                    or retained['settings'] != self.profile.values['guest_tester']['settings']):
                                raise Refused('reconciled task identity/runtime differs from the abandoned claim')
                            execution = json.loads(artifact(self.directory, prefix + 'execution.json'))
                            if not isinstance(execution, dict) or execution.get('format') != 1:
                                raise Refused('reconciled execution summary is malformed')
                            observation = execution.get('result') or execution.get('unverified_result')
                            if observation is not None:
                                if not isinstance(observation, dict) or observation.get('identity') != identity:
                                    raise Refused('reconciled observation identity differs from the abandoned claim')
                                detail = observation.get('detail')
                                if (observation.get('status') not in RESULT_STATES or not isinstance(detail, str)
                                        or not detail.strip() or len(detail) > 4000):
                                    raise Refused('reconciled diagnostic observation is malformed')
                                # No record() call: turn provenance and evidence
                                # acceptance were interrupted. Retain only the
                                # bounded claim needed by the review-warning report.
                                claim['observation'] = {key: observation[key] for key in ('identity', 'status', 'detail')}
                        except (OSError, ValueError, RuntimeError, KeyError, TypeError, tarfile.TarError) as error:
                            claim['reconciliation_evidence_error'] = str(error)[:1000]
                elif state != 'stopped':
                    raise CleanupPending('guest state cannot establish tester termination')
            claim.update(phase='failed', reconciled=True)
            entry.setdefault('automation_failure', {'check_id': claim.get('check_id', 'native-preflight'),
                             'status': 'could-not-run', 'kind': 'harness-failure', 'error': 'abandoned-coordinator',
                             'detail': 'Abandoned tester stopped by verified reconciliation.'})
            self.save()
            self.note('decision', f'Abandoned tester for `{pass_id}` reconciled; failed evidence retained, no test resumed.')

    def select_update(self, pass_id, latest):
        """Reobserve /latest at the action boundary instead of trusting initial planning.

If promotion happened while state was seeded, the ordinary path is selected and
reported as after-publication coverage. A default beyond the candidate refuses
continuation. No endpoint is modified by this operation.
The independent P0 installer fixture always uses the manual branch, even when
the old app supports discovery. Its success cannot stand in for the updater.
"""
        entry = self.selected(pass_id)
        if entry['phase'] != 'ready' or not (pass_id.startswith('upgrade-') or
                                            pass_id in ('p0-installer-upgrade', 'p0-updater-upgrade')):
            raise Refused('select an update only for a prepared upgrade pass')
        if self.data['plan'].get('p0_sequence'):
            self.require_priority(pass_id)
        if 'latest_at_selection' in entry:
            raise Refused('update selection already recorded; inspect the original observation')
        selected = update_path(self.data['plan']['candidate'], latest, entry['old_supports_override'])
        if pass_id == 'p0-installer-upgrade':
            selected = 'manual'
        receipt = self.directory / pass_id / 'latest-at-selection.json'
        atomic_json(receipt, {'latest': latest, 'update_path': selected})
        entry.update(latest_at_selection=latest, update_path=selected)
        self.save()
        self.note('decision', f'Pass `{pass_id}` selected `{selected}` from observed public default `{latest}`.')

    def manual_discovery_selection(self, entry):
        """Return the observed public default for a verified, unattempted manual discovery path.

Planning alone is provisional because the public default can move. An actual
discovery observation takes precedence over the selected path.
"""
        latest = entry.get('latest_at_selection')
        if (entry.get('id') != 'p0-updater-upgrade' or entry.get('update_path') != 'manual'
                or latest is None or 'update-discovery' not in entry['required']
                or 'update-discovery' in entry['results']):
            return None
        candidate = self.data['plan']['candidate']
        try:
            receipt = artifact(self.directory, 'p0-updater-upgrade/latest-at-selection.json')
            if (json.loads(receipt) != {'latest': latest, 'update_path': 'manual'}
                    or update_path(candidate, latest, entry['old_supports_override']) != 'manual'):
                return None
        except (OSError, ValueError, RuntimeError, tarfile.TarError):
            return None
        return latest

    def add_check(self, pass_id, check_id, gui=False):
        """Keep release-specific obligations out of the tiered fixtures."""
        entry = self.selected(pass_id)
        if entry.get('priority') == 'P0':
            raise Refused('add broader functionality checks to bonus passes, not P0')
        if entry.get('priority') == 'P1':
            raise Refused('add broader functionality checks to bonus passes, not P1')
        if entry['phase'] not in ('pending', 'ready'):
            raise Refused('add obligations before completing or failing a pass')
        if not re.fullmatch(r'extra-[a-z0-9-]+', check_id) or check_id in entry['required']:
            raise Refused('use a new extra-... check identifier')
        entry['required'][check_id] = {'gui': gui, 'command': False, 'continuity': False}
        self.save()
        self.note('decision', f'Added required addendum check `{check_id}` to `{pass_id}`.')

    def limit_scope(self, scope):
        """Persist a mid-session stop after P0, P1 or P2 without changing recorded results.

This fences checkpoints and fixtures not yet dispatched; the supervising agent
still owns any in-flight GUI work and clean guest shutdown. Stopping after P2
keeps the live feedback send in scope, so it rechecks that consent, as widening
back to full coverage does. Historical plans cannot acquire missing P0
coverage through a scope edit.
"""
        if scope not in SCOPE_TIERS or scope == 'full':
            raise Refused('choose a stop after P0, P1 or P2')
        if not self.data['plan'] or not self.data['plan'].get('p0_sequence'):
            raise Refused('this historical run has no explicit P0 fixtures; start a new P0 plan')
        if 'P2' in SCOPE_TIERS[scope] and self.data.get('gui_backend') == 'guest-native':
            require_setup(self.profile, feedback=True)
        self.data['scope'] = scope
        self.save()
        self.note('decision', f'{self.scope_label().capitalize()} requested; later tiers and bonus fixtures are outside scope.')
        self.settle()

    def scope_tiers(self):
        """Name the priority tiers this run's requested scope includes."""
        return SCOPE_TIERS[self.data.get('scope', 'full')]

    def in_scope(self, priority):
        """Say whether a checkpoint of this priority may still be dispatched; bonus work needs full scope."""
        return priority in self.scope_tiers() if priority in TIERS else self.data.get('scope', 'full') == 'full'

    def scope_label(self):
        """Describe the requested scope in the maintainer's words, for refusals and notes."""
        scope = self.data.get('scope', 'full')
        return 'full coverage' if scope == 'full' else 'stop after ' + scope.split('-')[0].upper() + ' checks'

    def require_no_failure(self):
        """Stop the whole run after a harness failure or a failed bonus check.

A preparation or automation error means the harness is unreliable, so nothing
else runs until a new run, with one exception: a P0 or P1 pair the maintainer
has already released (`cleanup --discard-failed` after deciding its retention).
Those pairs' fixtures are independent, and a relaunch that never brought the candidate
back ends as an automation error too, so a broken updater must not keep the
installer pair from showing whether the supervisors brick. A failed bonus
check stops bonus coverage, the only work left by then. Product failures in P0,
P1 and P2 fence their own chains instead (`chain_block`).
"""
        for entry in self.data['passes'].values():
            priority = entry.get('priority', 'bonus')
            released = priority in ('P0', 'P1') and entry['phase'] == 'deleted'
            if (not released and (entry.get('preparation_error') or entry.get('automation_failure'))) or (
                    priority == 'bonus'
                    and any(result['status'] != 'pass' for result in entry['results'].values())):
                raise Refused('stop after the first failed or unavailable check; preserve this run')

    def prerequisites(self, entry, check_id):
        """List the earlier checkpoints of this pass that a P0 or P1 checkpoint depends on (`prerequisite_checks`)."""
        return prerequisite_checks(entry['required'], check_id)

    def skip(self, pass_id, check_id, reason, cause):
        """Record that a P1 or P2 checkpoint can never run, and why (`dead_end`).

The skip is a terminal result like any other: it counts as could not run,
needs no evidence, and lets the send and cleanup proceed without waiting on a
check that cannot run.
When the decision comes from a checkpoint, keep its identity so a supported
retry can undo this scheduling decision without erasing an observation.
"""
        entry = self.selected(pass_id)
        result = {'status': 'skipped', 'evidence': [], 'detail': reason, 'sha256': {}}
        if cause is not None:
            result['derived_from'] = {'pass_id': cause[0], 'check_id': cause[1]}
        entry['results'][check_id] = result
        self.save()
        self.note('decision', f'Skipped `{pass_id}/{check_id}`: {reason}')

    def settle(self):
        """Skip every pending in-scope P1 or P2 checkpoint of a ready pass that can never run.

Runs after every recorded result, when the scope widens, before cleanup and
before the scheduler dispatches a checkpoint, so the manual `record` path and
the guest scheduler settle the same way. Checkpoints outside the requested
scope stay unrecorded until the scope reaches them. Checkpoints are visited in
pair order, so a skip that stops a later P1 checkpoint's chain is seen in the
same sweep. A checkpoint a guest tester has claimed is left to that tester.
"""
        if not (self.data['plan'] and self.data['plan'].get('p0_sequence')):
            return
        for pass_id, entry in self.data['passes'].items():
            if entry['phase'] != 'ready':
                continue
            claim = entry.get('tester', {})
            for check_id in entry['required']:
                if (check_id in entry['results'] or tier(entry, check_id) not in ('P1', 'P2')
                        or not self.in_scope(tier(entry, check_id))):
                    continue
                if claim.get('phase') in ('running', 'reconciliation-required') and claim.get('check_id') == check_id:
                    continue
                block = self.dead_end(pass_id, check_id)
                if block:
                    self.skip(pass_id, check_id, *block)

    def order_block(self, pass_id, check_id, stopping):
        """Return the reason and original checkpoint when pair order forbids this check.

A failed, unavailable or skipped result in one of the `stopping` tiers stops
the rest of that chain. A checkpoint whose place in the pair's order has
passed (a later one ran; a skip did not) cannot run any more: `old-sessions` skipped
under a stop after P0 must not start its "previous release" sessions on the
candidate once the run is widened again.
"""
        entry = self.selected(pass_id)
        ordered = list(entry['required'])
        # Once a later check ran, this earlier slot is spent independently
        # of any stop encountered afterward. This matters when P1 is enabled
        # after the installer has already crossed the old-session boundary.
        retried = {row['tester']['check_id'] for row in entry.get('checkpoint_retries', [])}
        later = [key for key in ordered[ordered.index(check_id) + 1:]
                 if key in retried or entry['results'].get(key, {}).get('status', 'skipped') != 'skipped']
        stopped = [key for key in ordered if key != check_id and key in entry['results']
                   and entry['results'][key]['status'] != 'pass' and tier(entry, key) in stopping]
        if later and ('P1' in stopping or not stopped):
            return f'{check_id} can no longer run: {later[0]}, later in the pair, already ran', (pass_id, later[0])
        if stopped:
            key = stopped[0]
            result = entry['results'][key]
            cause = skip_origin(result) if result['status'] == 'skipped' else (pass_id, key)
            # A skipped earlier slot stays skipped when its later witness
            # is retried. Dependents are blocked by that earlier slot itself.
            if cause and cause[0] == pass_id and cause[1] in ordered and ordered.index(cause[1]) > ordered.index(key):
                cause = pass_id, key
            return f'stop after the first failed or unavailable check: {key} stopped its chain', cause
        return None

    def dead_end(self, pass_id, check_id):
        """Return the reason and checkpoint cause, if known, for settling a pending P1 or P2 check.

P1 follows its pair's order (`order_block`). P2 is dead once a P0 check
anywhere in the run did not pass, or a P0 pair was released before its P0
checks finished: P0 can then never complete, and a live send from a release
that failed P0 adds nothing. Lost P0 evidence is not a dead end; it refuses
the send instead (`chain_block`), since it calls for the maintainer.
"""
        entry = self.selected(pass_id)
        priority = tier(entry, check_id)
        if priority == 'P1':
            return self.order_block(pass_id, check_id, ('P0', 'P1'))
        if priority != 'P2':
            return None
        for key in self.data['plan']['p0_sequence']:
            pair = self.selected(key)
            part = priority_entry(pair, 'P0')
            failed = next((check for check, result in part['results'].items() if result['status'] != 'pass'), None)
            if failed is not None:
                return f'a P0 check of {key} did not pass', (key, failed)
            if pair['phase'] == 'deleted':
                missing = next((check for check in part['required'] if check not in part['results']), None)
                if missing is not None:
                    return f'{key} was released before its P0 checks finished', (key, missing)
                failure = pair.get('automation_failure', {}).get('check_id')
                if failure in {*part['required'], 'native-preflight'}:
                    return f'{key} was released after a P0 fixture or automation failure', (key, failure)
                if pair.get('preparation_error'):
                    return f'{key} was released after a P0 fixture or automation failure', None
        return None

    def chain_block(self, pass_id, check_id, status):
        """Say why a P0, P1 or P2 checkpoint cannot be recorded now, or None when it can.

P2 waits for every P0 check of the run to pass with retained evidence, and for
the pair's P1 sequence to be over. P0 and P1 follow their pair's order
(`order_block`), where a P0 result stops only P0 and a P1 one stops P1. A pass
needs every prerequisite passed; a P0 failure may still be recorded out of
order, while P1 always needs its prerequisites. Bonus checks are fenced by
their pass instead.
"""
        entry = self.selected(pass_id)
        priority = tier(entry, check_id)
        if priority == 'P2':
            if not self.p0_executable_complete():
                return 'complete all P0 checks with valid evidence before P2'
            pending = [key for key in priority_entry(entry, 'P1')['required'] if key not in entry['results']]
            if 'P1' in self.scope_tiers() and pending:
                return f'finish the P1 sequence before P2: {pending[0]} has no result yet'
            return None
        if priority not in ('P0', 'P1'):
            return None
        block = self.order_block(pass_id, check_id, ('P0',) if priority == 'P0' else ('P0', 'P1'))
        if block:
            return block[0]
        earlier = self.prerequisites(entry, check_id)
        if (status == 'pass' or priority == 'P1') and any(
                entry['results'].get(key, {}).get('status') != 'pass' for key in earlier):
            return ('complete preceding P0 checks first' if priority == 'P0'
                    else 'complete preceding P0 and P1 checks of this pair first')
        return None

    def continue_full(self):
        """Restore requested full coverage after a temporary scope stop.

The caller must have authorization to widen scope, including live feedback.
Recheck setup consent and keep every observation and active-attempt fence;
this transition neither retries a send nor recreates a cleaned fixture.
"""
        if not self.data['plan'] or not self.data['plan'].get('p0_sequence'):
            raise Refused('this historical run has no explicit P0 plan')
        if self.data.get('gui_backend') == 'guest-native':
            require_setup(self.profile, feedback=True)
        self.data['scope'] = 'full'
        self.save()
        self.note('decision', 'Full coverage requested again; retained observations and send boundaries unchanged.')
        self.settle()

    def p0_executable_complete(self):
        """Require every P0 check of the run passed with retained evidence, before feedback or bonus coverage."""
        return all(self.verdict(priority_entry(self.selected(key), 'P0')) == 'pass'
                   for key in self.data['plan']['p0_sequence'])

    def planned_discovery_skip(self):
        """Explain why a prerelease run planned no in-app updater pair, or return None.

The updater only discovers stable releases, so the plan leaves that pair out
of an RC or dev run instead of cloning one only to skip its check. The formal
P0 verdict still counts discovery as unavailable.
"""
        if not self.data['plan'].get('p0_sequence') or 'p0-updater-upgrade' in self.data['plan']['p0_sequence']:
            return None
        candidate = self.data['plan']['candidate']
        kind = version(candidate)[4]
        if not kind:
            return None
        label = {'rc': 'release candidate', 'dev': 'development prerelease'}[kind]
        return (f'In-app update discovery was skipped as expected. {candidate} is a {label}. '
                'The in-app updater only discovers stable releases, so this run planned no updater pair. '
                f'The public update channel pointed to {self.data["plan"]["latest_at_plan"]} when it was planned.')

    def require_priority(self, pass_id):
        """Check tier and pair ordering before any guest is cloned.

P0 fixtures run in their planned order but do not depend on each other: a
failed fresh install or discovery says nothing about whether the supervisors
upgrade, so the next P0 pair may still run once the failed one is released
(which needs the maintainer's retention decision). The remote Mac fixture
needs P1 in scope and the P0 pairs released, including the installer pair.
Its result is independent of their results. When the remote Mac pass is planned, it finishes
and releases its guests before any bonus pair. Historical runs without an
explicit P0 plan keep their original
obligations; their observations are never retroactively relabeled as P0.
"""
        entry = self.selected(pass_id)
        self.require_no_failure()
        sequence = self.data['plan']['p0_sequence']
        if entry.get('priority') == 'P0':
            if any(self.selected(key)['phase'] == 'pending' for key in sequence[:sequence.index(pass_id)]):
                raise Refused('run the P0 fixtures in their planned order')
            return
        if pass_id == REMOTE_MAC_PASS:
            if 'P1' not in self.scope_tiers():
                raise Refused(f'{self.scope_label()}: the remote Mac fixture is outside this run scope')
            if any(self.selected(key)['phase'] != 'deleted' for key in sequence):
                raise Refused('finish the P0 sequence and clean up the installer pair before the remote Mac fixture')
            return
        if self.data.get('scope', 'full') != 'full':
            raise Refused(f'{self.scope_label()}: bonus fixtures are outside this run scope')
        if not self.p0_executable_complete():
            raise Refused('complete P0 fixtures with valid evidence before bonus work')
        if REMOTE_MAC_PASS in self.data['passes'] and self.selected(REMOTE_MAC_PASS)['phase'] != 'deleted':
            raise Refused('finish and clean up the remote Mac fixture before bonus work')

    def result_evidence_retained(self, result):
        """Recheck accepted bytes independently of the observation's status.

A failed or unavailable P1 or P2 observation still needs its exported evidence
before cleanup can delete the guests. Keep the same artifact limits as verdict checks,
including the larger bound for the guest-native event stream.
"""
        try:
            for ref, digest in result['sha256'].items():
                raw = (artifact(self.directory, ref, limit=96 * 1024 * 1024)
                       if ref == result.get('native', {}).get('events') else artifact(self.directory, ref))
                if hashlib.sha256(raw).hexdigest() != digest:
                    return False
        except (OSError, ValueError, RuntimeError, KeyError, tarfile.TarError):
            return False
        return True

    def retry_evidence_retained(self, entry, checks):
        """Verify the retry archives for these checkpoints and the pass-wide preflight."""
        judged = set(checks) | {'native-preflight'}
        if any(not self.result_evidence_retained(row) for row in entry.get('provider_retries', [])
               if row.get('check_id') in judged):
            return False
        if any(not self.result_evidence_retained(row) for row in entry.get('readiness_retries', [])
               if row.get('tester', {}).get('check_id') in judged):
            return False
        if any(not self.result_evidence_retained(row['result']) for row in entry.get('checkpoint_retries', [])
               if row.get('tester', {}).get('check_id') in judged):
            return False
        return True

    def native_preflight_retained(self, entry):
        """A native fixture keeps the readiness receipts even if its product checks could not run."""
        if self.data.get('gui_backend') == 'guest-native':
            preflight = entry.get('native_preflight', {})
            return bool(preflight.get('sha256')) and self.result_evidence_retained(preflight)
        return True

    def check_reopen_transition(self, pass_id, references, native, samples, contents):
        """Bind the reopen witness to the attempt and counter receipts the host is accepting.

Import passes the exact bytes it hashes. A retained verdict repeats the check
against the saved task and witness after checking their recorded hashes.
Historical host-GUI records have a run and pass but no guest attempt ID.
"""
        def read(ref):
            """Reuse import's bounded artifact reads when they are available."""
            return json.loads(contents[ref] if ref in contents else artifact(self.directory, ref))

        try:
            attempt = read(native['task'])['attempt'] if self.data.get('gui_backend') == 'guest-native' else None
            identity = {'attempt': attempt, 'run_id': self.data['run_id'], 'pass_id': pass_id, 'check_id': 'reopen'}
            require_reopen_transition(read(reopen_transition_ref(references)), identity, *samples)
        except (KeyError, TypeError, ValueError) as error:
            raise Refused('reopen transition evidence is invalid: ' + str(error)) from error

    def verdict(self, entry):
        """Missing, invalidated or skipped checks cannot become a release pass.

A recorded product failure decides first: a later harness failure or lost
retry evidence must not turn a confirmed break into could not run. Retry
history counts only for the checks being judged and for the pass-wide preflight.
"""
        results = entry['results']
        if any(v['status'] == 'fail' for v in results.values()):
            return 'fail'
        if not self.retry_evidence_retained(entry, entry['required']):
            return 'could not run'
        failure = entry.get('automation_failure', {})
        if failure.get('check_id') == 'native-preflight' or failure.get('check_id') in entry['required']:
            return 'could not run' if failure.get('status') == 'could-not-run' else 'fail'
        if entry.get('preparation_error') or set(results) != set(entry['required']):
            return 'could not run'
        if not self.native_preflight_retained(entry):
            return 'could not run'
        for check_id, result in results.items():
            if result['status'] != 'pass':
                return 'could not run'
            if not self.result_evidence_retained(result):
                return 'could not run'
            try:
                if 'continuity' in result or check_id == 'reopen':
                    samples = continuity_chain(self.directory, entry, check_id)
                    if check_id == 'reopen':
                        self.check_reopen_transition(entry['id'], result['evidence'], result.get('native', {}),
                                                     samples, {})
            except (OSError, ValueError, RuntimeError, KeyError, TypeError, tarfile.TarError):
                return 'could not run'
        return 'pass'

    def status(self):
        """Report the run and save it privately as `report.json`."""
        report = self.report()
        atomic_json(self.directory / 'report.json', report)
        return report

    def named_checks(self):
        """Map each tier's named results, in report order, onto the checkpoints answering them.

Each entry is (label, pass, checkpoints), where None means all of that pass's
checkpoints in the tier. The installer pair answers the installer upgrade,
both supervisor upgrades and both new agents for P0, its old sessions and
remote Linux menu for P1 (each old-session result folds in `old-sessions`,
which started that session), and feedback for P2. New layouts additionally
summarize the dedicated Mac pass as one P1 result. A planned pass that is absent
reports as skipped: a prerelease plans no updater pair. A historical run
without P0 fixtures names only the P1 checks it declared.
"""
        if not (self.data['plan'] and self.data['plan'].get('p0_sequence')):
            return {'P0': [], 'P1': [(check, pass_id, [check]) for pass_id, entry in self.data['passes'].items()
                                     for check in priority_entry(entry, 'P1')['required']], 'P2': []}
        installer = 'p0-installer-upgrade'
        results = {
            'P0': [('fresh-install-start', 'p0-fresh', None),
                   ('click-to-upgrade', 'p0-updater-upgrade', None),
                   ('installer-upgrade', installer, ['artifact-verification', 'old-fixture', 'update-relaunch']),
                   ('local-supervisor-upgrade', installer, ['local-supervisor-upgrade']),
                   ('remote-supervisor-upgrade', installer, ['remote-supervisor-upgrade']),
                   ('local-agent', installer, ['local-supervisor-agent']),
                   ('remote-agent', installer, ['remote-supervisor-agent'])],
            'P1': [('local-old-session', installer, ['old-sessions', 'local-old-session']),
                   ('remote-old-session', installer, ['old-sessions', 'remote-old-session']),
                   ('remote-host-remove', installer, ['remote-host-remove']),
                   ('remote-host-uninstall', installer, ['remote-host-uninstall'])],
            'P2': [('feedback-send', installer, ['p2-feedback-send'])]}
        if 'p2-announcements' in self.selected(installer)['required']:
            results['P2'].append(('announcements', installer, ['p2-announcements']))
        if REMOTE_MAC_PASS in self.data['passes']:
            results['P1'].append(('remote-mac-uninstall', REMOTE_MAC_PASS, None))
        return results

    def report(self):
        """Report P0, P1, P2, requested scope and full coverage independently.

Every answer rechecks retained bytes. Legacy observations and fixture smoke
remain distinguishable from the declared P0 product checks; unfinished
bonus work cannot erase a P0 pass or become a full acceptance pass. Each tier
is part of its fixture's full verdict but excluded from the other tiers'
projections.
"""
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
        if self.data.get('gui_backend') == 'guest-native':
            failures = {key: entry['automation_failure'] for key, entry in self.data['passes'].items()
                        if entry.get('automation_failure', {}).get('kind') == 'harness-failure'}
            retries = {key: entry['provider_retries'] for key, entry in self.data['passes'].items()
                       if entry.get('provider_retries')}
            readiness = {key: entry['readiness_retries'] for key, entry in self.data['passes'].items()
                         if entry.get('readiness_retries')}
            repaired = {key: entry['checkpoint_retries'] for key, entry in self.data['passes'].items()
                        if entry.get('checkpoint_retries')}
            output['automation'] = {'verdict': ('fail' if failures else 'recovered readiness failure'
                                               if readiness else 'recovered fixture prerequisite'
                                               if repaired else 'recovered provider interruption'
                                               if retries else 'no failure observed'),
                                    'failures': failures, 'provider_retries': retries, 'readiness_retries': readiness,
                                    'checkpoint_retries': repaired}
            # An aborted turn cannot prove a product failure, but losing turn
            # completion must not relabel its observed break as harmless setup.
            # A P1 harness failure the pair went on from keeps its claim in
            # `unverified_failures`. `old-sessions` runs on the previous
            # release before any candidate code, so a failure claimed there is
            # never a candidate failure.
            claims = [{'pass_id': key, 'check_id': entry['tester']['check_id'],
                       'detail': entry['tester']['observation']['detail']}
                      for key, entry in self.data['passes'].items()
                      if entry.get('tester', {}).get('check_id') not in ('native-preflight', 'old-sessions')
                      and (entry.get('tester', {}).get('observation') or {}).get('status') == 'fail']
            claims += [{'pass_id': key, 'check_id': claim['identity']['check_id'], 'detail': claim['detail']}
                       for key, entry in self.data['passes'].items() for claim in entry.get('unverified_failures', [])]
            output['unverified_product_failures'] = claims
            output['promotion_blocked_pending_review'] = bool(claims)
        if self.data['kind'] == 'release':
            p0_ids = self.data['plan'].get('p0_sequence', [])
            output['scope'] = self.data.get('scope', 'full')
            questions = self.named_checks()
            # Each tier's verdict is its own projection of the saved checks, so
            # a failed feedback send or lost session never erases a passing
            # upgrade, and a passing tier never borrows another tier's result.
            for priority in TIERS:
                entries = [part for part in (priority_entry(entry, priority) for entry in self.data['passes'].values())
                           if part['required']]
                block = {'verdict': combine([self.verdict(part) for part in entries]) if entries else 'not planned'}
                if priority == 'P0':
                    block['passes'] = p0_ids
                if questions[priority]:
                    block['checks'] = {}
                    for label, pass_id, keys in questions[priority]:
                        if pass_id not in self.data['passes']:
                            block['checks'][label] = 'skipped'
                            continue
                        part = priority_entry(self.selected(pass_id), priority)
                        if keys is not None:
                            part = subset(part, [key for key in keys if key in part['required']])
                        block['checks'][label] = self.verdict(part) if part['required'] else 'not planned'
                elif priority != 'P0':
                    block['checks'] = {}
                if self.data.get('gui_backend') == 'guest-native':
                    block['unverified_product_failures'] = [
                        value for value in output['unverified_product_failures']
                        if value['check_id'] in self.selected(value['pass_id'])['required']
                        and tier(self.selected(value['pass_id']), value['check_id']) == priority]
                output[priority.lower()] = block
            # A skipped discovery leaves the formal P0 verdict incomplete even
            # when every planned P0 check passed; the summary says so instead.
            discovery = self.planned_discovery_skip()
            if discovery:
                output['p0']['unavailable_checks'] = {'update-discovery': discovery}
                if output['p0']['verdict'] == 'pass':
                    output['p0']['verdict'] = 'could not run'
                if self.p0_executable_complete():
                    output['p0']['summary'] = f'All runnable P0 checks passed. {discovery}'
                if outcome == 'pass':
                    outcome = 'could not run'
                    output['verdict'] = outcome
            if output['scope'] == 'full':
                output['requested_verdict'] = outcome
            else:
                output['requested_verdict'] = combine([output[t.lower()]['verdict'] for t in SCOPE_TIERS[output['scope']]])
            candidate = self.data['plan']['candidate']
            observations = [self.data['plan']['latest_at_plan']] + [v.get('latest_at_selection')
                            for key, v in self.data['passes'].items() if key.startswith('upgrade-') or
                            key in ('p0-installer-upgrade', 'p0-updater-upgrade')]
            output['promotion_coverage'] = ('after-publication' if candidate in observations else
                                            'pending' if None in observations else 'before-public-default')
        return output

    def cleanup(self, pass_id, discard_failed=False):
        """Clean a validated pair while preserving P1's and P2's nonblocking results.

Evidence is rechecked first. Clean guest shutdown remains mandatory, and each
role's low-level manifest must prove unchanged ownership and observed absence.
A tiered pair needs any P0 checks passed, no automation failure, and a result
for every in-scope P1 and P2 checkpoint (a recorded skip counts). A pair that
fell out of scope before testing can be cleaned up without native preflight.
Any recorded test activity keeps that receipt required. Every P1 and P2
result keeps its exported evidence, and a failed or unavailable one has some:
its exported observation is what the maintainer reviews, so it needs no live
disposable pair, and holding the pair would stall the bonus work the failure
does not fence. Other incomplete or failed pairs need explicit discard
approval as before; none of these paths delete retained evidence.
"""
        entry = self.selected(pass_id)
        if entry.get('tester', {}).get('phase') in ('running', 'reconciliation-required'):
            raise Refused('cannot clean up while the independent guest tester is active')
        self.settle()
        if entry.get('priority') in ('P0', 'P1') and self.data['plan'].get('p0_sequence'):
            empty = hashlib.sha256(b'').hexdigest()
            later = [(key, tier(entry, key)) for key in entry['required'] if tier(entry, key) in ('P1', 'P2')]
            results = [entry['results'][key] for key, _ in later if key in entry['results']]
            p0 = priority_entry(entry, 'P0')
            in_scope = [key for key, priority in later if priority in self.scope_tiers()]
            needs_preflight = (p0['required'] or in_scope or entry['results']
                               or entry.get('native_preflight') or entry.get('tester'))
            complete = (entry['phase'] == 'ready' and not entry.get('preparation_error')
                        and not entry.get('automation_failure')
                        and (not p0['required'] or self.verdict(p0) == 'pass')
                        and all(key in entry['results'] for key in in_scope)
                        and (not needs_preflight or self.native_preflight_retained(entry))
                        and self.retry_evidence_retained(entry, set(in_scope) | entry['results'].keys())
                        and all(self.result_evidence_retained(result) for result in results)
                        and all(result['status'] in ('pass', 'skipped')
                                or any(digest != empty for digest in result['sha256'].values()) for result in results))
        else:
            complete = self.verdict(entry) == 'pass'
        if not complete and not discard_failed:
            raise Refused('pass is incomplete or failed; ask about retention before --discard-failed')
        c = self.pair(pass_id)
        with c.locked():
            for role in pass_roles(pass_id):
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
        child = sub.add_parser(name)
        child.add_argument('candidate')
        if name == 'init':
            scope = child.add_mutually_exclusive_group()
            scope.add_argument('--stop-after-p0', action='store_true',
                               help='complete the P0 checks only: no old sessions, feedback or bonus fixtures')
            scope.add_argument('--stop-after-p1', action='store_true',
                               help='complete P0 and the P1 continuity checks, without the feedback send')
            scope.add_argument('--stop-after-p2', action='store_true',
                               help='complete P0, P1 and both P2 checks, then refuse bonus fixtures')
    sub.add_parser('smoke')
    scope_commands = ('stop-after-p0', 'stop-after-p1', 'stop-after-p2')
    for name in ('serve', 'test', 'retry-readiness', 'retry-unavailable', 'reconcile-tester', 'status', 'select-update',
                 'record', 'add-check', 'cleanup', *scope_commands, 'continue-full'):
        child = sub.add_parser(name)
        child.add_argument('run_dir', type=Path)
        if name not in ('status', *scope_commands, 'continue-full'):
            child.add_argument('pass_id')
        if name == 'serve':
            child.add_argument('--headless', action='store_true')
        elif name == 'test':
            child.add_argument('--timeout', type=int, default=1200,
                               help='base checkpoint budget in seconds, shared with retries')
        elif name == 'retry-unavailable':
            child.add_argument('--reason', required=True, help='fixture repair and why repeating the checkpoint is safe')
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
        scope = 'full'
        if args.action == 'init':
            scope = ('p0-only' if args.stop_after_p0 else 'p1-only' if args.stop_after_p1
                     else 'p2-only' if args.stop_after_p2 else 'full')
            # Only scopes that reach P2 include the live feedback send.
            require_setup(profile, feedback='P2' in SCOPE_TIERS[scope])
        root = Path(profile.values['evidence_root'])
        root.mkdir(mode=0o700, parents=True, exist_ok=True)
        run = Run(root / ('brick-' + str(uuid.uuid4())), profile)
        run.create(release if args.action == 'init' else None,
                   inputs if args.action == 'init' else None, smoke=args.action == 'smoke', scope=scope)
        if args.action == 'smoke':
            smoke(run)
        else:
            with run.locked():
                print(json.dumps(run.report(), indent=2))
        return
    run = Run(args.run_dir, profile)
    if args.action in ('retry-readiness', 'retry-unavailable'):
        if args.action == 'retry-readiness':
            run.retry_readiness(args.pass_id)
        else:
            run.retry_unavailable(args.pass_id, args.reason)
        with run.locked():
            print(json.dumps(run.report(), indent=2))
        return
    if args.action == 'test':
        run.test(args.pass_id, timeout=args.timeout)
        with run.locked():
            print(json.dumps(run.report(), indent=2))
        return
    if args.action == 'reconcile-tester':
        run.reconcile_tester(args.pass_id)
        with run.locked():
            print(json.dumps(run.report(), indent=2))
        return
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
        elif args.action in scope_commands:
            run.limit_scope(args.action.removeprefix('stop-after-') + '-only')
        elif args.action == 'continue-full':
            run.continue_full()
        report = run.status() if args.action == 'status' else run.report()
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, KeyError, tarfile.TarError, subprocess.SubprocessError) as error:
        print(f'brick test refused/failed: {error}', file=sys.stderr)
        sys.exit(1)
