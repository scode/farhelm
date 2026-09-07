#!/usr/bin/env python3
"""Require an explanatory sleep-ok comment for delays in Rust and browser test code."""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import signal
import stat
import sys
import time

_previous_bytecode = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    from test_sleep_syntax import SyntaxBudget, external_modules, sleep_details
except ImportError:
    raise SystemExit('Install scripts/test-sleep-requirements.txt in an isolated Python environment first.')
finally:
    sys.dont_write_bytecode = _previous_bytecode
    del _previous_bytecode


class SourceBudget:
    """Bound source inventory size and stop between operations after cancellation or timeout."""

    def __init__(self, *, seconds=60, entries=100000, byte_limit=64 * 1024 * 1024):
        self.deadline = time.monotonic() + seconds
        self.entries = entries
        self.bytes = byte_limit
        self.signal = None
        self.syntax = SyntaxBudget(check=self.check)

    def cancel(self, number, _frame):
        """Record operator intent without opening files or printing from a signal handler."""
        if self.signal is None:
            self.signal = number

    def check(self):
        """Kernel-blocked filesystem calls cannot be interrupted by this cooperative deadline."""
        if self.signal is not None:
            raise ValueError('cancelled')
        if time.monotonic() >= self.deadline:
            raise ValueError('source check exceeded its elapsed-time budget')


def sources(root, budget):
    """Read supported source roots without following links or scanning generated dependency trees.

    Keep a bounded in-memory source snapshot so module resolution never opens
    a path merely because a Rust attribute named it. Missing source roots are
    errors, not evidence of an empty suite.
    """
    found = {}
    # Discovery begins below e2e, so check that fixed parent explicitly too.
    # O_NOFOLLOW on e2e/tests alone would protect only the tests component.
    parent_fd = os.open(root / 'e2e', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    os.close(parent_fd)
    pending = [root / 'crates', root / 'e2e' / 'tests']
    while pending:
        budget.check()
        directory = pending.pop()
        fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        try:
            with os.scandir(fd) as stream:
                for entry in stream:
                    budget.check()
                    budget.entries -= 1
                    if budget.entries < 0:
                        raise ValueError('source inventory exceeded its entry budget')
                    path = directory / entry.name
                    if entry.name in {'target', 'node_modules', '.git', '.jj'}:
                        continue
                    if entry.is_symlink():
                        raise ValueError(f'source inventory contains an uninspected symlink: {path.relative_to(root)}')
                    if entry.is_dir(follow_symlinks=False):
                        pending.append(path)
                        continue
                    relative = path.relative_to(root)
                    if not (relative.parts[0] == 'crates' and path.suffix == '.rs'
                            or relative.parts[:2] == ('e2e', 'tests') and path.suffix == '.ts'):
                        continue
                    if len(found) >= 10000:
                        raise ValueError('source inventory exceeded its file budget')
                    file_fd = os.open(entry.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=fd)
                    try:
                        info = os.fstat(file_fd)
                        if not stat.S_ISREG(info.st_mode) or info.st_size > 2 * 1024 * 1024:
                            raise ValueError('source is not a bounded regular file')
                        with os.fdopen(file_fd, 'rb', closefd=False) as source:
                            data = source.read(min(2 * 1024 * 1024, budget.bytes) + 1)
                        if len(data) > 2 * 1024 * 1024 or len(data) > budget.bytes:
                            raise ValueError('source inventory exceeded its byte budget')
                        budget.bytes -= len(data)
                        found[relative.as_posix()] = data
                    finally:
                        os.close(file_fd)
        finally:
            os.close(fd)
    return found


def integration_source(path):
    """Recognize a crate's integration-test tree, not an arbitrary module directory named tests."""
    return pathlib.PurePosixPath(path).parts[:1] == ('crates',) and pathlib.PurePosixPath(path).parts[2:3] == ('tests',)


def standard_crate_root(path):
    """Identify standard Cargo target entrypoints while preserving nested module-relative paths."""
    parts = pathlib.PurePosixPath(path).parts
    return (len(parts) == 4 and integration_source(path)
            or len(parts) == 5 and integration_source(path) and parts[-1] == 'main.rs'
            or len(parts) == 4 and parts[0] == 'crates' and parts[2] == 'src'
            and parts[3] in {'lib.rs', 'main.rs'}
            or len(parts) == 5 and parts[0] == 'crates' and parts[2:4] == ('src', 'bin')
            or len(parts) == 6 and parts[0] == 'crates' and parts[2:4] == ('src', 'bin')
            and parts[-1] == 'main.rs')


def inventory(source_files, budget):
    """Propagate external test-module scope before interpreting delay calls.

    Integration-test directories, browser tests and the teststate fixture
    crate are entirely test infrastructure. Elsewhere, test attributes and
    source-declared test-only module edges determine scope.
    """
    whole = {path for path in source_files if integration_source(path)
             or path.startswith(('e2e/tests/', 'crates/farhelm-teststate/'))}
    edges = {}
    for path, source in source_files.items():
        budget.check()
        if path.endswith('.rs'):
            edges[path] = external_modules(source, path, crate_root=standard_crate_root(path),
                                           budget=budget.syntax)
    changed = True
    while changed:
        changed = False
        for path, modules in edges.items():
            budget.check()
            for candidates, declared_test in modules:
                if path not in whole and not declared_test:
                    continue
                matches = [candidate for candidate in candidates if candidate in source_files]
                if len(matches) != 1:
                    raise ValueError(f'{path}: test module has missing or ambiguous source')
                if matches[0] not in whole:
                    whole.add(matches[0])
                    changed = True
    calls = []
    for path, source in sorted(source_files.items()):
        budget.check()
        language = 'rust' if path.endswith('.rs') else 'typescript'
        for item in sleep_details(source, language, whole_file=path in whole, budget=budget.syntax):
            budget.syntax.retain({'path': path})
            calls.append({'path': path, **item})
    budget.check()
    return calls


def main(argv=None):
    """Print violations or an explicit JSON inventory; both modes retain failing exit status."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[1])
    parser.add_argument('--inventory', action='store_true', help='print all recognized delays as JSON')
    args = parser.parse_args(argv)
    budget = SourceBudget()
    previous = {number: signal.signal(number, budget.cancel) for number in (signal.SIGINT, signal.SIGTERM)}
    try:
        files = sources(args.root.absolute(), budget)
        calls = inventory(files, budget)
        violations = [item for item in calls if item['rationale'] is None]
        if args.inventory:
            output = json.dumps({'files': len(files), 'calls': calls, 'unannotated': len(violations)}, indent=2)
            if len(output.encode('utf-8')) > 16 * 1024 * 1024:
                raise ValueError('formatted report exceeded its byte budget')
            budget.check()
            print(output, flush=True)
        else:
            for item in violations:
                print(f"{item['path']}:{item['line']}: {item['call']} needs // sleep-ok: <why>")
            print(f'{len(calls)} test delays inspected; {len(violations)} lack a rationale', flush=True)
        return 128 + budget.signal if budget.signal is not None else int(bool(violations))
    except (OSError, ValueError, RecursionError) as error:
        print(f'check-test-sleeps: incomplete check: {error}', file=sys.stderr)
        return 128 + budget.signal if budget.signal is not None else 2
    finally:
        for number, handler in previous.items():
            signal.signal(number, handler)


if __name__ == '__main__':
    raise SystemExit(main())
