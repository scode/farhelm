# Checkout fixtures inherit Git repository selection

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An inherited GIT_DIR can redirect a managed-checkout test's initialization into another fixture repository.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Ordinary foreign init can write template/configuration data under the inherited selector. The recorder may refuse some
retained-environment invocations before the body, so that is not universal dispatch proof. Stronger separate-git-dir
movement/deletion and user-work-loss claims were independently refuted. No environment variable of the test process was
modified during review.

Source locations: `crates/farhelm/tests/e2e/containerized_sessions.rs:84-90`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. Assigned new callers: f73589c5201a0408f2f1555c18a4efca6f6057be
and 2d98a04e3e291bf2b05e9f9bdaf2a9fc2595ee11; helper predates them. TLDR: with an inherited absolute GIT_DIR,
managed-checkout setup can initialize/reinitialize that selected Git directory instead of the fixture checkout. The
fixture can lack its intended .git and its setup acts outside its owned Git target. Source: managed_checkout invokes git
init --quiet --initial-branch=main with the checkout argument but no child-environment scrub (:84-90). The normal
recorder removes only unrequested FARHELM_* values (scripts/record-test-run.py:1281-1289); harness/testtrace do not
replace the test process environment (harness.rs:1653-1671; testtrace/src/lib.rs:994-1025). Installed primary Git docs
state GIT_DIR replaces the normal .git target (decoded /usr/share/man/man1/git-init.1.gz:41-49); hooks export
repository-selection variables for child Git commands (decoded /usr/share/man/man5/githooks.5.gz:41-50). A hook running
tests is an ordinary source of this environment. The isolation test later requires .git/HEAD (:687-688), so redirected
setup can fail it; the new account/wrapper calls do not independently prove where init wrote. Confidence: confirmed
conditional source path/documented command behavior; no experiment or observed foreign-directory mutation. Suggested
bucket other: test setup correctness and unintended reinitialization only. No established credential/process/data loss
or catastrophic tooling consequence. Git's init docs say existing contents are not overwritten (:49); do not import the
old repository-discovery fixture's gitfile/move damage into this helper. Potentially-highest concern and this narrowing
remain preserved under f4 for independent parent review. proposed_drop none. possible_cover:
TRIAGE_OUTCOMES.md:7244-7258 and :14297-14322 repaired unsanitized Git children in other named Rust/browser fixtures,
not this helper. SPEC_impl.md:3844-3850 covers production discovery, not this fixture; tooling acceptance :5687-5700
covers hostile-account and concurrent-run assumptions, not accidental Git-hook selection. Required coverage searches
found no exact Planned, unlanded plan, BUGS, queue, spec or filter match. A child-only Git environment scrub would keep
setup inside the owned fixture; never mutate the test process environment. The preceding reviewed-event file was
malformed JSON and rejected by evidence record; that failed event was not appended, and this valid event supplies the
completed disposition.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g126/f4`, `a037/f1`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
