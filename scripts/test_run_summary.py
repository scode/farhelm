"""Build portable counts from retained evidence without exporting private prose.

The caller owns bounded file discovery and reads. These pure classifiers never
infer a cause from narrative text or count missing evidence as an observed zero.
"""
from __future__ import annotations

import datetime
import re
import uuid


CLASSES = {
    "readiness", "replay-live", "fixture-premise", "peer-lifecycle", "process-interference",
    "budget", "ambiguous-observable", "pointer-focus", "substrate", "product",
    "deterministic-regression", "unknown",
}
CAUSES = {"established", "hypothesis", "unknown"}
ACTUAL_BROWSER_COUNTS = ("passed", "failed", "timed_out", "interrupted", "skipped", "not_run")
OUTCOME_BROWSER_COUNTS = ("expected", "unexpected", "flaky", "outcome_skipped")
BROWSER_COUNTS = ("tests", *ACTUAL_BROWSER_COUNTS, *OUTCOME_BROWSER_COUNTS, "expected_failures")
RUST_COUNTS = ("tests", "passed", "skipped", "failures", "errors", "flaky")


def iso_date(value):
    """Accept only the complete calendar-date spelling used in ledger headings."""
    if not re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", value):
        raise ValueError("date must be YYYY-MM-DD")
    return datetime.date.fromisoformat(value)


def tagged_value(body, name, allowed, *, final=False):
    """Classify explicit metadata, keeping missing and malformed historical fields separate."""
    lines = [line.strip() for line in body.splitlines() if line.strip()]
    fields = [line for line in lines if line.startswith(name + ":")]
    if not fields:
        return "missing"
    if len(fields) != 1 or (final and fields[0] != lines[-1]):
        return "invalid"
    value = fields[0][len(name) + 1:].strip()
    return value if value in allowed else "invalid"


def flake_summary(text, since=None):
    """Count dated latent-flake entries and only the cause/class tags actually present.

    A dated entry is the denominator, not an inferred number of distinct tests
    or failures. An optional lower date bound defines 'new'; invalid dates stay
    visible outside that selection because their age cannot be established.
    No heading, test name, hostname, command or raw observation is exported.
    """
    lower = iso_date(since) if since is not None else None
    result = {
        "dated_entries": 0, "before_since": 0, "invalid_date_headings": 0,
        "since": since, "first_date": None, "last_date": None,
        "cause": dict.fromkeys(sorted(CAUSES | {"missing", "invalid"}), 0),
        "class": dict.fromkeys(sorted(CLASSES | {"missing", "invalid"}), 0),
        "mentions_on_recurrence": 0, "mentions_not_kept": 0,
    }
    headings = list(re.finditer(r"^## (.+)$", text, re.MULTILINE))
    for index, match in enumerate(headings):
        heading = match[1]
        candidate = heading.split(" ", 1)[0]
        try:
            date = iso_date(candidate)
        except ValueError:
            result["invalid_date_headings"] += 1
            continue
        if lower is not None and date < lower:
            result["before_since"] += 1
            continue
        result["dated_entries"] += 1
        stamp = date.isoformat()
        result["first_date"] = min(result["first_date"] or stamp, stamp)
        result["last_date"] = max(result["last_date"] or stamp, stamp)
        end = headings[index + 1].start() if index + 1 < len(headings) else len(text)
        body = text[match.end():end]
        result["cause"][tagged_value(body, "Cause", CAUSES, final=True)] += 1
        result["class"][tagged_value(body, "Class", CLASSES)] += 1
        result["mentions_on_recurrence"] += int("on recurrence" in body.casefold())
        result["mentions_not_kept"] += int("not kept" in body.casefold())
    cause_tagged = sum(result["cause"][name] for name in CAUSES)
    class_tagged = sum(result["class"][name] for name in CLASSES)
    result["cause_tagged_entries"] = cause_tagged
    result["class_tagged_entries"] = class_tagged
    result["established_share"] = {
        "numerator": result["cause"]["established"], "dated_entry_denominator": result["dated_entries"],
        "cause_tagged_denominator": cause_tagged,
    }
    return result


def bounded_counts(value, names):
    """Copy only supported nonnegative integer fields into portable totals."""
    if not isinstance(value, dict):
        return None
    if any(type(value.get(name)) is not int or not 0 <= value[name] <= 1_000_000_000 for name in names):
        return None
    return {name: value[name] for name in names}


def browser_counts(value):
    """Validate both actual-result and expected-outcome denominators independently."""
    counts = bounded_counts(value, BROWSER_COUNTS)
    if counts is None:
        return None
    if (counts["tests"] != sum(counts[name] for name in ACTUAL_BROWSER_COUNTS)
            or counts["tests"] != sum(counts[name] for name in OUTCOME_BROWSER_COUNTS)
            or counts["expected_failures"] > min(counts["failed"], counts["expected"])):
        return None
    return counts


def reported_counts(runner):
    """Validate the recorder's exported counts without claiming to re-read raw reports.

    Portable summaries may outlive raw artifacts. These are reported case
    totals from structurally consistent, complete manifest reports, not a new
    execution measurement or authentication of the underlying report files.
    The I/O layer reports raw-file retention separately where it inspects it.
    """
    if not isinstance(runner, dict) or runner.get("prepared") is not True:
        return None
    report = runner.get("report")
    if not isinstance(report, dict) or report.get("complete") is not True:
        return None
    if runner.get("name") == "nextest":
        counts = bounded_counts(report.get("counts"), RUST_COUNTS)
        if counts is None or counts["tests"] != sum(counts[name] for name in ("passed", "skipped", "failures", "errors")):
            return None
        if counts["flaky"] > counts["tests"]:
            return None
        return {"runner": "nextest", "counts": counts}
    if runner.get("name") == "playwright":
        counts = browser_counts(report.get("counts"))
        errors = bounded_counts(report.get("counts"), ("global_errors",))
        engines = report.get("engines")
        if counts is None or errors is None or not isinstance(engines, dict):
            return None
        buckets = {name: browser_counts(engines.get(name)) for name in ("chromium", "webkit")}
        if any(bucket is None for bucket in buckets.values()):
            return None
        if any(counts[name] != sum(bucket[name] for bucket in buckets.values()) for name in BROWSER_COUNTS):
            return None
        return {"runner": "playwright", "counts": {**counts, **errors}, "engines": buckets}
    return None


def canonical_uuid(value):
    """Recognize recorder identities without accepting aliases for the same directory."""
    try:
        return isinstance(value, str) and str(uuid.UUID(value)) == value
    except ValueError:
        return False


def start_date(value):
    """Extract a UTC date only from an explicit timezone-aware recorded timestamp."""
    if not isinstance(value, str) or len(value) > 64:
        return None
    try:
        stamp = datetime.datetime.fromisoformat(value.replace("Z", "+00:00"))
        if stamp.tzinfo is None:
            return None
        return stamp.astimezone(datetime.timezone.utc).date().isoformat()
    except (ValueError, OverflowError):
        return None


def manifest_summary(manifest, run_id):
    """Classify independent command, recorder and coverage evidence into portable fields.

    A failed child can have incomplete diagnostics; a successful child can have
    an invalid runner report. Keep those observations separate. Missing source,
    output or substrate identity remains a coverage gap, never inferred success.
    """
    if (not isinstance(manifest, dict) or type(manifest.get("schema_version")) is not int
            or manifest["schema_version"] != 1 or not canonical_uuid(run_id)
            or manifest.get("run_id") != run_id):
        return None
    outcome = manifest.get("outcome")
    if not isinstance(outcome, str) or outcome not in {"running", "completed", "refused", "timed_out", "interrupted", "recorder-error"}:
        return None
    labels = manifest.get("labels")
    kind = labels.get("kind") if isinstance(labels, dict) else None
    if not isinstance(kind, str) or kind not in {"development", "repetition", "release"}:
        kind = "unknown"
    child = manifest.get("child_status")
    recorder = manifest.get("recorder")
    if not isinstance(child, dict) or not isinstance(recorder, dict):
        return None
    raw = child.get("raw_returncode")
    if raw is not None and (type(raw) is not int or not -255 <= raw <= 255):
        return None
    expected = {"raw_returncode": raw, "exit_code": raw if raw is not None and raw >= 0 else None,
                "signal": -raw if raw is not None and raw < 0 else None}
    if any(type(child.get(key)) is not type(value) or child.get(key) != value for key, value in expected.items()):
        return None
    exit_code = recorder.get("exit_code")
    if outcome == "running":
        if exit_code is not None:
            return None
    elif type(exit_code) is not int or not 0 <= exit_code <= 255:
        return None
    result = {
        "kind": kind, "outcome": outcome, "date": start_date(manifest.get("started_at")),
        "child": "unavailable" if raw is None else "passed" if raw == 0 else "signaled" if raw < 0 else "failed",
        "recorder": "running" if exit_code is None else "passed" if exit_code == 0 else "nonzero",
        "report": reported_counts(manifest.get("runner")), "gaps": [],
    }
    gaps = result["gaps"]
    if result["date"] is None:
        gaps.append("missing_or_invalid_start_date")
    if outcome == "running":
        gaps.append("in_progress_record")
    if raw != 0 and exit_code == 0:
        gaps.append("recorder_success_contradicts_child_status")
    if manifest.get("runner") is None:
        result["report_state"] = "unrequested"
    elif result["report"] is None:
        result["report_state"] = "missing_incomplete_or_invalid"
        gaps.append("runner_report_incomplete")
    else:
        result["report_state"] = "complete_reported_counts"
    source = manifest.get("source")
    if (not isinstance(source, dict) or source.get("complete") is not True
            or not isinstance(source.get("fingerprint_sha256"), str)
            or not re.fullmatch(r"[0-9a-f]{64}", source["fingerprint_sha256"])):
        gaps.append("source_identity_incomplete")
    tmux = manifest.get("tmux")
    if isinstance(tmux, dict) and tmux.get("mode") == "none" and tmux.get("uses_tmux") is False:
        result["substrate"] = "not_used"
    elif isinstance(tmux, dict) and tmux.get("matches_required_substrate") is True:
        actual = tmux.get("actual")
        if (isinstance(actual, dict) and isinstance(actual.get("binary_sha256"), str)
                and re.fullmatch(r"[0-9a-f]{64}", actual["binary_sha256"])):
            result["substrate"] = "reported_pinned"
        else:
            result["substrate"] = "incomplete"
    elif isinstance(tmux, dict) and tmux.get("matches_required_substrate") is False:
        result["substrate"] = "different_or_unavailable"
    else:
        result["substrate"] = "incomplete"
    if result["substrate"] in {"incomplete", "different_or_unavailable"}:
        gaps.append("pinned_substrate_not_established")
    console = manifest.get("console")
    if (not isinstance(console, dict) or console.get("worker_finished") is not True
            or type(console.get("dropped_or_pending_bytes")) is not int
            or console["dropped_or_pending_bytes"] != 0):
        gaps.append("console_incomplete")
    output = manifest.get("output")
    if (not isinstance(output, dict) or output.get("eof_observed") is not True
            or output.get("truncated") is not False or type(output.get("omitted_bytes")) is not int
            or output["omitted_bytes"] != 0):
        gaps.append("output_incomplete")
    traces = manifest.get("test_traces")
    if (not isinstance(traces, dict) or not isinstance(traces.get("collection"), dict)
            or traces["collection"].get("collection_complete") is not True):
        gaps.append("trace_collection_incomplete")
    if (type(recorder.get("forced_cleanup")) is not bool or recorder.get("cleanup_limit") is not None
            or recorder.get("error") is not None):
        gaps.append("cleanup_or_recorder_error")
    return result


def add_counts(destination, source):
    """Add already validated portable integer fields without reinterpreting their meaning."""
    for name, count in source.items():
        destination[name] = destination.get(name, 0) + count


def aggregate_runs(records, since=None):
    """Keep command-attempt and reported-case denominators separate for each run kind."""
    lower = iso_date(since).isoformat() if since is not None else None
    result = {"selected_records": 0, "excluded_before_since": 0, "excluded_undated": 0,
              "first_date": None, "last_date": None, "kinds": {}}
    for item in records:
        date = item["date"]
        if lower is not None and date is None:
            result["excluded_undated"] += 1
            continue
        if lower is not None and date < lower:
            result["excluded_before_since"] += 1
            continue
        result["selected_records"] += 1
        if date is not None:
            result["first_date"] = min(result["first_date"] or date, date)
            result["last_date"] = max(result["last_date"] or date, date)
        group = result["kinds"].setdefault(item["kind"], {
            "run_records": 0, "child_results": {}, "recorder_results": {}, "outcomes": {},
            "coverage_gaps": {}, "report_states": {}, "reported_cases": {}, "substrates": {},
            "raw_report_retention": {}, "output_retention": {}, "output_scans": {},
            "runs_with_skip_marker": 0, "runs_without_complete_marker_scan": 0,
        })
        group["run_records"] += 1
        for target, key in (("child_results", "child"), ("recorder_results", "recorder"), ("outcomes", "outcome"),
                            ("report_states", "report_state"), ("substrates", "substrate"),
                            ("raw_report_retention", "raw_report_retention")):
            add_counts(group[target], {item[key]: 1})
        add_counts(group["coverage_gaps"], {name: 1 for name in item["gaps"]})
        output = item["output_files"]
        add_counts(group["output_retention"], {output["retention"]: 1})
        add_counts(group["output_scans"], {output["scan"]: 1})
        group["runs_with_skip_marker"] += int(output["marker_observed"] is True)
        group["runs_without_complete_marker_scan"] += int(output["scan"] != "complete")
        report = item["report"]
        if report is not None:
            cases = group["reported_cases"].setdefault(report["runner"], {"report_denominator": 0, "counts": {}})
            cases["report_denominator"] += 1
            add_counts(cases["counts"], report["counts"])
            if "engines" in report:
                engines = cases.setdefault("engines", {})
                for name, counts in report["engines"].items():
                    add_counts(engines.setdefault(name, {}), counts)
    for group in result["kinds"].values():
        children = group["child_results"]
        group["nonzero_child_share"] = {
            "numerator": children.get("failed", 0) + children.get("signaled", 0),
            "observed_child_denominator": sum(children.get(name, 0) for name in ("passed", "failed", "signaled")),
            "unavailable_child_results": children.get("unavailable", 0),
        }
    return result
