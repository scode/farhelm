#!/usr/bin/env python3
"""Repeat one explicitly selected nextest command with retained evidence.

The recorder still owns each child process and its evidence. This command only
validates the finite batch request, assigns private attempt roots, and records
bounded scheduling state so a later pass cannot hide an earlier failure.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import pathlib
import sys
from typing import Any


# Planning must not leave bytecode in the checkout before the recorder measures
# its source tree. Match the recorder's temporary import policy.
_previous_bytecode_setting = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    import test_hunt
    from test_hunt import INDEX_LIMIT, MANIFEST_LIMIT, finite_timeout, repeat_count, load_recorder
finally:
    sys.dont_write_bytecode = _previous_bytecode_setting
    del _previous_bytecode_setting

COMMAND_ARG_LIMIT = 128
COMMAND_BYTES_LIMIT = 16 * 1024
SUMMARY_TEXT_LIMIT = 512
SCOPE_OPTIONS = {
    "-p", "--package", "--workspace", "--lib", "--bins", "--tests",
    "--test", "--bin", "-E", "--filter-expr",
}
SLOTS_DESCRIPTION = "4 nextest slots; retries 0"
# nextest_metadata::NextestExitCode distinguishes failed cases from build,
# setup, output and unexpected runner failures. Only failed cases repeat.
# https://nexte.st/rustdoc/src/nextest_metadata/exit_codes.rs
TEST_RUN_FAILED = 100


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Parse the hunt options and preserve the child argv after `--`."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repeat", required=True, type=repeat_count)
    parser.add_argument("--timeout", required=True, type=finite_timeout)
    parser.add_argument("--output-root", type=pathlib.Path)
    parser.add_argument("--execute", action="store_true")
    if "--" in argv:
        boundary = argv.index("--")
        options = argv[:boundary]
        command = argv[boundary + 1 :]
    else:
        options = argv
        command = []
    args = parser.parse_args(options)
    if "--" not in argv or not command:
        parser.error("a nonempty child command is required after `--`")
    if command[:3] != ["cargo", "nextest", "run"]:
        parser.error("the child command must begin with `cargo nextest run`")
    return argparse.Namespace(**vars(args), command=command)


def has_scope(command: list[str]) -> bool:
    """Return whether the command names a package, target, workspace, or filter."""

    for arg in command[3:]:
        option = arg.split("=", 1)[0]
        if option in SCOPE_OPTIONS:
            return True
    return False


def validate_request(args: argparse.Namespace, recorder: Any) -> None:
    """Apply hunt-specific scope rules and the recorder's nextest allowlist."""

    if not has_scope(args.command):
        raise ValueError(
            "nextest selection requires -p/--package, --workspace, --lib, --bins, "
            "--tests, --test, --bin, -E, or --filter-expr"
        )
    try:
        recorder.test_run_nextest.selection_args(args.command)
    except ValueError as error:
        raise ValueError(str(error)) from error
    product = args.repeat * args.timeout
    if not math.isfinite(product):
        raise ValueError("repeat multiplied by timeout must be finite")
    if len(args.command) > COMMAND_ARG_LIMIT or sum(len(arg.encode("utf-8")) for arg in args.command) > COMMAND_BYTES_LIMIT:
        raise ValueError("child command exceeds the bounded batch index input limit")


def plan(args: argparse.Namespace) -> dict[str, object]:
    """Describe the batch without probing tools or creating any filesystem state."""

    return {
        "execute": args.execute,
        "command": args.command,
        "repeat": args.repeat,
        "timeout_seconds": args.timeout,
        "maximum_child_command_seconds": args.repeat * args.timeout,
        "concurrency": SLOTS_DESCRIPTION,
        "tmux": "required (validation only; no build or install)",
        "metadata_and_cleanup_time_outside_child_command_sum": True,
        "output_root": os.fspath(args.output_root) if args.output_root else None,
    }


def summarize_attempt(
    attempt: int,
    attempt_root: pathlib.Path,
    recorder_exit: int,
    intent: Any,
    recorder: Any,
) -> dict[str, object]:
    """Translate fixed manifest evidence into a bounded scheduling summary."""

    evidence_path, manifest, evidence_error = test_hunt.attempt_evidence(attempt_root, recorder)
    summary: dict[str, object] = {
        "attempt": attempt,
        "status": "incomplete",
        "recorder_exit": recorder_exit,
        "evidence": os.fspath(pathlib.Path(evidence_path).relative_to(attempt_root.parent))
        if evidence_path else None,
        "manifest_present": manifest is not None,
        "report_complete": False,
    }
    if evidence_error:
        summary["incomplete_reason"] = evidence_error[:SUMMARY_TEXT_LIMIT]
    if manifest is None:
        return summary
    if (type(manifest.get("schema_version")) is not int or manifest["schema_version"] != 1
            or manifest.get("run_id") != pathlib.Path(evidence_path).name):
        summary["incomplete_reason"] = "manifest schema or run identity is unsupported"
        return summary
    outcome = manifest.get("outcome")
    manifest_recorder = manifest.get("recorder")
    manifest_child = manifest.get("child_status")
    if not isinstance(outcome, str) or not isinstance(manifest_recorder, dict) or not isinstance(manifest_child, dict):
        summary["incomplete_reason"] = "manifest status evidence is missing or invalid"
        return summary
    manifest_exit = manifest_recorder.get("exit_code")
    raw_child = manifest_child.get("raw_returncode")
    # Validate before copying: a malformed manifest must not smuggle nested
    # objects or arbitrarily large integers into every bounded index entry.
    if (type(manifest_exit) is not int or not 0 <= manifest_exit <= 255
            or manifest_exit != recorder_exit
            or (raw_child is not None and (type(raw_child) is not int or not -255 <= raw_child <= 255))
            or type(manifest_recorder.get("forced_cleanup")) is not bool
            or manifest_recorder.get("cleanup_limit") is not None and not isinstance(manifest_recorder.get("cleanup_limit"), str)
            or manifest_recorder.get("error") is not None and not isinstance(manifest_recorder.get("error"), str)):
        summary["incomplete_reason"] = "manifest status contradicts recorder result or has invalid types"
        return summary
    expected_child = recorder.child_status(raw_child)
    if any(type(manifest_child.get(key)) is not type(value) or manifest_child.get(key) != value
           for key, value in expected_child.items()):
        summary["incomplete_reason"] = "manifest child status fields contradict one another"
        return summary
    summary["outcome"] = outcome[:SUMMARY_TEXT_LIMIT]
    summary["manifest_recorder"] = (
        {
            "exit_code": manifest_recorder.get("exit_code"),
            "forced_cleanup": manifest_recorder.get("forced_cleanup"),
            "error": str(manifest_recorder.get("error"))[:SUMMARY_TEXT_LIMIT],
            "cleanup_limit": manifest_recorder.get("cleanup_limit", "")[:SUMMARY_TEXT_LIMIT]
            if manifest_recorder.get("cleanup_limit") is not None else None,
        }
        if isinstance(manifest_recorder, dict) else None
    )
    summary["child_status"] = (
        {key: manifest_child.get(key) for key in ("raw_returncode", "exit_code", "signal")}
        if isinstance(manifest_child, dict) else None
    )
    runner = manifest.get("runner")
    if isinstance(runner, dict) and runner.get("name") == "nextest" and runner.get("prepared") is True:
        report = runner.get("report")
        if isinstance(report, dict):
            complete = report.get("complete") is True
            summary["report_complete"] = complete
            if not complete:
                summary["incomplete_reason"] = "nextest report is incomplete"
                return summary
            counts = report.get("counts")
            if isinstance(counts, dict):
                names = ("tests", "passed", "skipped", "failures", "errors", "flaky")
                if all(name in counts and isinstance(counts[name], int) and not isinstance(counts[name], bool)
                       and 0 <= counts[name] <= 1_000_000_000 for name in names):
                    valid_counts = (
                        counts["tests"] == counts["passed"] + counts["skipped"]
                        + counts["failures"] + counts["errors"]
                        and counts["flaky"] <= counts["tests"]
                    )
                    if not valid_counts:
                        summary["incomplete_reason"] = "report counts contradict one another"
                    else:
                        summary["counts"] = {name: counts[name] for name in names}
                else:
                    summary["incomplete_reason"] = "report counts are missing or invalid"
            else:
                summary["incomplete_reason"] = "report counts are missing"
        else:
            summary["incomplete_reason"] = "nextest report is missing"
    else:
        summary["incomplete_reason"] = "nextest preparation evidence is missing"
    if "counts" not in summary or summary.get("incomplete_reason"):
        return summary
    if intent.received is not None:
        summary["status"] = "cancelled"
    elif manifest.get("outcome") != "completed":
        summary["incomplete_reason"] = f"recorder outcome is {manifest.get('outcome')!r}"[:SUMMARY_TEXT_LIMIT]
    elif manifest_recorder.get("cleanup_limit") is not None or manifest_recorder.get("error") is not None:
        summary["incomplete_reason"] = "recorder disclosed incomplete cleanup or an error"
    elif (not isinstance(manifest.get("console"), dict)
          or manifest["console"].get("worker_finished") is not True
          or type(manifest["console"].get("dropped_or_pending_bytes")) is not int
          or manifest["console"]["dropped_or_pending_bytes"] != 0):
        summary["incomplete_reason"] = "console forwarding is incomplete or its worker survived"
    elif (not isinstance(manifest.get("test_traces"), dict)
          or not isinstance(manifest["test_traces"].get("collection"), dict)
          or manifest["test_traces"]["collection"].get("collection_complete") is not True):
        summary["incomplete_reason"] = "trace collection is incomplete or uncollected"
    elif not isinstance(manifest.get("output"), dict) or manifest["output"].get("eof_observed") is not True:
        summary["incomplete_reason"] = "retained output did not reach EOF"
    elif manifest["output"].get("truncated") is not False or type(manifest["output"].get("omitted_bytes")) is not int or manifest["output"]["omitted_bytes"] != 0:
        summary["incomplete_reason"] = "retained output is incomplete"
    elif summary["counts"]["errors"]:
        # Nextest uses JUnit errors for execution failures and leaked handles,
        # including cases where its overall exit is the ordinary failure code.
        summary["incomplete_reason"] = "nextest reported execution or cleanup errors"
    elif recorder_exit == 0 and raw_child == 0 and summary["report_complete"]:
        counts = summary["counts"]
        assert isinstance(counts, dict)
        if counts["tests"] == 0 or counts["tests"] == counts["skipped"] or any(
            counts[name] for name in ("failures", "errors", "flaky")
        ):
            summary["incomplete_reason"] = "successful command has no passing executed cases"
        else:
            summary["status"] = "passed"
    elif raw_child == TEST_RUN_FAILED and recorder_exit == TEST_RUN_FAILED and any(
        summary["counts"][name] for name in ("failures", "flaky")
    ):
        summary["status"] = "failed"
    else:
        summary["incomplete_reason"] = "manifest command status is incomplete"
    return summary


def run_batch(args: argparse.Namespace, recorder: Any) -> int:
    """Schedule nextest attempts using its fixed policy and report classifier."""

    return test_hunt.run_batch(
        args, recorder, runner_name="nextest", concurrency=SLOTS_DESCRIPTION,
        summarize=summarize_attempt,
    )


def main(argv: list[str] | None = None) -> int:
    """Validate and print a plan, or execute the explicitly requested batch."""

    try:
        args = parse_args(sys.argv[1:] if argv is None else argv)
        recorder = load_recorder()
        validate_request(args, recorder)
        print(json.dumps(plan(args), sort_keys=True))
        if not args.execute:
            return 0
        return run_batch(args, recorder)
    except (ValueError, OSError, RuntimeError) as error:
        print(f"hunt-rust-tests: refused: {error}", file=sys.stderr)
        return 125


if __name__ == "__main__":
    raise SystemExit(main())
