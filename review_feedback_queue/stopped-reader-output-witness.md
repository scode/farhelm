# Establish flood output during the stopped-reader interval

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The browser flood test checks retained output precisely, but a pass does not establish that the flood ran while the page
stopped consuming frames. It can therefore leave the intended slow-viewer case unproved while appearing to cover it.

## Details

Source coordinates from the original records: `e2e/tests/grid-flood.spec.ts:100-127,146-184`. References below use these
paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The claimed premise is explicit: openGateAndStopReading's comment says the busy interval 'overlaps the flood by
construction' (105-109), and the test's Spec says 'the flood runs while it reads nothing' (146-148). In code, the page
calls ws.send at 122 and immediately busy-loops until its own clock advances three seconds (123-126). That establishes
blocked JS message delivery and a submitted gate message; it does not establish when the producer receives the gate or
starts output. The first observable flood premise is checked only after the busy interval, by polling the model for any
record (175-180). A second busy interval is conditional and skipped if FLOOD-DONE has already been drawn (183-184).

Fixture investigation: crates/farhelm-fixtures/src/fake_agent.rs:1985-2003 sets raw mode, prints/flushed READY, waits in
read_exact for the gate, then calls emit_flood_records. The writer at 2345-2358 has no pacing or duration floor; it
buffers 800,000 records and emits completion. Neither fixture nor test acknowledges a producer-side start or bounds the
async gate delivery relative to the page's three-second interval. Thus a schedule where gate handling starts after the
interval and output is complete by the first post-interval observation satisfies every final-state assertion without
exercising concurrent output and a stopped JS reader. No claim is made that this schedule was observed: runtime
experiments are prohibited. The confirmed issue is the absent premise/oracle, not an observed product failure or flake.

The final-history oracle at grid-flood.spec.ts:195-213 remains useful: it demands the exact 12,000-line tail plus
screen, consecutive record numbers, correct terminal capacity and no queued history. The earlier concern about a missing
helm-pause oracle stays withdrawn: the header expressly delegates pause-mechanism checks to Rust and says this test is
judged by final state (6-8). That is different from its own declared overlap premise, which these source checks do not
establish. For coverage to claim that premise, use a controlled producer/transport gate with an observable interval of
output owed while the JS reader is withheld; assert that premise before accepting the exact final tail. Alternatively
narrow the test's claimed coverage. This follows the supplied test-authoring checklist: 'Assert the fixture premise
before the action or measurement that depends on it.'

Suggested bucket: other; maintainer test evidence/coverage, no established security, data-loss or shipped user
consequence. Confidence: confirmed missing overlap oracle from source; no runtime reproduction claimed.
proposed_drop=false. possible_cover=none; queue unread. Provenance: e16ac4b2758a41fae246c1c15d4c0a5f1872d1ee moves this
test into the main suite and adjusts imports/comments; the timing body was carried over, not newly invented there.
Commissioner owns coverage and scope disposition. No remaining investigation lead about the fixture's timing guarantees.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g020/g020-c2`. The
retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their own
revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
