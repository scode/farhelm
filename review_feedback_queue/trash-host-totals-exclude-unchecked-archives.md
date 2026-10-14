# Host totals present unreadable archives as a complete zero

Reviewed commit: 0640cfee761414b1fd92fba3f70bd5cc6f7238a4

## TLDR

A host with unreadable archived work can display “0 checkouts · 0 B” instead of an unknown or partial total.

## Details

Suggested bucket: `other`. Confidence: confirmed by source inspection. Source: dedicated `gpt-6-astra` high adversarial
review. Proposed drop: none.

crates/farhelm-ui/src/list/trash.rs:510–511 derives the host count, bytes and partial flag exclusively from
listing.checkouts. Archives that fail ownership/path inspection are returned separately in listing.issues
(crates/farhelm-supervisor/src/checkout_trash.rs:293–322). An issue-only listing therefore reports an exact zero; a
mixed listing understates its total without marking it partial. The diagnostics remain visible, and the global total
separately handles unchecked entries, so this is misleading host-level accounting rather than lost data. Include
diagnostic entries in the uncertainty calculation and qualify the known count.

Possible cover: None found. SPEC.md:739 requires partial totals for unknown sizes; SPEC_impl.md:1692 says unknown host
counts stay unknown.

Status at frozen endpoint: Persists at pinned main in crates/farhelm-ui/src/list/trash.rs:605–606. No fixing commit
found.

Review coverage: full commit diff read: true. No reviewed-commit diff content remains unread. TRIAGE_OUTCOMES.md was
searched throughout, but unrelated sections were not read in full. Unchanged surrounding modules and backend tests were
not exhaustively reviewed. Later-added trash tests beyond the inspected current-main comparison were not read. One
combined output was truncated; required diff and Planned content were reopened separately. No Sol reports or new
working-copy queue additions were read.

Runtime evidence: None, as requested by default. Source-only review; no reproduction claims. Applied the supplied
test-authoring checklist to changed tests and fixtures. No files were created, modified or deleted; no fetch, rebase or
helper agents.
