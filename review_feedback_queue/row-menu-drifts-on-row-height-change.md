# Row menu drifts when a row above changes height

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With a row menu open, an agent above it finishing can make the menu (including an unconfirmed Delete for an ended
session) appear to belong to the next session down.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F16 / COR-MENU-DRIFT-ROW-HEIGHT`, tagged **possible**. Anchors and title: `crates/farhelm-ui/src/list/view.rs:770`,
`crates/farhelm-ui/src/list/view.rs:792`, `crates/farhelm-ui/src/list/row.rs:958`,
`crates/farhelm-ui/src/list/row.rs:1582`, `crates/farhelm-ui/src/rows.rs:177` — An open row menu can float over a
different row when a row above changes height at the same index

This is the same fixed-position row menu as F15, reached by a different path. The close triggers (the effect at line 792
and the index check in `commit_listing`) notice scrolls, resizes, create-form toggles, host-list shape changes and index
changes of the open row. They do not notice a row above the open one growing or shrinking in height while keeping its
index. The comment at lines 770–787 acknowledges this and accepts it as a rare residual, giving "a per-row error line
appearing" as the example.

There is a more frequent trigger than the comment assumes. Outside compact mode, a row gets an extra detail line when
`has_detail` is true (`row.rs:958`), and that happens whenever the session has ended (`ended_badge.is_some()`) or is
stale. So an ordinary agent exit on a row above the open menu adds a line (`row.rs:1582`) and pushes the open row down,
while under the activity, created or title sort orders the row's position may not change at all. The panel, possibly
already showing "confirm delete", then sits beside the neighbouring row. Its actions stay bound to the original session
and the prompt quotes that session's title, but visually it belongs to the next row down. No SPEC decision accepts this,
and the code's acceptance rests on a frequency estimate this trigger undercuts.

The suggested fix is to include the heights of rows above the open one (or their `has_detail` and error signatures) in
the close check, to re-measure the panel on every listing commit while a menu is open, or to watch the toggle's position
with a ResizeObserver.
