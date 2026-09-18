# Upload staging holds the lifecycle claim across unbounded disk I/O

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If the disk wedges while an upload is starting, the whole session becomes permanently unmanageable — it can never be
stopped, deleted, or added to again until the supervisor restarts.

## Details

Source: pre-pr-review-swarm, area data-transfer, 2026-09-17. Confidence: definite (systems p1, data-flow p3;
state-lifecycle p1 corroborated as possible). Merged from three inputs; raised to definite at confirm — the mechanism is
fully verified and the only open premise is the wedged-filesystem hazard the module treats as in-model
(`await_disk_stage`'s entire reason to exist).

`stage_upload` acquires the session's lifecycle claim (uploads.rs:786, with a carefully interruptible claim-wait), then
performs `ensure_session_dirs` (mkdir, 832-834), `canonicalize` (843), and `StagedStream::create` (bare `spawn_blocking`
file creation, 883-887) as unguarded awaits — no signal observation, no time bound. Every other disk stage (chunk
writes, publication) goes through `await_disk_stage`'s cancel/timeout guards; creation was missed. On a wedge,
delete/stop/restart/archive and new begins all queue behind the claim forever, and the stuck transfer leaks its channel
and admission slot. No party can unstick it: delete hasn't acquired the claim yet so it hasn't signalled, and the
transfer isn't watching for signals anyway.

Suggested fix: run the mkdir/canonicalize/create section under the same guards as the other disk stages (signal-aware
select plus the `upload_disk_stage` bound), returning a `RequestError` on timeout/cancel like the claim-wait arm. Late
completion is safe to abandon: mkdir is idempotent, canonicalize is read-only, and a late-created temp is uniquely named
inside `.staging/`, where the startup sweep removes it.
