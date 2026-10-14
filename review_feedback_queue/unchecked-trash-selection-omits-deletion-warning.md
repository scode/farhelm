# Unchecked-only bulk cleanup omits the permanent-loss warning

Reviewed commit: 0640cfee761414b1fd92fba3f70bd5cc6f7238a4

## TLDR

Cleanup can permanently delete archived work after showing a confirmation that only describes unchecked entries and
possible forgetting.

## Details

Suggested bucket: `highest`. Confidence: likely: the selection, omitted warning and deletion path are confirmed by
inspection; a transient filesystem failure recovering between listing and deletion was not reproduced.. Source:
dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none.

crates/farhelm-ui/src/list/trash.rs:81–103 includes diagnostic IDs in destructive selections but excludes them from the
checkout count. At 204–205, the permanent-loss warning appears only when that count exceeds zero. Trigger: all selected
archives produce diagnostics during listing—for example, their root temporarily cannot be opened—then become accessible
before submission. The frozen confirmation remains unchecked-only; submission at 337–369 checks connection identity, not
whether its warning covers the now-deletable folders. The existing supervisor lists opening failures as issues
(checkout_trash.rs:293–322), then independently reopens selected archives and recursively deletes verified folders
(385–399). Thus an unchecked ID can authorize actual deletion without the warning SPEC.md:739–741 requires for bulk
cleanup. Always disclose possible permanent deletion for unchecked selections, or distinguish forget-only authority from
deletion authority.

Possible cover: SPEC_impl.md:1695–1696 explicitly accepts selecting diagnostic IDs for forgetting, but does not accept
omitting the loss warning when those IDs subsequently authorize recursive deletion. Current SPEC.md:844–845 accepts
warning-free single-entry deletion, not bulk cleanup. No matching queue item, Planned item, BUGS entry or ledger
decision found.

Status at frozen endpoint: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: list/trash.rs:91–113 and
251–252 retain this selection and warning distinction; dispatch at 378–423 still sends only IDs.
checkout_trash.rs:385–399 retains the deletion path. No fixing commit identified.

Review coverage: full commit diff read: true. No commit-diff content remains unread. Initially truncated trash output
was replaced by complete numbered reads. The large triage ledger was searched and relevant sections read, not read end
to end. Unchanged surrounding modules and backend tests were inspected only in the listed ranges; no exhaustive backend
audit or visual browser inspection.

Runtime evidence: None. Static historical/current-source review only; no checkout files or reports created, modified or
deleted. Changed tests and fixtures were assessed against the supplied authoring checklist.
