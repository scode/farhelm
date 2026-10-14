# Screenshot implies template deletion waits for Save

Reviewed commit: ce66df247ff4ab5a5c2c0f7a75e67bfeab30977c

## TLDR

A reader could delete a saved template and close the dialog believing the deletion was never committed.

## Details

Suggested bucket: `highest`. Confidence: likely; misleading wording and immediate deletion are confirmed, but the
premise that a reader interprets the Save statement as covering deletion is unverified. Source: dedicated `gpt-6.1-sol`
high adversarial review. Proposed drop: none.

e2e/docs-shots/release-notes.spec.ts:79–84 annotates Duplicate/Delete with “Nothing is saved until you press save.” The
actual published PNG contains that wording. Deletion immediately calls the templates DELETE API and retains only a
temporary undo copy (crates/farhelm-ui/src/list/templates.rs:959–1019). SPEC.md:555–558 explicitly says deletion takes
effect immediately and closing the dialog ends undo. Trigger: interpret the annotation as covering deletion, delete a
template, then close without saving or undoing. Consequence: the saved template is lost. Scope: the v0.25.0 Templates
screenshot, attached by website/src/release-notes/v0.25.0.json:13–16. Qualify the Save statement as applying to edits
and duplicates, explicitly describe immediate deletion, and deliberately correct the published image.

Possible cover: None found. SPEC.md:555–558 accepts immediate deletion, not documentation implying deferred deletion. No
matching Planned item, BUGS entry, queue item, ledger decision, or filter.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703:
e2e/docs-shots/release-notes.spec.ts:88–93 retains the wording; the PNG pinned by main also visibly contains it.
Deletion remains immediate at crates/farhelm-ui/src/list/templates.rs:1097–1129.

Review coverage: full commit diff read: true. No commit diff content remains unread. Root SPEC.md, SPEC_impl.md and the
large triage ledger were searched and read selectively, not exhaustively. An oversized CHANGELOG.md read was truncated;
relevant content was reopened in smaller reads, but the entire historical changelog was not subsequently read. Other
published PNGs, browser rendering, live preview behavior and broader unchanged capture infrastructure were not fully
inspected.

Runtime evidence: No runtime tests, builds or reproductions run. Read-only Git/source checks and downloads of the two
published Templates images; scratch files only in the authorized task directory. No checkout files created, modified or
deleted.
