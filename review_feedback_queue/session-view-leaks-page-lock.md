# The session view can strand the page-wide operation lock

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If the open session is deleted from another window, or its host drops, while a restart or replace prompt is showing,
every button in the app stops working until the page is reloaded.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F7 / COR-PAGE-LOCK-LEAK-ON-UNMOUNT`, tagged **definite**. Anchors and title:
`crates/farhelm-ui/src/session_view.rs:1597`, `crates/farhelm-ui/src/session_view.rs:1676`,
`crates/farhelm-ui/src/session_view.rs:1810`, `crates/farhelm-ui/src/session_view.rs:1826`,
`crates/farhelm-ui/src/session_view.rs:1032`, `crates/farhelm-ui/src/session_view.rs:1094`,
`crates/farhelm-ui/src/session_view.rs:1401`, `crates/farhelm-ui/src/list/view.rs:1097` — The session view can strand
the page-wide operation lock, leaving the whole page inert until reload

The page has one shared mutual-exclusion token (`OpLock`, in `ops.rs`), created in `AppBody` and handed to both panes.
The session view holds it through the `PaneGate` wrapper. While the token is held, nearly every mutation in the app
refuses to start: sidebar row operations via `begin_row_op`, opening a row via `guarded_open`, create and host actions,
and the header's own buttons, which render disabled. `ops.rs` provides a cancellation-safe way to hold the token,
`claim_guard()`, which returns an `OpGuard` that releases on drop. Its doc comment says a manual `release()` at the end
of a component-scoped future "is therefore not a release guarantee", and `OpLock::claim`'s doc says "a leaked token
would leave the page permanently inert".

The session view does not use the guard. The header's Restart (line 1597), the header's Replace (1676), and the
interrupted card's Restart and Replace (1810, 1826) all call bare `lifecycle.claim()`. They hold the token while the
confirmation prompt is open and during the request, and release it only from the Cancel button or at the end of the
spawned task (1032, 1094). There is no `use_drop` fallback either: the one `use_drop` (1401) only unmounts terminals.
When `SessionView` unmounts, the spawned task is dropped before reaching `release()`, and a prompt's Cancel button
disappears along with the view, so the token stays held for good. Ordinary events that unmount the view in exactly that
window:

- Another client deletes the selected session. The next complete listing calls `on_removed` (`list/view.rs:1097`), which
  sets the selection to `None`.
- The interrupted card stops rendering while its Replace prompt is open, because the host went stale or another client
  restarted the session. The prompt and its Cancel go with it.
- In the browser build, the token prompt replaces the panes after a 401 while `AppBody`, the owner of the lock, stays
  mounted.
- The late auto-select from F9 swaps the selection.
- Possibly, a header replace races a listing read that already shows the source deleted.

After any of these, every row click, row operation, create and header action is refused or disabled, and nothing on
screen explains why. Only a page reload recovers.

The suggested fix is to hold an `OpGuard` in component state while a prompt is open and move it into the spawned task
when the user confirms, with a `use_drop` release as a backstop. The interrupted card's prompt should also either render
outside the branch that can disappear or be cleared when the card stops rendering.

## Additional site found at ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

The "Restart with" dialog, added after this item was reviewed, has the same leak and needs to be part of the same fix.

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F12 / COR-RESTARTWITH-LOCK`, tagged **definite**. Anchor and title:
`crates/farhelm-ui/src/session_view.rs:2003` — "Restart with" holds the page lock while its dialog is open and strands
it if the session view unmounts.

This is another case of the page-wide operation lock being released by hand (see F10/F11). That lock blocks nearly every
action in the app while it is held. The session header's "Restart with" button (the dialog that restarts a session with
a different model or effort) takes the lock with a bare claim the moment the dialog opens
(`crates/farhelm-ui/src/session_view.rs:2003`). It keeps holding it for as long as the dialog is open, including after a
failed attempt that leaves the dialog up. Only two places give the lock back: the dialog's Cancel handler (around line
2156), and the end of the restart task after a success (around line 1223). On a failure, the release is deliberately
skipped so that the dialog stays in charge.

If the session view goes away while the dialog is open, the dialog and its Cancel button go with it, and the lock stays
held until the page is reloaded. Several ordinary events cause that. Another client might delete the session. The
session might be deselected during a restart, which is the race described in F8: the supervisor briefly drops the
restarting session from its listings, so the browser thinks it was deleted and deselects it. Or a 401 might swap the
page for the token prompt. In every case the whole page goes dead with no explanation.

This is a new site of a problem the review feedback queue already tracks
(`review_feedback_queue/session-view-leaks-page-lock.md`). That item was reviewed before "Restart with" existed and
names only the header's Restart and Replace and the interrupted card's buttons. A fix scoped to the sites it lists would
miss this one. The recommendation is to fold it into that item's fix. Hold a self-releasing guard next to the dialog's
open state, move it into the restart task on submit, and hand it back when a failure keeps the dialog open.
Alternatively, add an unmount hook (`use_drop`) as a backstop that releases the lock if the view disappears while the
dialog holds it.
