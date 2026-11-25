# No test catches a regression in Delete's hangup of markerless tab shells

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

The fix that makes Delete give a macOS tab shell its hangup signal (so the shell can save its history) has no test that
would notice if Delete stopped passing the tab's identity to the cleanup. The fix could silently regress, and tab shells
on macOS would again be killed without saving history.

## Details

Concerns c8e1f31a94. Locations: `crates/farhelm-supervisor/src/service/teardown.rs:529-534` (target and roots built from
`HostSweepRoots`), `crates/farhelm-supervisor/src/service/session_hosts.rs:518-555` (the role split in `sweep_roots`),
and the test `delete_hangs_up_a_pinned_tab_shell_without_markers` in `crates/farhelm-supervisor/src/service/sweep.rs`.

The fix has three parts: (a) `SessionHosts::sweep_roots` classifies pinned host children into agents and tabs by
`HostRole` (live tabs and placeholder tabs always go to tabs; `RecordedHost::Ours` goes by `tag.role`); (b) teardown
builds `SweepTarget::WholeSession(host_roots.tabs...)` and passes it to `reap_process_tree`; (c)
`kill_process_tree_with_grace` adds identities in the `WholeSession` set to the SIGHUP list. The new unit test
constructs `SweepTarget::WholeSession(HashSet::from([ProcessRoot{pid,start}]))` by hand and calls `reap_process_tree`
directly, so it pins only (c).

`sweep_roots` and `HostSweepRoots` are consumed only by `teardown.rs`, and no test drives a Delete with a markerless
tab. The e2e Delete tests in `terminal_tabs.rs` run on Linux, where the tab shell's environment markers are readable, so
`found.hangup` already covers them. The new e2e `a_structured_bash_session_takes_input_and_saves_history_on_delete`
exercises a bash agent session, whose hangup comes from `ScopeUnits::with_main_launch`
(`hangup_main = launch.is_shell()`), not from the tab set. So changing `teardown.rs:529` back to
`WholeSession(HashSet::new())`, or routing `HostRole::Tab` roots into `agents` in `sweep_roots`, would keep every test
green while reintroducing the macOS history-loss bug the commit fixes.

Verify by reading the results of `grep -rn 'HostSweepRoots\|sweep_roots\|WholeSession(' crates/`. Fix: a
supervisor-level test that runs the Delete teardown with a tab host whose pinned child lacks markers (for example a
sleeper spawned without `SESSION_ID_ENV_VAR` that ignores SIGTERM, as the tab's child, or a seam that hides markers) and
asserts it died by SIGHUP; or at minimum a test that `sweep_roots` places a Tab-role host's child in `tabs`.

Bucket: other (a test coverage gap; the shipped code is correct as read). Not highest: the shipped code currently does
the right thing, the risk is a future regression, and history loss is not established data loss in the queue's sense.

Confidence: confirmed from source.

Possible cover: none known.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g12 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
