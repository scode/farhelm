## What this was about

The November 24 review triage found places where Farhelm's specs, docs and comments described behavior different from
the app. It also settled several behaviors that the specs had left ambiguous. This plan records those decisions and
closes the corresponding feedback. The Grok and custom-agent correction notices were the only product text changed. Each
now explains that an earlier Restart could have reopened the previously reported conversation, and a report from an
ancestor corrected the choice.

## Things you should know

All 53 assigned outcomes were applied. Two moved into the second draft because their changes belong in the same
paragraphs as the interrupted-session recovery and work-start ordering corrections. The four draft PRs each archive
their own decisions and remove their feedback from the active queue.

The app-wide input rule accepts narrowly timed background updates before their frame is shown. Destructive confirmations
and launches that retire a session still use the action and consent already shown. Template Delete acts on the named
template, and its displayed label decides whether Undo appears.

Credential removal is best effort at the next opportunity. For an already connected host that means reconciliation at
its next connection, with vendor-owned logins following the vendor's sign-out rules. A leaked credential needs
revocation at the vendor. An old unrecorded Claude settings write can survive the migration. The standing brick-test
announcement feed deliberately shares production verification, with the replay risk accepted in triage.

Settings returns focus to the control that opened it when that control is still available. A pending launcher submission
holds Suspend and Resume for its target host. Plain Replace and Restart leave those actions available. Clone and Replace
with prefill the agent's default when an older launch has a missing or unsupported permission mode. That default is
Guarded for agents that offer it.

A sandbox session Delete that was never sent has made no decision. A sent request that goes unanswered can still be
running or may already have deleted the session and sealed its sandbox if it was the last. A reply that says Delete
carries on leaves the window to follow later state updates. The release gate already blocks publication when build
artifacts are missing. Its regenerated workflow matches the committed workflow.

The hook-injection docs now explain when a custom command gets arguments at its placeholder. The internal launcher,
session pane and interrupted-card comments, Codex fixture, and Tensorlake setup test description stay aligned with their
respective contracts. There were no changes to the launcher, Settings or environment deletion logic.

## Open questions and possible follow-ups

The existing TODO still asks whether Clone and Replace with should prefill YOLO for an older agent launch with no saved
permission mode. Today the launcher can show Guarded even though a plain action that reuses the stored launch treats it
as YOLO for confirmation. The triage also selected a text correction for custom effort values beginning with a hyphen.
The spec now gives the supported `--effort=-fast` spelling.

## PRs

- [#4799](https://github.com/scode/farhelm/pull/4799/changes) records 19 accepted specification outcomes about frame
  timing, consent, credentials, workspace trust, hook files, spare directories and Settings.
- [#4800](https://github.com/scode/farhelm/pull/4800/changes) corrects 13 descriptions of terminal copy, activity
  ordering, interrupted recovery, CLI arguments, checkout caches, template prompts and hook reporting.
- [#4802](https://github.com/scode/farhelm/pull/4802/changes) corrects 11 descriptions of launcher placement and Tab
  order, attachment and cleanup, Tensorlake, and the release workflow.
- [#4805](https://github.com/scode/farhelm/pull/4805/changes) corrects 10 outcomes covering the Grok and custom-agent
  notices, environment deletion and Settings behavior, and source comments.

### Outcome mapping

<details>
<summary>Outcomes applied in PR #4799</summary>

- `trailing-signed-announcement-feed-replayable-across-files-and-versions.md`
- `trailing-deleted-credential-stays-on-connected-host-after-lookup-failure.md`
- `stale-permanent-delete-can-remove-repaired-template.md`
- `trailing-setup-rerun-omits-tailscale-allowing-ssh-downgrade.md`
- `stale-host-retry-can-continue-sandbox-delete.md`
- `launcher-alias-enter-can-apply-an-undrawn-model.md`
- `unshown-upgrade-highlights-can-be-recorded-seen.md`
- `notifications-act-on-unshown-list.md`
- `launcher-recent-highlight-can-change-path-spelling.md`
- `trailing-shift-click-mixes-live-range-anchor-with-displayed-row.md`
- `grok-manual-hook-replacement-omitted.md`
- `antigravity-hook-removal-can-delete-extra-files.md`
- `fresh-checkout-trust-can-bind-replacement-directory.md`
- `antigravity-trust-falls-back-to-uncanonical-folder.md`
- `trailing-unstamped-host-skips-legacy-claude-settings-cleanup.md`
- `hook-hint-settings-focus-return-mismatch.md`
- `trailing-plain-replace-and-restart-do-not-disable-suspend.md`
- `trailing-clone-replace-with-prefills-guarded-for-unset-permissions.md`
- `trailing-implicit-home-spare-ownership.md`

</details>

<details>
<summary>Outcomes applied in PR #4800</summary>

- `custom-command-spec-misstates-hook-arguments.md`
- `trailing-osc-52-copy-rule-omits-primary-selection-distinction.md`
- `trailing-work-start-spec-still-says-rename-replaces-sampling-state.md`
- `trailing-interrupted-recovery-guide-still-assumes-no-prior-process-remains.md`
- `trailing-clone-cli-reference-still-omits-environment-exception.md`
- `trailing-recent-activity-product-spec-still-promises-retired-capture-recovery.md`
- `trailing-docs-github-checkouts-stale-single-dir-cache-path.md`
- `trailing-spec-md-security-still-describes-ongoing-codex-merges.md`
- `template-leave-spec-still-inline.md`
- `trailing-cli-effort-hyphen-and-spec-drift.md`
- `trailing-codex-fixture-and-notifications-stale-corridor-comments.md`
- `trailing-interrupted-card-guide-still-names-only-host-restart.md`
- `trailing-work-start-guide-omits-deferral-while-lifecycle-claim-is.md`

</details>

<details>
<summary>Outcomes applied in PR #4802</summary>

- `trailing-tensorlake-add-tests-misleading-codex-null-docstring.md`
- `trailing-spec-impl-contradiction-orphan-discovery-local-read-bounds.md`
- `sealed-container-delete-spec-conflicts.md`
- `quick-launch-placement-contract-omits-local-host.md`
- `trailing-spec-md-status-contradicts-codex-pin-corrected-text.md`
- `trailing-registered-all-numeric-ssh-names-bypass-first-label-shortening.md`
- `trailing-interrupted-be-superseded-by-inferred-preparation-failure.md`
- `release-gate-regeneration-guidance-stale.md`
- `launcher-tab-order-omits-command-message.md`
- `trailing-exited-session-skips-portable-sweep.md`
- `trailing-restart-disposition-documentation-still-promises-no-external-change.md`

</details>

<details>
<summary>Outcomes applied in PR #4805</summary>

- `trailing-environment-delete-implementation-spec-still-describes-desktop-port-as.md`
- `trailing-session-pane-stale-doc-on-helm-pushes.md`
- `trailing-product-spec-still-promises-no-decision-after-unanswered-environment.md`
- `terminal-steady-block-comment-stale.md`
- `trailing-custom-pin-corrected-text-omits-prior-restart-consequence.md`
- `trailing-specs-and-core-omit-grok-and-custom-from-pin-correction-lists.md`
- `trailing-migration-comments-reboot-classification-remains-disputed.md`
- `trailing-delete-scope-comment-describes-old-stop-behavior.md`
- `tensorlake-key-clear-spec-misstates-leftover-refresh.md`
- `trailing-ticker-reduced-motion-doc-misstates-outline-fade.md`

</details>

## Checks run, reused and skipped

- Markdown formatting, Rust formatting, diff whitespace and instruction-link checks passed for the affected drafts. The
  instruction checker reported one inherited warning about a historical archive heading reference.
- The source-only sleep checker inspected 310 test delays and found no missing rationale on the final tested revision.
  The changelog format check accepted the new fragment.
- `python3 releasing/generate-workflow.py --check` with cargo-dist 0.32.0 passed at `8e404ace`. Later edits leave the
  workflow input and generator unchanged.
- `cargo nextest run --locked -p farhelm --test e2e -E 'test(=hook_identity::grok_parent_corrects_a_child_first_selection) | test(=hook_identity::custom_agent_positional_report_corrects_to_the_parent_and_restarts_it)'`
  passed both selected tests at `32369ff6`, recorder run `e6d0354c-b9f3-4706-b76e-d41471cfa398`. The tests used
  stand-ins and exercised the corrected notices and the captured conversation. Review changes after that run were prose
  and comments.
- The first three drafts needed no runtime tests. The docs page changed prose without changing links or examples. No
  live vendor run or visual review was needed for the last draft's notice text and existing notice presentation.

## Review outcome

Each draft passed the required independent Sol code review and a fresh reading of its commit and PR wording. Review
findings were addressed in each draft, including the difference between a Delete that was never sent and one that went
unanswered, vendor-owned login files, foreground conversation corrections, and the scope of the launcher-only Suspend
and Resume hold.

### Landing

#### What landed with it

Two plans landed this round: drift-instructions-delivery first, a new live drift test category that touches none of this
plan's files, then this one through [#4799](https://github.com/scode/farhelm/pull/4799/changes),
[#4800](https://github.com/scode/farhelm/pull/4800/changes), [#4802](https://github.com/scode/farhelm/pull/4802/changes)
and [#4805](https://github.com/scode/farhelm/pull/4805/changes). Since this plan was based, main gained the previous
rounds' drift work, the hook log's new event field with its website paragraph, and the other November 24 triage plan,
which archived 50 discarded findings. None of those changes the spec text or code this plan edits; the hook log
paragraph and this plan's edit to the same website page are in different passages. The rebase conflicted only in the
triage ledgers and the review queue's index, where both plans had removed different findings and appended different
decisions to the archive; I kept both sides, and a script check then confirmed the index matches the findings on disk
and no decision is in both the ledger and the archive.

#### Fixes made while landing

None.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. This round landed two
plans, drift-instructions-delivery and triage-spec-text-nov24. Both reports' checks ran before their stacks were rebased
onto the drift and triage changes that landed meanwhile, so the landing ran its checks on the two plans stacked together
on main, with the rebase's conflict resolutions in them (`140e156e`):

- `cargo nextest run --locked -p farhelm --test e2e` filtered to `real_agent_capture::drift::` and `hook_identity::`, on
  the test worker (recorder run `ea45d013-8e11-4428-86d3-84882969acf9`): all 67 passed, with no runtime skip. These are
  the drift tests that need no vendor CLI and every hook identity test, which includes the two that check the rewritten
  conversation notices.
- `cargo nextest run --locked -p farhelm-supervisor --lib` filtered to `core::vendor::`, `claude_pin` and
  `notifications::`, on the test worker (recorder run `34e4e046-fe01-45e1-b5b3-85f228cd1827`): all 17 passed. The notice
  texts live in those modules.
- `python3 -B scripts/run-harness-drift.py --list --category all`, on the test worker: it listed every category's cases,
  the six new instructions delivery cases included, starting no agent.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `python3 releasing/generate-workflow.py --check` (the release workflow's source file changed), the website build
  (`bun run build` in `website`), `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check`,
  `python3 scripts/check-agents-md.py` (no errors; one warning, about an old citation in the triage archive, already on
  main) and `python3 -B scripts/test-run-harness-drift.py` (recorder run `1d1aca37-53b3-415f-9350-5c808cf57091`, all 18
  passed), locally: passed.

Reused: the executor's live run of the instructions delivery category against the installed agents, recorded in its
report; the rebase changed how the category reaches the shared launch helper, not what it launches.

Skipped: the live drift cases themselves, since they spend vendor turns, need the vendor logins and are opt-in by
design; the rest of the workspace Rust suite and the desktop checks, since the only behavior change in either plan is
the text of two notices, which the tests above cover; and CI, which runs only on demand.

#### What the report says that is no longer true

Nothing. The checks the report lists ran before the rebase; the ones above ran after it.

#### Things you should know that the report does not say

- **The landing's review agreed with the spec changes it sampled.** It spot-checked about ten of them against the code
  and found them matching, and confirmed that the change to the release workflow's source file is comments only, which
  do not reach the generated workflow.
- **Two decisions corrected text and left a suggested code change undone**, as their recorded decision ("correct the
  text") allows. For the command line's `--effort`, a value starting with a hyphen given as a separate word
  (`farhelm agent create --effort -fast`) is still rejected; the report mentions this one. For the Tensorlake host setup
  tests, a misleading test comment was corrected, but the suggested extra check that setup never reads the agent's
  settings was not added. Either could be a small follow-up.
- **Two spec sentences this plan wrote are looser than the code.** The quick launch rule says it uses the open session's
  original host "if that host is ordinary", where the code means the same installation and not a sandbox or container.
  The rewritten sandbox Delete paragraph dropped "Delete does not guess"; the rule that Delete never resumes the sandbox
  is still stated elsewhere in the spec.
- **A queued finding about the Hosts menu's setup rerun still cites a finding this plan removed** (a stale host retry
  continuing a sandbox Delete). That decision is in the triage archive, and the queue's rules do not require updating
  the citation.
