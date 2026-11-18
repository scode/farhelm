# A login-file test assumes a shared path is missing

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

The missing-file test depends on a machine-wide file path staying absent instead of controlling that premise in its
fixture.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

The test only gets the intended read error if /nonexistent/auth.json is unreadable or absent on the runner. Use an
absent child of a test-owned temporary directory so the fixture controls that premise.

Reviewed locations: `crates/gui/farhelm-gui/src/settings/credential_tests.rs:680-689`. Haiku out-T6.md §3. Suggested
bucket **other**.
