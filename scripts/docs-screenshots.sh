#!/usr/bin/env bash
# Capture the docs website's screenshots (docs/docs-shots/SPEC.md).
#
# One command, so that "refresh the docs screenshots" is a thing an agent can
# be told without it inventing a procedure. Builds whatever is missing, boots
# the docs fleet through the dedicated Playwright config, runs every page's
# shots (or one page's), and prints each PNG's path. The PNGs land in the
# docs website's gitignored website/public/docs-shots-local/, where the local
# preview shows them at once. It never publishes: look at every picture, then
# run scripts/publish-docs-shots.sh.
#
# Screenshots are published in bulk (docs/docs-shots/SPEC.md, "Regenerated in
# bulk"). A complete run writes capture.json beside the shots, naming the main
# commit the checkout is based on, which the publish script requires and
# records in the manifest, with a checksum of every PNG so a publish can tell
# the shots are that run's. A one-page run (--only, for drafting) deletes it,
# and so does a run that cannot vouch for showing main's UI: one whose app
# code differs from that main commit, one that skipped the builds
# (--no-build), or one whose builds go to a CARGO_TARGET_DIR other than the
# checkout's own target/, which the stack never runs. Such a full run still
# captures, for the preview, but exits 3 to say its shots cannot be
# published; that is decided before the slow part, and said up front.
#
# Usage: scripts/docs-screenshots.sh [--only PAGE] [--no-build]
#   --only PAGE  capture only that page's shots (e2e/docs-shots/PAGE.spec.ts)
#   --no-build   skip the cargo and dx builds even if their outputs are missing (the
#                shots can then be previewed but not published)
#
# Prerequisites beyond the browser suite's own (rustup, dx, node, Playwright
# with Chromium under e2e/): passwordless ssh to every destination the
# scenario's remote hosts name, which the stack script checks before it boots
# anything.
#
# NOTE: no reliance on `set -e` (inert under some harnesses); every step
# carries its own guard.

repo="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
output="$repo/website/public/docs-shots-local"
only=""
build=true
while [ $# -gt 0 ]; do
  case "$1" in
    --only)
      shift
      test -n "${1:-}" || { echo "--only needs a page name" >&2; exit 2; }
      only="$1"
      ;;
    --no-build) build=false ;;
    -h | --help)
      # The header comment, however long it grows.
      awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
      exit 0
      ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

specs=()
if [ -n "$only" ]; then
  case "$only" in
    *[!a-z0-9-]* | -*) echo "--only takes a page name in lowercase kebab case, got '$only'" >&2; exit 2 ;;
  esac
  test -f "$repo/e2e/docs-shots/$only.spec.ts" || { echo "no shots for page '$only' (e2e/docs-shots/$only.spec.ts)" >&2; exit 2; }
  specs=("docs-shots/$only.spec.ts")
fi

# Whether this full run can produce a publishable record, and if not, why.
# Decided before building and capturing, which take minutes.
base=""
unpublishable=""
if [ -z "$only" ]; then
  if [ "$build" != true ]; then
    unpublishable="--no-build skips the builds, so nothing proves the binaries are this checkout's"
  elif [ -n "${CARGO_TARGET_DIR:-}" ] && [ "$(cd "$CARGO_TARGET_DIR" 2>/dev/null && pwd -P)" != "$(cd "$repo" && pwd -P)/target" ]; then
    unpublishable="CARGO_TARGET_DIR points outside this checkout, but the stack runs this checkout's target/"
  elif ! base=$(git -C "$repo" merge-base HEAD refs/remotes/origin/main 2>/dev/null); then
    unpublishable="cannot find this checkout's base on origin's main (a jj workspace without .git, or no origin/main)"
  elif ! git -C "$repo" diff --quiet "$base" -- crates/ Cargo.toml Cargo.lock; then
    # What the app is built from: the crates and the workspace manifest and
    # lockfile. Shot specs and pages may differ from main; that is the work.
    unpublishable="the app code (crates/, Cargo.toml, Cargo.lock) differs from main at $base"
  fi
  if [ -n "$unpublishable" ]; then
    echo "note: these shots will not be publishable: $unpublishable" >&2
  fi
fi

if [ "$build" = true ]; then
  # Both builds are incremental and cheap when nothing changed; running them
  # unconditionally is what makes "at the current version" true rather than
  # hopeful, since a stale binary or bundle would photograph the past.
  (cd "$repo" && cargo build) || { echo "cargo build failed" >&2; exit 1; }
  (cd "$repo/crates/farhelm-ui" && dx build --package farhelm-ui --platform web --release) || {
    echo "dx build failed" >&2
    exit 1
  }
fi
test -x "$repo/target/debug/farhelm" || { echo "missing target/debug/farhelm" >&2; exit 1; }
test -f "$repo/target/dx/farhelm-ui/release/web/public/index.html" || { echo "missing the dx web bundle" >&2; exit 1; }
test -d "$repo/e2e/node_modules/@playwright" || {
  echo "e2e/node_modules is missing — run 'cd e2e && npm ci && npx playwright install chromium' once" >&2
  exit 1
}

record="$output/capture.json"
rm -f "$record" || exit 1

# Clear the shots this run replaces, so a shot whose test was deleted or
# renamed does not linger and get published. A one-page run clears only that
# page's shots. Only the PNG files go, never the directories: the docs preview
# (Astro's dev server) serves this directory from its public folder, and once
# it has seen the directory deleted it answers 404 for everything in it until
# it is restarted, even after the directory is recreated.
if [ -n "$only" ]; then
  find "$output/$only" -name '*.png' -type f -delete 2>/dev/null
else
  find "$output" -name '*.png' -type f -delete 2>/dev/null
fi
mkdir -p "$output" || exit 1

(cd "$repo/e2e" && FARHELM_DOCS_SHOTS_OUTPUT="$output" npx playwright test -c docs-shots.config.ts --reporter=line "${specs[@]}") || {
  echo "the capture did not complete; see e2e/test-results for the trace" >&2
  exit 1
}
if [ -n "$only" ]; then
  found=$(find "$output/$only" -name '*.png' 2>/dev/null | sort)
else
  found=$(find "$output" -name '*.png' 2>/dev/null | sort)
fi
test -n "$found" || { echo "the capture reported success but wrote no PNGs under $output" >&2; exit 1; }
printf '%s\n' "$found"

# The record a publish needs, for a complete run that can vouch for showing
# main's UI at $base (see above). It lists every PNG with its checksum, so a
# shot overwritten afterwards (a stray single-spec Playwright run, an edit)
# no longer matches and the publish refuses the mix.
test -z "$only" || exit 0
if [ -n "$unpublishable" ]; then
  echo "captured, but these shots cannot be published: $unpublishable" >&2
  exit 3
fi
python3 - "$output" "$base" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >"$record.tmp" <<'EOF' || exit 1
import hashlib, json, pathlib, sys
output, base, at = sys.argv[1:]
root = pathlib.Path(output)
shots = {str(p.relative_to(root))[: -len(".png")]: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.rglob("*.png"))}
json.dump({"complete": True, "captured_from": base, "captured_at": at, "shots": shots}, sys.stdout, indent=2)
print()
EOF
mv "$record.tmp" "$record" || exit 1
echo "captured from main at $base" >&2
