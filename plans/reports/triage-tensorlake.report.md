## What this was about

Tensorlake sandboxes could show misleading cleanup or connection states, make users wait longer than the removal
deadline, or ask an agent's user to approve a launch before Farhelm found a missing local SSH key. This plan also
covered the test runner and regression tests that verify those paths. It accounts for all 30 triage outcomes in seven
PRs. Twenty-six were fixed, three became Plan follow-ups under the approved complexity gate, and one was closed without
a change under the test-tooling scope.

## Things you should know

- A leftover sandbox needs a tag that identifies another helm before Settings calls it “from another helm.” Sandbox
  removal stays within its polling deadline. Farhelm cleans and bounds provider-supplied details before displaying them,
  including when the status request succeeded. An unreadable private key gives a readiness failure.
- The ordinary Add path releases the shared recipe-source lock once the sandbox is running, before SSH readiness. This
  lets other launches using the same recipe source proceed while the sandbox connects. This covers both lock findings.
  Launch progress advances when preparation starts and when the prepared recipe becomes available. Package repair waits
  for a busy dpkg lock, including when a Podman recipe uses the same package script. Agent-requested Tensorlake launches
  check their local SSH key before asking for approval.
- After confirmed Suspend, the host stays Suspended through the first status check and shows Connecting only if it can
  actually dial. A partial Stop and suspend failure says the session stopped so it reads correctly for shells and
  agents. Agent-created disposable sessions still appear in the session list, and only a user's GUI launch can teach
  future environment choices. This also covers fresh managed checkouts.
- Once Farhelm inside a disposable environment has sealed it for deletion, it accepts no new sessions. If destruction
  fails, the host remains marked for deletion with the failure and Retry available, even if its connection returns.
  Retry attempts destruction again. The protocol notes now also explain why both a paused Podman container and a
  suspended Tensorlake sandbox can appear Suspended. Farhelm skips the supervisor dial while the reported state blocks
  it.
- The regression tests now exercise a redirect the client could actually follow and verify the original response is
  kept. They check that confirming setup cannot dial a suspended sandbox, that refusing setup on a sealed sandbox
  neither pauses its connection nor leaves a phantom setup run, and that the helm's host list includes the sandbox's
  environment metadata. Short, uniquely named test directories keep both SSH sockets available for the release test.
- The runner fails if any Rust shard fails or the entire selection is empty. It waits for other shards to finish
  collecting evidence and preserves a clone when the results cannot be retrieved. Use a fresh output directory for a
  rerun. Staging needs the checkout-local Cargo target so the archive and fixtures agree on executable paths. Paths with
  spaces and explicit payload directories ending in whitespace survive staging. A lost create reply can still leave an
  allocation needing manual cleanup. That is a resource-ownership issue before tests run, outside the approved evidence
  and verdict scope.
- The rebase keeps the managed-checkout home fallback alongside the preapproval key check and the launch-history rules.

## Open questions and possible follow-ups

- **Keep all sandbox leftovers visible when Tensorlake discovery fails.** A refresh during cleanup can hide other
  leftovers when discovery fails. The full fix must preserve the last discovery rows while retiring a completed cleanup
  row and showing the discovery error separately. Both the helm reply and the desktop's cleanup state need to carry
  those distinct facts.
- **Keep Tensorlake Suspend and Resume tied to the sandbox Farhelm created.** An earlier failed setup can leave a
  reserved name with no confirmed sandbox ID. Suspend or Resume can then reach someone else's same-name sandbox. The
  follow-up needs to settle the ID durably before sending a mutation, under the same provisioning lock, so a late
  provider reply or local write cannot let a retry act on the wrong sandbox.
- **Show Stop and suspend's continuing work and actual timeout failures.** The desktop can still hide both the reply
  that says the command continues after 20 seconds and a real Stop timeout. The continuing operation needs a progress
  message. The actual timeout needs a failure notice. Distinguishing them requires a typed helm reply, desktop changes
  and tests for both paths. These three items are recorded in TODO.md under Plan follow-up.

## Pull requests

- [#4536](https://github.com/scode/farhelm/pull/4536/changes) fixes the untagged leftovers label. The discovery-failure
  display is deferred.
- [#4544](https://github.com/scode/farhelm/pull/4544/changes) fixes the termination polling bound, status diagnostic
  cleaning and unreadable key. Suspend and Resume ownership is deferred.
- [#4549](https://github.com/scode/farhelm/pull/4549/changes) fixes both recipe-source lock findings, preparation
  progress, dpkg contention and the preapproval key check.
- [#4554](https://github.com/scode/farhelm/pull/4554/changes) fixes the Suspend connection flash, Stop and suspend
  program wording and agent launch history eligibility. The desktop's timeout handling is deferred.
- [#4555](https://github.com/scode/farhelm/pull/4555/changes) corrects the explanation of Retry after a sealed Delete
  fails and the protocol documentation for suspended sandboxes and paused containers.
- [#4559](https://github.com/scode/farhelm/pull/4559/changes) fixes failed-shard exit status, early sibling cleanup,
  failed evidence download, empty selections, the shared Cargo target diagnostic, checkout path quoting and payload path
  whitespace. The allocation-before-ledger finding is closed without a change.
- [#4566](https://github.com/scode/farhelm/pull/4566/changes) strengthens suspended setup, redirect refusal, populated
  host-list metadata, sealed setup and SSH fixture tests.

## Checks

- On the rebased tip `18e695c3`, recorded `cargo nextest run -p farhelm-helm --lib -E '<10 interaction cases>'` passed
  all ten selected tests. This covered the reconciled preapproval path, managed checkouts and history, Stop and suspend,
  and retry after sealed deletion. Run 1ad67ec6-36a3-43ce-95d0-e09438066c3d. Every selected case ran.
- Reused the unchanged behavior coverage below. Each used recorded
  `cargo nextest run -p farhelm-helm --lib -E '<named selection>'` unless otherwise stated. The range comparison matched
  the reviewed code for these paths, and the rebased tests above cover the intersections with upstream source changes.
  - Orphan label and cleanup admission, four passes at the final tag-only candidate on `f2c9c30b`,
    b589c5b0-10fd-49f6-b253-2d13c677f0e8.
  - Polling deadline, status diagnostics and unreadable key, applicable passes from `d2a9e310`,
    0544771d-8c32-47b6-8e26-34f9136fff0d. The removed lifecycle ownership change is outside that evidence.
  - Add and preparation, applicable passes from `8c8cd65d`, 302fa0c1-744a-4169-b360-612899736c4b. Final dpkg and proxy
    cases passed at `961faa7e`, 72429905-d2a2-445c-9066-0feca2516e12.
  - Suspend phase and history, applicable passes from `65bdc1d9` and `c64bf90b`, 4120c4c6-d2f3-4b0d-bc12-cf271cefe6c1
    and 6cd2800b-efbd-42ed-ab39-2d28256966a9.
  - Redirect, suspended setup and populated host-list fixture passed at `11f069f9`,
    ce22e7bd-d0eb-40a9-a6fd-63930d876a40, which included `-p farhelm-desktop` for the shared decoder. The final sealed
    setup and four SSH fixture cases passed at `012a0ecd`, b87969a7-d578-4f4c-bd93-457458fed02e.
  - `deno test -A scripts/tensorlake/sandbox-tests.ts scripts/tensorlake/rust-tests.ts` passed four tests at `da7492ea`,
    e6b87c08-d5c8-44ba-b01e-e4ddf48fe43c. The final local shard verdict and retention case passed with
    `deno test -A --filter 'shard verdicts' scripts/tensorlake/rust-tests.ts` at `b74be4f2`,
    19c060db-e9b5-40d3-a9ea-ebda479bc444. Main and the rebase did not change these scripts.
- On `18e695c3`, `dprint check <changed bookkeeping and tooling docs>`, `git diff --check <main>..HEAD`,
  `python3 scripts/plans-queue.py --repo scode/farhelm check` and `python3 releasing/check-changelog.py format` passed.
  `python3 scripts/check-agents-md.py` reported existing citation warnings and zero errors.
- Reused `deno check scripts/tensorlake/rust-tests.ts` at `b74be4f2` and
  `shellcheck scripts/tensorlake/bootstrap.sh scripts/tensorlake/run-rust-shard.sh` from `dcc4f67`. The reviewed rebase
  preserved their input scripts. Reused the `python -B scripts/check-test-sleeps.py` result at `012a0ecd` for this
  stack's test changes. The rebase's only source reconciliation was a production call signature.
- Skipped real Tensorlake execution and the full test battery. The approved checks use local fixtures and exercise the
  affected paths. The deferred desktop changes never entered the stack. The shipped desktop effects come from the helm's
  status and reply text. This stack changes no drawing code and publishes no golden updates.

## Review gate

All seven PRs passed their required independent Sol reviews and cold reads of the PR text. Opus inspected the early
Settings discovery candidate. That candidate was deferred in full once review established the separate discovery and
cleanup state required to preserve its rows. The rebase preserved the reviewed diff outside bookkeeping and the upstream
change to the shared launch validation call.

### Landing

#### What landed with it

The plan's seven PRs (#4536, #4544, #4549, #4554, #4555, #4559, #4566) landed in this round right after the desktop app
triage plan, with a landing fix, #4571. Both plans were based on the same main, which had gained only queue bookkeeping
since. Their SPEC.md, SPEC_impl.md and channel-type edits are in different places and agree; neither changes what goes
over the wire. They did interact in one place (see "Fixes made while landing").

An independent reviewer read the stack against the triage decisions, the specs and the Tensorlake harness rules, and
found no blocker:

- **Triage outcomes.** All 30 are accounted for: 26 fixed as decided, three deferred to "Plan follow-up" entries in
  TODO.md, and one closed without a change: if Tensorlake's create reply is lost, the test runner can leave a sandbox
  that needs cleaning up by hand, which the reviewer agreed is outside the plan's approved scope.
- **Cross-plan.** The desktop plan keeps leftover containers listed when Refresh fails, in the app alone; this plan
  deferred the Tensorlake version because there the helm itself replaces the list. That is consistent.
- **Docs and tests.** The three agent instruction files about the Tensorlake harness were updated together, SPEC text
  matches the helm's messages, and the new tests check their setup before acting.

#### Fixes made while landing

All in [#4571](https://github.com/scode/farhelm/pull/4571/changes):

- **Two desktop launcher tests broke.** This plan gave a sandbox host in the helm's shared host-list test data an
  environment, to pin that field's format. The desktop launcher's New and Clone tests check what happens on a sandbox
  host with no environment, and use that same host for it. With the environment added, New preselected it and Clone
  opened in the home folder instead of the session's folder. Only test data changed, not product behavior. The workspace
  suite caught it, and both tests pass on the desktop plan alone. Their data now removes that host's environment, as its
  own docs already said. The desktop app's own tests, which read the same data, still pass.
- **The report and a changelog fragment overclaimed.** Both say Stop and suspend's message after a partial failure now
  fits shells. The part about terminal tabs that could not be closed still comes from the supervisor and says "The agent
  stopped", so the report's line is only partly true. The fragment now says so.
- **A harness instruction was wrong.** It said a fresh `--out` merely preserves earlier results. In fact a rerun into a
  used output directory fails every shard, and only after the runner has uploaded about 1 GB of test files, unless
  `--staged` reuses an earlier upload. The line now says that.

#### Checks

This round landed the desktop app triage plan and the Tensorlake triage plan together, stacked in that order on main.
The checks ran on the combined result: the desktop plan changes the GUI library widely and the Tensorlake plan changes
the helm and its Tensorlake test harness, so the landing ran the Rust workspace suite, the lint runs, the end-to-end
smoke test and the full set of GUI reference screenshots. The worker runs were on a separate arm64 Linux test worker;
the rest ran locally on x86_64.

On both plans as delivered, stacked onto main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four nextest slots, no retries and four build jobs, on
  the test worker (recorder run `2a6c5499-13b5-4d21-afb9-5b091042da05`): 8278 of 8280 passed. The two failures were the
  desktop launcher's New and Clone tests broken by the Tensorlake plan's change to shared test data (that plan's "Fixes
  made while landing"). Narrowed locally: both passed on the desktop plan alone (run
  `c9d7f253-749f-44c6-925a-4eac026c4f6a`) and failed with the Tensorlake plan added (run
  `813271e6-59c9-45eb-85a9-abc60d2b35c4`).
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and both desktop app
  clippy runs (`-p farhelm-desktop --all-targets`, with and without `--features agent`), on the test worker, not through
  the recorder: passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `4bb63a13-d440-45fa-b532-fb5a776fa2bf`): 49 of
  its 50 steps passed and the paste step was skipped, as on every Linux run.
- `python3 scripts/check-gui-goldens.py`, every scene, locally (recorder run `dbc90d25-7653-4d3f-ac84-510fd4b8c072`):
  passed, the nine new scenes included.
- The GUI screenshot-batch tests (`python3 -B scripts/test-gui-screenshot-batch.py`), locally: passed.
- The Tensorlake harness's own checks (`deno check` and `deno test -A` on `scripts/tensorlake/rust-tests.ts` and
  `sandbox-tests.ts`, and `shellcheck` on its two shell scripts; no sandbox), locally: passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its isolated environment), `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`, locally: passed.

After the landing fixes (#4570, #4571):

- `cargo nextest run -p farhelm-gui --lib`, on the test worker (recorder run `2d36598c-3456-42f0-8d1a-9a000ee6b2d7`):
  all 3280 passed, the two launcher tests among them. Narrowed first locally at the combined top with the fix (run
  `4eefba0e-c50d-4d8d-b232-35bffc152685`): both passed.
- `cargo nextest run -p farhelm-desktop`, the desktop app's own tests, on the test worker at the Tensorlake plan's top
  with its fix (recorder run `a8d01b92-f47e-4b6f-813e-2decbe6ca87c`): all 184 passed. Its host listing reads the same
  shared test data the Tensorlake plan changed, and the workspace run leaves the desktop app out.
- `cargo clippy --all-targets -- -D warnings`, on the test worker: passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`, locally: passed.

The rest of the first runs still applies: the fixes change only comments, one spec sentence, the launcher tests'
fixture, a changelog fragment and a harness instruction line.

Skipped: real Tensorlake sandbox runs, which spend cloud resources and were not needed for the helm logic the unit and
contract tests cover; macOS, since no Mac was part of this landing; CI, which runs only on demand.

#### Things you should know that the report does not say

None of these is recorded anywhere else yet.

- **A cheap first half of the deferred Suspend and Resume fix.** The deferred outcome is a host row left by a failed
  setup, with no saved sandbox id and no create in progress, where Suspend or Resume then acts on another sandbox with
  the same name, changing the state and billing of a sandbox Farhelm does not own. Refusing when both facts are missing
  is about six lines and a test and closes exactly that case; the reviewer thought it worth doing soon.
- **The supervisor's "The agent stopped" notice** is shown for shells on plain Stop too. It would fit with the desktop
  plan's deferred entry about naming the program a Stop or Delete would end.
- **An over-built rewrite.** The Tensorlake error-message cleaner's ten-line body was rewritten as a longer chain to
  save allocations on small fields. It behaves the same, and restoring the old body would be simpler.
- **Empty test partitions** are only tested against a stand-in for the Tensorlake command line. If real nextest reports
  an empty partition differently from what the runner expects, a real run with more shards than test binaries could
  report a failure where no test failed.
