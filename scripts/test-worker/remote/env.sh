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

# Keep the worker's disk from filling. One full workspace build with the GUI and its tests takes about 55 GB of
# `target/` (measured on the first arm64 worker, 2026-11-19), and incremental builds only add to it. When less than
# 15 GB is free, start this run from a clean `target/`: slower, but a full disk fails every run after it. Run from the
# synced tree; prints what it did, for the summary.
keep_disk_free() {
  local free
  free=$(df --output=avail -BG . | tail -1 | tr -dc 0-9)
  if [ "${free:-0}" -lt 15 ]; then
    cargo clean >/dev/null 2>&1
    echo "disk: ${free} GB free, so target/ was cleaned first ($(df --output=avail -BG . | tail -1 | tr -dc 0-9) GB free now)"
  else
    echo "disk: ${free} GB free"
  fi
}

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
