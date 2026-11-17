#!/usr/bin/env python3
"""Drive the GPUI desktop app as an agent: launch it against a real helm and supervisor, then look and click.

NOTE: Debug builds only, and never against the live install. On Linux a run is headless, on a private Mutter; on a Mac
it opens a real window in the logged-in GUI session, which this script must run inside (over SSH is fine). Everything
a run starts lives under the run directory you name (which must not exist yet), and `stop` takes all of it down again.

    scripts/gui-agent.py RUN_DIR start [--sessions N | --fleet KIND,...] [--size WxH (Linux)] [--no-build]
                                       [--lock FILE] [--window-size WxH] [--appearance light|dark]
                                       [--templates] [--scripted-hosts] [--start-dialogs] [--payload-dir DIR]
                                       [--remote-host DEST ... | --user-session (Linux)]
    scripts/gui-agent.py RUN_DIR snapshot -i         # any gpui-agent command: click, type, press, wait, screenshot ...
    scripts/gui-agent.py RUN_DIR terminal-text [--wait TEXT [--timeout SECONDS]]
    scripts/gui-agent.py RUN_DIR stop

`start` builds `farhelm-desktop` with the `agent` feature (the test-automation bridge), the `farhelm` CLI the app
starts its supervisor from, the fake agents (`farhelm-fixtures`) and the bridge's own command line; on Linux starts a
private headless Mutter on its own session bus; launches the app (on that Mutter, or on the Mac's GUI session) with its
own state directory; and seeds sessions that run the fake agent, through the real helm API: `--sessions` basic ones, or
with `--fleet` one per kind named (FLEET_KINDS), after which it waits until the supervisor has read each replayed one
as the status its kind names. On Linux, rendering is Mesa's software Vulkan driver (lavapipe), the same as the
screenshot goldens; on a Mac it is Metal.
`--lock FILE` holds an exclusive `flock` on FILE from before the build until `stop`, for a machine whose one GUI
session several runs share (the test Mac's convention). `--window-size` and `--appearance` start the app's window at
that size and in that palette (agent builds only; `crates/gui/farhelm-gui/src/agent_startup.rs`), so a screenshot batch
can capture each theme and size; they are test hooks, not the app's own appearance setting. A run's helm is always
fresh, so the Welcome card would open over the app and catch the run's first click; `start` therefore turns the
window's start dialogs (that card and the announcement batch after it) off through the same agent-build overrides, and
`--start-dialogs` leaves them on to see the real start. `--templates` seeds a fixed set of launch templates with the
⚡ mark and number keys over them (SEEDED_TEMPLATES), for screenshot scenes of templates and quick launch.
`--scripted-hosts` makes the helm fake every command it runs on a host (setup, Update and the rest) with its scripted
provisioning backend
(SCRIPTED_HOSTS_ENV, debug builds only), controlled with `script_hosts`. `--payload-dir DIR` hands the app's embedded
helm a directory of staged `farhelm` payloads to install on and update real remote hosts from (PAYLOAD_DIR_ENV), which
a debug build otherwise has no release to download from; it is the one way to pass that setting in, since the caller's
own `FARHELM_*` variables never reach the app. `--remote-host DEST` (repeatable) makes the run a staged one: it starts
one more supervisor per destination, each in the run's own `r<N>` state directory, and has the app's helm register
each as an SSH host reached at DEST, which must be this machine under a spelling of its own. `--user-session` (Linux)
gives the app the caller's own runtime directory and session bus, so that its helm's Podman check reaches the user's
systemd manager and New container shows; such a run shares that manager, and `stop` removes what the run's helm and
supervisor left there by recorded name (USER_SESSION_ENV, `remove_user_session_objects`). Any other verb goes to the
vendored `gpui-agent` command line, already pointed at this run's app; `gpui-agent --help` and
vendor/gpui-agent/README.md list them. docs/gui-agent.md is the guide.

Why Mutter rather than the bridge's own Xvfb: the shipped app is built for Wayland only (the root Cargo.toml's
`gpui_platform` features), so runs use Wayland to match what users get. Agent builds do also compile GPUI's X11
backend (the vendored bridge crate turns it on), but driving that would test a path no shipped build has. Mutter also
exercises the client-side decoration path a GNOME user gets.

Names that could collide with another run (the Wayland display, the bridge socket, the runtime directory) all derive
from the run directory, so parallel runs from different directories never share anything (AGENTS.md, "Sharing the
machine with other agents"). The bridge socket is unauthenticated, so it sits in a 0700 directory.

Staged runs (docs/gui-agent.md, "Staged runs", has the whole arrangement and why). A staged run launches its sessions
by an agent harness's bare name, so it always runs the app, its supervisor and every remote supervisor with a private
HOME and PATH inside the run, where every harness name is a wrapper around the fake agent, and with the scripted
provisioning backend on, since a staged remote host is this machine. `staged_start` says which runs are staged.

User-session runs (docs/gui-agent.md, "User-session runs"). Everything else about the run stays private: its state,
its compositor (which keeps its own runtime directory and bus), the caller's `FARHELM_*` variables kept out. The one
thing shared is the caller's systemd user manager and, through the caller's HOME, its Podman storage. A staged run
refuses the option (`check_user_session`).
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
# A staged run's remote supervisors' sockets (`<run>/r<N>/...`, `remote_state_dir`) are shorter than `state`'s.
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

    Starts from `base` with every display and bridge variable from the caller's session removed. On Linux the app then
    gets the `runtime`, `display` and `dbus` given, which `session_vars` chooses (this run's Mutter and its private
    bus, unless the run is a user-session one), and rendering is pinned to lavapipe. On a Mac there is no display
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


# ---------------------------------------------------------------------------------------------------------------------
# Staged runs: the private HOME and PATH, and the remote hosts


# The variable naming the ensure-hosts file an agent build's embedded helm registers at startup (`ENSURE_HOSTS_ENV`,
# defined in crates/farhelm-desktop/src/agent.rs and read by `DesktopBootstrap::start` in lib.rs). Must match exactly.
ENSURE_HOSTS_ENV = "FARHELM_DESKTOP_AGENT_ENSURE_HOSTS"

# Every program the app launches an agent harness as, by bare name (`program` in crates/farhelm-helm/src/launches.rs,
# which test-gui-agent.py checks this against), each shadowed by a wrapper of the run's in a staged run. A launch the
# launcher or a template composes names one of these, and a staged scenario's sessions do too.
STAGED_AGENTS = ("codex", "claude", "muse", "cursor-agent", "opencode", "goose", "pi", "omp", "grok")

# The file a wrapper runs, with `sh`, from the session's working directory when it is there: a scenario writes one into
# each session's scratch directory to give it its own replay (`exec <fixtures> fake-agent --script replay ... "$@"`).
# The wrapper passes it every argument the session's command line and the supervisor gave the agent.
STAGED_AGENT_SCRIPT = ".staged-agent"

# The login shell a staged run's sessions start in, and the system directories its PATH holds after the wrappers.
# Fixed rather than the caller's because the private HOME carries only the profiles bash and sh read: a login bash reads
# its `.bash_profile`, which is what puts the wrappers first again after the system's profile (macOS's path_helper
# rebuilds PATH with the system's directories first), while a caller's zsh would read files nothing here writes.
STAGED_SHELL = "/bin/bash"
STAGED_SYSTEM_PATH = "/usr/bin:/bin:/usr/sbin:/sbin"

# Variables that point a program at the caller's own configuration rather than at HOME, dropped from a staged run's
# environment so that nothing the app starts can find the caller's shell startup files or an agent's configuration.
STAGED_DROPPED_ENV = (
    "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME", "ZDOTDIR", "BASH_ENV", "ENV"
)

# Credentials and vendor settings, dropped from a staged run's environment by name pattern (`staged_drops`): a vendor's
# own variables (`ANTHROPIC_API_KEY`, `CLAUDE_CODE_OAUTH_TOKEN`, `CODEX_HOME`, and under `AWS_` the cloud credentials
# Claude Code's Bedrock mode uses, `AWS_BEARER_TOKEN_BEDROCK` and `AWS_PROFILE` among them) and anything named like a
# key or a token (`OPENROUTER_API_KEY`, `GITHUB_TOKEN`, ...). A pattern rather than a list, because no list keeps up
# with every harness's variables; a staged run needs none of them, since nothing in it talks to a vendor.
STAGED_DROPPED_PREFIXES = (
    "ANTHROPIC_", "CLAUDE_", "OPENAI_", "CODEX_", "GEMINI_", "GOOGLE_", "XAI_", "GROK_", "AWS_"
)
STAGED_DROPPED_SUFFIXES = ("_KEY", "_TOKEN", "_SECRET", "_PASSWORD")

# How long a remote supervisor gets to bind its socket.
SUPERVISOR_SOCKET_SECONDS = 30


def staged_start(options) -> bool:
    """Whether a `start` with these options is a staged run, which always has the private HOME and PATH and the
    scripted provisioning backend: one with remote hosts. A scenario front end adds its own option here, so that a
    scenario with only the local host is staged as well; a plain `start` stays unstaged until the private environment
    has been checked on a Mac."""
    return bool(options.remote_host)


def staged_record(record: dict) -> bool:
    """Whether the run `record` describes is a staged one: `start` records its staging only for a staged run, and every
    later launch (a relaunch included) decides from this, never from options it no longer has."""
    return bool(record.get("staging"))


def wrapper_script(name: str, fixtures: str) -> str:
    """The wrapper that stands in for the real `name` (a harness's program) in a staged run: it runs the session's
    STAGED_AGENT_SCRIPT when its working directory has one, and the fake agent's basic script otherwise, passing every
    argument on. It never looks for another program of that name."""
    return (
        "#!/bin/sh\n"
        f"# A staged run's stand-in for `{name}` (scripts/gui-agent.py, \"Staged runs\"). It never runs the real one.\n"
        f'if [ -f "$PWD/{STAGED_AGENT_SCRIPT}" ]; then\n'
        f'  exec /bin/sh "$PWD/{STAGED_AGENT_SCRIPT}" "$@"\n'
        "fi\n"
        f'exec {shlex.quote(fixtures)} fake-agent --script basic "$@"\n'
    )


def prepare_staging(run: Path, fixtures: str) -> dict:
    """Create a staged run's private HOME and wrapper directory and return what the run records of them (`home`,
    `bin`, `shell`, `path`): the environment `staged_env` builds from it.

    The private HOME holds only a login profile (`.bash_profile`, and `.profile` for a shell that reads that) that puts
    the wrappers first on PATH. The supervisor starts every session through the login shell, which runs the system's
    profile before this one, so whatever the system adds to PATH lands behind the wrappers."""
    home, bin_dir = run / "staging" / "home", run / "staging" / "bin"
    home.mkdir(mode=0o700, parents=True)
    bin_dir.mkdir(mode=0o700)
    for name in STAGED_AGENTS:
        wrapper = bin_dir / name
        wrapper.write_text(wrapper_script(name, fixtures))
        wrapper.chmod(0o700)
    profile = (
        "# A staged run's private login profile (scripts/gui-agent.py, \"Staged runs\"): its wrappers first on PATH.\n"
        f'export PATH={shlex.quote(str(bin_dir))}:"$PATH"\n'
    )
    for name in (".bash_profile", ".profile"):
        (home / name).write_text(profile)
    return {"home": str(home), "bin": str(bin_dir), "shell": STAGED_SHELL, "path": f"{bin_dir}:{STAGED_SYSTEM_PATH}"}


def staged_drops(name: str) -> bool:
    """Whether a staged run's environment drops the variable `name`: one of STAGED_DROPPED_ENV, or a credential or
    vendor setting by the patterns STAGED_DROPPED_PREFIXES and STAGED_DROPPED_SUFFIXES."""
    return (
        name in STAGED_DROPPED_ENV
        or name.startswith(STAGED_DROPPED_PREFIXES)
        or name.endswith(STAGED_DROPPED_SUFFIXES)
    )


def staged_env(base: dict[str, str], staging: dict) -> dict[str, str]:
    """`base` with the staged run's private HOME, PATH and login shell in place of the caller's, and without the
    variables `staged_drops` names: those that would point a program back at the caller's configuration, and
    credentials."""
    env = {key: value for key, value in base.items() if not staged_drops(key)}
    env.update({"HOME": staging["home"], "PATH": staging["path"], "SHELL": staging["shell"]})
    return env


def shadowing_agents(directory: Path) -> list[str]:
    """The STAGED_AGENTS names a file in `directory` has.

    The launch shim puts the directory of the supervisor's `farhelm` first on a session's PATH, ahead of everything the
    login shell set (crates/farhelm-supervisor/src/launch.rs, `farhelm_bin_dir`), so a harness's program there would
    win over the wrappers. A staged run refuses to start when this finds one."""
    return [name for name in STAGED_AGENTS if (directory / name).exists()]


def parse_remote_hosts(destinations: list[str]) -> list[str]:
    """`--remote-host`'s destinations, in order; a UsageError for an empty one, one ssh would read as an option or that
    holds whitespace, or one listed twice (the helm keys a host on its destination)."""
    for destination in destinations:
        if not destination or destination.startswith("-") or any(c.isspace() for c in destination):
            raise UsageError(f"--remote-host takes an ssh destination such as localhost, not {destination!r}")
    if len(set(destinations)) != len(destinations):
        raise UsageError(f"each --remote-host must be a different spelling: {', '.join(destinations)}")
    return destinations


def check_ssh(destination: str) -> None:
    """Refuse a destination this machine cannot ssh to without a password, so a missing key fails `start` naming the
    destination instead of leaving the host unreachable in the window. The options are the old browser staging's probe,
    at least as strict as the helm's own dial (which sets `BatchMode` too)."""
    result = subprocess.run(
        ["ssh", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes", "-o", "ConnectTimeout=10", destination,
         "true"],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise UsageError(
            f"passwordless ssh to {destination!r} failed; a staged remote host is this machine reached over ssh: "
            f"{result.stderr.strip()}"
        )


def remote_state_dir(run: Path, index: int) -> Path:
    """The state directory of the run's `index`th remote supervisor (from 0, in `--remote-host` order)."""
    return run / f"r{index}"


def ensure_hosts_body(run: Path, destinations: list[str], farhelm: str) -> dict:
    """The ensure-hosts file's contents (the helm's `--ensure-hosts` format): each destination as an SSH host whose
    supervisor is this run's `farhelm` serving `remote_state_dir`. Both paths are given, and absolute, so the helm dials
    exactly the supervisor the run started, never an installation at that destination's default place."""
    return {
        "hosts": [
            {"ssh": destination, "remote_farhelm": farhelm, "remote_state_dir": str(remote_state_dir(run, index))}
            for index, destination in enumerate(destinations)
        ]
    }


def start_remote_supervisors(
    run: Path,
    destinations: list[str],
    farhelm: str,
    env: dict[str, str],
    on_started,
    socket_wait: float = SUPERVISOR_SOCKET_SECONDS,
) -> list[dict]:
    """Start one supervisor per destination, `farhelm supervisor run` in `remote_state_dir`, each in a session of its
    own with `env` and its log in `logs/r<N>.log`, and wait until each has bound its socket; return the run's record of
    them (`destination`, `state`, `pid`, `start`).

    `on_started` receives the record after every start, before any wait, so `stop` can find a supervisor that `start`
    left behind by failing or being killed later. A supervisor that exits, or binds no socket within `socket_wait`
    seconds, fails the start naming its log; the ones already started are left for `stop`."""
    remotes: list[dict] = []
    for index, destination in enumerate(destinations):
        state = remote_state_dir(run, index)
        state.mkdir(mode=0o700)
        with (run / "logs" / f"r{index}.log").open("wb") as log:
            child = subprocess.Popen(
                [farhelm, "supervisor", "run", "--state-dir", str(state)],
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
        remotes.append({"destination": destination, "state": str(state), "pid": child.pid,
                        "start": proc_start(child.pid)})
        on_started(remotes)
    for index, remote in enumerate(remotes):
        wait_for(
            Path(remote["state"]) / "supervisor.sock",
            socket_wait,
            f"the supervisor for {remote['destination']} (log {run}/logs/r{index}.log)",
            alive=lambda: running(remote["pid"], remote["start"]),
        )
    return remotes


def remote_supervisor_to_stop(remote: dict) -> tuple[int, str] | None:
    """A recorded remote supervisor as `(pid, start)` while it is still the process `start` started (`claim`), else
    None."""
    return claim(remote.get("pid"), remote.get("start"), remote["state"])


def stop_terminal_hosts(farhelm: str | None, state: Path) -> bool:
    """End every terminal host whose socket is in `state` with the command an uninstall runs; True when none may still
    run (also when the state directory never had any)."""
    if not farhelm or not (state / "hosts").exists():
        return True
    return subprocess.run([farhelm, "internal", "stop-terminal-hosts", "--state-dir", str(state)]).returncode == 0


# ---------------------------------------------------------------------------------------------------------------------
# User-session runs: the caller's runtime directory and session bus for the app, and what `stop` removes after one


# The two variables a `--user-session` run gives the app from the caller instead of the run's own. With them the app's
# embedded helm and its local supervisor reach the user's systemd manager (`systemctl --user`, `systemd-run --user`)
# and `podman` finds its real run root under the runtime directory; without them the helm's Podman check fails, so New
# container never shows.
USER_SESSION_ENV = ("XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS")

# What every container the helm creates for an environment session is named (`farhelm-sess-<helm tag>-...`, which
# `register_pending_session_container` in crates/farhelm-helm/src/store/podman.rs insists on). `stop` removes a name it
# recorded from the run's helm only when it has this shape, so an odd listing cannot make it remove anything else.
SESSION_CONTAINER_PREFIX = "farhelm-sess-"


def check_user_session(base: dict[str, str], platform: str, staged: bool) -> dict:
    """What a `--user-session` run records of the caller (`runtime`, `dbus`, from USER_SESSION_ENV in `base`); a
    UsageError when the option cannot do what it is for.

    It is Linux only: a Mac run already keeps the GUI session's environment. It refuses a caller without an absolute
    runtime directory or a bus address, since the app would then still miss the user manager. And it refuses a staged
    run: there the app's HOME is the run's private one, so `podman` would make itself a storage under it while its run
    root, under the caller's runtime directory, stays the account's, and two storages sharing one run root share its
    mount records (`containers/overlay-layers/mountpoints.json`), which could disturb the account's own containers."""
    if not platform.startswith("linux"):
        raise UsageError("--user-session is Linux only: a Mac run already uses its GUI session's environment")
    if staged:
        raise UsageError(
            "--user-session does not combine with a staged run: its private HOME would give Podman a storage of its "
            "own that shares the account's run root (docs/gui-agent.md, \"User-session runs\")"
        )
    runtime, dbus = (base.get(name) for name in USER_SESSION_ENV)
    if not runtime or not Path(runtime).is_absolute() or not dbus:
        raise UsageError(f"--user-session needs {' and '.join(USER_SESSION_ENV)} set in the caller's environment")
    return {"runtime": runtime, "dbus": dbus}


def session_vars(record: dict) -> tuple[str | None, str | None, str | None]:
    """The runtime directory, Wayland display and session bus everything after the compositor gets, as `bridge_env`
    takes them: the run's private ones, or in a user-session run the caller's runtime directory and bus with the
    compositor's socket named by its absolute path (a bare name would be looked up in the caller's runtime directory,
    where it is not; GPUI takes an absolute `WAYLAND_DISPLAY`). The compositor keeps its own either way
    (`start_mutter`), so it never registers on the caller's bus."""
    user = record.get("user_session")
    if not user:
        return record.get("runtime"), record.get("display"), record.get("dbus")
    return user["runtime"], str(Path(record["runtime"]) / record["display"]), user["dbus"]


def uuid_shaped(text) -> bool:
    """Whether `text` is a UUID in the canonical lowercase hyphenated spelling the supervisor mints session ids in."""
    try:
        return isinstance(text, str) and str(uuid.UUID(text)) == text
    except ValueError:
        return False


def run_owned_unit(unit: str, sessions: list[str]) -> bool:
    """Whether the user-manager unit `unit` is one the run's own supervisor named for one of `sessions`, the session
    ids `record_user_session_objects` recorded (UUIDs only): `farhelm-<session>-<generation>.scope`, a launch's scope,
    or `farhelm-<session>-tab-<tab uuid>.scope`, a terminal tab's (crates/farhelm-supervisor/src/scope.rs). Exactly
    these shapes, and nothing that merely starts with `farhelm-`.

    Not matched, because nothing ties them to the run: the throwaway probe scopes the supervisor and the helm name with
    fresh random ids (`farhelm-probe-<uuid>.scope`, `farhelm-env-probe-<hex>.scope`), which their creators stop at once.
    Nor the scope a container's Podman commands ran in (`farhelm-env-<container>-<hex>.scope`, which keeps the
    container's conmon): it empties and is collected once `podman rm` removed the container, and stopping it while the
    container is still there would only leave that container without its monitor."""
    stem = unit.removesuffix(".scope")
    for session in sessions:
        tail = stem.removeprefix(f"farhelm-{session}-")
        if stem != unit and tail != stem and (
            (tail.isascii() and tail.isdigit()) or (tail.startswith("tab-") and uuid_shaped(tail[4:]))
        ):
            return True
    return False


def record_user_session_objects(run: Path, record: dict, request=None) -> str | None:
    """Add to a user-session run's record the id of every session its helm lists on the local host and the name of
    every environment container it made there (`kind: podman` hosts placed on the local host), and save it, so `stop`
    can remove them by name once the helm is gone; return a warning when the helm could not be asked. `request` is
    `helm_request` unless a test hands in its own.

    Local only, because the run's scopes and containers are only those on this machine's user manager and Podman
    storage: a host added while the run is driven could be this machine's own installation reached over ssh, and its
    sessions' scopes are on the same user manager, so recording them would make `stop` end them. Runs at `stop`, before
    the app quits, since sessions and containers come and go while the run is driven. What earlier calls recorded is
    kept, so a second `stop` after the app is gone still removes it. A session id that is not a UUID, or a container
    whose name lacks SESSION_CONTAINER_PREFIX, is not recorded."""
    request = request or helm_request
    if not (run / "helm.json").exists():
        return None
    try:
        hosts = request(run, "GET", "/api/hosts")["hosts"]
        rows = request(run, "GET", "/api/sessions")["sessions"]
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        return f"could not ask the run's helm what it made ({error}); removing only what an earlier stop recorded"
    local = {host.get("id") for host in hosts if host.get("kind") == "local"}
    # The helm lists a session as `<host entry>:<id>`, the id after the colon being the bare one the supervisor minted
    # and names the session's scopes with.
    sessions = [str(row.get("id")).rpartition(":")[2] for row in rows if row.get("host") in local]
    containers = []
    for host in hosts:
        environment = host.get("environment") or {}
        place = (environment.get("target") or {}).get("place") or {}
        if host.get("kind") == "podman" and place.get("kind") == "podman_host" and place.get("host") in local:
            containers.append(environment.get("name"))
    user = record["user_session"]
    user["sessions"] = list(dict.fromkeys([*user.get("sessions", []), *filter(uuid_shaped, sessions)]))
    named = [name for name in containers if isinstance(name, str) and name.startswith(SESSION_CONTAINER_PREFIX)]
    user["containers"] = list(dict.fromkeys([*user.get("containers", []), *named]))
    (run / "run.json").write_text(json.dumps(record, indent=2))
    return None


def remove_user_session_objects(
    user: dict, base: dict[str, str], podman: str = "podman", systemctl: str = "systemctl"
) -> list[str]:
    """Remove what a user-session run left on the caller's account, by the names `record_user_session_objects`
    recorded, and return the problems: each recorded container (`podman rm --force --ignore`, which also ends the
    container's own scopes), then every unit of the user manager that `run_owned_unit` ties to a recorded session,
    stopped by its exact name, and a check that none of those is left. With nothing recorded it does nothing, so a run
    that never got as far as its app (or whose app crashed) cannot fail `stop` and keep its lock over a listing.

    Run after the app, its supervisor and its terminal hosts are down, so nothing of the run makes more. The units are
    listed without a pattern and matched here, never handed to systemctl as a glob. Images the run's helm built
    (`localhost/farhelm-<helm tag>-...`) are not removed; deleting the run's recipe in the app removes them."""
    sessions, containers = user.get("sessions", []), user.get("containers", [])
    if not (sessions or containers):
        return []
    env = {**base, "XDG_RUNTIME_DIR": user["runtime"], "DBUS_SESSION_BUS_ADDRESS": user["dbus"]}
    problems = []

    def call(*argv: str) -> subprocess.CompletedProcess | None:
        try:
            return subprocess.run(argv, env=env, stdin=subprocess.DEVNULL, capture_output=True, text=True)
        except OSError:
            return None

    def owned_units() -> list[str] | None:
        listed = call(systemctl, "--user", "list-units", "--all", "--plain", "--no-legend", "--type=scope")
        if listed is None or listed.returncode != 0:
            return None
        names = [line.split()[0] for line in listed.stdout.splitlines() if line.split()]
        return [name for name in names if run_owned_unit(name, sessions)]

    for name in containers:
        removed = call(podman, "rm", "--force", "--ignore", "--", name)
        if removed is None or removed.returncode != 0:
            problems.append(f"podman could not remove the run's container {name}")
    for unit in owned_units() or []:
        call(systemctl, "--user", "stop", "--", unit)
    left = owned_units()
    if left is None:
        problems.append("could not list the user manager's units to check that none of the run's is left")
    elif left:
        problems.append(f"the run's units are still on the user manager: {', '.join(left)}")
    return problems


# The scripted provisioning backend's switch, read by debug builds of the helm (`ProductionProvisioning::new` in
# crates/farhelm-helm/src/provisioning/service.rs), and the marker its directory must hold (`E2E_BACKEND_MARKER` in
# provisioning/e2e.rs). Both must match the helm's exactly.
SCRIPTED_HOSTS_ENV = "FARHELM_E2E_PROVISIONING_BACKEND_DIR"
SCRIPTED_HOSTS_MARKER = "farhelm-e2e-provisioning-v1\n"


def scripted_hosts_dir(run: Path) -> Path:
    """Where `start --scripted-hosts` puts the scripted provisioning backend's directory: inside the app's state
    directory, because the helm refuses a backend directory outside its own state."""
    return run / "state" / "scripted-hosts"


# The provisioning payload directory the desktop app hands its embedded helm (`helm_payload_dir` in
# crates/farhelm-desktop/src/bundle.rs), the same variable the standalone `farhelm helm run --payload-dir` reads. Every
# build reads it, not only agent builds. Must match the app's exactly.
PAYLOAD_DIR_ENV = "FARHELM_HELM_PAYLOAD_DIR"


def check_payload_dir(directory: Path) -> Path:
    """Refuse a `--payload-dir` the app could not use as given; return it unchanged otherwise.

    It must be absolute, as the run directory must, because a relative path would be resolved against whatever working
    directory the app ends up with, which nothing here promises. It must be an existing directory because the helm
    looks into it only when it installs on or updates a host, long after `start` has returned, so a typo would surface
    as a failed setup or Update deep inside the run instead of as a usage error. Its contents are not checked; the helm
    trusts them as given (`--payload-dir` in crates/farhelm-helm/src/lib.rs)."""
    if not directory.is_absolute():
        raise UsageError(f"--payload-dir must be an absolute path: {directory}")
    if not directory.is_dir():
        raise UsageError(f"--payload-dir is not an existing directory: {directory}")
    return directory


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
    start_dialogs: bool = False,
    payload_dir: Path | None = None,
    ensure_hosts: Path | None = None,
    staged: bool = False,
) -> list[str]:
    """The `--env` arguments `gpui-agent launch` passes to the app only: its state, its CLI and the helm file, plus the
    first window's size, a forced appearance, the scripted provisioning backend's directory, the payload directory and
    a staged run's ensure-hosts file when the run asks for them (unset otherwise, so the app starts at its own default
    size in the system's appearance, sets up hosts for real, has no payloads to push and registers no hosts).
    `staged` forces the scripted backend on whatever `scripted_hosts` says: a staged remote host is this machine, so a
    real setup, Update or uninstall of it would act on its own Farhelm installation. The start dialogs are off unless
    `start_dialogs` asks for them, because every run's helm is fresh and would otherwise open the Welcome card over the
    app. `scripts/gui-lifecycle.py` relaunches through `launch_env_args` with the defaults, so its relaunched app keeps
    them off too, as it must: the first launch marked nothing seen."""
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
    if not start_dialogs:
        pairs["FARHELM_DESKTOP_AGENT_START_DIALOGS"] = "off"
    if staged:
        scripted_hosts = scripted_hosts_dir(run)
    if scripted_hosts is not None:
        pairs[SCRIPTED_HOSTS_ENV] = str(scripted_hosts)
    if payload_dir is not None:
        pairs[PAYLOAD_DIR_ENV] = str(payload_dir)
    if ensure_hosts is not None:
        pairs[ENSURE_HOSTS_ENV] = str(ensure_hosts)
    args: list[str] = []
    for key, value in pairs.items():
        args += ["--env", f"{key}={value}"]
    return args


def launch_env_args(run: Path, record: dict, **options) -> list[str]:
    """`app_env_args` for a launch of the app the run `record` describes, `start`'s and every relaunch's: whether the
    run is staged comes from the record (`staged_record`), so no launch of a staged run can leave the scripted backend
    off. `options` are `app_env_args`'s other keyword arguments."""
    return app_env_args(run, record["binaries"], staged=staged_record(record), **options)


# What `start --fleet` can seed. `basic` is the prompt-and-echo fake agent `--sessions` seeds, the one to type into; the
# others run the fake agent's replay script, which prints a one-line transcript and then holds the screen the
# supervisor's classifier reads as that status (`ReplayThen` in crates/farhelm-fixtures/src/fake_agent.rs). Only those
# four have a status to wait for: `basic` is read as running while it prints and idle a few samples after, so it never
# settles on one the run could promise. `spawner` runs the fake agent's spawn script, the one way a scene gets an
# agent's approval card: a `spawn <folder>` line typed into it runs the real `farhelm spawn`, which waits on a card in
# the window. Like `basic`, it has no status to wait for.
FLEET_KINDS = ("basic", "running", "waiting", "idle", "exited", "spawner")

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
    fake agent: the basic script for `basic`, the spawn script for `spawner`, and the replay script holding `kind`'s
    screen otherwise (`transcript` is then the file it prints first).

    Only `waiting` declares an agent type (Claude): the generic status reading never reads a session as waiting, and
    declaring one requires the `{farhelm_args}` placeholder (SPEC.md, "Creation"), where the supervisor adds Claude's
    hook flags, which the fake agent ignores. Every other kind stays an undeclared command, like `basic`.
    """
    command = f"{shlex.quote(fixtures)} fake-agent --script basic"
    agent = None
    if kind == "spawner":
        # By bare name, which the session's launch shim resolves: it puts the directory of the `farhelm` binary the
        # supervisor runs, where `farhelm-fixtures` is built too, first on the session's PATH (the spawn script finds
        # `farhelm` the same way). A spawner's command is what its approval card shows, and a scene of that card
        # should not show the build machine's path.
        command = "farhelm-fixtures fake-agent --script spawn"
    elif kind != "basic":
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
    return claim(pid, proc_start(pid), str(state))


def claim(pid: int | None, start: str | None, state: str) -> tuple[int, str] | None:
    """A supervisor as `(pid, start)` while that pid is still running with that start time and its command line names
    the state directory `state`; None otherwise, so a pid reused by an unrelated process is never claimed."""
    if not pid or not start or not running(pid, start):
        return None
    argv = command_line(pid)
    return (pid, start) if argv and state in argv else None


def wait_for(path: Path, seconds: float, what: str, alive=None) -> None:
    """Wait until `path` exists, or fail naming `what`: after `seconds`, or at once when `alive()` (the process that
    would create it is still running) turns false."""
    deadline = time.monotonic() + seconds
    while not path.exists():
        if alive is not None and not alive():
            raise RuntimeError(f"{what} exited before {path} appeared")
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
    so its sessions start without a transient scope (the supervisor's ordinary fallback). Mutter keeps this bus also
    in a user-session run, where only the app gets the caller's: Mutter registers its service names
    (`org.gnome.Mutter.DisplayConfig` and others) on the bus it runs on, and on the caller's it would compete with a
    desktop session's own compositor for them.

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
    JSON reply, or `None` for a reply without a body (the 204 a preference write answers with). Waits for `helm.json`
    first, since the app writes it only once its helm is up."""
    wait_for(run / "helm.json", 30, "the app's helm file")
    helm = json.loads((run / "helm.json").read_text())
    request = urllib.request.Request(
        f"{helm['api_base']}{path}",
        data=None if body is None else json.dumps(body).encode(),
        headers={"Authorization": f"Bearer {helm['credential']}", "Content-Type": "application/json"},
        method=method,
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        reply = response.read()
    return json.loads(reply) if reply else None


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
        if kind in REPLAY_THEN:
            transcript_file = work / f"fixture-{index}.txt"
            transcript_file.write_text(transcript(index, kind))
        body = seed_body(index, SESSION_CWD, fixtures, kind, transcript_file)
        ids.append(helm_request(run, "POST", "/api/sessions", body)["id"])
    return ids


# The launch templates `start --templates` seeds, by name, each with the fields the helm stores (`PUT
# /api/templates/<name>` takes the fields alone), and the ⚡ mark and number keys over them. Screenshot scenes of
# templates and quick launch start from these instead of building templates through the Templates dialog, whose steps
# would break every scene whenever the editor changes. Each one is there for a scene:
#
# - `codex-deep` and `quick-claude` are the reference stack's agent templates (the old app's capture and the GUI's
#   template goldens staged them), on keys 1 and 8 as there.
# - `in-tmp` sets only a folder, so it cannot launch on its own: ⚡ on it shows New with the needs-more-choices banner,
#   and key 2 puts it in a quick-launch sequence as a chip.
# - `fixture-run` is a command launch of the basic fake agent that launches on its own: a sequence of key 3 alone
#   starts a session without showing New, which opens the naming dialog over it. Scenes reach the number keys
#   through a sequence because a sequence reads them bare, while the chord itself differs by platform (Ctrl+Shift on
#   Linux, Command on a Mac) and its action cannot be dispatched by name.
# - `elsewhere` names an install no host has, so New refuses it, for the refusal line above the search box.
#
# Nothing here may name the machine that runs the scenes: `fixture-run` names the fake agent by its bare binary name,
# which the session's launch shim resolves (as the `spawner` kind does), never a build path, and the folder is `/tmp`,
# as for seeded sessions (`SESSION_CWD`).
SEEDED_TEMPLATES = {
    "codex-deep": {"kind": "agent", "agent": "codex", "model": "gpt-6.1-sol", "effort": "high"},
    "quick-claude": {"kind": "agent", "agent": "claude", "permissions": "yolo"},
    "in-tmp": {"destination": {"folder": str(SESSION_CWD)}},
    "fixture-run": {"kind": "command", "command": "farhelm-fixtures fake-agent --script basic", "yolo": False},
    "elsewhere": {"host": "no-such-install"},
}
SEEDED_BINDINGS = {
    "quick_launch_template": "in-tmp",
    "template_shortcuts": {"1": "codex-deep", "2": "in-tmp", "3": "fixture-run", "8": "quick-claude"},
}


def seed_templates(run: Path) -> list[str]:
    """Store `SEEDED_TEMPLATES` in this run's helm, in that order, then the ⚡ mark and the number keys over them in one
    sparse preferences write (`SEEDED_BINDINGS`, which touches no other preference); return the names. The app shows
    them from the helm's push, so `start` does not wait for the window to have them; scenes wait for what they
    show."""
    for name, fields in SEEDED_TEMPLATES.items():
        helm_request(run, "PUT", f"/api/templates/{name}", fields)
    helm_request(run, "PUT", "/api/preferences", SEEDED_BINDINGS)
    return list(SEEDED_TEMPLATES)


def allow_copied_launches(run: Path) -> None:
    """Turn on the local host's "Start YOLO sessions without asking" in this run's own helm, for a `spawner` session.

    The spawn script's `farhelm spawn --inherit-agent` copies its own launch, a command Farhelm did not compose, and the
    helm refuses that outright on a host that asks first, before any card (SPEC.md, "Agent-spawned sessions"). This
    waives no approval: the spawn still waits on its card, which is what a scene of one shows. The run's helm is
    fresh and private to it, so nothing outside the run sees the setting.
    """
    local = next(row for row in helm_request(run, "GET", "/api/hosts")["hosts"] if row["kind"] == "local")
    helm_request(run, "POST", f"/api/hosts/{local['id']}/yolo-without-asking", {"yolo_without_asking": True})


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


def run_env(
    run: Path, record: dict, base: dict[str, str] | None = None, platform: str = sys.platform
) -> dict[str, str]:
    """The environment of everything the run starts after its compositor: the bridge's command line, the app it
    launches (and through the app its supervisor and every session), and a staged run's remote supervisors.

    That is `bridge_env` over the caller's environment (`base`, `os.environ` by default), and in a staged run (the
    record holds `staging`) over `staged_env` of it, so the private HOME and PATH reach all of them. The runtime
    directory, display and bus are the run's own, or the caller's in a user-session run (`session_vars`)."""
    base = dict(os.environ) if base is None else base
    if record.get("staging"):
        base = staged_env(base, record["staging"])
    return bridge_env(base, run, *session_vars(record), platform=platform)


def cli(run: Path, record: dict, args: list[str], capture: bool = False) -> subprocess.CompletedProcess:
    """Run the bridge's command line against this run's app."""
    env = run_env(run, record)
    return subprocess.run([record["binaries"]["cli"], *args], env=env, capture_output=capture, text=True)


def start(run: Path, args: list[str]) -> int:
    import argparse

    parser = argparse.ArgumentParser(prog="gui-agent.py RUN_DIR start")
    seeding = parser.add_mutually_exclusive_group()
    seeding.add_argument("--sessions", type=int, help="basic fake-agent sessions to seed (default 2)")
    seeding.add_argument(
        "--fleet",
        type=parse_fleet,
        help=f"seed one session per kind, in order, and wait for each replayed one's status ({', '.join(FLEET_KINDS)})",
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
    parser.add_argument(
        "--templates",
        action="store_true",
        help="seed the fixed launch templates, the quick-launch mark and number keys screenshot scenes use "
        "(see docs/gui-agent.md)",
    )
    parser.add_argument(
        "--start-dialogs",
        action="store_true",
        help="let the window's start dialogs (the Welcome card, then announcements) open as in a real launch; off by "
        "default (agent-build test hook)",
    )
    parser.add_argument(
        "--payload-dir",
        type=Path,
        metavar="DIR",
        help="an absolute directory of staged farhelm payloads the app's helm installs and updates remote hosts from",
    )
    parser.add_argument(
        "--remote-host",
        action="append",
        default=[],
        metavar="DEST",
        help="stage a remote host: this machine over ssh at DEST, served by a supervisor of the run's own (repeatable; "
        "makes the run a staged one, see docs/gui-agent.md)",
    )
    parser.add_argument(
        "--user-session",
        action="store_true",
        help="Linux only: give the app the caller's own XDG_RUNTIME_DIR and session bus, so its helm's Podman check "
        "reaches the user's systemd manager and New container shows. The run then shares that manager and the "
        "account's Podman storage; `stop` removes the run's containers and units by recorded name (not in a staged "
        "run; see docs/gui-agent.md, \"User-session runs\")",
    )
    options = parser.parse_args(args)
    if options.window_size:
        parse_size(options.window_size)
    if options.payload_dir is not None:
        check_payload_dir(options.payload_dir)
    remote_hosts = parse_remote_hosts(options.remote_host)
    # A staged run always has the private HOME and PATH and the scripted provisioning backend; nothing turns either off.
    staged = staged_start(options)
    # Before the ssh checks, so a refused combination fails without dialing anything.
    user_session = check_user_session(dict(os.environ), sys.platform, staged) if options.user_session else None
    for destination in remote_hosts:
        check_ssh(destination)
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
    if user_session:
        # Recorded, not read again later, so every launch of the run (a relaunch included) and `stop` use the
        # session `start` was given, whatever shell runs them.
        record["user_session"] = user_session

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
    if staged:
        shadows = shadowing_agents(Path(binaries["farhelm"]).parent)
        if shadows:
            raise RuntimeError(
                f"{Path(binaries['farhelm']).parent} holds {', '.join(shadows)}, which a staged session would run "
                "instead of the run's wrappers; remove them. Run `stop` to clean up."
            )
        record["staging"] = prepare_staging(run, binaries["fixtures"])
        save()
    if not mac:
        display = display_name(uuid.uuid4().hex[:8])
        record.update({"runtime": str(run / "runtime"), "display": display})
        size = options.size or DEFAULT_MONITOR
        mutter = start_mutter(run, run / "runtime", display, mutter_command(display, size), dict(os.environ))
        record.update({"mutter": {"pid": mutter["pid"], "start": mutter["start"]}, "dbus": mutter["dbus"]})
    save()
    ensure_hosts = None
    if remote_hosts:

        def record_remotes(remotes: list[dict]) -> None:
            record["remotes"] = remotes
            save()

        start_remote_supervisors(run, remote_hosts, binaries["farhelm"], run_env(run, record), record_remotes)
        ensure_hosts = run / "ensure-hosts.json5"
        ensure_hosts.write_text(json.dumps(ensure_hosts_body(run, remote_hosts, binaries["farhelm"]), indent=2) + "\n")
    if options.scripted_hosts or staged:
        prepare_scripted_hosts(run)
    launched = cli(
        run,
        record,
        [
            "launch",
            "--timeout",
            str(LAUNCH_TIMEOUT_SECONDS),
            *launch_env_args(
                run,
                record,
                window_size=options.window_size,
                appearance=options.appearance,
                scripted_hosts=scripted_hosts_dir(run) if options.scripted_hosts else None,
                start_dialogs=options.start_dialogs,
                payload_dir=options.payload_dir,
                ensure_hosts=ensure_hosts,
            ),
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
        if "spawner" in kinds:
            allow_copied_launches(run)
        ids = seed_sessions(run, kinds, binaries["fixtures"])
        print(f"seeded {len(ids)} fake-agent sessions: {', '.join(ids)}")
        targets = {id: kind for id, kind in zip(ids, kinds) if kind in REPLAY_THEN}
        if targets:
            wait_for_statuses(lambda: listed_states(run), targets)
            print(f"every seeded session reached its status: {', '.join(kinds)}")
    if options.templates:
        print(f"seeded launch templates: {', '.join(seed_templates(run))}")
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
    """Take down everything `start` started: the app, its supervisor and terminal hosts, a staged run's remote
    supervisors and their terminal hosts, then Mutter and its bus (on Linux), and last the lock holder. The shared lock
    is released only when every earlier step succeeded: if part of the run may still be running, the lock is kept (the
    message says so) and a second `stop` retries everything.

    Each step runs even if an earlier one failed, and each acts only on processes it can tie to this run: the app
    through the pid and start time recorded after launch, the supervisor through the app's record of it plus the state
    directory on its command line, a remote supervisor through its recorded pid and start time plus its state directory
    on its command line, Mutter through the pid and start time recorded when it started. A pid that has since been
    reused by another process is never signalled. A user-session run's containers and units on the caller's account
    are removed after the terminal hosts, by the names recorded from its helm before the app quit
    (`record_user_session_objects`, `remove_user_session_objects`).
    """
    problems = []
    app = record.get("app")
    user_session = record.get("user_session")
    if user_session and app:
        # Asked while the helm still answers: once the app is down only the record says what the run made. An app that
        # is already gone is not asked; that is worth a warning only when no earlier `stop` recorded anything.
        warning = None
        if running(app["pid"], app["start"]):
            warning = record_user_session_objects(run, record)
        elif "sessions" not in user_session:
            warning = "the app was already gone, so its helm could not say what it made; nothing is recorded to remove"
        if warning:
            print(f"gui-agent: warning: {warning}", file=sys.stderr)
    # The app first, the ordinary way: SIGTERM, which the app turns into a quit (it then waits for its helm to stop),
    # and SIGKILL if it is still there after the grace. Not the bridge's `close`, which signals the recorded pid and its
    # process group without checking that the pid is still the app.
    if app and not stop_process(app["pid"], app["start"], 15):
        problems.append(f"the app (pid {app['pid']}) did not stop")
    for name in SESSION_FILES:
        (run / "agent" / SESSION / name).unlink(missing_ok=True)
    state = Path(record["state"])
    supervisor = supervisor_to_stop(state)
    if supervisor and not stop_process(supervisor[0], supervisor[1], 20):
        problems.append(f"the supervisor (pid {supervisor[0]}) did not stop")
    farhelm = record.get("binaries", {}).get("farhelm")
    if not stop_terminal_hosts(farhelm, state):
        problems.append("stop-terminal-hosts could not confirm every terminal host stopped")
    for remote in record.get("remotes", []):
        found = remote_supervisor_to_stop(remote)
        if found and not stop_process(found[0], found[1], 20):
            problems.append(f"the supervisor for {remote['destination']} (pid {found[0]}) did not stop")
        elif not stop_terminal_hosts(farhelm, Path(remote["state"])):
            problems.append(f"stop-terminal-hosts could not confirm every terminal host of {remote['state']} stopped")
    if user_session:
        problems += remove_user_session_objects(user_session, dict(os.environ))
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
