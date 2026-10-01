# Deleting a session pauses typing in every terminal on the host

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

While a session is being deleted, typing and resizing in every other terminal on the same host waits until the delete
has archived its checkouts and written the database, which can be a noticeable pause when several checkouts are
archived.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F7 / COR-DELETE-LOCK-ARCHIVE`, tagged **possible**. Anchor and title:
`crates/farhelm-supervisor/src/service/teardown.rs:476` — Delete still holds the terminal-attachments lock through
checkout archive moves, fsyncs and the final database delete.

This is the same lock as F4 (the supervisor-wide `attachments` lock that every keystroke, detach, pause and resize on
the host takes, e.g. the input path in `crates/farhelm-supervisor/src/service/connection.rs`), but a different problem:
not an unbounded wait, but bounded disk work done while holding it. After taking the lock at `teardown.rs:450`, Delete
keeps it (`:476-818`) through stopping the terminal forwarders, the tmux kill, renaming the session's attachment files
into a quarantine folder, store reads, archiving any fresh-checkout folders the session owned (directory moves followed
by parent-directory fsyncs), and the final database transaction that removes the row (which fsyncs on commit). It
releases the lock only around `:832`. An earlier fix, PR #1164, moved just the subsequent file removal out from under
the lock, and a test (`delete_releases_the_attachments_guard_before_removing_files`) pins that.

SPEC.md's "Waiting between operations on one host" says terminal input and resizing must not wait on a delete of any
session. While a delete is in this section, typing in every terminal on the host waits. On a healthy disk this is a
handful of fsyncs per delete (more when several checkouts must be archived); the materiality of that pause was not
measured, which is why the finding is "possible". Hung-disk cases are excluded by SPEC.md's "Healthy local filesystems".

The suggested fix is to do only the minimum inside the lock: take the session's attachments out and mark the session as
being deleted, so a new attach is refused. Then release the lock before the tmux kill, quarantine, archive moves and
database transaction, and re-take it only to send detach notices. Extend the existing guard-release test so it parks at
the archive step and checks the lock is free.
