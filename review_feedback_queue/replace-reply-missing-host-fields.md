# Plain Replace drops the replacement's host fields

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After replacing a remote session, clicking New proposes the local machine and an empty directory instead of the replaced
session's host and folder.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F21 / COR-REPLACE-REPLY-MISSING-HOST`, tagged **definite**. Anchors and title:
`crates/farhelm-ui/src/list/view.rs:1719`, `crates/farhelm-ui/src/session_view.rs:1089` — Plain Replace hands the helm's
bare reply to the selection without host fields, so New loses the replacement's host

Session rows in a listing carry three host fields: `host` (the registry id), `host_identity` and `host_name`. Mutation
replies do not. The helm's replace endpoint returns the supervisor's bare `SessionInfo`, which decodes with all three
set to `None`. The `Session` docs say callers must fill those in before such a reply stands in for a row. The
create/replace-with form does so with `enrich_created_session`. The two plain Replace paths do not: the row menu's
`do_replace` calls `on_open.call(session)` (`list/view.rs:1719`), and the header replace calls
`on_replaced.call(new_session)` (`session_view.rs:1089`). Both hand the raw reply to `AppBody` as the new selection, and
nothing later refreshes `AppBody`'s selected copy from the listing.

The New button's default destination, `OpenDestination::of_session`, needs `session.host`. With it `None`, New falls
back to the local host and does not inherit the working directory. After replacing a session on a remote host, New
therefore proposes the local machine with an empty directory. SPEC defaults New to the selected session's directory and
installation. The header's Clone and Replace-with prefills also carry no host until the view's first detail read. The
suggested fix is to fill in both replace replies with the source row's host, identity and host name (replace always
keeps the host), or to re-seed the selection from the view's first detail read.
