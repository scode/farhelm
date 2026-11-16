#!/usr/bin/env bash
# Build the release `farhelm-desktop` binary, check it, and stage it with the
# app icon for cargo-dist.
#
# Usage: build-desktop-binary.sh [--self-test]
#
# ## Why this is still a script
#
# `farhelm-desktop` is an ordinary cargo binary, but the release still
# describes it to cargo-dist as a GENERIC package
# (`packaging/farhelm-desktop/dist.toml`, which says why and what may replace
# it), so that its archive also carries `Farhelm.icns`, from which
# `install.sh` assembles `Farhelm.app`. A generic package builds through its
# own `build-command`, which is this script: dist runs it with the package
# directory as the working directory, then collects both files from the
# package's `out-dir`, `target/dist-desktop`.
#
# It refuses to run unless FARHELM_RELEASE_BUILD is set and not empty: that
# variable is what makes `crates/farhelm-helm/build.rs` mark the build as
# release-shaped, and a desktop app built without it embeds a helm that
# defaults to no host payloads, so adding or updating a remote host from the
# shipped app would be refused.
#
# ## What it checks, and why
#
# The checks read the very file about to be archived, because a tag can point
# at a commit no other check ran on, and each of these failures ships
# something nobody notices until a Mac user installs or updates:
#
# - The installer's marker. `install.sh` refuses a desktop binary that lacks a
#   fixed piece of text (the refusal it prints when its own version folder is
#   missing), the sign of a build that understands the side-by-side layout. The
#   text is read at run time from that `grep -qF` line in `install.sh`, not
#   copied here, so this checks exactly what the installer will check; a test in
#   `crates/farhelm-desktop/src/bundle.rs` ties the same line to the Rust code.
#   Not finding that line is a refusal too.
# - No test-automation bridge (the crate's `agent` feature), whose
#   unauthenticated socket lets any process of the same user drive the app
#   (`docs/gui-agent.md`, "Why the bridge never ships"). The GUI already refuses
#   to compile it into an optimized build; this is the check on the artifact.
#   The text looked for is the name of the variable the vendored `gpui-agent`
#   crate reads to start serving its socket, read at run time from
#   `SOCKET_ENV` in `vendor/gpui-agent/crates/protocol/src/lib.rs`. That crate
#   is an optional dependency of `farhelm-gui` that only the `agent` feature
#   turns on, and nothing else in the dependency graph uses its protocol crate,
#   so a binary without the bridge cannot contain the text. Not finding the
#   constant is a refusal too.
# - The license notices. `farhelm-desktop --licenses` prints the notices that
#   must travel with the binary and exits before the app starts, so it runs
#   headless here; its output must hold the committed dependency notices'
#   first line, which must not be empty.
#
# `--self-test` is this script's own validation: it runs the checks and the
# staging against stand-in files in a temporary directory and builds nothing.
#
# Bash 3.2-safe on purpose: macOS runners' `/bin/bash` is still 3.2.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
out_dir="$repo/target/dist-desktop"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Every refusal is one plain line and a non-zero exit. The checks below spell
# out `|| refuse` and `|| exit 1` rather than lean on `set -e`, because the
# self-test runs them inside `if`, where bash ignores errexit.
refuse() {
  echo "build-desktop-binary.sh: $*" >&2
  exit 1
}

# The lines the checks read their texts from: `install.sh`'s marker check and
# the bridge's socket variable (see the header).
# shellcheck disable=SC2016 # `$` here is awk's, matched literally
marker_line='/grep -qF / && /"\$STAGING_DIR\/farhelm-desktop"/'
bridge_line='/pub const SOCKET_ENV: &str = "/'

# source_text <file> <quote> <awk pattern> <what>: print the one quoted string
# (the second field, split on <quote>) of the one line of <file> that matches
# <awk pattern>, or refuse naming <what>. This is how the checks read their
# texts from the files that define them rather than from copies here.
source_text() {
  local found
  found=$(awk -F "$2" "$3 { print \$2 }" "$1") || exit 1
  case $found in
    "" | *$'\n'*) refuse "cannot find the one $4 in $1" ;;
  esac
  printf '%s\n' "$found"
}

# check_binary <binary> <install.sh> <gpui-agent protocol lib.rs> <dependency
# notices>: refuse a binary the installer would turn away, one that carries the
# bridge, or one whose `--licenses` fails or lacks the notices file's first
# line. Runs the binary.
check_binary() {
  local binary=$1 marker bridge header
  [ -f "$binary" ] || refuse "no farhelm-desktop at $binary"
  marker=$(source_text "$2" "'" "$marker_line" "grep -qF line for farhelm-desktop") || exit 1
  bridge=$(source_text "$3" '"' "$bridge_line" "pub const SOCKET_ENV line") || exit 1
  LC_ALL=C grep -qF -e "$marker" "$binary" \
    || refuse "$binary lacks the text install.sh checks for ('$marker'), so the installer would refuse it"
  if LC_ALL=C grep -qF -e "$bridge" "$binary"; then
    refuse "$binary carries the test-automation bridge ($bridge), which must never ship"
  fi
  header=$(sed -n 1p "$4") || exit 1
  [ -n "$header" ] || refuse "the dependency notices $4 start with an empty line"
  "$binary" --licenses >"$work/licenses" || refuse "$binary --licenses failed"
  grep -qF -e "$header" "$work/licenses" \
    || refuse "$binary --licenses does not print the dependency notices ('$header')"
}

# stage <binary> <icon> <directory>: the two files dist collects. The icon is
# staged here and declared under `binaries` because that is the one collection
# path dist 0.32 honors for a generic package.
stage() {
  mkdir -p "$3" || exit 1
  install -m 0755 "$1" "$3/farhelm-desktop" || exit 1
  install -m 0644 "$2" "$3/Farhelm.icns" || exit 1
  echo "== wrote $3/farhelm-desktop and $3/Farhelm.icns"
}

build() {
  local target_dir binary
  # Checked first, before minutes of compiling; the header says why.
  [ -n "${FARHELM_RELEASE_BUILD:-}" ] \
    || refuse "FARHELM_RELEASE_BUILD is not set; without it the app's helm cannot provision remote hosts"
  cd "$repo" || exit 1
  # `-p` because `farhelm-desktop` is outside the workspace's default members.
  # Its own features only: never `agent`.
  echo "== building farhelm-desktop"
  cargo build --release --locked -p farhelm-desktop || exit 1
  # Cargo reads a relative CARGO_TARGET_DIR against its working directory,
  # which is the repository root here.
  target_dir=${CARGO_TARGET_DIR:-target}
  case $target_dir in /*) ;; *) target_dir="$repo/$target_dir" ;; esac
  binary="$target_dir/release/farhelm-desktop"
  check_binary "$binary" "$repo/scripts/install.sh" "$repo/vendor/gpui-agent/crates/protocol/src/lib.rs" \
    "$repo/crates/farhelm/notices/dependencies.txt"
  echo "== $binary carries the installer's marker and the notices, and no bridge"
  stage "$binary" "$repo/packaging/farhelm-desktop/Farhelm.icns" "$out_dir"
}

# --- Self-test ----------------------------------------------------------------
#
# One good stand-in must pass and be staged, and each refusal must fire for its
# own reason. Stand-in binaries are shell scripts whose comments hold the texts
# the checks grep for, and which answer `--licenses` or fail it. The marker and
# the bridge text come from the real `install.sh` and protocol source, so the
# self-test also proves both are still found there.

# stand_in <name> <comment text> <--licenses output, or empty to fail it>
stand_in() {
  printf '#!/bin/sh\n# %s\n' "$2" >"$work/$1"
  if [ -n "$3" ]; then
    printf "printf '%%s\\\\n' '%s'\n" "$3" >>"$work/$1"
  else
    printf 'exit 3\n' >>"$work/$1"
  fi
  chmod 0755 "$work/$1"
}

self_test() {
  local inst=$repo/scripts/install.sh proto=$repo/vendor/gpui-agent/crates/protocol/src/lib.rs
  local marker bridge failures=0 row binary installer protocol notices expected what
  marker=$(source_text "$inst" "'" "$marker_line" "grep -qF line for farhelm-desktop") || exit 1
  bridge=$(source_text "$proto" '"' "$bridge_line" "pub const SOCKET_ENV line") || exit 1
  printf 'Notices header\nmore\n' >"$work/notices"
  printf '\nmore\n' >"$work/empty-header"
  printf 'icon' >"$work/icon"
  printf 'no such line here\n' >"$work/plain"
  stand_in good "$marker" 'Notices header'
  stand_in old 'an older build' 'Notices header'
  stand_in bridge "$marker $bridge" 'Notices header'
  stand_in no-notices "$marker" ''
  stand_in wrong-notices "$marker" 'something else'
  if (check_binary "$work/good" "$inst" "$proto" "$work/notices" \
    && stage "$work/good" "$work/icon" "$work/out") >/dev/null \
    && [ -x "$work/out/farhelm-desktop" ] && [ -f "$work/out/Farhelm.icns" ]; then
    echo "ok: a good binary is checked and staged with the icon"
  else
    echo "FAIL: a good binary was refused or not staged" && failures=1
  fi
  # Each row: the stand-in, the installer, protocol source and notices it is
  # checked against, the refusal expected, and what the case is.
  for row in "old|$inst|$proto|notices|lacks the text install.sh checks for|a build without the marker" \
    "bridge|$inst|$proto|notices|carries the test-automation bridge|a build with the bridge" \
    "no-notices|$inst|$proto|notices|--licenses failed|a build whose --licenses fails" \
    "wrong-notices|$inst|$proto|notices|does not print the dependency notices|a build printing other notices" \
    "good|$inst|$proto|empty-header|start with an empty line|a notices file with an empty first line" \
    "good|$work/plain|$proto|notices|cannot find the one grep -qF line|an install.sh without the line" \
    "good|$inst|$work/plain|notices|cannot find the one pub const SOCKET_ENV|a protocol without SOCKET_ENV"; do
    IFS='|' read -r binary installer protocol notices expected what <<<"$row"
    if (check_binary "$work/$binary" "$installer" "$protocol" "$work/$notices") 2>"$work/err"; then
      echo "FAIL: $what was accepted" && failures=1
    elif grep -qF -e "$expected" "$work/err"; then
      echo "ok: $what is refused: $(cat "$work/err")"
    else
      echo "FAIL: $what is refused for another reason: $(cat "$work/err")" && failures=1
    fi
  done
  # The release-build refusal, through `build` itself. PATH is emptied so that,
  # should the refusal ever stop firing, the build fails at `cargo` instead of
  # compiling, and the message check below reports it.
  if (FARHELM_RELEASE_BUILD='' PATH=/nonexistent build) >/dev/null 2>"$work/err"; then
    echo "FAIL: a build without FARHELM_RELEASE_BUILD was accepted" && failures=1
  elif grep -qF -e "FARHELM_RELEASE_BUILD is not set" "$work/err"; then
    echo "ok: a build without FARHELM_RELEASE_BUILD is refused: $(cat "$work/err")"
  else
    echo "FAIL: a build without FARHELM_RELEASE_BUILD is refused for another reason: $(cat "$work/err")" && failures=1
  fi
  return "$failures"
}

case "${1:-}" in
  "") build ;;
  --self-test) self_test ;;
  *) refuse "usage: build-desktop-binary.sh [--self-test]" ;;
esac
