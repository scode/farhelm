# pane_states skips marker queries when no session has two windows

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In that rare state, a still-running tab disappears from the UI and cannot be closed.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as
`F14 / COR-PANESTATES-MARKER-SKIP`, tagged **possible**. Anchors and title: `tmux.rs:3431` — `pane_states` skips the
tab/agent marker query when no session has two windows

Farhelm tells tabs from agent windows by tmux window options it sets itself (`@farhelm-tab`, `@farhelm-agent`).
`pane_states` lists every pane on the server; it runs on every session-list poll and every terminal lookup. It fetches
those markers with a second `list-panes` query, but skips that query whenever no tmux session on the server has more
than one window (tmux.rs:3431). The docstring's justification is that "a session with one window can have no tab". That
is false once a session's agent window is gone while a tab window survives: the session's only window is a tab. No
product path does this, but it can happen out of band, for example when someone runs `tmux kill-pane` from inside a pane
(panes inherit `TMUX`, which points at the private server). The test suite provokes this state deliberately.

In that state, and only while no other session on the server has two windows, the marker query is skipped. Every pane
then looks unmarked, and `tabs_from_pane_states` finds no tabs. The surviving tab disappears from the session list,
cannot be attached, resized or closed (terminal lookup answers not-found), and is not reaped when its shell exits. There
is also a worse edge. If a session record has no stored pane id (a launch that crashed before recording it), reload's
fallback in `agent_pane_from_states` picks the lowest unmarked window and would adopt the tab as the agent pane, so a
later stop or restart would reap or respawn the user's tab. The suggested change is to fetch markers whenever a tab
could exist, or simply whenever any session exists (one extra `list-panes` per poll), and to correct the docstring.
