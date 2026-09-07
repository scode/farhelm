"""Prepare fixed-policy Playwright runs and validate their bounded reports.

The recorder owns process execution and evidence roots.  This module only
pins the runner policy and reads the two fixed report files without treating
missing evidence as a successful empty run.
"""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import re
import shutil
import signal
from typing import Callable

from test_run_nextest import read_regular

VERSION = "1.62.0"
TERMINATION_GRACE = 60.0
GRACEFUL_SIGNAL = signal.SIGINT
ARGV_LIMIT = 128
ARGV_BYTES_LIMIT = 16 * 1024
REPORT_LIMIT = 16 * 1024 * 1024
POLICY_LIMIT = 64 * 1024
NODE_LIMIT = 20_000
TEXT_LIMIT = 4096
ARTIFACTS = "playwright-artifacts"
REPORT = "playwright.json"
POLICY = "playwright-policy.json"
CONFIG = "playwright.config.ts"
REPORTER = "recorded-policy-reporter.cjs"


def environment_control(name: str) -> bool:
    """Recognize runner overrides, including Playwright's npm environment aliases.

    PWDEBUG and PW_TEST_* influence timeouts, reporters and source loading even
    with explicit CLI policy. npm aliases are case-normalized by Playwright's
    environment resolver; ordinary unrelated environment entries stay intact.
    """
    for prefix in ("npm_config_", "npm_package_config_"):
        if name.startswith(prefix):
            name = name[len(prefix):].upper()
            break
    return name == "PWDEBUG" or name.startswith(("PLAYWRIGHT_", "PWTEST_", "PW_TEST_"))


def selection_args(argv: list[str]) -> list[str]:
    """Accept only file patterns and grep selectors after `npx playwright test`.

    The adapter deliberately refuses all knobs which could weaken its fixed
    engine, retry, output, or snapshot policy.  It does not execute `npx`.
    """
    if argv[:3] != ["npx", "playwright", "test"]:
        raise ValueError("playwright mode requires `-- npx playwright test`")
    if len(argv) > ARGV_LIMIT or sum(len(a.encode()) for a in argv) > ARGV_BYTES_LIMIT:
        raise ValueError("playwright selection exceeds its argv limit")
    selected, index = [], 3
    options = {"-g", "--grep", "--grep-invert"}
    while index < len(argv):
        arg = argv[index]
        option, equals, value = arg.partition("=")
        if option in options:
            if equals:
                if not value:
                    raise ValueError(f"empty value for {option}")
                selected.append(arg)
            else:
                index += 1
                if index == len(argv) or not argv[index] or argv[index].startswith("-"):
                    raise ValueError(f"missing value for {option}")
                selected.extend((arg, argv[index]))
        elif arg.startswith("-"):
            raise ValueError(f"unsupported playwright selection option: {option}")
        else:
            if not arg:
                raise ValueError("empty positional Playwright selector")
            selected.append(arg)
        index += 1
    return selected


def _json_file(path: pathlib.Path, limit: int, *, parent_fd: int | None = None) -> tuple[bytes, object]:
    """Parse bounded regular-file bytes without following a final report symlink."""
    data = read_regular(path, limit, parent_fd=parent_fd)
    try:
        return data, json.loads(data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("malformed JSON") from error


def _manifest(path: pathlib.Path, expected: str) -> bytes:
    """Require an installed package to match the checked-in runner pin."""
    data, parsed = _json_file(path, POLICY_LIMIT)
    if not isinstance(parsed, dict) or parsed.get("version") != expected:
        raise ValueError("unsupported playwright version")
    return data


def prepare(run_dir: pathlib.Path, checkout: pathlib.Path, argv: list[str], env: dict[str, str], probe: Callable):
    """Resolve installed inputs and return a command with fixed execution policy.

    The caller supplies a private child-environment dictionary; this function
    removes ambient Playwright overrides and sets its fixed report destinations.
    Only the supplied bounded probe may spawn metadata commands. No dependency
    installation, browser download or application build happens here.
    """
    selected = selection_args(argv)
    e2e = checkout / "e2e"
    node = shutil.which("node", path=env.get("PATH", ""))
    if node is None:
        raise ValueError("node is required for recorded Playwright runs")
    node_path = pathlib.Path(node).resolve(strict=True)
    playwright = e2e / "node_modules" / "playwright"
    test = e2e / "node_modules" / "@playwright" / "test"
    manifests = {
        "playwright": _manifest(playwright / "package.json", VERSION),
        "@playwright/test": _manifest(test / "package.json", VERSION),
    }
    lock_data, lock = _json_file(e2e / "package-lock.json", POLICY_LIMIT)
    packages = lock.get("packages") if isinstance(lock, dict) else None
    if not isinstance(packages, dict):
        raise ValueError("package lock does not match installed Playwright version")
    for name in manifests:
        entry = packages.get(f"node_modules/{name}")
        if not isinstance(entry, dict) or entry.get("version") != VERSION:
            raise ValueError("package lock does not match installed Playwright version")
    cli = playwright / "cli.js"
    config = e2e / CONFIG
    reporter = e2e / REPORTER
    inputs = {"cli": cli, "config": config, "reporter": reporter}
    hashes = {name: hashlib.sha256(read_regular(path, REPORT_LIMIT)).hexdigest()
              for name, path in inputs.items()}
    hashes["package_lock"] = hashlib.sha256(lock_data).hexdigest()
    hashes.update({f"{name}_manifest": hashlib.sha256(data).hexdigest() for name, data in manifests.items()})
    removed = sorted(name for name in env if environment_control(name))
    for name in removed:
        del env[name]
    env["PLAYWRIGHT_JSON_OUTPUT_FILE"] = os.fspath(run_dir / REPORT)
    env["FARHELM_PLAYWRIGHT_POLICY_FILE"] = os.fspath(run_dir / POLICY)
    probes = {
        "node": probe([os.fspath(node_path), "--version"]),
        "playwright": probe([os.fspath(node_path), os.fspath(cli), "--version"]),
    }
    if any(not item.complete or item.stdout_sample_truncated for item in probes.values()):
        raise ValueError("Playwright version probe was incomplete")
    node_version = probes["node"].stdout_sample.decode("utf-8", "replace").strip()
    playwright_version = probes["playwright"].stdout_sample.decode("utf-8", "replace").strip()
    if (not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", node_version)
            or playwright_version != f"Version {VERSION}"):
        raise ValueError("Playwright version probe did not identify pinned tools")
    command = [
        os.fspath(node_path), os.fspath(cli), "test", "--config", os.fspath(config),
        "--workers=1", "--retries=0", "--repeat-each=1", "--forbid-only",
        "--fail-on-flaky-tests", "--max-failures=0", "--update-snapshots=none",
        f"--reporter=line,json,{reporter}", "--output", os.fspath(run_dir / ARTIFACTS), *selected,
    ]
    return command, {
        "name": "playwright", "version": VERSION, "removed_environment_names": removed,
        "owned_environment_names": ["PLAYWRIGHT_JSON_OUTPUT_FILE", "FARHELM_PLAYWRIGHT_POLICY_FILE"],
        "hashes": hashes, "version_probes": {name: item.evidence() for name, item in probes.items()},
        "selected_policy": {
            "workers": 1, "retries": 0, "repeat_each": 1,
            "forbid_only": True, "fail_on_flaky_tests": True,
        },
        "report": {
            "complete": False, "reason": "not collected",
            "paths": {"json": REPORT, "policy": POLICY, "artifacts": ARTIFACTS},
        },
    }


def _integer(value, reason):
    """Reject JSON booleans and coercions where a numeric policy is required."""
    if type(value) is not int or value < 0:
        raise ValueError(reason)
    return value


def _string(value, reason):
    """Bound identities before using them as keys or retaining them in summaries."""
    if not isinstance(value, str) or not value or len(value) > TEXT_LIMIT:
        raise ValueError(reason)
    return value


def _choice(value, choices, reason):
    """Malformed compound JSON values must fail validation, not set membership."""
    if not isinstance(value, str) or value not in choices:
        raise ValueError(reason)
    return value


def _spend(budget):
    """Bound aggregate traversal, including empty suites and specs with no cases."""
    budget[0] += 1
    if budget[0] > NODE_LIMIT:
        raise ValueError("report node budget exceeded")


def _walk(suites, budget, depth=0):
    """Walk only typed suite/spec arrays within a shared node and depth budget."""
    if depth > 128 or not isinstance(suites, list):
        raise ValueError("invalid suite nesting")
    for suite in suites:
        _spend(budget)
        if not isinstance(suite, dict):
            raise ValueError("invalid suite")
        specs = suite.get("specs", [])
        if not isinstance(specs, list):
            raise ValueError("invalid suite specs")
        for spec in specs:
            _spend(budget)
            yield spec
        yield from _walk(suite.get("suites", []), budget, depth + 1)


def _projects(config, policy, run_dir):
    """Reconcile two independent policy reports and retain only selected fields.

    Main JSON supplies project IDs used by cases; the supplementary reporter
    supplies resolved engine names unavailable in that JSON representation.
    Neither report alone establishes both the execution budget and engine map.
    """
    if not isinstance(config, dict) or config.get("version") != VERSION:
        raise ValueError("unsupported version")
    if config.get("shard") is not None:
        raise ValueError("unexpected sharding")
    for document in (config, policy):
        if (_integer(document.get("workers"), "invalid workers") != 1
                or document.get("forbidOnly") is not True
                or document.get("failOnFlakyTests") is not True):
            raise ValueError("inconsistent runner policy")
    if (_integer(policy.get("schema_version"), "invalid schema") != 1
            or policy.get("completed") is not True):
        raise ValueError("missing terminal policy")
    _choice(policy.get("status"), {"passed", "failed", "timedout", "interrupted"},
            "invalid terminal status")
    projects = config.get("projects")
    declared = policy.get("projects")
    if (not isinstance(projects, list) or not isinstance(declared, list)
            or len(projects) > 512 or len(declared) > 512):
        raise ValueError("invalid projects")

    ids = {}
    names = set()
    engines = {}
    output = os.fspath(run_dir / ARTIFACTS)
    for document_projects, supplementary in ((projects, False), (declared, True)):
        for project in document_projects:
            if not isinstance(project, dict):
                raise ValueError("invalid project")
            name = _string(project.get("name"), "invalid project name")
            if (_integer(project.get("retries"), "invalid retries") != 0
                    or _integer(project.get("repeatEach"), "invalid repeatEach") != 1
                    or project.get("outputDir") != output):
                raise ValueError("inconsistent project policy")
            if supplementary:
                if name not in names or name in engines:
                    raise ValueError("unknown or duplicate supplementary project")
                engines[name] = _choice(project.get("engine"), {"chromium", "webkit"},
                                        "unknown browser engine")
            else:
                identity = _string(project.get("id"), "invalid project id")
                if identity in ids or name in names:
                    raise ValueError("duplicate project")
                ids[identity] = name
                names.add(name)
    if set(engines) != names or set(engines.values()) != {"chromium", "webkit"}:
        raise ValueError("inconsistent engines")
    safe_projects = [
        {"name": name, "engine": engine, "retries": 0, "repeatEach": 1, "outputDir": ARTIFACTS}
        for name, engine in engines.items()
    ]
    return {identity: engines[name] for identity, name in ids.items()}, safe_projects


def _case(test):
    """Keep actual results separate from Playwright's expected/outcome categories.

    Under the fixed no-retry policy each case has at most one result. Playwright
    1.62 classifies interrupted and unrun cases as skipped outcomes; they remain
    distinct actual categories so consumers cannot mistake them for coverage.
    """
    outcome = _choice(test.get("status"), {"expected", "unexpected", "skipped", "flaky"},
                      "invalid test outcome")
    expected = _choice(test.get("expectedStatus"), {"passed", "failed", "skipped"},
                       "invalid expected status")
    results = test.get("results")
    if not isinstance(results, list) or len(results) > 1:
        raise ValueError("retry policy failure")
    actual = "not_run"
    if results:
        result = results[0]
        if not isinstance(result, dict) or _integer(result.get("retry"), "invalid retry") != 0:
            raise ValueError("invalid result")
        actual = _choice(result.get("status"),
                         {"passed", "failed", "timedOut", "skipped", "interrupted"},
                         "invalid actual status")
    if actual in {"not_run", "skipped", "interrupted"}:
        derived = "skipped"
    else:
        derived = "expected" if actual == expected else "unexpected"
    if outcome != derived:
        raise ValueError("inconsistent case outcome")
    # Setup failures and serial-suite blocking produce synthetic skipped
    # results for cases which never ran. Only an expected skip is intentional;
    # match the pinned runner's didNotRun distinction without losing interrupts.
    if actual == "skipped" and expected != "skipped":
        actual = "not_run"
    return (
        "timed_out" if actual == "timedOut" else actual,
        "outcome_skipped" if outcome == "skipped" else outcome,
        actual == "failed" and expected == "failed",
    )


def _counts(report, engines):
    """Count validated cases, rejecting duplicate identities and inconsistent totals."""
    errors = report.get("errors")
    if not isinstance(errors, list) or any(not isinstance(error, dict) for error in errors):
        raise ValueError("invalid global errors")
    budget = [len(errors)]
    if budget[0] > NODE_LIMIT:
        raise ValueError("report node budget exceeded")
    counts = dict.fromkeys((
        "tests", "passed", "failed", "timed_out", "interrupted", "skipped", "not_run",
        "expected", "unexpected", "flaky", "outcome_skipped", "expected_failures",
    ), 0)
    by_engine = {engine: counts.copy() for engine in ("chromium", "webkit")}
    counts["global_errors"] = len(errors)
    identities = set()
    for spec in _walk(report.get("suites"), budget):
        if not isinstance(spec, dict):
            raise ValueError("invalid spec")
        identity = _string(spec.get("id"), "invalid spec id")
        _string(spec.get("file"), "invalid spec file")
        _string(spec.get("title"), "invalid spec title")
        tests = spec.get("tests")
        if not isinstance(tests, list):
            raise ValueError("missing spec tests")
        for test in tests:
            _spend(budget)
            if not isinstance(test, dict):
                raise ValueError("invalid test")
            project = _string(test.get("projectId"), "invalid case project")
            if project not in engines:
                raise ValueError("unknown case project")
            key = (project, identity)
            if key in identities:
                raise ValueError("duplicate case")
            identities.add(key)
            actual, outcome, expected_failure = _case(test)
            for bucket in (counts, by_engine[engines[project]]):
                bucket["tests"] += 1
                bucket[actual] += 1
                bucket[outcome] += 1
                bucket["expected_failures"] += int(expected_failure)

    stats = report.get("stats")
    if not isinstance(stats, dict):
        raise ValueError("missing stats")
    for reported, counted in (("expected", "expected"), ("unexpected", "unexpected"),
                              ("flaky", "flaky"), ("skipped", "outcome_skipped")):
        if _integer(stats.get(reported), "invalid stats") != counts[counted]:
            raise ValueError("inconsistent counts")
    return counts, by_engine


def collect(run_dir: pathlib.Path) -> dict[str, object]:
    """Validate fixed reports without exporting raw test output or attachment paths.

    Completion means the two reports agree structurally, not that tests passed
    or either engine actually executed a case. Only the consumer can reconcile
    child status, cleanup and requested coverage. Counts appear only after the
    entire report validates; malformed partial evidence has no zero denominator.
    """
    result = {"complete": False, "paths": {
        "json": REPORT, "policy": POLICY, "artifacts": ARTIFACTS,
    }}
    try:
        root_fd = os.open(run_dir, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        try:
            raw, report = _json_file(pathlib.Path(REPORT), REPORT_LIMIT, parent_fd=root_fd)
            _, policy = _json_file(pathlib.Path(POLICY), POLICY_LIMIT, parent_fd=root_fd)
        finally:
            os.close(root_fd)
        if not isinstance(report, dict) or not isinstance(policy, dict):
            raise ValueError("malformed report")
        engines, projects = _projects(report.get("config"), policy, run_dir)
        counts, by_engine = _counts(report, engines)
        result.update(
            complete=True, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest(),
            counts=counts, engines=by_engine,
            policy={"status": policy["status"], "projects": projects},
        )
    except (OSError, RecursionError) as error:
        result["reason"] = type(error).__name__
    except ValueError as error:
        result["reason"] = str(error)
    return result


def success_problem(report: dict[str, object]) -> str | None:
    """Explain why a freshly collected report cannot support a successful command.

    Call only with this module's collect result, not an arbitrary loaded manifest.
    Structural completeness permits failed and interrupted reports so their actual
    counts survive. Success additionally requires a passed terminal policy, no
    unexpected outcomes, and actual execution under each engine. Expected failures
    count as executed assertions; skipped and unstarted cases do not.
    """
    if report.get("complete") is not True:
        return "Playwright report is missing or incomplete"
    counts = report["counts"]
    if report["policy"]["status"] != "passed":
        return "Playwright terminal status is not passed"
    if any(counts[name] for name in ("unexpected", "flaky", "global_errors", "interrupted", "timed_out")):
        return "Playwright report contradicts successful execution"
    for engine in ("chromium", "webkit"):
        bucket = report["engines"][engine]
        if bucket["passed"] + bucket["failed"] == 0:
            return "Playwright did not execute cases under both engines"
    return None
