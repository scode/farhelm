#!/usr/bin/env python3
"""Keep release-test commands and evidence outside the UI model's discretion.

The model chooses native UI actions and reports what it sees. This process owns
the finite command vocabulary, durable receipts and native-tool forwarding. A
failed operation closes the action gate before another tool call can execute;
returning an error to the model alone would still let a batched click run.
"""

import argparse
from collections import deque
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
import sys
import time
from urllib.request import urlopen

from checkpoint_contract import available_operations, require_applicable_operation
from guest_ui import NativeUI, tool_definition, wait_running
from window_geometry import calibrate, select_frame


LIMIT = 512 * 1024
OPERATIONS = ('prerequisites', 'verify_candidate', 'verify_previous', 'fixture_clean', 'install_candidate',
              'install_previous', 'launch', 'launch_override', 'versions', 'remote_version',
              'update_logs', 'public_latest', 'inspect_state', 'continuity_command',
              'continuity_before', 'continuity_remote_before', 'continuity_remote_after',
              'continuity_reopen_before', 'continuity_reopen_after', 'prior_receipts', 'window_frame', 'finder_frame')


def write_json(path, value, replace=False):
    """Keep complete private records, refusing accidental evidence replacement."""
    path = Path(path)
    content = json.dumps(value, indent=2).encode() + b'\n'
    if len(content) > LIMIT:
        raise RuntimeError('managed receipt exceeds its bound')
    if path.is_symlink() or (path.exists() and not replace):
        raise RuntimeError('managed record already exists or is redirected')
    temporary = path.with_name(path.name + '.pending')
    with temporary.open('xb') as stream:
        os.chmod(temporary, 0o600)
        stream.write(content)
    temporary.replace(path)


class Operations:
    """Execute named guest operations and retain facts independently of model prose.

All paths come from the host's claimed task. Tool arguments cannot provide a
command, source URL, filename or program. The persistent pass record carries
only receipt references and test observations, never authentication contents.
"""

    def __init__(self, task, output, runner=subprocess.run, home=None):
        self.task = task
        self.context = task['context']
        self.output = Path(output)
        self.work = self.output / 'work'
        self.home = Path.home() if home is None else Path(home)
        self.runner = runner
        self.pass_dir = Path(self.context['pass_directory'])
        if self.pass_dir.resolve() != self.output.parent.parent.resolve():
            raise RuntimeError('managed pass directory differs from the executor-owned task path')
        self.state_path = self.pass_dir / 'managed-state.json'
        self.state = json.loads(self.state_path.read_text()) if self.state_path.exists() else {'receipts': []}
        self.receipts = []
        self.completed = set()
        self.observations = {}
        self.latest_receipts = {}
        self.latest_images = {}
        self.failure = None

    def command(self, argv, *, cwd=None, env=None, data=None, timeout=60):
        """Check each fixed command before its caller can perform a later action."""
        result = self.runner(argv, cwd=cwd, env=env, input=data, capture_output=True, timeout=timeout)
        row = {'argv': argv, 'returncode': result.returncode,
               'stdout': result.stdout.decode(errors='replace'), 'stderr': result.stderr.decode(errors='replace')}
        if len(json.dumps(row).encode()) > LIMIT:
            raise RuntimeError('managed command output exceeds its bound')
        if result.returncode:
            # Preserve the exact failed command too; a failure must not require
            # a second model-written script to explain what happened.
            self.receipt('failed-command', row)
            raise RuntimeError(f'managed command exited {result.returncode}: {argv[0]}')
        return row

    def receipt(self, name, value):
        """Give each observation a unique filename and a digest-bound durable reference."""
        filename = f'managed-{len(self.receipts) + 1:03d}-{name}.json'
        path = self.work / filename
        write_json(path, value)
        self.receipts.append(filename)
        # A handoff is a view of earlier receipts, not a new fact to nest in
        # every later handoff. Keeping it out avoids exponential transcript growth.
        if name != 'prior-receipts':
            self.state['receipts'] = [row for row in self.state['receipts']
                                      if (row['check_id'], row['operation']) != (self.task['check_id'], name)]
            self.state['receipts'].append({'check_id': self.task['check_id'], 'operation': name,
                'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
        self.latest_receipts[name] = filename
        self.save_record()
        write_json(self.state_path, self.state, replace=True)
        return filename

    def save_record(self):
        """Persist even the first failed command so diagnostics do not depend on a model's final reply."""
        selected = list(self.latest_receipts.values())
        selected.extend(name for name in self.receipts if name in ('verification.json', 'fixture-clean.json', 'managed-window-frames.json')
                        or name.startswith('managed-counter-'))
        write_json(self.output / 'managed-operations.json', {
            'completed': sorted(self.completed), 'receipts': list(dict.fromkeys(selected)),
            'observations': self.observations,
            'continuity': self.state.get('continuity') if self.task['check_id'] in (
                'continuity-before', 'remote-update', 'reopen') else None}, replace=True)

    def fail(self, message):
        """Latch one failure durably before replying to either tool client."""
        if self.failure is None:
            self.failure = str(message)[:4000]
            write_json(self.output / 'managed-failure.json', {'error': self.failure})

    def dispatch(self, operation):
        """Refuse actions after failure, including otherwise harmless retries."""
        if self.failure is not None:
            raise RuntimeError('test action gate is closed: ' + self.failure)
        if operation not in OPERATIONS:
            raise RuntimeError('unknown managed operation')
        try:
            require_applicable_operation(self.task['check_id'], self.task['pass_id'],
                                         self.context.get('update_path'), operation)
            # Repeating an uncertain installer or launch can hide the first
            # outcome. Observation operations may poll; mutations run once.
            if operation in self.completed and operation in (
                    'install_previous', 'install_candidate', 'launch', 'launch_override'):
                raise RuntimeError('managed mutation already completed; it cannot be replayed')
            value = self.perform(operation)
            name = self.receipt(operation.replace('_', '-'), value)
            self.completed.add(operation)
            self.observations[operation] = value
            self.save_record()
            return {'operation': operation, 'receipt': name, 'observation': value}
        except Exception as error:
            self.fail(f'{type(error).__name__}: {error}')
            raise

    def ssh(self, script):
        """Use the manifest-pinned peer and fixed Python, never a model command string."""
        return self.command(['/usr/bin/ssh', '-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=yes',
            self.context['ssh']['alias'], '/usr/bin/python3', '-'], data=script.encode(), timeout=30)

    def version(self):
        """Separate installed payloads from the live app and its managed supervisor.

The GUI executable has an unversioned bundle path. Its direct supervisor child
uses the versioned payload path; old session reporters can outlive app updates
and must not be mistaken for that live child. The native UI readout separately
establishes the GUI's in-memory release, and PID changes establish its restart.
"""
        app = self.home / 'Applications/Farhelm.app'
        result = {'installed': None, 'bundle': None, 'processes': [], 'running_versions': []}
        marker = app / 'Contents/Versions/installed'
        if marker.is_file():
            result['installed'] = marker.read_text().strip()
            with (app / 'Contents/Info.plist').open('rb') as stream:
                result['bundle'] = plistlib.load(stream).get('CFBundleShortVersionString')
            result['cli'] = self.command([str(self.home / '.local/bin/farhelm'), '--version'], timeout=30)
        processes = self.command(['/bin/ps', '-axo', 'pid=,ppid=,comm='])['stdout'].splitlines()
        result['processes'] = [line for line in processes if 'farhelm' in line.lower()]
        rows = [line.split(maxsplit=2) for line in result['processes']]
        desktop = {int(row[0]) for row in rows if len(row) == 3
                   and row[2] == str(app / 'Contents/MacOS/farhelm-desktop')}
        for row in rows:
            if len(row) != 3 or int(row[1]) not in desktop:
                continue
            match = re.fullmatch(re.escape(str(app / 'Contents/Versions')) + r'/([^/]+)/farhelm', row[2])
            if match:
                result['running_versions'].append(match[1])
        result['running_versions'] = sorted(set(result['running_versions']))
        result['desktop_pids'] = sorted(desktop)
        result['desktop_running'] = bool(desktop)
        return result

    def perform(self, operation):
        """Implement the fixed vocabulary; native UI interactions stay in the official tool."""
        check = self.task['check_id']
        previous, candidate = self.context['previous'], self.context['candidate']
        source = Path(self.context['source_checkout'])
        signed = Path(self.context['signed_inputs'])
        app = self.home / 'Applications/Farhelm.app'
        if operation == 'prerequisites':
            mac = self.command(['/usr/sbin/sysctl', '-n', 'kern.hv_vmm_present'])
            if mac['stdout'].strip() != '1':
                raise RuntimeError('managed operations require a virtualized Mac')
            local = self.command([self.task['settings']['codex'], '--version'])
            remote = self.ssh('''from pathlib import Path
import subprocess,json,stat,os
p=Path.home()/'.codex/auth.json'
s=p.lstat()
assert stat.S_ISREG(s.st_mode) and s.st_mode & 0o777 == 0o600 and s.st_uid==os.getuid()
r=subprocess.run(['codex','--version'],capture_output=True,text=True,timeout=20)
assert r.returncode==0
print(json.dumps(dict(cli=r.stdout.strip(),authentication_metadata='private regular file owned by guest')))
''')
            result = {'virtualized_mac': mac, 'mac_cli': local, 'linux': remote}
            if not self.task['pass_id'].startswith('p0-'):
                result['native_geometry_toolchain'] = self.command(['/usr/bin/swift', '--version'])
            return result
        if operation == 'prior_receipts':
            rows = []
            for row in self.state['receipts']:
                if row['check_id'] == check:
                    continue
                path = Path(row['path'])
                if path.is_symlink() or not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != row['sha256']:
                    raise RuntimeError('a prior managed receipt changed or disappeared')
                rows.append(dict(check_id=row['check_id'], operation=row['operation'], value=json.loads(path.read_text())))
            return rows
        if operation in ('verify_candidate', 'verify_previous'):
            if check not in ('artifact-verification', 'old-fixture'):
                raise RuntimeError('signature verification belongs to artifact or old-fixture setup')
            commands = []
            if not source.exists():
                commands.append(self.command(['git', 'clone', '--quiet', self.context['source_url'], str(source)], timeout=180))
            commands.append(self.command(['git', 'fetch', '--quiet', 'origin', 'tag', previous], cwd=str(source), timeout=120))
            commit = self.command(['git', 'rev-parse', previous + '^{commit}'], cwd=str(source))
            if commit['stdout'].strip() != self.context['previous_commit']:
                raise RuntimeError('previous source commit differs from the release plan')
            version = candidate if operation == 'verify_candidate' else previous
            destination = signed if operation == 'verify_candidate' else self.pass_dir / 'old-signed-inputs'
            if not destination.exists():
                commands.append(self.command([sys.executable, str(Path(__file__).with_name('verify_release.py')),
                    '--previous', previous, '--version', version, '--output', str(destination)], cwd=str(source), timeout=180))
            report = json.loads((destination / 'verification.json').read_text())
            if report['version'] != version or report['previous_trust_ring'] != previous:
                raise RuntimeError('signed inputs belong to another release')
            if any(report[key] != 'verified' for key in ('signature', 'trusted_comment', 'installer_checksum')):
                raise RuntimeError('release verification is incomplete')
            # Keep the established host verification interface in the current
            # attempt, rather than asking a model to copy or reconstruct it.
            if operation == 'verify_candidate':
                write_json(self.work / 'verification.json', report)
                self.receipts.append('verification.json')
            return {'verification': report, 'commands': commands}
        if operation == 'fixture_clean':
            if check not in ('fixture-clean', 'old-fixture'):
                raise RuntimeError('cleanliness is a setup premise, never a repair operation')
            path = self.work / 'fixture-clean.json'
            self.command([sys.executable, str(Path(__file__).with_name('fixture_probe.py')), '--role', 'mac',
                '--ssh-alias', self.context['ssh']['alias'], '--output', str(path)], timeout=180)
            report = json.loads(path.read_text())
            self.receipts.append('fixture-clean.json')
            if report.get('clean') is not True:
                raise RuntimeError('fixture contains Farhelm state')
            return {'clean': True, 'roles': [row['role'] for row in report['observations']]}
        if operation in ('install_previous', 'install_candidate'):
            allowed = check == 'old-fixture' if operation == 'install_previous' else (
                check == 'install' or (check == 'update-relaunch' and self.context['update_path'] == 'manual'))
            if not allowed:
                raise RuntimeError('installer cannot substitute for native updater discovery or relaunch')
            if operation == 'install_previous' and not {'fixture_clean', 'verify_previous'} <= self.completed:
                raise RuntimeError('old installation requires this attempt to establish clean and verified inputs first')
            directory = self.pass_dir / 'old-signed-inputs' if operation == 'install_previous' else signed
            verification = json.loads((directory / 'verification.json').read_text())
            version = previous if operation == 'install_previous' else candidate
            if (verification['version'] != version
                    or hashlib.sha256((directory / 'install.sh').read_bytes()).hexdigest() != verification['installer_sha256']
                    or hashlib.sha256((directory / 'SHA256SUMS').read_bytes()).hexdigest() != verification['sha256sums_sha256']):
                raise RuntimeError('installer differs from the verified release')
            environ = dict(os.environ, FARHELM_VERSION=version, FARHELM_INSTALL_SUMS_FILE=str(directory / 'SHA256SUMS'))
            receipt = self.command(['/bin/sh', str(directory / 'install.sh')], env=environ, timeout=300)
            if operation == 'install_previous':
                state = self.home / '.local/state/farhelm'
                if state.exists():
                    raise RuntimeError('old app state already exists before first launch')
                state.mkdir(mode=0o700, parents=True)
                write_json(state / 'desktop-client.json', {'install_updates_automatically': False})
            return {'installer': receipt, 'versions': self.version()}
        if operation in ('launch', 'launch_override'):
            allowed = ('app-start', 'old-fixture', 'reopen') if operation == 'launch' else ('update-discovery', 'update-relaunch')
            if check not in allowed or (operation == 'launch_override' and self.context.get('update_path') != 'override'):
                raise RuntimeError('manual launch is forbidden at this checkpoint')
            if check == 'update-relaunch' and self.task['pass_id'].startswith('p0-'):
                raise RuntimeError('P0 relaunch must use the already-running app updater')
            if self.version()['desktop_running']:
                raise RuntimeError('quit the app normally through native UI before managed launch')
            logs = self.pass_dir / 'managed-logs'
            logs.mkdir(mode=0o700, exist_ok=True)
            number = len(list(logs.iterdir()))
            stdout, stderr = logs / f'launch-{number}.stdout', logs / f'launch-{number}.stderr'
            argv = ['/usr/bin/open', '-n', '--stdout', str(stdout), '--stderr', str(stderr)]
            if operation == 'launch_override':
                argv += ['--env', 'FARHELM_DESKTOP_UPDATE_LATEST=' + candidate]
            argv.append(str(app))
            receipt = self.command(argv)
            self.state['launch_log'] = str(stderr)
            self.state['launch_stdout'] = str(stdout)
            self.receipt('launch-command', receipt)
            expected = previous if check == 'old-fixture' or operation == 'launch_override' else candidate
            running = wait_running(self, expected.removeprefix('v'), label='launch')
            return {'command': receipt, 'running': running}
        if operation == 'versions':
            return self.version()
        if operation == 'remote_version':
            value = self.ssh('''from pathlib import Path
import subprocess,json
p=Path.home()/'.local/lib/farhelm/farhelm'
rows=[]
for argv in ([str(p),'--version'],['systemctl','--user','show','farhelm-supervisor.service','--property=ActiveState,SubState,MainPID,ExecStart']):
 r=subprocess.run(argv,capture_output=True,text=True,timeout=20)
 rows.append(dict(argv=argv,returncode=r.returncode,stdout=r.stdout,stderr=r.stderr))
print(json.dumps(rows))
raise SystemExit(0 if all(r['returncode']==0 for r in rows) else 1)
''')
            rows = json.loads(value['stdout'])
            if check in ('old-fixture', 'remote-update', 'remote-supervisor-agent'):
                expected = previous if check == 'old-fixture' else candidate
                if rows[0]['stdout'].strip() != 'farhelm ' + expected.removeprefix('v'):
                    raise RuntimeError('remote binary is not the expected release')
                if 'ActiveState=active' not in rows[1]['stdout'] or 'SubState=running' not in rows[1]['stdout']:
                    raise RuntimeError('remote supervisor is not active and running')
            return value
        if operation == 'public_latest':
            with urlopen('https://get.farhelm.io/latest', timeout=30) as response:
                value = response.read(256).decode().strip()
            if not re.fullmatch(r'v[0-9]+\.[0-9]+\.[0-9]+', value):
                raise RuntimeError('public latest response is malformed')
            return {'url': 'https://get.farhelm.io/latest', 'version': value}
        if operation == 'update_logs':
            path = self.state.get('launch_log')
            if path is None:
                return {'available': False, 'reason': 'this pass has no managed launch log'}
            path = Path(path)
            if not path.exists():
                return {'available': False, 'reason': 'launch log has not appeared yet'}
            if path.is_symlink() or path.stat().st_size > LIMIT:
                raise RuntimeError('managed launch log is redirected or exceeds its bound')
            lines = path.read_text(errors='replace').splitlines()
            return {'available': True, 'lines': [line for line in lines if 'updater' in line.lower()]}
        if operation == 'inspect_state':
            return self.inspect_state()
        if operation in ('window_frame', 'finder_frame'):
            return self.window_frame('farhelm' if operation == 'window_frame' else 'finder')
        if operation.startswith('continuity_'):
            return self.continuity(operation)
        raise RuntimeError('operation has no implementation')

    def window_frame(self, target):
        """Measure one pinned native window and retain the whole gesture trace in one bounded artifact.

The model sees only the current measurement. The aggregate keeps every sample
without growing each tool reply with the full history or exhausting the
twenty-reference evidence budget alongside the ten required native images.
"""
        if target == 'farhelm':
            version = self.version()
            if self.context['candidate'].removeprefix('v') not in version['running_versions']:
                raise RuntimeError('candidate process is absent for window measurement')
            process_ids = set(version['desktop_pids'])
        else:
            rows = self.command(['/bin/ps', '-axo', 'pid=,comm='])['stdout'].splitlines()
            process_ids = {int(row.split()[0]) for row in rows if '/Finder.app/Contents/MacOS/Finder' in row}
        if not process_ids:
            raise RuntimeError('the selected native app has no owned process')
        command = self.command(['/usr/bin/swift', str(Path(__file__).with_name('window_geometry.swift')),
                                *[str(pid) for pid in sorted(process_ids)]], timeout=60)
        self.receipt('window-command-' + target, command)
        previous = self.state.setdefault('window_measurements', {}).get(target)
        frame = select_frame(json.loads(command['stdout']), process_ids, previous['frame'] if previous else None)
        image = self.latest_images.get(target)
        observation = {'frame': frame, 'image': image, 'image_scale': calibrate(frame, image, previous),
                       'display': json.loads(command['stdout'])['display'],
                       'sampled_at': time.time()}
        if previous is not None:
            observation['delta'] = {key: frame[key] - previous['frame'][key] for key in ('x', 'y', 'width', 'height')}
        history = self.state.setdefault('window_frames', [])
        if len(history) >= 64:
            raise RuntimeError('native window observation trace exceeds its bound')
        history.append({'target': target, 'observation': observation, 'command': command})
        self.state['window_measurements'][target] = observation
        filename = 'managed-window-frames.json'
        write_json(self.work / filename, {'format': 1, 'observations': history}, replace=True)
        if filename not in self.receipts:
            self.receipts.append(filename)
        self.save_record()
        return observation

    def inspect_state(self):
        """Read only the test's session, template and display-setting columns.

Never return token-bearing tables or arbitrary SQL. Schema introspection lets
the historical upgrade fixtures expose the same bounded observation contract.
"""
        import sqlite3
        result = {}
        for database, tables in (('supervisor.db', {'sessions': ('id', 'name', 'generation', 'captured_conversation', 'conversation_source')}),
                                 ('helm.db', {'launch_templates': ('name', 'fields_json'),
                                              'preferences': ('compact', 'codex_trust', 'tunnel_consent', 'color_terminal_frames')})):
            path = self.home / '.local/state/farhelm' / database
            if not path.is_file():
                result[database] = {'present': False}
                continue
            with sqlite3.connect(path.as_uri() + '?mode=ro', uri=True) as connection:
                observed = {}
                for table, wanted in tables.items():
                    actual = {row[1] for row in connection.execute(f'PRAGMA table_info({table})')}
                    columns = [name for name in wanted if name in actual]
                    if columns:
                        observed[table] = [dict(zip(columns, row)) for row in connection.execute(
                            f'SELECT {",".join(columns)} FROM {table} LIMIT 100')]
                result[database] = observed
        return result

    def continuity(self, operation):
        """Retain original-process samples and explicit before/after milestones."""
        directory = '~/farhelm-brick-test/counter-' + self.task['run_id']
        if operation == 'continuity_command':
            return {'command': 'python3 ~/farhelm-brick-test/continuity.py run ' + directory}
        script = ('from pathlib import Path; import runpy,json; '
                  'm=runpy.run_path(str(Path.home()/"farhelm-brick-test/continuity.py")); '
                  f'print(json.dumps(m["sample"](Path({directory!r}).expanduser())))')
        sample = json.loads(self.ssh(script)['stdout'])
        if operation.endswith('_before') and operation != 'continuity_before':
            key = operation.removeprefix('continuity_').removesuffix('_before')
            self.state[key + '_before'] = sample
            return sample
        before = self.state.get('original_counter') if operation == 'continuity_before' else self.state.get(
            operation.removeprefix('continuity_').removesuffix('_after') + '_before')
        if before is None:
            before = sample
        from continuity import require_progress
        deadline = time.monotonic() + 10
        while True:
            after = json.loads(self.ssh(script)['stdout'])
            if after['counter'] > before['counter']:
                break
            if time.monotonic() >= deadline:
                raise RuntimeError('original counter did not advance')
            time.sleep(0.2)
        require_progress(before, after)
        original = self.state.get('original_counter')
        if original is not None:
            for key in ('pid', 'start_ticks', 'boot_id'):
                if original[key] != after[key]:
                    raise RuntimeError('original continuity process was replaced')
        self.state['original_counter'] = after
        before_name = f'managed-counter-{operation}-before.json'
        after_name = f'managed-counter-{operation}-after.json'
        write_json(self.work / before_name, before)
        write_json(self.work / after_name, after)
        self.receipts.extend([before_name, after_name])
        self.state['continuity'] = {'before': before_name, 'after': after_name}
        return {'before': before, 'after': after}


def forward(native, message, emit, incoming=None, pending=None, observe=None):
    """Forward one native request to completion before reading another model action.

Serial forwarding is deliberate: a command failure must close the gate before
a following native click, even if the model submitted both without checking.
Native notifications and server-to-client requests are preserved while waiting
for the matching response. A roots/list request can occur during the first CUA
call; its client reply must reach the provider without opening another action
lane. Other model requests wait in the same serial queue.
"""
    if observe is not None:
        observe('to-native', message)
    native.stdin.write(json.dumps(message) + '\n')
    native.stdin.flush()
    if 'id' not in message or 'method' not in message:
        return None
    for line in native.stdout:
        response = json.loads(line)
        if observe is not None:
            observe('from-native', response)
        if response.get('id') == message['id'] and 'method' not in response:
            return response
        emit(response)
        if 'id' in response and 'method' in response:
            if incoming is None or pending is None:
                raise RuntimeError('native provider requested a client response without a protocol channel')
            for client_line in incoming:
                client = json.loads(client_line)
                if 'method' not in client or client.get('method', '').startswith('notifications/'):
                    if observe is not None:
                        observe('to-native', client)
                    native.stdin.write(json.dumps(client) + '\n')
                    native.stdin.flush()
                    if client.get('id') == response['id'] and 'method' not in client:
                        break
                else:
                    pending.append(client)
            else:
                raise RuntimeError('client closed before answering the native provider')
    raise RuntimeError('native provider closed before replying')


def serve(task, output, native, incoming, emit, operations=None, observe=None):
    """Expose native CUA plus a finite command tool behind one failure gate."""
    operations = Operations(task, output) if operations is None else operations
    ui = NativeUI(operations)
    incoming = iter(incoming)
    pending = deque()
    while True:
        try:
            message = pending.popleft() if pending else json.loads(next(incoming))
        except StopIteration:
            break
        if observe is not None:
            observe('from-client', message)
        method = message.get('method')
        try:
            native_message = message
            ui_parameters = None
            if method == 'tools/call':
                if operations.failure is not None:
                    raise RuntimeError('test action gate is closed: ' + operations.failure)
                parameters = message['params']
                if parameters['name'] == 'brick_action':
                    arguments = parameters.get('arguments', {})
                    if set(arguments) != {'operation'}:
                        raise RuntimeError('brick_action accepts only a named operation')
                    value = operations.dispatch(arguments['operation'])
                    emit({'jsonrpc': '2.0', 'id': message['id'], 'result': {
                        'content': [{'type': 'text', 'text': json.dumps(value)}]}})
                    continue
                if parameters['name'] != 'brick_ui':
                    raise RuntimeError('only structured native UI and fixed operations are available')
                ui_parameters = parameters['arguments']
                native_message = ui.prepare(message)
            response = forward(native, native_message, emit, incoming, pending, observe)
            if response is None:
                continue
            if method == 'tools/list' and 'result' in response:
                allowed = available_operations(task['check_id'], task['pass_id'], task['context'].get('update_path'))
                response['result']['tools'] = [tool_definition(), {'name': 'brick_action',
                    'description': 'Run one fixed release-test operation and retain its receipt. A failure blocks all later tools.',
                    'inputSchema': {'type': 'object', 'properties': {'operation': {'type': 'string', 'enum': sorted(allowed)}},
                                    'required': ['operation'], 'additionalProperties': False}}]
            if method == 'tools/call' and (response.get('error') or response.get('result', {}).get('isError')):
                # The event stream keeps the full provider reply. The failure
                # latch needs only text, never embedded screenshot bytes or
                # pages of first-use API documentation.
                result = response.get('result', {})
                detail = response.get('error') or [block.get('text', '') for block in result.get('content', [])
                                                   if block.get('type') == 'text']
                operations.fail('native operation failed: ' + json.dumps(detail))
            elif ui_parameters is not None:
                ui.complete(ui_parameters, response)
                if ui_parameters['method'] == 'restart':
                    # A native click and an AX read must not straddle process
                    # teardown in the same JS call. The process oracle owns
                    # this handoff before a second native request is allowed.
                    rebind = ui.settle_restart(message)
                    response = forward(native, rebind, emit, incoming, pending, observe)
                    if response.get('error') or response.get('result', {}).get('isError'):
                        raise RuntimeError('native candidate rebind failed after process readiness')
            emit(response)
        except Exception as error:
            operations.fail(f'{type(error).__name__}: {error}')
            emit({'jsonrpc': '2.0', 'id': message.get('id'), 'result': {
                'isError': True, 'content': [{'type': 'text', 'text': str(error)}]}})


def main():
    """Launch the official guest native provider without another host permission surface."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--task', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    task = json.loads(args.task.read_text())
    # A lost bridge may have completed an installer or click before its reply
    # was lost. Refuse an automatic client restart instead of replaying actions.
    write_json(args.output / 'managed-bridge.json', {'attempt': task['attempt'], 'started': True})
    resources = Path(task['settings']['desktop_app']) / 'Contents/Resources/cua_node'
    argv = [str(resources / 'bin/node'), str(resources / 'lib/node_modules/@oai/cua-repl/bin/cua-repl.mjs')]
    protocol = args.output / 'managed-protocol.jsonl'
    def observe(direction, value):
        """Retain bounded protocol progress without tool arguments, images or private result bodies."""
        if protocol.exists() and protocol.stat().st_size > 256 * 1024:
            raise RuntimeError('managed protocol diagnostics exceed their bound')
        with protocol.open('a') as stream:
            os.chmod(protocol, 0o600)
            row = {'direction': direction, 'method': value.get('method'),
                   'id': value.get('id'), 'error': bool(value.get('error'))}
            if direction == 'from-native' and value.get('method') == 'elicitation/create':
                metadata = value.get('params', {}).get('_meta', {})
                row['approval_tool'] = {key: metadata.get(key) for key in ('connector_id', 'tool_name')}
            stream.write(json.dumps(row) + '\n')
    def emit(value):
        """Keep stdout exclusively MCP protocol; diagnostics belong to stderr."""
        observe('to-client', value)
        print(json.dumps(value), flush=True)
    with subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True) as native:
        try:
            serve(task, args.output, native, sys.stdin, emit, observe=observe)
        finally:
            native.terminate()
            native.wait(timeout=10)


if __name__ == '__main__':
    main()
