# A stalled shell-tab request can close an unrelated launcher draft

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

A host slow to open or close a shell tab can make Farhelm reconnect to a helm that was still answering. If you were
editing a new session in the meantime, the launcher closes and loses that draft.

## Details

Bucket: **high**. GUI migration final whole-tree review, R4 Sol stalled tab request (D).

Source references: `crates/gui/farhelm-gui/src/terminal/set.rs:780–800` at the reviewed commit (main
`crates/gui/farhelm-gui/src/terminal/set.rs:784–804`), `crates/farhelm-helm/src/sessions.rs:5037–5052, 5062–5079`,
`crates/farhelm-helm/src/client.rs:2914–2925`, `crates/gui/farhelm-gui/src/launcher/entry.rs:76`,
`crates/farhelm-helm-client/src/channel.rs:76, 431–433, 514–517`, `crates/gui/farhelm-gui/src/session.rs:207–216`.

The tab routes wait directly for the supervisor. More than 30 seconds without that command reply trips the channel
watchdog even as heartbeat replies arrive. Opening a tab does not preclude opening the launcher and typing a command
during the wait. The connection reset retires the old UI session and closes all of its layers.

Sol rated this source-confirmed and critical. Opus verification D confirmed the unbounded waits and rated it noncritical
because the request must stall and the lost draft is unsaved form input. High reflects disruption of unrelated
operations. SPEC_impl.md “The UI channel” (lines 236–239 and 453–461) puts the watchdog above the host reply bound. The
documented disposal of drafts on a genuine reset covers the reset behavior, while the missing bound makes a host stall
look like a helm failure.

Bound Open tab and Close tab at HOST_REPLY_BOUND. Test a supervisor that holds one tab request beyond the bound while
the helm continues to answer.
