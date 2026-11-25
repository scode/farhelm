# Drift copy-back treats "terminal hosts ended" as "agent gone"

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

When the maintainer runs the live drift checks for Grok or Antigravity, each case copies files the agent rewrote in its
private copy of the configuration (a refreshed login, say) back into the maintainer's real configuration. The test
decides that is safe as soon as its terminal hosts are gone, but the agent itself can still be running: after a Delete
that could not confirm the agent was killed, after a session create that failed partway (the test never deletes that
session), or when the agent left a detached process outside Farhelm's cleanup. An agent that saves again after the copy
loses that save with the private copy, without any note, which can sign the maintainer out of that vendor. The first two
cases fail the drift case visibly, though nothing says a vendor write may have been lost; the third can follow a passing
case.

## Details

Concerns b784617a19, which introduced the `stopped` gate, `DriftEnv::with_session` and the rationale; the rationale is
repeated at the tip after 39f269bc0.

Main references: `crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs:741-759` (`DriftEnv::with_session`:
`create_session_with_extras(..).await?` returns before Delete on a failed create, and Delete's error goes only into
`after_cleanup`), `:873-875` (`stopped = state.close()`, then `config.finish(stopped.is_ok())`);
`crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:589-595` (the `finish` doc: "Copy-back waits for
confirmed host shutdown, so a still-running vendor cannot race it") and the module doc at lines 18-21 of the same file.

Host end does not stop the agent: `crates/terminal-host/SPEC.md`, "Ending", says the host "exits 0 without waiting for
anything: not for the child, nor for anything it started". On the normal path the agent is stopped by the session
Delete, whose teardown fails with `TeardownError::Sweep` when the process-tree kill is not confirmed
(`crates/farhelm-supervisor/src/service/handlers.rs:1180-1340`, `teardown_failure_message`; the helm client's
`delete_session` at `crates/farhelm-helm/src/client.rs:3837-3851` says "Only `Ok` means the session and its state are
actually gone"). The teardown's `stopped` ignores that, so copy-back runs as confirmed on three routes.

Route 1: Delete returns a Sweep error (agent tree not confirmed killed) while the hosts are later confirmed ended by
`state.close()` (`TestDir::close` -> `hosts::stop_hosts_within`).

Route 2: session creation fails after the launch started. `with_session` returns through `?` and never calls Delete. The
supervisor's failed-create rollback (`crates/farhelm-supervisor/src/service/core.rs:11668-11705`) runs
`reap_process_tree` with `ScopeKillFailure::Warn`, so a scope-kill failure is only logged (`sweep.rs:275-281`), and when
the portable sweep itself fails it keeps the session as a launching record and returns an error, which the drift case
also never deletes.

Route 3: a detached process that scrubs or overwrites its environment markers escapes the marker sweep (the `sweep.rs`
doc section "What the markers do NOT deliver"; SPEC_impl.md, "Process-tree ownership": "SPEC.md accepts that such a
detached process escapes the sweep"), and per-launch systemd scopes contain it only where a functional user manager
exists.

On every route `Mirror::copy_back` scans `links` once, a rename-save that lands after that scan is never seen, no
`dropped_without_shutdown` note is emitted, and the mirror's tempdir is deleted on return. Routes 1 and 2 fail the case
visibly (the delete or create error is kept), but nothing says a vendor write may have been lost; route 3 can follow a
passing case. The doc comments credit host shutdown, which will mislead a future editor who, say, moves deletion after
the gate or adds a category that skips `with_session`.

Verify by reading the cited lines. Fix: have `DriftEnv` track whether every session it created was deleted successfully
and whether any create failed, and pass `stopped = state.close().is_ok() && all_deletes_confirmed && no_failed_create`
to `finish`, so the unconfirmed routes produce `dropped_without_shutdown` notes instead of a copy-back. For route 3,
re-scan the mirror after copy-back for links replaced since the scan and name them. Correct the doc comments to name
Delete's sweep as the guarantee and state its accepted limit.

Bucket: other (maintainer test tooling; a lost vendor login is recoverable by logging in again). Not highest: maintainer
tooling only, with no credential exposure.

Confidence: confirmed for the control flow of routes 1 and 2 and for the escape in route 3; likely for the consequence,
with the unverified premise that a supported vendor (Grok or Antigravity) rename-saves a file while alive on one of
these routes before the mirror is deleted. Route 2 needs the vendor to have run long enough during a failed create to
write such a file, which makes it the least likely.

Possible cover: SPEC_impl.md, "Process-tree ownership" ("a program that rewrites its process title can overwrite it, and
SPEC.md accepts that such a detached process escapes the sweep"), touches route 3 only. That acceptance concerns the
product's stop promise and does not make the test's copy-back claim true, so it is context, not a drop basis; routes 1
and 2 have no cover.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g01 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed. Widened after an independent check reported routes 2 and 3, which the
reviewer confirmed against source.
