# The remembered-selection lookup overrides the user's choice

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a large fleet, a session clicked right after the page loads can be swapped a few seconds later for the previously
remembered one, and one failed read makes the app forget the remembered session.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F9 / COR-REMEMBERED-LOOKUP-OVERRIDES-SELECTION`, tagged **definite**. Anchors and title:
`crates/farhelm-ui/src/list/view.rs:2136`, `crates/farhelm-ui/src/list/view.rs:2140`,
`crates/farhelm-ui/src/list/view.rs:2143` — The remembered-selection lookup opens its late result over the user's own
choice, bypasses busy guards, and retires the remembered id on transient errors

When nothing is selected, `ListView`'s auto-select effect opens a session so the right pane is not empty. It prefers the
remembered selection, which is stored in the helm's shared preferences. On a large fleet the helm truncates the listing,
so the remembered session may simply be absent from it. In that case the effect spawns `fetch_session(remembered)`, a
detail read that can take up to 60 seconds because the helm may have to query a remote supervisor over SSH.

That task has two problems.

- When it finishes with `Ok(Some(session))`, it calls `on_open` unconditionally (line 2140). It does not check that the
  selection is still empty, nor does it apply the `ops.busy_now()` / `pending` guards that the effect's synchronous
  branch (line 2156) and `guarded_open` use. If the user clicked a row while the lookup was running, the late reply
  replaces their choice: `SessionView` remounts, detaches the terminals the user just opened, and can tear down a view
  with a prompt open, which is one of F7's triggers.
- Every other outcome falls into the `_ =>` arm (line 2143) and sets `remembered_dead`. That includes `Err`, meaning a
  transport failure or a timeout, not just a 404. The block comment above says "only a definite not-found retires it";
  the inline comment says "(or unreadable)", so the code's own comments disagree. Once retired, the remembered session
  is skipped for as long as this `ListView` stays mounted. (`api::fetch_session`'s own doc also notes that on this route
  even a 404 is ambiguous, because the detail route searches a capped listing.)

A background reply overriding a newer user action is the more serious half. The suggested fix is to apply the result
only if nothing is selected, the lock is free and `pending` is empty; to retire the id only on `Ok(None)`; and to let an
`Err` retry on the next listing.
