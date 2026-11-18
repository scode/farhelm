# Session pane repeats last-known status calculations

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

The Details bar and the unavailable-session view separately calculate the same last known status and activity age.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

Both project the same session data into a status and an activity age and already share their renderers. Share the
projections and keep each surface's visibility decisions local.

Reviewed locations: `crates/gui/farhelm-gui/src/session_pane/details.rs:239-245`,
`crates/gui/farhelm-gui/src/session_pane/details.rs:260-264`,
`crates/gui/farhelm-gui/src/session_pane/unavailable.rs:561-569`. Haiku out-P7.md §1. Suggested bucket **other**.
