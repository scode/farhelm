"""Keep a durable per-run timing record for recorded browser-suite runs.

The recorder's evidence lives under a temporary tree (`default_output_root` in
record-test-run.py) that the machine's cleanup reclaims, and a full run's raw
Playwright report is megabytes. How long each part of the suite takes is worth
keeping longer than that: it is what shows whether the suite is getting
slower, and what any later scheduling of work across workers or machines would
be planned from. So after every recorded Playwright run the recorder writes one
small JSON file here, outside the temporary tree, with the run's identity and
labels and the summed test time and outcome counts of each (engine, spec file)
project, the unit Playwright hands to a worker.

The directory is `$XDG_STATE_HOME/farhelm-e2e-timings`, or
`~/.local/state/farhelm-e2e-timings` without an absolute `XDG_STATE_HOME`. That
is a fixed path under the home directory, which AGENTS.md otherwise rules out
for harnesses because concurrent agents collide on shared names. The
maintainer chose it deliberately for this durable record, and the collision
the rule guards against cannot happen here: every run writes only its own
file, named by its run id, and never reads or rewrites another. It is a
sibling of, never inside, `~/.local/state/farhelm`, the live installation's
state.

Writing the record is best effort by contract: it must never change a
recorded run's outcome. The caller catches every failure and notes it in the
run's manifest instead.
"""
from __future__ import annotations

import json
import os
import pathlib
import socket
import tempfile

import test_run_playwright

SCHEMA_VERSION = 1
DIRECTORY_NAME = "farhelm-e2e-timings"


def timings_dir(environment: dict[str, str]) -> pathlib.Path:
    """Where timing records go, from the recorder's ambient environment.

    The environment is passed in, as for the evidence root, so tests choose
    the location through the child environment they start the recorder with
    rather than by touching their own process environment.
    """
    state_home = environment.get("XDG_STATE_HOME")
    if state_home and os.path.isabs(state_home):
        return pathlib.Path(state_home) / DIRECTORY_NAME
    home = environment.get("HOME")
    base = pathlib.Path(home) if home else pathlib.Path.home()
    return base / ".local" / "state" / DIRECTORY_NAME


def unit_timings(run_dir: pathlib.Path) -> list[dict[str, object]]:
    """Per-project summed test time and outcome counts from a run's Playwright report.

    Call only after the recorder's own collection judged the report complete;
    this reads the same bounded files but does not repeat that validation. A
    case without a numeric duration counts as zero time and is flagged on its
    project, so a malformed duration shows up in the record instead of
    silently shortening it.
    """
    _, report = test_run_playwright._json_file(run_dir / test_run_playwright.REPORT,
                                               test_run_playwright.REPORT_LIMIT)
    _, policy = test_run_playwright._json_file(run_dir / test_run_playwright.POLICY,
                                               test_run_playwright.POLICY_LIMIT)
    names = {project["id"]: project["name"] for project in report["config"]["projects"]}
    engines = {project["name"]: project["engine"] for project in policy["projects"]}
    units: dict[str, dict[str, object]] = {}
    for spec in test_run_playwright._walk(report["suites"], [0]):
        for test in spec["tests"]:
            name = names[test["projectId"]]
            unit = units.setdefault(name, {
                "project": name, "engine": engines.get(name), "file": spec.get("file"),
                "tests": 0, "duration_ms": 0, "missing_durations": 0,
                **{status: 0 for status in ("passed", "failed", "timedOut", "skipped", "interrupted", "not_run")},
            })
            unit["tests"] += 1
            results = test.get("results") or []
            if not results:
                unit["not_run"] += 1
                continue
            result = results[0]
            status = result.get("status")
            # The same normalization as the recorder's own counts
            # (test_run_playwright._case): a skipped result for a case that was
            # not meant to be skipped is a setup failure or a blocked serial
            # suite, so it never ran, and must not read as an intentional skip.
            if status == "skipped" and test.get("expectedStatus") != "skipped":
                status = "not_run"
            unit[status if status in unit else "not_run"] += 1
            duration = result.get("duration")
            if isinstance(duration, (int, float)) and not isinstance(duration, bool) and duration >= 0:
                unit["duration_ms"] += int(duration)
            else:
                unit["missing_durations"] += 1
    return sorted(units.values(), key=lambda unit: unit["project"])


def publish(environment: dict[str, str], manifest: dict[str, object], run_dir: pathlib.Path,
            workers: int) -> pathlib.Path:
    """Write this run's timing record atomically and return its path.

    The record names the run (id, start, commit, the caller's selection and
    concurrency labels, the worker count, the host) so a reader can tell
    full-suite runs from selections and compare like with like, then lists the
    units. It is written to a temporary name in the same directory and
    renamed into place, so a reader never sees half a record.
    """
    source = manifest.get("source") or {}
    porcelain = source.get("porcelain") or {}
    record = {
        "schema_version": SCHEMA_VERSION,
        "run_id": manifest["run_id"],
        "started_at": manifest["started_at"],
        "finished_at": manifest.get("finished_at"),
        "command_seconds": (manifest.get("command") or {}).get("duration_seconds"),
        "outcome": manifest.get("outcome"),
        "exit_code": (manifest.get("recorder") or {}).get("exit_code"),
        "head": source.get("head"),
        "worktree_clean": porcelain.get("bytes") == 0 if porcelain.get("complete") else None,
        "labels": {key: (manifest.get("labels") or {}).get(key) for key in ("kind", "selection", "concurrency")},
        "workers": workers,
        "host": socket.gethostname(),
        "units": unit_timings(run_dir),
    }
    directory = timings_dir(environment)
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    # The run id is a UUID, so the name is unique without coordination; the
    # date prefix only keeps a directory listing in time order.
    target = directory / f"{str(manifest['started_at'])[:10]}-{manifest['run_id']}.json"
    descriptor, temporary = tempfile.mkstemp(dir=directory, prefix=".partial-", suffix=".json")
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(record, handle, indent=1, sort_keys=True)
            handle.write("\n")
        os.replace(temporary, target)
    except BaseException:
        pathlib.Path(temporary).unlink(missing_ok=True)
        raise
    return target
