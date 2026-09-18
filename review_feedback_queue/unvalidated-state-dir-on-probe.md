# Probed registration with a bad state-dir path permanently bricks the entry

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Registering a probed host whose state-directory path is bad (empty or containing a NUL byte) permanently bricks the host
entry: every connection attempt fails with an opaque error, and the only recovery is to delete the host and re-add it,
losing its cached sessions and history. Re-probing can even overwrite a previously good path with a bad one.

## Details

Source: pre-pr-review-swarm, area stores, 2026-09-17. Confidence: definite. Same gap as
`unvalidated-state-dir-on-add.md`, reached through probe registration; coordinator verified.

`register_probed_ssh_host` (store.rs:3375, validation at store.rs:3380-3393) validates destination and `remote_farhelm`
but stores `remote_state_dir` unchecked on BOTH the insert and converge branches — the converge branch can overwrite a
previously good value with a bad one. Same consequence and recovery as the add path: an opaque dial failure on every
attempt, and remove-plus-re-add as the only fix (re-add mints a new HostId, cascading away cached sessions and create
history).

Suggested fix: apply the same refusal on both branches before writing.
