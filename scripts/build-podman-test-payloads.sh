#!/usr/bin/env bash
# Prepare same-source provisioning payloads for real Podman environment tests.
# Nextest calls this once when that module is selected and exposes its output
# environment only to matching tests. Archive workers supply locally prepared
# payloads through FARHELM_TEST_PODMAN_PAYLOAD_DIR for use on clones, where Cargo is unavailable.
# An explicit FARHELM_HELM_PAYLOAD_DIR takes precedence and is used read-only.
#
# The output is a release-shaped farhelm archive plus the pinned static tmux,
# using the ordinary installer inputs. Packaged output stays under the checkout's
# target/. Cargo honors CARGO_TARGET_DIR and temporary copies are removed on exit.
# stdout is only the payload directory, so the archive packer can reuse this producer.
set -euo pipefail

if [ "${FARHELM_TEST_SKIP_PODMAN:-}" = 1 ]; then
  echo "Podman payload setup skipped by FARHELM_TEST_SKIP_PODMAN=1" >&2
  exit 0
fi

repo=$(cd "$(dirname "$0")/.." && pwd)
case "$(uname -s)/$(uname -m)" in
  Linux/x86_64) target=x86_64-unknown-linux-musl ;;
  Linux/aarch64 | Linux/arm64) target=aarch64-unknown-linux-musl ;;
  *) echo "Podman test payloads require a native Linux x86_64 or arm64 host" >&2; exit 2 ;;
esac

payload_dir=${FARHELM_HELM_PAYLOAD_DIR:-${FARHELM_TEST_PODMAN_PAYLOAD_DIR:-}}
if [ -z "$payload_dir" ]; then
  cd "$repo"
  cargo_target_dir=${CARGO_TARGET_DIR:-$repo/target}
  case "$cargo_target_dir" in /*) ;; *) cargo_target_dir="$repo/$cargo_target_dir" ;; esac
  # As in the CentOS provisioning fixture, the separate native musl target
  # supplies a static payload for Ubuntu containers. A newer development host's
  # dynamically linked glibc binary is incompatible with those containers.
  rustup target add "$target" >&2
  cargo build --locked -p farhelm --target "$target" >&2
  farhelm_payload="$cargo_target_dir/$target/debug/farhelm"
  tmux_payload="$("$repo/scripts/build-pinned-tmux-ci.sh")/tmux"
  "$repo/scripts/check-static-elf.sh" "$farhelm_payload" "$target" >&2
  "$repo/scripts/check-static-elf.sh" "$tmux_payload" "$target" >&2

  payload_dir="$repo/target/podman-test-payloads/$target"
  mkdir -p "$payload_dir"
  stage=$(mktemp -d "$payload_dir/.stage.XXXXXX")
  trap 'rm -rf "$stage"' EXIT
  package="farhelm-$target"
  mkdir "$stage/$package"
  install -m 0755 "$farhelm_payload" "$stage/$package/farhelm"
  install -m 0644 "$repo/LICENSE" "$stage/$package/LICENSE"
  tar -czf "$stage/$package.tar.gz" -C "$stage" "$package"
  install -m 0755 "$tmux_payload" "$stage/tmux-$target"
  mv "$stage/$package.tar.gz" "$payload_dir/$package.tar.gz"
  mv "$stage/tmux-$target" "$payload_dir/tmux-$target"
fi

# NEXTEST_ENV uses one NAME=value per line. Reject line breaks in the path
# to preserve its exact spelling in a single environment record.
case "$payload_dir" in
  *$'\n'* | *$'\r'*) echo "the payload directory cannot contain a line break" >&2; exit 2 ;;
  /*) ;;
  *) echo "the payload directory must be absolute" >&2; exit 2 ;;
esac
test -d "$payload_dir"
test -f "$payload_dir/farhelm-$target.tar.gz"
test -f "$payload_dir/tmux-$target"
if [ -n "${NEXTEST_ENV:-}" ]; then
  printf 'FARHELM_HELM_PAYLOAD_DIR=%s\n' "$payload_dir" >> "$NEXTEST_ENV"
fi
printf '%s\n' "$payload_dir"
