## What this was about

Interrupted background maintenance could leave a Git lock in a host's repository cache. Later launches needing to prune
an upstream reference could then fail repeatedly while cloning that repository, until someone repaired the cache or it
expired. Completed checkouts were independent and unaffected. The maintainer chose automatic removal of leftover Git
locks while Farhelm holds its existing exclusive cache lock.

## Things you should know

Both ordinary checkout preparation and background cache maintenance now remove leftover packed-reference, HEAD,
configuration, shallow-history and per-reference lock files before running Git. The permanent Farhelm cache lock stays
in place, so competing preparations remain serialized. Genuine Git and filesystem failures still fail visibly; there is
no fallback clone or discarded cache. The code, regression and specification changes total 88 added/deleted lines,
within the plan's 100-line limit.

The approved outcome moved to the triage archive with its PR link, and its feedback file and index entry were removed in
the same PR. A release-note fragment describes the checkout fix. No live installation or cloud resource was used.

## Open questions and possible follow-ups

None required for this plan. The regression plants the leftover lock directly; it does not claim to have interrupted a
real maintenance process at the instant Git created that lock.

## The PRs

- [#3296](https://github.com/scode/farhelm/pull/3296/changes): unblock checkouts after interrupted cache cleanup; open
  draft based on main, head `ce8e621a0a9608d3ec52657ec672e444e92a9401`.

## Checks run, reused and skipped

- Run `522eecca-a133-4fcb-a68b-fad4e9467920`: through the recorder's controlled nextest mode, with pinned nextest
  0.9.148 and pinned tmux, four slots and zero retries:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^repo_cache::/) | test(/^launch::tests::.*cache/) | test(=launch::tests::d2_real_git_offline_clones_the_instead_of_fixture)'`.
  All 13 selected tests passed, including cache admission, serialization, real Git cloning and maintenance, retained
  failures, and cache eviction. The other 1,810 cases were selected out. The source was main `6df05030` plus the working
  diff, fingerprint `3a30cc03816ea3cef632706075d8824fbf2f3246c75aaae124eb7d59ce2c6c7f`.
- The regression was then aligned literally with the ledger's upstream-branch deletion criterion, preserving its
  existing tag coverage. Run `da2e1719-e389-486c-8475-26a8715a8b4c`, using the same recorder policy and substrate,
  selected only `test(=launch::tests::d2_real_git_offline_clones_the_instead_of_fixture)`: one passed, 1,822 selected
  out. Its source was main `6df05030` plus the final working diff, fingerprint
  `21fbce4b6eb9a84b6e55ef3f1fbdd666c348a2e2fe16b047fea125123ef0d718`. It establishes a packed cached branch, deletes it
  upstream, proves the planted lock blocks pruning, then checks that an ordinary checkout succeeds and prunes that
  branch. The first run's other 12 cases are reused: their code and production inputs did not change. Both retained
  records have complete source and runner reports, output EOF, no omitted/truncated output and no forced cleanup; no
  runtime substrate skip was printed. Later changes only filled in the archive's PR URL.
- Passed `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the isolated
  interpreter's `python -B scripts/check-test-sleeps.py` (429 delays, zero missing rationales), dprint on the changed
  Markdown, and `python3 releasing/check-changelog.py format`. An initial archive line-wrap formatting refusal was
  corrected and rechecked. `python3 scripts/check-agents-md.py` passed with zero errors and one existing archived
  heading-reference warning.
- Skipped browser, full-workspace, systemd and cloud tests: this changes offline cache Git preparation, covered by the
  focused cache tests. It changes no UI, protocol, provider or process-containment behavior. Main was unchanged at the
  final fetch, so no rebase was needed.

## Review gate outcome

The required single fresh-context GPT-6.1 Sol reviewer at high effort passed the implementation and then the final
branch-deletion adjustment, including the specification, test contract, changelog and outcome graduation. No findings
were rejected. A separate fresh-context commit-message reader passed the title and explanation.
