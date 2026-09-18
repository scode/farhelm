# Deleting a terminal-less session while the server is down is always refused

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

A leftover session entry with no terminal (for example from a launch that failed early) can never be deleted while tmux
is down — every attempt fails and retries fail identically, until a server happens to exist.

## Details

Source: pre-pr-review-swarm, area supervisor-teardown, 2026-09-17. Confidence: definite.

For a terminal-less entry, delete checks `self.tmux.has_session()` and means to tolerate a missing server (`None`,
"nothing to kill") — but the guards test `error.to_string().contains("no server running" | "error
connecting to")`
(teardown.rs:774-777), and `to_string()` on an `anyhow::Error` renders only the OUTERMOST layer — which `has_session`
unconditionally sets to `"checking tmux session liveness"` (tmux.rs:2477, context wrap on every non-absent `Err`).
tmux's raw diagnostics live two layers down, so neither guard can ever match (the codebase's own comment confirms the
rendering rule: "anyhow displays only the outermost context by default", tmux.rs:~2228). The arms are unreachable: every
no-server/absent-socket case falls into the `Err` arm and fails the delete closed. Deleting a terminal-less session (a
launch that failed before its window, a restart-gap entry) while the private server is down — or its socket file is gone
— is DETERMINISTICALLY refused, contradicting the comment's own promise ("must still delete cleanly", "Both mean nothing
to kill", teardown.rs:760-763). Retries fail identically; the session is stuck until a server happens to exist. No test
covers this path (teardown.rs's test module has five tests, none exercising terminal-less delete against a dead server).

To verify, stop the private server (or remove its socket) with a terminal-less entry present and delete it: the delete
is refused.

Suggested fix: match against the full chain (`format!("{error:#}")`) — or better, follow the codebase's own
`tmux_said_any` standard: downcast to `TmuxCommandFailure` and match raw stderr exactly. A plain substring on the
rendered chain would reintroduce the path-embedding hole those docs explicitly close (a state-dir path containing the
phrases plus an unrelated error would skip a kill that was needed — the unlisted-but-running outcome this module exists
to prevent).
