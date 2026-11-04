#!/bin/bash
# Run one shard of Farhelm's browser suite on a bootstrapped Tensorlake sandbox, inside one attached exec. Whatever an
# exec leaves running cannot be relied on afterwards (the sandbox freezes it when it idles), so the tests and the
# services they need start here, in the exec that runs them.
#
# The supervisor puts sessions in transient systemd user scopes, so this needs a systemd user manager, and how it gets
# one depends on what PID 1 is. A sandbox created from the stock image boots Tensorlake's own init, which starts no
# systemd at all; that is the template sandbox during make-template's probe. bootstrap.sh's packages pull in
# systemd-sysv, which installs /sbin/init, and clones of a template boot that, so on them PID 1 is a real systemd with
# logind. Both cases are handled below, and a shard whose user manager is not up fails before any test runs. The script
# also makes self-ssh to localhost work, whichever SSH server answers there (see "self-ssh" below), so the harness's
# second, ssh-to-localhost host is reachable, as on a developer machine with passwordless self-ssh; a shard where it
# does not work fails before any test runs too.
#
# Usage: run-shard.sh <label> <playwright project>... [-- <extra playwright arguments>]
# Projects are named as in playwright.config.ts (e.g. webkit-settings); extra arguments such as
# `-g <title> --repeat-each=3` narrow a rerun. FARHELM_E2E_SUITE_TERMINALS in this script's environment (`host`) is
# kept for the run, which the recorder would otherwise scrub: it turns the stacks to terminal hosts
# (e2e/host-terminal-specs.ts). Writes /home/tl-user/out/<label>/summary.txt and console.log; the recorder's run
# directory under /tmp/farhelm-tmp/test-runs keeps the rest.
set -u
label=$1; shift
W=/home/tl-user/farhelm
OUT=/home/tl-user/out/$label
mkdir -p "$OUT"
uid=$(id -u)
export XDG_RUNTIME_DIR=/run/user/$uid
export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
# Set when the user manager cannot be had; the shard then stops before running anything.
setup_error=
pid1=$(cat /proc/1/comm 2>/dev/null)
# PID 1's name decides, with systemd's private socket as a fallback for a /proc that hides PID 1. Do not test for
# /run/systemd/system (what sd_booted() checks) instead: the Tensorlake-init path below creates that directory itself,
# and it is still there for every later exec on that sandbox.
if [ "$pid1" = systemd ] || [ -S /run/systemd/private ]; then
  # --- systemd user manager, PID 1 is systemd: let logind own it
  # Lingering makes logind start user@<uid>.service and keep it running with no login session open. bootstrap.sh
  # already marks tl-user as lingering, so logind normally starts the manager at boot; enabling it here is the backstop,
  # and its failure is fatal. Without lingering, every ssh-to-localhost login (the self-ssh check below, the harness's
  # second host) starts a user manager that logind stops ten seconds after that session ends (UserStopDelaySec),
  # killing every session scope in it mid-spec. The hand-built manager below is worse here: the self-ssh login makes
  # logind start the real user@<uid>.service in the cgroup that path takes over, and stopping it SIGKILLs this shell and
  # the whole test run. The exec's own cgroup lies outside systemd's tree, but `systemd-run --user --scope` still works
  # from it, because the user manager asks PID 1 to move processes it cannot move itself. Nothing from the
  # Tensorlake-init path below may run here: its pkill and rmdir sweep would hit the lingering manager and systemd's own
  # cgroups.
  for _ in $(seq 1 40); do
    systemctl is-active --quiet systemd-logind && break
    sleep 0.5
  done
  if sudo -n loginctl enable-linger "$uid"; then
    linger=ok
  else
    linger=FAILED
    setup_error="loginctl enable-linger failed"
  fi
else
  linger="n/a (no systemd as PID 1)"
  # --- leftovers from an earlier exec on this sandbox
  # An earlier exec's processes may or may not still run (an sshd it started can outlive it), and its files stay: a
  # stale bus socket refuses connections, and a cgroup that still has the old manager's child groups cannot take this
  # shell (cgroup v2 forbids processes in a non-leaf group with controllers enabled). Remove both, children first.
  pkill -u "$(id -u)" -x systemd 2>/dev/null; pkill -u "$(id -u)" -x dbus-daemon 2>/dev/null
  rm -f "/run/user/$(id -u)/bus"
  if test -d "/sys/fs/cgroup/user.slice"; then
    find /sys/fs/cgroup/user.slice -depth -mindepth 1 -type d -exec sudo -n rmdir {} + 2>/dev/null
  fi
  # --- systemd user manager, PID 1 is Tensorlake's init: build one by hand
  # sd_booted() only checks for /run/systemd/system, so create it, give tl-user a delegated cgroup v2 subtree, and start
  # a session bus and `systemd --user` there (probed: transient scopes then start normally).
  sudo -n mkdir -p /run/systemd/system
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
  if ! systemctl --user is-system-running >/dev/null 2>&1; then
    /usr/lib/systemd/systemd --user >"$XDG_RUNTIME_DIR/systemd-user.log" 2>&1 &
  fi
fi
# Up to 20 s, as long as the logind wait above: on a busy, freshly booted clone the manager may only start now.
for _ in $(seq 1 40); do
  state=$(systemctl --user is-system-running 2>&1)
  case "$state" in running|degraded) break ;; esac
  sleep 0.5
done
case "$state" in running|degraded) ;; *) setup_error=${setup_error:-"the systemd user manager is not running"} ;; esac
{
  echo "pid 1: ${pid1:-unreadable}"
  echo "linger: $linger"
  echo "systemd --user: $state"
} >"$OUT/summary.txt"
# Without a user manager the supervisor silently falls back to running sessions unscoped, and the specs can still pass
# on that fallback, which is not the substrate this run claims to test (.agents/validation.md). Stop loudly instead.
if [ -n "$setup_error" ]; then
  echo "setup failed: $setup_error; no tests were run" >>"$OUT/summary.txt"
  cat "$OUT/summary.txt"
  exit 1
fi
# --- self-ssh
# The harness's second host is an ssh login to localhost, so a shard without self-ssh loses every multi-host spec's
# coverage: those specs fail on a host stuck connecting, or skip. Whoever serves port 22, three things must hold:
# known_hosts has its current host key, under both names ssh may check; tl-user's key is in the authorized_keys that
# server reads; and a real login works. Anything less fails the shard before any test runs.
#
# On clones seen in November 2026, Tensorlake's in-sandbox daemon runs its own sshd on port 22, with a host key and an
# authorized_keys file under /run/tl/ssh (for its ssh gateway). The image's ssh.socket then fails to bind and stays
# failed, so that platform sshd is the one answering. Its host key differs from the one the template's known_hosts
# recorded, and it never reads ~/.ssh/authorized_keys. Without the platform sshd (as when the October 2026 template was
# built and probed), ssh.socket or an sshd started here answers, with the keys bootstrap.sh made.
known_hosts=/home/tl-user/.ssh/known_hosts
platform_ssh=/run/tl/ssh
# Under a booted systemd, ssh.socket holds port 22 and no sshd shows until the first connection; the platform sshd
# shows as a running sshd. Start one only when neither answers.
sudo -n mkdir -p /run/sshd
systemctl is-active --quiet ssh.socket 2>/dev/null || pgrep -x sshd >/dev/null || sudo -n /usr/sbin/sshd
# Replace whatever the template recorded for localhost with the key of the sshd that answers now, so the file holds
# exactly what this sandbox serves and repeated shards on one sandbox do not pile up entries.
ssh-keygen -R localhost -f "$known_hosts" >/dev/null 2>&1
ssh-keygen -R 127.0.0.1 -f "$known_hosts" >/dev/null 2>&1
# Scan 127.0.0.1 itself and write the keys under both names. `ssh-keyscan localhost` tries only the first address
# localhost resolves to, which is ::1, where the platform sshd does not listen, and then records nothing; ssh itself
# falls back to 127.0.0.1 and checks the key under both names.
scanned=$(ssh-keyscan -T 10 127.0.0.1 2>/dev/null | grep -v '^#')
if [ -n "$scanned" ]; then
  printf '%s\n' "$scanned" | sed 's/^127\.0\.0\.1 /localhost,127.0.0.1 /' >>"$known_hosts"
fi
# The platform sshd reads only its own authorized_keys (and for every user, root included: this lets tl-user's key in
# as anyone, which is acceptable on a throwaway clone). Whether the platform ever rewrites that file while a sandbox runs
# is unknown; it was written at boot and left alone for the length of the runs observed.
if sudo -n test -f "$platform_ssh/authorized_keys"; then
  if ! sudo -n grep -qxF -f /home/tl-user/.ssh/id_ed25519.pub "$platform_ssh/authorized_keys"; then
    # tl-user reads its own key; only tee needs root, to append to the platform's file.
    # shellcheck disable=SC2024
    sudo -n tee -a "$platform_ssh/authorized_keys" </home/tl-user/.ssh/id_ed25519.pub >/dev/null
  fi
  ssh_server="Tensorlake's sshd"
else
  ssh_server="the image's sshd"
fi
if ssh_error=$(ssh -o BatchMode=yes -o ConnectTimeout=10 localhost true 2>&1); then
  echo "self-ssh: ok ($ssh_server)" >>"$OUT/summary.txt"
else
  {
    echo "self-ssh: FAILED ($ssh_server): $(printf '%s' "$ssh_error" | tail -1)"
    echo "setup failed: self-ssh to localhost does not work; no tests were run"
  } >>"$OUT/summary.txt"
  cat "$OUT/summary.txt"
  exit 1
fi
# --- the shard
export PATH="$W/.ci-tmux:$PATH"
projects=()
extra=()
while [ $# -gt 0 ] && [ "$1" != "--" ]; do projects+=("--project=$1"); shift; done
if [ "${1:-}" = "--" ]; then shift; extra=("$@"); fi
keep=()
if [ -n "${FARHELM_E2E_SUITE_TERMINALS:-}" ]; then
  keep=(--keep-farhelm-env FARHELM_E2E_SUITE_TERMINALS)
  echo "suite terminals: $FARHELM_E2E_SUITE_TERMINALS" >>"$OUT/summary.txt"
fi
cd "$W/e2e" || exit 1
python3 ../scripts/record-test-run.py --kind development --tmux required "${keep[@]}" \
  --selection "sandbox shard $label: ${FARHELM_E2E_SUITE_TERMINALS:+$FARHELM_E2E_SUITE_TERMINALS terminals: }${projects[*]} ${extra[*]}" \
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
