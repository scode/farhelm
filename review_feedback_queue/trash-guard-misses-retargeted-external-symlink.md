# Trash deletion misses a session folder reached through a retargeted symlink

Reviewed commit: a1b1f6137c34f28f812855152755e99ba898e9dc

## TLDR

Emptying the trash can delete files being used by a retained session’s terminal tab.

## Details

Suggested bucket: `highest`. Confidence: confirmed mechanism by source inspection; no runtime reproduction. Whether the
explicit guard limitation accepts this destructive consequence requires triage.. Source: dedicated `gpt-6.1-sol` high
adversarial review. Proposed drop: none; retain for adjudication of the specification conflict.

At the reviewed commit, store.rs:2525–2549 collects only raw cwd and admission-time canonical_cwd.
checkout_trash.rs:389–399 compares those spellings against the archive and then removes it. Trigger: create a session
through an external symlink pointing outside the checkout root; later retarget that symlink to a recorded archive and
open a terminal tab. core.rs:12146–12172 and 12267–12273 open the tab through the current raw path without updating the
stored canonical path or taking directory admission. Neither stored spelling identifies the archive, so trash deletion
proceeds despite the retained session using it. No timing race or modification of the archive/root is required.
SPEC.md:908–912 explicitly accepts opening tabs through changed symlinks, while :736–739 requires protecting retained
session folders. Introduced by the new deletion guard; the historical commit exposes the backend API, with the app
control added subsequently.

Possible cover: SPEC_impl.md at the reviewed commit:1668–1670 explicitly describes using admission-time canonical paths
without resolving unrelated cwds. That describes the limitation, but does not explicitly accept deleting a folder
reached by the supported changed-symlink tab operation. Current SPEC.md:2760–2769 excludes manipulation of managed
paths, while preserving symlink rules; this trigger changes an external alias only.

Status at frozen endpoint: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: checkout_trash.rs deletion
logic is unchanged; store.rs:2563–2585 still collects the same spellings; core.rs:4913–4930 and 12256–12325 retain the
two admission paths. No fixing commit found.

Review coverage: full commit diff read: true. No commit diff content remains unread. Truncated relevant output was
reopened. Unrelated portions of the large triage ledger and specifications were searched but not exhaustively read;
unchanged surrounding test bodies were not all read in full. Two guessed tabs/driver module paths did not exist; tab
lifecycle was traced in core.rs instead. No checkout files or reports were created, modified or deleted. No nested
agents were used; proposed highest-priority drops remain pending independent review.

Runtime evidence: None, as requested by default. Conclusions use historical source inspection and pinned-main
comparisons; no runtime reproduction or filesystem-specific inode-reuse experiment was performed.
