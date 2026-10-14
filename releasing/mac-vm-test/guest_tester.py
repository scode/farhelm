#!/usr/bin/env python3
"""Run the independent UI tester inside an already prepared Mac guest.

Fixture setup owns permissions and authentication. This executor never grants
either: an approval request during a product check is an automation failure,
even when the tester exits successfully. Its process group belongs to this one
attempt, independently of Farhelm and its supervisor, so app updates cannot
terminate the test driver and a deadline cannot leave that driver running.
"""

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import shutil
import subprocess
import time


MAX_INPUT = 1024 * 1024
MAX_LOG = 96 * 1024 * 1024
IDENTIFIER = re.compile(r'[a-z0-9][a-z0-9-]{0,100}')
UPGRADE_PASS = re.compile(r'upgrade-v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)')
RESULT_STATES = ('pass', 'fail', 'could-not-run', 'permission-block', 'automation-failure')
EXECUTOR_RECORDS = ('task.json', 'execution.json', 'events.jsonl', 'stderr.txt', 'result.json',
                    'model-result.json', 'prompt.txt', 'final.txt', 'executor-state.json', 'cancel.json')
PRIVATE_NAME_PARTS = ('auth', 'token', 'credential', 'keychain')


class TesterError(RuntimeError):
    """An attempt cannot supply valid, unattended guest-native evidence."""


def native_args(settings):
    """Use the official bundle's native provider rather than browser-only defaults.

CLI overrides leave the packaged plugin and global vendor configuration alone.
All paths refer to the guest; credentials never appear in subprocess argv.
Authentication is supplied separately by the authorized host preparation step.
"""
    resources = Path(settings['desktop_app']) / 'Contents/Resources/cua_node'
    modules = resources / 'lib/node_modules'
    env = {
        'CUA_REPL_ENABLED_SURFACES': 'computer',
        'CUA_REPL_NODE_REPL_PATH': str(resources / 'bin/node_repl'),
        'NODE_REPL_NODE_PATH': str(resources / 'bin/node'),
        'NODE_REPL_NODE_MODULE_DIRS': str(modules),
        'NODE_REPL_TRUSTED_CODE_PATHS': f'{Path.home() / ".codex"}:{modules}',
        'NODE_REPL_TRUSTED_SERVICES': json.dumps({'sky': '@oai/sky/service'}, separators=(',', ':')),
    }
    values = {
        'model_reasoning_effort': settings['effort'],
        'mcp_servers.cua_repl.command': str(resources / 'bin/node'),
        'mcp_servers.cua_repl.args': [str(modules / '@oai/cua-repl/bin/cua-repl.mjs')],
        **{'mcp_servers.cua_repl.env.' + key: value for key, value in env.items()},
    }
    return [settings['codex'], *[part for key, value in values.items()
                                for part in ('-c', key + '=' + json.dumps(value))]]


def request(value):
    """Validate the task envelope before it can name guest files or start a process.

The host supplies release context, not arbitrary executor paths. Separate
attempt identifiers prevent a retry from overwriting its predecessor's files.
"""
    required = {'format', 'attempt', 'run_id', 'pass_id', 'check_id', 'gui',
                'timeout_seconds', 'settings', 'context', 'instructions'}
    if not isinstance(value, dict) or set(value) != required or type(value['format']) is not int or value['format'] != 1:
        raise TesterError('unsupported guest task envelope')
    for key in ('attempt', 'run_id', 'pass_id', 'check_id'):
        # Only the planner's stable-baseline pass names contain dots. Keep
        # attempt/run/check identifiers unchanged instead of widening every
        # filesystem component's grammar to accommodate one phase's names.
        identifier = value[key]
        valid = isinstance(identifier, str) and len(identifier) <= 101 and (
            IDENTIFIER.fullmatch(identifier) or (key == 'pass_id' and UPGRADE_PASS.fullmatch(identifier)))
        if not valid:
            raise TesterError('unsafe guest task identifier')
    if type(value['gui']) is not bool or type(value['timeout_seconds']) is not int:
        raise TesterError('task needs an explicit GUI boundary and integer deadline')
    if not 30 <= value['timeout_seconds'] <= 3600:
        raise TesterError('guest tester deadline must be 30..3600 seconds')
    settings = value['settings']
    if not isinstance(settings, dict) or set(settings) != {'codex', 'python', 'desktop_app', 'model', 'effort'}:
        raise TesterError('incomplete guest tester settings')
    for key in ('codex', 'python', 'desktop_app'):
        path = settings[key]
        if not isinstance(path, str) or not path.startswith('/') or '\x00' in path:
            raise TesterError('tester executables must use explicit guest absolute paths')
    if not isinstance(settings['model'], str) or not re.fullmatch(r'[a-zA-Z0-9][a-zA-Z0-9_.-]*', settings['model']):
        raise TesterError('unsafe tester model')
    if settings['effort'] not in ('low', 'medium', 'high'):
        raise TesterError('unsupported tester reasoning effort')
    if not isinstance(value['context'], dict) or not isinstance(value['instructions'], str):
        raise TesterError('task context and instructions are required')
    return value


def prompt(task):
    """Give a fresh tester the exact checkpoint contract without old transcript bulk.

Ordinary Farhelm confirmations are authorized product interactions, unlike
missing OS/CUA/authentication prerequisites. Keeping that distinction explicit
prevents an otherwise unattended test from inventing a manual approval pause.
"""
    identity = {key: task[key] for key in ('attempt', 'run_id', 'pass_id', 'check_id')}
    return f'''You are the independent tester INSIDE a disposable macOS Tart guest.
This is an actual test, not fixture setup. Its fixture permissions, CLI installation,
authentication and guest-only authorization have already been prepared.
All needed ordinary product actions inside this disposable guest are authorized,
including Farhelm's "start YOLO session anyway" confirmations, workspace trust,
updater restart and local/remote supervisor upgrades specified by this checkpoint.
Never ask the maintainer to approve a test action. Do not select "don't ask again"
or change confirmation preferences merely to avoid a normal product confirmation.

Use ONLY genuine mcp cua_repl native APIs for every GUI action. First invocation
must use one documented entrypoint alone; inspect its returned documentation.
The native REPL is not a general Node shell: do not assume process or other
Node globals exist. Use guest command execution for independent process receipts.
Accessibility element indices belong to the snapshot that returned them. Before
each indexed action inspect the latest full state and identify its current target;
never reuse an index across a refresh or navigation. Verify the resulting state.
Never use host/Tart computer use, a browser backend, AppleScript, raw Sky, private
OS input APIs, shell UI automation, or product HTTP/DB manipulation. Command
execution is allowed only for published installer setup, signed-input verification,
explicit independent command/service/identity evidence and evidence serialization.
Remain independent of Farhelm: it may quit/restart during your task.
Never read authentication files, unrelated configuration or environment secrets.
Never grant OS/CUA permissions, log in, install the tester/runtime, weaken a policy,
or work around a tool refusal during this test. Such a block is a harness bug:
write status permission-block with its exact error and stop immediately.
Stop on the first actual product failure, unavailable prerequisite or model error.
Expected absence in a fresh fixture, or a source/output directory that this task
has not created yet, is a normal premise rather than a broken prerequisite.
Inspect such premises with structured existence probes or guarded command
branches. Interpret negative diagnostics against the checkpoint's contract;
a failed required operation or an unexpected missing prerequisite still stops.
Before searching source for product paths or behavior, enumerate actual files
under context source_checkout with rg --files or pathlib. Search only paths
observed in that inventory; never guess crate names or directories. A familiar
layout from another checkout is not evidence of this release's source layout.
An echoed prompt, CLI version, tool availability flag or clean process exit is not
proof of successful agent output. Inspect reactive field values before submission.

Task identity: {json.dumps(identity)}
Release/fixture context: {json.dumps(task['context'])}
Checkpoint instructions:
{task['instructions']}

Write result.json in the current evidence directory. It must contain exactly:
identity (the exact task identity object), status (pass/fail/could-not-run/permission-block/automation-failure),
detail (actual observed behavior, <=4000 characters), evidence (relative file paths
of retained command receipts and actual native images, at most 20), and continuity
(null or an object with before/after relative receipt filenames).
Nested files inside this attempt are allowed, such as old-signed-inputs/verification.json.
Each slash-separated path component must match [A-Za-z0-9][A-Za-z0-9_.-]*.
Never use absolute paths, dot components, spaces, plus signs or symlinks.
No component may equal any of these executor record names, in any letter case:
{json.dumps(EXECUTOR_RECORDS)}.
No component may start with native-capture-, in any letter case, and no path may
contain any of these substrings, in any letter case: {json.dumps(PRIVATE_NAME_PARTS)}.
Use a receipt name such as old-verification-command.json instead of a reserved
name such as old-signed-inputs/stderr.txt. Do not reference case-only aliases of
the same path; use the actual spelling consistently across evidence/continuity.
All referenced evidence together must be <=16 MiB; each file must be nonempty.
Use fail ONLY for an observed Farhelm product failure. Errors in your tester
scripts, evidence preparation or unsupported APIs are automation-failure, even
if a product observation succeeded before them. Missing product prerequisites
are could-not-run; OS/CUA/authentication refusals are permission-block.
For GUI checks request actual native screenshots of the important acceptance
states, including the final observed state. The executor saves up to ten recent
distinct native images that fit the evidence budget and adds them to accepted evidence.
Do not encode, serialize or write images yourself. Do not use btoa, Buffer or
image-saving helper scripts. Evidence may be empty when native images alone
satisfy this GUI checkpoint; keep command/continuity receipts where required.
Do not create or reference native-capture-* files; the executor owns those names.
Do not substitute desktop screencapture or retain auth/session tokens.
No result file may be overwritten from an earlier attempt. Final answer references
result.json. Finish within {task['timeout_seconds']} seconds; the executor enforces
the deadline and retains incomplete evidence. Do not claim unrun checks passed.
'''


def native_events(events):
    """Require actual native tool execution and reject refused/failed UI operations.

This checks execution provenance, not image semantics. The operator still
judges the retained screenshots and observation. It deliberately ignores the
word 'permission' in ordinary tool documentation and product confirmation text.
"""
    calls = []
    completed = False
    for row in events:
        if row.get('type') in ('error', 'turn.failed'):
            raise TesterError('model turn failed; inspect retained private event log')
        if row.get('type') == 'turn.completed':
            completed = True
        item = row.get('item', {})
        if row.get('type') != 'item.completed' or item.get('type') != 'mcp_tool_call':
            continue
        if item.get('server') != 'cua_repl' or item.get('tool') != 'js':
            raise TesterError('GUI task used an unexpected MCP backend')
        result = item.get('result') or {}
        if item.get('status') != 'completed' or item.get('error') or result.get('isError') or result.get('is_error'):
            raise TesterError('native computer-use operation failed or was refused')
        calls.append(item)
    if not completed:
        raise TesterError('tester turn did not complete')
    return calls


def native_image_digests(events):
    """Bind an image file to bytes returned by actual native CUA, not a fabricated PNG.

The event stream can establish capture provenance without knowing whether the
pixels prove the claimed product behavior. That interpretation stays explicit
in the observation and independent evaluation.
"""
    digests = set()
    for call in native_events(events):
        for value in (call.get('result') or {}).get('content', []):
            if value.get('type') == 'image':
                encoded = value.get('data', '')
                if not isinstance(encoded, str) or len(encoded) > 24 * 1024 * 1024:
                    raise TesterError('native image exceeds the evidence bound')
                digests.add(hashlib.sha256(base64.b64decode(encoded, validate=True)).hexdigest())
    return digests


def evidence_file(directory, reference):
    """Resolve only selected regular files inside this attempt's working tree.

Signed-input verification naturally creates subdirectories. Supporting those
paths avoids requiring model-written copy helpers, while every component still
belongs to this attempt: no traversal, symlink directories, credentials or
executor records may cross the private export boundary.
"""
    if (not isinstance(reference, str)
            or not re.fullmatch(r'[a-zA-Z0-9][a-zA-Z0-9_.-]*(/[a-zA-Z0-9][a-zA-Z0-9_.-]*)*', reference)):
        raise TesterError('evidence must name a file in this attempt only')
    if any(word in reference.lower() for word in PRIVATE_NAME_PARTS):
        raise TesterError('authentication stores cannot be evidence')
    relative = Path(reference)
    # macOS fixtures commonly use a case-insensitive volume; spelling alone
    # must not allow a directory to alias one of the executor's metadata files.
    if any(component.lower() in EXECUTOR_RECORDS for component in relative.parts):
        raise TesterError('model evidence cannot replace executor records')
    root = Path(directory).resolve()
    path = root
    for component in relative.parts:
        path = path / component
        if path.is_symlink():
            raise TesterError('evidence file is missing or escapes the attempt')
    if not path.is_file() or not path.resolve().is_relative_to(root):
        raise TesterError('evidence file is missing or escapes the attempt')
    if not 0 < path.stat().st_size <= 16 * 1024 * 1024:
        raise TesterError('evidence file is empty or over 16 MiB')
    return path


def validate_observation(task, value, directory, allow_empty=False):
    """Fence task identity and export-safe files independently of turn completion.

A zero exit cannot make a permission pause, absent image or stale result pass.
Only files belonging to this attempt may be returned; symlink escapes and
authentication-store names are rejected before collection.
"""
    keys = {'identity', 'status', 'detail', 'evidence', 'continuity'}
    identity = {key: task[key] for key in ('attempt', 'run_id', 'pass_id', 'check_id')}
    if not isinstance(value, dict) or set(value) != keys or value['identity'] != identity:
        raise TesterError('missing or mismatched tester result identity')
    if value['status'] not in RESULT_STATES:
        raise TesterError('unsupported tester verdict')
    if not isinstance(value['detail'], str) or not value['detail'].strip() or len(value['detail']) > 4000:
        raise TesterError('tester observation is absent or exceeds its bound')
    references = value['evidence']
    if (not isinstance(references, list) or len(references) > 20
            or (not references and value['status'] == 'pass' and not allow_empty)):
        raise TesterError('tester needs a bounded nonempty evidence list')
    if value['continuity'] is not None:
        if not isinstance(value['continuity'], dict) or set(value['continuity']) != {'before', 'after'}:
            raise TesterError('invalid continuity references')
        references = references + list(value['continuity'].values())
    total = 0
    spellings = {}
    for ref in references:
        if isinstance(ref, str):
            canonical = ref.lower()
            if canonical in spellings and spellings[canonical] != ref:
                raise TesterError('evidence paths cannot use case aliases')
            spellings[canonical] = ref
        path = evidence_file(directory, ref)
        total += path.stat().st_size
    if total > 16 * 1024 * 1024:
        raise TesterError('attempt evidence exceeds the aggregate export bound')
    return value


def retain_native_captures(task, value, directory, events):
    """Persist tool-returned image bytes without asking the model to encode them.

Image serialization is executor work: unsupported model-side encoding APIs
must not invalidate an otherwise useful product observation. Keep the latest
ten distinct captures, within the existing reference and byte budgets. Failed
turns may retain earlier successful captures as diagnostics, but this helper
cannot grant native completion or a product pass.
"""
    value = validate_observation(task, value, directory, allow_empty=True)
    references = value['evidence'] + list((value['continuity'] or {}).values())
    if any(component.lower().startswith('native-capture-') for ref in references for component in Path(ref).parts):
        raise TesterError('native capture filenames belong to the executor')
    captures = {}
    for row in events:
        item = row.get('item', {})
        result = item.get('result') or {}
        if (row.get('type') != 'item.completed' or item.get('type') != 'mcp_tool_call'
                or item.get('server') != 'cua_repl' or item.get('tool') != 'js'
                or item.get('status') != 'completed' or item.get('error')
                or result.get('isError') or result.get('is_error')):
            continue
        for content in result.get('content', []):
            if content.get('type') != 'image':
                continue
            encoded = content.get('data', '')
            if not isinstance(encoded, str) or len(encoded) > 24 * 1024 * 1024:
                raise TesterError('native image exceeds the evidence bound')
            data = base64.b64decode(encoded, validate=True)
            digest = hashlib.sha256(data).hexdigest()
            captures.pop(digest, None)
            captures[digest] = data
    available = min(10, 20 - len(value['evidence']))
    root = Path(directory)
    references = value['evidence'] + list((value['continuity'] or {}).values())
    total = sum((root / ref).stat().st_size for ref in references)
    selected = []
    # Favor recent acceptance states without allowing an earlier large capture
    # to discard all useful diagnostics. The pass gate still requires an actual
    # image if no screenshot fits alongside the checkpoint's command receipts.
    for data in reversed(list(captures.values())):
        if len(selected) == available:
            break
        if total + len(data) <= 16 * 1024 * 1024:
            selected.append(data)
            total += len(data)
    selected.reverse()
    retained = list(value['evidence'])
    for index, data in enumerate(selected, 1):
        extension = 'png' if data.startswith(b'\x89PNG\r\n\x1a\n') else 'jpg' if data.startswith(b'\xff\xd8') else 'bin'
        name = f'native-capture-{index:03d}.{extension}'
        with (root / name).open('xb') as stream:
            os.chmod(root / name, 0o600)
            stream.write(data)
        retained.append(name)
    return dict(value, evidence=retained)


def validate_result(task, value, directory, events):
    """A pass needs a completed native stream, beyond merely export-safe observations.

Failure diagnostics use the structural validator separately: losing native
completion must not discard an otherwise safe screenshot or failure receipt.
"""
    value = validate_observation(task, value, directory)
    root = Path(directory).resolve()
    calls = native_events(events)
    if value['status'] == 'pass' and task['gui'] and not calls:
        raise TesterError('passing GUI check has no actual native CUA calls')
    if value['status'] == 'pass' and task['gui']:
        images = native_image_digests(events)
        if not any(hashlib.sha256((root / ref).read_bytes()).hexdigest() in images for ref in value['evidence']):
            raise TesterError('GUI evidence does not contain an actual native tool-returned image')
    return value


def stop_owned_group(process):
    """Reap only this attempt's isolated child group on timeout or cancellation.

The executor retains the Popen handle, not a persisted numeric PID. Reaping
waits before returning so teardown cannot race a still-running tester.
"""
    # The leader may exit while its MCP children remain. Keep its unreaped
    # identity until group termination, including the normal completion path.
    child_exited(process)  # ECHILD refuses signals if another waiter stole ownership.
    try:
        signal_owned_group(process, signal.SIGTERM)
        deadline = time.monotonic() + 5
        while not child_exited(process) and time.monotonic() < deadline:
            time.sleep(0.05)
        # Kill any remaining MCP descendants while the leader's identity is
        # still reserved by WNOWAIT. Waiting first permits numeric PID reuse.
        signal_owned_group(process, signal.SIGKILL)
        deadline = time.monotonic() + 5
        while group_live(process.pid):
            if time.monotonic() >= deadline:
                raise TesterError('owned tester group still has live members after termination')
            time.sleep(0.05)
    finally:
        process.wait(timeout=5)


def group_live(group):
    """Inspect only process identifiers/state while the unreaped leader pins its group.

Darwin can return EPERM when killpg targets an all-zombie group. A successful
bounded process-table read distinguishes that case from a real signal denial;
absence of output or a failed ps command must never establish cleanup.
"""
    table = subprocess.run(['/bin/ps', '-axo', 'pid=,pgid=,stat='], check=True,
                           capture_output=True, timeout=5).stdout
    if not table or len(table) > 1024 * 1024:
        raise TesterError('process-group observation is absent or exceeds its bound')
    for row in table.decode().splitlines():
        parts = row.split()
        if len(parts) != 3:
            raise TesterError('malformed process-group observation')
        if int(parts[1]) == group and not parts[2].startswith('Z'):
            return True
    return False


def signal_owned_group(process, signum):
    """Ignore Darwin's zombie-only refusal, but propagate denial for any live peer."""
    try:
        os.killpg(process.pid, signum)
    except ProcessLookupError:
        pass
    except PermissionError:
        if group_live(process.pid):
            raise


def child_exited(process):
    """Observe exit without releasing the process identity used for group teardown.

Popen.poll() reaps the leader. WNOWAIT keeps that identity owned until every
termination signal has been sent, so a descendant cannot outlive its attempt
and a recycled group identifier cannot receive the attempt's cleanup signal.
"""
    return os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is not None


def export_attempt(root, summary):
    """Collect only executor records and validated result references.

The model's working directory is not an export boundary: it may contain scratch
trees, unrelated files or unsafe symlinks. Failure exports intentionally omit
unvalidated model-created artifacts. All exported bytes remain private.
"""
    destination = root / 'export'
    destination.mkdir(mode=0o700)
    names = {'task.json', 'execution.json', 'events.jsonl', 'stderr.txt', 'model-result.json'}
    if summary['status'] != 'automation-failure':
        names.add('result.json')
        names.update(summary['result']['evidence'])
        names.update((summary['result']['continuity'] or {}).values())
    elif summary.get('unverified_result'):
        names.add('result.json')
        names.update(summary['unverified_result']['evidence'])
        names.update((summary['unverified_result']['continuity'] or {}).values())
    for name in names:
        metadata = name in ('task.json', 'execution.json', 'events.jsonl', 'stderr.txt', 'model-result.json')
        source = (root if metadata or (name == 'result.json' and (root / name).is_file()) else root / 'work') / name
        if not metadata and name != 'result.json':
            source = evidence_file(root / 'work', name)
        if source.is_symlink() or not source.is_file():
            continue
        # Oversized diagnostics stay inside the guest. Export their size and
        # the failure summary rather than an archive our importer cannot read.
        limit = MAX_LOG if name in ('events.jsonl', 'stderr.txt') else 16 * 1024 * 1024
        if source.stat().st_size > limit:
            continue
        target = destination / name
        # Create only the selected file's parents, privately. Never copy its
        # neighboring scratch files or the whole signed-input directory.
        parent = destination
        for component in Path(name).parts[:-1]:
            parent = parent / component
            parent.mkdir(mode=0o700, exist_ok=True)
        with source.open('rb') as stream, target.open('xb') as output:
            shutil.copyfileobj(stream, output)
        target.chmod(0o600)


def failure_observation(task, raw, work, events_path, logs_bounded):
    """Preserve safe observations even when interruption leaves a partial event row.

This is diagnostic salvage, never successful execution validation. A complete
command receipt remains useful when the last JSONL write was interrupted or
automatic image retention failed; neither case may erase the model's original
product-failure claim. The normal success path still parses every row strictly.
"""
    value = json.loads(raw)
    if isinstance(value, dict) and isinstance(value.get('evidence'), list):
        # The raw model record remains unchanged in metadata, but a diagnostic
        # fallback must never export model-made files under executor image names.
        value = dict(value, evidence=[ref for ref in value['evidence']
                                     if not isinstance(ref, str) or not any(
                                         component.lower().startswith('native-capture-') for component in Path(ref).parts)])
        continuity = value.get('continuity')
        if isinstance(continuity, dict) and any(isinstance(ref, str) and any(
                component.lower().startswith('native-capture-') for component in Path(ref).parts)
                for ref in continuity.values()):
            value['continuity'] = None
    value = validate_observation(task, value, work, allow_empty=True)
    if not logs_bounded:
        return value
    events = []
    try:
        lines = events_path.read_bytes().splitlines()
    except OSError:
        return value
    for line in lines:
        try:
            row = json.loads(line)
            if isinstance(row, dict):
                events.append(row)
        except ValueError:
            # Only diagnostics may ignore a damaged row. No salvaged event
            # stream is used to establish native completion or a product pass.
            continue
    try:
        return retain_native_captures(task, value, work, events)
    except (OSError, ValueError, RuntimeError, TypeError, AttributeError):
        return value


def run(task, directory, spawner=subprocess.Popen, clock=time.monotonic, pause=time.sleep):
    """Execute one fresh, bounded tester turn and retain success and failure records.

Fresh turns avoid growing a release-long model context. The attempt directory
is exclusive and logs are private; a hard size/deadline boundary covers vendor
silence as well as verbose tool output. Neither runner environment nor parent
authentication configuration is changed.
"""
    task = request(task)
    if not hasattr(os, 'waitid') or not hasattr(os, 'WNOWAIT'):
        raise TesterError('guest executor requires Python with waitid/WNOWAIT before any child starts')
    root = Path(directory)
    root.mkdir(mode=0o700, parents=False)
    work = root / 'work'
    work.mkdir(mode=0o700)
    (root / 'task.json').write_text(json.dumps(task, indent=2) + '\n')
    (root / 'prompt.txt').write_text(prompt(task))
    state = root.parent / 'executor-state.json'
    state.write_text(json.dumps({'attempt': task['attempt'], 'phase': 'running'}))
    argv = native_args(task['settings']) + [
        'exec', '--yolo', '-m', task['settings']['model'], '--skip-git-repo-check',
        '--json', '-C', str(work), '-o', str(root / 'final.txt'), prompt(task),
    ]
    process = None
    cleanup_ok = False
    try:
        with (root / 'events.jsonl').open('xb') as output, (root / 'stderr.txt').open('xb') as errors:
            process = spawner(argv, stdin=subprocess.DEVNULL, stdout=output, stderr=errors,
                              start_new_session=True, close_fds=True)
            deadline = clock() + task['timeout_seconds']
            while not child_exited(process):
                if (root.parent / 'cancel.json').exists():
                    raise TesterError('guest tester cancelled by its owning coordinator')
                if clock() >= deadline:
                    raise TesterError('guest tester exceeded its deadline')
                if sum((root / name).stat().st_size for name in ('events.jsonl', 'stderr.txt')) > MAX_LOG:
                    raise TesterError('guest tester output exceeded its bound')
                # Scheduling backoff follows an owned-child readiness check;
                # elapsed time never establishes product or GUI readiness.
                pause(min(0.5, max(0, deadline - clock())))
            stop_owned_group(process)
            cleanup_ok = True
        if process.returncode != 0:
            raise TesterError('guest tester process failed; inspect retained diagnostics')
        if sum((root / name).stat().st_size for name in ('events.jsonl', 'stderr.txt')) > MAX_LOG:
            raise TesterError('guest tester output exceeded its bound')
        result = work / 'result.json'
        if result.is_symlink() or not result.is_file() or result.stat().st_size > MAX_INPUT:
            raise TesterError('guest tester did not retain a bounded result')
        events = [json.loads(line) for line in (root / 'events.jsonl').read_text().splitlines() if line]
        raw = result.read_bytes()
        (root / 'model-result.json').write_bytes(raw)
        value = retain_native_captures(task, json.loads(raw), work, events)
        value = validate_observation(task, value, work)
        (root / 'result.json').write_text(json.dumps(value, indent=2) + '\n')
        value = validate_result(task, value, work, events)
        if value['status'] == 'automation-failure':
            raise TesterError('tester reported automation failure: ' + value['detail'])
        summary = {'format': 1, 'status': value['status'], 'result': value,
                   'native_calls': len(native_events(events)), 'exit_code': process.returncode}
    except BaseException as error:
        cleanup_error = None
        if process is not None and process.returncode is None:
            try:
                stop_owned_group(process)
                cleanup_ok = True
            except BaseException as cleanup:
                cleanup_error = str(cleanup)
        if process is not None and not cleanup_ok and cleanup_error is None:
            cleanup_error = 'owned-group cleanup was not observed'
        summary = {'format': 1, 'status': 'automation-failure', 'error': str(error),
                   'cleanup_error': cleanup_error,
                   'exit_code': process.returncode if process is not None else None}
        result = work / 'result.json'
        try:
            accepted = root / 'result.json'
            if accepted.is_file():
                summary['unverified_result'] = validate_observation(task, json.loads(accepted.read_text()), work)
            elif not result.is_symlink() and result.is_file() and result.stat().st_size <= MAX_INPUT:
                raw = result.read_bytes()
                (root / 'model-result.json').write_bytes(raw)
                events_path = root / 'events.jsonl'
                size = sum((root / name).stat().st_size for name in ('events.jsonl', 'stderr.txt'))
                value = failure_observation(task, raw, work, events_path, size <= MAX_LOG)
                summary['unverified_result'] = value
                accepted.write_text(json.dumps(value, indent=2) + '\n')
        except (OSError, ValueError, RuntimeError):
            pass
        (root / 'execution.json').write_text(json.dumps(summary, indent=2) + '\n')
        export_attempt(root, summary)
        state.write_text(json.dumps({'attempt': task['attempt'],
                                    'phase': 'cleanup-failed' if cleanup_error else 'stopped'}))
        raise
    (root / 'execution.json').write_text(json.dumps(summary, indent=2) + '\n')
    export_attempt(root, summary)
    state.write_text(json.dumps({'attempt': task['attempt'], 'phase': 'stopped'}))
    return summary


def main():
    """Refuse accidental host execution before reading task input or starting Codex."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--task', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    virtual = subprocess.run(['/usr/sbin/sysctl', '-n', 'kern.hv_vmm_present'],
                             capture_output=True, check=True, timeout=5)
    if virtual.stdout.strip() != b'1':
        raise TesterError('executor requires a virtualized Mac guest')
    if args.task.is_symlink() or args.task.stat().st_size > MAX_INPUT:
        raise TesterError('task input is missing, redirected or too large')
    os.umask(0o077)
    def interrupted(signum, frame):
        """Route controller cancellation through the executor's owned-child cleanup."""
        raise TesterError('guest executor interrupted')
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    summary = run(json.loads(args.task.read_text()), args.output)
    print(json.dumps({'status': summary['status'], 'native_calls': summary['native_calls']}), flush=True)
    return 0 if summary['status'] == 'pass' else 1


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f'guest tester failed: {error}', flush=True)
        raise SystemExit(1)
