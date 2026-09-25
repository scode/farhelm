# Reload records a false "never started" error after a crash mid-restart

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A session restarted during a supervisor upgrade shows an error blaming the cgroup scope wrapper or the user's shell rc
files.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F4 / COR-RELOAD-FALSE-NEVER-STARTED`, tagged **definite**. Anchors and title: `service/core.rs:5543`,
`service/core.rs:5653`, `service/launch_artifacts.rs:57`, `service/core.rs:12253`, `service/core.rs:12384` — After a
supervisor exit during restart, reload records a false "agent was never started" error

This is the same crash window as F3, seen on hosts where launches are wrapped in a systemd user _scope_ (a transient
cgroup unit that tracks the agent's processes). Each launch writes a _launch spec_: a private JSON file
`launch/<id>.<generation>.json` holding the agent argv and session credential. Inside the tmux pane, the login shell
runs Farhelm's _launch shim_, which reads and deletes that file and then execs the agent. If the shim cannot exec, it
writes a _sentinel_ file recording the failure. Restart writes the N+1 spec (`core.rs:12253`) before asking tmux to
respawn the pane (`core.rs:12384`). If the supervisor dies between those two steps, the N+1 spec sits on disk unread and
the old pane is dead.

On startup, reload's sentinel branch runs before the stale-pane rule described in F3. With no sentinel present, it calls
`wrapper_failure_detail(…, row.launch_scoped, pane_dead = true)` (`core.rs:5543`). That classifier
(`launch_artifacts.rs:57`) concludes that a scoped launch with a dead pane and an unconsumed spec means "the agent was
never started: … the transient cgroup scope wrapper, or the login shell itself — exited first". Reload commits that as
`SentinelError`, a permanent `Error` outcome. The stale-pane rule that exists to keep this row `Launching` never gets a
chance to run, because the dead pane it sees is the previous generation's and says nothing about N+1.

The result is a durable error with a specific and false cause, pointing the user at their shell rc files or the scope
wrapper, when the only thing that happened was a supervisor restart mid-restart. The suggested fix is to apply the
stale-pane test (empty stored pane, generation above 0, found pane dead) before calling `wrapper_failure_detail`, and to
pass `pane_dead = false` when it matches, so the row stays `Launching`/unknown. Alternatively, the classifier could
require evidence that the dead pane belongs to this generation.
