# Stopping a session kills the user's own tmux server, and work in it, when its agent started it

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

If an agent starts a per-user shared program on demand while none is running yet, such as your own tmux server (agents
commonly run `tmux new -d` for background jobs) or an ssh ControlMaster connection, that program becomes the shared one
for the whole host. Work you later do outside Farhelm lives inside it: windows you open in that tmux server from an
ordinary SSH login, or ssh connections that reuse that master. Stopping or restarting the Farhelm session kills the
shared program and everything you ran inside it, which is your work, not the session's.

## Details

Found by the 2026-10-22 prioritized persistent review, group `036-kill-sweep-2of3` (finding f1).

Suggested bucket: highest (an ordinary Stop or Restart kills user processes and work that are not the session's).

Confidence: likely. The code path is confirmed by reading, and recording checked the marker claim and closure. That a
tmux server keeps the environment it started with comes from the `/tmp` experiment recorded in `TRIAGE_OUTCOMES.md`,
"sweep-can-claim-supervisor-or-tmux-server.md". How often agents start the user's first tmux server is unverified.

Mechanism: Farhelm removes `TMUX` and `TMUX_PANE` from agents (`PRIVATE_TMUX_ENV_VARS`,
`crates/farhelm-supervisor/src/launch.rs:129-143`; SPEC.md "Ownership during cleanup and provisioning"), so a plain
`tmux` in a session reaches the user's own server, or starts one when none is running. A server started that way is
forked from the agent, so it sits in the launch's cgroup scope and carries `FARHELM_SESSION_ID` and `FARHELM_AGENT_ID`
for its lifetime. Panes the user later creates in it, from any login, are its children.

On Stop (`stop_live_agent`, `crates/farhelm-supervisor/src/service/sweep.rs:1880`) and the restart reap
(`crates/farhelm-supervisor/src/service/core.rs` around 11342), `reap_process_tree` (`sweep.rs:1316`) first kills the
whole launch scope (`kill_scope`, `sweep.rs:1610`), which reaches the server and every pane in it. Then
`kill_process_tree` (`sweep.rs:1068`) claims the server through the agent marker (`SweepTarget::claims`,
`sweep.rs:252-261`), and the parent-PID closure in `enumerate_tree` (`sweep.rs:588-643`) expands from it to every pane.
Either mechanism alone is enough. An ssh ControlMaster/ControlPersist master the agent's ssh spawns first has the same
shape: it daemonizes in the scope with the markers, later user connections multiplex through it, and Stop drops them.

Trigger: a host where the user had no tmux server running (or no live ControlMaster for that destination) when the agent
first used one; the user then works in it from outside Farhelm and later stops or restarts the session.

How to verify: on a systemd host with no user tmux server running, start a session whose agent runs
`tmux new -d -s bg sleep 1000`. From a separate SSH login run `tmux new -s mine` and start a long job. Stop the Farhelm
session; the `mine` session and its job are gone.

Fix direction (a design decision, not mechanical): either specify this as accepted, as the tab case already is, or keep
shared per-user daemons out of agent containment. A code-level exclusion would need a way to recognize a shared server;
the spec forbids building that for the private tmux server but not for the user's own. Documenting it for users, with
advice to start such daemons outside sessions, is the cheap option.

Possible cover, for triage to judge (the reviewer marked it "probably covered" but not fully, and asked for an
independent check before any drop): SPEC.md "Lifecycle operations" says "Stop and delete reap everything the agent
started", and SPEC.md "Ownership during cleanup and provisioning" says "Session teardown covers ordinary agent
descendants, including background servers", so killing the agent-started server itself is specified. SPEC.md "Session
view" accepts the same consequence for a tab whose startup files start "a detached personal tmux server"
(`TRIAGE_OUTCOMES.md`, "tab-close-kills-rc-started-services.md"). No cover names an agent-started shared daemon or the
user's later work hosted in it from outside Farhelm. SPEC_impl.md's process-tree containment section is related.
