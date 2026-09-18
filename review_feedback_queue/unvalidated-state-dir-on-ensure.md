# A bad state-dir line in the ensure file bricks a host on every boot

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

One bad state-directory path in the ensure-hosts file permanently bricks that host entry on every boot: every connection
attempt fails with an opaque error, and the only recovery is to delete the host and re-add it, losing its cached
sessions and history.

## Details

Source: pre-pr-review-swarm, area stores, 2026-09-17. Confidence: definite. Same gap as
`unvalidated-state-dir-on-add.md`, reached through the ensure path; coordinator verified.

`ensure_ssh_hosts` (store.rs:3515, up-front validation at store.rs:3516-3540) checks destination usability, in-batch
repeats, and `remote_farhelm`, but never `remote_state_dir`. A bad line in the ensure file therefore bricks a host on
every boot, against the batch's "validates every entry up front" all-or-nothing contract. Same consequence and recovery
as the add path: an opaque dial failure on every attempt, and remove-plus-re-add as the only fix (re-add mints a new
HostId, cascading away cached sessions and create history).

Suggested fix: validate each entry's `remote_state_dir` in the up-front loop so the batch still fails atomically.
