# A confirmation test can accept the wrong focus target

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

The macOS-style Reset test can pass when focus lands on the wrong drawn control and stays there.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

The focus equality check can pass while both observations point at the same wrong drawn control. Assert the Reset
dialog’s own focus handle before and after its answer settles.

Reviewed locations: `crates/gui/farhelm-gui/src/prompt_tests.rs:1972-1997`. Haiku out-T4.md §1. Suggested bucket
**other**.

C3's verification correction narrows the original claim. `test_support.rs:221–233,539–546` and `focus.rs:1811–1834`
reject an empty or undrawn focus handle. The remaining weakness is that the test accepts the same wrong drawn handle
before and after settlement. Check that it is the dialog's own handle at both points.
