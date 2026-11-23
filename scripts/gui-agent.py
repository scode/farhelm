#!/usr/bin/env python3
"""Drive the GPUI desktop app as an agent: launch it against a real helm and supervisor, then look and click.

NOTE: Debug builds only, and never against the live install. On Linux a run is headless, on a private Mutter; on a Mac
it opens a real window in the logged-in GUI session, which this script must run inside (over SSH is fine). Everything
a run starts lives under the run directory you name (which must not exist yet), and `stop` takes all of it down again.

    scripts/gui-agent.py RUN_DIR start [--sessions N | --fleet KIND,... | --scenario FILE] [--size WxH (Linux)]
                                       [--no-build] [--lock FILE] [--window-size WxH] [--appearance light|dark]
                                       [--templates] [--scripted-hosts] [--start-dialogs] [--payload-dir DIR]
                                       [--remote-host DEST ... | --user-session (Linux)]
    scripts/gui-agent.py RUN_DIR snapshot -i         # any gpui-agent command: click, type, press, wait, screenshot ...
    scripts/gui-agent.py RUN_DIR terminal-text [--wait TEXT [--timeout SECONDS]]
    scripts/gui-agent.py RUN_DIR capture OUT.png [--keep-focus] [--park id:TARGET]
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
each as an SSH host reached at DEST, which must be this machine under a spelling of its own. `--scenario FILE` stages
the fleet a docs-media `scenario.json5` describes, a staged run whose remote hosts are the scenario's: its hosts with
their aliases, its sessions with their titles, agent types, YOLO launches, statuses and seen state and with the ages
and folders the app pins on their rows, the window at its viewport and the open session opened (see "Scenario runs"
below). `--user-session` (Linux)
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
provisioning backend on, since a staged remote host is this machine. Its remote hosts are reached through a private sshd
of the run's own ("The private sshd" below), never the machine's own ssh setup. `staged_start` says which runs are
staged.

Scenario runs (docs/gui-agent.md, "Scenario runs" and "Scenario captures"). `load_scenario` checks the whole file before
anything starts; `stage_scenario` stages it through the helm's API once the app is up; `capture` takes its picture.

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
import urllib.parse
import urllib.request
import uuid
from pathlib import Path
from typing import Callable

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
STAGED_AGENTS = ("codex", "claude", "muse", "cursor-agent", "opencode", "goose", "pi", "omp", "grok", "agy")

# The programs the helm launches for its shell choices (also in `program`), which get no wrapper: a shell is no
# vendor's program, so a staged shell session runs the machine's own shell under the private HOME, which holds nothing
# of the caller's. A `bash` wrapper would also shadow the bash that a session's own commands run by name.
STAGED_REAL_SHELLS = ("bash", "zsh")

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
# `SSH_AUTH_SOCK` is the caller's ssh agent, which holds the caller's real keys: a staged run reaches its hosts through
# its private sshd with a key of its own ("The private sshd" below), so nothing in it has a use for the agent.
STAGED_DROPPED_ENV = (
    "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME", "ZDOTDIR", "BASH_ENV", "ENV",
    "SSH_AUTH_SOCK",
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
    scripted provisioning backend: one with remote hosts, or one staging a scenario (`--scenario`), whose sessions are
    launched as `claude` and `codex` by bare name even when it has only the local host. A plain `start` stays unstaged
    until the private environment has been checked on a Mac."""
    return bool(options.remote_host or getattr(options, "scenario", None))


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


def check_ssh(destination: str, ssh: str) -> None:
    """Refuse a destination the run cannot reach through `ssh` (the run's shim, `start_private_sshd`), so a broken
    connection fails `start` naming the destination instead of leaving the host unreachable in the window. The options
    are the old browser staging's probe, at least as strict as the helm's own dial (which sets `BatchMode` too)."""
    result = subprocess.run(
        [ssh, "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes", "-o", "ConnectTimeout=10", destination,
         "true"],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise UsageError(
            f"ssh to {destination!r} through the run's private sshd failed: {result.stderr.strip()}; its log is "
            f"logs/sshd.log in the run directory. Run `stop` to clean up."
        )


# ---------------------------------------------------------------------------------------------------------------------
# Staged runs: the private sshd
#
# A staged remote host is this machine reached over ssh. The helm dials each one with `ssh` from its PATH, so a staged
# run puts a shim of that name first on the private PATH (`staging/bin/ssh`). The shim runs the real ssh with a client
# configuration of the run's own (`ssh -F`, so neither `~/.ssh/config` nor the system's is read) that sends every staged
# destination to a private sshd: started by the run as the caller's own user, listening on 127.0.0.1 at a port the
# kernel picked, with a host key and an authorized key generated into `<run>/sshd/`, and stopped by `stop`. The
# configuration refuses every other destination the helm dials. It is not a sandbox: options given on ssh's command
# line win over the file, and the helm passes none that change the host. No account's `~/.ssh` is read or written, the
# caller's ssh agent is not used, and the machine needs no passwordless ssh to itself, which the shared test Mac does not
# have. Linux runs use it too, so a staged run works the same on any machine, whatever keys it holds.
#
# The remote end is still the caller's own account: sshd runs the helm's command (`farhelm internal stdio`, which joins
# the run's remote supervisor) through the account's login shell with the account's real HOME, as a real `ssh
# localhost` did before. That command starts nothing a session runs; the sessions are the remote supervisors', which
# `start` launched with the staged HOME and PATH.

# The name the client knows the private sshd's host key by, whatever spelling of this machine it dials (`HostKeyAlias`),
# so one line of the run's known_hosts file covers every staged destination.
SSHD_HOST_KEY_ALIAS = "farhelm-staged-sshd"

# How long the private sshd gets to listen.
SSHD_READY_SECONDS = 15


def sshd_dir(run: Path) -> Path:
    """The private sshd's directory: its configuration, both key pairs, the authorized key and known_hosts."""
    return run / "sshd"


def destination_host(destination: str) -> str:
    """The host part of an ssh destination (`me@localhost` -> `localhost`), which is what an ssh config `Host` matches."""
    return destination.rpartition("@")[2]


def sshd_config_text(directory: Path, port: int, user: str) -> str:
    """The private sshd's configuration: loopback only, public-key login for `user` alone with the run's own key, and
    nothing else a session could use (no forwarding, no terminal, no environment from the client).

    `StrictModes no` because the files live under the run directory, below a world-writable `/tmp`, which sshd's
    ownership checks would refuse; the directory itself is private (0700). `UsePAM no` because a non-root sshd cannot
    use PAM; public-key login does not need it."""
    return "".join(f"{line}\n" for line in (
        "# A staged run's private sshd (scripts/gui-agent.py, \"The private sshd\").",
        f"Port {port}",
        "ListenAddress 127.0.0.1",
        "AddressFamily inet",
        f"HostKey {directory / 'host_key'}",
        f"AuthorizedKeysFile {directory / 'authorized_keys'}",
        f"PidFile {directory / 'sshd.pid'}",
        f"AllowUsers {user}",
        "PubkeyAuthentication yes", "PasswordAuthentication no", "KbdInteractiveAuthentication no", "UsePAM no",
        "StrictModes no", "PermitRootLogin no", "AllowAgentForwarding no", "AllowTcpForwarding no", "X11Forwarding no",
        "PermitTunnel no", "PermitTTY no", "PermitUserEnvironment no", "PermitUserRC no", "PrintMotd no",
    ))


def ssh_client_config_text(directory: Path, port: int, destinations: list[str]) -> str:
    """The client configuration the run's ssh shim hands ssh with `-F`: each staged destination's host sent to the
    private sshd with the run's key and known_hosts and nothing of the caller's (no agent, no default identities, no
    global known hosts, no proxy), and every other host refused through a proxy command that fails.

    The helm's own `-o` options (its connection sharing, `BatchMode`, its overrides) come on the command line and win
    over this file, as they win over a user's config. The refusal block is last because ssh takes the first value it
    finds for each keyword, which is also why the staged block sets `ProxyCommand none`."""
    hosts = " ".join(dict.fromkeys(destination_host(destination) for destination in destinations))
    refusal = "echo 'a staged run reaches only its staged hosts' >&2; exit 1"
    return "".join(f"{line}\n" for line in (
        "# A staged run's ssh client configuration (scripts/gui-agent.py, \"The private sshd\").",
        f"Host {hosts}",
        "  HostName 127.0.0.1",
        f"  Port {port}",
        f"  HostKeyAlias {SSHD_HOST_KEY_ALIAS}",
        f"  UserKnownHostsFile {directory / 'known_hosts'}",
        "  GlobalKnownHostsFile /dev/null", "  StrictHostKeyChecking yes", f"  IdentityFile {directory / 'client_key'}",
        "  IdentitiesOnly yes", "  IdentityAgent none", "  ProxyCommand none", "  ProxyJump none",
        "Host *",
        f"  ProxyCommand /bin/sh -c {shlex.quote(refusal)}",
    ))


def ssh_shim_script(ssh: str, config: Path) -> str:
    """The `ssh` a staged run's PATH finds first: the real one (`ssh`, an absolute path found outside the run) with the
    run's client configuration, every argument passed on."""
    return (
        "#!/bin/sh\n"
        "# A staged run's ssh (scripts/gui-agent.py, \"The private sshd\"): the helm's dials reach only the run's sshd.\n"
        f'exec {shlex.quote(ssh)} -F {shlex.quote(str(config))} "$@"\n'
    )


def start_private_sshd(run: Path, destinations: list[str], bin_dir: Path, on_started: Callable[[dict], None]) -> dict:
    """Start the run's private sshd for `destinations` and write the ssh shim into `bin_dir` (the staged PATH's first
    directory); return the run's record of the sshd (`pid`, `start`, `config`, `port`, `shim`).

    `on_started` receives the record as soon as the sshd process exists, before the wait, so `stop` can end an sshd that
    `start` left behind by failing or being killed later. The programs are the machine's own `sshd`, `ssh` and
    `ssh-keygen`; a missing one fails naming it. The port is one the kernel picked a moment earlier (bound to port 0 and
    released); an sshd that exits or does not listen within SSHD_READY_SECONDS fails the start naming its log
    (`logs/sshd.log`), the port taken in between included, which is rare enough to leave to a second `start`."""
    import shutil
    import socket

    found = {}
    for name, fallback in (("sshd", "/usr/sbin/sshd"), ("ssh", None), ("ssh-keygen", None)):
        path = shutil.which(name) or fallback
        if not path or not Path(path).is_file():
            raise UsageError(f"a staged run with remote hosts needs `{name}` (OpenSSH) on this machine")
        # sshd re-executes itself and refuses to run from a relative path.
        found[name] = str(Path(path).resolve())
    directory = sshd_dir(run)
    directory.mkdir(mode=0o700)
    for key in ("host_key", "client_key"):
        made = subprocess.run(
            [found["ssh-keygen"], "-q", "-t", "ed25519", "-N", "", "-C", SSHD_HOST_KEY_ALIAS, "-f",
             str(directory / key)],
            stdin=subprocess.DEVNULL, capture_output=True, text=True,
        )
        if made.returncode != 0:
            raise RuntimeError(f"ssh-keygen could not make the run's {key}: {made.stderr.strip()}")
    # `restrict` turns off everything a key may otherwise grant (forwarding, a terminal, `~/.ssh/rc`); the helm's
    # command needs none of it.
    client_public = (directory / "client_key.pub").read_text().strip()
    (directory / "authorized_keys").write_text(f"restrict {client_public}\n")
    host_public = " ".join((directory / "host_key.pub").read_text().split()[:2])
    (directory / "known_hosts").write_text(f"{SSHD_HOST_KEY_ALIAS} {host_public}\n")
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    config, client, log = directory / "sshd_config", directory / "ssh_config", run / "logs" / "sshd.log"
    config.write_text(sshd_config_text(directory, port, pwd.getpwuid(os.getuid()).pw_name))
    with log.open("wb") as output:
        child = subprocess.Popen(
            [found["sshd"], "-D", "-e", "-f", str(config)],
            stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT, start_new_session=True,
        )
    record = {"pid": child.pid, "start": proc_start(child.pid), "config": str(config), "port": port}
    on_started(record)
    deadline = time.monotonic() + SSHD_READY_SECONDS
    while True:
        try:
            socket.create_connection(("127.0.0.1", port), timeout=1).close()
            break
        except OSError:
            pass
        if child.poll() is not None:
            raise RuntimeError(f"the run's private sshd exited with status {child.returncode} (log {log})")
        if time.monotonic() > deadline:
            raise RuntimeError(f"the run's private sshd did not listen within {SSHD_READY_SECONDS} s (log {log})")
        time.sleep(0.1)  # sleep-ok: polling for a listener another process opens; there is no event to wait on
    client.write_text(ssh_client_config_text(directory, port, destinations))
    # ssh refuses a configuration file others could write; 0600 like the keys.
    client.chmod(0o600)
    shim = bin_dir / "ssh"
    shim.write_text(ssh_shim_script(found["ssh"], client))
    shim.chmod(0o700)
    record["shim"] = str(shim)
    on_started(record)
    return record


def stop_private_sshd(record: dict) -> list[str]:
    """Stop the run's private sshd and every process under it; return the problems.

    The sshd is signalled only while its recorded pid and start time match and its command line names the run's sshd
    configuration (sshd rewrites its process title, which keeps the path in it). The processes under it are each
    connection's and the helm's `farhelm internal stdio` commands they ran. The run's ssh clients, the helm's detached
    shared connections (`ControlPersist`) among them, end by themselves once their connection is gone: the shim reaches
    nothing but this sshd."""
    pid, start = record.get("pid"), record.get("start")
    if not (pid and start and running(pid, start) and record["config"] in " ".join(command_line(pid) or [])):
        return []
    return [f"a process of the run's private sshd (pid {left}) did not stop" for left in stop_tree(pid, start, 5)]


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
    session_pins: Path | None = None,
    staged: bool = False,
) -> list[str]:
    """The `--env` arguments `gpui-agent launch` passes to the app only: its state, its CLI and the helm file, plus the
    first window's size, a forced appearance, the scripted provisioning backend's directory, the payload directory, a
    staged run's ensure-hosts file and a scenario run's pinned session values when the run asks for them (unset
    otherwise, so the app starts at its own default size in the system's appearance, sets up hosts for real, has no
    payloads to push, registers no hosts and shows every session as the helm pushes it).
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
    if session_pins is not None:
        pairs[SESSION_PINS_ENV] = str(session_pins)
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


# ---------------------------------------------------------------------------------------------------------------------
# Scenario runs: a docs-media scenario's fleet, staged through the helm's API
#
# `start --scenario FILE` stages the fleet a `scenario.json5` describes (docs/readme-hero/SPEC.md says what is real in
# it and what is staged). The scenario is read and checked first (`load_scenario`), before anything is created or any
# destination dialed, so a typo fails as a usage error instead of as a picture that looks plausible and is wrong. Its
# remote hosts become `--remote-host` destinations, so the run is staged (`staged_start`). Once the app is up,
# `stage_scenario` names the hosts, creates the sessions on them, waits for every status, sets the seen state, and
# checks each row's permission shield in the window. Ported from the browser suite's `e2e/readme-hero/scenario.ts` and
# `stage.ts` (removed in #3401; `git show 6ef88c9a^:e2e/readme-hero/stage.ts`). Each session's age and folder, and each
# host's launch history, checkout trash and file checks ("the staged host values" below), which the old staging
# rewrote in the helm's replies, reach the app in a file (`session_pins_body`). The scenario's `~` folders are created
# in the run's private home (`create_scenario_dirs`), so the launcher's folder check finds them for real. The window
# uses the viewport's logical size. On Linux the caller can select its output scale separately.

# The wrappers a scenario session may name, each with the launch harness (`LaunchHarness`'s wire name in
# crates/farhelm-proto/src/launch.rs) its agent launch asks for. The helm runs each harness's program by its bare name
# (`program` in crates/farhelm-helm/src/launches.rs), which is the wrapper's name, so the run's wrapper of that name
# (STAGED_AGENTS) answers the launch.
SCENARIO_HARNESSES = {"claude": "claude", "codex": "codex"}

# The session list's preferences the old staging set before the page loaded: the activity order the scenario's row order
# is written for, and full rows. One sparse write, which touches no other preference.
SCENARIO_PREFERENCES = {"list_sort": "activity", "compact": False}

# The variable naming the pins file to an agent build of the app (`SESSION_PINS_ENV` in
# crates/gui/farhelm-gui/src/agent_startup.rs, which reads it), and the file's name in the run directory.
SESSION_PINS_ENV = "FARHELM_DESKTOP_AGENT_SESSION_PINS"
SESSION_PINS_FILE = "session-pins.json"


def _sibling(name: str):
    """The script `name` beside this one, loaded as a module: the scenario loader (`scenario_json5.py`), so that `start`
    reads a scenario exactly as the publish script and every capture flow do, and the screenshot batch, which owns the
    Linux window frame's numbers. Loaded by path, so it works also under `python3 -I`."""
    import importlib.util

    module_name = name.removesuffix(".py").replace("-", "_")
    spec = importlib.util.spec_from_file_location(module_name, Path(__file__).with_name(name))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_scenario(path: Path, user: str | None) -> dict:
    """Read and check the scenario file at `path` (`plan_scenario`); a UsageError for a file that cannot be read or
    parsed, or that the checks refuse. `user` is the account name a `$USER` in a host's ssh destination stands for."""
    loader = _sibling("scenario_json5.py")
    try:
        document = loader.load(path)
    except (OSError, loader.ScenarioSyntaxError) as error:
        raise UsageError(f"--scenario: {error}") from None
    return plan_scenario(document, Path(path).resolve().parent, user, str(path))


def plan_scenario(document, directory: Path, user: str | None, source: str) -> dict:
    """The scenario `document`, checked and completed in place for staging, with `source` added: each remote host's
    `ssh` with `$USER` expanded, and each session with `yolo` and `open` (false), `then` (the replay shape its `status`
    implies) and `exit_code` (0) filled in where the file leaves them out, and its `transcript` resolved against
    `directory` into an absolute path (or None). A session's `cwd` and `age` are checked and kept as written, for the
    pins file (`session_pins_body`). `source` names the file in errors.

    The checks are the old browser loader's (`e2e/readme-hero/scenario.ts`) for the fields staged here, strict on
    purpose: the scenario is edited by hand, and a typo that silently dropped a session or misread a status would make
    a picture that looks plausible and is wrong. On top of them a transcript must name an existing file, since the
    replay would otherwise fail inside a session long after `start` returned. Two remote hosts with one destination
    are refused by `parse_remote_hosts`, which every staged run's destinations go through. `$USER` is the one
    substitution: a self-ssh spelling naming the account is the cheapest second destination a machine has, and the
    account name must not be written into a public file."""

    def fail(message: str):
        raise UsageError(f"{source}: {message}")

    if not isinstance(document, dict):
        fail("a scenario is an object")
    viewport = document.get("viewport")
    if not (isinstance(viewport, dict) and all(
        type(viewport.get(name)) is int and viewport[name] > 0 for name in ("width", "height")
    )):
        fail("viewport needs positive integer width and height")

    hosts = document.get("hosts")
    if not isinstance(hosts, list) or not hosts:
        fail("at least one host is required")
    keys, aliases = set(), set()
    for host in hosts:
        if not isinstance(host, dict) or not isinstance(host.get("key"), str) or not host["key"]:
            fail(f"host {host!r} needs a key")
        key = host["key"]
        if not isinstance(host.get("alias"), str) or not host["alias"]:
            fail(f"host {key} needs an alias")
        if key in keys:
            fail(f"duplicate host key {key}")
        keys.add(key)
        # The app finds a host's staged values by its alias (`staged_host_body`), so two hosts may not share one.
        if host["alias"] in aliases:
            fail(f"duplicate host alias {host['alias']}")
        aliases.add(host["alias"])
        if host.get("kind") == "remote":
            if not isinstance(host.get("ssh"), str) or not host["ssh"]:
                fail(f"host {key}: a remote host needs an ssh destination")
            if "$USER" in host["ssh"]:
                if not user:
                    fail(f"host {key} uses $USER but USER is not set")
                host["ssh"] = host["ssh"].replace("$USER", user)
        elif host.get("kind") != "local":
            fail(f"host {key}: kind must be local or remote")
    if sum(host["kind"] == "local" for host in hosts) != 1:
        fail("exactly one host must be kind: local")

    sessions = document.get("sessions")
    if not isinstance(sessions, list) or not sessions:
        fail("at least one session is required")
    titles = set()
    previous_age = 0
    for session in sessions:
        if not isinstance(session, dict) or not isinstance(session.get("title"), str) or not session["title"]:
            fail("every session needs a title")
        name = f"session {session['title']}"
        # Titles are the sessions' identity in staging and in the captures, across hosts too.
        if session["title"] in titles:
            fail(f"duplicate session title {session['title']}")
        titles.add(session["title"])
        if session.get("host") not in keys:
            fail(f"{name}: unknown host {session.get('host')!r}")
        if session.get("wrapper") not in SCENARIO_HARNESSES:
            fail(f"{name}: wrapper must be one of {', '.join(SCENARIO_HARNESSES)}")
        # The folder and age the app pins on the session's row (`session_pins_body`).
        if not isinstance(session.get("cwd"), str) or not session["cwd"]:
            fail(f"{name}: cwd is required")
        if type(session.get("age")) is not int or session["age"] < 0:
            fail(f"{name}: age must be a non-negative integer")
        # The file lists rows in the order they should appear, with ages that agree (its comment on `sessions`), so a
        # younger session below an older one is a mistake in the file.
        if session["age"] < previous_age:
            fail(f"{name}: ages must not decrease down the list")
        previous_age = session["age"]
        for flag in ("yolo", "open"):
            if not isinstance(session.setdefault(flag, False), bool):
                fail(f"{name}: {flag} must be true or false")
        if not isinstance(session.get("seen", False), bool):
            fail(f"{name}: seen must be true or false")
        if session.get("status") not in REPLAY_THEN:
            fail(f"{name}: status must be one of {', '.join(REPLAY_THEN)}")
        # `then` is what the replay does once its transcript ends, when that is not what `status` implies (the demo
        # video's sessions stop on an input directive, so their end is not the status staging waits for).
        if session.setdefault("then", REPLAY_THEN[session["status"]]) not in REPLAY_THEN.values():
            fail(f"{name}: then must be one of {', '.join(REPLAY_THEN.values())}")
        if session["status"] == "idle" and "seen" not in session:
            fail(f"{name}: an idle session needs seen: true or false")
        if type(session.setdefault("exit_code", 0)) is not int:
            fail(f"{name}: exit_code must be an integer")
        transcript = session.get("transcript")
        if transcript is not None:
            if not isinstance(transcript, str) or not (directory / transcript).is_file():
                fail(f"{name}: transcript {transcript!r} is not a file in {directory}")
            session["transcript"] = (directory / transcript).resolve()
    if sum(session["open"] for session in sessions) != 1:
        fail("exactly one session must carry open: true")
    check_host_values(document, keys, fail)
    # Checked here, so a folder that would leave the private home fails before anything is created.
    scenario_home_dirs(document, fail)
    document["source"] = source
    return document


# ---------------------------------------------------------------------------------------------------------------------
# Scenario runs: the staged host values
#
# What a scenario stages per host beyond its sessions, for the docs screenshots (docs/docs-shots/SPEC.md, "What is real
# and what is not"): the launch history behind the launcher's recent setups and recent folders (`launch_history`,
# `folders`), the checkout trash (`checkout_trash`) and a terminal's file checks (`terminal_files`). A staged run cannot
# produce any of them for real: the helm records each staged session's launch in its scratch directory under the run, a
# machine path the capture refuses; the run has no managed checkouts; and a printed path is looked up in that scratch
# directory. The browser capture rewrote each in transit. The maintainer decided on 2026-11-18 to extend the app's pins
# file seam instead (the "Staged host values" section of crates/gui/farhelm-gui/src/agent_startup.rs), so these go
# into the pins file by host alias (`session_pins_body`), and an agent build substitutes them for what the host answers.
# Every field is optional: a scenario without one stages that value empty for every host, which also hides what the
# real host would have answered.

# The wire names of the launch choices a staged recent setup may carry: `LaunchHarness`, `LaunchEffort` and
# `LaunchPermission` in crates/farhelm-proto/src/launch.rs (snake_case; scripts/test-gui-agent.py checks them against
# that source). `custom` is left out of the permissions on purpose: it needs its own flags, which a scenario has no
# field for. The `custom` harness is left out for the same reason: it needs a registered custom agent's ID.
LAUNCH_HARNESSES = ("cursor", "codex", "claude", "muse", "goose", "pi", "omp", "open_code", "grok", "antigravity",
                    "bash", "zsh")
LAUNCH_EFFORTS = ("off", "minimal", "low", "medium", "high", "xhigh", "max", "ultra")
LAUNCH_PERMISSIONS = ("guarded", "yolo", "approve", "smart_approve", "chat")

# Each staged host field's entries: the keys an entry must have, with their kinds, and the keys it may have.
HOST_VALUE_FIELDS = {
    "launch_history": ({"host": str, "cwd": str, "harness": str, "age": int},
                       {"model": str, "effort": str, "permissions": str, "workspace_trust": bool}),
    "folders": ({"host": str, "cwd": str, "age": int}, {}),
    "checkout_trash": ({"host": str, "repository": str, "checkout": str, "root": str, "age": int, "bytes": int}, {}),
    "terminal_files": ({"host": str, "path": str, "shown_path": str, "size": int}, {}),
}


def check_host_values(document: dict, keys: set[str], fail) -> None:
    """Check the scenario's staged host fields (HOST_VALUE_FIELDS) in place, calling `fail` with the first mistake: each
    an optional list of objects with exactly the keys its field allows, every string non-empty, every number a
    non-negative integer, `host` naming a scenario host, a recent setup's choices among the wire's names, and a trash
    root an absolute path. As strict as the session checks, for the same reason: a typo would otherwise stage nothing
    and leave a picture that looks plausible and is wrong.

    A recent setup's model and effort are only checked for their shape: whether the launcher's catalog offers them for
    the harness is the launcher's own check, which a staged setup goes through like any real one (the substitution is
    upstream of it), so a model the catalog lacks shows the launcher's refusal in the picture, as the scenario's comment
    warns."""
    for field, (required, optional) in HOST_VALUE_FIELDS.items():
        entries = document.get(field, [])
        if not isinstance(entries, list):
            fail(f"{field} must be a list")
        for index, entry in enumerate(entries):
            name = f"{field}[{index}]"
            if not isinstance(entry, dict):
                fail(f"{name} must be an object")
            unknown = set(entry) - set(required) - set(optional)
            if unknown:
                fail(f"{name}: unknown field {sorted(unknown)[0]}")
            for key, kind in {**required, **optional}.items():
                if key not in entry:
                    if key in required:
                        fail(f"{name}: {key} is required")
                    continue
                value = entry[key]
                # `bool` is an `int` to Python, so the kinds are compared exactly.
                if type(value) is not kind or (kind is str and not value) or (kind is int and value < 0):
                    shape = {str: "a non-empty string", int: "a non-negative integer", bool: "true or false"}[kind]
                    fail(f"{name}: {key} must be {shape}")
            if entry["host"] not in keys:
                fail(f"{name}: unknown host {entry['host']!r}")
            for key, allowed in (("harness", LAUNCH_HARNESSES), ("effort", LAUNCH_EFFORTS),
                                 ("permissions", LAUNCH_PERMISSIONS)):
                if key in entry and entry[key] not in allowed:
                    fail(f"{name}: {key} must be one of {', '.join(allowed)}")
            if "root" in entry and not entry["root"].startswith("/"):
                fail(f"{name}: root must be an absolute path")


def staged_selection(entry: dict) -> dict:
    """A recent setup's choices in the wire's `LaunchSelection` shape, which the app reads them in: the harness, the
    model, effort and permissions or null, and workspace trust only when the scenario gives it (the wire leaves it out
    when unset)."""
    selection = {key: entry.get(key) for key in ("model", "effort", "permissions")}
    selection = {"harness": entry["harness"], **selection}
    if "workspace_trust" in entry:
        selection["workspace_trust"] = entry["workspace_trust"]
    return selection


def staged_host_body(plan: dict, host: dict) -> dict:
    """The pins file's entry for scenario host `host`: its alias, which the app matches it by, and the scenario's values
    for it in the shapes `StagedHost` in crates/gui/farhelm-gui/src/agent_startup.rs reads, in the file's order. Every
    list is present, an empty one included: the app replaces what a staged host answers whatever the list holds."""

    def mine(field: str) -> list[dict]:
        return [entry for entry in plan.get(field, []) if entry["host"] == host["key"]]

    return {
        "alias": host["alias"],
        "launches": [{"cwd": entry["cwd"], "selection": staged_selection(entry), "age": entry["age"]}
                     for entry in mine("launch_history")],
        "folders": [{"cwd": entry["cwd"], "age": entry["age"]} for entry in mine("folders")],
        "trash": [{key: entry[key] for key in ("repository", "checkout", "root", "age", "bytes")}
                  for entry in mine("checkout_trash")],
        "files": [{key: entry[key] for key in ("path", "shown_path", "size")} for entry in mine("terminal_files")],
    }


def scenario_home_dirs(plan: dict, fail=None) -> list[str]:
    """The folders under `~` the scenario names, as paths relative to the home, each once, in the file's order: every
    session's `cwd`, every recent setup's and recent folder's, and every launch template's folder destination.

    `start` creates them in the run's private home (`create_scenario_dirs`), which every staged supervisor, local and
    remote, expands `~` against: the supervisor resolves `~` from its own HOME (`expand_tilde_cwd` in
    crates/farhelm-supervisor/src/service/core.rs), and `start` gives each the staged HOME. So the launcher's real
    folder check finds them, and shows no "does not exist" warning under the folder field. A folder outside `~` is left
    alone. With `fail`, a `~` folder that would leave the home (a `..` part) or names another user's (`~user`) is
    refused; without it, such a folder is skipped."""
    folders = [session.get("cwd") for session in plan.get("sessions", [])]
    folders += [entry.get("cwd") for field in ("launch_history", "folders") for entry in plan.get(field, [])]
    for template in plan.get("templates", []) or []:
        destination = ((template or {}).get("fields") or {}).get("destination") or {}
        folders.append(destination.get("folder") if isinstance(destination, dict) else None)
    found = []
    for folder in folders:
        if not isinstance(folder, str) or not folder.startswith("~") or folder == "~":
            continue
        parts = folder[2:].split("/") if folder.startswith("~/") else None
        if parts is None or any(part in ("..", ".") for part in parts):
            if fail:
                fail(f"folder {folder!r} must be ~ or under ~/ without . or .. parts")
            continue
        relative = "/".join(part for part in parts if part)
        if relative and relative not in found:
            found.append(relative)
    return found


def create_scenario_dirs(home: Path, plan: dict) -> list[Path]:
    """Create the scenario's `~` folders (`scenario_home_dirs`) in the staged run's private `home`, private like it, and
    return them."""
    made = []
    for relative in scenario_home_dirs(plan):
        directory = home / relative
        directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        made.append(directory)
    return made


def session_pins_body(plan: dict) -> dict:
    """The pins file a scenario run hands the app (SESSION_PINS_ENV): each session's title, `age` and `cwd`, which an
    agent build shows in place of the session's real activity and scratch folder, and each scenario host's staged
    values by its alias (`staged_host_body`), which it shows in place of what the host answers (why and how: the
    "Staged session values" and "Staged host values" sections of crates/gui/farhelm-gui/src/agent_startup.rs).

    The shape, which the app reads strictly (an unknown or missing key fails its launch):

        {"sessions": [{"title": ..., "age": SECONDS, "cwd": "~/..."}],
         "hosts": [{"alias": ...,
                    "launches": [{"cwd": ..., "selection": {"harness": ..., "model": ..., "effort": ...,
                                                            "permissions": ...}, "age": SECONDS}],
                    "folders": [{"cwd": ..., "age": SECONDS}],
                    "trash": [{"repository": ..., "checkout": ..., "root": "/...", "age": SECONDS, "bytes": N}],
                    "files": [{"path": ..., "shown_path": "/...", "size": N}]}]}

    Every age is seconds before the moment the app read the file."""
    return {
        "sessions": [
            {"title": session["title"], "age": session["age"], "cwd": session["cwd"]} for session in plan["sessions"]
        ],
        "hosts": [staged_host_body(plan, host) for host in plan["hosts"]],
    }


def write_session_pins(run: Path, plan: dict) -> Path:
    """Write the scenario `plan`'s pins file (`session_pins_body`) into the run directory and return its path, the
    value SESSION_PINS_ENV hands the app."""
    path = run / SESSION_PINS_FILE
    path.write_text(json.dumps(session_pins_body(plan), indent=2) + "\n")
    return path


def scenario_remote_hosts(plan: dict) -> list[str]:
    """The ssh destinations of the scenario's remote hosts, in its order: the run's `--remote-host` list."""
    return [host["ssh"] for host in plan["hosts"] if host["kind"] == "remote"]


def scenario_window_size(plan: dict, platform: str = sys.platform) -> str:
    """The `--window-size` a scenario run asks for: the viewport as the app's own drawing, so on Linux larger by the
    window's frame on every side and its title bar on top, the numbers the screenshot batch crops off
    (`LINUX_FRAME` and `LINUX_TITLE_BAR` in scripts/gui-screenshot-batch.py, which say where they come from).
    These are logical dimensions at any output scale."""
    frame, bar = 0, 0
    if platform.startswith("linux"):
        batch = _sibling("gui-screenshot-batch.py")
        frame, bar = batch.LINUX_FRAME, batch.LINUX_TITLE_BAR
    viewport = plan["viewport"]
    return f"{viewport['width'] + 2 * frame}x{viewport['height'] + 2 * frame + bar}"


def scenario_creation_order(plan: dict) -> list[int]:
    """The order `stage_scenario` creates the sessions in, as indexes into the scenario's list: the open session first,
    then the rest in the scenario's order.

    The window picks a session to open by itself as soon as its list has one, and keeps it: with no visit history or
    remembered selection on a fresh helm, its pick is the newest-created session, and a selection stays until that
    session is gone (`crates/gui/farhelm-gui/src/session_list/selection.rs`). The old staging set the `last_selected`
    preference before the page loaded, so the page's own pick never landed on an idle row and marked it seen; the window
    here is up before the helm has any session, so creating the open session alone first is what makes the window's
    own pick the one the scenario names."""
    opened = next(index for index, session in enumerate(plan["sessions"]) if session["open"])
    return [opened, *(index for index in range(len(plan["sessions"])) if index != opened)]


def staged_agent_script(session: dict, fixtures: str) -> str:
    """The STAGED_AGENT_SCRIPT a scenario session's working directory holds, which the run's `claude` or `codex` wrapper
    runs: the fake agent's replay of the session's transcript, in the dialect of its wrapper (whose question menus the
    supervisor's reading of that agent type recognizes), holding the screen its `then` names (with its exit code for an
    exit), with every argument the wrapper got passed on, where the supervisor's own per-launch flags land."""
    argv = [fixtures, "fake-agent", "--script", "replay", "--dialect", session["wrapper"]]
    if session.get("transcript") is not None:
        argv += ["--transcript", str(session["transcript"])]
    argv += ["--then", session["then"]]
    if session["then"] == "exit":
        argv += ["--exit-code", str(session["exit_code"])]
    return (
        "#!/bin/sh\n"
        "# A scenario session's replay (scripts/gui-agent.py, \"Scenario runs\"), run by the staged wrapper.\n"
        f'exec {" ".join(shlex.quote(arg) for arg in argv)} "$@"\n'
    )


def scenario_session_body(session: dict, host_id: int, cwd: Path) -> dict:
    """The `POST /api/sessions` body that creates a scenario session on the host `host_id`, in `cwd`: an agent launch
    of its wrapper's harness, shaped as the launcher's Agent tab sends one (`create_body` in
    crates/gui/farhelm-gui/src/launcher/request.rs), YOLO for a `yolo` session and Guarded, the launcher's default for
    both harnesses, for every other. No model or effort, so the helm adds no flags for them.

    An agent launch rather than a command launch (the migration's decision of 2026-11-16), so that the helm records an
    agent session the way a user's launch makes one: it composes the command itself (`compile` in
    crates/farhelm-helm/src/launches.rs), the row's mark, status reading and shield come from that launch with nothing
    in the listing rewritten, and the details bar shows the composed command. A command launch of the same agent type
    would have to carry `{farhelm_args}`, and its stored command, placeholder and all, is what the details bar showed.
    The composed command runs the harness's program by its bare name, which the run's wrapper answers.

    A YOLO one also carries `confirm_yolo`: every host starts out asking before a YOLO launch, and confirming this one
    launch is what a user answering that question does, which, unlike turning the question off for the host, leaves
    no setting behind that the picture could show."""
    return {
        "host": host_id,
        "cwd": str(cwd),
        "title": session["title"],
        "launch": {
            "harness": SCENARIO_HARNESSES[session["wrapper"]],
            "model": None,
            "effort": None,
            "permissions": "yolo" if session["yolo"] else "guarded",
        },
        "confirm_yolo": session["yolo"],
    }


def scenario_host_ids(plan: dict, listing: list[dict]) -> tuple[dict[str, int], list[str]]:
    """The helm's host id of every scenario host found connected in `listing` (`GET /api/hosts`), by key, and the hosts
    not yet connected as `key=phase` (`missing` for one the listing lacks). The local host is the listing's `local`
    row; a remote one is the SSH row whose destination is its expanded ssh spelling."""
    ids, pending = {}, []
    for host in plan["hosts"]:
        if host["kind"] == "local":
            match = next((row for row in listing if row.get("kind") == "local"), None)
        else:
            match = next(
                (row for row in listing if row.get("kind") == "ssh" and row.get("destination") == host["ssh"]), None
            )
        phase = ((match or {}).get("state") or {}).get("phase")
        if phase == "connected":
            ids[host["key"]] = match["id"]
        else:
            pending.append(f"{host['key']}={phase or 'missing'}")
    return ids, pending


def seen_body(session: dict, row: dict) -> dict | None:
    """The `PUT /api/sessions/{id}/seen` body for a scenario session, from its listed `row`, or None for a session the
    scenario leaves to the window: only an idle session's seen state shows (its dot is blue while unseen, grey once
    seen; SPEC.md, "Status"), as in the old staging.

    A seen one is stamped with its row's own activity as the helm lists it (`effective_activity`): the window compares
    the pinned stamp (`session_pins_body`), which is older than any real activity of the session, since the app counted
    it back from its own start, so the real one reads as seen there too. The old staging stamped `now - age`, which
    agreed with the ages it rewrote in transit."""
    if session["status"] != "idle":
        return None
    if not session["seen"]:
        return {"seen_activity_at": None}
    return {"seen_activity_at": row.get("last_activity_at") or row.get("created_at")}


def agent_track_id(key: str) -> str:
    """The element id the session list gives the agent track of the row of session `key`, the one node naming the row's
    agent mark and shield together (`AGENT_ID_SUFFIX` in crates/gui/farhelm-gui/src/session_list/row_marks.rs)."""
    return f"session-row-{key}-agent"


def shield_problems(plan: dict, ids: dict[int, str], entries: list[dict]) -> tuple[list[str], list[str]]:
    """The scenario sessions whose row's shield the window does not show yet, and those whose shield is the wrong one,
    from a `snapshot -a --json`'s `entries`. `ids` maps each session's index in the scenario to its helm key.

    The old staging's last check before the capture, followed here: each row has exactly one shield, slashed for a
    `yolo` session and plain for every other. The shield is read from the first paragraph of the agent track's
    accessible name (`Description::of` in crates/gui/farhelm-gui/src/session_list/row_marks.rs), which for an agent
    launch is `<agent> — ` and the heading of its stored permission's help (`permission_help` in
    crates/gui/farhelm-gui/src/permission/words.rs): `YOLO: ...` or `Guarded · ...`. A status-line observation of
    YOLO, which draws the slashed shield whatever the launch chose, comes first in its place, so it fails the check."""
    missing, wrong = [], []
    for index, key in ids.items():
        session = plan["sessions"][index]
        names = [
            entry.get("name", "")
            for entry in entries
            if (entry.get("key") or {}).get("path", "").rpartition("/")[2] == agent_track_id(key)
        ]
        if len(names) != 1:
            missing.append(f"{session['title']} ({len(names)} shields)")
            continue
        # Neither heading holds the other, and the observation's "line: <mode>" holds neither.
        want = "— YOLO:" if session["yolo"] else "— Guarded"
        first = names[0].split("\n\n")[0]
        if want not in first:
            wrong.append(f"{session['title']}: {first!r} does not say {want!r}")
    return missing, wrong


def poll(what: str, probe, timeout: float, interval: float):
    """Call `probe()`, which returns `(value, pending)`, until its value is not None, and return that value; after
    `timeout` seconds fail saying what was awaited and what `pending` last said."""
    deadline = time.monotonic() + timeout
    while True:
        value, pending = probe()
        if value is not None:
            return value
        if time.monotonic() > deadline:
            raise RuntimeError(f"timed out after {timeout:.0f} s waiting for {what}: {pending}")
        time.sleep(interval)  # sleep-ok: polling the helm or the window; the harness subscribes to no events


def stage_scenario(
    run: Path,
    plan: dict,
    fixtures: str,
    snapshot,
    request=None,
    timeout: float = SETTLE_TIMEOUT_SECONDS,
    interval: float = 1.0,
) -> dict[int, str]:
    """Stage the scenario `plan` in the run's helm, once the app is up; return each session's helm key by its index in
    the scenario. `snapshot()` returns the window's `snapshot -a --json` entries; `request` is `helm_request` unless a
    test hands in its own.

    In order: wait until every scenario host is connected (so no create races a host's ssh handshake), give each its
    alias, set SCENARIO_PREFERENCES, create the open session and wait until the window has opened it (the helm's
    `last_selected`, which the window writes for its own pick), create the others (`scenario_creation_order` says why
    in that order), each in its own scratch directory `<run>/work/<index>` holding its STAGED_AGENT_SCRIPT, wait until
    the supervisor reads every session as its status, set the idle sessions' seen state, and check every row's shield
    in the window (`shield_problems`). Every status is the supervisor's own reading of a live screen; nothing here
    writes one in."""
    request = request or helm_request

    def hosts_connected():
        ids, pending = scenario_host_ids(plan, request(run, "GET", "/api/hosts")["hosts"])
        return (None if pending else ids), ", ".join(pending)

    host_ids = poll("every scenario host to connect", hosts_connected, timeout, interval)
    for host in plan["hosts"]:
        request(run, "POST", f"/api/hosts/{host_ids[host['key']]}/alias", {"alias": host["alias"]})
    request(run, "PUT", "/api/preferences", SCENARIO_PREFERENCES)

    work = run / "work"
    work.mkdir(exist_ok=True)
    ids: dict[int, str] = {}
    for position, index in enumerate(scenario_creation_order(plan)):
        session = plan["sessions"][index]
        cwd = work / str(index)
        cwd.mkdir()
        (cwd / STAGED_AGENT_SCRIPT).write_text(staged_agent_script(session, fixtures))
        body = scenario_session_body(session, host_ids[session["host"]], cwd)
        ids[index] = request(run, "POST", "/api/sessions", body)["id"]
        if position == 0:
            opened = ids[index]

            def window_opened():
                selected = (request(run, "GET", "/api/preferences") or {}).get("last_selected")
                return (True if selected == opened else None), f"last_selected={selected}"

            poll(f"the window to open {session['title']!r}", window_opened, timeout, interval)

    # Waited for by title, so a session that does not settle is named the way the scenario names it.
    titles = {key: plan["sessions"][index]["title"] for index, key in ids.items()}
    wait_for_statuses(
        lambda: {titles.get(key, key): state for key, state in listed_states(run, request).items()},
        {titles[key]: plan["sessions"][index]["status"] for index, key in ids.items()},
        timeout,
        interval,
    )
    by_id = {key: index for index, key in ids.items()}
    for row in request(run, "GET", "/api/sessions")["sessions"]:
        index = by_id.get(row.get("id"))
        body = None if index is None else seen_body(plan["sessions"][index], row)
        if body is not None:
            request(run, "PUT", f"/api/sessions/{urllib.parse.quote(row['id'], safe='')}/seen", body)

    def shields_drawn():
        missing, wrong = shield_problems(plan, ids, snapshot())
        return (None if missing else wrong), f"no single shield yet for {', '.join(missing)}"

    wrong = poll("every row's shield in the window", shields_drawn, timeout, interval)
    if wrong:
        raise RuntimeError(f"rows show the wrong permission shield: {'; '.join(wrong)}")
    return ids


# ---------------------------------------------------------------------------------------------------------------------
# Scenario captures: the picture of a staged scenario, and what has to hold before it is taken
#
# `capture OUT.png` on a scenario run takes the docs-media picture (docs/gui-agent.md, "Scenario captures"), the GPUI
# counterpart of the browser capture's last steps (`e2e/readme-hero/capture.spec.ts`, removed in #3401). OUT is written
# only when every check passed. The README hero is the first caller (`scripts/readme-screenshot.sh`); the docs
# screenshots and the demo video are meant to reuse these helpers when they are rebuilt.

# The element id of an open session's terminal (`attached.rs` in crates/gui/farhelm-gui/src/terminal), clicked to give
# it focus when the `terminal_text` probe says no terminal holds it: a fresh window's focus is on New session.
TERMINAL_ELEMENT = "terminal-grid"

# Where `capture` keeps the window's shot in the run directory, cropped on Linux, whatever its checks found.
CAPTURE_FILE = "capture.png"

# How long the open session's terminal gets to show its transcript's last line, and the window to stop changing.
TRANSCRIPT_SECONDS = 30
IDLE_MILLISECONDS = 15000
# How long the window's drawing must stay the same before a capture counts it as settled: the bridge's own idle wait.
IDLE_STILL_SECONDS = 0.3


def settled_hash(tree: dict, skip: str = TERMINAL_ELEMENT) -> str:
    """What a capture compares to tell that the window stopped changing: every element and text run of the bridge's
    raw tree (`gpui-agent tree`), as the bridge's own `wait --idle` hashes them, except the elements inside `skip`'s
    element and the text they paint.

    The bridge's idle wait hashes the terminal's contents too, and a running session's agent redraws a clock in its
    terminal every second ("Verifying… (2m 16s)"). Where reading the tree takes a second, as on a loaded machine, two
    reads never match, and every capture of a scenario with a running open session timed out waiting for idle. The
    terminal is waited on separately, for its transcript's last line, so leaving its contents out loses nothing a
    picture depends on; the rest of the window (dialogs settling, layout, the list) is still waited for.
    """
    nodes = tree.get("nodes", [])
    inside: set[int] = set()
    for index in range(len(nodes)):
        at = index
        while at is not None and 0 <= at < len(nodes):
            if nodes[at].get("id") == skip:
                inside.add(index)
                break
            at = nodes[at].get("parent")
    kept_nodes = [node for index, node in enumerate(nodes) if index not in inside]
    kept_texts = [text for text in tree.get("texts", []) if text.get("node") not in inside]
    return json.dumps([kept_nodes, kept_texts], sort_keys=True)


def settle(read_tree: Callable[[], dict], timeout: float, still: float = IDLE_STILL_SECONDS,
           clock: Callable[[], float] = time.monotonic, pause: Callable[[float], None] = time.sleep) -> None:
    """Wait until the window's drawing apart from the terminal (`settled_hash`) has stayed the same for `still`
    seconds, reading it with `read_tree`; a RuntimeError after `timeout` seconds of change. The bridge's `wait --idle`
    with the terminal left out."""
    start = clock()
    last, since = None, start
    while True:
        current = settled_hash(read_tree())
        now = clock()
        if current != last:
            last, since = current, now
        elif now - since >= still:
            return
        if now - start > timeout:
            raise RuntimeError(f"the window did not stop changing (its terminal aside) within {timeout:g}s")
        pause(0.05)


def last_transcript_line(path: Path) -> str:
    """The last line a replay transcript prints, as the terminal shows it (comment and blank lines skipped, written
    escapes removed, trimmed): the open session's readiness check, as the browser staging's `lastTranscriptLine`."""
    import re

    printed = []
    for line in Path(path).read_text().split("\n"):
        if line.startswith("##") or not line.strip():
            continue
        printed.append(re.sub(r"\x1b\[[0-9;]*m", "", line.replace("\\e", "\x1b")).strip())
    if not printed or not printed[-1]:
        raise UsageError(f"{path} prints nothing")
    return printed[-1]


def _inside(rect: dict, x: float, y: float) -> bool:
    """Whether the point lies in the bridge's rectangle (`x`, `y`, `w`, `h` in window pixels)."""
    return rect["x"] <= x < rect["x"] + rect["w"] and rect["y"] <= y < rect["y"] + rect["h"]


def _drawn(rect: dict) -> bool:
    """Whether a bridge rectangle covers any of the window (a clipped or scrolled-out element's is empty)."""
    return rect["w"] > 0 and rect["h"] > 0


def park_targets(entries: list[dict], tree: dict, *, rejected: list[tuple[str, str]] | None = None) -> list[str]:
    """The `hover` targets that park the pointer where nothing in the window reacts to it, best first, as `id:` paths
    from a `snapshot -a --json`'s `entries`, judged against the same frame's raw `tree`.

    A target qualifies when the centre the bridge aims at lies outside every drawn element with a `hover` or `tooltip`
    flag, which would draw a highlight or a tooltip into the picture (a terminal link's tooltip once showed the run's
    real path). Only the first entry of each id path is considered, since the bridge resolves that path to the first
    entry. When `rejected` is supplied, record why each named candidate was filtered out before asking the bridge."""
    hot = [
        (
            node["visible"],
            node.get("id") or node.get("label") or f"tree node {index} ({node.get('type', 'unnamed element')})",
        )
        for index, node in enumerate(tree.get("nodes", []))
        if _drawn(node["visible"]) and (node["flags"].get("hover") or node["flags"].get("tooltip"))
    ]
    targets, seen = [], set()
    for entry in entries:
        path = (entry.get("key") or {}).get("path", "")
        if not path or path in seen:
            continue
        seen.add(path)
        box = entry["visible"]
        target = f"id:{path}"
        x, y = box["x"] + box["w"] / 2, box["y"] + box["h"] / 2
        if not _drawn(box):
            if rejected is not None:
                rejected.append((target, "the element has no visible area"))
            continue
        blocker = next((name for rect, name in hot if _inside(rect, x, y)), None)
        if blocker is None:
            targets.append(target)
        elif rejected is not None:
            rejected.append((target, f"its centre is inside the reacting element {blocker}"))
    return targets


def park_failure(rejected: list[tuple[str, str]]) -> str:
    """Keep a large frame or verbose bridge error from burying the useful parking diagnostics."""
    def compact(text: str, width: int) -> str:
        # Long paths can carry the useful ID at the end. Bridge errors can also name the blocker after a long label.
        text = " ".join(text.split())
        if len(text) <= width:
            return text
        head = width // 2
        return f"{text[:head]}…{text[-(width - head - 1):]}"

    message = "no spot in the window is clear of hover and tooltip reactions to park the pointer on"
    if not rejected:
        return f"{message}\n  no named candidates in the snapshot"
    limit = 12
    lines = [message]
    lines.extend(f"  {compact(target, 100)}: {compact(reason, 180)}" for target, reason in rejected[:limit])
    if len(rejected) > limit:
        lines.append(f"  ... and {len(rejected) - limit} more rejected candidates")
    return "\n".join(lines)


def capture_needles(run: Path, record: dict, user: str, home: str | None, hosts: list[str],
                    hostname: str | None) -> tuple[list[str], list[str]]:
    """What a capture refuses to find in the window: `(anywhere, drawn)`.

    `anywhere`, refused in drawn text, accessible labels and the terminal alike: the run directory, the checkout, the
    build directory and the account's home, each as given and as resolved (a Mac's `/tmp` is `/private/tmp`).

    `drawn`, refused only where the picture shows it: each staged destination naming the account (`user@localhost`),
    the account followed by `@`, and the hostname. Accessible labels may hold them, since they are never drawn, and the
    host rail names each host by its destination there. The bare account name is not refused: a short one is an
    ordinary word."""
    places = [run, REPO, Path(record["binaries"]["farhelm"]).parent.parent]
    if home:
        places.append(Path(home))
    anywhere = []
    for place in places:
        for spelling in (str(place), os.path.realpath(place)):
            if spelling not in anywhere and spelling != "/":
                anywhere.append(spelling)
    drawn = [host for host in hosts if "@" in host and host.partition("@")[0] == user]
    drawn.append(f"{user}@")
    if hostname:
        for name in (hostname, hostname.split(".")[0]):
            if name and name not in drawn and name != "localhost":
                drawn.append(name)
    return anywhere, drawn


def leaks(tree: dict, terminal: str, anywhere: list[str], drawn: list[str]) -> list[str]:
    """Each place the window shows a needle of `capture_needles`, as `<where>: <needle> in <text>`.

    Drawn text is the frame's visible text runs and field values, the window title and the focused terminal's text;
    text out of view and accessible labels get only the `anywhere` needles. Ids and source locations (which name the
    build machine's crate cache) are not drawn and not looked at. A `drawn` name matches only as a whole word, so a
    hostname `mac` does not refuse "this machine". Each text is reported once."""
    import re

    found = []

    def holds(text: str, needle: str, word: bool) -> bool:
        if not word:
            return needle in text
        # A boundary only where the needle has a word character at that end: `someone@` must still match
        # `someone@elsewhere`.
        before = r"(?<![\w.-])" if re.match(r"\w", needle) else ""
        after = r"(?![\w-])" if re.search(r"\w$", needle) else ""
        return re.search(before + re.escape(needle) + after, text) is not None

    def look(where: str, text: str | None, needles: list[str]) -> None:
        for needle in needles:
            if text and holds(text, needle, needle not in anywhere):
                found.append(f"{where}: {needle!r} in {text[:200]!r}")
                return

    for run in tree.get("texts", []):
        look("drawn text" if _drawn(run["visible"]) else "hidden text", run.get("text"),
             anywhere + (drawn if _drawn(run["visible"]) else []))
    for node in tree.get("nodes", []):
        look("accessible label", node.get("label"), anywhere)
        look("field value", node.get("value"), anywhere + (drawn if _drawn(node["visible"]) else []))
    look("window title", tree.get("title"), anywhere + drawn)
    look("terminal", terminal, anywhere + drawn)
    return found


def expected_capture_size(plan: dict) -> tuple[int, int]:
    """The pixel size a scenario's picture must have: its viewport times its scale, the check the publish script
    repeats (`expected_size` in scripts/publish-readme-hero.sh)."""
    viewport = plan["viewport"]
    scale = viewport.get("scale", 1)
    return viewport["width"] * scale, viewport["height"] * scale


def size_problem(actual: tuple[int, int] | None, plan: dict, window: dict) -> str | None:
    """Why a capture of `actual` pixels is not the scenario's picture, or None when it is. `window` is the bridge's
    `windows` entry, quoted to show whether the window fits the display and whether its scale matches the scenario."""
    want = expected_capture_size(plan)
    if actual == want:
        return None
    viewport = plan["viewport"]
    got = "no readable PNG" if actual is None else f"{actual[0]}x{actual[1]} pixels"
    return (
        f"the capture is {got}, but the scenario declares {want[0]}x{want[1]} ({viewport['width']}x"
        f"{viewport['height']} at scale {viewport.get('scale', 1)}); the window is {window.get('width', 0):.0f}x"
        f"{window.get('height', 0):.0f} points at scale {window.get('scale')}. A window smaller than requested "
        "means the display is too small. On Linux, start the run with a --size that fits the window and the "
        "scenario's --scale"
    )


def capture(run: Path, record: dict, args: list[str], platform: str = sys.platform, bridge_cli=None) -> int:
    """`capture OUT.png`: the scenario run's picture, written to OUT only if every check passes (see the section
    comment above). Each step says what it waits for or what it found; a failure raises a RuntimeError naming the step,
    and the shot taken so far stays at CAPTURE_FILE in the run directory. `platform` decides the Linux frame crop and
    `bridge_cli` is `cli` unless a test hands in a stand-in bridge. `--keep-focus` leaves dialogs and their fields
    focused, skipping the terminal click and transcript wait so a picture can show those surfaces. `--park` uses the
    shot's chosen spot directly; the bridge still refuses a covered target."""
    import argparse
    import shutil

    parser = argparse.ArgumentParser(prog="gui-agent.py RUN_DIR capture")
    parser.add_argument("output", type=Path, help="where to write the PNG once every check passed")
    parser.add_argument("--keep-focus", action="store_true",
                        help="keep the current focus; skip the terminal click and transcript wait for dialog shots")
    parser.add_argument("--park", metavar="TARGET", help="park on this id: target instead of searching for a spot")
    options = parser.parse_args(args)
    if options.park is not None and (not options.park.startswith("id:") or len(options.park) == 3):
        raise UsageError("--park must be an id: target")
    if not record.get("scenario"):
        raise UsageError("capture takes the picture of a scenario run; start this run with --scenario")
    bridge_cli = bridge_cli or cli
    # The account from the user database, for the scenario's `$USER` and the needles alike, whatever `$USER` says.
    account = pwd.getpwuid(os.getuid())
    plan = load_scenario(Path(record["scenario"]["file"]), account.pw_name)
    opened = next(session for session in plan["sessions"] if session["open"])
    batch = _sibling("gui-screenshot-batch.py")

    def bridge(*verb: str) -> str:
        result = bridge_cli(run, record, list(verb), capture=True)
        if result.returncode != 0:
            raise RuntimeError(f"the bridge's `{' '.join(verb)}` failed: {result.stderr.strip()}")
        return result.stdout

    def terminal() -> str | None:
        answer = bridge_cli(run, record, ["get", "probe", "terminal_text"], capture=True)
        return None if answer.returncode != 0 or answer.stdout.strip() == "null" else probe_text(answer.stdout)

    def park() -> None:
        if options.park is not None:
            bridge("hover", options.park)
            print(f"capture: pointer parked on {options.park}", flush=True)
        else:
            # The bridge aims at an element's centre, so each candidate is tried in turn: one that something else
            # covers at its centre is refused by the bridge, and the next is tried.
            entries = json.loads(bridge("snapshot", "-a", "--json", "--no-source"))["entries"]
            filtered, refused = [], []
            for target in park_targets(entries, json.loads(bridge("tree")), rejected=filtered):
                result = bridge_cli(run, record, ["hover", target], capture=True)
                if result.returncode == 0:
                    print(f"capture: pointer parked on {target}", flush=True)
                    break
                reason = result.stderr.strip() or f"exit status {result.returncode}"
                refused.append((target, f"the bridge refused hover: {reason}"))
            else:
                # Actual hover attempts offer the most direct lead, so show those before screened-out candidates.
                raise RuntimeError(park_failure(refused + filtered))
        settle(lambda: json.loads(bridge("tree")), IDLE_MILLISECONDS / 1000)

    # Park first: on a Mac the pointer is the real one, wherever it was left on the screen, and resting on a link in the
    # terminal it opens a tooltip naming the link's real target, which covers the terminal and refuses the click.
    park()
    # A dialog shot must keep its field focused: clicking the terminal could dismiss the surface being photographed.
    if not options.keep_focus:
        if terminal() is None:
            bridge("click", f"id:{TERMINAL_ELEMENT}")
        if opened.get("transcript") is not None:
            line = last_transcript_line(opened["transcript"])

            def replayed():
                text = terminal()
                return (text if text is not None and line in text else None), f"it shows {text!r}"

            poll(f"the open session's terminal to show {line!r}", replayed, TRANSCRIPT_SECONDS, 0.25)
        print("capture: the open session's terminal holds focus and shows its transcript's last line", flush=True)
    # And again, since the click left the pointer in the middle of the terminal.
    park()

    # Leak check, around the shot: what the window draws and holds may not name the machine.
    host_names = scenario_remote_hosts(plan)
    needles = capture_needles(run, record, account.pw_name, account.pw_dir, host_names, os.uname().nodename)

    def leak_check(when: str) -> None:
        tree = json.loads(bridge("tree"))
        found = leaks(tree, terminal() or "", *needles)
        if found:
            raise RuntimeError(f"the window shows a machine detail {when} the shot, so it is not taken:\n  " +
                               "\n  ".join(found))
        # Counts, not the needles: they are this machine's paths and names, and this output gets pasted.
        print(f"capture: leak check {when} the shot passed: {len(tree.get('texts', []))} text runs, "
              f"{len(tree.get('nodes', []))} elements and the terminal hold none of {len(needles[0])} machine paths, "
              f"and nothing drawn holds any of {len(needles[1])} account and host names", flush=True)

    leak_check("before")
    shot = run / CAPTURE_FILE
    shot.unlink(missing_ok=True)
    bridge("screenshot", str(shot))
    leak_check("after")
    window = json.loads(bridge("windows", "--json"))[0]
    if platform.startswith("linux"):
        # The app draws its decorations in logical pixels and the bridge captures the whole buffer.
        scale = int(window["scale"])
        problem = batch.crop_frame(shot, batch.LINUX_FRAME_MARGIN * scale, batch.LINUX_FRAME_BORDER * scale,
                                   batch.LINUX_TITLE_BAR * scale)
        if problem:
            raise RuntimeError(f"the Linux window frame could not be cropped off {shot}: {problem}")
    actual = batch.png_size(shot)
    print(f"capture: the shot is {actual[0]}x{actual[1]} pixels" if actual else "capture: the shot is unreadable",
          flush=True)
    problem = size_problem(actual, plan, window)
    if problem:
        raise RuntimeError(f"{problem}. Nothing was written to {options.output}; the shot is kept at {shot} to look "
                           "at, and it is not the scenario's picture")
    options.output.parent.mkdir(parents=True, exist_ok=True)
    staged = options.output.with_name(options.output.name + ".partial")
    shutil.copyfile(shot, staged)
    os.replace(staged, options.output)
    print(options.output)
    return 0


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
    scale: int = 1,
) -> dict:
    """Start the run's compositor (`command`, normally [`mutter_command`]); return what the app needs to join it.

    `dbus-run-session` gives Mutter a bus no desktop shares, and the bus address is read from Mutter's own environment
    because dbus-run-session does not print it. Under that bus the supervisor cannot reach the user's systemd manager,
    so its sessions start without a transient scope (the supervisor's ordinary fallback). Mutter keeps this bus also
    in a user-session run, where only the app gets the caller's: Mutter registers its service names
    (`org.gnome.Mutter.DisplayConfig` and others) on the bus it runs on, and on the caller's it would compete with a
    desktop session's own compositor for them.

    Mutter takes its integer output scale from org.gnome.desktop.interface. Give only this process tree a private
    GSettings keyfile and config directory, so the scale and monitor layout cannot come from the user's desktop.

    If the compositor does not come up (no Wayland socket within `socket_wait` seconds, or no bus address), everything
    it started is stopped before the error propagates: `run.json` does not exist yet, so `stop` could not find it.
    """
    env = {k: v for k, v in base.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS")}
    config = run / "mutter-config"
    settings = config / "glib-2.0/settings/keyfile"
    settings.parent.mkdir(parents=True)
    settings.write_text(f"[org/gnome/desktop/interface]\nscaling-factor={scale}\n")
    env.update(XDG_RUNTIME_DIR=str(runtime), XDG_CONFIG_HOME=str(config), GSETTINGS_BACKEND="keyfile")
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


def listed_states(run: Path, request=None) -> dict[str, str | None]:
    """Every session the helm lists, by id, with the state of its status (`running`, `waiting`, ...). `request` is
    `helm_request` unless a test hands in its own."""
    rows = (request or helm_request)(run, "GET", "/api/sessions")["sessions"]
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
    """Wait until `read_states()` (the listing's state of each session, keyed as `targets` is: by id, or a scenario's
    sessions by title) reports every session in `targets` at its target state, or fail naming each session that is not
    and the state it is in.

    The statuses are the supervisor's own classification of each live screen, never written in by the harness, so a
    scene can show a waiting or idle session only after the supervisor has read it as one.
    """

    def settled():
        pending = unsettled(targets, read_states())
        return (None if pending else True), ", ".join(pending)

    poll("every seeded session to reach its status", settled, timeout, interval)


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


def window_entries(run: Path, record: dict) -> list[dict]:
    """Every element of the window, clipped and scrolled-out ones included, as the bridge's `snapshot -a --json` lists
    them; a RuntimeError when the bridge cannot answer."""
    result = cli(run, record, ["snapshot", "-a", "--json", "--no-source"], capture=True)
    if result.returncode != 0:
        raise RuntimeError(f"the bridge could not take a snapshot: {result.stderr.strip()}")
    return json.loads(result.stdout)["entries"]


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
    seeding.add_argument(
        "--scenario",
        type=Path,
        metavar="FILE",
        help="stage the fleet a docs-media scenario.json5 describes: its hosts, sessions, statuses and seen state, "
        "with the window at its viewport (makes the run a staged one; see docs/gui-agent.md, \"Scenario runs\")",
    )
    parser.add_argument("--size", help="Linux only: Mutter's virtual monitor in logical points (default 1600x1200)")
    parser.add_argument("--scale", type=int, choices=(1, 2), help="Linux only: Mutter's output scale (default 1)")
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
    # A scenario is read and checked before anything else happens, so a mistake in it creates nothing and dials nothing.
    scenario = None
    if options.scenario is not None:
        if options.remote_host:
            raise UsageError("--scenario stages the scenario's own hosts; it does not take --remote-host")
        scenario = load_scenario(options.scenario, pwd.getpwuid(os.getuid()).pw_name)
    remote_hosts = parse_remote_hosts(scenario_remote_hosts(scenario) if scenario else options.remote_host)
    # An explicit --window-size wins over the scenario's viewport.
    window_size = options.window_size or (scenario_window_size(scenario) if scenario else None)
    # A staged run always has the private HOME and PATH and the scripted provisioning backend; nothing turns either off.
    staged = staged_start(options)
    # Before anything is created or dialed, so a refused combination leaves nothing behind.
    user_session = check_user_session(dict(os.environ), sys.platform, staged) if options.user_session else None
    mac = sys.platform == "darwin"
    if mac:
        # The window opens in the console's GUI session, so this must run as the user logged in there (SSH is fine:
        # a process of that user can open windows in that user's session, which the test Mac's earlier runs showed).
        owner = console_owner()
        if owner != os.getuid():
            raise UsageError(f"no GUI session of yours on this Mac (the console belongs to {user_name(owner)})")
        if options.size or options.scale:
            raise UsageError("--size and --scale set Mutter's virtual monitor on Linux; a Mac run uses the real screen")
    elif not sys.platform.startswith("linux"):
        raise UsageError("runs need Linux or macOS")
    else:
        missing = [path for path in (LAVAPIPE_ICD, MESA_EGL_VENDOR) if not Path(path).exists()]
        if missing:
            raise UsageError(f"the software renderer is not installed: {', '.join(missing)} missing")
        if window_size:
            # Mutter never grows its virtual monitor, and the app fits a first window to the display, so a window
            # larger than the monitor would open smaller than asked and be captured under the wrong size.
            monitor = options.size or DEFAULT_MONITOR
            (want_w, want_h), (have_w, have_h) = parse_size(window_size), parse_size(monitor)
            if want_w > have_w or want_h > have_h:
                asked = f"the scenario's window {window_size}"
                if options.window_size:
                    asked = f"--window-size {window_size}"
                raise UsageError(f"{asked} does not fit Mutter's {monitor} monitor; pass --size")
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
    missing = [path for path in binaries.values() if not Path(path).is_file()]
    if missing:
        # Before anything else starts (a staged run's sshd, Mutter), so `--no-build` on an empty target dir fails clean.
        raise UsageError(f"not built: {', '.join(missing)}; build them or drop --no-build. Run `stop` to clean up.")
    if staged:
        shadows = shadowing_agents(Path(binaries["farhelm"]).parent)
        if shadows:
            raise RuntimeError(
                f"{Path(binaries['farhelm']).parent} holds {', '.join(shadows)}, which a staged session would run "
                "instead of the run's wrappers; remove them. Run `stop` to clean up."
            )
        record["staging"] = prepare_staging(run, binaries["fixtures"])
        save()
        if scenario:
            # Before any supervisor starts, so the launcher's first folder check on any staged host finds them.
            create_scenario_dirs(Path(record["staging"]["home"]), scenario)
    if remote_hosts:
        # The staged hosts' sshd and the shim the helm dials them with ("The private sshd"), recorded before the wait so
        # `stop` finds an sshd that a failing or killed `start` left; then each destination is dialed once through the
        # shim, the way the helm will.
        def record_sshd(sshd: dict) -> None:
            record["sshd"] = sshd
            save()

        sshd = start_private_sshd(run, remote_hosts, Path(record["staging"]["bin"]), record_sshd)
        for destination in remote_hosts:
            check_ssh(destination, sshd["shim"])
    if not mac:
        display = display_name(uuid.uuid4().hex[:8])
        record.update({"runtime": str(run / "runtime"), "display": display})
        scale = options.scale or 1
        width, height = parse_size(options.size or DEFAULT_MONITOR)
        # Mutter's mode is in physical pixels. Keep the requested room for the window at either output scale.
        size = f"{width * scale}x{height * scale}"
        mutter = start_mutter(run, run / "runtime", display, mutter_command(display, size), dict(os.environ), scale=scale)
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
    session_pins = write_session_pins(run, scenario) if scenario else None
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
                window_size=window_size,
                appearance=options.appearance,
                scripted_hosts=scripted_hosts_dir(run) if options.scripted_hosts else None,
                start_dialogs=options.start_dialogs,
                payload_dir=options.payload_dir,
                ensure_hosts=ensure_hosts,
                session_pins=session_pins,
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
    if scenario:
        ids = stage_scenario(run, scenario, binaries["fixtures"], snapshot=lambda: window_entries(run, record))
        # What a capture step needs to find the staged sessions again: each one's helm key by its scenario title.
        record["scenario"] = {
            # Absolute, so `capture` reads the same file whatever directory it is run from.
            "file": str(options.scenario.resolve()),
            "sessions": {scenario["sessions"][index]["title"]: key for index, key in ids.items()},
            "open": ids[scenario_creation_order(scenario)[0]],
        }
        save()
        print(f"staged {len(scenario['hosts'])} hosts and {len(ids)} sessions from {options.scenario}; every session "
              "reached its status and every row shows its shield")
    kinds = [] if scenario else options.fleet or ["basic"] * (2 if options.sessions is None else options.sessions)
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
    """Take down everything `start` started: the app, its supervisor and terminal hosts, a staged run's private sshd,
    its remote supervisors and their terminal hosts, then Mutter and its bus (on Linux), and last the lock holder. The
    shared lock is released only when every earlier step succeeded: if part of the run may still be running, the lock is kept (the
    message says so) and a second `stop` retries everything.

    Each step runs even if an earlier one failed, and each acts only on processes it can tie to this run: the app
    through the pid and start time recorded after launch, the supervisor through the app's record of it plus the state
    directory on its command line, a staged run's private sshd through its recorded pid and start time plus its
    configuration on its command line (`stop_private_sshd`), a remote supervisor through its recorded pid and start
    time plus its state directory on its command line, Mutter through the pid and start time recorded when it started.
    A pid that has since been reused by another process is never signalled. A user-session run's containers and units
    on the caller's account are removed after the terminal hosts, by the names recorded from its helm before the app quit
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
    # Once the app (and with it the helm's ssh clients) is down, and before the remote supervisors: the sshd's
    # connections ran the `farhelm internal stdio` commands that joined them.
    if record.get("sshd"):
        problems += stop_private_sshd(record["sshd"])
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
        if verb == "capture":
            try:
                return capture(run, record, rest)
            except RuntimeError as error:
                print(f"gui-agent: capture: {error}", file=sys.stderr)
                return 1
        return cli(run, record, [verb, *rest]).returncode
    except UsageError as error:
        print(f"gui-agent: {error}", file=sys.stderr)
        return 2
    except FileNotFoundError as error:
        print(f"gui-agent: {error}; was `start` run for this directory?", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
