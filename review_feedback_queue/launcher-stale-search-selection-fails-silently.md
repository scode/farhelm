# A stale launcher result can disappear without explanation

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Choosing a vanished host or stale recent setup can clear the search or do nothing without saying why.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

Enter on a stale recent setup silently does nothing, and a vanished host result can clear the search as if it were
applied. Use the launcher’s existing stale-result warning before clearing a query or abandoning a requested launch.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/view/recents.rs:149-165` and :204-217,
`crates/gui/farhelm-gui/src/launcher/view/search.rs:196-218`, :242-264 and :309-320. Haiku out-P5.md §3. Suggested
bucket **other**.

Possible cover is the rare-diagnostics filter. The reviewed route includes stale choices from a dialog that stays open
and offers no established sub-second input requirement. The existing launcher coverage for host fallback concerns
picking a different destination.
