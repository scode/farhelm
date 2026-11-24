#!/usr/bin/env python3
"""Upgrade the installed agent CLIs, then run the opt-in real-agent drift cases, one recorded case at a time."""

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
import harness_upgrades

ROOT = Path(__file__).resolve().parents[1]
CASE_PATTERN = re.compile(r"real_agent_capture::drift::([a-z][a-z0-9_]*)::drift_([a-z][a-z0-9_-]*): test")
CaseKey = tuple[str, str]
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


def discover(binary: Path, env: dict[str, str]) -> dict[CaseKey, str]:
    """Use the ignored Rust entry points as the inventory of categories and their harnesses."""
    listing = subprocess.run(
        [str(binary), "--ignored", "--list"], cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE, check=True
    )
    cases = {}
    for line in listing.stdout.splitlines():
        if match := CASE_PATTERN.fullmatch(line):
            cases[(match[1], match[2])] = line.removesuffix(": test")
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
            # Two turns, both composer startups, and private fixture cleanup share this budget.
            "--timeout", "720",
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


def prepare(names: list[str], env: dict[str, str], upgrade: bool) -> dict[str, harness_upgrades.Upgrade]:
    """Upgrade (or only probe) each selected harness before any case runs, so every case tests the latest release.

    A drift case with no recipe is reported as a failed upgrade rather than silently tested on whatever is installed:
    adding a harness to the suite means deciding how it gets upgraded too.
    """
    upgrades = {}
    for name in names:
        recipe = harness_upgrades.RECIPES.get(name)
        if recipe is None:
            result = harness_upgrades.Upgrade("failed", "unknown", "unknown", "no recipe in scripts/harness_upgrades.py")
        elif upgrade:
            result = harness_upgrades.upgrade(recipe, env)
        else:
            result = harness_upgrades.probe(recipe, env)
        upgrades[name] = result
        print(harness_upgrades.describe(name, result), flush=True)
    return upgrades


def summarize(results: dict[CaseKey, Result], explicit: bool, upgrades: dict[str, harness_upgrades.Upgrade]) -> int:
    """Group verdicts while keeping unknown coverage separate from success.

    Each line names the version the case ran against, since a verdict is only meaningful for that version. A failed
    upgrade fails the run even when its case passed: the pass is real, but for a release users no longer get.
    """
    print(f"{'Verdict':<14}  {'Category':<22}  {'Harness':<13}  {'Version':<34}  Detail")
    for verdict, title in LABELS.items():
        for (category, name), result in results.items():
            if result.verdict == verdict:
                detail = result.detail.splitlines()[0]
                evidence = f"  ({result.evidence})" if result.evidence is not None else ""
                tested = upgrades[name].after if name in upgrades else "unknown"
                print(f"{title:<14}  {category:<22}  {name:<13}  {tested:<34}  {detail}{evidence}")
    stale = [name for name, result in upgrades.items() if result.status == "failed"]
    if stale:
        print(f"UPGRADE FAILED for {', '.join(stale)}: those cases ran against the version that was already installed")
    return int(bool(stale) or any(
        result.verdict == "failed" or (explicit and result.verdict == "could_not_run")
        for result in results.values()
    ))


def main(argv: list[str] | None = None) -> int:
    """Build and list once, upgrade the selected harnesses, then keep going after each individual verdict."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--harness", action="append", default=[], metavar="NAME")
    parser.add_argument(
        "--category", action="append", default=[], metavar="NAME",
        help="select a drift category, or all. By default runs base and lists all",
    )
    parser.add_argument("--list", action="store_true", help="build and list the ignored cases without launching agents")
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument(
        "--no-upgrade", action="store_true", help="test the installed versions, e.g. to reproduce a failure on them",
    )
    modes.add_argument("--upgrade-only", action="store_true", help="upgrade the selected harnesses and run no case")
    options = parser.parse_args(argv)
    env = child_environment(dict(os.environ))
    if options.upgrade_only:
        # Upgrading needs no test binary, so skip the build and select from the recipes instead of discovered cases.
        if options.category:
            parser.error("--category cannot be used with --upgrade-only")
        if options.list:
            parser.error("--list cannot be used with --upgrade-only")
        unknown = sorted(set(options.harness) - harness_upgrades.RECIPES.keys())
        if unknown:
            parser.error(f"unknown harness: {', '.join(unknown)} (choose from {', '.join(harness_upgrades.RECIPES)})")
        upgrades = prepare(list(dict.fromkeys(options.harness or harness_upgrades.RECIPES)), env, upgrade=True)
        return int(any(result.status == "failed" for result in upgrades.values()))
    try:
        binary = build_binary(env)
        cases = discover(binary, env)
    except (RuntimeError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"harness drift: {error}", file=sys.stderr)
        return 1
    categories = sorted({category for category, _ in cases})
    requested = options.category or (["all"] if options.list else ["base"])
    unknown = sorted(set(requested) - set(categories) - {"all"})
    if unknown:
        parser.error(f"unknown category: {', '.join(unknown)} (choose from {', '.join(categories)}, or all)")
    selected_categories = categories if "all" in requested else list(dict.fromkeys(requested))
    available = sorted({name for category, name in cases if category in selected_categories})
    unknown = sorted(set(options.harness) - {name for _, name in cases})
    if unknown:
        parser.error(f"unknown harness: {', '.join(unknown)} (choose from {', '.join(available)})")
    selected = [
        (category, name)
        for category in selected_categories
        for name in dict.fromkeys(options.harness or available)
        if (category, name) in cases
    ]
    if not selected:
        parser.error(
            f"no cases match the selected categories and harnesses (available harnesses: {', '.join(available)})"
        )
    if options.list:
        print("\n".join(f"{category}/{name}" for category, name in selected))
        return 0
    names = list(dict.fromkeys(name for _, name in selected))
    upgrades = prepare(names, env, upgrade=not options.no_upgrade)
    results = {}
    for category, name in selected:
        key = (category, name)
        try:
            results[key] = recorded_case(binary, cases[key], f"{category}/{name}", env)
        except OSError as error:
            results[key] = Result("failed", str(error))
    return summarize(results, bool(options.harness), upgrades)


if __name__ == "__main__":
    raise SystemExit(main())
