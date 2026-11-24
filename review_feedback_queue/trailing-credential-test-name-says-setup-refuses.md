# A credential-delivery test name says a failed Claude lookup refuses setup

Reviewed commit: 107915bb765efe671d5012f461ed3c340cca43af

## TLDR

No user-visible effect. A test's name tells future developers that a missing Claude login makes host setup fail, when
setup actually finishes and shows the problem as a note on its credentials step; someone trusting the name could "fix"
setup into refusing.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g11-credentials/f3`)

Location: `crates/farhelm-helm/src/provisioning/credential_delivery_tests.rs:1353`. Summary: Test name says a failed
Claude lookup refuses setup; setup completes with a notice

Reviewed commit: 107915bb765efe671d5012f461ed3c340cca43af (named the test and wrote SPEC_impl "During setup, a failed
Claude lookup is a setup error"); 58076d8d78478d84f1182c95268abe76083cfdca corrected the spec and the docstring but kept
the name. The tip e159ffc79ab147be44e92a92b59571650a4f322d still has it.

TLDR: No user-visible effect. A test's name tells future developers that a missing Claude login makes host setup fail,
when setup actually finishes and shows the problem as a note on its credentials step; someone trusting the name could
"fix" setup into refusing.

Details: credential_delivery_tests.rs:1353 `claude_lookup_failure_allows_codex_login_rotation_and_refuses_setup` asserts
`setup(...).unwrap_err() == WARNING` (line ~1411). The only callers, tensorlake_add.rs:689-695 and 1571-1582, take
`.err()` as a notice and mark the step Completed. SPEC_impl.md "Named credential catalog" (line 6207) now says: "During
setup, a failed Claude lookup is returned to host setup, which shows it as a notice on its completed credentials step",
and the test's own docstring (updated in 58076d8d7) says the same. Fix: rename to e.g. `..._and_reports_to_setup`.

Suggested bucket: other; a misleading test name with no runtime effect.

Confidence: confirmed (source).

proposed_drop: none.

possible_cover: none.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g11-credentials/f3`.
