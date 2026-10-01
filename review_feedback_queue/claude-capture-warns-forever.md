# A missing Claude transcript logs a warning every few seconds forever

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

Every exited Claude session whose conversation file was deleted makes the supervisor log a warning every couple of
seconds for as long as the session exists, filling the journal and burying real warnings.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F24 / COR-CAPTURE-WARN-LOOP`, tagged **definite**. Anchor and title:
`crates/farhelm-supervisor/src/service/capture.rs:1714` — a missing or changed Claude conversation record is re-checked
and warned about on every capture pass, forever.

Farhelm learns which Claude conversation a session is running (so it can offer Resume) partly by scanning Claude Code's
on-disk conversation records. Once a session's conversation has been captured this way, the supervisor's capture pass,
which runs on the supervisor's 2-second ticker over every session it holds (exited ones included), re-checks that record
file with `reverify_capture` (`capture.rs:1698-1774`). It stats the file and, if it changed, re-reads it to confirm it
still names the same conversation.

Every unhappy outcome of that check just logs a `warn!` and returns without changing anything. That covers the file
being gone, failing to stat it, the file now naming a different conversation, the file being unrecognizable, and a read
error. Nothing records that this verdict was already reached. The common case is an exited Claude session kept around
for resume whose transcript was deleted, by Claude Code's own cleanup or by the user. It logs a WARN every couple of
seconds for as long as the session exists. That survives supervisor restarts too, because on reload the record path is
restored from the database (`core.rs:5832-5838`) and checked again. When the record changed but still exists, the whole
file is re-read on every pass, because the stored stamp (size and mtime) is never updated in those branches.

The result is a journal that fills on hosts with old sessions, real warnings buried in noise, and repeated filesystem
reads for a verdict that cannot change. Keeping the captured identity is correct and intentional: the docs say a missing
file must never un-claim a conversation. Only the repetition is the bug. Suggested change: in the "gone" branch,
atomically replace the captured record path with an empty one. An empty path already hits an early return at
`capture.rs:1711`, and the conversation id itself is kept. In the other branches, store the current stamp after warning,
so the warning repeats only if the file changes again.
