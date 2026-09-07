"""Prepare reproducible nextest runs and inspect their retained report.

The recorder owns signals, child reaping and output capture. This module owns
only nextest-specific argv, configuration and report semantics. It never runs
tests or retries on its own, and missing report evidence stays incomplete.
"""

from __future__ import annotations

import hashlib
import argparse
import json
import os
import pathlib
import shutil
import stat
import time
import xml.etree.ElementTree as ET
from typing import Callable

try:
    import tomllib
except ModuleNotFoundError:
    tomllib = None


VERSION = "0.9.143"
THREADS = 4
TERMINATION_GRACE = 10.0
CONFIG_LIMIT = 64 * 1024
BINARY_LIMIT = 256 * 1024 * 1024
REPORT_LIMIT = 16 * 1024 * 1024
READ_SECONDS = 5.0
VALUE_OPTIONS = {
    "-p", "--package", "--exclude", "--bin", "--test", "--features",
    "--target", "--target-dir", "-E", "--filter-expr", "--run-ignored",
}
FLAG_OPTIONS = {
    "--workspace", "--lib", "--bins", "--tests", "--all-features",
    "--no-default-features", "--release", "--locked",
}


def selection_args(argv: list[str]) -> list[str]:
    """Accept target/feature/filter selection while keeping runner policy explicit.

    An allowlist also rejects abbreviated or joined policy flags, alternate
    profiles, cargo configuration injection and libtest passthrough. Advanced
    experiments can use the generic recorder, with their actual argv retained.
    """

    if argv[:3] != ["cargo", "nextest", "run"]:
        raise ValueError("nextest mode requires `-- cargo nextest run` followed by selection options")
    result = []
    index = 3
    while index < len(argv):
        arg = argv[index]
        option, separator, value = arg.partition("=")
        if option in VALUE_OPTIONS:
            if separator:
                if not value:
                    raise ValueError(f"empty value for {option}")
                result.append(arg)
            else:
                index += 1
                if index == len(argv) or not argv[index] or argv[index].startswith("-"):
                    raise ValueError(f"missing value for {option}")
                result.extend([arg, argv[index]])
        elif arg in FLAG_OPTIONS:
            # --locked is always inserted by this adapter, including when a
            # caller omits it. Avoid giving clap a duplicate flag.
            if arg != "--locked":
                result.append(arg)
        else:
            raise ValueError(f"unsupported nextest selection option: {option}")
        index += 1
    return result


def read_regular(path: pathlib.Path, limit: int, *, parent_fd: int | None = None) -> bytes:
    """Read a bounded regular file without following its final symlink.

    The deadline bounds runnable reads; a filesystem operation stuck in the
    kernel is outside this guarantee. Parent paths belong to the operator's
    checkout or the recorder's private run, not an arbitrary artifact walk.
    """

    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=parent_fd)
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_size > limit:
            raise ValueError("expected a regular file within its evidence size limit")
        deadline = time.monotonic() + READ_SECONDS
        data = bytearray()
        while len(data) <= limit:
            if time.monotonic() >= deadline:
                raise TimeoutError("evidence read deadline")
            chunk = os.read(fd, min(65536, limit + 1 - len(data)))
            if not chunk:
                return bytes(data)
            data.extend(chunk)
        raise ValueError("evidence grew beyond its size limit")
    finally:
        os.close(fd)


def write_private(path: pathlib.Path, data: bytes) -> None:
    """Create a run-owned artifact exclusively, so an earlier run is never replaced."""

    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as output:
        output.write(data)


def prepare(
    run_dir: pathlib.Path,
    checkout: pathlib.Path,
    argv: list[str],
    env: dict[str, str],
    probe: Callable,
) -> tuple[list[str], dict[str, object]]:
    """Snapshot effective inputs before the recorder starts its nextest child.

    The returned argv invokes the resolved binary directly. Removing ambient
    nextest settings affects only the child environment; variable names, never
    their values, are retained. The existing bounded probe owns version lookup.
    """

    selected = selection_args(argv)
    if tomllib is None:
        raise ValueError("nextest mode requires Python 3.11 or newer")
    removed = sorted(name for name in env if name.startswith("NEXTEST_"))
    for name in removed:
        del env[name]
    binary_name = shutil.which("cargo-nextest", path=env.get("PATH", ""))
    if binary_name is None:
        raise ValueError(f"cargo-nextest {VERSION} is required; install the pinned runner first")
    binary = pathlib.Path(binary_name).resolve(strict=True)
    binary_hash = hashlib.sha256(read_regular(binary, BINARY_LIMIT)).hexdigest()
    version = probe([str(binary), "nextest", "--version"])
    if (not version.complete
            or version.stdout_sample.decode("utf-8", "replace").split()[:2] != ["cargo-nextest", VERSION]):
        raise ValueError(f"nextest version probe must identify exactly cargo-nextest {VERSION}")

    config = read_regular(checkout / ".config" / "nextest.toml", CONFIG_LIMIT)
    parsed = tomllib.loads(config.decode("utf-8"))
    profile = parsed["profile"]["default"]
    # The outer recorder's ten-second grace is meaningful only while the
    # runner's signal grace remains five seconds. Report lookup also relies
    # on this exact relative path and on the absence of a higher-priority store.
    if ("store" in parsed or profile["slow-timeout"]["grace-period"] != "5s"
            or profile["junit"]["path"] != "junit.xml"
            or profile["junit"].get("report-skipped") != "all"):
        raise ValueError("nextest configuration changed the recorded-run cleanup/report contract")
    config_path = run_dir / "nextest.toml"
    tool_path = run_dir / "nextest-store.toml"
    store = run_dir / "nextest"
    write_private(config_path, config)
    tool_config = f"[store]\ndir = {json.dumps(str(store), ensure_ascii=False)}\n".encode()
    write_private(tool_path, tool_config)
    command = [
        str(binary), "nextest", "run", "--config-file", str(config_path),
        "--tool-config-file", f"recorder:{tool_path}", "--user-config-file", "none",
        "--profile", "default", "--test-threads", str(THREADS), "--retries", "0",
        "--flaky-result", "fail", "--no-tests", "fail", "--locked", *selected,
    ]
    evidence = {
        "name": "nextest", "version": VERSION, "binary": str(binary), "binary_sha256": binary_hash,
        "version_probe": version.evidence(), "removed_environment_names": removed,
        "config": {"path": config_path.name, "sha256": hashlib.sha256(config).hexdigest()},
        "tool_config": {"path": tool_path.name, "sha256": hashlib.sha256(tool_config).hexdigest()},
        "profile": "default", "test_threads": THREADS, "retries": 0,
        "report": {"complete": False, "path": "nextest/default/junit.xml", "reason": "not collected"},
    }
    return command, evidence


def checked_counts(element: ET.Element, cases: list[ET.Element]) -> dict[str, int]:
    """Check one declared suite or aggregate against its actual case outcomes."""

    counts = {"tests": len(cases), "skipped": 0, "failures": 0, "errors": 0, "flaky": 0}
    for case in cases:
        if sum(case.find(name) is not None for name in ("skipped", "failure", "error")) > 1:
            raise ValueError("JUnit case outcomes overlap")
        for xml_name, count_name in (("skipped", "skipped"), ("failure", "failures"), ("error", "errors")):
            counts[count_name] += int(case.find(xml_name) is not None)
        counts["flaky"] += int(any(child.tag.startswith("flaky") for child in case))
    for name in ("tests", "skipped", "failures", "errors"):
        if int(element.attrib[name]) != counts[name]:
            raise ValueError("JUnit totals disagree with its cases")
    counts["passed"] = counts["tests"] - counts["skipped"] - counts["failures"] - counts["errors"]
    return counts


def collect(run_dir: pathlib.Path) -> dict[str, object]:
    """Describe the actual JUnit report, without turning a missing one into a zero-test pass.

    Parsing is bounded by file size, and entity declarations are refused. The
    report's totals and per-case outcomes are checked against one another;
    command status remains an independent observation in the main manifest.
    """

    result = {"complete": False, "path": "nextest/default/junit.xml", "byte_limit": REPORT_LIMIT}
    try:
        data = read_regular(run_dir / result["path"], REPORT_LIMIT)
        result.update(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
        text = data.decode("utf-8")
        if "<!DOCTYPE" in text or "<!ENTITY" in text:
            raise ValueError("XML declarations are not supported")
        root = ET.fromstring(text)
        if root.tag != "testsuites":
            raise ValueError("expected a nextest testsuites report")
        for suite in root.findall("./testsuite"):
            checked_counts(suite, suite.findall("./testcase"))
        cases = root.findall("./testsuite/testcase")
        counts = checked_counts(root, cases)
        result.update(complete=True, counts=counts, run_id=root.attrib.get("uuid"))
    except (OSError, ValueError, KeyError, ET.ParseError) as error:
        result["reason"] = type(error).__name__
    return result


def export_evidence(run_dir: pathlib.Path, destination: pathlib.Path) -> dict[str, object]:
    """Copy only bounded runner inputs and JUnit bytes, retaining omissions explicitly.

    Every input directory component beneath the run root is opened without following links. No raw test directory is
    walked. Export preserves report bytes, including malformed XML, because a report parser failure is useful evidence
    too. This collection status describes files, not test outcomes. Existing destination files are never replaced.
    """
    files = (
        (("nextest.toml",), "nextest.toml", CONFIG_LIMIT),
        (("nextest-store.toml",), "nextest-store.toml", CONFIG_LIMIT),
        (("nextest", "default", "junit.xml"), "nextest-junit.xml", REPORT_LIMIT),
    )
    result = {"complete": True, "files": [], "errors": []}
    root_fd = None
    try:
        root_fd = os.open(run_dir, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        destination.mkdir(mode=0o700, parents=False, exist_ok=True)
        if destination.is_symlink() or not destination.is_dir():
            raise ValueError("export destination must be a directory, not a link")
        for parts, name, limit in files:
            held = []
            try:
                parent = root_fd
                for component in parts[:-1]:
                    parent = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                                     dir_fd=parent)
                    held.append(parent)
                data = read_regular(pathlib.Path(parts[-1]), limit, parent_fd=parent)
                write_private(destination / name, data)
                result["files"].append({
                    "source": "/".join(parts), "path": name, "bytes": len(data),
                    "sha256": hashlib.sha256(data).hexdigest(),
                })
            except (OSError, ValueError) as error:
                result["complete"] = False
                result["errors"].append({"source": "/".join(parts), "reason": type(error).__name__})
            finally:
                for fd in reversed(held):
                    os.close(fd)
    except (OSError, ValueError) as error:
        result["complete"] = False
        result["errors"].append({"source": "run or destination directory", "reason": type(error).__name__})
    finally:
        if root_fd is not None:
            os.close(root_fd)
    return result


def main() -> None:
    """Expose fixed-layout export for release failure collection, including incomplete-file status."""
    parser = argparse.ArgumentParser(description="Export bounded nextest evidence; omissions remain in JSON output.")
    parser.add_argument("run_dir", type=pathlib.Path)
    parser.add_argument("destination", type=pathlib.Path)
    args = parser.parse_args()
    print(json.dumps(export_evidence(args.run_dir, args.destination), sort_keys=True))


if __name__ == "__main__":
    main()
