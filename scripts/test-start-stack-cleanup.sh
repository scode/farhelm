#!/usr/bin/env bash
# Cancellation cleanup for the browser stack: killing Playwright (or
# whatever spawned start-stack.sh) must still free the stack's ports and
# state, without touching anything else.
#
# Playwright runs start-stack.sh detached in its own process group and
# SIGTERMs that group on its graceful paths — but it handles SIGINT only,
# so a SIGTERM, SIGKILL, or crash of the Playwright leader skips teardown
# entirely and used to strand the shell with its helm, supervisors, ports,
# and state lock, unreachable to the recorder's group cleanup. The script
# now watches its spawner and runs its normal TERM trap when the spawner
# goes away. This script proves that for spawner SIGKILL and SIGTERM, plus
# the normal trap path, with an unrelated tmux server, process, and state
# lookalike standing by as the "other stacks" that must survive. A
# layering guard first pins the `exec` prefix the watcher depends on: the
# Playwright leader must direct-parent the script, not a resident shell.
#
# Needs a built tree (cargo build, dx web build); picks its own free port
# per run and boots three real stacks on it plus a seconds-long layering
# guard, so a few minutes.
# NOT wired into CI: like the rest of the browser suite it is far too slow
# for per-PR runs. Run it by hand when start-stack.sh's lifetime handling
# changes.
#
# NOTE: no reliance on `set -e` — every step carries its own guard.

repo="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
bin="$repo/target/debug/farhelm"
stack_script="$repo/e2e/start-stack.sh"
stack_info="$repo/e2e/.stack-info.json"

# One free loopback port per run, never a fixed one: start-stack.sh takes
# its port as an INPUT through FARHELM_E2E_PORT (playwright.config.ts picks
# it in production, and the script refuses to run without it), and a fixed
# choice here is exactly the collision between concurrent runs that AGENTS.md
# forbids for every harness. The pick-then-bind window is the same trade
# e2e/stack-port.ts makes: another process can take the port in the
# milliseconds before the helm binds it, and the loss is loud (the boot
# phase fails) rather than silent. Exported once so every boot below
# inherits it through the spawner.
FARHELM_E2E_PORT="$(python3 -c '
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
')" || exit 1
export FARHELM_E2E_PORT
port="http://127.0.0.1:$FARHELM_E2E_PORT/"

failures=0
fail() {
  echo "FAIL: $1" >&2
  failures=$((failures + 1))
}
pass() {
  echo "ok: $1"
}

test -x "$bin" || { echo "missing $bin — run cargo build first" >&2; exit 1; }
test -f "$repo/target/dx/farhelm-ui/release/web/public/index.html" || {
  echo "missing web dist — run dx build first" >&2
  exit 1
}
curl -s -m 2 -o /dev/null "$port" && {
  echo "something already answers $port — stop it first" >&2
  exit 1
} || true

# Decoys: the "other stacks". A tmux server on its own socket (kill-server
# scope must not escape), a plain process (no stray kills), and a state
# lookalike dir (rm -rf scope must not escape). The lookalike's fh-e2e
# name puts it in the sweep family on purpose; that couples this test to
# the sweep's 60-minute backstop, which a minutes-long run never nears.
decoy_tag="decoy-$$"
decoy_sock="/tmp/tmux-$decoy_tag.sock"
decoy_dir="/tmp/fh-e2e.$decoy_tag"
tmux -S "$decoy_sock" new-session -d -x 80 -y 24 || exit 1
sleep 300 &
decoy_pid=$!
mkdir -p "$decoy_dir" || exit 1
echo "decoy" >"$decoy_dir/marker" || exit 1

spawner_pid=""
script_pid=""
cleanup_test() {
  if test -n "$spawner_pid" && kill -0 "$spawner_pid" 2>/dev/null; then
    kill -KILL "$spawner_pid" 2>/dev/null
  fi
  if test -n "$script_pid" && kill -0 "$script_pid" 2>/dev/null; then
    kill -KILL "$script_pid" 2>/dev/null
  fi
  sleep 3
  "$bin" internal sweep-test-state 2>/dev/null || true
  kill "$decoy_pid" 2>/dev/null
  tmux -S "$decoy_sock" kill-server 2>/dev/null
  rm -rf "$decoy_dir"
}
trap cleanup_test EXIT

check_decoys() {
  local phase="$1"
  tmux -S "$decoy_sock" has-session 2>/dev/null \
    || fail "$phase: decoy tmux server died"
  kill -0 "$decoy_pid" 2>/dev/null \
    || fail "$phase: decoy process died"
  test "$(cat "$decoy_dir/marker" 2>/dev/null)" = "decoy" \
    || fail "$phase: decoy state dir touched"
}

await_stack_ready() {
  local phase="$1"
  local _
  for _ in $(seq 1 90); do
    test -f "$stack_info" && break
    kill -0 "$spawner_pid" 2>/dev/null || break
    sleep 1
  done
  if ! test -f "$stack_info"; then
    if kill -0 "$spawner_pid" 2>/dev/null; then
      fail "$phase: stack never became ready with spawner still alive (boot too slow or helm failing)"
    else
      fail "$phase: spawner died during boot (script exited before readiness)"
    fi
    return 1
  fi
  # The info file is published before the helm starts, so its presence
  # alone proves nothing: the kill below must hit a LIVE stack, or a
  # boot self-exit would be indistinguishable from the cleanup.
  for _ in $(seq 1 30); do
    curl -s -m 2 -o /dev/null "$port" && return 0
    kill -0 "$spawner_pid" 2>/dev/null || break
    sleep 1
  done
  if kill -0 "$spawner_pid" 2>/dev/null; then
    fail "$phase: helm never answered $port with spawner still alive"
  else
    fail "$phase: spawner died while waiting for the helm to answer"
  fi
  return 1
}

boot_stack() {
  local phase="$1"
  spawner_pid=""
  script_pid=""
  rm -f "$stack_info"
  # A persisting spawner that direct-parents the script: plain
  # `bash -c 'bash ...'` would exec the script into its own pid, leaving
  # no spawner to kill. The direct-child shape is what production has
  # once the configured command's `exec` collapses Playwright's `sh -c`
  # hop (the layering guard pins that); these kill phases assume it.
  bash -c 'bash "$0" & wait' "$stack_script" &
  spawner_pid=$!
  await_stack_ready "$phase" || return 1
  script_pid="$(pgrep -P "$spawner_pid" | head -n 1)"
  test -n "$script_pid" || { fail "$phase: no script child of spawner"; return 1; }
  return 0
}

stack_state() {
  python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["state"])' "$stack_info"
}

# Processes still mentioning the state dir, except the ssh control mux:
# it self-exits within its 60s ControlPersist window, holds no port or
# lock, and the normal trap path has always left it behind too. The path
# is matched literally: its dots must not become wildcards that could
# sweep an unrelated process into a failure.
state_processes() {
  local escaped
  escaped="$(printf '%s' "$1" | sed 's/[][\\.^$*+?{}()|]/\\&/g')"
  pgrep -fa "$escaped" 2>/dev/null | grep -v "\[mux\]" || true
}
# NOTE: the sed program above is single-quoted inside the `$(...)` — that
# is what keeps `$`, `*`, and `\\` literal for sed. The outer double
# quotes only protect the substitution's result. Quoting the program
# itself with double quotes would expand `$*` and collapse `\\&`,
# silently unescaping the pattern and blinding every caller.

# Poll until the stack is fully gone: port closed, state dir and stack
# info removed, and no remaining process mentioning the state dir (shell,
# watcher, helm, supervisors, tmux clients).
await_gone() {
  local phase="$1" state="$2"
  local _
  for _ in $(seq 1 20); do
    if ! curl -s -m 2 -o /dev/null "$port" \
      && test ! -d "$state" \
      && test ! -f "$stack_info" \
      && test -z "$(state_processes "$state")"; then
      return 0
    fi
    sleep 1
  done
  return 1
}

report_leftovers() {
  local phase="$1" state="$2"
  curl -s -m 2 -o /dev/null "$port" && fail "$phase: port still answers"
  test -d "$state" && fail "$phase: state dir still exists ($state)"
  test -f "$stack_info" && fail "$phase: .stack-info.json still exists"
  local leftovers
  leftovers="$(state_processes "$state")"
  test -n "$leftovers" && fail "$phase: processes still reference state: $leftovers"
}

# After a failed phase, forcibly remove that stack so later phases start
# from a clean port and still give independent signals. The script and
# spawner go first by pid: their command lines never mention the state
# dir, so the pattern below would not match them. The bracket in the
# pattern keeps pkill from matching its own command line.
emergency_cleanup() {
  local state="$1"
  test -n "$script_pid" && kill -KILL "$script_pid" 2>/dev/null || true
  test -n "$spawner_pid" && kill -KILL "$spawner_pid" 2>/dev/null || true
  local pattern="${state%?}[${state: -1}]"
  pkill -KILL -f "$pattern" 2>/dev/null || true
  sleep 2
  rm -rf "$state"
  rm -f "$stack_info"
}

# Phase 0: the configured webServer command must exec through the shell.
# Playwright launches it as `/bin/sh -c "<command>"` detached; on Linux
# that shell is dash, which stays resident instead of replacing itself.
# Without an `exec` prefix the script's parent would be that dash wrapper
# — which survives the leader's death — and the orphan watcher would wait
# on a pid that never goes away. The kill phases below assume the fixed
# layering (leader direct-parents the script), so this phase pins the two
# facts it rests on: the CONFIGURED command starts with `exec` (read from
# playwright.config.ts, so a dropped prefix or a reformat fails loudly
# here instead of silently unpinning the guard), and this host's shell
# honors it (a stub hop collapses to its command). Boots nothing.
configured="$(sed -n 's/^ *command: "\(.*\)",\?$/\1/p' "$repo/e2e/playwright.config.ts" | head -n 1)"
case "$configured" in
  "exec "*)
    pass "webServer command starts with exec"
    ;;
  *)
    fail "layering-phase: webServer command is '$configured', expected an exec prefix (see playwright.config.ts)"
    ;;
esac
/bin/sh -c "exec sleep 60" &
hop_pid=$!
sleep 2
hop_comm="$(ps -p "$hop_pid" -o comm= 2>/dev/null | tr -d ' ')"
if test "$hop_comm" = "sleep"; then
  pass "shell honors exec (hop collapses)"
else
  fail "layering-phase: /bin/sh -c 'exec ...' left '$hop_comm' resident, expected it to collapse"
fi
kill -KILL "$hop_pid" 2>/dev/null || true

# Phase 1: the spawner is SIGKILLed — no teardown signal exists that the
# script could have received, so only the orphan watcher can clean up.
boot_stack "sigkill-phase" || exit 1
state="$(stack_state)" || exit 1
kill -KILL "$spawner_pid" || exit 1
wait "$spawner_pid" 2>/dev/null || true
if await_gone "sigkill-phase" "$state"; then
  pass "spawner SIGKILL frees the stack"
else
  report_leftovers "sigkill-phase" "$state"
  emergency_cleanup "$state"
fi
check_decoys "sigkill-phase"
spawner_pid=""
script_pid=""

# Phase 2: the spawner is SIGTERMed — the Playwright-without-a-handler
# shape this watcher exists for.
boot_stack "sigterm-phase" || exit 1
state="$(stack_state)" || exit 1
kill -TERM "$spawner_pid" || exit 1
wait "$spawner_pid" 2>/dev/null || true
if await_gone "sigterm-phase" "$state"; then
  pass "spawner SIGTERM frees the stack"
else
  report_leftovers "sigterm-phase" "$state"
  emergency_cleanup "$state"
fi
check_decoys "sigterm-phase"
spawner_pid=""
script_pid=""

# Phase 3: the normal path still works — TERM to the script itself (what
# Playwright's graceful teardown delivers) cleans up fully.
boot_stack "trap-phase" || exit 1
state="$(stack_state)" || exit 1
kill -TERM "$script_pid" || exit 1
# Bounded: the spawner exits only after the script does, so an unbounded
# wait here would hang exactly when the trap under test fails.
for _ in $(seq 1 10); do
  kill -0 "$spawner_pid" 2>/dev/null || break
  sleep 1
done
if await_gone "trap-phase" "$state"; then
  pass "script SIGTERM frees the stack"
else
  report_leftovers "trap-phase" "$state"
  emergency_cleanup "$state"
fi
check_decoys "trap-phase"
spawner_pid=""
script_pid=""

if test "$failures" -gt 0; then
  echo "$failures check(s) failed" >&2
  exit 1
fi
echo "all start-stack cleanup checks passed"
