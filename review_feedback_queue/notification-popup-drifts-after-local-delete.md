# A notification popup can stay at a deleted row's old layout

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

Opening a lower session's notifications while an upper session is being deleted can leave the popup detached from its
bell when deletion finishes. Ordinary subsequent listings do not correct its placement.

## Details

Delete success removes the upper row through `remove_row_reporting_menu_move`, checking `menu_open` but not `bell_open`
(`crates/farhelm-ui/src/list/view.rs:2054–2069`; `rows.rs:330–350`). The keyed lower row stays mounted, retaining the
position measured when its bell opened (`list/bell.rs:105–137,174–180`). Subsequent listings compare against the
already-pruned list (`list/view.rs:1402–1430`), so the deletion no longer triggers bell dismissal.

The precise sequence is: keep another session selected; delete unselected, non-checkout A; while its request is pending,
open B's bell below A; finish placement before the successful delete reply removes A locally. No earlier listing may
already have removed A, and no scroll, resize or selection-related dismissal may intervene. Use a viewport where
removing A does not resize the sidebar. Deletion can include a SIGTERM grace of up to five seconds
(`crates/farhelm-supervisor/src/service/sweep.rs:30–50,1621–1723`); it can finish earlier. This is not evidence of
measured frequency or a five-second window on every deletion.

Reconcile both the open menu and bell before discarding the old layout baseline. Clear still targets B, the session
whose bell the user opened; no wrong-target mutation or notification loss is established. Closing/reopening or a layout
dismissal repairs the display. This differs from automatic-dismissal focus loss: here the popup remains mounted.

Suggested bucket: **other**. Confidence: **likely**; state and placement paths are traced, but browser layout and
occurrence frequency are unverified. No runtime reproduction.

Possible cover: FILTER.md's **Rare, self-correcting glitches and imprecise diagnostics**. The display-only consequence
fits, including correction when leaving the view. Persistence across refreshes alone does not defeat that filter.
Whether the required ordering is rare is unresolved; uncertain matches are retained. The ledger's
`row-menu-drifts-after-own-delete.md` and `row-menu-drifts-on-row-height-change.md` concern action menus, not this
omitted bell path. SPEC.md Status gives the bell row-menu placement; SPEC_impl's row-menu contract requires movement
dismissal. No exact Planned, BUGS or current queue match was found.

Proposed drop: none; filter only if the rare-trigger premise is established. Provenance: persistent Astra/high review,
`ui-session-list-2/f2`.
