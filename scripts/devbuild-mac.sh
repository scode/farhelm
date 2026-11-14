#!/usr/bin/env bash
# Build Farhelm from this working copy and run the desktop app, able to update
# remote hosts to the same build.
#
# The app is `farhelm-desktop`: the GPUI window with the helm embedded in its
# process, which starts the local supervisor from the `farhelm` beside it. Both
# are plain cargo builds; there is no app bundle, and the app runs as the bare
# binary from `target/devbuild/`, the way `crates/farhelm-desktop/README.md`
# describes running it from a build.
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
# and the runnable app in `target/devbuild/`. Nothing is installed: no app
# bundle, no ~/Applications, no ~/.local/bin. Outside the checkout, the build
# tools keep their usual caches (cargo's registry under ~/.cargo, zig's own),
# and the running app has its state, in a directory of its own:
# `~/.local/state/farhelm-devbuild` (under XDG_STATE_HOME when that is set),
# beside the installed app's `~/.local/state/farhelm`, which a devbuild never
# opens unless FARHELM_DESKTOP_STATE_DIR names it. So a devbuild has its own
# helm, host list, databases and local supervisor, and can run beside the
# installed app; see "Which state the app uses" below.
#
# ## How remote Update works with this build
#
# The helm updates a host by pushing a static Linux `farhelm` over SSH. A
# release build downloads it from its own release and checks its signature;
# this build has no release, so the script builds the Linux `farhelm` from this
# working copy and points the app at it with `FARHELM_HELM_PAYLOAD_DIR`. Files
# there are trusted as the operator's and not verified (SPEC_impl.md, the
# provisioning payload paragraphs under D2/D3).
#
# ## Requirements
#
# On the Mac: Rust through rustup, the Xcode Command Line Tools (not Xcode:
# GPUI is built with its `runtime_shaders` feature, so no Metal compiler is
# needed), and `cargo-zigbuild` and `zig` for the Linux cross-builds with the
# rustup targets `x86_64-unknown-linux-musl` and/or
# `aarch64-unknown-linux-musl`. The script checks for each before building
# anything and prints the commands that install whatever is missing.

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

# --- Which state the app uses ---------------------------------------------------
#
# The desktop app keeps everything in one state directory: the helm's
# database and host list, its own settings and window files, and the local
# supervisor it starts (socket, terminal hosts, session database). The
# installed app uses the default one, `$XDG_STATE_HOME/farhelm` or
# `~/.local/state/farhelm` (`farhelm_supervisor::default_state_dir`, whose
# rule this repeats: an XDG_STATE_HOME that is empty or relative is ignored).
#
# A devbuild uses a fixed directory of its own beside it, `farhelm-devbuild`,
# so by default it never opens the installed app's databases and never
# applies a migration from the working copy to them. Separate state means a
# separate helm with its own host list (remote hosts have to be added once in
# the devbuild) and a separate local supervisor with its own local sessions.
# The directory is fixed rather than per checkout so that the host list
# survives `cargo clean` and is shared by every checkout and every PR
# `scripts/devbuild-github.sh` builds. An explicit FARHELM_DESKTOP_STATE_DIR
# still wins, for a per-PR directory or, deliberately, the real state.
#
# Remote hosts are the one thing a devbuild still shares with the installed
# app, by design: Update on a host replaces the `farhelm` its supervisor
# runs, whichever app drives it.

if [ -n "${XDG_STATE_HOME:-}" ] && [ "${XDG_STATE_HOME#/}" != "$XDG_STATE_HOME" ]; then
  state_home=$XDG_STATE_HOME
else
  state_home="$HOME/.local/state"
fi
installed_state="$state_home/farhelm"
devbuild_state="$state_home/farhelm-devbuild"

# A path with its symlinks resolved when it exists, and as given when it does
# not yet, so that two spellings of one directory compare equal (macOS's /tmp
# is a symlink to /private/tmp).
canonical_dir() {
  (cd "$1" 2>/dev/null && pwd -P) || printf '%s\n' "$1"
}

# --- A Farhelm already running on the same state --------------------------------
#
# Two apps on one state directory do not work: the second finds the first's
# local supervisor answering and uses it instead of starting the `farhelm`
# just built, so the change under test would not even run locally, and its
# embedded helm is refused the state the first app's helm holds. Two apps on
# different state directories coexist, so the installed app and a devbuild
# can run side by side.
#
# What state a running app uses is inferred, best effort, from where its
# program lives. A devbuild launched by this script leaves `launch-state.<pid>`
# beside its program, naming the state that launch used (`launch` writes it
# under its own pid just before it execs the app, which keeps the pid). That
# file is what is trusted, because `--run-only` may start a build on other
# state than it was built for. A devbuild without one was launched by an
# older version of this script; for it the STAGED stamp's second line is the
# best guess, and an empty line there is a build from before devbuilds had
# their own state, which ran on the installed app's. Anything else is taken
# to be the installed app on the default state, which misjudges an installed
# app started with a FARHELM_DESKTOP_STATE_DIR of its own. The check fails
# open when `pgrep` is missing or cannot list processes.
#
# It runs before the build, so the answer comes before minutes of compiling,
# and again just before the launch, because another app may have been started
# while the build ran. `--build-only` launches nothing and skips it.

refuse_if_state_in_use() {
  local wanted pid exe dir theirs
  wanted=$(canonical_dir "$state")
  for pid in $(pgrep -x -U "$(id -u)" farhelm-desktop 2>/dev/null); do
    exe=$(ps -o comm= -p "$pid" 2>/dev/null) || continue
    dir=$(dirname "$exe")
    if [ -f "$dir/launch-state.$pid" ]; then
      theirs=$(sed -n 1p "$dir/launch-state.$pid")
    elif [ -f "$dir/STAGED" ]; then
      theirs=$(sed -n 2p "$dir/STAGED")
      [ -n "$theirs" ] || theirs=$installed_state
    else
      theirs=$installed_state
    fi
    if [ "$(canonical_dir "$theirs")" = "$wanted" ]; then
      cat >&2 <<EOF
Farhelm is already running on this build's state directory
($state; farhelm-desktop, pid $pid at $exe).
Quit that one first, then run this again. To run both side by side instead,
give this one separate state:
  FARHELM_DESKTOP_STATE_DIR=<empty directory> $invocation
EOF
      exit 1
    fi
  done
}

# A build restages `target/devbuild/payloads` from scratch, and a dev app
# started from this checkout earlier reads that directory whenever Update
# runs. Rebuilding under it would make its Update push the new build's Linux
# binaries to hosts it drives with the old build's helm, with nothing to
# tell the two apart (both report the development version), or pull files
# out from under an Update in progress. So every build refuses while a
# `farhelm-desktop` from this checkout's `target/devbuild` is running, even
# `--build-only`, and whatever state either of them uses. The comparison
# resolves symlinks on both sides because macOS's /tmp is one.
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
# moves it). Its second line is the state directory the build was made for,
# which `--run-only` uses unless told otherwise, and which
# `refuse_if_state_in_use` reads to tell what state a running devbuild has.
stamp="$devbuild/STAGED"

# The state this run uses: an explicit FARHELM_DESKTOP_STATE_DIR, else for
# `--run-only` the one the build was made for, else the devbuild's own. A
# build made with a per-PR directory, typically to keep a schema migration
# away from other builds, must not be started on the shared devbuild state
# just because the start command was run without the variable, hence the
# stamp. An empty recorded line is a build from before devbuilds had their
# own state; it gets the devbuild state, since separate state is never less
# safe.
if [ -n "${FARHELM_DESKTOP_STATE_DIR:-}" ]; then
  state=$FARHELM_DESKTOP_STATE_DIR
elif [ "$run_only" -eq 1 ] && [ -f "$stamp" ] && [ -n "$(sed -n 2p "$stamp")" ]; then
  state=$(sed -n 2p "$stamp")
else
  state=$devbuild_state
fi

# Starting a second copy of a build does not restage anything, so only the
# building modes need this one.
if [ "$run_only" -eq 0 ]; then
  refuse_if_this_devbuild_running
fi
if [ "$build_only" -eq 0 ]; then
  refuse_if_state_in_use
fi

# --- Run ------------------------------------------------------------------------
#
# In the foreground, so its log is this terminal and Ctrl-C quits it. The
# supervisor it starts is detached and keeps running after that, logging to
# `supervisor.log` in the state directory; the next build's first launch finds
# it running another `farhelm` file (`install_atomically` gives every build a
# new inode) and replaces it. An ambient FARHELM_DESKTOP_FARHELM would point
# the app at some other `farhelm` than the one just built, so it is removed. The
# state directory is passed explicitly every time, because the app's own
# default is the installed app's state. `exec` keeps this shell's pid for the
# app, which is what lets `launch-state.$$` name the app's state for
# `refuse_if_state_in_use`; files left by apps that have since exited are
# removed first, since their pids may be reused.
launch() {
  refuse_if_state_in_use
  local old
  for old in "$devbuild"/launch-state.*; do
    [ -e "$old" ] || continue
    kill -0 "${old##*.}" 2>/dev/null || rm -f "$old"
  done
  printf '%s\n' "$state" >"$devbuild/launch-state.$$"
  echo "== starting farhelm-desktop built from $(sed -n 1p "$stamp")"
  echo "   state: $state"
  exec env -u FARHELM_DESKTOP_FARHELM FARHELM_DESKTOP_STATE_DIR="$state" \
    FARHELM_HELM_PAYLOAD_DIR="$payloads" "$devbuild/farhelm-desktop"
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
# one is fixed and the app's build has run is a waste of that time. Each entry
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

# Cargo's bin directory goes first. It holds rustup's `cargo` and the
# `cargo-zigbuild` that `cargo install` puts there, and a shell started before
# rustup was installed may not have it on PATH yet. Being first also keeps a
# cargo from elsewhere (Homebrew's `rust`, say, which does not read the
# checkout's toolchain pin) from being the one that builds.
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
  # The caller may be outside this checkout. Read its pin here and name that
  # toolchain in the printed fixes, which the caller pastes into their shell.
  rust_toolchain=$(
    cd "$repo" || exit 1
    rustup show active-toolchain || {
      rustup toolchain install >&2 || exit 1
      rustup show active-toolchain
    }
  ) || exit 1
  rust_toolchain=${rust_toolchain%% *}
  installed_targets=$(rustup target list --installed --toolchain "$rust_toolchain") || exit 1
fi
# One musl target per Linux architecture whose payload is built. The Mac's own
# target comes with the toolchain.
for target in "${arches[@]/%/-unknown-linux-musl}"; do
  printf '%s\n' "$installed_targets" | grep -qx "$target" \
    || need "rustup target $target" "rustup target add${rust_toolchain:+ --toolchain $rust_toolchain} $target"
done

# `cargo-zigbuild --version`, not `cargo zigbuild --version`: the subcommand
# form rejects --version (seen with cargo-zigbuild 0.23.4).
cargo-zigbuild --version >/dev/null 2>&1 \
  || need "cargo-zigbuild" "cargo install --locked cargo-zigbuild"
command -v zig >/dev/null 2>&1 || need "zig" "brew install zig"

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
  # This also gives a newly installed rustup the checkout's pin before any
  # target or cargo-installed tool is requested in the pasted block.
  printf '  cd %q\n' "$repo" >&2
  for fix in "${missing_fixes[@]}"; do
    echo "  $fix" >&2
  done
  if ! command -v brew >/dev/null 2>&1 && printf '%s\n' "${missing_fixes[@]}" | grep -q '^brew '; then
    echo "(the brew commands need Homebrew: https://brew.sh)" >&2
  fi
  exit 1
fi

# Forced rather than honored: the request this script serves is that it touch
# nothing outside the checkout, and the staging below reads the binaries from
# `$repo/target`.
export CARGO_TARGET_DIR="$repo/target"
mkdir -p "$devbuild"
rm -f "$stamp"
# Taken before building, so it describes the source the build read.
built_from=$(git -C "$repo" log -1 --format='%h %s' 2>/dev/null) || built_from="an unknown commit"
if [ -n "$(git -C "$repo" status --porcelain 2>/dev/null)" ]; then
  built_from="$built_from, with uncommitted changes"
fi

# --- The desktop app and the CLIs ---------------------------------------------
#
# The desktop app is an ordinary cargo build. It is outside the workspace's
# default members, hence `-p`, and it never gets the `agent` feature: the GUI
# refuses to compile that test bridge into an optimized build
# (docs/gui-agent.md, "Why the bridge never ships").
#
# The native `farhelm` is the app's managed local supervisor and the `farhelm`
# its sessions call. The Linux ones are what Update pushes to remote hosts.
# Each package is its own cargo invocation, as in a release, so that one
# package's dependency features are not unified into the other's binary.
#
# Plain `release` profile, not the `dist` profile releases use for `farhelm`:
# the dist profile adds thin LTO, which makes every incremental rebuild relink
# the world.

echo "== building the desktop app"
(cd "$repo" && cargo build --locked --release -p farhelm-desktop)

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
# rule, `bundled_farhelm` in `crates/farhelm-desktop/src/bundle.rs`), so the
# two go in one directory and no override is needed. They stay two bare
# binaries, not an `.app`: the devbuild of the old app ran a bare binary too,
# `crates/farhelm-desktop/README.md` describes running the app that way, and
# nothing the app does needs a bundle yet. (GPUI turns its system
# notifications off outside a bundle and logs so at startup; the app does not
# use them.)
install_atomically "$CARGO_TARGET_DIR/release/farhelm-desktop" "$devbuild/farhelm-desktop"
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

printf '%s\n%s\n' "$built_from" "$state" >"$stamp"
echo "== staged in $devbuild, built from $built_from"
echo "   payloads for: ${arches[*]}"
echo "   state: $state"

# The start command is an absolute path, so it works from any directory, and
# it is the last thing printed, so it is what a reader of a long build log
# finds at the bottom. A state directory other than the devbuild's own is
# spelled out in it, even though `--run-only` would take it from the stamp
# anyway, so that the command shows which state the app will use.
if [ "$build_only" -eq 1 ]; then
  echo
  echo "Run this to start farhelm:"
  if [ "$state" != "$devbuild_state" ]; then
    printf '  FARHELM_DESKTOP_STATE_DIR=%q %q --run-only\n' "$state" "$repo/scripts/devbuild-mac.sh"
  else
    printf '  %q --run-only\n' "$repo/scripts/devbuild-mac.sh"
  fi
  exit 0
fi

launch
