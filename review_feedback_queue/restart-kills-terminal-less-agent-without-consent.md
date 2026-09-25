# Restart kills a terminal-less session's agent without consent

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A restart can kill a running agent without the confirmation the product promises.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F6 / COR-RESTART-CONSENT`, tagged **possible**. Anchors and title: `service/core.rs:9918-9969`,
`service/core.rs:10005-10027`, `service/core.rs:9740-9746` — Restart treats a session with no recorded terminal as not
running and kills a possibly live agent without the required consent

Restarting a session whose agent is still running must be confirmed. The client has to send `stop_if_running`, and the
supervisor re-checks liveness through the tmux pane rather than trusting the client's status. `restart_session`'s docs
(core.rs:9740-9746) say "A status of `Unknown` — no terminal, or a launch never confirmed — is treated as
possibly-alive".

The code does the opposite for an entry with no recorded terminal (`entry.terminal` is `None`). The supervisor's
in-memory session entry can lack a terminal in two documented cases. One is an ambiguous create retained by
`publish_retained_launch` (core.rs:8258-8273), whose docs describe it as the handle on "an agent which may already be
running". The other is a supervisor reload that kept a row before tmux exposed its pane (teardown.rs:401-410). With no
terminal, `pane_state` and `alive_pane` are both `None` (core.rs:9918-9969), so the consent check is skipped entirely.

Restart then takes the "agent already exited" branch (core.rs:10005-10027). It runs the process sweep with the
`AgentOnly` target and no pane root, which SIGKILLs every same-user process carrying this session's agent marker
(`FARHELM_AGENT_ID`), including a live agent. `relaunch_into_terminal` then runs `tmux kill-session` on the session's
durable tmux name before creating a fresh one. Delete handles the same shape more carefully: it probes the durable name
with `has_session_for_terminal_less_delete` before acting.

SPEC.md says "Restart on a session whose agent is still running confirms, stops the agent, then relaunches." Here a live
agent is killed without that consent. It also skips the `StopRequested` intent and the stop annotation, which is the
direction the code's own comment calls the one that must not happen.

The premise is that a terminal-less entry's agent is actually alive at restart time, which the code explicitly allows
for. For a terminal-less entry, probe the durable tmux name as Delete does. If a live pane exists, require
`stop_if_running` and stop through `stop_live_agent`. If tmux cannot be asked, refuse.

For the user, a restart can kill a running agent without the confirmation the product promises.
