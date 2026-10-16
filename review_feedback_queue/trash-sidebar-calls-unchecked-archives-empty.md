# Trash advertises an empty collection while unchecked archives remain

Reviewed commit: 4f200ea902679492f69eadd13cb963db52a77077

## TLDR

The trash button can say the trash is empty even though opening it reveals archived work Farhelm could not inspect.

## Details

Suggested bucket: `other`. Confidence: confirmed statically; no browser reproduction. Source: separate adversarial
trailing reviewer. This adjacent presentation gap predates the host-summary correction. Proposed drop: none.

`crates/farhelm-ui/src/list/trash.rs:382–394` counts only `listing.checkouts`. For an issue-only successful listing,
`all_ready` remains true (`402–411`), so `536–557` emits “trash: empty” and the accessible name “checkout trash, empty”;
`595–600` draws the empty icon. The host summary now correctly reports unchecked entries, contradicting the standing
sidebar presentation. This persists through the review tip.

Verify with a successful issue-only listing and inspect both visible and accessible button text. Preserve uncertainty
about unchecked archives instead of equating zero checked rows with an empty collection.

Possible cover: the completed `trash-host-totals-exclude-unchecked-archives.md` ledger outcome expressly scopes its fix
to the host summary. It does not accept the sidebar claim. No exact Planned item, BUGS entry, queue item or filter was
found.
