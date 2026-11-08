#!/bin/bash
# Bootstrap a Tensorlake sandbox (Ubuntu 24.04, Tensorlake's own init as PID 1, user tl-user) into a Farhelm browser-test
# worker, once, before it is checkpointed as a template. Installs the base packages, systemd and D-Bus, an SSH server for
# the harness's ssh-to-localhost second host, unpacks the pushed source and binaries into /home/tl-user/farhelm, installs
# the e2e node modules and Playwright's Chromium and WebKit with their system dependencies, and gives tl-user
# passwordless self-ssh. Nothing boots systemd on this sandbox itself, but the packages bring in systemd-sysv's
# /sbin/init, and clones of the template boot it, so on a clone PID 1 is systemd with logind; run-shard.sh handles both.
# Expects /home/tl-user/stage/src.tar.gz and bin.tar.gz. Each step stops the script on failure.
set -u
W=/home/tl-user/farhelm
S=/home/tl-user/stage
step() { echo "== $(date -u +%H:%M:%S) $1"; }
step apt
# Package scripts (libc6 among them) call systemctl when /run/systemd/system exists. Without systemd as PID 1 nothing
# answers, and the marker is there only because run-shard.sh created it for a test run, so make sure it is absent while
# installing. Under a booted systemd the directory is systemd's own and the calls reach it: leave it alone.
if [ "$(cat /proc/1/comm 2>/dev/null)" != systemd ] && ! [ -S /run/systemd/private ]; then
  sudo -n rm -rf /run/systemd/system
fi
sudo -n env DEBIAN_FRONTEND=noninteractive dpkg --configure -a >/dev/null 2>&1
sudo -n apt-get update -qq || exit 1
# Podman and its helpers are for the Rust tests (rust-tests.ts): the containerized e2e tests need rootless Podman.
sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq build-essential ca-certificates curl git rsync \
  systemd dbus dbus-user-session openssh-server python3 podman uidmap passt slirp4netns fuse-overlayfs >/dev/null || exit 1
# On clones, which boot systemd, systemd-resolved owns /etc/resolv.conf and was observed (November 2026) with no upstream
# server at all, so nothing on a clone could resolve a name. The tests need none, but anything installed while
# diagnosing a clone does; give resolved public servers.
sudo -n install -d -m 755 /etc/systemd/resolved.conf.d || exit 1
printf '%s\n' '[Resolve]' 'DNS=1.1.1.1 8.8.8.8' 'FallbackDNS=9.9.9.9' \
  | sudo -n tee /etc/systemd/resolved.conf.d/farhelm-dns.conf >/dev/null || exit 1
# Mark tl-user as lingering (what `loginctl enable-linger` records; no logind runs here to ask). On a clone, logind then
# starts tl-user's user manager at boot and keeps it with no login session open, which run-shard.sh relies on.
sudo -n install -d -m 755 /var/lib/systemd/linger || exit 1
sudo -n touch /var/lib/systemd/linger/tl-user || exit 1
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
step podman
# The containerized e2e tests never pull images (.agents/validation.md); the template carries the one they use.
podman pull -q docker.io/library/ubuntu:24.04 >/dev/null || { echo "podman pull failed"; exit 1; }
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
podman --version
df -h /home | tail -1
step finished
