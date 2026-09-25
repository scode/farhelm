# The input client attaches by bare pane id

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Nothing visible today; risk for future commands that fall back to the current window.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as
`F12 / COR-INPUT-BARE-PANE`, tagged **possible**. Anchors and title: `tmux/input.rs:35-36`, `tmux.rs:2675` — The input
client attaches by bare pane id, making a tab the session's current window

tmux keeps a "current window" per session, and some commands fall back to it. The output client and the sink attach with
the exact session target `-t =<session>`. The input client from F11 instead attaches with `-t %N`, the terminal's bare
pane id (input.rs:35-36). For `attach-session`, a pane target also makes that pane's window the session's current
window. Reviewers verified on 3.7c that `-C attach -f no-output -t %1` against a tab's pane made the tab the current
window, and that it stayed current after the client exited. So once someone attaches to a tab, the session's current
window is that tab rather than the agent. That contradicts `new_window`'s own docs (tmux.rs:2675), which explain that
tabs are created with `-d` specifically so that "opening a tab must change nothing about any other terminal", and that a
bare session target must not silently address whichever tab was touched last. A bare `%N` is also not scoped to the
session; after a tmux server restart recycles pane ids, it could attach this client to an unrelated session and move
that session's current window.

Nothing visible goes wrong today, which is why this is only possible. Reviewers checked that every destructive or input
command Farhelm sends to a vanished `=<session>:.%N` pane fails outright ("can't find pane") rather than falling back.
That covers `send-keys`, `kill-window`, `respawn-pane`, `resize-window`, `set-option -w` and `capture-pane`. Only
`display-message`-style queries fall back to the current window's active pane, and those are used by `window_size` and
the replay's mode query, where a wrong answer is harmless or fails the replay. The risk is to future code that relies on
the current window. The suggested change is to attach with `-t =<session>` like the other two clients. The `send-keys`
commands already carry the session-paired pane target, so the attach does not need the pane.
