#!/usr/bin/env python3
"""Drive a real agent TUI (or any terminal program) by hand or from a script: start, read the screen, type, wait.

Testing how a real `claude`, `codex`, `goose` and so on behave in a terminal means running the installed program in a
pane, reading what it draws, and typing at it. This is the one tool for that. Each run gets its own tmux server on a
socket in a fresh random directory directly under `/tmp` (unix socket paths are limited to about 108 bytes, and
concurrent agents on this machine must never meet on a fixed name), so it never touches a live Farhelm, its tmux
server, or its sessions. The server uses settings close to the ones a Farhelm supervisor gives its panes, so agents
render as they would there, but matching Farhelm exactly is not the goal: the goal is to see what the agent does.

Every command after `start` names the run by the directory `start` printed:

    run=$(scripts/agent-tui.py start --cwd "$scratch" -- cursor-agent)
    scripts/agent-tui.py wait "$run" 'Trust|trust' --timeout 30   # prints the screen once it matches
    scripts/agent-tui.py settle "$run" --quiet 3                   # or: wait until the screen stops changing
    scripts/agent-tui.py screen "$run"                             # the visible grid, as Farhelm samples it
    scripts/agent-tui.py type "$run" 'hello'                       # literal text, not submitted
    scripts/agent-tui.py keys "$run" Enter                         # named keys: Enter, Escape, Down, C-c
    scripts/agent-tui.py stop "$run"                               # kills the server and removes the directory

The program starts with this process's environment minus `TMUX`, `TMUX_PANE`, and every `FARHELM_*` variable. Inside
a Farhelm session those would point the agent under test at the outer tmux server or the live helm; without them it
behaves as it would when started fresh.

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

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
PINNED_TMUX = REPO_ROOT / ".ci-tmux/tmux"

# Close to a Farhelm supervisor's pane settings, so agents render as they do under Farhelm: no status line or prefix
# key in the way, no escape delay, true colour, focus events (Claude nags on its idle screen without them), and a dead
# pane kept on screen so a crash can still be read.
TMUX_CONFIG = """\
set -s exit-empty off
set -s escape-time 0
set -s default-terminal 'xterm-256color'
set -as terminal-features ',xterm-256color:RGB'
set -s focus-events on
set -g status off
set -g prefix None
set -g history-limit 12000
setw -g remain-on-exit on
"""

# The pane size observed on the development fleet; scripts/capture-agent-screens.py uses the same one.
DEFAULT_SIZE = "133x42"
TARGET = "agent"
POLL_SECONDS = 0.5


def agent_env() -> dict[str, str]:
    """This environment minus anything that would tie the program to an outer tmux or a Farhelm session."""

    return {
        key: value
        for key, value in os.environ.items()
        if key not in ("TMUX", "TMUX_PANE") and not key.startswith("FARHELM_")
    }


def tmux_binary(explicit: str | None) -> str:
    """The tmux to run: an explicit one, else the repo's pinned build when present, else whatever is on PATH."""

    if explicit:
        return explicit
    if PINNED_TMUX.exists():
        return str(PINNED_TMUX)
    found = shutil.which("tmux")
    if found is None:
        sys.exit("agent-tui: no tmux found (build .ci-tmux with scripts/build-pinned-tmux-ci.sh, or pass --tmux)")
    return found


class Run:
    """One started program, addressed by its run directory, which holds the socket, the config, and the tmux path."""

    def __init__(self, directory: pathlib.Path) -> None:
        self.dir = directory
        tmux_file = directory / "tmux"
        if not tmux_file.exists():
            sys.exit(f"agent-tui: {directory} is not a run directory from `agent-tui.py start`")
        self.base = [tmux_file.read_text().strip(), "-S", str(directory / "s"), "-f", str(directory / "tmux.conf")]

    def tmux(self, *args: str, check: bool = True) -> str:
        done = subprocess.run(
            [*self.base, *args], env=agent_env(), capture_output=True, text=True, timeout=30, check=False
        )
        if check and done.returncode != 0:
            raise SystemExit(f"agent-tui: tmux {' '.join(args)} failed: {done.stderr.strip()}")
        return done.stdout

    def screen(self) -> str:
        """The visible grid, without scrollback: what a Farhelm supervisor's sampler sees."""

        return self.tmux("capture-pane", "-p", "-t", TARGET)

    def title(self) -> str:
        return self.tmux("display-message", "-p", "-t", TARGET, "#{pane_title}").rstrip("\n")

    def dead(self) -> bool:
        return self.tmux("display-message", "-p", "-t", TARGET, "#{pane_dead}").strip() == "1"


def start(args: argparse.Namespace) -> int:
    if not args.command:
        sys.exit("agent-tui: start needs a command after --")
    if not re.fullmatch(r"\d+x\d+", args.size):
        sys.exit(f"agent-tui: --size takes COLSxROWS, got {args.size!r}")
    cols, rows = args.size.split("x")
    cwd = pathlib.Path(args.cwd or os.getcwd()).resolve()
    if not cwd.is_dir():
        sys.exit(f"agent-tui: --cwd {cwd} is not a directory")
    directory = pathlib.Path(tempfile.mkdtemp(prefix="fh-tui-", dir="/tmp"))
    (directory / "tmux.conf").write_text(TMUX_CONFIG)
    (directory / "tmux").write_text(tmux_binary(args.tmux) + "\n")
    run = Run(directory)
    # One shell word per argument, so the pane runs exactly the argv given rather than a re-split string.
    command = " ".join(_shell_quote(part) for part in args.command)
    run.tmux("new-session", "-d", "-s", TARGET, "-x", cols, "-y", rows, "-c", str(cwd), command)
    print(directory)
    return 0


def _shell_quote(part: str) -> str:
    return "'" + part.replace("'", "'\\''") + "'"


def screen(args: argparse.Namespace) -> int:
    run = Run(pathlib.Path(args.run))
    if args.title:
        print(run.title())
    sys.stdout.write(run.screen())
    return 0


def type_text(args: argparse.Namespace) -> int:
    Run(pathlib.Path(args.run)).tmux("send-keys", "-t", TARGET, "-l", args.text)
    return 0


def keys(args: argparse.Namespace) -> int:
    Run(pathlib.Path(args.run)).tmux("send-keys", "-t", TARGET, *args.keys)
    return 0


def wait(args: argparse.Namespace) -> int:
    """Poll the screen until the pattern matches; print that screen. A dead program or the deadline fails the wait,
    printing the last screen, since no later screen can be awaited from a program that exited."""

    run = Run(pathlib.Path(args.run))
    pattern = re.compile(args.pattern, re.MULTILINE)
    deadline = time.monotonic() + args.timeout
    while True:
        text = run.screen()
        if pattern.search(text):
            sys.stdout.write(text)
            return 0
        if run.dead():
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
    where no particular text can be awaited. A dead program or the deadline fails, printing the last screen."""

    run = Run(pathlib.Path(args.run))
    deadline = time.monotonic() + args.timeout
    last = run.screen()
    since = time.monotonic()
    while True:
        time.sleep(POLL_SECONDS)
        text = run.screen()
        if text != last:
            last, since = text, time.monotonic()
        elif time.monotonic() - since >= args.quiet:
            sys.stdout.write(text)
            return 0
        if run.dead():
            sys.stdout.write(text)
            print("agent-tui: the program exited before its screen settled", file=sys.stderr)
            return 1
        if time.monotonic() >= deadline:
            sys.stdout.write(text)
            print(f"agent-tui: the screen kept changing for {args.timeout:g}s", file=sys.stderr)
            return 1


def stop(args: argparse.Namespace) -> int:
    directory = pathlib.Path(args.run)
    Run(directory).tmux("kill-server", check=False)
    shutil.rmtree(directory, ignore_errors=True)
    return 0


def self_test(args: argparse.Namespace) -> int:
    """Prove start, type, keys, wait, screen and stop against a small shell program instead of a real agent.

    It also proves the environment scrub, since a tool that leaked `FARHELM_*` into the program under test would let
    an agent reach the live helm of the session running the test.
    """

    script = pathlib.Path(__file__).resolve()
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="fh-tui-test-", dir="/tmp"))
    env = dict(os.environ, FARHELM_SELF_TEST_LEAK="leaked")
    program = 'printf "ready leak=[%s]\\n" "$FARHELM_SELF_TEST_LEAK"; read line; echo "got [$line]"; sleep 30'

    def cli(*argv: str) -> subprocess.CompletedProcess[str]:
        extra = ["--tmux", args.tmux] if args.tmux and argv[0] == "start" else []
        return subprocess.run(
            [sys.executable, str(script), argv[0], *extra, *argv[1:]], env=env, capture_output=True, text=True
        )

    run_dir = ""
    try:
        started = cli("start", "--cwd", str(scratch), "--", "sh", "-c", program)
        assert started.returncode == 0, started.stderr
        run_dir = started.stdout.strip()
        ready = cli("wait", run_dir, r"^ready", "--timeout", "10")
        assert ready.returncode == 0, ready.stderr
        assert "leak=[]" in ready.stdout, f"FARHELM_* reached the program: {ready.stdout!r}"
        assert cli("type", run_dir, "hello there").returncode == 0
        assert cli("keys", run_dir, "Enter").returncode == 0
        got = cli("wait", run_dir, r"got \[hello there\]", "--timeout", "10")
        assert got.returncode == 0, got.stdout + got.stderr
        assert "got [hello there]" in cli("screen", run_dir).stdout
        settled = cli("settle", run_dir, "--quiet", "1", "--timeout", "10")
        assert settled.returncode == 0 and "got [hello there]" in settled.stdout, settled.stdout + settled.stderr
        missed = cli("wait", run_dir, "never appears", "--timeout", "1")
        assert missed.returncode == 1, "a wait that never matches must fail"
        assert cli("stop", run_dir).returncode == 0
        assert not pathlib.Path(run_dir).exists(), "stop must remove the run directory"
    finally:
        if run_dir and pathlib.Path(run_dir).exists():
            cli("stop", run_dir)
        shutil.rmtree(scratch, ignore_errors=True)
    print("agent-tui: self-test passed")
    return 0


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("start", help="start a command in a private tmux server; prints the run directory")
    p.add_argument("--cwd", help="directory to start in (default: the current one; prefer a fresh scratch directory)")
    p.add_argument("--size", default=DEFAULT_SIZE, help=f"pane size as COLSxROWS (default {DEFAULT_SIZE})")
    p.add_argument("--tmux", help="tmux binary (default: .ci-tmux/tmux if built, else tmux on PATH)")
    p.add_argument("command", nargs=argparse.REMAINDER, help="-- then the command and its arguments")
    p.set_defaults(func=start)

    p = sub.add_parser("screen", help="print the visible screen")
    p.add_argument("run")
    p.add_argument("--title", action="store_true", help="print the pane title first")
    p.set_defaults(func=screen)

    p = sub.add_parser("type", help="type literal text, without pressing Enter")
    p.add_argument("run")
    p.add_argument("text")
    p.set_defaults(func=type_text)

    p = sub.add_parser("keys", help="send named keys (tmux send-keys names: Enter, Escape, Up, C-c, ...)")
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

    p = sub.add_parser("stop", help="kill the program's tmux server and remove its run directory")
    p.add_argument("run")
    p.set_defaults(func=stop)

    p = sub.add_parser("self-test", help="check the tool against a small shell program")
    p.add_argument("--tmux", help="tmux binary to test with")
    p.set_defaults(func=self_test)

    args = parser.parse_args(argv)
    if args.cmd == "start" and args.command and args.command[0] == "--":
        args.command = args.command[1:]
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
