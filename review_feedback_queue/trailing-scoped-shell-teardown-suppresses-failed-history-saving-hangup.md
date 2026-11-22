# Scoped shell teardown suppresses a failed history-saving hangup

Reviewed commit: fc2b0b46e6211c7ca4001d05d90221f35ee5bd97

## TLDR

If the scope's hangup request fails to start, a first-class shell can be forcibly stopped with no chance to save history
through that signal and the operation can still report success.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/sweep.rs:1898`.

tip scope.rs:1241-1253 starts one bounded command per signal. Scope.rs:1681-1705 and bounded_command.rs:67-78,212-218
propagate a command spawn error before any manager command dispatch. Tip sweep.rs:1892-1902 queues a SIGHUP error with
ordinary scope advisories, then :1903-1936 waits and can send SIGKILL, treating eventual scope collection as success.
This permits an isolated pre-dispatch HUP failure followed by a successful force cleanup of a SIGTERM-ignoring shell. A
later portable walk cannot hang up a shell whose identity is already gone. SPEC.md:2074-2075 and SPEC_impl.md:4713-4716
require the shell to get a hangup and grace. Verified cgroup collection answers the separate cleanup guarantee in
SPEC.md:2272-2279. Existing sweep.rs:3120-3187 checks errors reported despite scope retirement, and :3553-3587 checks
successful shell hangup. Fake scope.rs:1224-1238 records a signal before its error, so those assertions do not
distinguish a genuine pre-dispatch HUP failure. Preserve the cleanup result while retrying a safely deliverable hangup
before force, or surface the unconfirmed history-saving step when its delivery failed. Bucket: other for the
source-supported lost opportunity and silent result. Actual loss of user-owned history and the platform/workload
conditions that make it material were not established. Confidence: source-supported failure propagation and ordering,
with no observed runtime failure. Possible coverage: lifecycle_unit if a signal-specific delivery-failure assertion
exists. L6 covers successful-path history evidence separately.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `stop/d4`.
