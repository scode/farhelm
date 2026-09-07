#!/usr/bin/env python3
"""Print a portable manual summary of retained test evidence and latent-flake tags.

No test execution, retention deletion or ledger update occurs. Redirect stdout
where the summary should be archived; private paths and raw output stay local.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import signal
import sys

_previous_bytecode_setting = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    from test_run_inventory import Budget, DIRECTORY_FLAGS, Inventory, InventoryLimit
    from test_run_summary import aggregate_runs, flake_summary, iso_date
finally:
    sys.dont_write_bytecode = _previous_bytecode_setting
    del _previous_bytecode_setting


def report(args, inventory):
    """Collect declared input directories independently and expose any incomplete discovery."""
    limit = None
    ledger = {"state": "missing_or_invalid"}
    try:
        for kind in ("root", "run", "batch"):
            for path in getattr(args, kind):
                inventory.directory(path, kind)
        ledger_fd = None
        try:
            ledger_fd = os.open(args.flakes.parent, DIRECTORY_FLAGS)
            text = inventory.budget.read(ledger_fd, args.flakes.name, 4 * 1024 * 1024).decode("utf-8")
            ledger = {"state": "read", **flake_summary(text, args.since)}
        except (OSError, ValueError):
            inventory.issue("flake_ledger_unreadable_or_invalid")
        finally:
            if ledger_fd is not None:
                os.close(ledger_fd)
    except InventoryLimit as error:
        limit = str(error)
    benign = {"run_duplicate_copy", "batch_duplicate_copy", "unrecognized_root_entry"}
    complete = limit is None and not any(name not in benign for name in inventory.issues)
    batches = {"batch_denominator": len(inventory.batches), "states": {},
               "declared_planned": 0, "declared_started": 0, "declared_finished": 0, "declared_not_started": 0,
               "date_filter": "not applied: indexes do not record batch start dates"}
    for _, item in inventory.batches.values():
        batches["states"][item["state"]] = batches["states"].get(item["state"], 0) + 1
        for name in ("planned", "started", "finished", "not_started"):
            batches["declared_" + name] += item[name]
    return {
        "schema_version": 1,
        "discovery": {"complete": complete, "limit_reached": limit, "issues": inventory.issues,
                      "input_directories": sum(len(getattr(args, kind)) for kind in ("root", "run", "batch")),
                      "unique_valid_run_records": len(inventory.runs)},
        "since": args.since,
        "runs": aggregate_runs((value for _, value in inventory.runs.values()), args.since),
        "batches": batches, "flake_ledger": ledger,
        "interpretation": [
            "Run, reported-case and dated-flake-entry denominators measure different things.",
            "Reported case totals come from consistent complete manifest reports; raw reports were not revalidated.",
            "Raw presence is unverified retention, not proof of report authenticity or coverage.",
            "SKIPPED is an output marker, not a test count; even a complete scan cannot prove all substrate coverage ran.",
            "Missing evidence and unavailable child results are not observed zero-failure runs.",
            "Only retained inputs are counted; no long-term flake-rate improvement is established.",
        ],
    }


def main(argv=None):
    """Publish one read-only report, preserving cancellation and partial-discovery status."""
    parser = argparse.ArgumentParser(description=__doc__)
    for kind in ("root", "run", "batch"):
        parser.add_argument("--" + kind, action="append", type=pathlib.Path, default=[])
    parser.add_argument("--flakes", type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[1] / "FLAKES.md")
    parser.add_argument("--since")
    parser.add_argument("--scan-output", action="store_true")
    args = parser.parse_args(argv)
    if sum(len(getattr(args, kind)) for kind in ("root", "run", "batch")) > 32:
        parser.error("at most 32 input directories may be selected")
    if args.since is not None:
        try:
            iso_date(args.since)
        except ValueError as error:
            parser.error(str(error))
    budget = Budget()
    inventory = Inventory(budget=budget, scan_output=args.scan_output)
    previous = {number: signal.signal(number, budget.cancel) for number in (signal.SIGINT, signal.SIGTERM)}
    try:
        result = report(args, inventory)
        print(json.dumps(result, indent=2, sort_keys=True), flush=True)
        return 128 + budget.signal if budget.signal is not None else 0 if result["discovery"]["complete"] else 125
    except OSError:
        print("summarize-test-runs: could not publish summary", file=sys.stderr)
        return 128 + budget.signal if budget.signal is not None else 125
    finally:
        for number, handler in previous.items():
            signal.signal(number, handler)


if __name__ == "__main__":
    raise SystemExit(main())
