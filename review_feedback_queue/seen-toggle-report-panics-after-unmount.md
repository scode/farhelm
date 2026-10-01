# A read/unread toggle can crash the app after a remount

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

In the desktop app, marking a session read or unread as the first action after a token rotation can crash the window. In
the browser, the same can happen if another request triggers the sign-in prompt while the toggle is being saved.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F48 / COR-SEEN-REPORT-PANIC`, tagged **definite**. Anchor and title: `crates/farhelm-ui/src/list/view.rs:2643`
— a manual read/unread toggle can crash the UI if the session list unmounts before its write returns.

The sidebar lets the user mark a session read or unread by hand, from the row's menu or by clicking the row's status
dot. The UI saves that choice to the helm through a small write queue in `crates/farhelm-ui/src/api.rs`
(`spawn_seen_writer`). On purpose, the writer runs as a "forever" task (Dioxus's `spawn_forever`) that is not tied to
any component, so that leaving a view does not cancel a write already underway. When the write finishes, the task calls
a "report" callback the caller handed in.

For the manual toggle, the report callback (`view.rs:2643-2656`) updates the session list's per-row error map, which is
a piece of component state (a Dioxus signal owned by the session list), by calling `errors.write()`. In the pinned
dioxus-signals 0.7.10, `write()` is literally `self.try_write().unwrap()`. If the session list has unmounted by the time
the write returns, the signal's owner is gone, `try_write()` returns a "dropped" error, and the `unwrap` panics inside
the background task, with nothing to catch it. The automatic "mark seen when opened" path in
`crates/farhelm-ui/src/session_view.rs` (around line 1039) does not have this problem, because its report only logs.

There are two concrete ways for the list to unmount mid-write:

- Desktop app (confirmed by reading): the desktop app keeps a separate credential for its webview, and after a token
  rotation the first request that gets a 401 triggers a credential refresh. That refresh calls
  `require_desktop_webview_reauth()`, which bumps a signal that makes the app's authentication gate stop rendering the
  whole app body (list included) before the retried request goes out. If the toggle is the first request after a
  rotation, its report runs after the list is gone and panics whether the retry succeeds or fails.
- Browser (possible): any other request's 401 makes the browser swap the whole authenticated page for the token prompt
  (`crates/farhelm-ui/src/lib.rs:1259`, the same unmount that F10 relies on). If a toggle's write is in flight at that
  moment, its report panics when it lands.

The consequence is a crash (the desktop window, or the WebAssembly runtime in the browser) where the user should have
seen at most an error line on the row. This is new evidence on the tracked queue item
`desktop-reauth-remount-loses-action.md`, which notes that these "forever" writers survive the remount but not that
their report then crashes. The suggested fix is small: in this report use `errors.try_write()` and silently drop the
update when it fails, and document on the report type (`SeenWriteReport`) that a report can run after its caller has
unmounted.
