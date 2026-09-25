# In-flight uploads are aborted silently on reconnect

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A file dropped into a terminal can silently disappear, with no error, if the connection hiccups or the session restarts
while it is uploading.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F18 / COR-UPLOAD-ABORTED-SILENTLY`, tagged **definite**. Anchors and title:
`crates/farhelm-ui/assets/terminal.js:2232`, `crates/farhelm-ui/assets/terminal.js:2115`,
`crates/farhelm-ui/assets/terminal.js:1461` — Uploads in flight are silently aborted when the terminal reconnects or
remounts on restart

Dropping or pasting a file into a terminal uploads it to the session's host through the helm and then types the
host-side path into the terminal. This is handled by `installAttachments` in `terminal.js`. Its `dispose()` aborts every
in-flight upload (`controller.abort()`, line 2232) and clears the pane's status line. `send()` swallows the resulting
failure with `if (disposed) return;` (line 2115), so neither a success nor an error line is ever shown.

`dispose()` does not run only when the user leaves the session. It runs on every unmount of the terminal "island": every
reconnect attempt (`runReconnect` calls `farhelmTerm.unmount`, line 1461), and every restart remount. The upload does
not depend on the terminal WebSocket at all; it is a separate HTTP request. So a brief connection hiccup, or a restart,
while a file is uploading makes the upload vanish with no message.

SPEC "Attachments" says "Upload failures must be visible; an attachment must never disappear silently." Separately, SPEC
accepts that a cancelled upload whose publication was already under way may leave a complete file on the host without
the client learning its path, and that a retry may create a second copy. So the orphaned file is itself accepted
behavior; what violates SPEC is the silence. The suggested fix is either to keep uploads alive across reconnect and
restart remounts, or to leave a visible line such as "upload of <name> was interrupted (it may have been published)".
