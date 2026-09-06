#!/usr/bin/env python3
"""Run one command while retaining bounded, private evidence about the run.

This recorder is intentionally a wrapper, not a test runner. It does not retry,
schedule work, interpret test counts, or turn descriptive labels into claims
about what the command exercised.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import locale
import math
import os
import pathlib
import platform
import queue
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import threading
import time
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import BinaryIO, Callable, Iterable

# Importing a repository-local helper must not create bytecode before source
# fingerprinting. Restore the caller's setting after this one import so loading
# the recorder as a module does not change unrelated import behavior.
_previous_bytecode_setting = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    import test_run_traces
finally:
    sys.dont_write_bytecode = _previous_bytecode_setting
    del _previous_bytecode_setting


SCHEMA_VERSION = 1
PROBE_TIMEOUT_SECONDS = 5.0
PROBE_SAMPLE_LIMIT = 64 * 1024
PROBE_KILL_GRACE_SECONDS = 0.5
CHUNK_SIZE = 1024 * 1024
TAIL_CHUNK_COUNT = 7
OUTPUT_LIMIT = CHUNK_SIZE * (1 + TAIL_CHUNK_COUNT)
CHILD_KILL_GRACE_SECONDS = 2.0
POST_KILL_DRAIN_SECONDS = 0.5
SOURCE_ENTRY_LIMIT = 100_000
SOURCE_PROBLEM_LIMIT = 32
SOURCE_PATH_BUFFER_LIMIT = 1024 * 1024
CONSOLE_QUEUE_CHUNKS = 64
CONSOLE_FINAL_DRAIN_SECONDS = 0.25


# Request validation and bounded diagnostic delivery.


class UsageRefusal(Exception):
    """An invalid request that must not spawn the requested command."""


class RecorderFailure(Exception):
    """An internal evidence or lifecycle failure that invalidates the run."""


class WaitOwnershipLost(RecorderFailure):
    """Another waiter consumed the child's status; numeric signals and inferred exits are unsafe."""


def require_wait_ownership() -> None:
    """Refuse unsupported interpreters before even a metadata probe can leave a child behind."""

    required = ("waitid", "P_PID", "WEXITED", "WNOHANG", "WNOWAIT", "CLD_EXITED")
    if any(not hasattr(os, name) for name in required):
        raise UsageRefusal("Python must provide os.waitid with WNOWAIT; macOS requires Python 3.13 or newer")


class RefusingArgumentParser(argparse.ArgumentParser):
    """Report CLI mistakes through the recorder's refusal path when possible."""

    def error(self, message: str) -> None:
        raise UsageRefusal(message)


@dataclass
class SignalIntent:
    """Capture signal-handler intent without doing cleanup in the handler."""

    received: int | None = None

    def handle(self, signum: int, _frame: object) -> None:
        if self.received is None:
            self.received = signum


@dataclass
class ProbeResult:
    """Bounded evidence from one metadata subprocess."""

    argv: list[str]
    returncode: int | None
    timed_out: bool
    interrupted: bool
    stdout_bytes: int
    stdout_sha256: str
    stdout_sample: bytes
    stdout_sample_truncated: bool
    stderr_bytes: int
    stderr_sha256: str
    stderr_sample: bytes
    stderr_sample_truncated: bool
    duration_seconds: float
    spawn_error: str | None = None
    pipe_drain_truncated: bool = False
    lifecycle_error: str | None = None

    @property
    def complete(self) -> bool:
        return (
            self.spawn_error is None
            and self.lifecycle_error is None
            and not self.timed_out
            and not self.interrupted
            and not self.pipe_drain_truncated
            and self.returncode == 0
        )

    def evidence(self, include_stdout_sample: bool = True) -> dict[str, object]:
        """Return JSON-safe probe evidence without decoding arbitrary bytes."""

        result: dict[str, object] = {
            "argv": self.argv,
            "returncode": self.returncode,
            "timed_out": self.timed_out,
            "interrupted": self.interrupted,
            "duration_seconds": self.duration_seconds,
            "pipe_drain_truncated": self.pipe_drain_truncated,
            "stdout": {
                "bytes": self.stdout_bytes,
                "sha256": self.stdout_sha256,
                "sample_truncated": self.stdout_sample_truncated,
            },
            "stderr": {
                "bytes": self.stderr_bytes,
                "sha256": self.stderr_sha256,
                "sample_base64": base64.b64encode(self.stderr_sample).decode("ascii"),
                "sample_truncated": self.stderr_sample_truncated,
            },
        }
        if include_stdout_sample:
            stdout = result["stdout"]
            assert isinstance(stdout, dict)
            stdout["sample_base64"] = base64.b64encode(self.stdout_sample).decode("ascii")
        if self.spawn_error is not None:
            result["spawn_error"] = self.spawn_error
        if self.lifecycle_error is not None:
            result["lifecycle_error"] = self.lifecycle_error
        return result


class StreamCapture:
    """Hash a stream in full while retaining only a bounded prefix."""

    def __init__(self, sample_limit: int = PROBE_SAMPLE_LIMIT) -> None:
        self.byte_count = 0
        self.digest = hashlib.sha256()
        self.sample = bytearray()
        self.sample_limit = sample_limit

    def add(self, data: bytes) -> None:
        self.byte_count += len(data)
        self.digest.update(data)
        remaining = self.sample_limit - len(self.sample)
        if remaining > 0:
            self.sample.extend(data[:remaining])

    @property
    def truncated(self) -> bool:
        return self.byte_count > len(self.sample)


def utc_now() -> str:
    """Return a schema timestamp in UTC RFC 3339 form."""

    return datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def finite_positive(value: str) -> float:
    """Parse a finite positive duration for argparse."""

    try:
        parsed = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a number") from error
    if not math.isfinite(parsed) or parsed <= 0:
        raise argparse.ArgumentTypeError("must be finite and greater than zero")
    return parsed


def farhelm_name(value: str) -> str:
    """Accept only explicit FARHELM_ environment variable names."""

    if not value.startswith("FARHELM_") or "=" in value or not value:
        raise argparse.ArgumentTypeError("must be a FARHELM_ variable name without a value")
    return value


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Parse recorder options and require an explicit `--` command boundary."""

    parser = RefusingArgumentParser(description=__doc__, add_help=True)
    parser.add_argument("--kind", required=True, choices=("development", "repetition", "release"))
    parser.add_argument("--selection", required=True)
    parser.add_argument("--concurrency", required=True)
    parser.add_argument("--tmux", choices=("warn", "required", "none"), default="warn")
    parser.add_argument("--timeout", type=finite_positive)
    parser.add_argument("--output-root", type=pathlib.Path)
    parser.add_argument("--keep-farhelm-env", action="append", default=[], type=farhelm_name)
    if "--" in argv:
        boundary = argv.index("--")
        option_argv = argv[:boundary]
        command = argv[boundary + 1 :]
    else:
        option_argv = argv
        command = []
    parsed = parser.parse_args(option_argv)
    if "--" not in argv:
        raise UsageRefusal("missing `--` before the command argv")
    if not command:
        raise UsageRefusal("no command argv follows `--`")
    parsed.command = command
    return parsed


def default_output_root(environment: dict[str, str]) -> pathlib.Path:
    """Choose a test-evidence state tree separate from Farhelm product state."""

    state_home = environment.get("XDG_STATE_HOME")
    if state_home:
        return pathlib.Path(state_home) / "farhelm-test-runs"
    return pathlib.Path.home() / ".local" / "state" / "farhelm-test-runs"


def resolved_future_path(path: pathlib.Path) -> pathlib.Path:
    """Resolve symlinks in existing ancestors of a path that may not exist yet."""

    return path.expanduser().resolve(strict=False)


def is_within(path: pathlib.Path, parent: pathlib.Path) -> bool:
    """Return whether a resolved path is equal to or below a resolved parent."""

    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def private_mode(path: pathlib.Path) -> bool:
    """Treat a directory as private when group and other have no permissions."""

    return stat.S_IMODE(path.stat().st_mode) & 0o077 == 0


def best_effort_write(fd: int, data: bytes, timeout: float = 0.1) -> int:
    """Write a short diagnostic without changing shared descriptor flags."""

    if not data:
        return 0
    result = {"written": 0}

    def deliver() -> None:
        view = memoryview(data)
        while view:
            try:
                count = os.write(fd, view)
            except (InterruptedError, BlockingIOError):
                continue
            except OSError:
                return
            if count <= 0:
                return
            result["written"] += count
            view = view[count:]

    writer = threading.Thread(target=deliver, name="recorder-diagnostic", daemon=True)
    writer.start()
    writer.join(timeout)
    return result["written"]


def observe_exit(process: subprocess.Popen[bytes]) -> int | None:
    """Observe a child exit without releasing ownership of its numeric group ID.

    Popen.poll reaps the leader. After that, even an existence probe cannot
    distinguish surviving descendants from an unrelated recycled process group.
    Both lifecycle loops keep the leader waitable until their final group
    signal, then consume the real status through reap_owned_child. This assumes
    exclusive ownership of waiting for the child; an unexpected ECHILD refuses
    further group signals rather than guessing which process now owns the ID.
    """

    if process.returncode is not None:
        return process.returncode
    try:
        status = os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    except InterruptedError:
        return None
    except ChildProcessError as error:
        raise WaitOwnershipLost("child wait ownership was lost; refusing process-group signals") from error
    if status is None:
        return None
    if status.si_code == os.CLD_EXITED:
        return status.si_status
    return -status.si_status


def terminate_group(process: subprocess.Popen[bytes], signum: int) -> bool:
    """Signal only while the unreaped direct child still pins the owned group ID."""

    if process.returncode is not None:
        return False
    observe_exit(process)  # ECHILD must prevent even a signal-zero existence probe.

    try:
        os.killpg(process.pid, signum)
        return True
    except ProcessLookupError:
        return False
    except PermissionError as error:
        raise RecorderFailure(f"could not signal owned process group: {error}") from error


def process_group_exists(process: subprocess.Popen[bytes]) -> bool:
    """Check the pinned group independently of leader exit or output pipes.

    Cancellation still owes cleanup when a descendant closes its output and
    ignores the first signal. Neither leader exit nor EOF establishes that
    the group is gone. A group containing only zombies may remain visible;
    waiting through the bounded grace period is harmless in that case.
    """

    return terminate_group(process, 0)


def reap_owned_child(process: subprocess.Popen[bytes], timeout: float) -> int:
    """Reap a real status within a deadline without Popen's synthetic zero on ECHILD.

    All group signals precede this call. Only a successful waitpid may populate
    returncode; losing the status remains an ownership error even if a previous
    non-reaping observation saw the child exit. The caller retains that observation
    separately for partial evidence.
    """

    if process.returncode is not None:
        return process.returncode
    deadline = time.monotonic() + timeout
    while True:
        try:
            pid, status = os.waitpid(process.pid, os.WNOHANG)
        except InterruptedError:
            pid = 0
        except ChildProcessError as error:
            raise WaitOwnershipLost("child wait ownership was lost during final reap") from error
        if pid == process.pid:
            process.returncode = os.waitstatus_to_exitcode(status)
            return process.returncode
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise subprocess.TimeoutExpired(process.args, timeout)
        time.sleep(min(0.01, remaining))


# Metadata probes share one bounded subprocess implementation. Source-specific
# interpretation happens later and cannot weaken these time or memory limits.


def bounded_probe(
    argv: list[str],
    *,
    cwd: pathlib.Path,
    env: dict[str, str],
    intent: SignalIntent,
    timeout: float = PROBE_TIMEOUT_SECONDS,
    stdout_consumer: Callable[[bytes], None] | None = None,
) -> ProbeResult:
    """Run a metadata command with bounded time and retained memory.

    The full stdout and stderr byte streams are hashed. Only a prefix is kept,
    and an optional streaming consumer can derive evidence without accumulating
    the probe output.
    """

    started = time.monotonic()
    stdout_capture = StreamCapture()
    stderr_capture = StreamCapture()
    if intent.received is not None:
        # Metadata assembly still needs an explicit missing observation, but
        # interruption is not permission to start another diagnostic child.
        return ProbeResult(
            argv, None, False, True,
            0, stdout_capture.digest.hexdigest(), b"", False,
            0, stderr_capture.digest.hexdigest(), b"", False,
            time.monotonic() - started,
        )
    try:
        process = subprocess.Popen(
            argv,
            cwd=os.fspath(cwd),
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
    except OSError as error:
        return ProbeResult(
            argv,
            None,
            False,
            False,
            0,
            hashlib.sha256().hexdigest(),
            b"",
            False,
            0,
            hashlib.sha256().hexdigest(),
            b"",
            False,
            time.monotonic() - started,
            f"{type(error).__name__}: {error}",
        )

    selector: selectors.BaseSelector | None = None
    streams: dict[int, tuple[BinaryIO, StreamCapture, bool]] = {}
    deadline = started + timeout
    timed_out = False
    interrupted = False
    termination_started: float | None = None
    kill_attempted_at: float | None = None
    leader_exited_at: float | None = None
    pipe_drain_truncated = False
    lifecycle_error: str | None = None
    observed_returncode: int | None = None
    try:
        # A successful spawn creates the cleanup obligation. Selector allocation
        # and partial registration can fail too, especially under descriptor pressure.
        assert process.stdout is not None and process.stderr is not None
        selector = selectors.DefaultSelector()
        for stream, capture, is_stdout in (
            (process.stdout, stdout_capture, True),
            (process.stderr, stderr_capture, False),
        ):
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ)
            streams[stream.fileno()] = (stream, capture, is_stdout)
        while True:
            now = time.monotonic()
            observed = observe_exit(process)
            if observed is not None:
                observed_returncode = observed
                if leader_exited_at is None:
                    leader_exited_at = now
            if termination_started is None and intent.received is not None:
                interrupted = True
                terminate_group(process, intent.received)
                termination_started = now
            elif termination_started is None and now >= deadline:
                timed_out = True
                terminate_group(process, signal.SIGTERM)
                termination_started = now
            elif (
                termination_started is not None
                and kill_attempted_at is None
                and now - termination_started >= PROBE_KILL_GRACE_SECONDS
            ):
                terminate_group(process, signal.SIGKILL)
                kill_attempted_at = now

            # A runnable recorder must stop waiting even when SIGKILL cannot
            # release a leader from a kernel wait. Reaping has its own bounded
            # attempt below; neither that attempt nor pipe closure proves death.
            if kill_attempted_at is not None and now - kill_attempted_at >= PROBE_KILL_GRACE_SECONDS:
                pipe_drain_truncated = pipe_drain_truncated or bool(streams)
                for stream, _capture, _is_stdout in list(streams.values()):
                    selector.unregister(stream)
                    stream.close()
                streams.clear()
                break

            if (
                streams
                and leader_exited_at is not None
                and now - leader_exited_at >= PROBE_KILL_GRACE_SECONDS
            ):
                terminate_group(process, signal.SIGKILL)
                kill_attempted_at = now if kill_attempted_at is None else kill_attempted_at
                pipe_drain_truncated = True
                for stream, _capture, _is_stdout in list(streams.values()):
                    selector.unregister(stream)
                    stream.close()
                streams.clear()

            for key, _events in selector.select(0.05):
                stream, capture, is_stdout = streams[key.fd]
                try:
                    chunk = os.read(key.fd, 64 * 1024)
                except BlockingIOError:
                    continue
                if chunk:
                    capture.add(chunk)
                    if is_stdout and stdout_consumer is not None:
                        stdout_consumer(chunk)
                else:
                    selector.unregister(stream)
                    stream.close()
                    streams.pop(key.fd, None)

            observed = observe_exit(process)
            if observed is not None:
                observed_returncode = observed
            if observed is not None and not streams:
                # EOF says nothing about descendants that redirected output.
                # A cancelled probe must finish its group cleanup obligation.
                if (
                    termination_started is None
                    or kill_attempted_at is not None
                    or not process_group_exists(process)
                ):
                    break
        try:
            returncode = reap_owned_child(process, PROBE_KILL_GRACE_SECONDS)
        except subprocess.TimeoutExpired:
            returncode = observed_returncode
            lifecycle_error = "probe leader remained alive after cleanup; lifecycle incomplete"
    except WaitOwnershipLost as error:
        # Preserve everything observed before ownership disappeared. Returning
        # partial evidence lets metadata assembly finish before run() refuses
        # command execution; no cleanup signal is safe after this boundary.
        returncode = observed_returncode
        lifecycle_error = str(error)
        pipe_drain_truncated = pipe_drain_truncated or bool(streams)
    except Exception as error:
        lifecycle_error = f"{type(error).__name__}: {error}"
        returncode = observed_returncode
        pipe_drain_truncated = True
        ownership_lost = False
        try:
            terminate_group(process, signal.SIGKILL)
        except Exception as cleanup_error:
            ownership_lost = isinstance(cleanup_error, WaitOwnershipLost)
            lifecycle_error += f"; signal cleanup: {cleanup_error}"
        # A signaling failure does not waive reaping an owned direct child;
        # losing ownership does. Both failures remain attached to partial output.
        if not ownership_lost:
            try:
                returncode = reap_owned_child(process, PROBE_KILL_GRACE_SECONDS)
            except Exception as cleanup_error:
                lifecycle_error += f"; reap cleanup: {cleanup_error}"
    finally:
        # Include unregistered pipes and attempt every close. Diagnostic close
        # failure must not discard the child status and output already captured.
        for resource in (selector, process.stdout, process.stderr):
            try:
                if resource is not None:
                    resource.close()
            except Exception as close_error:
                lifecycle_error = (lifecycle_error + "; " if lifecycle_error else "") + f"close: {close_error}"

    interrupted = interrupted or intent.received is not None
    return ProbeResult(
        argv,
        returncode,
        timed_out,
        interrupted,
        stdout_capture.byte_count,
        stdout_capture.digest.hexdigest(),
        bytes(stdout_capture.sample),
        stdout_capture.truncated,
        stderr_capture.byte_count,
        stderr_capture.digest.hexdigest(),
        bytes(stderr_capture.sample),
        stderr_capture.truncated,
        time.monotonic() - started,
        pipe_drain_truncated=pipe_drain_truncated,
        lifecycle_error=lifecycle_error,
    )


def probe_text(result: ProbeResult) -> str | None:
    """Decode a complete small probe response when it is valid UTF-8."""

    if not result.complete or result.stdout_sample_truncated:
        return None
    try:
        return result.stdout_sample.decode("utf-8").strip()
    except UnicodeDecodeError:
        return None


def discover_checkout(cwd: pathlib.Path, env: dict[str, str], intent: SignalIntent) -> tuple[pathlib.Path | None, ProbeResult]:
    """Find the checkout before selecting a safe evidence directory."""

    probe = bounded_probe(
        ["git", "rev-parse", "--show-toplevel"], cwd=cwd, env=env, intent=intent
    )
    text = probe_text(probe)
    return (pathlib.Path(text).resolve() if text else None, probe)


def checkout_marker_ancestor(cwd: pathlib.Path) -> pathlib.Path | None:
    """Conservatively locate a checkout fence when Git itself cannot answer."""

    for candidate in (cwd, *cwd.parents):
        try:
            (candidate / ".git").lstat()
            return candidate.resolve()
        except FileNotFoundError:
            continue
        except OSError:
            return candidate.resolve()
    return None


# Evidence files are private and incrementally useful. The manifest favors a
# previous complete snapshot over exposing a newer but torn JSON document.


class Manifest:
    """Publish complete schema snapshots through atomic replacement."""

    def __init__(self, run_dir: pathlib.Path, data: dict[str, object]) -> None:
        self.run_dir = run_dir
        self.path = run_dir / "manifest.json"
        self.data = data

    def write(self) -> None:
        encoded = (json.dumps(self.data, indent=2, sort_keys=True) + "\n").encode("utf-8")
        temporary = self.run_dir / ".manifest.json.tmp"
        fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        try:
            view = memoryview(encoded)
            while view:
                written = os.write(fd, view)
                if written <= 0:
                    raise RecorderFailure("manifest write returned no progress")
                view = view[written:]
            os.fsync(fd)
        finally:
            os.close(fd)
        os.replace(temporary, self.path)


class OutputStore:
    """Persist a bounded head and rolling tail without buffering output in RAM."""

    def __init__(self, run_dir: pathlib.Path) -> None:
        self.run_dir = run_dir
        self.observed = 0
        self.head_size = 0
        self.tail_files: list[tuple[pathlib.Path, int]] = []
        self._head_fd = self._open(run_dir / "output-head.log")
        self._tail_fd: int | None = None
        self._tail_sequence = 0

    @staticmethod
    def _open(path: pathlib.Path) -> int:
        return os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)

    def _start_tail(self) -> None:
        if len(self.tail_files) == TAIL_CHUNK_COUNT:
            old_path, _old_size = self.tail_files.pop(0)
            old_path.unlink()
        self._tail_sequence += 1
        path = self.run_dir / f"output-tail-{self._tail_sequence:06d}.log"
        self._tail_fd = self._open(path)
        self.tail_files.append((path, 0))

    def write(self, data: bytes) -> None:
        """Retain data incrementally while never exceeding the payload budget."""

        self.observed += len(data)
        offset = 0
        if self.head_size < CHUNK_SIZE:
            count = min(len(data), CHUNK_SIZE - self.head_size)
            self._write_all(self._head_fd, data[:count])
            self.head_size += count
            offset = count

        while offset < len(data):
            if self._tail_fd is None or self.tail_files[-1][1] == CHUNK_SIZE:
                if self._tail_fd is not None:
                    os.close(self._tail_fd)
                self._start_tail()
            path, size = self.tail_files[-1]
            count = min(len(data) - offset, CHUNK_SIZE - size)
            assert self._tail_fd is not None
            self._write_all(self._tail_fd, data[offset : offset + count])
            self.tail_files[-1] = (path, size + count)
            offset += count

    @staticmethod
    def _write_all(fd: int, data: bytes) -> None:
        view = memoryview(data)
        while view:
            written = os.write(fd, view)
            if written <= 0:
                raise RecorderFailure("output log write returned no progress")
            view = view[written:]

    def close(self) -> None:
        for fd in (self._head_fd, self._tail_fd):
            if fd is not None:
                try:
                    os.close(fd)
                except OSError:
                    pass
        self._head_fd = -1
        self._tail_fd = None

    def evidence(self) -> dict[str, object]:
        retained = self.head_size + sum(size for _path, size in self.tail_files)
        files = []
        if self.head_size:
            files.append({"name": "output-head.log", "bytes": self.head_size, "role": "head"})
        files.extend(
            {"name": path.name, "bytes": size, "role": "tail"}
            for path, size in self.tail_files
        )
        return {
            "policy": "1 MiB head plus seven rolling 1 MiB tail chunks",
            "limit_bytes": OUTPUT_LIMIT,
            "observed_bytes": self.observed,
            "retained_bytes": retained,
            "omitted_bytes": self.observed - retained,
            "truncated": self.observed > retained,
            "files_in_read_order": files,
        }


class ConsoleForwarder:
    """Forward output on a daemon thread so a slow console cannot stop the child clock."""

    def __init__(self, fd: int = 1) -> None:
        self.fd = fd
        self.items: queue.Queue[bytes | None] = queue.Queue(maxsize=CONSOLE_QUEUE_CHUNKS)
        self.observed = 0
        self.forwarded = 0
        self.rejected = 0
        self._lock = threading.Lock()
        self._thread = threading.Thread(target=self._run, name="recorder-console", daemon=True)
        self._thread.start()

    def offer(self, data: bytes) -> None:
        with self._lock:
            self.observed += len(data)
        try:
            self.items.put_nowait(data)
        except queue.Full:
            with self._lock:
                self.rejected += len(data)

    def _run(self) -> None:
        while True:
            item = self.items.get()
            if item is None:
                return
            view = memoryview(item)
            while view:
                try:
                    count = os.write(self.fd, view)
                except InterruptedError:
                    continue
                except OSError:
                    return
                if count <= 0:
                    return
                with self._lock:
                    self.forwarded += count
                view = view[count:]

    def finish(self) -> dict[str, int]:
        try:
            self.items.put_nowait(None)
        except queue.Full:
            pass
        self._thread.join(CONSOLE_FINAL_DRAIN_SECONDS)
        with self._lock:
            return {
                "observed_bytes": self.observed,
                "forwarded_bytes": self.forwarded,
                "dropped_or_pending_bytes": self.observed - self.forwarded,
                "queue_rejected_bytes": self.rejected,
            }


def hash_file(path: pathlib.Path) -> tuple[str | None, str | None]:
    """Stream a regular file into SHA256 while checking a deadline between reads."""

    digest = hashlib.sha256()
    deadline = time.monotonic() + PROBE_TIMEOUT_SECONDS
    try:
        if not stat.S_ISREG(path.stat().st_mode):
            return None, "not a regular file"
        with path.open("rb", buffering=0) as handle:
            while True:
                chunk = handle.read(1024 * 1024)
                if not chunk:
                    break
                digest.update(chunk)
                if time.monotonic() >= deadline:
                    return None, f"hashing exceeded {PROBE_TIMEOUT_SECONDS} seconds"
    except OSError as error:
        return None, f"{type(error).__name__}: {error}"
    return digest.hexdigest(), None


def add_problem(problems: list[dict[str, str]], path: bytes | None, message: str) -> None:
    """Keep a bounded byte-safe sample of source-capture problems."""

    if len(problems) >= SOURCE_PROBLEM_LIMIT:
        return
    item = {"message": message}
    if path is not None:
        item["path_base64"] = base64.b64encode(path).decode("ascii")
    problems.append(item)


@dataclass
class UntrackedConsumer:
    """Build an aggregate identity from Git's NUL-delimited untracked path stream."""

    checkout_bytes: bytes
    deadline: float
    pending: bytearray = field(default_factory=bytearray)
    digest: object = field(default_factory=hashlib.sha256)
    count: int = 0
    total_content_bytes: int = 0
    complete: bool = True
    problems: list[dict[str, str]] = field(default_factory=list)
    problem_count: int = 0
    discarding_oversized_path: bool = False

    def consume(self, data: bytes) -> None:
        if self.discarding_oversized_path:
            separator = data.find(0)
            if separator < 0:
                return
            self.discarding_oversized_path = False
            data = data[separator + 1 :]
        self.pending.extend(data)
        while True:
            separator = self.pending.find(0)
            if separator < 0:
                if len(self.pending) > SOURCE_PATH_BUFFER_LIMIT:
                    self._fail(None, f"untracked path exceeded {SOURCE_PATH_BUFFER_LIMIT} bytes")
                    self.pending.clear()
                    self.discarding_oversized_path = True
                return
            if separator > SOURCE_PATH_BUFFER_LIMIT:
                self._fail(None, f"untracked path exceeded {SOURCE_PATH_BUFFER_LIMIT} bytes")
                del self.pending[: separator + 1]
                continue
            path = bytes(self.pending[:separator])
            del self.pending[: separator + 1]
            self._consume_path(path)

    def _fail(self, path: bytes | None, message: str) -> None:
        self.complete = False
        self.problem_count += 1
        add_problem(self.problems, path, message)

    def _consume_path(self, relative: bytes) -> None:
        if not relative or os.path.isabs(relative) or b".." in relative.split(b"/"):
            self._fail(relative, "Git returned an unsafe untracked path")
            return
        if self.count >= SOURCE_ENTRY_LIMIT:
            self._fail(relative, f"entry limit {SOURCE_ENTRY_LIMIT} exceeded")
            return
        if time.monotonic() >= self.deadline:
            self._fail(relative, "untracked content hashing exceeded the source deadline")
            return
        self.count += 1
        path = os.path.join(self.checkout_bytes, relative)
        try:
            metadata = os.lstat(path)
            if stat.S_ISLNK(metadata.st_mode):
                kind = b"symlink"
                payload = os.readlink(path)
                if isinstance(payload, str):
                    payload = os.fsencode(payload)
                content_digest = hashlib.sha256(payload).digest()
                content_size = len(payload)
            elif stat.S_ISREG(metadata.st_mode):
                kind = b"file"
                file_digest = hashlib.sha256()
                content_size = 0
                flags = os.O_RDONLY
                if hasattr(os, "O_NOFOLLOW"):
                    flags |= os.O_NOFOLLOW
                fd = os.open(path, flags)
                try:
                    while True:
                        chunk = os.read(fd, 1024 * 1024)
                        if not chunk:
                            break
                        file_digest.update(chunk)
                        content_size += len(chunk)
                        if time.monotonic() >= self.deadline:
                            raise TimeoutError("untracked content hashing exceeded the source deadline")
                finally:
                    os.close(fd)
                content_digest = file_digest.digest()
            else:
                self._fail(relative, "unsupported untracked filesystem type")
                return
        except (OSError, TimeoutError) as error:
            self._fail(relative, f"{type(error).__name__}: {error}")
            return

        self.total_content_bytes += content_size
        assert hasattr(self.digest, "update")
        self.digest.update(len(relative).to_bytes(8, "big"))
        self.digest.update(relative)
        self.digest.update(len(kind).to_bytes(1, "big"))
        self.digest.update(kind)
        self.digest.update(stat.S_IMODE(metadata.st_mode).to_bytes(2, "big"))
        self.digest.update(content_size.to_bytes(8, "big"))
        self.digest.update(content_digest)

    def finish(self) -> None:
        if self.pending and not self.discarding_oversized_path:
            self._fail(None, "Git returned an unterminated untracked path")


def capture_source(
    checkout: pathlib.Path | None,
    top_level_probe: ProbeResult,
    env: dict[str, str],
    intent: SignalIntent,
) -> dict[str, object]:
    """Capture a whole-checkout identity without retaining source contents."""

    evidence: dict[str, object] = {
        "complete": False,
        "lifecycle_complete": top_level_probe.lifecycle_error is None,
        "git_top_level_probe": top_level_probe.evidence(),
    }
    if checkout is None:
        evidence["limits"] = ["Git top-level was unavailable or invalid"]
        return evidence

    evidence["git_top_level"] = os.fspath(checkout)
    head = bounded_probe(["git", "rev-parse", "HEAD"], cwd=checkout, env=env, intent=intent)
    tracked = bounded_probe(
        ["git", "diff", "--no-ext-diff", "--no-textconv", "--binary", "HEAD", "--"],
        cwd=checkout,
        env=env,
        intent=intent,
    )
    porcelain = bounded_probe(
        ["git", "status", "--porcelain=v1", "-z", "--untracked-files=all"],
        cwd=checkout,
        env=env,
        intent=intent,
    )
    untracked_deadline = time.monotonic() + PROBE_TIMEOUT_SECONDS
    consumer = UntrackedConsumer(os.fsencode(checkout), untracked_deadline)
    untracked = bounded_probe(
        ["git", "ls-files", "--others", "--exclude-standard", "-z"],
        cwd=checkout,
        env=env,
        intent=intent,
        stdout_consumer=consumer.consume,
    )
    consumer.finish()
    evidence["lifecycle_complete"] = all(
        probe.lifecycle_error is None for probe in (top_level_probe, head, tracked, porcelain, untracked)
    )

    head_text = probe_text(head)
    head_valid = head_text is not None and len(head_text) == 40 and all(
        character in "0123456789abcdefABCDEF" for character in head_text
    )
    untracked_complete = untracked.complete and consumer.complete
    evidence.update(
        {
            "head": head_text if head_valid else None,
            "head_probe": head.evidence(),
            "tracked_diff": {
                "complete": tracked.complete,
                "bytes": tracked.stdout_bytes,
                "sha256": tracked.stdout_sha256 if tracked.complete else None,
                "probe": tracked.evidence(include_stdout_sample=False),
            },
            "porcelain": {
                "complete": porcelain.complete,
                "bytes": porcelain.stdout_bytes,
                "sha256": porcelain.stdout_sha256 if porcelain.complete else None,
                "sample_base64": base64.b64encode(porcelain.stdout_sample).decode("ascii"),
                "sample_truncated": porcelain.stdout_sample_truncated,
                "probe": porcelain.evidence(include_stdout_sample=False),
            },
            "untracked_tree": {
                "complete": untracked_complete,
                "entry_count": consumer.count,
                "content_bytes": consumer.total_content_bytes,
                "sha256": consumer.digest.hexdigest() if untracked_complete else None,
                "problem_count": consumer.problem_count,
                "problem_samples": consumer.problems,
                "entry_limit": SOURCE_ENTRY_LIMIT,
                "probe": untracked.evidence(include_stdout_sample=False),
            },
        }
    )
    complete = head_valid and tracked.complete and porcelain.complete and untracked_complete
    evidence["complete"] = complete
    if complete:
        combined = hashlib.sha256()
        for label, value in (
            ("head", head_text),
            ("tracked_diff", tracked.stdout_sha256),
            ("porcelain", porcelain.stdout_sha256),
            ("untracked_tree", consumer.digest.hexdigest()),
        ):
            encoded = f"{label}\0{value}\0".encode("ascii")
            combined.update(encoded)
        evidence["fingerprint_sha256"] = combined.hexdigest()
    else:
        evidence["fingerprint_sha256"] = None
        evidence["limits"] = ["One or more source identity components were incomplete"]
    return evidence


# Substrate and host identity describe what the command inherited without
# persisting FARHELM values, which may be credentials.


def parse_tmux_pin(checkout: pathlib.Path | None) -> tuple[str | None, str | None]:
    """Read TMUX_VERSION as data without executing the source-pin file."""

    if checkout is None:
        return None, "Git checkout unavailable"
    path = checkout / ".github" / "release" / "source-pins.env"
    try:
        if not stat.S_ISREG(path.stat().st_mode):
            return None, f"source pin is not a regular file: {path}"
        with path.open("rb", buffering=0) as handle:
            encoded = handle.read(PROBE_SAMPLE_LIMIT + 1)
        if len(encoded) > PROBE_SAMPLE_LIMIT:
            return None, f"source pin exceeds {PROBE_SAMPLE_LIMIT} bytes: {path}"
        lines = encoded.decode("utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        return None, f"could not read {path}: {error}"
    values = []
    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        name, separator, value = stripped.partition("=")
        if separator and name.strip() == "TMUX_VERSION":
            values.append(value.strip())
    if len(values) != 1 or not values[0]:
        return None, "source-pins.env must contain exactly one non-empty TMUX_VERSION"
    return values[0], None


def capture_tmux(
    mode: str,
    checkout: pathlib.Path | None,
    env: dict[str, str],
    intent: SignalIntent,
) -> tuple[dict[str, object], list[str]]:
    """Compare the controlled tmux binary with both repository pin identities."""

    if mode == "none":
        return {"mode": "none", "checked": False, "uses_tmux": False}, []

    problems: list[str] = []
    expected_version, pin_error = parse_tmux_pin(checkout)
    if pin_error:
        problems.append(pin_error)

    resolved_text = shutil.which("tmux", path=env.get("PATH"))
    resolved = pathlib.Path(resolved_text).resolve() if resolved_text else None
    version_probe: ProbeResult | None = None
    actual_version: str | None = None
    actual_hash: str | None = None
    actual_hash_error: str | None = None
    if resolved is None:
        problems.append("tmux is missing from the controlled PATH")
    else:
        version_probe = bounded_probe([os.fspath(resolved), "-V"], cwd=pathlib.Path.cwd(), env=env, intent=intent)
        actual_version = probe_text(version_probe)
        if actual_version is None:
            problems.append("tmux version probe failed or returned invalid output")
        actual_hash, actual_hash_error = hash_file(resolved)
        if actual_hash_error:
            problems.append(f"could not hash resolved tmux: {actual_hash_error}")

    expected_binary = checkout / ".ci-tmux" / "tmux" if checkout is not None else None
    expected_hash: str | None = None
    expected_hash_error: str | None = None
    if expected_binary is None:
        problems.append("repository pinned tmux path is unavailable")
    else:
        expected_hash, expected_hash_error = hash_file(expected_binary)
        if expected_hash_error:
            problems.append(f"could not hash repository pinned tmux: {expected_hash_error}")

    if expected_version is not None and actual_version != f"tmux {expected_version}":
        problems.append(f"tmux version mismatch: expected tmux {expected_version}, found {actual_version!r}")
    if expected_hash is not None and actual_hash is not None and expected_hash != actual_hash:
        problems.append("tmux executable hash does not match the repository-built .ci-tmux/tmux")

    evidence: dict[str, object] = {
        "mode": mode,
        "checked": True,
        "uses_tmux": True,
        "lifecycle_complete": version_probe is None or version_probe.lifecycle_error is None,
        "matches_required_substrate": not problems,
        "problems": problems,
        "expected": {
            "version": expected_version,
            "binary_path": os.fspath(expected_binary) if expected_binary else None,
            "binary_sha256": expected_hash,
            "binary_hash_error": expected_hash_error,
        },
        "actual": {
            "resolved_path": os.fspath(resolved) if resolved else None,
            "version_output": actual_version,
            "binary_sha256": actual_hash,
            "binary_hash_error": actual_hash_error,
            "version_probe": version_probe.evidence() if version_probe else None,
        },
    }
    return evidence, problems


def environment_evidence(
    ambient: dict[str, str], child: dict[str, str], requested: Iterable[str],
    *, recorder_owned: Iterable[str] = (),
) -> dict[str, object]:
    """Describe FARHELM_ scrubbing by variable name without exposing values."""

    ambient_names = sorted(name for name in ambient if name.startswith("FARHELM_"))
    requested_names = sorted(set(requested))
    owned = set(recorder_owned)
    retained = sorted(name for name in requested_names if name in ambient and name not in owned)
    return {
        "farhelm": {
            "ambient_names": ambient_names,
            "requested_names": requested_names,
            "retained_names": retained,
            "removed_names": sorted(set(ambient_names) - set(retained) - owned),
            "overridden_names": sorted(set(ambient_names) & owned),
            "recorder_owned_names": sorted(owned),
            "requested_but_absent_names": sorted(set(requested_names) - set(ambient_names)),
            "child_names": sorted(name for name in child if name.startswith("FARHELM_")),
        },
        "locale": {
            "LANG": ambient.get("LANG"),
            "LC_ALL": ambient.get("LC_ALL"),
            "LC_CTYPE": ambient.get("LC_CTYPE"),
            "preferred_encoding": locale.getpreferredencoding(False),
            "filesystem_encoding": sys.getfilesystemencoding(),
        },
    }


def controlled_environment(ambient: dict[str, str], requested: Iterable[str]) -> dict[str, str]:
    """Copy the environment while removing every unrequested FARHELM_ value."""

    retained = set(requested)
    return {
        name: value
        for name, value in ambient.items()
        if not name.startswith("FARHELM_") or name in retained
    }


def platform_evidence() -> dict[str, object]:
    """Record portable host and interpreter identity relevant to test behavior."""

    # platform.uname().processor can start an unbounded `uname -p` subprocess.
    # Kernel-provided machine identity is sufficient here and creates no child
    # outside the recorder's owned probe lifecycle, even after interruption.
    uname = os.uname()
    return {
        "system": uname.sysname,
        "release": uname.release,
        "version": uname.version,
        "machine": uname.machine,
        "cpu_count": os.cpu_count(),
        "python_implementation": platform.python_implementation(),
        "python_version": platform.python_version(),
    }


# Command lifecycle is a separate phase from probes: only this clock implements
# --timeout, and only this process group belongs to the requested command.


@dataclass
class CommandResult:
    """Terminal child lifecycle evidence and the recorder's conventional exit."""

    outcome: str
    recorder_exit: int
    child_returncode: int | None
    command_duration: float | None
    forced_cleanup: bool
    error: str | None = None
    cleanup_limit: str | None = None


def run_command(
    argv: list[str],
    cwd: pathlib.Path,
    env: dict[str, str],
    timeout: float | None,
    intent: SignalIntent,
    output: OutputStore,
    console: ConsoleForwarder,
) -> CommandResult:
    """Own one process group through spawn, stream, termination, and bounded drain."""

    try:
        process = subprocess.Popen(
            argv,
            cwd=os.fspath(cwd),
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
    except OSError as error:
        return CommandResult("recorder-error", 125, None, None, False, f"spawn failed: {error}")

    stream = process.stdout
    selector: selectors.BaseSelector | None = None
    started = time.monotonic()
    deadline = started + timeout if timeout is not None else None
    termination_reason: str | None = None
    termination_signal: int | None = None
    termination_started: float | None = None
    leader_exited_at: float | None = None
    pipe_open = True
    forced_cleanup = False
    cleanup_attempted_at: float | None = None
    cleanup_limit: str | None = None
    error: str | None = None
    observed_returncode: int | None = None

    try:
        # Protect setup as part of the owned child lifecycle; a selector failure
        # after Popen must not leave an unrecorded command running in its session.
        assert stream is not None
        os.set_blocking(stream.fileno(), False)
        selector = selectors.DefaultSelector()
        selector.register(stream, selectors.EVENT_READ)
        while True:
            now = time.monotonic()
            returncode = observe_exit(process)
            if returncode is not None:
                observed_returncode = returncode
            if returncode is not None and leader_exited_at is None:
                leader_exited_at = now

            if termination_reason is None and intent.received is not None:
                termination_reason = "interrupted"
                termination_signal = intent.received
                terminate_group(process, termination_signal)
                termination_started = now
            elif termination_reason is None and deadline is not None and now >= deadline:
                termination_reason = "timed_out"
                termination_signal = signal.SIGTERM
                terminate_group(process, signal.SIGTERM)
                termination_started = now

            if (
                termination_started is not None
                and cleanup_attempted_at is None
                and now - termination_started >= CHILD_KILL_GRACE_SECONDS
            ):
                found_group = terminate_group(process, signal.SIGKILL)
                forced_cleanup = found_group or forced_cleanup
                cleanup_attempted_at = now
                if not found_group:
                    cleanup_limit = "owned process group no longer existed; escaped descendants may remain"

            if (
                termination_reason is None
                and leader_exited_at is not None
                and pipe_open
                and cleanup_attempted_at is None
                and now - leader_exited_at >= CHILD_KILL_GRACE_SECONDS
            ):
                found_group = terminate_group(process, signal.SIGKILL)
                forced_cleanup = found_group or forced_cleanup
                cleanup_attempted_at = now
                if not found_group:
                    cleanup_limit = "owned process group no longer existed; escaped descendants may remain"

            if (
                pipe_open
                and cleanup_attempted_at is not None
                and now - cleanup_attempted_at >= POST_KILL_DRAIN_SECONDS
            ):
                selector.unregister(stream)
                stream.close()
                pipe_open = False
                if cleanup_limit is None:
                    cleanup_limit = "output pipe remained open after owned-group cleanup; escaped descendants may remain; pipe closed at the drain deadline"

            # The leader's exit is not a prerequisite for reaching the bounded
            # final wait. SIGKILL may remain pending in an uninterruptible wait;
            # preserve the partial run and disclose that lifecycle failure.
            if (
                cleanup_attempted_at is not None
                and now - cleanup_attempted_at >= POST_KILL_DRAIN_SECONDS
            ):
                break

            if pipe_open:
                for key, _events in selector.select(0.05):
                    try:
                        chunk = os.read(key.fd, 64 * 1024)
                    except BlockingIOError:
                        continue
                    except OSError as read_error:
                        raise RecorderFailure(f"child output read failed: {read_error}") from read_error
                    if chunk:
                        output.write(chunk)
                        console.offer(chunk)
                    else:
                        selector.unregister(stream)
                        stream.close()
                        pipe_open = False
            else:
                time.sleep(0.01)

            returncode = observe_exit(process)
            if returncode is not None:
                observed_returncode = returncode
            if returncode is not None and not pipe_open:
                # Cancellation owns the whole group, including descendants
                # that ignore signals and no longer hold our output pipe.
                if (
                    termination_reason is None
                    or cleanup_attempted_at is not None
                    or not process_group_exists(process)
                ):
                    break
        try:
            returncode = reap_owned_child(process, POST_KILL_DRAIN_SECONDS)
        except subprocess.TimeoutExpired as wait_error:
            raise RecorderFailure("child leader did not exit after forced cleanup") from wait_error
    except Exception as lifecycle_error:
        error = f"{type(lifecycle_error).__name__}: {lifecycle_error}"
        ownership_lost = isinstance(lifecycle_error, WaitOwnershipLost)
        try:
            forced_cleanup = terminate_group(process, signal.SIGKILL) or forced_cleanup
        except RecorderFailure as cleanup_error:
            error += f"; cleanup failed: {cleanup_error}"
            ownership_lost = ownership_lost or isinstance(cleanup_error, WaitOwnershipLost)
        if ownership_lost:
            # Popen.wait fabricates zero after ECHILD. Keep the last real
            # observation, or unknown, rather than presenting that zero as evidence.
            returncode = observed_returncode
        else:
            try:
                reap_owned_child(process, POST_KILL_DRAIN_SECONDS)
            except subprocess.TimeoutExpired:
                error += "; child leader remained alive after cleanup"
                returncode = process.returncode
            except WaitOwnershipLost as wait_error:
                error += f"; cleanup failed: {wait_error}"
                returncode = observed_returncode
            else:
                returncode = process.returncode
    finally:
        try:
            if selector is not None:
                selector.close()
        finally:
            if stream is not None:
                stream.close()

    duration = time.monotonic() - started
    if error is not None:
        return CommandResult(
            "recorder-error", 125, returncode, duration, forced_cleanup, error, cleanup_limit
        )
    if termination_reason == "timed_out":
        return CommandResult(
            "timed_out", 124, returncode, duration, forced_cleanup, cleanup_limit=cleanup_limit
        )
    if termination_reason == "interrupted":
        assert termination_signal is not None
        return CommandResult(
            "interrupted",
            128 + termination_signal,
            returncode,
            duration,
            forced_cleanup,
            cleanup_limit=cleanup_limit,
        )
    assert returncode is not None
    recorder_exit = returncode if returncode >= 0 else 128 + -returncode
    return CommandResult(
        "completed", recorder_exit, returncode, duration, forced_cleanup, cleanup_limit=cleanup_limit
    )


def child_status(returncode: int | None) -> dict[str, int | None]:
    """Expose both Python's raw status and conventional exit/signal fields."""

    return {
        "raw_returncode": returncode,
        "exit_code": returncode if returncode is not None and returncode >= 0 else None,
        "signal": -returncode if returncode is not None and returncode < 0 else None,
    }


def prepare_root(
    requested: pathlib.Path,
    checkout: pathlib.Path | None,
    ambient: dict[str, str],
) -> pathlib.Path:
    """Create or validate the evidence root without weakening existing permissions."""

    root = resolved_future_path(requested)
    if checkout is not None and is_within(root, checkout):
        raise UsageRefusal(f"output root is inside the tested checkout: {root}")

    home_value = ambient.get("HOME")
    home = pathlib.Path(home_value) if home_value else pathlib.Path.home()
    state_home_value = ambient.get("XDG_STATE_HOME")
    live_states = {resolved_future_path(home / ".local" / "state" / "farhelm")}
    if state_home_value:
        live_states.add(resolved_future_path(pathlib.Path(state_home_value) / "farhelm"))
    for live_state in live_states:
        if is_within(root, live_state):
            raise UsageRefusal(f"output root is inside live Farhelm state: {root}")

    try:
        root.mkdir(mode=0o700, parents=True)
    except FileExistsError:
        # Concurrent first invocations share the root, not their UUID child.
        # Validate the winner's directory without changing its permissions.
        pass
    else:
        os.chmod(root, 0o700)
    if not root.is_dir():
        raise UsageRefusal(f"output root is not a directory: {root}")
    if not private_mode(root):
        raise UsageRefusal(f"existing output root has group or other permissions: {root}")
    return root


def initial_manifest(
    run_id: str,
    args: argparse.Namespace,
    cwd: pathlib.Path,
    environment: dict[str, object],
) -> dict[str, object]:
    """Build the running record before metadata probes or child spawn."""

    return {
        "schema_version": SCHEMA_VERSION,
        "run_id": run_id,
        "outcome": "running",
        "started_at": utc_now(),
        "finished_at": None,
        "duration_seconds": None,
        "command": {"argv": args.command, "cwd": os.fspath(cwd), "timeout_seconds": args.timeout},
        "labels": {
            "kind": args.kind,
            "selection": args.selection,
            "concurrency": args.concurrency,
            "interpretation": "descriptive labels supplied by the caller; no test counts inferred",
        },
        "environment": environment,
        "platform": platform_evidence(),
        "source": None,
        "tmux": None,
        "output": None,
        "console": None,
        "test_traces": None,
        "child_status": child_status(None),
        "recorder": {
            "exit_code": None,
            "forced_cleanup": False,
            "cleanup_limit": None,
            "error": None,
        },
    }


def finalize(
    manifest: Manifest,
    *,
    outcome: str,
    recorder_exit: int,
    total_started: float,
    child_returncode: int | None = None,
    command_duration: float | None = None,
    forced_cleanup: bool = False,
    error: str | None = None,
    cleanup_limit: str | None = None,
) -> None:
    """Publish one terminal outcome without erasing partial evidence."""

    manifest.data["outcome"] = outcome
    manifest.data["finished_at"] = utc_now()
    manifest.data["duration_seconds"] = time.monotonic() - total_started
    command = manifest.data["command"]
    assert isinstance(command, dict)
    command["duration_seconds"] = command_duration
    manifest.data["child_status"] = child_status(child_returncode)
    manifest.data["recorder"] = {
        "exit_code": recorder_exit,
        "forced_cleanup": forced_cleanup,
        "error": error,
        "cleanup_limit": cleanup_limit,
    }
    manifest.write()


def run(argv: list[str]) -> int:
    """Execute the recorder and return its conventional process status."""

    total_started = time.monotonic()
    ambient = dict(os.environ)
    intent = SignalIntent()
    prior_handlers = {
        signum: signal.signal(signum, intent.handle) for signum in (signal.SIGINT, signal.SIGTERM)
    }
    run_dir: pathlib.Path | None = None
    requested_root: pathlib.Path | None = None
    manifest: Manifest | None = None
    output: OutputStore | None = None
    console: ConsoleForwarder | None = None
    trace_fd: int | None = None
    try:
        args = parse_args(argv)
        require_wait_ownership()
        cwd = pathlib.Path.cwd().resolve()
        child_env = controlled_environment(ambient, args.keep_farhelm_env)
        checkout, top_level_probe = discover_checkout(cwd, child_env, intent)
        requested_root = args.output_root or default_output_root(ambient)
        root = prepare_root(requested_root, checkout or checkout_marker_ancestor(cwd), ambient)
        run_id = str(uuid.uuid4())
        run_dir = root / run_id
        run_dir.mkdir(mode=0o700)
        os.chmod(run_dir, 0o700)
        data = initial_manifest(
            run_id,
            args,
            cwd,
            environment_evidence(ambient, child_env, args.keep_farhelm_env),
        )
        data["source"] = {
            "complete": False,
            "lifecycle_complete": top_level_probe.lifecycle_error is None,
            "git_top_level_probe": top_level_probe.evidence(),
            "limits": ["source capture not completed"],
        }
        manifest = Manifest(run_dir, data)
        manifest.write()
        best_effort_write(2, f"test-run evidence: {run_dir}\n".encode())

        if top_level_probe.lifecycle_error is not None:
            raise RecorderFailure("initial metadata probe lifecycle incomplete; partial evidence retained")

        if intent.received is not None:
            exit_code = 128 + intent.received
            finalize(
                manifest,
                outcome="interrupted",
                recorder_exit=exit_code,
                total_started=total_started,
                error="recorder interrupted before metadata capture",
            )
            return exit_code

        manifest.data["source"] = capture_source(checkout, top_level_probe, child_env, intent)
        tmux, tmux_problems = capture_tmux(args.tmux, checkout, child_env, intent)
        manifest.data["tmux"] = tmux
        manifest.write()

        source = manifest.data["source"]
        assert isinstance(source, dict)
        if source.get("lifecycle_complete") is False or tmux.get("lifecycle_complete") is False:
            raise RecorderFailure("metadata probe lifecycle incomplete; partial evidence retained")

        if intent.received is not None:
            exit_code = 128 + intent.received
            finalize(
                manifest,
                outcome="interrupted",
                recorder_exit=exit_code,
                total_started=total_started,
                error="recorder interrupted during metadata capture",
            )
            return exit_code

        if tmux_problems:
            diagnostic = "tmux substrate mismatch:\n  - " + "\n  - ".join(tmux_problems) + "\n"
            best_effort_write(2, diagnostic.encode("utf-8", "replace"))
            if args.tmux == "required":
                finalize(
                    manifest,
                    outcome="refused",
                    recorder_exit=125,
                    total_started=total_started,
                    error="required tmux substrate did not match",
                )
                return 125

        trace_root, trace_fd = test_run_traces.create_run_root(run_dir)
        child_env[test_run_traces.TRACE_ENV] = os.fspath(trace_root)
        manifest.data["environment"] = environment_evidence(
            ambient, child_env, args.keep_farhelm_env, recorder_owned=(test_run_traces.TRACE_ENV,)
        )
        trace_identity = os.fstat(trace_fd)
        manifest.data["test_traces"] = {
            "root": trace_root.name,
            "device": trace_identity.st_dev,
            "inode": trace_identity.st_ino,
            "environment_name": test_run_traces.TRACE_ENV,
            "collection": {"status": "uncollected", "collection_complete": False},
        }
        # Publish the recovery location before the command can create traces.
        # A killed recorder leaves this running record and its raw fixed files;
        # the standalone collector can export them without rerunning the test.
        manifest.write()
        output = OutputStore(run_dir)
        console = ConsoleForwarder()
        result = run_command(args.command, cwd, child_env, args.timeout, intent, output, console)
        output.close()
        manifest.data["output"] = output.evidence()
        manifest.data["console"] = console.finish()
        # Commit the command result while storage is still available. Optional
        # archive output can consume the remaining space; neither that failure
        # nor a later manifest-write failure may replace this observed result.
        finalize(
            manifest,
            outcome=result.outcome,
            recorder_exit=result.recorder_exit,
            total_started=total_started,
            child_returncode=result.child_returncode,
            command_duration=result.command_duration,
            forced_cleanup=result.forced_cleanup,
            error=result.error,
            cleanup_limit=result.cleanup_limit,
        )
        try:
            collected = test_run_traces.collect(trace_fd, run_dir / "traces.tar")
        except Exception as error:
            collected = {"status": "incomplete", "collection_complete": False,
                         "errors": [{"kind": type(error).__name__}]}
        manifest.data["test_traces"]["collection"] = collected
        manifest.data["duration_seconds"] = time.monotonic() - total_started
        manifest.data["finished_at"] = utc_now()
        try:
            manifest.write()
        except Exception as error:
            best_effort_write(
                2, f"test trace collection publication failed ({type(error).__name__}); "
                "earlier command result retained\n".encode(),
            )
        return result.recorder_exit
    except UsageRefusal as error:
        best_effort_write(2, f"record-test-run: refused: {error}\n".encode("utf-8", "replace"))
        if manifest is not None:
            try:
                finalize(
                    manifest,
                    outcome="refused",
                    recorder_exit=125,
                    total_started=total_started,
                    error=str(error),
                )
            except OSError:
                pass
        elif run_dir is not None:
            best_effort_write(2, f"test-run evidence: {run_dir}\n".encode())
        elif requested_root is not None:
            best_effort_write(
                2,
                f"test-run evidence unavailable at requested root: {requested_root}\n".encode(
                    "utf-8", "replace"
                ),
            )
        return 125
    except Exception as error:
        message = f"{type(error).__name__}: {error}"
        best_effort_write(2, f"record-test-run: recorder error: {message}\n".encode("utf-8", "replace"))
        if output is not None:
            output.close()
        if console is not None:
            console_evidence = console.finish()
            if manifest is not None:
                manifest.data["console"] = console_evidence
        if manifest is not None:
            try:
                if output is not None:
                    manifest.data["output"] = output.evidence()
                finalize(
                    manifest,
                    outcome="recorder-error",
                    recorder_exit=125,
                    total_started=total_started,
                    error=message,
                )
            except OSError:
                pass
        return 125
    finally:
        if trace_fd is not None:
            try:
                os.close(trace_fd)
            except OSError:
                pass
        for signum, handler in prior_handlers.items():
            signal.signal(signum, handler)


def main() -> None:
    raise SystemExit(run(sys.argv[1:]))


if __name__ == "__main__":
    main()
