#!/bin/bash
# Prepare an Ubuntu 24.04 machine (x86_64 or arm64, passwordless sudo, systemd) as a Farhelm test worker. Idempotent:
# every step checks or converges, so it can run again after a change here or a failed run. Workers may be reset to a
# fresh image at any time, so `test-worker.sh` runs this before any run on a worker whose marker (written last, below)
# is missing or names another version of this script. with-lock.sh holds exclusive worker ownership throughout.
#
# The worker builds the tests itself, unlike a boat sandbox, which runs binaries built on the development machine. So
# besides what `scripts/boat/sandbox/bootstrap.sh` installs (rootless Podman with the exact production base image, the
# distribution's tmux, Mutter and Mesa's software renderer for the GPUI app, the app's build packages, fallback fonts,
# passwordless self-ssh), it adds:
# - rustup, with the toolchain left to `rust-toolchain.toml` on first use;
# - `musl-tools`, `file` and binutils, which `scripts/build-podman-test-payloads.sh` needs for the real Podman test;
# - a static BusyBox at `/usr/bin/busybox`, the offline root filesystem of the opt-in `scoped_podman_real` test;
# - an OpenSSH server, for the Rust suite's ssh-to-localhost second host (a worker reached through `tailscale ssh` may
#   not run one);
# - lingering for the account, so its systemd user manager runs for non-login ssh commands;
# - Bun, for the OMP reporter's live admission tests, and the marker that silences Ubuntu's sudo hint in login shells;
# - subordinate uid and gid ranges for the account, which rootless Podman needs and an account created before `uidmap`
#   was installed lacks.
set -u
step() { echo "== $(date -u +%H:%M:%S) $1"; }
me=$(id -un)

step apt
sudo -n env DEBIAN_FRONTEND=noninteractive dpkg --configure -a >/dev/null 2>&1
sudo -n apt-get update -qq || exit 1
sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq build-essential ca-certificates curl git rsync \
  python3 podman uidmap passt slirp4netns fuse-overlayfs tmux mutter dbus-user-session mesa-vulkan-drivers \
  libegl-mesa0 libxkbcommon0 libfontconfig1 librsvg2-bin fonts-dejavu-core fonts-freefont-ttf \
  pkg-config libfontconfig-dev libfreetype-dev libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev libglib2.0-dev \
  musl-tools file binutils openssh-server util-linux unzip busybox-static >/dev/null || exit 1

step account
grep -q "^$me:" /etc/subuid || sudo -n usermod --add-subuids 100000-165535 "$me" || exit 1
grep -q "^$me:" /etc/subgid || sudo -n usermod --add-subgids 100000-165535 "$me" || exit 1
sudo -n loginctl enable-linger "$me" || exit 1

step shell
# Ubuntu's /etc/bash.bashrc prints a hint about sudo when an account in the sudo group starts an interactive bash whose
# HOME lacks `.sudo_as_admin_successful`. Tests start such shells with HOME pointed at a scratch directory, so the
# marker cannot silence it, and a test that reads the shell's output (the supervisor's window command) sees the hint.
# Turn the hint off for the whole worker.
touch "$HOME/.sudo_as_admin_successful"
# The bashrc line is matched and rewritten literally, $HOME included.
# shellcheck disable=SC2016
if grep -q '^if \[ ! -e "\$HOME/.sudo_as_admin_successful" \]' /etc/bash.bashrc; then
  sudo -n sed -i 's|^if \[ ! -e "\$HOME/.sudo_as_admin_successful" \]|if false \&\& [ ! -e "$HOME/.sudo_as_admin_successful" ]|' \
    /etc/bash.bashrc || exit 1
fi

step bun
# The OMP reporter's live admission tests need Bun on PATH; without it they print SKIPPED.
if ! command -v "$HOME/.bun/bin/bun" >/dev/null; then
  installer=$(mktemp) || exit 1
  curl --proto '=https' --tlsv1.2 -fsSL https://bun.sh/install -o "$installer" || exit 1
  bash "$installer" >/dev/null || exit 1
  rm -f "$installer"
fi
"$HOME/.bun/bin/bun" --version

step rust
if ! command -v "$HOME/.cargo/bin/rustup" >/dev/null; then
  installer=$(mktemp) || exit 1
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$installer" || exit 1
  sh "$installer" -y --profile minimal --default-toolchain none --no-modify-path >/dev/null || exit 1
  rm -f "$installer"
fi
"$HOME/.cargo/bin/rustup" --version 2>/dev/null | head -1

step podman
podman system migrate >/dev/null 2>&1
podman pull -q docker.io/library/ubuntu@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852 \
  >/dev/null || { echo "podman pull failed"; exit 1; }

step ssh
sudo -n systemctl enable --now ssh >/dev/null 2>&1 || sudo -n systemctl enable --now ssh.socket >/dev/null 2>&1
install -d -m 700 "$HOME/.ssh" || exit 1
test -f "$HOME/.ssh/id_ed25519" || ssh-keygen -q -t ed25519 -N '' -f "$HOME/.ssh/id_ed25519" || exit 1
grep -qF "$(cat "$HOME/.ssh/id_ed25519.pub")" "$HOME/.ssh/authorized_keys" 2>/dev/null \
  || cat "$HOME/.ssh/id_ed25519.pub" >>"$HOME/.ssh/authorized_keys" || exit 1
chmod 600 "$HOME/.ssh/authorized_keys" || exit 1
for host in localhost 127.0.0.1; do ssh-keygen -q -f "$HOME/.ssh/known_hosts" -R "$host" >/dev/null 2>&1; done
ssh-keyscan -4 -H localhost 127.0.0.1 >>"$HOME/.ssh/known_hosts" 2>/dev/null
ssh -o BatchMode=yes localhost true || { echo "self-ssh failed"; exit 1; }

step versions
# shellcheck source=SCRIPTDIR/env.sh
. "$(dirname "$0")/env.sh"
substrate
mutter --version 2>/dev/null | head -1
dpkg-query -W -f='mesa-vulkan-drivers ${Version}\n' mesa-vulkan-drivers
df -h / | tail -1
# The runs bootstrap first whenever this marker is missing (a worker reset to a fresh image) or names another
# version of this script (test-worker.sh compares it with the hash of the copy it is about to run).
marker=$(dirname "$0")/bootstrap-version
(set -o pipefail && sha256sum "$0" | cut -d' ' -f1 >"$marker") \
  && mv -f "$marker" "$HOME/farhelm-test-worker/bootstrapped" || exit 1
step finished
