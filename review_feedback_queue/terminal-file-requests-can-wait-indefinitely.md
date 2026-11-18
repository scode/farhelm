# A terminal file check or download can wait indefinitely

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

A terminal file link can stay at “Checking…” or “Downloading…” indefinitely while the host still appears connected and
the rest of Farhelm works.

## Details

Bucket: **high**. GUI migration final whole-tree review, R4 Sol stalled file checks and saves.

Source references: `crates/gui/farhelm-gui/src/terminal/files.rs:162`,
`crates/gui/farhelm-gui/src/terminal/files/download.rs:89`, `crates/farhelm-helm-client/src/transfers.rs:300, 367`,
`crates/farhelm-helm/src/downloads.rs:124, 187`, `crates/farhelm-helm/src/client/downloads.rs:40, 91`,
`crates/farhelm-helm/src/client.rs:2924`.

A connected supervisor can leave just StatFile or BeginDownload unanswered while still exchanging other host traffic.
The client uses a separate HTTP transport without a request deadline. Its wait ends when the work finishes or its reply
receiver closes. The helm is waiting for the host with a request that has no timeout. Leaving the view or disconnecting
can end the wait.

Sol rated this source-confirmed and noncritical. There is no separate verification verdict. SPEC.md “Healthy local
filesystems” and “Remote input, session defaults, and availability” accept some waits involving a hung filesystem or an
arbitrarily broken host. They are possible cover for those causes. This route also concerns a connected supervisor's
ordinary file request, and SPEC_impl.md “Session-host file reads” describes cancellation and disconnect without an
admission deadline or progress timer. The UI-channel watchdog does not run for these HTTP requests.

Bound stat and download admission separately from legitimate streaming time. Preserve cancellation and the existing
staging cleanup, and surface a host-wait refusal. Test a supervisor that stays connected and answers unrelated messages.
