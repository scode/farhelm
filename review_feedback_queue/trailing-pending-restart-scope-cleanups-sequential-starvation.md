# Sequential startup retry of pending restart scope cleanups can starve later obligations

Reviewed commit: 729ed16229e0b2cd660f9898182bac305be679fd

## TLDR

If multiple failed restart cleanups accumulate on a Linux host and the earliest one(s) stay stuck or slow to confirm,
restarting the supervisor can spend its entire startup cleanup budget on those earlier entries and never reach later
ones that could be cleaned up cleanly, leaving orphaned agent processes running across restarts.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/host_restart.rs:883-934`.

Source-verified. `SessionStore::pending_scope_cleanups` (`crates/farhelm-supervisor/src/store.rs:3430-3441`) returns
obligations in deterministic `ORDER BY session_id, generation`. `Supervisor::retry_pending_scope_cleanups`
(`crates/farhelm-supervisor/src/service/core/host_restart.rs:883-934`) processes them sequentially in a
`for (id, generation) in pending` loop inside a single `tokio::time::timeout_at(deadline, work)` bounded by
`ORPHAN_RECLAIM_BUDGET` (60s, `host_reclaim.rs:184`, wired at `core.rs:8122-8127`). Each unconfirmable obligation can
consume up to 15s in `podman::stop_before_signals` (`podman.rs:18,266`, capped at `deadline - 10s`), 7s to ~25s in
`kill_scope` (`KILL_GRACE` 5s + `SCOPE_CONFIRM_TIMEOUT` 2s, or `SYSTEMCTL_TIMEOUT` 5s per timed-out query), and 7s+ in
the fallback `kill_process_tree` (`sweep.rs:1629`), totaling ~14-35s per stuck obligation. Because unconfirmed
obligations stay in `pending_scope_cleanups` at the front of the deterministic sort order, 2-3 stuck obligations can
exhaust the 60s budget on every supervisor start and permanently starve later cleanable obligations. By contrast,
`reclaim_orphaned_hosts` (`host_reclaim.rs:369-428`) runs each item concurrently in a `JoinSet` so one slow item cannot
starve others. Fix direction: run pending scope cleanups concurrently with a bounded `JoinSet` (sharing `ScopeManager`'s
existing `Probing` coordination and the startup `deadline`) or randomize/rotate retry order so stuck obligations cannot
permanently starve others. Caveat: requires multiple accumulated unconfirmable delete-during-restart scope obligations
on the same host.

Reviewer metadata (g18-sqlite-and-restart-cleanup/f1): suggested bucket: other (could NOT be in highest: leaving
processes from an already-deleted session running is a resource leak, not loss of user data, credentials, or active user
work); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g18-sqlite-and-restart-cleanup/f1`.
