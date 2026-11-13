#!/usr/bin/env python3
"""Drive a real agent TUI (or any terminal program) by hand or from a script: start, read the screen, type, wait.

Testing how a real `claude`, `codex`, `goose` and so on behave in a terminal means running the installed program in a
terminal, reading what it draws, and typing at it. This is the one tool for that. Each run starts the program in a real
Farhelm terminal host, as a supervisor starts an agent: the supervisor's own emulator answers the program's startup
queries, the program gets the environment a supervisor gives an agent (`TERM_PROGRAM=farhelm` among it), and the screen
reads back as status detection reads it. Each run lives in a fresh random directory directly under `/tmp` (unix socket
paths are limited to about 104 bytes, and concurrent agents on this machine must never meet on a fixed name), so it
never touches a live Farhelm, its supervisor, or its sessions. It needs the built `farhelm` (`cargo build -p farhelm`;
`--farhelm` names another).

Every command after `start` names the run by the directory `start` printed:

    run=$(scripts/agent-tui.py start --cwd "$scratch" -- cursor-agent)
    scripts/agent-tui.py wait "$run" 'Trust|trust' --timeout 30   # prints the screen once it matches
    scripts/agent-tui.py settle "$run" --quiet 3                   # or: wait until the screen stops changing
    scripts/agent-tui.py screen "$run"                             # the visible grid, as Farhelm samples it
    scripts/agent-tui.py type "$run" 'hello'                       # literal text, not submitted
    scripts/agent-tui.py keys "$run" Enter                         # named keys: Enter, Escape, Down, C-c
    scripts/agent-tui.py stop "$run"                               # ends the program and removes the directory

The program starts with this process's environment minus `TMUX`, `TMUX_PANE`, every `FARHELM_*` variable, and the
markers an enclosing agent session sets for its children (`agent_screen_host.OUTER_SESSION_MARKERS`, such as
`CLAUDECODE`). Inside a Farhelm session or an agent's own session those would point the program under test at the
user's tmux, the live helm, or the agent running this tool; without them it behaves as it would when started fresh.

Read `.agents/real-agent-tui.md` before using this on real agents: it covers scratch directories, what a vendor records
when you answer its prompts, and how to wait on the screen instead of sleeping.

Every subcommand exits non-zero on failure, so a calling script can guard each step with `|| exit 1`.
"""

from __future__ import annotations

import argparse
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import time

from agent_screen_host import HostError, HostRun, default_farhelm

# The terminal size observed on the development fleet; scripts/capture-agent-screens.py uses the same one.
DEFAULT_SIZE = "133x42"
POLL_SECONDS = 0.5


def start(args: argparse.Namespace) -> int:
    if not args.command:
        sys.exit("agent-tui: start needs a command after --")
    if not re.fullmatch(r"\d+x\d+", args.size):
        sys.exit(f"agent-tui: --size takes COLSxROWS, got {args.size!r}")
    cols, rows = (int(part) for part in args.size.split("x"))
    cwd = pathlib.Path(args.cwd or os.getcwd()).resolve()
    if not cwd.is_dir():
        sys.exit(f"agent-tui: --cwd {cwd} is not a directory")
    run = HostRun.start(pathlib.Path(args.farhelm), cwd, cols, rows, args.command)
    print(run.dir)
    return 0


def screen(args: argparse.Namespace) -> int:
    text, title, _ = HostRun(pathlib.Path(args.run)).state()
    if args.title:
        print(title)
    sys.stdout.write(text)
    return 0


def type_text(args: argparse.Namespace) -> int:
    HostRun(pathlib.Path(args.run)).type(args.text)
    return 0


def keys(args: argparse.Namespace) -> int:
    HostRun(pathlib.Path(args.run)).keys(*args.keys)
    return 0


def wait(args: argparse.Namespace) -> int:
    """Poll the screen until the pattern matches; print that screen. An exited program or the deadline fails the wait,
    printing the last screen, since no later screen can be awaited from a program that exited."""

    run = HostRun(pathlib.Path(args.run))
    pattern = re.compile(args.pattern, re.MULTILINE)
    deadline = time.monotonic() + args.timeout
    while True:
        text, _, exited = run.state()
        if pattern.search(text):
            sys.stdout.write(text)
            return 0
        if exited:
            sys.stdout.write(text)
            print(f"agent-tui: the program exited before {args.pattern!r} appeared", file=sys.stderr)
            return 1
        if time.monotonic() >= deadline:
            sys.stdout.write(text)
            print(f"agent-tui: {args.pattern!r} did not appear within {args.timeout:g}s", file=sys.stderr)
            return 1
        time.sleep(POLL_SECONDS)


def settle(args: argparse.Namespace) -> int:
    """Wait until the screen has not changed for `--quiet` seconds; print it. For "what does it show at start?",
    where no particular text can be awaited. An exited program or the deadline fails, printing the last screen."""

    run = HostRun(pathlib.Path(args.run))
    deadline = time.monotonic() + args.timeout
    last = run.screen()
    since = time.monotonic()
    while True:
        time.sleep(POLL_SECONDS)
        text, _, exited = run.state()
        if text != last:
            last, since = text, time.monotonic()
        elif time.monotonic() - since >= args.quiet:
            sys.stdout.write(text)
            return 0
        if exited:
            sys.stdout.write(text)
            print("agent-tui: the program exited before its screen settled", file=sys.stderr)
            return 1
        if time.monotonic() >= deadline:
            sys.stdout.write(text)
            print(f"agent-tui: the screen kept changing for {args.timeout:g}s", file=sys.stderr)
            return 1


def stop(args: argparse.Namespace) -> int:
    """End an identified run; leave an unrecognized directory and its contents alone.

    HostRun's rejection must reach the CLI error handler: a missing run marker is no authority to delete a path.
    """

    HostRun(pathlib.Path(args.run)).stop()
    return 0


def self_test(args: argparse.Namespace) -> int:
    """Prove start, type, keys, wait, screen and stop against a small shell program instead of a real agent.

    It also proves the environment: a tool that leaked `FARHELM_*` into the program under test would let an agent
    reach the live helm of the session running the test, one that leaked `TMUX` or an outer agent session's markers
    would show agents behaving as they do in no Farhelm terminal, and one that did not give the production terminal's
    environment would too. Each of those is set in the tool's own environment here, so the checks fail when the scrub
    does not happen. It also proves that `stop` ends the program and its terminal host, not just the run directory,
    and refuses a non-run directory without removing its contents, including when an empty argument names the cwd.
    """

    script = pathlib.Path(__file__).resolve()
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="fh-tui-test-", dir="/tmp"))
    pid_file = scratch / "pid"
    env = dict(
        os.environ, FARHELM_SELF_TEST_LEAK="leaked", TMUX="/tmp/self-test-tmux,1,0", CLAUDE_CODE_CHILD_SESSION="1"
    )
    program = (
        f'echo $$ > {_shell_quote(str(pid_file))}; '
        'printf "ready leak=[%s] term=[%s] tmux=[%s] outer=[%s]\\n" "$FARHELM_SELF_TEST_LEAK" "$TERM_PROGRAM" '
        '"${TMUX-}" "${CLAUDE_CODE_CHILD_SESSION-}"; read line; echo "got [$line]"; sleep 30'
    )

    def cli(*argv: str) -> subprocess.CompletedProcess[str]:
        extra = ["--farhelm", args.farhelm] if argv[0] == "start" else []
        return subprocess.run(
            [sys.executable, str(script), argv[0], *extra, *argv[1:]], env=env, capture_output=True, text=True
        )

    run_dir = ""
    try:
        canary = scratch / "keep"
        canary.write_text("unrelated work")
        assert not (scratch / "farhelm").exists(), "premise: this directory is not a run"
        for non_run in (str(scratch), ".", ""):
            refused = subprocess.run(
                [sys.executable, str(script), "stop", non_run], cwd=scratch, capture_output=True, text=True
            )
            assert refused.returncode != 0, "stop must refuse a non-run directory"
            assert canary.read_text() == "unrelated work", "stop must preserve a non-run directory's contents"
        started = cli("start", "--cwd", str(scratch), "--", "sh", "-c", program)
        assert started.returncode == 0, started.stderr
        run_dir = started.stdout.strip()
        ready = cli("wait", run_dir, r"^ready", "--timeout", "10")
        assert ready.returncode == 0, ready.stderr
        assert "leak=[]" in ready.stdout, f"FARHELM_* reached the program: {ready.stdout!r}"
        assert "term=[farhelm]" in ready.stdout, f"not the production terminal's environment: {ready.stdout!r}"
        assert "tmux=[]" in ready.stdout, f"TMUX reached the program: {ready.stdout!r}"
        assert "outer=[]" in ready.stdout, f"an outer agent session's marker reached the program: {ready.stdout!r}"
        child = int(pid_file.read_text())
        host = _host_pids(run_dir)
        assert host or not pathlib.Path("/proc/self").exists(), "premise: the run's terminal host process is found"
        # The space between the words comes from the `Space` key, so that the key is checked too.
        assert cli("type", run_dir, "hello").returncode == 0
        assert cli("keys", run_dir, "Space").returncode == 0
        assert cli("type", run_dir, "there").returncode == 0
        assert cli("keys", run_dir, "Enter").returncode == 0
        got = cli("wait", run_dir, r"got \[hello there\]", "--timeout", "10")
        assert got.returncode == 0, got.stdout + got.stderr
        assert "got [hello there]" in cli("screen", run_dir).stdout
        settled = cli("settle", run_dir, "--quiet", "1", "--timeout", "10")
        assert settled.returncode == 0 and "got [hello there]" in settled.stdout, settled.stdout + settled.stderr
        missed = cli("wait", run_dir, "never appears", "--timeout", "1")
        assert missed.returncode == 1, "a wait that never matches must fail"
        refused = cli("keys", run_dir, "NoSuchKey")
        assert refused.returncode != 0, "an unknown key name must fail"
        assert cli("stop", run_dir).returncode == 0
        assert not pathlib.Path(run_dir).exists(), "stop must remove the run directory"
        for pid, what in [(child, "the program"), *((pid, "the terminal host") for pid in host)]:
            assert _gone_within(pid, 10), f"{what} (pid {pid}) still runs after stop"
    finally:
        if run_dir and pathlib.Path(run_dir).exists():
            cli("stop", run_dir)
        shutil.rmtree(scratch, ignore_errors=True)
    print("agent-tui: self-test passed")
    return 0


def _shell_quote(part: str) -> str:
    return "'" + part.replace("'", "'\\''") + "'"


def _host_pids(run_dir: str) -> list[int]:
    """The processes whose command line names a socket in `run_dir`: the run's terminal host. Empty where `/proc` is
    missing (macOS), which leaves the host-lifetime check to Linux."""

    proc = pathlib.Path("/proc")
    if not (proc / "self").exists():
        return []
    socket_dir = (run_dir.rstrip("/") + "/hosts/").encode()
    pids = []
    for entry in proc.iterdir():
        if not entry.name.isdigit():
            continue
        try:
            argv = (entry / "cmdline").read_bytes().split(b"\0")
        except OSError:
            continue
        if b"terminal-host" in argv and any(arg.startswith(socket_dir) for arg in argv):
            pids.append(int(entry.name))
    return pids


def _gone_within(pid: int, seconds: float) -> bool:
    """Whether no process `pid` runs within `seconds` (a zombie counts as gone: it runs nothing)."""

    deadline = time.monotonic() + seconds
    while True:
        try:
            os.kill(pid, 0)
            state = pathlib.Path(f"/proc/{pid}/stat")
            if state.exists() and state.read_text().rsplit(")", 1)[-1].split()[0] == "Z":
                return True
        except ProcessLookupError:
            return True
        except OSError:
            pass
        if time.monotonic() >= deadline:
            return False
        time.sleep(0.1)


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("start", help="start a command in a terminal host; prints the run directory")
    p.add_argument("--cwd", help="directory to start in (default: the current one; prefer a fresh scratch directory)")
    p.add_argument("--size", default=DEFAULT_SIZE, help=f"terminal size as COLSxROWS (default {DEFAULT_SIZE})")
    p.add_argument("--farhelm", default=str(default_farhelm()), help="the built farhelm (default: %(default)s)")
    p.add_argument("command", nargs=argparse.REMAINDER, help="-- then the command and its arguments")
    p.set_defaults(func=start)

    p = sub.add_parser("screen", help="print the visible screen")
    p.add_argument("run")
    p.add_argument("--title", action="store_true", help="print the terminal's title first")
    p.set_defaults(func=screen)

    p = sub.add_parser("type", help="type literal text, without pressing Enter")
    p.add_argument("run")
    p.add_argument("text")
    p.set_defaults(func=type_text)

    p = sub.add_parser(
        "keys",
        help="send named keys (tmux's names: Enter, Escape, Tab, BTab, BSpace, Space, Up, Down, Left, Right, Home, "
        "End, PageUp, PageDown, Delete, S-Enter, F1-F12, C-a to C-z)",
    )
    p.add_argument("run")
    p.add_argument("keys", nargs="+")
    p.set_defaults(func=keys)

    p = sub.add_parser("wait", help="wait until a regex matches the screen; print that screen")
    p.add_argument("run")
    p.add_argument("pattern")
    p.add_argument("--timeout", type=float, default=30)
    p.set_defaults(func=wait)

    p = sub.add_parser("settle", help="wait until the screen stops changing; print it")
    p.add_argument("run")
    p.add_argument("--quiet", type=float, default=3, help="seconds without a change (default 3)")
    p.add_argument("--timeout", type=float, default=60)
    p.set_defaults(func=settle)

    p = sub.add_parser("stop", help="end the program and its terminal host and remove the run directory")
    p.add_argument("run")
    p.set_defaults(func=stop)

    p = sub.add_parser("self-test", help="check the tool against a small shell program")
    p.add_argument("--farhelm", default=str(default_farhelm()), help="the built farhelm (default: %(default)s)")
    p.set_defaults(func=self_test)

    args = parser.parse_args(argv)
    if args.cmd == "start" and args.command and args.command[0] == "--":
        args.command = args.command[1:]
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        return args.func(args)
    except HostError as error:
        print(f"agent-tui: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
