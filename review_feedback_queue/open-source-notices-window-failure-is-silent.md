# Open-source notices can fail without explanation

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Choosing Open-source notices can do nothing when the notices window cannot open.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

A failed window open leaves Open-source notices with no visible result. Surface the window-opening error in the main
window or in a native alert when no main window exists.

Reviewed locations: `crates/gui/farhelm-gui/src/desktop/licenses.rs:93-99`. Haiku out-P2.md §3. Suggested bucket
**other**.

SPEC_impl.md “Dependencies and licenses” describes the notices surface. The failure branch can occur on a direct user
click, and no rare-trigger premise was established for the diagnostic filter.
