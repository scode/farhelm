# A failed delete strands attachments in quarantine until startup destroys them

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If a session delete fails at the last step, the session's saved files silently become unreachable — and the next
supervisor restart permanently deletes them while the session itself survives.

## Details

Source: pre-pr-review-swarm, area supervisor-teardown, 2026-09-17. Confidence: possible (needs a SQLite failure at row
removal).

In `teardown_session`, the fail-closed block quarantines the session's attachments directory (rename into
`quarantine/<id>-<uuid>/`, teardown.rs:832-833) and then removes the DB row (:841-844). If the row removal fails (SQLite
error — rare, but exactly the failure class the fail-closed design contemplates), the failure path (:848-858) sends
"detached during a failed delete" notices and returns `TeardownError::FailClosed` — while the
`quarantined: Option<PathBuf>` local is silently dropped. Nothing renames the directory back, and the handler cannot do
it either: `teardown_session` returns only `Result<(), TeardownError>`, so the parked path is lost to every caller. The
only production `discard_quarantined` callers are the success path and startup reconciliation, so no other path repairs
this. Two consequences: (1) interim — the session is retained, listed, and re-attachable, but its committed attachments
are unreachable (reads fail; the session dir is gone); a retry-delete still works, but the session serves a lie until
then; (2) if the delete is never retried, the next supervisor startup runs `discard_quarantine_root`, which removes
EVERYTHING in quarantine unconditionally (attachments.rs:446-458, no known-sessions check — the parked `{id}-{uuid}`
name couldn't match one anyway), permanently deleting the LIVE session's committed attachments while the session itself
survives. That is user-data loss caused by a failed delete — contradicting the design's own stated contract at the
quarantine site: "a crash between it and the commit leaves debris the next startup reconciles rather than a live session
whose attachments have silently vanished" (teardown.rs:820-826). That reasoning covers only a crash, not this returned
error — and the outcome it promises is impossible is exactly what this path produces.

To verify, fail the row-removal write during a delete and observe the parked directory is never renamed back.

Suggested fix: on the failure path in `teardown_session`, when `quarantined` is `Some(parked)`, rename it back to the
session dir (best-effort, log loudly on failure). The rename-back is safe: in-flight uploads were cancelled and joined
before the sweep and the caller's lifecycle claim blocks new transfers, so nothing can have recreated the directory in
between.
