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

# build_archive OUT_TAR PACKAGE TARGET CONTENT_LINE
# Builds one well-formed release archive: PACKAGE-TARGET/<binary>, holding a
# shell script that prints CONTENT_LINE. <binary> is "farhelm-desktop" for
# the farhelm-desktop package and "farhelm" for everything else, matching
# RELEASE_ARCHIVES.
build_archive() {
  local out=$1 package=$2 target=$3 content=$4
  local binary=farhelm
  [ "$package" = farhelm-desktop ] && binary=farhelm-desktop
  local stage
  stage=$(mktemp -d "$WORKDIR/archive-stage.XXXXXX")
  write_binary "$stage/$package-$target" "$binary" "$content"
  tar -czf "$out" -C "$stage" "$package-$target/$binary"
  rm -rf "$stage"
}

# build_good_release DIR VERSION
# The full six-asset inventory (plan §1 / RELEASE_ARCHIVES), all checksums
# correct: farhelm and farhelm-desktop for every target, both tmux builds,
# SHA256SUMS. Every farhelm/farhelm-desktop member prints "farhelm VERSION"
# / "farhelm-desktop VERSION".
build_good_release() {
  local dir=$1 version=$2
  mkdir -p "$dir"
  build_archive "$dir/farhelm-x86_64-unknown-linux-musl.tar.gz" farhelm x86_64-unknown-linux-musl "farhelm $version"
  build_archive "$dir/farhelm-aarch64-unknown-linux-musl.tar.gz" farhelm aarch64-unknown-linux-musl "farhelm $version"
  build_archive "$dir/farhelm-aarch64-apple-darwin.tar.gz" farhelm aarch64-apple-darwin "farhelm $version"
  build_archive "$dir/farhelm-desktop-aarch64-apple-darwin.tar.gz" farhelm-desktop aarch64-apple-darwin "farhelm-desktop $version"
  printf '#!/bin/sh\necho "tmux fixture x86_64"\n' >"$dir/tmux-x86_64-unknown-linux-musl"
  printf '#!/bin/sh\necho "tmux fixture aarch64"\n' >"$dir/tmux-aarch64-unknown-linux-musl"
  chmod 755 "$dir/tmux-x86_64-unknown-linux-musl" "$dir/tmux-aarch64-unknown-linux-musl"
  (cd "$dir" && sha256sum -- *.tar.gz tmux-* >SHA256SUMS)
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
# One archive (the x86_64 Linux target only -- that is the platform this
# harness itself runs as, so no uname shim is needed to reach it) whose
# member list has TWO entries basename-matching "farhelm". Exercises the
# "more than one candidate member" refusal.
build_two_member_archive_release() {
  local dir=$1 version=$2
  mkdir -p "$dir"
  local stage
  stage=$(mktemp -d "$WORKDIR/two-member-stage.XXXXXX")
  write_binary "$stage/a" farhelm "farhelm $version"
  write_binary "$stage/b" farhelm "farhelm $version"
  tar -czf "$dir/farhelm-x86_64-unknown-linux-musl.tar.gz" -C "$stage" a/farhelm b/farhelm
  rm -rf "$stage"
  (cd "$dir" && sha256sum -- farhelm-x86_64-unknown-linux-musl.tar.gz >SHA256SUMS)
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
  mkdir -p "$stage/farhelm-x86_64-unknown-linux-musl"
  ln -s /nonexistent-target "$stage/farhelm-x86_64-unknown-linux-musl/farhelm"
  # tar does not dereference a symlink source by default -- the archive
  # member itself is a symlink entry, which is exactly the case under test.
  tar -czf "$dir/farhelm-x86_64-unknown-linux-musl.tar.gz" -C "$stage" farhelm-x86_64-unknown-linux-musl/farhelm
  rm -rf "$stage"
  (cd "$dir" && sha256sum -- farhelm-x86_64-unknown-linux-musl.tar.gz >SHA256SUMS)
}

# build_zero_rows_release DIR VERSION (F18)
# A valid archive, but an EMPTY SHA256SUMS -- zero matching rows, not one.
build_zero_rows_release() {
  local dir=$1 version=$2
  mkdir -p "$dir"
  build_archive "$dir/farhelm-x86_64-unknown-linux-musl.tar.gz" farhelm x86_64-unknown-linux-musl "farhelm $version"
  : >"$dir/SHA256SUMS"
}

# build_duplicate_rows_release DIR VERSION (F18)
# A valid archive whose SHA256SUMS lists it TWICE (both lines correct) --
# ambiguous multiplicity even though every individual line verifies.
build_duplicate_rows_release() {
  local dir=$1 version=$2
  mkdir -p "$dir"
  build_archive "$dir/farhelm-x86_64-unknown-linux-musl.tar.gz" farhelm x86_64-unknown-linux-musl "farhelm $version"
  (cd "$dir" && sha256sum -- farhelm-x86_64-unknown-linux-musl.tar.gz >SHA256SUMS)
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
  write_binary "$stage/farhelm-x86_64-unknown-linux-musl" not-farhelm "farhelm $version"
  tar -czf "$dir/farhelm-x86_64-unknown-linux-musl.tar.gz" -C "$stage" farhelm-x86_64-unknown-linux-musl/not-farhelm
  rm -rf "$stage"
  (cd "$dir" && sha256sum -- farhelm-x86_64-unknown-linux-musl.tar.gz >SHA256SUMS)
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
  mkdir -p "$stage/farhelm-x86_64-unknown-linux-musl"
  printf '#!/bin/sh\necho "farhelm %s (decoy, should never run)"\n' "$version" \
    >"$stage/farhelm-x86_64-unknown-linux-musl/farhelm.extra"
  chmod 755 "$stage/farhelm-x86_64-unknown-linux-musl/farhelm.extra"
  ln -s /nonexistent-target "$stage/farhelm-x86_64-unknown-linux-musl/farhelm"
  tar -czf "$dir/farhelm-x86_64-unknown-linux-musl.tar.gz" -C "$stage" \
    farhelm-x86_64-unknown-linux-musl/farhelm.extra farhelm-x86_64-unknown-linux-musl/farhelm
  rm -rf "$stage"
  (cd "$dir" && sha256sum -- farhelm-x86_64-unknown-linux-musl.tar.gz >SHA256SUMS)
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
BASE_TOOLS=(uname mkdir mktemp rmdir ls cp curl tar gzip sha256sum shasum openssl awk sed grep tr head cut mv rm chmod cat sysctl sleep)

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
# isolated $HOME and $FARHELM_INSTALL_DIR the caller provides, always
# `/bin/sh` naming install.sh by absolute path (so no scenario's stripped-
# down $PATH needs to contain "sh" itself).
#
# Sets globals RC (exit status), OUT (stdout), ERR (stderr) for the caller
# to assert against.
# ---------------------------------------------------------------------------
run_install() {
  local path_dir=$1 home=$2 install_dir=$3 base_url=$4 version=$5
  local out_file err_file
  out_file=$(mktemp "$WORKDIR/out.XXXXXX")
  err_file=$(mktemp "$WORKDIR/err.XXXXXX")
  set +e
  env -i \
    PATH="$path_dir" \
    HOME="$home" \
    FARHELM_INSTALL_DIR="$install_dir" \
    FARHELM_RELEASE_BASE_URL="$base_url" \
    FARHELM_VERSION="$version" \
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
  local path_dir=$1 home=$2 install_dir=$3 base_url=$4 version=$5
  env -i \
    PATH="$path_dir" \
    HOME="$home" \
    FARHELM_INSTALL_DIR="$install_dir" \
    FARHELM_RELEASE_BASE_URL="$base_url" \
    FARHELM_VERSION="$version" \
    /bin/sh "$INSTALL_SH" >"$WORKDIR/bg-out" 2>"$WORKDIR/bg-err" &
  BG_PID=$!
}

# find_snapshot DIR -- a stable, sorted directory listing used for the
# "nothing outside FARHELM_INSTALL_DIR changed" check. Includes file types
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
corrupt_checksum "$WWW/badchecksum" farhelm-x86_64-unknown-linux-musl.tar.gz
build_two_member_archive_release "$WWW/twomember" 1.2.3
build_nonregular_member_release "$WWW/nonregular"
build_good_release "$WWW/prerelease" 1.2.3-rc.1
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
# Scenario: fresh install (also the base case every later scenario's
# "update" and "rollback" tests build on).
# ===========================================================================
echo
echo "== fresh install =="
HOME1="$WORKDIR/home1"
INSTALL1="$HOME1/.local/bin"
mkdir -p "$HOME1"
run_install "$TOOLCHAIN_FULL" "$HOME1" "$INSTALL1" "$BASE/good" 1.2.3
check "fresh install exits 0" [ "$RC" -eq 0 ]
check "fresh install writes farhelm" [ -x "$INSTALL1/farhelm" ]
check "fresh install reports its own version" contains "$("$INSTALL1/farhelm" --version)" "farhelm 1.2.3"
check "fresh install installs mode 0755" [ "$(stat -c %a "$INSTALL1/farhelm")" = "755" ]
check "fresh install reports Installed" contains "$OUT" "Installed farhelm 1.2.3 to $INSTALL1."
check "fresh install has no leftover staging/lock/backup dot-files" [ -z "$(find "$INSTALL1" -maxdepth 1 -name '.farhelm*')" ]

# ===========================================================================
# Scenario: update (re-run against the same release) -- the ".old" rollback
# text must NOT appear, since nothing failed.
# ===========================================================================
echo
echo "== update (re-run) =="
run_install "$TOOLCHAIN_FULL" "$HOME1" "$INSTALL1" "$BASE/good" 1.2.3
check "update exits 0" [ "$RC" -eq 0 ]
check "update reports Updated" contains "$OUT" "Updated. Restart what is running:"
check "update mentions systemctl restart line" contains "$OUT" "systemctl --user restart farhelm-supervisor farhelm-helm"
check "update does not print a rollback message" not_contains "$OUT$ERR" "was restored"
check "update leaves no leftover staging/lock/backup dot-files" [ -z "$(find "$INSTALL1" -maxdepth 1 -name '.farhelm*')" ]

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
run_install "$TOOLCHAIN_FULL" "$HOME_REDIRECT" "$INSTALL_REDIRECT" "$BASE/redirect" 1.2.3
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
MAC_TOOLS="$WORKDIR/toolchain-mac"
mkdir -p "$MAC_TOOLS"
cp -a "$TOOLCHAIN_FULL"/. "$MAC_TOOLS/"
# uname is a symlink from the cp -a above (to the real /usr/bin/uname); `cat
# >` on it would follow the symlink and try to overwrite the real system
# binary in place (and fail with EACCES, since it is root-owned) rather than
# replacing what $MAC_TOOLS/uname points AT. Remove the symlink first so
# this writes a fresh regular file instead.
rm -f "$MAC_TOOLS/uname"
cat >"$MAC_TOOLS/uname" <<'EOF'
#!/bin/sh
case "$1" in
  -s) echo Darwin ;;
  -m) echo arm64 ;;
esac
EOF
chmod 755 "$MAC_TOOLS/uname"

HOME_MAC="$WORKDIR/home-mac"
INSTALL_MAC="$HOME_MAC/.local/bin"
mkdir -p "$HOME_MAC"
run_install "$MAC_TOOLS" "$HOME_MAC" "$INSTALL_MAC" "$BASE/good" 1.2.3
check "macOS-shaped fresh install exits 0" [ "$RC" -eq 0 ]
check "macOS-shaped fresh install writes farhelm-desktop too" [ -x "$INSTALL_MAC/farhelm-desktop" ]
check "macOS-shaped fresh install reports the desktop binary" contains "$OUT" "Installed farhelm 1.2.3 (and farhelm-desktop) to $INSTALL_MAC."
check "macOS-shaped fresh install: farhelm reports 1.2.3" \
  [ "$("$INSTALL_MAC/farhelm" --version)" = "farhelm 1.2.3" ]

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
run_install "$MAC_TOOLS_FAILDESKTOP" "$HOME_MAC" "$INSTALL_MAC" "$BASE/good-v2" 1.2.4
check "forced desktop-replace failure exits 1" [ "$RC" -ne 0 ]
check "forced desktop-replace failure prints the exact required message" \
  contains "$ERR" "update failed while replacing farhelm-desktop; the previous farhelm was restored"
check "forced desktop-replace failure restores the OLD (1.2.3) farhelm byte-for-byte" \
  [ "$(cat "$INSTALL_MAC/farhelm")" = "$OLD_FARHELM_CONTENT" ]
check "forced desktop-replace failure leaves the OLD (1.2.3) farhelm-desktop untouched" \
  [ "$(cat "$INSTALL_MAC/farhelm-desktop")" = "$OLD_DESKTOP_CONTENT" ]
check "forced desktop-replace failure: farhelm still reports 1.2.3 (not 1.2.4)" \
  [ "$("$INSTALL_MAC/farhelm" --version)" = "farhelm 1.2.3" ]
check "forced desktop-replace failure leaves no leftover staging/lock/backup dot-files" \
  [ -z "$(find "$INSTALL_MAC" -maxdepth 1 -name '.farhelm*')" ]

# Retry the SAME 1.2.4 update, this time without the forced failure -- both
# destinations must now contain the NEW (1.2.4) content, proving the
# earlier rollback did not somehow leave a partial or stuck state behind.
MAC_TOOLS_V2="$WORKDIR/toolchain-mac-v2"
mkdir -p "$MAC_TOOLS_V2"
cp -a "$MAC_TOOLS"/. "$MAC_TOOLS_V2/"
run_install "$MAC_TOOLS_V2" "$HOME_MAC" "$INSTALL_MAC" "$BASE/good-v2" 1.2.4
check "a real update to 1.2.4 after the forced failure succeeds" [ "$RC" -eq 0 ]
check "real update: farhelm now reports 1.2.4" [ "$("$INSTALL_MAC/farhelm" --version)" = "farhelm 1.2.4" ]
check "real update: farhelm-desktop content changed from the 1.2.3 original" \
  [ "$(cat "$INSTALL_MAC/farhelm-desktop")" != "$OLD_DESKTOP_CONTENT" ]

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
run_install "$TOOLCHAIN_FULL" "$HOMEFIRSTFAIL" "$INSTALLFIRSTFAIL" "$BASE/good" 1.2.3
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

run_install "$TOOLS_FAILFIRST" "$HOMEFIRSTFAIL" "$INSTALLFIRSTFAIL" "$BASE/good" 1.2.3
check "F3: forced first-replacement failure exits 1" [ "$RC" -ne 0 ]
check "F3: forced first-replacement failure prints the exact required message" \
  contains "$ERR" "install failed while replacing farhelm; the previous farhelm (if any) was restored"
check "F3: forced first-replacement failure restores the OLD farhelm byte-for-byte" \
  [ "$(cat "$INSTALLFIRSTFAIL/farhelm")" = "$OLD_FIRSTFAIL_CONTENT" ]
check "F3: forced first-replacement failure leaves no leftover staging/lock/backup dot-files" \
  [ -z "$(find "$INSTALLFIRSTFAIL" -maxdepth 1 -name '.farhelm*')" ]

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
run_install "$TOOLCHAIN_FULL" "$HOMEDIRDEST" "$INSTALLDIRDEST" "$BASE/good" 1.2.3
check "F4: directory-collision install exits 1" [ "$RC" -ne 0 ]
check "F4: directory-collision install names the problem" contains "$ERR" "is not a regular file"
check "F4: directory-collision install leaves the directory's contents untouched" \
  [ "$(cat "$INSTALLDIRDEST/farhelm/keepme")" = "sentinel" ]

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
run_install "$TOOLCHAIN_FULL" "$HOMECRASH" "$INSTALLCRASH" "$BASE/good" 1.2.3
check "F31 setup: initial install exits 0" [ "$RC" -eq 0 ]
OLD_CRASH_CONTENT=$(cat "$INSTALLCRASH/farhelm")

cp "$INSTALLCRASH/farhelm" "$INSTALLCRASH/.farhelm.old"
printf '#!/bin/sh\necho "farhelm 9.9.9-mid-swap"\n' >"$INSTALLCRASH/farhelm"
chmod 755 "$INSTALLCRASH/farhelm"
mkdir "$INSTALLCRASH/.farhelm-install.lock"
echo 999999 >"$INSTALLCRASH/.farhelm-install.lock/pid" # a pid nothing on this machine holds
# The journal lives INSIDE the lock directory and names binaries, not
# paths: "PARK cli" is the whole record for "the old farhelm was moved
# aside to its .old backup".
printf 'PARK cli\n' >"$INSTALLCRASH/.farhelm-install.lock/journal"

run_install "$TOOLCHAIN_FULL" "$HOMECRASH" "$INSTALLCRASH" "$BASE/good" 1.2.3
check "F31: recovery run exits 1 (it repairs and asks for a retry, not a silent continue)" \
  [ "$RC" -ne 0 ]
check "F31: recovery run reports what it did" contains "$ERR" "recovered from an interrupted install/update"
check "F31: recovery restores the pre-crash farhelm byte-for-byte" \
  [ "$(cat "$INSTALLCRASH/farhelm")" = "$OLD_CRASH_CONTENT" ]
check "F31: recovery removes the stale lock" [ ! -e "$INSTALLCRASH/.farhelm-install.lock" ]
check "F31: recovery removes the durable backup once restored" [ ! -e "$INSTALLCRASH/.farhelm.old" ]
check "F31: recovery removes the journal" [ ! -e "$INSTALLCRASH/.farhelm-install.lock/journal" ]

run_install "$TOOLCHAIN_FULL" "$HOMECRASH" "$INSTALLCRASH" "$BASE/good" 1.2.3
check "F31: an ordinary run after recovery succeeds" [ "$RC" -eq 0 ]

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
run_install "$ROSETTA_TOOLS" "$HOMEROSETTA" "$HOMEROSETTA/.local/bin" "$BASE/good" 1.2.3
check "F12: Rosetta-shaped Darwin/x86_64 with arm64 hardware installs" [ "$RC" -eq 0 ]
check "F12: Rosetta-shaped install fetches the aarch64-apple-darwin assets (farhelm-desktop too)" \
  [ -x "$HOMEROSETTA/.local/bin/farhelm-desktop" ]

INTEL_TOOLS="$WORKDIR/toolchain-intel"
mkdir -p "$INTEL_TOOLS"
cp -a "$ROSETTA_TOOLS"/. "$INTEL_TOOLS/"
rm -f "$INTEL_TOOLS/sysctl"
HOMEINTEL="$WORKDIR/homeintel"
mkdir -p "$HOMEINTEL"
run_install "$INTEL_TOOLS" "$HOMEINTEL" "$HOMEINTEL/.local/bin" "$BASE/good" 1.2.3
check "F12: a genuine Intel Mac (no arm64 hardware sysctl) is still rejected" [ "$RC" -ne 0 ]
check "F12: genuine Intel Mac rejection names the platform" contains "$ERR" "no release build for Darwin x86_64"

# ===========================================================================
# Scenario: Linux aarch64 platform mapping (F17) -- the only one of the
# three shipped targets none of the scenarios above ever selects (they are
# all either the harness's own x86_64 Linux host or macOS-shaped via a
# uname shim).
# ===========================================================================
echo
echo "== F17: Linux aarch64 platform mapping =="
AARCH64_TOOLS="$WORKDIR/toolchain-aarch64"
mkdir -p "$AARCH64_TOOLS"
cp -a "$TOOLCHAIN_FULL"/. "$AARCH64_TOOLS/"
rm -f "$AARCH64_TOOLS/uname"
cat >"$AARCH64_TOOLS/uname" <<'EOF'
#!/bin/sh
case "$1" in
  -s) echo Linux ;;
  -m) echo aarch64 ;;
esac
EOF
chmod 755 "$AARCH64_TOOLS/uname"

HOMEAARCH64="$WORKDIR/homeaarch64"
mkdir -p "$HOMEAARCH64"
run_install "$AARCH64_TOOLS" "$HOMEAARCH64" "$HOMEAARCH64/.local/bin" "$BASE/good" 1.2.3
check "F17: Linux aarch64 install exits 0" [ "$RC" -eq 0 ]
check "F17: Linux aarch64 install produces a working farhelm" \
  contains "$("$HOMEAARCH64/.local/bin/farhelm" --version 2>/dev/null || true)" "farhelm 1.2.3"
check "F17: Linux aarch64 does not also install farhelm-desktop" [ ! -e "$HOMEAARCH64/.local/bin/farhelm-desktop" ]

# ===========================================================================
# Scenario: 404 (no SHA256SUMS at all).
# ===========================================================================
echo
echo "== 404 (no SHA256SUMS) =="
HOME404="$WORKDIR/home404"
mkdir -p "$HOME404"
run_install "$TOOLCHAIN_FULL" "$HOME404" "$HOME404/.local/bin" "$BASE/norelease" 1.2.3
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
run_install "$TOOLCHAIN_FULL" "$HOMEBADSUM" "$HOMEBADSUM/.local/bin" "$BASE/badchecksum" 1.2.3
check "checksum mismatch exits 1" [ "$RC" -ne 0 ]
check "checksum mismatch names the failure" contains "$ERR" "checksum mismatch"

# ===========================================================================
# Scenario: malformed archive -- two members named farhelm.
# ===========================================================================
echo
echo "== malformed archive: two members named farhelm =="
HOMETWO="$WORKDIR/hometwo"
mkdir -p "$HOMETWO"
run_install "$TOOLCHAIN_FULL" "$HOMETWO" "$HOMETWO/.local/bin" "$BASE/twomember" 1.2.3
check "two-member archive exits 1" [ "$RC" -ne 0 ]
check "two-member archive names the count" contains "$ERR" "expected exactly 1"

# ===========================================================================
# Scenario: malformed archive -- a non-regular (symlink) member.
# ===========================================================================
echo
echo "== malformed archive: non-regular member =="
HOMENONREG="$WORKDIR/homenonreg"
mkdir -p "$HOMENONREG"
run_install "$TOOLCHAIN_FULL" "$HOMENONREG" "$HOMENONREG/.local/bin" "$BASE/nonregular" 1.2.3
check "non-regular member exits 1" [ "$RC" -ne 0 ]
check "non-regular member names the problem" contains "$ERR" "is not a regular file"

# ===========================================================================
# Scenario: version normalization, including a -rc.N prerelease (D15).
# ===========================================================================
echo
echo "== version normalization =="
for spec in "1.2.3-rc.1" "v1.2.3-rc.1"; do
  HOMEV="$WORKDIR/homev-${spec//./-}"
  mkdir -p "$HOMEV"
  run_install "$TOOLCHAIN_FULL" "$HOMEV" "$HOMEV/.local/bin" "$BASE/prerelease" "$spec"
  check "FARHELM_VERSION=$spec exits 0" [ "$RC" -eq 0 ]
  check "FARHELM_VERSION=$spec normalizes to farhelm 1.2.3-rc.1" \
    contains "$("$HOMEV/.local/bin/farhelm" --version 2>/dev/null || true)" "farhelm 1.2.3-rc.1"
done

HOMEPLAIN="$WORKDIR/homeplain"
mkdir -p "$HOMEPLAIN"
run_install "$TOOLCHAIN_FULL" "$HOMEPLAIN" "$HOMEPLAIN/.local/bin" "$BASE/good" "1.2.3"
check "bare X.Y.Z (no leading v) installs" [ "$RC" -eq 0 ]
HOMEVPLAIN="$WORKDIR/homevplain"
mkdir -p "$HOMEVPLAIN"
run_install "$TOOLCHAIN_FULL" "$HOMEVPLAIN" "$HOMEVPLAIN/.local/bin" "$BASE/good" "v1.2.3"
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
  run_install "$TOOLCHAIN_FULL" "$HOMEINV" "$HOMEINV/.local/bin" "$BASE/good" "$v"
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
  run_install "$TOOLS_MISSING" "$HOMEM" "$HOMEM/.local/bin" "$BASE/good" 1.2.3
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
run_install "$TOOLS_NO_CHECKSUM" "$HOMENOSUM" "$HOMENOSUM/.local/bin" "$BASE/good" 1.2.3
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
  run_install "$TOOLS_ONLY" "$HOMEONLY" "$HOMEONLY/.local/bin" "$BASE/good" 1.2.3
  check "checksum fallback via only $only succeeds" [ "$RC" -eq 0 ]
  check "checksum fallback via only $only produces a working farhelm" \
    contains "$("$HOMEONLY/.local/bin/farhelm" --version 2>/dev/null || true)" "farhelm 1.2.3"
done

# ===========================================================================
# Scenario: closing-message contract -- PATH warning, restart reminder, the
# four standing bullets, and the tmux hint across five tmux fixtures.
# ===========================================================================
echo
echo "== closing-message contract =="

HOMEPATHOUT="$WORKDIR/homepathout"
mkdir -p "$HOMEPATHOUT"
run_install "$TOOLCHAIN_FULL" "$HOMEPATHOUT" "$HOMEPATHOUT/.local/bin" "$BASE/good" 1.2.3
check "PATH warning appears when install dir is not on PATH" contains "$OUT" "is not on your PATH."
check "PATH warning includes a pasteable export line" contains "$OUT" "export PATH="

HOMEPATHIN="$WORKDIR/homepathin"
INSTALLPATHIN="$HOMEPATHIN/.local/bin"
mkdir -p "$INSTALLPATHIN"
run_install "$INSTALLPATHIN:$TOOLCHAIN_FULL" "$HOMEPATHIN" "$INSTALLPATHIN" "$BASE/good" 1.2.3
check "PATH warning is absent when install dir already on PATH" not_contains "$OUT" "is not on your PATH."

# The four always-present bullets (D8), on the fresh install captured above.
check "closing message: install summary" contains "$OUT" "Installed farhelm 1.2.3 to"
check "closing message: helm setup guidance" contains "$OUT" "run 'farhelm helm setup'"
check "closing message: do-not-run guidance" contains "$OUT" "Do NOT run it if this machine runs the desktop app"
check "closing message: SSH provisioning mention" contains "$OUT" "that helm installs the supervisor here over"

# F20: every settled semantic clause, line by line, across all four
# fresh/update x Linux/macOS combinations -- not just short substrings.
# Each invocation below puts the install directory ON PATH (no PATH-
# warning noise; that is covered separately above) and uses an at-floor
# tmux fixture (no tmux-hint noise; that is covered separately below), so
# every assertion here is squarely about the STANDING closing message.
assert_closing_message_contract() {
  local label=$1 has_desktop=$2 is_update=$3 version=$4
  local tools="$WORKDIR/toolchain-f20-$label"
  mkdir -p "$tools"
  cp -a "$TOOLCHAIN_FULL"/. "$tools/"
  write_fake_tmux "$tools" "tmux 3.7c"
  if [ "$has_desktop" = yes ]; then
    rm -f "$tools/uname"
    cat >"$tools/uname" <<'UNAMEEOF'
#!/bin/sh
case "$1" in
  -s) echo Darwin ;;
  -m) echo arm64 ;;
esac
UNAMEEOF
    chmod 755 "$tools/uname"
  fi
  local home="$WORKDIR/home-f20-$label"
  local install="$home/.local/bin"
  mkdir -p "$install"
  if [ "$is_update" = yes ]; then
    printf '#!/bin/sh\necho "farhelm 0.0.1-old"\n' >"$install/farhelm"
    chmod 755 "$install/farhelm"
    if [ "$has_desktop" = yes ]; then
      printf '#!/bin/sh\necho "farhelm-desktop 0.0.1-old"\n' >"$install/farhelm-desktop"
      chmod 755 "$install/farhelm-desktop"
    fi
  fi
  run_install "$install:$tools" "$home" "$install" "$BASE/good" "$version"
  check "F20 ($label): install exits 0" [ "$RC" -eq 0 ]

  # The install summary line, exactly (single or dual binary).
  if [ "$has_desktop" = yes ]; then
    check "F20 ($label): install summary names both binaries" \
      contains "$OUT" "Installed farhelm $version (and farhelm-desktop) to $install."
  else
    check "F20 ($label): install summary names farhelm only" \
      contains "$OUT" "Installed farhelm $version to $install."
    check "F20 ($label): install summary does not also claim farhelm-desktop" \
      not_contains "$OUT" "(and farhelm-desktop)"
  fi

  # The helm-setup paragraph, every line.
  check "F20 ($label): helm-setup paragraph line 1" \
    contains "$OUT" "If this machine should run your helm (the web UI on 127.0.0.1:7433) and host"
  check "F20 ($label): helm-setup paragraph line 2" \
    contains "$OUT" "agent sessions itself, run 'farhelm helm setup' — it writes and starts the helm"
  check "F20 ($label): helm-setup paragraph line 3" contains "$OUT" "and supervisor user units."

  # The do-not-run paragraph, every line.
  check "F20 ($label): do-not-run paragraph line 1" \
    contains "$OUT" "Do NOT run it if this machine runs the desktop app (farhelm-desktop starts its"
  check "F20 ($label): do-not-run paragraph line 2" \
    contains "$OUT" "own helm and local supervisor), or if it is a Linux session host you will add"
  check "F20 ($label): do-not-run paragraph line 3" \
    contains "$OUT" "from another helm's hosts panel (that helm installs the supervisor here over"
  check "F20 ($label): do-not-run paragraph line 4" \
    contains "$OUT" "SSH), or if you only want a browser tab against a helm elsewhere (nothing to"
  check "F20 ($label): do-not-run paragraph line 5" contains "$OUT" "set up)."

  # The restart-reminder block: present (every line) on an update, wholly
  # absent on a fresh install -- including its macOS-specific lines even
  # on a fresh LINUX install, and vice versa, since the whole block is one
  # unconditional unit covering both platforms.
  if [ "$is_update" = yes ]; then
    check "F20 ($label): restart-reminder heading" contains "$OUT" "Updated. Restart what is running:"
    check "F20 ($label): restart-reminder Linux line" \
      contains "$OUT" "Linux: systemctl --user restart farhelm-supervisor farhelm-helm"
    check "F20 ($label): restart-reminder macOS line 1" \
      contains "$OUT" "macOS: quit and reopen farhelm-desktop (it owns the embedded helm and any"
    check "F20 ($label): restart-reminder macOS line 2" \
      contains "$OUT" "supervisor it started as child processes; a supervisor you started by hand"
    check "F20 ($label): restart-reminder macOS line 3" \
      contains "$OUT" "with 'farhelm supervisor run' is reused as-is and needs restarting yourself)."
    check "F20 ($label): restart-reminder sessions-survive line 1" \
      contains "$OUT" "Running sessions survive either way — they live in tmux, which neither"
    check "F20 ($label): restart-reminder sessions-survive line 2" contains "$OUT" "restart touches."
  else
    check "F20 ($label): no restart-reminder on a fresh install" \
      not_contains "$OUT" "Updated. Restart what is running:"
  fi

  check "F20 ($label): no PATH warning (install dir is on PATH)" not_contains "$OUT" "is not on your PATH"
  check "F20 ($label): no tmux hint (at-floor fixture)" not_contains "$OUT" "tmux 3.7c or newer is required"
}
assert_closing_message_contract "fresh-linux" no no 1.2.3
assert_closing_message_contract "fresh-macos" yes no 1.2.3
assert_closing_message_contract "update-linux" no yes 1.2.3
assert_closing_message_contract "update-macos" yes yes 1.2.3

# tmux hint: absent, malformed, below-floor, exactly-at-floor, above-floor.
run_tmux_case() {
  local label=$1 tmux_output=$2 expect_hint=$3 expect_have=$4
  local tools="$WORKDIR/toolchain-tmux-$label"
  mkdir -p "$tools"
  cp -a "$TOOLCHAIN_FULL"/. "$tools/"
  if [ -n "$tmux_output" ]; then
    write_fake_tmux "$tools" "$tmux_output"
  fi
  local home="$WORKDIR/home-tmux-$label"
  mkdir -p "$home"
  run_install "$tools" "$home" "$home/.local/bin" "$BASE/good" 1.2.3
  if [ "$expect_hint" = yes ]; then
    check "tmux hint ($label): present" contains "$OUT" "tmux 3.7c or newer is required"
    check "tmux hint ($label): reports '$expect_have'" contains "$OUT" "this machine has $expect_have."
  else
    check "tmux hint ($label): absent" not_contains "$OUT" "tmux 3.7c or newer is required"
  fi
}
run_tmux_case "absent" "" yes "none"
run_tmux_case "malformed" "tmux next-3.8" yes "none"
run_tmux_case "below-floor" "tmux 3.6" yes "tmux 3.6"
run_tmux_case "at-floor" "tmux 3.7c" no ""
run_tmux_case "above-floor" "tmux 3.8" no ""

# ===========================================================================
# Scenario: no side effects outside FARHELM_INSTALL_DIR (F27) -- systemctl/
# launchctl are never invoked, and nothing under $HOME changes except
# FARHELM_INSTALL_DIR itself.
# ===========================================================================
echo
echo "== no service side effects, nothing outside FARHELM_INSTALL_DIR changes =="
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
# pre-create $HOMESIDE/.local itself (install.sh's `mkdir -p` would create
# it anyway as INSTALLSIDE's parent) so the before/after snapshot diff below
# is not tripped up by that expected, install-dir-adjacent directory coming
# into existence -- only its "bin" child is meant to be excluded from "must
# not change" by the $INSTALLSIDE filter.
mkdir -p "$HOMESIDE/.local" "$HOMESIDE/.config" "$HOMESIDE/Documents"
echo "untouched" >"$HOMESIDE/.bashrc"
echo "untouched" >"$HOMESIDE/Documents/notes.txt"

BEFORE_SNAPSHOT=$(find_snapshot "$HOMESIDE")
run_install "$SENTINEL_TOOLS" "$HOMESIDE" "$INSTALLSIDE" "$BASE/good" 1.2.3
check "sentinel-guarded install exits 0" [ "$RC" -eq 0 ]
check "systemctl/launchctl were never invoked" [ ! -s "$SENTINEL_LOG" ]
check ".bashrc is untouched" [ "$(cat "$HOMESIDE/.bashrc")" = "untouched" ]
check "Documents/notes.txt is untouched" [ "$(cat "$HOMESIDE/Documents/notes.txt")" = "untouched" ]

AFTER_SNAPSHOT=$(find_snapshot "$HOMESIDE")
DIFF_OUTSIDE_INSTALL=$(diff <(printf '%s\n' "$BEFORE_SNAPSHOT" | grep -Fv "$INSTALLSIDE") \
  <(printf '%s\n' "$AFTER_SNAPSHOT" | grep -Fv "$INSTALLSIDE") || true)
check "nothing outside FARHELM_INSTALL_DIR changed under \$HOME" [ -z "$DIFF_OUTSIDE_INSTALL" ]

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
run_install_bg "$TOOLCHAIN_FULL" "$HOMETERM" "$INSTALLTERM" "$BASE/slow" 1.2.3
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
  run_install "$TOOLCHAIN_FULL" "$home" "$install" "$BASE/$fixture" "$version"
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
# (F24) -- the Round 1 collision test only ever exercised the Linux
# farhelm path; a regression that validates only that path could still let
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
run_install "$MAC_TOOLS" "$HOMEMACDIRDEST" "$INSTALLMACDIRDEST" "$BASE/good" 1.2.3
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
run_install "$TOOLCHAIN_FULL" "$HOME503" "$INSTALL503" "$BASE/sums503" 1.2.3
check "F25: 503 exits 1" [ "$RC" -ne 0 ]
check "F25: 503 uses the generic diagnostic naming the code and URL" \
  contains "$ERR" "download failed (HTTP 503): $BASE/sums503/SHA256SUMS"
check "F25: 503 preserves the existing sentinel farhelm" [ "$(cat "$INSTALL503/farhelm")" = "$SENTINEL_503" ]
check "F25: 503 leaves no staging/lock/journal residue" \
  [ -z "$(find "$INSTALL503" -maxdepth 1 -name '.farhelm*' 2>/dev/null || true)" ]

# ===========================================================================
# Scenario: FARHELM_INSTALL_DIR omitted entirely (F26) -- proves the
# documented default ($HOME/.local/bin) is what the PRODUCTION expression
# evaluates to, not just what every other scenario's run_install happens
# to pass.
# ===========================================================================
echo
echo "== F26: default install directory (FARHELM_INSTALL_DIR unset) =="
HOMEDEFAULT="$WORKDIR/homedefault"
mkdir -p "$HOMEDEFAULT"
DEFAULT_OUT=$(mktemp "$WORKDIR/out.XXXXXX")
DEFAULT_ERR=$(mktemp "$WORKDIR/err.XXXXXX")
set +e
env -i PATH="$TOOLCHAIN_FULL" HOME="$HOMEDEFAULT" FARHELM_RELEASE_BASE_URL="$BASE/good" FARHELM_VERSION=1.2.3 \
  /bin/sh "$INSTALL_SH" >"$DEFAULT_OUT" 2>"$DEFAULT_ERR"
DEFAULT_RC=$?
set -e
check "F26: default install dir exits 0" [ "$DEFAULT_RC" -eq 0 ]
check "F26: farhelm lands at \$HOME/.local/bin/farhelm" [ -x "$HOMEDEFAULT/.local/bin/farhelm" ]
check "F26: nothing else under \$HOME was created" \
  [ "$(find "$HOMEDEFAULT" -type f 2>/dev/null | wc -l)" -eq 1 ]

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
want_code=0
for arg in "\$@"; do
  if [ "\$prev" = "-o" ]; then
    out="\$arg"
  fi
  if [ "\$arg" = "%{http_code}" ]; then
    want_code=1
  fi
  prev="\$arg"
  last="\$arg"
done
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
env -i PATH="$TOOLS_F27" HOME="$HOMEF27" FARHELM_INSTALL_DIR="$INSTALLF27" FARHELM_VERSION="$F27_VERSION" \
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
# install directory. This is what actually exercises the `{ ... }`
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
  local install="$WORKDIR/f22-install"
  rm -rf "$home" "$install"
  mkdir -p "$home" "$install"
  : >"$CURL_CALL_LOG"
  set +e
  env -i PATH="$CURL_RECORDER" HOME="$home" FARHELM_INSTALL_DIR="$install" FARHELM_VERSION=1.2.3 \
    /bin/sh "$prefix_file" >/dev/null 2>/dev/null
  local rc=$?
  set -e
  F22_CHECKED=$((F22_CHECKED + 1))
  local created
  created=$(find "$install" -mindepth 1 2>/dev/null | wc -l)
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
run_install_bg "$TOOLS_F23A" "$HOMEF23" "$INSTALLF23" "$BASE/good" 1.2.3
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
run_install "$TOOLCHAIN_FULL" "$HOMEF23" "$INSTALLF23" "$BASE/good" 1.2.3
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
  [ -z "$(find "$INSTALLF23" -maxdepth 1 -name '.farhelm*' 2>/dev/null || true)" ]

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
run_install "$TOOLCHAIN_FULL" "$HOMELOCKFILE" "$INSTALLLOCKFILE" "$BASE/good" 1.2.3
check "F6: a regular file at the lock path is refused" [ "$RC" -ne 0 ]
check "F6: refusal names it as not a farhelm lock" contains "$ERR" "not a farhelm install lock"
check "F6: the regular file is untouched" [ "$(cat "$INSTALLLOCKFILE/.farhelm-install.lock")" = "not a lock" ]

HOMELOCKDIR="$WORKDIR/homelockdir"
INSTALLLOCKDIR="$HOMELOCKDIR/.local/bin"
mkdir -p "$INSTALLLOCKDIR/.farhelm-install.lock"
echo "unrelated data" >"$INSTALLLOCKDIR/.farhelm-install.lock/somefile"
run_install "$TOOLCHAIN_FULL" "$HOMELOCKDIR" "$INSTALLLOCKDIR" "$BASE/good" 1.2.3
check "F6: a nonempty unrelated directory at the lock path is refused" [ "$RC" -ne 0 ]
check "F6: refusal names it as not a farhelm lock (directory case)" contains "$ERR" "not a farhelm install lock"
check "F6: the unrelated directory's contents are untouched" \
  [ "$(cat "$INSTALLLOCKDIR/.farhelm-install.lock/somefile")" = "unrelated data" ]

# ===========================================================================
# Scenario: a reserved backup path already has something at it before a
# FRESH transaction starts (F7) -- refused before any mutation, whatever it
# is preserved exactly.
# ===========================================================================
echo
echo "== F7: a pre-existing backup path aborts before any mutation =="
HOMEBACKUPCOLLISION="$WORKDIR/homebackupcollision"
INSTALLBACKUPCOLLISION="$HOMEBACKUPCOLLISION/.local/bin"
mkdir -p "$INSTALLBACKUPCOLLISION"
printf '#!/bin/sh\necho "farhelm 1.2.3"\n' >"$INSTALLBACKUPCOLLISION/farhelm"
chmod 755 "$INSTALLBACKUPCOLLISION/farhelm"
mkdir -p "$INSTALLBACKUPCOLLISION/.farhelm.old"
echo "unexpected" >"$INSTALLBACKUPCOLLISION/.farhelm.old/stray"
run_install "$TOOLCHAIN_FULL" "$HOMEBACKUPCOLLISION" "$INSTALLBACKUPCOLLISION" "$BASE/good" 1.2.3
check "F7: pre-existing backup-path collision exits 1" [ "$RC" -ne 0 ]
check "F7: pre-existing backup-path collision names the problem" contains "$ERR" "already exists"
check "F7: the collision directory's contents are untouched" \
  [ "$(cat "$INSTALLBACKUPCOLLISION/.farhelm.old/stray")" = "unexpected" ]
check "F7: the existing farhelm is untouched (refused before ANY move)" \
  [ "$("$INSTALLBACKUPCOLLISION/farhelm" --version)" = "farhelm 1.2.3" ]

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
echo 999999 >"$INSTALLRECOVERYBAD/.farhelm-install.lock/pid"
printf 'PARK cli\n' >"$INSTALLRECOVERYBAD/.farhelm-install.lock/journal"

run_install "$TOOLCHAIN_FULL" "$HOMERECOVERYBAD" "$INSTALLRECOVERYBAD" "$BASE/good" 1.2.3
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
env -i PATH="$TOOLCHAIN_FULL" HOME="$HOMEUMASK" FARHELM_INSTALL_DIR="$INSTALLUMASK" \
  FARHELM_RELEASE_BASE_URL="$BASE/good" FARHELM_VERSION=1.2.3 \
  /bin/sh -c 'umask 000; exec /bin/sh "$1"' _ "$INSTALL_SH" >"$WORKDIR/f9-out" 2>"$WORKDIR/f9-err"
F9_RC=$?
set -e
check "F9: umask-000 install exits 0" [ "$F9_RC" -eq 0 ]
check "F9: the installed binary is mode 0755" [ "$(stat -c %a "$INSTALLUMASK/farhelm")" = "755" ]
check "F9: the newly-created install directory is not group/world-writable" \
  [ "$(stat -c %a "$INSTALLUMASK")" = "755" ]

# ===========================================================================
# Scenario: multiline tmux output is rejected as a whole, not scanned line
# by line, for a valid version (F12).
# ===========================================================================
echo
echo "== F12: multiline tmux -V output is rejected wholesale =="
run_tmux_case "banner-then-valid" "$(printf 'some vendor banner\ntmux 3.7c')" yes "none"
run_tmux_case "two-valid-lines" "$(printf 'tmux 3.7c\ntmux 3.8')" yes "none"

# ===========================================================================
# Scenario: a colon-containing install directory is never treated as
# representable in PATH (F15), even when the AMBIENT PATH deceptively
# already contains the directory's two halves as separate, adjacent
# entries -- a naive substring membership test could mistake that for the
# real thing and wrongly suppress the warning.
# ===========================================================================
echo
echo "== F15: colon-containing install directory =="
HOMECOLON="$WORKDIR/homecolon"
INSTALLCOLON="$HOMECOLON/.local/bin:extra"
mkdir -p "$INSTALLCOLON"
DECEPTIVE_PATH="$TOOLCHAIN_FULL:$HOMECOLON/.local/bin:extra:/usr/bin"
set +e
env -i PATH="$DECEPTIVE_PATH" HOME="$HOMECOLON" FARHELM_INSTALL_DIR="$INSTALLCOLON" \
  FARHELM_RELEASE_BASE_URL="$BASE/good" FARHELM_VERSION=1.2.3 \
  /bin/sh "$INSTALL_SH" >"$WORKDIR/f15-out" 2>"$WORKDIR/f15-err"
F15_RC=$?
set -e
check "F15: colon-containing install dir still installs successfully" [ "$F15_RC" -eq 0 ]
check "F15: colon warning fires despite a deceptive PATH match" \
  contains "$(cat "$WORKDIR/f15-out")" "cannot be represented in PATH"
check "F15: colon warning names the actual directory" contains "$(cat "$WORKDIR/f15-out")" "$INSTALLCOLON"

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
  run_install "$MAC_TOOLS" "$home" "$install" "$BASE/good" 1.2.3
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

run_install "$TOOLS_R3F1" "$HOME_R3F1" "$INSTALL_R3F1" "$BASE/good-v2" 1.2.4
check "R3 F1: the forced update failure exits 1" [ "$RC" -ne 0 ]
check "R3 F1: farhelm is restored to the OLD (1.2.3) bytes" \
  [ "$(cat "$INSTALL_R3F1/farhelm")" = "$R3_OLD_FARHELM" ]
# The sharp one: a rollback that skipped its work would leave the desktop
# binary parked under its .old name and nothing at all at this path.
check "R3 F1: farhelm-desktop still exists, at the OLD (1.2.3) bytes" \
  [ "$(cat "$INSTALL_R3F1/farhelm-desktop")" = "$R3_OLD_DESKTOP" ]
check "R3 F1: no leftover lock, journal, or backup" \
  [ -z "$(find "$INSTALL_R3F1" -maxdepth 1 -name '.farhelm*' 2>/dev/null || true)" ]

run_install "$MAC_TOOLS" "$HOME_R3F1" "$INSTALL_R3F1" "$BASE/good-v2" 1.2.4
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

run_install "$TOOLS_R3F2A" "$HOME_R3F2A" "$INSTALL_R3F2A" "$BASE/good-v2" 1.2.4
check "R3 F2a: the run exits 1" [ "$RC" -ne 0 ]
check "R3 F2a: the explicit rollback reports it could not finish" \
  contains "$ERR" "automatic rollback could not fully complete"
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
run_install "$MAC_TOOLS" "$HOME_R3F2A" "$INSTALL_R3F2A" "$BASE/good-v2" 1.2.4
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
run_install "$TOOLS_R3F2B" "$HOME_R3F2B" "$INSTALL_R3F2B" "$BASE/good-v2" 1.2.4 2>/dev/null
check "R3 F2b: the killed run exits non-zero" [ "$RC" -ne 0 ]
check "R3 F2b: the killed run left its lock behind (no trap ran)" \
  [ -d "$INSTALL_R3F2B/.farhelm-install.lock" ]
check "R3 F2b: the killed run left its journal behind" \
  [ -e "$INSTALL_R3F2B/.farhelm-install.lock/journal" ]
check "R3 F2b: farhelm-desktop was restored before the kill" \
  [ "$(cat "$INSTALL_R3F2B/farhelm-desktop")" = "$R3_OLD_DESKTOP" ]

run_install "$MAC_TOOLS" "$HOME_R3F2B" "$INSTALL_R3F2B" "$BASE/good-v2" 1.2.4
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
# FARHELM_INSTALL_DIR is documented as taking any pathname, and a pathname
# may legally contain a pipe or a newline. A journal that spelled moves as
# "TYPE|SRC|DEST" could not represent either: the pipe shifts fragments into
# the wrong fields and the newline manufactures extra apparent records, so
# rollback would quietly no-op and report success. Records naming binaries
# instead of paths make the directory's name irrelevant to the format.
#
# Supported rather than refused: the one place a newline in the directory
# name genuinely cannot be served is the pasteable `export PATH=...` hint,
# and that already falls back to "add it to PATH by hand" instead of
# printing something unsafe.
# ===========================================================================
echo
echo "== R3 F3: an install directory containing '|' and a newline =="
HOME_R3F3="$WORKDIR/home-r3f3"
INSTALL_R3F3=$(printf '%s/.local/bin|pipe\nnewline' "$HOME_R3F3")
r3_seed_macos_pair "R3 F3" "$HOME_R3F3" "$INSTALL_R3F3"

run_install "$MAC_TOOLS_FAILDESKTOP" "$HOME_R3F3" "$INSTALL_R3F3" "$BASE/good-v2" 1.2.4
check "R3 F3: the forced update failure exits 1" [ "$RC" -ne 0 ]
check "R3 F3: farhelm is restored byte-for-byte" \
  [ "$(cat "$INSTALL_R3F3/farhelm")" = "$R3_OLD_FARHELM" ]
check "R3 F3: farhelm-desktop is restored byte-for-byte" \
  [ "$(cat "$INSTALL_R3F3/farhelm-desktop")" = "$R3_OLD_DESKTOP" ]
check "R3 F3: no leftover lock, journal, or backup" \
  [ -z "$(find "$INSTALL_R3F3" -maxdepth 1 -name '.farhelm*' 2>/dev/null || true)" ]

run_install "$MAC_TOOLS" "$HOME_R3F3" "$INSTALL_R3F3" "$BASE/good-v2" 1.2.4
check "R3 F3: a following run installs 1.2.4 cleanly" [ "$RC" -eq 0 ]
check "R3 F3: the following run's farhelm reports 1.2.4" \
  [ "$("$INSTALL_R3F3/farhelm" --version)" = "farhelm 1.2.4" ]
# The closing report is only reached by a run that gets that far, hence the
# assertion here rather than on the deliberately-failed run above.
check "R3 F3: the newline-named directory gets the by-hand PATH guidance" \
  contains "$OUT" "Add it to PATH by hand."

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
run_install "$TOOLCHAIN_FULL" "$HOME_R3F4BAD" "$INSTALL_R3F4BAD" "$BASE/good" 1.2.3
check "R3 F4 (bad record): setup install exits 0" [ "$RC" -eq 0 ]
R3F4_FARHELM=$(cat "$INSTALL_R3F4BAD/farhelm")
printf '#!/bin/sh\necho "farhelm 0.0.1-parked"\n' >"$INSTALL_R3F4BAD/.farhelm.old"
R3F4_BACKUP=$(cat "$INSTALL_R3F4BAD/.farhelm.old")
mkdir "$INSTALL_R3F4BAD/.farhelm-install.lock"
echo 999999 >"$INSTALL_R3F4BAD/.farhelm-install.lock/pid"
# A well-formed record followed by a path-bearing one in the retired
# "TYPE|SRC|DEST" spelling: the whole journal must be rejected, not
# partially replayed up to the record that offends.
{
  printf 'PARK cli\n'
  printf 'INSTALL|%s|%s\n' "$R3_OUTSIDE" "$INSTALL_R3F4BAD/farhelm"
} >"$INSTALL_R3F4BAD/.farhelm-install.lock/journal"

run_install "$TOOLCHAIN_FULL" "$HOME_R3F4BAD" "$INSTALL_R3F4BAD" "$BASE/good" 1.2.3
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
run_install "$TOOLCHAIN_FULL" "$HOME_R3F4LOCK" "$INSTALL_R3F4LOCK" "$BASE/good" 1.2.3
check "R3 F4 (extra lock entry): the run exits 1" [ "$RC" -ne 0 ]
check "R3 F4 (extra lock entry): refused as not a farhelm lock" \
  contains "$ERR" "not a farhelm install lock"
check "R3 F4 (extra lock entry): the stray file is untouched" \
  [ "$(cat "$INSTALL_R3F4LOCK/.farhelm-install.lock/stray")" = "unrelated data" ]

HOME_R3F4SYM="$WORKDIR/home-r3f4sym"
INSTALL_R3F4SYM="$HOME_R3F4SYM/.local/bin"
mkdir -p "$INSTALL_R3F4SYM"
ln -s "$R3_OUTSIDE" "$INSTALL_R3F4SYM/.farhelm-install.journal"
run_install "$TOOLCHAIN_FULL" "$HOME_R3F4SYM" "$INSTALL_R3F4SYM" "$BASE/good" 1.2.3
check "R3 F4 (retired journal path): the install still succeeds" [ "$RC" -eq 0 ]
check "R3 F4 (retired journal path): the planted symlink is neither followed nor removed" \
  [ -L "$INSTALL_R3F4SYM/.farhelm-install.journal" ]
check "R3 F4 (retired journal path): its target is byte-for-byte untouched" \
  [ "$(cat "$R3_OUTSIDE")" = "outside sentinel" ]

# ===========================================================================
echo
echo "== summary =="
echo "$CHECKS checks, $FAILURES failed"
if [ "$FAILURES" -ne 0 ]; then
  exit 1
fi
