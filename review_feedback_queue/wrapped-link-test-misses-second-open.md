# A wrapped-link test can pass when a valid click does nothing

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

The test checks that the second click gives no refusal but never verifies that it opens the link.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

Absence of refusal cannot distinguish a second open from a dropped click. The test platform records the last opened URL,
so seed that slot with a different URL through the test app before the second click and assert that the wrapped link
replaces it.

Reviewed locations: `crates/gui/farhelm-gui/src/terminal/attached/tests.rs:4878-4899`. Haiku out-T8.md §1. Suggested
bucket **other**.
