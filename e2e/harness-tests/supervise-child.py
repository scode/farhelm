#!/usr/bin/env python3
"""Own one intentional Playwright child through cancellation and orphan cleanup.

Node reaps subprocesses before publishing their exit events, so a later numeric group
signal cannot safely clean up a CLI's surviving descendants. This helper keeps the
leader waitable until group signals finish. On Linux it also becomes a subreaper:
Playwright starts detached browser groups, whose orphaned members then become this
helper's children and can be killed and reaped under the same wait-ownership rule.

The parent's stdin pipe is a cancellation lease, never the CLI's input. EOF catches
parent death as well as explicit cancellation. Output goes directly to the parent's
bounded readers. Non-Linux systems retain group cleanup only; detached descendants
are outside that platform's guarantee. SIGKILL of this helper and uninterruptible
kernel operations remain outside the cleanup contract.
"""

import argparse
import ctypes
import os
from pathlib import Path
import selectors
import signal
import subprocess
import sys
import time


POLL_SECONDS = 0.02
TERM_GRACE_SECONDS = 1.0
CLEANUP_SECONDS = 3.0
CHILD_LIST_BYTES = 64 * 1024
CHILD_LIST_COUNT = 4096


def observe_exit(pid):
    """Observe a direct child without releasing the PID needed by a subsequent signal.

    ECHILD is deliberately not converted to an ordinary exit: it means wait ownership
    was lost, and the caller must refuse all further numeric signals for this PID.
    """
    try:
        result = os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    except InterruptedError:
        return None
    if result is None:
        return None
    if result.si_code == os.CLD_EXITED:
        return result.si_status
    return -result.si_status


def signal_owned(pid, signum, *, group=False):
    """Signal only while this single-threaded supervisor retains direct wait ownership."""
    observe_exit(pid)
    try:
        if group:
            os.killpg(pid, signum)
        else:
            os.kill(pid, signum)
    except ProcessLookupError:
        pass


def enable_subreaper():
    """Arrange kernel reparenting before spawn, without adopting unrelated process trees."""
    if sys.platform != "linux":
        return False
    libc = ctypes.CDLL(None, use_errno=True)
    # PR_SET_CHILD_SUBREAPER changes this helper only. It does not affect the parent
    # test process, the user's services, or processes outside this descendant tree.
    if libc.prctl(36, 1, 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno(), "cannot enable child subreaper")
    return True


def adopted_children():
    """Read a bounded kernel child list; each PID still needs wait-ownership validation."""
    with Path(f"/proc/self/task/{os.getpid()}/children").open("rb") as source:
        data = source.read(CHILD_LIST_BYTES + 1)
    if len(data) > CHILD_LIST_BYTES:
        raise RuntimeError("owned child list exceeded its byte bound")
    values = data.split()
    if len(values) > CHILD_LIST_COUNT:
        raise RuntimeError("owned child list exceeded its count bound")
    return [int(value) for value in values]


def reap_adopted(deadline):
    """Kill and reap detached orphans, repeating as their children are reparented here.

    No PID from procfs is trusted by itself. waitid proves it is still this helper's
    child, and no wait releases that identity until its last signal is complete.
    """
    while True:
        children = adopted_children()
        if not children:
            return
        for pid in children:
            try:
                signal_owned(pid, signal.SIGKILL)
                os.waitpid(pid, os.WNOHANG)
            except ChildProcessError:
                # It was never ours, or ownership is already gone. Neither case
                # authorizes a signal using this numeric PID.
                continue
        if time.monotonic() >= deadline:
            raise TimeoutError("detached descendant cleanup exceeded its deadline")
        time.sleep(POLL_SECONDS)


def supervise(argv, timeout):
    """Preserve a normal CLI exit only after owned cleanup has completed.

    Cancellation and the independent deadline return 124 regardless of the CLI's
    reaction. Cleanup failure returns through main as 125, never as Playwright's
    normal test-failure code 1.
    """
    subreaper = enable_subreaper()
    cancelled = False

    def cancel(_signum, _frame):
        nonlocal cancelled
        cancelled = True

    previous_handlers = {
        signum: signal.signal(signum, cancel)
        for signum in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP)
    }
    child = None
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(sys.stdin.fileno(), selectors.EVENT_READ)
            child = subprocess.Popen(argv, stdin=subprocess.DEVNULL, start_new_session=True)
            deadline = time.monotonic() + timeout
            termination_at = None
            while True:
                status = observe_exit(child.pid)
                if status is not None:
                    break
                if time.monotonic() >= deadline:
                    cancelled = True
                if cancelled and termination_at is None:
                    signal_owned(child.pid, signal.SIGTERM, group=True)
                    termination_at = time.monotonic()
                if termination_at is not None and time.monotonic() - termination_at >= TERM_GRACE_SECONDS:
                    break
                if selector.select(POLL_SECONDS):
                    # Any readable lease state ends ownership: the only valid parent
                    # behavior is to hold this pipe open without writing payloads.
                    cancelled = True
                    selector.unregister(sys.stdin.fileno())
            signal_owned(child.pid, signal.SIGKILL, group=True)
            child.wait(timeout=CLEANUP_SECONDS)
            if subreaper:
                reap_adopted(time.monotonic() + CLEANUP_SECONDS)
            else:
                print("child supervision: detached descendants are outside non-Linux group cleanup", file=sys.stderr)
            if cancelled:
                return 124
            assert status is not None
            return status if status >= 0 else 128 - status
    finally:
        # Exceptions use the same ownership rule. Popen.returncode becomes non-None
        # only after our final wait; never send a group signal after that point.
        try:
            if child is not None and child.returncode is None:
                signal_owned(child.pid, signal.SIGKILL, group=True)
                child.wait(timeout=CLEANUP_SECONDS)
            if child is not None and subreaper:
                reap_adopted(time.monotonic() + CLEANUP_SECONDS)
        finally:
            for signum, handler in previous_handlers.items():
                signal.signal(signum, handler)


def main():
    """Keep supervisor errors distinguishable from intentional test failures."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeout", type=float, default=25)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command or not 0 < args.timeout <= 300:
        parser.error("a command and timeout in (0, 300] are required")
    try:
        return supervise(command, args.timeout)
    except Exception as error:
        print(f"child supervision failed: {type(error).__name__}: {error}", file=sys.stderr)
        return 125


if __name__ == "__main__":
    sys.exit(main())
