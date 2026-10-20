# Preserve the recorded location of a trashed checkout

## What this was about

Deleting the last session that uses a managed checkout moves its folder to Farhelm's trash. After an interrupted move,
startup recovery finishes it. An older recovery attempt could overwrite a different trash location recorded by a newer
attempt, leaving the checkout's files intact on disk but absent from Farhelm's trash list.

The maintainer initially requested a reproduction, then replaced that request with a direct fix: refuse a destination
write based on an older value, within a small change confined to the supervisor's checkout records. The earlier
reproduction work was not resumed, reused or run for this fix.

## Things you should know

An attempt now checks that the recorded trash destination still matches the value it read before replacing or clearing
it. A changed destination causes the existing wrong-state error before that attempt tries another rename. Recording the
first trash destination also requires an empty destination. The newer destination and its files remain untouched by that
refusal. Ordinary collision retries keep working.

The fix adds about 50 lines of production code and about 180 lines of tests, within the agreed limits. It adds no
database migration or new background work. The existing specification already describes the intended behavior and needed
no change. The feedback item is removed with its index entry, and the triage ledger records the fix.

## Open questions and possible follow-ups

No decision is needed to land this fix. The cause that lets archive recovery outlive a cancelled startup remains outside
the agreed scope. A destination comparison detects a changed value; it does not detect intervening work that leaves that
value unchanged. This plan makes no claim about the natural frequency of the original race.

## The PRs

- [Draft PR #2374](https://github.com/scode/farhelm/pull/2374/changes): preserve newer checkout trash locations.

## Checks run, reused and skipped

- Recorded supervisor tests: 75 selected cases passed with pinned nextest and tmux, covering checkout records, trash,
  collision retries, managed-checkout Delete and startup archive recovery. Recorder run
  `426ca02a-29a6-4127-9b20-05faebd09a57`; complete report and output, no runtime skip markers. The production code has
  not changed since this run. After review strengthened the test that only a pending checkout record may be changed, the
  three guard regressions were rerun: all three passed, recorder run `4a364afa-c629-462e-997d-93c30ad56c36`. Both runs
  cover main `22af69e7` plus this change; the intervening main changes added only unrelated TODO entries, so their
  coverage remains valid after rebase. The exact tested working trees are identified by source fingerprints
  `ce11b7cf3d31c6906c20006d19d1332ba6f6aebe17a5faf7bc656c7367b6a3ea` and
  `d6f9412590592a3788961640d6a277d85127f32d20b52f2d3e18372adf423383`, respectively. The final PR revision is
  `3d416fc157cc1945056434233e3067547b3f0d5c`; its differences from the second tested tree are explanatory comments, the
  ledger's PR link, and the unrelated main changes just described.
- `cargo clippy -p farhelm-supervisor --lib --tests -- -D warnings` passed after the corrections. Rust formatting,
  changed-Markdown formatting, changelog format and the instruction-link check passed. The isolated test-delay checker
  inspected 304 delays with no missing rationale. The final edits after these checks only clarify comments and add the
  PR link to the ledger; formatting was checked again for those files.
- No full workspace, browser, desktop, installer or provisioning suite ran: the changed behavior is inside the
  supervisor's checkout registry and the targeted tests exercise its callers. No competing-supervisor reproduction ran.
  No runtime evidence from an earlier executor was reused.

The two portable test commands below ran through `scripts/record-test-run.py` with
`--runner nextest --kind development --tmux required`, four slots and zero retries, after preparing the pinned nextest
and tmux on PATH:

```console
cargo nextest run -p farhelm-supervisor --lib -E 'test(working_copies::) | test(checkout_trash::) | (test(service::teardown::tests::) & (test(archive) | test(last_reference))) | test(a_supervisor_without_the_state_dir_claim_reconciles_nothing_durably)'
cargo nextest run -p farhelm-supervisor --lib -E 'test(stale_archive_destination_is_refused_before_any_rename) | test(stale_rename_results_preserve_the_newer_archive_journal) | test(journal_writes_require_both_the_expected_state_and_destination)'
```

## The review gate's outcome

The required single Opus 5.5 high reviewer checked the full change and its complexity limit. The test that only pending
checkout records may be changed was strengthened, fixture premises made explicit, and release/ledger wording clarified.
The reviewer also raised a same-destination case; the executor retained the agreed value-comparison scope and documented
its unchanged-value limit instead of adding another filesystem observation. The reviewer accepted that scope, withdrew
the finding as a requirement violation, and concluded that the acceptance criteria are met with no blocking findings.
The two remaining documentation nits were corrected. The commit/PR text passed a fresh cold read after one rewrite.
