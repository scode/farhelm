#!/usr/bin/env python3
"""Suggest explicit recorded hunts for changes against a named Git base.

This is a conservative selection aid, not a dependency proof or a test runner.
It only reads Git metadata and Cargo manifests. Every suggested hunt remains
operator-invoked; unmapped inputs and feature-specific coverage need review.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import pathlib
import re
import shlex
import signal
import sys
import tomllib

# Importing the metadata reader must not change the tree we are planning for.
_previous_bytecode_setting = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    import test_hunt
    import test_run_playwright
finally:
    sys.dont_write_bytecode = _previous_bytecode_setting
    del _previous_bytecode_setting

PATH_LIMIT = 1000
MEMBER_LIMIT = 128
MANIFEST_LIMIT = 64 * 1024
# This helper is imported with #[path] across packages, outside Cargo's
# dependency graph. Keep that known ownership explicit rather than inferring
# that its directory is its only consumer.
SHARED_RUST_HELPERS = {
    "crates/farhelm-testtrace/tests/support/process.rs": {"farhelm-testtrace", "farhelm-testtrace-macros"},
}


def git_output(recorder, cwd, arguments, *, intent=None):
    """Read bounded Git output, refusing partial discovery rather than planning a subset."""
    result = recorder.bounded_probe(["git", *arguments], cwd=cwd, env=dict(os.environ),
                                    intent=intent if intent is not None else recorder.SignalIntent())
    if not result.complete or result.stdout_sample_truncated:
        raise ValueError("Git discovery failed or exceeded its output/time limit")
    return result.stdout_sample.decode("utf-8", "strict")


def relative_path(value):
    """Accept canonical repository-relative paths without silently normalizing traversal."""
    path = pathlib.PurePosixPath(value)
    if not value or path.is_absolute() or ".." in path.parts or path.as_posix() != value:
        raise ValueError("expected a canonical repository-relative path")
    return path


def discover_changes(cwd, base, recorder, *, intent=None):
    """Include tracked edits, deletions and untracked files in a bounded change selection.

    Git diff against the base includes staged and unstaged changes. Rename
    detection is disabled so both the removed and added paths affect selection.
    Separate reads are not an atomic snapshot; regenerate after changing source.
    """
    if not base or base.startswith("-"):
        raise ValueError("base must name a commit, not a Git option")
    root = pathlib.Path(git_output(recorder, cwd, ["rev-parse", "--show-toplevel"], intent=intent).strip()).resolve()
    resolved = git_output(recorder, root, ["rev-parse", "--verify", "--end-of-options", base + "^{commit}"], intent=intent).strip()
    head = git_output(recorder, root, ["rev-parse", "HEAD"], intent=intent).strip()
    changed = git_output(recorder, root, ["diff", "--name-only", "--no-renames", "-z", resolved, "--"], intent=intent)
    untracked = git_output(recorder, root, ["ls-files", "--others", "--exclude-standard", "-z"], intent=intent)
    paths = sorted(set(filter(None, (changed + untracked).split("\0"))))
    if len(paths) > PATH_LIMIT:
        raise ValueError("changed path count exceeds the planning limit")
    for path in paths:
        relative_path(path)
    return root, resolved, head, paths


def workspace(root, reader):
    """Read explicit local members and reverse dependencies without invoking Cargo.

    Dependency edges include normal, development, build and target-specific
    tables. Workspace aliases are resolved to package names. Unsupported member
    glob syntax is refused, so newly added members cannot silently disappear.
    """
    def manifest(path):
        return tomllib.loads(reader(path, MANIFEST_LIMIT).decode("utf-8"))

    document = manifest(root / "Cargo.toml")
    configuration = document.get("workspace", {})
    members = configuration.get("members")
    if not isinstance(members, list) or not 0 < len(members) <= MEMBER_LIMIT:
        raise ValueError("workspace members are missing or exceed the planning limit")
    packages = {}
    for member in members:
        if not isinstance(member, str) or any(char in member for char in "*?["):
            raise ValueError("planner requires explicit workspace member paths")
        relative_path(member)
        if not (root / member).resolve().is_relative_to(root):
            raise ValueError("workspace member escapes the checkout")
        data = manifest(root / member / "Cargo.toml")
        name = data.get("package", {}).get("name")
        if not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9_-]+", name) or name in packages:
            raise ValueError("workspace package name is invalid or duplicated")
        packages[name] = {"path": member, "manifest": data, "dependents": set()}

    inherited = configuration.get("dependencies", {})
    for consumer, package in packages.items():
        data = package["manifest"]
        tables = [data, *data.get("target", {}).values()]
        for table in tables:
            for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
                for alias, dependency in table.get(kind, {}).items():
                    if isinstance(dependency, dict) and dependency.get("workspace") is True:
                        dependency = inherited.get(alias)
                        if dependency is None:
                            raise ValueError("workspace dependency alias has no declaration")
                    target = dependency.get("package", alias) if isinstance(dependency, dict) else alias
                    if target in packages:
                        packages[target]["dependents"].add(consumer)
    return packages


def dependent_closure(initial, packages):
    """Include tests in consumers whose linked code changes with the selected packages."""
    selected = set(initial)
    pending = list(initial)
    while pending:
        for name in packages[pending.pop()]["dependents"] - selected:
            selected.add(name)
            pending.append(name)
    return selected


def selections(paths, packages, exists):
    """Map known test inputs conservatively and disclose every unmapped path.

    Rust source changes widen through package dependencies. A changed Rust
    integration test selects its whole target rather than guessing module names
    from filenames. Browser harness and application changes widen to both-engine
    suite coverage; direct spec edits retain their file selection.
    """
    rust = {}
    browser = set()
    browser_reasons = []
    manual = []
    omitted = []

    def add_package(name, reason):
        rust.setdefault((name, None), set()).add(reason)

    for value in paths:
        path = relative_path(value)
        if path.suffix == ".md":
            omitted.append({"path": value, "reason": "prose requires applicable document checks, not a hunt"})
            continue
        if value in {"Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml", ".cargo/config.toml",
                     ".config/nextest.toml", ".github/release/source-pins.env"}:
            for name in packages:
                add_package(name, value + " affects workspace build inputs")
            browser_reasons.append(value + " can affect the browser application")
            continue
        if value.startswith("e2e/"):
            if value.startswith("e2e/tests/") and value.endswith(".spec.ts") and exists(value):
                browser.add(value.removeprefix("e2e/"))
            else:
                browser_reasons.append(value + " changes shared browser inputs or removes a selector")
            continue
        if value in SHARED_RUST_HELPERS:
            consumers = SHARED_RUST_HELPERS[value]
            for name in consumers & packages.keys():
                add_package(name, value + " is shared by cross-package path imports")
            if not consumers <= packages.keys():
                manual.append(value)
            continue
        owner = next((name for name, package in packages.items()
                      if value.startswith(package["path"] + "/")), None)
        if owner is None:
            manual.append(value)
            continue
        relative = value[len(packages[owner]["path"]) + 1:]
        if relative.startswith("tests/") and relative.endswith(".rs"):
            # Cargo's conventional integration targets are a top-level .rs
            # file or a directory with main.rs. Explicit custom targets need
            # package-wide coverage until their paths are resolved by a human.
            pieces = pathlib.PurePosixPath(relative).parts
            target = pathlib.PurePosixPath(pieces[1]).stem if len(pieces) == 2 else pieces[1]
            custom = packages[owner]["manifest"].get("test", [])
            conventional = (len(pieces) == 2 or exists(packages[owner]["path"] + "/tests/" + target + "/main.rs"))
            if (custom or not conventional or not exists(value)
                    or packages[owner]["manifest"].get("package", {}).get("autotests") is False
                    or target in {"common", "harness", "helpers"}
                    or not re.fullmatch(r"[A-Za-z0-9_-]+", target)):
                add_package(owner, value + " has ambiguous test-target ownership")
                manual.append(value)
            else:
                rust.setdefault((owner, target), set()).add(value + " selects its integration target")
            continue
        if relative.endswith(".rs") or relative == "Cargo.toml" or relative.startswith("assets/"):
            for name in dependent_closure({owner}, packages):
                add_package(name, value + " affects this package or a dependency")
            browser_reasons.append(value + " may affect the application or shared test support")
        else:
            manual.append(value)

    # A package-wide request subsumes its individual integration targets.
    for name, target in list(rust):
        if target is not None and (name, None) in rust:
            rust[(name, None)].update(rust.pop((name, target)))
    return rust, (["."] if browser_reasons else sorted(browser)), browser_reasons, manual, omitted


def browser_groups(selectors):
    """Partition literal selectors within the actual browser adapter's argv limits.

    Both argument count and encoded bytes matter. No selector is discarded or
    replaced with implicit suite-wide coverage merely to fit the command line.
    """
    groups = []
    current = []
    for selector in selectors:
        try:
            test_run_playwright.selection_args(["npx", "playwright", "test", *current, selector])
        except ValueError:
            if not current:
                raise ValueError("a browser selector exceeds the runner's argv limits")
            groups.append(current)
            current = [selector]
            test_run_playwright.selection_args(["npx", "playwright", "test", selector])
        else:
            current.append(selector)
    if current:
        groups.append(current)
    return groups


def build_plan(paths, packages, exists, repeat, timeout):
    """Expose commands, widening reasons and finite cost without executing them."""
    rust, browser, reasons, manual, omitted = selections(paths, packages, exists)
    commands = []
    for (package, target), why in sorted(rust.items(), key=lambda pair: (pair[0][0], pair[0][1] or "")):
        child = ["cargo", "nextest", "run", "-p", package]
        if target is not None:
            child += ["--test", target]
        commands.append({"cwd": ".", "runner": "nextest", "reasons": sorted(why), "child": child,
                         "concurrency": "4 nextest slots; retries 0"})
    if browser:
        # Playwright interprets positional file selectors as regular expressions,
        # so quote filenames rather than letting punctuation change selection.
        selectors = ["."] if browser == ["."] else [re.escape(path) + "$" for path in browser]
        groups = browser_groups(selectors)
        for index, group in enumerate(groups):
            why = list(reasons or ["changed browser spec files"])
            if len(groups) > 1:
                why.append(f"selection partition {index + 1} of {len(groups)} fits runner argv limits")
            commands.append({"cwd": "e2e", "runner": "playwright", "reasons": why,
                             "child": ["npx", "playwright", "test", *group],
                             "concurrency": "1 worker; chromium and webkit; retries 0"})
    for command in commands:
        script = "scripts/hunt-rust-tests.py" if command["runner"] == "nextest" else "../scripts/hunt-browser-tests.py"
        command["argv"] = ["python3", script, "--repeat", str(repeat), "--timeout", str(timeout),
                           "--execute", "--", *command.pop("child")]
        command["shell"] = shlex.join(command["argv"])
        command["maximum_child_command_seconds"] = repeat * timeout
    maximum = len(commands) * repeat * timeout
    if not math.isfinite(maximum):
        raise ValueError("combined child-command budget must be finite")
    return {
        "schema_version": 1, "executed": False, "paths": paths, "commands": commands,
        "manual_review_paths": manual, "omitted_paths": omitted,
        "maximum_sequential_child_command_seconds": maximum,
        "limits": ["metadata, prerequisite builds and cleanup are additional; this is not a wall-clock deadline",
                   "no proof of test dependency completeness; review features, doctests and unmapped inputs",
                   "expensive commands need prepared sandboxes; no installation or build was performed",
                   "regenerate after source edits; each executed hunt records source independently"],
    }


def main(argv=None):
    """Resolve changed inputs and print suggested hunts; never run a suggested command."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--repeat", required=True, type=test_hunt.repeat_count)
    parser.add_argument("--timeout", required=True, type=test_hunt.finite_timeout)
    args = parser.parse_args(argv)
    intent = None
    previous = {}
    try:
        recorder = test_hunt.load_recorder()
        try:
            recorder.require_wait_ownership()
        except recorder.UsageRefusal as error:
            raise ValueError(str(error)) from error
        # Share cancellation across metadata probes. The recorder's bounded
        # probe can then finish owned-child cleanup before planning stops.
        intent = recorder.SignalIntent()
        previous = {number: signal.signal(number, intent.handle) for number in (signal.SIGINT, signal.SIGTERM)}
        root, base, head, paths = discover_changes(pathlib.Path.cwd(), args.base, recorder, intent=intent)
        packages = workspace(root, recorder.test_run_nextest.read_regular)
        result = build_plan(paths, packages, lambda path: (root / path).is_file(), args.repeat, args.timeout)
        result.update(base_commit=base, observed_head=head)
        if intent.received is not None:
            return 128 + intent.received
        print(json.dumps(result, indent=2, sort_keys=True), flush=True)
        return 128 + intent.received if intent.received is not None else 0
    except (ValueError, OSError, RuntimeError, TypeError, AttributeError) as error:
        print(f"plan-test-hunts: refused: {error}", file=sys.stderr)
        return 128 + intent.received if intent is not None and intent.received is not None else 125
    finally:
        for number, handler in previous.items():
            signal.signal(number, handler)


if __name__ == "__main__":
    raise SystemExit(main())
