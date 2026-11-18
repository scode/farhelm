# A single accept error can stop a localhost tunnel

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

One listener error can leave a tunnel’s port visible in Farhelm after it stopped accepting connections. The next list
refresh can remove the mapping without a reason.

## Details

Bucket: **other**. GUI migration final whole-tree review, R10 Opus 1.

Source references: `crates/farhelm-helm/src/tunnels.rs:126–132, 297–298, 447`.

The localhost accept loop breaks on any accept error. The listener task's end is only discovered by later pruning.
Creation bumps the events feed, but ending on an accept error does not, leaving a stale pill and a dead port until a
read prunes it. A transient connection abort or descriptor exhaustion is enough if returned by accept.

Opus rated this possible and noncritical. The loop and pruning are source-confirmed. Frequency with the actual listener
runtime is unverified. FILTER.md's rare, self-correcting rule could cover a short-lived listener refusal if the tunnel
recovered, but an existing mapping loses its listener and needs reopening. SPEC.md's Terminal experience section
describes the tunnel as living through ordinary host disconnects and until explicit closing, deletion or quit.

Retry transient accept failures with bounded backoff and notify the GUI when the listener stops permanently. Verify with
a controllable listener error rather than exhausting the host's descriptors.
