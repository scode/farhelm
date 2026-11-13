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
    if failure := cargo_failure(result.returncode, messages, result.stderr):
        sys.stderr.write(f"check-gui-lints: {failure}")
        return 1
    problems = unmet(expected, diagnostics(messages)) + uncovered(banned_paths(config), expected)
    for problem in problems:
        print(f"check-gui-lints: {problem}", file=sys.stderr)
    print(f"check-gui-lints: {len(expected)} demonstrations, {len(problems)} problems")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(run())
