#!/usr/bin/env bash
# Make the pinned tmux release available at .ci-tmux/tmux, building it from
# the checksummed source pins when the cache did not already supply it, and
# refuse anything but the pinned version.
#
# This is the build half of what scripts/test-tmux-pinned-shutdown.sh used
# to do on its own. It is split out because the pinned build is no longer
# only the focused teardown suite's concern: with the floor becoming the
# regression-tested version (TODO.md's 2026-08-22 decision; the runtime
# check lands later in the same stack), every CI job that drives tmux —
# the full suite, the desktop smoke — has to run on this binary rather than
# on whatever the runner's distro ships below the floor.
# Those jobs prepend the printed directory to PATH; the shutdown suite still
# calls this first and then runs its scenarios.
#
# Idempotent and cache-friendly: a binary that already exists at the target
# path is reused, and the version assertion turns a cached binary of the
# WRONG TMUX RELEASE into a loud failure instead of a green run. That is
# all it can see — a binary built from a stale libevent, ncurses, or zig
# still reports the pinned version — so every other build input must roll
# the cache key instead. The assertion applies to the cached path too,
# deliberately.
#
# Prints the directory holding the binary on stdout — nothing else — so a
# caller can do `PATH="$(scripts/build-pinned-tmux-ci.sh):$PATH"`.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
binary="$repo/.ci-tmux/tmux"

# Earlier versions kept the zig venv at .ci-tmux-python in the checkout, about
# 350 MB that nothing used after the build and that `cargo clean` does not
# reach. Reclaim one an older build left behind.
rm -rf "$repo/.ci-tmux-python"

# Unset first so the guard below can only be satisfied by the pin file
# itself, never by a TMUX_VERSION inherited from the caller's environment.
unset TMUX_VERSION
# shellcheck source-path=SCRIPTDIR
# shellcheck source=../.github/release/source-pins.env
. "$repo/.github/release/source-pins.env"
test -n "${TMUX_VERSION:-}" || {
  echo "source-pins.env did not define TMUX_VERSION" >&2
  exit 2
}

if ! test -x "$binary"; then
  case "$(uname -s)/$(uname -m)" in
    Linux/x86_64) target=x86_64-unknown-linux-musl ;;
    Linux/aarch64 | Linux/arm64) target=aarch64-unknown-linux-musl ;;
    *)
      echo "the pinned tmux build supports Linux x86_64 and arm64" >&2
      exit 2
      ;;
  esac
  # The venv exists only to deliver the pinned zig to this one build, so it
  # lives in a temporary directory that goes away with the build, and pip's
  # wheel cache is skipped for the same reason: nothing would ever read either
  # again, since the finished binary is what the next run reuses.
  python_env=$(mktemp -d "${TMPDIR:-/tmp}/farhelm-tmux-python.XXXXXX")
  trap 'rm -rf "$python_env"' EXIT
  # Build chatter goes to stderr so stdout stays the one-line contract above.
  {
    python3 -m venv "$python_env"
    "$python_env/bin/pip" install --no-cache-dir --require-hashes \
      -r "$repo/.github/release/ziglang-requirements.txt"
    zig=$("$python_env/bin/python" -c \
      'import pathlib, ziglang; print(pathlib.Path(ziglang.__file__).parent / "zig")')
    mkdir -p "$(dirname "$binary")"
    ZIG="$zig" "$repo/scripts/build-private-tmux.sh" "$target" "$binary"
  } >&2
fi

version=$("$binary" -V)
if test "$version" != "tmux $TMUX_VERSION"; then
  echo "expected pinned tmux $TMUX_VERSION at $binary, found: $version" >&2
  exit 1
fi

dirname "$binary"
