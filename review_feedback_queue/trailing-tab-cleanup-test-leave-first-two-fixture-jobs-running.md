# Tab cleanup test can leave its first two fixture jobs running after failure

Reviewed commit: fcb6fa5e4a40e5e1a4bcc505ad5ffa32dfa95b79

## TLDR

A failure in the regression can leave the test's own background work running for the fixture's remaining sleep interval.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/host_tabs.rs:1022`.

host_tabs.rs:1010-1013 explicitly makes each job unmarked, immune to SIGHUP and independent of terminal reads. Lines
1022-1047 start the first two such jobs and rely on Close and Stop, the operations being tested, to kill them. Only the
third job gets the independent guard at :1053-1054. The harness guard covers host sockets (SPEC_impl.md:2378-2385,
harness.rs:1054-1064). Crates/terminal-host/SPEC.md:149-155 says host Shutdown hangs up the PTY and does not wait for
child or descendants. Arm each job's fixture guard as its PID is learned, before asserting product cleanup. Bucket:
other because impact is test-run hygiene. Confidence: confirmed by source that jobs with the asserted premises can
survive when a failure occurs before cleanup. The new unmarked shell also receives only the hangup at host shutdown, but
its response to terminal EOF was not established, so the finding does not count it as a confirmed survivor. Possible
coverage: none for the first two jobs.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_e2e/l5`.
