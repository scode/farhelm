# Windows the agent opens on the private tmux server are never reaped

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A dev server the agent started in its own tmux window keeps running after Stop, may still hold its port when Restart
launches the new agent, and may survive Delete — each reported as complete.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F23 / SEC-AGENT-WINDOWS`, tagged **possible**. Anchors and title: `service/sweep.rs:245-251`,
`service/sweep.rs:419-441`, `service/teardown.rs:408-413` — Windows the agent opens on the private tmux server are never
reaped by Stop or Restart, and Delete only SIGHUPs them

The agent runs inside a tmux session on the supervisor's private tmux server, with `$TMUX` pointing at that server. SPEC
supports "creating windows from inside a session", and agents use this for background servers, for example
`tmux new-window 'npm run dev'`. A process started that way escapes every reaping mechanism:

- **Parent-pid walk.** It is a child of the tmux server, not of the agent.
- **Marker scan.** tmux builds the new window's environment from the server's global environment, so the agent's
  `FARHELM_SESSION_ID` and `FARHELM_AGENT_ID` are not passed on (reviewer-verified). The scan (sweep.rs:245-251,
  sweep.rs:419-441) finds nothing.
- **Cgroup kill.** The tmux server forks it, so it is outside the agent's systemd scope.

Stop and Restart target only the agent's processes, never touch it, and report success. Restart can start the new agent
while the old server still holds its port. Delete's sweep finds nothing either. Only `kill-session`'s SIGHUP reaches the
pane's foreground process group (teardown.rs:408-413), anything nohup'd or setsid'd survives, and Delete reports success
without confirming.

SPEC says Stop reaps "everything the agent started" and Restart reaps a prior run's leftovers "never alongside them".
SPEC_impl says the sweep's `Ok(())` means nothing is left running. This is a supported way for an agent to start
processes, and no mechanism covers it.

The open premise is whether windows the agent creates count as "started by the agent"; the interaction itself is
supported. Put the session marker into the tmux session's own environment when the terminal is created
(`new-session -e FARHELM_SESSION_ID=<id>` or `set-environment -t =<name>`) so later windows inherit it. Add a distinct
"session window" marker kind so Stop can select these windows deliberately, without pulling the agent's rc-file daemons
into the legacy bucket; SPEC_impl excludes startup-file services from cleanup.

For the user, a dev server the agent started in its own tmux window keeps running after Stop, may still hold its port
when Restart launches the new agent, and may survive Delete, each reported as complete.
