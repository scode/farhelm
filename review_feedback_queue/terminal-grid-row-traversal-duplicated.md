# Terminal grid features have two row walkers

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Terminal word selection and file-path recognition each interpret the same grid row separately, so their treatment of the
same cells can drift.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

Both walks fill gaps, split plain runs into single cells, preserve the width of other runs, and remove the wide-wrap
pad. Have a shared row walker yield column, width, and text, then let path matching and word selection build their byte
and UTF-16 mappings respectively.

Reviewed locations: `crates/gui/farhelm-grid-model/src/paths.rs:335-383`,
`crates/gui/farhelm-grid-model/src/selection.rs:241-281`. Haiku out-P1.md §1. Suggested bucket **other**.
