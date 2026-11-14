#!/usr/bin/env python3
"""Drive the GPUI desktop app as an agent: launch it against a real helm and supervisor, then look and click.

NOTE: Debug builds only, and never against the live install. On Linux a run is headless, on a private Mutter; on a Mac
it opens a real window in the logged-in GUI session, which this script must run inside (over SSH is fine). Everything
a run starts lives under the run directory you name (which must not exist yet), and `stop` takes all of it down again.

    scripts/gui-agent.py RUN_DIR start [--sessions N | --fleet KIND,...] [--size WxH (Linux)] [--no-build]
                                       [--lock FILE] [--window-size WxH] [--appearance light|dark]
                                       [--scripted-hosts]
    scripts/gui-agent.py RUN_DIR snapshot -i          # any gpui-agent command: click, type, press, wait, screenshot ...
    scripts/gui-agent.py RUN_DIR terminal-text [--wait TEXT [--timeout SECONDS]]
    scripts/gui-agent.py RUN_DIR stop

`start` builds `farhelm-desktop` with the `agent` feature (the test-automation bridge), the `farhelm` CLI the app
starts its supervisor from, the fake agents (`farhelm-fixtures`) and the bridge's own command line; on Linux starts a
private headless Mutter on its own session bus; launches the app (on that Mutter, or on the Mac's GUI session) with its
own state directory; and seeds sessions that run the fake agent, through the real helm API: `--sessions` basic ones, or
with `--fleet` one per kind named (FLEET_KINDS), after which it waits until the supervisor has read each one as the
status its kind names. On Linux,
rendering is Mesa's software Vulkan driver (lavapipe), the same as the screenshot goldens; on a Mac it is Metal.
`--lock FILE` holds an exclusive `flock` on FILE from before the build until `stop`, for a machine whose one GUI
session several runs share (the test Mac's convention). `--window-size` and `--appearance` start the app's window at
that size and in that palette (agent builds only; `crates/gui/farhelm-gui/src/agent_startup.rs`), so a screenshot batch
can capture each theme and size; they are test hooks, not the app's own appearance setting. `--scripted-hosts` makes
the helm fake every command it runs on a host (setup, Update and the rest) with its scripted provisioning backend
(SCRIPTED_HOSTS_ENV, debug builds only), controlled with `script_hosts`. Any other verb goes to the vendored
`gpui-agent` command line, already pointed at this run's app; `gpui-agent --help` and
vendor/gpui-agent/README.md list them. docs/gui-agent.md is the guide.

Why Mutter rather than the bridge's own Xvfb: the shipped app is built for Wayland only (the root Cargo.toml's
`gpui_platform` features), so runs use Wayland to match what users get. Agent builds do also compile GPUI's X11
backend (the vendored bridge crate turns it on), but driving that would test a path no shipped build has. Mutter also
exercises the client-side decoration path a GNOME user gets.

Names that could collide with another run (the Wayland display, the bridge socket, the runtime directory) all derive
from the run directory, so parallel runs from different directories never share anything (AGENTS.md, "Sharing the
machine with other agents"). The bridge socket is unauthenticated, so it sits in a 0700 directory.
"""

from __future__ import annotations

import fcntl
import json
import os
import pwd
import shlex
import signal
import subprocess
import sys
import time
import urllib.request
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Mesa's software drivers, at the paths Debian and Ubuntu install them to (the same ones scripts/check-gui-goldens.py
# uses), so a machine's real GPU, if it has one, is never picked.
LAVAPIPE_ICD = "/usr/share/vulkan/icd.d/lvp_icd.json"
MESA_EGL_VENDOR = "/usr/share/glvnd/egl_vendor.d/50_mesa.json"

# The bridge command line's session name inside this run's private GPUI_AGENT_HOME. One run holds one app, so a fixed
# name is fine: the home directory is what keeps runs apart.
SESSION = "app"

# A Unix socket path holds at most 107 bytes on Linux. A run creates three kinds of socket under its directory: the
# bridge's, the supervisor's, and one per terminal host (named by 8 characters, `NAME_LEN` in the supervisor's
# hosts/names.rs). The longest suffix decides how deep a run directory may be; a socket that does not fit fails deep
# inside the app instead of as a usage error.
SOCKET_SUFFIXES = (f"/agent/{SESSION}/app.sock", "/state/supervisor.sock", "/state/hosts/" + "x" * 8)
LONGEST_SOCKET_SUFFIX = max(SOCKET_SUFFIXES, key=len)
# sun_path is 108 bytes on Linux and 104 on macOS, each including the terminating NUL.
MAX_SOCKET_PATH = 103 if sys.platform == "darwin" else 107

# The bridge session's files `stop` removes once the app is down, the same ones the bridge's own `close` removes.
SESSION_FILES = ("app.pid", "xvfb.pid", "app.sock", "socket", "display", "window", "refs.json")

# Mutter's virtual monitor on Linux when `start` is not given `--size`.
DEFAULT_MONITOR = "1600x1200"

# How long the app gets to answer on its socket: it starts a helm and a supervisor before the window opens.
LAUNCH_TIMEOUT_SECONDS = 120

# How long `start --lock` waits for another run to release the shared lock before giving up.
LOCK_WAIT_SECONDS = 1800

# The lock holder: a detached Python child that takes an exclusive flock on the lock file (the same lock `lockf -k` on
# macOS and `flock` on Linux take), writes a marker once it holds it, and then sleeps until `stop` ends it. A child of
# its own, rather than the script, because `start` and `stop` are separate invocations and the lock must span both.
#
# While it waits it retries a non-blocking flock and gives up as soon as its parent (`start`, argv[3]) is gone. `start`
# is often killed while it waits: an agent's shell timeout sends SIGTERM and a dropped SSH session sends SIGHUP, and
# neither runs Python's clean-up. A holder that went on waiting would take the lock later and keep it, with nothing
# left to release it. Once it holds the lock it is meant to outlive `start`; `stop` ends it.
LOCK_HOLDER = (
    "import fcntl, os, sys, time\n"
    "lock = open(sys.argv[1], 'a')\n"
    "parent = int(sys.argv[3])\n"
    "while True:\n"
    "    try:\n"
    "        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)\n"
    "        break\n"
    "    except BlockingIOError:\n"
    "        if os.getppid() != parent:\n"
    "            sys.exit(0)\n"
    "        time.sleep(0.2)\n"
    "open(sys.argv[2], 'w').write('held')\n"
    "while True:\n"
    "    time.sleep(3600)\n"
)


class UsageError(Exception):
    """A mistake in how the script was called; printed without a traceback."""


# ---------------------------------------------------------------------------------------------------------------------
# Pure helpers (scripts/test-gui-agent.py covers these)


def check_run_dir(run: Path) -> None:
    """Refuse a run directory the run cannot own: relative, or so deep that its deepest socket path would not fit."""
    if not run.is_absolute():
        raise UsageError(f"the run directory must be an absolute path: {run}")
    if len(str(run)) + len(LONGEST_SOCKET_SUFFIX) > MAX_SOCKET_PATH:
        raise UsageError(
            f"the run directory is too deep: the socket {run}{LONGEST_SOCKET_SUFFIX} would exceed the "
            f"{MAX_SOCKET_PATH}-byte Unix socket path limit"
        )


def display_name(token: str) -> str:
    """The run's Wayland display name. Mutter creates it inside the run's own runtime directory."""
    return f"wayland-fh-{token}"


def bridge_env(
    base: dict[str, str],
    run: Path,
    runtime: str | None,
    display: str | None,
    dbus: str | None,
    platform: str = sys.platform,
) -> dict[str, str]:
    """The environment for the bridge's command line and, through it, the app.

    Starts from `base` with every display and bridge variable from the caller's session removed. On Linux the app can
    then only reach this run's Mutter and session bus, and rendering is pinned to lavapipe. On a Mac there is no display
    to choose (the app opens its window in the GUI session it runs in), so only the run's own bridge home is added.
    """
    env = {
        key: value
        for key, value in base.items()
        if key not in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DBUS_SESSION_BUS_ADDRESS")
        and not key.startswith("GPUI_AGENT")
        and not key.startswith("FARHELM_")
    }
    if platform.startswith("linux"):
        env.update(
            {
                "XDG_RUNTIME_DIR": runtime,
                "WAYLAND_DISPLAY": display,
                "DBUS_SESSION_BUS_ADDRESS": dbus,
                "VK_ICD_FILENAMES": LAVAPIPE_ICD,
                "__EGL_VENDOR_LIBRARY_FILENAMES": MESA_EGL_VENDOR,
                "LIBGL_ALWAYS_SOFTWARE": "true",
            }
        )
    env.update({"GPUI_AGENT_HOME": str(run / "agent"), "GPUI_AGENT_SESSION": SESSION})
    # A debug build logs at debug level by default, tens of thousands of lines a minute from the helm alone; the
    # caller's own RUST_LOG still wins.
    env.setdefault("RUST_LOG", "info")
    return env


def parse_size(text: str) -> tuple[int, int]:
    """`WIDTHxHEIGHT` as two positive integers; a UsageError for anything else."""
    width, sep, height = text.partition("x")
    if not (sep and width.isdigit() and height.isdigit() and int(width) > 0 and int(height) > 0):
        raise UsageError(f"a size is WIDTHxHEIGHT in pixels, not {text!r}")
    return int(width), int(height)


# The scripted provisioning backend's switch, read by debug builds of the helm (`ProductionProvisioning::new` in
# crates/farhelm-helm/src/provisioning/service.rs), and the marker its directory must hold (`E2E_BACKEND_MARKER` in
# provisioning/e2e.rs). Both must match the helm's exactly.
SCRIPTED_HOSTS_ENV = "FARHELM_E2E_PROVISIONING_BACKEND_DIR"
SCRIPTED_HOSTS_MARKER = "farhelm-e2e-provisioning-v1\n"


def scripted_hosts_dir(run: Path) -> Path:
    """Where `start --scripted-hosts` puts the scripted provisioning backend's directory: inside the app's state
    directory, because the helm refuses a backend directory outside its own state."""
    return run / "state" / "scripted-hosts"


def prepare_scripted_hosts(run: Path) -> Path:
    """Create the scripted backend's directory with its marker and an empty `config.json` (every host absent, so a
    probe offers a setup plan and every step succeeds), and return it. Steps control the fake hosts with
    `script_hosts`; the helm appends what it did to `events.jsonl` beside it."""
    directory = scripted_hosts_dir(run)
    # The app would create its state directory private; created first here, it must be just as private.
    directory.parent.mkdir(mode=0o700, exist_ok=True)
    directory.mkdir(mode=0o700)
    (directory / "ENABLED").write_text(SCRIPTED_HOSTS_MARKER)
    script_hosts(run, {})
    return directory


def script_hosts(run: Path, config: dict) -> None:
    """Replace what the run's fake hosts do (docs/gui-agent.md, "Scripted hosts" has the format).

    The helm rereads `config.json` at every probe and step, and every 20 ms while a step is held, so a file rewritten in
    place could be read half-written and fail a step the test never meant to fail. The new file is written beside it and
    renamed over it, which the helm sees whole or not at all (the old browser suite wrote it the same way)."""
    directory = scripted_hosts_dir(run)
    staged = directory / "config.json.new"
    staged.write_text(json.dumps(config, indent=2) + "\n")
    os.replace(staged, directory / "config.json")


def app_env_args(
    run: Path,
    binaries: dict[str, str],
    window_size: str | None = None,
    appearance: str | None = None,
    scripted_hosts: Path | None = None,
) -> list[str]:
    """The `--env` arguments `gpui-agent launch` passes to the app only: its state, its CLI and the helm file, plus the
    first window's size, a forced appearance and the scripted provisioning backend's directory when the run asks for
    them (unset otherwise, so the app starts at its own default size in the system's appearance and sets up hosts for
    real)."""
    pairs = {
        # The app's state directory: never the default one, which could be a real installation's.
        "FARHELM_DESKTOP_STATE_DIR": str(run / "state"),
        # The CLI the app starts its supervisor from: this build's, not one found beside the app or on PATH.
        "FARHELM_DESKTOP_FARHELM": binaries["farhelm"],
        # Where the agent build writes its helm's address and credential, for seeding
        # (crates/farhelm-desktop/src/agent.rs).
        "FARHELM_DESKTOP_AGENT_HELM_FILE": str(run / "helm.json"),
    }
    # Read by the GUI's agent-build startup overrides (crates/gui/farhelm-gui/src/agent_startup.rs).
    if window_size is not None:
        pairs["FARHELM_DESKTOP_AGENT_WINDOW_SIZE"] = window_size
    if appearance is not None:
        pairs["FARHELM_DESKTOP_AGENT_APPEARANCE"] = appearance
    if scripted_hosts is not None:
        pairs[SCRIPTED_HOSTS_ENV] = str(scripted_hosts)
    args: list[str] = []
    for key, value in pairs.items():
        args += ["--env", f"{key}={value}"]
    return args


# What `start --fleet` can seed. `basic` is the prompt-and-echo fake agent `--sessions` seeds, the one to type into; the
# others run the fake agent's replay script, which prints a one-line transcript and then holds the screen the
# supervisor's classifier reads as that status (`ReplayThen` in crates/farhelm-fixtures/src/fake_agent.rs). Only those
# four have a status to wait for: `basic` is read as running while it prints and idle a few samples after, so it never
# settles on one the run could promise.
FLEET_KINDS = ("basic", "running", "waiting", "idle", "exited")

# The replay script's `--then` shape for each status a fleet session is seeded to reach.
REPLAY_THEN = {"running": "spin", "waiting": "menu", "idle": "quiet", "exited": "exit"}

# How long `start --fleet` waits for every seeded session to reach its status. The supervisor samples every session on
# a two-second tick and calls one idle after three quiet samples, so a small fleet settles well within this.
SETTLE_TIMEOUT_SECONDS = 120


def parse_fleet(text: str) -> list[str]:
    """`--fleet`'s comma-separated kinds, in order (one session each); a UsageError for an empty list or an unknown
    kind."""
    kinds = text.split(",")
    unknown = [kind for kind in kinds if kind not in FLEET_KINDS]
    if not text or unknown:
        raise UsageError(f"--fleet takes a comma-separated list of {', '.join(FLEET_KINDS)}, not {text!r}")
    return kinds


def transcript(index: int, kind: str) -> str:
    """What a replayed fleet session prints before it settles, so its terminal shows whose it is and has a last line
    to wait for (`now holds its <kind> screen`, for `terminal-text --wait`).

    Three short lines rather than one: lines that fit a narrow window's terminal (about 55 columns at 900 pixels) do
    not wrap, and under an exited session's exit line the app lifts a screen that fills the pane by a row or two (`lift`
    in the terminal's attached.rs), which hid a one-line transcript printed on the first row.
    """
    return f"Fixture {index}: a seeded fake agent\nIt replayed this transcript and\nnow holds its {kind} screen\n"


def seed_body(index: int, cwd: Path, fixtures: str, kind: str = "basic", transcript: Path | None = None) -> dict:
    """A `POST /api/sessions` body for the `index`th seeded session, titled `Fixture <index>`: a command launch of the
    fake agent, the basic script for `basic` and the replay script holding `kind`'s screen otherwise (`transcript` is
    then the file it prints first).

    Only `waiting` declares an agent type (Claude): the generic status reading never reads a session as waiting, and
    declaring one requires the `{farhelm_args}` placeholder (SPEC.md, "Creation"), where the supervisor adds Claude's
    hook flags, which the fake agent ignores. Every other kind stays an undeclared command, like `basic`.
    """
    command = f"{shlex.quote(fixtures)} fake-agent --script basic"
    agent = None
    if kind != "basic":
        command = (
            f"{shlex.quote(fixtures)} fake-agent --script replay --dialect claude"
            f" --transcript {shlex.quote(str(transcript))} --then {REPLAY_THEN[kind]}"
        )
        if kind == "waiting":
            command += " {farhelm_args}"
            agent = "claude"
    return {
        "cwd": str(cwd),
        "title": f"Fixture {index}",
        "command": {"command": command, "yolo": False, "agent": agent, "resume": None},
    }


def unsettled(targets: dict[str, str], states: dict[str, str | None]) -> list[str]:
    """The seeded sessions not yet at their target status, as `id=state` (state `missing` for a session the listing
    does not show), from `targets` (id to the state the listing must report) and `states` (the listing's state of each
    id)."""
    return [f"{id}={states.get(id) or 'missing'}" for id, want in targets.items() if states.get(id) != want]


# Process identity. Linux reads /proc; macOS has no /proc, so there the same facts come from `ps`. Both give a start
# time that, together with the pid, names one process.


def ps_env(base: dict[str, str]) -> dict[str, str]:
    """The environment `ps` runs in: `base` with the C locale and UTC.

    `start` records a process's start time and `stop`, often a separate SSH session, reads it again; `ps` formats it
    in the caller's locale and time zone, so without this two differently configured shells would disagree and `stop`
    would take every recorded process for gone and signal nothing.
    """
    return {**base, "LC_ALL": "C", "TZ": "UTC0"}


def _ps(pid: int, *fields: str) -> str | None:
    """`ps -o <field>= ... -p <pid>` on macOS, or None once the process is gone.

    One `-o` per field: BSD `ps` reads `-o state=,lstart=` as a single field whose header text is everything after the
    `=`, which prints a header line and folds the state into what looks like the start time.
    """
    columns = [arg for field in fields for arg in ("-o", f"{field}=")]
    result = subprocess.run(
        ["ps", "-ww", *columns, "-p", str(pid)], capture_output=True, text=True, env=ps_env(dict(os.environ))
    )
    out = result.stdout.strip()
    return out if result.returncode == 0 and out else None


def _proc_status(pid: int) -> tuple[str, str] | None:
    """A process's `(start time, state letter)`, or None once it is gone."""
    if sys.platform == "darwin":
        # One call: `state` (its first letter is the state) and then `lstart`, a date that is fixed-width in the C
        # locale ("Fri Nov 13 17:01:02 2026"), which `ps_env` pins.
        out = _ps(pid, "state", "lstart")
        if not out:
            return None
        state, _, lstart = out.partition(" ")
        return (lstart.strip(), state[0]) if lstart.strip() else None
    try:
        stat = Path(f"/proc/{pid}/stat").read_text()
    except (FileNotFoundError, ProcessLookupError):
        return None
    fields = stat[stat.rindex(")") + 2 :].split(" ")
    return (fields[19], fields[0])


def proc_start(pid: int) -> str | None:
    """A process's start time (clock ticks since boot on Linux, `ps`'s lstart on macOS), or None once it is gone.

    Together with the pid it names one process: a pid reused later has a different start time, so nothing here ever
    signals a stranger that inherited a recorded pid.
    """
    status = _proc_status(pid)
    return status[0] if status else None


def running(pid: int, start: str) -> bool:
    """Whether the process `pid`/`start` names is still running: it exists, has that start time, and is not a zombie.

    A process that exited but whose parent has not reaped it yet keeps its process-table entry, start time included,
    until it is reaped; it is gone all the same, and signalling it again changes nothing.
    """
    status = _proc_status(pid)
    return status is not None and status[0] == start and status[1] not in ("Z", "X")


def command_line(pid: int) -> list[str] | None:
    """A process's argument vector, or None once it is gone. macOS's `ps` joins it with spaces, so there an argument
    containing a space splits; callers only look for a path they chose, which has none."""
    if sys.platform == "darwin":
        out = _ps(pid, "command")
        return out.split(" ") if out else None
    try:
        return [a.decode(errors="replace") for a in Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")]
    except (FileNotFoundError, ProcessLookupError):
        return None


def parent_links() -> dict[int, int]:
    """Every live process's parent, from /proc on Linux and `ps` on macOS."""
    parents: dict[int, int] = {}
    if sys.platform == "darwin":
        out = subprocess.run(["ps", "-axo", "pid=,ppid="], capture_output=True, text=True, check=True).stdout
        for line in out.splitlines():
            pid, ppid = line.split()
            parents[int(pid)] = int(ppid)
        return parents
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            stat = (entry / "stat").read_text()
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
        parents[int(entry.name)] = int(stat[stat.rindex(")") + 2 :].split(" ")[1])
    return parents


# ---------------------------------------------------------------------------------------------------------------------
# Processes


def descendants(root: int) -> list[int]:
    """Every live process under `root`, found through parent links (root included while it lives)."""
    parents = parent_links()
    tree = {root} if root in parents else set()
    grew = True
    while grew:
        grew = False
        for pid, parent in parents.items():
            if parent in tree and pid not in tree:
                tree.add(pid)
                grew = True
    return sorted(tree)


def stop_process(pid: int, start: str, grace: float) -> bool:
    """SIGTERM the process `pid`/`start` names, then SIGKILL it after `grace` seconds. True when it is gone."""
    if not running(pid, start):
        return True
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        return True
    deadline = time.monotonic() + grace
    while time.monotonic() < deadline:
        if not running(pid, start):
            return True
        time.sleep(0.1)  # sleep-ok: polling a foreign process for exit; there is no event to wait on
    if not running(pid, start):
        return True
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        return True
    time.sleep(0.2)  # sleep-ok: let the kernel finish the killed process before checking
    return not running(pid, start)


def stop_tree(root: int, root_start: str | None, grace: float) -> list[int]:
    """Stop `root` and everything under it, deepest first, while `root` is still the process `root_start` names.

    Each process's identity is read before any signal goes out, so a pid that exits and is reused meanwhile is left
    alone. Returns the pids that did not stop.
    """
    if root_start is None or proc_start(root) != root_start:
        return []
    tree = [(pid, proc_start(pid)) for pid in descendants(root)]
    return [pid for pid, ticks in reversed(tree) if ticks and not stop_process(pid, ticks, grace)]


def app_identity(run: Path) -> dict | None:
    """The app's pid and start time, from the pid file the bridge wrote at launch, or None if it is not running."""
    try:
        pid = int((run / "agent" / SESSION / "app.pid").read_text().strip())
    except (FileNotFoundError, ValueError):
        return None
    ticks = proc_start(pid)
    return {"pid": pid, "start": ticks} if ticks else None


def supervisor_to_stop(state: Path) -> tuple[int, str] | None:
    """The run's supervisor as `(pid, start)`, or None when there is none this run can claim.

    The app records the supervisor it manages in `desktop-supervisor.json`; that pid counts as the run's only while its
    command line names the run's state directory, so a pid reused by an unrelated process is never claimed.
    """
    try:
        pid = json.loads((state / "desktop-supervisor.json").read_text())["pid"]
    except (FileNotFoundError, KeyError, ValueError):
        return None
    ticks = proc_start(pid)
    if not ticks:
        return None
    argv = command_line(pid)
    return (pid, ticks) if argv and str(state) in argv else None


def wait_for(path: Path, seconds: float, what: str) -> None:
    """Wait until `path` exists, or fail naming `what`."""
    deadline = time.monotonic() + seconds
    while not path.exists():
        if time.monotonic() > deadline:
            raise RuntimeError(f"{what} did not appear at {path} within {seconds:.0f} s")
        time.sleep(0.1)  # sleep-ok: polling for a file another process creates


# ---------------------------------------------------------------------------------------------------------------------
# Verbs


def hold_lock(
    run: Path,
    lock: Path,
    on_started=None,
    wait: float = LOCK_WAIT_SECONDS,
) -> tuple[dict, subprocess.Popen]:
    """Start the lock holder for `lock` and wait until it holds the lock; return its identity (for `stop`) and process.

    `on_started` receives the identity before the wait begins, so the caller can record it where `stop` will find it:
    a `start` killed mid-wait leaves a holder that gives up by itself (see LOCK_HOLDER), but a `start` killed just after
    the holder got the lock leaves one that only `stop` can end.

    Fails at once, with the holder's own error, if the holder exits without the lock (a lock path that cannot be
    opened, say), and after `wait` seconds if another run keeps the lock; either way the holder it started is stopped.
    """
    held = run / "lock.held"
    stderr_path = run / "logs" / "lock.stderr"
    with stderr_path.open("wb") as stderr:
        holder = subprocess.Popen(
            [sys.executable, "-c", LOCK_HOLDER, str(lock), str(held), str(os.getpid())],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=stderr,
            start_new_session=True,
        )
    record = {"pid": holder.pid, "start": proc_start(holder.pid), "path": str(lock)}
    try:
        if on_started:
            on_started(record)
        deadline = time.monotonic() + wait
        while not held.exists():
            if holder.poll() is not None:
                detail = stderr_path.read_text(errors="replace").strip() or f"exit status {holder.returncode}"
                raise RuntimeError(f"could not take the lock on {lock}: {detail}")
            if time.monotonic() > deadline:
                raise RuntimeError(f"the lock on {lock} is still held by another run after {wait:.0f} s")
            time.sleep(0.1)  # sleep-ok: polling for the holder's marker or its exit; there is no event to wait on
    except BaseException:
        stop_process(record["pid"], record["start"], 5)
        holder.wait()
        raise
    return record, holder


def console_owner() -> int | None:
    """The uid of the user logged in at the Mac's console (the owner of /dev/console), or None if no one is (root
    owns the console at the login window). Compared by uid: `$USER` can be unset, and the `stat` first on `PATH` may
    be GNU's, which reads `-f` differently."""
    try:
        uid = os.stat("/dev/console").st_uid
    except FileNotFoundError:
        return None
    return uid if uid != 0 else None


def user_name(uid: int | None) -> str:
    """A uid's login name for messages, or a placeholder."""
    if uid is None:
        return "no one"
    try:
        return pwd.getpwuid(uid).pw_name
    except KeyError:
        return f"uid {uid}"


def build(target: str) -> None:
    """Build the four programs a run needs into `target`."""
    subprocess.run(
        ["cargo", "build", "--locked", "-p", "farhelm-desktop", "--features", "agent", "-p", "farhelm", "-p",
         "farhelm-fixtures"],
        cwd=REPO,
        check=True,
    )
    # The bridge's command line has its own workspace and lockfile under vendor/; building it from the checkout root
    # uses the repository's toolchain (vendor/README.md, "Staying out of the workspace's checks").
    subprocess.run(
        ["cargo", "build", "--locked", "--manifest-path", "vendor/gpui-agent/Cargo.toml", "-p", "gpui-agent-cli",
         "--target-dir", target],
        cwd=REPO,
        check=True,
    )


def target_dir() -> str:
    """The workspace's target directory as cargo resolves it (CARGO_TARGET_DIR included)."""
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"],
        cwd=REPO,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return json.loads(out)["target_directory"]


def mutter_command(display: str, size: str) -> list[str]:
    """The command that runs the run's compositor: headless Mutter, Wayland only, on a session bus of its own."""
    return ["dbus-run-session", "--", "mutter", "--headless", "--wayland", "--no-x11", "--virtual-monitor", size,
            "--wayland-display", display]


def start_mutter(
    run: Path,
    runtime: Path,
    display: str,
    command: list[str],
    base: dict[str, str],
    socket_wait: float = 30,
) -> dict:
    """Start the run's compositor (`command`, normally [`mutter_command`]); return what the app needs to join it.

    `dbus-run-session` gives Mutter a bus no desktop shares, and the bus address is read from Mutter's own environment
    because dbus-run-session does not print it. Under that bus the supervisor cannot reach the user's systemd manager,
    so its terminal hosts start without a transient scope (the supervisor's ordinary fallback).

    If the compositor does not come up (no Wayland socket within `socket_wait` seconds, or no bus address), everything
    it started is stopped before the error propagates: `run.json` does not exist yet, so `stop` could not find it.
    """
    env = {k: v for k, v in base.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS")}
    env["XDG_RUNTIME_DIR"] = str(runtime)
    logs = run / "logs"
    with (logs / "mutter.stdout").open("wb") as stdout, (logs / "mutter.stderr").open("wb") as stderr:
        child = subprocess.Popen(
            command, env=env, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr, start_new_session=True
        )
    record = {"pid": child.pid, "start": proc_start(child.pid)}
    try:
        wait_for(runtime / display, socket_wait, "Mutter's Wayland socket")
        for pid in descendants(child.pid):
            try:
                argv = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")
                if argv[0].endswith(b"mutter"):
                    environ = Path(f"/proc/{pid}/environ").read_bytes().split(b"\0")
                    for line in environ:
                        if line.startswith(b"DBUS_SESSION_BUS_ADDRESS="):
                            record["dbus"] = line.split(b"=", 1)[1].decode()
            except (FileNotFoundError, ProcessLookupError):
                continue
        if "dbus" not in record:
            raise RuntimeError(f"no session bus address in Mutter's environment; see {logs}/mutter.stderr")
    except BaseException:
        stop_tree(record["pid"], record["start"], 5)
        child.wait()
        raise
    return record


def helm_request(run: Path, method: str, path: str, body: dict | None = None):
    """One request to the app's embedded helm with the credential the agent build wrote to `helm.json`; the decoded
    JSON reply. Waits for `helm.json` first, since the app writes it only once its helm is up."""
    wait_for(run / "helm.json", 30, "the app's helm file")
    helm = json.loads((run / "helm.json").read_text())
    request = urllib.request.Request(
        f"{helm['api_base']}{path}",
        data=None if body is None else json.dumps(body).encode(),
        headers={"Authorization": f"Bearer {helm['credential']}", "Content-Type": "application/json"},
        method=method,
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.loads(response.read())


def listed_states(run: Path) -> dict[str, str | None]:
    """Every session the helm lists, by id, with the state of its status (`running`, `waiting`, ...)."""
    rows = helm_request(run, "GET", "/api/sessions")["sessions"]
    return {row["id"]: (row.get("status") or {}).get("state") for row in rows}


# The working directory every seeded session runs in. The sessions' folder shows on every row (`host:/tmp`), and SPEC.md
# puts it in the session's details bar too, so it has to read the same on every run and every machine and name nothing
# of the machine that took the picture. The run directory (`/tmp/farhelm-tmp/<uuid>/...`) was not stable: each capture
# showed its own random scratch path, and every picture differed from the last for that alone.
#
# NOTE: Not the home directory, though it is the folder the old app's pictures came closest to: the row folds a home to
# `~` only for a path below it, and leaves a bare `/home/<user>` alone (`abbreviate_home` in the session list), so the
# account name would show. `/tmp` exists everywhere and is used as it is: nothing is created there, so the rule against a
# harness making a fixed path under `/tmp` is kept. The fake agent writes nothing to its working directory; the
# transcripts it replays stay in the run directory.
SESSION_CWD = Path("/tmp")


def seed_sessions(run: Path, kinds: list[str], fixtures: str) -> list[str]:
    """Create one fake-agent session per entry of `kinds` (see FLEET_KINDS) through the embedded helm's API, in that
    order, each running in `SESSION_CWD`; return their ids. A replayed kind's transcript is a file in the run's `work`
    directory, so it goes with the run."""
    work = run / "work"
    work.mkdir(exist_ok=True)
    ids = []
    for index, kind in enumerate(kinds, start=1):
        transcript_file = None
        if kind != "basic":
            transcript_file = work / f"fixture-{index}.txt"
            transcript_file.write_text(transcript(index, kind))
        body = seed_body(index, SESSION_CWD, fixtures, kind, transcript_file)
        ids.append(helm_request(run, "POST", "/api/sessions", body)["id"])
    return ids


def wait_for_statuses(
    read_states, targets: dict[str, str], timeout: float = SETTLE_TIMEOUT_SECONDS, interval: float = 1.0
) -> None:
    """Wait until `read_states()` (the listing's state by id) reports every id in `targets` at its target state, or
    fail naming each session that is not and the state it is in.

    The statuses are the supervisor's own classification of each live screen, never written in by the harness, so a
    scene can show a waiting or idle session only after the supervisor has read it as one.
    """
    deadline = time.monotonic() + timeout
    while True:
        pending = unsettled(targets, read_states())
        if not pending:
            return
        if time.monotonic() > deadline:
            raise RuntimeError(
                f"seeded sessions did not reach their status within {timeout:.0f} s: {', '.join(pending)}"
            )
        time.sleep(interval)  # sleep-ok: polling the helm's listing; the harness subscribes to no status events


def cli(run: Path, record: dict, args: list[str], capture: bool = False) -> subprocess.CompletedProcess:
    """Run the bridge's command line against this run's app."""
    env = bridge_env(dict(os.environ), run, record.get("runtime"), record.get("display"), record.get("dbus"))
    return subprocess.run([record["binaries"]["cli"], *args], env=env, capture_output=capture, text=True)


def start(run: Path, args: list[str]) -> int:
    import argparse

    parser = argparse.ArgumentParser(prog="gui-agent.py RUN_DIR start")
    seeding = parser.add_mutually_exclusive_group()
    seeding.add_argument("--sessions", type=int, help="basic fake-agent sessions to seed (default 2)")
    seeding.add_argument(
        "--fleet",
        type=parse_fleet,
        help=f"seed one session per kind, in order, and wait for each one's status ({', '.join(FLEET_KINDS)})",
    )
    parser.add_argument("--size", help="Linux only: Mutter's virtual monitor (default 1600x1200)")
    parser.add_argument("--no-build", action="store_true", help="use the binaries already in the target directory")
    parser.add_argument(
        "--lock",
        type=Path,
        help="hold an exclusive flock on this file from before the build until `stop` (a shared GUI session's lock)",
    )
    parser.add_argument("--window-size", help="the app window's first size, WxH (agent-build test hook)")
    parser.add_argument(
        "--appearance", choices=("light", "dark"), help="start the app in this palette (agent-build test hook)"
    )
    parser.add_argument(
        "--scripted-hosts",
        action="store_true",
        help="fake the commands the helm runs on hosts with its scripted backend (see docs/gui-agent.md)",
    )
    options = parser.parse_args(args)
    if options.window_size:
        parse_size(options.window_size)
    mac = sys.platform == "darwin"
    if mac:
        # The window opens in the console's GUI session, so this must run as the user logged in there (SSH is fine:
        # a process of that user can open windows in that user's session, which the test Mac's earlier runs showed).
        owner = console_owner()
        if owner != os.getuid():
            raise UsageError(f"no GUI session of yours on this Mac (the console belongs to {user_name(owner)})")
        if options.size:
            raise UsageError("--size sets Mutter's virtual monitor on Linux; a Mac run uses the real screen")
    elif not sys.platform.startswith("linux"):
        raise UsageError("runs need Linux or macOS")
    else:
        missing = [path for path in (LAVAPIPE_ICD, MESA_EGL_VENDOR) if not Path(path).exists()]
        if missing:
            raise UsageError(f"the software renderer is not installed: {', '.join(missing)} missing")
        if options.window_size:
            # Mutter never grows its virtual monitor, and the app fits a first window to the display, so a window
            # larger than the monitor would open smaller than asked and be captured under the wrong size.
            monitor = options.size or DEFAULT_MONITOR
            (want_w, want_h), (have_w, have_h) = parse_size(options.window_size), parse_size(monitor)
            if want_w > have_w or want_h > have_h:
                raise UsageError(
                    f"--window-size {options.window_size} does not fit Mutter's {monitor} monitor; pass --size"
                )
    run.mkdir(mode=0o700)
    for sub in ("agent", "logs") if mac else ("runtime", "agent", "logs"):
        (run / sub).mkdir(mode=0o700)
    record: dict = {"state": str(run / "state")}

    def save() -> None:
        (run / "run.json").write_text(json.dumps(record, indent=2))

    if options.lock:
        # Taken before the build: on the test Mac heavy builds hold the same Mac-wide lock as real-window runs. The
        # holder is recorded before the wait, so a `start` that fails or is killed at any later point still leaves
        # `stop` able to release the lock.
        def record_lock(lock: dict) -> None:
            record["lock"] = lock
            save()

        hold_lock(run, options.lock, on_started=record_lock)
    target = target_dir()
    if not options.no_build:
        build(target)
    binaries = {
        "desktop": f"{target}/debug/farhelm-desktop",
        "farhelm": f"{target}/debug/farhelm",
        "fixtures": f"{target}/debug/farhelm-fixtures",
        "cli": f"{target}/debug/gpui-agent",
    }
    record["binaries"] = binaries
    save()
    if not mac:
        display = display_name(uuid.uuid4().hex[:8])
        record.update({"runtime": str(run / "runtime"), "display": display})
        size = options.size or DEFAULT_MONITOR
        mutter = start_mutter(run, run / "runtime", display, mutter_command(display, size), dict(os.environ))
        record.update({"mutter": {"pid": mutter["pid"], "start": mutter["start"]}, "dbus": mutter["dbus"]})
    save()
    scripted_hosts = prepare_scripted_hosts(run) if options.scripted_hosts else None
    launched = cli(
        run,
        record,
        [
            "launch",
            "--timeout",
            str(LAUNCH_TIMEOUT_SECONDS),
            *app_env_args(run, binaries, options.window_size, options.appearance, scripted_hosts),
            "--",
            binaries["desktop"],
        ],
    )
    # The app's identity, recorded whether or not the launch succeeded (a launch that timed out can leave the app
    # running), so `stop` signals it only while that pid is still this app.
    record["app"] = app_identity(run)
    save()
    if launched.returncode != 0:
        print(
            f"the app did not start; its log is {run}/agent/{SESSION}/app.log. Run `stop` to clean up.",
            file=sys.stderr,
        )
        return launched.returncode
    kinds = options.fleet or ["basic"] * (2 if options.sessions is None else options.sessions)
    if kinds:
        ids = seed_sessions(run, kinds, binaries["fixtures"])
        print(f"seeded {len(ids)} fake-agent sessions: {', '.join(ids)}")
        targets = {id: kind for id, kind in zip(ids, kinds) if kind != "basic"}
        if targets:
            wait_for_statuses(lambda: listed_states(run), targets)
            print(f"every seeded session reached its status: {', '.join(kinds)}")
    return 0


def terminal_text(run: Path, record: dict, args: list[str] | None = None, interval: float = 0.25) -> int:
    """Print the focused terminal's text through the app's `terminal_text` probe, which the terminal registers.

    The probe answers null when no terminal holds focus (a script clicked a button, say), and that is reported as such
    with a failing status rather than printed, so it cannot pass for an empty screen. Click the terminal or Tab back
    into it first.

    `--wait TEXT [--timeout SECONDS]` (default 30) asks again until the focused terminal's text contains TEXT, then
    prints it; on timeout it fails saying what the last answer was. It is the readiness check for a terminal that was
    just opened: a click hands the terminal focus and starts its attach, and the screen it catches up to arrives after
    the click returns, so a screenshot or a keystroke right after the click could land before it.
    """
    import argparse

    parser = argparse.ArgumentParser(prog="gui-agent.py RUN_DIR terminal-text")
    parser.add_argument("--wait", metavar="TEXT", help="wait until the focused terminal's text contains TEXT")
    parser.add_argument("--timeout", type=float, default=30, help="seconds --wait waits (default 30)")
    options = parser.parse_args(args or [])
    deadline = time.monotonic() + options.timeout
    while True:
        result = cli(run, record, ["get", "probe", "terminal_text"], capture=True)
        if result.returncode != 0:
            problem = f"no terminal yet (the app has no `terminal_text` probe): {result.stderr.strip()}"
        elif result.stdout.strip() == "null":
            problem = "no terminal holds focus"
        elif options.wait is not None and options.wait not in probe_text(result.stdout):
            problem = f"the focused terminal does not show {options.wait!r}; it shows {result.stdout.strip()}"
        else:
            print(result.stdout, end="")
            return 0
        if options.wait is None or time.monotonic() > deadline:
            print(problem, file=sys.stderr)
            return 1
        time.sleep(interval)  # sleep-ok: polling the app's probe; the bridge has no wait on a probe's value


def probe_text(stdout: str) -> str:
    """The text a `terminal_text` probe answer carries: the bridge prints it as a JSON string, whose escapes (a quote,
    a line break) must be undone before matching; anything that does not decode is matched as printed."""
    try:
        value = json.loads(stdout)
    except ValueError:
        return stdout
    return value if isinstance(value, str) else stdout


def stop(run: Path, record: dict) -> int:
    """Take down everything `start` started: the app, its supervisor and terminal hosts, then Mutter and its bus (on
    Linux), and last the lock holder. The shared lock is released only when every earlier step succeeded: if part of
    the run may still be running, the lock is kept (the message says so) and a second `stop` retries everything.

    Each step runs even if an earlier one failed, and each acts only on processes it can tie to this run: the app
    through the pid and start time recorded after launch, the supervisor through the app's record of it plus the state
    directory on its command line, Mutter through the pid and start time recorded when it started. A pid that has since
    been reused by another process is never signalled.
    """
    problems = []
    # The app first, the ordinary way: SIGTERM, which the app turns into a quit (it then waits for its helm to stop),
    # and SIGKILL if it is still there after the grace. Not the bridge's `close`, which signals the recorded pid and its
    # process group without checking that the pid is still the app.
    app = record.get("app")
    if app and not stop_process(app["pid"], app["start"], 15):
        problems.append(f"the app (pid {app['pid']}) did not stop")
    for name in SESSION_FILES:
        (run / "agent" / SESSION / name).unlink(missing_ok=True)
    state = Path(record["state"])
    supervisor = supervisor_to_stop(state)
    if supervisor and not stop_process(supervisor[0], supervisor[1], 20):
        problems.append(f"the supervisor (pid {supervisor[0]}) did not stop")
    farhelm = record.get("binaries", {}).get("farhelm")
    if farhelm and (state / "hosts").exists():
        hosts = subprocess.run([farhelm, "internal", "stop-terminal-hosts", "--state-dir", str(state)])
        if hosts.returncode != 0:
            problems.append("stop-terminal-hosts could not confirm every terminal host stopped")
    mutter = record.get("mutter")
    if mutter:
        for pid in stop_tree(mutter["pid"], mutter["start"], 10):
            problems.append(f"compositor process {pid} did not stop")
    lock = record.get("lock")
    if lock and problems:
        problems.append(
            f"keeping the lock on {lock['path']} while part of the run may still be running; run `stop` again"
        )
    elif lock and not stop_process(lock["pid"], lock["start"], 5):
        problems.append(f"the lock holder (pid {lock['pid']}) did not stop; {lock['path']} may still be locked")
    for problem in problems:
        print(f"gui-agent: {problem}", file=sys.stderr)
    return 1 if problems else 0


def main(argv: list[str]) -> int:
    if len(argv) < 2 or argv[0] in ("-h", "--help"):
        print(__doc__)
        return 0 if argv[:1] in (["-h"], ["--help"]) else 2
    run, verb, rest = Path(argv[0]), argv[1], argv[2:]
    try:
        check_run_dir(run)
        if verb == "start":
            return start(run, rest)
        record = json.loads((run / "run.json").read_text())
        if verb == "stop":
            return stop(run, record)
        if verb == "terminal-text":
            return terminal_text(run, record, rest)
        return cli(run, record, [verb, *rest]).returncode
    except UsageError as error:
        print(f"gui-agent: {error}", file=sys.stderr)
        return 2
    except FileNotFoundError as error:
        print(f"gui-agent: {error}; was `start` run for this directory?", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
