#!/bin/bash
# Bootstrap a Tensorlake sandbox (Ubuntu 24.04, Tensorlake's own init as PID 1, user tl-user) into a Farhelm browser-test
# worker, once, before it is checkpointed as a template. Installs the base packages, systemd and D-Bus (started per test
# exec by run-shard.sh, since nothing here boots them), an SSH server for the harness's ssh-to-localhost second host,
# unpacks the pushed source and binaries into /home/tl-user/farhelm, installs the e2e node modules and Playwright's
# Chromium and WebKit with their system dependencies, and gives tl-user passwordless self-ssh. Expects
# /home/tl-user/stage/src.tar.gz and bin.tar.gz. Each step stops the script on failure.
set -u
W=/home/tl-user/farhelm
S=/home/tl-user/stage
step() { echo "== $(date -u +%H:%M:%S) $1"; }
step apt
# Package scripts (libc6 among them) call systemctl when /run/systemd/system exists, and nothing answers here; the
# marker is created only by run-shard.sh for a test run, so make sure it is absent while installing.
sudo -n rm -rf /run/systemd/system
sudo -n env DEBIAN_FRONTEND=noninteractive dpkg --configure -a >/dev/null 2>&1
sudo -n apt-get update -qq || exit 1
sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq build-essential ca-certificates curl git rsync \
  systemd dbus dbus-user-session openssh-server python3 >/dev/null || exit 1
step unpack
mkdir -p "$W" || exit 1
tar -xzf "$S/src.tar.gz" -C "$W" || exit 1
tar -xzf "$S/bin.tar.gz" -C "$W" || exit 1
step npm
cd "$W/e2e" || exit 1
npm ci --no-audit --no-fund >/dev/null 2>&1 || { echo "npm ci failed"; exit 1; }
step playwright
sudo -n env "PATH=$PATH" npx playwright install-deps chromium webkit >/dev/null 2>&1 || { echo "install-deps failed"; exit 1; }
npx playwright install chromium webkit >/dev/null 2>&1 || { echo "browser install failed"; exit 1; }
step ssh
install -d -m 700 /home/tl-user/.ssh || exit 1
test -f /home/tl-user/.ssh/id_ed25519 || ssh-keygen -q -t ed25519 -N '' -f /home/tl-user/.ssh/id_ed25519 || exit 1
cat /home/tl-user/.ssh/id_ed25519.pub >> /home/tl-user/.ssh/authorized_keys || exit 1
chmod 600 /home/tl-user/.ssh/authorized_keys || exit 1
sudo -n ssh-keygen -A >/dev/null || exit 1
step versions
node --version
"$W/target/debug/farhelm" --version 2>&1 | head -1
"$W/.ci-tmux/tmux" -V
npx playwright --version
df -h /home | tail -1
step finished
