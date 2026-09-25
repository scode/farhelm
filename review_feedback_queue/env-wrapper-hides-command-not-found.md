# Farhelm's env wrapper turns command-not-found into exit 127

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If `goose`, `pi` or `omp` isn't installed or on the login PATH, the session shows "exited 127" instead of a launch
error.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F18 / COR-ENV-WRAPPER-127`, tagged **definite**. Anchors and title: `service/core.rs:2800`, `service/core.rs:2511`,
`service/core.rs:2547`, `service/core.rs:2577`, `launch.rs:883-895` — Farhelm's own `env` wrapper turns "command not
found" for Goose, Pi and OMP into exited (127) instead of error

Farhelm starts each agent in two steps. The supervisor (the per-host daemon that owns sessions) writes a "launch spec",
a private JSON file holding the agent's argv. The user's login shell inside the session's tmux pane then runs
`farhelm internal launch <spec>`. That small program, the "shim", reads the spec and `exec`s the agent in place of
itself. The shim is also how Farhelm tells "could not start" apart from "started and then ended". If `exec` fails (for
example ENOENT because the program is not installed), the shim writes a "sentinel" file recording the failure
(`launch.rs:883-895`). Status classification turns a sentinel into the **error** state. A dead pane with no sentinel
becomes **exited** with the pane's exit code. SPEC.md "Creation" states the contract: error "when the agent process
could not be started at all (exec failure, command not found)", and exited "when it started and then ended".

That distinction breaks for three agent kinds. For Goose, Pi and OMP, Farhelm's reporter injection needs to hand the
agent a few private environment variables, such as the path of the `farhelm` binary its reporter hook should call.
`with_launch_environment` (`service/core.rs:2800`) does this by rewriting the argv to `env FARHELM_…=… goose|pi|omp …`.
It is called from the Goose, OMP and Pi branches at `:2511`, `:2547` and `:2577`. The shim therefore always `exec`s
`env`, which always exists, so the shim never writes a sentinel. If `goose`, `pi` or `omp` is missing from the login
PATH, `env` prints `No such file or directory` and exits 127. Farhelm records that as an ordinary exit with code 127.
Nothing in status classification treats 127 specially. The rewrite applies to every hooked Pi and OMP launch (the
default), every recognized interactive Goose launch, and again on every restart, because injection is redone at each
spawn.

For the user, a session whose program is not installed shows "exited (127)" instead of a launch error. That breaks the
SPEC promise, and the wrapper that causes it is Farhelm's own, not anything the user wrote. The suggested fix is to stop
using an argv `env` prefix for these launch-only controls. Carry them in the launch spec instead (for example a
`LaunchSpec::env` list with `#[serde(default)]` so older specs still parse) and apply them with `Command::env` in the
shim's `agent_command`. The shim then `exec`s the real program, and ENOENT produces a sentinel as it already does for
Claude and Codex. A more complete variant would have the shim recognize a leading `env NAME=value…` and apply those
words itself before `exec`ing the program. That would also cover the `env` prefix the helm's structured launcher adds on
its own.
