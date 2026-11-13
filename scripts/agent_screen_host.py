"""Run one program in a real Farhelm terminal host and read, type and press keys at it.

The shared backend of `scripts/agent-tui.py` and `scripts/capture-agent-screens.py`. Both exist to see what a real,
installed agent draws in a Farhelm terminal, so the program runs the way a supervisor runs an agent: in a terminal host
started from the built `farhelm`, with the supervisor's own emulator answering the program's startup queries and the
environment a supervisor gives an agent (`TERM_PROGRAM=farhelm` and the rest). The Rust side is the hidden
`farhelm internal agent-screen` command (`crates/farhelm-supervisor/src/agent_screen.rs`, whose module docs describe
the control socket this talks to).

Each run lives in its own fresh directory directly under `/tmp`, holding the host's socket, the control socket, the
command's log and its pid: unix socket paths are limited to about 104 bytes, and concurrent runs on a shared machine
must never meet on a fixed name. Nothing of a live Farhelm is involved, and the program gets no `FARHELM_*`, `TMUX` or
`TMUX_PANE` variable of this process's, nor an enclosing agent session's markers (`program_env`): inside a Farhelm
session those would point the program under test at the live helm or at a tmux the caller runs in.
"""

from __future__ import annotations

import json
import os
import pathlib
import select
import shutil
import socket
import subprocess
import tempfile
import time

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

# How long `farhelm internal agent-screen` may take to start the host and adopt it. A loaded machine can take seconds.
START_SECONDS = 60
# How long one control request may take. Input waits until the host has written it to the program's terminal, which a
# program that stops reading its terminal can hold up.
REQUEST_SECONDS = 30
# How long `stop` waits for the command to shut its host down before falling back on the state directory's stop.
STOP_SECONDS = 20


class HostError(Exception):
    """The run failed: the command did not start, or a request was refused or got no answer."""


def default_farhelm() -> pathlib.Path:
    """The `farhelm` a plain `cargo build -p farhelm` of this checkout makes, honouring `CARGO_TARGET_DIR`."""

    target = os.environ.get("CARGO_TARGET_DIR")
    root = pathlib.Path(target) if target else REPO_ROOT / "target"
    return root / "debug" / "farhelm"


# Variables an enclosing agent session sets for its own children. This tool is usually run by an agent, and a program
# under test that inherited them would believe it runs below that session: Claude Code 2.1.319 turns off its
# transcript saving and says so on screen when it sees `CLAUDE_CODE_CHILD_SESSION`. A supervisor started as a service
# has none of them. Named one by one rather than by prefix, because the same prefixes also carry a user's own
# configuration and credentials (`CLAUDE_CODE_OAUTH_TOKEN`, `CODEX_HOME`), which must stay.
OUTER_SESSION_MARKERS = (
    "CLAUDECODE", "CLAUDE_CODE_CHILD_SESSION", "CLAUDE_CODE_ENTRYPOINT", "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_MESSAGING_SOCKET", "CLAUDE_CODE_MESSAGING_TOKEN", "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_SESSION_ID", "CLAUDE_EFFORT", "CLAUDE_PID", "CODEX_SANDBOX", "CODEX_SANDBOX_NETWORK_DISABLED",
    "CODEX_THREAD_ID",
)


def program_env() -> dict[str, str]:
    """This environment minus anything that would tie the program to an outer tmux, a Farhelm session, or the agent
    session running this tool."""

    return {
        key: value
        for key, value in os.environ.items()
        if key not in ("TMUX", "TMUX_PANE", *OUTER_SESSION_MARKERS) and not key.startswith("FARHELM_")
    }


class HostRun:
    """One started program, addressed by its run directory.

    The directory records the `farhelm` it was started with and the command's pid, so that a later process (each
    `agent-tui.py` subcommand is one) can talk to it and stop it.
    """

    def __init__(self, directory: pathlib.Path, process: subprocess.Popen[bytes] | None = None) -> None:
        # Absolute but not resolved, so that it matches the `--run-dir` the command was started with, which `stop`
        # looks for on its command line (resolving would turn macOS's `/tmp` into `/private/tmp`).
        self.dir = directory.absolute()
        self.process = process
        farhelm_file = directory / "farhelm"
        if not farhelm_file.exists():
            raise HostError(f"{directory} is not a run directory of a started program")
        self.farhelm = farhelm_file.read_text().strip()

    @classmethod
    def start(
        cls, farhelm: pathlib.Path, cwd: pathlib.Path, cols: int, rows: int, argv: list[str]
    ) -> HostRun:
        """Start `argv` in `cwd` in a terminal host of `cols` x `rows`, and return once the program runs.

        On failure nothing is left running and the run directory is gone; the error carries the command's log.
        """

        if not farhelm.is_file():
            raise HostError(f"{farhelm} is missing; build it with `cargo build -p farhelm`, or name one with --farhelm")
        directory = pathlib.Path(tempfile.mkdtemp(prefix="fh-tui-", dir="/tmp"))
        (directory / "farhelm").write_text(str(farhelm.resolve()) + "\n")
        log = directory / "log"
        command = [
            str(farhelm), "internal", "agent-screen", "--run-dir", str(directory), "--cwd", str(cwd.resolve()),
            "--size", f"{cols}x{rows}", "--", *argv,
        ]
        with log.open("wb") as log_file:
            # Its own session, so that a terminal's Ctrl+C or a hangup of the caller's terminal does not reach it: the
            # run outlives the `start` that made it.
            process = subprocess.Popen(
                command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=log_file, env=program_env(),
                start_new_session=True,
            )
        assert process.stdout is not None
        ready, _, _ = select.select([process.stdout], [], [], START_SECONDS)
        line = process.stdout.readline() if ready else b""
        process.stdout.close()
        run = cls(directory, process)
        if line.strip() != b"ready":
            # Read the log before `stop` removes the directory it is in, once the command had a moment to finish
            # writing it: a refused argument or an older `farhelm` without this command exits at once.
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass
            detail = log.read_text(errors="replace").strip() if log.exists() else ""
            run.stop()
            raise HostError(f"farhelm internal agent-screen did not start: {detail or 'no output'}")
        (directory / "pid").write_text(f"{process.pid}\n")
        return run

    def request(self, payload: dict[str, object]) -> dict[str, object]:
        """Send one request on the control socket and return its answer, raising `HostError` on a refusal."""

        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as conn:
            conn.settimeout(REQUEST_SECONDS)
            try:
                conn.connect(str(self.dir / "control"))
                conn.sendall(json.dumps(payload).encode() + b"\n")
                chunks = []
                while not chunks or not chunks[-1].endswith(b"\n"):
                    chunk = conn.recv(1 << 20)
                    if not chunk:
                        break
                    chunks.append(chunk)
            except OSError as error:
                raise HostError(f"the run does not answer ({error}); is it stopped?") from error
        try:
            answer = json.loads(b"".join(chunks))
        except ValueError as error:
            raise HostError(f"the run gave no answer to {payload['op']}") from error
        if not answer.get("ok"):
            raise HostError(f"{payload['op']} refused: {answer.get('error')}")
        return answer

    def state(self) -> tuple[str, str, bool]:
        """The visible screen as status detection reads it, the title, and whether the program has exited."""

        answer = self.request({"op": "screen"})
        return str(answer["screen"]), str(answer["title"]), bool(answer["exited"])

    def screen(self) -> str:
        return self.state()[0]

    def title(self) -> str:
        return self.state()[1]

    def exited(self) -> bool:
        return self.state()[2]

    def type(self, text: str) -> None:
        """Type literal text, in one write, without submitting it."""

        self.request({"op": "type", "text": text})

    def keys(self, *names: str) -> None:
        """Press named keys (`Enter`, `Escape`, `Down`, `C-u`; the full list is `agent_screen::key_input`'s)."""

        self.request({"op": "keys", "keys": list(names)})

    def stop(self) -> None:
        """End the program and its host, and remove the run directory. Safe to call on a run that already ended.

        The command shuts its host down on `stop`. A command that is gone or does not finish in time may have left its
        host running, so the state directory's own stop (`farhelm internal stop-terminal-hosts`) runs either way, and
        the directory is removed whatever happened before.

        Only a command known to be this run's is ever signalled, because agents on this machine share one unix user
        and a pid can be reused: this process's own child (which cannot be reused before it is reaped), or a pid from
        the run's pid file that just answered the stop request and whose command line still names the run directory.
        A command that did not answer is taken as gone and left to the state directory's stop.
        """

        try:
            self.request({"op": "stop"})
            answered = True
        except HostError:
            answered = False
        if self.process is not None:
            try:
                self.process.wait(timeout=STOP_SECONDS)
            except subprocess.TimeoutExpired:
                self.process.kill()
                try:
                    self.process.wait(timeout=STOP_SECONDS)
                except subprocess.TimeoutExpired:
                    pass
        elif answered:
            pid = self._pid()
            deadline = time.monotonic() + STOP_SECONDS
            while pid is not None and self._is_ours(pid):
                if time.monotonic() >= deadline:
                    try:
                        os.kill(pid, 9)
                    except OSError:
                        pass
                    break
                time.sleep(0.1)
        try:
            subprocess.run(
                [self.farhelm, "internal", "stop-terminal-hosts", "--state-dir", str(self.dir)],
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=program_env(),
                timeout=60, check=False,
            )
        except (OSError, subprocess.TimeoutExpired):
            pass
        shutil.rmtree(self.dir, ignore_errors=True)

    def _is_ours(self, pid: int) -> bool:
        """Whether `pid` still runs this run's command: it exists and, where `/proc` can say (Linux), its command line
        names the run directory. On a system without `/proc` the existence check is all there is."""

        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return False
        except PermissionError:
            # Another user's process: certainly not this run's command.
            return False
        cmdline = pathlib.Path(f"/proc/{pid}/cmdline")
        if not pathlib.Path("/proc/self").exists():
            return True
        try:
            return str(self.dir).encode() in cmdline.read_bytes().split(b"\0")
        except OSError:
            return False

    def _pid(self) -> int | None:
        if self.process is not None:
            return self.process.pid
        try:
            return int((self.dir / "pid").read_text())
        except (OSError, ValueError):
            return None
