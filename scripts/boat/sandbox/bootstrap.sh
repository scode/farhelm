#!/bin/bash
# Bootstrap a boat sandbox (Ubuntu 24.04 VM, systemd as PID 1, user `user` lingering, an sshd on port 22) into a
# Farhelm test worker, once per sandbox. `boat-tests.ts` runs it on every claim; a marker makes a finished bootstrap a
# no-op, and a rerun after a failure redoes every step.
#
# What boat's image already has and the workers rely on: a real systemd with logind and a lingering user manager for
# `user` (the supervisor's transient scopes need it), Python 3.12, Mesa's software renderer (`mesa-vulkan-drivers`,
# `libegl-mesa0`, the GPUI app's lavapipe), glibc 2.39 (the development machine's locally built debug binaries run
# unchanged), and an sshd on port 22 for the Rust suite's ssh-to-localhost second host.
# What this adds:
# - rootless Podman with the exact production base image, for the real environment Rust test (`podman_environments`);
# - the distribution's tmux, for the few Rust tests that start a real tmux server (ending an earlier release's
#   terminals, the test-state cleanup, a user's own /usr/bin/tmux under a session's scope); without it they print
#   SKIPPED and pass, which is lost coverage, not evidence;
# - Mutter, for the GPUI app runs (`gui-agent.py` starts a private headless Mutter; `dbus-run-session` is already there);
# - librsvg's `rsvg-convert`, which draws the docs screenshots' callouts onto their captures (`scripts/docs_callouts.py`);
# - the GPUI app's build packages (`.agents/validation.md`, the workspace nextest entry, plus GIO's for the desktop app
#   and xkbcommon's X11 part, which the image lacks), for a docs capture that builds the app here so its record can be
#   published (`boat-tests.ts docs --build-there`);
# - fallback fonts for the symbols agents print that the app's bundled fonts lack (`●` from DejaVu, `⎿` from FreeMono):
#   the image has none, so a docs capture drew them as empty boxes where the development machines draw the glyphs;
# - a key for passwordless self-ssh.
set -u
H=/home/user
MARK=$H/.farhelm-bootstrapped
step() { echo "== $(date -u +%H:%M:%S) $1"; }
test -f "$MARK" && { echo "already bootstrapped"; exit 0; }
step apt
sudo -n env DEBIAN_FRONTEND=noninteractive dpkg --configure -a >/dev/null 2>&1
sudo -n apt-get update -qq || exit 1
sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq build-essential ca-certificates curl git rsync \
  python3 podman uidmap passt slirp4netns fuse-overlayfs tmux mutter dbus-user-session mesa-vulkan-drivers \
  libegl-mesa0 libxkbcommon0 libfontconfig1 librsvg2-bin fonts-dejavu-core fonts-freefont-ttf \
  pkg-config libfontconfig-dev libfreetype-dev libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev libglib2.0-dev \
  >/dev/null || exit 1
step podman
podman pull -q docker.io/library/ubuntu@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852 \
  >/dev/null || { echo "podman pull failed"; exit 1; }
step ssh
install -d -m 700 $H/.ssh || exit 1
test -f $H/.ssh/id_ed25519 || ssh-keygen -q -t ed25519 -N '' -f $H/.ssh/id_ed25519 || exit 1
grep -qF "$(cat $H/.ssh/id_ed25519.pub)" $H/.ssh/authorized_keys 2>/dev/null \
  || cat $H/.ssh/id_ed25519.pub >>$H/.ssh/authorized_keys || exit 1
chmod 600 $H/.ssh/authorized_keys || exit 1
for host in localhost 127.0.0.1; do ssh-keygen -q -f $H/.ssh/known_hosts -R "$host" >/dev/null 2>&1; done
ssh-keyscan -4 -H localhost 127.0.0.1 >>$H/.ssh/known_hosts 2>/dev/null
ssh -o BatchMode=yes localhost true || { echo "self-ssh failed"; exit 1; }
step versions
podman --version
tmux -V
mutter --version 2>/dev/null | head -1
rsvg-convert --version
dpkg-query -W -f='${Package} ${Version}\n' mesa-vulkan-drivers
df -h / | tail -1
touch "$MARK"
step finished
