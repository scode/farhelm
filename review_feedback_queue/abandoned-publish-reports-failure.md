# A commit that times out still publishes while reporting failure

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Uploading a file can report "failed, nothing was stored" while the file actually lands — so the user retries and ends up
with the same file twice, or believes a sensitive upload never happened when it did.

## Details

Source: pre-pr-review-swarm, area data-transfer, 2026-09-17. Confidence: definite. Five lenses agreed independently
(state-lifecycle, general, systems, data-flow, edge-inputs). Coordinator confirmed the Drop mechanics and the
contradicted docs.

`commit_upload` publishes on a blocking thread (`staged.publish_no_clobber`: fsync plus a hard link into the session's
attachments directory), awaited through `await_disk_stage` with a 30 s bound
(`crates/farhelm-supervisor/src/service/uploads.rs:1100`). On timeout or cancel the operation is abandoned by dropping
its handle — but a dropped blocking task keeps running, and if its `link()` then succeeds, the file IS published while
the commit was already answered with failure (1107-1148). The abandoned task's `StagedStream` sets `finished = true` on
success (files.rs:649-650), so `Drop` removes only the temp name (710-712) and the published file stays permanently;
startup reconciliation never touches published files of known sessions. This contradicts the documented promise that "a
timed-out stage never contributes to a published file" (920-928): the rationale ("Drop removes the staging file") holds
for chunk writes, but for publication completion IS the link. Reachable three ways: client abort racing the commit,
connection teardown during the publish window, or a wedged filesystem hitting the 30 s bound. A retry then publishes the
same bytes under a `-1` name, and the abandoned publish can win a no-clobber race against it.

Suggested fix: do not abandon the publish task — keep the handle on the cancel/timeout path and hand it to a detached
reaper that awaits it and unlinks the destination if it succeeded late, before (or just after) answering the commit with
failure. The link-based no-clobber walk keeps a concurrent retry safe.
