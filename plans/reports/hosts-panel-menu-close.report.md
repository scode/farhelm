## What this was about

When a host setup failed with its progress already expanded, the new failure text could make the hosts panel taller. An
open host or session menu below it stayed at its old position, beside a different row, while its actions still applied
to the original target. The maintainer chose to close sidebar menus whenever the hosts panel changes height.

The existing menu-close trigger now observes the hosts panel's rendered size as well as its logical state. An expanded
setup failure therefore closes the menu rather than leaving it behind. The old comment accepting this gap was corrected;
the existing specification already requires this behavior and needed no change.

## Things you should know

Any host-panel size change closes an open host or session menu and the session notification popup. This includes growth
below an open host menu, an accepted side effect of observing the whole panel. Opening a menu on a stable panel still
works normally. The initial resize callback needed no special filtering in the tested browser paths.

The change is 63 added or removed lines of code and tests, below the agreed 150-line gate. The outcome ledger records
that this size gate replaced the earlier time estimate. The feedback item and its index entry are removed, and a fixed
changelog fragment is included.

## Open questions and possible follow-ups

None required for this plan. This change does not add a host-row reorder guard or revisit the separately documented
session-row layout cases; it covers the hosts panel changing size.

## The PRs

- [Draft #2379](https://github.com/scode/farhelm/pull/2379/changes): close menus when host rows resize.

## Checks run, reused and skipped

Run during execution:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo check -p farhelm-ui --features desktop`: passed. The CLI and release web bundle were built for browser tests.
- `npx playwright test 'provisioning\.spec\.ts|sidebar\.spec\.ts' -g 'expanded setup failure closes|setup disclosure transitions|two provisioning-busy rows|a menu dismissed by a layout change|opening host details closes|opening a menu does not move|resizing the viewport closes|listing refresh with nothing layout-relevant'`
  ran through the recorder with pinned tmux, one worker and no retries: 18 passed, nine each on Chromium and WebKit,
  with no skipped or flaky cases. The selection covers the new expanded-setup failure, existing setup disclosure,
  busy-host reconciliation, menu focus restoration, stable menu opening, host-details changes, irrelevant listing
  refreshes and viewport resizing. Run `b39bec8b-a35e-4e0d-b728-2069e33fe2af`, source fingerprint
  `65609de1b0ede957e8eddedb80baf3c2fafbe0a949f7892ff1dffd355967f0ec`; the complete output contains no runtime `SKIPPED`
  marker.
- A negative control disabled only observation of the hosts panel in the new Chromium test. It failed at the intended
  menu-removal assertion after proving the panel grew and the sidebar did not scroll: the menu remained open. Run
  `25208d60-5041-40de-b24f-e4fa1e1f632f`. That temporary test modification was removed before the final passing run. An
  earlier attempt selected no tests because its title filter was too strict, run `bc551f76-2adf-4b9a-9c3a-09e6045f101d`;
  it supplies no behavior evidence. Both failed attempts remain retained.
- The isolated `python -B scripts/check-test-sleeps.py`: passed, 304 delays inspected, none missing a reason.
  `python3 scripts/check-agents-md.py`, targeted `dprint check`, and `python3 releasing/check-changelog.py format`:
  passed.

The checks still cover the change after its rebase from main `18e51c3e` onto `fdae1c36`: the entire intervening diff
contains only plan queue and report bookkeeping. The source and regression diff is identical to tested revision
`acfddc78b8a562a6471e4455a97d9ec74650abca`. The later outcome-ledger URL update changes no executable behavior. No older
executor's runtime evidence was reused.

Skipped the full Rust and browser batteries, JavaScript tests, installer checks and desktop runtime smoke: this change
adds a rendered-size event to existing menu dismissal, covered directly by the selected tests on both browser engines
and by desktop compilation. It changes no JavaScript, installer, session lifecycle or desktop shell behavior.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues. Its test review prompted checking
that sidebar scrolling could not explain the menu closing; the final regression asserts unchanged scroll position, and
the negative control confirms the new panel observer is necessary. The reviewer inspected the strengthened fixture. The
commit title and body passed a separate fresh cold read. A separate documentation pass covered both source files, the
regression, outcome ledger, feedback index and changelog fragment.

### Landing

Landed as #2379, unchanged from what the report describes. It was the third of four plans in this landing round, after
host-entry-session-ids and helm-lock-outlives-work and before template-list-current-labels.

#### What landed since the stack was based, and whether it interacted

Since the PR's base, main gained host-entry-session-ids in this round (sessions identified by their host entry),
helm-lock-outlives-work (the helm keeps its state-folder lock through shutdown), and documentation and plan bookkeeping.
A fresh-context reviewer and I read them against this plan: host-entry-session-ids changes no host-panel code, host IDs
in the UI stay the same numbers, and this plan's new browser test uses host IDs only; the helm change is not in the UI.
template-list-current-labels, landing next, changes only the Cmd-J list. The only conflicts were in the review feedback
queue's index, where each plan removes its own entries; all removals are kept. The landed code is identical to what the
landing's checks below tested.

#### Fixes made during the landing

None.

#### Possible follow-ups

- As the report says, any change in the hosts panel's height now closes an open host or session menu. Browser tests that
  open a session row's menu without first waiting for the host rows to settle could become flaky because of this. None
  failed for this reason in the full browser run here.

#### Checks

Run now, on the four plans of this round stacked together on main:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, `dprint check` on every changed file, and
  `python3 releasing/check-changelog.py format`: all passed.
- The whole Rust workspace through the test recorder with pinned nextest and tmux, four slots and no retries, run
  `7067248e-06f2-4bd7-90ff-63250d8fff42`: every UI test passed (its four failures were host-entry-session-ids' outdated
  helm tests, fixed in that plan; the whole helm library then passed in run `cbf11585-1556-4055-ba01-7041b35a4f28`). The
  UI library built with desktop support: 714 passed, run `1560df43-83f8-4586-87cf-c71d56f481fc`. `node --test` in the
  UI's JavaScript test directory: 311 passed, run `5f9b15b4-5549-46ed-8f7a-3528415256fa`.
- The full browser suite (`npx playwright test`) on Chromium and WebKit, one worker, no retries, run
  `8214f916-6dda-4c7a-9d91-35ed24e150e8`: the host provisioning file with this plan's new test passed, and none of the
  run's failures involved menus closing: all were host-entry-session-ids' tests that still expected the old session IDs,
  fixed and rerun there.
- Skipped: the native Mac app (not runnable here).

Nothing in the executor's report became untrue at landing.
