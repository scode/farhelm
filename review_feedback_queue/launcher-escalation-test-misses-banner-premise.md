# A launcher test can pass without showing its escalation banner

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

The test intended to prove that switching agents clears an escalation warning can pass when the warning never appeared.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

The fixture establishes the switch to OpenCode, but the test can pass without the banner ever appearing. Assert the
OpenCode banner before the switch to Codex, preferably in the shared fixture.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/escalation_tests.rs:48-63,114-123`. Haiku out-T3.md §1.
Suggested bucket **other**.
