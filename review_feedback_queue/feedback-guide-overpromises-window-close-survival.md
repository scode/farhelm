# Feedback guide overpromises window-close survival

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The guide says feedback sending survives closing the window, although closing the last desktop window ends its embedded
helm.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The inspected desktop Dioxus lifecycle contradicts the unconditional guide promise. Closing an ordinary browser page
while an independently owned helm continues is a different path. The exact Quit exception does not make the window-close
wording correct. No feedback was sent or window opened during review.

Source locations: `website/src/content/docs/docs/using/send-feedback.md:15-16`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Later synthesis pass; original f1 retained without a new ID. Reviewed full tip d6645f771bee8ed03a66486ef8755834937d759d.
Assigned commit 67ebc0755bb39921136d7d314dc6af26ea6f37e2 only changes Send casing; the sentence is present at exclusive
base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c. TLDR: closing the desktop window while feedback is pending can terminate
the send despite the guide's unconditional promise. Confirmed source route: UI api.rs:5145-5182 submits one ordinary
request with no queue/retry; helm feedback.rs:152-174 uses run_owned; helm lib.rs:2482-2500 implements that as
tokio::spawn, which outlives a disconnected caller only while its runtime/process lives. The helm is a desktop thread
(desktop.rs:573-602); production configuration retains Config::new's close policy (desktop.rs:222-266;
desktop/menu.rs:81-94). Cargo.lock pins Dioxus Desktop 0.7.10, whose version-matched cached config.rs:124-125 and
app.rs:195-217 establish exit on last-window close. Normal event-loop exit terminates the process (desktop.rs:769-772).
Synthesis adds explicit contract support: SPEC_impl.md:49-53 says accepted-action ownership does not require work to
outlive its process, and SPEC.md:3529-3537 requires prompt Quit without waiting for requests. These narrow the
correction to documentation; no code change to delay Quit is justified. Browser-page disconnect, dialog Cancel/Escape
and native window close have different owners. e2e/tests/feedback.spec.ts:595-674 exercises only dialog close paths with
an intercepted response, not the native process lifetime. Suggested bucket other; lost form text is not highest
user-owned-work loss. Confidence source-confirmed, no native or network experiment. proposed_drop null. possible_cover:
process-exit specification accepts code behavior, not the guide's contrary promise; TRIAGE_OUTCOMES.md:6909-6925 covers
only dialog Cancel/Escape. Initial coverage searches found no matching Planned item, BUGS entry, unlanded plan, queue
item or ledger outcome. Parent has audited mandatory delivery COMPLETE and explicitly retained this as OTHER. No new
verification uncertainty arose in synthesis.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g083/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
