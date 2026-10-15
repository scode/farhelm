"""Host-side wiring for an independent, unattended guest-native test driver.

The host owns credentials, Tart lifecycle and retained evidence; the guest owns
product commands and UI. No function here drives a host window or grants a
guest permission. Setup readiness is explicit, and a permission pause after
that boundary fails the automation rather than asking an operator to continue.
"""

import json
import os
from pathlib import Path
import re
import stat
import subprocess
import uuid
from urllib.parse import urlparse

from control import Refused
from guest_tester import request


HERE = Path(__file__).resolve().parent


class Unavailable(Refused):
    """A declared product check cannot run on this release path or substrate."""


class CheckStopped(Refused):
    """A valid guest observation stopped the sequence and was already recorded."""


class CleanupPending(Refused):
    """Guest termination was not acknowledged; retain the active cleanup fence."""


def private_json(path):
    """Read a small operator-owned record without admitting public or redirected files."""
    path = Path(path)
    if path.is_symlink() or not path.is_file() or path.stat().st_mode & 0o077 or path.stat().st_size > 65536:
        raise Refused('setup record must be a private regular file <=64 KiB')
    return json.loads(path.read_text())


def require_setup(profile, feedback=False):
    """Fence actual tests on recorded credential consent and clean native fixtures.

These records are operator attestations, not cryptographic certificates. Their
base identities must match the profile, and runtime preflight still exercises
the actual clone. Credential-copy consent does not authorize P1's live feedback
send; that separate consent is required before any in-scope run starts.
Historical profiles remain usable for status and cleanup.
"""
    config = profile.values.get('guest_tester')
    if config is None:
        raise Refused('guest-native fixture setup required: configure profile format 2')
    authorization = private_json(config['authorization'])
    if (authorization.get('format') != 1 or authorization.get('approved') is not True
            or authorization.get('scope') != 'disposable-test-clones'
            or authorization.get('destinations') != ['mac', 'linux']
            or authorization.get('auth_source') != config['auth_source']):
        raise Refused('fixture setup lacks standing consent for this credential and both disposable roles')
    if feedback and authorization.get('feedback_send_approved') is not True:
        raise Unavailable('fixture setup lacks explicit consent for the fixed live P1 feedback send')
    receipt = private_json(config['fixture_receipt'])
    expected = {role: profile.values[role + '_base'] for role in ('mac', 'linux')}
    obligations = {'native-screenshot-click-type', 'native-after-reopen', 'clone-permission-carryover',
                   'auth-not-in-bases', 'farhelm-clean', 'linux-codex-ready'}
    if (receipt.get('format') != 1 or receipt.get('bases') != expected
            or not isinstance(receipt.get('checks'), dict)
            or any(receipt['checks'].get(key) is not True for key in obligations)
            or receipt.get('settings') != config['settings']):
        raise Refused('guest-native fixture receipt is incomplete or names different bases/runtime')
    return config


def copy_credentials(control, config):
    """Inject only the explicitly authorized file through stdin into owned working clones.

Prepared images must have no refreshed login. Therefore initial injection is a
prerequisite, not a reactive permission workaround. Neither argv nor journal
contains the bytes; a single immutable read feeds both intended destinations.
"""
    source = Path(config['auth_source'])
    descriptor = os.open(source, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        info = os.fstat(descriptor)
        if not stat.S_ISREG(info.st_mode) or info.st_mode & 0o077 or not 0 < info.st_size <= 1024 * 1024:
            raise Refused('authorized authentication source must be a bounded private regular file')
        with os.fdopen(descriptor, 'rb', closefd=False) as stream:
            content = stream.read(1024 * 1024 + 1)
        if len(content) > 1024 * 1024:
            raise Refused('authorized authentication source grew past its bound')
    finally:
        os.close(descriptor)
    script = ('from pathlib import Path; import os,sys; '
              'd=Path.home()/".codex"; '
              'assert not d.is_symlink(), "redirected auth directory"; '
              'd.mkdir(mode=0o700,exist_ok=True); '
              'p=d/"auth.json"; assert not p.exists() and not p.is_symlink(), "fixture contains authentication"; '
              'fd=os.open(p,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600); '
              's=os.fdopen(fd,"wb"); s.write(sys.stdin.buffer.read()); s.flush(); os.fsync(s.fileno()); s.close(); '
              'assert p.stat().st_mode & 0o777 == 0o600')
    for role in ('mac', 'linux'):
        python = config['settings']['python'] if role == 'mac' else '/usr/bin/python3'
        control.mutate(['exec', '-i', control.owned(role), python, '-c', script],
                       'Inject authorized Codex authentication into this disposable guest only; stdin excluded.',
                       input=content, capture_output=True)


def source_url(source):
    """Use a public HTTPS checkout without passing host Git credentials into the guest."""
    value = source.git('remote', 'get-url', 'origin').strip()
    match = re.fullmatch(r'git@github\.com:([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)', value)
    if match:
        value = 'https://github.com/' + match[1]
    parsed = urlparse(value)
    if (parsed.scheme != 'https' or parsed.username or parsed.password or parsed.query or parsed.fragment
            or parsed.hostname != 'github.com' or not re.fullmatch(r'/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', parsed.path)):
        raise Refused('guest source checkout needs a credential-free public GitHub HTTPS origin')
    return value


def instructions(check_id, pass_id, update_path=None):
    """Keep P0 and P1's declared contracts distinct from bonus acceptance work.

Release metadata, source commits, paths and SSH alias are supplied separately.
The recipe remains authoritative for broader checks, and the guest retains its
small shared observation record so fresh tester turns need no huge transcript.
Bonus upgrades combine discovery and relaunch; P0 schedules them separately.
"""
    contracts = {
        'p1-feedback-send': '''This is the separately authorized P1 check after all P0 checks passed.
Require context feedback_authorized true before opening Send; otherwise report
could-not-run without sending. Read RECIPE.md's P1 contract. Through native CUA
send its exact two-paragraph brick-test message with this candidate tag to the
normal farhelm.io feedback service; leave contact blank. Verify the dialog names
the candidate version and desktop surface. Retain actual native images of the
filled dialog before Send and closed dialog after success (or failure state).
Pass only on observed successful closure; no command success receipt is needed.
Do not send another message or inspect the private inbox. A failed product send
is a P1 failure, independent from P0; finish this observation immediately.''',
        'artifact-verification': '''Clone the context source_url into source_checkout if absent, fetch the previous
tag, and require git rev-parse previous^{commit} equals previous_commit. From that
checkout run context verification_helper with previous, candidate and output
signed_inputs. Copy verification.json to this evidence directory and retain it.
Verify against the previous tag's compiled keys, never candidate/main keys. Do
not install Farhelm in this step.''',
        'fixture-clean': '''Run the supplied context fixture_probe with context guest_python:
--role mac --ssh-alias context.ssh.alias --output fixture-clean.json.
Use the absolute helper path and actual ssh.alias value from context, not the
field names as literal arguments.
It performs read-only Mac and pinned Linux inspection, including known product
paths, processes and service managers. Require exit0, clean true and exactly
one Mac and one Linux observation, both clean true; retain fixture-clean.json.
Do not write replacement probes or search source for cleanliness paths. The
helper handles systemd's expected empty no-match exit explicitly; an unknown
failure still stops. Dirty fixtures or helper/transport errors are could-not-run,
never a Farhelm product fail; an explicit OS/authentication refusal is
permission-block. Do not delete state to manufacture a clean premise.''',
        'install': '''Use signed_inputs/install.sh with FARHELM_VERSION=candidate and
FARHELM_INSTALL_SUMS_FILE=signed_inputs/SHA256SUMS. Retain exact installed CLI
version and installer output. Install only inside Mac; no services on host.''',
        'app-start': '''Launch the actual installed ~/Applications/Farhelm.app with guest open and named
stdout/stderr capture. Through native CUA require candidate app version, usable
session list and connected local supervisor at candidate. Retain native image
and independent installed/running version evidence. No remote provisioning,
conversation, templates, settings or other product checks in this fixture.''',
        'old-fixture': '''Install previous from its published version path, verifying its signature and
installer checksum with source-known keys. Disable automatic app updates in the
guest's desktop-client.json BEFORE first launch, only if no Farhelm state exists.
Launch the old installed app normally, verify previous app/local supervisor and
disabled auto updates. Only for p0-updater-upgrade or a bonus upgrade fixture,
provision the pinned SSH Linux host through the old app's normal native Add host
GUI and require its old remote version. p0-installer-upgrade needs no remote
product setup or conversation. Retain native image and command/version receipts.''',
        'update-discovery': '''Use the freshly selected context update_path. It must be ordinary or override;
manual cannot satisfy discovery. For override, normally quit the old app and
guest-open it with FARHELM_DESKTOP_UPDATE_LATEST=candidate, retaining the startup
announcement. Click the old app's actual Check for updates, then its offered
install action. Require discovery and installation of candidate, with actual
native evidence. Never run a manual installer to conceal failed discovery.''',
        'update-relaunch': '''For manual path only, run the verified candidate installer while the old app
runs, setting BOTH FARHELM_VERSION and FARHELM_INSTALL_SUMS_FILE. In all paths
wait on the old app's real Restart to update offer, click it and require the
candidate app to return usable. Do NOT open the candidate manually if its helper
fails. For override path prove startup forwarding and removal after relaunch
using the public latest observation and new app update hover; report an
indistinguishable endpoint as a gap. Retain native image and version/command
receipts. Do not require the remote supervisor upgraded in this checkpoint.''',
        'local-supervisor-agent': '''Require connected candidate local supervisor; its managed relaunch normally
upgrades it automatically. Use local Update if the product offers it. Through
native Farhelm GUI launch ONE genuine Codex session on local, model gpt-6.1-sol,
low effort, yolo, per-run trust true. Send: What is 17 + 25? Explain in one short
sentence. Require separate actual answer 42 with sensible explanation, terminal
responsiveness and idle/completion. Retain native image and candidate running
version evidence. No resume/settings/template prerequisites.''',
        'remote-supervisor-agent': '''Use native Farhelm remote Update. Require the formerly old remote supervisor
connected at candidate, independently verify its running systemd user service
over pinned SSH. Launch ONE genuine Codex session through Farhelm on that remote
host, gpt-6.1-sol low effort yolo/trust true where supported. Send: What is 17 +
25? Explain in one short sentence. Require actual answer 42, explanation,
responsive terminal and completion. A plain command is insufficient. Retain
native image plus remote version/service evidence.''',
        'native-preflight': '''Before any Farhelm installation or product testing, use real native CUA to
bind Finder, obtain actual native screenshot/AX, and perform an ordinary Finder
click and nonsecret text-entry operation. Require all native operations usable
with NO OS/CUA/login/setup prompts. Retain a real returned native image plus a
small command receipt proving virtualized Mac and runtime versions. Through
pinned SSH verify Linux Codex is executable and its injected auth file is mode
0600 without reading/printing it. Do not create product state or grant anything.
Any prerequisite refusal is permission-block/automation failure, never a pass.''',
    }
    if check_id in contracts:
        text = contracts[check_id]
        # Auto updates are off in every old fixture. Bonus has no separate
        # discovery checkpoint, so it must initiate the selected native update
        # before waiting for Restart; P0 already completed that action earlier.
        if check_id == 'update-relaunch' and pass_id.startswith('upgrade-') and update_path in ('ordinary', 'override'):
            text = contracts['update-discovery'] + '\nThen complete the relaunch:\n' + text
        if check_id in ('app-start', 'old-fixture', 'update-relaunch'):
            text += ('\nIf the ordinary welcome card appears, dismiss it with got it before testing the session UI. '
                     'Do not select Send feedback now or begin P1 early.')
        if check_id == 'update-relaunch' and pass_id == 'p0-installer-upgrade':
            text += ('\nThis installer fixture also requires its local supervisor connected at candidate, '
                     'with independent running-version evidence; there is no later supervisor checkpoint here.')
        return text
    return (f'Perform RECIPE.md checkpoint {check_id} for {pass_id}, and read its matching '
            'acceptance contract and the staged filled addendum.md. The context check_contracts '
            'contains all declared obligations; extra checks use the addendum\'s corresponding '
            'action, expected result and evidence contract. Missing instructions are could-not-run. '
            'Keep shared_state.json current with only explicit session, '
            'conversation, template, setting and continuity observations needed by later '
            'checkpoints; never save credentials. Preserve the original Linux process for '
            'continuity and retain independent before/after receipts as local files. '
            'Do not replace skipped or unavailable bonus checks with a pass.')


def stage(control, task, source_files):
    """Install a single exclusive executor/task tree by journaled stdin transfer.

The input contains public scripts and private noncredential context. Credentials
use a different explicitly authorized operation and cannot enter this bundle.
"""
    request(task)
    payload = json.dumps({'task': task, 'files': source_files}).encode()
    script = ('import json,sys,os; from pathlib import Path; os.umask(0o077); '
              'v=json.load(sys.stdin); t=v["task"]; '
              'd=Path.home()/"farhelm-brick-test"/t["run_id"]/t["pass_id"]/t["attempt"]; '
              'd.mkdir(mode=0o700,parents=True,exist_ok=False); '
              '(d/"task.json").write_text(json.dumps(t)); '
              '[(d/k).write_text(s) for k,s in v["files"].items()]; print(d)')
    result = control.mutate(['exec', '-i', control.owned('mac'), task['settings']['python'], '-c', script],
                            'Stage independent guest-native tester and one fresh checkpoint task; no credentials.',
                            input=payload, capture_output=True)
    return result.stdout.decode().strip()


def execute(control, task, source_files):
    """Run and export one attempt, retaining model/tool failures as failed observations.

The guest owns its child group deadline. Host Tart timeout is longer so ordinary
deadline cleanup can finish and produce diagnostics. Export never means pass:
the coordinator separately validates the structured result and native stream.
"""
    directory = stage(control, task, source_files)
    try:
        control.mutate(['exec', control.owned('mac'), task['settings']['python'],
                        directory + '/guest_tester.py', '--task', directory + '/task.json',
                        '--output', directory + '/output'],
                       'Run the guest-only tester; OS/CUA approval blocks fail this attempt.',
                       timeout=task['timeout_seconds'] + 30, capture_output=True)
    except subprocess.TimeoutExpired:
        cancel(control, task, directory)
    except subprocess.SubprocessError:
        # A nonzero guest exit is evidence to import, not permission to retry or
        # discard it. Missing export/summary remains an explicit harness failure.
        pass
    except BaseException:
        cancel(control, task, directory)
        raise
    label = 'tester-' + task['attempt']
    probe = ('import json;from pathlib import Path; '
             'p=Path(' + repr(directory) + ')/"executor-state.json"; '
             'print(p.read_text() if p.is_file() else "null")')
    try:
        observed = json.loads(control.command('exec', control.owned('mac'), task['settings']['python'],
                                              '-c', probe, capture_output=True, timeout=10).stdout)
        if observed != {'attempt': task['attempt'], 'phase': 'stopped'}:
            raise CleanupPending('guest tester termination was not acknowledged')
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        raise CleanupPending('guest tester termination observation failed') from error
    control.collect('mac', directory + '/output/export', label)
    return label


def cancel(control, task, directory):
    """Stop a remote attempt by its exclusive lease, never a remembered numeric PID.

The executor observes this marker while retaining its live Popen identity and
publishes stopped only after group cleanup. A missing acknowledgement leaves
the attempt failed and requires reconciliation; it cannot authorize a retry.
"""
    script = ('import json,time;from pathlib import Path; '
              'd=Path(' + repr(directory) + '); '
              '(d/"cancel.json").write_text(' + repr(json.dumps({'attempt': task['attempt']})) + '); '
              'deadline=time.monotonic()+20; p=d/"executor-state.json";\n'
              'while time.monotonic()<deadline:\n'
              ' if p.exists():\n'
              '  v=json.loads(p.read_text());\n'
              '  if v.get("attempt")==' + repr(task['attempt']) + ' and v.get("phase")=="stopped": break\n'
              ' time.sleep(0.1)\n'
              'else: raise RuntimeError("guest cancellation not acknowledged; reconcile before cleanup")')
    try:
        control.mutate(['exec', control.owned('mac'), task['settings']['python'], '-c', script],
                       'Cancel this exclusive guest attempt and await owned-child cleanup acknowledgement.',
                       capture_output=True, timeout=25)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        raise CleanupPending('guest cancellation was not acknowledged; retain cleanup fence') from error


def task_for(run, pass_id, check_id, context, timeout=1200):
    """Bind one checkpoint to a fresh model turn and a distinct retained attempt."""
    entry = run.selected(pass_id)
    gui = check_id == 'native-preflight' or entry['required'][check_id]['gui']
    attempt = str(uuid.uuid4())
    context = dict(context, verification_helper=context['pass_directory'] + '/' + attempt + '/verify_release.py',
                   fixture_probe=context['pass_directory'] + '/' + attempt + '/fixture_probe.py',
                   guest_python=run.profile.values['guest_tester']['settings']['python'])
    return request({'format': 1, 'attempt': attempt, 'run_id': run.data['run_id'],
                    'pass_id': pass_id, 'check_id': check_id, 'gui': gui, 'timeout_seconds': timeout,
                    'settings': run.profile.values['guest_tester']['settings'], 'context': context,
                    'instructions': instructions(check_id, pass_id, context.get('update_path'))})
