#!/usr/bin/env bash
# Hold the desktop window's asset requests and the web bundle's asset files to
# the same set.
#
# ## What is being guarded
#
# D6 gives the native app no bundle directory to read from. Every `asset!()`
# the window asks for is answered by `desktop::serve_asset` out of the UI tree
# compiled into the binary (`FARHELM_UI_DIST`, D12), and that tree is whatever
# `dx build --platform web` produced. Nothing at compile time forces the two to
# agree. A file the desktop build references and the web dist does not contain
# is a hard 404 in a native window — no fallback, no console anybody reads —
# and the symptom is a page that renders chrome and no terminal.
#
# The distribution plan wanted this validated on a macOS build. It runs on
# Linux instead: the hashed names are a hash over file CONTENTS (dioxus-cli's
# `opt::hash::add_hash_to_asset`), so they do not vary by platform, and the
# desktop and web builds of this crate differ only in which renderer feature
# is on.
#
# ## Why this builds with dx and not with cargo
#
# `cargo build -p farhelm-desktop` produces a binary whose asset paths are all
# `BundledAsset::PLACEHOLDER_HASH` — the literal sentence "This should be
# replaced by dx as part of the build process...". The `asset!()` macro emits
# that placeholder into a `__ASSETS__` link section, and dx fills the real
# names in AFTER the link, by scanning the built binary's symbol table and
# writing the hashed path back at each symbol's offset. rustc never learns
# them. So a plain-cargo binary cannot be asked what it will request, and this
# script builds through dx to get one that can answer.
#
# Source read for that mechanism: dioxus-cli 0.7.9 (`src/build/assets.rs`,
# `src/opt/hash.rs`) — the newest crates.io source available locally. CI and
# this repository pin dx 0.7.10, whose behaviour is confirmed empirically
# rather than from source: the placeholder check below fails a plain-cargo
# binary and passes a dx-built one, on every run.
#
# NOTE: that is a fact about the SHIPPED binary too, not just about this
# check. Whatever produces the release artifact has to run dx over it, or the
# app will ask for one nonsensical path fourteen times and load nothing.
#
# ## Proving the gate can fail
#
# A comparator nobody has watched reject anything is indistinguishable from
# one that always passes, so the fixture comparisons run on EVERY invocation,
# before the builds: two synthetic sets that differ in one direction each,
# both of which must be rejected, plus an equal pair that must not be. Their
# expected failure output is suppressed. `--self-test` runs only those and
# stops, for when you want the answer in two seconds.
#
# That covers the decision, not the pipeline feeding it. For the whole thing
# end to end, MOVE one unconditional declaration out of `declare_assets!` in
# `crates/farhelm-ui/src/lib.rs` — leave the line itself intact, and leave its
# uses alone — so that it is still compiled, still bundled and still requested
# while dropping out of `all_assets()`. Deleting it instead proves nothing:
# the macro generates the declaration and the inventory entry from the same
# line, so deletion removes the asset from both compared sets (or fails to
# compile, if its uses remain). This script must then report that asset as a
# '-' line and exit non-zero. Restore it afterwards. Done on 2026-08-25 with
# `client-log-shim.js`, which the gate reported with exit 1 — and the unit
# test `asset_declarations_stay_inside_the_inventory_macro` rejects the same
# mutation automatically.
#
# Usage: scripts/check-desktop-assets.sh [--self-test]
# Requires: dx (dioxus-cli, the version pinned in the workspace Cargo.toml),
# the wasm32-unknown-unknown target, and a cargo toolchain.
# Honors CARGO_TARGET_DIR, relative or absolute.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)

# Resolve CARGO_TARGET_DIR ONCE, against the directory this script was invoked
# from, and export the absolute result. A relative value is legal and cargo
# resolves it per process working directory — and this script runs cargo from
# three different directories (the web build from `crates/farhelm-ui`, the
# desktop build from the repository root, its own path checks from wherever
# the caller stood). Left relative, one setting would name three different
# trees: the script could build artifacts and then report them missing, read a
# stale bundle, or `rm -rf` a dx tree belonging to somebody else's build.
if [ -n "${CARGO_TARGET_DIR:-}" ]; then
  mkdir -p "$CARGO_TARGET_DIR"
  CARGO_TARGET_DIR=$(cd "$CARGO_TARGET_DIR" && pwd)
else
  CARGO_TARGET_DIR="$repo/target"
fi

# Both dx builds get a target directory of their OWN, nested under whichever
# one the caller named. dx computes its output tree as `<target dir>/dx` and
# offers no override for it (`--out-dir` is a `dx bundle` flag), so the only
# way to stop stepping on other users of the shared tree is to move the whole
# target directory.
#
# This matters because the shared `target/dx` is genuinely contended: the
# repository supports several jj workspaces over one target directory, and
# `scripts/desktop-smoke.sh` builds into it too. This script wipes that tree
# and then inspects two build generations across four phases; an overlapping
# dx command anywhere in that window could delete the executable it is about
# to run or leave it comparing outputs from different revisions. Isolation
# beats a lock here, because a developer typing `dx build` by hand would not
# take the lock.
#
# The cost is a separate compilation cache — cargo keys that per target
# directory — so this pays a cold build the first time and on every
# dependency change. Acceptable for a CI job that already builds the world.
export CARGO_TARGET_DIR="$CARGO_TARGET_DIR/asset-check"

dist="$CARGO_TARGET_DIR/dx/farhelm-ui/release/web/public"
desktop_bin="$CARGO_TARGET_DIR/dx/farhelm-desktop/debug/linux/app/farhelm-desktop"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# The whole verdict, over two sorted files: what the web bundle contains and
# what the desktop build says it will request. A function so `--self-test` can
# reach it without building anything.
#
# Exit 0 means the sets are equal. Any difference is fatal in BOTH directions:
# an extra requested path is the 404 this gate exists for, and an extra
# bundled file means an asset was removed from the code but is still shipping,
# or that something declares an asset outside `declare_assets!`.
compare_asset_sets() {
  local present=$1 requested=$2 diff_out="$work/diff"
  if diff -u "$present" "$requested" >"$diff_out"; then
    echo "== ok: $(wc -l <"$requested") assets, requested set == bundled set"
    return 0
  fi
  echo "asset sets differ between the desktop build and the web bundle" >&2
  echo "  '-' lines are in the web bundle and never requested by the desktop build" >&2
  echo "  '+' lines are requested by the desktop build and absent from the bundle" >&2
  sed '1,2d' "$diff_out" >&2
  return 1
}

# Prove `compare_asset_sets` still rejects a divergence, in each direction,
# before anything trusts its silence. Runs on every invocation — the real
# inputs below are equal whenever the repository is healthy, so nothing else
# ever exercises the failing branches, and a regression in one of them would
# stay green until the day it was needed. The expected failure output is
# suppressed; only a WRONG verdict says anything.
self_test() {
  printf '/assets/a.js\n/assets/b.css\n' >"$work/st-both"
  printf '/assets/a.js\n' >"$work/st-short"
  compare_asset_sets "$work/st-both" "$work/st-both" >/dev/null ||
    { echo "self-test: equal sets were rejected" >&2; return 1; }
  if compare_asset_sets "$work/st-both" "$work/st-short" >/dev/null 2>&1; then
    echo "self-test: a bundled-but-unrequested asset was accepted" >&2
    return 1
  fi
  if compare_asset_sets "$work/st-short" "$work/st-both" >/dev/null 2>&1; then
    echo "self-test: a requested-but-unbundled asset was accepted" >&2
    return 1
  fi
  echo "== self-test ok: the comparison rejects a divergence in both directions"
}

self_test

if [ "${1:-}" = "--self-test" ]; then
  exit 0
fi

# dx never prunes: its output tree accumulates every generation it has ever
# built, so a directory listing there is a union across history, not a
# snapshot of this build. Comparing against that union would let a removed or
# renamed asset keep passing forever on the strength of a leftover file.
# Wiping first is what makes the dist listing below mean "what this source
# tree produces" — and it is only safe to wipe because the tree is this
# script's own (see the CARGO_TARGET_DIR nesting above).
rm -rf "$CARGO_TARGET_DIR/dx"

echo "== building the web bundle"
# `--package` is not optional here: the workspace has `default-members`, and
# dx 0.7.10 canonicalizes those relative paths against its own working
# directory (`packages/cli/src/workspace.rs`), so a bare `dx build` from
# inside `crates/farhelm-ui` panics on a path that does not exist. Naming the
# package takes an earlier return that never reads `default-members`.
(cd "$repo/crates/farhelm-ui" && dx build --package farhelm-ui --platform web --release) >"$work/dx-web.log" 2>&1 ||
  { cat "$work/dx-web.log" >&2; echo "dx web build failed" >&2; exit 1; }

echo "== building farhelm-desktop"
(cd "$repo" && dx build --package farhelm-desktop --platform desktop) >"$work/dx-desktop.log" 2>&1 ||
  { cat "$work/dx-desktop.log" >&2; echo "dx desktop build failed" >&2; exit 1; }

test -x "$desktop_bin" || { echo "no desktop binary at $desktop_bin" >&2; exit 1; }
test -f "$dist/index.html" || { echo "no web bundle at $dist" >&2; exit 1; }

# Run the binary directly rather than through `cargo run`. `Asset::resolve`
# consults `dioxus_core_types::is_bundled_app()`, which is a RUNTIME look at
# `CARGO_MANIFEST_DIR`: with it set, every asset resolves to its absolute
# source path instead of the `/assets/...` path the webview will request.
# Scrubbing it here means the check still works if someone wraps this script
# in a cargo invocation.
env -u CARGO_MANIFEST_DIR "$desktop_bin" --print-assets | sort -u >"$work/requested"

if grep -q 'replaced by dx' "$work/requested"; then
  echo "the desktop binary carries placeholder asset names; dx did not process it" >&2
  exit 1
fi

# The dist's `assets/` holds our own files plus dx's wasm-bindgen output: the
# module `index.html` loads, and the `.wasm` that module fetches. Neither is an
# `asset!()` and neither is ever requested over the desktop asset route, so
# both are excluded rather than treated as a mismatch. They are identified by
# what they ARE (referenced from index.html; a wasm module) instead of by a
# name pattern, so a change to dx's naming does not quietly turn the
# exclusion into a hole.
grep -o 'assets/[A-Za-z0-9._-]*' "$dist/index.html" | sed 's#^#/#' | sort -u >"$work/glue"
find "$dist/assets" -maxdepth 1 -type f -printf '/assets/%f\n' |
  grep -v '\.wasm$' | sort -u | comm -23 - "$work/glue" >"$work/present"

compare_asset_sets "$work/present" "$work/requested"
