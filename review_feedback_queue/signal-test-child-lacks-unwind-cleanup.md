# Signal regression child lacks unwind cleanup

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A failed signal-log regression can leave its owned sleeper running; the runner's leak report does not prove the child
was cleaned up.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The child acquisition lacks an unwind-safe owner covering later assertions. Pinned nextest inherited-handle leak
reporting is diagnostic evidence, not descendant cleanup. This is a test-owned process lifetime defect. Exact same-run
PID-reuse acceptance remains a separate narrow exclusion.

Source locations: `crates/farhelm-supervisor/src/service/sweep.rs:3522-3539`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis of original g179/f1, introduced by fb81031deb85a5fb9f1110b4aa9dbbe238f048af and reviewed at
d6645f771bee8ed03a66486ef8755834937d759d. The test starts sleep 60 and performs fallible identity lookup and
signal/assertion checks before any unconditional kill-and-reap guard exists. On a panic while that child is still live,
std::process::Child drops without killing/reaping; the testtrace macro and run_sync only catch/log/rethrow (macros
lib.rs:65-72; support lib.rs:975-991). New source checks resolved the prior nextest uncertainty: GitHub
cargo-nextest-0.9.148 tag object 69f89204b7e5b8e2c97432216a616cbe79caf4ba points to
cd1d6d5467322dbc0d39a80cb671d000ac302798. At that immutable source, nextest-runner/src/runner/executor.rs:929-936 exits
its running loop when the test process exits; :1071-1087 runs detect_fd_leaks; :1136-1146 returns the result.
detect_fd_leaks at :1471-1547 waits for descriptor EOF or timeout and returns Leaked, explicitly ignoring signals once
the test is done; it does not signal descendants. runner/unix.rs:29-48 sets a process group but its Job is an empty
value with no cleanup owner. Farhelm .config/nextest.toml:20-21 sets a five-minute slow-test boundary and a one-second
leak-failure boundary; the latter is reporting, not a descendant lifetime limit. Thus the fixture can survive the failed
test and nextest's one-second leak report for the remaining portion of its 60-second sleep, absent independent outer
cleanup. Exact whole-recorder cleanup at run termination remains outside these checks, so no unconditional full-minute
leak under the complete recorder is claimed. The ownership gap and nextest-only residual are confirmed by source, with
no runtime reproduction. Suggested bucket other; not potentially-highest. proposed_drop none. possible_cover unchanged:
no exact coverage; analogous prior fixtures/decisions have different scope. A kill-and-reap guard armed immediately
after spawn addresses this failure path.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g179/f1`, `a018/g179-f1`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
