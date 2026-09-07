"""Schedule finite recorded test batches without choosing runner semantics.

Each CLI validates its selection and classifies its own report. This module
owns private attempt roots, bounded index publication and shared cancellation;
a later successful attempt cannot erase a failure already recorded.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import math
import os
import pathlib
import signal
import shlex
import sys
import uuid
from typing import Any, Callable

SCRIPT_DIR = pathlib.Path(__file__).resolve().parent
RECORDER_PATH = SCRIPT_DIR / "record-test-run.py"
INDEX_LIMIT = 2 * 1024 * 1024
MANIFEST_LIMIT = 2 * 1024 * 1024


def load_recorder():
    """Load the recorder without creating bytecode or running its CLI."""

    spec = importlib.util.spec_from_file_location("farhelm_hunt_recorder", RECORDER_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load the run recorder")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


def finite_timeout(value: str) -> float:
    """Parse the finite positive per-attempt timeout accepted by the recorder."""

    try:
        result = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a number") from error
    if not math.isfinite(result) or result <= 0:
        raise argparse.ArgumentTypeError("must be finite and greater than zero")
    return result


def repeat_count(value: str) -> int:
    """Parse the bounded number of attempts without accepting numeric coercion."""

    try:
        result = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be an integer") from error
    if str(result) != value or not 1 <= result <= 1000:
        raise argparse.ArgumentTypeError("must be an integer from 1 through 1000")
    return result


def write_index(path: pathlib.Path, data: dict[str, object]) -> None:
    """Atomically publish a bounded batch snapshot without following links."""

    encoded = (json.dumps(data, indent=2, sort_keys=True) + "\n").encode("utf-8")
    if len(encoded) > INDEX_LIMIT:
        raise RuntimeError("batch index exceeds its size limit")
    temporary = path.with_name(f".{path.name}.tmp")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
    fd = os.open(temporary, flags, 0o600)
    try:
        view = memoryview(encoded)
        while view:
            written = os.write(fd, view)
            if written <= 0:
                raise OSError("batch index write made no progress")
            view = view[written:]
        os.fsync(fd)
    finally:
        os.close(fd)
    os.replace(temporary, path)


def private_batch_root(recorder: Any, requested: pathlib.Path | None, cwd: pathlib.Path) -> pathlib.Path:
    """Create the recorder-validated private directory for one batch."""

    ambient = dict(os.environ)
    base = requested or recorder.default_output_root(ambient)
    try:
        root = recorder.prepare_root(base, recorder.checkout_marker_ancestor(cwd), ambient)
    except recorder.UsageRefusal as error:
        raise ValueError(str(error)) from error
    batch = root / str(uuid.uuid4())
    batch.mkdir(mode=0o700)
    os.chmod(batch, 0o700)
    return batch


def read_manifest(run_dir: pathlib.Path, recorder: Any) -> tuple[dict[str, object] | None, str | None]:
    """Read one fixed manifest through a held no-follow directory descriptor."""

    fd = None
    try:
        fd = os.open(run_dir, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        data = recorder.test_run_nextest.read_regular(pathlib.Path("manifest.json"), MANIFEST_LIMIT, parent_fd=fd)
        parsed = json.loads(data.decode("utf-8"))
        if not isinstance(parsed, dict):
            return None, "manifest is not a JSON object"
        return parsed, None
    except FileNotFoundError:
        return None, "manifest is absent"
    except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
        return None, f"manifest is unreadable or malformed: {type(error).__name__}"
    finally:
        if fd is not None:
            os.close(fd)


def attempt_evidence(attempt_root: pathlib.Path, recorder: Any) -> tuple[str | None, dict[str, object] | None, str | None]:
    """Find one canonical UUID child while refusing links and oversized directories."""

    candidates: list[pathlib.Path] = []
    try:
        with os.scandir(attempt_root) as entries:
            for entry in entries:
                if candidates:
                    return None, None, "attempt root has more than one evidence entry"
                if entry.is_symlink():
                    return None, None, "attempt root contains a symlink"
                if not entry.is_dir(follow_symlinks=False):
                    return None, None, "attempt root contains a non-directory entry"
                try:
                    if str(uuid.UUID(entry.name)) != entry.name:
                        return None, None, "evidence directory is not a canonical UUID"
                except ValueError:
                    return None, None, "evidence directory is not a UUID"
                candidates.append(pathlib.Path(entry.path))
    except OSError as error:
        return None, None, f"attempt evidence enumeration failed: {type(error).__name__}"
    if len(candidates) != 1:
        return None, None, "recorder evidence directory is absent or ambiguous"
    manifest, reason = read_manifest(candidates[0], recorder)
    return os.fspath(candidates[0]), manifest, reason


def run_batch(
    args: argparse.Namespace,
    recorder: Any,
    *,
    runner_name: str,
    concurrency: str,
    summarize: Callable[..., dict[str, object]],
) -> int:
    """Schedule validated attempts, stopping on cancellation or incomplete evidence.

    The caller validates finite budgets and runner selection before calling.
    Its summary callback must return a bounded dictionary with status passed,
    failed, cancelled, or incomplete; only ordinary passed/failed attempts may
    authorize another invocation. One signal intent spans the entire batch.
    """

    cwd = pathlib.Path.cwd().resolve()
    batch = private_batch_root(recorder, args.output_root, cwd)
    index_path = batch / "index.json"
    print(f"hunt evidence: {index_path}", file=sys.stderr, flush=True)
    intent = recorder.SignalIntent()
    previous = {signum: signal.signal(signum, intent.handle) for signum in (signal.SIGINT, signal.SIGTERM)}
    attempts: list[dict[str, object]] = []
    index: dict[str, object] = {
        "schema_version": 1,
        "state": "running",
        "planned": args.repeat,
        "started": 0,
        "finished": 0,
        "not_started": args.repeat,
        "command": args.command,
        "timeout_seconds": args.timeout,
        "concurrency": concurrency,
        "tmux": "required (validation only; no build or install)",
        "attempts": attempts,
    }
    try:
        write_index(index_path, index)
        aggregate_failure = False
        incomplete = False
        for number in range(1, args.repeat + 1):
            if intent.received is not None:
                incomplete = True
                break
            attempt_root = batch / f"attempt-{number:04d}"
            attempt_root.mkdir(mode=0o700)
            os.chmod(attempt_root, 0o700)
            index["started"] = number
            index["not_started"] = args.repeat - number
            write_index(index_path, index)
            recorder_args = [
                "--kind", "repetition", "--runner", runner_name, "--tmux", "required",
                "--selection", shlex.join(args.command),
                "--concurrency", concurrency, "--timeout", str(args.timeout),
                "--output-root", os.fspath(attempt_root), "--", *args.command,
            ]
            try:
                recorder_exit = recorder.run(recorder_args, signal_intent=intent)
            except Exception:
                recorder_exit = 125
                incomplete = True
            summary = summarize(number, attempt_root, recorder_exit, intent, recorder)
            attempts.append(summary)
            index["finished"] = len(attempts)
            index["not_started"] = args.repeat - len(attempts)
            write_index(index_path, index)
            if intent.received is not None:
                incomplete = True
                break
            if summary["status"] == "failed":
                aggregate_failure = True
            elif summary["status"] != "passed":
                incomplete = True
                break
        if intent.received is not None:
            index["state"] = "cancelled"
            index["signal"] = intent.received
            write_index(index_path, index)
            return 128 + intent.received
        if incomplete or len(attempts) != args.repeat:
            index["state"] = "incomplete"
            write_index(index_path, index)
            return 125
        index["state"] = "failed" if aggregate_failure else "completed"
        write_index(index_path, index)
        if intent.received is not None:
            index["state"] = "cancelled"
            index["signal"] = intent.received
            write_index(index_path, index)
            return 128 + intent.received
        return 1 if aggregate_failure else 0
    except Exception:
        index["state"] = "incomplete"
        if intent.received is not None:
            index["state"] = "cancelled"
            index["signal"] = intent.received
        try:
            write_index(index_path, index)
        except Exception:
            pass
        return 128 + intent.received if intent.received is not None else 125
    finally:
        for signum, handler in previous.items():
            signal.signal(signum, handler)
