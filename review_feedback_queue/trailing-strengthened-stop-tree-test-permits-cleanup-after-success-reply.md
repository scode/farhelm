# Strengthened Stop tree test permits cleanup after the success reply

Reviewed commit: 76ebed7af03dbd2a1ed896993d7a5b945f317e33

## TLDR

The test can pass even if Stop tells the user it finished while the host's child or descendant is still running.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/host_terminals.rs:302`.

host_terminals.rs:294-310 now uses a hangup-immune descendant but retains the parent revision's post-reply polling for
both PIDs. harness.rs:2483-2495 permits up to the wait budget for each to disappear after Stop success. The subsequent
asynchronous Liveness::Exited wait is a distinct and legitimate wait for the host's exit record.
SPEC.md:2067-2073,2272-2279 requires confirmed process cleanup before reporting success. Sweep.rs:2127-2174 awaits the
reap before completing Stop, and handlers.rs:1153-1159 sends SessionStopped after the outcome. Assert that both
processes are already gone as soon as the reply is observed and keep polling only for publication of the exit record.
Bucket: other for the test gap. Confidence: confirmed by source for the allowed post-reply window. No premature reply
was observed. Possible coverage: none within this changed live-main Stop case. lifecycle_unit/f5 concerns the separate
Stop path for an already-dead child and its exit record.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_e2e/f10`.
