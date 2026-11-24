## What this was about

When Farhelm could not read a saved Claude or Codex recipe credential, an update could erase that agent's entry from the
shared launch credentials file on a connected host. Even saving an unrelated GitHub token could trigger the loss.

The plan called for keeping both agents' last delivered launch credentials and naming the failed lookup in the host
warning. The shared file and its delivery records stay intact. Farhelm can still deliver a separate Codex subscription
login or GitHub token.

## Things you should know

- Host details name each failed lookup and include any later delivery error. Saving either agent's shared launch
  credential reports the failed lookup because the replacement could not reach the host. A Claude lookup error during
  host setup is also reported.
- A separate delivery error can still appear in a save reply after an earlier file reached the host. For example, saving
  a Codex subscription login can install the new login and still report a later GitHub lookup error. If the Claude
  launch lookup also failed, host details carry that reason and the subscription login's save reply reports only the
  GitHub error. If the separate Codex login lookup fails while the launch file is held, Farhelm can still deliver a
  valid GitHub token.
- When Farhelm cannot remove an old managed credential from an agent's settings file, it still leaves that agent's new
  launch entry out until cleanup succeeds.

## Open questions and possible follow-ups

No open follow-ups from this plan.

## Pull requests

- Preserve recipe launch credentials when a lookup fails. [#4751](https://github.com/scode/farhelm/pull/4751/changes)

## Checks

- Ran the recorder with `cargo nextest run -p farhelm-helm --lib -E 'test(credential_delivery::tests::)'`. All 33
  selected tests passed on `e9aa4fe2` (run `5e0ac01a-d6ad-4787-bb41-86f1f5553786`), with no runtime skips. The selection
  covers both saved launch credentials, setup, separate login and GitHub delivery, warning routing, existing cleanup
  behavior, and delivery on active connections.
- Ran `cargo clippy --locked -p farhelm-helm --all-targets -- -D warnings` on `e9aa4fe2`. It passed. Reused that check
  and the 33-test pass after rebasing over #4747, which changed the desktop update pill. The helm code, fixtures and
  dependencies tested here kept their contents.
- Passed `cargo fmt --all -- --check`,
  `dprint check SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/codex-recipe-launch-credentials.md`,
  `python3 releasing/check-changelog.py format`, `git diff --check 5b1cfcdb 21099ef2`, and
  `python -B scripts/check-test-sleeps.py` using the isolated interpreter on the rebased working revision `21099ef2`.
  The draft has the same tree.
- The earlier nextest selections passed 32 tests each at `326a6e12` (run `f92292f8-ac44-49b4-8bc6-3cc84afaf3d5`) and
  `b3dd33e6` (run `cb06b6dd-1ab7-4c54-b925-2e72eb7ffe6c`). Repeated the selection after the save reply changed.
- Skipped the full workspace suite and desktop captures. The helm selection exercises the changed delivery and warning
  contracts, and the desktop uses the existing warning presentation.

## Review gate

The independent Sol review found that a later delivery error could hide the original lookup warning. The follow-up
review found that sending the combined warning to every save reply would give an unrelated login save a misleading
lookup reason. A scope review confirmed that host details can show both reasons while saves keep any existing delivery
error and add only lookup warnings for the credentials they use. The final Sol re-review found both issues resolved. An
independent reader checked the commit wording.

### Landing

#### What landed with it

This was the only plan landed this time: its PR ([#4751](https://github.com/scode/farhelm/pull/4751/changes)) plus one
landing fix, [#4754](https://github.com/scode/farhelm/pull/4754/changes). While the plan was being built, main gained
only the desktop sidebar's update-pill colors (light-mode-upgrade-pill-pulse) and plan bookkeeping, neither of which
touches how the helm delivers credentials, so nothing interacted with this plan.

#### Fixes made while landing

#4754 changes no behavior: it touches only comments, SPEC_impl.md and the changelog fragment. Some terms below: the
launch credentials file is the one file on a host that holds both Claude's and Codex's launch credentials; the Codex
subscription login is a separate file; a delivery is one run of the helm bringing a host's credential files up to date.

- **A caveat the report and changelog left out.** A connected host keeps the recipe's credential selection from when it
  connected until it reconnects. Farhelm refuses to delete a credential a recipe still selects, so a user deletes one by
  first deselecting it in the recipe; on a host that stayed connected, the next delivery then still looks it up and
  fails. Under this plan that keeps the whole launch credentials file as it was. Until the host reconnects, the deleted
  credential stays on the host (before the plan, the next delivery of any kind took it off), and saving a replacement
  for the other agent's launch credential reports that delivery failed, because the new value cannot reach the host
  either. The changelog fragment now says this. It follows from keeping the file intact on any failed lookup, which the
  TODO entry behind the plan asked for, so I did not change it; a possible follow-up is below.
- **SPEC_impl.md corrections.** It said a failed Codex subscription login lookup lets the GitHub token still deliver.
  That is true only when a launch credential lookup already failed in the same delivery; otherwise a failed Codex
  subscription login lookup still stops the delivery before the GitHub token, as before the plan. It also called a
  failed Claude lookup during host setup a setup error, while host setup only shows it as a notice on a step that still
  completes. Both now say what happens. Comments in the delivery code and its tests now explain the warnings and why the
  tests matter.

#### Checks

The plan changes how the helm delivers recipe credentials to connected hosts, so the landing ran every test of the helm
crate, clippy with warnings as errors over the workspace and over the app binary, and the formatting checks
(`cargo fmt`, `dprint`), the test-sleep check and the changelog format check. Worker runs were on a separate arm64 Linux
test worker; the rest ran locally on x86_64 Linux. One test run failed, as described below; no other check failed.

On the plan's PR rebased onto main, before the landing fix:

- `cargo nextest run -p farhelm-helm`, on the test worker (recorder run `f2abe974-d3aa-4c85-b5e7-855b3e63ac7e`): 1885
  passed and one failed,
  `provisioning::service::tensorlake_add::tests::codex_subscription_and_none_enter_setup_without_an_endpoint`, which
  fails now and then on main as well (see below). A second run on the same tree (`fefd26ea-338b-4d0e-a308-90c6aba9b9e2`)
  passed all 1886.
- That test alone, on the test worker: on the plan's tree, 1 failure in 5 runs (`39c5a440-f8f6-417e-bf9e-a50df9b6a95d`)
  and 1 in 20 (`52a1b811-c73a-45b3-860f-d0d0449eb38c`); on main's tree, none in 10
  (`1c85fb63-f210-471e-8d90-ed63a46ae941`) and 2 in 20 (`975c7642-2c13-4786-81c5-a15bde4a0d07`). Locally on the plan's
  tree it passed three times (`e3521cb9-d0e7-48c3-85c0-9070881679c1`, `0629a246-84b2-4d21-869f-41483896a501`,
  `a393e82f-aad0-4749-a345-e56d6fa57473`). The failed runs are kept.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix (#4754), which changes only comments, SPEC_impl.md and the changelog fragment:
`cargo clippy -p farhelm-helm --all-targets -- -D warnings` on the test worker, and the formatter, sleep check,
`dprint check` and changelog format check locally: passed. No tests were rerun, since nothing they run changed.

Skipped: the real Podman environment suite, which delivers credentials into a real container end to end, since the plan
keeps delivery's success path (which files it writes and records, in what order) unchanged and the helm's own tests
cover the new failure paths with a stand-in host; the rest of the workspace suite, since the change is inside the helm's
delivery code; macOS, since nothing platform-specific changed and no Mac was used; and CI, which runs only on demand.

#### Things you should know that the report does not say

- **A test that fails now and then, already on main.** The worker run of the whole helm crate failed one test outside
  the 33 credential delivery tests the executor ran: the test of adding a Tensorlake host whose recipe uses a Codex
  subscription or no Codex credentials. It reads the host's delivery record after setup and expects no Codex settings
  cleanup to be recorded, but the delivery that runs once the new host connects records one. From the code and the
  failed records, the test fails when that delivery finishes before it reads the record; that cause is a hypothesis. On
  the test worker, run alone, it failed 2 of 25 times on the plan's tree and 2 of 30 times on main's tree, so it
  predates the plan and the plan does not change it. [#4757](https://github.com/scode/farhelm/pull/4757/changes) is a
  draft PR for you to merge that records it in FLAKES.md, adds its Deflake entry to TODO.md and keeps the deflake sweep
  from running it. The test itself still needs a fix; I recommend asserting what setup must not do (read or rewrite the
  agent settings, which the test's stand-in backend records) instead of the stamp a later delivery writes, as the
  Deflake entry also says.
- Setup now reports a failed Claude lookup even when the host already had a delivered launch file; before, it returned
  quietly. Host setup shows it only as a notice on its credentials step, so this is harmless.
- When the Codex subscription login cannot be read but both launch credentials can, the GitHub token does not reach the
  host in that delivery, as before the plan; only when a launch credential lookup also failed does the GitHub token
  still go through. The plan did not ask for that difference, and no test covers the first case. The second follow-up
  below would settle it.

#### Possible follow-ups

- **Tell a deleted credential apart from a transient read error.** On a connected host, the deselect-then-delete case
  above is the one ordinary way for a lookup to fail; the other is an error reading the helm's database. Treating "this
  credential no longer exists" as "this agent no longer has launch credentials" would take the deleted credential off
  the host at once and stop blocking the other agent's rotations. At minimum, the warning could say that reconnecting
  the host clears it. I recommend the first, since keeping a deleted credential on a host is the opposite of what a user
  who deleted it expects.
- **Make a failed Codex subscription login lookup behave the same either way.** Never letting it stop the delivery would
  mean an unreadable Codex login no longer keeps a new GitHub token off the host; always stopping would keep the older
  behavior in both cases. I recommend never stopping, with a test for it, since that matches what the plan was for: one
  unreadable credential should not hold back unrelated ones.
