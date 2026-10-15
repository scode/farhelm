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
# Usage: scripts/devbuild-mac.sh [--arch x86_64|aarch64]... [--build-only]
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
# same local supervisor as an installed app. Run one or the other, not both.
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
# On the Mac: `dx` (the Dioxus CLI version the repo pins, 0.7.10), `cargo-zigbuild`
# and `zig` for the Linux cross-builds (`cargo install --locked
# cargo-zigbuild`, `brew install zig`), and the rustup targets
# `x86_64-unknown-linux-musl` and/or `aarch64-unknown-linux-musl`. The script
# checks for each and says what is missing before building anything.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)

usage() {
  sed -n 's/^# \{0,1\}//; /^Usage:/,/^$/p' "$0" | sed '/^$/d'
}

# --- Arguments --------------------------------------------------------------

arches=()
build_only=0
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
if [ "${#arches[@]}" -eq 0 ]; then
  arches=(x86_64 aarch64)
fi

# --- Preflight ----------------------------------------------------------------
#
# Every missing tool is reported at once, before any build starts, because the
# builds take minutes and finding the second missing tool only after the first
# one is fixed and the web build has run is a waste of that time.

missing=()

# Cargo's bin directory goes first because both cargo-installed tools here
# have namesakes: Homebrew's deno installs its own `dx`, and when Homebrew's
# bin precedes ~/.cargo/bin on PATH, a bare `dx` runs deno instead of the
# Dioxus CLI. The version check still runs, so a `dx` from somewhere else, or
# a Dioxus CLI other than the pinned one, is reported rather than used.
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
dx_version=0.7.10
case "$(dx --version 2>/dev/null)" in
  "dioxus $dx_version"*) ;;
  *) missing+=("dx $dx_version, the Dioxus CLI (cargo install --locked dioxus-cli --version $dx_version)") ;;
esac
command -v zig >/dev/null 2>&1 || missing+=("zig (brew install zig)")
# `cargo-zigbuild --version`, not `cargo zigbuild --version`: the subcommand
# form rejects --version (seen with cargo-zigbuild 0.23.4).
cargo-zigbuild --version >/dev/null 2>&1 || missing+=("cargo-zigbuild (cargo install --locked cargo-zigbuild)")
command -v curl >/dev/null 2>&1 || missing+=("curl")
installed_targets=$(rustup target list --installed)
for arch in "${arches[@]}"; do
  if ! printf '%s\n' "$installed_targets" | grep -qx "$arch-unknown-linux-musl"; then
    missing+=("rustup target $arch-unknown-linux-musl (rustup target add $arch-unknown-linux-musl)")
  fi
done
if [ "${#missing[@]}" -gt 0 ]; then
  echo "missing prerequisites:" >&2
  for item in "${missing[@]}"; do
    echo "  $item" >&2
  done
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
devbuild="$CARGO_TARGET_DIR/devbuild"
mkdir -p "$devbuild"

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
payloads="$devbuild/payloads"
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
    description=$(file -b "$binary")
    case "$description" in
      "ELF 64-bit LSB executable, $machine,"*"statically linked"*) ;;
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

echo "== staged in $devbuild"
echo "   payloads for: ${arches[*]}"

if [ "$build_only" -eq 1 ]; then
  echo "   run it with: FARHELM_HELM_PAYLOAD_DIR=$payloads $devbuild/farhelm-desktop"
  exit 0
fi

# --- Run ------------------------------------------------------------------------
#
# In the foreground, so its log is this terminal and Ctrl-C quits it (taking the
# supervisor it started with it). An ambient FARHELM_DESKTOP_FARHELM would point
# the app at some other `farhelm` than the one just built, so it is removed.
echo "== starting farhelm-desktop"
exec env -u FARHELM_DESKTOP_FARHELM FARHELM_HELM_PAYLOAD_DIR="$payloads" "$devbuild/farhelm-desktop"
