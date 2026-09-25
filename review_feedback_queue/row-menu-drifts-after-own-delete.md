# Row menu drifts onto the wrong row after an optimistic delete

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After deleting one session, an actions menu opened meanwhile on another session can appear to belong to the row below
it, so its Delete can remove a different session than the one the user thinks they chose.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F15 / COR-MENU-DRIFT-AFTER-OWN-DELETE`, tagged **definite**. Anchors and title:
`crates/farhelm-ui/src/list/view.rs:1536`, `crates/farhelm-ui/src/rows.rs:177` — An open row menu drifts onto the wrong
row after this client's own optimistic delete

Each sidebar row has an actions menu whose panel is `position: fixed`, placed at screen coordinates measured once, when
the menu opens (`menu_panel::menu_panel_style`). Because it is never re-measured, the code closes it whenever the row it
belongs to might have moved. Scroll and resize close it, and so does the create form or host list changing shape. Inside
`commit_listing`, `rows::menu_row_reordered` closes the menu when the open row's index differs between the previous and
the new listing.

`do_delete`'s success path skips all of that. It removes the deleted row directly from the `listing` signal
(`current.sessions.retain(...)`, line 1536) instead of going through `commit_listing`, so the reorder check never runs.
Nothing disables other rows' menus while a delete is in flight, and an ended session deletes with no prompt (F1). A
concrete sequence:

1. Delete ended row A. The DELETE request is now pending.
2. While it is pending, open row B's menu. B sits somewhere below A.
3. The DELETE succeeds. A is removed locally and every row below moves up one slot, but B's panel stays where it was
   measured, which is now next to row C's toggle.
4. The next listing from the feed is compared against the already-pruned list, finds B at the same index, and leaves the
   panel open and misplaced.

The panel's buttons still act on B, but visually it now belongs to C. If B has ended, its Delete runs immediately, with
no prompt naming B. This is the wrong-row hazard the placement comments were written to prevent, reached by a path the
guard never sees. The suggested fix is to close `menu_open`, or run the reorder check, whenever the optimistic removal
drops a row above the open one.
