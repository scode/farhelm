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
