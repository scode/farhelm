#!/usr/bin/env bash
# Boot the real stack for Playwright: supervisor (private tmux) + helm
# (loopback :7434) + one fake-agent session, then run the helm in the
# foreground so Playwright's webServer can watch it.
#
# Prerequisites (CI runs these; locally too):
#   cargo build
#   cd crates/farhelm-ui && dx build --platform web --release
#
# State lives in a fresh mktemp dir directly under /tmp for two reasons:
# unix socket paths are limited to ~108 bytes (SUN_LEN), so a deep
# tempdir breaks the supervisor socket; and a fixed, predictable name
# would let any other local user pre-create the directory that ends up
# holding this run's supervisor socket.
#
# NOTE: no reliance on `set -e` (inert under some harnesses; see the
# project's shell guidance) — every step carries its own guard.

repo="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
bin="$repo/target/debug/farhelm"
dist="$repo/target/dx/farhelm-ui/release/web/public"

test -x "$bin" || {
  echo "missing $bin — run cargo build first" >&2
  exit 1
}
test -f "$dist/index.html" || {
  echo "missing web dist — run dx build first" >&2
  exit 1
}

# Backstop for runs that died without their trap (a SIGKILL, a crashed
# shell). Four guards, each load-bearing: `-type d` also excludes
# symlinks (find lstats, so a link to a directory is -type l), which
# stops a local user planting a link and having us kill THEIR tmux
# server; `-O` skips anything we do not own; `-mmin +60` avoids
# destroying a concurrently running suite (two worktrees, two agents)
# mid-test; and NUL delimiting (`-print0` / `read -d ''`) keeps a
# directory NAME from smuggling in a second path — a dirname containing
# a newline would otherwise split into an extra line that resolves
# relative to this script's cwd, steering the `rm -rf` at whatever it
# names.
while IFS= read -r -d '' stale; do
  test -O "$stale" || continue
  tmux -S "$stale/tmux.sock" kill-server 2>/dev/null
  rm -rf "$stale"
done < <(find /tmp -maxdepth 1 -name 'fh-e2e.*' -type d -mmin +60 -print0 2>/dev/null)

state="$(mktemp -d /tmp/fh-e2e.XXXXXX)" || exit 1
work="$state/work"
mkdir -p "$work" || exit 1

# Trap installed BEFORE anything is spawned: a TERM during the socket
# wait below would otherwise exit with no trap and orphan the supervisor
# plus its daemonized tmux server, which nothing reaps for an hour.
cleanup() {
  kill "${helm_pid:-}" 2>/dev/null
  kill "${sup_pid:-}" 2>/dev/null
  # The tmux server daemonizes out of this process group, so killing the
  # supervisor does not reap it; it needs its own shutdown.
  tmux -S "$state/tmux.sock" kill-server 2>/dev/null
  rm -rf "$state"
}
trap cleanup EXIT
trap 'exit 143' TERM INT

"$bin" supervisor run --state-dir "$state" >"$state/supervisor.log" 2>&1 &
sup_pid=$!

# Give the supervisor a moment to bind its socket before the helm dials.
for _ in $(seq 1 50); do
  test -S "$state/supervisor.sock" && break
  sleep 0.1
done

# The helm runs as a child, NOT via exec: bash does not run EXIT traps
# across exec, so an exec'd helm would leave the supervisor and the
# private tmux server orphaned after every run.
"$bin" helm run \
  --state-dir "$state" \
  --port 7434 \
  --ui-dist "$dist" \
  --cwd "$work" \
  --agent "'$bin' internal fake-agent --script basic" \
  --title "e2e-session" &
helm_pid=$!

wait "$helm_pid"
