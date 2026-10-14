# Right-click can overwrite newer clipboard contents

Independent drop decision: a fresh `gpt-6.1-sol` high reviewer rejected the pre-existing-only exclusion. The tracked
plain-click decision does not cover right-click with reporting disabled. Ordinary context-menu mouseup delivery and
clipboard replacement remain unverified; retain that premise for triage.

Reviewed commit: 57b5c0da0b72b0efb2f5c4c7a0693dd89fc40a45

## TLDR

A right-click in a terminal without mouse reporting can replace newer clipboard contents with an old terminal selection.

## Details

Suggested bucket: `highest`. Confidence: likely; ordinary context-menu mouseup delivery and the resulting clipboard
write were not reproduced interactively. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: Outside
this commit's regression scope: the first parent already performs the same write for this trigger. Retained here for
independent drop review; no independent verdict was obtained..

At crates/farhelm-ui/assets/terminal.js:5217–5232, every mouse button starts a copy gesture; 5279–5292 passes no button
restriction to the copy predicate. assets/copy-on-select.js:77–80 accepts retained text whenever trackingAtPress is
false. The vendored xterm.js:1 SelectionService.handleMouseDown only processes primary-button selection, and its non-Mac
default rightClickSelectsWord is false. Trigger: select terminal text, copy different content elsewhere, then
right-click that terminal without mouse reporting. The retained selection can overwrite the newer clipboard. This
behavior already exists in the first parent: terminal.js:5213–5228,5275–5283 and copy-on-select.js:76–77.

Possible cover: The removed plain-clicks-overwrite-clipboard-old-selection.md describes an unforced press under legacy
mouse reporting, whereas this trigger has reporting disabled. No exact queue, ledger, Planned or BUGS coverage found.

Status at frozen endpoint: Persists by inspection at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703:
terminal.js:5309–5324,5372–5385 retains the unrestricted gesture and predicate; copy-on-select.js and vendored xterm.js
are unchanged.

Review coverage: full commit diff read: true. No commit-diff hunks remain unread. Unchanged portions outside the named
ranges were not exhaustively reviewed; the full specifications, 13,089-line triage ledger and minified vendor bundle
were not read end to end. Initially truncated combined outputs were followed by separate reads of the affected diff
content. No new actionable regression was established. Highest proposed drops remain pending independent review.

Runtime evidence: None run, as requested. Review was read-only; no checkout or scratch files were created, modified or
deleted. Changed-test authoring checklist applied by inspection. Browser focus ordering, real context-menu mouseup
delivery and clipboard writes remain untested.
