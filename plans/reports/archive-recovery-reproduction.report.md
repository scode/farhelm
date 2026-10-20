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

### Landing

Landed as #2374 in its own landing round, unchanged from what the report describes. This was the plan's second goal: it
was first a reproduction of the race, which the maintainer replaced on 2026-10-20 with this direct fix after the
reproduction run was stopped.

#### What landed since the stack was based, and whether it interacted

Since the PR's base, main gained only documentation and plan bookkeeping: a triage record and new TODO entries before
the landing started, and, while it ran, a planning change for five other triage outcomes and a change to how planning
pages are served. None of them touches the supervisor. The two supervisor changes that landed shortly before this plan
was based, workspace-trust-destination (a checkout's identity is checked before it is trusted) and
launch-enters-working-dir (sessions enter their folder), were already in its base; a fresh-context reviewer confirmed
that neither writes a checkout's recorded trash location, so neither interacts with the new guard. The only overlap was
in the triage record, where the planning change edited other outcomes' entries. The landed code is identical to what the
landing's checks below tested.

The reviewer also confirmed that every place that writes a checkout's recorded trash location is now guarded, that a
legitimate Delete or recovery cannot be refused by mistake (within one supervisor each write compares against the value
it just read or wrote), and that the tests run one step at a time, with no racing supervisors, cancelled startups or
pauses.

#### Fixes made during the landing

None.

#### Things the landing found

One behavior change to know about, needing no action: when a move into the trash is refused and its record is put back,
and that put-back is itself refused because a newer attempt changed the record, Delete now reports that the checkout
record changed state instead of "rename refused". Delete and startup recovery both handle this correctly, and the
message Delete shows names the newer trash location.

#### Possible follow-ups

- The guard compares the recorded location's value, so one narrower schedule remains open, as the code and the report
  already note: if both attempts recorded the same location, and the late attempt's rename then failed with "already
  exists" rather than "missing source", it would record a fresh location again, and the checkout's files would again
  stay on disk but drop out of the trash list. On local disks the "missing source" error wins, so this does not happen
  there; network or unusual filesystems were not checked. Checking that the folder being moved still exists before
  recording a new location would close it.

#### Checks

Run now, on the PR rebased onto main:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the isolated test-sleep check, `dprint check` on the changed
  Markdown, and `python3 releasing/check-changelog.py format`: all passed.
- Through the test recorder with pinned nextest and tmux, four slots and no retries: the whole protocol, helm and
  supervisor libraries, the `farhelm` binary's tests, and the end-to-end tests for managed checkouts, session lifecycle
  and structured launches
  (`cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm --lib --bins --test e2e -E 'kind(lib) | kind(bin) | (binary(e2e) & (test(github_checkouts::) | test(session_lifecycle::) | test(structured_launches::)))'`):
  2840 passed, run `3901ca21-19aa-46e6-a59f-79c9a3355feb`. The executor had run a narrower selection (75 tests covering
  checkout records, the trash, collision retries, managed-checkout Delete and startup archive recovery).
- Skipped: JavaScript, browser and desktop tests, since the change is confined to the supervisor's checkout records and
  touches no UI.

Nothing in the executor's report became untrue at landing.
