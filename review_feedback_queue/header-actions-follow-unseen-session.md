# Header actions can hit a session the window has yet to show

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

A Stop, Restart, Replace or Delete aimed at one session can act on a different session when the selection just changed.
Skipping confirmations can make the wrong action take effect immediately.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R1 Sol 1 + Opus 2.

Source references: `crates/gui/farhelm-gui/src/session_pane/header.rs:734, 741, 748, 1095–1098, 1177`,
`crates/gui/farhelm-gui/src/session_pane.rs:392–415, 510–515`,
`crates/gui/farhelm-gui/src/session_list/menu.rs:401–407`, `vendor/gpui-pre/src/window.rs:6403`.

The header button captures its verb and pane. At delivery, the handler gets the currently mounted session key. A
selection change (including automatic selection after another client deletes the open session) updates that key and
invalidates the window before GPUI replaces the mouse listeners in the rendered frame. A click on the old header can
therefore take the new session's action and guards. An enabled confirmation can also open for the new session. The
separate environment control uses the same live-target lookup, so a stale Resume can reach the next environment.

Sol called the source route confirmed and critical. Opus called it possible and noncritical because the click has about
one frame to land. Both described the same target-selection gap. The established skipped Stop/Delete route can end the
wrong session's processes, which puts it in the highest bucket.

Capture the drawn key in each header control, as the row menu already does. Refuse a stale press after the selection
changes. Exercise input dispatched to A's frame after B mounts. SPEC.md's “One GUI at a time” accepts some actions
delivered before redraw, and keeps explicit destructive guards in force (lines 5135–5138). The skipped confirmation rule
at lines 2063–2065 binds the action to what was rendered.
