# An invisible model list can pass its height test

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

The launcher model list's height test can pass if the list collapses to zero height.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

A list with a recorded zero-height box passes the upper-bound check. Establish that the populated list shows its first
row and that scrolling reveals a row that was initially out of view.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/view_tests.rs:1984-1999`. Haiku out-T4.md §1. Suggested bucket
**other**.
