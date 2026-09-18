# A refused delete still destroys an in-flight upload

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Deleting a session with an upload still in progress destroys the upload even when the delete itself is refused — the
file is gone and the session wrongly reports it was deleted, although the session still exists.

## Details

Source: pre-pr-review-swarm, area supervisor-teardown, 2026-09-17. Confidence: possible (needs a refusal coincident with
an in-flight upload).

Delete cancels/uploads-joins with `session_gone=true` (teardown.rs:512) BEFORE any preflight, while archive
(teardown.rs:290-297) deliberately cancels only after its read-only preflights pass ("a refused archive must not discard
an upload and then claim nothing changed"). Concrete trigger: delete with an in-flight upload on a session whose tmux
session was renamed → unrecognized-owner `PaneProbe` refusal (teardown.rs:561-566), row retained ("The row and the map
entry survive the refusal", :558-560) — but the upload is already destroyed and its late commit is told
NotFound/"session deleted" for a session that still exists. The delete-side comment's "the transfer is doomed either
way" (teardown.rs:503) is false on every refusal path. Data loss (in-flight upload destroyed) plus misreport, on a
refused operation that promises to change nothing durable.

To verify, start an upload, rename the session's tmux session out from under it, then delete: the delete is refused yet
the upload is gone.

Suggested fix: move `abort_session_uploads` in `teardown_session` to just before `reap_process_tree`, mirroring archive
— zero cost to the write-window bound since the preflights are read-only; fix the comment.
