# Removing the notification bell skips focus return

Reviewed commit: 8a9b9b3de5abaa82738866543366f92247273fa3

## TLDR

Clearing notifications from another window while this window reads them can remove the list without returning keyboard
focus to the sidebar. Browser fallback and the next Tab destination remain unverified.

## Details

The added close effect requires the bell component to survive and render `open=None`
(`crates/farhelm-ui/src/list/bell.rs:112–122`). Listing reconciliation closes its parent state when the last
notification vanishes (`list/view.rs:1394–1405`) and publishes the listing in the same callback (`1590`). The row omits
the component entirely when empty (`list/row.rs:1049–1056,1794–1808`), so the effect never observes a closing prop on
this path. Local Clear separately targets the surviving row button (`list/bell.rs:238–246`); external clear does not
call it.

Return focus from a surviving owner or an appropriate unmount path, preserving independently moved focus. A regression
can empty the focused list through a later listing and inspect focus afterward. This is an incomplete edge of the
assigned fix, not a newly introduced external-clear behavior.

Suggested bucket: **other**. Confidence: **likely**, with static lifetime omission established and browser focus
behavior unexecuted. No security or user-work-loss consequence is established. Proposed drop: none.

Possible cover: `SPEC.md:3218–3236`, “One GUI at a time,” accepts best-effort cross-GUI updates and stale views but does
not clearly settle recovery when a fresh listing does arrive. `TRIAGE_OUTCOMES.md:16853–16865` decided every close
should return focus and records that work complete; that is motivation, not explicit acceptance of this omission. No
exact Planned, BUGS, current queue or filter match was established. No narrow human timing window is required, and
occurrence rarity was not measured.

Provenance: `notifications-whole-1/f1` in the trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. The
bell-to-bell handback concern instead extends the existing shared-helper record; it is a different lifecycle boundary.
No runtime reproduction was performed.

Original finding IDs: `notifications-whole-1/f1`.
