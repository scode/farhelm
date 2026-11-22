# Restart-gap Open Tab checks survivors but never witnesses whether a tab started

Reviewed commit: fcb6fa5e4a40e5e1a4bcc505ad5ffa32dfa95b79

## TLDR

The test can pass if a refused Open Tab launches a shell and cleans it up before replying.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/tab_lifecycle_edges.rs:89`.

tab_lifecycle_edges.rs:64-106 confirms state-directory ownership and checks restart guidance plus no surviving record or
host process after the error. host_creates.rs:96-115 counts only live processes at inspection.
service/core.rs:13867-13923 currently refuses at the restart-gap gate before running tab startup, as
SPEC_impl.md:2015-2017 specifies. But that same contract at :2022-2034 makes a later launch failure remove the record
and end the host, so those final emptiness checks cannot distinguish an attempted launch followed by cleanup from
correct admission. host_tabs.rs production :625-658 has a start-observation seam after recording and before sending the
child spec. Keep the final leak checks and additionally count attempts that reach the spec handoff, asserting the count
stays unchanged across the refused request. Bucket: other for the assertion gap. Confidence: confirmed by source. No
attempted launch was observed. Possible coverage: none in this changed case, and f4 is separate because it concerns
which terminal-gap representation returns the same response.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_e2e/f11`.
