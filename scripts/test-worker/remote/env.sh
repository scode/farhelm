# Sourced by the worker scripts: the environment a non-login ssh command lacks. Rustup's cargo is on PATH, and the
# systemd user manager and its bus are found the way a login session finds them (the supervisor's transient scopes need
# both; bootstrap.sh enables lingering so the manager runs without a login).
# shellcheck shell=bash
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"

# Make the tree's pinned toolchain rustup's default too. Inside the tree `rust-toolchain.toml` selects it anyway, but
# the macro compile-contract test runs cargo from a scratch project outside it, where rustup otherwise has no toolchain
# at all. Run from the synced tree.
use_pinned_toolchain() {
  local channel
  channel=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)
  [ -n "$channel" ] && rustup default "$channel" >/dev/null 2>&1
}
XDG_RUNTIME_DIR=/run/user/$(id -u)
export XDG_RUNTIME_DIR
export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"

# Keep the worker's disk from filling with a checkout's own build output.
#
# One full workspace build with the GUI and its tests takes about 55 GB of `target/` (measured on the first arm64
# worker, 2026-11-19), and incremental builds only add to it, so a tree that is reused round after round grows until a
# build runs out of space halfway. A free-space check alone did not prevent that: on 2026-11-23 the landing monitor's
# own `target/` had grown to 128 GB of a 135 GB disk, the old 15 GB threshold let a run start with too little room for a
# cold build, and the build then failed with "No space left on device", which looked like the worker's fault.
#
# So every run, when it starts, holds this checkout to its own share: its build output (`target/`, and the `target/` of
# each vendored crate, which are outside the workspace and build on their own) is at most DISK_TREE_BUDGET_GB, and the
# run starts with DISK_BUILD_ROOM_GB free, room for a cold full build. When either is missed it cleans up after itself,
# cheapest first: rustc's incremental caches, which cost only a rebuild of the workspace's own crates, then all of its
# build output. If the room is still missing once that output is gone, the rest of the disk belongs to other trees or
# files, which this checkout must not touch: it says how much they use and returns 1, so the run stops before a build
# fails halfway. The check runs only at a run's start, so two checkouts building at once can still need more than the
# room each saw. Run from the synced tree; prints what it did, for the summary.
DISK_TREE_BUDGET_GB=60
DISK_BUILD_ROOM_GB=55
# Free and used space on the tree's file system in whole GB; empty when df cannot say.
disk_free_gb() { df --output=avail -BG . 2>/dev/null | tail -1 | tr -dc 0-9; }
disk_used_gb() { df --output=used -BG . 2>/dev/null | tail -1 | tr -dc 0-9; }
# This checkout's build output directories that exist, one per line. Only these are ever measured or removed.
own_build_dirs() {
  local dir
  for dir in target vendor/*/target; do
    [ -d "$dir" ] && printf '%s\n' "$dir"
  done
}
# Their combined size in whole GB, 0 when there are none.
own_build_gb() {
  local dirs
  mapfile -t dirs < <(own_build_dirs)
  if [ "${#dirs[@]}" -eq 0 ]; then echo 0; else du -s -c -BG "${dirs[@]}" 2>/dev/null | tail -1 | cut -f1 | tr -dc 0-9; fi
}
keep_disk_free() {
  local free own action='' dirs
  free=$(disk_free_gb) own=$(own_build_gb)
  if [ -z "$free" ]; then
    # Without a reading, cleaning could wipe a healthy tree and blaming others would be a guess.
    echo "disk: free space unknown (df gave no reading); this checkout's build output is ${own:-?} GB; nothing removed"
    return 0
  fi
  if [ "${own:-0}" -gt "$DISK_TREE_BUDGET_GB" ] || [ "$free" -lt "$DISK_BUILD_ROOM_GB" ]; then
    mapfile -t dirs < <(own_build_dirs)
    [ "${#dirs[@]}" -eq 0 ] || find "${dirs[@]}" -type d -name incremental -prune -exec rm -rf {} + 2>/dev/null
    action='its incremental caches were removed first'
    free=$(disk_free_gb) own=$(own_build_gb)
  fi
  if [ "${own:-0}" -gt "$DISK_TREE_BUDGET_GB" ] || [ "${free:-0}" -lt "$DISK_BUILD_ROOM_GB" ]; then
    # rm rather than cargo clean: it needs neither a readable manifest nor a working toolchain, and it covers the
    # vendored crates' output too. What it removes is only own_build_dirs.
    mapfile -t dirs < <(own_build_dirs)
    [ "${#dirs[@]}" -eq 0 ] || rm -rf -- "${dirs[@]}"
    action='its build output was removed first'
    free=$(disk_free_gb) own=$(own_build_gb)
  fi
  echo "disk: ${free:-?} GB free; this checkout's build output is ${own:-?} GB${action:+ ($action)}"
  if [ "${free:-0}" -lt "$DISK_BUILD_ROOM_GB" ]; then
    if [ "${own:-0}" -gt 0 ]; then
      echo "disk: under $DISK_BUILD_ROOM_GB GB free and this checkout's build output could not all be removed"
    else
      echo "disk: FULL BY OTHER WORKLOADS: under $DISK_BUILD_ROOM_GB GB free with this checkout's build output removed;" \
        "other trees and files on the worker use $(disk_used_gb) GB"
    fi
    return 1
  fi
}

# The status a run ends with when keep_disk_free found the disk full by other workloads, so a caller can tell it apart
# from a failed build or test. The runners that source this file use it.
# shellcheck disable=SC2034
DISK_FULL_STATUS=75

# Print the facts a run's evidence depends on, so a summary shows whether the substrate was really there.
substrate() {
  # The development machine's commit this tree mirrors (snapshot.sh), run from inside the synced tree.
  echo "source commit: $(cat ../source.txt 2>/dev/null || echo unknown)"
  echo "arch: $(uname -m); $(grep -E '^PRETTY_NAME=' /etc/os-release | cut -d= -f2)"
  echo "systemd --user: $(systemctl --user is-system-running 2>&1)"
  echo "podman: $(podman --version 2>&1)"
  echo "tmux: $(command -v tmux 2>&1) $(tmux -V 2>&1)"
  if ssh -o BatchMode=yes localhost true 2>/dev/null; then echo "self-ssh: ok"; else echo "self-ssh: FAILED"; fi
}
