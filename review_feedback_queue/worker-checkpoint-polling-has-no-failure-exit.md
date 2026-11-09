# Checkpoint polling lacks a bounded failure exit

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Worker checkpoint polling can wait indefinitely for completion; the exact service transition needed for the suspected
permanent wait remains unresolved.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The loop accepts completed-only state without a deadline or failed-state branch. Primary CLI version/parser/status
sources were checked, but they do not establish the matching service's durable failure transition for the historical
operation. No anonymous HTTP response or missing observation is treated as proof of that transition. No cloud operation
or CLI probe ran.

Source locations: `scripts/tensorlake/sandbox-tests.ts:205-208`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Required separate discovery pass, same reviewed full commit d6645f771bee8ed03a66486ef8755834937d759d and original
provenance. Further primary service-source investigation performed without cloud/CLI/runtime probes: fetched the
official tensorlakeai/tensorlake recursive tree at immutable f66fd83b32d2cbcbb9759982bc06b78b7e06e2c3, verified its
metadata reports truncated:false, searched snapshot/checkpoint/executor/upload paths, inspected the CLI snapshot/list
clients from synthesis, and inspected official Python SnapshotStatus/readiness definitions models.py:137-173. Those
define in_progress/local_ready/completed/failed/deleting/replicating/unknown but do not implement server transitions.
Enumerated anonymous public organization repositories; no snapshot service repository was identified in that returned
inventory. Anonymous GitHub repository metadata GETs for concretely relevant platform-api (named in official release
commit), indexify and compute-engine-internal (named by official VENDORED_FROM) each returned HTTP404. Inspected
official VENDORED_FROM:1-50; it explicitly describes private artifact/function-client source but is not itself proof
about the sandbox snapshot state machine, so no such inference is made. Public cloud-sdk root and sandbox module/model
paths were inspected/searched and provide client-side interfaces, not the missing producer. Web queries for the exact
local_ready/failed snapshot combination did not locate an applicable primary server implementation. Actual result:
accessible primary sources establish states and the client stopping at local_ready; no public server transition
implementation was located by these concrete checks. Exact remaining premises unchanged: historical CLI/service source
mapping, and failed/stalled durable-upload transition after successful local_ready return. This is a bounded search
result, not a universal claim that the source cannot exist or is private. Matching server transition source or
authenticated checkpoint history would settle it. Retain likely/ investigated-uncertain, other tooling consequence,
proposed_drop null, possible_cover none. No actual failed service run, cloud bill, catastrophic consequence, or
authenticated private-environment evidence claimed.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g111/f6`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.

## Trailing-review extension through 9e3ab60b

Reviewed commit: f17458c5ed6159cac9f621cc2350360d00fa396c

Unverified after investigation: Rust staging also polls a snapshot indefinitely until completed, with no deadline. The
actual service transition needed to leave that wait permanently stuck remains unestablished.

Suggested bucket: **other**. Recorder confidence: **unverified after investigation**. The original reviewer's
confidence, qualifications and severity history are retained below; source-confirmed conditional behavior is not a
runtime observation.

Extend the existing browser-runner item with the Rust path. Preserve its investigated missing producer premise: enum
definitions and product-builder failure tests do not establish a durable failure after this CLI-stage operation returns
local_ready.

## Recorded source assessment for the Rust runner

Reviewed commit f17458c5ed6159cac9f621cc2350360d00fa396c. Recording-only classification correction: retained uncertain
finding, not an open lead. Confidence/consequence revision following parent recording-time evidence; original records
preserved. Certain source fact: rust-tests.ts:73-76 polls until a matching completed snapshot row, with no deadline or
other-state exit, and :78-79 defers its finally cleanup until that wait returns/throws. Conditional consequence:
repeated successful non-completed/missing-row listings would keep the invocation polling. Actual permanent service stall
is UNVERIFIED AFTER INVESTIGATION, not a confirmed failed-checkpoint route. Checks already performed: full runner stage
and snapshotRows parser; .agents/tensorlake.md checkpoint procedure; api.rs:172-198 SnapshotStatus; build.rs:1270-1325
and build_tests.rs:1288-1325; SPEC_impl.md:6320-6344. Those product builder tests and assumed failed enum variant do NOT
show that this CLI-stage operation produces durable failure after its successful checkpoint command. Parent additionally
reports official CLI/client source and public SnapshotStatus definitions establish that checkpoint can return
local_ready before later completed durability, but targeted public-source searches found no matching server
implementation or actual permanent durable-failure-after-local_ready history. I did not independently inspect that
external source and do not claim I did. Missing premise: this operation can remain non-completed permanently after the
successful CLI checkpoint stage, with repeated successful listing responses, rather than eventually completing or
returning an error. Settling evidence: matching service implementation/contract or retained real operation history
demonstrating that transition/stall (or a guarantee ruling it out); an owned provider reproduction could establish it
but was forbidden by this source-only review. Suggested bucket other if actual stall established, no highest route.
proposed_drop false: retain as explicitly unverified after investigation for parent judgment. possible_cover none
checked. No live failure observed. This preserves the prior lowered consequence assessment; source-confirmed absence of
a deadline remains unchanged. Permitted investigation is complete. No new investigation or discovery accompanied this
recording correction; the last discovery pass remains empty.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g045/l8`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
