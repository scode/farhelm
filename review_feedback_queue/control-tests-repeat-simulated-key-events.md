# Control and approval tests repeat simulated key calls

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Control and approval tests each construct and deliver the same simulated key presses and releases.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

These test helpers construct the same key events, park the executor and check invariants. Move the component press and
release helpers to test support and reuse them for the radio and approval cases.

Reviewed locations: `crates/gui/farhelm-gui/src/components/tests/radio_group.rs:265-274`,
`crates/gui/farhelm-gui/src/components/tests/mod.rs:76-97`,
`crates/gui/farhelm-gui/src/approvals/region_tests.rs:1447-1463`, `crates/gui/farhelm-gui/src/test_support.rs:221-227`.
Haiku out-T1.md §2. Suggested bucket **other**.
