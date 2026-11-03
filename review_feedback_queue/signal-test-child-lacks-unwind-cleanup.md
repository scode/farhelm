# Signal regression child lacks unwind cleanup

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A failed signal-log, supervisor or terminal-host fixture can leave its owned processes running. The runner's leak report
does not prove those processes were cleaned up.

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

## Additional fixture ownership gaps through a98147fb

Reviewed commit for this additional scope: `a98147fb2eaaefb448473107529ada69114d7b03`. These are source-confirmed
conditional test-ownership gaps, all `other`; no production shutdown defect, runtime occurrence, guaranteed survivor
duration under the outer recorder or loss of user-owned work is claimed. The original signal-test assessment above
retains its historical revision and evidence.

The supervisor ownership work in `4a37e4ea9fe39becefb30e855a41b49cd654316f` adds these direct-child cases:

- `crates/farhelm-supervisor/src/managed_supervisor.rs:357–394,487–514` holds a plain `Child` across fallible identity,
  readiness and assertion work. A reaper-thread handoff supplies no unconditional kill-and-reap owner for failure.
- `crates/farhelm/src/uninstall.rs:1254–1276` starts a stand-in before fallible identity, readiness and durable-record
  work. Its cleanup owner exists only after successful construction; `Drop` at `1297–1306` cannot cover a constructor
  failure. The stand-in can retain the supervisor lock as well as its process.
- `managed_supervisor.rs:556–566` at `05ce9d59f38bfcaf5cdc40162c26695c37d7e44b` enables the replacement test on macOS
  with a plain child across copy, rename and observation. The Linux ordering predates that platform change.

The terminal-host fixtures introduced in `d5a7f8de5ea949628ce146dcd7944852baa40db1` need distinct ownership remedies:

- `crates/terminal-host/src/host.rs:2326` evaluates `idle_child()` before `stepped_on` constructs `Stepped`.
  `idle_child` at `2167–2175` starts `/bin/sleep 1000` with null stdio; `stepped_on` at `1862–1879` can panic on
  listener setup before its destructor is armed. Closing this fixture's regular-file master does not hang up the
  sleeper. Arm a direct-child kill-and-reap owner at acquisition and transfer it after setup.
- `crates/terminal-host/src/spawn.rs:978–1039` starts a HUP-resistant shell and background job before fallible readiness
  observations and pipe setup. This case needs process-group cleanup and direct-child reaping, with ownership
  transferred or disarmed when `report_ready` calls `Host::abandon`. An always-active raw-PID guard after abandonment
  could act on a reused identity.
- `crates/terminal-host/tests/host_lifecycle.rs:700–709` starts a HUP-resistant child, then performs fallible protocol
  observations before creating `KillOnDrop`. The common fixture owner at `tests/common/mod.rs:217–252` ends the host,
  not this deliberately surviving child. The child belongs to the terminal host; the test cannot reap it as its own
  `std::process::Child`. Acquire effective cleanup ownership before observations without changing the required
  production behavior that normal `Shutdown` leaves this child alive.

These locations extend the existing failure-before-cleanup finding rather than establish acceptance of it. Searches of
the specifications, Planned work, unlanded plans, known bugs, queue, ledger and filters found no exact coverage. The
same-run PID-reuse exception concerns a different mechanism. Pending browser/indirect-child publication discards do not
explicitly cover these directly acquired fixture resources.

Provenance: trailing-review run `4b58b499-b330-4a1c-841a-38eda725531a`; `g020/f2`, `g021/f1`, `g032/f1`, `g035/f1`,
`g038/f2`, with independent corroboration of the lifecycle instance by `g036/f2`. All used Codex gpt-6.1-sol, high
reasoning. Their original event histories and the different cleanup ownership boundaries remain in private evidence.
