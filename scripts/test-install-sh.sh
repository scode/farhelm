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

# build_archive OUT_TAR PACKAGE TARGET CONTENT_LINE [no-icns] [no-marker]
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
#
# A farhelm-desktop program also carries, as a comment line, the text every
# desktop build since the side-by-side version layout contains, which
# install.sh checks for before installing anything; pass "no-marker" as the
# sixth argument to build a release from before that layout.
build_archive() {
  local out=$1 package=$2 target=$3 content=$4 icns=${5:-icns} marker=${6:-marker}
  local binary=farhelm
  [ "$package" = farhelm-desktop ] && binary=farhelm-desktop
  local stage
  stage=$(mktemp -d "$WORKDIR/archive-stage.XXXXXX")
  write_binary "$stage/$package-$target" "$binary" "$content"
  if [ "$package" = farhelm-desktop ] && [ "$marker" != no-marker ]; then
    printf '# farhelm-desktop needs its own version of the farhelm binary at its folder\n' \
      >>"$stage/$package-$target/$binary"
  fi
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
BASE_TOOLS=(uname mkdir mktemp rmdir ls cp curl tar gzip sha256sum shasum openssl awk sed grep tr head cut mv rm chmod cat sysctl sleep find ln date readlink touch)

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
build_good_release "$WWW/good-v3" 1.2.5
build_good_release "$WWW/good-v4" 1.2.6
# A release from before the side-by-side version layout: a valid release in
# every other respect, without the desktop marker.
build_good_release "$WWW/prelayout" 1.2.4
build_archive "$WWW/prelayout/farhelm-desktop-aarch64-apple-darwin.tar.gz" \
  farhelm-desktop aarch64-apple-darwin "farhelm-desktop 1.2.4" icns no-marker
(cd "$WWW/prelayout" && sha256sum -- *.tar.gz >SHA256SUMS)

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
# Helpers for the side-by-side version layout (SPEC_impl.md, "Side-by-side
# versions inside Farhelm.app"): the app bundle is the whole installation,
# ~/.local/bin/farhelm only a symlink to its forwarder.
# ===========================================================================
MAC_TOOLS="$TOOLCHAIN_FULL"

# assert_current_record HOME -- the app's ownership record is exactly
# "farhelm-app-v2", NUL, the canonical path of HOME's Terminal link, NUL.
# Python computes the expected bytes independently of the installer's shell.
assert_current_record() {
  python3 - "$1" <<'PY'
import os
import sys

home = sys.argv[1]
link = os.fsencode(os.path.realpath(os.path.join(home, ".local", "bin"))) + b"/farhelm"
expected = b"farhelm-app-v2\0" + link + b"\0"
record = os.path.join(home, "Applications", "Farhelm.app", "Contents", ".farhelm-installation")
with open(record, "rb") as source:
    raise SystemExit(0 if source.read() == expected else 1)
PY
}

# forward ARGS... -- run a program (normally a forwarder) under an empty,
# explicit environment, so nothing of this harness's own environment (a
# FARHELM_SUPERVISOR_SOCK from a session this suite might run in, say)
# reaches it. Extra VAR=VALUE assignments go before the program.
forward() {
  env -i PATH=/usr/bin:/bin "$@"
}

# app_launchable APP -- the app as macOS and Farhelm would start it works:
# Info.plist names the main program, which exists; the version folder that
# program starts its supervisor from (its compiled version, which the
# fixture prints) exists; and the Installed record names a version whose
# farhelm the forwarder runs.
app_launchable() {
  local app=$1 desktop_version installed
  [ -x "$app/Contents/MacOS/farhelm-desktop" ] || return 1
  grep -q '<string>farhelm-desktop</string>' "$app/Contents/Info.plist" || return 1
  desktop_version=$("$app/Contents/MacOS/farhelm-desktop") || return 1
  desktop_version=${desktop_version#farhelm-desktop }
  [ -x "$app/Contents/Versions/$desktop_version/farhelm" ] || return 1
  IFS= read -r installed <"$app/Contents/Versions/installed" || return 1
  [ -x "$app/Contents/Versions/$installed/farhelm" ] || return 1
  [ "$(forward "$app/Contents/MacOS/farhelm" --version)" = "farhelm $installed" ]
}

# versions_of APP -- the version folders in APP, sorted, space-separated.
versions_of() {
  local entry names=()
  for entry in "$1/Contents/Versions"/*/; do
    [ -d "$entry" ] || continue
    entry=${entry%/}
    names+=("${entry##*/}")
  done
  printf '%s\n' "${names[@]}" | sort | paste -sd' ' -
}

# seed_old_layout HOME VERSION [legacy] -- the installation the layout
# before this one produced: farhelm and farhelm-desktop copied into
# ~/.local/bin with a NUL-separated record of their checksums, and
# Farhelm.app holding copies of both in Contents/MacOS with its own record
# (or, with "legacy", the recordless bundle installers built before records
# existed). Written directly from the documented formats, since the
# installer that produced it is gone.
seed_old_layout() {
  python3 - "$1" "$2" "${3:-recorded}" <<'PY'
import hashlib
import os
import sys

home, version, kind = sys.argv[1:4]
bin_dir = os.path.join(home, ".local", "bin")
os.makedirs(bin_dir, exist_ok=True)
app = os.path.join(home, "Applications", "Farhelm.app")
contents = os.path.join(app, "Contents")
for sub in ("MacOS", "Resources"):
    os.makedirs(os.path.join(contents, sub), exist_ok=True)

def write(path, data, mode):
    with open(path, "wb") as target:
        target.write(data)
    os.chmod(path, mode)

def digest(path):
    with open(path, "rb") as source:
        return hashlib.sha256(source.read()).hexdigest().encode()

cli = f'#!/bin/sh\necho "farhelm {version}"\n'.encode()
desktop = f'#!/bin/sh\necho "farhelm-desktop {version}"\n'.encode()
for directory in (bin_dir, os.path.join(contents, "MacOS")):
    write(os.path.join(directory, "farhelm"), cli, 0o755)
    write(os.path.join(directory, "farhelm-desktop"), desktop, 0o755)
canonical = os.fsencode(os.path.realpath(bin_dir))
write(
    os.path.join(bin_dir, ".farhelm-installation"),
    b"\0".join([b"farhelm-standalone", canonical,
                digest(os.path.join(bin_dir, "farhelm")),
                digest(os.path.join(bin_dir, "farhelm-desktop"))]) + b"\0",
    0o600,
)
plist = os.path.join(contents, "Info.plist")
write(plist, (
    '<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0">\n<dict>\n'
    '\t<key>CFBundleIdentifier</key>\n\t<string>org.scode.farhelm.desktop</string>\n'
    f'\t<key>CFBundleShortVersionString</key>\n\t<string>{version}</string>\n'
    '</dict>\n</plist>\n').encode(), 0o644)
icns = os.path.join(contents, "Resources", "Farhelm.icns")
write(icns, f"fake icns: farhelm-desktop {version}\n".encode(), 0o644)
if kind != "legacy":
    write(
        os.path.join(contents, ".farhelm-installation"),
        b"\0".join([b"farhelm-app", canonical,
                    digest(os.path.join(contents, "MacOS", "farhelm")),
                    digest(os.path.join(contents, "MacOS", "farhelm-desktop")),
                    digest(plist), digest(icns)]) + b"\0",
        0o600,
    )
PY
}

# ===========================================================================
# Scenario: fresh install builds the side-by-side layout. Why: the bundle is
# the whole installation now, and every later update and every running
# session depends on its exact shape (the forwarder's path above all, which
# sessions started by earlier desktop supervisors already hold).
# ===========================================================================
echo
echo "== fresh install: the side-by-side layout =="
HOME1="$WORKDIR/home1"
INSTALL1="$HOME1/.local/bin"
APP1="$HOME1/Applications/Farhelm.app"
mkdir -p "$HOME1"
run_install "$MAC_TOOLS" "$HOME1" "$BASE/good" 1.2.3
check "fresh install exits 0" [ "$RC" -eq 0 ]
check "fresh install reports Installed" contains "$OUT" "Farhelm 1.2.3 is installed."
check "fresh install: the version folder holds farhelm" [ -x "$APP1/Contents/Versions/1.2.3/farhelm" ]
check "fresh install: the versioned farhelm is the release's" \
  [ "$("$APP1/Contents/Versions/1.2.3/farhelm" --version)" = "farhelm 1.2.3" ]
check "fresh install: the versioned farhelm is mode 0755" [ "$(stat -c %a "$APP1/Contents/Versions/1.2.3/farhelm")" = 755 ]
check "fresh install: the Installed record names the version" [ "$(cat "$APP1/Contents/Versions/installed")" = 1.2.3 ]
check "fresh install: only the installed version is kept" [ "$(versions_of "$APP1")" = 1.2.3 ]
check "fresh install: the forwarder is a shell script at Contents/MacOS/farhelm" \
  [ "$(head -n 1 "$APP1/Contents/MacOS/farhelm")" = "#!/bin/sh" ]
check "fresh install: the forwarder is mode 0755" [ "$(stat -c %a "$APP1/Contents/MacOS/farhelm")" = 755 ]
check "fresh install: the main program is the release's farhelm-desktop" \
  [ "$("$APP1/Contents/MacOS/farhelm-desktop")" = "farhelm-desktop 1.2.3" ]
check "fresh install: Info.plist names the stable bundle identifier" \
  contains "$(cat "$APP1/Contents/Info.plist")" "<string>org.scode.farhelm.desktop</string>"
check "fresh install: Info.plist points CFBundleExecutable at farhelm-desktop" \
  contains "$(cat "$APP1/Contents/Info.plist")" "<key>CFBundleExecutable</key>
	<string>farhelm-desktop</string>"
check "fresh install: Info.plist carries the version" contains "$(cat "$APP1/Contents/Info.plist")" "<string>1.2.3</string>"
check "fresh install: the icon is the archive's" \
  [ "$(cat "$APP1/Contents/Resources/Farhelm.icns")" = "fake icns: farhelm-desktop 1.2.3" ]
check "fresh install: the ownership record names the Terminal link" assert_current_record "$HOME1"
check "fresh install: the ownership record is owner-only" [ "$(stat -c %a "$APP1/Contents/.farhelm-installation")" = 600 ]
check "fresh install: ~/.local/bin/farhelm is a symlink to the forwarder" \
  [ "$(readlink "$INSTALL1/farhelm")" = "$APP1/Contents/MacOS/farhelm" ]
check "fresh install: farhelm from the Terminal reaches the installed version" \
  [ "$(forward "$INSTALL1/farhelm" --version)" = "farhelm 1.2.3" ]
check "fresh install: no farhelm-desktop in the bin directory" [ ! -e "$INSTALL1/farhelm-desktop" ]
check "fresh install: no bin-directory record" [ ! -e "$INSTALL1/.farhelm-installation" ]
check "fresh install: nothing of the run is left in the bin directory" \
  [ -z "$(find "$INSTALL1" -mindepth 1 -maxdepth 1 -name '.*')" ]
check "fresh install: nothing of the run is left beside the app" \
  [ -z "$(find "$HOME1/Applications" -mindepth 1 -maxdepth 1 -name '.*')" ]
check "fresh install: the app is launchable" app_launchable "$APP1"

# ===========================================================================
# Scenario: re-running the same release changes nothing that matters. Why:
# a reinstall must be an ordinary update, never a rebuild that would drop a
# version a running Farhelm is using.
# ===========================================================================
echo
echo "== update: the same release again =="
FORWARDER_INODE=$(stat -c %i "$APP1/Contents/MacOS/farhelm")
VERSION_DIR_INODE=$(stat -c %i "$APP1/Contents/Versions/1.2.3")
run_install "$MAC_TOOLS" "$HOME1" "$BASE/good" 1.2.3
check "same-release update exits 0" [ "$RC" -eq 0 ]
check "same-release update reports ready" contains "$OUT" "Farhelm 1.2.3 is ready."
check "same-release update gives restart advice" contains "$OUT" "Quit and reopen Farhelm to finish updating."
check "same-release update keeps the one version" [ "$(versions_of "$APP1")" = 1.2.3 ]
check "same-release update keeps the existing version folder itself" \
  [ "$(stat -c %i "$APP1/Contents/Versions/1.2.3")" = "$VERSION_DIR_INODE" ]
check "same-release update leaves an unchanged forwarder file alone" \
  [ "$(stat -c %i "$APP1/Contents/MacOS/farhelm")" = "$FORWARDER_INODE" ]
check "same-release update leaves the app launchable" app_launchable "$APP1"
check "same-release update leaves nothing of the run beside the app" \
  [ -z "$(find "$HOME1/Applications" -mindepth 1 -maxdepth 1 -name '.*')" ]

# ===========================================================================
# Scenario: updating to a new release while an older Farhelm runs. Why: this
# is the point of the layout; the running version's folder must survive, the
# new version must be what starts next, and a session of the running
# Farhelm must keep reaching the running version.
# ===========================================================================
echo
echo "== update: a new release, side by side =="
APP1_DESKTOP_INODE=$(stat -c %i "$APP1/Contents/MacOS/farhelm-desktop")
APP1_INODES="$(stat -c %i "$APP1") $(stat -c %i "$APP1/Contents")"
run_install "$MAC_TOOLS" "$HOME1" "$BASE/good-v2" 1.2.4
check "new-release update exits 0" [ "$RC" -eq 0 ]
check "new-release update reports ready" contains "$OUT" "Farhelm 1.2.4 is ready."
check "new-release update keeps the old version beside the new" [ "$(versions_of "$APP1")" = "1.2.3 1.2.4" ]
check "new-release update: the old version folder is untouched" \
  [ "$("$APP1/Contents/Versions/1.2.3/farhelm" --version)" = "farhelm 1.2.3" ]
check "new-release update: Installed names the new version" [ "$(cat "$APP1/Contents/Versions/installed")" = 1.2.4 ]
check "new-release update: the main program is the new one" \
  [ "$("$APP1/Contents/MacOS/farhelm-desktop")" = "farhelm-desktop 1.2.4" ]
check "new-release update: the app folder itself is never replaced" \
  [ "$(stat -c %i "$APP1") $(stat -c %i "$APP1/Contents")" = "$APP1_INODES" ]
check "new-release update: the main program was renamed over, not rewritten" \
  [ "$(stat -c %i "$APP1/Contents/MacOS/farhelm-desktop")" != "$APP1_DESKTOP_INODE" ]
check "new-release update: Info.plist names the new version" contains "$(cat "$APP1/Contents/Info.plist")" "<string>1.2.4</string>"
check "new-release update: the icon is the new release's" \
  [ "$(cat "$APP1/Contents/Resources/Farhelm.icns")" = "fake icns: farhelm-desktop 1.2.4" ]
check "new-release update: the Terminal reaches the new version" \
  [ "$(forward "$INSTALL1/farhelm" --version)" = "farhelm 1.2.4" ]
SESSION_STATE="$WORKDIR/session-state-1"
mkdir -p "$SESSION_STATE"
printf '1.2.3\n' >"$SESSION_STATE/running-version"
check "new-release update: a session of the running 1.2.3 still reaches 1.2.3" \
  [ "$(forward FARHELM_SUPERVISOR_SOCK="$SESSION_STATE/supervisor.sock" "$INSTALL1/farhelm" --version)" = "farhelm 1.2.3" ]
check "new-release update leaves the app launchable" app_launchable "$APP1"
check "new-release update leaves nothing of the run beside the app" \
  [ -z "$(find "$HOME1/Applications" -mindepth 1 -maxdepth 1 -name '.*')" ]

# ===========================================================================
# Scenario: which old versions an update removes. Why: versions pile up
# otherwise, but removing the one a running Farhelm started from would break
# its sessions and its own later launches. Kept: the new version, the one it
# replaced, and the one named by the Running record in the default state
# directory (honoring an absolute XDG_STATE_HOME); anything not named like a
# version is not the installer's and stays.
# ===========================================================================
echo
echo "== update: keeping the running, replaced and new versions =="
mkdir -p "$HOME1/.local/state/farhelm"
printf '1.2.3\n' >"$HOME1/.local/state/farhelm/running-version"
mkdir -p "$APP1/Contents/Versions/notes"
printf 'mine\n' >"$APP1/Contents/Versions/notes/readme"
run_install "$MAC_TOOLS" "$HOME1" "$BASE/good-v3" 1.2.5
check "cleanup (running 1.2.3): update exits 0" [ "$RC" -eq 0 ]
check "cleanup (running 1.2.3): running, replaced and new versions are kept" \
  [ "$(versions_of "$APP1")" = "1.2.3 1.2.4 1.2.5 notes" ]
check "cleanup: a folder not named like a version is untouched" [ "$(cat "$APP1/Contents/Versions/notes/readme")" = mine ]
rm -f "$HOME1/.local/state/farhelm/running-version"
run_install "$MAC_TOOLS" "$HOME1" "$BASE/good-v4" 1.2.6
check "cleanup (nothing running): update exits 0" [ "$RC" -eq 0 ]
check "cleanup (nothing running): only the replaced and new versions remain" \
  [ "$(versions_of "$APP1")" = "1.2.5 1.2.6 notes" ]
XDG_HOME_STATE="$WORKDIR/xdg-state-1"
mkdir -p "$XDG_HOME_STATE/farhelm"
printf '1.2.5\n' >"$XDG_HOME_STATE/farhelm/running-version"
run_install "$MAC_TOOLS" "$HOME1" "$BASE/good-v2" 1.2.4 XDG_STATE_HOME="$XDG_HOME_STATE"
check "cleanup (XDG_STATE_HOME running 1.2.5): update exits 0" [ "$RC" -eq 0 ]
check "cleanup (XDG_STATE_HOME running 1.2.5): the running version is kept" \
  [ "$(versions_of "$APP1")" = "1.2.4 1.2.5 1.2.6 notes" ]
# A Farhelm opened from Finder uses the default state directory even when
# the shell running the installer names another one; its version must
# survive updates from that shell (two, so that "just replaced" alone
# cannot be what keeps it).
mkdir -p "$HOME1/.local/state/farhelm"
printf '1.2.4\n' >"$HOME1/.local/state/farhelm/running-version"
rm -f "$XDG_HOME_STATE/farhelm/running-version"
run_install "$MAC_TOOLS" "$HOME1" "$BASE/good-v3" 1.2.5 XDG_STATE_HOME="$XDG_HOME_STATE"
run_install "$MAC_TOOLS" "$HOME1" "$BASE/good-v4" 1.2.6 XDG_STATE_HOME="$XDG_HOME_STATE"
check "cleanup (default-dir Running under another XDG_STATE_HOME): updates exit 0" [ "$RC" -eq 0 ]
check "cleanup (default-dir Running under another XDG_STATE_HOME): the running version is kept" \
  [ "$(versions_of "$APP1")" = "1.2.4 1.2.5 1.2.6 notes" ]
check "cleanup leaves the app launchable" app_launchable "$APP1"

# Only names exactly like the folders the installer writes are pruned: not
# one with a leading "v", and not one whose name holds a version on one of
# its lines (grep matches line by line).
HOME_ODD="$WORKDIR/home-odd-versions"
mkdir -p "$HOME_ODD"
run_install "$MAC_TOOLS" "$HOME_ODD" "$BASE/good" 1.2.3
ODD_VERSIONS="$HOME_ODD/Applications/Farhelm.app/Contents/Versions"
mkdir -p "$ODD_VERSIONS/v1.2.3" "$ODD_VERSIONS/notes
1.2.3"
printf 'mine\n' >"$ODD_VERSIONS/v1.2.3/readme"
printf 'mine\n' >"$ODD_VERSIONS/notes
1.2.3/readme"
run_install "$MAC_TOOLS" "$HOME_ODD" "$BASE/good-v2" 1.2.4
run_install "$MAC_TOOLS" "$HOME_ODD" "$BASE/good-v3" 1.2.5
check "cleanup (odd names): the updates exit 0" [ "$RC" -eq 0 ]
check "cleanup (odd names): a v-prefixed folder is untouched" [ "$(cat "$ODD_VERSIONS/v1.2.3/readme")" = mine ]
check "cleanup (odd names): a folder with a version on one line of its name is untouched" \
  [ "$(cat "$ODD_VERSIONS/notes
1.2.3/readme")" = mine ]

# ===========================================================================
# Scenario: an update stopped at any step leaves a working app. Why: an
# update can be killed at any instant (SIGKILL, power loss), and the step
# order is what guarantees the app then launches the old version or the new
# one. An `mv` double performs each real rename and then SIGKILLs the
# installer right after the one named by KILL_AFTER, so every intermediate
# state the order allows is inspected. A rerun after removing the lock the
# killed run left (which the next run's refusal asks for) finishes the job.
# ===========================================================================
echo
echo "== update: interrupted after each step =="
INTERRUPT_TOOLS="$WORKDIR/toolchain-interrupt"
mkdir -p "$INTERRUPT_TOOLS"
cp -a "$MAC_TOOLS"/. "$INTERRUPT_TOOLS/"
rm -f "$INTERRUPT_TOOLS/mv"
cat >"$INTERRUPT_TOOLS/mv" <<'MVEOF'
#!/bin/sh
# Test double: the real mv, then a SIGKILL of the installer (this process's
# parent shell) right after the rename whose destination ends in KILL_AFTER.
eval "last=\${$#}"
/bin/mv "$@" || exit
case "$last" in
  *"$KILL_AFTER") kill -9 "$PPID" ;;
esac
MVEOF
chmod 755 "$INTERRUPT_TOOLS/mv"
# What each record must still say right after the kill, which is what pins
# the order: the Installed record and Info.plist switch only after the new
# version's folder and main program are in place, and Info.plist last.
interrupt_expect() {
  case "$1" in
    Versions/1.2.4 | MacOS/farhelm-desktop | Resources/Farhelm.icns) echo "1.2.3 1.2.3" ;;
    Versions/installed) echo "1.2.4 1.2.3" ;;
    *) echo "1.2.4 1.2.4" ;;
  esac
}
plist_version() {
  sed -n '/CFBundleVersion/{n;s/.*<string>\(.*\)<\/string>.*/\1/p;}' "$1/Contents/Info.plist"
}
for step in Versions/1.2.4 MacOS/farhelm-desktop Resources/Farhelm.icns Versions/installed Contents/Info.plist Contents/.farhelm-installation; do
  label=${step//\//-}
  home="$WORKDIR/home-interrupt-$label"
  app="$home/Applications/Farhelm.app"
  mkdir -p "$home"
  run_install "$MAC_TOOLS" "$home" "$BASE/good" 1.2.3
  check "interrupt after $step: setup install exits 0" [ "$RC" -eq 0 ]
  run_install "$INTERRUPT_TOOLS" "$home" "$BASE/good-v2" 1.2.4 KILL_AFTER="$step"
  check "interrupt after $step: the installer was killed" [ "$RC" -eq 137 ]
  check "interrupt after $step: the app is launchable" app_launchable "$app"
  check "interrupt after $step: the Installed record and Info.plist are at the expected step" \
    [ "$(cat "$app/Contents/Versions/installed") $(plist_version "$app")" = "$(interrupt_expect "$step")" ]
  check "interrupt after $step: the Terminal link still works" \
    contains "$(forward "$home/.local/bin/farhelm" --version)" "farhelm 1.2."
  check "interrupt after $step: the lock is left behind" [ -d "$home/Applications/.farhelm-app.lock" ]
  run_install "$MAC_TOOLS" "$home" "$BASE/good-v2" 1.2.4
  check "interrupt after $step: a rerun refuses on the left-behind lock" \
    contains "$ERR" "$home/Applications/.farhelm-app.lock"
  rmdir "$home/Applications/.farhelm-app.lock"
  run_install "$MAC_TOOLS" "$home" "$BASE/good-v2" 1.2.4
  check "interrupt after $step: a rerun finishes the update" [ "$RC" -eq 0 ]
  check "interrupt after $step: the finished update installed 1.2.4" \
    [ "$(cat "$app/Contents/Versions/installed")" = 1.2.4 ]
  check "interrupt after $step: the finished update leaves the app launchable" app_launchable "$app"
done

# ===========================================================================
# Scenario: an update repairs what an interrupted uninstall or a killed
# installer left. Why: an uninstall stopped after removing the icon's
# folder, or an installer killed between copying a file beside its
# destination and renaming it, must not leave an app that no later update
# can fix and that uninstall refuses as foreign.
# ===========================================================================
echo
echo "== update: repairing leftovers =="
HOME_REPAIR="$WORKDIR/home-repair"
mkdir -p "$HOME_REPAIR"
run_install "$MAC_TOOLS" "$HOME_REPAIR" "$BASE/good" 1.2.3
REPAIR_APP="$HOME_REPAIR/Applications/Farhelm.app"
rm -rf "$REPAIR_APP/Contents/Resources" "$REPAIR_APP/Contents/MacOS/farhelm-desktop"
printf 'half copied\n' >"$REPAIR_APP/Contents/MacOS/.farhelm-new.4242.farhelm-desktop"
printf 'half copied\n' >"$REPAIR_APP/Contents/.farhelm-new.4242.Info.plist"
ln -s /nonexistent "$REPAIR_APP/Contents/MacOS/.farhelm-new.4243.link"
rm -f "$REPAIR_APP/Contents/Info.plist"
mkdir -p "$REPAIR_APP/Contents/Versions/1.2.4"
run_install "$MAC_TOOLS" "$HOME_REPAIR" "$BASE/good-v2" 1.2.4
check "repair: the update exits 0" [ "$RC" -eq 0 ]
check "repair: the app is launchable again" app_launchable "$REPAIR_APP"
check "repair: the icon is back" [ -f "$REPAIR_APP/Contents/Resources/Farhelm.icns" ]
check "repair: a killed run's half-copied files are gone" \
  [ -z "$(find "$REPAIR_APP" -name '.farhelm-new.4242.*')" ]
check "repair: a symlink with a staging-like name is not the installer's and stays" \
  [ -L "$REPAIR_APP/Contents/MacOS/.farhelm-new.4243.link" ]
check "repair: a version folder without its farhelm is completed" \
  [ "$(forward "$REPAIR_APP/Contents/Versions/1.2.4/farhelm" --version)" = "farhelm 1.2.4" ]
rm -f "$REPAIR_APP/Contents/MacOS/.farhelm-new.4243.link"

# ===========================================================================
# Scenario: the forwarder's contract. Why: every session Farhelm started
# holds the forwarder's path in its hook command lines, reporter variables
# and PATH, so what it runs, and that it passes everything through unchanged
# and replaces itself with the program, is relied on by sessions that
# outlive any one version (SPEC_impl.md, "What running sessions hold across
# versions"). A synthetic bundle holds the installed forwarder and versions
# whose farhelm reports its arguments, environment, stdin, pid and exit.
# ===========================================================================
echo
echo "== the forwarder =="
FWD_APP="$WORKDIR/forwarder/Farhelm.app"
mkdir -p "$FWD_APP/Contents/MacOS"
cp "$APP1/Contents/MacOS/farhelm" "$FWD_APP/Contents/MacOS/farhelm"
for v in 7.0.0 8.0.0; do
  mkdir -p "$FWD_APP/Contents/Versions/$v"
  cat >"$FWD_APP/Contents/Versions/$v/farhelm" <<PROBEEOF
#!/bin/sh
echo "version $v"
echo "pid \$\$"
for arg in "\$@"; do printf 'arg [%s]\n' "\$arg"; done
printf 'probe [%s]\n' "\${FORWARDER_PROBE-unset}"
env | grep -c '^farhelm_forwarder_' || true
if [ "\${1:-}" = stdin ]; then cat; fi
exit "\${PROBE_EXIT:-0}"
PROBEEOF
  chmod 755 "$FWD_APP/Contents/Versions/$v/farhelm"
done
printf '7.0.0\n' >"$FWD_APP/Contents/Versions/installed"
FWD="$FWD_APP/Contents/MacOS/farhelm"
FWD_STATE="$WORKDIR/forwarder/state"
mkdir -p "$FWD_STATE"

check "forwarder (no session): runs the Installed version" contains "$(forward "$FWD")" "version 7.0.0"
check "forwarder (session, no Running record): runs the Installed version" \
  contains "$(forward FARHELM_SUPERVISOR_SOCK="$FWD_STATE/supervisor.sock" "$FWD")" "version 7.0.0"
printf '8.0.0\n' >"$FWD_STATE/running-version"
check "forwarder (session with a Running record): runs the running version" \
  contains "$(forward FARHELM_SUPERVISOR_SOCK="$FWD_STATE/supervisor.sock" "$FWD")" "version 8.0.0"
# shellcheck disable=SC2016 # the literal $HOME is the point: it must reach the program unexpanded
FWD_ARGS=$(forward "$FWD" 'two words' "it's" '' '--flag=a "b"' '$HOME' '*')
check "forwarder: arguments pass through unchanged" [ "$(printf '%s\n' "$FWD_ARGS" | grep '^arg ')" = "arg [two words]
arg [it's]
arg []
arg [--flag=a \"b\"]
arg [\$HOME]
arg [*]" ]
check "forwarder: the environment passes through" \
  contains "$(forward FORWARDER_PROBE=kept "$FWD")" "probe [kept]"
check "forwarder: an unset variable stays unset" contains "$(forward "$FWD")" "probe [unset]"
check "forwarder: none of its own variables reach the program" \
  [ "$(forward "$FWD" | tail -n 1)" = 0 ]
check "forwarder: stdin passes through" [ "$(printf 'piped\n' | forward "$FWD" stdin | tail -n 1)" = piped ]
set +e
forward PROBE_EXIT=7 "$FWD" >/dev/null
FWD_RC=$?
set -e
check "forwarder: the program's exit status is the forwarder's" [ "$FWD_RC" -eq 7 ]
# Started directly, not through forward(): a backgrounded shell function is
# its own process, whose pid would not be the forwarder's.
env -i PATH=/usr/bin:/bin "$FWD" >"$WORKDIR/forwarder/pid.out" &
FWD_PID=$!
wait "$FWD_PID"
check "forwarder: replaces itself with the program (exec, same pid)" \
  [ "$(sed -n 's/^pid //p' "$WORKDIR/forwarder/pid.out")" = "$FWD_PID" ]
mkdir -p "$WORKDIR/forwarder/bin" "$WORKDIR/forwarder/links"
ln -s "$FWD" "$WORKDIR/forwarder/links/farhelm"
ln -s ../links/farhelm "$WORKDIR/forwarder/bin/farhelm"
check "forwarder: reached through a chain of symlinks, absolute and relative" \
  contains "$(forward "$WORKDIR/forwarder/bin/farhelm")" "version 7.0.0"
check "forwarder: reached by name on PATH" \
  contains "$(env -i PATH="$WORKDIR/forwarder/bin:/usr/bin:/bin" farhelm)" "version 7.0.0"
printf '9.9.9\n' >"$FWD_STATE/running-version"
set +e
FWD_MISSING_ERR=$(forward FARHELM_SUPERVISOR_SOCK="$FWD_STATE/supervisor.sock" "$FWD" 2>&1 >/dev/null)
FWD_RC=$?
set -e
check "forwarder (Running names a missing version): exits non-zero" [ "$FWD_RC" -ne 0 ]
check "forwarder (Running names a missing version): says so in one line" \
  [ "$FWD_MISSING_ERR" = "farhelm: version 9.9.9 is not installed in $(cd -P "$FWD_APP/Contents/Versions" && pwd -P); reinstall Farhelm" ]
printf '../7.0.0\n' >"$FWD_STATE/running-version"
set +e
forward FARHELM_SUPERVISOR_SOCK="$FWD_STATE/supervisor.sock" "$FWD" >/dev/null 2>&1
FWD_RC=$?
set -e
check "forwarder: a version naming another path is refused" [ "$FWD_RC" -ne 0 ]
if command -v shellcheck >/dev/null 2>&1; then
  check "forwarder: the installed script passes shellcheck" shellcheck -s sh "$FWD"
fi

# ===========================================================================
# Scenario: the one-time move from the layout that copied both binaries
# into ~/.local/bin and the bundle. Why: that is every existing
# installation; its copies the installer owns must go, the user's own files
# must not, and the bundle must become the new layout.
# ===========================================================================
echo
echo "== moving from the old layout =="
HOME_OLD="$WORKDIR/home-old-layout"
seed_old_layout "$HOME_OLD" 1.2.3
run_install "$MAC_TOOLS" "$HOME_OLD" "$BASE/good-v2" 1.2.4
check "old layout: the update exits 0" [ "$RC" -eq 0 ]
check "old layout: reported as an update" contains "$OUT" "Farhelm 1.2.4 is ready."
check "old layout: the bundle is the new layout" [ "$(versions_of "$HOME_OLD/Applications/Farhelm.app")" = 1.2.4 ]
check "old layout: the bundle's record is the new one" assert_current_record "$HOME_OLD"
check "old layout: farhelm is now the Terminal link" \
  [ "$(readlink "$HOME_OLD/.local/bin/farhelm")" = "$HOME_OLD/Applications/Farhelm.app/Contents/MacOS/farhelm" ]
check "old layout: the old farhelm-desktop copy is gone" [ ! -e "$HOME_OLD/.local/bin/farhelm-desktop" ]
check "old layout: the old bin-directory record is gone" [ ! -e "$HOME_OLD/.local/bin/.farhelm-installation" ]
check "old layout: no kept copies (they were the installer's)" \
  [ -z "$(find "$HOME_OLD/.local/bin" -name '*.replaced-*')" ]
check "old layout: the app is launchable" app_launchable "$HOME_OLD/Applications/Farhelm.app"

HOME_LEGACY="$WORKDIR/home-legacy-bundle"
seed_old_layout "$HOME_LEGACY" 1.2.3 legacy
run_install "$MAC_TOOLS" "$HOME_LEGACY" "$BASE/good-v2" 1.2.4
check "recordless legacy bundle: the update exits 0" [ "$RC" -eq 0 ]
check "recordless legacy bundle: replaced by the new layout" assert_current_record "$HOME_LEGACY"

HOME_LEGACY_EXTRA="$WORKDIR/home-legacy-extra"
seed_old_layout "$HOME_LEGACY_EXTRA" 1.2.3 legacy
printf 'mine\n' >"$HOME_LEGACY_EXTRA/Applications/Farhelm.app/Contents/notes"
LEGACY_EXTRA_BEFORE=$(find "$HOME_LEGACY_EXTRA" -type f -exec sha256sum {} \; | sort)
run_install "$MAC_TOOLS" "$HOME_LEGACY_EXTRA" "$BASE/good-v2" 1.2.4
check "legacy shape plus a user file: refused" [ "$RC" -eq 1 ]
check "legacy shape plus a user file: the refusal says why" contains "$ERR" "is not an app bundle this installer built"
check "legacy shape plus a user file: nothing changed" \
  [ "$(find "$HOME_LEGACY_EXTRA" -type f -exec sha256sum {} \; | sort)" = "$LEGACY_EXTRA_BEFORE" ]

HOME_OTHER_DIR="$WORKDIR/home-other-dir"
seed_old_layout "$HOME_OTHER_DIR" 1.2.3
python3 - "$HOME_OTHER_DIR/Applications/Farhelm.app/Contents/.farhelm-installation" <<'PY'
import sys
path = sys.argv[1]
fields = open(path, "rb").read().split(b"\0")
fields[1] = b"/elsewhere/bin"
open(path, "wb").write(b"\0".join(fields))
PY
OTHER_DIR_BEFORE=$(find "$HOME_OTHER_DIR" -type f -exec sha256sum {} \; | sort)
run_install "$MAC_TOOLS" "$HOME_OTHER_DIR" "$BASE/good-v2" 1.2.4
check "old bundle recorded for another directory: refused" [ "$RC" -eq 1 ]
check "old bundle recorded for another directory: nothing changed" \
  [ "$(find "$HOME_OTHER_DIR" -type f -exec sha256sum {} \; | sort)" = "$OTHER_DIR_BEFORE" ]

# ===========================================================================
# Scenario: an app bundle this installer did not build is the user's, and
# a refusal changes nothing at all. Why: SPEC.md's installation rule (a file
# is not destroyed because its name matches), and since the app is the
# whole installation there is nothing else to install first.
# ===========================================================================
echo
echo "== a foreign Farhelm.app =="
HOME_FOREIGN="$WORKDIR/home-foreign"
mkdir -p "$HOME_FOREIGN/Applications/Farhelm.app/Contents"
cat >"$HOME_FOREIGN/Applications/Farhelm.app/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
	<key>CFBundleIdentifier</key><string>com.example.unrelated</string>
</dict></plist>
EOF
run_install "$MAC_TOOLS" "$HOME_FOREIGN" "$BASE/good" 1.2.3
check "foreign Farhelm.app: refused" [ "$RC" -eq 1 ]
check "foreign Farhelm.app: the refusal names the bundle" contains "$ERR" "Farhelm.app exists and is not an app bundle this installer built"
check "foreign Farhelm.app: the user's bundle is untouched" \
  contains "$(cat "$HOME_FOREIGN/Applications/Farhelm.app/Contents/Info.plist")" "com.example.unrelated"
check "foreign Farhelm.app: no Terminal link was created" [ ! -e "$HOME_FOREIGN/.local/bin/farhelm" ]
check "foreign Farhelm.app: the lock was released" [ ! -e "$HOME_FOREIGN/Applications/.farhelm-app.lock" ]

# A current-layout record that names another link folder (one that exists
# elsewhere, say ~/dotfiles/bin when ~/.local/bin was a symlink to it) is
# still this installation's: the app's location is fixed, and the installer
# only ever records this home's link.
HOME_OTHER_LINK="$WORKDIR/home-other-link"
mkdir -p "$HOME_OTHER_LINK"
run_install "$MAC_TOOLS" "$HOME_OTHER_LINK" "$BASE/good" 1.2.3
check "other-link record setup: install exits 0" [ "$RC" -eq 0 ]
mkdir -p "$WORKDIR/other-link-bin"
printf 'farhelm-app-v2\000%s/farhelm\000' "$(cd -P "$WORKDIR/other-link-bin" && pwd -P)" \
  >"$HOME_OTHER_LINK/Applications/Farhelm.app/Contents/.farhelm-installation"
run_install "$MAC_TOOLS" "$HOME_OTHER_LINK" "$BASE/good-v2" 1.2.4
check "other-link record: accepted, since the fixed app can only be this home's" [ "$RC" -eq 0 ]
check "other-link record: rewritten to name this home's link" assert_current_record "$HOME_OTHER_LINK"

# The bin directory moved under the installation: ~/.local/bin is now a
# symlink to where it went, so the recorded link's folder resolves here.
# Why: without this, every later update refused the app as foreign and
# only deleting it by hand recovered.
HOME_MOVED_BIN="$WORKDIR/home-moved-bin"
mkdir -p "$HOME_MOVED_BIN"
run_install "$MAC_TOOLS" "$HOME_MOVED_BIN" "$BASE/good" 1.2.3
check "moved bin directory setup: install exits 0" [ "$RC" -eq 0 ]
mv "$HOME_MOVED_BIN/.local/bin" "$HOME_MOVED_BIN/bin-moved"
ln -s ../bin-moved "$HOME_MOVED_BIN/.local/bin"
run_install "$MAC_TOOLS" "$HOME_MOVED_BIN" "$BASE/good-v2" 1.2.4
check "moved bin directory: the update exits 0" [ "$RC" -eq 0 ]
check "moved bin directory: the record now names the resolved link" assert_current_record "$HOME_MOVED_BIN"

# The recorded link's folder no longer exists at all (a renamed home).
HOME_RENAMED="$WORKDIR/home-renamed"
mkdir -p "$HOME_RENAMED"
run_install "$MAC_TOOLS" "$HOME_RENAMED" "$BASE/good" 1.2.3
printf 'farhelm-app-v2\000/nonexistent/old-home/.local/bin/farhelm\000' \
  >"$HOME_RENAMED/Applications/Farhelm.app/Contents/.farhelm-installation"
run_install "$MAC_TOOLS" "$HOME_RENAMED" "$BASE/good-v2" 1.2.4
check "renamed home: the update exits 0" [ "$RC" -eq 0 ]
check "renamed home: the record now names this home's link" assert_current_record "$HOME_RENAMED"

# ===========================================================================
# Scenario: a release from before the side-by-side layout is refused before
# anything changes. Why: its desktop app would start its supervisor from its
# sibling, which in this layout is the forwarder.
# ===========================================================================
echo
echo "== a release from before the layout =="
HOME_PRELAYOUT="$WORKDIR/home-prelayout"
mkdir -p "$HOME_PRELAYOUT"
run_install "$MAC_TOOLS" "$HOME_PRELAYOUT" "$BASE/prelayout" 1.2.4
check "pre-layout release: refused" [ "$RC" -eq 1 ]
check "pre-layout release: the refusal names it" \
  contains "$ERR" "❌ Farhelm 1.2.4 is too old for this installer: it cannot be updated while it runs."
check "pre-layout release: no app" [ ! -e "$HOME_PRELAYOUT/Applications/Farhelm.app" ]
check "pre-layout release: no Terminal link" [ ! -e "$HOME_PRELAYOUT/.local/bin/farhelm" ]
check "pre-layout release: no staging left" [ -z "$(find "$HOME_PRELAYOUT" -name '.farhelm*')" ]
PRELAYOUT_BEFORE=$(find "$HOME1" -type f -exec sha256sum {} \; | sort)
run_install "$MAC_TOOLS" "$HOME1" "$BASE/prelayout" 1.2.4
check "pre-layout release over an installation: refused" [ "$RC" -eq 1 ]
check "pre-layout release over an installation: every file is unchanged" \
  [ "$(find "$HOME1" -type f -exec sha256sum {} \; | sort)" = "$PRELAYOUT_BEFORE" ]

# A release whose desktop archive predates the icon cannot satisfy the app
# contract either, and is refused the same way.
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
check "pre-icon release creates no bundle" [ ! -e "$HOME_PREICON/Applications/Farhelm.app" ]

# ===========================================================================
# Scenario: the bin directory. Why: ~/.local/bin/farhelm is only a link
# now, but a file of the user's own at that name must survive under a
# visible name, a directory there must not be touched, and a farhelm-desktop
# of the user's own is not the installer's to remove.
# ===========================================================================
echo
echo "== the bin directory =="
kept_files() { find "$1" -maxdepth 1 -name 'farhelm.replaced-*' | sort; }

HOME_K1="$WORKDIR/home-k1"
INSTALL_K1="$HOME_K1/.local/bin"
mkdir -p "$INSTALL_K1"
printf '#!/bin/sh\necho my own wrapper\n' >"$INSTALL_K1/farhelm"
K1_OWN=$(cat "$INSTALL_K1/farhelm")
K1_TOOLS="$WORKDIR/toolchain-k1"
mkdir -p "$K1_TOOLS"
cp -a "$TOOLCHAIN_FULL"/. "$K1_TOOLS/"
write_fake_tmux "$K1_TOOLS" "tmux $TMUX_FLOOR"
check "K1 premise: tmux meets the floor" [ "$("$K1_TOOLS/tmux" -V)" = "tmux $TMUX_FLOOR" ]
run_install "$K1_TOOLS" "$HOME_K1" "$BASE/good" 1.2.3
check "K1 (the user's own farhelm): the install exits 0" [ "$RC" -eq 0 ]
K1_KEPT=$(kept_files "$INSTALL_K1")
check "K1: exactly one kept copy exists" [ "$(printf '%s\n' "$K1_KEPT" | grep -c .)" -eq 1 ]
check "K1: the kept copy is the user's file byte-for-byte" [ "$(cat "$K1_KEPT")" = "$K1_OWN" ]
check "K1: farhelm is now the Terminal link" [ -L "$INSTALL_K1/farhelm" ]
K1_EXPECTED="✅ Farhelm 1.2.3 is installed.

   Open Farhelm from Spotlight or ~/Applications.

ℹ️  ~/.local/bin/farhelm was not installed by this installer, so it was renamed
   to ~/.local/bin/${K1_KEPT##*/} - farhelm installation still
   proceeded.

   To uninstall later, run: ~/.local/bin/farhelm uninstall"
check "K1: exact fresh-install report and kept-file placement" [ "$OUT" = "$K1_EXPECTED" ]
run_install "$TOOLCHAIN_FULL" "$HOME_K1" "$BASE/good" 1.2.3
check "K2 (an update over the link): exits 0" [ "$RC" -eq 0 ]
check "K2: no new kept copy" [ "$(kept_files "$INSTALL_K1")" = "$K1_KEPT" ]
check "K2: the closing message mentions no kept copy" not_contains "$OUT" "was not installed by this installer"

HOME_K3="$WORKDIR/home-k3"
seed_old_layout "$HOME_K3" 1.2.3
printf '#!/bin/sh\necho edited since\n' >"$HOME_K3/.local/bin/farhelm"
run_install "$TOOLCHAIN_FULL" "$HOME_K3" "$BASE/good-v2" 1.2.4
check "K3 (an old-layout copy edited since): the update exits 0" [ "$RC" -eq 0 ]
check "K3: the edited file is kept" [ "$(cat "$(kept_files "$HOME_K3/.local/bin")")" = "#!/bin/sh
echo edited since" ]

HOME_K4="$WORKDIR/home-k4"
mkdir -p "$HOME_K4/.local/bin"
printf 'my own desktop\n' >"$HOME_K4/.local/bin/farhelm-desktop"
run_install "$TOOLCHAIN_FULL" "$HOME_K4" "$BASE/good" 1.2.3
check "K4 (the user's own farhelm-desktop): the install exits 0" [ "$RC" -eq 0 ]
check "K4: the user's farhelm-desktop is untouched" [ "$(cat "$HOME_K4/.local/bin/farhelm-desktop")" = "my own desktop" ]

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
  check "K5: a kept copy exists" [ -n "$K5_KEPT" ]
  chmod 600 "$K5_KEPT"
  check "K5: the kept copy keeps its bytes" [ "$(cat "$K5_KEPT")" = "unreadable" ]
fi

HOMEDIRDEST="$WORKDIR/homedirdest"
INSTALLDIRDEST="$HOMEDIRDEST/.local/bin"
mkdir -p "$INSTALLDIRDEST/farhelm"
echo "sentinel" >"$INSTALLDIRDEST/farhelm/keepme"
run_install "$TOOLCHAIN_FULL" "$HOMEDIRDEST" "$BASE/good" 1.2.3
check "a directory at ~/.local/bin/farhelm: exits 1" [ "$RC" -ne 0 ]
check "a directory at ~/.local/bin/farhelm: names the problem" contains "$ERR" "is a directory"
check "a directory at ~/.local/bin/farhelm: its contents are untouched" \
  [ "$(cat "$INSTALLDIRDEST/farhelm/keepme")" = "sentinel" ]
check "a directory at ~/.local/bin/farhelm: the app itself is installed" \
  app_launchable "$HOMEDIRDEST/Applications/Farhelm.app"

# ===========================================================================
# Scenario: the bundle lock. Why: two installers must not change the same
# bundle at once, and a lock found held (or left by an interrupted run,
# which nothing can tell apart) refuses with nothing changed.
# ===========================================================================
echo
echo "== the bundle lock =="
HOME_LOCKED="$WORKDIR/home-locked"
mkdir -p "$HOME_LOCKED"
run_install "$MAC_TOOLS" "$HOME_LOCKED" "$BASE/good" 1.2.3
check "bundle lock setup: install exits 0" [ "$RC" -eq 0 ]
mkdir "$HOME_LOCKED/Applications/.farhelm-app.lock"
LOCKED_BEFORE=$(find "$HOME_LOCKED" -type f -exec sha256sum {} \; | sort)
run_install "$MAC_TOOLS" "$HOME_LOCKED" "$BASE/good-v2" 1.2.4
check "a held bundle lock: refused" [ "$RC" -eq 1 ]
check "a held bundle lock: the refusal names the lock" contains "$ERR" "$HOME_LOCKED/Applications/.farhelm-app.lock"
check "a held bundle lock: nothing changed" \
  [ "$(find "$HOME_LOCKED" -type f -exec sha256sum {} \; | sort)" = "$LOCKED_BEFORE" ]
check "a held bundle lock: the lock is left in place" [ -d "$HOME_LOCKED/Applications/.farhelm-app.lock" ]


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
check "redirect-chain install produced a working farhelm" contains "$(forward "$INSTALL_REDIRECT/farhelm" --version 2>/dev/null || true)" "farhelm 1.2.3"

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
  [ -x "$HOMEROSETTA/Applications/Farhelm.app/Contents/MacOS/farhelm-desktop" ]

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
    contains "$(forward "$HOMEV/.local/bin/farhelm" --version 2>/dev/null || true)" "farhelm 1.2.3-rc.1"
done
for spec in "1.2.3-dev.1" "v1.2.3-dev.1"; do
  HOMEV="$WORKDIR/homev-${spec//./-}"
  mkdir -p "$HOMEV"
  run_install "$TOOLCHAIN_FULL" "$HOMEV" "$BASE/prerelease-dev" "$spec"
  check "FARHELM_VERSION=$spec exits 0" [ "$RC" -eq 0 ]
  check "FARHELM_VERSION=$spec normalizes to farhelm 1.2.3-dev.1" \
    contains "$(forward "$HOMEV/.local/bin/farhelm" --version 2>/dev/null || true)" "farhelm 1.2.3-dev.1"
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
    contains "$(forward "$HOMEV/.local/bin/farhelm" --version 2>/dev/null || true)" "farhelm 0.0.0-unreleased"
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
    contains "$(forward "$HOMEONLY/.local/bin/farhelm" --version 2>/dev/null || true)" "farhelm 1.2.3"
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
    check "report ($label): update premise is the current layout" assert_current_record "$home"
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
  [ "$(find "$HOMEDEFAULT" -type f 2>/dev/null | wc -l)" -eq 7 ]

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
  contains "$(forward "$INSTALLF27/farhelm" --version 2>/dev/null || true)" "farhelm $F27_VERSION"

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
check "F9: the installed farhelm is mode 0755" \
  [ "$(stat -c %a "$HOMEUMASK/Applications/Farhelm.app/Contents/Versions/1.2.3/farhelm")" = "755" ]
for f9_dir in Applications Applications/Farhelm.app Applications/Farhelm.app/Contents \
  Applications/Farhelm.app/Contents/MacOS Applications/Farhelm.app/Contents/Versions \
  Applications/Farhelm.app/Contents/Versions/1.2.3; do
  check "F9: $f9_dir is not group/world-writable" [ "$(stat -c %a "$HOMEUMASK/$f9_dir")" = "755" ]
done
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
