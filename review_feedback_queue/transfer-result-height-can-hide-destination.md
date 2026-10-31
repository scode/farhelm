# Transfer result height can hide its destination

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A long transfer result may hide the saved destination or refusal without an expansion or scroll control.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The result area is capped at 4.5em with hidden overflow. Source/layout checks establish the cap and absent local
recovery control; a supported concrete rendered geometry and full reachability consequence remain unmeasured. Existing
reachable-scrolling acceptance does not automatically cover the hidden result.

Source locations: `crates/farhelm-ui/assets/app.css:1688-1711; crates/farhelm-ui/assets/terminal-files.js:620-650`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full tip d6645f771bee8ed03a66486ef8755834937d759d. Assigned casing commit
bd97d312165a02c351903f6bdf2dffcbe9ef8f33 preserves this pre-existing presentation contract issue; it did not introduce
the cap (exclusive base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c already has app.css:1514-1538). TLDR: a sufficiently
long file-transfer result can hide the actual saved filename or the reason a modifier-click did not open it, with no
ordinary pointer/keyboard control to reveal the cut-off text. Source evidence: current app.css:1662 preserves
wrapping/spacing and :1688-1711 sets max-height:4.5em, overflow:hidden and pointer-events:none.
terminal-files.js:569-593 creates a nonfocusable role=status text node with no title, copy or expand control; a success
retires its accessibility announcement after five seconds. :620-628 places the full local destination before the opening
outcome in that same node. Helm downloads.rs:204-235 returns the full published path and open_error. :269-285 preserves
the original basename, including long names, and :390-409 publishes using that basename. This means a long admitted file
name can use up the bounded notice before the unique suffix/extension or opening reason. SPEC.md:2270-2277 says the
desktop shows where the file went and explains an opening failure. Actual checks: inspected the complete message
construction and CSS rules, the native response/name producer, existing browser result checks
(terminal-files.spec.ts:330-395,425-485), and the mock notice fixture from the initial pass. Tests assert
textContent/position/opacity but do not establish full text geometry; the JS fixture models no layout. There is no
alternate full-result control in the checked producer. Accessibility caveat: role=status initially carries the complete
text, so this is a visual/pointer-access issue, not a claim that assistive technology never receives it. Missing
premise: a measured supported native/browser window/font and an admitted long-result fixture demonstrating that the
saved filename or reason lies below the cap. Source proves what happens when content overflows, but cannot measure
actual line wrapping; the source-only review restriction prohibits the engine/native check. Resolving evidence:
capture/bounds for a real long-basename native save (or native opener refusal), with scrollHeight versus clientHeight
and the missing final text, or proof of another ordinary full-result presentation. Coverage searches across SPEC.md,
SPEC_impl.md, TODO.md, BUGS.md, unlanded plans/queue, queue INDEX and TRIAGE_OUTCOMES.md found no exact coverage.
Suggested bucket other; confidence likely with the geometric premise above; proposed_drop false. possible_cover:
FILTER.md Rare, self-correcting glitches and imprecise diagnostics if this is judged wholly a rare diagnostic-only
trigger; rarity and the successful-save-destination scope were not established, so no exclusion made. No security,
wrong-target action, false save failure or loss of user work is alleged. Parent-retained approval-container-policy
omission is unrelated and not refuted.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g064/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
