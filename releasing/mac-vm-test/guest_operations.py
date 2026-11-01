#!/usr/bin/env python3
"""Keep release-test commands and evidence outside the UI model's discretion.

The model chooses native UI actions and reports what it sees. This process owns
the finite command vocabulary, durable receipts and native-tool forwarding. A
failed operation closes the action gate before another tool call can execute;
returning an error to the model alone would still let a batched click run.
"""

import argparse
import calendar
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

from checkpoint_contract import SIDES, available_operations, host_identity, require_applicable_operation
from guest_ui import NativeUI, tool_definition, wait_running
from window_geometry import calibrate, select_frame


LIMIT = 512 * 1024
OPERATIONS = ('prerequisites', 'verify_candidate', 'verify_previous', 'fixture_clean', 'install_candidate',
              'install_previous', 'launch', 'launch_override', 'versions', 'remote_version',
              'update_logs', 'public_latest', 'inspect_state', 'remote_inspect_state', 'terminal_hosts',
              'remote_terminal_hosts', 'host_survival', 'remote_host_survival', 'old_session_after',
              'remote_old_session_after', 'continuity_command',
              'continuity_before', 'continuity_remote_before', 'continuity_remote_after',
              'continuity_reopen_before', 'continuity_reopen_after', 'prior_receipts', 'window_frame', 'finder_frame')
# Only the test's session identity columns. `terminal_kind` is the stored kind
# once the product keeps one; schema introspection drops any absent column, so
# older releases' databases keep the same observation contract.
SESSION_COLUMNS = ('id', 'name', 'generation', 'captured_conversation', 'conversation_source', 'terminal_kind')
# Like remote_version's default install path, the Linux scripts assume the
# default state directory, `~/.local/state/farhelm`. A base that gave the
# provisioned supervisor an XDG_STATE_HOME would need them to follow it.
REMOTE_INSPECT = '''from pathlib import Path
import contextlib,json,sqlite3
p=Path.home()/'.local/state/farhelm/supervisor.db'
wanted=%r
observed={'present':False}
if p.is_file():
 with contextlib.closing(sqlite3.connect(p.as_uri()+'?mode=ro',uri=True)) as c:
  actual={row[1] for row in c.execute('PRAGMA table_info(sessions)')}
  columns=[name for name in wanted if name in actual]
  observed={'sessions':[dict(zip(columns,row)) for row in c.execute('SELECT '+','.join(columns)+' FROM sessions LIMIT 100')]} if columns else {}
print(json.dumps({'supervisor.db':observed}))
''' % (SESSION_COLUMNS,)
# Linux has exact argv (NUL-separated) and kernel start ticks. The script itself
# keeps only hosts whose last two arguments are `--socket` and a path in this
# state directory's hosts/, plus their direct children, and sends no other
# command line, not even the children's, off the guest.
REMOTE_HOSTS = '''from pathlib import Path
import json
prefix=str(Path.home()/'.local/state/farhelm/hosts')+'/'
boot=Path('/proc/sys/kernel/random/boot_id').read_text().strip()
rows=[]
for entry in Path('/proc').iterdir():
 if not entry.name.isdigit():
  continue
 try:
  argv=[part.decode(errors='replace') for part in (entry/'cmdline').read_bytes().split(b'\\0') if part]
  fields=(entry/'stat').read_text().rsplit(')',1)[1].split()
  row=dict(pid=int(entry.name),ppid=int(fields[1]),start=dict(start_ticks=int(fields[19]),boot_id=boot))
 except (OSError,ValueError,IndexError):
  continue
 if fields[0] in ('Z','X'):
  continue
 socket=argv[-1] if len(argv)>=2 and argv[-2]=='--socket' else ''
 row['args']=' --socket '+socket if socket.startswith(prefix) else ''
 rows.append(row)
hosts={row['pid'] for row in rows if row['args']}
print(json.dumps(dict(hosts_dir=prefix[:-1],rows=[row for row in rows if row['pid'] in hosts or row['ppid'] in hosts])))
'''


def parse_mac_processes(table):
    """Read `ps -o pid=,ppid=,stat=,lstart=,args=` rows: host candidates and their children, plus a skip count.

`lstart` is the kernel's process start time printed to the second under the C
locale: with the pid it is the Mac's platform start identity here, since pid
reuse within one second on a quiet guest is not a realistic collision. Zombies
are not live processes. A line that does not parse (a command line with an
unescaped newline, say) is skipped and counted rather than ending the
observation: hosts and their children always parse. Only processes whose
arguments contain ` --socket ` keep their command line here, and children keep
none; `select_hosts` then keeps nothing but a matching socket path, so no
other process's arguments reach a receipt.
"""
    rows, unparsed = [], 0
    for line in table.splitlines():
        parts = line.split(None, 8)
        try:
            pid, ppid = int(parts[0]), int(parts[1])
        except (ValueError, IndexError):
            unparsed += 1
            continue
        if len(parts) < 8:
            unparsed += 1
            continue
        if parts[2].startswith('Z'):
            continue
        rows.append({'pid': pid, 'ppid': ppid, 'start': ' '.join(parts[3:8]),
                     'args': parts[8] if len(parts) == 9 else ''})
    hosts = {row['pid'] for row in rows if ' --socket ' in row['args']}
    kept = [dict(row, args=row['args'] if row['pid'] in hosts else '') for row in rows
            if row['pid'] in hosts or row['ppid'] in hosts]
    return kept, unparsed


def select_hosts(rows, hosts_dir):
    """Identify terminal hosts by their socket in the supervisor's host directory, with their children.

A host's only argument is its socket (terminal-host SPEC.md, "Command line and
child spec"), and the supervisor keeps those sockets directly in
`<state dir>/hosts/`. Matching that, rather than a program name, keeps the
observation independent of which build started the host. Children are reported
as found; whether exactly one live child is required is the caller's premise.
"""
    prefix = hosts_dir.rstrip('/') + '/'
    hosts = []
    for row in rows:
        _, marker, socket = row['args'].rpartition(' --socket ')
        name = socket[len(prefix):]
        if not marker or not socket.startswith(prefix) or not name or '/' in name:
            continue
        children = sorted(({'pid': child['pid'], 'start': child['start']} for child in rows
                           if child['ppid'] == row['pid']), key=lambda child: child['pid'])
        hosts.append({'pid': row['pid'], 'start': row['start'], 'socket': socket, 'children': children})
    return sorted(hosts, key=lambda host: host['pid'])


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

    def command(self, argv, *, cwd=None, env=None, data=None, timeout=60, private_output=False):
        """Check each fixed command before its caller can perform a later action.

`private_output` marks a command whose stdout can hold other processes' command
lines (a full process table). Its caller filters that output before anything
is retained, so a failure keeps only the exit status, the byte count and a
bounded stderr: a raw table must never reach a receipt, whatever the outcome.
Such output is held in memory only, so it gets a larger bound than receipts.
"""
        result = self.runner(argv, cwd=cwd, env=env, input=data, capture_output=True, timeout=timeout)
        row = {'argv': argv, 'returncode': result.returncode,
               'stdout': result.stdout.decode(errors='replace'), 'stderr': result.stderr.decode(errors='replace')}
        if len(json.dumps(row).encode()) > (16 * 1024 * 1024 if private_output else LIMIT):
            raise RuntimeError('managed command output exceeds its bound')
        if result.returncode:
            # Preserve the exact failed command too; a failure must not require
            # a second model-written script to explain what happened.
            retained = row if not private_output else dict(
                row, stdout=f'<{len(result.stdout)} bytes withheld: may hold other processes\' command lines>',
                stderr=row['stderr'][:2000])
            self.receipt('failed-command', retained)
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
            # The survival comparison is an observation that also runs once:
            # repeated after the old session's Restart it would observe the
            # legitimate replacement and overwrite the comparison that counts.
            if operation in self.completed and operation in (
                    'install_previous', 'install_candidate', 'launch', 'launch_override',
                    'host_survival', 'remote_host_survival'):
                raise RuntimeError('managed mutation or one-time observation already completed; it cannot be replayed')
            value = self.perform(operation)
            name = self.receipt(operation.replace('_', '-'), value)
            self.completed.add(operation)
            self.observations[operation] = value
            self.save_record()
            return {'operation': operation, 'receipt': name, 'observation': value}
        except Exception as error:
            self.fail(f'{type(error).__name__}: {error}')
            raise

    def ssh(self, script, private_output=False):
        """Use the manifest-pinned peer and fixed Python, never a model command string."""
        return self.command(['/usr/bin/ssh', '-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=yes',
            self.context['ssh']['alias'], '/usr/bin/python3', '-'], data=script.encode(), timeout=30,
            private_output=private_output)

    def version(self):
        """Separate installed payloads from the live app and the supervisor it uses.

The GUI executable has an unversioned bundle path. Its supervisor uses the
versioned payload path; old session reporters and terminal hosts can outlive
app updates and must not be mistaken for it. A supervisor the app started is
its direct child. Since the app starts it detached, it outlives a quit, and a
later launch of the same app reuses it: by then it is no longer the app's
child, so the app's own supervisor record (`desktop-supervisor.json`) names
it instead. As the product's own stop does, the record is trusted only when
the live process matches all of it: pid, program path and start time (to the
second `ps` shows). It counts only while an app runs, so this still reports
the running app's supervisor, not a leftover one. The native UI readout
separately establishes the GUI's in-memory release, and PID changes establish
its restart.
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
        record = self.recorded_supervisor() if desktop else None
        for row in rows:
            if len(row) != 3:
                continue
            recorded = record is not None and int(row[0]) == record['pid'] and row[2] == record['program']
            if not (int(row[1]) in desktop or (recorded and self.started_at(record))):
                continue
            match = re.fullmatch(re.escape(str(app / 'Contents/Versions')) + r'/([^/]+)/farhelm', row[2])
            if match:
                result['running_versions'].append(match[1])
        result['running_versions'] = sorted(set(result['running_versions']))
        result['desktop_pids'] = sorted(desktop)
        result['desktop_running'] = bool(desktop)
        return result

    def recorded_supervisor(self):
        """Read the app's supervisor record, or None when absent, unreadable or incomplete.

Older releases tether their supervisor and write no record.
"""
        try:
            value = json.loads((self.home / '.local/state/farhelm/desktop-supervisor.json').read_text())
        except (OSError, ValueError):
            return None
        if (not isinstance(value, dict) or type(value.get('pid')) is not int or type(value.get('start')) is not int
                or not isinstance(value.get('program'), str)):
            return None
        return value

    def started_at(self, record):
        """Report whether the recorded pid's live process started in the record's start second.

The record keeps the kernel start time in microseconds since the epoch; `ps`
prints it to the second, here in UTC so that `timegm` reads it back without
the local daylight-saving ambiguity of one autumn hour. A pid that no longer
exists is an ordinary answer (False), not a failed command.
"""
        result = self.runner(['/bin/ps', '-p', str(record['pid']), '-o', 'lstart='], cwd=None,
                             env=dict(os.environ, LC_ALL='C', TZ='UTC'), input=None, capture_output=True, timeout=30)
        text = ' '.join(result.stdout.decode(errors='replace').split())
        if result.returncode or not text:
            return False
        try:
            started = calendar.timegm(time.strptime(text, '%a %b %d %H:%M:%S %Y'))
        except ValueError:
            return False
        return started == record['start'] // 1_000_000

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
            return [dict(check_id=row['check_id'], operation=row['operation'], value=self.read_receipt(row))
                    for row in self.state['receipts'] if row['check_id'] != check]
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
        if operation == 'remote_inspect_state':
            return json.loads(self.ssh(REMOTE_INSPECT)['stdout'])
        if operation in ('terminal_hosts', 'remote_terminal_hosts'):
            return self.terminal_hosts('remote' if operation.startswith('remote_') else 'local')
        if operation in ('host_survival', 'remote_host_survival'):
            return self.host_survival('remote' if operation.startswith('remote_') else 'local')
        if operation in ('old_session_after', 'remote_old_session_after'):
            return self.old_session_after('remote' if operation.startswith('remote_') else 'local')
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

    def read_receipt(self, row):
        """Return a retained receipt's value only while its bytes still match the recorded digest."""
        path = Path(row['path'])
        if path.is_symlink() or not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != row['sha256']:
            raise RuntimeError('a prior managed receipt changed or disappeared')
        return json.loads(path.read_text())

    def prior_value(self, check, operation, required=True):
        """Read the latest retained value of one earlier checkpoint's operation in this pass.

This is how a fresh tester turn's machine-owned comparison reaches the old
fixture's observations, without the model relaying them: the pass's managed
state outlives each attempt, and the digest binding refuses a changed file.
"""
        name = operation.replace('_', '-')
        rows = [row for row in self.state['receipts'] if row['check_id'] == check and row['operation'] == name]
        if not rows:
            if required:
                raise RuntimeError(f'{check} retained no {name} receipt in this pass')
            return None
        return self.read_receipt(rows[-1])

    def sample_hosts(self, side):
        """Observe every live terminal host on one side of the pair, with its children's identities.

The Mac reads its process table through `ps`; the Linux peer reads `/proc`
over the pinned SSH alias, so its start identity is the kernel's start ticks
plus boot identity, as in the continuity oracle. Both outputs can name other
processes, so neither reaches a receipt raw, even on failure. Returns the
hosts and how many process-table lines could not be parsed.
"""
        if side == 'remote':
            observed = json.loads(self.ssh(REMOTE_HOSTS, private_output=True)['stdout'])
            return select_hosts(observed['rows'], observed['hosts_dir']), 0
        table = self.command(['/bin/ps', '-axww', '-o', 'pid=,ppid=,stat=,lstart=,args='],
                             env=dict(os.environ, LC_ALL='C'), private_output=True)['stdout']
        rows, unparsed = parse_mac_processes(table)
        return select_hosts(rows, str(self.home / '.local/state/farhelm/hosts')), unparsed

    def sample_sessions(self, side):
        """Read the side's session rows (the fixed identity columns only) at this moment."""
        if side == 'remote':
            observed = json.loads(self.ssh(REMOTE_INSPECT)['stdout'])
        else:
            observed = self.inspect_state()
        return observed.get('supervisor.db', {}).get('sessions', [])

    def old_session(self, side):
        """Return the old fixture's one captured session row for this side, from its digest-bound receipt."""
        rows = [row for row in self.prior_value('old-fixture', SIDES[side]['state']).get(
            'supervisor.db', {}).get('sessions', []) if row.get('captured_conversation')]
        if len(rows) != 1:
            raise RuntimeError('the old fixture retained no single captured session to compare')
        return {key: rows[0][key] for key in SESSION_COLUMNS if key in rows[0]}

    def terminal_hosts(self, side):
        """Record the side's terminal hosts, refusing a sample taken before the moment it stands for.

At the updater pair's old fixture this is the baseline (elsewhere an old
fixture starts no session, and the sample is a plain diagnostic), so it is
refused until this attempt
has observed the side's one captured session: sampled earlier, an empty list
would misread a host-run session as tmux. The baseline is either no host (the
previous release runs the old session in tmux) or exactly that session's one
host with its one live child; more would make the survival comparison
ambiguous, and a host contradicts a previous release whose source has none.
At the new-session check it reads the session rows itself and is refused
until a session other than the old one exists; it reports the new rows (with
their stored terminal kind, where the schema has one) beside the hosts the old
fixture did not see, so the import can compare the two. No later decision
depends on when these samples were taken: the old-session check samples again.
"""
        names = SIDES[side]
        check = self.task['check_id']
        baseline_turn = check == 'old-fixture' and self.task['pass_id'] == 'p0-updater-upgrade'
        if check == 'old-fixture' and not baseline_turn:
            # Other old fixtures start no session: this is only a diagnostic.
            hosts, unparsed = self.sample_hosts(side)
            return {'hosts': hosts, 'unparsed_lines': unparsed}
        if baseline_turn:
            rows = self.observations.get(names['state'], {}).get('supervisor.db', {}).get('sessions', [])
            if len(rows) != 1 or not rows[0].get('captured_conversation'):
                raise RuntimeError('sample the old fixture terminal hosts only after this attempt observed '
                                   'its one captured session')
        hosts, unparsed = self.sample_hosts(side)
        if baseline_turn:
            if len(hosts) > 1 or any(len(host['children']) != 1 for host in hosts):
                raise RuntimeError('the old fixture needs at most one terminal host, with one live child')
            if hosts and self.context.get('previous_can_create_hosts') is False:
                raise RuntimeError('a terminal host contradicts the previous release source, which has none')
            return {'hosts': hosts, 'unparsed_lines': unparsed}
        baseline = self.prior_value('old-fixture', names['hosts'], required=False)
        known = {host_identity(host) for host in (baseline or {'hosts': []})['hosts']}
        value = {'hosts': hosts, 'new_hosts': [host for host in hosts if host_identity(host) not in known],
                 'unparsed_lines': unparsed}
        if check == names['agent_check']:
            old_id = self.old_session(side)['id']
            value['new_sessions'] = [row for row in self.sample_sessions(side) if row.get('id') != old_id]
            if not value['new_sessions']:
                raise RuntimeError('sample the new session terminal hosts only after the new agent session exists')
        return value

    def host_survival(self, side):
        """Compare the old session and its terminal with the old fixture, from samples this operation takes, once.

Every input to the branch is taken here, before the old session is touched,
or read from the old fixture's digest-bound receipts; none depends on when
the tester called an earlier observation. No host at the old fixture means
the old session ran in tmux, one host that it ran in a terminal host. Either
way it survived only if its session row keeps its id and launch generation
and still has a captured conversation; in the host branch also the same host process, socket
and child (pid and start identity), so a replacement that restored the screen
and resumed the conversation still reads as not survived. Whether the
candidate creates hosts comes from hosts present now that the old fixture did
not have (the new agent session's, which must still exist), cross-checked
against the new sessions' stored terminal kind where the schema has it. Called
after a Restart, every branch reads as not survived: a late call can only be
refused, never pass.
"""
        names = SIDES[side]
        old_hosts = self.prior_value('old-fixture', names['hosts'])['hosts']
        old = self.old_session(side)
        current, unparsed = self.sample_hosts(side)
        sessions = self.sample_sessions(side)
        rows = [row for row in sessions if row.get('id') == old['id']]
        new_sessions = [row for row in sessions if row.get('id') != old['id']]
        if not new_sessions:
            raise RuntimeError('the new agent session from the agent check is gone; it decides what the candidate creates')
        branch = 'host' if old_hosts else 'tmux'
        row = rows[0] if len(rows) == 1 else None
        session_survived = (row is not None and row.get('generation') == old.get('generation')
                            and bool(row.get('captured_conversation')))
        observed = None
        if old_hosts:
            observed = next((host for host in current if host['pid'] == old_hosts[0]['pid']), None)
        host_survived = observed == old_hosts[0] if old_hosts else None
        baseline = {host_identity(host) for host in old_hosts}
        creates = any(host_identity(host) not in baseline for host in current)
        disagreements = []
        kinds = [entry['terminal_kind'] for entry in new_sessions if 'terminal_kind' in entry]
        if kinds and ('host' in kinds) != creates:
            disagreements.append(f'new sessions store terminal_kind {kinds} but a new host is {"" if creates else "not "}running')
        for label, entry in (('old fixture', old), ('current', row or {})):
            if 'terminal_kind' in entry and entry['terminal_kind'] != branch:
                disagreements.append(f'{label} old session stores terminal_kind {entry["terminal_kind"]!r}, '
                                     f'not the observed {branch!r}')
        return {'branch': branch, 'survived': session_survived and host_survived is not False,
                'session_survived': session_survived, 'host_survived': host_survived,
                'old_fixture_host': old_hosts[0] if old_hosts else None, 'observed_host': observed,
                'old_session': old, 'current_session': row, 'candidate_creates_hosts': creates,
                'restart_required': branch == 'host' or creates, 'pre_restart_hosts': current,
                'kind_disagreements': disagreements, 'unparsed_lines': unparsed}

    def old_session_after(self, side):
        """Take the old-session check's final sample: its session row and the hosts, after any Restart.

Refused until `host_survival` ran in this attempt, so "after" is relative to a
comparison the coordinator owns. Hosts are compared with that comparison's
own pre-Restart sample, so a host counts as the restarted session's only if
it appeared since. A call made before the Restart shows the old launch
generation and no new host, which the import refuses when a Restart was due.
"""
        names = SIDES[side]
        if names['survival'] not in self.completed:
            raise RuntimeError(f'call {names["survival"]} before observing the old session afterwards')
        survival = self.observations[names['survival']]
        hosts, unparsed = self.sample_hosts(side)
        rows = [row for row in self.sample_sessions(side) if row.get('id') == survival['old_session']['id']]
        before = {host_identity(host) for host in survival['pre_restart_hosts']}
        identities = {host_identity(host) for host in hosts}
        old_host = survival['old_fixture_host']
        return {'session': rows[0] if len(rows) == 1 else None, 'hosts': hosts,
                'appeared_hosts': [host for host in hosts if host_identity(host) not in before],
                'old_host_present': old_host is not None and host_identity(old_host) in identities,
                'unparsed_lines': unparsed}

    def inspect_state(self):
        """Read only the test's session, template and display-setting columns.

Never return token-bearing tables or arbitrary SQL. Schema introspection lets
the historical upgrade fixtures expose the same bounded observation contract.
"""
        import contextlib
        import sqlite3
        result = {}
        for database, tables in (('supervisor.db', {'sessions': SESSION_COLUMNS}),
                                 ('helm.db', {'launch_templates': ('name', 'fields_json'),
                                              'preferences': ('compact', 'codex_trust', 'tunnel_consent', 'color_terminal_frames')})):
            path = self.home / '.local/state/farhelm' / database
            if not path.is_file():
                result[database] = {'present': False}
                continue
            # A connection's own context manager only ends a transaction; close it too.
            with contextlib.closing(sqlite3.connect(path.as_uri() + '?mode=ro', uri=True)) as connection:
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
