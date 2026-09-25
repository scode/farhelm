# Ticker, listing and stop leave unread launch specs

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Credentials from a launch that never reached the agent stay on disk until the session is deleted.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F22 / COR-OBSERVER-SPEC-NOT-CLEANED`, tagged **definite**. Anchors and title: `service/ticker.rs:1049`,
`service/ticker.rs:1080-1110`, `service/listing.rs:224-275`, `service/handlers.rs:913` — The ticker, listing and stop
observers also clean launch specs only after Error

While the supervisor runs, several code paths watch for a session's pane dying and record the outcome in the database.
The ticker is the supervisor's periodic background pass (`service/ticker.rs:1049`, `:1080-1110`). `ListSessions` records
outcomes as it answers a listing (`service/listing.rs:224-275`), and single-session replies such as rename do the same
(`service/handlers.rs:913`). The Stop operation commits its own result. Like reload in F21, each of these calls
`cleanup_launch_artifacts` only when the committed outcome is **error**.

The trigger is a pane that dies before the shim ever ran: an rc file that runs `exec zsh` or `exit`, a crashing shell,
or the user stopping the session while a slow rc file is still running. Every observer records **exited**, or for Stop
an exited outcome annotated "stopped by user" (`sweep.rs` `StopCompleted`). The spec is left behind in every case.
`observe_entry` could reach Error through `wrapper_failure_detail`, but that classifier only applies to scoped launches
(never on macOS or Linux without a systemd user manager). Stop's annotated exit is also outside what the
sentinel/wrapper check can override. With the pane dead, the spec can never be consumed, and it survives until Delete.

The suggested fix: whenever any observer commits a final outcome for a dead or missing pane of the current launch,
best-effort remove that launch's spec, as in F21. This is the same leak as F21 but in the runtime observers rather than
startup reconciliation, so fixing one does not fix the other. F20 is about earlier generations, not the current one.

Restater note: the anchor `service/handlers.rs:913` is inside `session_info_now`, the observer used by the rename reply,
not by Stop. Stop's commit and its Error-only cleanup are at `service/handlers.rs:~1289-1300` and
`service/sweep.rs:~1749` (`stop_live_agent` recording `StopCompleted`). I confirmed that no spec cleanup happens on the
stop path either, so the claim holds; only the anchor is imprecise.
