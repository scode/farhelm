# Readiness regression does not establish that the first stage has exited

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The regression can pass while a terminal-start helper remains alive, leaving its claimed lifecycle guarantee untested.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed assertion gap.

Source locations: crates/farhelm/tests/terminal_host_spawn.rs:368-386.

the assertion at 375 excludes only the test process as the daemon's parent; zombie filtering at 376-380 does not reject
a live first stage. At the reviewed commit, terminal-host/src/spawn.rs:85-121 explicitly forks the daemon from the first
stage, with daemon execution at 90-94 and the first stage reading its readiness pipe at 110-120. Before the first stage
exits, its PID differs from the test PID, so the assertion already passes. Info at test 382-386 does not observe the
first stage. Establish the first stage's exit and reap directly, or prove that the daemon no longer has that owned
first-stage process as parent. The actual production implementation still waits for first-stage exit: original
supervisor hosts/spawn.rs:276-283, tip hosts/spawn.rs:420-444. Therefore this is a source-confirmed test oracle gap, not
a current production leak or a measured flake.

lines 369-378 obtain the long-lived host's parent, require only that it differ from the test PID, then reject zombie
children of the test. If spawn returns after the second stage binds and starts its child but before its first stage
exits, the host's parent is that still-running first stage, which differs from the test; that first stage is not a
zombie, so both assertions pass. The Info check at 380-384 establishes readiness, not first-stage exit. Require evidence
that the host is reparented away from the first stage, or observe that no owned first-stage process remains, rather than
merely excluding the test as parent.

Possible coverage and caveats: none; full starting-a-host contract at tip SPEC_impl.md:1763-1774 requires first-stage
exit status as readiness. Required coverage sources were searched in the initial pass; TODO Planned is empty, no
queue/plan/ledger item matches this first-stage-parent oracle, and no FILTER.md rule accepts persistent test blind
spots.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g031/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 1512cdb6e7db601f8b07cfe596bddf101b1a3e45;
independently checked unchanged at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only
review; runtime occurrence and frequency were not measured.
