# A fresh-terminal restart kills the session's tabs, then reports them as alive

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Restarting a session whose agent pane died can silently kill all of its extra shells (tabs) — and the restart reply
still lists them as present, so the UI offers tabs that no longer exist and their processes are gone for good.

## Details

Source: pre-pr-review-swarm, area supervisor-lifecycle, 2026-09-17. Confidence: definite. Two lenses agreed
(correctness-state-lifecycle p1, correctness-edge-inputs p1). Coordinator confirmed all four mechanics.

Contract: a restart touches the agent terminal alone; tabs and their attachments must survive it untouched
(core.rs:8764-8769, SPEC.md). `relaunch_into_terminal` reads the tab list before anything destructive
(`crates/farhelm-supervisor/src/service/core.rs:8181-8190`) and carries it into the reply on that premise. But when the
recorded pane is gone while the tmux session still exists (`reuse.is_none()`, core.rs:8231-8232), the fresh-terminal
branch runs `tmux kill-session` on the whole session (core.rs:8239-8243; `TmuxDriver::kill_session` issues
`kill-session`, tmux.rs:2527-2528), destroying every tab window, builds a new agent-only session, and still replies with
the pre-kill tab list via `publish_relaunched` (core.rs:8446-8460). Meanwhile `detach_for_restart` detaches only the
agent attachment (core.rs:8770-8773), so clients on the destroyed tabs keep streaming dead panes instead of being told
to reattach.

Suggested fix, confined to the fresh-terminal branch: report the post-restart discovery (empty) instead of carrying the
pre-restart tabs into the reply, and detach the orphaned tab attachments with a truthful reason, since unlike the
pane-reuse path their windows did not survive.
