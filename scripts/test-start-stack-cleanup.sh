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
# the normal trap path and the reaping of a replacement "remote"
# supervisor, with an unrelated tmux server, process, and another stack's
# live run dir (holding a live terminal host) standing by as the "other
# stacks" that must survive. Every booted stack also gets a real terminal
# host planted in each of its two state dirs, which its cleanup must end. A
# layering guard first pins the `exec` prefix the watcher depends on: the
# Playwright leader must direct-parent the script, not a resident shell.
#
# Needs a built tree (cargo build, dx web build); picks its own free port
# per run and boots five real stacks on it plus a seconds-long layering
# guard, so a few minutes.
# NOT wired into CI: like the rest of the browser suite it is far too slow
# for per-PR runs. Run it by hand when start-stack.sh's lifetime handling
# changes.
#
# NOTE: no reliance on `set -e` — every step carries its own guard.

repo="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
bin="$repo/target/debug/farhelm"
fixtures="$repo/target/debug/farhelm-fixtures"
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
# The same clock as start-stack.sh's lifecycle markers, so a kill sent here
# can be lined up against the marker showing when the stack's trap ran.
stamp() {
  printf '%s' "${EPOCHREALTIME:-$(date +%s)}"
}

test -x "$bin" || { echo "missing $bin — run cargo build first" >&2; exit 1; }
test -x "$fixtures" || { echo "missing $fixtures — run cargo build first" >&2; exit 1; }
test -f "$repo/target/dx/farhelm-ui/release/web/public/index.html" || {
  echo "missing web dist — run dx build first" >&2
  exit 1
}
curl -s -m 2 -o /dev/null "$port" && {
  echo "something already answers $port — stop it first" >&2
  exit 1
} || true

# Decoys: the "other stacks". A tmux server on its own socket (kill-server
# scope must not escape), a plain process (no stray kills), and another
# stack's live run dir (rm -rf scope must not escape) holding a live
# terminal host (the terminal-host stop must not escape either; started
# below, once the trap that ends it is in place).
#
# The run dir is a real one in farhelm-teststate's terms: a fresh
# `mktemp` name in the sweep family (`fh-e2e.` plus six random characters,
# the only shape the sweep looks at) with its `fh-run.lock` flocked by this
# script, on fd 8, for the whole test. The lock file is backdated past the
# sweep's grace, so it is the held lock alone, not the lock's youth, that
# keeps a sweep off; a sweep that stopped checking the lock would reap the
# decoy and end its host. One sweep pass is observed directly below (the
# sweep oracle), beside a dead canary run that it must reap; every boot's
# startup sweep then passes over the decoy too, but those run unobserved
# (`|| true`, and a sweep reports only removals), so for them the decoy's
# survival shows only that no sweep that ran reaped it.
# Every long-lived child is spawned with `8>&-`, so that only this
# script holds the lock: once it exits, a decoy kept by a failed stop is a
# dead run that a later sweep retries.
command -v flock >/dev/null 2>&1 || { echo "flock(1) is required" >&2; exit 1; }
decoy_dir="$(mktemp -d /tmp/fh-e2e.XXXXXX)" || exit 1
exec 8>"$decoy_dir/fh-run.lock" || exit 1
flock -w 10 8 || exit 1
# Set file $1's times two hours back: past the sweep's grace for a lock.
backdate() {
  python3 -c '
import os, sys, time
then = time.time() - 2 * 3600
os.utime(sys.argv[1], (then, then))
' "$1"
}
backdate "$decoy_dir/fh-run.lock" || exit 1
# Premises: the sweep would judge this dir (its name has the family shape)
# and would find it locked (a second flock on the file cannot be taken).
[[ "${decoy_dir##*/}" =~ ^fh-e2e\.[A-Za-z0-9]{6}$ ]] \
  || { echo "the decoy $decoy_dir is not a sweep candidate" >&2; exit 1; }
if flock -n "$decoy_dir/fh-run.lock" true 8>&-; then
  echo "the decoy's lock is not held" >&2
  exit 1
fi
decoy_tag="decoy-${decoy_dir##*.}"
decoy_sock="/tmp/tmux-$decoy_tag.sock"
tmux -S "$decoy_sock" new-session -d -x 80 -y 24 8>&- || exit 1
sleep 300 8>&- &
decoy_pid=$!
echo "decoy" >"$decoy_dir/marker" || exit 1

spawner_pid=""
script_pid=""
decoy_host_pid=""
decoy_host_socket=""
# The terminal hosts planted in the stack under test: "<pid> <socket>" per
# line, reset by every boot.
stack_hosts=""
cleanup_test() {
  local _
  # A stack still up when the test aborts gets its own TERM first, so that
  # its trap ends the hosts planted in it; SIGKILL, which skips that trap,
  # is only for a stack that does not finish within the bound.
  if test -n "$script_pid" && kill -0 "$script_pid" 2>/dev/null; then
    kill -TERM "$script_pid" 2>/dev/null
    for _ in $(seq 1 15); do
      kill -0 "$script_pid" 2>/dev/null || break
      sleep 1
    done
  fi
  if test -n "$spawner_pid" && kill -0 "$spawner_pid" 2>/dev/null; then
    kill -KILL "$spawner_pid" 2>/dev/null
  fi
  if test -n "$script_pid" && kill -0 "$script_pid" 2>/dev/null; then
    kill -KILL "$script_pid" 2>/dev/null
  fi
  sleep 3
  "$fixtures" sweep-test-state 2>/dev/null 8>&- || true
  kill "$decoy_pid" 2>/dev/null
  tmux -S "$decoy_sock" kill-server 2>/dev/null
  # The decoy's host ends the way a stack's do. When that cannot be
  # confirmed, the directory stays, since its socket is the only way left
  # to reach the host, and the host is sent SIGTERM if it is still this
  # test's; once this script exits, the dir's lock is free and a later
  # sweep retries it as a dead run.
  if stop_state_hosts "$decoy_dir"; then
    rm -rf "$decoy_dir"
  else
    echo "the decoy's terminal host could not be confirmed stopped; keeping $decoy_dir" >&2
    if test -n "$decoy_host_pid" && host_running "$decoy_host_pid" "$decoy_host_socket"; then
      kill -TERM "$decoy_host_pid" 2>/dev/null
    fi
  fi
}
trap cleanup_test EXIT

# End the terminal hosts in state dir $1 with the command a stack's cleanup
# runs, and say whether none may still run. A dir with no `hosts` entry has
# none. The command's lines go to a log in that dir, which stays with it
# when the stop fails, and never to this script's stderr, which may be a
# closed pipe by the time a cleanup runs.
stop_state_hosts() {
  [ -e "$1/hosts" ] || [ -L "$1/hosts" ] || return 0
  "$bin" internal stop-terminal-hosts --state-dir "$1" \
    </dev/null >/dev/null 2>>"$1/stop-terminal-hosts.log" 8>&-
}

# Start a real terminal host in state dir $1, the way a supervisor starts
# one, and print "<pid> <socket>". Checked here, before anything relies on
# it: the pid must be a live process whose command line names the socket,
# which is what the host's own command line does.
plant_host() {
  local line pid socket
  line="$("$fixtures" spawn-terminal-host --farhelm "$bin" --state-dir "$1" 8>&-)" || return 1
  read -r pid socket <<<"$line"
  host_running "$pid" "$socket" || {
    echo "a planted terminal host is not running as one: $line" >&2
    return 1
  }
  printf '%s %s\n' "$pid" "$socket"
}

# Whether $1 is a live process serving terminal host socket $2. The command
# line check keeps a reused pid from passing for the host; `-ww` because a
# host's command line (two absolute paths) runs past ps's default width.
host_running() {
  case "$(ps -ww -p "$1" -o args= 2>/dev/null)" in
    *"internal terminal-host --socket $2") return 0 ;;
  esac
  return 1
}

decoy_host="$(plant_host "$decoy_dir")" || exit 1
read -r decoy_host_pid decoy_host_socket <<<"$decoy_host"

check_decoys() {
  local phase="$1"
  tmux -S "$decoy_sock" has-session 2>/dev/null \
    || fail "$phase: decoy tmux server died"
  kill -0 "$decoy_pid" 2>/dev/null \
    || fail "$phase: decoy process died"
  test "$(cat "$decoy_dir/marker" 2>/dev/null)" = "decoy" \
    || fail "$phase: decoy state dir touched"
  host_running "$decoy_host_pid" "$decoy_host_socket" \
    || fail "$phase: decoy terminal host died"
  test -S "$decoy_host_socket" \
    || fail "$phase: decoy terminal host's socket removed"
}

# The sweep oracle. A canary dead run sits beside the decoy: the same
# shape, its lock backdated the same way but held by nobody, and no
# sockets. One sweep pass from this script must name the canary as reaped,
# which shows that the pass judged this family's directories rather than
# erroring out early (it always exits 0), while the decoy and its host,
# judged in the same pass, stay.
canary="$(mktemp -d /tmp/fh-e2e.XXXXXX)" || exit 1
: >"$canary/fh-run.lock" || exit 1
backdate "$canary/fh-run.lock" || exit 1
sweep_output="$("$fixtures" sweep-test-state 2>&1 8>&-)"
case "$sweep_output" in
  *"reaped stale test state $canary"*) pass "sweep oracle: a sweep pass reaps a dead run beside the live decoy" ;;
  *)
    fail "sweep-oracle: the sweep did not reap the dead canary $canary: $sweep_output"
    rm -rf "$canary"
    ;;
esac
check_decoys "sweep-oracle"

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
    curl -s -m 2 -o /dev/null "$port" && {
      echo "$phase: stack ready at $(stamp)"
      return 0
    }
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
  stack_hosts=""
  rm -f "$stack_info"
  # A persisting spawner that direct-parents the script: plain
  # `bash -c 'bash ...'` would exec the script into its own pid, leaving
  # no spawner to kill. The direct-child shape is what production has
  # once the configured command's `exec` collapses Playwright's `sh -c`
  # hop (the layering guard pins that); these kill phases assume it.
  bash -c 'bash "$0" & wait' "$stack_script" 8>&- &
  spawner_pid=$!
  await_stack_ready "$phase" || return 1
  script_pid="$(pgrep -P "$spawner_pid" | head -n 1)"
  test -n "$script_pid" || { fail "$phase: no script child of spawner"; return 1; }
  # One host in each of the stack's state dirs, the local and the nested
  # "remote", standing in for the terminals a supervisor would have started:
  # the stack's cleanup must end both.
  local local_state remote_state local_host remote_host
  local_state="$(stack_state)" || return 1
  remote_state="$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["remote_state"])' "$stack_info")" || return 1
  local_host="$(plant_host "$local_state")" || { fail "$phase: planting a host in $local_state"; return 1; }
  remote_host="$(plant_host "$remote_state")" || { fail "$phase: planting a host in $remote_state"; return 1; }
  stack_hosts="$local_host
$remote_host"
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

# The planted terminal hosts of the stack under test that still run, one
# "<pid> <socket>" per line. A host's own command line names the state dir,
# so state_processes would list it too; this names it as a host.
running_stack_hosts() {
  local pid socket
  while read -r pid socket; do
    test -n "$pid" || continue
    host_running "$pid" "$socket" && printf '%s %s\n' "$pid" "$socket"
  done <<<"$stack_hosts"
}

# Poll until the stack is fully gone: port closed, state dir and stack
# info removed, the planted hosts ended, and no remaining process
# mentioning the state dir (shell, watcher, helm, supervisors, tmux
# clients, terminal hosts).
await_gone() {
  local phase="$1" state="$2"
  local _
  for _ in $(seq 1 20); do
    if ! curl -s -m 2 -o /dev/null "$port" \
      && test ! -d "$state" \
      && test ! -f "$stack_info" \
      && test -z "$(running_stack_hosts)" \
      && test -z "$(state_processes "$state")"; then
      return 0
    fi
    sleep 1
  done
  return 1
}

# Wait (bounded) until a pid runs with the command line start-stack.sh takes
# for a supervisor of this remote state dir. A stand-in that died early would
# otherwise let await_gone pass on absence without proving any cleanup. The
# pattern copies start-stack.sh's own supervisor lookup; change both together.
await_replacement_running() {
  local pid="$1" remote="$2"
  local _
  for _ in $(seq 1 20); do
    case "$(ps -p "$pid" -o args= 2>/dev/null)" in
      *" supervisor run --state-dir $remote "*) return 0 ;;
    esac
    sleep 0.25
  done
  return 1
}

report_leftovers() {
  local phase="$1" state="$2"
  curl -s -m 2 -o /dev/null "$port" && fail "$phase: port still answers"
  test -d "$state" && fail "$phase: state dir still exists ($state)"
  test -f "$stack_info" && fail "$phase: .stack-info.json still exists"
  local leftovers hosts
  hosts="$(running_stack_hosts)"
  test -n "$hosts" && fail "$phase: terminal hosts still run: $hosts"
  leftovers="$(state_processes "$state")"
  test -n "$leftovers" && fail "$phase: processes still reference state: $leftovers"
  dump_stack_state "$phase" "$state"
}

# What a failed phase still has to show, captured before emergency_cleanup
# destroys it. The only failure on record (FLAKES.md, 2026-09-25 parent
# SIGTERM cleanup) left every service running, and the cleanup then deleted
# the logs, so nothing said which startup step the stack was in or whether
# its trap had run. The process tree under the script names the foreground
# command the shell is blocked on and how long it has run (`etime`), which
# is where a deferred trap would be waiting; the log tails show what the
# two supervisors were doing at the time. The helm writes no log file of
# its own: its output, and start-stack.sh's lifecycle markers, are already
# in this run's console above.
dump_stack_state() {
  local phase="$1" state="$2" root pids log
  echo "--- $phase: stack state at $(stamp) ---" >&2
  root="${script_pid:-$spawner_pid}"
  if test -n "$root"; then
    # Every descendant of the script (watcher, helm, supervisors, and any
    # foreground curl or command substitution), walked from a single
    # snapshot of the process table rather than by matching names.
    pids="$(ps -A -o pid=,ppid= | awk -v root="$root" '
      { parent[$1] = $2 }
      END {
        keep[root] = 1
        do {
          grew = 0
          for (p in parent) if (!(p in keep) && (parent[p] in keep)) { keep[p] = 1; grew = 1 }
        } while (grew)
        sep = ""
        for (p in keep) { printf "%s%s", sep, p; sep = "," }
      }')"
    ps -o pid,ppid,pgid,stat,etime,wchan,args -p "$pids" >&2 2>/dev/null \
      || echo "(no live process under $root)" >&2
  fi
  for log in "$state"/*.log; do
    test -f "$log" || continue
    echo "--- last 40 lines of ${log#"$state"/} ---" >&2
    tail -n 40 "$log" >&2
  done
}

# After a failed phase, forcibly remove that stack so later phases start
# from a clean port and still give independent signals. The script and
# spawner go first by pid: their command lines never mention the state
# dir, so the pattern below would not match them. The bracket in the
# pattern keeps pgrep from matching its own command line.
#
# Terminal hosts are the exception to the forcible part. A signal sent is
# not a host confirmed gone, and removing the dir would delete the sockets
# that are the only way left to reach one, so hosts are skipped by the
# kill and ended through the stop command instead, and the dir goes only
# once both of its state dirs' stops succeed. Otherwise it is kept (its
# lock died with the stack's script) for a later sweep.
emergency_cleanup() {
  local state="$1" pid args signalled=""
  test -n "$script_pid" && kill -KILL "$script_pid" 2>/dev/null || true
  test -n "$spawner_pid" && kill -KILL "$spawner_pid" 2>/dev/null || true
  local pattern="${state%?}[${state: -1}]"
  for pid in $(pgrep -f "$pattern" 2>/dev/null); do
    args="$(ps -ww -p "$pid" -o args= 2>/dev/null)" || args=""
    case "$args" in
      *" internal terminal-host --socket "*) ;;
      # Could not be inspected: maybe a host, so not signalled, but waited
      # for like the rest.
      '') signalled="$signalled $pid" ;;
      *)
        kill -KILL "$pid" 2>/dev/null
        signalled="$signalled $pid"
        ;;
    esac
  done
  # Signalled is not gone, and anything of the stack's still running could
  # still start a host after the scan below; so the scan waits (bounded)
  # until everything signalled is seen gone, and the dir is kept otherwise.
  # shellcheck disable=SC2086 # the pid lists split into one pid per word
  if ! processes_gone 10 ${script_pid:-} ${spawner_pid:-} $signalled; then
    echo "emergency cleanup: processes of $state still run; keeping it" >&2
    rm -f "$stack_info"
    return
  fi
  local stopped=true
  stop_state_hosts "$state" || stopped=false
  stop_state_hosts "$state/remote" || stopped=false
  if [ "$stopped" = true ]; then
    rm -rf "$state"
  else
    echo "emergency cleanup: terminal hosts in $state could not be confirmed stopped; keeping it" >&2
  fi
  rm -f "$stack_info"
}

# Whether pid $1 is confirmed gone: `kill -0` fails, or `ps` reports a
# zombie. A `ps` that fails or prints nothing confirms nothing.
process_gone() {
  local stat
  kill -0 "$1" 2>/dev/null || return 0
  stat="$(ps -p "$1" -o stat= 2>/dev/null)" || stat=""
  case "$stat" in
    Z*) return 0 ;;
  esac
  return 1
}

# Wait about $1 seconds for every pid after it to be confirmed gone, and say
# whether they all are.
processes_gone() {
  local end=$((SECONDS + $1)) pid all
  shift
  while :; do
    all=true
    for pid in "$@"; do
      process_gone "$pid" || all=false
    done
    [ "$all" = true ] && return 0
    [ "$SECONDS" -ge "$end" ] && return 1
    sleep 0.2
  done
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
/bin/sh -c "exec sleep 60" 8>&- &
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
echo "sigkill-phase: KILL sent to spawner $spawner_pid at $(stamp)"
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
echo "sigterm-phase: TERM sent to spawner $spawner_pid at $(stamp)"
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
echo "trap-phase: TERM sent to script $script_pid at $(stamp)"
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

# Phases 4 and 5: the replacement "remote" supervisor. When
# terminal-multihost.spec.ts takes the stack's second host down, it starts a
# replacement detached from its Playwright worker (later projects on the
# same stack still need that host) and records its pid in a file the stack
# publishes. The stack's cleanup is then the only thing that reaps it, and
# it must reap exactly that process: phase 4 records a stand-in whose
# command line is a supervisor on this stack's remote state dir and expects
# it gone with the rest of the stack; phase 5 records the unrelated decoy
# process, the shape a stale file with a reused pid has, and expects it to
# survive (check_decoys).
replacement_phase() {
  local phase="$1" record="$2"
  boot_stack "$phase" || exit 1
  state="$(stack_state)" || exit 1
  local remote pidfile
  remote="$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["remote_state"])' "$stack_info")" || exit 1
  pidfile="$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["remote_replacement_pidfile"])' "$stack_info")" || exit 1
  if test "$record" = stand-in; then
    # `exec -a` gives a plain sleep the argv a real replacement has; it
    # cannot take the real supervisor's place, which holds the state dir.
    bash -c 'exec -a "$0" sleep 300' "farhelm supervisor run --state-dir $remote --boot-id-file $remote/boot-id" 8>&- &
    replacement_pid=$!
    echo "$replacement_pid" >"$pidfile" || exit 1
    # Still signal the stack on failure, so it is torn down either way.
    await_replacement_running "$replacement_pid" "$remote" \
      || fail "$phase: stand-in supervisor never ran as one, so the phase proves no cleanup (its command line: $(ps -p "$replacement_pid" -o args= 2>/dev/null || echo 'not running'))"
  else
    echo "$decoy_pid" >"$pidfile" || exit 1
  fi
  kill -TERM "$script_pid" || exit 1
  for _ in $(seq 1 10); do
    kill -0 "$spawner_pid" 2>/dev/null || break
    sleep 1
  done
  # await_gone also proves the stand-in is gone: its command line names the
  # state dir.
  if await_gone "$phase" "$state"; then
    pass "$phase: stack freed"
  else
    report_leftovers "$phase" "$state"
    emergency_cleanup "$state"
  fi
  check_decoys "$phase"
  spawner_pid=""
  script_pid=""
}
replacement_phase "replacement-phase" stand-in
replacement_phase "stale-replacement-phase" decoy

if test "$failures" -gt 0; then
  echo "$failures check(s) failed" >&2
  exit 1
fi
echo "all start-stack cleanup checks passed"
