#!/usr/bin/env bash
# Build the two static tmux binaries a release publishes (D5) and leave them
# where cargo-dist collects them.
#
# tmux is a release asset because provisioning installs it on hosts whose own
# tmux is below the version floor: the helm downloads
# `tmux-<target>` beside the `farhelm` archives and pushes it over SSH. It is
# published unarchived, one bare binary per Linux architecture, which is why
# it is an `extra-artifact` rather than part of any package's archive — see
# `crates/farhelm/Cargo.toml` for that declaration and the working-directory
# rule it depends on.
#
# ## Why this provisions its own toolchain
#
# dist builds extra artifacts in the GLOBAL job, and `github-build-setup`
# steps are spliced only into the per-target build jobs. That job does run
# things first — it checks out, installs the cached dist executable, and
# downloads the local build artifacts — but none of them is OURS: no step
# supplies zig, a Python environment, or anything else this build needs. So it
# installs the pinned zig itself, through the same checksummed `ziglang` wheel
# the retired hand-written release workflow used, instead of inheriting one.
#
# Both binaries are cross-compiled here regardless of the runner's own
# architecture: zig is the C toolchain, so the aarch64 build does not need an
# aarch64 runner. Nothing is cached — this runs a handful of times a year, and
# a stale cache of a security-relevant binary is worth less than the ten
# minutes it costs to rebuild.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
out_dir="$repo/target/tmux-assets"
python_env="$repo/target/tmux-assets-python"

cd "$repo"
mkdir -p "$out_dir"

# `--require-hashes` makes this an exact-artifact install: pip refuses the
# whole requirements file if any hash is missing or wrong, so a compromised
# index cannot substitute a different zig.
echo "== installing the pinned zig toolchain"
rm -rf "$python_env"
python3 -m venv "$python_env"
"$python_env/bin/pip" install --quiet --require-hashes -r .github/release/ziglang-requirements.txt
ZIG=$("$python_env/bin/python" -c 'import pathlib, ziglang; print(pathlib.Path(ziglang.__file__).parent / "zig")')
export ZIG
test -x "$ZIG"

for target in x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do
  echo "== building tmux for $target"
  scripts/build-private-tmux.sh "$target" "$out_dir/tmux-$target"
done

# The producer-side half of the same verdict the release signing gate makes on
# the published bytes. It catches the two failures that would otherwise reach a
# host and fail there: a binary that needs a dynamic loader (the destination is
# any distro, possibly with no matching libc) and a binary built for the wrong
# architecture (the two targets differ only by a build-script argument, so a
# mix-up is a plausible edit away and invisible in the file name).
#
# Delegated to `check-static-elf.sh` rather than open-coded here, because the
# signing gate needs the identical assertion and two copies would drift. That
# script also treats a readelf that could not READ the payload as a refusal;
# the `if readelf … | grep -q` shape this replaced reported such a payload as
# static, since bash exempts an `if` condition from errexit.
for target in x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do
  scripts/check-static-elf.sh "$out_dir/tmux-$target" "$target"
done
