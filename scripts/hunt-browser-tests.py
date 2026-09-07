#!/usr/bin/env python3
"""Plan or explicitly run a finite browser reproduction under both engines.

Every attempt uses the recorder's fixed Playwright policy and private evidence.
Ordinary assertion failures may repeat; missing evidence, runner errors and
unfinished cleanup stop the batch so a later pass cannot conceal them.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import pathlib
import sys

# Planning must not dirty the checkout before the recorder measures its tree.
_previous_bytecode_setting = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    import test_hunt
finally:
    sys.dont_write_bytecode = _previous_bytecode_setting
    del _previous_bytecode_setting

CONCURRENCY = "1 Playwright worker; chromium and webkit; retries 0; repeat-each 1"


def parse_args(argv):
    """Require a finite explicit selection before probing or executing tools."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repeat", required=True, type=test_hunt.repeat_count)
    parser.add_argument("--timeout", required=True, type=test_hunt.finite_timeout)
    parser.add_argument("--output-root", type=pathlib.Path)
    parser.add_argument("--execute", action="store_true")
    if "--" not in argv:
        parser.error("a selected Playwright command is required after `--`")
    boundary = argv.index("--")
    args = parser.parse_args(argv[:boundary])
    args.command = argv[boundary + 1:]
    return args


def validate_request(args, recorder):
    """Reuse the fixed-policy allowlist and forbid an implicit full-suite hunt."""
    selected = recorder.test_run_playwright.selection_args(args.command)
    if not selected:
        raise ValueError("select browser test files or a grep expression explicitly")
    if not math.isfinite(args.repeat * args.timeout):
        raise ValueError("repeat multiplied by timeout must be finite")


def plan(args):
    """Expose selection and cost without installing, building or writing files."""
    return {
        "execute": args.execute, "command": args.command, "repeat": args.repeat,
        "timeout_seconds": args.timeout,
        "maximum_child_command_seconds": args.repeat * args.timeout,
        "concurrency": CONCURRENCY,
        "tmux": "required (validation only; no build or install)",
        "prerequisites": "installed pinned Playwright, both engines and built application",
        "metadata_and_cleanup_time_outside_child_command_sum": True,
        "output_root": os.fspath(args.output_root) if args.output_root else None,
    }


def completed_capture_problem(manifest, recorder_exit, recorder):
    """Require internally consistent child status and finished owned resources.

    A report can be structurally complete while console forwarding or cleanup
    is still incomplete. Such an attempt must not authorize another process.
    Only small selected fields enter the index; raw evidence stays in the run.
    """
    child = manifest.get("child_status")
    status = manifest.get("recorder")
    if not isinstance(child, dict) or not isinstance(status, dict):
        return "child or recorder status is missing"
    raw = child.get("raw_returncode")
    if type(raw) is not int or not -255 <= raw <= 255:
        return "child return code is missing or invalid"
    expected_child = recorder.child_status(raw)
    if any(type(child.get(key)) is not type(value) or child.get(key) != value
           for key, value in expected_child.items()):
        return "child status fields contradict one another"
    if type(status.get("exit_code")) is not int or status["exit_code"] != recorder_exit:
        return "recorder result contradicts manifest"
    if manifest.get("outcome") != "completed":
        return "recorder did not complete the command normally"
    if (type(status.get("forced_cleanup")) is not bool
            or status.get("cleanup_limit") is not None or status.get("error") is not None):
        return "recorder cleanup or status is incomplete"
    console = manifest.get("console")
    if (not isinstance(console, dict) or console.get("worker_finished") is not True
            or type(console.get("dropped_or_pending_bytes")) is not int
            or console["dropped_or_pending_bytes"] != 0):
        return "console forwarding is incomplete or its worker survived"
    traces = manifest.get("test_traces")
    if (not isinstance(traces, dict) or not isinstance(traces.get("collection"), dict)
            or traces["collection"].get("collection_complete") is not True):
        return "trace collection is incomplete"
    output = manifest.get("output")
    if (not isinstance(output, dict) or output.get("eof_observed") is not True
            or output.get("truncated") is not False
            or type(output.get("omitted_bytes")) is not int or output["omitted_bytes"] != 0):
        return "retained output is incomplete"
    return None


def summarize_attempt(number, attempt_root, recorder_exit, intent, recorder):
    """Revalidate retained browser reports before allowing another attempt.

    Reusing the bounded report collector keeps browser count and policy rules
    in one place. The manifest and freshly read report must agree; a partial or
    subsequently changed report has no trustworthy denominator in this index.
    """
    path, manifest, error = test_hunt.attempt_evidence(attempt_root, recorder)
    summary = {
        "attempt": number, "status": "incomplete", "recorder_exit": recorder_exit,
        "evidence": os.fspath(pathlib.Path(path).relative_to(attempt_root.parent)) if path else None,
        "manifest_present": manifest is not None, "report_complete": False,
    }
    if error or manifest is None:
        summary["incomplete_reason"] = (error or "manifest is absent")[:512]
        return summary
    if (type(manifest.get("schema_version")) is not int or manifest["schema_version"] != 1
            or manifest.get("run_id") != pathlib.Path(path).name):
        summary["incomplete_reason"] = "manifest schema or run identity is unsupported"
        return summary
    runner = manifest.get("runner")
    if (not isinstance(runner, dict) or runner.get("name") != "playwright"
            or runner.get("prepared") is not True):
        summary["incomplete_reason"] = "Playwright preparation evidence is missing"
        return summary
    report = recorder.test_run_playwright.collect(pathlib.Path(path))
    if report.get("complete") is not True or report != runner.get("report"):
        summary["incomplete_reason"] = "retained browser reports are incomplete or disagree with manifest"
        return summary
    summary.update(report_complete=True, counts=report["counts"], engines=report["engines"])
    problem = completed_capture_problem(manifest, recorder_exit, recorder)
    if intent.received is not None:
        summary["status"] = "cancelled"
        return summary
    if problem:
        summary["incomplete_reason"] = problem
        return summary
    raw = manifest["child_status"]["raw_returncode"]
    summary["child_status"] = recorder.child_status(raw)
    counts = report["counts"]
    terminal = report["policy"]["status"]
    # Engine coverage is necessary but not sufficient: another selected case
    # can remain unstarted even when each engine has a passing assertion.
    if counts["not_run"]:
        summary["incomplete_reason"] = "selected browser cases did not start"
        return summary
    if recorder_exit == raw == 0:
        problem = recorder.test_run_playwright.success_problem(report)
        if problem is None:
            summary["status"] = "passed"
            return summary
    elif (recorder_exit == raw == 1 and terminal == "failed" and counts["unexpected"] > 0
          and not any(counts[name] for name in ("global_errors", "interrupted", "flaky"))
          and all(bucket["passed"] + bucket["failed"] + bucket["timed_out"] > 0
                  for bucket in report["engines"].values())):
        # A case timeout is itself a reproducible test failure. Runner-wide
        # timeouts, interrupted cases and setup errors never enter this branch.
        summary["status"] = "failed"
        return summary
    summary["incomplete_reason"] = problem or "browser outcome does not establish a complete test attempt"
    return summary


def main(argv=None):
    """Print the finite plan, executing only when the operator requests it."""
    try:
        args = parse_args(sys.argv[1:] if argv is None else argv)
        recorder = test_hunt.load_recorder()
        validate_request(args, recorder)
        print(json.dumps(plan(args), sort_keys=True))
        if not args.execute:
            return 0
        return test_hunt.run_batch(args, recorder, runner_name="playwright",
                                   concurrency=CONCURRENCY, summarize=summarize_attempt)
    except (ValueError, OSError, RuntimeError) as error:
        print(f"hunt-browser-tests: refused: {error}", file=sys.stderr)
        return 125


if __name__ == "__main__":
    raise SystemExit(main())
