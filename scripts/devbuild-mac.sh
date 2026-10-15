#!/usr/bin/env bash
# Build Farhelm from this working copy and run the desktop app, able to update
# remote hosts to the same build.
#
# NOTE: This is a developer fast path, not a release. Nothing is signed, the
# version reads `0.0.0-unreleased`, and the app's self-updater is off (it only
# runs for a release installed in ~/Applications/Farhelm.app). It is meant for
# trying a change on the Mac and on real remote hosts without waiting for a
# release workflow.
#
# Usage: scripts/devbuild-mac.sh [--arch x86_64|aarch64]... [--build-only | --run-only]
#
# With no mode it builds, then runs the app. `--build-only` stops after the
# build and prints the command that starts the result; `--run-only` is that
# command, and starts what an earlier build staged without building anything.
# `scripts/devbuild-github.sh` uses the pair to hand a built PR or branch to
# the maintainer.
#
# `--arch` picks which Linux host architectures to build payloads for. It may
# be given more than once, and defaults to both. A host whose architecture was
# not built cannot be updated from this run: its payload is simply absent, and
# the helm refuses Update for that host rather than pushing anything stale.
#
# ## What it touches
#
# Builds land in this checkout's `target/` (CARGO_TARGET_DIR is forced to it),
# the runnable app in `target/devbuild/`, and the tmux payloads it downloads in
# `target/devbuild/tmux/`. Nothing is installed: no app bundle, no
# ~/Applications, no ~/.local/bin. Outside the checkout, the build tools keep
# their usual caches (cargo's registry under ~/.cargo, dx's and zig's own),
# and the running app has its state: like any Farhelm desktop app it uses the default state
# directory (or `FARHELM_DESKTOP_STATE_DIR`), so it sees the same hosts and the
# same local supervisor as an installed app. Run one or the other, not both;
# the script refuses to launch while another Farhelm app is running.
#
# ## How remote Update works with this build
#
# The helm updates a host by pushing a static Linux `farhelm` and a static tmux
# over SSH. A release build downloads those from its own release and checks
# their signatures; this build has no release, so the script builds the Linux
# `farhelm` from this working copy and points the app at it with
# `FARHELM_HELM_PAYLOAD_DIR`. Files there are trusted as the operator's and not
# verified (SPEC_impl.md, the provisioning payload paragraphs under D2/D3). The
# tmux payloads come from the latest stable release, as long as it pins the
# same tmux version as the working copy (see "tmux payloads" below), fetched
# once and checked against its SHA256SUMS over TLS (the same trust a first
# `curl | sh` install has, not the signature).
#
# ## Requirements
#
# On the Mac: Rust through rustup, the Xcode Command Line Tools, `dx` (the
# Dioxus CLI version the repo pins, 0.7.10), the rustup target
# `wasm32-unknown-unknown` for the web UI, `cargo-zigbuild` and `zig` for the
# Linux cross-builds with the rustup targets `x86_64-unknown-linux-musl`
# and/or `aarch64-unknown-linux-musl`, and, to run the result, tmux. The
# script checks for each before building anything and prints the commands
# that install whatever is missing.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)

# Kept for the hint that tells the user how to rerun this exact command.
invocation="$0${*:+ $*}"

usage() {
  sed -n 's/^# \{0,1\}//; /^Usage:/,/^$/p' "$0" | sed '/^$/d'
}

# --- Arguments --------------------------------------------------------------

arches=()
build_only=0
run_only=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --arch)
      [ "$#" -ge 2 ] || {
        echo "--arch needs a value: x86_64 or aarch64" >&2
        exit 2
      }
      case "$2" in
        x86_64 | aarch64) arches+=("$2") ;;
        *)
          echo "unknown --arch $2; expected x86_64 or aarch64" >&2
          exit 2
          ;;
      esac
      shift 2
      ;;
    --build-only)
      build_only=1
      shift
      ;;
    --run-only)
      run_only=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done
if [ "$build_only" -eq 1 ] && [ "$run_only" -eq 1 ]; then
  echo "--build-only and --run-only exclude each other" >&2
  exit 2
fi
# Payloads are staged by a build, so an architecture choice at run time could
# only be silently ignored.
if [ "$run_only" -eq 1 ] && [ "${#arches[@]}" -gt 0 ]; then
  echo "--arch has no effect with --run-only; the payloads come from the last build" >&2
  exit 2
fi
if [ "${#arches[@]}" -eq 0 ]; then
  arches=(x86_64 aarch64)
fi

if [ "$(uname -s)" != Darwin ]; then
  echo "devbuild-mac.sh builds and runs the macOS desktop app; run it on a Mac" >&2
  exit 1
fi

# --- A Farhelm that is already running --------------------------------------
#
# This build shares the default state directory with an installed Farhelm:
# the same hosts, the same local supervisor, the same databases. Started
# beside a running app, this one finds that app's local supervisor answering
# and uses it instead of starting the `farhelm` just built, so the change
# under test would not even be running locally (and its embedded helm would
# then be refused the state directory the running app's helm holds). So
# refuse to start while another one is running. It is best effort by design: it looks for a
# `farhelm-desktop` process of this user's, which covers the installed app
# and an earlier run of this script, and it fails open when `pgrep` is
# missing or cannot list processes. An explicit FARHELM_DESKTOP_STATE_DIR
# means the caller chose separate state, and two apps on separate state
# directories coexist, so the check stands down.
#
# It runs before the build, so the answer comes before minutes of compiling,
# and again just before the launch, because the installed app may have been
# started while the build ran. `--build-only` launches nothing and skips it.

refuse_if_farhelm_running() {
  [ -z "${FARHELM_DESKTOP_STATE_DIR:-}" ] || return 0
  local pids
  pids=$(pgrep -x -U "$(id -u)" farhelm-desktop 2>/dev/null) || return 0
  cat >&2 <<EOF
Farhelm is already running (farhelm-desktop, pid $(echo "$pids" | tr '\n' ' ' | sed 's/ $//')).
This build uses the same state, hosts and local supervisor as an installed
Farhelm, so quit that one first, then run this again. To run both side by
side instead, give this one separate state:
  FARHELM_DESKTOP_STATE_DIR=<empty directory> $invocation
EOF
  exit 1
}

# A build restages `target/devbuild/payloads` from scratch, and a dev app
# started from this checkout earlier reads that directory whenever Update
# runs. Rebuilding under it would make its Update push the new build's Linux
# binaries to hosts it drives with the old build's helm, with nothing to
# tell the two apart (both report the development version), or pull files
# out from under an Update in progress. So every build refuses while a
# `farhelm-desktop` from this checkout's `target/devbuild` is running, even
# `--build-only`, and even with FARHELM_DESKTOP_STATE_DIR set, which turns off
# only the shared-state check above. The comparison resolves symlinks on
# both sides because macOS's /tmp is one.
refuse_if_this_devbuild_running() {
  local devbuild_real pid exe
  devbuild_real=$(cd "$repo/target/devbuild" 2>/dev/null && pwd -P) || return 0
  for pid in $(pgrep -x -U "$(id -u)" farhelm-desktop 2>/dev/null); do
    exe=$(ps -o comm= -p "$pid" 2>/dev/null) || continue
    if [ "$(cd "$(dirname "$exe")" 2>/dev/null && pwd -P)" = "$devbuild_real" ]; then
      echo "a Farhelm built here earlier is still running (pid $pid); quit it before rebuilding, since it would pick up the new build's payloads" >&2
      exit 1
    fi
  done
}

# The state directory is recorded with the build and spelled out in the start
# command, which may run from anywhere, so a relative one would name a
# different directory then. An empty one is unset here because this script
# already treats it as unset, and the app would not.
if [ -n "${FARHELM_DESKTOP_STATE_DIR+set}" ] && [ -z "$FARHELM_DESKTOP_STATE_DIR" ]; then
  unset FARHELM_DESKTOP_STATE_DIR
fi
case "${FARHELM_DESKTOP_STATE_DIR:-/}" in
  /*) ;;
  *)
    echo "FARHELM_DESKTOP_STATE_DIR must be an absolute path, got '$FARHELM_DESKTOP_STATE_DIR'" >&2
    exit 2
    ;;
esac

devbuild="$repo/target/devbuild"
payloads="$devbuild/payloads"
# Written last by a build that staged everything, removed first by the next
# one, so its presence means `target/devbuild` holds one complete build. Its
# first line names the commit that build came from, which `--run-only`
# repeats, since the checkout may have moved on since (devbuild-github.sh
# moves it). Its second line is the FARHELM_DESKTOP_STATE_DIR the build ran
# with, usually empty.
stamp="$devbuild/STAGED"

# A build made with a separate state directory, typically to keep a schema
# migration away from the real state, must not be started on the real state
# just because the start command was run without the variable. So when
# `--run-only` gets no FARHELM_DESKTOP_STATE_DIR, it takes the build's. An
# explicit one still wins: separate state is never less safe.
if [ "$run_only" -eq 1 ] && [ -z "${FARHELM_DESKTOP_STATE_DIR:-}" ] && [ -f "$stamp" ]; then
  recorded_state=$(sed -n 2p "$stamp")
  if [ -n "$recorded_state" ]; then
    export FARHELM_DESKTOP_STATE_DIR="$recorded_state"
    echo "== using the state directory this build was made with: $recorded_state"
  fi
fi

# Starting a second copy of a build does not restage anything, so only the
# building modes need this one.
if [ "$run_only" -eq 0 ]; then
  refuse_if_this_devbuild_running
fi
if [ "$build_only" -eq 0 ]; then
  refuse_if_farhelm_running
fi

# --- Run ------------------------------------------------------------------------
#
# In the foreground, so its log is this terminal and Ctrl-C quits it (taking the
# supervisor it started with it). An ambient FARHELM_DESKTOP_FARHELM would point
# the app at some other `farhelm` than the one just built, so it is removed.
launch() {
  refuse_if_farhelm_running
  echo "== starting farhelm-desktop built from $(sed -n 1p "$stamp")"
  exec env -u FARHELM_DESKTOP_FARHELM FARHELM_HELM_PAYLOAD_DIR="$payloads" "$devbuild/farhelm-desktop"
}

# `--run-only` needs none of the build tools checked below, so it leaves
# before they are.
if [ "$run_only" -eq 1 ]; then
  if ! [ -f "$stamp" ]; then
    echo "no complete build is staged in $devbuild; build it first with $0 --build-only" >&2
    exit 1
  fi
  launch
fi

# --- Preflight ----------------------------------------------------------------
#
# Every missing tool is reported at once, before any build starts, because the
# builds take minutes and finding the second missing tool only after the first
# one is fixed and the web build has run is a waste of that time. Each entry
# pairs what is missing with the command that installs it, and the commands
# are printed together, in dependency order, so they can be pasted as a block.
# Nothing in the block may stop to ask a question, because the lines pasted
# after it would become the answer; hence rustup's `-y`. The Xcode Command
# Line Tools are the exception: `xcode-select --install` only opens an
# installer window and returns, and everything else needs them, so when they
# are missing that is the only step printed.

# An empty name adds an install step that is not a prerequisite of its own.
missing_names=()
missing_fixes=()
need() {
  missing_names+=("$1")
  missing_fixes+=("$2")
}

# Cargo's bin directory goes first because both cargo-installed tools here
# have namesakes: Homebrew's deno installs its own `dx`, and when Homebrew's
# bin precedes ~/.cargo/bin on PATH, a bare `dx` runs deno instead of the
# Dioxus CLI. The version check still runs, so a `dx` from somewhere else, or
# a Dioxus CLI other than the pinned one, is reported rather than used.
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"

# The C compiler and linker every native build needs.
have_clt=1
xcode-select -p >/dev/null 2>&1 || {
  have_clt=0
  need "Xcode Command Line Tools" "xcode-select --install"
}

have_rust=1
if ! command -v cargo >/dev/null 2>&1 || ! command -v rustup >/dev/null 2>&1; then
  have_rust=0
  # The second command puts cargo on the current shell's PATH, so the cargo
  # installs printed after it work when the block is pasted in one go.
  need "Rust, through rustup" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
  # shellcheck disable=SC2016 # printed for the user's shell to expand
  need "" '. "$HOME/.cargo/env"'
fi

# Without rustup there is no target list to read; every target is then
# reported missing, which is true once rustup is installed.
installed_targets=""
if [ "$have_rust" -eq 1 ]; then
  installed_targets=$(rustup target list --installed)
fi
# wasm32 for the web UI dx builds into the desktop app; one musl target per
# Linux architecture whose payload is built.
for target in wasm32-unknown-unknown "${arches[@]/%/-unknown-linux-musl}"; do
  printf '%s\n' "$installed_targets" | grep -qx "$target" \
    || need "rustup target $target" "rustup target add $target"
done

dx_version=0.7.10
case "$(dx --version 2>/dev/null)" in
  "dioxus $dx_version"*) ;;
  *) need "dx $dx_version, the Dioxus CLI" "cargo install --locked dioxus-cli --version $dx_version" ;;
esac
# `cargo-zigbuild --version`, not `cargo zigbuild --version`: the subcommand
# form rejects --version (seen with cargo-zigbuild 0.23.4).
cargo-zigbuild --version >/dev/null 2>&1 \
  || need "cargo-zigbuild" "cargo install --locked cargo-zigbuild"
command -v zig >/dev/null 2>&1 || need "zig" "brew install zig"

# Not a build tool: the app's local supervisor runs sessions in tmux, and the
# app refuses to start without one. Checked here so that a build is not
# wasted on a machine that cannot run it. The directories are the ones the
# app itself searches besides PATH (`MACOS_TMUX_PREFIXES`), since an app
# started from Finder has no shell PATH.
if ! command -v tmux >/dev/null 2>&1 \
  && ! [ -x /opt/homebrew/bin/tmux ] && ! [ -x /usr/local/bin/tmux ] && ! [ -x /opt/local/bin/tmux ]; then
  need "tmux" "brew install tmux"
fi

if [ "${#missing_names[@]}" -gt 0 ]; then
  echo "missing prerequisites:" >&2
  for item in "${missing_names[@]}"; do
    [ -z "$item" ] || echo "  - $item" >&2
  done
  if [ "$have_clt" -eq 0 ]; then
    echo "install the Xcode Command Line Tools first, with the window this opens:" >&2
    echo "  xcode-select --install" >&2
    echo "then run this script again for the commands that install the rest" >&2
    exit 1
  fi
  echo "install them with:" >&2
  for fix in "${missing_fixes[@]}"; do
    echo "  $fix" >&2
  done
  if ! command -v brew >/dev/null 2>&1 && printf '%s\n' "${missing_fixes[@]}" | grep -q '^brew '; then
    echo "(the brew commands need Homebrew: https://brew.sh)" >&2
  fi
  exit 1
fi

# macOS has `shasum`, Linux has `sha256sum`; both print `<hex>  <name>`.
if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
else
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi

# Forced rather than honored: the request this script serves is that it touch
# nothing outside the checkout, and `build-desktop-binary.sh` reads dx's output
# from `$repo/target` regardless of CARGO_TARGET_DIR.
export CARGO_TARGET_DIR="$repo/target"
mkdir -p "$devbuild"
rm -f "$stamp"
# Taken before building, so it describes the source the build read.
built_from=$(git -C "$repo" log -1 --format='%h %s' 2>/dev/null) || built_from="an unknown commit"
if [ -n "$(git -C "$repo" status --porcelain 2>/dev/null)" ]; then
  built_from="$built_from, with uncommitted changes"
fi

# --- The web UI and the desktop app -------------------------------------------
#
# The same two dx builds the release makes, in the order it makes them; see
# crates/farhelm-desktop/README.md for why a plain `cargo build` of the desktop
# app renders an empty window. `target/dx` is wiped first because dx keeps
# every generation it has built there, and a stale web bundle embeds silently.

echo "== building the web UI"
rm -rf "$CARGO_TARGET_DIR/dx"
(cd "$repo/crates/farhelm-ui" && dx build --package farhelm-ui --platform web --release)

# Exported for every build below, not just the desktop one. `farhelm-helm`'s
# build script reruns whenever this variable changes, so building the CLI
# without it would recompile the helm on every alternation between the two
# builds. It also makes the CLI release-shaped, as the released one is.
export FARHELM_UI_DIST="$CARGO_TARGET_DIR/dx/farhelm-ui/release/web/public"

echo "== building the desktop app"
"$repo/scripts/build-desktop-binary.sh"

# --- The CLIs -----------------------------------------------------------------
#
# The native one is the app's managed local supervisor and the `farhelm` its
# sessions call. The Linux ones are what Update pushes to remote hosts. Plain
# `release` profile, not the `dist` profile releases use: the dist profile adds
# thin LTO, which makes every incremental rebuild relink the world.

echo "== building farhelm for this machine"
(cd "$repo" && cargo build --locked --release -p farhelm)

for arch in "${arches[@]}"; do
  echo "== building farhelm for $arch-unknown-linux-musl"
  (cd "$repo" && cargo zigbuild --locked --release -p farhelm --target "$arch-unknown-linux-musl")
done

# --- Staging ------------------------------------------------------------------

# Copy beside the destination and rename over it, so the destination is a new
# inode. Overwriting a binary in place on macOS can get later executions of it
# killed (the kernel caches code signatures per vnode), and a supervisor started
# by an earlier run of this script may still be executing the old file.
install_atomically() {
  local src=$1 dest=$2
  cp "$src" "$dest.tmp.$$"
  chmod 0755 "$dest.tmp.$$"
  mv -f "$dest.tmp.$$" "$dest"
}

# The app starts its supervisor from the `farhelm` beside it (the "sibling"
# rule in `crates/farhelm-ui/src/desktop/bundle.rs`), so the two go in one
# directory and no override is needed.
install_atomically "$CARGO_TARGET_DIR/dist-desktop/farhelm-desktop" "$devbuild/farhelm-desktop"
install_atomically "$CARGO_TARGET_DIR/release/farhelm" "$devbuild/farhelm"

# The payload directory is rebuilt from scratch each run. Leaving an earlier
# run's archive for an architecture not built this time would let Update push
# that older build to such a host without any sign that it was stale.
rm -rf "$payloads"
mkdir -p "$payloads"

# The helm extracts the archive member whose file name is `farhelm`
# (`assets::farhelm_archive_for` and `extract_single_member`), so each archive
# holds exactly that one file. COPYFILE_DISABLE and --no-xattrs keep macOS's tar
# from adding `._farhelm` AppleDouble entries and extended-attribute headers.
for arch in "${arches[@]}"; do
  target="$arch-unknown-linux-musl"
  binary="$CARGO_TARGET_DIR/$target/release/farhelm"
  # check-static-elf.sh needs `readelf`, which macOS does not ship, so on a
  # Mac without binutils it falls back to `file`, which macOS does ship and
  # which says enough to catch a dynamically linked or wrong-machine binary.
  if command -v readelf >/dev/null 2>&1; then
    "$repo/scripts/check-static-elf.sh" "$binary" "$target"
  else
    case "$arch" in
      x86_64) machine="x86-64" ;;
      aarch64) machine="ARM aarch64" ;;
    esac
    # Static PIE is as good as a plain static executable here, and
    # check-static-elf.sh accepts it too; `file` words it differently.
    description=$(file -b "$binary")
    case "$description" in
      "ELF 64-bit LSB executable, $machine,"*"statically linked"*) ;;
      "ELF 64-bit LSB pie executable, $machine,"*"static-pie linked"*) ;;
      *)
        echo "$binary is not a static $arch Linux executable: $description" >&2
        exit 1
        ;;
    esac
  fi
  COPYFILE_DISABLE=1 tar --no-xattrs -czf "$payloads/farhelm-$target.tar.gz" \
    -C "$(dirname "$binary")" farhelm
done

# --- tmux payloads ------------------------------------------------------------
#
# tmux is not built here: the static tmux of the latest stable release is
# staged instead, which is only right while that release pins the same tmux
# version as this working copy. The supervisor refuses a tmux older than the
# floor its build expects, so pushing a mismatched one could leave a host
# without a working supervisor until it is updated again. The script
# therefore compares the two pins (TMUX_VERSION in
# .github/release/source-pins.env) and refuses on a mismatch rather than
# stage a tmux this build may reject.
#
# Downloads are cached under target/devbuild/tmux/<tmux version>/, so after
# the first run this needs no network until the working copy's pin changes.

tmux_version=$(sed -n 's/^TMUX_VERSION=//p' "$repo/.github/release/source-pins.env")
case "$tmux_version" in
  '' | *[!0-9A-Za-z.]* | *..*)
    echo "could not read a tmux version from .github/release/source-pins.env (got '$tmux_version')" >&2
    exit 1
    ;;
esac
tmux_cache="$devbuild/tmux/$tmux_version"
release_tag=""
for arch in "${arches[@]}"; do
  name="tmux-$arch-unknown-linux-musl"
  if ! [ -f "$tmux_cache/$name" ]; then
    if [ -z "$release_tag" ]; then
      release_tag=$(curl -fsSL --proto '=https' https://get.farhelm.io/latest)
      # The tag goes into URLs below, so anything but a plain version (a `/`
      # or `..` especially) is refused rather than trusted.
      case "$release_tag" in
        v[0-9]*) ;;
        *) release_tag="invalid: $release_tag" ;;
      esac
      case "$release_tag" in
        *[!0-9A-Za-z.-]* | *..*)
          echo "https://get.farhelm.io/latest returned an unexpected value: $release_tag" >&2
          exit 1
          ;;
      esac
      release_tmux=$(curl -fsSL --proto '=https' \
        "https://raw.githubusercontent.com/scode/farhelm/$release_tag/.github/release/source-pins.env" \
        | sed -n 's/^TMUX_VERSION=//p')
      if [ "$release_tmux" != "$tmux_version" ]; then
        echo "this working copy pins tmux $tmux_version, but release $release_tag ships tmux '${release_tmux:-unknown}';" >&2
        echo "staging that tmux could leave updated hosts with a tmux their new supervisor refuses, so stopping here" >&2
        exit 1
      fi
    fi
    echo "== downloading $name (tmux $tmux_version) from release $release_tag"
    mkdir -p "$tmux_cache"
    curl -fsSL --proto '=https' -o "$tmux_cache/SHA256SUMS.tmp" "https://get.farhelm.io/$release_tag/SHA256SUMS"
    curl -fsSL --proto '=https' -o "$tmux_cache/$name.tmp" "https://get.farhelm.io/$release_tag/$name"
    expected=$(awk -v n="$name" '$2 == n { print $1 }' "$tmux_cache/SHA256SUMS.tmp")
    actual=$(sha256 "$tmux_cache/$name.tmp")
    rm -f "$tmux_cache/SHA256SUMS.tmp"
    if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
      echo "$name from $release_tag does not match its SHA256SUMS entry (expected '$expected', got '$actual')" >&2
      rm -f "$tmux_cache/$name.tmp"
      exit 1
    fi
    mv -f "$tmux_cache/$name.tmp" "$tmux_cache/$name"
  fi
  cp "$tmux_cache/$name" "$payloads/$name"
  chmod 0755 "$payloads/$name"
done

printf '%s\n%s\n' "$built_from" "${FARHELM_DESKTOP_STATE_DIR:-}" >"$stamp"
echo "== staged in $devbuild, built from $built_from"
echo "   payloads for: ${arches[*]}"

# The start command is an absolute path, so it works from any directory, and
# it is the last thing printed, so it is what a reader of a long build log
# finds at the bottom. A separate state directory is spelled out in it, even
# though `--run-only` would take it from the stamp anyway, so that the
# command shows which state the app will use.
if [ "$build_only" -eq 1 ]; then
  echo
  echo "Run this to start farhelm:"
  if [ -n "${FARHELM_DESKTOP_STATE_DIR:-}" ]; then
    printf '  FARHELM_DESKTOP_STATE_DIR=%q %q --run-only\n' "$FARHELM_DESKTOP_STATE_DIR" "$repo/scripts/devbuild-mac.sh"
  else
    printf '  %q --run-only\n' "$repo/scripts/devbuild-mac.sh"
  fi
  exit 0
fi

launch
