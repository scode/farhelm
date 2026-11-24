# Saving a delivered Codex login reports it as not delivered when GitHub fails later in the pass

Reviewed commit: 107915bb765efe671d5012f461ed3c340cca43af

## TLDR

When the user saves a new Codex login, Farhelm can install it on the host and still tell the user it was not delivered,
giving a GitHub token problem as the reason. The user may then retry or troubleshoot a delivery that already worked.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g11-credentials/f5`)

Location: `crates/farhelm-helm/src/provisioning/credential_delivery.rs:719`. Summary: Saving a Codex login that was
delivered reports it as not delivered when GitHub fails later in the pass

Revision of candidate f5, re-recorded as a finding: its earlier reason was neither a refutation in code nor a spec
acceptance.

Reviewed commit: 107915bb765efe671d5012f461ed3c340cca43af (rewrote the reply selection in push_one and added the test
that pins this reply). The behavior predates it: the old apply returned Err for the whole pass. The tip
e159ffc79ab147be44e92a92b59571650a4f322d still has it.

TLDR: When the user saves a new Codex login, Farhelm can install it on the host and still tell the user it was not
delivered, giving a GitHub token problem as the reason. The user may then retry or troubleshoot a delivery that already
worked.

Details: push_one (credential_delivery.rs:714-724) returns Some(error.with_warning(earlier)) for any Err from apply,
regardless of which slot failed. apply_codex_login writes auth.json and commits its stamp (lines 867-900) before
apply_github_token (line 810) runs, so a later GitHub failure (lookup, install, or gh helper setup) turns a delivered
Codex login save into "Saved; not delivered to <host>: could not resolve the recipe's GitHub credential". The test
codex_login_save_filters_an_unrelated_launch_lookup_warning (credential_delivery_tests.rs ~1450-1515) asserts exactly
this reply. SPEC.md lines 535-536 say the save "reports `Saved; not delivered to <host>: <reason>` for failures",
meaning failures to deliver the replacement; neither SPEC.md nor SPEC_impl.md accepts reporting a later slot's failure
for a replacement that reached the host. SPEC_impl.md line 6204-6205 ("Host details report both the failed launch lookup
and any later failure that ends the pass") covers host details, not the save reply. Fix: have push_one report a hard
error only when the pushed name's slot is at or after the failing slot (e.g. track which slot failed in DeliveryError
and select by uses_*), keep the host warning as is, and update the test. The same reasoning applies to a Claude/Codex
endpoint save followed by a Codex-login or GitHub failure.

Suggested bucket: other. The reply is wrong in the conservative direction; no credential is lost or exposed, and host
details show the real failure.

Confidence: confirmed (source).

proposed_drop: none.

possible_cover: the new test's docstring calls this "the existing pass error contract", which may reflect a maintainer
decision recorded somewhere outside the specs (not searched per the brief).

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g11-credentials/f5`.
