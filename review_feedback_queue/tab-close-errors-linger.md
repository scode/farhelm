# Tab-close errors linger for vanished tabs

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A "close tab" error can linger under the tab strip for a tab that no longer exists, with no way to dismiss it short of
leaving the session.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as `F23 / COR-TAB-ERRORS-LINGER`,
tagged **possible**. Anchors and title: `crates/farhelm-ui/src/session_view.rs:1170`,
`crates/farhelm-ui/src/session_view.rs:1199`, `crates/farhelm-ui/src/session_view.rs:1946` — Tab-close errors for tabs
that have since disappeared stay on screen for the life of the view

The session view stores per-tab operation errors in `tab_errors`, a map keyed by tab id. A failed tab close inserts
"close tab: …" under that tab's id (line 1199). The only thing that removes the entry is a later close attempt for the
same tab (line 1170). Rendering shows every entry (line 1946).

The common race makes this sticky. The helm answers an unknown tab id with 404, and `close_tab` reports a 404 as an
error. So if another client closes the tab, or the tab's shell exits and the supervisor reaps it, just before this close
lands, the user gets a "close tab: … not found" line. The tab then drops out of the strip on the next detail read, and
with it the only control that could clear the message. The line stays under the strip until the user leaves the session.
This is cosmetic, but misleading. The suggested fix is to prune `tab_errors` keys that are neither `TAB_OPEN_ERROR_KEY`
nor a listed tab whenever a detail read commits (the place where `closed_tabs` is already pruned), or to filter the
rendering against the visible tabs.
