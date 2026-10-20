# Honest trash summaries

## What this was about

The sidebar could call the trash empty when it contained archived folders Farhelm could not inspect. Separately, the
trash dialog could present an incomplete measured size as a complete total when a host held more than 10,000 archives.
The maintainer approved both fixes with size limits: 100 changed lines for the sidebar and about 15 non-test lines for
the total, discarding either outcome if its fix exceeded its limit.

Both fixes fit: the sidebar change is 77 added/deleted code, test and documentation lines; the total change is 14
non-test lines, plus 46 test lines excluded from that gate.

## Things you should know

Unchecked archives now keep the sidebar's can filled and its tooltip and accessible name qualified. For example, an
unchecked-only listing says "0 known archived checkouts, some entries or hosts unchecked". The badge still counts
inspected checkouts; inspection failures do not invent an exact checkout count.

The size total now follows all successful archive listings, including those too large to select for deletion. Known
bytes remain visible with the partial qualifier when any size is unknown. A fully measured large listing remains a
complete size. The 10,000-ID cleanup limit and deletion safeguards are unchanged.

## Open questions and possible follow-ups

None required for these outcomes.

## The PRs

- [#2402](https://github.com/scode/farhelm/pull/2402/changes): preserve uncertainty in the sidebar's trash description
  and icon.
- [#2404](https://github.com/scode/farhelm/pull/2404/changes): keep unknown sizes marked partial above the cleanup batch
  limit; based on #2402.

## Checks run, reused and skipped

- Recorded `cargo nextest run -p farhelm-ui --lib -E 'test(list::trash::tests::)'` passed four tests for #2402, run
  `315b0ccc-a179-425b-8f53-aae3eb587e07`, and five at the combined tip, run `ddcd8cc5-7bb3-4eb1-8bfd-fade40796781`. Both
  used pinned nextest and tmux, four slots and zero retries. The 545 other tests were filtered out in each run; there
  were no runtime substrate skips or failed attempts.
- The units cover unchecked-only and mixed listings, duplicate inspection diagnostics, known and unknown sizes, settled
  and pending empty listings, and a successful listing of 10,001 unique IDs. The latter is below the control reply size
  ceiling, cannot be selected for cleanup, retains its measured bytes as partial, and becomes complete when its last
  unknown size is supplied.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, changed Markdown dprint checks,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and the isolated test-sleep
  checker passed for both outcomes. The sleep checker found 304 delays and no missing rationales.
- Checks are reused after commit creation and the ledger's PR-link amendments because executable code and tests are
  unchanged: the tested working revisions were `a02328b9` and `71e7d2a4`, and the final heads are `680d8e6b` (#2402) and
  `faa847bc` (#2404). New main changes from base `dca5f578` through `1e95a12c` were read in full: only other plans'
  claims and a delivery report changed. No product, specification or governing instruction interaction required a rebase
  or another runtime run.
- Browser, desktop, installer, provisioning and broader Rust suites were skipped. These changes alter pure summary
  decisions and existing text/icon wiring, covered by the focused units, without changing event handling, layout,
  lifecycle or deletion authority. CI was not dispatched; it does not run automatically on PRs.

## The review gate's outcome

Each PR received exactly one fresh-context gpt-6.1-sol review at high effort with the full acceptance criteria and
test-authoring instructions. Both found no issues and independently confirmed their size gates. Fresh cold readers
checked commit and PR wording; the sidebar message was rewritten to remove ambiguous jargon. No findings remain.
