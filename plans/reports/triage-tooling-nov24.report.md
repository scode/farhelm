## What this was about

November 24 triage assigned 67 findings in maintainer tooling. The rule was to fix crashes and evidence failures, and to
close robustness findings that did not meet that rule. The six PRs fix 34 findings, archive 23 without a code change,
close two that main had already fixed, and record eight findings in seven follow-up entries. The follow-ups need broader
changes to the evidence format, restart witness, capture publication, or process cleanup.

## Things you should know

The installer fallback follows HTTPS redirects only. Drift checks and test workers keep results tied to the actual run,
even after shell startup, cancellation or an interrupted transfer. The brick test keeps evidence from earlier successful
checks when a later check fails or the requested scope narrows. Its announcement check waits up to 60 seconds for the
bell action after the signed feed is available. An app launched with the test feed clears it when starting its next
instance. A missing signed fixture still fails the publication gate, and the reason identifies the missing fixture.

Local checks exercised both reopen commands and the Linux toolbar captures. Native macOS reopen and the taller Mac
toolbar crop still need verification on a Mac. A live Mac VM brick test remains pending. The portable brick tests
exercised the announcement wait, retry, and evidence decisions with stand-ins.

The launcher comparison now checks the current Host menu, field and template controls. Related guarded Replace and
custom-agent screenshots were fixed together in the third draft. The docs Host menu capture starts with compact rows
after the preceding Details picture. These edits overlap with the launcher, custom-agent, Podman, and sessions UI plans
only where they exercise or photograph those features. The updater process follow-up overlaps with the hooks and trust
plan's live-agent verification.

### Outcome disposition by PR

### [#4801](https://github.com/scode/farhelm/pull/4801/changes)

- `trailing-harness-upgrades-https-redirect-downgrade.md` — Fixed. Fallback installer downloads require HTTPS at the
  starting URL, every redirect and the final response.

### [#4809](https://github.com/scode/farhelm/pull/4809/changes)

- `installed-agent-runner-can-leave-interrupted-build-running.md` — No change. A runner-only termination can leave work
  using resources until it exits, with no completed build attributed to the interrupted run.
- `interrupted-drift-case-retains-failed-verdict.md` — Fixed. The finalized recorder outcome distinguishes cancellation.
  A case verdict already written is retained, and a cancelled case without one leaves the interruption record.
- `docs-shot-partial-failure-hides-success-paths.md` — No change. The run reports failure and retains the screenshots
  that succeeded. Only the console listing is absent.
- `invalid-boat-member-limit-clears-previews.md` — Fixed. A malformed member limit is refused before clearing previews
  or staging a capture.
- `docs-shot-run-can-overwrite-earlier-failure-frame.md` — Fixed. The rejected frame is kept under a unique name that a
  later shot cannot overwrite.
- `focused-golden-update-replaces-unpublished-full-captures.md` — Fixed. A focused update writes to its own capture
  directory and preserves the previous full update.
- `golden-output-backpressure-can-block-cancellation-cleanup.md` — No change. A paused reader delays cancellation until
  output flows again. The retained pictures and verdict stay attributable to the run.
- `failed-golden-reference-setup-can-leave-partial-files.md` — No change. Incomplete reference preparation leaves files
  only in the private comparison directory and returns an error.
- `shell-release-shot-parks-on-removed-control.md` — Fixed. The Bash and Zsh release recapture parks on the launcher's
  current destination heading.
- `antigravity-release-shot-waits-on-folded-field.md` — Fixed. Deliberate recapture reveals More options, opens it, and
  parks the pointer on the visible Conversation ID row.
- `golden-worker-interrupt-bypasses-cancel.md` — No change. A worker-only interruption can take away the optional review
  page. Publication still requires the complete capture set.
- `golden-report-omits-failed-scenes.md` — Deferred to “Show missing renders on the portable golden changes page” in
  TODO.md. The comparison wrapper needs per-appearance outcomes to separate failed renders from ordinary pixel
  differences.
- `capture-close-setup-error-discards-frames.md` — Fixed. If the capture's stop socket cannot be created, the fallback
  stops the owned terminal host. A successful fallback lets publication keep the frames already captured.

### [#4818](https://github.com/scode/farhelm/pull/4818/changes)

- `capture-drive-error-discards-staged-screens.md` — Fixed. A drive controller failure is reported while its previously
  verified screens are published and later harnesses run.
- `capture-fixture-copy-can-break-screen-pair.md` — Deferred to “Publish captured agent screens as recoverable pairs” in
  TODO.md. A screen and its title need a shared publication and interruption recovery contract.
- `capture-stop-retry-requires-original-binary.md` — No change. An unavailable original build already leaves a
  diagnostic and retains the run for the compatible-executable recovery path.
- `centos-gate-can-stage-sibling-binary.md` — Fixed. The run builds its payload in checkout-local Cargo directories and
  the ELF check, provision and update use its staged copy.
- `centos-gate-resolves-relative-target-from-caller.md` — Fixed. The ELF check and fixture get the same absolute staged
  path, independent of the caller’s directory and nextest’s Cargo target.
- `capture-test-python-minimum-undocumented.md` — No change. The documented test host already needs Python 3.11 for its
  recorder and tooling, and this standalone test has no older Python support promise.
- `worker-staging-failure-leaves-in-progress-evidence.md` — No change. The failed staging invocation returns an error
  before the workload starts, with its own source stamp and no verdict to lose.
- `trailing-tensorlake-sandbox-tests-allocates-before-ledger.md` — No change. The maintainer already declined this
  allocation and cleanup ordering change. The window precedes any test evidence.
- `trailing-drift-runner-list-upgrade-only-conflict.md` — Discarded as already fixed by
  [#4781](https://github.com/scode/farhelm/pull/4781/changes). Argument parsing rejects --list with --upgrade-only
  before any vendor action.
- `trailing-harness-upgrades-orphans-updater-grandchildren.md` — Deferred to “Stop the agent updater's process group
  before running drift cases” in TODO.md. The fix needs teardown across timeouts, fallback and signals during process
  startup to preserve version attribution.
- `trailing-test-worker-disk-guard-df-du-edge-cases.md` — No change. A failed recheck may force a cold build or a
  misleading cleanup summary before a test starts, with no completed verdict or evidence affected.
- `trailing-release-shot-custom-agent-failure-strands-settings-draft.md` — Fixed. A failed fill closes Settings and
  discards any staged draft before returning the shot’s error.
- `release-verifier-retry-overruns-deadline.md` — No change. A retry can complete past its shared allowance. The
  retained verdict describes work that ran and the timing-budget defect falls outside the narrow evidence rule.
- `trailing-release-shot-guarded-replace-failure-leaves-yolo-question.md` — Fixed. A failed Confirm reply or missing
  guarded choice cancels the outstanding question and waits for its title to disappear.

### [#4821](https://github.com/scode/farhelm/pull/4821/changes)

- `release-verifier-does-not-bind-relaunch-samples.md` — Deferred to “Bind the brick test's reopen samples to an app
  relaunch” in TODO.md. Proving the transition needs ordered receipts across guest operations and host import.
- `trailing-indented-blank-lines-defeat-release-note-pr-coverage-checks.md` — Fixed. The coverage check and bullet
  validation both ignore whitespace-only lines.
- `trailing-brick-test-cleanup-skips-p1-retry-and-preflight-evidence.md` — Fixed. Cleanup requires a prepared test VM
  and keeps the Mac preflight receipt and retry evidence for checks already run, including checks omitted from a
  narrowed scope.
- `trailing-brick-test-dead-end-misses-deleted-p0-automation-failure.md` — Fixed. When an installer test's fixtures or
  automation failed, discarding the prepared VM pair now marks the remaining bonus checks unavailable. Existing product
  results stay in the report.
- `trailing-brick-test-fixture-probe-forwards-sys-executable.md` — Fixed. The clean-fixture probe uses the absolute
  Python path configured for the Mac peer.
- `trailing-brick-test-linux-remote-start-accepts-empty-sessions.md` — Fixed. The Linux starting snapshot must prove
  that the database already contains a saved session before the menu action can run.
- `trailing-brick-test-machine-profile-conflicting-mac-address.md` — No change. Existing identity checks refuse the
  mismatched base before it can provide test evidence.
- `trailing-brick-test-old-sessions-nonexistent-unavailable-status.md` — No change. The conflicting wording can trigger
  a diagnostic error but the saved run still reports the previous-release observation as unavailable.
- `trailing-brick-test-old-sessions-stdout-prints-fail.md` — No change. The live progress line can disagree with the
  durable previous-release verdict. The recorded verdict is already unavailable.
- `trailing-brick-test-p1-automation-failure-drops-unverified-fail.md` — Fixed. The host saves the unverified product
  failure before attempting to import the result's evidence.
- `trailing-brick-test-p1-permission-block-swallowed.md` — Fixed. A macOS permission refusal is saved even if the native
  driver exits unsuccessfully or its remaining evidence cannot be imported. It still prevents further test actions on
  that VM.
- `trailing-brick-test-record-settle-breaks-retry-unavailable.md` — Deferred to “Keep the brick test's installer pair
  retryable after an unavailable P0 check” in TODO.md. Derived skips need attribution and replay rules that preserve
  independent checkpoint history.
- `trailing-brick-test-require-no-failure-allows-rerun-on-failed-p0.md` — Fixed. A failed attempt refuses direct rerun
  before writing a new tester claim and leaves the supported recovery transition available.

### [#4825](https://github.com/scode/farhelm/pull/4825/changes)

- `trailing-brick-test-unavailable-retry-p2-stale-dict.md` — No change. The P2 guard rejects retry; the finding concerns
  the test's reference after reload rather than a runtime verdict.
- `trailing-brick-test-announcement-override-check-untested.md` — No change. Both launch paths perform the live
  environment check. The missing negative case is a test gap.
- `trailing-brick-test-missing-announcement-feed-reported-as-candidate-failure.md` — No change. The documented
  publication gate records a P2 failure when the fixture is missing. The report still fails that check and names the
  exact unpublished file and HTTP 404.
- `trailing-brick-test-announcement-check-has-no-delivery-wait.md` — Fixed. The guest waits up to 60 seconds for the
  announcement action and returns to a normal launch before reporting absent delivery as unavailable.
- `trailing-drift-turn2-hook-check-passes-on-noop-stop-ack.md` — Deferred to “Require a committed hook report after
  Restart in the drift checks” in TODO.md. The fresh witness needs the resumed generation's committed report, not a
  transport acknowledgment.
- `trailing-drift-grok-home-overwritten-by-login-shell-rc.md` — Fixed. The launch wrapper reapplies the private Grok
  configuration root after login shell initialization on Start and Restart.
- `trailing-drift-private-config-copy-back-aborts-first-error.md` — Discarded as already fixed by
  [#4783](https://github.com/scode/farhelm/pull/4783/changes). Copy-back retains a note for each failed file, removes
  the temporary copy and continues checking the real hook files.
- `trailing-grok-drift-test-uses-hook-witness-fresh-after-noop-acks.md` — Deferred to “Require a committed hook report
  after Restart in the drift checks” in TODO.md. The same committed-report witness covers Grok's resumed turn.
- `trailing-test-worker-stop-during-prep-mutates-and-prunes.md` — Fixed. Stop is checked during preparation and a
  canceled invocation that never starts its workload leaves all earlier test evidence in place.
- `trailing-test-worker-collect-discards-recorded-exit-status.md` — Fixed. Collect retrieves the completion code along
  with the source stamp, records it, and returns it after evidence is copied.
- `trailing-test-worker-status-hides-bootstrap-lock.md` — No change. The hidden bootstrap lease affects a diagnostic
  line without changing admission or already recorded results.
- `container-comparison-targets-removed-control.md` — Fixed. The comparison opens Host, chooses New container in its
  menu and waits for the Container host control.
- `launcher-comparison-heading-selector-case.md` — Fixed. Comparison steps wait for the rendered Destination id and use
  its painted uppercase label for dismissal, or the pinned Reset choices button after scrolling.

### [#4828](https://github.com/scode/farhelm/pull/4828/changes)

- `container-golden-stays-on-loading.md` — Deferred to “Show the settled build status in the New container golden” in
  TODO.md. The scene needs a usable container-host check as well as the exact recipe-place build record. Refreshing the
  single scene requires a coherent reference publication against the launcher and recipe-host work that also changes
  those pictures.
- `trailing-session-toolbar-golden-height-clips-on-macos.md` — Fixed. The macOS frame fits both title-band rows and
  their spacing and padding. Linux uses its existing frame.
- `deflake-sweep-fixtures-inherit-git-hooks.md` — Fixed. The scratch history ignores machine commit hooks so a developer
  hook cannot fail setup before the sweep-base verdict.
- `deflake-base-refusal-test-misses-run-state.md` — No code change. The start path already resolves and refuses the base
  before constructing state. The finding is a coverage gap.
- `docs-shots-share-host-details-state.md` — Fixed. The Details picture restores compact Hosts rows after capture,
  including after a failed capture, so the next shot can start from the compact view.
- `upgrade-highlights-feed-can-outgrow-desktop-reader.md` — No code change. The deliberate reader cap leaves substantial
  room for the current highlights, and no release evidence is lost at the current feed size.
- `capture-stop-retry-can-lose-starting-host.md` — No code change. The reported orphan needs an interrupt during process
  creation and a retry before the host appears. The alleged late startup after directory removal was unconfirmed and no
  captured evidence was shown lost.
- `trailing-empty-highlights-release-path-lacks-consumer-test.md` — No code change. The generator and desktop can
  consume a release with an empty highlights list. The reported gap is in coverage and presentation.
- `trailing-test-worker-relative-checkout-breaks-lock-isolation.md` — Fixed. Dot paths resolve to the same worker lock
  as the named checkout. Explicit names keep reaching the existing worker tree, including after the local checkout is
  removed.
- `trailing-tensorlake-rust-tests-summary-head-60-evicts-count.md` — Fixed. The dispatcher reads nextest's final test
  count from the retained console, where a crowded diagnostics excerpt cannot omit it.
- `trailing-mac-reboot-shell-discards-command-status-tag.md` — No code change. A refused reboot can time out with a less
  direct diagnostic in the manual test. The failure still fails the test and does not hide a release verdict.
- `trailing-brick-test-exiting-stdio-proxy-mid-scan-fails-remove.md` — Fixed. When command-line data disappears
  mid-read, recheck that the executable is gone before reporting the process as a leftover. A live process with
  unreadable arguments still counts.
- `trailing-announcement-override-survives-app-relaunch.md` — Fixed. Both Restart to update and automatic reopen clear
  the old app's release-test announcement override when launching the next instance.

## Open questions and possible follow-ups

- Keep the portable golden report honest when one appearance never renders. The viewer currently lists image differences
  and has no per-render record for missing appearances.
- Publish captured agent screen and title fixtures as a recoverable pair. A reader can otherwise see a new screen beside
  an old title after an interruption.
- Stop the agent updater's process group before measuring the CLI version. A descendant can still replace the executable
  after its parent times out and make a drift result cite the wrong version.
- Bind the brick test's reopen samples to two witnessed app lifetimes. The samples can otherwise come from one instance.
- Keep the brick test's installer pair retryable after an unavailable check. The test still skips later lifecycle and
  bonus checks based on the earlier unavailable result. A retry needs to withdraw only skips derived from that
  checkpoint.
- Require a fresh, committed hook report after Restart in the Codex, Antigravity and Grok drift checks. A drift test can
  pass even though the restarted agent wrote no new report, because the acknowledgment also covers ignored callbacks.
  Two findings share this follow-up.
- Show a settled build note in the New container golden. The reference depends on both a ready Container host and a
  build record for the exact recipe on that host. The fixture and both references need an isolated refresh on the
  current launcher.

## The PRs

- Require HTTPS when the fallback installer follows redirects.
  [#4801](https://github.com/scode/farhelm/pull/4801/changes)
- Keep comparison, capture and drift evidence tied to the run.
  [#4809](https://github.com/scode/farhelm/pull/4809/changes)
- Check private provisioning and release screenshot capture. [#4818](https://github.com/scode/farhelm/pull/4818/changes)
- Preserve release test receipts across failures and retries.
  [#4821](https://github.com/scode/farhelm/pull/4821/changes)
- Keep test exits and comparison captures attributable. [#4825](https://github.com/scode/farhelm/pull/4825/changes)
- Keep maintainer verification runs independent. [#4828](https://github.com/scode/farhelm/pull/4828/changes)

## Checks run, reused and skipped

- The stack was rebased onto `dfae22dc28c3`. Main added drift coverage for workspace trust, hook timing, and
  instructions. The rebase kept its supervisor setup and private configuration checks around the Grok launch change. Ran
  pinned `cargo nextest run --locked -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'` at `1518c9af9316`
  (record `c3b073ef-189c-41bf-b71f-385a66964b80`). All 32 selected cases passed, with no runtime skips.
- Reused the installer fallback stand-ins at `fa20bd49827e` (20 cases, `30419455-cfcc-4483-a6ac-8bb5ed249f5d`). Package
  redirects were exercised with controlled responses and without upgrading a vendor installation. Ran
  `python3 -B scripts/test-run-harness-drift.py`. The installer branch did not change in the rebase.
- Reused the selected docs, golden, and capture fixtures at `d4aee68f3fe7`. The final run records include
  `5e6bcd3e-aae5-41b9-9e0e-148858578276` (29 docs cases), `dbcbac5c-bbf1-4c15-a4c7-543f0d8f4694` (21 golden comparison
  cases), `75ddebba-0c04-4efc-88d2-5e00bce0f436` (13 capture cases), `64151df1-7ee8-4a2b-91fc-ccf204b9fe45` (12 capture
  shutdown cases), and `42e64411-09c4-42ff-969b-c38b0766556a` (six local command stand-ins). The actual staged shell and
  Antigravity pictures were inspected. The successful Antigravity run was `db5c575b-2b18-4360-9e0e-6e76a9883553`.
  Commands were `scripts/test-docs-shots.py`, `scripts/test-check-gui-goldens.py`, `scripts/test-gui-agent.py`, and
  `scripts/test-capture-agent-screens.py` under Python, plus `deno test -A scripts/boat/boat-tests.ts`. The private
  screenshot used
  `scripts/docs_shots.py --no-build --only release-notes --recapture-release-shot release-notes/v0-42-0-antigravity` and
  a private output directory. The rebase preserved the exercised paths.
- Reused the native local CentOS provision-and-update check at `1c6765e6ec02` (`4f766a39-8b03-4d47-a7e4-9d7ee18cf376`,
  inner nextest record `678614d6-7af1-44c9-aae8-46b8732ad775`). Ran `scripts/test-provision-centos.sh` from outside the
  checkout with a relative Cargo target. A build timeout in `438d72cd-3de8-4569-b8a5-4ffd71960652` and an interrupted
  test case in `1f176f6f-1be9-4ffa-896e-2fed67eb1cff` gave no result. The final run completed the real provision and
  update case. The same draft passed 31 cases via `python3 -B scripts/test-docs-shots.py`
  (`a2110448-bab1-4f6c-95c8-34cd63aa13f4`) and 13 via `python3 -B scripts/test-capture-agent-screens.py`
  (`373a8390-d72a-4a44-85b8-623731c3742e`). The exercised files were preserved.
- Reused 324 portable release checks and the changelog checker at `1986ecd37508`
  (`21eb1504-4da6-4c08-a1b0-736c2a0b75d8`, `9f334df9-8470-4931-b175-374a5af8fc5e`). Ran
  `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` and
  `python3 -B releasing/check-changelog.py --self-test`. Main's workflow changes described the artifact handoff. The
  brick-test paths exercised here were preserved.
- Reused 17 local worker collection and cancellation cases and two private launcher comparison images at `557fa098c6ec`.
  The runs were `334954c2-795d-4341-9aa3-1b2f922e8459` and `35f95b58-8ca9-4a7e-8c0d-669bfa8bbbdc`. Ran
  `python3 -B scripts/test-worker/test-test-worker.py` and a private two-scene capture through the screenshot-batch
  module. The images showed the New container controls and Templates list. The rebase preserved the worker and capture
  inputs.
- Reused the final step-six local test results at `48b1e92e53b2`. They include 18 worker cases
  (`c027f17b-79aa-4479-88af-39beb05dff92`), three shard dispatcher cases (`5c02ddff-0a61-4c60-b396-ddde251ccb03`), 67
  guest probes (`2e25d6f6-f195-4be6-b301-cf76a027efd2`), 20 deflake cases (`743d160c-a925-4866-9843-1368b705772e`), 31
  docs shot cases (`41adebcd-ad5d-4930-b05f-9a2fde57ae5f`), and two desktop reopen command tests
  (`f8f922ce-73bf-4938-84bc-c0e32c4af5d4`). The selected docs shots ran together against a previously built, private
  staged app and were visually inspected (`2fa76b78-a99b-4743-abee-66a1c4dbbb1b`). The Linux session toolbar comparison
  passed both appearances (`6177f613-cf51-4999-99f4-ed4e536b9b6c`). No change after these checks affected their code or
  fixture behavior. Commands were `python3 -B scripts/test-worker/test-test-worker.py`,
  `deno test -A scripts/tensorlake/rust-tests.ts`, and Python's `releasing/mac-vm-test/test_guest_operations.py`,
  `deflake/test-deflake.py`, and `scripts/test-docs-shots.py`. The desktop command selected the automatic-reopen and
  Restart to update tests from `cargo nextest run --locked -p farhelm-desktop --lib`. The renders used
  `scripts/docs-screenshots.sh --local --no-build --only manage-hosts/details --only start-a-session/host-menu` and
  `python3 scripts/check-gui-goldens.py --scene session-toolbar --jobs 1`.
- Formatting checks (dprint, Rustfmt, and Deno), ShellCheck, the Rust test delay check (`scripts/check-test-sleeps.py`),
  changelog format (`releasing/check-changelog.py format`), and instruction references (`scripts/check-agents-md.py`)
  passed. The six PRs' exact queue deletions and archive records were also checked against the plan. The instruction
  check has one existing warning about a historical archive citation.
- The Linux checkout cannot exercise Mac LaunchServices or render the Mac toolbar. Worker and Tensorlake validation ran
  locally as directed. Vendor upgrades and chat calls were unnecessary for the controlled installer and evidence
  fixtures. This scope also left the full workspace suite and release publication out of the check run.

## Review gate outcome

Each PR was independently reviewed by one fresh-context reviewer. The stack also received a cold read of each PR
message. Review corrections covered the real CentOS evidence path, release-test retry records, the bounded announcement
wait, test worker exit attribution, and launcher screenshot selectors. The last review preserved access to the worker's
records after a local checkout disappears and corrected shard counting when earlier output looks like a summary. The
dropped suggestions and their scope decisions are in `farhelm-plan-triage-tooling-nov24-log.md` beside the checkouts.
