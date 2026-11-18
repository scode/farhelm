# A header question can look like it belongs to the next session

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

A Restart or similar question can stay under the next session's button even though answering it still acts on the
original session.

## Details

Bucket: **high**. GUI migration final whole-tree review, R1 Opus 4.

Source references: `crates/gui/farhelm-gui/src/session_pane/header.rs:630–640, 901–924`,
`crates/gui/farhelm-gui/src/session_pane.rs:392–415`.

Header action anchors are keyed by verb across the window. The caret anchors include the session key so the layer
disappears when its session is no longer drawn. A lifecycle question freezes its original target, but a shared anchor
can let it stay visible underneath the new session's action. A Restart question need not quote the session title.

Opus rated this possible and noncritical. The shared anchor is visible in source. The reviewer could not establish a
shipped no-press selection change that leaves the old question answerable. High records the potentially misleading
confirmation placement. An ordinary route that produces a wrong-session destructive answer would move this to highest.

Key lifecycle question anchors by session, following the caret anchors. Verify with a pending question through a
selection change while its original session remains eligible to answer. SPEC_impl.md's “Questions: the prompt” governs
the frozen answer. The existing pre-admission question and stale guard findings have different triggers.
