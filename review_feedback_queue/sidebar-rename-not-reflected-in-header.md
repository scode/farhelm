# A sidebar rename never reaches the open session's header

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a helm upgrade leaves the page in the build-mismatch state, renaming the open session updates the sidebar but its
header keeps the old name until the page is reloaded.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F14 / COR-RENAME-NOT-IN-HEADER`, tagged **definite**. Anchors and title: `crates/farhelm-ui/src/session_view.rs:348`,
`crates/farhelm-ui/src/session_view.rs:350`, `crates/farhelm-ui/src/lib.rs:1514`,
`crates/farhelm-ui/src/list/view.rs:1992` — A sidebar rename of the open session never reaches its header

`SessionView` copies its `session` prop into its own signal once, at mount:
`let mut current = use_signal(||
session.clone());` (line 350). From then on it renders the header from that copy. Only
the view's own detail reads update it (`current.set(fresh)`), and there is no `use_reactive` on the prop.

`AppBody`'s `on_renamed` handler (`lib.rs:1514`) patches the selected session's title in `AppBody`'s signal. Its comment
says this is so "the titlebar can never sit on the old name… a latched build mismatch withdraws [the feed]". That
changes the prop passed to `SessionView`. But the view is keyed by id and the id is unchanged, so Dioxus re-renders it
without remounting, and nothing copies the new title into the view's `current`. The fix `on_renamed` was written to
provide therefore never takes effect. With the feed off (latched build mismatch), the header keeps the old title for as
long as the view stays open, and the header's Clone and Replace-with prefills use the same stale copy. With a healthy
feed, the view's feed-driven detail read hides the bug, and according to the reviewer the only e2e assertion on the
header title after a rename runs with a healthy feed.

The suggested fix is to copy prop changes into `current` (for example `use_reactive` on the title), or to pass renames
through a signal the view reads. A browser test under build mismatch should cover it.
