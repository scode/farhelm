# Lifecycle fixture cleanup stops after release failure

Reviewed commit: 8bd92e1336565dfc263fe5f3fa2fef413a82a87a

## TLDR

One fixture-release failure can skip the remaining browser-test cleanup and leave shared settings or resources behind.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `e2e/tests/tensorlake-hosts.spec.ts:318-326 at 8bd92e1336565dfc263fe5f3fa2fef413a82a87a`.

Source assessment (read with the later corrections below) (review record g037/g037-l1, event 4):

Reviewed commit: 8bd92e1336565dfc263fe5f3fa2fef413a82a87a. TLDR: If an inline-recipe browser test cannot delete its
temporary recipe, it leaves its test API key saved and can cause unrelated later browser tests on that same stack to
fail their clean-account premise. Details: e2e/tests/tensorlake-hosts.spec.ts:117-120 saves test-key, then :318-326
awaits DELETE and asserts its response before attempting the independent account-clear PUT. A transport rejection at
:319 or a storage-error HTTP response rejected by :322 exits finally before :323-326. The recipe route's storage-error
returns are source-established in crates/farhelm-helm/src/tensorlake/recipes.rs:672-675,685-691, with fallible storage
operations at :440-468. This does not require a vendor cleanup operation or a billed recipe: local storage can fail on
the resource-free path too. e2e/playwright.config.ts:103-109,137-146,154-162 establishes a long-lived stack and the
requirement that tests put shared state back; helpers/evidence.ts supplies no account reset. A later palette case in
this very test loop requires api_key_set=false at tensorlake-hosts.spec.ts:94-102. The exact cleanup contract is already
encoded in helpers/cleanup.ts:1-20: attempt every owned release even if an earlier one fails, catch each step
separately, collect errors, preserve a body failure as primary via stderr, and otherwise throw an AggregateError after
every attempt. The new sequential finally bypasses that mechanism, so a DELETE failure both skips an independent
restoration attempt and may replace the original body error. Suggested fix: run recipe deletion and account clearing as
separate cleanupOwnedSteps callbacks and track bodyFailed, following the helper's contract. Suggested bucket: other,
because the established impact is test contamination and misleading diagnostics, not harm to product users or their
work. Confidence: confirmed source-level conditional failure path; no runtime observation and no assertion that a
refusal has occurred or that this fixture deliberately produces one. Practical limits: if the entire helm or request
context remains unavailable, the key-clear attempt may fail too; independent attempts cannot guarantee restoration, but
must not omit it solely because recipe deletion failed. No security/data-loss route is established. Temporal status:
introduced in 8bd92e1336565dfc263fe5f3fa2fef413a82a87a and remains at pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b:323-331. proposed_drop: false; this revises the prior dismissal, whose
lack-of-injected-failure premise did not refute the required cleanup failure path. possible_cover: not searched per
brief.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g037/g037-l1`. The retained
private log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
