# A restart failure test can miss a change in where it appears

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

The updater test can pass if a restart failure moves inline under a new element ID instead of staying in the hover help.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

A restart failure drawn inline under a new ID would pass both negative selector checks. Check the readout's inline
content and assert that the restart failure is available in hover help.

Reviewed locations: `crates/gui/farhelm-gui/src/update_readout/view/tests.rs:512-520`,
`crates/gui/farhelm-gui/src/update_readout/view.rs:364-405`. Haiku out-T8.md §1. Suggested bucket **other**.
