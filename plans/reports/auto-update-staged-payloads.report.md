# Keep locally staged builds to updates you start

## What this was about

A desktop build from a working copy can use a locally staged directory to update hosts with builds from that same
source. These files can be unsigned. With automatic remote-host updates enabled, Farhelm could install them on an
eligible host without a separate update action.

You chose to keep these builds available for updates you start yourself. Farhelm now skips automatic remote-host updates
whenever it is using that directory. The skip happens before the update probes, even if the saved preference is on.

## Things you should know

Update, Update all and Re-run still use the staged builds. Always update in the attention list still starts the batch
you clicked for and saves the preference. Future automatic updates use that preference when the payload source permits
them. With a staged directory, the helm logs the skip once at startup.

## Open questions and possible follow-ups

None.

## Pull requests

- Skip locally staged builds during automatic remote-host updates.
  [#4358](https://github.com/scode/farhelm/pull/4358/changes)

## Checks

- Linux `cargo nextest run -p farhelm-helm --lib` with
  `-E 'test(provisioning::service::automatic::tests::) | test(/provisioning::tests::.*(automatic_|staged_payloads_require_manual_updates)/)'`
  passed all nine selected tests (recorder `5413c5eb-0fac-483e-acb0-4ccd30b02796`). The new test verified zero automatic
  probes with both an older and a lower-protocol host present. Automatic admission also declined a valid plan, which the
  manual update then used to install the exact staged bytes. The first build with the same selection was interrupted
  before tests ran (recorder `f9c973d9-4668-4474-a204-671358b98fb5`).
- The passing run covered main at `c9cd5e1e` with the plan's changes. The rebase onto `fae1fde9` preserved the plan's
  changes. The intervening host-add refusal fix and plan bookkeeping left the automatic update path untouched.
- `cargo fmt --all -- --check`, `dprint check` on the changed Markdown, `python3 releasing/check-changelog.py format`
  and `python -B scripts/check-test-sleeps.py` passed. The instruction link checker reported one existing triage
  citation warning.
- Workspace, desktop and live SSH checks were skipped. The regression drives the update plan, admission and payload
  installation directly with scripted host connections.

## Review

An independent source review confirmed that the skip happens before automatic planning and admission. It caught wording
that listed too few ways to start an update yourself. The specs and changelog now cover all user-initiated update
actions. The commit wording passed a separate cold read.

### Landing

#### What landed with it

Nothing else landed in this round. Since this plan's work started, main gained the previous round's three plans
(Antigravity workspace trust, launcher model family shortcuts, and template placeholder checks on save) and two TODO
entries. An independent reviewer and I read this plan against them, and none of them touches automatic host updates or
where host builds come from. On the combined main, every automatic path in this plan checks for a staged directory
first: when the app starts, whenever a host's state changes, and when an automatic update is about to start. Updates you
start still install the staged builds: Update, Update all, Re-run, and the updates that clicking **Always update** in
the attention list starts right away.

#### Fixes made while landing

One went in as a second PR, [#4366](https://github.com/scode/farhelm/pull/4366/changes). The instructions agents follow
for a devbuild said you could turn automatic host updates on for a devbuild in Settings. Every devbuild runs with staged
host builds, so after this plan a devbuild never updates hosts automatically, and those instructions now say so. The
report missed this. The same PR adds a context paragraph to the changelog fragment: only a desktop app started with a
locally staged set of host builds is affected, as devbuilds are, and since you chose no release note, the entry can
likely be dropped at release.

#### Checks

The plan changes only the helm's code (automatic remote-host updates skip a locally staged directory of host builds) and
one test, plus SPEC.md, SPEC_impl.md, TODO.md, a review item it settles and a changelog fragment. So the landing ran the
helm crate's whole test suite and the main lint (clippy) on a separate arm64 Linux test machine, and the small checks
locally on x86_64.

On the plan as delivered, stacked on main:

- `cargo nextest run -p farhelm-helm`, on the test machine (recorder run `9147fd19-eaac-41d8-b3f7-662528ddd30b`): all
  1789 tests passed. The two skipped tests need a real Mac host and are always skipped in this run.
- `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: passed. The executor's
  report does not mention running clippy.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format` (formatting, test and changelog lints), locally: passed.

The landing fix (#4366) changes only agent instructions and the changelog fragment: `dprint check`,
`python3 scripts/check-agents-md.py` and `python3 releasing/check-changelog.py format` passed, and the runs above still
apply. Skipped: the workspace suite, since nothing outside the helm crate changed; the desktop app's tests and the
end-to-end smoke test, since no app code changed and no smoke step updates a host automatically; and Mac runs, since no
macOS-specific code changed.

#### Things you should know that the report does not say

In a devbuild, the Settings help for **Update remote hosts automatically** and the hover help on **Always update** still
promise automatic updates, which no longer happen there. The Update buttons stay, so nothing gets stuck. You decided on
no app change while planning, so the landing left that text alone.

#### What you should still do

Nothing. If you want the devbuild help text above to say that a devbuild never updates automatically, that is a small
follow-up.
