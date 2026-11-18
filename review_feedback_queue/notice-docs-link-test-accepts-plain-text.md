# A notice link test can pass if the link stops working

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

The test only checks escaping after the notice recognizes a link, so it can pass if the address is displayed as plain
text.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

The escaping assertion runs only for a recognized link. Assert the expected runs for these two inputs so a regression
that rejects the query or fragment characters, or fails to trim the trailing period, fails the test.

Reviewed locations: `crates/gui/farhelm-gui/src/notices_bell/tests.rs:318-327`. Haiku out-T4.md §1. Suggested bucket
**other**.
