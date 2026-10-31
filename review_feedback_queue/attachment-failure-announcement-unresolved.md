# Attachment failure announcement remains unresolved

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Attachment errors have no explicit live announcement, so screen-reader users may miss a failed paste without navigating
to its message.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The failure is inserted into ordinary non-live DOM and the inspected bridge adds no announcement mechanism. The text
remains manually accessible. Actual screen-reader speech/events and the applicable announcement requirement were not
established after source checks; no assistive-technology run occurred.

Source locations:
`crates/farhelm-ui/src/session_view.rs:2856-2860,2905-2909; crates/farhelm-ui/assets/terminal.js:1878-1901`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full tip d6645f771bee8ed03a66486ef8755834937d759d. Assigned wording commit
bd97d312165a02c351903f6bdf2dffcbe9ef8f33 preserves this pre-existing markup; exclusive base
da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c already has the unannotated status containers at
session_view.rs:2816-2819,2865-2868. TLDR: a screen-reader user may miss an attachment failure or a
published-but-not-inserted path unless they manually inspect the page after pasting/dropping. Source basis: the
agent/tab attach-status nodes have no role=status/alert or aria-live; enclosing terminal-panes and terminal-pane markup
has no live-region role (session_view.rs:2778-2860). terminal.js:1878-1901 clears/rebuilds plain
attach-error/attach-busy divs with textContent. The upload catch/disconnected-success paths report only into these
notices (2250-2300), not into terminal output. The separate page-wide disconnected-drop path likewise inserts a plain
error div (2615-2643). The neighboring drag-copy notice explicitly uses a pre-existing role=status region and documents
reliable speech (session_view.rs:2862-2875). Actual checks: read the enclosing markup, status painting,
disconnected-drop handling and related attachment tests; searched UI source/nonvendored assets for live regions,
announcement helpers and screenReaderMode; searched the attachment tests for announcement/ARIA assertions (none found).
The xterm AccessibilityManager source excerpts subscribe to terminal onA11yChar/onLineFeed/onA11yTab and insert their
live region inside the terminal element, whereas attachment notices are siblings, so those excerpts do not supply an
announcement bridge for the checked notices. No other attachment-specific bridge was found in scoped source searches.
The first broad search accidentally included one-line minified vendor output and was clipped; no claim relies on its
unprinted tail. It was replaced by a complete bounded nonvendor search and two short exact vendor excerpts. Missing
premise: actual speech/accessibility-event behavior for a supported screen reader and browser/native client while focus
stays in the terminal during a real attachment failure. No such tool execution is authorized, and source alone cannot
establish platform-specific announcements or supported screen-reader usability. Resolving evidence: an
accessibility/speech recording of a failed attachment or detached completed upload, compared with the neighboring
working drag-copy status notice; an existing generic announcement bridge would also refute the missing path if
demonstrated. SPEC.md:2284-2320 promises visible failures and no silent disappearance, but does not explicitly require a
live-region implementation, so this is an accessibility concern rather than a claim of violating an explicit ARIA
clause. Coverage searches across SPEC.md, SPEC_impl.md, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue and queue
INDEX found no exact matching acceptance, planned repair or known finding. Suggested bucket other; confidence likely
with the assistive-technology premise above; proposed_drop false; possible_cover none identified. No user-work loss,
wrong-target action, or security consequence is alleged. f1 transfer-result clipping and parent-retained approval-policy
concerns are distinct.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g064/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
