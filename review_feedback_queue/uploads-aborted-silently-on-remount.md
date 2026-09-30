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

### Extension at de774a1ee8815ce833da77deac593a55d82f7be3: Upload results and failures are wiped by the reconnect remount within half a second

Source: gap-filling review pass, 2026-09-30, slice ui-terminal. Same root (the reconnect remount), different trigger:
uploads that already finished, whose landed path or failure message the remount erases. The reviewer suggested extending
this item rather than filing a new one. Reviewer's confidence: confirmed (traced end to end).

If you drop a file while the connection is failing, Farhelm deliberately shows "X landed at <path>; copy it from here"
instead of typing the path in. That message, and any "attaching X failed: …" message, vanishes about half a second
later, when the terminal starts reconnecting on its own. You never learn where the file went or why it failed.

- When an upload finishes on a terminal whose socket is no longer open, `send()` shows `LANDED_TEXT` with the published
  path (`terminal.js` lines 2185-2189). This is the only place that path is ever shown.
- That only happens after the socket died, and a socket death always starts recovery: `socketEnded` →
  `noteTransportLoss` → `scheduleReconnect`, first rung 500 ms (`RETRY_LADDER_MS[0]`).
- `runReconnect` then calls `farhelmTerm.unmount(el)` (line 1542) → `island.attachments.dispose()` (line 5085).
  `dispose()` blanks the status element (lines 2323-2327).
- The new mount starts with an empty `errors` list and paints nothing.
- Upload failures caused by the same outage are shown by `fail()` and erased the same way. That includes a fetch
  rejected because the helm restarted, which is exactly when the socket also dies.
- The message survives only where no automatic remount follows: a takeover or stall, or manual-only capability.

Consequences:

- SPEC.md's "upload failures must be visible; an attachment must never disappear silently" is violated.
- `LANDED_TEXT` is effectively unreachable whenever auto-recovery can run.

Fix: keep the status line's messages across remounts of the same element, e.g. hold `errors` in a per-element map owned
outside the island and have the next mount repaint it. Or have `dispose()` stop clearing messages that are failures or
landed paths. The queue item's fix for aborted uploads should go through the same mechanism.
