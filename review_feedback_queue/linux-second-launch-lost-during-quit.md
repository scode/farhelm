# A Linux relaunch can be lost as Farhelm quits

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

If you reopen Farhelm while the old window is quitting, the new process can say it handed off to that window and exit.
The old process then exits too, leaving no window.

## Details

Bucket: **other**. GUI migration final whole-tree review, R7 Opus 2.

Source references: `crates/farhelm-desktop/src/second_launch.rs:38–52, 73–85, 91–97`,
`crates/farhelm-desktop/src/lib.rs:164–173, 211–219`.

A Linux launch counts Unix socket connect as a successful handoff and exits before taking the app lock. The old instance
can still have its socket until the quit callback retires it. The kernel can queue the connection before the listener or
window processes activation, and the window can quit without acknowledging it.

Opus rated this source-confirmed and noncritical, with a narrow quit window. There is no separate verification verdict.
FILTER.md's rare, self-correcting rule is possible cover because a later relaunch can work. The forwarded launch returns
success and reports the handoff even though no window comes forward, so the success exception makes coverage unclear.

Acknowledge once the running window admits the activation. If it is quitting or disappears first, allow the new
invocation to take the normal startup path. Check a handoff queued just before quit.
