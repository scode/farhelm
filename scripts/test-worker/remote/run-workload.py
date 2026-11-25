"""Run a worker command and own Stop through the end of its process group.

Like the test recorder, keep the direct group leader waitable through all
signals. Bash wait or Popen.poll would reap it early and leave a numeric group
ID that could refer to a different run. This watcher holds no worker leases.
"""

import os
from pathlib import Path
import signal
import subprocess
import sys
import time


GRACE_SECONDS = 35
CHECK_INTERVAL = 0.1


def is_live_group_member(stat: str, group: int) -> bool:
    """A zombie main thread can still have living siblings in the workload."""
    # The command name can contain whitespace or parentheses. The remaining fields start at state.
    fields = stat.rpartition(") ")[2].split()
    if len(fields) <= 17 or fields[2] != str(group):
        return False
    state = fields[0]
    if state == "Z":
        # num_threads is field 20 of /proc/PID/stat, including the two stripped fields.
        return int(fields[17]) > 1
    return state not in ("X", "x")


def group_is_live(group: int) -> bool:
    """Linux can report zombie group members to killpg(0) after they stop doing work."""
    with os.scandir("/proc") as processes:
        for entry in processes:
            if not entry.name.isdecimal():
                continue
            try:
                stat = Path(entry.path, "stat").read_text(errors="replace")
            except (FileNotFoundError, ProcessLookupError, PermissionError):
                continue
            if is_live_group_member(stat, group):
                return True
    return False


def observed_exit(process: subprocess.Popen):
    """The leader can be dead while its unreaped PID still pins the group."""
    return os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)


def signal_group(process: subprocess.Popen, kind: signal.Signals) -> None:
    """Refuse signals if the direct parent has lost wait ownership."""
    observed_exit(process)
    try:
        os.killpg(process.pid, kind)
    except ProcessLookupError:
        pass
    except PermissionError as error:
        print(f"test-worker: could not signal workload with {kind.name}: {error}", file=sys.stderr)


def cancel(process: subprocess.Popen) -> int:
    """Keep both leases until live writers are gone, including children whose shell already exited."""
    signal_group(process, signal.SIGTERM)
    # GUI recorder shutdown has its own 30-second cleanup budget for the independent session it starts.
    deadline = time.monotonic() + GRACE_SECONDS
    forced = False
    while observed_exit(process) is None or group_is_live(process.pid):
        if not forced and time.monotonic() >= deadline:
            signal_group(process, signal.SIGKILL)
            forced = True
        time.sleep(CHECK_INTERVAL)
    process.wait()
    return 137 if forced else 143


def run(stage: Path, argv: list[str]) -> int:
    """Normal completion follows the runner. A Stop request extends ownership to its surviving group."""
    stop = stage / "stop"
    if stop.exists():
        return 143

    process = subprocess.Popen(argv, start_new_session=True)
    # The controller counts a run toward evidence retention only after spawn.
    try:
        (stage / "running").touch()
    except OSError:
        cancel(process)
        raise
    while True:
        status = observed_exit(process)
        if stop.exists():
            return cancel(process)
        if status is not None:
            code = process.wait()
            return code if code >= 0 else 128 - code
        time.sleep(CHECK_INTERVAL)


if __name__ == "__main__":
    stage_path, *command = sys.argv[1:]
    sys.exit(run(Path(stage_path), command))
