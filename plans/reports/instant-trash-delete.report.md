## What this was about

Permanently deleting managed checkouts from the trash made the user wait for every file to be removed and disabled other
page actions. The maintainer chose immediate acceptance with background removal, a bulk confirmation that closes the
dialog, and progress on the trash icon.

The host now verifies each selected checkout, moves it aside within the trash directory, and answers before removing its
files. One background worker frees space sequentially without holding the lock used by session launches, restarts and
Deletes. Bulk confirmation closes the dialog immediately and sends one batch per host. An entry's own Delete keeps the
dialog open. The trash icon animates while requests or reported removal attempts are active, and its tooltip names the
hosts freeing space.

## Things you should know

Disk space returns after the trash entry disappears. Accepted removals survive a supervisor restart through the existing
working-copy records. A removal failure logs a warning, retains the hidden directory and record, and is retried only at
the next supervisor startup. Failed leftovers alone do not animate the icon or keep the page polling.

The existing identity checks, no-symlink/no-mount traversal and protection for retained session directories remain. New
sessions cannot enter either spelling of a directory accepted for removal, including through canonical aliases or a
fresh checkout rooted there. Review exposed a moved-ancestor alias case; the admission check now compares the root's
current location as well as its recorded location.

Refusals and uncertain replies remain available when the dialog is reopened. An unrelated successful delete does not
clear an uncertain result. Removing a host clears its notices, including notices from a late reply; a disconnection or
failed host refresh does not. The existing fifteen-minute request deadline remains for older hosts that still remove
files before answering. Older hosts omit the new progress indication and remain usable.

The Managed checkout trash section on the Session list documentation page now matches this behavior. Specifications and
the changelog were included with their corresponding code changes, so no separate documentation PR was needed. The
original slow-deletion TODO entry was removed. The live installation was not used.

## Open questions and possible follow-ups

No required work remains. The existing TODO about showing the user when trash cannot finish freeing space remains;
background failures currently have warning logs rather than a new user-facing notice.

## PRs

- [#3131: empty trash without waiting for file removal](https://github.com/scode/farhelm/pull/3131/changes) — draft,
  host behavior and recovery, commit `cd6be88bff9bd2a48d9d946ecbbcf39ff595942c`.
- [#3134: close trash after bulk cleanup confirmation](https://github.com/scode/farhelm/pull/3134/changes) — draft,
  dialog behavior, progress, retained notices and browser coverage, commit `46b3ec8e537e3efa0fbff00fc04192f879371d63`.

## Checks run, reused and skipped

Runtime checks used the repository recorder. Rust selections used pinned nextest 0.9.148, four slots, zero retries and
required pinned tmux. Selected-out tests are not passes.

- The final host safety selection passed 33 cases in `bccb24b1-88db-45d9-b0b7-b4af57ba9305`:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(checkout_trash) | test(service::trash_remover::) | test(existing_create_refuses_a_pending_archive_with_its_source_present)'`.
  It covers the reply before held removal, free host admission, startup recovery, retained failures, sequential
  attempts, identity refusals, journal collision rollback and direct, symlinked and moved-root launch refusals. It also
  verifies that startup recovery retains the state-directory claim after losing its awaiter. The recorded source
  fingerprint begins `10a18193294ffcb7`; the pushed host commit has the same tested behavior. Its complete JUnit report
  and retained output show no runtime skips, truncated output or forced cleanup. This final run supersedes the earlier
  32-case supervisor run `5b14a510-1fba-4094-8cf0-bc8bbf22f493`.
- Proto and helm trash tests passed in the earlier 32-case combined run `609cd896-c4f2-4939-b29c-6357f1764d35`:
  `cargo nextest run -p farhelm-supervisor -p farhelm-proto -p farhelm-helm --lib -E 'test(checkout_trash) | test(service::trash_remover::)'`.
  Only the unchanged proto and helm coverage is reused from that pre-correction run; the final supervisor selection
  above supersedes its host coverage.
- The UI trash selection passed ten cases in `0fd47051-71f8-4c69-b179-c75eda5d3d43`:
  `cargo nextest run -p farhelm-ui --lib -E 'test(list::trash::)'`. This includes frozen selections, confirmation
  authority and notice lifetime. The recorded source fingerprint begins `344858cc20324b20` and covers the final UI
  behavior.
- The CSS animation and token suites passed 34 cases in `9822576a-ac56-43fb-ad70-1dfc211abc6f`:
  `node --test app-css-animations.test.js app-css-tokens.test.js`, from the UI JavaScript test directory.
- The trash-dialog and GitHub-checkout browser specs passed all 78 cases, 39 per engine, in
  `ba46e9de-a6f8-4f69-96f6-f2528b3cbf98`:
  `npx playwright test tests/trash-dialog.spec.ts tests/github-checkouts.spec.ts`, from `e2e/`. Playwright 1.64.0 used
  three workers and zero retries on Chromium and WebKit. These cover immediate bulk closure, single-entry cleanup,
  motion preferences, retained refusals and uncertain results, removed hosts, targeted polling and real archive
  preservation. The complete report has no skips, interruptions, unstarted cases or global errors; all three stacks
  completed cleanup. The source fingerprint matches the UI unit run above. Chromium confirmation and WebKit
  preserved-trash screenshots were inspected.
- `cargo clippy -p farhelm-supervisor -p farhelm-proto -p farhelm-helm -p farhelm-ui --all-targets -- -D warnings`
  passed on the combined rebased source, including the startup ownership correction. Earlier per-unit lint runs also
  passed after test-style warnings were corrected. Browser prerequisite CLI and fixture builds and the Dioxus 0.7.10
  release web build passed before the rebase. Those artifacts were not treated as builds of the rebased tree. An initial
  invocation selected an unrelated executable named dx and failed before compilation; subsequent builds used the
  installed Dioxus CLI explicitly.
- Rust formatting, dprint for the changed files it includes, and changelog format passed. The isolated pinned-parser
  delay checker inspected 420 delays with no missing rationale. The documentation page is excluded from dprint and
  received a manual prose check; no headings or links changed.
- The full Rust and browser batteries, native desktop execution, installer, provisioning, release and CI suites were
  skipped: the focused host safety, protocol, UI and browser checks cover this change's concrete risks. WebKit browser
  coverage represents the desktop engine family, not a native macOS app check. No instruction, heading or path change
  required the agent-instructions checker. No docs preview was started during this unattended incidental docs edit.
- The stack was rebased cleanly onto `844a7209` after all six credentials PRs landed. Independent audits read the
  complete upstream changes and checked host admission, recovery, protocol forwarding, host-list authority, Settings
  ownership, shared API code and CSS. No semantic conflict was found. Both PR deltas were unchanged by the rebase before
  the separate startup ownership correction. The UI unit, CSS and browser results above are reused from their identified
  pre-rebase source: credential warnings do not change connected-host authority, Settings does not remount Trash, and no
  Trash algorithm, selector, modal or animation changed. The startup correction changes ownership retention only; fresh
  host tests cover it. No browser rebuild or rerun was needed for those preserved boundaries.

## Review gate

Each PR passed the required single fresh-context gpt-6.1-sol review at high effort, with the full test-authoring
checklist supplied. The host reviewer found two admission gaps, including canonical paths after an ancestor move; both
were corrected and the same reviewer cleared the final delta. A separate scope reassessment found no unnecessary
subsystem. A rebase audit also identified startup recovery's missing owner reference for in-process cancellation; the
correction and its real database-boundary regression passed the same host reviewer's recheck. The shipped process
already retained ownership through runtime shutdown. The UI reviewer found the uncertainty and removed-host notice
issues described above; both were corrected and cleared on recheck. Each PR's final text passed a valid fresh cold read.
A separate documentation pass covered every touched file.
