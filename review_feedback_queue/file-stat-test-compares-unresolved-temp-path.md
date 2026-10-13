# File-stat test fails through a symlinked temporary directory

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

The supervisor's new file-download test fails on ordinary macOS installations even when the feature behaves correctly.

## Details

Suggested bucket: `other`. Confidence: confirmed by code inspection; not executed on macOS. Source: adversarial
file-security reviewer. Proposed drop: none.

Introduced by `871499e5b39d52eb4b00b54090f9af1f1d8a108b`. The test
`service::downloads::tests::stat_checks_kind_readability_size_and_symlink_target` compares `info.path` against the
unresolved fixture pathname (`crates/farhelm-supervisor/src/service/downloads.rs:413–422`). Production `inspect`
canonicalizes that result (`94–110`). `farhelm_teststate::tempdir` deliberately creates beneath literal `/tmp` and
returns that spelling (`crates/farhelm-teststate/src/lib.rs:68,643–644,665–667,691–711`). On macOS, `/tmp` resolves to
`/private/tmp`, so the string assertion fails. This supervisor unit test is not Linux-only.

Run the exact test through the recorder on macOS to verify. Canonicalize the expected pathname before comparing; keep
the symlink-target assertions, which establish the actual file-read contract.

Possible cover: TRIAGE_OUTCOMES.md's `tilde-replay-test-equates-display-canonical-paths.md`,
`checkout-recovery-compares-canonical-roots-unresolved-fixture.md`, and
`directory-browse-test-compares-canonical-output-uncanonicalized-fixture.md` record similar mistakes in other tests.
Their named scopes do not cover this newly added unit test. The related Rust file-download e2e module is Linux-only;
this finding does not claim it fails on macOS.
