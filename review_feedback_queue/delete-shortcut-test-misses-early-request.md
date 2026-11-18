# A Delete shortcut test can miss an early request

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

The test can pass if Delete sends a request before the user answers the question, even though Enter is supposed to
authorize that request.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

The test drains commands only after Enter and accepts any matching DELETE. A shortcut that sends DELETE early and still
draws a question would pass, so check for zero DELETEs before Enter and exactly one afterward.

Reviewed locations: `crates/gui/farhelm-gui/src/session_pane/tests.rs:838-869`. Haiku out-T6.md §1. Suggested bucket
**other**.

The current test checks the exact session a path. `destructive-action-tests-omit-request-targets.md` covers other tests'
missing targets. SPEC.md “Desktop window chrome” keeps the Delete question on this shortcut.
