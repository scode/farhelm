# A mark-window failure at create drops the pane

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Rarely, a newly created session's agent runs but the session can't be opened until the supervisor restarts.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F11 / COR-CREATE-MARKWINDOW-DROPS-PANE`, tagged **possible**. Anchors and title: `service/core.rs:12415-12427`,
`service/core.rs:9306-9322`, `service/core.rs:8267-8271` — When marking the agent window fails at create, the pane id
tmux returned is dropped and a running agent is published without a terminal

This is the create-time counterpart of F10. When creating a session, `spawn_agent` runs tmux `new-session`, receives the
new pane id, and then tags the window with `mark_window`. The tag is a tmux window option that tells a later reload
which window holds the agent and which hold the user's extra terminal tabs. A tagging failure is deliberately fatal, but
it is returned as `SpawnFailure::Tmux { spec_path, error }` (core.rs:12415-12427), which cannot carry the pane id.

`launch_reserved` treats any `SpawnFailure::Tmux` as ambiguous and asks tmux whether the session exists. Here it does,
because `new-session` succeeded, so the `Ok(true)` branch (core.rs:9306-9322) keeps the launching record and publishes
the entry with `terminal: None` while the agent is starting. `publish_retained_launch`'s contract (core.rs:8267-8271)
says a terminal is present when the failed path received a pane from tmux. This path did receive one, but it never
reaches the caller.

As a result, the session has no terminal in memory until the supervisor next restarts and reload rediscovers the pane.
Reload can still find it, because with only one window it falls back to the lowest untagged non-tab pane
(`agent_pane_from_states`). No other runtime path republishes a pane for the entry. Until then the session cannot be
opened. Lifecycle operations read "no terminal" as "not running", so a Restart would skip the consent prompt and reap
the live agent, and Stop takes its terminal-less path.

This depends on `mark_window` failing right after a successful `new-session`, for example a tmux command timing out
under load, so it is rare. The fix is to add `pane: Option<String>` to `SpawnFailure::Tmux` and publish
`Some(Terminal { .. })` in `launch_reserved`'s `Ok(true)` branch. A retry of the idempotent `mark_window` before failing
could be added as well. What the user sees: rarely, a new session's agent is running but the session cannot be opened
until the supervisor restarts.
