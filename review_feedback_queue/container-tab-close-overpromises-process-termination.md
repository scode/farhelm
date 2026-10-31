# Container tab Close overpromises process termination

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The tab Close warning and internal contract say all started processes end, although detached container work can survive.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The UI warning/tooltip and shared CloseTab documentation omit the explicit container detached-work exception. The
retained defect is inaccurate explanation, not a demand to kill deliberately detached work or a finding that the
exception itself is wrong. The original-agent/client-detach consequence is a separate highest item.

Source locations:
`crates/farhelm-ui/src/tabs.rs:233-234,299,477-491; crates/farhelm-ui/src/session_view.rs:2710,2717; e2e/tests/terminal-tabs.spec.ts:2200-2222`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Later synthesis complete at d6645f771bee8ed03a66486ef8755834937d759d. Retain original f1 and parent-reported shared root
with a002/f13; do not create a duplicate ID. Independent source checks confirm the internal protocol overpromise at
farhelm-proto/src/lib.rs:3439-3446, the GUI confirmation at tabs.rs:234 -> session_view.rs:2710, both tooltip consumers
tabs.rs:299 and session_view.rs:2717, and exact wording tests tabs.rs:477-491 and terminal-tabs.spec.ts:2200-2222.
Browser fixture premise checked: openSessionWithTabs at terminal-tabs.spec.ts:534-555 calls createTabSession;
helpers/terminal-suite.ts:415-431 creates an ordinary command launch with no container selection. Thus that passing test
would establish ordinary wording delivery only, not correctness for the later container path; no test execution is
claimed here. SPEC.md:1568-1573 explicitly and intentionally allows detached container tab processes to survive Close
until Stop/Delete. Source and prior-pass cleanup checks agree. Align prose and its tests with that distinction; no
cleanup behavior change is needed. Suggested bucket other; confirmed interface/spec mismatch, no established
security/user-work-loss route. proposed_drop null. possible_cover remains the explicit survival acceptance, which does
not explicitly authorize contradictory GUI/protocol text. Historical quoted approvals/runtime results were not
authenticated or needed. No additional verified or uncertain finding ID arose during synthesis.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g065/f1`, `a002/f13`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
