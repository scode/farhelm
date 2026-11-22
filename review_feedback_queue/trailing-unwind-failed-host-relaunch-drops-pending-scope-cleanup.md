# Failed replacement start in unwind_failed_host_relaunch omits pending scope cleanup when the row is gone

Reviewed commit: 729ed16229e0b2cd660f9898182bac305be679fd

## TLDR

If a session's row disappears while a restart is launching its replacement and the replacement host fails after starting
a scoped process, failing to confirm the scope teardown drops the cleanup obligation on the next supervisor start
instead of retrying it.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/host_restart.rs:768-787`.

Source-verified. Commit 729ed16229e0b2cd660f9898182bac305be679fd records `(id, generation)` in `pending_scope_cleanups`
only in `tear_down_relaunch_deleted_meanwhile` (`crates/farhelm-supervisor/src/service/core/host_restart.rs:825-839`),
reached when `spawn_agent_in_host` succeeds and `ConfirmRunning` finds the row missing (`host_restart.rs:313-325`).
However, `relaunch_into_host` has a second teardown path when `spawn_agent_in_host` fails after launching a host
(`Err(HostSpawnFailure::Host { .. })` at `host_restart.rs:280-288`), calling `unwind_failed_host_relaunch`
(`host_restart.rs:740-792`). If `end_launch_host` confirms the failed replacement host ended (or `HostLeft::Nothing`
because the host died after spawning its scoped child) and `reap_process_tree` fails because the scope cannot be
confirmed (`host_restart.rs:768-781`), `unwind_failed_host_relaunch` returns `RelaunchFailure::ambiguous` without
checking whether the session row still exists or recording a pending scope cleanup. Back in `Supervisor::relaunch`
(`crates/farhelm-supervisor/src/service/core.rs:13025-13101`), if `self.store.session(&id)` returns `Ok(None)`, the
session is not republished and only the launch host record `<id>.<gen>.host` remains on disk. On the next supervisor
start, `reclaim_orphaned_hosts` (`host_reclaim.rs:715-721`) finds nothing serving that host's socket
(`AtRecordedSocket::Gone`) and removes the host record without retrying the scope cleanup. Caveat: in normal in-process
operation, `restart_session_named` holds `lifecycle_locks.claim(session_id)` across `relaunch` (`core.rs:12274,12656`)
and `handle_delete_session` / `handle_delete_session_or_seal` wait on the same lock (`handlers.rs:1243,1389`), so row
disappearance mid-relaunch has the same narrow reachability boundary (external store deletion or lock bypass) as
`tear_down_relaunch_deleted_meanwhile` itself. Fix direction: in `relaunch` when `still_exists` is false on an ambiguous
failure with a recorded scope (or in `unwind_failed_host_relaunch` when `scope.is_some()` and the row is absent), record
`(id, generation)` via `store.record_pending_scope_cleanup`.

Reviewer metadata (g18-sqlite-and-restart-cleanup/f2): suggested bucket: other (could NOT be in highest: leaving
processes from a failed restart of an already-deleted session running is a resource leak, not loss of user data,
credentials, or active user work); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g18-sqlite-and-restart-cleanup/f2`.
