# Focus checks repeat the drawn-control test

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Several focus paths reimplement the check that a control is present in the frame.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

These sites separately check whether the window root contains a focus handle in the current frame. Route the root checks
through `is_drawn` and retain the layer-specific conditions at the callers.

Reviewed locations: `crates/gui/farhelm-gui/src/focus.rs:761-765`, :1228-1234, :1427-1431 and :1684-1688. Haiku
out-P3.md §1. Suggested bucket **other**.
