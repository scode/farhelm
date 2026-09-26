#!/usr/bin/env bash
# Boot the fleet the README hero capture photographs (docs/readme-hero/SPEC.md):
# one local supervisor, one supervisor per remote destination the config
# hands over, and a helm on the port Playwright chose, run in the foreground
# so Playwright's webServer can watch it.
#
# This is e2e/start-stack.sh generalised from one remote to N and stripped
# of everything the ordinary suite stages for its tests (the startup
# session, the provisioning backend, the structured-launch shim). The
# capture spec creates its own sessions through the API once the helm is
# up, because what they are and how they behave is the scenario's business,
# not the stack's. The comments here cover only what differs; the reasoning
# behind the shared shape (why the port is an input, why the state dir is a
# shallow mktemp under /tmp, why the flock, why the orphan watcher, why the
# helm is a child and not an exec) is in the original and is not repeated.
#
# Inputs, all from readme-hero.config.ts's webServer.env:
#   FARHELM_E2E_PORT        the helm's port
#   FARHELM_HERO_REMOTES    JSON array of ssh destinations, one per remote host
#   FARHELM_E2E_STACK_INFO  where to publish what was booted
#
# Every remote is ssh to THIS machine under its own spelling. The helm keys
# hosts on the exact destination string, so distinct spellings are distinct
# hosts; the capture fails loudly if one of them cannot connect, because a
# picture missing a host is the wrong picture, not a degraded one.
#
# NOTE: no reliance on `set -e` (inert under some harnesses); every step
# carries its own guard.

repo="$(cd "$(dirname "$0")/../.." && pwd)" || exit 1
bin="$repo/target/debug/farhelm"
# The replay fixture and the state sweep: test fixtures, built by
# `cargo build` beside the product binary but never shipped with it.
fixtures="$repo/target/debug/farhelm-fixtures"
dist="$repo/target/dx/farhelm-ui/release/web/public"

test -x "$bin" || { echo "missing $bin — run cargo build first" >&2; exit 1; }
test -x "$fixtures" || { echo "missing $fixtures — run cargo build first" >&2; exit 1; }
test -f "$dist/index.html" || { echo "missing web dist — run dx build first" >&2; exit 1; }

port="${FARHELM_E2E_PORT:-}"
test -n "$port" || { echo "FARHELM_E2E_PORT is unset — readme-hero.config.ts sets it" >&2; exit 1; }
case "$port" in
  *[!0-9]* | '' | ??????*) echo "FARHELM_E2E_PORT must be a TCP port number, got '$port'" >&2; exit 1 ;;
esac
if [ "$port" -lt 1 ] || [ "$port" -gt 65535 ]; then
  echo "FARHELM_E2E_PORT is out of range: $port" >&2
  exit 1
fi

remotes_json="${FARHELM_HERO_REMOTES:-}"
test -n "$remotes_json" || { echo "FARHELM_HERO_REMOTES is unset — readme-hero.config.ts sets it" >&2; exit 1; }
stack_info="${FARHELM_E2E_STACK_INFO:-}"
test -n "$stack_info" || { echo "FARHELM_E2E_STACK_INFO is unset — readme-hero.config.ts sets it" >&2; exit 1; }

# One destination per line for the shell loop below; python owns the JSON.
remotes="$(printf '%s' "$remotes_json" | python3 -c '
import json, sys
for destination in json.load(sys.stdin):
    if not isinstance(destination, str) or not destination or "\n" in destination:
        raise SystemExit("bad remote destination")
    print(destination)
')" || exit 1

# Refuse early, with the destination named, rather than let the helm report
# an unreachable host after the capture has already waited on it. The same
# options the suite's self-ssh probe uses, so the answer cannot differ.
while IFS= read -r destination; do
  test -n "$destination" || continue
  ssh -o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout=10 "$destination" true >/dev/null 2>&1 || {
    echo "passwordless ssh to '$destination' failed; the scenario's remote hosts need it on this machine" >&2
    exit 1
  }
done <<<"$remotes"

"$fixtures" sweep-test-state || true

# Same prefix family as the ordinary suite so the teststate sweep reaps a
# run that died without its trap.
state="$(mktemp -d /tmp/fh-e2e.XXXXXX)" || exit 1
if command -v flock >/dev/null 2>&1; then
  exec 9>"$state/fh-run.lock" || exit 1
  flock -w 10 9 || exit 1
fi
auth_state="$repo/e2e/.auth/storage-state.json"
# The staged sessions are created with the invocation `claude` or `codex`,
# literally, so the row badge, the derived agent kind, and the session
# header all read the way a real launch does. Those names resolve to the
# wrappers below because every supervisor here runs with a private HOME
# whose login profile puts the wrapper directory first on PATH, the same
# arrangement the ordinary stack uses for its structured-launch shim. Each
# wrapper hands off to a per-session script the capture spec writes into
# that session's scratch working directory, which is how seven sessions
# named `claude` can each replay a different transcript.
wrappers="$state/wrappers"
home="$state/home"
work="$state/work"
mkdir -p "$wrappers" "$home" "$work" || exit 1
for name in claude codex; do
  cat >"$wrappers/$name" <<'EOF' || exit 1
#!/bin/sh
# README hero wrapper: run the replay the capture spec left in this
# session's working directory, passing through whatever the supervisor
# appended for the real vendor.
exec sh "$PWD/.hero-agent" "$@"
EOF
  chmod 700 "$wrappers/$name" || exit 1
done
printf '%s\n' "export PATH=$wrappers:\$PATH" >"$home/.bash_profile" || exit 1
bash_shell=$(command -v bash) || exit 1

remote_states=()
remote_pids=()

cleanup() {
  kill "${watcher_pid:-}" 2>/dev/null
  wait "${watcher_pid:-}" 2>/dev/null
  kill "${helm_pid:-}" 2>/dev/null
  kill "${sup_pid:-}" 2>/dev/null
  for pid in "${remote_pids[@]}"; do kill "$pid" 2>/dev/null; done
  rm -f "$stack_info"
  rm -f "$auth_state"
  tmux -S "$state/tmux.sock" kill-server 2>/dev/null
  for remote_state in "${remote_states[@]}"; do
    tmux -S "$remote_state/tmux.sock" kill-server 2>/dev/null
  done
  rm -rf "$state"
}
trap cleanup EXIT
trap 'exit 143' TERM INT

spawner_pid=$PPID
script_name=${0##*/}
orphan_watch() {
  command -v ps >/dev/null 2>&1 || exit 0
  while kill -0 "$spawner_pid" 2>/dev/null; do
    sleep 2
  done
  case "$(ps -p "$$" -o args= 2>/dev/null)" in
    *"$script_name"*) kill -TERM $$ 2>/dev/null ;;
  esac
}
orphan_watch 9>&- &
watcher_pid=$!

HOME="$home" SHELL="$bash_shell" "$bin" supervisor run --state-dir "$state" >"$state/supervisor.log" 2>&1 9>&- &
sup_pid=$!

i=0
while IFS= read -r destination; do
  test -n "$destination" || continue
  remote_state="$state/remote-$i"
  mkdir -p "$remote_state" || exit 1
  HOME="$home" SHELL="$bash_shell" "$bin" supervisor run --state-dir "$remote_state" >"$state/remote-$i.log" 2>&1 9>&- &
  remote_pids+=("$!")
  remote_states+=("$remote_state")
  i=$((i + 1))
done <<<"$remotes"

for sock in "$state/supervisor.sock" "${remote_states[@]/%//supervisor.sock}"; do
  for _ in $(seq 1 50); do
    test -S "$sock" && break
    sleep 0.1
  done
done

# The ensure file and the stack info, both generated because they carry this
# run's paths. The remote entries are built by zipping the destination list
# with the state directories created above, in the same order.
ensure="$state/ensure-hosts.json5"
printf '%s\n' "$remotes" | python3 -c '
import json, sys
bin_path, ensure_path, info_path, state, wrappers, work, port, fixtures = sys.argv[1:9]
destinations = [line for line in sys.stdin.read().split("\n") if line]
hosts = [{
    "ssh": destination,
    "remote_farhelm": bin_path,
    "remote_state_dir": f"{state}/remote-{index}",
} for index, destination in enumerate(destinations)]
with open(ensure_path, "w") as ensure:
    json.dump({"hosts": hosts}, ensure)
with open(info_path, "w") as info:
    json.dump({
        "farhelm": bin_path,
        "fixtures": fixtures,
        "state": state,
        "port": int(port),
        "wrappers": wrappers,
        "work": work,
        "remotes": [{"ssh": host["ssh"], "state": host["remote_state_dir"]} for host in hosts],
    }, info)
' "$bin" "$ensure" "$stack_info" "$state" "$wrappers" "$work" "$port" "$fixtures" || exit 1

"$bin" helm run \
  --state-dir "$state" \
  --port "$port" \
  --ui-dist "$dist" \
  --ensure-hosts "$ensure" 9>&- &
helm_pid=$!

wait "$helm_pid"
