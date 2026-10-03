#!/usr/bin/env bash
# Integration harness for scripts/install.sh.
#
# install.sh is POSIX sh and deliberately has no test hooks of its own — it
# is meant to be readable end to end, not instrumented — so this harness
# tests it the way a user runs it: as a real `/bin/sh` child process against
# a real HTTP server serving real (tiny, fake) release fixtures, with every
# assertion made from the outside (exit status, stdout/stderr text, and
# what changed on disk). Nothing here imports or sources install.sh; every
# scenario is a fresh, fully isolated invocation.
#
# Written in bash (not POSIX sh, unlike install.sh itself): this file is
# never piped into a stranger's shell the way install.sh is, so there is no
# portability constraint driving it, and bash's arrays, [[ ]], and local
# variables make the fixture/assertion bookkeeping far less error-prone.
#
# Isolation rule (CLAUDE.md: tests never mutate the environment of the
# process running them): every install.sh invocation goes through
# run_install(), which execs `/bin/sh install.sh` under `env -i` with an
# explicit, minimal environment — this script's own $PATH, $HOME, and every
# other variable are never touched. "Isolated command directories" (F26)
# means each scenario gets its own synthetic $PATH pointing at a curated
# set of tool symlinks, built once and pared down per scenario, rather than
# this script hiding real tools from itself.
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)
INSTALL_SH="$REPO_ROOT/scripts/install.sh"

# The tmux floor the installer has to advise, read from the release pins rather
# than spelled out in this file. install.sh cannot read the pins itself (it
# runs from `curl | sh` with nothing beside it), so its floor is a literal, and
# the supervisor's TMUX_FLOOR is tied to the same pin by a Rust test. Deriving
# every floor-dependent case below from the pin is what makes a floor bump fail
# here until install.sh's comparison and message follow it; with the floor
# spelled out in this file too, the old installer and its old tests would keep
# agreeing with each other.
read_pinned_tmux_floor() {
  local pins="$REPO_ROOT/.github/release/source-pins.env" lines
  lines=$(grep -c '^TMUX_VERSION=' "$pins") || true
  if [ "$lines" != 1 ]; then
    echo "expected exactly one TMUX_VERSION in $pins, found $lines" >&2
    exit 1
  fi
  sed -n 's/^TMUX_VERSION=//p' "$pins"
}
TMUX_FLOOR=$(read_pinned_tmux_floor)
if ! [[ "$TMUX_FLOOR" =~ ^([0-9]+)\.([0-9]+)([a-z]?)$ ]]; then
  echo "the pinned tmux version '$TMUX_FLOOR' is not MAJOR.MINOR[letter]" >&2
  exit 1
fi
TMUX_FLOOR_MAJOR=${BASH_REMATCH[1]}
TMUX_FLOOR_MINOR=${BASH_REMATCH[2]}
TMUX_FLOOR_LETTER=${BASH_REMATCH[3]}
TMUX_FLOOR_HINT="Farhelm needs tmux $TMUX_FLOOR or newer before it can start."

WORKDIR=$(mktemp -d "${TMPDIR:-/tmp}/farhelm-install-test.XXXXXX")
SERVER_PID=""

cleanup() {
  if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  rm -rf "$WORKDIR"
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# Assertion bookkeeping: a running pass/fail count and a one-line verdict per
# check, printed as it happens (TAP-adjacent, not literal TAP) so a failure
# in scenario 12 of 40 is still easy to find without re-reading everything.
# ---------------------------------------------------------------------------
CHECKS=0
FAILURES=0

pass() {
  CHECKS=$((CHECKS + 1))
  printf 'ok - %s\n' "$1"
}

fail() {
  CHECKS=$((CHECKS + 1))
  FAILURES=$((FAILURES + 1))
  printf 'NOT OK - %s\n' "$1" >&2
  if [ "$#" -gt 1 ]; then
    printf '    %s\n' "$2" >&2
  fi
}

# check DESCRIPTION -- asserts $1 is true (exit 0); everything else is a
# thin wrapper around this so failures always print with the same shape.
check() {
  local desc=$1
  shift
  if "$@"; then
    pass "$desc"
  else
    fail "$desc" "condition failed: $*"
  fi
}

contains() { [[ "$1" == *"$2"* ]]; }
not_contains() { [[ "$1" != *"$2"* ]]; }

# ---------------------------------------------------------------------------
# Fixture construction. Every release fixture is a directory of the exact
# assets a real GitHub release publishes: archives named
# `<package>-<target>.tar.gz` holding one member at `<package>-<target>/
# <binary>`, bare tmux-<target> files, and a SHA256SUMS covering whichever
# of those are present. The "binaries" are two-line shell scripts, per the
# committed fixtures' own convention (crates/farhelm-helm/tests/fixtures/
# release/README.md) — except farhelm's specifically prints `farhelm
# <version>`, because install.sh's own version check depends on that exact
# text.
# ---------------------------------------------------------------------------

# write_binary DIR MEMBER_NAME CONTENT_LINE
write_binary() {
  local dir=$1 member=$2 content=$3
  mkdir -p "$dir"
  printf '#!/bin/sh\necho "%s"\n' "$content" >"$dir/$member"
  chmod 755 "$dir/$member"
}

# build_archive OUT_TAR PACKAGE TARGET CONTENT_LINE [no-icns]
# Builds one well-formed release archive: PACKAGE-TARGET/<binary>, holding a
# shell script that prints CONTENT_LINE. <binary> is "farhelm-desktop" for
# the farhelm-desktop package and "farhelm" for everything else, matching
# RELEASE_ARCHIVES.
#
# A farhelm-desktop archive also carries PACKAGE-TARGET/Farhelm.icns, the way
# real desktop archives have since the app icon shipped, with the
# deterministic content "fake icns: CONTENT_LINE" so bundle assertions can
# byte-compare it. Pass "no-icns" as the fifth argument to build the OLDER
# desktop-archive shape (releases that predate the icon), which install.sh
# refuses before replacing any installed file.
build_archive() {
  local out=$1 package=$2 target=$3 content=$4 icns=${5:-icns}
  local binary=farhelm
  [ "$package" = farhelm-desktop ] && binary=farhelm-desktop
  local stage
  stage=$(mktemp -d "$WORKDIR/archive-stage.XXXXXX")
  write_binary "$stage/$package-$target" "$binary" "$content"
  local members=("$package-$target/$binary")
  if [ "$package" = farhelm-desktop ] && [ "$icns" != no-icns ]; then
    printf 'fake icns: %s\n' "$content" >"$stage/$package-$target/Farhelm.icns"
    chmod 644 "$stage/$package-$target/Farhelm.icns"
    members+=("$package-$target/Farhelm.icns")
  fi
  tar -czf "$out" -C "$stage" "${members[@]}"
  rm -rf "$stage"
}

# build_good_release DIR VERSION -- the Mac CLI/app pair, independently hashed.
# Linux release assets belong to provisioning and are not installer fixtures.
build_good_release() {
  local dir=$1 version=$2
  mkdir -p "$dir"
  build_archive "$dir/farhelm-aarch64-apple-darwin.tar.gz" farhelm aarch64-apple-darwin "farhelm $version"
  build_archive "$dir/farhelm-desktop-aarch64-apple-darwin.tar.gz" farhelm-desktop aarch64-apple-darwin "farhelm-desktop $version"
  (cd "$dir" && sha256sum -- *.tar.gz >SHA256SUMS)
}

# corrupt_checksum DIR ARCHIVE_NAME
# Flips ARCHIVE_NAME's last hex digit in DIR/SHA256SUMS to a DIFFERENT
# digit ("1" unless it was already "1", in which case "0"), so the
# byte-for-byte archive still downloads fine but no longer verifies. Always
# a real change regardless of what digit was originally there -- an
# unconditional "set the last digit to X" would occasionally be a no-op
# (and silently turn this into a checksum-MATCH fixture) on the roughly
# one-in-sixteen real hashes that already end in X.
corrupt_checksum() {
  local dir=$1 archive=$2
  awk -v arch="$archive" '
    $2 == arch {
      last = substr($1, length($1), 1)
      replacement = (last == "1") ? "0" : "1"
      $1 = substr($1, 1, length($1) - 1) replacement
    }
    { print }
  ' "$dir/SHA256SUMS" >"$dir/SHA256SUMS.tmp"
  mv "$dir/SHA256SUMS.tmp" "$dir/SHA256SUMS"
}

# build_two_member_archive_release DIR VERSION
# One Mac CLI archive whose
# member list has TWO entries basename-matching "farhelm". Exercises the
# "more than one candidate member" refusal.
build_two_member_archive_release() {
  local dir=$1 version=$2
  mkdir -p "$dir"
  local stage
  stage=$(mktemp -d "$WORKDIR/two-member-stage.XXXXXX")
  write_binary "$stage/a" farhelm "farhelm $version"
  write_binary "$stage/b" farhelm "farhelm $version"
  tar -czf "$dir/farhelm-aarch64-apple-darwin.tar.gz" -C "$stage" a/farhelm b/farhelm
  rm -rf "$stage"
  (cd "$dir" && sha256sum -- farhelm-aarch64-apple-darwin.tar.gz >SHA256SUMS)
}

# build_nonregular_member_release DIR VERSION
# One archive whose "farhelm" member is a SYMLINK rather than a regular
# file -- the basename match alone would accept it; the type check must
# not.
build_nonregular_member_release() {
  local dir=$1
  mkdir -p "$dir"
  local stage
  stage=$(mktemp -d "$WORKDIR/nonregular-stage.XXXXXX")
  mkdir -p "$stage/farhelm-aarch64-apple-darwin"
  ln -s /nonexistent-target "$stage/farhelm-aarch64-apple-darwin/farhelm"
  # tar does not dereference a symlink source by default -- the archive
  # member itself is a symlink entry, which is exactly the case under test.
  tar -czf "$dir/farhelm-aarch64-apple-darwin.tar.gz" -C "$stage" farhelm-aarch64-apple-darwin/farhelm
  rm -rf "$stage"
  (cd "$dir" && sha256sum -- farhelm-aarch64-apple-darwin.tar.gz >SHA256SUMS)
}

# build_zero_rows_release DIR VERSION (F18)
# A valid archive, but an EMPTY SHA256SUMS -- zero matching rows, not one.
build_zero_rows_release() {
  local dir=$1 version=$2
  mkdir -p "$dir"
  build_archive "$dir/farhelm-aarch64-apple-darwin.tar.gz" farhelm aarch64-apple-darwin "farhelm $version"
  : >"$dir/SHA256SUMS"
}

# build_duplicate_rows_release DIR VERSION (F18)
# A valid archive whose SHA256SUMS lists it TWICE (both lines correct) --
# ambiguous multiplicity even though every individual line verifies.
build_duplicate_rows_release() {
  local dir=$1 version=$2
  mkdir -p "$dir"
  build_archive "$dir/farhelm-aarch64-apple-darwin.tar.gz" farhelm aarch64-apple-darwin "farhelm $version"
  (cd "$dir" && sha256sum -- farhelm-aarch64-apple-darwin.tar.gz >SHA256SUMS)
  cat "$dir/SHA256SUMS" "$dir/SHA256SUMS" >"$dir/SHA256SUMS.tmp"
  mv "$dir/SHA256SUMS.tmp" "$dir/SHA256SUMS"
}

# build_zero_members_release DIR VERSION (F18)
# A checksum-valid archive whose sole member is named something OTHER than
# "farhelm" -- the basename search must find zero candidates, not silently
# extract the wrong file.
build_zero_members_release() {
  local dir=$1 version=$2
  mkdir -p "$dir"
  local stage
  stage=$(mktemp -d "$WORKDIR/zero-members-stage.XXXXXX")
  write_binary "$stage/farhelm-aarch64-apple-darwin" not-farhelm "farhelm $version"
  tar -czf "$dir/farhelm-aarch64-apple-darwin.tar.gz" -C "$stage" farhelm-aarch64-apple-darwin/not-farhelm
  rm -rf "$stage"
  (cd "$dir" && sha256sum -- farhelm-aarch64-apple-darwin.tar.gz >SHA256SUMS)
}

# build_wrong_version_release DIR ACTUAL_VERSION (F18)
# A checksum-valid, correctly-shaped archive whose farhelm prints a
# DIFFERENT version than the one that will be requested -- the
# post-extraction `farhelm --version` sanity check, not the checksum, is
# what must catch this.
build_wrong_version_release() {
  local dir=$1 actual_version=$2
  build_good_release "$dir" "$actual_version"
}

# build_decoy_bypass_release DIR (F18, regression for F10)
# An archive with TWO entries: a REGULAR decoy "farhelm.extra" (whose name
# contains the real member's name as a substring -- what a naive substring
# search over `tar tv` output could mismatch) and the REAL member "farhelm"
# itself, which is a SYMLINK. Basename selection must choose only the exact
# "farhelm" entry, and the regular-file check on it must see the symlink,
# not get fooled by the earlier decoy's "-" type character.
build_decoy_bypass_release() {
  local dir=$1 version=$2
  mkdir -p "$dir"
  local stage
  stage=$(mktemp -d "$WORKDIR/decoy-stage.XXXXXX")
  mkdir -p "$stage/farhelm-aarch64-apple-darwin"
  printf '#!/bin/sh\necho "farhelm %s (decoy, should never run)"\n' "$version" \
    >"$stage/farhelm-aarch64-apple-darwin/farhelm.extra"
  chmod 755 "$stage/farhelm-aarch64-apple-darwin/farhelm.extra"
  ln -s /nonexistent-target "$stage/farhelm-aarch64-apple-darwin/farhelm"
  tar -czf "$dir/farhelm-aarch64-apple-darwin.tar.gz" -C "$stage" \
    farhelm-aarch64-apple-darwin/farhelm.extra farhelm-aarch64-apple-darwin/farhelm
  rm -rf "$stage"
  (cd "$dir" && sha256sum -- farhelm-aarch64-apple-darwin.tar.gz >SHA256SUMS)
}

# ---------------------------------------------------------------------------
# A minimal HTTP server: static files under $1, plus one deliberate
# redirect prefix. Every request under /redirect/<path> answers 302 to
# /redirect-real/<path> rather than serving directly -- GitHub serves
# release assets (SHA256SUMS included) through exactly this kind of
# redirect, and without `-L` on that specific request the installer never
# reaches the manifest at all (that was F1: a real regression, only visible
# through an actual redirect, which is why this fixture exists rather than
# serving every file directly).
# ---------------------------------------------------------------------------
start_server() {
  local root=$1
  local server_py="$WORKDIR/fixture-server.py"
  cat >"$server_py" <<'PY'
import http.server
import os
import sys
import time

root, port, log_path = sys.argv[1], int(sys.argv[2]), sys.argv[3]


class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, fmt, *args):
        with open(log_path, "a") as f:
            f.write("%s %s\n" % (self.address_string(), fmt % args))

    def do_GET(self):
        # A redirect to a host that does not exist (.invalid never
        # resolves): only reachable if the installer pins every connection
        # of its loopback test mode to this machine, which is the point.
        remote = "/redirect-remote/"
        if self.path.startswith(remote):
            target = "http://remote.invalid:%d/redirect-real/%s" % (port, self.path[len(remote):])
            self.send_response(302)
            self.send_header("Location", target)
            self.end_headers()
            return
        prefix = "/redirect/"
        if self.path.startswith(prefix):
            target = "/redirect-real/" + self.path[len(prefix):]
            self.send_response(302)
            self.send_header("Location", target)
            self.end_headers()
            return
        # A deterministic non-404 manifest failure (F25): every request
        # under /sums503/ answers 503, regardless of path, so the generic
        # "download failed (HTTP <code>)" branch has something other than a
        # 404 to be tested against.
        if self.path.startswith("/sums503/"):
            self.send_response(503)
            self.end_headers()
            return
        # A deliberate delay before every response under /slow/, so a test
        # has a reliable window to send install.sh a signal mid-download
        # (F29: proving the INT/TERM/HUP traps actually clean up, not just
        # the EXIT trap on an ordinary success/failure).
        if self.path.startswith("/slow/"):
            time.sleep(2)
        super().do_GET()


os.chdir(root)
http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
PY

  SERVER_PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')
  SERVER_LOG="$WORKDIR/server.log"
  : >"$SERVER_LOG"
  python3 "$server_py" "$root" "$SERVER_PORT" "$SERVER_LOG" &
  SERVER_PID=$!

  local tries=100
  until curl -fsS -o /dev/null "http://127.0.0.1:$SERVER_PORT/good/SHA256SUMS" 2>/dev/null; do
    tries=$((tries - 1))
    if [ "$tries" -le 0 ]; then
      echo "fixture server never came up" >&2
      exit 1
    fi
    sleep 0.1
  done
}

server_request_count() {
  wc -l <"$SERVER_LOG" | tr -d ' '
}

# ---------------------------------------------------------------------------
# A curated $PATH: symlinks to the real tools install.sh needs, built once
# and then pared down (a symlink omitted, or a fake substituted) per
# scenario, so "prerequisite missing" and "tmux at version X" tests control
# exactly one variable each rather than install.sh's whole ambient
# environment.
# ---------------------------------------------------------------------------
# gzip is not something install.sh calls directly or documents as a
# prerequisite -- but GNU tar's `-z` shells out to a separate `gzip`
# binary to decompress, so a toolchain missing it would make every "should
# succeed" scenario fail for a reason that has nothing to do with what is
# actually under test. It has no "omit gzip" scenario of its own.
BASE_TOOLS=(uname mkdir mktemp rmdir ls cp curl tar gzip sha256sum shasum openssl awk sed grep tr head cut mv rm chmod cat sysctl sleep find ln date)

# make_toolchain DIR [OMIT...]
# Populates DIR with symlinks to every tool in BASE_TOOLS found on this
# machine's real PATH, except any named in OMIT.
make_toolchain() {
  local dir=$1
  shift
  local omit=("$@")
  mkdir -p "$dir"
  local tool real skip
  for tool in "${BASE_TOOLS[@]}"; do
    skip=0
    for o in "${omit[@]:-}"; do
      if [ "$tool" = "$o" ]; then
        skip=1
      fi
    done
    if [ "$skip" -eq 1 ]; then
      continue
    fi
    real=$(command -v "$tool" 2>/dev/null || true)
    # Not every tool in BASE_TOOLS exists on every OS (sysctl is macOS-only,
    # for instance) -- that is expected, not a setup failure, so this must
    # not be a bare `A && B` whose failure when $real is empty would abort
    # the whole harness under `set -e`.
    if [ -n "$real" ]; then
      ln -sf "$real" "$dir/$tool"
    fi
  done
  # The installer runs Mac-shaped even on the Linux/BusyBox CI hosts.
  # Replace the symlink, never write through it to the host's real uname.
  rm -f "$dir/uname"
  cat >"$dir/uname" <<'UNAMEEOF'
#!/bin/sh
case "$1" in
  -s) echo Darwin ;;
  -m) echo arm64 ;;
esac
UNAMEEOF
  chmod 755 "$dir/uname"
}

# write_fake_tmux DIR VERSION_OUTPUT
# Adds a `tmux` to DIR that ignores its arguments and just prints
# VERSION_OUTPUT (the whole line, including the leading "tmux " that real
# tmux -V produces) -- used for the closing-message tmux-hint fixtures
# below. An empty VERSION_OUTPUT is used for the "malformed" case, printing
# something that fails to parse as a version at all.
write_fake_tmux() {
  local dir=$1 output=$2
  mkdir -p "$dir"
  printf '#!/bin/sh\necho "%s"\n' "$output" >"$dir/tmux"
  chmod 755 "$dir/tmux"
}

TOOLCHAIN_FULL="$WORKDIR/toolchain-full"
make_toolchain "$TOOLCHAIN_FULL"

# ---------------------------------------------------------------------------
# run_install: the one place every scenario invokes the installer. Always
# `env -i` (this script's own environment never leaks in), always an
# isolated $HOME supplied by the caller, always
# `/bin/sh` naming install.sh by absolute path (so no scenario's stripped-
# down $PATH needs to contain "sh" itself).
#
# Sets globals RC (exit status), OUT (stdout), ERR (stderr) for the caller
# to assert against.
# ---------------------------------------------------------------------------
run_install() {
  local path_dir=$1 home=$2 base_url=$3 version=$4
  # Anything after the four fixed arguments is extra VAR=VALUE assignments
  # spliced into the child environment (still under `env -i`), for the few
  # scenarios whose command doubles need explicit fixture inputs.
  shift 4
  local out_file err_file
  out_file=$(mktemp "$WORKDIR/out.XXXXXX")
  err_file=$(mktemp "$WORKDIR/err.XXXXXX")
  set +e
  env -i \
    PATH="$path_dir" \
    HOME="$home" \
    FARHELM_INSTALL_TEST_BASE_URL="$base_url" \
    FARHELM_VERSION="$version" \
    "$@" \
    /bin/sh "$INSTALL_SH" >"$out_file" 2>"$err_file"
  RC=$?
  set -e
  OUT=$(cat "$out_file")
  ERR=$(cat "$err_file")
}

# seed_legacy_install HOME OLD_DIR -- model ownership records from older installers.
# The installer no longer accepts a custom directory, but its moved/foreign
# record rules still protect old installations. Start from verified current
# bytes, relocate only the fixture's flat files, and rebind both records to
# the old path. Every subsequent installer invocation uses ~/.local/bin.
seed_legacy_install() {
  local home=$1 old=$2
  run_install "$MAC_TOOLS" "$home" "$BASE/good" 1.2.3
  check "legacy fixture: baseline install succeeds" [ "$RC" -eq 0 ]
  mkdir -p "$(dirname "$old")"
  mv "$home/.local/bin" "$old"
  python3 - "$home" "$old" <<'PYRECORD'
import os
from pathlib import Path
import sys
home, old = map(Path, sys.argv[1:])
for record in [old / ".farhelm-installation", home / "Applications/Farhelm.app/Contents/.farhelm-installation"]:
    fields = record.read_bytes().split(b"\0")
    fields[1] = os.fsencode(old.resolve())
    record.write_bytes(b"\0".join(fields))
PYRECORD
  check "legacy fixture: standalone record matches relocated bytes" assert_standalone_record "$old"
  check "legacy fixture: bundle record names the old installation" assert_bundle_record "$home/Applications/Farhelm.app" "$old"
}

# run_install_bg: like run_install, but starts install.sh in the background
# and returns immediately with its pid in BG_PID -- for the one scenario
# (F29) that needs to send it a signal mid-run rather than wait for it to
# finish on its own. Point base_url at a /slow/-prefixed fixture (see
# start_server) so there is a reliable window to act in.
run_install_bg() {
  local path_dir=$1 home=$2 base_url=$3 version=$4
  env -i \
    PATH="$path_dir" \
    HOME="$home" \
    FARHELM_INSTALL_TEST_BASE_URL="$base_url" \
    FARHELM_VERSION="$version" \
    /bin/sh "$INSTALL_SH" >"$WORKDIR/bg-out" 2>"$WORKDIR/bg-err" &
  BG_PID=$!
}

# find_snapshot DIR -- a stable, sorted directory listing used for the
# "nothing outside the bin directory and bundle changed" check. Includes file types
# so a file silently becoming a directory (or vice versa) would show up
# too, not just its name.
find_snapshot() {
  find "$1" -mindepth 0 -exec sh -c 'printf "%s %s\n" "$(stat -c %F "$1" 2>/dev/null || echo "?")" "$1"' _ {} \; | sort
}

# assert_standalone_record INSTALL_DIR
# Compares the complete NUL-delimited record with an independent Python
# oracle. Python receives the pathname as an argument, so it can retain
# trailing newlines and use realpath and byte hashing without copying the
# installer's shell canonicalization.
assert_standalone_record() {
  python3 - "$1" <<'PY'
import hashlib
import os
import sys

install_dir = sys.argv[1]
canonical = os.fsencode(os.path.realpath(install_dir))

def digest(path):
    with open(path, "rb") as source:
        return hashlib.sha256(source.read()).hexdigest().encode()

expected = b"\0".join([
    b"farhelm-standalone",
    canonical,
    digest(os.path.join(install_dir, "farhelm")),
    digest(os.path.join(install_dir, "farhelm-desktop")),
]) + b"\0"
with open(os.path.join(install_dir, ".farhelm-installation"), "rb") as record:
    raise SystemExit(0 if record.read() == expected else 1)
PY
}

# A conventional directory must not acquire pwd's output terminator in its
# recorded identity. The separate awkward-path case allows real newlines.
assert_stable_path_field() {
  python3 - "$1" <<'PY'
import os
import sys

install_dir = sys.argv[1]
with open(os.path.join(install_dir, ".farhelm-installation"), "rb") as record:
    fields = record.read().split(b"\0")
expected = os.fsencode(os.path.realpath(install_dir))
raise SystemExit(0 if fields[1] == expected and not fields[1].endswith(b"\n") else 1)
PY
}

# The repair must replace stale metadata, not merely return success while
# leaving the old release's ownership assertion in place.
assert_records_differ() {
  ! cmp -s "$1" "$2"
}

# assert_bundle_record APP_PATH INSTALL_DIR
# Checks the app-local ownership record against an independent Python oracle
# over the staged bundle files. This proves the record describes content,
# rather than only proving that a file with the expected name exists.
assert_bundle_record() {
  python3 - "$1" "$2" <<'PY'
import hashlib
import os
import sys

app_path, install_dir = sys.argv[1:3]
canonical = os.fsencode(os.path.realpath(install_dir))

def digest(path):
    with open(path, "rb") as source:
        return hashlib.sha256(source.read()).hexdigest().encode()

expected = b"\0".join([
    b"farhelm-app",
    canonical,
    digest(os.path.join(app_path, "Contents", "MacOS", "farhelm")),
    digest(os.path.join(app_path, "Contents", "MacOS", "farhelm-desktop")),
    digest(os.path.join(app_path, "Contents", "Info.plist")),
    digest(os.path.join(app_path, "Contents", "Resources", "Farhelm.icns")),
]) + b"\0"
with open(os.path.join(app_path, "Contents", ".farhelm-installation"), "rb") as record:
    raise SystemExit(0 if record.read() == expected else 1)
PY
}

echo "== Fixture setup =="
WWW="$WORKDIR/www"
mkdir -p "$WWW"
build_good_release "$WWW/good" 1.2.3
mkdir -p "$WWW/redirect-real"
build_good_release "$WWW/redirect-real" 1.2.3
mkdir -p "$WWW/norelease" # deliberately empty: every request 404s
build_good_release "$WWW/badchecksum" 1.2.3
corrupt_checksum "$WWW/badchecksum" farhelm-aarch64-apple-darwin.tar.gz
build_two_member_archive_release "$WWW/twomember" 1.2.3
build_nonregular_member_release "$WWW/nonregular"
build_good_release "$WWW/prerelease" 1.2.3-rc.1
build_good_release "$WWW/prerelease-dev" 1.2.3-dev.1
build_good_release "$WWW/unreleased" 0.0.0-unreleased
mkdir -p "$WWW/slow"
cp -r "$WWW/good"/. "$WWW/slow/"
mkdir -p "$WWW/sums503" # never actually read: the server 503s the whole prefix
build_zero_rows_release "$WWW/zerorows" 1.2.3
build_duplicate_rows_release "$WWW/duprows" 1.2.3
build_zero_members_release "$WWW/zeromembers" 1.2.3
build_wrong_version_release "$WWW/wrongversion" 1.2.2
build_decoy_bypass_release "$WWW/decoybypass" 1.2.3
# A second, genuinely DIFFERENT release for the macOS rollback oracle (F19):
# using the same version twice would make "restored the old bytes" and
# "kept the new bytes" indistinguishable, since both would just happen to
# be identical content.
build_good_release "$WWW/good-v2" 1.2.4

start_server "$WWW"
BASE="http://127.0.0.1:$SERVER_PORT"
echo "fixture server: $BASE"

# ===========================================================================
# Unsupported platforms must refuse before even a download or mkdir. The
# fixture HOME does not exist, so its absence proves no installer path ran.
# ===========================================================================
LINUX_TOOLS="$WORKDIR/toolchain-linux"
mkdir -p "$LINUX_TOOLS"
cp -a "$TOOLCHAIN_FULL"/. "$LINUX_TOOLS/"
rm -f "$LINUX_TOOLS/uname" "$LINUX_TOOLS/curl"
cat >"$LINUX_TOOLS/uname" <<'UNAMEEOF'
#!/bin/sh
case "$1" in
  -s) echo Linux ;;
  -m) echo x86_64 ;;
esac
UNAMEEOF
cat >"$LINUX_TOOLS/curl" <<'CURLEOF'
#!/bin/sh
printf 'unexpected download\n' >"$HOME-download"
exit 1
CURLEOF
chmod 755 "$LINUX_TOOLS/uname" "$LINUX_TOOLS/curl"
HOME_LINUX="$WORKDIR/home-linux-refused"
run_install "$LINUX_TOOLS" "$HOME_LINUX" "$BASE/good" 1.2.3
check "Linux refuses with exit 1" [ "$RC" -eq 1 ]
check "Linux leaves HOME absent" [ ! -e "$HOME_LINUX" ]
check "Linux never downloads" [ ! -e "$HOME_LINUX-download" ]
check "Linux has no stdout report" [ -z "$OUT" ]
LINUX_REFUSAL="$(cat <<'MESSAGE'
❌ This installer only supports macOS for now.
   Linux is supported for running a helm and session hosts; only this installer is
   limited, and that will be fixed. If you want to install on Linux, please open an
   issue and it will be prioritized: https://github.com/scode/farhelm/issues
MESSAGE
)"
check "Linux refusal matches the approved text" [ "$ERR" = "$LINUX_REFUSAL" ]

# ===========================================================================
# Scenario: fresh install (also the base case every later scenario's
# "update" and "rollback" tests build on).
# ===========================================================================
echo
echo "== fresh install =="
HOME1="$WORKDIR/home1"
INSTALL1="$HOME1/.local/bin"
mkdir -p "$HOME1"
run_install "$TOOLCHAIN_FULL" "$HOME1" "$BASE/good" 1.2.3
check "fresh install exits 0" [ "$RC" -eq 0 ]
check "fresh install writes farhelm" [ -x "$INSTALL1/farhelm" ]
check "fresh install reports its own version" contains "$("$INSTALL1/farhelm" --version)" "farhelm 1.2.3"
check "fresh install installs mode 0755" [ "$(stat -c %a "$INSTALL1/farhelm")" = "755" ]
check "fresh install reports Installed" contains "$OUT" "Farhelm 1.2.3 is installed."
check "fresh install writes owner-only standalone metadata" [ "$(stat -c %a "$INSTALL1/.farhelm-installation")" = "600" ]
check "fresh install writes the exact standalone metadata fields" assert_standalone_record "$INSTALL1"
check "fresh install has no appended canonical-path newline" assert_stable_path_field "$INSTALL1"
check "fresh install has no leftover staging/lock/backup dot-files" [ -z "$(find "$INSTALL1" -maxdepth 1 -name '.farhelm*' ! -name '.farhelm-installation')" ]

# ===========================================================================
# Scenario: update (re-run against the same release) -- the ".old" rollback
# text must NOT appear, since nothing failed.
# ===========================================================================
echo
echo "== update (re-run) =="
run_install "$TOOLCHAIN_FULL" "$HOME1" "$BASE/good" 1.2.3
check "update exits 0" [ "$RC" -eq 0 ]
check "update reports ready" contains "$OUT" "Farhelm 1.2.3 is ready."
check "update gives macOS restart advice" contains "$OUT" "quit and reopen Farhelm"
check "update does not print a rollback message" not_contains "$OUT$ERR" "was restored"
check "update leaves no leftover staging/lock/backup dot-files" [ -z "$(find "$INSTALL1" -maxdepth 1 -name '.farhelm*' ! -name '.farhelm-installation')" ]

# ===========================================================================
# Scenario: redirect chain (F1) -- the SAME fresh-install assertions, but
# every request answers 302 first. If SHA256SUMS's download does not follow
# redirects, this fails exactly the way F1 described: a generic download
# error before anything is verified.
# ===========================================================================
echo
echo "== fresh install through a 302 redirect chain (F1) =="
HOME_REDIRECT="$WORKDIR/home-redirect"
INSTALL_REDIRECT="$HOME_REDIRECT/.local/bin"
mkdir -p "$HOME_REDIRECT"
run_install "$TOOLCHAIN_FULL" "$HOME_REDIRECT" "$BASE/redirect" 1.2.3
check "redirect-chain install exits 0" [ "$RC" -eq 0 ]
check "redirect-chain install produced a working farhelm" contains "$("$INSTALL_REDIRECT/farhelm" --version 2>/dev/null || true)" "farhelm 1.2.3"

# ===========================================================================
# Scenario: rollback on a second-binary (farhelm-desktop) failure, macOS-
# shaped via a `uname` shim (F3, F4, F7, F19, F24, F31). Established pair
# first at 1.2.3, then an ATTEMPTED UPDATE TO A DIFFERENT VERSION (1.2.4)
# with an `mv` double that fails only the farhelm-desktop move.
#
# F19: the old (1.2.3) and new (1.2.4) fixtures are genuinely distinct
# binaries, not the same version installed twice. Using the same version
# for both would make "restored the OLD bytes" and "kept the NEW bytes"
# indistinguishable outcomes -- a rollback bug that left the new farhelm
# installed beside the old farhelm-desktop would still pass a same-version
# byte comparison, since old and new bytes would be identical either way.
# ===========================================================================
echo
echo "== macOS-shaped install, then rollback on farhelm-desktop failure =="
MAC_TOOLS="$TOOLCHAIN_FULL"

HOME_MAC="$WORKDIR/home-mac"
INSTALL_MAC="$HOME_MAC/.local/bin"
mkdir -p "$HOME_MAC"
run_install "$MAC_TOOLS" "$HOME_MAC" "$BASE/good" 1.2.3
check "macOS-shaped fresh install exits 0" [ "$RC" -eq 0 ]
check "macOS-shaped fresh install writes farhelm-desktop too" [ -x "$INSTALL_MAC/farhelm-desktop" ]
check "macOS-shaped fresh install reports success" contains "$OUT" "Farhelm 1.2.3 is installed."
check "macOS-shaped fresh install: farhelm reports 1.2.3" \
  [ "$("$INSTALL_MAC/farhelm" --version)" = "farhelm 1.2.3" ]

# The Farhelm.app bundle: assembled from the committed binaries plus the
# archive's icon, executable KEPT as "farhelm-desktop" (case-insensitive
# APFS would collide an executable named "Farhelm" with the required
# "farhelm" CLI sibling in the same directory — asserting the exact name
# here is what keeps that from regressing on this case-SENSITIVE test
# host, where the collision itself cannot reproduce).
MAC_APP="$HOME_MAC/Applications/Farhelm.app"
check "macOS-shaped fresh install reports where to open Farhelm" \
  contains "$OUT" "Farhelm from Spotlight or ~/Applications."
check "bundle: Info.plist names the stable bundle identifier" \
  contains "$(cat "$MAC_APP/Contents/Info.plist")" "<string>org.scode.farhelm.desktop</string>"
check "bundle: Info.plist points CFBundleExecutable at farhelm-desktop" \
  contains "$(cat "$MAC_APP/Contents/Info.plist")" "<string>farhelm-desktop</string>"
check "bundle: Info.plist carries the installed version" \
  contains "$(cat "$MAC_APP/Contents/Info.plist")" "<string>1.2.3</string>"
check "bundle: executable is a byte-for-byte copy of the installed farhelm-desktop" \
  [ "$(cat "$MAC_APP/Contents/MacOS/farhelm-desktop")" = "$(cat "$INSTALL_MAC/farhelm-desktop")" ]
check "bundle: CLI sibling is a byte-for-byte copy of the installed farhelm" \
  [ "$(cat "$MAC_APP/Contents/MacOS/farhelm")" = "$(cat "$INSTALL_MAC/farhelm")" ]
check "bundle: executable carries the executable bit" [ -x "$MAC_APP/Contents/MacOS/farhelm-desktop" ]
check "bundle: CLI sibling carries the executable bit" [ -x "$MAC_APP/Contents/MacOS/farhelm" ]
check "bundle: icon is the archive's Farhelm.icns byte-for-byte" \
  [ "$(cat "$MAC_APP/Contents/Resources/Farhelm.icns")" = "fake icns: farhelm-desktop 1.2.3" ]
check "macOS standalone metadata has all installed hashes" assert_standalone_record "$INSTALL_MAC"
check "macOS standalone metadata is owner-only" [ "$(stat -c %a "$INSTALL_MAC/.farhelm-installation")" = "600" ]
check "bundle metadata has the exact content fields" assert_bundle_record "$MAC_APP" "$INSTALL_MAC"
check "bundle metadata is owner-only" [ "$(stat -c %a "$MAC_APP/Contents/.farhelm-installation")" = "600" ]

OLD_FARHELM_CONTENT=$(cat "$INSTALL_MAC/farhelm")
OLD_DESKTOP_CONTENT=$(cat "$INSTALL_MAC/farhelm-desktop")

MAC_TOOLS_FAILDESKTOP="$WORKDIR/toolchain-mac-faildesktop"
mkdir -p "$MAC_TOOLS_FAILDESKTOP"
cp -a "$MAC_TOOLS"/. "$MAC_TOOLS_FAILDESKTOP/"
# Same reasoning as the uname replacement above: mv here is a symlink to the
# real /usr/bin/mv, and `cat >` on it would try to overwrite that real
# binary rather than replace what the symlink points at.
rm -f "$MAC_TOOLS_FAILDESKTOP/mv"
cat >"$MAC_TOOLS_FAILDESKTOP/mv" <<'MVEOF'
#!/bin/sh
# Test double: behaves exactly like the real mv, except it refuses to
# install the newly STAGED farhelm-desktop -- proving install.sh's rollback
# path for that specific, documented failure mode. Deliberately narrower
# than "any move onto .../farhelm-desktop": install.sh's own rollback also
# moves the OLD farhelm-desktop back from its durable ".old" backup onto
# that same destination, and if this double blocked that move too, the
# rollback it exists to verify could never actually succeed.
eval "last=\${$#}"
first=$1
case "$last" in
  */farhelm-desktop)
    case "$first" in
      *.farhelm-install.*/farhelm-desktop)
        echo "fake mv: forced failure for $first -> $last" >&2
        exit 1
        ;;
    esac
    ;;
esac
exec /bin/mv "$@"
MVEOF
chmod 755 "$MAC_TOOLS_FAILDESKTOP/mv"

# Attempt to update to 1.2.4 (NOT 1.2.3) -- the forced failure must leave
# BOTH destinations at the 1.2.3 content, never a 1.2.3/1.2.4 mix.
run_install "$MAC_TOOLS_FAILDESKTOP" "$HOME_MAC" "$BASE/good-v2" 1.2.4
check "forced desktop-replace failure exits 1" [ "$RC" -ne 0 ]
check "forced desktop-replace failure prints the exact required message" \
  contains "$ERR" "install/update failed while replacing farhelm-desktop; the previous installation (if any) was restored"
check "forced desktop-replace failure restores the OLD (1.2.3) farhelm byte-for-byte" \
  [ "$(cat "$INSTALL_MAC/farhelm")" = "$OLD_FARHELM_CONTENT" ]
check "forced desktop-replace failure leaves the OLD (1.2.3) farhelm-desktop untouched" \
  [ "$(cat "$INSTALL_MAC/farhelm-desktop")" = "$OLD_DESKTOP_CONTENT" ]
check "forced desktop-replace failure: farhelm still reports 1.2.3 (not 1.2.4)" \
  [ "$("$INSTALL_MAC/farhelm" --version)" = "farhelm 1.2.3" ]
check "forced desktop-replace failure leaves no leftover staging/lock/backup dot-files" \
  [ -z "$(find "$INSTALL_MAC" -maxdepth 1 -name '.farhelm*' ! -name '.farhelm-installation')" ]
# The failure hit step 6 (the journaled replace), so step 7 never ran: the
# bundle must still be the intact 1.2.3 one, matching the restored binaries.
check "forced desktop-replace failure leaves the 1.2.3 bundle untouched" \
  contains "$(cat "$MAC_APP/Contents/Info.plist")" "<string>1.2.3</string>"

# Retry the SAME 1.2.4 update, this time without the forced failure -- both
# destinations must now contain the NEW (1.2.4) content, proving the
# earlier rollback did not somehow leave a partial or stuck state behind.
MAC_TOOLS_V2="$WORKDIR/toolchain-mac-v2"
mkdir -p "$MAC_TOOLS_V2"
cp -a "$MAC_TOOLS"/. "$MAC_TOOLS_V2/"
run_install "$MAC_TOOLS_V2" "$HOME_MAC" "$BASE/good-v2" 1.2.4
check "a real update to 1.2.4 after the forced failure succeeds" [ "$RC" -eq 0 ]
check "real update: farhelm now reports 1.2.4" [ "$("$INSTALL_MAC/farhelm" --version)" = "farhelm 1.2.4" ]
check "real update: farhelm-desktop content changed from the 1.2.3 original" \
  [ "$(cat "$INSTALL_MAC/farhelm-desktop")" != "$OLD_DESKTOP_CONTENT" ]
check "real update: bundle rebuilt at the new version" \
  contains "$(cat "$MAC_APP/Contents/Info.plist")" "<string>1.2.4</string>"
check "real update: bundle executable tracks the new farhelm-desktop" \
  [ "$(cat "$MAC_APP/Contents/MacOS/farhelm-desktop")" = "$(cat "$INSTALL_MAC/farhelm-desktop")" ]
check "real update: bundle metadata matches the new bundle" assert_bundle_record "$MAC_APP" "$INSTALL_MAC"

# ===========================================================================
# Scenario: bundle edge shapes, all macOS-shaped. Each gets its own fresh
# HOME so bundle presence/absence assertions cannot bleed between cases.
# ===========================================================================
echo
echo "== Farhelm.app bundle edge shapes =="

# A release whose desktop archive predates the icon cannot satisfy the app
# contract. Refuse before replacing binaries, and remove ephemeral staging.
build_good_release "$WWW/good-preicon" 1.2.3
build_archive "$WWW/good-preicon/farhelm-desktop-aarch64-apple-darwin.tar.gz" \
  farhelm-desktop aarch64-apple-darwin "farhelm-desktop 1.2.3" no-icns
(cd "$WWW/good-preicon" && sha256sum -- *.tar.gz >SHA256SUMS)
HOME_PREICON="$WORKDIR/home-preicon"
mkdir -p "$HOME_PREICON"
run_install "$MAC_TOOLS" "$HOME_PREICON" "$BASE/good-preicon" 1.2.3
check "pre-icon release is refused" [ "$RC" -eq 1 ]
check "pre-icon release names the too-old release" \
  contains "$ERR" "❌ Farhelm 1.2.3 is too old for this installer: it has no Mac app."
check "pre-icon release explains the supported versions" \
  contains "$ERR" "   Pick 0.2.1 or newer, or leave FARHELM_VERSION unset for the latest release."
check "pre-icon release leaves no staging or lock" \
  [ -z "$(find "$HOME_PREICON/.local/bin" -mindepth 1 -maxdepth 1 -name '.farhelm*')" ]
check "pre-icon release installs no binary" [ ! -e "$HOME_PREICON/.local/bin/farhelm" ]
check "pre-icon release creates no bundle" [ ! -e "$HOME_PREICON/Applications/Farhelm.app" ]

# Refusing an old release must also preserve a working installation, not
# merely leave a fresh home empty. Snapshot every installed file's bytes so
# an early replacement followed by a refusal cannot look like a safe gate.
HOME_PREICON_UPDATE="$WORKDIR/home-preicon-update"
run_install "$MAC_TOOLS" "$HOME_PREICON_UPDATE" "$BASE/good-v2" 1.2.4
check "pre-icon update premise: current install succeeds" [ "$RC" -eq 0 ]
check "pre-icon update premise: flat ownership is valid" \
  assert_standalone_record "$HOME_PREICON_UPDATE/.local/bin"
check "pre-icon update premise: app ownership is valid" \
  assert_bundle_record "$HOME_PREICON_UPDATE/Applications/Farhelm.app" "$HOME_PREICON_UPDATE/.local/bin"
PREICON_BYTES=$(find "$HOME_PREICON_UPDATE" -type f -exec sha256sum {} \; | sort)
run_install "$MAC_TOOLS" "$HOME_PREICON_UPDATE" "$BASE/good-preicon" 1.2.3
check "pre-icon update is refused" [ "$RC" -eq 1 ]
check "pre-icon update preserves every installed file" \
  [ "$(find "$HOME_PREICON_UPDATE" -type f -exec sha256sum {} \; | sort)" = "$PREICON_BYTES" ]
check "pre-icon update leaves no staging or lock" \
  [ -z "$(find "$HOME_PREICON_UPDATE/.local/bin" -name '.farhelm*' ! -name '.farhelm-installation')" ]

# A Farhelm.app that is NOT a farhelm bundle belongs to the user: refuse to
# replace it, but the binaries must still have been committed (the bundle
# step runs after the transaction on purpose).
HOME_FOREIGN="$WORKDIR/home-foreign"
mkdir -p "$HOME_FOREIGN/Applications/Farhelm.app/Contents"
cat >"$HOME_FOREIGN/Applications/Farhelm.app/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
	<key>CFBundleIdentifier</key><string>com.example.unrelated</string>
</dict></plist>
EOF
run_install "$MAC_TOOLS" "$HOME_FOREIGN" "$BASE/good" 1.2.3
check "foreign Farhelm.app: install exits 1" [ "$RC" -ne 0 ]
check "foreign Farhelm.app: refusal names the problem" \
  contains "$ERR" "does not look like a farhelm app bundle; refusing to replace it."
check "foreign Farhelm.app: the binaries were still committed" \
  [ "$("$HOME_FOREIGN/.local/bin/farhelm" --version)" = "farhelm 1.2.3" ]
check "foreign Farhelm.app: the user's bundle is untouched" \
  contains "$(cat "$HOME_FOREIGN/Applications/Farhelm.app/Contents/Info.plist")" "com.example.unrelated"

# Bundle ownership comes from the bundle's own record, not from its
# Info.plist mentioning Farhelm (triage: installer-deletes-farhelm-app-on-grep).
# Why this matters: the installer used to rm -rf any Farhelm.app whose
# Info.plist contained "farhelm", which includes a bundle the user built or
# customised, and refused its own half-uninstalled bundle. Spec: replace a
# bundle whose record names this installation (even with Info.plist gone),
# replace the recordless shape this script built before records existed,
# and refuse everything else, leaving it untouched.

# write_legacy_bundle APP: the recordless layout this script built between
# the bundle's introduction (#310) and its ownership record (#673).
write_legacy_bundle() {
  local app=$1
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  printf '<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0">\n<dict>\n\t<key>CFBundleIdentifier</key>\n\t<string>org.scode.farhelm.desktop</string>\n\t<key>CFBundleShortVersionString</key>\n\t<string>0.9.0</string>\n</dict>\n</plist>\n' \
    >"$app/Contents/Info.plist"
  echo old-cli >"$app/Contents/MacOS/farhelm"
  echo old-desktop >"$app/Contents/MacOS/farhelm-desktop"
  echo old-icon >"$app/Contents/Resources/Farhelm.icns"
}

# A bundle that merely mentions farhelm (the old hand-rolled trial's shape
# among them) is the user's: refused, not deleted.
HOME_STALE="$WORKDIR/home-stale"
mkdir -p "$HOME_STALE/Applications/Farhelm.app/Contents/MacOS"
printf '<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0"><dict>\n\t<key>CFBundleIdentifier</key><string>org.farhelm.desktop-trial</string>\n</dict></plist>\n' \
  >"$HOME_STALE/Applications/Farhelm.app/Contents/Info.plist"
echo mine >"$HOME_STALE/Applications/Farhelm.app/Contents/MacOS/leftover"
run_install "$MAC_TOOLS" "$HOME_STALE" "$BASE/good" 1.2.3
check "farhelm-mentioning foreign bundle: install exits 1" [ "$RC" -ne 0 ]
check "farhelm-mentioning foreign bundle: refusal names the problem" \
  contains "$ERR" "does not look like a farhelm app bundle; refusing to replace it."
check "farhelm-mentioning foreign bundle: the user's files are untouched" \
  [ "$(cat "$HOME_STALE/Applications/Farhelm.app/Contents/MacOS/leftover")" = "mine" ]

# The recordless installer bundle is replaced, so an update does not leave a
# stale second Farhelm behind.
HOME_LEGACY="$WORKDIR/home-legacy"
LEGACY_APP="$HOME_LEGACY/Applications/Farhelm.app"
write_legacy_bundle "$LEGACY_APP"
check "legacy bundle premise: it carries no record" [ ! -e "$LEGACY_APP/Contents/.farhelm-installation" ]
run_install "$MAC_TOOLS" "$HOME_LEGACY" "$BASE/good" 1.2.3
check "legacy installer bundle: install exits 0" [ "$RC" -eq 0 ]
check "legacy installer bundle: rebuilt at the new version" \
  contains "$(cat "$LEGACY_APP/Contents/Info.plist")" "<string>1.2.3</string>"
check "legacy installer bundle: now carries a verifiable record" \
  assert_bundle_record "$LEGACY_APP" "$HOME_LEGACY/.local/bin"

# The same legacy shape with one extra file is no longer provably the
# installer's: refused.
HOME_LEGACYX="$WORKDIR/home-legacy-extra"
LEGACYX_APP="$HOME_LEGACYX/Applications/Farhelm.app"
write_legacy_bundle "$LEGACYX_APP"
echo mine >"$LEGACYX_APP/Contents/MacOS/my-helper"
run_install "$MAC_TOOLS" "$HOME_LEGACYX" "$BASE/good" 1.2.3
check "legacy shape plus a user file: install exits 1" [ "$RC" -ne 0 ]
check "legacy shape plus a user file: the user's file is untouched" \
  [ "$(cat "$LEGACYX_APP/Contents/MacOS/my-helper")" = "mine" ]

# The legacy layout with a customised identifier, the original pair left
# behind in an XML comment, is the user's: the comment must not vouch for it.
HOME_LEGACYC="$WORKDIR/home-legacy-comment"
LEGACYC_APP="$HOME_LEGACYC/Applications/Farhelm.app"
write_legacy_bundle "$LEGACYC_APP"
printf '<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0">\n<dict>\n<!-- Original identity:\n\t<key>CFBundleIdentifier</key>\n\t<string>org.scode.farhelm.desktop</string>\n-->\n\t<key>CFBundleIdentifier</key>\n\t<string>com.example.custom-farhelm</string>\n</dict>\n</plist>\n' \
  >"$LEGACYC_APP/Contents/Info.plist"
LEGACYC_PLIST=$(cat "$LEGACYC_APP/Contents/Info.plist")
run_install "$MAC_TOOLS" "$HOME_LEGACYC" "$BASE/good" 1.2.3
check "legacy layout, commented-out identifier: install exits 1" [ "$RC" -ne 0 ]
check "legacy layout, commented-out identifier: the bundle is untouched" \
  [ "$(cat "$LEGACYC_APP/Contents/Info.plist")" = "$LEGACYC_PLIST" ]
check "legacy layout, commented-out identifier: its executables are untouched" \
  [ "$(cat "$LEGACYC_APP/Contents/MacOS/farhelm-desktop")" = "old-desktop" ]

# A recorded bundle is replaced wholesale even after an interrupted
# uninstall removed its Info.plist, and a file it gained since is gone
# afterwards (assembly is rm -rf + mv of a staged tree, never an edit).
HOME_HALF="$WORKDIR/home-half"
HALF_APP="$HOME_HALF/Applications/Farhelm.app"
mkdir -p "$HOME_HALF"
run_install "$MAC_TOOLS" "$HOME_HALF" "$BASE/good" 1.2.3
check "half-uninstalled bundle setup: first install exits 0" [ "$RC" -eq 0 ]
rm -f "$HALF_APP/Contents/Info.plist"
echo stale >"$HALF_APP/Contents/MacOS/leftover"
check "half-uninstalled bundle premise: record present" [ -f "$HALF_APP/Contents/.farhelm-installation" ]
check "half-uninstalled bundle premise: Info.plist gone" [ ! -e "$HALF_APP/Contents/Info.plist" ]
run_install "$MAC_TOOLS" "$HOME_HALF" "$BASE/good-v2" 1.2.4
check "half-uninstalled bundle: update exits 0" [ "$RC" -eq 0 ]
check "half-uninstalled bundle: rebuilt at the new version" \
  contains "$(cat "$HALF_APP/Contents/Info.plist")" "<string>1.2.4</string>"
check "half-uninstalled bundle: no stale file survives the wholesale swap" \
  [ ! -e "$HALF_APP/Contents/MacOS/leftover" ]

# A record that names a DIFFERENT installation's directory does not make the
# bundle this installation's to replace.
HOME_OTHERDIR="$WORKDIR/home-otherdir"
mkdir -p "$HOME_OTHERDIR"
seed_legacy_install "$HOME_OTHERDIR" "$HOME_OTHERDIR/first/bin"
check "other-directory record setup: first install exits 0" [ "$RC" -eq 0 ]
OTHERDIR_PLIST=$(cat "$HOME_OTHERDIR/Applications/Farhelm.app/Contents/Info.plist")
run_install "$MAC_TOOLS" "$HOME_OTHERDIR" "$BASE/good-v2" 1.2.4
check "other-directory record: install exits 1" [ "$RC" -ne 0 ]
check "other-directory record: the existing bundle is untouched" \
  [ "$(cat "$HOME_OTHERDIR/Applications/Farhelm.app/Contents/Info.plist")" = "$OTHERDIR_PLIST" ]
check "other-directory record: the refusal names the installation the bundle belongs to" \
  contains "$ERR" "belongs to the farhelm installation in $(realpath "$HOME_OTHERDIR/first/bin"), which is still installed"

# An installation that moved takes its bundle with it (TODO: "Installer
# refuses its own app bundle after the install directory moves"). Why: the
# record names the directory the bundle was built from, and before this
# every install after a move refused the bundle as foreign, leaving the app
# on the old version with only a manual delete to recover. Spec: a record
# naming a directory that no longer holds a farhelm installation, or that
# now resolves to this installation's directory, is this installation's.

# An old custom installation is gone; a new default-path install adopts its bundle.
HOME_MOVED="$WORKDIR/home-moved"
MOVED_APP="$HOME_MOVED/Applications/Farhelm.app"
mkdir -p "$HOME_MOVED"
seed_legacy_install "$HOME_MOVED" "$HOME_MOVED/old/bin"
check "moved install setup: first install exits 0" [ "$RC" -eq 0 ]
check "moved install premise: the bundle's record names the old directory" \
  assert_bundle_record "$MOVED_APP" "$HOME_MOVED/old/bin"
rm -rf "$HOME_MOVED/old"
check "moved install premise: the old directory is gone" [ ! -e "$HOME_MOVED/old" ]
run_install "$MAC_TOOLS" "$HOME_MOVED" "$BASE/good-v2" 1.2.4
check "moved install: update exits 0" [ "$RC" -eq 0 ]
check "moved install: bundle rebuilt at the new version" \
  contains "$(cat "$MOVED_APP/Contents/Info.plist")" "<string>1.2.4</string>"
check "moved install: the record now names the new directory" \
  assert_bundle_record "$MOVED_APP" "$HOME_MOVED/.local/bin"

# The old directory survives but holds no installation any more (its
# binaries and record went; the directory itself stayed).
HOME_EMPTIED="$WORKDIR/home-emptied"
EMPTIED_APP="$HOME_EMPTIED/Applications/Farhelm.app"
mkdir -p "$HOME_EMPTIED"
seed_legacy_install "$HOME_EMPTIED" "$HOME_EMPTIED/old/bin"
check "emptied old directory setup: first install exits 0" [ "$RC" -eq 0 ]
check "emptied old directory premise: the bundle's record names the old directory" \
  assert_bundle_record "$EMPTIED_APP" "$HOME_EMPTIED/old/bin"
rm -f "$HOME_EMPTIED/old/bin/farhelm" "$HOME_EMPTIED/old/bin/farhelm-desktop" \
  "$HOME_EMPTIED/old/bin/.farhelm-installation"
check "emptied old directory premise: the directory itself remains" [ -d "$HOME_EMPTIED/old/bin" ]
check "emptied old directory premise: it holds no installation record or binaries" \
  [ ! -e "$HOME_EMPTIED/old/bin/.farhelm-installation" ] && [ -z "$(ls -A "$HOME_EMPTIED/old/bin")" ]
run_install "$MAC_TOOLS" "$HOME_EMPTIED" "$BASE/good-v2" 1.2.4
check "emptied old directory: update exits 0" [ "$RC" -eq 0 ]
check "emptied old directory: the record now names the new directory" \
  assert_bundle_record "$EMPTIED_APP" "$HOME_EMPTIED/.local/bin"

# ~/.local/bin moved elsewhere and replaced by a symlink to its new home:
# the record's path now resolves to this installation's directory.
HOME_LINKED="$WORKDIR/home-linked"
LINKED_APP="$HOME_LINKED/Applications/Farhelm.app"
mkdir -p "$HOME_LINKED"
run_install "$MAC_TOOLS" "$HOME_LINKED" "$BASE/good" 1.2.3
check "symlinked install setup: first install exits 0" [ "$RC" -eq 0 ]
check "symlinked install premise: the bundle's record names the original directory" \
  assert_bundle_record "$LINKED_APP" "$HOME_LINKED/.local/bin"
mv "$HOME_LINKED/.local/bin" "$HOME_LINKED/.local/bin-real"
ln -s bin-real "$HOME_LINKED/.local/bin"
check "symlinked install premise: the old path is now a symlink" [ -L "$HOME_LINKED/.local/bin" ]
check "symlinked install premise: it resolves to the moved installation" \
  [ "$(realpath "$HOME_LINKED/.local/bin")" = "$(realpath "$HOME_LINKED/.local/bin-real")" ] \
  && [ -f "$HOME_LINKED/.local/bin-real/.farhelm-installation" ]
run_install "$MAC_TOOLS" "$HOME_LINKED" "$BASE/good-v2" 1.2.4
check "symlinked install: update exits 0" [ "$RC" -eq 0 ]
check "symlinked install: bundle rebuilt at the new version" \
  contains "$(cat "$LINKED_APP/Contents/Info.plist")" "<string>1.2.4</string>"
check "symlinked install: the record names the resolved directory" \
  assert_bundle_record "$LINKED_APP" "$HOME_LINKED/.local/bin-real"

# A directory name holding newlines, one of them trailing, is a supported
# install location; moving it must be recognized like any other. The record
# stores the path byte for byte, so this is the case a line-based reading of
# the record would get wrong.
HOME_NLMOVE="$WORKDIR/home-nl-move"
NLMOVE_APP="$HOME_NLMOVE/Applications/Farhelm.app"
printf -v NLMOVE_OLD '%s/old\nbin\n' "$HOME_NLMOVE"
mkdir -p "$HOME_NLMOVE"
seed_legacy_install "$HOME_NLMOVE" "$NLMOVE_OLD"
check "newline-named moved install setup: first install exits 0" [ "$RC" -eq 0 ]
check "newline-named moved install premise: the record names the newline-named directory" \
  assert_bundle_record "$NLMOVE_APP" "$NLMOVE_OLD"
# First, with the old installation still in place: the bundle is that
# installation's and is refused, naming its directory byte for byte. The
# truncated readings a line-based parse could produce (without the trailing
# newline, or only the first line) name directories that do not exist, so
# only an exact reading refuses here.
NLMOVE_PLIST=$(cat "$NLMOVE_APP/Contents/Info.plist")
check "newline-named install premise: the truncated readings name no directory" \
  [ ! -e "${NLMOVE_OLD%$'\n'}" ] && [ ! -e "$HOME_NLMOVE/old" ]
run_install "$MAC_TOOLS" "$HOME_NLMOVE" "$BASE/good-v2" 1.2.4
check "newline-named install still in place: install exits 1" [ "$RC" -ne 0 ]
check "newline-named install still in place: the bundle is untouched" \
  [ "$(cat "$NLMOVE_APP/Contents/Info.plist")" = "$NLMOVE_PLIST" ]
check "newline-named install still in place: the refusal names the exact directory" \
  contains "$ERR" "belongs to the farhelm installation in $NLMOVE_OLD, which is still installed"
rm -rf "$NLMOVE_OLD"
check "newline-named moved install premise: the old directory is gone" [ ! -e "$NLMOVE_OLD" ]
run_install "$MAC_TOOLS" "$HOME_NLMOVE" "$BASE/good-v2" 1.2.4
check "newline-named moved install: update exits 0" [ "$RC" -eq 0 ]
check "newline-named moved install: the record now names the new directory" \
  assert_bundle_record "$NLMOVE_APP" "$HOME_NLMOVE/.local/bin"

# An installation that is intact but cannot be looked into (its directory's
# parent unsearchable) is not an installation that disappeared. Why: `test
# -e` is false for a path it cannot look up, so reading that as "gone" would
# replace a bundle whose installation still exists.
if [ "$(id -u)" -ne 0 ]; then
  HOME_LOCKED="$WORKDIR/home-locked"
  LOCKED_APP="$HOME_LOCKED/Applications/Farhelm.app"
  mkdir -p "$HOME_LOCKED"
  seed_legacy_install "$HOME_LOCKED" "$HOME_LOCKED/first/bin"
  check "unsearchable installation setup: first install exits 0" [ "$RC" -eq 0 ]
  check "unsearchable installation premise: the record names the first directory" \
    assert_bundle_record "$LOCKED_APP" "$HOME_LOCKED/first/bin"
  LOCKED_PLIST=$(cat "$LOCKED_APP/Contents/Info.plist")
  chmod 000 "$HOME_LOCKED/first"
  check "unsearchable installation premise: its record cannot be looked up" \
    [ ! -e "$HOME_LOCKED/first/bin/.farhelm-installation" ]
  run_install "$MAC_TOOLS" "$HOME_LOCKED" "$BASE/good-v2" 1.2.4
  chmod 755 "$HOME_LOCKED/first"
  check "unsearchable installation premise: the first installation is intact" \
    [ -f "$HOME_LOCKED/first/bin/.farhelm-installation" ]
  check "unsearchable installation: install exits 1" [ "$RC" -ne 0 ]
  check "unsearchable installation: the bundle is untouched" \
    [ "$(cat "$LOCKED_APP/Contents/Info.plist")" = "$LOCKED_PLIST" ]
fi

# A record with six NULs in the wrong places: the NUL that belongs after the
# magic is a newline, and an extra NUL makes up the count. Why: a parse that
# counts NULs and then reads lines accepts this as naming a vanished
# directory, and would replace the bundle it sits in.
HOME_MIXREC="$WORKDIR/home-mixed-record"
MIXREC_APP="$HOME_MIXREC/Applications/Farhelm.app"
mkdir -p "$MIXREC_APP/Contents"
{
  printf 'farhelm-app\n/nonexistent/old/bin\000\000'
  for _ in 1 2 3 4; do printf '%064d\000' 0 | tr 0 a; done
} >"$MIXREC_APP/Contents/.farhelm-installation"
echo mine >"$MIXREC_APP/Contents/my-file"
check "mixed-framing record premise: it holds exactly six NULs" \
  [ "$(tr -cd '\000' <"$MIXREC_APP/Contents/.farhelm-installation" | tr '\000' x)" = xxxxxx ]
run_install "$MAC_TOOLS" "$HOME_MIXREC" "$BASE/good" 1.2.3
check "mixed-framing record: install exits 1" [ "$RC" -ne 0 ]
check "mixed-framing record: the refusal is the foreign-bundle one" \
  contains "$ERR" "does not look like a farhelm app bundle; refusing to replace it."
check "mixed-framing record: the bundle's own file is untouched" \
  [ "$(cat "$MIXREC_APP/Contents/my-file")" = "mine" ]

# A record that is not framed exactly as the installer writes it is not
# ownership evidence, even when it reads as naming a vanished directory
# line by line. Why: recognizing a moved installation replaces the bundle
# wholesale, so a loose parse would let a foreign bundle's look-alike file
# authorize deleting it.
HOME_FAKEREC="$WORKDIR/home-fake-record"
FAKEREC_APP="$HOME_FAKEREC/Applications/Farhelm.app"
mkdir -p "$FAKEREC_APP/Contents"
printf 'farhelm-app\n/nonexistent/old/bin\na\nb\nc\nd\n' >"$FAKEREC_APP/Contents/.farhelm-installation"
echo mine >"$FAKEREC_APP/Contents/my-file"
check "newline-framed record premise: it names a directory that does not exist" [ ! -e /nonexistent/old/bin ]
run_install "$MAC_TOOLS" "$HOME_FAKEREC" "$BASE/good" 1.2.3
check "newline-framed record: install exits 1" [ "$RC" -ne 0 ]
check "newline-framed record: the refusal is the foreign-bundle one" \
  contains "$ERR" "does not look like a farhelm app bundle; refusing to replace it."
check "newline-framed record: the bundle's own file is untouched" \
  [ "$(cat "$FAKEREC_APP/Contents/my-file")" = "mine" ]

# The bundle is built while the install lock is still held, and swapped by
# renaming. Released first, another installer for the same directory could
# commit different binaries between the bundle's two copies (a bundle mixing
# versions under one version number) or swap the bundle at the same time; and
# deleting the old bundle in place could be interrupted into a half-deleted
# one no later run accepts. A `cp` double records whether the lock exists
# each time a binary is copied into the bundle.
HOME_BUNDLELOCK="$WORKDIR/home-bundlelock"
INSTALL_BUNDLELOCK="$HOME_BUNDLELOCK/.local/bin"
mkdir -p "$HOME_BUNDLELOCK"
run_install "$MAC_TOOLS" "$HOME_BUNDLELOCK" "$BASE/good" 1.2.3
check "bundle lock setup: first install exits 0" [ "$RC" -eq 0 ]
MAC_TOOLS_CPWATCH="$WORKDIR/toolchain-mac-cpwatch"
mkdir -p "$MAC_TOOLS_CPWATCH"
cp -a "$MAC_TOOLS"/. "$MAC_TOOLS_CPWATCH/"
rm -f "$MAC_TOOLS_CPWATCH/cp"
cat >"$MAC_TOOLS_CPWATCH/cp" <<'CPEOF'
#!/bin/sh
# Test double: a real cp that, for each copy into the bundle's executables,
# records whether the install lock exists at that moment.
eval "last=\${$#}"
case "$last" in
  */Farhelm.app/Contents/MacOS/*)
    if [ -d "$CPWATCH_LOCK" ]; then echo held >>"$CPWATCH_LOG"; else echo free >>"$CPWATCH_LOG"; fi
    ;;
esac
exec /bin/cp "$@"
CPEOF
chmod 755 "$MAC_TOOLS_CPWATCH/cp"
CPWATCH_LOG_FILE="$WORKDIR/cpwatch.log"
run_install "$MAC_TOOLS_CPWATCH" "$HOME_BUNDLELOCK" "$BASE/good-v2" 1.2.4 \
  CPWATCH_LOCK="$INSTALL_BUNDLELOCK/.farhelm-install.lock" \
  CPWATCH_LOG="$CPWATCH_LOG_FILE"
check "bundle lock: update exits 0" [ "$RC" -eq 0 ]
check "bundle lock: both bundle copies ran while the install lock was held" \
  [ "$(cat "$CPWATCH_LOG_FILE")" = "$(printf 'held\nheld')" ]
check "bundle lock: the lock is released after the bundle step" \
  [ ! -e "$INSTALL_BUNDLELOCK/.farhelm-install.lock" ]
check "bundle lock: the bundle carries the new version" \
  contains "$(cat "$HOME_BUNDLELOCK/Applications/Farhelm.app/Contents/Info.plist")" "<string>1.2.4</string>"
check "bundle lock: the swap nests nothing inside the bundle" \
  [ ! -e "$HOME_BUNDLELOCK/Applications/Farhelm.app/Farhelm.app" ]
check "bundle lock: the previous bundle is not left aside" \
  [ "$(ls -A "$HOME_BUNDLELOCK/Applications")" = "Farhelm.app" ]

# Every install directory shares the one bundle name, so the bundle has its
# own lock: a run that finds it held refuses the bundle step (the binaries
# are installed) and leaves the lock and the bundle alone. Unrelated hidden
# entries in ~/Applications, whatever their names, are never touched.
mkdir "$HOME_BUNDLELOCK/Applications/.farhelm-app.lock"
mkdir "$HOME_BUNDLELOCK/Applications/.farhelm-app-replaced.1"
echo keep >"$HOME_BUNDLELOCK/Applications/.farhelm-app-replaced.1/sentinel"
BUNDLELOCK_PLIST=$(cat "$HOME_BUNDLELOCK/Applications/Farhelm.app/Contents/Info.plist")
run_install "$MAC_TOOLS" "$HOME_BUNDLELOCK" "$BASE/good" 1.2.3
check "bundle lock: a held bundle lock fails the bundle step" [ "$RC" -ne 0 ]
check "bundle lock: the refusal names the bundle lock" \
  contains "$ERR" "$HOME_BUNDLELOCK/Applications/.farhelm-app.lock"
check "bundle lock: the binaries were still committed" \
  [ "$("$INSTALL_BUNDLELOCK/farhelm" --version)" = "farhelm 1.2.3" ]
check "bundle lock: the held lock is left in place" [ -d "$HOME_BUNDLELOCK/Applications/.farhelm-app.lock" ]
check "bundle lock: the bundle is untouched while another run holds the lock" \
  [ "$(cat "$HOME_BUNDLELOCK/Applications/Farhelm.app/Contents/Info.plist")" = "$BUNDLELOCK_PLIST" ]
rmdir "$HOME_BUNDLELOCK/Applications/.farhelm-app.lock"
run_install "$MAC_TOOLS" "$HOME_BUNDLELOCK" "$BASE/good" 1.2.3
check "bundle lock: with the lock free the bundle is rebuilt" [ "$RC" -eq 0 ]
check "bundle lock: an unrelated hidden entry is untouched" \
  [ "$(cat "$HOME_BUNDLELOCK/Applications/.farhelm-app-replaced.1/sentinel")" = keep ]
check "bundle lock: nothing of the run is left behind" \
  [ "$(find "$HOME_BUNDLELOCK/Applications" -mindepth 1 -maxdepth 1 -printf '%f\n' | LC_ALL=C sort | tr '\n' ' ')" = ".farhelm-app-replaced.1 Farhelm.app " ]

# A swap that fails after the old bundle was moved aside puts it back: the
# public name must never be left empty or half built by a failure. An `mv`
# double refuses the move of the new bundle into place (source inside the
# private build directory, destination the public name); with RESTORE_FAILS
# set it also refuses moving the old bundle back, and then the old bundle
# must survive at the private path the installer names, not be deleted.
MAC_TOOLS_SWAPFAIL="$WORKDIR/toolchain-mac-swapfail"
mkdir -p "$MAC_TOOLS_SWAPFAIL"
cp -a "$MAC_TOOLS"/. "$MAC_TOOLS_SWAPFAIL/"
rm -f "$MAC_TOOLS_SWAPFAIL/mv"
cat >"$MAC_TOOLS_SWAPFAIL/mv" <<'MVEOF'
#!/bin/sh
# Test double: a real mv that refuses the bundle swap's move into place and,
# when RESTORE_FAILS is set, the cleanup's move of the old bundle back.
eval "last=\${$#}"
first=$1
case "$first" in
  */.farhelm-app-build.*/Farhelm.app)
    echo "fake mv: forced failure publishing $first" >&2
    exit 1
    ;;
  */.farhelm-app-build.*/previous)
    if [ -n "${RESTORE_FAILS:-}" ]; then
      echo "fake mv: forced failure restoring $first" >&2
      exit 1
    fi
    ;;
esac
exec /bin/mv "$@"
MVEOF
chmod 755 "$MAC_TOOLS_SWAPFAIL/mv"
SWAP_APP="$HOME_BUNDLELOCK/Applications/Farhelm.app"
SWAP_PLIST=$(cat "$SWAP_APP/Contents/Info.plist")
SWAP_RECORD=$(od -An -tx1 "$SWAP_APP/Contents/.farhelm-installation")
run_install "$MAC_TOOLS_SWAPFAIL" "$HOME_BUNDLELOCK" "$BASE/good-v2" 1.2.4
check "bundle swap failure: the install exits 1" [ "$RC" -ne 0 ]
check "bundle swap failure: the old bundle is back in place" \
  [ "$(cat "$SWAP_APP/Contents/Info.plist")" = "$SWAP_PLIST" ]
check "bundle swap failure: the old bundle keeps its record" \
  [ "$(od -An -tx1 "$SWAP_APP/Contents/.farhelm-installation")" = "$SWAP_RECORD" ]
check "bundle swap failure: the bundle lock is released" \
  [ ! -e "$HOME_BUNDLELOCK/Applications/.farhelm-app.lock" ]
check "bundle swap failure: no private build directory is left" \
  [ -z "$(find "$HOME_BUNDLELOCK/Applications" -mindepth 1 -maxdepth 1 -name '.farhelm-app-build.*')" ]
run_install "$MAC_TOOLS" "$HOME_BUNDLELOCK" "$BASE/good-v2" 1.2.4
check "bundle swap failure: an ordinary re-run rebuilds the bundle" [ "$RC" -eq 0 ]
check "bundle swap failure: the rebuilt bundle carries the new version" \
  contains "$(cat "$SWAP_APP/Contents/Info.plist")" "<string>1.2.4</string>"

SWAP_PLIST=$(cat "$SWAP_APP/Contents/Info.plist")
run_install "$MAC_TOOLS_SWAPFAIL" "$HOME_BUNDLELOCK" "$BASE/good" 1.2.3 \
  RESTORE_FAILS=1
check "bundle restore failure: the install exits 1" [ "$RC" -ne 0 ]
check "bundle restore failure: the message names where the old bundle is" \
  contains "$ERR" "could not be put back; it is at"
SWAP_KEPT=$(find "$HOME_BUNDLELOCK/Applications" -mindepth 2 -maxdepth 2 -path '*/.farhelm-app-build.*/previous')
check "bundle restore failure: the old bundle is kept in the private directory" [ -n "$SWAP_KEPT" ]
check "bundle restore failure: the kept bundle is the old one" \
  [ "$(cat "$SWAP_KEPT/Contents/Info.plist" 2>/dev/null)" = "$SWAP_PLIST" ]
check "bundle restore failure: the bundle lock is released" \
  [ ! -e "$HOME_BUNDLELOCK/Applications/.farhelm-app.lock" ]

# An uninstall interrupted at the very end of removing the bundle leaves its
# retry copy of the bundle record, `.Farhelm.app.uninstall-receipt`, next to
# the bundle. After a reinstall of another version that copy disagreed with
# the new bundle's own record, and uninstall refused forever with advice
# (re-run the installer) that could not clear it. The installer now removes
# the copy when it is this installation's own record, and leaves any other
# installation's copy alone.
HOME_RECEIPT="$WORKDIR/home-receipt"
mkdir -p "$HOME_RECEIPT"
run_install "$MAC_TOOLS" "$HOME_RECEIPT" "$BASE/good" 1.2.3
check "leftover receipt setup: first install exits 0" [ "$RC" -eq 0 ]
RECEIPT_COPY="$HOME_RECEIPT/Applications/.Farhelm.app.uninstall-receipt"
cp "$HOME_RECEIPT/Applications/Farhelm.app/Contents/.farhelm-installation" "$RECEIPT_COPY"
run_install "$MAC_TOOLS" "$HOME_RECEIPT" "$BASE/good-v2" 1.2.4
check "leftover receipt: the reinstall exits 0" [ "$RC" -eq 0 ]
check "leftover receipt: this installation's stale copy is removed" [ ! -e "$RECEIPT_COPY" ]

# A leftover receipt whose installation has since moved here is this
# installation's own (TODO: "Accept a leftover uninstall receipt after the
# install directory moves"). Why: the receipt names the directory it was
# written for, and before this an install from the new directory refused it
# as foreign and told the user to finish "that installation's" uninstall,
# which is this one. Spec: the receipt gets the bundle record's moved-here
# rule; here the named directory no longer exists, the install builds the
# bundle, and the receipt is removed.
HOME_MOVEDRECEIPT="$WORKDIR/home-movedreceipt"
mkdir -p "$HOME_MOVEDRECEIPT"
seed_legacy_install "$HOME_MOVEDRECEIPT" "$HOME_MOVEDRECEIPT/old/bin"
check "moved receipt setup: first install exits 0" [ "$RC" -eq 0 ]
MOVEDRECEIPT_APP="$HOME_MOVEDRECEIPT/Applications/Farhelm.app"
MOVEDRECEIPT_COPY="$HOME_MOVEDRECEIPT/Applications/.Farhelm.app.uninstall-receipt"
cp "$MOVEDRECEIPT_APP/Contents/.farhelm-installation" "$MOVEDRECEIPT_COPY"
rm -rf "$MOVEDRECEIPT_APP" "$HOME_MOVEDRECEIPT/old"
check "moved receipt premise: the receipt names the old directory" \
  contains "$(tr '\000' '\n' <"$MOVEDRECEIPT_COPY")" "$HOME_MOVEDRECEIPT/old/bin"
check "moved receipt premise: the old directory is gone" [ ! -e "$HOME_MOVEDRECEIPT/old" ]
run_install "$MAC_TOOLS" "$HOME_MOVEDRECEIPT" "$BASE/good-v2" 1.2.4
check "moved receipt: the install from the new directory exits 0" [ "$RC" -eq 0 ]
check "moved receipt: the bundle is built for the new directory" \
  assert_bundle_record "$MOVEDRECEIPT_APP" "$HOME_MOVEDRECEIPT/.local/bin"
check "moved receipt: the leftover receipt is removed" [ ! -e "$MOVEDRECEIPT_COPY" ]

# The other move shape: the recorded path is still there, now a symlink to
# where the installation lives.
HOME_LINKEDRECEIPT="$WORKDIR/home-linkedreceipt"
mkdir -p "$HOME_LINKEDRECEIPT"
run_install "$MAC_TOOLS" "$HOME_LINKEDRECEIPT" "$BASE/good" 1.2.3
check "linked receipt setup: first install exits 0" [ "$RC" -eq 0 ]
LINKEDRECEIPT_APP="$HOME_LINKEDRECEIPT/Applications/Farhelm.app"
LINKEDRECEIPT_COPY="$HOME_LINKEDRECEIPT/Applications/.Farhelm.app.uninstall-receipt"
cp "$LINKEDRECEIPT_APP/Contents/.farhelm-installation" "$LINKEDRECEIPT_COPY"
rm -rf "$LINKEDRECEIPT_APP"
mv "$HOME_LINKEDRECEIPT/.local/bin" "$HOME_LINKEDRECEIPT/.local/bin-real"
ln -s bin-real "$HOME_LINKEDRECEIPT/.local/bin"
check "linked receipt premise: the recorded path is now a symlink" [ -L "$HOME_LINKEDRECEIPT/.local/bin" ]
run_install "$MAC_TOOLS" "$HOME_LINKEDRECEIPT" "$BASE/good-v2" 1.2.4
check "linked receipt: the reinstall exits 0" [ "$RC" -eq 0 ]
check "linked receipt: the leftover receipt is removed" [ ! -e "$LINKEDRECEIPT_COPY" ]
check "linked receipt: the bundle is rebuilt" [ -d "$LINKEDRECEIPT_APP" ]

# Another installation's leftover receipt is that installation's only way to
# finish its uninstall. Installation A is installed, its uninstall is
# interrupted after the bundle is gone (its receipt copy survives), and then
# installation B, in another directory, installs: B must refuse the bundle
# step, leave A's receipt byte for byte, and create no bundle to disagree with.
HOME_FOREIGNRECEIPT="$WORKDIR/home-foreignreceipt"
mkdir -p "$HOME_FOREIGNRECEIPT"
seed_legacy_install "$HOME_FOREIGNRECEIPT" "$HOME_FOREIGNRECEIPT/a/bin"
check "foreign receipt setup: installation A exits 0" [ "$RC" -eq 0 ]
FOREIGN_APP="$HOME_FOREIGNRECEIPT/Applications/Farhelm.app"
FOREIGN_COPY="$HOME_FOREIGNRECEIPT/Applications/.Farhelm.app.uninstall-receipt"
cp "$FOREIGN_APP/Contents/.farhelm-installation" "$FOREIGN_COPY"
rm -rf "$FOREIGN_APP"
# The premise the refusal now rests on: A's directory still holds its
# installation, so the moved-installation rule does not adopt A's receipt.
check "foreign receipt premise: installation A is still installed" \
  [ -f "$HOME_FOREIGNRECEIPT/a/bin/.farhelm-installation" ]
FOREIGN_BYTES=$(od -An -tx1 "$FOREIGN_COPY")
run_install "$MAC_TOOLS" "$HOME_FOREIGNRECEIPT" "$BASE/good-v2" 1.2.4
check "foreign receipt: installation B refuses the bundle step" [ "$RC" -ne 0 ]
check "foreign receipt: the refusal names the receipt and how to clear it" \
  contains "$ERR" "$FOREIGN_COPY was left by an interrupted farhelm uninstall and is not this installation's record"
check "foreign receipt: B's binaries are still installed" \
  [ "$("$HOME_FOREIGNRECEIPT/.local/bin/farhelm" --version)" = "farhelm 1.2.4" ]
check "foreign receipt: A's receipt is unchanged" [ "$(od -An -tx1 "$FOREIGN_COPY")" = "$FOREIGN_BYTES" ]
check "foreign receipt: no bundle was created beside it" [ ! -e "$FOREIGN_APP" ]

# ===========================================================================
# Scenario: rollback when the FIRST replacement (farhelm itself) fails
# (F3) -- distinct from the farhelm-desktop case above: this is the move
# whose failure the EXIT trap could otherwise race, deleting the only
# backup (farhelm.old) before install.sh gets a chance to restore it.
# ===========================================================================
echo
echo "== F3: rollback when the FIRST replacement (farhelm) fails =="
HOMEFIRSTFAIL="$WORKDIR/homefirstfail"
INSTALLFIRSTFAIL="$HOMEFIRSTFAIL/.local/bin"
mkdir -p "$HOMEFIRSTFAIL"
run_install "$TOOLCHAIN_FULL" "$HOMEFIRSTFAIL" "$BASE/good" 1.2.3
check "F3 setup: initial install exits 0" [ "$RC" -eq 0 ]
OLD_FIRSTFAIL_CONTENT=$(cat "$INSTALLFIRSTFAIL/farhelm")

TOOLS_FAILFIRST="$WORKDIR/toolchain-failfirst"
mkdir -p "$TOOLS_FAILFIRST"
cp -a "$TOOLCHAIN_FULL"/. "$TOOLS_FAILFIRST/"
rm -f "$TOOLS_FAILFIRST/mv"
cat >"$TOOLS_FAILFIRST/mv" <<'MVEOF'
#!/bin/sh
# Test double: fails only the move that installs the newly staged farhelm
# (source under a staging directory), never a restore-from-backup move
# (source ending in .farhelm.old) or anything else -- otherwise this
# double would block install.sh's own rollback along with the thing it is
# supposed to be testing.
eval "last=\${$#}"
first=$1
case "$first" in
  *.farhelm-install.*/farhelm)
    case "$last" in
      */farhelm)
        echo "fake mv: forced failure for $first -> $last" >&2
        exit 1
        ;;
    esac
    ;;
esac
exec /bin/mv "$@"
MVEOF
chmod 755 "$TOOLS_FAILFIRST/mv"

run_install "$TOOLS_FAILFIRST" "$HOMEFIRSTFAIL" "$BASE/good" 1.2.3
check "F3: forced first-replacement failure exits 1" [ "$RC" -ne 0 ]
check "F3: forced first-replacement failure prints the exact required message" \
  contains "$ERR" "install/update failed while replacing farhelm; the previous farhelm (if any) was restored"
check "F3: forced first-replacement failure restores the OLD farhelm byte-for-byte" \
  [ "$(cat "$INSTALLFIRSTFAIL/farhelm")" = "$OLD_FIRSTFAIL_CONTENT" ]
check "F3: forced first-replacement failure leaves no leftover staging/lock/backup dot-files" \
  [ -z "$(find "$INSTALLFIRSTFAIL" -maxdepth 1 -name '.farhelm*' ! -name '.farhelm-installation')" ]

# ===========================================================================
# Scenario: refuses before touching anything when a destination exists and
# is not a regular file (F4) -- a directory collision must be rejected up
# front, not partially processed.
# ===========================================================================
echo
echo "== F4: refuses when the destination exists and is not a regular file =="
HOMEDIRDEST="$WORKDIR/homedirdest"
INSTALLDIRDEST="$HOMEDIRDEST/.local/bin"
mkdir -p "$INSTALLDIRDEST/farhelm" # farhelm is a DIRECTORY here, not a file
echo "sentinel" >"$INSTALLDIRDEST/farhelm/keepme"
run_install "$TOOLCHAIN_FULL" "$HOMEDIRDEST" "$BASE/good" 1.2.3
check "F4: directory-collision install exits 1" [ "$RC" -ne 0 ]
check "F4: directory-collision install names the problem" contains "$ERR" "is not a regular file"
check "F4: directory-collision install leaves the directory's contents untouched" \
  [ "$(cat "$INSTALLDIRDEST/farhelm/keepme")" = "sentinel" ]

# ===========================================================================
# Scenario: ownership metadata collisions and publication failure happen
# after the binary commit, but never follow or destroy a foreign target.
# ===========================================================================
echo
echo "== installer ownership metadata refusal and repair =="
HOMEMETA="$WORKDIR/home-meta"
INSTALLMETA="$HOMEMETA/.local/bin"
mkdir -p "$HOMEMETA"
run_install "$TOOLCHAIN_FULL" "$HOMEMETA" "$BASE/good" 1.2.3
check "metadata refusal setup succeeds" [ "$RC" -eq 0 ]
META_OUTSIDE="$WORKDIR/metadata-outside"
printf 'outside sentinel\n' >"$META_OUTSIDE"
rm "$INSTALLMETA/.farhelm-installation"
ln -s "$META_OUTSIDE" "$INSTALLMETA/.farhelm-installation"
run_install "$TOOLCHAIN_FULL" "$HOMEMETA" "$BASE/good-v2" 1.2.4
check "metadata symlink refusal exits 1" [ "$RC" -ne 0 ]
check "metadata symlink refusal names the metadata target" contains "$ERR" "ownership metadata"
check "metadata symlink refusal leaves the link in place" [ -L "$INSTALLMETA/.farhelm-installation" ]
check "metadata symlink refusal does not touch the link target" [ "$(cat "$META_OUTSIDE")" = "outside sentinel" ]
check "metadata symlink refusal leaves committed binaries usable" [ "$("$INSTALLMETA/farhelm" --version)" = "farhelm 1.2.4" ]
rm "$INSTALLMETA/.farhelm-installation"
ln -s "$WORKDIR/missing-metadata-target" "$INSTALLMETA/.farhelm-installation"
run_install "$TOOLCHAIN_FULL" "$HOMEMETA" "$BASE/good-v2" 1.2.4
check "dangling metadata symlink refusal exits 1" [ "$RC" -ne 0 ]
check "dangling metadata symlink remains in place" [ -L "$INSTALLMETA/.farhelm-installation" ]
rm "$INSTALLMETA/.farhelm-installation"

mkdir "$INSTALLMETA/.farhelm-installation"
run_install "$TOOLCHAIN_FULL" "$HOMEMETA" "$BASE/good-v2" 1.2.4
check "metadata directory refusal exits 1" [ "$RC" -ne 0 ]
check "metadata directory refusal leaves the directory in place" [ -d "$INSTALLMETA/.farhelm-installation" ]
rm -rf "$INSTALLMETA/.farhelm-installation"

TOOLS_FAILMETA="$WORKDIR/toolchain-failmeta"
HOMEFAILMETA="$WORKDIR/home-failmeta"
INSTALLFAILMETA="$HOMEFAILMETA/.local/bin"
mkdir -p "$HOMEFAILMETA"
run_install "$TOOLCHAIN_FULL" "$HOMEFAILMETA" "$BASE/good" 1.2.3
check "metadata publication failure setup succeeds" [ "$RC" -eq 0 ]
cp "$INSTALLFAILMETA/.farhelm-installation" "$WORKDIR/old-metadata-record"
mkdir -p "$TOOLS_FAILMETA"
cp -a "$TOOLCHAIN_FULL"/. "$TOOLS_FAILMETA/"
rm -f "$TOOLS_FAILMETA/mv"
cat >"$TOOLS_FAILMETA/mv" <<'MVEOF'
#!/bin/sh
eval "last=\${$#}"
case "$last" in
  */.farhelm-installation)
    echo "fake mv: forced metadata publication failure" >&2
    exit 1
    ;;
esac
exec /bin/mv "$@"
MVEOF
chmod 755 "$TOOLS_FAILMETA/mv"
run_install "$TOOLS_FAILMETA" "$HOMEFAILMETA" "$BASE/good-v2" 1.2.4
check "forced metadata publication failure exits 1" [ "$RC" -ne 0 ]
check "forced metadata publication failure explains the committed binaries" \
  contains "$ERR" "binaries in $INSTALLFAILMETA are installed and usable"
check "forced metadata publication failure leaves the new binary installed" \
  [ "$("$INSTALLFAILMETA/farhelm" --version)" = "farhelm 1.2.4" ]
check "forced metadata publication failure leaves the prior record intact" \
  cmp -s "$WORKDIR/old-metadata-record" "$INSTALLFAILMETA/.farhelm-installation"
check "forced metadata publication failure leaves no rollback debris" \
  [ -z "$(find "$INSTALLFAILMETA" -maxdepth 1 -name '.farhelm-install.*' -o -name '.farhelm-install.lock' -o -name '.farhelm.old')" ]
run_install "$TOOLCHAIN_FULL" "$HOMEFAILMETA" "$BASE/good-v2" 1.2.4
check "metadata publication repair succeeds" [ "$RC" -eq 0 ]
check "metadata publication repair writes current ownership fields" \
  assert_standalone_record "$INSTALLFAILMETA"
check "metadata publication repair changes the CLI digest" \
  assert_records_differ "$WORKDIR/old-metadata-record" "$INSTALLFAILMETA/.farhelm-installation"

# ===========================================================================
# Scenario: a subsequent run detects and repairs an interrupted swap (F2,
# F3, F4, F31) -- simulates exactly the wreckage a SIGKILL between "park
# the old binary" and "install the new one" would leave: a durable .old
# backup, a NEW (already-swapped) farhelm, a transaction journal recording
# the PARK move that was in flight, and a lock directory whose pid is
# guaranteed not to belong to any real running process. The journal (not
# just the presence of a ".old" file) is what the current design actually
# keys recovery off; a stale lock with no journal is treated as "nothing
# to roll back" rather than "restore from whatever .old happens to exist".
# ===========================================================================
echo
echo "== F31: recovers from a stale lock + journal (simulated crash) =="
HOMECRASH="$WORKDIR/homecrash"
INSTALLCRASH="$HOMECRASH/.local/bin"
mkdir -p "$INSTALLCRASH"
run_install "$TOOLCHAIN_FULL" "$HOMECRASH" "$BASE/good" 1.2.3
check "F31 setup: initial install exits 0" [ "$RC" -eq 0 ]
OLD_CRASH_CONTENT=$(cat "$INSTALLCRASH/farhelm")

cp "$INSTALLCRASH/farhelm" "$INSTALLCRASH/.farhelm.old"
printf '#!/bin/sh\necho "farhelm 9.9.9-mid-swap"\n' >"$INSTALLCRASH/farhelm"
chmod 755 "$INSTALLCRASH/farhelm"
mkdir "$INSTALLCRASH/.farhelm-install.lock"
chmod 0700 "$INSTALLCRASH/.farhelm-install.lock"
echo 999999 >"$INSTALLCRASH/.farhelm-install.lock/pid" # a pid nothing on this machine holds
# The journal lives INSIDE the lock directory and names binaries, not
# paths: "PARK cli" is the whole record for "the old farhelm was moved
# aside to its .old backup".
printf 'PARK cli\n' >"$INSTALLCRASH/.farhelm-install.lock/journal"

run_install "$TOOLCHAIN_FULL" "$HOMECRASH" "$BASE/good" 1.2.3
check "F31: recovery run exits 1 (it repairs and asks for a retry, not a silent continue)" \
  [ "$RC" -ne 0 ]
check "F31: recovery run reports what it did" contains "$ERR" "recovered from an interrupted install/update"
check "F31: recovery restores the pre-crash farhelm byte-for-byte" \
  [ "$(cat "$INSTALLCRASH/farhelm")" = "$OLD_CRASH_CONTENT" ]
check "F31: recovery removes the stale lock" [ ! -e "$INSTALLCRASH/.farhelm-install.lock" ]
check "F31: recovery removes the durable backup once restored" [ ! -e "$INSTALLCRASH/.farhelm.old" ]
check "F31: recovery removes the journal" [ ! -e "$INSTALLCRASH/.farhelm-install.lock/journal" ]

run_install "$TOOLCHAIN_FULL" "$HOMECRASH" "$BASE/good" 1.2.3
check "F31: an ordinary run after recovery succeeds" [ "$RC" -eq 0 ]

# ===========================================================================
# Scenario: stale-lock recovery is exclusive. Two runs started after the same
# crash used to both replay the journal: the second pass, working from its
# own earlier read, undid the first pass's restore and deleted the binary.
# Recovery now takes a sidecar claim (`.farhelm-install.lock.recovering`)
# first. A run that finds the claim held refuses and changes nothing; once
# the claim is gone, recovery proceeds as in F31 and releases its claim.
# ===========================================================================
echo
echo "== stale-lock recovery refuses while another run holds the recovery claim =="
HOMECLAIM="$WORKDIR/homeclaim"
INSTALLCLAIM="$HOMECLAIM/.local/bin"
mkdir -p "$INSTALLCLAIM"
run_install "$TOOLCHAIN_FULL" "$HOMECLAIM" "$BASE/good" 1.2.3
check "claim setup: initial install exits 0" [ "$RC" -eq 0 ]
OLD_CLAIM_CONTENT=$(cat "$INSTALLCLAIM/farhelm")
cp "$INSTALLCLAIM/farhelm" "$INSTALLCLAIM/.farhelm.old"
printf '#!/bin/sh\necho "farhelm 9.9.9-mid-swap"\n' >"$INSTALLCLAIM/farhelm"
chmod 755 "$INSTALLCLAIM/farhelm"
MID_SWAP_CLAIM_CONTENT=$(cat "$INSTALLCLAIM/farhelm")
mkdir "$INSTALLCLAIM/.farhelm-install.lock"
chmod 0700 "$INSTALLCLAIM/.farhelm-install.lock"
echo 999999 >"$INSTALLCLAIM/.farhelm-install.lock/pid"
printf 'PARK cli\n' >"$INSTALLCLAIM/.farhelm-install.lock/journal"
mkdir "$INSTALLCLAIM/.farhelm-install.lock.recovering"

run_install "$TOOLCHAIN_FULL" "$HOMECLAIM" "$BASE/good" 1.2.3
check "claim: a run finding the recovery claim held refuses" [ "$RC" -ne 0 ]
check "claim: the refusal says another run is recovering" \
  contains "$ERR" "is recovering from an interrupted run"
check "claim: the refused run leaves the mid-swap binary alone" \
  [ "$(cat "$INSTALLCLAIM/farhelm")" = "$MID_SWAP_CLAIM_CONTENT" ]
check "claim: the refused run leaves the backup alone" [ -e "$INSTALLCLAIM/.farhelm.old" ]
check "claim: the refused run leaves the journal alone" \
  [ -e "$INSTALLCLAIM/.farhelm-install.lock/journal" ]

rmdir "$INSTALLCLAIM/.farhelm-install.lock.recovering"
run_install "$TOOLCHAIN_FULL" "$HOMECLAIM" "$BASE/good" 1.2.3
check "claim: once the claim is free, recovery restores the previous farhelm" \
  [ "$(cat "$INSTALLCLAIM/farhelm")" = "$OLD_CLAIM_CONTENT" ]
check "claim: recovery releases its claim" [ ! -e "$INSTALLCLAIM/.farhelm-install.lock.recovering" ]
check "claim: recovery removes the stale lock" [ ! -e "$INSTALLCLAIM/.farhelm-install.lock" ]

# ===========================================================================
# Scenario: a stale lock that names the installer's OWN pid. In containers
# and other deterministic launch environments sh often gets the same small
# pid every start, so a SIGKILLed run's lock can name the pid of the next
# run itself; `kill -0 $$` always succeeds, and the installer used to refuse
# every run as "already running". A wrapper records its own pid in the lock
# and then `exec`s the installer, which therefore inherits that exact pid.
# ===========================================================================
echo
echo "== stale lock naming the installer's own pid =="
HOMEOWNPID="$WORKDIR/homeownpid"
INSTALLOWNPID="$HOMEOWNPID/.local/bin"
mkdir -p "$INSTALLOWNPID"
run_install "$TOOLCHAIN_FULL" "$HOMEOWNPID" "$BASE/good" 1.2.3
check "own-pid setup: initial install exits 0" [ "$RC" -eq 0 ]
mkdir "$INSTALLOWNPID/.farhelm-install.lock"
chmod 0700 "$INSTALLOWNPID/.farhelm-install.lock"
OWNPID_WRAPPER="$WORKDIR/own-pid-wrapper.sh"
# A literal wrapper that takes both paths from its environment, so no path
# is ever spliced into shell text (a space or metacharacter in the checkout
# or TMPDIR would otherwise break it or run as code).
cat >"$OWNPID_WRAPPER" <<'WRAPPER'
#!/bin/sh
echo $$ >"$OWNPID_LOCK_PID"
exec /bin/sh "$OWNPID_REAL_INSTALL"
WRAPPER
REAL_INSTALL_SH=$INSTALL_SH
INSTALL_SH=$OWNPID_WRAPPER
run_install "$TOOLCHAIN_FULL" "$HOMEOWNPID" "$BASE/good" 1.2.3 \
  OWNPID_LOCK_PID="$INSTALLOWNPID/.farhelm-install.lock/pid" \
  OWNPID_REAL_INSTALL="$REAL_INSTALL_SH"
INSTALL_SH=$REAL_INSTALL_SH
check "own-pid: a lock naming the installer's own pid is treated as stale" [ "$RC" -eq 0 ]
check "own-pid: it is not reported as another running install" \
  not_contains "$ERR" "is already running"
check "own-pid: the stale lock is gone afterwards" [ ! -e "$INSTALLOWNPID/.farhelm-install.lock" ]

# ===========================================================================
# Scenario: Apple silicon under Rosetta (F12) -- `uname -m` reports
# `x86_64` (the CURRENT process is translated) while `sysctl -n
# hw.optional.arm64` confirms the underlying hardware is Apple silicon;
# the arm64 assets must be installed. A genuine Intel Mac (the sysctl
# absent or not "1") must still be rejected.
# ===========================================================================
echo
echo "== F12: Apple silicon under Rosetta =="
ROSETTA_TOOLS="$WORKDIR/toolchain-rosetta"
mkdir -p "$ROSETTA_TOOLS"
cp -a "$TOOLCHAIN_FULL"/. "$ROSETTA_TOOLS/"
rm -f "$ROSETTA_TOOLS/uname" "$ROSETTA_TOOLS/sysctl"
cat >"$ROSETTA_TOOLS/uname" <<'EOF'
#!/bin/sh
case "$1" in
  -s) echo Darwin ;;
  -m) echo x86_64 ;;
esac
EOF
chmod 755 "$ROSETTA_TOOLS/uname"
cat >"$ROSETTA_TOOLS/sysctl" <<'EOF'
#!/bin/sh
case "$*" in
  "-n hw.optional.arm64") echo 1 ;;
esac
EOF
chmod 755 "$ROSETTA_TOOLS/sysctl"

HOMEROSETTA="$WORKDIR/homerosetta"
mkdir -p "$HOMEROSETTA"
run_install "$ROSETTA_TOOLS" "$HOMEROSETTA" "$BASE/good" 1.2.3
check "F12: Rosetta-shaped Darwin/x86_64 with arm64 hardware installs" [ "$RC" -eq 0 ]
check "F12: Rosetta-shaped install fetches the aarch64-apple-darwin assets (farhelm-desktop too)" \
  [ -x "$HOMEROSETTA/.local/bin/farhelm-desktop" ]

INTEL_TOOLS="$WORKDIR/toolchain-intel"
mkdir -p "$INTEL_TOOLS"
cp -a "$ROSETTA_TOOLS"/. "$INTEL_TOOLS/"
rm -f "$INTEL_TOOLS/sysctl"
HOMEINTEL="$WORKDIR/homeintel"
mkdir -p "$HOMEINTEL"
run_install "$INTEL_TOOLS" "$HOMEINTEL" "$BASE/good" 1.2.3
check "F12: a genuine Intel Mac (no arm64 hardware sysctl) is still rejected" [ "$RC" -ne 0 ]
check "F12: genuine Intel Mac rejection names the platform" contains "$ERR" "no release build for Darwin x86_64"

# ===========================================================================
# Scenario: 404 (no SHA256SUMS at all).
# ===========================================================================
echo
echo "== 404 (no SHA256SUMS) =="
HOME404="$WORKDIR/home404"
mkdir -p "$HOME404"
run_install "$TOOLCHAIN_FULL" "$HOME404" "$BASE/norelease" 1.2.3
check "404 exits 1" [ "$RC" -ne 0 ]
check "404 names the version and the HTTP code" contains "$ERR" "no SHA256SUMS for v1.2.3"
check "404 message mentions HTTP 404" contains "$ERR" "(HTTP 404)"
check "404 leaves the install dir with no leftover staging/lock dot-files" \
  [ -z "$(find "$HOME404/.local/bin" -maxdepth 1 -name '.farhelm-install.*' 2>/dev/null || true)" ]

# ===========================================================================
# Scenario: checksum mismatch.
# ===========================================================================
echo
echo "== checksum mismatch =="
HOMEBADSUM="$WORKDIR/homebadsum"
mkdir -p "$HOMEBADSUM"
run_install "$TOOLCHAIN_FULL" "$HOMEBADSUM" "$BASE/badchecksum" 1.2.3
check "checksum mismatch exits 1" [ "$RC" -ne 0 ]
check "checksum mismatch names the failure" contains "$ERR" "checksum mismatch"
check "checksum mismatch has the error prefix" contains "$ERR" "❌ farhelm-aarch64-apple-darwin.tar.gz: checksum mismatch"

# ===========================================================================
# Scenario: malformed archive -- two members named farhelm.
# ===========================================================================
echo
echo "== malformed archive: two members named farhelm =="
HOMETWO="$WORKDIR/hometwo"
mkdir -p "$HOMETWO"
run_install "$TOOLCHAIN_FULL" "$HOMETWO" "$BASE/twomember" 1.2.3
check "two-member archive exits 1" [ "$RC" -ne 0 ]
check "two-member archive names the count" contains "$ERR" "expected exactly 1"

# ===========================================================================
# Scenario: malformed archive -- a non-regular (symlink) member.
# ===========================================================================
echo
echo "== malformed archive: non-regular member =="
HOMENONREG="$WORKDIR/homenonreg"
mkdir -p "$HOMENONREG"
run_install "$TOOLCHAIN_FULL" "$HOMENONREG" "$BASE/nonregular" 1.2.3
check "non-regular member exits 1" [ "$RC" -ne 0 ]
check "non-regular member names the problem" contains "$ERR" "is not a regular file"

# ===========================================================================
# Scenario: tar lists the member by name but cannot produce its verbose
# (type) listing. The installer runs under `set -e`, and that listing's
# failure used to end the run right at the assignment, silently (tar's error
# is discarded), before the extraction step's named refusals could print. A
# `tar` shim fails only the verbose listing; every other use goes to the real
# one.
# ===========================================================================
echo
echo "== tar cannot list the member's type =="
TVFAIL_TOOLS="$WORKDIR/toolchain-tvfail"
mkdir -p "$TVFAIL_TOOLS"
cp -a "$TOOLCHAIN_FULL"/. "$TVFAIL_TOOLS/"
REAL_TAR=$(command -v tar)
rm -f "$TVFAIL_TOOLS/tar"
# shellcheck disable=SC2016 # $1 and $@ belong to the generated shim, not here
printf '#!/bin/sh\ncase "$1" in\n  tvzf) exit 2 ;;\nesac\nexec %s "$@"\n' "$REAL_TAR" \
  >"$TVFAIL_TOOLS/tar"
chmod 755 "$TVFAIL_TOOLS/tar"
HOMETVFAIL="$WORKDIR/hometvfail"
mkdir -p "$HOMETVFAIL"
run_install "$TVFAIL_TOOLS" "$HOMETVFAIL" "$BASE/good" 1.2.3
check "tv-listing failure exits 1" [ "$RC" -ne 0 ]
check "tv-listing failure is explained, not silent" contains "$ERR" "could not list"

# ===========================================================================
# Scenario: version normalization, including the -rc.N and -dev.N prereleases
# (D15). Both suffixes go through the same pattern, so both are driven here:
# a pattern edit that kept one and dropped the other would otherwise only
# show up when someone pinned a dev build for real.
# ===========================================================================
echo
echo "== version normalization =="
for spec in "1.2.3-rc.1" "v1.2.3-rc.1"; do
  HOMEV="$WORKDIR/homev-${spec//./-}"
  mkdir -p "$HOMEV"
  run_install "$TOOLCHAIN_FULL" "$HOMEV" "$BASE/prerelease" "$spec"
  check "FARHELM_VERSION=$spec exits 0" [ "$RC" -eq 0 ]
  check "FARHELM_VERSION=$spec normalizes to farhelm 1.2.3-rc.1" \
    contains "$("$HOMEV/.local/bin/farhelm" --version 2>/dev/null || true)" "farhelm 1.2.3-rc.1"
done
for spec in "1.2.3-dev.1" "v1.2.3-dev.1"; do
  HOMEV="$WORKDIR/homev-${spec//./-}"
  mkdir -p "$HOMEV"
  run_install "$TOOLCHAIN_FULL" "$HOMEV" "$BASE/prerelease-dev" "$spec"
  check "FARHELM_VERSION=$spec exits 0" [ "$RC" -eq 0 ]
  check "FARHELM_VERSION=$spec normalizes to farhelm 1.2.3-dev.1" \
    contains "$("$HOMEV/.local/bin/farhelm" --version 2>/dev/null || true)" "farhelm 1.2.3-dev.1"
done

# The 0.0.0-unreleased sentinel every build of main carries installs ONLY
# through the test-only base URL: test-uninstall.py depends on the first
# half, and the second half keeps the sentinel from ever being requested from
# GitHub, where no such release exists. Driven in both spellings because the
# acceptance is an exact string match rather than the pattern above.
for spec in "0.0.0-unreleased" "v0.0.0-unreleased"; do
  HOMEV="$WORKDIR/homev-${spec//./-}"
  mkdir -p "$HOMEV"
  run_install "$TOOLCHAIN_FULL" "$HOMEV" "$BASE/unreleased" "$spec"
  check "FARHELM_VERSION=$spec installs from the test base URL" [ "$RC" -eq 0 ]
  check "FARHELM_VERSION=$spec installs farhelm 0.0.0-unreleased" \
    contains "$("$HOMEV/.local/bin/farhelm" --version 2>/dev/null || true)" "farhelm 0.0.0-unreleased"
done
HOMEUNREL="$WORKDIR/homeunreleased-nobase"
mkdir -p "$HOMEUNREL"
run_install "$TOOLCHAIN_FULL" "$HOMEUNREL" "" "0.0.0-unreleased"
check "FARHELM_VERSION=0.0.0-unreleased without the test base URL exits 1" [ "$RC" -ne 0 ]
check "FARHELM_VERSION=0.0.0-unreleased without the test base URL is a version error" \
  contains "$ERR" "is not X.Y.Z"

HOMEPLAIN="$WORKDIR/homeplain"
mkdir -p "$HOMEPLAIN"
run_install "$TOOLCHAIN_FULL" "$HOMEPLAIN" "$BASE/good" "1.2.3"
check "bare X.Y.Z (no leading v) installs" [ "$RC" -eq 0 ]
HOMEVPLAIN="$WORKDIR/homevplain"
mkdir -p "$HOMEVPLAIN"
run_install "$TOOLCHAIN_FULL" "$HOMEVPLAIN" "$BASE/good" "v1.2.3"
check "vX.Y.Z installs" [ "$RC" -eq 0 ]

# ===========================================================================
# Scenario: invalid FARHELM_VERSION values -- every one of these must fail
# WITHOUT making any network request (checked against the fixture server's
# request log, which must not grow).
# ===========================================================================
echo
echo "== invalid FARHELM_VERSION values =="
REQUESTS_BEFORE_INVALID=$(server_request_count)
INVALID_VERSIONS=(
  "abc"
  "1.2"
  "01.2.3"
  "1.2.3.4"
  "1.2.3-beta.1"
  $'1.2.3\nrm -rf /tmp/should-not-run'
)
i=0
for v in "${INVALID_VERSIONS[@]}"; do
  i=$((i + 1))
  HOMEINV="$WORKDIR/homeinv$i"
  mkdir -p "$HOMEINV"
  run_install "$TOOLCHAIN_FULL" "$HOMEINV" "$BASE/good" "$v"
  check "invalid FARHELM_VERSION #$i exits 1" [ "$RC" -ne 0 ]
  check "invalid FARHELM_VERSION #$i names the problem" contains "$ERR" "is not X.Y.Z"
  check "invalid FARHELM_VERSION #$i created no staging directory" \
    [ -z "$(find "$HOMEINV/.local/bin" -mindepth 1 -maxdepth 1 2>/dev/null || true)" ]
done
REQUESTS_AFTER_INVALID=$(server_request_count)
check "invalid FARHELM_VERSION values made zero network requests" \
  [ "$REQUESTS_BEFORE_INVALID" -eq "$REQUESTS_AFTER_INVALID" ]

# ===========================================================================
# Scenario: missing prerequisites, one tool at a time, via a PATH shim --
# each must fail by name, before any network request, and without creating
# a staging directory.
# ===========================================================================
echo
echo "== missing prerequisites =="
for missing in curl tar; do
  TOOLS_MISSING="$WORKDIR/toolchain-missing-$missing"
  make_toolchain "$TOOLS_MISSING" "$missing"
  HOMEM="$WORKDIR/homemissing-$missing"
  mkdir -p "$HOMEM"
  before=$(server_request_count)
  run_install "$TOOLS_MISSING" "$HOMEM" "$BASE/good" 1.2.3
  after=$(server_request_count)
  check "missing $missing exits 1" [ "$RC" -ne 0 ]
  check "missing $missing is named in the message" contains "$ERR" "$missing"
  check "missing $missing made no network request" [ "$before" -eq "$after" ]
  check "missing $missing created no staging directory" \
    [ -z "$(find "$HOMEM/.local/bin" -mindepth 1 -maxdepth 1 2>/dev/null || true)" ]
done

TOOLS_NO_CHECKSUM="$WORKDIR/toolchain-no-checksum"
make_toolchain "$TOOLS_NO_CHECKSUM" sha256sum shasum openssl
HOMENOSUM="$WORKDIR/homenosum"
mkdir -p "$HOMENOSUM"
# F21: prove this is refused BEFORE any network access or staging-directory
# creation, not merely that it fails eventually -- if checksum-tool
# detection ever moved after a download, this is what would catch it.
before_nosum=$(server_request_count)
run_install "$TOOLS_NO_CHECKSUM" "$HOMENOSUM" "$BASE/good" 1.2.3
after_nosum=$(server_request_count)
check "no checksum tool at all exits 1" [ "$RC" -ne 0 ]
check "no checksum tool names the requirement" contains "$ERR" "sha256sum-or-shasum-or-openssl"
check "no checksum tool made no network request" [ "$before_nosum" -eq "$after_nosum" ]
check "no checksum tool created no staging/lock/journal entry" \
  [ -z "$(find "$HOMENOSUM/.local/bin" -mindepth 1 -maxdepth 1 2>/dev/null || true)" ]

# Each supported checksum-tool fallback completes a real install on its own.
for only in sha256sum shasum openssl; do
  omit=()
  for t in sha256sum shasum openssl; do
    if [ "$t" != "$only" ]; then
      omit+=("$t")
    fi
  done
  TOOLS_ONLY="$WORKDIR/toolchain-only-$only"
  make_toolchain "$TOOLS_ONLY" "${omit[@]}"
  HOMEONLY="$WORKDIR/homeonly-$only"
  mkdir -p "$HOMEONLY"
  run_install "$TOOLS_ONLY" "$HOMEONLY" "$BASE/good" 1.2.3
  check "checksum fallback via only $only succeeds" [ "$RC" -eq 0 ]
  check "checksum fallback via only $only produces a working farhelm" \
    contains "$("$HOMEONLY/.local/bin/farhelm" --version 2>/dev/null || true)" "farhelm 1.2.3"
done

# ===========================================================================
# Approved report text, including blank lines and order. Real updates start
# from a verified installer-owned pair, so kept-file notices cannot hide a
# wrong update classification. tmux fixtures isolate the launch prerequisite.
# ===========================================================================
echo
echo "== closing-message contract =="
assert_closing_message_contract() {
  local label=$1 is_update=$2 tmux_kind=$3
  local tools="$WORKDIR/toolchain-report-$label" home="$WORKDIR/home-report-$label"
  mkdir -p "$tools"
  cp -a "$TOOLCHAIN_FULL"/. "$tools/"
  case "$tmux_kind" in
    floor) write_fake_tmux "$tools" "tmux $TMUX_FLOOR" ;;
    old) write_fake_tmux "$tools" "tmux 3.4" ;;
  esac
  if [ "$is_update" = yes ]; then
    run_install "$tools" "$home" "$BASE/good" 1.2.3
    check "report ($label): update premise succeeds" [ "$RC" -eq 0 ]
    check "report ($label): update premise owns both binaries" assert_standalone_record "$home/.local/bin"
  fi
  run_install "$tools" "$home" "$BASE/good-v2" 1.2.4
  check "report ($label): install succeeds" [ "$RC" -eq 0 ]
  local expected
  if [ "$is_update" = yes ]; then
    expected='✅ Farhelm 1.2.4 is ready.

   Quit and reopen Farhelm to finish updating. Your sessions keep running.'
  else
    expected='✅ Farhelm 1.2.4 is installed.'
    if [ "$tmux_kind" = floor ]; then
      expected+='

   Open Farhelm from Spotlight or ~/Applications.'
    fi
  fi
  expected+='

   To uninstall later, run: ~/.local/bin/farhelm uninstall'
  if [ "$tmux_kind" != floor ]; then
    expected+="

⚠️  $TMUX_FLOOR_HINT"
    if [ "$tmux_kind" = absent ]; then
      expected+='
   This Mac has none. Install it with Homebrew: brew install tmux'
    else
      expected+='
   This Mac has tmux 3.4. Upgrade it with Homebrew: brew upgrade tmux'
    fi
    expected+='
   No Homebrew yet? Install it first: https://brew.sh/'
    if [ "$is_update" = yes ]; then
      expected+='

   Then quit and reopen Farhelm to finish updating.'
    else
      expected+='

   Then open Farhelm from Spotlight or ~/Applications.'
    fi
  fi
  check "report ($label): exact approved plain text" [ "$OUT" = "$expected" ]
  check "report ($label): redirected stdout has no escapes" not_contains "$OUT" $'\033'
  check "report ($label): redirected stderr has no escapes" not_contains "$ERR" $'\033'
  check "report ($label): no progress on redirected stderr" not_contains "$ERR" 'Downloading Farhelm'
}
for report_tmux in floor absent old; do
  assert_closing_message_contract "fresh-$report_tmux" no "$report_tmux"
  assert_closing_message_contract "update-$report_tmux" yes "$report_tmux"
done

# NO_COLOR must not be needed to make redirected output safe to consume.
# Both unset and set cases are covered; the exact-report cases above use unset.
run_install "$TOOLCHAIN_FULL" "$WORKDIR/home-no-color" "$BASE/good" 1.2.3 NO_COLOR=1
check "NO_COLOR redirected install succeeds" [ "$RC" -eq 0 ]
check "NO_COLOR redirected stdout has no escapes" not_contains "$OUT" $'\033'
check "NO_COLOR redirected stderr has no escapes" not_contains "$ERR" $'\033'

# Drive stdout and stderr through separate pseudo-terminals. Checking all four
# combinations catches a report that accidentally uses stderr's TTY decision.
# Read the terminals while the child runs: waiting first could fill a PTY buffer
# and deadlock a failure diagnostic or curl's progress output.
check "terminal output and independent stream gating" python3 - "$INSTALL_SH" "$TOOLCHAIN_FULL" "$LINUX_TOOLS" "$WORKDIR" "$BASE" <<'PYTTY'
import errno
import os
from pathlib import Path
import pty
import re
import selectors
import subprocess
import sys
import tempfile
import time

installer, tools, linux_tools, work, base = sys.argv[1:]
assert not (Path(tools) / "tmux").exists(), "terminal-report fixture requires absent tmux"


def capture(label, stdout_tty, stderr_tty, no_color=None, linux=False):
    """Capture bounded child output without inheriting the harness environment.

    A PTY supplies isatty, not shell interactivity. Each stream owns its own
    terminal, so styling and progress decisions can be checked independently.
    """
    env = dict(PATH=linux_tools if linux else tools,
               HOME=str(Path(work) / ("tty-home-" + label)),
               FARHELM_INSTALL_TEST_BASE_URL=base + "/good",
               FARHELM_VERSION="1.2.3", TERM="xterm")
    if no_color is not None:
        env["NO_COLOR"] = no_color
    buffers = [bytearray(), bytearray()]
    terminals = []
    outputs = []
    with selectors.DefaultSelector() as selector:
        for index, terminal in enumerate((stdout_tty, stderr_tty)):
            if terminal:
                master, slave = pty.openpty()
                terminals.append((master, slave))
                outputs.append(slave)
                selector.register(master, selectors.EVENT_READ, index)
            else:
                outputs.append(tempfile.TemporaryFile())
        child = subprocess.Popen(["/bin/sh", installer], env=env,
                                 stdin=subprocess.DEVNULL,
                                 stdout=outputs[0], stderr=outputs[1])
        for _, slave in terminals:
            os.close(slave)
        deadline = time.monotonic() + 45
        try:
            while selector.get_map() or child.poll() is None:
                if time.monotonic() >= deadline:
                    raise AssertionError(f"{label}: installer exceeded terminal-test deadline")
                for key, _ in selector.select(0.2):
                    try:
                        data = os.read(key.fd, 8192)
                    except OSError as error:
                        if error.errno != errno.EIO:
                            raise
                        data = b""
                    if not data:
                        selector.unregister(key.fd)
                    buffers[key.data].extend(data)
                    assert len(buffers[key.data]) <= 65536, f"{label}: excessive output"
            rc = child.wait(timeout=1)
        finally:
            if child.poll() is None:
                child.kill()
            child.wait()
            for master, _ in terminals:
                os.close(master)
            for index, output in enumerate(outputs):
                if not isinstance(output, int):
                    output.seek(0)
                    buffers[index].extend(output.read(65537))
                    output.close()
                    assert len(buffers[index]) <= 65536, f"{label}: excessive redirected output"
    out, err = (bytes(data).replace(b"\r\n", b"\n") for data in buffers)
    assert rc == (1 if linux else 0), (label, rc, out, err)
    return out, err


for out_tty, err_tty in ((True, True), (True, False), (False, True), (False, False)):
    label = f"{int(out_tty)}-{int(err_tty)}"
    out, err = capture(label, out_tty, err_tty)
    assert (b"\x1b[1;32m" in out) == out_tty, (label, out)
    assert (b"\x1b]8;;https://brew.sh/" in out) == out_tty, (label, out)
    assert (b"Downloading Farhelm 1.2.3" in err) == err_tty, (label, err)
    assert (b"[1/2]" in err and b"[2/2]" in err) == err_tty, (label, err)
    assert (b"100.0%" in err) == err_tty, (label, err)
    if err_tty:
        assert err.count(b"100.0%") == 2, ("only the two archive transfers get bars", err)
        assert b"\x1b[1m" in err, err
    if not out_tty:
        assert b"\x1b" not in out, out
    if not err_tty:
        assert b"\x1b" not in err, err
    if out_tty and err_tty:
        assert b"This Mac has none." in out, out
        plain = re.sub(rb"\x1b\[[0-9;]*m", b"", out)
        plain = re.sub(rb"\x1b\]8;;[^\x1b]*\x1b\\", b"", plain)
        expected = ("✅ Farhelm 1.2.3 is installed.\n\n"
                    "   To uninstall later, run: ~/.local/bin/farhelm uninstall\n\n"
                    "⚠️  Farhelm needs tmux 3.7c or newer before it can start.\n"
                    "   This Mac has none. Install it with Homebrew: brew install tmux\n"
                    "   No Homebrew yet? Install it first: https://brew.sh/\n\n"
                    "   Then open Farhelm from Spotlight or ~/Applications.\n").encode()
        assert plain == expected, (plain, expected)
        print("Observed terminal report:\n" + out.decode())
        print("Observed terminal progress:\n" + err.decode())

out, err = capture("no-color", True, True, no_color="1")
assert b"\x1b[" not in out and b"\x1b[" not in err, (out, err)
assert b"\x1b]8;;https://brew.sh/" in out, out
assert b"Downloading Farhelm" in err and b"100.0%" in err, err
out, err = capture("empty-no-color", True, True, no_color="")
assert b"\x1b[1;32m" in out and b"\x1b[1m" in err, (out, err)
for terminal in (False, True):
    out, err = capture("linux-" + str(terminal), False, terminal, linux=True)
    assert not out, out
    assert "❌ ".encode() in err, err
    assert (b"\x1b[1;31m" in err) == terminal, err
    assert b"Downloading" not in err, err
PYTTY

# Boundary comparison stays tied to the pinned floor. A present but malformed
# tmux reports its raw version output rather than claiming tmux is absent.
run_tmux_case() {
  local label=$1 tmux_output=$2 expect_hint=$3
  local tools="$WORKDIR/toolchain-tmux-$label" home="$WORKDIR/home-tmux-$label"
  mkdir -p "$tools"
  cp -a "$TOOLCHAIN_FULL"/. "$tools/"
  if [ -n "$tmux_output" ]; then
    write_fake_tmux "$tools" "$tmux_output"
  fi
  run_install "$tools" "$home" "$BASE/good" 1.2.3
  check "tmux hint ($label): install succeeds" [ "$RC" -eq 0 ]
  if [ "$expect_hint" = yes ]; then
    check "tmux hint ($label): present" contains "$OUT" "$TMUX_FLOOR_HINT"
    if [ -n "$tmux_output" ]; then
      check "tmux hint ($label): reports the actual version output" \
        contains "$OUT" "This Mac has tmux ${tmux_output#tmux }. Upgrade it with Homebrew: brew upgrade tmux"
    else
      check "tmux hint ($label): reports absence" contains "$OUT" "This Mac has none."
    fi
  else
    check "tmux hint ($label): absent" not_contains "$OUT" "$TMUX_FLOOR_HINT"
  fi
}
run_tmux_case "absent" "" yes
run_tmux_case "malformed" "tmux next-3.8" yes
# The boundary cases come from the pinned floor (see read_pinned_tmux_floor):
# the previous release both bare and with the last possible patch letter
# (so a floor raised by a minor release cannot leave the old letter check
# accepting e.g. 3.7z), the same minor without its patch letter and with the
# letter before it, the floor itself, and one minor release above. A `.0`
# floor has no minor to step down, so its previous release is taken from the
# major below.
if [ "$TMUX_FLOOR_MINOR" -gt 0 ]; then
  previous_release="$TMUX_FLOOR_MAJOR.$((TMUX_FLOOR_MINOR - 1))"
else
  previous_release="$((TMUX_FLOOR_MAJOR - 1)).99"
fi
run_tmux_case "below-floor" "tmux $previous_release" yes
run_tmux_case "below-floor-last-patch" "tmux ${previous_release}z" yes
if [ -n "$TMUX_FLOOR_LETTER" ]; then
  no_letter="tmux $TMUX_FLOOR_MAJOR.$TMUX_FLOOR_MINOR"
  run_tmux_case "below-floor-no-letter" "$no_letter" yes
  if [ "$TMUX_FLOOR_LETTER" != a ]; then
    letters=abcdefghijklmnopqrstuvwxyz
    prefix=${letters%%"$TMUX_FLOOR_LETTER"*}
    previous_letter="tmux $TMUX_FLOOR_MAJOR.$TMUX_FLOOR_MINOR${prefix: -1}"
    run_tmux_case "below-floor-letter" "$previous_letter" yes
  fi
fi
run_tmux_case "at-floor" "tmux $TMUX_FLOOR" no
run_tmux_case "above-floor" "tmux $TMUX_FLOOR_MAJOR.$((TMUX_FLOOR_MINOR + 1))" no

# ===========================================================================
# Scenario: no side effects outside the bin directory and bundle (F27).
# Neither systemctl nor launchctl may run.
# ===========================================================================
echo
echo "== no side effects outside the installation and bundle =="
SENTINEL_TOOLS="$WORKDIR/toolchain-sentinel"
mkdir -p "$SENTINEL_TOOLS"
cp -a "$TOOLCHAIN_FULL"/. "$SENTINEL_TOOLS/"
SENTINEL_LOG="$WORKDIR/sentinel.log"
: >"$SENTINEL_LOG"
for svc in systemctl launchctl; do
  cat >"$SENTINEL_TOOLS/$svc" <<EOF
#!/bin/sh
echo "$svc called: \$*" >>"$SENTINEL_LOG"
EOF
  chmod 755 "$SENTINEL_TOOLS/$svc"
done

HOMESIDE="$WORKDIR/homeside"
INSTALLSIDE="$HOMESIDE/.local/bin"
mkdir -p "$HOMESIDE"
# Seed a few ordinary files elsewhere under $HOME to prove they survive, and
# pre-create the two installation parents so the snapshot compares only
# unrelated entries. The flat bin directory and Farhelm.app are the only
# subtrees excluded from that comparison.
mkdir -p "$HOMESIDE/.local" "$HOMESIDE/Applications" "$HOMESIDE/.config" "$HOMESIDE/Documents"
echo "untouched" >"$HOMESIDE/.bashrc"
echo "untouched" >"$HOMESIDE/Documents/notes.txt"

BEFORE_SNAPSHOT=$(find_snapshot "$HOMESIDE")
run_install "$SENTINEL_TOOLS" "$HOMESIDE" "$BASE/good" 1.2.3
check "sentinel-guarded install exits 0" [ "$RC" -eq 0 ]
check "systemctl/launchctl were never invoked" [ ! -s "$SENTINEL_LOG" ]
check ".bashrc is untouched" [ "$(cat "$HOMESIDE/.bashrc")" = "untouched" ]
check "Documents/notes.txt is untouched" [ "$(cat "$HOMESIDE/Documents/notes.txt")" = "untouched" ]

AFTER_SNAPSHOT=$(find_snapshot "$HOMESIDE")
DIFF_OUTSIDE_INSTALL=$(diff <(printf '%s\n' "$BEFORE_SNAPSHOT" | grep -Fv -e "$INSTALLSIDE" -e "$HOMESIDE/Applications/Farhelm.app") \
  <(printf '%s\n' "$AFTER_SNAPSHOT" | grep -Fv -e "$INSTALLSIDE" -e "$HOMESIDE/Applications/Farhelm.app") || true)
check "nothing outside the installation and bundle changed under \$HOME" [ -z "$DIFF_OUTSIDE_INSTALL" ]

# ===========================================================================
# Scenario: SIGTERM mid-run cleans up (F29) -- proves the INT/TERM/HUP
# traps actually run cleanup, not just the EXIT trap on an ordinary
# success or failure. Uses the /slow/ fixture prefix (a deliberate 2s
# server-side delay on every response) to get a reliable window in which
# to signal install.sh while it is blocked inside a download, well after
# it has created its staging directory.
# ===========================================================================
echo
echo "== F29: SIGTERM mid-run cleans up (signal traps, not just EXIT) =="
HOMETERM="$WORKDIR/hometerm"
INSTALLTERM="$HOMETERM/.local/bin"
mkdir -p "$HOMETERM"
run_install_bg "$TOOLCHAIN_FULL" "$HOMETERM" "$BASE/slow" 1.2.3
sleep 0.5
if kill -0 "$BG_PID" 2>/dev/null; then
  kill -TERM "$BG_PID"
fi
TERM_RC=0
wait "$BG_PID" 2>/dev/null || TERM_RC=$?
check "SIGTERM mid-run: the process exited (non-zero, interrupted)" [ "$TERM_RC" -ne 0 ]
check "SIGTERM mid-run: no leftover staging directory" \
  [ -z "$(find "$INSTALLTERM" -maxdepth 1 -name '.farhelm-install.*' 2>/dev/null || true)" ]

# ===========================================================================
# Scenario: remaining integrity-gate branches (F18) -- each seeds a
# distinct sentinel farhelm at the destination first, so the assertions
# prove not just "refused" but "refused, and the existing installation is
# byte-for-byte untouched, with no staging/lock/journal/backup residue".
# ===========================================================================
echo
echo "== F18: remaining integrity-gate branches =="
check_integrity_gate() {
  local label=$1 fixture=$2 version=$3 expect_err=$4
  local home="$WORKDIR/gate-$label"
  local install="$home/.local/bin"
  mkdir -p "$install"
  printf '#!/bin/sh\necho "SENTINEL-%s"\n' "$label" >"$install/farhelm"
  chmod 755 "$install/farhelm"
  local sentinel
  sentinel=$(cat "$install/farhelm")
  run_install "$TOOLCHAIN_FULL" "$home" "$BASE/$fixture" "$version"
  check "F18 ($label): exits 1" [ "$RC" -ne 0 ]
  check "F18 ($label): names the problem" contains "$ERR" "$expect_err"
  check "F18 ($label): sentinel farhelm preserved byte-for-byte" [ "$(cat "$install/farhelm")" = "$sentinel" ]
  check "F18 ($label): no leftover staging/lock/journal/backup entries" \
    [ -z "$(find "$install" -maxdepth 1 -name '.farhelm*' 2>/dev/null || true)" ]
}
check_integrity_gate "zerorows" zerorows 1.2.3 "expected exactly 1"
check_integrity_gate "duprows" duprows 1.2.3 "expected exactly 1"
check_integrity_gate "zeromembers" zeromembers 1.2.3 "expected exactly 1"
check_integrity_gate "wrongversion" wrongversion 1.2.3 "expected 'farhelm 1.2.3'"
check_integrity_gate "decoybypass" decoybypass 1.2.3 "is not a regular file"

# ===========================================================================
# Scenario: macOS destination collision on farhelm-desktop specifically
# (F24) -- checking only the CLI destination could still let
# `mv` place a staged desktop executable inside a user-owned
# farhelm-desktop DIRECTORY while reporting success.
# ===========================================================================
echo
echo "== F24: macOS destination collision on farhelm-desktop =="
HOMEMACDIRDEST="$WORKDIR/homemacdirdest"
INSTALLMACDIRDEST="$HOMEMACDIRDEST/.local/bin"
mkdir -p "$INSTALLMACDIRDEST"
printf '#!/bin/sh\necho "farhelm 1.2.3"\n' >"$INSTALLMACDIRDEST/farhelm"
chmod 755 "$INSTALLMACDIRDEST/farhelm"
mkdir -p "$INSTALLMACDIRDEST/farhelm-desktop"
echo "sentinel" >"$INSTALLMACDIRDEST/farhelm-desktop/keepme"
run_install "$MAC_TOOLS" "$HOMEMACDIRDEST" "$BASE/good" 1.2.3
check "F24: macOS desktop-directory collision exits 1" [ "$RC" -ne 0 ]
check "F24: macOS desktop-directory collision names the problem" contains "$ERR" "is not a regular file"
check "F24: farhelm-desktop/keepme is untouched" [ "$(cat "$INSTALLMACDIRDEST/farhelm-desktop/keepme")" = "sentinel" ]
check "F24: the existing farhelm is untouched (refused before ANY move)" \
  [ "$("$INSTALLMACDIRDEST/farhelm" --version)" = "farhelm 1.2.3" ]

# ===========================================================================
# Scenario: SHA256SUMS answers a non-404 HTTP error (F25) -- the generic
# "download failed" branch, previously exercised only by the 404 path.
# ===========================================================================
echo
echo "== F25: SHA256SUMS returns 503 =="
HOME503="$WORKDIR/home503"
INSTALL503="$HOME503/.local/bin"
mkdir -p "$INSTALL503"
printf '#!/bin/sh\necho "SENTINEL-503"\n' >"$INSTALL503/farhelm"
chmod 755 "$INSTALL503/farhelm"
SENTINEL_503=$(cat "$INSTALL503/farhelm")
run_install "$TOOLCHAIN_FULL" "$HOME503" "$BASE/sums503" 1.2.3
check "F25: 503 exits 1" [ "$RC" -ne 0 ]
check "F25: 503 uses the generic diagnostic naming the code and URL" \
  contains "$ERR" "download failed (HTTP 503): $BASE/sums503/SHA256SUMS"
check "F25: 503 preserves the existing sentinel farhelm" [ "$(cat "$INSTALL503/farhelm")" = "$SENTINEL_503" ]
check "F25: 503 leaves no staging/lock/journal residue" \
  [ -z "$(find "$INSTALL503" -maxdepth 1 -name '.farhelm*' 2>/dev/null || true)" ]

# ===========================================================================
# Scenario: fixed installation layout (F26), including the always-built app.
# ===========================================================================
echo
echo "== F26: fixed install directory and app =="
HOMEDEFAULT="$WORKDIR/homedefault"
mkdir -p "$HOMEDEFAULT"
DEFAULT_OUT=$(mktemp "$WORKDIR/out.XXXXXX")
DEFAULT_ERR=$(mktemp "$WORKDIR/err.XXXXXX")
set +e
env -i PATH="$TOOLCHAIN_FULL" HOME="$HOMEDEFAULT" FARHELM_INSTALL_TEST_BASE_URL="$BASE/good" FARHELM_VERSION=1.2.3 \
  /bin/sh "$INSTALL_SH" >"$DEFAULT_OUT" 2>"$DEFAULT_ERR"
DEFAULT_RC=$?
set -e
check "F26: default install dir exits 0" [ "$DEFAULT_RC" -eq 0 ]
check "F26: farhelm lands at \$HOME/.local/bin/farhelm" [ -x "$HOMEDEFAULT/.local/bin/farhelm" ]
check "F26: nothing else under \$HOME was created" \
  [ "$(find "$HOMEDEFAULT" -type f 2>/dev/null | wc -l)" -eq 8 ]

# ===========================================================================
# Scenario: the real production download URL, with no FARHELM_RELEASE_
# BASE_URL override (F27) -- every OTHER successful scenario in this file
# bypasses actual construction of https://github.com/scode/farhelm/
# releases/download/vX.Y.Z/<asset>; this is the one that proves that URL
# shape itself, via a curl double that refuses anything not shaped exactly
# like it (a repo-name, tag-prefix, or path regression would show up as a
# hard failure here, not a silently-passing test). Deliberately does not
# touch the separate releases/latest lookup, per F27's own scope.
# ===========================================================================
echo
echo "== F27: production download URL shape (no base-URL override) =="
F27_VERSION=1.2.3
F27_FIXTURE_DIR="$WWW/good"
CURL_DOUBLE_F27="$WORKDIR/curl-double-f27.sh"
cat >"$CURL_DOUBLE_F27" <<CURLDOUBLE
#!/bin/sh
expected_prefix="https://github.com/scode/farhelm/releases/download/v${F27_VERSION}/"
fixture_dir="${F27_FIXTURE_DIR}"
out=""
prev=""
last=""
proto=0
proto_redir=0
proto_value=""
proto_redir_value=""
want_code=0
for arg in "\$@"; do
  if [ "\$prev" = "-o" ]; then
    out="\$arg"
  fi
  if [ "\$arg" = "%{http_code}" ]; then
    want_code=1
  fi
  if [ "\$arg" = "--proto" ]; then
    proto=1
  fi
  if [ "\$arg" = "--proto-redir" ]; then
    proto_redir=1
  fi
  if [ "\$prev" = "--proto" ]; then
    proto_value="\$arg"
  fi
  if [ "\$prev" = "--proto-redir" ]; then
    proto_redir_value="\$arg"
  fi
  prev="\$arg"
  last="\$arg"
done
if [ "\$proto" -ne 1 ] || [ "\$proto_redir" -ne 1 ] || \
   [ "\$proto_value" != "=https" ] || [ "\$proto_redir_value" != "=https" ]; then
  echo "curl double: default request omitted HTTPS protocol pins" >&2
  exit 1
fi
case "\$last" in
  "\${expected_prefix}"*)
    asset=\${last#"\$expected_prefix"}
    if [ -f "\$fixture_dir/\$asset" ]; then
      if [ -n "\$out" ]; then
        cp "\$fixture_dir/\$asset" "\$out"
      fi
      if [ "\$want_code" -eq 1 ]; then
        printf '200'
      fi
      exit 0
    fi
    echo "curl double: no fixture for \$asset" >&2
    exit 22
    ;;
  *)
    echo "curl double: refusing unexpected URL: \$last" >&2
    exit 1
    ;;
esac
CURLDOUBLE
chmod +x "$CURL_DOUBLE_F27"

TOOLS_F27="$WORKDIR/toolchain-f27"
mkdir -p "$TOOLS_F27"
cp -a "$TOOLCHAIN_FULL"/. "$TOOLS_F27/"
rm -f "$TOOLS_F27/curl"
cp "$CURL_DOUBLE_F27" "$TOOLS_F27/curl"
chmod +x "$TOOLS_F27/curl"

HOMEF27="$WORKDIR/homef27"
INSTALLF27="$HOMEF27/.local/bin"
mkdir -p "$INSTALLF27"
set +e
env -i PATH="$TOOLS_F27" HOME="$HOMEF27" FARHELM_VERSION="$F27_VERSION" \
  /bin/sh "$INSTALL_SH" >"$WORKDIR/f27-out" 2>"$WORKDIR/f27-err"
F27_RC=$?
set -e
check "F27: install against the real production URL shape exits 0" [ "$F27_RC" -eq 0 ]
check "F27: installed farhelm reports the requested version" \
  contains "$("$INSTALLF27/farhelm" --version 2>/dev/null || true)" "farhelm $F27_VERSION"

# ===========================================================================
# Scenario: the curl|sh truncation invariant (F1, F22) -- every byte
# prefix of install.sh, at every 4 KiB boundary across the whole file and
# then one byte at a time across the last 64 bytes, must fail closed: a
# non-zero exit, zero curl invocations, and zero files created in the
# fixture home. This is what actually exercises the `{ ... }`
# wrapper's fail-closed property end to end, rather than trusting it by
# inspection.
# ===========================================================================
echo
echo "== F22/F1: curl|sh truncation invariant =="
CURL_RECORDER="$WORKDIR/toolchain-f22"
mkdir -p "$CURL_RECORDER"
cp -a "$TOOLCHAIN_FULL"/. "$CURL_RECORDER/"
rm -f "$CURL_RECORDER/curl"
CURL_CALL_LOG="$WORKDIR/f22-curl-calls.log"
cat >"$CURL_RECORDER/curl" <<CURLEOF
#!/bin/sh
echo "curl called: \$*" >>"$CURL_CALL_LOG"
exit 1
CURLEOF
chmod +x "$CURL_RECORDER/curl"

FULL_SCRIPT_SIZE=$(wc -c <"$INSTALL_SH")
F22_FAILURES=0
F22_CHECKED=0

f22_check_prefix() {
  local off=$1
  local prefix_file="$WORKDIR/f22-prefix.sh"
  head -c "$off" "$INSTALL_SH" >"$prefix_file"
  local home="$WORKDIR/f22-home"
  rm -rf "$home"
  mkdir -p "$home"
  : >"$CURL_CALL_LOG"
  set +e
  env -i PATH="$CURL_RECORDER" HOME="$home" FARHELM_VERSION=1.2.3 \
    /bin/sh "$prefix_file" >/dev/null 2>/dev/null
  local rc=$?
  set -e
  F22_CHECKED=$((F22_CHECKED + 1))
  local created
  created=$(find "$home" -mindepth 1 2>/dev/null | wc -l)
  if [ "$rc" -eq 0 ] || [ -s "$CURL_CALL_LOG" ] || [ "$created" -ne 0 ]; then
    F22_FAILURES=$((F22_FAILURES + 1))
    printf 'NOT OK - F22: prefix at byte %s did not fail closed (rc=%s, curl_called=%s, files_created=%s)\n' \
      "$off" "$rc" "$([ -s "$CURL_CALL_LOG" ] && echo yes || echo no)" "$created" >&2
  fi
}

off=4096
while [ "$off" -lt "$FULL_SCRIPT_SIZE" ]; do
  f22_check_prefix "$off"
  off=$((off + 4096))
done
# Stops one byte short of the full size: the very last byte is the file's
# own trailing newline, and a "prefix" missing only that newline is not a
# meaningful truncation at all -- everything through the closing `}` has
# already arrived, so the shell legitimately parses and runs the complete
# script (which then makes its normal, real curl calls, exactly as
# intended). Every OTHER byte in this range is missing real content and
# must still fail closed.
start=$((FULL_SCRIPT_SIZE - 64))
[ "$start" -lt 1 ] && start=1
off=$start
while [ "$off" -lt "$((FULL_SCRIPT_SIZE - 1))" ]; do
  f22_check_prefix "$off"
  off=$((off + 1))
done
check "F22: all $F22_CHECKED truncated byte prefixes failed closed (no success, no curl call, no file created)" \
  [ "$F22_FAILURES" -eq 0 ]

# ===========================================================================
# Scenario: two installers racing the same lock (F23) -- a live concurrency
# test, not just the stale-lock recovery F31 already covers. An `mv`
# double pauses installer A on its FIRST move, which can only happen after
# `acquire_lock` has fully returned (lock directory created AND pid
# written) -- pausing INSIDE `mkdir` itself was tried first and does not
# work, because acquire_lock's own `mkdir "$LOCK_DIR"` call would never
# return control to write the pid file, and a second installer would then
# see an unpublished lock and correctly (but unhelpfully, for this test)
# refuse as "not readable yet" rather than "already running". Pausing on
# the first `mv` instead gives a clean window strictly inside replacement,
# with lock ownership already fully published.
# ===========================================================================
echo
echo "== F23: two installers racing the same lock =="
SYNC_READY="$WORKDIR/f23-ready"
SYNC_GO="$WORKDIR/f23-go"
SYNC_FIRSTMV_DONE="$WORKDIR/f23-firstmv-done"
rm -f "$SYNC_READY" "$SYNC_GO" "$SYNC_FIRSTMV_DONE"
TOOLS_F23A="$WORKDIR/toolchain-f23a"
mkdir -p "$TOOLS_F23A"
cp -a "$TOOLCHAIN_FULL"/. "$TOOLS_F23A/"
rm -f "$TOOLS_F23A/mv"
cat >"$TOOLS_F23A/mv" <<MVEOF
#!/bin/sh
# Test double: performs every move exactly like real mv. On the FIRST call
# only (tracked by a marker file, since a transaction makes several moves)
# it also signals readiness and waits for a "go" file before returning --
# giving the test a reliable window strictly inside replacement, after the
# lock is fully published.
if [ ! -e "$SYNC_FIRSTMV_DONE" ]; then
  : >"$SYNC_FIRSTMV_DONE"
  /bin/mv "\$@" || exit 1
  : >"$SYNC_READY"
  tries=200
  while [ ! -e "$SYNC_GO" ]; do
    tries=\$((tries - 1))
    if [ "\$tries" -le 0 ]; then
      exit 1
    fi
    sleep 0.05
  done
  exit 0
fi
exec /bin/mv "\$@"
MVEOF
chmod +x "$TOOLS_F23A/mv"

HOMEF23="$WORKDIR/homef23"
INSTALLF23="$HOMEF23/.local/bin"
mkdir -p "$INSTALLF23"
run_install_bg "$TOOLS_F23A" "$HOMEF23" "$BASE/good" 1.2.3
F23_PID_A=$BG_PID

f23_tries=200
while [ ! -e "$SYNC_READY" ]; do
  f23_tries=$((f23_tries - 1))
  if [ "$f23_tries" -le 0 ]; then
    echo "F23 setup: installer A never reached its first move" >&2
    break
  fi
  sleep 0.05
done
check "F23: installer A reached its first move (lock fully published)" [ -e "$SYNC_READY" ]
check "F23: installer A is still running (paused mid-replacement, not finished)" kill -0 "$F23_PID_A"

BEFORE_F23_SNAPSHOT=$(find_snapshot "$INSTALLF23")
run_install "$TOOLCHAIN_FULL" "$HOMEF23" "$BASE/good" 1.2.3
check "F23: installer B refuses while A holds the lock" [ "$RC" -ne 0 ]
check "F23: installer B's refusal names another running install" contains "$ERR" "already running"
AFTER_F23_B_SNAPSHOT=$(find_snapshot "$INSTALLF23")
check "F23: installer B changed nothing while refusing" [ "$BEFORE_F23_SNAPSHOT" = "$AFTER_F23_B_SNAPSHOT" ]

: >"$SYNC_GO"
wait "$F23_PID_A" 2>/dev/null || true
check "F23: installer A completes successfully once released" [ -x "$INSTALLF23/farhelm" ]
check "F23: installer A's farhelm actually runs" \
  contains "$("$INSTALLF23/farhelm" --version 2>/dev/null || true)" "farhelm 1.2.3"
check "F23: no leftover staging/lock/journal after the race resolves" \
  [ -z "$(find "$INSTALLF23" -maxdepth 1 -name '.farhelm*' ! -name '.farhelm-installation' 2>/dev/null || true)" ]

# ===========================================================================
# Scenario: the lock path is something other than our own lock (F6) -- a
# regular file, and a nonempty directory holding unrelated data. Both must
# be refused outright and left byte-for-byte untouched; neither may ever
# reach the recursive-delete path a genuine stale lock does.
# ===========================================================================
echo
echo "== F6: lock-path collisions are refused, never destroyed =="
HOMELOCKFILE="$WORKDIR/homelockfile"
INSTALLLOCKFILE="$HOMELOCKFILE/.local/bin"
mkdir -p "$INSTALLLOCKFILE"
echo "not a lock" >"$INSTALLLOCKFILE/.farhelm-install.lock"
run_install "$TOOLCHAIN_FULL" "$HOMELOCKFILE" "$BASE/good" 1.2.3
check "F6: a regular file at the lock path is refused" [ "$RC" -ne 0 ]
check "F6: refusal names it as not a farhelm lock" contains "$ERR" "not a farhelm install lock"
check "F6: the regular file is untouched" [ "$(cat "$INSTALLLOCKFILE/.farhelm-install.lock")" = "not a lock" ]

HOMELOCKDIR="$WORKDIR/homelockdir"
INSTALLLOCKDIR="$HOMELOCKDIR/.local/bin"
mkdir -p "$INSTALLLOCKDIR/.farhelm-install.lock"
echo "unrelated data" >"$INSTALLLOCKDIR/.farhelm-install.lock/somefile"
run_install "$TOOLCHAIN_FULL" "$HOMELOCKDIR" "$BASE/good" 1.2.3
check "F6: a nonempty unrelated directory at the lock path is refused" [ "$RC" -ne 0 ]
check "F6: refusal names it as not a farhelm lock (directory case)" contains "$ERR" "not a farhelm install lock"
check "F6: the unrelated directory's contents are untouched" \
  [ "$(cat "$INSTALLLOCKDIR/.farhelm-install.lock/somefile")" = "unrelated data" ]

# A lock with the expected names is still foreign when its directory is
# writable by another account. Once that boundary is checked, the same
# fixture is made owner-only so the dead-PID path proves a real installer lock
# remains recoverable.
HOMELOCKMODE="$WORKDIR/homelockmode"
INSTALLLOCKMODE="$HOMELOCKMODE/.local/bin"
mkdir -p "$INSTALLLOCKMODE"
mkdir "$INSTALLLOCKMODE/.farhelm-install.lock"
chmod 0770 "$INSTALLLOCKMODE/.farhelm-install.lock"
run_install "$TOOLCHAIN_FULL" "$HOMELOCKMODE" "$BASE/good" 1.2.3 QUOTING_STYLE=shell-always
check "F6: a group-writable shaped lock is refused" [ "$RC" -ne 0 ]
check "F6: a writable lock refusal keeps the existing message" \
  contains "$ERR" "not a farhelm install lock"
check "F6: the group-writable lock is left untouched" \
  [ -d "$INSTALLLOCKMODE/.farhelm-install.lock" ]

chmod 0700 "$INSTALLLOCKMODE/.farhelm-install.lock"
printf '999999\n' >"$INSTALLLOCKMODE/.farhelm-install.lock/pid"
run_install "$TOOLCHAIN_FULL" "$HOMELOCKMODE" "$BASE/good" 1.2.3
check "F6: an owner-only stale lock follows recovery" [ "$RC" -eq 0 ]
check "F6: an owner-only stale lock is removed after recovery" \
  [ ! -e "$INSTALLLOCKMODE/.farhelm-install.lock" ]

# ===========================================================================
# Scenario: a backup with no journal is committed debris (F7). A backup with
# a journal remains recovery state and follows the existing stale-lock path;
# an unrelated neighboring file is never swept.
# ===========================================================================
echo
echo "== F7: journal-free backups are committed debris =="
HOMEBACKUPDEBRIS="$WORKDIR/homebackupdebris"
INSTALLBACKUPDEBRIS="$HOMEBACKUPDEBRIS/.local/bin"
mkdir -p "$INSTALLBACKUPDEBRIS"
printf 'committed old bytes\n' >"$INSTALLBACKUPDEBRIS/.farhelm.old"
printf 'foreign bytes\n' >"$INSTALLBACKUPDEBRIS/.not-a-farhelm-backup"
run_install "$TOOLCHAIN_FULL" "$HOMEBACKUPDEBRIS" "$BASE/good" 1.2.3
check "F7: journal-free backup debris is removed by a successful run" [ "$RC" -eq 0 ]
check "F7: journal-free backup debris is gone" [ ! -e "$INSTALLBACKUPDEBRIS/.farhelm.old" ]
check "F7: debris cleanup emits one notice" contains "$ERR" "removed committed backup debris"
check "F7: a foreign neighboring file is untouched" \
  [ "$(cat "$INSTALLBACKUPDEBRIS/.not-a-farhelm-backup")" = "foreign bytes" ]
check "F7: override source is reported" contains "$ERR" "using FARHELM_INSTALL_TEST_BASE_URL=$BASE/good"

HOMEBACKUPJOURNAL="$WORKDIR/homebackupjournal"
INSTALLBACKUPJOURNAL="$HOMEBACKUPJOURNAL/.local/bin"
mkdir -p "$INSTALLBACKUPJOURNAL/.farhelm-install.lock"
chmod 0700 "$INSTALLBACKUPJOURNAL/.farhelm-install.lock"
printf '999999\n' >"$INSTALLBACKUPJOURNAL/.farhelm-install.lock/pid"
printf 'PARK cli\n' >"$INSTALLBACKUPJOURNAL/.farhelm-install.lock/journal"
printf '#!/bin/sh\necho "farhelm 1.2.3-backup"\n' >"$INSTALLBACKUPJOURNAL/.farhelm.old"
chmod 755 "$INSTALLBACKUPJOURNAL/.farhelm.old"
run_install "$TOOLCHAIN_FULL" "$HOMEBACKUPJOURNAL" "$BASE/good" 1.2.3
check "F7: journaled backup keeps recovery refusal behavior" [ "$RC" -ne 0 ]
check "F7: journaled backup is recovered into farhelm" \
  [ "$("$INSTALLBACKUPJOURNAL/farhelm" --version)" = "farhelm 1.2.3-backup" ]
check "F7: journaled recovery leaves no backup" [ ! -e "$INSTALLBACKUPJOURNAL/.farhelm.old" ]

HOMEINVALIDBASE="$WORKDIR/homeinvalidbase"
INSTALLINVALIDBASE="$HOMEINVALIDBASE/.local/bin"
run_install "$TOOLCHAIN_FULL" "$HOMEINVALIDBASE" \
  "http://user:pass@127.0.0.1:$SERVER_PORT/good" 1.2.3
check "F7: override userinfo is refused" [ "$RC" -eq 1 ]
check "F7: userinfo refusal names FARHELM_INSTALL_TEST_BASE_URL" \
  contains "$ERR" "FARHELM_INSTALL_TEST_BASE_URL"
check "F7: userinfo refusal creates no install state" \
  [ ! -e "$INSTALLINVALIDBASE" ]

run_install "$TOOLCHAIN_FULL" "$HOMEINVALIDBASE" \
  "$BASE/good?query=refused" 1.2.3
check "F7: override query is refused" [ "$RC" -eq 1 ]
check "F7: query refusal names FARHELM_INSTALL_TEST_BASE_URL" \
  contains "$ERR" "FARHELM_INSTALL_TEST_BASE_URL"

# ===========================================================================
# Scenario: stale-lock recovery finds its recovery DESTINATION corrupted
# (F8) -- farhelm has become a directory since the simulated crash. The
# rollback must refuse rather than let `mv` silently place the backup
# INSIDE that directory, and the lock/journal/backup must all survive for
# a human (or a later, successful recovery attempt) to act on.
# ===========================================================================
echo
echo "== F8: stale-lock recovery refuses to restore into a corrupted destination =="
HOMERECOVERYBAD="$WORKDIR/homerecoverybad"
INSTALLRECOVERYBAD="$HOMERECOVERYBAD/.local/bin"
mkdir -p "$INSTALLRECOVERYBAD"
printf '#!/bin/sh\necho "farhelm 1.2.3-backup"\n' >"$INSTALLRECOVERYBAD/.farhelm.old"
chmod 755 "$INSTALLRECOVERYBAD/.farhelm.old"
mkdir -p "$INSTALLRECOVERYBAD/farhelm" # farhelm has BECOME a directory since the simulated crash
echo "unrelated" >"$INSTALLRECOVERYBAD/farhelm/somefile"
mkdir "$INSTALLRECOVERYBAD/.farhelm-install.lock"
chmod 0700 "$INSTALLRECOVERYBAD/.farhelm-install.lock"
echo 999999 >"$INSTALLRECOVERYBAD/.farhelm-install.lock/pid"
printf 'PARK cli\n' >"$INSTALLRECOVERYBAD/.farhelm-install.lock/journal"

run_install "$TOOLCHAIN_FULL" "$HOMERECOVERYBAD" "$BASE/good" 1.2.3
check "F8: recovery into a corrupted destination exits 1" [ "$RC" -ne 0 ]
check "F8: recovery failure message names manual intervention" contains "$ERR" "LEFT IN PLACE"
check "F8: the lock is left in place" [ -e "$INSTALLRECOVERYBAD/.farhelm-install.lock" ]
check "F8: the journal is left in place" [ -e "$INSTALLRECOVERYBAD/.farhelm-install.lock/journal" ]
check "F8: the last-recoverable backup is NOT consumed" [ -e "$INSTALLRECOVERYBAD/.farhelm.old" ]
check "F8: the corrupted destination's contents are untouched" \
  [ "$(cat "$INSTALLRECOVERYBAD/farhelm/somefile")" = "unrelated" ]

# ===========================================================================
# Scenario: new install-directory components are not group/world-writable
# even under a permissive umask (F9).
# ===========================================================================
echo
echo "== F9: new install directory is not group/world-writable under umask 000 =="
HOMEUMASK="$WORKDIR/homeumask"
INSTALLUMASK="$HOMEUMASK/.local/bin"
mkdir -p "$HOMEUMASK"
set +e
# shellcheck disable=SC2016 # the single quotes are deliberate: "$1" must reach the INNER sh, not expand in this one
env -i PATH="$TOOLCHAIN_FULL" HOME="$HOMEUMASK" \
  FARHELM_INSTALL_TEST_BASE_URL="$BASE/good" FARHELM_VERSION=1.2.3 \
  /bin/sh -c 'umask 000; exec /bin/sh "$1"' _ "$INSTALL_SH" >"$WORKDIR/f9-out" 2>"$WORKDIR/f9-err"
F9_RC=$?
set -e
check "F9: umask-000 install exits 0" [ "$F9_RC" -eq 0 ]
check "F9: the installed binary is mode 0755" [ "$(stat -c %a "$INSTALLUMASK/farhelm")" = "755" ]
check "F9: the newly-created install directory is not group/world-writable" \
  [ "$(stat -c %a "$INSTALLUMASK")" = "755" ]
check "F9: the newly-created HOME .local directory is not group/world-writable" \
  [ "$(stat -c %a "$HOMEUMASK/.local")" = "755" ]

check "F9: Applications directory is not group/world-writable" \
  [ "$(stat -c %a "$HOMEUMASK/Applications")" = "755" ]
check "F9: bundle Contents directory is not group/world-writable" \
  [ "$(stat -c %a "$HOMEUMASK/Applications/Farhelm.app/Contents")" = "755" ]

# ===========================================================================
# Scenario: multiline tmux output is rejected as a whole, not scanned line
# by line, for a valid version (F12).
# ===========================================================================
echo
echo "== F12: multiline tmux -V output is rejected wholesale =="
run_tmux_case "banner-then-valid" "$(printf 'some vendor banner\ntmux %s' "$TMUX_FLOOR")" yes
run_tmux_case "two-valid-lines" "$(printf 'tmux %s\ntmux 3.8' "$TMUX_FLOOR")" yes

# ===========================================================================
# Scenario: a colon-containing HOME still installs without PATH advice.
# The fixed full-path uninstall command replaces PATH repair instructions,
# including for directory names that cannot form a single PATH entry.
# ===========================================================================
echo
echo "== F15: colon-containing install directory =="
HOMECOLON="$WORKDIR/home:colon"
INSTALLCOLON="$HOMECOLON/.local/bin"
mkdir -p "$INSTALLCOLON"
DECEPTIVE_PATH="$TOOLCHAIN_FULL:$HOMECOLON/.local/bin:extra:/usr/bin"
set +e
env -i PATH="$DECEPTIVE_PATH" HOME="$HOMECOLON" \
  FARHELM_INSTALL_TEST_BASE_URL="$BASE/good" FARHELM_VERSION=1.2.3 \
  /bin/sh "$INSTALL_SH" >"$WORKDIR/f15-out" 2>"$WORKDIR/f15-err"
F15_RC=$?
set -e
check "F15: colon-containing install dir still installs successfully" [ "$F15_RC" -eq 0 ]
check "F15: no PATH repair advice" not_contains "$(cat "$WORKDIR/f15-out")" "PATH"
# shellcheck disable=SC2088 # The approved output uses a literal home-relative command.
check "F15: full-path uninstall command is printed" contains "$(cat "$WORKDIR/f15-out")" '~/.local/bin/farhelm uninstall'

# ===========================================================================
# Round 3: the install-transaction journal itself.
#
# Everything below exercises the recovery machinery rather than the install
# it protects, so every scenario is macOS-shaped (two binaries, the only
# shape where a rollback has more than one pair of moves to unwind and can
# therefore be interrupted BETWEEN them) and updates 1.2.3 -> 1.2.4 with
# genuinely distinct fixtures, so "restored the old bytes" and "kept the
# new bytes" are distinguishable outcomes.
#
# The absolute paths of the real mktemp/awk, captured once: the doubles
# below have to delegate to the genuine tool, and their own directory is
# what $PATH points at inside the scenario.
# ===========================================================================
REAL_MKTEMP=$(command -v mktemp)
REAL_AWK=$(command -v awk)

# r3_seed_macos_pair LABEL HOME INSTALL_DIR -- establishes the 1.2.3 pair a
# round-3 scenario then tries (and fails) to update, and leaves its exact
# bytes in R3_OLD_FARHELM / R3_OLD_DESKTOP for the "restored byte-for-byte"
# assertions.
r3_seed_macos_pair() {
  local label=$1 home=$2 install=$3
  mkdir -p "$home"
  run_install "$MAC_TOOLS" "$home" "$BASE/good" 1.2.3
  check "$label: setup installs the 1.2.3 macOS pair" [ "$RC" -eq 0 ]
  R3_OLD_FARHELM=$(cat "$install/farhelm")
  R3_OLD_DESKTOP=$(cat "$install/farhelm-desktop")
}

# ===========================================================================
# Scenario: rollback's own scaffolding cannot silently skip work (R3 F1).
#
# The failure this guards against is not a wrong restore but a rollback that
# does NOTHING and returns success: its caller reads that as "the previous
# installation is back" and deletes the journal and lock on the strength of
# it, stranding the user with a half-swapped pair and no recovery state. The
# original implementation reversed the journal through an `mktemp` scratch
# file and an `awk` pass, checked neither, and returned success when both
# failed -- which is exactly the state a disk-full or I/O-error condition
# (the very conditions rollback runs in) produces.
#
# So the doubles here fail `mktemp` and `awk` from the moment replacement
# has begun, detected by the durable ".farhelm.old" backup the first PARK
# creates. Before that point both tools work normally, because staging and
# checksum verification legitimately need them; after it, the current
# implementation must not need them AT ALL.
# ===========================================================================
echo
echo "== R3 F1: rollback needs no scratch scaffolding it does not check =="
HOME_R3F1="$WORKDIR/home-r3f1"
INSTALL_R3F1="$HOME_R3F1/.local/bin"
r3_seed_macos_pair "R3 F1" "$HOME_R3F1" "$INSTALL_R3F1"

TOOLS_R3F1="$WORKDIR/toolchain-r3f1"
mkdir -p "$TOOLS_R3F1"
cp -a "$MAC_TOOLS_FAILDESKTOP"/. "$TOOLS_R3F1/"
rm -f "$TOOLS_R3F1/mktemp" "$TOOLS_R3F1/awk"
cat >"$TOOLS_R3F1/mktemp" <<EOF
#!/bin/sh
if [ -e "$INSTALL_R3F1/.farhelm.old" ]; then
  echo "fake mktemp: forced failure once replacement has begun" >&2
  exit 1
fi
exec "$REAL_MKTEMP" "\$@"
EOF
cat >"$TOOLS_R3F1/awk" <<EOF
#!/bin/sh
if [ -e "$INSTALL_R3F1/.farhelm.old" ]; then
  echo "fake awk: forced failure once replacement has begun" >&2
  exit 1
fi
exec "$REAL_AWK" "\$@"
EOF
chmod 755 "$TOOLS_R3F1/mktemp" "$TOOLS_R3F1/awk"

run_install "$TOOLS_R3F1" "$HOME_R3F1" "$BASE/good-v2" 1.2.4
check "R3 F1: the forced update failure exits 1" [ "$RC" -ne 0 ]
check "R3 F1: farhelm is restored to the OLD (1.2.3) bytes" \
  [ "$(cat "$INSTALL_R3F1/farhelm")" = "$R3_OLD_FARHELM" ]
# The sharp one: a rollback that skipped its work would leave the desktop
# binary parked under its .old name and nothing at all at this path.
check "R3 F1: farhelm-desktop still exists, at the OLD (1.2.3) bytes" \
  [ "$(cat "$INSTALL_R3F1/farhelm-desktop")" = "$R3_OLD_DESKTOP" ]
check "R3 F1: no leftover lock, journal, or backup" \
  [ -z "$(find "$INSTALL_R3F1" -maxdepth 1 -name '.farhelm*' ! -name '.farhelm-installation' 2>/dev/null || true)" ]

run_install "$MAC_TOOLS" "$HOME_R3F1" "$BASE/good-v2" 1.2.4
check "R3 F1: a following run installs 1.2.4 cleanly" [ "$RC" -eq 0 ]
check "R3 F1: the following run's farhelm reports 1.2.4" \
  [ "$("$INSTALL_R3F1/farhelm" --version)" = "farhelm 1.2.4" ]

# ===========================================================================
# Scenario: an undo step fails, and the EXIT handler replays (R3 F2a).
#
# Two things go wrong on purpose: the staged farhelm-desktop refuses to
# install (triggering rollback), and rollback's LAST step -- restoring
# .farhelm.old over farhelm -- refuses too. The explicit rollback therefore
# reports failure, install.sh exits, and its EXIT handler immediately runs
# the same rollback again over the same journal.
#
# That replay is the hazard. By the time it runs, the first pass has already
# put the OLD farhelm-desktop back; a replay that re-ran the `INSTALL
# desktop` undo would remove it, and with the backup already consumed the
# `PARK desktop` undo would shrug at its absence -- a binary destroyed by
# the machinery meant to save it. The UNDONE markers are what make the
# replay skip finished work.
# ===========================================================================
echo
echo "== R3 F2a: a failed undo step, then the EXIT handler's replay =="
HOME_R3F2A="$WORKDIR/home-r3f2a"
INSTALL_R3F2A="$HOME_R3F2A/.local/bin"
r3_seed_macos_pair "R3 F2a" "$HOME_R3F2A" "$INSTALL_R3F2A"

TOOLS_R3F2A="$WORKDIR/toolchain-r3f2a"
mkdir -p "$TOOLS_R3F2A"
cp -a "$MAC_TOOLS"/. "$TOOLS_R3F2A/"
rm -f "$TOOLS_R3F2A/mv"
cat >"$TOOLS_R3F2A/mv" <<'MVEOF'
#!/bin/sh
# Test double: fails the staged farhelm-desktop install (so a rollback
# happens at all), and also fails the farhelm restore (so that rollback
# cannot finish). Every other move -- notably the farhelm-desktop restore
# whose survival across the replay is the point -- goes through untouched.
eval "last=\${$#}"
first=$1
case "$first" in
  *.farhelm-install.*/farhelm-desktop)
    case "$last" in
      */farhelm-desktop)
        echo "fake mv: forced failure installing the staged farhelm-desktop" >&2
        exit 1
        ;;
    esac
    ;;
  */.farhelm.old)
    echo "fake mv: forced failure restoring .farhelm.old" >&2
    exit 1
    ;;
esac
exec /bin/mv "$@"
MVEOF
chmod 755 "$TOOLS_R3F2A/mv"

run_install "$TOOLS_R3F2A" "$HOME_R3F2A" "$BASE/good-v2" 1.2.4
check "R3 F2a: the run exits 1" [ "$RC" -ne 0 ]
check "R3 F2a: the explicit rollback reports it could not finish" \
  contains "$ERR" "automatic rollback could not fully complete"
check "R3 F2a: the explicit rollback names the failed replacement" \
  contains "$ERR" "install/update failed while replacing farhelm-desktop"
check "R3 F2a: the failed rollback does not claim restoration" \
  not_contains "$ERR" "was restored"
check "R3 F2a: the EXIT handler replayed and reported the same" \
  contains "$ERR" "interrupted while updating"
check "R3 F2a: the restored farhelm-desktop survives the replay byte-for-byte" \
  [ "$(cat "$INSTALL_R3F2A/farhelm-desktop")" = "$R3_OLD_DESKTOP" ]
check "R3 F2a: the un-restored farhelm is still recoverable from its backup" \
  [ "$(cat "$INSTALL_R3F2A/.farhelm.old")" = "$R3_OLD_FARHELM" ]
check "R3 F2a: the lock is left in place" [ -d "$INSTALL_R3F2A/.farhelm-install.lock" ]
check "R3 F2a: the journal is left in place" [ -e "$INSTALL_R3F2A/.farhelm-install.lock/journal" ]
# Stranded recovery state is also the only durable chance to observe the
# journal's mode: it is instructions this script later acts on, so no
# other account may append to it.
check "R3 F2a: the journal is owner-only" \
  [ "$(stat -c %a "$INSTALL_R3F2A/.farhelm-install.lock/journal")" = "600" ]

# The stranded state is not a dead end: with the sabotaged `mv` gone, the
# stale lock's own recovery path finishes the job.
run_install "$MAC_TOOLS" "$HOME_R3F2A" "$BASE/good-v2" 1.2.4
check "R3 F2a: a following run recovers rather than installing" [ "$RC" -ne 0 ]
check "R3 F2a: the following run reports the recovery" \
  contains "$ERR" "recovered from an interrupted install/update"
check "R3 F2a: farhelm is back at the OLD bytes" \
  [ "$(cat "$INSTALL_R3F2A/farhelm")" = "$R3_OLD_FARHELM" ]
check "R3 F2a: farhelm-desktop is still the OLD bytes" \
  [ "$(cat "$INSTALL_R3F2A/farhelm-desktop")" = "$R3_OLD_DESKTOP" ]
check "R3 F2a: recovery clears the lock and journal" \
  [ ! -e "$INSTALL_R3F2A/.farhelm-install.lock" ]

# ===========================================================================
# Scenario: killed immediately after one restore (R3 F2b).
#
# The same replay hazard as F2a, reached the other way: instead of a failed
# step handing the journal to this process's own EXIT handler, the process
# dies outright between two undo steps and a LATER RUN's stale-lock recovery
# picks the journal up. Crucially the crash lands after a restore that was
# performed but not yet marked done, which is the one window where a replay
# has to reason about a step it cannot observe having happened.
#
# Deviation from the spec's wording: the double kills with SIGKILL, not
# SIGTERM. SIGTERM is caught -- install.sh turns it into a plain exit and
# its EXIT handler would complete the rollback in-process, leaving a second
# run nothing to recover. Only a signal that skips trap handlers entirely
# produces the "the journal outlives the process that wrote it" state this
# scenario is about.
# ===========================================================================
echo
echo "== R3 F2b: killed mid-rollback; a second run finishes it =="
HOME_R3F2B="$WORKDIR/home-r3f2b"
INSTALL_R3F2B="$HOME_R3F2B/.local/bin"
r3_seed_macos_pair "R3 F2b" "$HOME_R3F2B" "$INSTALL_R3F2B"

TOOLS_R3F2B="$WORKDIR/toolchain-r3f2b"
mkdir -p "$TOOLS_R3F2B"
cp -a "$MAC_TOOLS"/. "$TOOLS_R3F2B/"
rm -f "$TOOLS_R3F2B/mv"
cat >"$TOOLS_R3F2B/mv" <<'MVEOF'
#!/bin/sh
# Test double: fails the staged farhelm-desktop install to trigger a
# rollback, then -- on the farhelm-desktop restore, the first undo step
# that actually moves a file back -- performs the move for real and kills
# the installer outright, before it can record that the step finished.
# $PPID is install.sh's own shell: this double is exec'd directly by it.
eval "last=\${$#}"
first=$1
case "$first" in
  *.farhelm-install.*/farhelm-desktop)
    case "$last" in
      */farhelm-desktop)
        echo "fake mv: forced failure installing the staged farhelm-desktop" >&2
        exit 1
        ;;
    esac
    ;;
  */.farhelm-desktop.old)
    /bin/mv "$@" || exit 1
    kill -KILL "$PPID"
    exit 0
    ;;
esac
exec /bin/mv "$@"
MVEOF
chmod 755 "$TOOLS_R3F2B/mv"

# The stderr redirection is on the CALL, not inside run_install: what it
# silences is this harness shell's own "Killed" notice for a foreground
# child that died on an uncatchable signal, which is expected here and
# would otherwise read as a harness error. run_install captures the
# installer's own streams into files, so nothing under test is hidden.
run_install "$TOOLS_R3F2B" "$HOME_R3F2B" "$BASE/good-v2" 1.2.4 2>/dev/null
check "R3 F2b: the killed run exits non-zero" [ "$RC" -ne 0 ]
check "R3 F2b: the killed run left its lock behind (no trap ran)" \
  [ -d "$INSTALL_R3F2B/.farhelm-install.lock" ]
check "R3 F2b: the killed run left its journal behind" \
  [ -e "$INSTALL_R3F2B/.farhelm-install.lock/journal" ]
check "R3 F2b: farhelm-desktop was restored before the kill" \
  [ "$(cat "$INSTALL_R3F2B/farhelm-desktop")" = "$R3_OLD_DESKTOP" ]

run_install "$MAC_TOOLS" "$HOME_R3F2B" "$BASE/good-v2" 1.2.4
check "R3 F2b: the second run recovers rather than installing" [ "$RC" -ne 0 ]
check "R3 F2b: the second run reports the recovery" \
  contains "$ERR" "recovered from an interrupted install/update"
check "R3 F2b: farhelm ends at the OLD bytes" \
  [ "$(cat "$INSTALL_R3F2B/farhelm")" = "$R3_OLD_FARHELM" ]
# The one a replay of an unmarked journal destroys: the second run must not
# re-undo the `INSTALL desktop` record whose undo the first run completed.
check "R3 F2b: farhelm-desktop ends at the OLD bytes, not deleted by the replay" \
  [ "$(cat "$INSTALL_R3F2B/farhelm-desktop")" = "$R3_OLD_DESKTOP" ]
check "R3 F2b: recovery clears the lock and journal" \
  [ ! -e "$INSTALL_R3F2B/.farhelm-install.lock" ]

# ===========================================================================
# Scenario: an install directory whose NAME carries the journal's own
# delimiters (R3 F3).
#
# A home directory may legally contain a pipe or a newline, so its
# ~/.local/bin installation inherits those characters. A journal that spelled moves as
# "TYPE|SRC|DEST" could not represent either: the pipe shifts fragments into
# the wrong fields and the newline manufactures extra apparent records, so
# rollback would quietly no-op and report success. Records naming binaries
# instead of paths make the directory's name irrelevant to the format.
#
# A newline-containing home remains supported. The fixed home-relative
# uninstall command avoids printing an unsafe PATH assignment.
# ===========================================================================
echo
echo "== R3 F3: an install directory containing '|' and a newline =="
printf -v HOME_R3F3 '%s/home-r3f3|pipe\nnewline\n' "$WORKDIR"
INSTALL_R3F3="$HOME_R3F3/.local/bin"
r3_seed_macos_pair "R3 F3" "$HOME_R3F3" "$INSTALL_R3F3"

run_install "$MAC_TOOLS_FAILDESKTOP" "$HOME_R3F3" "$BASE/good-v2" 1.2.4
check "R3 F3: the forced update failure exits 1" [ "$RC" -ne 0 ]
check "R3 F3: farhelm is restored byte-for-byte" \
  [ "$(cat "$INSTALL_R3F3/farhelm")" = "$R3_OLD_FARHELM" ]
check "R3 F3: farhelm-desktop is restored byte-for-byte" \
  [ "$(cat "$INSTALL_R3F3/farhelm-desktop")" = "$R3_OLD_DESKTOP" ]
check "R3 F3: no leftover lock, journal, or backup" \
  [ -z "$(find "$INSTALL_R3F3" -maxdepth 1 -name '.farhelm*' ! -name '.farhelm-installation' 2>/dev/null || true)" ]

run_install "$MAC_TOOLS" "$HOME_R3F3" "$BASE/good-v2" 1.2.4
check "R3 F3: a following run installs 1.2.4 cleanly" [ "$RC" -eq 0 ]
check "R3 F3: the following run's farhelm reports 1.2.4" \
  [ "$("$INSTALL_R3F3/farhelm" --version)" = "farhelm 1.2.4" ]
check "R3 F3: standalone metadata preserves the awkward canonical path" assert_standalone_record "$INSTALL_R3F3"
# The closing report is only reached by a run that gets that far, hence the
# assertion here rather than on the deliberately-failed run above.
# shellcheck disable=SC2088 # The approved output uses a literal home-relative command.
check "R3 F3: newline-named HOME still reports the uninstall command" \
  contains "$OUT" '~/.local/bin/farhelm uninstall'

# ===========================================================================
# Scenario: recovery state is only ever trusted where this script alone
# could have put it (R3 F4).
#
# Three faces of the same rule. A journal record outside the fixed
# vocabulary means the journal is not the one this script wrote, so nothing
# in it may be acted on. A lock directory holding anything beyond "pid" and
# "journal" is not this script's lock at all. And the pathname the journal
# USED to occupy, beside the binaries where anyone writing to the install
# directory could pre-create it, is now nothing to this script -- planting a
# symlink there must not turn an install into an append to somewhere else.
# ===========================================================================
echo
echo "== R3 F4: untrusted recovery state is refused, not obeyed =="
R3_OUTSIDE="$WORKDIR/r3f4-outside-sentinel"
echo "outside sentinel" >"$R3_OUTSIDE"

HOME_R3F4BAD="$WORKDIR/home-r3f4bad"
INSTALL_R3F4BAD="$HOME_R3F4BAD/.local/bin"
mkdir -p "$HOME_R3F4BAD"
run_install "$TOOLCHAIN_FULL" "$HOME_R3F4BAD" "$BASE/good" 1.2.3
check "R3 F4 (bad record): setup install exits 0" [ "$RC" -eq 0 ]
R3F4_FARHELM=$(cat "$INSTALL_R3F4BAD/farhelm")
printf '#!/bin/sh\necho "farhelm 0.0.1-parked"\n' >"$INSTALL_R3F4BAD/.farhelm.old"
R3F4_BACKUP=$(cat "$INSTALL_R3F4BAD/.farhelm.old")
mkdir "$INSTALL_R3F4BAD/.farhelm-install.lock"
chmod 0700 "$INSTALL_R3F4BAD/.farhelm-install.lock"
echo 999999 >"$INSTALL_R3F4BAD/.farhelm-install.lock/pid"
# A well-formed record followed by a path-bearing one in the retired
# "TYPE|SRC|DEST" spelling: the whole journal must be rejected, not
# partially replayed up to the record that offends.
{
  printf 'PARK cli\n'
  printf 'INSTALL|%s|%s\n' "$R3_OUTSIDE" "$INSTALL_R3F4BAD/farhelm"
} >"$INSTALL_R3F4BAD/.farhelm-install.lock/journal"

run_install "$TOOLCHAIN_FULL" "$HOME_R3F4BAD" "$BASE/good" 1.2.3
check "R3 F4 (bad record): the run exits 1" [ "$RC" -ne 0 ]
check "R3 F4 (bad record): the refusal names the unrecognized record" \
  contains "$ERR" "does not recognise"
check "R3 F4 (bad record): the refusal says the state is preserved" contains "$ERR" "LEFT IN PLACE"
check "R3 F4 (bad record): the lock survives" [ -d "$INSTALL_R3F4BAD/.farhelm-install.lock" ]
check "R3 F4 (bad record): the journal survives" \
  [ -e "$INSTALL_R3F4BAD/.farhelm-install.lock/journal" ]
check "R3 F4 (bad record): the installed farhelm is untouched" \
  [ "$(cat "$INSTALL_R3F4BAD/farhelm")" = "$R3F4_FARHELM" ]
check "R3 F4 (bad record): the backup is not consumed (no partial replay)" \
  [ "$(cat "$INSTALL_R3F4BAD/.farhelm.old")" = "$R3F4_BACKUP" ]
check "R3 F4 (bad record): the outside sentinel is untouched" \
  [ "$(cat "$R3_OUTSIDE")" = "outside sentinel" ]

HOME_R3F4LOCK="$WORKDIR/home-r3f4lock"
INSTALL_R3F4LOCK="$HOME_R3F4LOCK/.local/bin"
mkdir -p "$INSTALL_R3F4LOCK/.farhelm-install.lock"
echo 999999 >"$INSTALL_R3F4LOCK/.farhelm-install.lock/pid"
printf 'PARK cli\n' >"$INSTALL_R3F4LOCK/.farhelm-install.lock/journal"
echo "unrelated data" >"$INSTALL_R3F4LOCK/.farhelm-install.lock/stray"
run_install "$TOOLCHAIN_FULL" "$HOME_R3F4LOCK" "$BASE/good" 1.2.3
check "R3 F4 (extra lock entry): the run exits 1" [ "$RC" -ne 0 ]
check "R3 F4 (extra lock entry): refused as not a farhelm lock" \
  contains "$ERR" "not a farhelm install lock"
check "R3 F4 (extra lock entry): the stray file is untouched" \
  [ "$(cat "$INSTALL_R3F4LOCK/.farhelm-install.lock/stray")" = "unrelated data" ]

HOME_R3F4SYM="$WORKDIR/home-r3f4sym"
INSTALL_R3F4SYM="$HOME_R3F4SYM/.local/bin"
mkdir -p "$INSTALL_R3F4SYM"
ln -s "$R3_OUTSIDE" "$INSTALL_R3F4SYM/.farhelm-install.journal"
run_install "$TOOLCHAIN_FULL" "$HOME_R3F4SYM" "$BASE/good" 1.2.3
check "R3 F4 (retired journal path): the install still succeeds" [ "$RC" -eq 0 ]
check "R3 F4 (retired journal path): the planted symlink is neither followed nor removed" \
  [ -L "$INSTALL_R3F4SYM/.farhelm-install.journal" ]
check "R3 F4 (retired journal path): its target is byte-for-byte untouched" \
  [ "$(cat "$R3_OUTSIDE")" = "outside sentinel" ]

# ===========================================================================
# Scenario: an existing farhelm the ownership record does not vouch for is
# kept, not destroyed (triage: installer-overwrites-user-farhelm-file).
#
# Why this matters: the installer used to move any regular file named
# farhelm aside and then delete it, so a user's own wrapper script in a
# installation directory vanished while the run reported an update.
# Spec: a destination whose SHA-256 matches the executable-directory record
# is replaced with no leftover; anything else (a foreign file, a Farhelm
# from before the record existed, a recorded binary edited since) is kept
# under a visible NAME.replaced-* name and named in the closing message.
# ===========================================================================
echo
echo "== K1-K4: unrecorded existing farhelm is kept =="
kept_files() { find "$1" -maxdepth 1 -name 'farhelm.replaced-*' | sort; }

HOME_K1="$WORKDIR/home-k1"
INSTALL_K1="$HOME_K1/.local/bin"
mkdir -p "$INSTALL_K1"
printf '#!/bin/sh\necho my own wrapper\n' >"$INSTALL_K1/farhelm"
K1_OWN=$(cat "$INSTALL_K1/farhelm")
check "K1 premise: no ownership record before the install" [ ! -e "$INSTALL_K1/.farhelm-installation" ]
K1_TOOLS="$WORKDIR/toolchain-k1"
mkdir -p "$K1_TOOLS"
cp -a "$TOOLCHAIN_FULL"/. "$K1_TOOLS/"
write_fake_tmux "$K1_TOOLS" "tmux $TMUX_FLOOR"
check "K1 premise: tmux meets the floor" [ "$("$K1_TOOLS/tmux" -V)" = "tmux $TMUX_FLOOR" ]
run_install "$K1_TOOLS" "$HOME_K1" "$BASE/good" 1.2.3
check "K1 (foreign file): the install exits 0" [ "$RC" -eq 0 ]
K1_KEPT=$(kept_files "$INSTALL_K1")
check "K1 (foreign file): exactly one kept copy exists" [ "$(printf '%s\n' "$K1_KEPT" | grep -c .)" -eq 1 ]
check "K1 (foreign file): the kept copy is the user's file byte-for-byte" [ "$(cat "$K1_KEPT")" = "$K1_OWN" ]
check "K1 (foreign file): farhelm itself was replaced" [ "$(cat "$INSTALL_K1/farhelm")" != "$K1_OWN" ]
K1_EXPECTED="✅ Farhelm 1.2.3 is installed.

   Open Farhelm from Spotlight or ~/Applications.

ℹ️  ~/.local/bin/farhelm was not installed by this installer, so it was renamed
   to ~/.local/bin/${K1_KEPT##*/} - farhelm installation still
   proceeded.

   To uninstall later, run: ~/.local/bin/farhelm uninstall"
check "K1 (foreign file): exact fresh-install report and kept-file placement" [ "$OUT" = "$K1_EXPECTED" ]

HOME_K2="$WORKDIR/home-k2"
INSTALL_K2="$HOME_K2/.local/bin"
mkdir -p "$HOME_K2"
run_install "$TOOLCHAIN_FULL" "$HOME_K2" "$BASE/good" 1.2.3
check "K2 setup: the first install exits 0" [ "$RC" -eq 0 ]
check "K2 premise: the record vouches for the installed farhelm" assert_standalone_record "$INSTALL_K2"
run_install "$TOOLCHAIN_FULL" "$HOME_K2" "$BASE/good" 1.2.3
check "K2 (recorded update): the update exits 0" [ "$RC" -eq 0 ]
check "K2 (recorded update): no kept copy is left behind" [ -z "$(kept_files "$INSTALL_K2")" ]
check "K2 (recorded update): the closing message mentions no kept copy" not_contains "$OUT" "was not installed by this installer"

HOME_K3="$WORKDIR/home-k3"
INSTALL_K3="$HOME_K3/.local/bin"
mkdir -p "$HOME_K3"
run_install "$TOOLCHAIN_FULL" "$HOME_K3" "$BASE/good" 1.2.3
check "K3 setup: the first install exits 0" [ "$RC" -eq 0 ]
K3_PREVIOUS=$(cat "$INSTALL_K3/farhelm")
# A Farhelm installed before #673 has no record at all.
rm -f "$INSTALL_K3/.farhelm-installation"
run_install "$TOOLCHAIN_FULL" "$HOME_K3" "$BASE/good" 1.2.3
check "K3 (pre-record install): the update exits 0" [ "$RC" -eq 0 ]
K3_KEPT=$(kept_files "$INSTALL_K3")
check "K3 (pre-record install): the previous farhelm is kept" [ "$(cat "$K3_KEPT")" = "$K3_PREVIOUS" ]
check "K3 (pre-record install): the new record is written" assert_standalone_record "$INSTALL_K3"

HOME_K4="$WORKDIR/home-k4"
INSTALL_K4="$HOME_K4/.local/bin"
mkdir -p "$HOME_K4"
run_install "$TOOLCHAIN_FULL" "$HOME_K4" "$BASE/good" 1.2.3
check "K4 setup: the first install exits 0" [ "$RC" -eq 0 ]
printf '#!/bin/sh\necho edited since\n' >"$INSTALL_K4/farhelm"
K4_EDITED=$(cat "$INSTALL_K4/farhelm")
run_install "$TOOLCHAIN_FULL" "$HOME_K4" "$BASE/good" 1.2.3
check "K4 (digest mismatch): the update exits 0" [ "$RC" -eq 0 ]
check "K4 (digest mismatch): the edited file is kept" [ "$(cat "$(kept_files "$INSTALL_K4")")" = "$K4_EDITED" ]

# An unreadable file proves nothing about ownership, so it is kept too; it
# must not abort the install. Root reads mode-000 files anyway, so the
# premise only holds for an ordinary user.
if [ "$(id -u)" -ne 0 ]; then
  HOME_K5="$WORKDIR/home-k5"
  INSTALL_K5="$HOME_K5/.local/bin"
  mkdir -p "$INSTALL_K5"
  printf 'unreadable\n' >"$INSTALL_K5/farhelm"
  chmod 000 "$INSTALL_K5/farhelm"
  check "K5 premise: the existing farhelm cannot be read" [ ! -r "$INSTALL_K5/farhelm" ]
  run_install "$TOOLCHAIN_FULL" "$HOME_K5" "$BASE/good" 1.2.3
  check "K5 (unreadable file): the install exits 0" [ "$RC" -eq 0 ]
  K5_KEPT=$(kept_files "$INSTALL_K5")
  check "K5 (unreadable file): a kept copy exists" [ -n "$K5_KEPT" ]
  check "K5 (unreadable file): the kept copy keeps its mode" [ "$(stat -c %a "$K5_KEPT")" = 0 ]
  chmod 600 "$K5_KEPT"
  check "K5 (unreadable file): the kept copy keeps its bytes" [ "$(cat "$K5_KEPT")" = "unreadable" ]
fi

# ===========================================================================
# Scenario: the installer's download source (triage:
# installer-mirror-var-drops-https-no-signature).
#
# Why this matters: the installer used to honour FARHELM_RELEASE_BASE_URL,
# the helm's documented mirror setting, dropping its HTTPS-only pinning and
# checking no signature. An operator who exported it for the helm got an
# unauthenticated install. Spec: only the test-only
# FARHELM_INSTALL_TEST_BASE_URL redirects the installer; plain HTTP is
# accepted there only for a loopback fixture; the helm's variable has no
# effect on the installer at all.
# ===========================================================================
echo
echo "== M1-M2: installer download source =="

HOME_M1="$WORKDIR/home-m1"
INSTALL_M1="$HOME_M1/.local/bin"
run_install "$TOOLCHAIN_FULL" "$HOME_M1" "http://mirror.example.invalid/good" 1.2.3
check "M1 (non-loopback http test URL): refused with exit 1" [ "$RC" -eq 1 ]
check "M1 (non-loopback http test URL): the refusal names the variable" \
  contains "$ERR" "FARHELM_INSTALL_TEST_BASE_URL must be an https URL"
check "M1 (non-loopback http test URL): no install state is created" [ ! -e "$INSTALL_M1" ]

# A curl double records the URL it was asked for and fails, so the check
# needs no network and proves where the installer would have downloaded
# from with only the helm's variable set.
TOOLS_M2="$WORKDIR/toolchain-m2"
mkdir -p "$TOOLS_M2"
cp -a "$TOOLCHAIN_FULL"/. "$TOOLS_M2/"
rm -f "$TOOLS_M2/curl"
M2_LOG="$WORKDIR/m2-curl.log"
: >"$M2_LOG"
cat >"$TOOLS_M2/curl" <<CURLEOF
#!/bin/sh
for arg in "\$@"; do
  case "\$arg" in
    http://* | https://*) printf '%s\n' "\$arg" >>"$M2_LOG" ;;
  esac
done
exit 22
CURLEOF
chmod 755 "$TOOLS_M2/curl"
HOME_M2="$WORKDIR/home-m2"
run_install "$TOOLS_M2" "$HOME_M2" "" 1.2.3 FARHELM_RELEASE_BASE_URL="$BASE/good"
check "M2 premise: the curl double was asked for at least one URL" [ -s "$M2_LOG" ]
check "M2 (helm mirror variable set): no request goes to that mirror" not_contains "$(cat "$M2_LOG")" "$BASE"
check "M2 (helm mirror variable set): the download goes to GitHub over HTTPS" \
  contains "$(head -n 1 "$M2_LOG")" "https://github.com/"
check "M2 (helm mirror variable set): the installer does not mention it" not_contains "$ERR" "FARHELM_RELEASE_BASE_URL"

# A loopback test URL that redirects to another host must still only talk
# to this machine: the redirect names remote.invalid, which cannot resolve,
# so the install only succeeds if every connection was pinned to loopback.
HOME_M3="$WORKDIR/home-m3"
mkdir -p "$HOME_M3"
M3_BEFORE=$(grep -c 'redirect-real' "$SERVER_LOG" || true)
run_install "$TOOLCHAIN_FULL" "$HOME_M3" "$BASE/redirect-remote" 1.2.3
check "M3 (loopback URL redirecting to another host): install exits 0" [ "$RC" -eq 0 ]
check "M3 (loopback URL redirecting to another host): the redirected requests reached this machine" \
  [ "$(grep -c 'redirect-real' "$SERVER_LOG")" -gt "$M3_BEFORE" ]

# ===========================================================================
# Scenario: the app bundle's Info.plist mode does not follow the caller's
# umask (triage: app-info-plist-uses-caller-umask).
#
# Why this matters: Info.plist's LSEnvironment can inject code into the app,
# and under a permissive umask such as 002 the heredoc that writes it left it
# group-writable, where every other local account in `staff` could edit it.
# Spec: the plist is 0644 whatever the umask, like the rest of the bundle.
# ===========================================================================
echo
echo "== P1: Info.plist mode under umask 002 =="
HOME_P1="$WORKDIR/home-p1"
mkdir -p "$HOME_P1"
P1_OLD_UMASK=$(umask)
umask 002
check "P1 premise: the installer runs under umask 002" [ "$(umask)" = 0002 ]
run_install "$MAC_TOOLS" "$HOME_P1" "$BASE/good" 1.2.3
umask "$P1_OLD_UMASK"
check "P1: macOS-shaped install exits 0" [ "$RC" -eq 0 ]
check "P1: Info.plist is 0644 despite the umask" \
  [ "$(stat -c %a "$HOME_P1/Applications/Farhelm.app/Contents/Info.plist")" = 644 ]

# ===========================================================================
echo
echo "== summary =="
echo "$CHECKS checks, $FAILURES failed"
if [ "$FAILURES" -ne 0 ]; then
  exit 1
fi
