# Idle input clients let tmux notifications pile up

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After long idle periods with terminals open, tmux memory grows and the next keystroke can be slow.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as
`F11 / COR-INPUT-NOTIFY-PILEUP`, tagged **definite**. Anchors and title: `tmux/input.rs:22-72`, `tmux/input.rs:218`,
`tmux/input.rs:317-341` — Idle input clients never read tmux's server-wide notifications, so tmux server memory grows
without bound

Each attached terminal also has a third control-mode client, the input client, which carries keystrokes as `send-keys`
commands and reads tmux's confirmation of each. `open_input_client` (input.rs:22-72) attaches it with `-f no-output`.
That flag suppresses pane output only. tmux still sends the client its other notifications, many of them server-wide:
`%sessions-changed`, `%client-session-changed` and `%client-detached` for every other client's attach and detach,
`%unlinked-window-add/close/renamed` for windows in other sessions, and `%layout-change` for its own session. The
client's stdout is read only inside `InputClient::send` (input.rs:218), which skips this chatter while it waits for each
command's reply (`read_command_block`, input.rs:317-341). A terminal that is attached but never typed into never reads
its input client at all.

Once the 64 KiB pipe fills, tmux keeps queueing notifications for that client in its own memory, with no bound. The
reviewers measured this on 3.7c. About 5 KB of notifications accumulated per 20 cycles of ordinary churn: another client
attaching and detaching, a session created and killed, a window resized. One unread client plus 80,000 rename
notifications took the tmux server from 1.7 MB to 8.2 MB, compared with 3.4 MB for the same load with no such client.
Every Farhelm attach, detach, tab open, and session create or delete produces this kind of event, and each idle attached
terminal carries its own backlog.

The practical effect is slow. On a host where terminals stay open for days without typing, such as a desktop window
watching an agent, the tmux server that holds every session gradually grows. When a keystroke finally arrives, `send`
has to parse the whole backlog while holding the supervisor-wide `attachments` lock (see F4). The suggested fix is to
drain the input client continuously, for example with a small reader task that discards notifications and forwards only
the `%begin`/`%end`/`%error` reply blocks to `send` over a channel, keeping the existing reply-pairing contract.
