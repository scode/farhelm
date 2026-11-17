#!/usr/bin/env bash
# Capture the docs website's screenshots (docs/docs-shots/SPEC.md).
#
# One command, so that "refresh the docs screenshots" is a thing an agent can
# be told without it inventing a procedure. Runs the GPUI test harness through
# scripts/docs_shots.py for every page module (or one page), and prints each
# PNG's path. Local only for now; the Mac path comes with the first published
# page. On Linux every shot stops at the size check because it renders at
# scale 1. The PNGs land in the docs website's gitignored
# website/public/docs-shots-local/, where the local preview shows them at
# once. It never publishes.
#
# Screenshots are published in bulk (docs/docs-shots/SPEC.md, "Regenerated in
# bulk"). A complete run writes capture.json beside the shots, naming the main
# commit the checkout is based on, which the publish script requires and
# records in the manifest, with a checksum of every PNG so a publish can tell
# the shots are that run's. A one-page run (--only, for drafting) deletes it,
# and so does a run that cannot vouch for showing main's UI: one whose app
# code differs from that main commit, or one that skipped the build
# (--no-build). Such a full run still captures, for the preview, but exits 3
# to say its shots cannot be
# published; that is decided before the slow part, and said up front.
#
# Published release shots are frozen (docs/docs-shots/SPEC.md, "Release
# shots are frozen"). Release-shot argument validation is retained, but no
# page module captures release shots yet, so recapture is refused.
#
# Usage: scripts/docs-screenshots.sh --local [--only PAGE] [--no-build] [--recapture-release-shot NAME]...
#   --local      capture on this machine through the GPUI test harness
#   --only PAGE  capture only that page's shots (scripts/docs-shots/PAGE.py)
#   --no-build   reuse the harness's existing binaries (the
#                shots can then be previewed but not published)
#   --recapture-release-shot NAME
#                also capture the published release shot NAME
#                (release-notes/v<X-Y-Z>-<what>) from the current UI, to correct it;
#                repeatable; currently refused because no page module captures one
#
# Prerequisites: the GPUI harness's build and runtime tools (docs/gui-agent.md).
# The harness builds the agent app and stages its own fleet and private sshd.
#
# NOTE: no reliance on `set -e` (inert under some harnesses); every step
# carries its own guard.

repo="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
output="$repo/website/public/docs-shots-local"
only=""
build=true
local_mode=false
recapture=()
while [ $# -gt 0 ]; do
  case "$1" in
    --local) local_mode=true ;;
    --only)
      shift
      test -n "${1:-}" || { echo "--only needs a page name" >&2; exit 2; }
      only="$1"
      ;;
    --no-build) build=false ;;
    --recapture-release-shot)
      shift
      test -n "${1:-}" || { echo "--recapture-release-shot needs a shot name" >&2; exit 2; }
      recapture+=("$1")
      ;;
    -h | --help)
      # The header comment, however long it grows.
      awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
      exit 0
      ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

if [ "$local_mode" != true ]; then
  echo "pass --local; the Mac path comes with the first published page" >&2
  exit 2
fi

if [ -n "$only" ]; then
  case "$only" in
    *[!a-z0-9-]* | -*) echo "--only takes a page name in lowercase kebab case, got '$only'" >&2; exit 2 ;;
  esac
fi

# A recapture must name a shot that is frozen now: a published release shot,
# by the definition the publish script shares. Keep the validation before
# refusing the unsupported capture, so a typo still explains what is wrong.
if [ "${#recapture[@]}" -gt 0 ]; then
  if [ -n "$only" ] && [ "$only" != release-notes ]; then
    echo "--recapture-release-shot works only on a full run or with --only release-notes" >&2
    exit 2
  fi
  python3 - "$repo/website/src/data/docs-shots.json" "$repo/website/src/release-notes" "${recapture[@]}" <<'EOF' || exit 2
import json, pathlib, sys
manifest, releases, *names = (pathlib.Path(arg) for arg in sys.argv[1:])
published = set(json.loads(manifest.read_text(encoding="utf-8")).get("shots", {})) if manifest.is_file() else set()
listed = set()
for listing in sorted(releases.glob("*.json")) if releases.is_dir() else []:
    listed.update(shot.get("name") for shot in json.loads(listing.read_text(encoding="utf-8")).get("screenshots", []))
for name in map(str, names):
    if name not in listed or name not in published:
        raise SystemExit(
            f"--recapture-release-shot {name}: not a published release shot "
            "(one that a website/src/release-notes/*.json lists and website/src/data/docs-shots.json holds)"
        )
EOF
  echo "no page module captures release shots yet" >&2
  exit 2
fi

if [ -n "$only" ]; then
  test -f "$repo/scripts/docs-shots/$only.py" || { echo "no shots for page '$only' (scripts/docs-shots/$only.py)" >&2; exit 2; }
fi

# Whether this full run can produce a publishable record, and if not, why.
# Decided before building and capturing, which take minutes.
base=""
unpublishable=""
if [ -z "$only" ]; then
  if [ "$build" != true ]; then
    unpublishable="--no-build skips the builds, so nothing proves the binaries are this checkout's"
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

record="$output/capture.json"
rm -f "$record" || exit 1

# Clear the shots this run replaces, so a shot whose declaration was deleted or
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

# The harness owns the build directory, fleet and capture checks; pages share
# one app run at the scenario's viewport.
engine=(python3 -B "$repo/scripts/docs_shots.py" --out "$output")
[ -z "$only" ] || engine+=(--only "$only")
[ "$build" = true ] || engine+=(--no-build)
"${engine[@]}" || {
  echo "the capture did not complete; see the run directory printed above" >&2
  exit 1
}
if [ -n "$only" ]; then
  found=$(find "$output/$only" -name '*.png' 2>/dev/null | sort)
else
  found=$(find "$output" -name '*.png' 2>/dev/null | sort)
fi
if [ -z "$found" ] && [ "$only" = release-notes ]; then
  # Every shot on that page is a published release shot, skipped as frozen;
  # an empty result is then the expected one rather than a broken run.
  echo "no Release notes shot to capture: published release shots are frozen (see --recapture-release-shot)" >&2
  exit 0
fi
test -n "$found" || { echo "the capture reported success but wrote no PNGs under $output" >&2; exit 1; }
printf '%s\n' "$found"

# The record a publish needs, for a complete run that can vouch for showing
# main's UI at $base (see above). It lists every PNG with its checksum, so a
# shot overwritten afterwards (a stray single-page run, an edit)
# no longer matches and the publish refuses the mix. It also lists the
# published release shots this run recaptured on purpose, each of which must
# be among its PNGs: the publish refuses any other published release shot
# it finds in a record, so a capture by tooling that predates the freeze
# cannot replace one by accident.
test -z "$only" || exit 0
if [ -n "$unpublishable" ]; then
  echo "captured, but these shots cannot be published: $unpublishable" >&2
  exit 3
fi
python3 - "$output" "$base" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "${recapture[@]}" >"$record.tmp" <<'EOF' || exit 1
import hashlib, json, pathlib, sys
output, base, at, *recaptured = sys.argv[1:]
root = pathlib.Path(output)
shots = {str(p.relative_to(root))[: -len(".png")]: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.rglob("*.png"))}
for name in recaptured:
    if name not in shots:
        raise SystemExit(f"asked to recapture {name}, but the run wrote no such shot")
record = {
    "complete": True,
    "captured_from": base,
    "captured_at": at,
    "shots": shots,
    "recaptured_release_shots": sorted(set(recaptured)),
}
json.dump(record, sys.stdout, indent=2)
print()
EOF
mv "$record.tmp" "$record" || exit 1
echo "captured from main at $base" >&2
