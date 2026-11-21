#!/usr/bin/env bash
# Cargo-dist can compute a manifest from whichever build outputs are visible.
# Require each expected input before it can turn a partial listing into a release.
set -euo pipefail

directory=$1
phase=$2
archive_list=${RELEASE_ARCHIVES:?release archive names are required}
# YAML already terminates its block with a newline. Pass its bytes through
# so Bash does not append another newline and invent an empty archive name.
mapfile -t archives < <(printf '%s' "$archive_list")

require_file() {
  if [[ ! -f "$1" || ! -s "$1" || -L "$1" ]]; then
    printf 'Missing or invalid release input: %s\n' "$1" >&2
    exit 1
  fi
}

require_file "$directory/plan-dist-manifest.json"
for archive in "${archives[@]}"; do
  target=${archive%.tar.gz}
  target=${target#farhelm-}
  target=${target#desktop-}
  require_file "$directory/$target-dist-manifest.json"
  require_file "$directory/$archive"
  require_file "$directory/$archive.sha256"
done

case "$phase" in
  local) ;;
  global|publish)
    require_file "$directory/global-dist-manifest.json"
    require_file "$directory/sha256.sum"
    ;;
  *)
    printf 'Unknown release artifact phase: %s\n' "$phase" >&2
    exit 2
    ;;
esac

if [[ "$phase" == publish ]]; then
  require_file dist-manifest.json
  # Granular manifests are build inputs. The final manifest is already on this
  # runner and can go straight to the release without an Actions round trip.
  mkdir release-assets
  cp -- dist-manifest.json release-assets/
  for asset in "$directory"/*; do
    case "${asset##*/}" in
      *-dist-manifest.json|dist-manifest.json) ;;
      *) cp -- "$asset" release-assets/ ;;
    esac
  done

  # Keep the allowlist ahead of the write to GitHub. A new dist output belongs
  # here only once the release contract explicitly accounts for it.
  {
    printf '%s\n' dist-manifest.json sha256.sum
    printf '%s\n' "${archives[@]}" "${archives[@]/%/.sha256}"
  } | LC_ALL=C sort > expected-release-assets
  find release-assets -mindepth 1 -maxdepth 1 -printf '%f\n' | LC_ALL=C sort > staged-release-assets
  diff -u expected-release-assets staged-release-assets
fi
