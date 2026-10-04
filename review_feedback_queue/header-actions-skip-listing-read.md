# Header Replace/Restart never refresh the sidebar

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After replacing a session from its header while the "reload for the new version" notice is up, the sidebar keeps showing
the deleted session and never shows the new one.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F13 / COR-HEADER-ACTIONS-SKIP-LISTING-READ`, tagged **definite**. Anchors and title:
`crates/farhelm-ui/src/session_view.rs:1089` — Header Replace and Restart never refresh the sidebar listing

When Replace succeeds from the session header, the task calls `on_replaced(new_session)`, which changes `AppBody`'s
selection, and releases the lock (line 1089 and following). It never asks the sidebar to re-read the listing. A
successful header restart asks only for the session view's own detail read (`refresh_after_restart`), not a listing
read. The sidebar's equivalents do refresh: `do_replace` calls `refresh(Trigger::Explicit)`, and so does the
replace-with composer, because otherwise the change is only learned when the feed next fires.

When the feed is healthy, it covers this within moments. Under a latched build mismatch the feed and the fallback poll
are both withdrawn, so after a header replace the sidebar keeps showing the deleted source row, whose actions all fail
with not-found, and never shows the new session. After a header restart, the row keeps its old ended status. The
suggested fix is to make the success paths of header replace and restart request an explicit listing read, through a
callback that `AppBody` routes to `ListView`.

Scope after later fixes: the original report also linked the stale ended status to an unconfirmed Delete killing a live
agent (F2). Commit `7586c7292ee035388039fcff82d46f2e3cb8da4d`
([#1152](https://github.com/scode/farhelm/pull/1152/changes)) fixed that consequence: unconfirmed Delete now checks
current agent and tab liveness under the supervisor's lifecycle lock and refuses before teardown. An independent review
confirmed this narrowing against `14c6b8b8`. The missing sidebar refresh remains unresolved.
