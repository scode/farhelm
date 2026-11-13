#!/bin/bash
# Bootstrap a Tensorlake sandbox (Ubuntu 24.04, Tensorlake's own init as PID 1, user tl-user) into the template that
# Farhelm's Rust test shards (rust-tests.ts) clone, once, before it is checkpointed. Installs the base packages, systemd
# and D-Bus, rootless Podman with the exact production base pulled, an SSH server for the tests' ssh-to-localhost hosts,
# and gives tl-user passwordless self-ssh. Nothing boots systemd on this sandbox itself, but the packages bring in
# systemd-sysv's /sbin/init, and clones of the template boot it, so on a clone PID 1 is systemd with logind, which
# run-rust-shard.sh requires. Each step stops the script on failure.
set -u
step() { echo "== $(date -u +%H:%M:%S) $1"; }
step apt
# Package scripts (libc6 among them) call systemctl when /run/systemd/system exists. Without systemd as PID 1 nothing
# answers, and the marker can only be a leftover from something that created it by hand (the removed browser shard
# runner did, to start a user manager here), so make sure it is absent while installing. Under a booted systemd the
# directory is systemd's own and the calls reach it: leave it alone.
if [ "$(cat /proc/1/comm 2>/dev/null)" != systemd ] && ! [ -S /run/systemd/private ]; then
  sudo -n rm -rf /run/systemd/system
fi
sudo -n env DEBIAN_FRONTEND=noninteractive dpkg --configure -a >/dev/null 2>&1
sudo -n apt-get update -qq || exit 1
# Podman and its helpers are for the real environment Rust tests (rust-tests.ts), and so is tmux: Farhelm runs none,
# but the tests that end an earlier release's tmux server, the test-state cleanup and the test of a user's own tmux
# start a real one, and skip without it.
sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq build-essential ca-certificates curl git rsync \
  systemd dbus dbus-user-session openssh-server python3 podman uidmap passt slirp4netns fuse-overlayfs tmux \
  >/dev/null || exit 1
# On clones, which boot systemd, systemd-resolved owns /etc/resolv.conf and was observed (November 2026) with no upstream
# server at all, so nothing on a clone could resolve a name. Real recipe builds install packages and agents over the
# network. Give resolved public servers for those builds and for diagnosis.
sudo -n install -d -m 755 /etc/systemd/resolved.conf.d || exit 1
printf '%s\n' '[Resolve]' 'DNS=1.1.1.1 8.8.8.8' 'FallbackDNS=9.9.9.9' \
  | sudo -n tee /etc/systemd/resolved.conf.d/farhelm-dns.conf >/dev/null || exit 1
# Mark tl-user as lingering (what `loginctl enable-linger` records; no logind runs here to ask). On a clone, logind then
# starts tl-user's user manager at boot and keeps it with no login session open, which run-rust-shard.sh relies on.
sudo -n install -d -m 755 /var/lib/systemd/linger || exit 1
sudo -n touch /var/lib/systemd/linger/tl-user || exit 1
step podman
# The real environment fixture requires the exact managed-base pin in local storage.
# Its production recipe build then uses the network for apt and official installers.
podman pull -q docker.io/library/ubuntu@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852 \
  >/dev/null || { echo "podman pull failed"; exit 1; }
step ssh
install -d -m 700 /home/tl-user/.ssh || exit 1
test -f /home/tl-user/.ssh/id_ed25519 || ssh-keygen -q -t ed25519 -N '' -f /home/tl-user/.ssh/id_ed25519 || exit 1
cat /home/tl-user/.ssh/id_ed25519.pub >> /home/tl-user/.ssh/authorized_keys || exit 1
chmod 600 /home/tl-user/.ssh/authorized_keys || exit 1
sudo -n ssh-keygen -A >/dev/null || exit 1
step versions
tmux -V
podman --version
df -h /home | tail -1
step finished
