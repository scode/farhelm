# Shallow-pause test observes recovery beyond its declared boundary

Reviewed commit: 9d2ff7da94f0f16c533b44533c10d3fed7cbd94a

## TLDR

Unverified after investigation: a legitimate later recovery may fail the shallow-pause regression because its observer
continues beyond the declared output boundary. The required tmux and runtime scheduling has not been demonstrated.

## Details

Suggested bucket: **other**. Recorder confidence: **unverified after investigation**. The original reviewer's
confidence, qualifications and severity history are retained below; source-confirmed conditional behavior is not a
runtime observation.

The earlier decision removing a Deflake entry and the queued stale exclusion concern sweep selection, not this observer
boundary. This conditional test-discrimination defect has no established production failure. A complete filter match is
unclear while the actual event schedule and reliability consequence remain unresolved.

## Recorded source assessment

Reviewed 9d2ff7da94f0f16c533b44533c10d3fed7cbd94a. TLDR: an unrelated later recovery can potentially fail the
shallow-pause regression despite the test's declared 1,000-record observation limit. Unverified after investigation for
the actual timing-dependent failure. Source-confirmed mismatch: :1766-1797 ends byte collection at continuation but
never freezes the observer or caps its events by the producer boundary. connection.rs:1675-1697 queues bytes and pump
:1762-1778 can then process Paused; tmux/stream.rs:1315-1368 emits Bytes and Paused as successive independent decoded
events, and :1049-1072 documents legitimate timing-dependent pause after lag. client.rs:4100-4103 supplies a 256-event
BytesOnly queue; :1012-1065 consumes buffered bytes independently. A permitted ordering is enqueue the continuation
bytes, process a later buffered Paused notification and record recovery, then let the waiter consume the earlier
continuation. The code contains no acknowledgement barrier preventing that ordering. Countervailing source:
testtrace-macros :249-259 defaults to current-thread and no await follows wait_for_bytes before the assertion; the
proposed failure must occur before the waiter consumes continuation, not concurrently during the assertion. Missing
premise: actual tmux production/read-ahead and runtime scheduling that supplies a legitimate post-boundary Paused early
enough, without overflow/detach or an earlier reset already failing the byte checks. This is not demonstrated by
repository source, and native execution is forbidden by the brief; no frequency, reproduction or scheduler starvation is
asserted. Available checks completed: forwarder send/pump/recovery, parser dispatch and documented pause meanings,
client queue size and receive ordering, macro runtime defaults and test's await positions. Settling evidence: a focused
recorded instrumentation run tracking recovery's producer byte/record position, or a controlled fixture holding
consumption after continuation is queued and supplying a later legitimate tmux pause, showing that this assertion
rejects that later event. Suggested other (test reliability only), highest not possible. proposed_drop none;
possible_cover none. Converted from lead to explicit unverified finding after available source investigation; all prior
l2 records and l1 withdrawal remain intact.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g014/l2`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
