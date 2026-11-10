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

### Landing

Landed alone (the only plan in this landing round), as #3296. Nothing reached main between the plan's base and its merge
apart from planning bookkeeping, so nothing could interact with it. The landing made no changes.

#### Review

The fix removes Git's own lock files from a host's repository cache, so the landing's fresh-context review attacked the
claim that makes that safe: that no Git process can still be running and own such a lock when Farhelm removes it. It
holds. Every way Farhelm runs Git in the cache does so while holding Farhelm's own lock on that cache, and that Git
process ends with whatever holds Farhelm's lock. A visible launch's Git runs in the session's own terminal process
group, which is hung up when its owner dies, and Stop and Delete signal the whole tree. The background ready-checkout
builder and cache cleanup run in their own session, whose process group is killed outright. Automatic Git housekeeping
runs in the foreground, so no Git process detaches. Every release since v0.26.0 uses the same lock and the same
foreground housekeeping, so an older Farhelm running alongside after an update cannot be using the cache unseen either.

#### Things you should know

From the review; none of it affects users in normal use. The report says no follow-ups are required; the landing adds
two possible ones, in the first two items below.

- Before this change, Git's lock file was a backstop: had a Git process somehow outlived Farhelm's lock, the next fetch
  would have failed cleanly instead of racing it. Now Farhelm deletes that file. One manual action defeats the
  guarantee: killing Farhelm's processes by hand with SIGKILL during background cache cleanup can leave its `git`
  running with nothing to stop it, and a later launch could then race it on the cache's branch list. Nothing in Farhelm
  does this. Possible follow-up, recommended but not urgent: have the kernel end cache Git processes when their parent
  dies, which is cheap hardening on Linux.
- SPEC_impl.md says cache Git processes "end with their preparation job", and the cache code's comment says they die
  with it. Strictly, Git killed with an ordinary termination signal can still run its own cleanup just after Farhelm's
  lock is released. The worst result found is that Git's final step of a ref update fails, so that launch's checkout
  fails and a retry succeeds; the cache is not damaged. Possible follow-up, optional: soften that wording, which the
  hardening above would also make true.
- A cache path that is a plain file instead of a directory now fails a checkout with the misleading "could not clear
  cache Git locks: Not a directory" rather than Git's own error. The changed test expects the new message. No working
  cache is affected.
- The repair covers Git's default way of storing branches. A cache using Git's newer "reftable" storage, or with
  per-branch history logs turned on, can leave lock files the repair does not remove. Farhelm turns on neither, but
  cache Git runs with the user's own Git settings, so a global setting could.
- The test proves the fix: it confirms a planted `packed-refs.lock` blocks pruning a deleted branch, then shows the next
  checkout succeeds and prunes it. It covers that one lock on a launch's checkout clone, not the walk through `refs/`,
  the refusal to follow a symlinked `refs/`, or the repair on the background cleanup path.

#### Checks

Run now on this development machine, on the tree that merged:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`
  passed.
- Recorded
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^repo_cache::/) | test(/^launch::/) | test(/spare_build/)'`,
  with the pinned tmux: 104 passed (run `e536c9cb-7e49-496f-840b-971921692f66`). This is wider than the executor's 13
  cache tests: every checkout clone and the cache maintenance job now run the repair first, and these are the
  supervisor's tests of both, including the real-Git checkout tests.
- Recorded `cargo nextest run -p farhelm --test e2e -E 'test(/^github_checkouts::/)'`: 6 passed (run
  `23cc3be3-606f-4169-9a81-426ee44dd5ef`), a real supervisor making checkouts.
- Recorded `cargo nextest run -p farhelm -p farhelm-supervisor --lib --test spare_builder -E <filter>` with the filter
  `binary(spare_builder) | test(/^service::spare_builder::/) | test(/^spare_build::/)`: 25 passed (run
  `45c326c5-4e63-40bd-b4e9-beb508f8133a`). These run the real background builder, including its cache-only cleanup and
  the case where its owner is killed with SIGKILL, which the repair's safety argument relies on.

All three ran with the pinned tmux first on `PATH`, and none printed a runtime `SKIPPED` line.

Reused from the executor: nothing; the runs above include its 13 cache tests.

Skipped: browser specs (no UI change), the rest of the workspace's Rust tests (only the supervisor's cache preparation
changed, and the selections above cover its callers), and hosted CI (manual-only, not dispatched).
