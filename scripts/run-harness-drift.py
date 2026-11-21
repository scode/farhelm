#!/usr/bin/env python3
"""Run the opt-in real-agent drift cases, one recorded case at a time."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
from typing import NamedTuple

from agent_screen_host import OUTER_SESSION_MARKERS

ROOT = Path(__file__).resolve().parents[1]
CASE_PATTERN = re.compile(r"real_agent_capture::drift::drift_([a-z][a-z0-9_-]*): test")
VERDICT_ENV = "FARHELM_DRIFT_VERDICT_FILE"
EVIDENCE_PREFIX = "test-run evidence: "
LABELS = {"passed": "PASSED", "could_not_run": "COULD NOT RUN", "failed": "FAILED"}


class Result(NamedTuple):
    """A terminal verdict and its private test evidence, if the recorder started."""

    verdict: str
    detail: str
    evidence: Path | None = None


def child_environment(environment: dict[str, str]) -> dict[str, str]:
    """Keep the caller's vendor logins without nesting the agent inside the invoking session."""
    return {
        key: value
        for key, value in environment.items()
        if key not in (*OUTER_SESSION_MARKERS, "TMUX", "TMUX_PANE") and not key.startswith("FARHELM_")
    }


def build_binary(env: dict[str, str]) -> Path:
    """Use Cargo's artifact record to locate the e2e test binary, including custom target directories."""
    built = subprocess.run(
        ["cargo", "test", "--locked", "--no-run", "-p", "farhelm", "--test", "e2e", "--message-format=json-render-diagnostics"],
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        check=True,
    )
    binaries = set()
    for line in built.stdout.splitlines():
        if not line.startswith("{"):
            continue
        item = json.loads(line)
        if (
            item.get("reason") == "compiler-artifact"
            and item["target"]["name"] == "e2e"
            and "test" in item["target"]["kind"]
            and item["executable"]
        ):
            binaries.add(Path(item["executable"]))
    if len(binaries) != 1:
        raise RuntimeError("Cargo did not report one executable e2e test artifact")
    return binaries.pop()


def discover(binary: Path, env: dict[str, str]) -> dict[str, str]:
    """Discover only ignored drift cases. The Rust test entry points are the harness inventory."""
    listing = subprocess.run(
        [str(binary), "--ignored", "--list"], cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE, check=True
    )
    cases = {}
    for line in listing.stdout.splitlines():
        if match := CASE_PATTERN.fullmatch(line):
            cases[match[1]] = line.removesuffix(": test")
    if not cases:
        raise RuntimeError("No ignored harness drift cases were found. The real-agent e2e suite requires Linux.")
    return dict(sorted(cases.items()))


def read_verdict(path: Path, exit_code: int, evidence: Path | None = None) -> Result:
    """A skip is explicit, and even a valid success record cannot override a nonzero exit."""
    try:
        payload = json.loads(path.read_text())
        verdict, detail = payload["verdict"], payload["detail"]
        if verdict not in LABELS or not isinstance(detail, str) or not detail.strip():
            raise ValueError("unknown or incomplete test verdict")
    except (OSError, ValueError, TypeError, KeyError):
        # Timeouts and spawn failures can end the recorder before libtest can write a verdict.
        if evidence is not None:
            try:
                manifest = json.loads((evidence / "manifest.json").read_text())
                version = manifest["schema_version"]
                if type(version) is not int or version != 1:
                    raise ValueError("unrecognized recorder manifest schema")
                if manifest["outcome"] == "timed_out":
                    return Result("failed", "recorder timed out before the test wrote a verdict")
                error = manifest["recorder"]["error"]
                if isinstance(error, str) and error.strip():
                    return Result("failed", f"recorder failed before the test wrote a verdict: {error}")
            except (OSError, ValueError, TypeError, KeyError):
                pass
        return Result("failed", f"test exited {exit_code} without a readable drift verdict")
    if exit_code != 0 and verdict != "failed":
        return Result("failed", f"test reported {LABELS[verdict]} but its recorder exited {exit_code}")
    return Result(verdict, detail)


def recorded_case(binary: Path, test: str, name: str, env: dict[str, str]) -> Result:
    """Run an exact case directly after one build, and retain its structured outcome with the recorder's files."""
    with tempfile.TemporaryDirectory(prefix="fh-drift-") as private:
        path = Path(private) / "verdict.json"
        command = [
            sys.executable, "-B", str(ROOT / "scripts/record-test-run.py"),
            "--kind", "development",
            "--selection", f"real {name} harness drift",
            "--concurrency", "one test, one libtest thread",
            "--timeout", "480",
            "--keep-farhelm-env", VERDICT_ENV,
            "--", str(binary), "--ignored", "--exact", test, "--test-threads", "1", "--nocapture",
        ]
        evidence = None
        recorder = None
        interrupted = None

        def forward_signal(signum, _frame):
            """Let the recorder stop and reap its test process group before this runner exits."""
            nonlocal interrupted
            interrupted = interrupted or signum
            if recorder is not None:
                recorder.send_signal(signum)

        previous = {
            signum: signal.signal(signum, forward_signal) for signum in (signal.SIGINT, signal.SIGTERM)
        }
        try:
            with subprocess.Popen(
                command, cwd=ROOT, env={**env, VERDICT_ENV: str(path)}, stderr=subprocess.PIPE, text=True, errors="replace",
            ) as recorder:
                # A runner-only cancellation can arrive while Popen is handing ownership to us.
                if interrupted is not None:
                    recorder.send_signal(interrupted)
                assert recorder.stderr is not None
                for line in recorder.stderr:
                    sys.stderr.write(line)
                    # The recorder announces its own directory before it starts the test subprocess.
                    if evidence is None and line.startswith(EVIDENCE_PREFIX):
                        candidate = Path(line.removeprefix(EVIDENCE_PREFIX).strip())
                        if candidate.is_absolute() and (candidate / "manifest.json").is_file():
                            evidence = candidate
                result = read_verdict(path, recorder.wait(), evidence)
            if evidence is None:
                return Result("failed", f"recorder did not retain evidence: {result.detail}")
            try:
                (evidence / "drift-verdict.json").write_text(
                    json.dumps({"verdict": result.verdict, "detail": result.detail}, indent=2) + "\n"
                )
            except OSError as error:
                return Result("failed", f"could not retain the drift verdict: {error}", evidence)
            return result._replace(evidence=evidence)
        finally:
            for signum, handler in previous.items():
                signal.signal(signum, handler)
            # A simultaneous spawn or recorder error must still end the suite after cancellation.
            if interrupted is not None:
                raise SystemExit(128 + interrupted)


def summarize(results: dict[str, Result], explicit: bool) -> int:
    """Group verdicts while keeping unknown coverage separate from success."""
    print(f"{'Verdict':<14}  {'Harness':<13}  Detail")
    for verdict, title in LABELS.items():
        for name, result in results.items():
            if result.verdict == verdict:
                detail = result.detail.splitlines()[0]
                evidence = f"  ({result.evidence})" if result.evidence is not None else ""
                print(f"{title:<14}  {name:<13}  {detail}{evidence}")
    return int(any(
        result.verdict == "failed" or (explicit and result.verdict == "could_not_run")
        for result in results.values()
    ))


def main(argv: list[str] | None = None) -> int:
    """Build and list once, then keep going after each individual verdict."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--harness", action="append", default=[], metavar="NAME")
    parser.add_argument("--list", action="store_true", help="build and list the ignored cases without launching agents")
    options = parser.parse_args(argv)
    env = child_environment(dict(os.environ))
    try:
        binary = build_binary(env)
        cases = discover(binary, env)
    except (RuntimeError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"harness drift: {error}", file=sys.stderr)
        return 1
    unknown = sorted(set(options.harness) - cases.keys())
    if unknown:
        parser.error(f"unknown harness: {', '.join(unknown)} (choose from {', '.join(cases)})")
    if options.list:
        print("\n".join(cases))
        return 0
    results = {}
    for name in dict.fromkeys(options.harness or cases):
        try:
            results[name] = recorded_case(binary, cases[name], name, env)
        except OSError as error:
            results[name] = Result("failed", str(error))
    return summarize(results, bool(options.harness))


if __name__ == "__main__":
    raise SystemExit(main())
