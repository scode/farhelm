# Restart with checks custom_reporting and publishes restart_offer against stale row.launch/entry.snapshot before update_launch commits

Reviewed commit: 7164b111339f33f9ec0b296488c38102191f6d9b

## TLDR

When a user edits a registered custom agent's reporting or resumption setting and uses Restart with to apply it, a
conversation ID reported right after the restarted agent starts is judged against the previous launch's reporting
setting, and the Restart reply (and listings before launch update completes) report a stale restart offer. Enabling
reporting can permanently discard a valid startup report (leaving Restart unavailable), disabling reporting can admit
and persist a report and process pin before the new launch is saved, and enabling resumption via named Restart with can
return a stale NoResumeCommand offer (or stay stuck at resume=None if update_launch fails).

## Details

**high.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/vendor/positional.rs:139`.

In `Supervisor::report_positional_conversation`
(`crates/farhelm-supervisor/src/service/core/vendor/positional.rs:127-144`), custom-agent report admission reloads the
session row from SQLite under the capture claim, verifies `row.generation == generation`, and gates reporting on
`row.launch.custom_reporting() == Some(false)`. However, during `Restart with`, `Supervisor::relaunch`
(`crates/farhelm-supervisor/src/service/core.rs:12816-13004`) opens the new launch generation (`begin_relaunch_named`
increments `row.generation` while leaving `row.launch` unchanged), spawns and starts the new custom agent
(`relaunch_into_terminal` -> `relaunch_into_host` at
`crates/farhelm-supervisor/src/service/core/host_restart.rs:239-396`), confirms `Running` in SQLite, publishes the new
terminal and `claim.generation` into `self.sessions` (`publish_relaunched` at `core.rs:13297-13380`, still carrying the
old `entry.info.launch` and `entry.snapshot`), awaits `self.end_replaced` (tearing down the previous terminal host over
IPC/timeout) and `self.restart_reply` (SQLite checkout metadata lookup), and only _after_ `relaunch_into_terminal`
returns calls `self.store.update_launch(&id, claim.generation, &bundle.launch)` (`core.rs:12966-12970`) and swaps
`current` to `bundle.snapshot` (`core.rs:12993`).

Scope verified in Turn 2:

- `restart_with_refusal` (`core.rs:3652-3665`) rejects changing between built-in and custom agents (or between different
  custom agent IDs) and rejects dropping a resume command (`!replacement.has_resume()`), so those transitions cannot
  reach `relaunch`.
- `Restart with` + `named_start` _can_ transition `resume` from `None` to `Some(...)` (e.g., after ticking Supports
  session resumption on a custom agent registration; `restart_with_refusal` only checks `replacement.has_resume()`, and
  `restart_session_named:12336` checks `replacement.resume_argv()?.is_some()`). In that transition — as well as when
  toggling `custom_reporting` — `publish_relaunched` (`core.rs:13311-13317`) computes `info.restart_offer` from the
  _pre-restart_ `entry.snapshot` (returning stale `NoResumeCommand`, `NoConversationReporting`, or `Resume`),
  `ListSessions` (`status.rs:345-355`) reads the stale `entry.snapshot` during `end_replaced`/`restart_reply`,
  `core.rs:12994-12998` updates `invocation`, `agent_kind`, and `launch` on the `SessionRestarted` reply without
  recomputing `info.restart_offer`, and if `update_launch` fails (`core.rs:12967-12979`), `row.launch` and
  `current.snapshot` permanently keep the old `custom_reporting` and `resume: None`.
- Most critically for `custom_reporting`: because report admission uses `capture_locks` rather than `lifecycle_locks`
  and `report_files.rs:240-266` checks `row.generation == entry.generation`, any custom-agent report delivered once
  `publish_relaunched` runs (e.g., a wrapper invoking `farhelm custom-agent report-conversation --id` on startup, as
  documented in `custom-agent-contract.md` §4 and exercised in `hook_identity.rs:2343`) while `end_replaced` or
  `restart_reply` is in flight sees `row.generation == claim.generation` with the _stale_ `row.launch`:
  1. Flipping `custom_reporting` `false -> true`: `positional.rs:139` returns `ErrorKind::InvalidRequest` ("conversation
     reporting is disabled for this launch"). In `report_files.rs:250-265`, `error.kind != ErrorKind::Internal` and
     `relaunched_since` returns `false` because `row.generation` is already `claim.generation`, so the drop file is
     marked `Disposition::Settled` and permanently deleted.
  2. Flipping `custom_reporting` `true -> false`: `positional.rs:139` sees the old `Some(true)` and admits the report,
     committing the conversation ID and `ReportPin` to SQLite for the new generation despite `SPEC_impl.md` ("Custom
     agents") stating "A structured launch that disabled reporting refuses capture even if it supplied a resume
     command".

How to verify: Trace `Supervisor::relaunch` (`core.rs:12816-13004`) -> `relaunch_into_host` (`host_restart.rs:239-396`)
-> `publish_relaunched` (`core.rs:13297-13380`) alongside `report_files.rs:228-266`, `status.rs:345-355`, and
`positional.rs:122-144`.

Fix: Persist `bundle.launch` atomically in `begin_relaunch_named` (restoring the previous launch on `abort_relaunch`) or
before spawning/publishing, and pass `bundle.launch`/`bundle.snapshot` into `publish_relaunched` so `row.launch`,
`entry.snapshot`, and `info.restart_offer` all reflect the new generation before the replacement process can report or
be published.

Reviewer metadata (g42b_custom_agents_sup_service/f1): suggested bucket: high — switching a custom agent's reporting or
resumption setting with Restart with can permanently discard a valid startup report (leaving Restart unavailable), admit
a report when reporting was disabled, or return a stale restart offer; it does not directly cause security compromise or
loss of user-owned work/files, so it stays below `highest`.; confidence: confirmed (established directly from source in
`core.rs`, `host_restart.rs`, `report_files.rs`, `status.rs`, and `positional.rs` at
`1b3fe49af86979aebc2719965682a17f8c6366ac`).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g42b_custom_agents_sup_service/f1`.
