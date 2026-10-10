# macOS environment test accepts failed observations as success

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The macOS environment test treats failed observation as successful withholding.

## Details

F181 — **definite** — `crates/farhelm-supervisor/src/procs.rs:2060` — macOS environment test accepts failed observations
as success

Every environment read can fail, or the fixture can exit, while the negative assertion still passes. The test cannot
distinguish successful argument-only observations from having observed nothing, so it can certify an operating-system
premise with a broken observation path. Establish the live sleep image and readable arguments, require successful
environment observations, and verify liveness throughout the window before cleanup.

## Evidence and triage context

- crates/farhelm-supervisor/src/procs.rs:2027–2040 describes the intended evidence as a successful fetch containing argv
  but withholding the environment.
- crates/farhelm-supervisor/src/procs.rs:2044–2051 spawns /bin/sleep with the marker, but lines 2057–2076 neither
  establish subsequent liveness nor require any successful observation. If every read returns None, marker_seen remains
  false and the assertion passes.
- crates/farhelm-supervisor/src/procs.rs:1515–1542 maps syscall failure, including an exited process, to None. Lines
  685–720 separately parse successful replies and can return Some with an empty environment.
- crates/farhelm-supervisor/src/procs.rs:208–219 explicitly distinguishes the expected successful argv-only answer, Some
  with no environment entries, from None.
- crates/farhelm-supervisor/src/service/sweep.rs:323–325 consumes read_environ; lines 434–439 skip processes whose
  environment cannot be observed. This explains the production relevance without establishing a current cleanup defect.
- crates/farhelm-supervisor/src/procs.rs:2013–2023 positively tests a different, locally built child. That can catch a
  universally broken reader but does not establish successful observation of the Apple-binary fixture.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:2181–2186 accepts macOS withholding Apple platform binaries' environments and the resulting
  detached-process cleanup gap. It does not cover the test passing without a successful observation.
- TRIAGE_OUTCOMES.md:4055–4077, heading delete-roots-only-agent-pane.md, addresses process-walk roots for tab cleanup.
  It does not address this macOS observation oracle.
- TRIAGE_OUTCOMES.md:5891–5908, heading process-snapshot-requires-supervisor-witness.md, addresses an empty
  process-table snapshot, not failed environment observations in this test.

Caveats:

- The defect is in a macOS-only test.
- No current production cleanup failure or live macOS reproduction was established.
- A stronger assertion should distinguish observation failure from valid empty environment data rather than assume they
  are interchangeable.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_runtime_sec_01:p1:F2`.

- `gap_supervisor_runtime_sec_01:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
