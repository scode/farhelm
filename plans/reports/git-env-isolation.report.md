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

### Landing

Landed on 2026-10-10 (UTC) as #1798. This round landed five triage plans together, in order: untrusted-text-escaping,
git-env-isolation, os-readback-fixes, harness-tooling-fixes and ssh-config-atomic. Each rebased onto main with only
conflicts in the review queue's index, where each plan removes only its own entries. Since their stacks were based, main
gained this day's earlier landings (sounds, file downloads, the reboot follow-up of the supervisor's timer sweep) and
the 2026-10-10 spec triage; of the files these plans touch, only the helm's supervisor client changed upstream (download
routing), away from the log line one of them changes. A separate reviewer read all five against each other and main by
reading the code only, and checked each against its triage decisions and completion criteria.

#### Review before merging

No findings. The scrub removes exactly git's own list of repository-locating variables apart from the three
configuration ones, which the decision keeps so configuration and credentials still work; it covers every git command
checkout preparation runs, and leaves the hook and the agent their environment. The regression test sets the variable
only in the child it starts, never in the test process.

#### Checks

- Run now, on the first four stacked in landing order: `dist generate --check` (the release workflow matches its
  sources), `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the
  supervisor, helm, UI and protocol unit tests in full through the recorder with pinned tmux 3.7c, four slots and no
  retries (run `1b086587`, 2699 of 2699), and on Chromium and WebKit with one worker and no retries the spawn, header,
  readers and change-feed specs (run `50ffb9fe`, 44 passed; the two skipped are the real-Claude spawn cases).
- Run now, after the landing's fixes, with ssh-config-atomic stacked on top:
  `cargo clippy -p farhelm-helm --all-targets`, `shellcheck` on the provisioning script, and the helm's client tests
  including the new log-escaping test (run `0067a921`, 68 of 68). The new test was also seen to fail with the escaping
  removed, then restored.
- Reused from the executors: their focused runs for each fix, the hosted macOS compile of the argument-reading change,
  and the deflake end-to-end evaluation.

Nothing in the report above was made untrue by the landing.
