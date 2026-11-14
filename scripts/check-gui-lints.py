#!/usr/bin/env python3
"""Prove that every GUI lint ban still fires, by linting a crate that commits one violation per ban.

The GUI crates' bans live in `crates/gui/clippy.toml` (SPEC_impl.md, "GUI architecture", "How the rules are
enforced"). Clippy matches a banned path by name, so a GPUI or GPUI Kit upgrade that moves or renames an item turns its
ban into a silent no-op: nothing warns that the path no longer refers to anything the GUI could call. This check closes
that gap from both ends:

- `crates/gui/lint-demo` holds one demo function per ban, each starting with an `// expect: <lint> [<path>]` line. The
  script runs clippy on that crate with its `violations` feature and requires, for every expect line, a diagnostic of
  that lint inside that function whose message names the path.
- Every entry in the clippy.toml must have an expect line, so a new ban without a demonstration fails too. The demo
  crate depends on everything a ban names (tokio included, which no GUI crate uses), so every ban can be demonstrated.

It also holds the bans' exceptions to their owners. A `#[allow(clippy::disallowed_*)]` (or `expect`) switches a ban
off for one item, and the foundation ownership table (`docs/gpui/foundation-ownership.md`, the lint exceptions column)
lists the few items that may carry one: the owners of what the bans protect. The check fails on such an attribute
anywhere else in `crates/gui/` (the demo crate aside), so a new exception is a change to this list, which review sees,
rather than a line that slips into a file the ownership check only reports.

The same goes for the two switches that act on a whole crate rather than an item: a `[lints.clippy]` (or `[lints.rust]`)
entry in a GUI crate's Cargo.toml that allows or expects a ban, its group or every warning (directly, or through
`lints.workspace = true` and the root manifest's `[workspace.lints]`), and a `clippy.toml` or `.clippy.toml` anywhere
under `crates/gui/` other than the shared one, which clippy would read instead of the shared bans for that crate. No
crate has a reason for either, so both fail outright. The manifests are reported paths in the ownership table, so
without this a crate-level switch would pass both the path check and the attribute scan above.

The crate is a non-default workspace member, so it shares Cargo.lock and the vendored GPUI patches, and its violations
compile only with the feature; ordinary builds see an empty crate. Run from anywhere. `scripts/test-check-gui-lints.py`
covers the parsing with fixtures and runs no cargo.
"""
from __future__ import annotations

import json
import pathlib
import re
import subprocess
import sys
import tomllib

REPO = pathlib.Path(__file__).resolve().parent.parent
CLIPPY_TOML = REPO / "crates/gui/clippy.toml"
DEMO = REPO / "crates/gui/lint-demo"
DEMO_PACKAGE = "farhelm-gui-lint-demo"

FUNCTION = re.compile(r"^\s*fn\s+(\w+)")

# The items that may switch a ban off, as (file under crates/gui, item name): the owners in the ownership table's lint
# exceptions column. Adding one is a foundation change.
EXCEPTION_OWNERS = {
    ("farhelm-gui/src/cache.rs", "element"),
    ("farhelm-gui/src/focus.rs", "apply"),
    ("farhelm-gui/src/focus.rs", "track_raw"),
    ("farhelm-gui/src/layers.rs", "overlay"),
    ("farhelm-gui/src/theme.rs", "raw"),
    ("farhelm-gui/src/ticker.rs", "timer"),
}
# The clippy groups that hold the `disallowed_*` lints; switching one off switches the bans off too, as does rustc's
# `warnings`. One list for both scans below, so a group added here is caught in attributes and manifests alike.
BAN_GROUPS = ("style", "all")
# A lint attribute that would switch a `disallowed_*` ban off: the lint by name, or a group holding it, or every
# warning.
SWITCHES_A_BAN = re.compile(rf"clippy\s*::\s*(disallowed_\w+|{'|'.join(BAN_GROUPS)})\b|\bwarnings\b")
ITEM = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:fn|mod|impl|struct|enum|const|static|trait|type)\b\s*(\w*)")
EXPECT = re.compile(r"^\s*// expect: (\w+)(?: (\S+))?\s*$")


def expectations(source: str) -> list[tuple[str, str, str | None, int, int]]:
    """Return (function, lint, path, first line, last line) for each `// expect:` line, 1-based lines.

    A function's range runs from its `fn` line to the line before the next top-level `fn` (or the end of the file), which
    is all the demo crate's flat layout needs: no nesting, one violation per function.
    """
    lines = source.splitlines()
    starts = [(i + 1, m.group(1)) for i, line in enumerate(lines) if (m := FUNCTION.match(line)) and not line.startswith(" ")]
    found = []
    for index, (start, name) in enumerate(starts):
        end = starts[index + 1][0] - 1 if index + 1 < len(starts) else len(lines)
        for line in lines[start - 1 : end]:
            if m := EXPECT.match(line):
                found.append((name, m.group(1), m.group(2), start, end))
    return found


def banned_paths(config: dict) -> set[str]:
    """Every banned path in a parsed clippy.toml; each needs a demonstration."""
    return {
        entry if isinstance(entry, str) else entry["path"]
        for key in ("disallowed-methods", "disallowed-types")
        for entry in config.get(key, [])
    }


def ban_exceptions(source: str) -> list[tuple[int, str | None]]:
    """(1-based line, item name or None) for each attribute in `source` that allows or expects a `disallowed_*` lint.

    An attribute is read from its `#[` or `#![` line to the line where its brackets close. The item it switches off is
    the next item line after it (attributes and comments between are skipped); an inner attribute (`#![...]`), or one on
    a statement rather than an item, has no item name, and so is never an owner's.
    """
    lines = source.splitlines()
    found = []
    index = 0
    while index < len(lines):
        stripped = re.sub(r"^#\s*(!?)\s*\[", r"#\1[", lines[index].lstrip())
        if not (stripped.startswith("#[") or stripped.startswith("#![")):
            index += 1
            continue
        start, depth, text = index, 0, ""
        while index < len(lines):
            text += lines[index] + "\n"
            depth += lines[index].count("[") - lines[index].count("]")
            index += 1
            if depth <= 0:
                break
        if not re.search(r"\b(allow|expect)\s*\(", text) or not SWITCHES_A_BAN.search(text):
            continue
        item = None
        if not stripped.startswith("#!["):
            follow = index
            while follow < len(lines) and re.match(r"\s*(#|//|$)", lines[follow]):
                follow += 1
            if follow < len(lines) and (match := ITEM.match(lines[follow])):
                item = match.group(1) or None
        found.append((start + 1, item))
    return found


def stray_exceptions(files: list[tuple[str, str]]) -> list[str]:
    """Problems for each ban exception in `files` ((path under crates/gui, source)) that no owner holds, and for each
    owner that does not hold exactly one: an owner's name matched a second time (a method or nested function of that
    name in the same file) is not the owner, and an owner matched never is a stale entry in the list."""
    problems = []
    held: dict[tuple[str, str | None], int] = {}
    for path, source in files:
        for line, item in ban_exceptions(source):
            held[(path, item)] = held.get((path, item), 0) + 1
            if (path, item) not in EXCEPTION_OWNERS:
                problems.append(
                    f"{path}:{line}: a lint ban is switched off outside its owners "
                    "(docs/gpui/foundation-ownership.md, lint exceptions)"
                )
    for owner in sorted(EXCEPTION_OWNERS):
        if held.get(owner, 0) != 1:
            problems.append(f"{owner[0]}: owner {owner[1]} holds {held.get(owner, 0)} ban exceptions, not exactly one")
    return problems


# A lint table key that names a `disallowed_*` ban, a group holding it, or rustc's `warnings`. Keys may use dashes or
# underscores, as Cargo accepts both.
SWITCHED_KEY = re.compile(rf"^(disallowed[_-]\w+|{'|'.join(BAN_GROUPS)}|warnings)$")


def lint_level(value) -> str | None:
    """The level of one lint table entry: `"allow"`, or `{ level = "allow", priority = 1 }`."""
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        return value.get("level")
    return None


def manifest_switches(path: str, manifest: dict, workspace: dict) -> list[str]:
    """Problems for each entry in a GUI crate's parsed Cargo.toml (`path` names it in messages) that switches a ban off
    for the whole crate. A crate that inherits the workspace's lints (`lints.workspace = true`) is checked against the
    root manifest's `[workspace.lints]` instead, since that is the table that applies to it."""
    lints = manifest.get("lints", {})
    origin = path
    if lints.get("workspace") is True:
        lints = workspace.get("workspace", {}).get("lints", {})
        origin = f"{path} (through the workspace's lints)"
    problems = []
    for tool in ("clippy", "rust"):
        for key, value in lints.get(tool, {}).items():
            if SWITCHED_KEY.match(key) and lint_level(value) in ("allow", "expect"):
                problems.append(
                    f"{origin}: [lints.{tool}] {key} = {lint_level(value)} switches a GUI lint ban off for the whole "
                    "crate; no crate may (the bans' only exceptions are items, in the ownership table)"
                )
    return problems


def stray_configs(paths: list[str]) -> list[str]:
    """Problems for each clippy configuration file under crates/gui (paths relative to it) other than the shared one:
    clippy reads the nearest one to a crate, so a second file would replace every shared ban for the crates under it."""
    return [
        f"{path}: a clippy configuration under crates/gui other than the shared clippy.toml would replace its bans"
        for path in paths
        if path != "clippy.toml"
    ]


def diagnostics(messages: list[dict]) -> list[tuple[str, int, str]]:
    """(lint name without `clippy::`, primary line, message) for each clippy diagnostic in cargo's JSON messages."""
    out = []
    for message in messages:
        if message.get("reason") != "compiler-message":
            continue
        diagnostic = message["message"]
        code = (diagnostic.get("code") or {}).get("code") or ""
        if not code.startswith("clippy::"):
            continue
        for span in diagnostic.get("spans", []):
            if span.get("is_primary"):
                out.append((code.removeprefix("clippy::"), span["line_start"], diagnostic["message"]))
    return out


def unmet(expected, found) -> list[str]:
    """Human-readable failures: expectations with no matching diagnostic in their function's line range.

    Clippy quotes the banned path in backticks, and only the exact backticked path counts, so the `gpui::rgb` demo is not
    satisfied by a diagnostic about `gpui::rgba`.
    """
    problems = []
    for function, lint, path, start, end in expected:
        hit = any(
            lint == name and start <= line <= end and (path is None or f"`{path}`" in text)
            for name, line, text in found
        )
        if not hit:
            problems.append(f"{function}: no `{lint}` diagnostic{f' for {path}' if path else ''}")
    return problems


def uncovered(paths: set[str], expected) -> list[str]:
    """Banned paths that no demo function demonstrates."""
    demonstrated = {path for _, _, path, _, _ in expected if path}
    return [f"{path}: banned in crates/gui/clippy.toml but has no demo in crates/gui/lint-demo" for path in sorted(paths - demonstrated)]


def cargo_failure(returncode: int, messages: list[dict], stderr: str) -> str | None:
    """Why clippy produced nothing to check, or None when its output can be checked.

    The demo crate is expected to fail to compile (its unwrap and expect denies are errors), so a non-zero exit alone
    proves nothing. What does: a non-clippy compiler error (the demo no longer type-checks, e.g. after a GPUI upgrade),
    or no compiler messages at all, which means cargo stopped before compiling (a stale Cargo.lock under `--locked`, a
    missing toolchain); its stderr says why, so it is passed through.
    """
    errors = [
        m["message"]["rendered"]
        for m in messages
        if m.get("reason") == "compiler-message"
        and m["message"]["level"] == "error"
        and not ((m["message"].get("code") or {}).get("code") or "").startswith("clippy::")
    ]
    if errors:
        return "the demo crate does not compile:\n" + "".join(errors)
    if not any(m.get("reason") == "compiler-message" for m in messages):
        return f"cargo clippy produced no diagnostics (exit {returncode}); its stderr:\n{stderr}"
    return None


def run() -> int:
    expected = expectations((DEMO / "src/lib.rs").read_text())
    config = tomllib.loads(CLIPPY_TOML.read_text())
    result = subprocess.run(
        ["cargo", "clippy", "--locked", "-p", DEMO_PACKAGE, "--features", "violations", "--message-format=json"],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    messages = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
    # The exception scan needs no build, so it runs and reports even when the demo crate does not compile.
    gui = REPO / "crates/gui"
    sources = [
        (path.relative_to(gui).as_posix(), path.read_text())
        for path in sorted(gui.rglob("*.rs"))
        if DEMO not in path.parents
    ]
    workspace = tomllib.loads((REPO / "Cargo.toml").read_text())
    manifests = [
        path
        for path in sorted(gui.rglob("Cargo.toml"))
        if DEMO not in path.parents and "target" not in path.relative_to(gui).parts
    ]
    configs = [
        path.relative_to(gui).as_posix()
        for name in ("clippy.toml", ".clippy.toml")
        for path in sorted(gui.rglob(name))
        if "target" not in path.relative_to(gui).parts
    ]
    strays = (
        stray_exceptions(sources)
        + [
            problem
            for path in manifests
            for problem in manifest_switches(
                path.relative_to(gui).as_posix(), tomllib.loads(path.read_text()), workspace
            )
        ]
        + stray_configs(configs)
    )
    if failure := cargo_failure(result.returncode, messages, result.stderr):
        for problem in strays:
            print(f"check-gui-lints: {problem}", file=sys.stderr)
        sys.stderr.write(f"check-gui-lints: {failure}")
        return 1
    problems = unmet(expected, diagnostics(messages)) + uncovered(banned_paths(config), expected) + strays
    for problem in problems:
        print(f"check-gui-lints: {problem}", file=sys.stderr)
    print(f"check-gui-lints: {len(expected)} demonstrations, {len(problems)} problems")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(run())
