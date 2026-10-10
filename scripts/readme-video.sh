#!/usr/bin/env bash
# Record the README demo video (docs/readme-video/SPEC.md).
#
# One command, so that "refresh the demo video" is a thing an agent can be
# told without it inventing a procedure. Builds whatever is missing, boots
# the video scenario's fleet through the dedicated Playwright config, runs
# the choreography in e2e/readme-video/capture.spec.ts while recording, and
# prints where the MP4, its marks file, and the review stills landed. It
# never publishes: watch the video, look at the stills, then upload it.
#
# Usage: scripts/readme-video.sh [--output PATH] [--no-build]
#   --output PATH  where to write the MP4 (default target/readme-video/readme-video.mp4)
#   --no-build     skip the cargo and dx builds even if their outputs are missing
#
# Prerequisites are the README screenshot's (rustup, dx, node, Playwright
# with Chromium under e2e/, passwordless ssh to the scenario's remote
# destinations) plus ffmpeg with libx264 on PATH.
#
# NOTE: no reliance on `set -e` (inert under some harnesses); every step
# carries its own guard.

repo="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
output="$repo/target/readme-video/readme-video.mp4"
build=true
while [ $# -gt 0 ]; do
  case "$1" in
    --output)
      shift
      test -n "${1:-}" || { echo "--output needs a path" >&2; exit 2; }
      output="$1"
      ;;
    --no-build) build=false ;;
    -h | --help)
      sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done
case "$output" in
  /*) ;;
  *) output="$PWD/$output" ;;
esac
case "$output" in
  *.mp4) ;;
  *) echo "--output must end in .mp4" >&2; exit 2 ;;
esac

# The fleet launches checkout-local outputs even with --no-build. An outside
# build directory would therefore record older binaries after a good build.
# Resolve an accepted override once so cargo and dx see the same absolute path.
if [ -n "${CARGO_TARGET_DIR:-}" ]; then
  target="$(python3 - "$CARGO_TARGET_DIR" "$repo/target" <<'PYTARGET'
import os
import sys
# An explicit target/ is valid before the first build creates it. realpath
# follows existing symlinks without requiring the final directory to exist.
target = os.path.realpath(sys.argv[1])
if target != os.path.realpath(sys.argv[2]):
    raise SystemExit(1)
print(target)
PYTARGET
  )" || {
    echo "CARGO_TARGET_DIR must name this checkout's target/; capture refused" >&2
    exit 1
  }
  export CARGO_TARGET_DIR="$target"
fi

# Checked before the builds so a missing encoder fails in a second rather
# than after a full staging and choreography run.
ffmpeg -hide_banner -encoders 2>/dev/null | grep -q libx264 || {
  echo "ffmpeg with libx264 is required on PATH" >&2
  exit 1
}

if [ "$build" = true ]; then
  # Unconditional for the reason scripts/readme-screenshot.sh gives: a stale
  # binary or bundle would record the past.
  (cd "$repo" && cargo build) || { echo "cargo build failed" >&2; exit 1; }
  (cd "$repo/crates/farhelm-ui" && dx build --package farhelm-ui --platform web --release) || {
    echo "dx build failed" >&2
    exit 1
  }
fi
test -x "$repo/target/debug/farhelm" || { echo "missing target/debug/farhelm" >&2; exit 1; }
test -f "$repo/target/dx/farhelm-ui/release/web/public/index.html" || { echo "missing the dx web bundle" >&2; exit 1; }
test -d "$repo/e2e/node_modules/@playwright" || {
  echo "e2e/node_modules is missing — run 'cd e2e && npm install && npx playwright install chromium' once" >&2
  exit 1
}

mkdir -p "$(dirname "$output")" || exit 1
rm -f "$output"
(cd "$repo/e2e" && FARHELM_VIDEO_OUTPUT="$output" npx playwright test -c readme-video.config.ts --reporter=line) || {
  echo "the recording did not complete; see e2e/test-results for the trace" >&2
  exit 1
}
test -s "$output" || { echo "the recording reported success but wrote nothing at $output" >&2; exit 1; }
echo "$output"
echo "${output%.mp4}.marks.json"
echo "${output%.mp4}-stills/"
