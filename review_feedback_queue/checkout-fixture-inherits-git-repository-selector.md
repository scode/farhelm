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

## Trailing review through ca9b0a19

Running the new unit-test fixture with an inherited absolute Git repository selector can initialize and mutate a
repository outside its temporary fixture.

Suggested bucket: **extend**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source defect, not runtime-reproduced. Source: test_guest_workflow.py:324-326 supplies no child env to git init or its
mutating helper, then :328-336 runs add, commit and globally ordinary version tags. -C changes directory but does not
cancel GIT_DIR/GIT_WORK_TREE; local primary docs /usr/share/man/man1/git-init.1.gz raw lines 41-49 explicitly give
GIT_DIR priority, and /usr/share/man/man5/githooks.5.gz raw lines 41-50 explain hook inheritance and the need to clear
selectors for foreign repositories. A Git-hook context is an incidental producer, not only a deliberately exported test
environment. No claim that init itself overwrites existing contents: the docs say it does not. Exact later effects
depend on inherited worktree/index and existing paths/tags; fixture isolation already fails at init. Fix: construct a
child-only Git environment for init, mutations and Source reads, clearing repository/index/config selectors; do not
modify the running test process environment.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:releasing/mac-vm-test/test_guest_workflow.py:321-336.

The new test_previous_source_cross_check_answers_only_where_the_layout_can launches git init, add, commit and tag
through subprocess.run without env isolation. git -C selects a directory but does not clear
GIT_DIR/GIT_WORK_TREE/GIT_COMMON_DIR. The helper overrides identity/signing/hooks but leaves repository selection
inherited.

running the new unit-test fixture with an inherited absolute Git repository selector can initialize and mutate a
repository outside its temporary fixture. Source-confirmed consequence: initialization targets the inherited Git
directory; add/commit/tag also inherit selectors rather than using a guaranteed owned repository. Later mutation success
depends on worktree/index contents, config and existing tags, and is not claimed for every inherited environment.
Suggested bucket other, confidence confirmed conditional source defect. No runtime reproduction and no
catastrophic/user-work-loss consequence claimed. Source: tip test_guest_workflow.py:174-194 does not scrub the parent
environment; :324-326 provides no child env; :328-336 runs the mutation sequence. Tip brick_test.py:68-74 Source.git
also inherits the environment, so its later reads do not independently establish fixture ownership. Additional synthesis
check: tip BRICK-TEST.md:545-552 documents unittest discovery through the recorder; record-test-run.py:1281-1289 copies
all non-FARHELM_ variables, and :1703-1707 passes that environment into checkout discovery. The recorder is not a
Git-selector scrub. This is a conditional route with an inherited absolute GIT_DIR, not a claim that every Git hook
exports an absolute path or that every recorder invocation admits it. Installed primary Git docs git.1.gz raw :205-229
state that GIT_DIR disables repository discovery and -C only chooses cwd; :1426-1434, :1465-1480 describe
all-core-command selectors and the default index. git-init.1.gz:41-49 confirms GIT_DIR selects init's target and that
reinitializing does not itself overwrite existing contents. githooks.5.gz:41-50 establishes incidental selector
inheritance and prescribes clearing selectors for foreign repositories. proposed_drop none. possible_cover unchanged:
queue checkout-fixture-inherits-git-repository-selector.md names the Rust containerized_sessions fixture; ledger
:7244-7258 and :14297-14322 names earlier Rust/browser fixes, not this new Python fixture. Ledger :13675-13687 discard
is deliberately configured devbuild reset/clean, not incidental test-hook inheritance. SPEC_impl.md:6795-6808 accepts
hostile other-account/concurrent-run/same-run PID assumptions, not this route; Planned is empty; required coverage
search found no exact trigger/consequence/scope match. Keep as a distinct named caller; parent may extend the existing
Git-fixture queue item while preserving g201/f1 provenance. Fix should use child-only isolation for fixture
initialization/mutations and Source reads, never mutate the test process environment.

Possible coverage and caveats: review_feedback_queue/checkout-fixture-inherits-git-repository-selector.md concerns
containerized_sessions.rs initialization, not this new Python fixture or its commit/tag writes.
TRIAGE_OUTCOMES.md:7244-7258 and :14297-14322 repaired named Rust/browser fixture callers; :13675-13687 discarded a
devbuild reset/clean issue under a deliberately set environment, not tests under incidental hook inheritance.
SPEC_impl.md:6795-6808 accepts hostile-other-account/concurrent-run assumptions and same-run PID signaling, not
inherited Git selection. TODO.md Planned (:23-24) is empty. Searches across SPEC.md, SPEC_impl.md, TODO.md, BUGS.md,
plans/queue, queue INDEX and TRIAGE_OUTCOMES.md found no exact accepted coverage. FILTER.md does not cover ordinary
inherited hook context causing writes to a different repository.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g201/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 55f1e79f56ac6cf7b536badfc7c2ae59e9d74ca6.. This
was a source-only review; runtime occurrence and frequency were not measured.
