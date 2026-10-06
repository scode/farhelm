#!/usr/bin/env python3
"""Observe a real Linux session process across app and supervisor updates.

Run this as the plain-command session, then call observe over independent SSH.
PID, kernel start ticks and boot identity must all remain unchanged, and the
counter file must advance. A terminal replay or replacement command cannot
satisfy that contract. Rebooting Linux legitimately ends the workload.
"""

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import sys
import time


def process_identity(pid):
    """Use Linux kernel process identity rather than a recycled numeric PID.

Field 22 of /proc/PID/stat is the process start time in clock ticks. The comm
field may contain spaces and parentheses, so split only after its final closing
parenthesis. Zombies are not live session processes, even while /proc exists.
"""
    if sys.platform != 'linux':
        raise RuntimeError('the continuity oracle requires Linux /proc')
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    if fields[0] in ('Z', 'X'):
        raise RuntimeError('continuity process is stopped')
    return {'pid': pid, 'start_ticks': int(fields[19]),
            'boot_id': Path('/proc/sys/kernel/random/boot_id').read_text().strip()}


def workload(directory):
    """Append durable counter observations from one process without restarting it.

Exclusive file creation prevents accidentally reusing a prior run's evidence.
The one-second cadence is the workload, not a readiness delay: observers must
read actual advancement and prove the original process still exists.
"""
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    identity = process_identity(os.getpid())
    with (directory / 'identity.json').open('x') as stream:
        os.chmod(directory / 'identity.json', 0o600)
        json.dump(identity, stream)
    with (directory / 'counter.log').open('x', buffering=1) as stream:
        os.chmod(directory / 'counter.log', 0o600)
        counter = 0
        while True:
            stamp = datetime.now(timezone.utc).isoformat(timespec='seconds')
            line = f'{stamp} {counter}\n'
            stream.write(line)
            print(line, end='', flush=True)
            counter += 1
            time.sleep(1)


def sample(directory):
    """Read live process identity and only a bounded tail of its counter file.

An incomplete final line is ignored until a later observation. A metadata file
without a live matching process is a failed continuity check, not old evidence
that can be redisplayed and called progress.
"""
    recorded = json.loads((directory / 'identity.json').read_text())
    try:
        current = process_identity(recorded['pid'])
    except FileNotFoundError as error:
        raise RuntimeError('continuity process is stopped') from error
    if current != recorded:
        raise RuntimeError('continuity process identity changed')
    with (directory / 'counter.log').open('rb') as stream:
        stream.seek(0, 2)
        stream.seek(max(0, stream.tell() - 4096))
        tail = stream.read().decode('ascii')
    lines = tail.splitlines()
    if not tail.endswith('\n'):
        lines = lines[:-1]
    if not lines:
        raise RuntimeError('counter has no complete observation yet')
    stamp, counter = lines[-1].split()
    return dict(current, counter=int(counter), timestamp=stamp)


def require_progress(before, after):
    """Refuse replayed counters and replacements even when their output advances."""
    for key in ('pid', 'start_ticks', 'boot_id'):
        if before[key] != after[key]:
            raise RuntimeError('continuity process identity changed')
    if after['counter'] <= before['counter']:
        raise RuntimeError('counter did not advance')


def main():
    """Keep workload execution and independent observation as separate commands."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('run', 'observe'))
    parser.add_argument('directory', type=Path)
    parser.add_argument('--before', type=Path)
    args = parser.parse_args()
    if args.action == 'run':
        workload(args.directory)
    else:
        value = sample(args.directory)
        if args.before:
            require_progress(json.loads(args.before.read_text()), value)
        print(json.dumps(value))


if __name__ == '__main__':
    main()
