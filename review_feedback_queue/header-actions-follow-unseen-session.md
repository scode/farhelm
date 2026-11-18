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

### Interrupted-session card Replace

Also reviewed at `c9575db61abb31bebe43da0f11a76712e18725c1` in the Haiku pass (H-P7-2), checked by gpt-6.2-sol. The
interrupted-session card has its own Replace button. The listener resolves the session pane's newly mounted key when the
click arrives. With confirmation skipping enabled, it can dispatch Replace directly for that different session.

- Verdict: ALREADY QUEUED: header-actions-follow-unseen-session.md
- Route checked: `session_pane/header.rs:734–793,819–832`; `session_pane.rs:392–415,510–515`;
  `SPEC.md:2063–2065,5135–5138`.
- Reason: The native listener keeps its action and gets the target from the newly mounted pane before redraw. The queued
  highest bucket exceeds this charter's bar: the verified route is bounded by one redraw and no widening was shown. The
  target binding still conflicts with the render-based skip rule.

- Verdict: NOT CRITICAL
- Route checked: `session_pane/card.rs:457–480,558–578`; `session_list/replace.rs:225–252`;
  `SPEC.md:2063–2065,5135–5138`.
- Reason: The separate card button also reads the mounted key on delivery, so skipped Replace can send for the newly
  selected session. The header queue item covers the same design error only in header controls. The card route is still
  limited to the old listener's redraw window.

The verifier assessed the header route as below the review's immediate-fix threshold because it fits within a single
redraw. The queue's `highest` bucket still applies to the established wrong-session process-loss route. Capture the
card's drawn key too and exercise a stale card click after the pane switches sessions.
