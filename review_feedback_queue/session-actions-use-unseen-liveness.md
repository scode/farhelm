# Session actions take broader consent from unseen updates

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Deleting an ended session can kill an agent that restarted after the screen was drawn. Stop, Restart and Replace can
also act on more running work than the window showed.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R1 Sol 2 + Opus 1.

Source references: `crates/gui/farhelm-gui/src/session_pane/header.rs:748–751, 1095–1098`,
`crates/gui/farhelm-gui/src/session_list/menu.rs:566–577`,
`crates/gui/farhelm-gui/src/session_list/delete.rs:91, 271–276` at the reviewed commit (main
`crates/gui/farhelm-gui/src/session_list/delete.rs:91, 279–284`),
`crates/gui/farhelm-gui/src/session_list/stop.rs:198–201`, `crates/gui/farhelm-gui/src/session_list/replace.rs:246–249`,
`crates/gui/farhelm-gui/src/launcher/view.rs:1293–1297, 1306–1314`, `vendor/gpui-pre/src/window.rs:6403`.

Keep the same session open with its agent ended and no tabs alive. With Skip session confirmations enabled, restart it
from another surface. When a click arrives after the new row reaches the store but before the desktop redraws, Delete
computes Unconditional from the current row. The drawn row would have authorized NothingAlive. The shared admission path
also feeds Stop, Restart and Replace. Opus found the same gap in the launcher for Restart with / Replace with, where the
source warning's guard is derived again at the press.

Sol rated it critical and source-confirmed. Opus rated it likely and noncritical given the concurrent restart and
frame-sized window. Highest follows from the established authority to terminate running work beyond what the screen
authorized.

Carry the render's guard through the click and keep the supervisor's check at execution. Test a store liveness update
between render and dispatch. SPEC.md lines 2044–2049, 2063–2065 and 2079–2084 explicitly preserve these guards,
including inside the sub-second window accepted at lines 5135–5138. The queued sandbox Stop and suspend findings concern
that action's separate environment checks.
