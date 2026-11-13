#!/usr/bin/env python3
"""Keep the GUI crates' structure: no path to the helm, GPUI and GPUI Kit only in the app crate, no unwrap or expect.

SPEC_impl.md, "GUI architecture", "Where the GUI meets the helm": everything the GUI knows about the helm arrives over
the UI channel, a terminal stream or plain HTTP, even in the desktop app, which runs the helm in its own process. The
desktop binary that embeds the helm is the only place both meet. A dependency edge from a GUI crate to the helm crate
would let GUI code call helm internals and quietly end the split that keeps a remote helm possible, so it is refused
here rather than left to review.

Rules, over `cargo metadata --format-version 1 --locked --all-features` (so an optional dependency cannot hide an edge),
following normal and build dependencies transitively (dev-dependencies do not count; tests may stand up a helm):

- No crate under `crates/gui/` may reach `farhelm-helm` or `farhelm-supervisor`.
- No crate under `crates/gui/` other than the GPUI app crates in `GPUI_CRATES` may reach GPUI (`gpui-pre`) or GPUI Kit
  (`gpui-component`, `gpui-base`). The framework-free crates (grid model, input mapping, escape table, store and draft
  logic) stay testable without a window, and Kit stays behind the app crate's component layer.
- Every crate under `crates/gui/` denies clippy's `unwrap_used` and `expect_used` in its own `[lints.clippy]`, or
  inherits a `[workspace.lints.clippy]` table that does. Nearly all GUI code runs inside platform callbacks, where a
  panic aborts the app on macOS, so a GUI crate that forgets the table is a crate where the rule silently stops.
  Only manifests are read: a crate-level `#![allow(clippy::unwrap_used)]` in source would still switch the rule off,
  and the review gate covers that.

`scripts/test-check-crate-layering.py` covers the rules with fixture metadata and manifests. Runs from any directory.
"""
from __future__ import annotations

import collections
import json
import pathlib
import subprocess
import sys
import tomllib

# The crates under crates/gui/ that ARE the GPUI app and may depend on GPUI and GPUI Kit. Everything else under
# crates/gui/ is framework-free. Adding a crate here is a layering decision; say why in its Cargo.toml.
GPUI_CRATES = {
    "farhelm-gui",
    # One deliberate violation of each GUI lint ban (scripts/check-gui-lints.py), so it needs the GPUI and Kit types.
    "farhelm-gui-lint-demo",
}
HELM_CRATES = {"farhelm-helm", "farhelm-supervisor"}
FRAMEWORK_CRATES = {"gpui-pre", "gpui-component", "gpui-base"}
DENIED_LINTS = ("unwrap_used", "expect_used")


def violations(metadata: dict, gui_dir: pathlib.Path) -> list[str]:
    """Every dependency violation in parsed `cargo metadata` output, as a sentence naming the offending chain."""
    packages = {p["id"]: p for p in metadata["packages"]}
    edges = {
        node["id"]: [
            dep["pkg"]
            for dep in node["deps"]
            if any(kind.get("kind") in (None, "build") for kind in dep.get("dep_kinds", [{"kind": None}]))
        ]
        for node in metadata["resolve"]["nodes"]
    }
    problems = []
    for member in gui_members(metadata, packages, gui_dir):
        name = packages[member]["name"]
        for target, chain in reachable(member, edges, packages).items():
            route = " -> ".join(chain)
            if target in HELM_CRATES:
                problems.append(f"{name} reaches {target} ({route}); GUI crates talk to the helm only over its API")
            elif target in FRAMEWORK_CRATES and name not in GPUI_CRATES:
                problems.append(
                    f"{name} reaches {target} ({route}); only the GPUI app crates may depend on GPUI or GPUI Kit"
                )
    return problems


def lint_violations(metadata: dict, gui_dir: pathlib.Path, workspace_manifest: dict) -> list[str]:
    """GUI crates whose manifest does not deny `unwrap_used` and `expect_used`, directly or through the workspace."""
    packages = {p["id"]: p for p in metadata["packages"]}
    workspace_clippy = workspace_manifest.get("workspace", {}).get("lints", {}).get("clippy", {})
    problems = []
    for member in gui_members(metadata, packages, gui_dir):
        manifest = tomllib.loads(pathlib.Path(packages[member]["manifest_path"]).read_text())
        lints = manifest.get("lints", {})
        clippy = workspace_clippy if lints.get("workspace") is True else lints.get("clippy", {})
        missing = [lint for lint in DENIED_LINTS if level(clippy.get(lint)) not in ("deny", "forbid")]
        if missing:
            problems.append(
                f"{packages[member]['name']} does not deny {', '.join(missing)} in [lints.clippy]; GUI crates never "
                "unwrap or expect outside tests"
            )
    return problems


def level(setting) -> str | None:
    """A lint's level from a `[lints]` entry, which is either a string or a table with a `level` key."""
    return setting.get("level") if isinstance(setting, dict) else setting


def gui_members(metadata: dict, packages: dict, gui_dir: pathlib.Path) -> list[str]:
    """Workspace members whose manifest lives under `gui_dir`, sorted by package name."""
    members = [
        member
        for member in metadata["workspace_members"]
        if pathlib.Path(packages[member]["manifest_path"]).resolve().is_relative_to(gui_dir.resolve())
    ]
    return sorted(members, key=lambda m: packages[m]["name"])


def reachable(start: str, edges: dict, packages: dict) -> dict[str, list[str]]:
    """Package names reachable from `start`, each with one shortest dependency chain of names (breadth first)."""
    seen = {start: [packages[start]["name"]]}
    queue = collections.deque([start])
    while queue:
        current = queue.popleft()
        for dep in edges.get(current, []):
            if dep not in seen:
                seen[dep] = seen[current] + [packages[dep]["name"]]
                queue.append(dep)
    return {packages[pkg]["name"]: chain for pkg, chain in seen.items() if pkg != start}


def main() -> int:
    repo = pathlib.Path(__file__).resolve().parent.parent
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--all-features"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        sys.stderr.write(f"check-crate-layering: cargo metadata failed (exit {result.returncode}):\n{result.stderr}")
        return 1
    metadata = json.loads(result.stdout)
    workspace_manifest = tomllib.loads((repo / "Cargo.toml").read_text())
    gui_dir = repo / "crates/gui"
    problems = violations(metadata, gui_dir) + lint_violations(metadata, gui_dir, workspace_manifest)
    for problem in problems:
        print(f"check-crate-layering: {problem}", file=sys.stderr)
    print(f"check-crate-layering: {len(problems)} violations")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
