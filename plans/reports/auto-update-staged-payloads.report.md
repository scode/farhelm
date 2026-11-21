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
