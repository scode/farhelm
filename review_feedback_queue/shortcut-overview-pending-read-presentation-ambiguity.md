# Shortcut overview pending-read presentation is ambiguous

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The shortcut overview can show cached bindings while its fresh opening read is pending; the required first-visible
policy is unresolved.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The cached presentation path is source-confirmed. Existing switcher decisions address a different surface and do not
explicitly settle the overview's pending interval; once-per-opening and failed-read rules do not resolve it either. No
unseen launch or harmful input route was established.

Source locations: `crates/farhelm-ui/src/keyboard_shortcuts.rs:150-171`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis revision of g155/f1; original event and provenance remain intact. Reviewed commit
d6645f771bee8ed03a66486ef8755834937d759d; assigned introducing commit d702a9039b55b4612a8b056740d7d0c606187135. TLDR:
Keyboard shortcuts can show previously cached template names while its opening preference read remains pending.
Conditional source mechanism is confirmed: rows use cached shared preferences at 88-107; only the independent platform
resource gates sections at 168-171; the binding fetch at 150-160 updates that cache when it completes. Successful
completion corrects the display. Actual checks in this synthesis: reread the complete rows/component at
keyboard_shortcuts.rs:65-213, SPEC.md:2390-2410, SPEC_impl.md:1380-1409, plan acceptance criteria 42-67, the full
analogous switcher feedback file, its ledger entry at TRIAGE_OUTCOMES.md:20036-20046, and overview browser test 262-330.
Results: once-per-opening read, binding-only overlay and failed-read caching are explicit; no requirement for a
pending-read loading indicator or hidden names is explicit. The test establishes changed cached/server bindings and
waits for eventual fresh names; its route does not hold the overview read, so it supplies no observation of the pending
interval. Missing premise: whether 'current' in SPEC.md:2405 requires that the first visible binding names await the
opening read, rather than permitting cached names until the specified refresh completes. Available source leaves that
presentation timing question unstated. A maintainer/spec clarification would settle normative compliance; a future
held-response browser/native check could establish real presentation timing, but could not alone settle the contract. No
runtime work was performed; no historical private runtime evidence or quoted approval was authenticated. Suggested
bucket other. Confidence: confirmed conditional display mechanism, unverified contract violation after investigation.
proposed_drop false because coverage remains ambiguous. possible_cover: SPEC.md:2405-2406; SPEC_impl.md:1405-1407;
plans/queue/shortcuts-overview.md:57-59 expressly adopts Cmd+J's refresh model;
review_feedback_queue/quick-switcher-pending-read-keeps-old-shortcut-labels.md:19-32 and TRIAGE_OUTCOMES.md:20036-20046
concern the same interval in the switcher. That recorded discard is substantial possible coverage, but does not
explicitly name the overview, so it is not automatically exact coverage. No new finding is inferred from absence of a
pending-state test, and lack of an operation from this dialog does not settle its presentation requirements. No
unauthorized action, security consequence, harmful launch or loss of user work is claimed. Failed reads and moves after
opening are excluded. Parent-reported g153/f4 concerns number-chord action descriptions independently of freshness; do
not merge it into f1 or treat correct key/modifier labels as refuting it. No new verified finding arose in synthesis.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g155/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
