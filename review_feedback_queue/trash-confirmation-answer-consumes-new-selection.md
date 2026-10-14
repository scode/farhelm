# Potential stale confirmation can consume a different pending selection

Independent `gpt-6-astra` high exclusion audit rejected the proposed drop. The missing identity fence is established;
actual browser/desktop delivery of an old answer after selection replacement remains unverified. Retain this conditional
wrong-target deletion candidate for human triage; do not present it as a reproduced deletion.

Reviewed commit: 0640cfee761414b1fd92fba3f70bd5cc6f7238a4

## TLDR

If an old confirmation event survives a change of pending selection, it could delete the newly selected host’s archives.

## Details

Suggested bucket: `highest`. Confidence: likely only if the unverified queued-event sequence is reachable; not
established as a product bug. Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop: Propose dropping
as insufficiently demonstrated. Independent drop review is required because the hypothetical consequence is deletion of
the wrong selection; no independent reviewer was spawned under the task restriction..

ConfirmBand passes no confirmation identity to onsubmit (crates/farhelm-ui/src/list/trash.rs:172–208). The shared submit
handler reads whichever confirmation is current at :328–334, while host and global triggers can replace that value at
:521–534,557–562. Thus an old affirmative callback is not bound to its displayed host/IDs. The source establishes the
missing identity check, but this review did not demonstrate a shipped browser/desktop event sequence that delivers the
old answer after another trigger changes the selection. A keyed confirmation would reject that sequence.

Possible cover: No exact coverage. ops.rs:291–316 describes identity-bound confirmation handling.
TRIAGE_OUTCOMES.md:6825–6854 establishes related queued-answer cases, but does not establish this different event
sequence. FILTER.md excludes destructive actions applied to the wrong target.

Status at frozen endpoint: The same structural concern remains at pinned main: trash.rs:478–482 reads the current
confirmation, and :618–631 can replace it. Reachability remains unverified.

Review coverage: full commit diff read: true. No reviewed-commit diff content remains unread. TRIAGE_OUTCOMES.md was
searched throughout, but unrelated sections were not read in full. Unchanged surrounding modules and backend tests were
not exhaustively reviewed. Later-added trash tests beyond the inspected current-main comparison were not read. One
combined output was truncated; required diff and Planned content were reopened separately. No Sol reports or new
working-copy queue additions were read.

Runtime evidence: None, as requested by default. Source-only review; no reproduction claims. Applied the supplied
test-authoring checklist to changed tests and fixtures. No files were created, modified or deleted; no fetch, rebase or
helper agents.
