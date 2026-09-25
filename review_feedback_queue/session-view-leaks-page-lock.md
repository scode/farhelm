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
