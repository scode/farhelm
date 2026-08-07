#!/usr/bin/env bash
# Boot the real stack for Playwright: TWO supervisors (each with its own
# private tmux) + helm (loopback :7434) + one fake-agent session, then run
# the helm in the foreground so Playwright's webServer can watch it.
#
# The helm takes no session or transport flags any more (PLAN_M6.md item
# 5): it drives every host in its registry, and the machine this script
# runs on is the registry's reserved local row — reached over the
# supervisor socket in the shared --state-dir below, which is why the
# first supervisor started here IS the helm's local host with nothing to
# register. The startup session is therefore created the way every client
# creates one, through POST /api/sessions, once the helm is serving. That
# makes this harness the first consumer of both new surfaces, which is the
# point: a create path only the UI exercises is a create path the harness
# cannot tell you is broken.
#
# The second supervisor is registered as an ssh-to-localhost "remote"
# through a generated --ensure-hosts file, so the stack Playwright drives
# is a FLEET rather than one host. It carries no sessions and nothing here
# operates on it, so every existing assertion — all of which are about the
# local host — sees exactly what it saw before, including list counts.
#
# Prerequisites (CI runs these; locally too):
#   cargo build
#   cd crates/farhelm-ui && dx build --platform web --release
#
# Also needs `curl` and `python3` on PATH: the startup session is created
# through the helm's own HTTP API now, and the two JSON documents involved
# (the hosts list, the create body) are not things a shell should be
# parsing or building by hand. Both ship with every distribution this
# suite runs on.
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
# Assigned BEFORE the trap below, not where it is first used. The cleanup
# expands it, so a signal arriving while it was still unset expanded to
# `tmux -S /tmux.sock kill-server` — a command aimed at a path outside this
# run entirely. Set-then-trap is the ordering that makes that
# unconstructible rather than unlikely.
remote_state="$state/remote"
# Where this run publishes what it built, for the multi-host tests that
# have to reach BEHIND the API — killing the "remote" supervisor to make a
# host go unreachable, and re-registering it afterwards with the same
# install fields the ensure file used.
#
# Under the repo rather than in /tmp, and that is the security half of the
# choice rather than convenience: every other path here is a fresh mktemp
# dir precisely because a fixed, predictable /tmp name lets another local
# user pre-create it, and a fixed name for THIS file would reintroduce
# exactly that. The repo checkout is already the user's own. Removed by the
# cleanup trap, and gitignored, so a HANDLED exit or signal leaves no
# misleading leftover describing a stack that no longer exists — an
# untrappable death (SIGKILL, OOM) can still strand a stale file, so
# readers must not treat its presence as proof of a live stack.
stack_info="$repo/e2e/.stack-info.json"
auth_state="$repo/e2e/.auth/storage-state.json"
mkdir -p "$work" "$remote_state" || exit 1

# Trap installed BEFORE anything is spawned: a TERM during the socket
# wait below would otherwise exit with no trap and orphan the supervisor
# plus its daemonized tmux server, which nothing reaps for an hour.
cleanup() {
  kill "${helm_pid:-}" 2>/dev/null
  kill "${sup_pid:-}" 2>/dev/null
  kill "${remote_sup_pid:-}" 2>/dev/null
  rm -f "$stack_info"
  rm -f "$auth_state"
  # The tmux servers daemonize out of this process group, so killing a
  # supervisor does not reap its own; each needs its own shutdown, and the
  # "remote" has one of its own precisely because its state dir is
  # isolated.
  tmux -S "$state/tmux.sock" kill-server 2>/dev/null
  tmux -S "$remote_state/tmux.sock" kill-server 2>/dev/null
  rm -rf "$state"
}
trap cleanup EXIT
trap 'exit 143' TERM INT

"$bin" supervisor run --state-dir "$state" >"$state/supervisor.log" 2>&1 &
sup_pid=$!

# A SECOND supervisor, on its own state directory inside this run's
# tempdir, registered below as the ssh-localhost "remote" — the two-host
# fleet PLAN_M6.md's testing decisions call for, with the harness as the
# ensure file's first consumer.
#
# Isolated state dir, always: this "remote" runs on the same machine, and
# `remote_state_dir` on its registry row is what keeps a host that says
# remote from ever touching the developer's real ~/.local/state/farhelm.
# It lives UNDER $state so the existing cleanup and the stale-run sweep
# both already cover it.
#
# Where passwordless self-ssh is unavailable the second host simply shows
# unreachable, which is an honest state and not a failure: nothing here
# creates sessions on it, so the list and every existing assertion are
# unaffected — and the multi-host tests skip loudly rather than failing
# (terminal.spec.ts's fleet probe). Those tests are the fleet's consumer,
# and two of them reach past the API at this supervisor: one kills it to
# drive a host through unreachable, and both re-register it. Everything
# they need to do that is published in $stack_info below.
"$bin" supervisor run --state-dir "$remote_state" >"$state/remote-supervisor.log" 2>&1 &
remote_sup_pid=$!

# Give both supervisors a moment to bind their sockets before the helm
# dials. The local one is waited on strictly (the helm's own local row
# reaches it, and every existing test operates there); the remote's is
# best-effort for the same reason its ssh reachability is.
for _ in $(seq 1 50); do
  test -S "$state/supervisor.sock" && break
  sleep 0.1
done
for _ in $(seq 1 50); do
  test -S "$remote_state/supervisor.sock" && break
  sleep 0.1
done

# The ensure file registering the remote. Generated rather than checked in
# because two of its three values are this run's own paths.
ensure="$state/ensure-hosts.json5"
python3 -c '
import json, sys
print(json.dumps({
    "hosts": [{
        "ssh": "localhost",
        "remote_farhelm": sys.argv[1],
        "remote_state_dir": sys.argv[2],
    }],
}))
' "$bin" "$remote_state" >"$ensure" || exit 1

# Published BEFORE the helm starts, deliberately: Playwright treats the
# stack as ready the moment the helm answers, so a file written after that
# could be missing for the first test that looks. Everything in it is
# already known here, and the one thing that is not — the helm's own host
# ids — the tests read from /api/hosts, which is the authority for them
# anyway.
#
# The state directory is now published for one narrow reason: auth.spec.ts and
# the global setup invoke the shipped token CLI against it. Browser behavior
# still reaches the helm through HTTP; no test reads helm.db directly.
python3 -c '
import json, sys
print(json.dumps({
    "farhelm": sys.argv[1],
    "remote_state": sys.argv[2],
    "remote_supervisor_pid": int(sys.argv[3]),
    "remote_ssh": "localhost",
    "state": sys.argv[4],
}))
' "$bin" "$remote_state" "$remote_sup_pid" "$state" >"$stack_info" || exit 1

# Mint before the helm starts so its first protected request sees the same
# durable token the harness CLI printed. It is captured only long enough to
# pipe through stdin into the JSON request body, never placed in an argv;
# Playwright obtains its credential independently through the same command.
token="$("$bin" helm token show --state-dir "$state")" || exit 1

# The helm runs as a child, NOT via exec: bash does not run EXIT traps
# across exec, so an exec'd helm would leave the supervisor and the
# private tmux server orphaned after every run.
"$bin" helm run \
  --state-dir "$state" \
  --port 7434 \
  --ui-dist "$dist" \
  --ensure-hosts "$ensure" &
helm_pid=$!

base="http://127.0.0.1:7434"
auth_header="$state/harness-auth-header"

# Exchange once for the curl-based setup path. The browser uses its own
# exchange in global-setup.ts. Each consumer holds its own explicit device
# secret, matching the product boundary instead of sharing ambient state.
token_body="$(printf '%s' "$token" | python3 -c 'import json, sys; print(json.dumps({"token": sys.stdin.read()}))')" || exit 1
authenticated=false
for _ in $(seq 1 200); do
  kill -0 "$helm_pid" 2>/dev/null || {
    echo "helm exited before token exchange" >&2
    exit 1
  }
  exchange_body="$(printf '%s' "$token_body" | curl -sS -m 5 --fail-with-body \
    -X POST "$base/api/auth/token" \
    -H 'content-type: application/json' \
    --data-binary @- 2>/dev/null)" && {
      device_secret="$(printf '%s' "$exchange_body" | python3 -c '
import json, sys
value = json.load(sys.stdin).get("device_secret")
if not isinstance(value, str) or not value:
    raise SystemExit(1)
print(value)
')" || exit 1
      printf 'Authorization: Bearer %s\n' "$device_secret" >"$auth_header" || exit 1
      chmod 600 "$auth_header" || exit 1
      authenticated=true
      break
    }
  sleep 0.1
done
test "$authenticated" = true || {
  echo "the helm never accepted the harness web token" >&2
  exit 1
}

# The id of the reserved local row, once its connection to the supervisor
# above is actually up.
#
# Both halves matter. Asking /api/hosts rather than assuming an id keeps
# this script off an implementation detail of how helm.db mints rows; and
# waiting for `connected` rather than merely for the API to answer is what
# stops the create below from racing the connection — a create against a
# host that has not finished connecting is refused as a precondition
# failure, correctly, and would look like a flaky harness.
connected_local_host() {
  local body id
  for _ in $(seq 1 200); do
    kill -0 "$helm_pid" 2>/dev/null || {
      echo "helm exited before it began serving" >&2
      return 1
    }
    body="$(curl -sS -m 5 -H "@$auth_header" "$base/api/hosts" 2>/dev/null)" && {
      id="$(printf '%s' "$body" | python3 -c '
import json, sys
for host in json.load(sys.stdin)["hosts"]:
    if host["kind"] == "local" and host["state"]["phase"] == "connected":
        print(host["id"])
        break
' 2>/dev/null)" && test -n "$id" && {
        printf '%s' "$id"
        return 0
      }
    }
    sleep 0.1
  done
  echo "the local host never reported a connected supervisor" >&2
  return 1
}

host="$(connected_local_host)" || exit 1

# The body is built by python rather than by string interpolation because
# the invocation carries shell quoting of its own ('$bin' ...) that would
# otherwise have to survive being pasted into JSON by hand.
# The invocation is quoted with `shlex.join` rather than by wrapping $bin in
# literal single quotes: the supervisor parses it with shell-words, so a
# checkout path containing an apostrophe (or a space, or a quote) would
# otherwise produce an invocation that parses into the wrong argv — or fails
# to parse at all. Real quoting costs nothing here and removes the whole
# class.
create_body="$(python3 -c '
import json, shlex, sys
print(json.dumps({
    "cwd": sys.argv[1],
    "invocation": shlex.join([sys.argv[2], "internal", "fake-agent", "--script", "basic"]),
    "title": "e2e-session",
    "host": int(sys.argv[3]),
}))
' "$work" "$bin" "$host")" || exit 1

curl -sS -m 30 --fail-with-body \
  -X POST "$base/api/sessions" \
  -H "@$auth_header" \
  -H 'content-type: application/json' \
  -d "$create_body" >/dev/null || {
  echo "creating the startup session through the API failed" >&2
  exit 1
}

wait "$helm_pid"
