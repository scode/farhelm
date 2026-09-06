#!/usr/bin/env bash
# Build the release `farhelm-desktop` binary and hand it to cargo-dist.
#
# ## Why this is not a cargo build
#
# `farhelm-desktop`'s `asset!()` paths are written into the binary by dx AFTER
# the link: manganis emits a placeholder into a `__ASSETS__` section and dx
# scans the built file and overwrites each one with the real content-hashed
# name. rustc never learns them. A binary from a plain `cargo build` therefore
# asks for the same nonsensical placeholder path for every asset and renders
# an empty window — see `crates/farhelm-desktop/README.md` and
# `scripts/check-desktop-assets.sh` for the long version.
#
# cargo-dist has no post-build hook, so the way to get a dx-built binary into
# a dist-managed archive is to declare the shell as a GENERIC package whose
# `build-command` is this script (`packaging/farhelm-desktop/dist.toml`). dist
# runs it with the package directory as the working directory, then collects
# `farhelm-desktop` from the package's `out-dir`.
#
# ## What this does NOT do
#
# It does not build the web bundle and it does not wipe `target/dx`: the
# release workflow's build setup (`.github/dist-build-setup.yml`) already did
# both, and `FARHELM_UI_DIST` points into the tree it produced. A
# `rm -rf target/dx` here would delete the very bundle this build has to embed.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
out_dir="$repo/target/dist-desktop"
dx_root="$repo/target/dx/farhelm-desktop/release"

# A desktop binary without the embedded UI tree is a window that 404s every
# asset request (D12), and it would also be missing D13's release marker. That
# is a supported DEVELOPER build and an unshippable release artifact, so this
# refuses rather than quietly producing one.
: "${FARHELM_UI_DIST:?FARHELM_UI_DIST must point at a dx-built web bundle; a release desktop binary embeds it}"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cd "$repo"

# `--package` names the crate to build. It is not optional here for a reason
# specific to this crate: `farhelm-desktop` is EXCLUDED from the workspace's
# `default-members` (so ordinary builds never compile WebKit), so a selection
# left to dx would pick something else entirely. `--platform desktop` resolves
# to the host's native bundle format, making this a native build on whatever
# runner dist scheduled it on — there is no cross-compilation path here.
echo "== building farhelm-desktop with dx"
dx build --package farhelm-desktop --platform desktop --release

# dx nests the executable differently per platform: macOS gets an `.app`
# bundle (named by PascalCasing the binary name), Linux gets a plain `app/`
# directory. Both known layouts are named explicitly so the common case fails
# with a useful message rather than a mystery, and the search below is the
# fallback for a dx that has moved things again. `.dSYM` is excluded because
# macOS debug bundles contain a DWARF file with the same name as the binary.
binary=""
for candidate in \
  "$dx_root/macos/FarhelmDesktop.app/Contents/MacOS/farhelm-desktop" \
  "$dx_root/linux/app/farhelm-desktop"; do
  if [ -f "$candidate" ]; then
    binary=$candidate
    break
  fi
done

if [ -z "$binary" ]; then
  # A read loop rather than `mapfile`: this runs on macOS runners, where
  # `/bin/bash` is still 3.2 and has no such builtin.
  found=()
  while IFS= read -r candidate; do
    found+=("$candidate")
  done < <(find "$dx_root" -type f -name farhelm-desktop -not -path '*.dSYM*')
  if [ "${#found[@]}" -ne 1 ]; then
    echo "expected exactly one farhelm-desktop under $dx_root, found ${#found[@]}" >&2
    # `${a[@]+"${a[@]}"}` because expanding an EMPTY array under `set -u` is an
    # error in bash 3.2, which is the bash a macOS runner has.
    for path in ${found[@]+"${found[@]}"}; do
      echo "  $path" >&2
    done
    exit 1
  fi
  binary=${found[0]}
  echo "== dx layout changed; using $binary"
fi

# The whole reason this script exists, asserted on the artifact itself: ask the
# binary what assets it will request and refuse one still carrying manganis's
# placeholder. `CARGO_MANIFEST_DIR` is scrubbed because `Asset::resolve`
# consults it at RUNTIME and would resolve every asset to its absolute source
# path, hiding exactly the failure this checks for.
#
# Captured into a variable rather than piped into `grep` so that a binary that
# fails to run at all fails this script. Inside an `if`, a broken pipeline is
# just a false condition, and the check would pass by saying nothing.
env -u CARGO_MANIFEST_DIR "$binary" --print-assets | sort -u >"$work/requested"
if [ ! -s "$work/requested" ]; then
  echo "$binary reported no assets at all; something is wrong with the build" >&2
  exit 1
fi
if grep -q 'replaced by dx' "$work/requested"; then
  echo "$binary carries placeholder asset names; dx did not process it" >&2
  exit 1
fi

# Rewritten names are necessary but not sufficient: they also have to name
# files that EXIST in the bundle this binary embedded. `check-desktop-assets.sh`
# owns that comparison — it is the shared parity verdict, in both
# directions — and this calls it rather than growing a second, weaker copy.
# This is the shipped-artifact parity check. It remains necessary even though
# ordinary CI no longer pays for a separate dx asset rebuild: a tag can point
# at a commit that never passed PR CI, and this compares the exact bytes about
# to be archived.
echo "== comparing requested assets against the embedded bundle"
"$repo/scripts/check-desktop-assets.sh" --compare "$FARHELM_UI_DIST" "$work/requested"

mkdir -p "$out_dir"
install -m 0755 "$binary" "$out_dir/farhelm-desktop"
echo "== wrote $out_dir/farhelm-desktop"

# The app icon ships in the archive next to the binary so `install.sh` can
# assemble `Farhelm.app` from checksummed release files. It is staged here and
# declared in `binaries` because that is the one collection path dist 0.32
# honors for a generic package — see the `binaries` comment in
# `packaging/farhelm-desktop/dist.toml` for why `include` was not an option.
install -m 0644 "$repo/packaging/farhelm-desktop/Farhelm.icns" "$out_dir/Farhelm.icns"
echo "== wrote $out_dir/Farhelm.icns"
