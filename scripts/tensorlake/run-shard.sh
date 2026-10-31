#!/bin/bash
# Run one shard of Farhelm's browser suite on a bootstrapped Tensorlake sandbox, inside one attached exec (a sandbox
# freezes everything between execs, so the services this needs live only as long as this script).
#
# The image boots no systemd, but the supervisor puts sessions in transient systemd user scopes. sd_booted() only checks
# for /run/systemd/system, so this creates it, gives tl-user a delegated cgroup v2 subtree, and starts a session bus and
# `systemd --user` there (probed: transient scopes then start normally). It also starts sshd so the harness's second,
# ssh-to-localhost host is reachable, as on a developer machine with passwordless self-ssh.
#
# Usage: run-shard.sh <label> <playwright project>... [-- <extra playwright arguments>]
# Projects are named as in playwright.config.ts (e.g. webkit-settings); extra arguments such as
# `-g <title> --repeat-each=3` narrow a rerun. Writes /home/tl-user/out/<label>/summary.txt and console.log; the
# recorder's run directory under /tmp/farhelm-tmp/test-runs keeps the rest.
set -u
label=$1; shift
W=/home/tl-user/farhelm
OUT=/home/tl-user/out/$label
mkdir -p "$OUT"
uid=$(id -u)
# --- leftovers from an earlier exec on this sandbox
# Processes die when an exec ends but their files stay: a stale bus socket refuses connections, and a cgroup that still
# has the old manager's child groups cannot take this shell (cgroup v2 forbids processes in a non-leaf group with
# controllers enabled). Remove both, children first.
pkill -u "$(id -u)" -x systemd 2>/dev/null; pkill -u "$(id -u)" -x dbus-daemon 2>/dev/null
rm -f "/run/user/$(id -u)/bus"
if test -d "/sys/fs/cgroup/user.slice"; then
  find /sys/fs/cgroup/user.slice -depth -mindepth 1 -type d -exec sudo -n rmdir {} + 2>/dev/null
fi
# --- systemd user manager
sudo -n mkdir -p /run/systemd/system
export XDG_RUNTIME_DIR=/run/user/$uid
sudo -n install -d -m 700 -o "$uid" -g "$uid" "$XDG_RUNTIME_DIR"
cg=/sys/fs/cgroup/user.slice/user-$uid.slice/user@$uid.service
sudo -n mkdir -p "$cg"
for f in /sys/fs/cgroup/cgroup.subtree_control /sys/fs/cgroup/user.slice/cgroup.subtree_control \
         /sys/fs/cgroup/user.slice/user-$uid.slice/cgroup.subtree_control; do
  echo "+cpu +memory +pids" | sudo -n tee "$f" >/dev/null 2>&1
done
sudo -n chown -R "$uid:$uid" "$cg"
echo $$ | sudo -n tee "$cg/cgroup.procs" >/dev/null
test -S "$XDG_RUNTIME_DIR/bus" || dbus-daemon --session --address="unix:path=$XDG_RUNTIME_DIR/bus" --fork --nopidfile
export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
if ! systemctl --user is-system-running >/dev/null 2>&1; then
  /usr/lib/systemd/systemd --user >"$XDG_RUNTIME_DIR/systemd-user.log" 2>&1 &
fi
for _ in $(seq 1 20); do
  state=$(systemctl --user is-system-running 2>&1)
  case "$state" in running|degraded) break ;; esac
  sleep 0.5
done
echo "systemd --user: $state" >"$OUT/summary.txt"
# --- self-ssh
sudo -n mkdir -p /run/sshd
pgrep -x sshd >/dev/null || sudo -n /usr/sbin/sshd
ssh-keyscan -H localhost 127.0.0.1 >>/home/tl-user/.ssh/known_hosts 2>/dev/null
ssh -o BatchMode=yes localhost true && echo "self-ssh: ok" >>"$OUT/summary.txt" || echo "self-ssh: FAILED" >>"$OUT/summary.txt"
# --- the shard
export PATH="$W/.ci-tmux:$PATH"
projects=()
extra=()
while [ $# -gt 0 ] && [ "$1" != "--" ]; do projects+=("--project=$1"); shift; done
if [ "${1:-}" = "--" ]; then shift; extra=("$@"); fi
cd "$W/e2e" || exit 1
python3 ../scripts/record-test-run.py --kind development --tmux required \
  --selection "sandbox shard $label: ${projects[*]} ${extra[*]}" \
  --concurrency '3 browser workers, one stack each; retries 0' \
  -- npx playwright test "${projects[@]}" "${extra[@]}" --retries=0 --reporter=line >"$OUT/console.log" 2>&1
code=$?
{
  echo "exit: $code"
  grep -E 'test-run evidence' "$OUT/console.log"
  grep -vE '^\[[0-9]+/[0-9]+\]|^\[WebServer\]|^\[stack' "$OUT/console.log" | grep -E '^\s+[0-9]+ (passed|failed|flaky|skipped|did not run)|  [0-9]+\) '
} >>"$OUT/summary.txt"
cat "$OUT/summary.txt"
exit "$code"
