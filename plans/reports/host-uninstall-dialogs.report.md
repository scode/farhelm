## Since the last review

None of the six PRs had merged when landing stopped on six failing browser cases. This round updates #2305's progress
tests and #2317's refusal test, restores #2317's missing button hover text and TODO wording, and rebases all six PRs
onto current main. No PR was added or dropped; all remain drafts in the original order.

The suspected status regression was a contradictory test expectation. The host-state fixture explicitly supplied an
active setup run with no steps, then expected the idle host's “old version” label. Running setup is supposed to occupy
that status slot. The fixture now supplies one real step and checks “setting up”, its count and current step; the
ordinary old-version cases and the absence of Update during setup remain covered. No product status change was needed.

Review also found a real Remove-dialog race: a dismissal arriving just after confirmation, before the busy controls
rendered, could release the page behind the still-open dialog. The panel now checks its live request state before
releasing isolation or returning focus. A recorded reproduction failed for header close, Cancel and Escape on both
engines before the fix; all six variants pass after it.

Both requested browser files now pass in full: 234 cases, 117 each on Chromium and WebKit, with no skips, failures or
interruptions.

## What this was about

Host setup and uninstall confirmations used plain text that was hard to scan and did not name every action a run would
take. Progress appeared separately in the host row, several refusals appeared as red sidebar text, and the uninstall
result occupied several lines of the narrow hosts panel. The maintainer chose one helm-written step list for
confirmation and progress, numbered confirmations, progress in the row, dialogs for definite refusals, and a one-line
uninstall result.

The six PRs implement that choice. Each planned action appears once under the same name in confirmation and progress,
with its paths and explanation in the confirmation. Setup and uninstall open their row for that run without changing the
panel-wide Details setting; the user can fold it. A failure opens the row for diagnosis and success folds it. Update
uses the same progress rail. The uninstall result has its kept-data path on hover and through Copy. Definite setup,
uninstall, Retry, adopt and Remove refusals appear in the action's dialog.

## Things you should know

The helm remains the only author of step wording. Copy plan returns its exact text. Home-relative path chips retain full
paths on hover. A refusal during the helm's recheck after confirmation remains a failed step in the expanded host row,
as agreed. Update submission refusals and host-list read errors retain their previous presentation. A lost submission
response is uncertain, so it remains a warning rather than claiming the action was refused. The numbered inline Update
confirmation is a defensive path; normal Update submits without showing it.

Remove keeps its confirmation and page isolation until the helm answers, then shows any refusal there. If the saved
preference skips confirmation, a refusal opens a short dialog. Remove's hover text now states that it only forgets the
host; its supervisor and sessions keep running. A definite add-host setup refusal reopens the add form with the
destination, without restoring the consumed setup offer.

The five stale browser tests now assert those progress/dialog contracts while retaining their original checks for
page-lock release, busy-host Update suppression, fixed-menu geometry, retry destination and host preservation. The TODO
entry about refusal wording now acknowledges that refusals appear in dialogs.

The Manage hosts documentation describes the new behavior. The Add a remote host setup-plan screenshot still shows the
old text-box confirmation.

## Open questions and possible follow-ups

No implementation decision is outstanding. A later docs screenshot refresh should recapture the Add a remote host
setup-plan image. This unattended run did not start a preview or capture screenshots.

## PRs

- [#2298 — list all host actions in confirmations](https://github.com/scode/farhelm/pull/2298/changes): helm wording and
  matching plan/progress data, draft on main.
- [#2303 — show host confirmations as numbered steps](https://github.com/scode/farhelm/pull/2303/changes): shared
  stepper and exact-plan copy, draft on #2298.
- [#2305 — show host setup and uninstall progress in rows](https://github.com/scode/farhelm/pull/2305/changes): progress
  rail, one-off expansion and corrected browser expectations, draft on #2303.
- [#2307 — keep host uninstall results on one line](https://github.com/scode/farhelm/pull/2307/changes): compact result
  with hover, Copy and dismissal, draft on #2305.
- [#2311 — show host planning failures in dialogs](https://github.com/scode/farhelm/pull/2311/changes): planning and
  definite confirmed-action refusals, draft on #2307.
- [#2317 — keep host action refusals in dialogs](https://github.com/scode/farhelm/pull/2317/changes): remaining refusal
  paths, live Remove dismissal guard, restored hover/TODO wording, docs and three completed TODO removals, draft on
  #2311.

## Checks run, reused and skipped

### Run this round

On the corrected source, `cargo build`, the Dioxus 0.7.10 release web build
(`dx build --package farhelm-ui --platform web --release`), `cargo check -p farhelm-ui --features desktop`,
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `dprint check`, diff whitespace and the
isolated `python -B scripts/check-test-sleeps.py` check all passed. The delay checker inspected 304 delays and found no
missing rationale. The CLI/helm build preceded the UI-only dismissal correction; its binary sources are unchanged. The
release web build and desktop compilation include that correction.

All browser commands ran from `e2e` through `python3 ../scripts/record-test-run.py --runner playwright`, with pinned
tmux 3.7c required, Playwright 1.64.0, one worker and zero retries. Setup/provisioning scenarios use the harness's
isolated simulated provisioning backend; the suite also starts its own real helm and supervisors. It never uses the live
installation.

- The unchanged phase-table reproduction, child command
  `npx playwright test 'terminal-multihost\.spec\.ts' -g 'host-list-phase-table'`, failed 2/2 in
  `9119a48e-0232-47fc-8cbf-d5a4c134d3f4`. Both engines reached the contradictory old-version expectation. This is
  retained failure evidence, not a passing run.
- The six corrected cases, selected by
  `npx playwright test 'provisioning\.spec\.ts' 'terminal-multihost\.spec\.ts' -g 'accepted ADD registers|setup disclosure transitions|failed ADD rerun probes|update all skips a host|a failed removal stays|host-list-phase-table'`,
  passed 12/12 in `d6c13de7-16ff-45f2-8ca7-9d3ba50f71e2`, six per engine, on clean head `56215e5d`.
- The pre-fix Remove race, child command
  `npx playwright test 'terminal-multihost\.spec\.ts' -g 'Remove keeps its dialog open'`, failed all six variants in
  `2967786c-53ed-4286-9d0a-0fe17c33a0d6`: three dismissal routes per engine lost row isolation during the held request.
  The same selection passed 6/6 after the correction in `e972bd25-043c-475c-8a3f-295d0ad54ab5` on clean head `b909a072`.
  Both reports are complete; the failure remains retained privately.
- The complete requested files, child command
  `npx playwright test 'provisioning\.spec\.ts' 'terminal-multihost\.spec\.ts'`, passed 234/234 in
  `b7cee740-024c-4ff8-960f-c9291688bd99`, 117 per engine, on clean head `b909a072`. The report and source identity are
  complete, output is untruncated, cleanup completed, and there were no failures, skips, flaky outcomes, interruptions
  or global errors. This run includes the corrected six cases and all three Remove-race variants per engine.

The final rebase from `bcc46054` to `e9b727cf` brought only quick-launch wording, TODO additions, triage records and
approved-plan cleanup. It changed no runtime source. Its spec edits concern launch sequences; this plan changes
Topology. The rebase was conflict-free and preserved the new TODO entries. Application sources used by the builds are
unchanged across that rebase.

### Reused

The prior round's helm and UI unit evidence still covers the unchanged step model, refusal routing and disclosure
decisions. Child command `cargo nextest run -p farhelm-helm -p farhelm-ui -E 'test(provisioning::) | test(hosts::)'`
passed 309 selected cases in `81ddd4ed-dbdc-4040-b015-770c34a60d1c`, before final modal corrections, on recorded
working-tree fingerprint `956a5ce4940f` above parent `3d953ffa`. The UI-only command
`cargo nextest run -p farhelm-ui -E 'test(provisioning::) | test(hosts::)'` passed 60/60 in each of
`53d4e147-e6ab-4164-a4fb-4c057b144cff` (fingerprint `875224ce0c85`, parent `3d953ffa`) and
`6fec4922-5656-4f84-9b04-de04497afb7a` (fingerprint `19b1d5f1735a`, parent `5f541143`). These were uncommitted PR
working trees, so the fingerprints identify the tested source more precisely than their parent commits. All ran through
the recorder with pinned nextest 0.9.148 and tmux, four slots and zero retries, complete reports and no runtime skip
markers. Reported skipped counts are unselected cases.

The later rebases preserve that tested unit logic. This round changes browser fixtures, static hover text and the Remove
component's dismissal ownership. The new before/after browser cases and full browser files cover that event boundary;
the old unit results do not claim that coverage.

### Skipped

The workspace Rust battery, unrelated browser files, installer, provisioning-container and desktop-shell runtime checks
were skipped because this follow-up changes host browser expectations and a shared dialog's event ownership, covered by
the requested full files and desktop compilation. No desktop shell, IPC, installer or deployment logic changed. The
website build was skipped because this round changes no website files and the earlier Manage hosts edit changed no links
or headings. The complete two requested browser files replace the earlier reliance on named browser cases for this
delivery. No screenshot capture or docs preview runs unattended.

## Review gate

Each original PR passed its required fresh-context gpt-6.1-sol high review. This round received fresh reviews of the
changed #2305 and #2317 units with the complete test-authoring checklist. #2305 had no findings. #2317's pre-render
Remove dismissal finding was accepted, reproduced, corrected in the same PR and reconciled by its reviewer. A separate
review at the executing model confirmed the correction uses the existing live guard without adding another coordination
mechanism. No finding remains open. Commit and PR wording is unchanged from its previous cold-read gate.
