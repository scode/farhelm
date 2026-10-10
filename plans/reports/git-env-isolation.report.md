## What this was about

Creating a session in a fresh GitHub checkout could overwrite or delete branches and tags in another repository if the
login shell set `GIT_DIR` or a similar Git repository override. Farhelm's cache refresh inherited that override, so Git
could operate on the other repository despite the cache path Farhelm named. Separately, repository-discovery tests
inherited Git overrides and could alter the repository of the developer running them, including moving its Git metadata
into temporary storage.

The maintainer approved both fixes together, provided they stayed small. Both are implemented in one draft PR: Farhelm's
checkout-preparation Git commands discard repository-local overrides, and the discovery test fixtures discard inherited
repository selection and configuration overrides on their Git children.

## Things you should know

Git configuration and credentials remain available during checkout creation. The post-clone hook and agent still receive
their ordinary login environment; the isolation applies only to Farhelm's own preparation Git commands. The regression
reaches Ready through an offline Git URL rewrite, while preserving a foreign repository's branches, tags, Git-directory
identity and configuration.

Both outcomes stayed within their complexity gates. There is no new environment plumbing, shared command helper or test
seam. The feedback items are removed and both ledger entries identify the completed change and PR.

## Open questions and possible follow-ups

None. Neither outcome was dropped or left awaiting a decision.

## PRs

- [#1798](https://github.com/scode/farhelm/pull/1798/changes): protect unrelated repositories during fresh-checkout
  creation and repository-discovery test setup; one commit, draft, based on main.

## Checks run, reused and skipped

- Two exact pre-fix regressions were recorded and retained. Run `46d3d1fa-b94e-4a7d-9421-b79764642ddd` failed because
  the redirected fetch encountered Git's checked-out-branch protection. After detaching the foreign fixture's HEAD and
  asserting that premise, run `da2cff9c-c61b-423d-9686-1a72650a7818` failed because the foreign branch and tag were
  actually pruned. These are failed pre-fix observations, not passes or latent flakes.
- Recorder run `63757a2f-5e8d-4140-84ef-21a62423604a`:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(launch::tests::) | test(repository_discovery::tests::)'`
  passed all 69 selected cases, including the regression; 1018 cases were selected out or ignored. Four slots, zero
  retries, pinned nextest and required pinned tmux. The retained report is complete and output contains no runtime
  `SKIPPED` marker.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the isolated
  `python -B scripts/check-test-sleeps.py` (281 calls, zero unannotated), changed-Markdown `dprint check`, and
  `python3 releasing/check-changelog.py format` passed.
- The runtime evidence covers the changed Rust sources on base `13ff946c`. It is reused for the final PR because
  subsequent edits were Markdown bookkeeping and a queue-only rebase onto `ee306896`; the complete upstream diff
  contained no product or specification change. No Rust behavior changed after that run.
- Broader workspace, browser, desktop, installer and release checks were skipped: the changed behavior is confined to
  checkout-preparation Git children and a discovery test helper, covered by the selected real-Git preparation and
  discovery tests. No hosted CI run was requested.

## Review gate outcome

A fresh native `gpt-6.1-sol` reviewer at high effort reviewed correctness, design, idiomatic Rust and the full
test-authoring checklist. It confirmed the environment boundaries, child-only injection and bounded scope. Its only
finding was two orphaned description tails in the feedback index; those were removed and the exact corrected index diff
and formatting were checked locally. No source or test defects remained. A fresh native `gpt-6.1-sol` medium reader
found that the commit/PR wording conveys the destructive trigger and why cleanup is confined to Git children, with no
contradiction or convention violation. Actual native model identity and usage counters were not exposed.
