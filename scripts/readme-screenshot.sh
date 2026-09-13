#!/usr/bin/env bash
# Produce the README hero screenshot (docs/readme-hero/SPEC.md).
#
# One command, so that "refresh the README screenshot" is a thing an agent
# can be told without it inventing a procedure. Builds whatever is missing,
# boots the scenario's fleet through the dedicated Playwright config, and
# prints where the PNG landed. It never publishes: look at the picture, then
# run scripts/publish-readme-hero.sh on it.
#
# Usage: scripts/readme-screenshot.sh [--output PATH] [--no-build]
#   --output PATH  where to write the PNG (default target/readme-hero/readme-hero.png)
#   --no-build     skip the cargo and dx builds even if their outputs are missing
#
# Prerequisites beyond the browser suite's own (rustup, dx, node, Playwright
# with Chromium under e2e/): passwordless ssh to every destination the
# scenario's remote hosts name, which the stack script checks before it
# boots anything.
#
# NOTE: no reliance on `set -e` (inert under some harnesses); every step
# carries its own guard.

repo="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
output="$repo/target/readme-hero/readme-hero.png"
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
  echo "e2e/node_modules is missing — run 'cd e2e && npm install && npx playwright install chromium' once" >&2
  exit 1
}

mkdir -p "$(dirname "$output")" || exit 1
rm -f "$output"
# Chromium only; the config defines a single project. Reporter kept to the
# line form so the one line that matters, the output path, is not buried.
(cd "$repo/e2e" && FARHELM_HERO_OUTPUT="$output" npx playwright test -c readme-hero.config.ts --reporter=line) || {
  echo "the capture did not complete; see e2e/test-results for the trace" >&2
  exit 1
}
test -s "$output" || { echo "the capture reported success but wrote nothing at $output" >&2; exit 1; }
echo "$output"
