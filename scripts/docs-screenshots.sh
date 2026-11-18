#!/usr/bin/env bash
# Capture the docs website's screenshots (docs/docs-shots/SPEC.md).
#
# One command, so that "refresh the docs screenshots" is a thing an agent can
# be told without it inventing a procedure. Runs the GPUI test harness through
# scripts/docs_shots.py for every page module (or one page), and prints each
# PNG's path. Linux renders at the scenario's scale on a private Mutter.
# --mac ships the capture through the README hero's Mac transport.
# The PNGs land in the docs website's gitignored
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
# shots are frozen"). The engine skips them unless explicitly recaptured.
#
# Usage: scripts/docs-screenshots.sh (--local | --mac DEST) [--only PAGE] [--no-build] [--recapture-release-shot NAME]...
#   --local      capture on this machine through the GPUI test harness
#   --mac DEST   capture on a Mac over ssh, with the FARHELM_MEDIA_* transport
#                settings documented in scripts/readme-screenshot.sh, except
#                FARHELM_MEDIA_MAC: this command always needs --mac DEST
#   --only PAGE  capture only that page's shots (scripts/docs-shots/PAGE.py)
#   --no-build   reuse the harness's existing binaries (the
#                shots can then be previewed but not published)
#   --recapture-release-shot NAME
#                also capture the published release shot NAME
#                (release-notes/v<X-Y-Z>-<what>) from the current UI, to correct it;
#                declare its capture steps in release-notes.py first; repeatable
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
mac=""
recapture=()
while [ $# -gt 0 ]; do
  case "$1" in
    --local) local_mode=true ;;
    --mac)
      shift
      test -n "${1:-}" || { echo "--mac needs an ssh destination" >&2; exit 2; }
      mac="$1"
      ;;
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

if { [ "$local_mode" != true ] && [ -z "$mac" ]; } || { [ "$local_mode" = true ] && [ -n "$mac" ]; }; then
  echo "pass either --local or --mac DEST" >&2
  exit 2
fi

if [ -n "$only" ]; then
  case "$only" in
    *[!a-z0-9-]* | -*) echo "--only takes a page name in lowercase kebab case, got '$only'" >&2; exit 2 ;;
  esac
fi

engine=(python3 -B "$repo/scripts/docs_shots.py")
selection=()
[ -z "$only" ] || selection+=(--only "$only")
for shot in "${recapture[@]}"; do
  selection+=(--recapture-release-shot "$shot")
done
# Reject a mistyped correction before clearing the last capture. SSH joins argv into shell words, so any
# shot name sent to the Mac must first match a validated kebab-case declaration in its release's listing.
if [ "${#recapture[@]}" -gt 0 ]; then
  "${engine[@]}" --check-selection "${selection[@]}" || exit 2
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

# Ship only the capture. The source checkout owns Git provenance and writes
# capture.json after fetching the checked PNGs.
mac_capture_script() {
  cat <<'EOF'
src=$1 lock=$2 out=$3 log=$4 build=$5
shift 5
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export USER="${USER:-$(/usr/bin/id -un)}"
export CARGO_BUILD_JOBS=6
cd "$HOME/$src" || exit 1
mkdir -p "$HOME/$out" || exit 1
set -- --out "$HOME/$out" --lock "$HOME/$lock" "$@"
[ "$build" = true ] || set -- "$@" --no-build
python3 -B scripts/docs_shots.py "$@" </dev/null >"$HOME/$log" 2>&1
status=$?
cat "$HOME/$log"
test "$status" -eq 0
EOF
}

fetch_from_mac() {
  local out="$1"
  # rsync verifies the transfer even when ssh loses the remote exit status.
  rsync -a --include='*/' --include='*.png' --exclude='*' -e "${ssh_cmd[*]}" "$mac:$out/" "$output/" || return 1
  remote "/bin/rm -rf $out" || echo "could not remove the fetched pictures from the Mac" >&2
}

capture() {
  if [ -n "$mac" ]; then
    # shellcheck source=scripts/media-mac.sh
    source "$repo/scripts/media-mac.sh" || return 1
    capture_on_mac "$repo" "$mac" "$build" docs "${selection[@]}"
  else
    local flags=()
    [ "$build" = true ] || flags+=(--no-build)
    "${engine[@]}" --out "$output" "${selection[@]}" "${flags[@]}"
  fi
}
capture
status=$?
if [ "$status" -eq 2 ]; then
  exit 2
elif [ "$status" -ne 0 ]; then
  echo "the capture did not complete; see the run directory printed above" >&2
  exit 1
fi
if [ -n "$only" ]; then
  found=$(find "$output/$only" -name '*.png' 2>/dev/null | sort)
else
  found=$(find "$output" -name '*.png' 2>/dev/null | sort)
fi
if [ -z "$found" ] && [ "$only" = release-notes ]; then
  # An empty page is normal between releases, or when only frozen declarations remain.
  echo "no Release notes shot to capture" >&2
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
