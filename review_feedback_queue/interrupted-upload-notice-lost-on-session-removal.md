# An interrupted upload can vanish when its session goes away

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

When you switch away during an upload and the old session is replaced or deleted, Farhelm can cancel the upload without
ever showing you that it stopped.

## Details

Bucket: **high**. GUI migration final whole-tree review, R2 Opus 2.

Source references: `crates/gui/farhelm-gui/src/session_pane.rs:405–415`,
`crates/gui/farhelm-gui/src/terminal/attachments.rs:369–393, 472, 663–705`.

Changing the open session drops its terminal set. The upload handoff cancels the transfer and stores the interruption
line under the old session key. It is painted only when that terminal returns. Replace can select a new session
immediately, and pruning after Replace or Delete can discard the record without ever drawing it. The local source file
stays intact.

Opus rated this source-confirmed and noncritical. SPEC.md “Attachments” at line 3248 requires upload failures to be
visible. The existing attachment-failure-announcement-unresolved.md concerns screen-reader announcement of an error that
remains visible in the old app, a separate route.

Publish interruption to a view that survives the terminal (for example a notice naming the old session and file). Test
leaving during transfer, then replacing or deleting the original session without reopening it.
