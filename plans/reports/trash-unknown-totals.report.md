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

### Landing

Landed as #2402 and #2404, merged bottom-up, with the same content the report describes, rebased onto this round's main
(see the end of these notes). It was the fifth of sixteen plans in this landing round, after launcher-retry-identity,
launcher-composer-drafts, terminal-link-and-copy-fixes and transfer-leftover-cleanup.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the four plans earlier in this round, and documentation and
plan bookkeeping. A fresh-context reviewer and I read them and the other plans of this round against this plan. None of
them changes the trash summary, the trash dialog, or the per-host archive listings they read, and no browser check
expects the old tooltip or accessible-name wording.

The one browser failure in the round's full run was on the Templates button, which sits next to the trash button in the
sessions header. Because this plan changes the trash button, I considered whether it could make the Templates button
move. The twenty repeats of that test ran on the combined stack, which includes this change, and all passed (see
Checks), so I do not attribute the failure to this plan. The only conflicts were in the review feedback queue's index,
where each plan removes its own entries; all removals are kept.

#### Fixes made during the landing

None.

#### Possible follow-ups

- SPEC.md describes the trash button with only two states: an outline can when empty, and an open-lid can with a count
  when it holds checkouts. It does not say what to show when archives exist that Farhelm could not check. This plan
  shows the not-empty can, with a tooltip and accessible name saying some entries or hosts are unchecked, as the report
  describes. Possible follow-up: add that case to SPEC.md, or decide it should look different.

#### Checks

Every plan in this round was executed on a main from before round 107's session-identity change, so none had been tested
against it. That is why this round ran the whole Rust workspace and the full browser suite rather than a selection. All
of the following ran now, on all sixteen plans of this round stacked together on main, including the fixes made while
landing plans in this round:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and its `--self-test`,
  `python3 releasing/notes-review-page.py --self-test`, `scripts/check-desktop-assets.sh`, and the website build
  (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `40f2b98b-b160-4239-a653-44cd0bcdcf96`: 3948 passed.
  `cargo nextest run -p farhelm-ui --features desktop`, run `d288b68b-249f-4f21-9fe0-d4d6d1c4a738`: 717 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `f82f83f8-67c9-4e07-99ed-467c9830dc9f`: 317 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, run
  `81d76a96-912f-47ed-8e1f-57423edbeb32` (kept): 2183 passed and 14 skipped. Of the skips, 9 are the WebKit clipboard
  cases, because Playwright cannot grant WebKit clipboard permissions; 4 run only against a real, signed-in agent; and 1
  is a pixel capture that runs only on Chromium. One test failed: on WebKit, "model-only number key opens New with a
  temporary composition explanation" in `templates.spec.ts` timed out at its first step, waiting 60 seconds for the
  header's Templates button to settle before clicking it. No plan in this round changes that button or that test, and
  the file's other WebKit tests that open the same dialog passed.
  `npx playwright test --project=webkit-templates -g 'model-only number key opens New with a temporary composition explanation' --repeat-each=20`
  then passed all 20 (run `6c382916-77a1-4ecb-b4ee-a1dae8cac848`). The cause is probably machine load during the full
  run, not something this round broke, but that is not confirmed; on one observation it is not recorded as a flake.
- Skipped: the native Mac app (not runnable here) and the installer (unchanged).

The report was written on a main from before round 107's session-identity change, so its statement that no rebase was
needed and its focused checks no longer describe what landed; the combined runs above supersede them. Nothing else in
the executor's report became untrue at landing.
