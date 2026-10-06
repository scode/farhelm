#!/usr/bin/env python3
"""Operate disposable Tart guests without claiming pre-existing VMs.

The manifest records a random namespace and each successful clone's MAC address.
A name alone is insufficient authority for cleanup: a replaced VM at that name
must be refused. Every mutation gets an intent before execution and a result
afterward in the operator's append-only journal. Guest input is never journaled.
This controls the fixture; GUI checks and release verdicts remain agent work.
"""

import argparse
import contextlib
import fcntl
import json
import os
from pathlib import Path
import re
import shlex
import socket
import subprocess
import sys
import time
import uuid
from datetime import datetime, timezone


class Refused(RuntimeError):
    """The fixture lacks evidence that a requested mutation is owned and safe."""


def atomic_json(path, value):
    """Keep the previous ownership record if serializing or writing fails.

The per-run lock serializes callers. Replacement, rather than rewriting in
place, prevents an interrupted write from turning a manifest into partial JSON.
"""
    temporary = path.with_suffix('.incoming')
    with temporary.open('w') as stream:
        os.chmod(temporary, 0o600)
        json.dump(value, stream, indent=2)
        stream.write('\n')
        stream.flush()
        os.fsync(stream.fileno())
    temporary.replace(path)


class Control:
    """Bind all VM mutations to one private manifest and external journal.

The command runner is injected so refusal tests can prove no Tart invocation
occurred. Child environment overrides do not modify this process's environment.
The operator must authorize guest commands; this is ownership checking, not a
sandbox for arbitrary commands supplied by another party.
"""

    def __init__(self, directory, journal, tart_home, tart='tart', runner=subprocess.run):
        self.directory = Path(directory)
        self.journal = Path(journal)
        self.tart_home = Path(tart_home)
        self.tart = tart
        self.runner = runner
        self.manifest_path = self.directory / 'manifest.json'
        self.data = None
        # Retain a spawned child before startup validation can fail. Callers must
        # keep supervising these handles even when start() never returns one.
        self.processes = []

    def command(self, *args, **kwargs):
        """Use the same Tart home for inspection and mutations, with pruning off."""
        child_environment = dict(os.environ, TART_HOME=str(self.tart_home), TART_NO_AUTO_PRUNE='1')
        return self.runner([self.tart, *args], env=child_environment, check=True,
                           timeout=kwargs.pop('timeout', 30), **kwargs)

    def record(self, kind, event, command, detail):
        """Refuse to mutate when the append-only intent cannot be persisted.

Stdout, guest input, and credentials never enter this journal. Private output
belongs in the run directory, whose location is recorded as an evidence label.
"""
        stamp = datetime.now(timezone.utc).isoformat(timespec='seconds')
        entry = (f'\n## {stamp} | {kind} | {event}\n\n'
                 f'- Host: `{socket.gethostname()}`\n'
                 f'- Actor: Farhelm Tart control script\n'
                 f'- Run: `{self.data["run_id"]}`\n'
                 f'- Command: `{command}`\n- Detail: {detail}\n')
        with self.journal.open('a') as stream:
            fcntl.flock(stream, fcntl.LOCK_EX)
            stream.write(entry)
            stream.flush()
            os.fsync(stream.fileno())

    @contextlib.contextmanager
    def locked(self):
        """Serialize manifest changes; do not inherit the lock in guest jobs."""
        with (self.directory / '.lock').open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            self.data = json.loads(self.manifest_path.read_text())
            uuid.UUID(self.data['run_id'])
            if self.data.get('format') != 1:
                raise Refused('unsupported manifest format')
            yield

    def save(self):
        """Persist ownership before returning control to another invocation."""
        atomic_json(self.manifest_path, self.data)

    def identity(self, name):
        """Read the VM's hardware identity, refusing absent or redirected disks."""
        directory = self.tart_home / 'vms' / name
        if directory.is_symlink():
            raise Refused('VM directory is a symbolic link')
        value = json.loads((directory / 'config.json').read_text())['macAddress']
        if not isinstance(value, str) or not value:
            raise Refused('VM has no usable MAC address')
        return value

    def owned(self, role):
        """Require both the minted name and unchanged hardware before mutation."""
        if role not in ('mac', 'linux'):
            raise Refused('unknown guest role')
        entry = self.data['vms'].get(role)
        expected = f'fh-{self.data["run_id"]}-{role}'
        if not entry or entry.get('name') != expected or entry.get('phase') != 'ready':
            raise Refused('no successfully created owned clone for this role')
        if self.identity(expected) != entry['mac_address']:
            raise Refused('VM identity changed; refusing the replacement')
        return expected

    def mutate(self, args, detail, **kwargs):
        """Record failures and timeouts without treating them as successful state."""
        event = str(uuid.uuid4())
        display = shlex.join([self.tart, *args])
        self.record('intent', event, display, detail)
        try:
            result = self.command(*args, **kwargs)
        except BaseException as error:
            self.record('result', event, display, f'Failed: {type(error).__name__}; inspect live state before retry.')
            raise
        self.record('result', event, display, 'Command exited 0; operation-specific poststate still applies.')
        return result

    def clone(self, role, source):
        """Create one fresh clone, refusing collisions and unfinished attempts.

A pending record is retained across interruption. It deliberately grants no
cleanup authority until successful cloning and identity capture are complete;
an agent must reconcile an interrupted clone rather than guess what happened.
"""
        if role not in ('mac', 'linux'):
            raise Refused('unknown guest role')
        if role in self.data['vms']:
            raise Refused('role already recorded; use a new run for a fresh pass')
        name = f'fh-{self.data["run_id"]}-{role}'
        listed = json.loads(self.command('list', '--source', 'local', '--format', 'json', capture_output=True).stdout)
        if any(item['Name'] == name for item in listed):
            raise Refused('name already exists; no ownership claimed')
        self.data['vms'][role] = {'name': name, 'source': source, 'phase': 'creating'}
        self.save()
        self.mutate(['clone', source, name], 'Create a new owned clone; automatic pruning disabled.', timeout=1800)
        entry = self.data['vms'][role]
        entry.update(mac_address=self.identity(name), phase='ready')
        self.save()
        self.record('decision', str(uuid.uuid4()), 'clone poststate', f'Owned clone `{name}` recorded with hardware identity.')

    def state(self, role):
        """Inspect the exact owned VM; historical journal entries are not state."""
        name = self.owned(role)
        return json.loads(self.command('get', name, '--format', 'json', capture_output=True).stdout)

    def start(self, role, headless=False, spawner=subprocess.Popen):
        """Return the owning Tart process only after live running state is observed.

The startup intent is closed while the VM runs, rather than remaining unmatched
until shutdown hours later. The manifest retains the owned process PID for
diagnosis, but later code must not signal that numeric PID without establishing
ownership again. self.processes retains every spawned handle even if validation
raises, so callers can keep supervising a failed start. No timeout force-stops it.
"""
        name = self.owned(role)
        if self.state(role)['State'] != 'stopped':
            raise Refused('guest is not stopped')
        argv = [self.tart, 'run', '--no-audio', '--no-clipboard']
        if headless:
            argv.append('--no-graphics')
        argv.append(name)
        event = str(uuid.uuid4())
        display = shlex.join(argv)
        self.record('intent', event, display, 'Start owned VM; foreground caller retains its process.')
        try:
            child_env = dict(os.environ, TART_HOME=str(self.tart_home), TART_NO_AUTO_PRUNE='1')
            process = spawner(argv, env=child_env)
            self.processes.append(process)
            self.data['vms'][role]['run_pid'] = process.pid
            self.save()
            deadline = time.monotonic() + 30
            while True:
                if process.poll() is not None:
                    raise Refused('Tart exited before running state was established')
                if self.state(role)['State'] == 'running':
                    break
                if time.monotonic() >= deadline:
                    raise Refused('startup timed out; inspect the recorded process and live VM')
                time.sleep(0.1)
        except BaseException as error:
            self.record('result', event, display, f'Startup failed: {type(error).__name__}; inspect live state, no forced-stop fallback.')
            raise
        self.record('result', event, display, f'Live VM state is running; owned Tart process PID {process.pid}. Guest readiness is separate.')
        return process

    def ready(self, role, timeout):
        """Poll actual command/desktop or user-manager readiness within a bound.

An assigned IP is not readiness. Each probe has its own timeout, and the final
error retains the last diagnostic without copying potentially private stdout
into the historical journal. GUI screenshots remain a separate required check.
"""
        name = self.owned(role)
        probe = ('test "$(/usr/sbin/sysctl -n kern.hv_vmm_present)" = 1 && '
                 'test "$(/usr/bin/stat -f %Su /dev/console)" = "$(id -un)"') if role == 'mac' else (
                 'test -S "/run/user/$(id -u)/bus" && '
                 'XDG_RUNTIME_DIR="/run/user/$(id -u)" systemctl --user show-environment >/dev/null && '
                 'systemctl is-active --quiet ssh')
        deadline = time.monotonic() + timeout
        last = 'no successful guest command'
        while time.monotonic() < deadline:
            try:
                remaining = max(0.1, min(5, deadline - time.monotonic()))
                self.command('exec', name, '/bin/sh', '-c', probe, capture_output=True, timeout=remaining)
                print(f'{role}: command readiness established')
                return
            except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
                last = type(error).__name__
            # Polling backs off between explicit readiness probes; elapsed time
            # alone never marks the fixture ready.
            time.sleep(min(0.5, max(0, deadline - time.monotonic())))
        raise Refused(f'{role}: readiness timed out ({last}); collect guest diagnostics')

    def run_foreground(self, role, headless=False, spawner=subprocess.Popen):
        """Own every spawned Tart child through startup failure and guest shutdown.

The manifest lock is released before waiting, so another invocation can inspect
or cleanly stop a failed startup. Report the failure immediately, then retain
the foreground session until its child ends; a saved numeric PID is no substitute.
"""
        try:
            with self.locked():
                self.start(role, headless, spawner)
        except BaseException as error:
            print(f'Startup failed: {error}; retained children remain supervised until guest shutdown.',
                  file=sys.stderr, flush=True)
            raise
        finally:
            for process in self.processes:
                status = process.wait()
                self.record('decision', str(uuid.uuid4()), 'foreground Tart process ended',
                            f'Owned process exited {status}; guest shutdown or live inspection establishes final VM state.')
                if status != 0:
                    raise Refused(f'Tart process ended with exit status {status}')

    def delete(self, role):
        """Delete only an unchanged owned VM whose live state is stopped."""
        name = self.owned(role)
        if self.state(role)['State'] != 'stopped':
            raise Refused('guest must be shut down before deletion')
        self.mutate(['delete', name], 'Delete stopped owned clone after evidence export.')
        listed = json.loads(self.command('list', '--source', 'local', '--format', 'json', capture_output=True).stdout)
        if any(item['Name'] == name for item in listed):
            raise Refused('delete command exited but VM is still listed')
        self.data['vms'][role]['phase'] = 'deleted'
        self.save()

    def shutdown(self, role, timeout=60, clock=time.monotonic, pause=time.sleep):
        """Require stopped guest state even if shutdown closes its command channel.

macOS may terminate the guest agent before Tart receives the command's exit
status. Preserve that failed-command journal entry, then inspect the owned VM;
only observed stopped state satisfies shutdown. A still-running guest is left
alone for diagnosis rather than forcibly stopped or retried.
"""
        name = self.owned(role)
        try:
            self.mutate(['exec', name, '/usr/bin/sudo', '-n', '/sbin/shutdown', '-h', 'now'],
                        'Clean guest shutdown requested; no forced host stop fallback.')
        except subprocess.CalledProcessError:
            pass
        deadline = clock() + timeout
        while self.state(role)['State'] != 'stopped':
            if clock() >= deadline:
                raise Refused('guest did not stop; preserve it for diagnosis')
            pause(0.5)
        self.record('decision', str(uuid.uuid4()), 'shutdown poststate', 'Live Tart state is stopped.')

    def collect(self, role, guest_directory, label, spawner=subprocess.Popen):
        """Export guest evidence privately with time and disk bounds.

Only explicitly named evidence directories should be collected; never collect
the whole guest home or authentication stores. A failed copy stays as a partial
archive for diagnosis. No archive is silently overwritten, published, or counted
as complete merely because a child exited successfully.
"""
        if not re.fullmatch(r'[a-zA-Z0-9][a-zA-Z0-9_-]*', label):
            raise Refused('evidence label must be a single portable filename')
        target = self.directory / f'{label}.tar'
        partial = self.directory / f'{label}.partial'
        if target.exists() or partial.exists():
            raise Refused('evidence label already exists')
        name = self.owned(role)
        argv = [self.tart, 'exec', name, '/usr/bin/tar', '-C', guest_directory, '-cf', '-', '.']
        event = str(uuid.uuid4())
        display = shlex.join(argv)
        self.record('intent', event, display, f'Export private evidence to `{target}`; limit 128 MiB / 60 seconds.')
        with partial.open('xb') as stream:
            os.chmod(partial, 0o600)
            child_env = dict(os.environ, TART_HOME=str(self.tart_home), TART_NO_AUTO_PRUNE='1')
            try:
                process = spawner(argv, stdout=stream, stderr=subprocess.DEVNULL, env=child_env)
            except BaseException:
                self.record('result', event, display, f'Export launch failed; empty partial artifact retained at `{partial}`.')
                raise
            deadline = time.monotonic() + 60
            try:
                while process.poll() is None:
                    if partial.stat().st_size > 128 * 1024 * 1024 or time.monotonic() >= deadline:
                        raise Refused('evidence copy exceeded its size or time bound')
                    # Poll the owned child and artifact size; no name-pattern
                    # process search can accidentally count this watcher.
                    time.sleep(0.1)
                if process.returncode != 0 or partial.stat().st_size > 128 * 1024 * 1024:
                    raise Refused('evidence export failed or exceeded its size bound')
            except BaseException:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
                self.record('result', event, display, f'Export failed; partial artifact retained at `{partial}`.')
                raise
            stream.flush()
            os.fsync(stream.fileno())
        partial.replace(target)
        self.record('result', event, display, f'Export completed at `{target}` ({target.stat().st_size} bytes); inspect archive before teardown.')


def main():
    """Expose one explicit operation at a time so an agent can inspect evidence."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run-dir', required=True, type=Path)
    parser.add_argument('--journal', required=True, type=Path)
    parser.add_argument('--tart-home', type=Path, default=Path.home() / '.tart')
    parser.add_argument('--tart', default='tart')
    sub = parser.add_subparsers(dest='action', required=True)
    sub.add_parser('init')
    for action in ('clone', 'run', 'ready', 'ip', 'exec', 'collect', 'shutdown', 'delete', 'status'):
        child = sub.add_parser(action)
        child.add_argument('role', choices=('mac', 'linux'))
        if action == 'clone':
            child.add_argument('source')
        if action == 'run':
            child.add_argument('--headless', action='store_true')
        if action == 'ready':
            child.add_argument('--timeout', type=float, default=120)
        if action == 'collect':
            child.add_argument('guest_directory')
            child.add_argument('label')
        if action == 'exec':
            child.add_argument('--timeout', type=float, default=300)
            child.add_argument('--stdin', action='store_true')
            child.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    control = Control(args.run_dir, args.journal, args.tart_home, args.tart)
    if args.action == 'init':
        args.run_dir.mkdir(mode=0o700)
        control.data = {'format': 1, 'run_id': str(uuid.uuid4()), 'vms': {}}
        control.save()
        control.record('decision', str(uuid.uuid4()), 'init', f'Private fixture/evidence directory: `{args.run_dir}`.')
        print(control.data['run_id'])
        return
    # run stays in the foreground of its owning terminal/tool session. Release
    # the manifest lock before waiting so readiness/exec can use the same run.
    if args.action == 'run':
        control.run_foreground(args.role, args.headless)
        return
    with control.locked():
        if args.action == 'clone':
            control.clone(args.role, args.source)
        elif args.action == 'ready':
            if args.timeout <= 0:
                raise Refused('readiness timeout must be positive')
            control.ready(args.role, args.timeout)
        elif args.action == 'status':
            print(json.dumps(control.state(args.role), indent=2))
        elif args.action == 'delete':
            control.delete(args.role)
        elif args.action == 'collect':
            control.collect(args.role, args.guest_directory, args.label)
        elif args.action == 'ip':
            control.command('ip', control.owned(args.role), '--wait', '10')
        elif args.action == 'exec':
            command = args.command
            if command and command[0] == '--':
                command = command[1:]
            if not command or args.timeout <= 0:
                raise Refused('provide a guest command and positive timeout')
            argv = ['exec'] + (['-i'] if args.stdin else []) + [control.owned(args.role), *command]
            control.mutate(argv, 'Operator-authorized guest command; stdin omitted from journal.', timeout=args.timeout)
        elif args.action == 'shutdown':
            control.shutdown(args.role)


if __name__ == '__main__':
    try:
        main()
    except (Refused, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f'control refused/failed: {error}', file=sys.stderr)
        sys.exit(1)
