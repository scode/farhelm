# An omitted session row can drop the open shell tabs

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

A list update can take away the shell tab you’re using and make the agent terminal receive your next typing. The shell
on the host can keep running.

## Details

Bucket: **high**. GUI migration final whole-tree review, R2 Opus 1 + addendum.

Source references: `crates/gui/farhelm-gui/src/terminal/set.rs:478–547` at the reviewed commit (affected main lines near
`crates/gui/farhelm-gui/src/terminal/set.rs:481–550`), `crates/gui/farhelm-gui/src/session_pane.rs:440–460`,
`crates/gui/farhelm-gui/src/session.rs:268`, `crates/gui/farhelm-gui/src/session_pane/carried_tests.rs:276–283`,
`crates/farhelm-helm/src/ui_channel.rs:610–625`.

The tab reconciler uses an empty list when the current session has no row in Available Sessions. It then drops the tab
terminal entities, their streams and any pending paste or upload, clears the chosen tab, and puts terminal home on the
agent. A filtered quick-return test shows a shipped update can omit the open row, though its terminal set was freshly
rebuilt. The damaging route needs the omitted row to belong to a session with an already-mounted live shell, for example
beyond a truncated listing.

Opus rated the source path likely and noncritical, with frequency and the damaging cut-list route unverified. High
reflects the possible loss of access to the active view and unintended focus change. Establishing ordinary delivery to a
different program with harmful input would warrant revisiting highest.

Keep the tab set when the list cannot prove the session is gone. Use the same distinction as listing_proves_gone, or
retain its independent selected-session facts. Test an omission with an unchanged live tab and a subsequent
authoritative removal. SPEC.md lines 5172–5178 accept crowding in the capped merged list. Whether that acceptance covers
loss of an already-open terminal view is unclear. SPEC_impl.md lines 397–407 describes an open selection and terminal
staying when the list leaves their row out, and provides selected_cut for that case.
