# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

Collection record for the abbreviated review through `00de0b6c`:
[complete findings, candidates, withdrawals, leads and returned proposals](records/review-through-00de0b6c/README.md).
This record is bookkeeping, not an additional feedback item. Provisional buckets preserve unresolved stronger
consequences.

## Highest priority: security or data loss

- [`template-delete-can-target-unshown-editor.md`](template-delete-can-target-unshown-editor.md) — Delete can remove a
  different template from the one shown. The old button can also suppress Undo.

- [`stale-permanent-delete-can-remove-repaired-template.md`](stale-permanent-delete-can-remove-repaired-template.md) — A
  stale Delete permanently button can remove a repaired revision of the same template without offering Undo. The
  specification’s acceptance of that exact case is disputed.

- [`test-state-guards-accept-replaceable-parent.md`](test-state-guards-accept-replaceable-parent.md) — With some shared
  temporary directories, the ownership check can accept a test-state path another account can replace.

- [`deflake-run-consumers-skip-state-ownership-check.md`](deflake-run-consumers-skip-state-ownership-check.md) — Deflake
  Stop can trust run state from a location Start would refuse and affect a process outside that sweep.

- [`settings-close-can-save-unlisted-drafts.md`](settings-close-can-save-unlisted-drafts.md) — Save and Close can write
  an unlisted form and clear the saved alert token outside the user’s confirmation.

- [`settings-queued-save-can-commit-later-draft.md`](settings-queued-save-can-commit-later-draft.md) — A queued Settings
  save can commit later edits to an approved form, including changes that clear a saved alert token.

- [`settings-footer-can-save-the-wrong-page.md`](settings-footer-can-save-the-wrong-page.md) — A stale Settings footer
  can save the wrong page’s draft and overwrite the stored Tensorlake key.

- [`launcher-resize-changes-enter-action.md`](launcher-resize-changes-enter-action.md) — Resizing Replace with can make
  Enter delete the source session while the visible template choice still offers to fill the form.

- [`launcher-recent-enter-before-display.md`](launcher-recent-enter-before-display.md) — Moving the highlight to a
  recent setup can arm Replace before the new Enter action appears, allowing deletion under the earlier template cue.

- [`hook-save-uses-newer-replace-warning.md`](hook-save-uses-newer-replace-warning.md) — Set up and launch can stop and
  delete a source that became live while hook settings were saving, without a new answer from the user.

- [`hook-settings-toggle-uses-unshown-state.md`](hook-settings-toggle-uses-unshown-state.md) — Space can save the
  all-hosts hook opt-in while the switch still shows the on state the user intended to turn off.

- [`grok-manual-hook-replacement-omitted.md`](grok-manual-hook-replacement-omitted.md) — Enabling Grok hook setup can
  replace local edits in the file the previous Farhelm guide asked users to create. The setup prompt leaves that
  consequence unstated.

- [`antigravity-hook-removal-can-delete-extra-files.md`](antigravity-hook-removal-can-delete-extra-files.md) — Settings
  directs users to remove Antigravity’s entire plugin directory even though setup manages two files. Any other files in
  that directory would also be removed.

- [`failed-restart-clears-goose-resume-target.md`](failed-restart-clears-goose-resume-target.md) — A failed named Goose
  Restart can leave the saved resume target cleared if Farhelm also cannot restore the previous outcome. The client only
  sees the launch error.

- [`replace-can-submit-before-checkout-warning.md`](replace-can-submit-before-checkout-warning.md) — Replace with can
  submit a fresh checkout before its updated warning is shown, allowing the previous checkout to move to trash without
  the displayed disclosure.

- [`fresh-checkout-trust-can-bind-replacement-directory.md`](fresh-checkout-trust-can-bind-replacement-directory.md) —
  Farhelm can grant saved Claude trust to a replacement directory after a configured checkout step changes the path that
  was originally checked.

## High priority: material UX degradation

- [`tensorlake-lifecycle-falls-back-to-unowned-name.md`](tensorlake-lifecycle-falls-back-to-unowned-name.md) — If
  Tensorlake setup refuses a sandbox name already in use, the failed Farhelm host can still offer Suspend or Resume and
  act on the existing sandbox.

- [`template-leave-prompts-can-stack.md`](template-leave-prompts-can-stack.md) — Two outstanding leave questions can
  cause the remaining answer to save or discard a different template draft.

- [`antigravity-trust-warning-omitted-from-accessibility.md`](antigravity-trust-warning-omitted-from-accessibility.md) —
  Screen-reader users can enable automatic trust without access to the visible permission and trust-list warnings.

- [`approval-omits-automatic-trust-warning-from-accessibility.md`](approval-omits-automatic-trust-warning-from-accessibility.md)
  — Screen-reader users can miss the trusted-workspace status and warning on a launch or Restart approval.

- [`stale-host-retry-can-continue-sandbox-delete.md`](stale-host-retry-can-continue-sandbox-delete.md) — A Retry still
  shown as reconnecting can continue a sealed sandbox deletion if the failed deletion state arrives before the menu
  updates.

- [`settings-footer-can-discard-the-wrong-page.md`](settings-footer-can-discard-the-wrong-page.md) — A save refusal can
  switch pages before a stale Discard press runs, making it discard a different draft.

- [`settings-close-can-miss-detached-credential-save.md`](settings-close-can-miss-detached-credential-save.md) —
  Settings can close without waiting for a credential save whose editor is already closed.

- [`settings-close-can-miss-conflicting-tensorlake-keys.md`](settings-close-can-miss-conflicting-tensorlake-keys.md) —
  Save and Close can skip the conflicting-key check and write a second Tensorlake key after the first save completes.

- [`settings-close-can-retry-refused-save.md`](settings-close-can-retry-refused-save.md) — Save and Close can miss one
  pending refusal and retry the rejected draft without a new decision.

- [`settings-close-can-save-newly-staged-codex-login.md`](settings-close-can-save-newly-staged-codex-login.md) — A file
  read can change the Codex login draft while the Save and Close question is open, leaving the chosen file unreviewed
  before it is saved.

- [`settings-close-can-hide-credential-delivery-warnings.md`](settings-close-can-hide-credential-delivery-warnings.md) —
  Save and Close can hide which hosts failed to receive a saved credential replacement.

- [`launcher-hidden-recent-remains-enter-target.md`](launcher-hidden-recent-remains-enter-target.md) — A recent setup
  can scroll out of view while Enter still uses it to submit the launcher.

- [`launcher-underlining-can-stall-search.md`](launcher-underlining-can-stall-search.md) — Underlining matches in a long
  saved wrapper can do enough work on the GUI thread to slow launcher search.

- [`hook-save-launches-into-later-folder.md`](hook-save-launches-into-later-folder.md) — Set up and launch can start a
  session in a default folder that resolves after the user answers.

- [`launch-without-hook-choice-lost.md`](launch-without-hook-choice-lost.md) — A launch accepted with Launch without can
  still install hooks after the user enables setup for future launches while provisioning continues.

- [`terminal-link-panel-abbreviates-destination.md`](terminal-link-panel-abbreviates-destination.md) — The terminal link
  panel can abbreviate the destination it will open, with no way to read the complete URL before opening it.

- [`suspend-mixes-list-and-refresh-observations.md`](suspend-mixes-list-and-refresh-observations.md) — Suspend can use
  an old idle session list alongside a newer successful refresh and skip asking before it freezes active work.

- [`hook-settings-close-hides-opt-out-failure.md`](hook-settings-close-hides-opt-out-failure.md) — Closing Settings
  while an opt-out saves can hide the refusal and leave automatic hook setup active.

- [`wayland-ssh-paste-blocks-window.md`](wayland-ssh-paste-blocks-window.md) — Pasting an SSH answer on Wayland can
  block the GUI while another application supplies clipboard data.

## Other: correctness, diagnostics, cleanup, or convenience

- [`browser-shard-failure-sweeps-running-siblings.md`](browser-shard-failure-sweeps-running-siblings.md) — An early
  shard exception can clean up workers that sibling shards are still using. Additional reviewed scope: A Rust runner
  infrastructure or local evidence-save exception can sweep sibling sandboxes before their work and evidence collection
  settle.

- [`browser-shard-failures-exit-successfully.md`](browser-shard-failures-exit-successfully.md) — The browser harness can
  exit successfully even when its shards report validation failures. Additional reviewed scope: The Rust sandbox command
  can exit successfully even when its shard results say tests failed.

- [`worker-evidence-deleted-after-download-failure.md`](worker-evidence-deleted-after-download-failure.md) — The harness
  can delete a failed worker's evidence even when downloading that evidence failed.

- [`worker-allocation-precedes-ownership-ledger.md`](worker-allocation-precedes-ownership-ledger.md) — A created worker
  can fall outside cleanup when ledger writing fails or remote creation succeeds before the local request times out.

- [`successful-suspend-flashes-connecting.md`](successful-suspend-flashes-connecting.md) — A confirmed sandbox
  suspension can briefly change back to Connecting during its status recheck, even though the sandbox remains suspended.

- [`sandbox-termination-poll-exceeds-phase-deadline.md`](sandbox-termination-poll-exceeds-phase-deadline.md) — Sandbox
  deletion can exceed its stated termination-polling allowance because each lookup receives a fresh timeout beyond the
  phase's remaining time.

- [`sandbox-status-diagnosis-bypasses-cleaning.md`](sandbox-status-diagnosis-bypasses-cleaning.md) — Accepted sandbox
  failure descriptions bypass normal cleaning and redaction before reaching clients. An actual provider description
  containing a credential remains unverified after investigation.

- [`unreadable-sandbox-ssh-key-reports-ready.md`](unreadable-sandbox-ssh-key-reports-ready.md) — A damaged or unreadable
  saved SSH key pair can still be presented as ready, postponing the failure until later sandbox operations.

- [`suspended-plan-test-erases-its-suspension-premise.md`](suspended-plan-test-erases-its-suspension-premise.md) — The
  suspended-host setup test changes installation paths and erases its own Suspended state before checking the guard. The
  resulting failing scheduler ordering remains unverified after investigation.

- [`tunnel-fixture-assumes-simple-short-temporary-root.md`](tunnel-fixture-assumes-simple-short-temporary-root.md) — The
  tunnel transport tests can reject correct behavior when the temporary root needs path escaping or is too long for SSH
  sharing.

- [`redirect-regression-omits-location-header.md`](redirect-regression-omits-location-header.md) — The redirect
  regression does not supply a redirect destination, so it cannot prove that Farhelm refuses to follow one.

- [`package-repair-does-not-wait-for-dpkg-lock.md`](package-repair-does-not-wait-for-dpkg-lock.md) — The repair step
  does not share the package installer's lock-wait policy and could fail under contention. A supported concurrent
  package-manager trigger remains unverified after investigation.

- [`rust-sandbox-runner-passes-empty-selection.md`](rust-sandbox-runner-passes-empty-selection.md) — The Rust sandbox
  runner can announce all shards passed while running no tests, including with zero or nonnumeric shard counts and an
  empty overall filter selection.

- [`rust-sandbox-pack-ignores-cargo-target-dir.md`](rust-sandbox-pack-ignores-cargo-target-dir.md) — A checkout built
  only into a shared or external Cargo target directory cannot package its Rust sandbox tests: the runner still requires
  its fixture binary under the default target path.

- [`rust-sandbox-staging-misquotes-checkout-path.md`](rust-sandbox-staging-misquotes-checkout-path.md) — The Rust
  sandbox runner cannot stage an ordinary checkout whose absolute path contains spaces, because nested remote shell
  commands interpolate that path without quoting it.

- [`trailing-http-golden-omits-populated-environment.md`](trailing-http-golden-omits-populated-environment.md) — The new
  environment host JSON is omitted from every golden contract row. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-agent-environment-missing-local-ssh-key-preflight.md`](trailing-agent-environment-missing-local-ssh-key-preflight.md)
  — Sandbox agent launches ask for approval before checking the local SSH key. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-history-admits-agent-origin.md`](trailing-environment-history-admits-agent-origin.md) — Agent
  environment launches enter GUI place-default and recent-history bookkeeping. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-sealed-delete-comment-hold-versus-seal-record.md`](trailing-sealed-delete-comment-hold-versus-seal-record.md)
  — Deletion test comment wrongly forbids releasing the failed host's connection hold. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-shared-suspended-view-doc-retains-tensorlake-only-contract.md`](trailing-shared-suspended-view-doc-retains-tensorlake-only-contract.md)
  — Suspended view documentation retains the known Tensorlake-only vocabulary. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ordinary-tensorlake-add-keeps-source-mutex-through-ssh-readiness.md`](trailing-ordinary-tensorlake-add-keeps-source-mutex-through-ssh-readiness.md)
  — Ordinary Add retains source exclusion during post-Running SSH readiness. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-sealed-setup-test-overlay-hides-hold-and-run-leak.md`](trailing-sealed-setup-test-overlay-hides-hold-and-run-leak.md)
  — Sealed-setup regression test cannot observe its claimed no-hold boundary. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ordinary-tensorlake-add-outer-source-lock-across-ssh-readiness.md`](trailing-ordinary-tensorlake-add-outer-source-lock-across-ssh-readiness.md)
  — Ordinary Add holds recipe source exclusion while waiting for SSH. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-provisioning-shared-environment-preparation-progress.md`](trailing-provisioning-shared-environment-preparation-progress.md)
  — Environment preparation stays on Select the recipe source in launch progress. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-rust-worker-trims-explicit-payload-directory.md`](trailing-rust-worker-trims-explicit-payload-directory.md)
  — Archive packer changes an accepted explicit payload directory. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`stop-and-suspend-hides-timeout-reason.md`](stop-and-suspend-hides-timeout-reason.md) — When Stop and suspend exceeds
  its reply deadline, Farhelm can leave you without the explanation that it is still working or that Stop already
  failed.

- [`shell-stop-suspend-error-names-agent.md`](shell-stop-suspend-error-names-agent.md) — A partial Stop and suspend
  failure can say an agent stopped after stopping Bash or Zsh.

- [`template-leave-spec-still-inline.md`](template-leave-spec-still-inline.md) — The implementation spec still describes
  the Templates leave question as inline, although the app centers it.

- [`launcher-model-field-can-reinterpret-show-all.md`](launcher-model-field-can-reinterpret-show-all.md) — An arriving
  model catalog can turn Enter on the highlighted Show all option into a model selection.

- [`legacy-template-can-ask-to-save-automatic-cleanup.md`](legacy-template-can-ask-to-save-automatic-cleanup.md) — A
  legacy template can ask to save when the only remaining change is automatic cleanup of a retired trust field.

- [`ensure-hosts-repair-omits-settings-loss.md`](ensure-hosts-repair-omits-settings-loss.md) — The repair message omits
  that removing and re-adding a staged host forgets its saved organization and settings.

- [`sealed-container-delete-spec-conflicts.md`](sealed-container-delete-spec-conflicts.md) — The product and
  implementation specs disagree about where a late sealed container deletion failure is reported.

- [`tensorlake-key-clear-spec-misstates-leftover-refresh.md`](tensorlake-key-clear-spec-misstates-leftover-refresh.md) —
  The implementation spec says clearing the Tensorlake API key refreshes the leftovers list. The app clears and
  invalidates that list instead.

- [`antigravity-recovery-republishes-old-trust-status.md`](antigravity-recovery-republishes-old-trust-status.md) — A
  recovered terminal can initially show Antigravity’s old trust status and date the first fresh screen as new activity.

- [`antigravity-restart-can-send-stale-trust-alert.md`](antigravity-restart-can-send-stale-trust-alert.md) — A refresh
  during Restart can send a pending trust alert after the Antigravity process that asked has stopped.

- [`antigravity-trust-falls-back-to-uncanonical-folder.md`](antigravity-trust-falls-back-to-uncanonical-folder.md) — If
  the folder can no longer be resolved or encoded during launch preparation, the Antigravity trust setting can use the
  submitted spelling.

- [`antigravity-dismissal-can-hide-next-trust-hint.md`](antigravity-dismissal-can-hide-next-trust-hint.md) — A delayed
  close from an earlier Antigravity trust hint can hide a later hint in the same session.

- [`antigravity-trust-hint-omits-live-announcement.md`](antigravity-trust-hint-omits-live-announcement.md) — The new
  Antigravity trust hint emits no live announcement request for screen readers.

- [`unavailable-antigravity-trust-switch-announced-as-off.md`](unavailable-antigravity-trust-switch-announced-as-off.md)
  — Accessibility metadata reports an unread trust setting as off without marking its switch unavailable.

- [`antigravity-trust-link-can-hide-read-failure.md`](antigravity-trust-link-can-hide-read-failure.md) — When the trust
  setting cannot be read, the hint’s Settings link can leave the unavailable switch and its error below the fold.

- [`antigravity-trust-dismissal-lost-during-recovery.md`](antigravity-trust-dismissal-lost-during-recovery.md) — A
  terminal recovery can bring back an Antigravity trust hint the user already dismissed for the same question.

- [`launcher-alias-enter-can-apply-an-undrawn-model.md`](launcher-alias-enter-can-apply-an-undrawn-model.md) — Enter can
  fill in an agent and model from a new family alias while the launcher still says models are loading.

- [`installed-agent-hook-preflight-accepts-incomplete-setup.md`](installed-agent-hook-preflight-accepts-incomplete-setup.md)
  — The installed-agent check can report agent drift when its manual conversation hook is configured for the wrong
  event.

- [`installed-agent-teardown-hides-test-failure.md`](installed-agent-teardown-hides-test-failure.md) — A teardown
  failure can hide the original launch or Restart failure in the installed-agent check.

- [`drift-suite-lacks-blocking-update-classification.md`](drift-suite-lacks-blocking-update-classification.md) — The
  drift guide calls for an unavailable verdict on a blocking CLI update, but the suite has no update-specific
  classification. The exact verdict for the documented Codex screen has not been established.

- [`installed-agent-readiness-oracle-is-too-weak.md`](installed-agent-readiness-oracle-is-too-weak.md) — Launch-only
  checks can pass on identifying text without establishing that the agent is interactive.

- [`installed-agent-turn-oracle-accepts-old-output.md`](installed-agent-turn-oracle-accepts-old-output.md) — The
  first-turn check can credit text that was already visible before its prompt was submitted.

- [`installed-agent-runner-can-leave-interrupted-build-running.md`](installed-agent-runner-can-leave-interrupted-build-running.md)
  — Stopping only the installed-agent runner during the preliminary build can leave Cargo running.

- [`installed-agent-preflight-can-misclassify-executable-access.md`](installed-agent-preflight-can-misclassify-executable-access.md)
  — Binary preflight can mark an inaccessible executable as available and misreport the resulting launch failure as
  drift.

- [`interrupted-drift-case-retains-failed-verdict.md`](interrupted-drift-case-retains-failed-verdict.md) — A cancelled
  case can leave a failed drift verdict in its retained evidence before the test binary starts.

- [`docs-shot-partial-failure-hides-success-paths.md`](docs-shot-partial-failure-hides-success-paths.md) — One failed
  page can hide the paths of sibling screenshots that were captured successfully and kept in the archive.

- [`invalid-boat-member-limit-clears-previews.md`](invalid-boat-member-limit-clears-previews.md) — An invalid Boat
  worker limit can erase selected previews and the publish record before any capture starts.

- [`docs-shots-share-host-details-state.md`](docs-shots-share-host-details-state.md) — A docs screenshot can show
  different host row details depending on which other shots run on the same worker.

- [`docs-shot-run-can-overwrite-earlier-failure-frame.md`](docs-shot-run-can-overwrite-earlier-failure-frame.md) — A
  later shot can overwrite the raw image retained for a failed capture, leaving its error linked to the wrong frame.

- [`manual-update-check-can-be-consumed-by-automatic-skip.md`](manual-update-check-can-be-consumed-by-automatic-skip.md)
  — A manual update request can disappear when it joins a background check whose policy decision already skipped
  installation.

- [`settings-shows-unreadable-updater-as-on.md`](settings-shows-unreadable-updater-as-on.md) — Settings can still show
  automatic app updates as on when the updater has paused them because preferences are unreadable.

- [`menu-shortcuts-omitted-from-native-accessibility.md`](menu-shortcuts-omitted-from-native-accessibility.md) — The new
  menu shortcut labels do not reach the native accessibility shortcut fields.

- [`feedback-external-link-cue-is-visual-only.md`](feedback-external-link-cue-is-visual-only.md) — The feedback dialog’s
  Discord link omits the browser-opening cue from its accessible description.

- [`upgrade-highlights-feed-can-outgrow-desktop-reader.md`](upgrade-highlights-feed-can-outgrow-desktop-reader.md) — The
  cumulative highlights feed can exceed the desktop reader’s limit while still passing the release promotion check.

- [`upgrade-highlights-can-open-after-sign-in.md`](upgrade-highlights-can-open-after-sign-in.md) — Upgrade highlights
  can take focus after a startup SSH question has already opened and closed.

- [`unshown-upgrade-highlights-can-be-recorded-seen.md`](unshown-upgrade-highlights-can-be-recorded-seen.md) — Escape or
  Enter can mark upgrade highlights seen before they appear on screen.

- [`upgrade-highlights-opt-out-refusal-is-hidden.md`](upgrade-highlights-opt-out-refusal-is-hidden.md) — A refused Don’t
  show again save can close the highlights dialog without telling the user.

- [`release-gate-regeneration-guidance-stale.md`](release-gate-regeneration-guidance-stale.md) — The release workflow’s
  maintainer instructions can leave the generated workflow stale for the next tag.

- [`settings-close-save-can-hide-notification-refusal.md`](settings-close-save-can-hide-notification-refusal.md) — Save
  and Close can take the user to Notifications while leaving the relevant error below the viewport.

- [`settings-close-save-can-hide-credential-error.md`](settings-close-save-can-hide-credential-error.md) — Settings can
  keep a failed credential save’s explanation behind the Credentials overview.

- [`settings-close-save-stalls-silently-during-tests.md`](settings-close-save-stalls-silently-during-tests.md) — Save
  and Close can give up without explanation while a Notifications or Tensorlake test is running.

- [`settings-close-can-dismiss-a-newer-question.md`](settings-close-can-dismiss-a-newer-question.md) — A successful Save
  and Close can dismiss a newer unanswered tunnel-permission question.

- [`tensorlake-key-discard-lacks-preservation-test.md`](tensorlake-key-discard-lacks-preservation-test.md) — The new
  Tensorlake Discard coverage does not prove that abandoning a replacement keeps the saved key.

- [`ntfy-test-no-longer-witnesses-pending-save-lock.md`](ntfy-test-no-longer-witnesses-pending-save-lock.md) — The
  changed Notifications test can pass if Save wrongly becomes available on an edited form while Send test is pending.

- [`sound-preview-overlay-blocks-settings-switch.md`](sound-preview-overlay-blocks-settings-switch.md) — The Play
  overlay leaves parts of its sound switch row unclickable.

- [`golden-runner-bypasses-cargo-runtime-setup.md`](golden-runner-bypasses-cargo-runtime-setup.md) — Custom screenshot
  builds can succeed but fail to run when they depend on Cargo’s runner or shared-library setup.

- [`focused-golden-update-replaces-unpublished-full-captures.md`](focused-golden-update-replaces-unpublished-full-captures.md)
  — A valid focused golden update can overwrite a complete set of unpublished captures.

- [`golden-runner-misdecodes-cargo-json-paths.md`](golden-runner-misdecodes-cargo-json-paths.md) — A successful golden
  build can fail before rendering when Cargo reports a path the runner decodes or splits incorrectly.

- [`golden-output-backpressure-can-block-cancellation-cleanup.md`](golden-output-backpressure-can-block-cancellation-cleanup.md)
  — Blocked parallel render output can delay cancellation cleanup and retain the update lock.

- [`failed-golden-reference-setup-can-leave-partial-files.md`](failed-golden-reference-setup-can-leave-partial-files.md)
  — A failed reference fetch can strand partial files from a golden comparison that never started rendering.

- [`staged-host-update-controls-promise-unattended-installs.md`](staged-host-update-controls-promise-unattended-installs.md)
  — Host update controls can still promise automatic installation while a staged build requires a manual update.

- [`update-accessible-names-can-collide-across-host-kinds.md`](update-accessible-names-can-collide-across-host-kinds.md)
  — An SSH host and a container with the same display name can still have identical accessible Update labels.

- [`sound-preview-regression-misses-separate-mappings.md`](sound-preview-regression-misses-separate-mappings.md) — The
  preview test exercises one sound and can miss a wrong mapping on the three other Play controls.

- [`recipe-tunnel-edit-marks-only-card.md`](recipe-tunnel-edit-marks-only-card.md) — A recipe with multiple tunnels
  marks only the card as edited, leaving the changed row and field unidentified.

- [`recipe-delete-test-misses-warning-clipping.md`](recipe-delete-test-misses-warning-clipping.md) — The recipe Delete
  regression checks that the prompt box fits without verifying that its warning text remains visible.

- [`app-settings-shared-upgrade-option-has-local-heading.md`](app-settings-shared-upgrade-option-has-local-heading.md) —
  The App heading says This Mac even though it contains a helm-shared highlights setting.

- [`settings-radio-click-can-use-clipped-bounds.md`](settings-radio-click-can-use-clipped-bounds.md) — The new Settings
  card geometry changes where an accessibility activation is aimed when a radio row is partly offscreen. A failure on a
  supported native adapter remains unverified.

- [`container-leftover-confirmation-host-identity-uncertain.md`](container-leftover-confirmation-host-identity-uncertain.md)
  — A registered-container cleanup confirmation passes the displayed host number. Whether it can reach a different
  installation after that number’s identity changes remains unverified.

- [`launcher-escape-discards-template-edits.md`](launcher-escape-discards-template-edits.md) — Escape can discard Save
  as template edits while the launcher still shows the search results that received the key.

- [`launcher-incomplete-template-banner-misplaced.md`](launcher-incomplete-template-banner-misplaced.md) — The
  incomplete-template message appears below the form and asks the user to complete the form below it.

- [`quick-launch-placement-contract-omits-local-host.md`](quick-launch-placement-contract-omits-local-host.md) — The
  spec promises New's placement defaults, while quick launch beside a disposable session deliberately defaults to the
  helm's own host.

- [`launcher-tab-order-omits-command-message.md`](launcher-tab-order-omits-command-message.md) — The specified Tab order
  omits a command-message stop that can appear before the form.

- [`podman-launch-lifetime-notice-hidden.md`](podman-launch-lifetime-notice-hidden.md) — The launcher only explains in
  Host help that deleting the last session removes a new container and its files.

- [`launcher-held-host-name-clipped.md`](launcher-held-host-name-clipped.md) — Replace with and Restart with can clip
  the source host's name with no visual way to reveal its full spelling.

- [`launcher-shell-group-unlabelled.md`](launcher-shell-group-unlabelled.md) — The launcher separates Bash and Zsh from
  agent choices with a divider that exposes no accessible Shells label.

- [`stacked-template-shot-does-not-check-effort.md`](stacked-template-shot-does-not-check-effort.md) — The
  stacked-template capture can pass without verifying that the selected effort matches its caption.

- [`shell-release-shot-parks-on-removed-control.md`](shell-release-shot-parks-on-removed-control.md) — The shell
  release-note recapture tries to park on a launcher title that the form no longer draws.

- [`antigravity-release-shot-waits-on-folded-field.md`](antigravity-release-shot-waits-on-folded-field.md) — The
  Antigravity release-note recapture waits for a conversation field that stays folded under More options.

- [`container-comparison-targets-removed-control.md`](container-comparison-targets-removed-control.md) — The container
  comparison scene targets the removed Runs on control and cannot reach the new Host menu route.

- [`launcher-comparison-heading-selector-case.md`](launcher-comparison-heading-selector-case.md) — Launcher comparison
  scenes wait for mixed-case heading text while the launcher paints it in uppercase.

- [`launcher-host-picker-omits-expanded-state.md`](launcher-host-picker-omits-expanded-state.md) — The Host picker gives
  assistive technology no way to tell whether Enter selects a destination or tries to launch.

- [`vanished-recent-traps-launcher-enter.md`](vanished-recent-traps-launcher-enter.md) — When the highlighted recent
  setup disappears, Enter can stay bound to the missing choice and do nothing.

- [`launcher-recent-highlight-mutes-yolo-cue.md`](launcher-recent-highlight-mutes-yolo-cue.md) — Selecting a recent
  setup replaces its red YOLO styling with ordinary highlight styling, leaving only the text cue.

- [`launcher-recent-accepts-modified-return.md`](launcher-recent-accepts-modified-return.md) — The new launcher accepts
  modified Return on highlighted recent setups in addition to the displayed Return shortcut.

- [`launcher-reconnect-discards-search-selection.md`](launcher-reconnect-discards-search-selection.md) — A reconnect can
  clear the selected search result and make Enter apply a different result.

- [`launcher-highlights-escape-notation.md`](launcher-highlights-escape-notation.md) — The launcher can underline the
  escape notation displayed for a path even when that notation was absent from the matching path.

- [`launcher-host-reorder-test-accepts-old-host.md`](launcher-host-reorder-test-accepts-old-host.md) — The Host menu
  reorder test can pass without demonstrating that Enter selected the visible host.

- [`launcher-tab-test-omits-placement-premise.md`](launcher-tab-test-omits-placement-premise.md) — The launcher test for
  all placements can pass when its fixture omits an environment offer.

- [`launcher-undo-leaves-template-source-mark.md`](launcher-undo-leaves-template-source-mark.md) — Undo can restore the
  earlier launch settings and leave the reverted template marked as their source.

- [`launcher-template-rows-rebuild-draft.md`](launcher-template-rows-rebuild-draft.md) — The wide launcher rebuilds the
  same draft data for every template row while it checks held fields.

- [`quick-launch-loses-template-source-mark.md`](quick-launch-loses-template-source-mark.md) — A launcher opened for
  review or correction by quick launch can omit the source marker for the template that filled it.

- [`launcher-template-row-edits-behind-save-panel.md`](launcher-template-row-edits-behind-save-panel.md) — A template
  row can change the launcher draft while Save as template remains open with the older values.

- [`launcher-disabled-setup-enter-gives-no-reason.md`](launcher-disabled-setup-enter-gives-no-reason.md) — Enter on a
  disabled setup can do nothing without showing the refusal reason to a keyboard user.

- [`launcher-recent-fill-option-named-replace.md`](launcher-recent-fill-option-named-replace.md) — Recent setup options
  can announce Replace while activating the option only fills the form.

- [`launcher-typed-recent-mislabels-enter.md`](launcher-typed-recent-mislabels-enter.md) — Enter on a typed recent
  result only fills the form even though its key cue says Replace.

- [`launcher-recent-hover-can-follow-old-row.md`](launcher-recent-hover-can-follow-old-row.md) — A recent setup can
  briefly show hover help for the previous setup in its list position.

- [`launcher-announces-two-selected-setups.md`](launcher-announces-two-selected-setups.md) — The launcher can report
  both the setup that filled the form and the current keyboard target as selected.

- [`golden-worker-interrupt-bypasses-cancel.md`](golden-worker-interrupt-bypasses-cancel.md) — A screenshot worker
  interrupted on its own can leave the runner packaging a partial result or certifying an update after the review worker
  was interrupted.

- [`golden-reference-failure-hides-cached-images.md`](golden-reference-failure-hides-cached-images.md) — One unavailable
  screenshot reference can hide other originals that were already cached.

- [`golden-report-packages-partial-capture.md`](golden-report-packages-partial-capture.md) — A failed screenshot
  renderer can leave a partial PNG that the changes page labels as a new capture.

- [`golden-report-omits-failed-scenes.md`](golden-report-omits-failed-scenes.md) — A portable screenshot report can omit
  failed scenes without telling its reviewer that the run was incomplete.

- [`notifications-act-on-unshown-list.md`](notifications-act-on-unshown-list.md) — Closing Notifications or clearing
  resolved entries can act on notices the window has drawn but has yet to show.

- [`orphan-delete-promises-unadmitted-cleanup.md`](orphan-delete-promises-unadmitted-cleanup.md) — Sandbox Delete can
  report cleanup in progress before the helm has accepted it, so a later admission failure never appears when Settings
  refreshes.

- [`restart-cleanup-comment-misstates-record-order.md`](restart-cleanup-comment-misstates-record-order.md) — The Restart
  cleanup comment promises a host record survives a failed sweep even though removal happens first.

- [`restart-failure-hides-unconfirmed-cleanup.md`](restart-failure-hides-unconfirmed-cleanup.md) — After a failed
  replacement, Restart can restore the previous run and show only the launch error while cleanup of the replacement
  scope is unconfirmed.

- [`sidebar-retry-warning-missing-accessible-name.md`](sidebar-retry-warning-missing-accessible-name.md) — A retrying
  host's warning appears as an amber edge without an equivalent in the session row's accessible description.

- [`exited-shell-suspend-confirmation-wording.md`](exited-shell-suspend-confirmation-wording.md) — Stop and suspend can
  still tell users it will stop a shell that has already exited.

- [`terminal-steady-block-comment-stale.md`](terminal-steady-block-comment-stale.md) — The terminal sender's comment
  describes a blinking cursor even though the updated contract specifies steady block behavior.

- [`shift-enter-test-assumes-one-program-read.md`](shift-enter-test-assumes-one-program-read.md) — The Shift+Enter
  integration check assumes the program receives the key in one read, which the terminal host contract leaves to the PTY
  and program.

- [`terminal-primary-selection-hint-delayed.md`](terminal-primary-selection-hint-delayed.md) — A checkpoint can delay
  the terminal's selection notice and leave stale drag guidance visible.

- [`terminal-prehello-test-misses-output-loss.md`](terminal-prehello-test-misses-output-loss.md) — The terminal
  lifecycle test checks a substring and can pass when other expected output is missing at exit.

- [`capture-stop-retry-can-lose-starting-host.md`](capture-stop-retry-can-lose-starting-host.md) — Retrying cleanup
  after an interrupted capture start can remove the recovery directory before the host finishes starting.

- [`capture-close-setup-error-discards-frames.md`](capture-close-setup-error-discards-frames.md) — A shutdown socket
  setup failure can skip cleanup and discard already staged agent screens.

- [`capture-drive-error-discards-staged-screens.md`](capture-drive-error-discards-staged-screens.md) — A capture
  controller error can discard previously verified frames even after shutdown handling runs.

- [`capture-fixture-copy-can-break-screen-pair.md`](capture-fixture-copy-can-break-screen-pair.md) — A fixture copy
  failure can damage the previous screen and title pair and discard the staged replacement.

- [`capture-stop-retry-requires-original-binary.md`](capture-stop-retry-requires-original-binary.md) — A retained
  capture cannot use the advertised stop retry when the exact build path disappears, even if a compatible executable is
  available.

- [`worker-refusal-can-copy-old-evidence.md`](worker-refusal-can-copy-old-evidence.md) — A test worker refusal can
  return evidence from an earlier invocation under the current run's output directory.

- [`release-verifier-retry-overruns-deadline.md`](release-verifier-retry-overruns-deadline.md) — The release verifier
  can start a retry whose action allowance extends past the shared checkpoint deadline.

- [`release-verifier-does-not-bind-relaunch-samples.md`](release-verifier-does-not-bind-relaunch-samples.md) — The
  release verifier can accept continuity samples without establishing that they span the app relaunch.

- [`centos-gate-can-stage-sibling-binary.md`](centos-gate-can-stage-sibling-binary.md) — The CentOS gate can stage a
  sibling checkout's supervisor from a shared Cargo output directory.

- [`centos-gate-resolves-relative-target-from-caller.md`](centos-gate-resolves-relative-target-from-caller.md) — The
  CentOS gate can verify the wrong file when Cargo output is relative and the command starts outside the checkout.

- [`release-verifier-can-downgrade-product-failure.md`](release-verifier-can-downgrade-product-failure.md) — Missing
  retry diagnostics can downgrade a recorded product failure to Could not run.

- [`agent-settings-search-omits-agent-name.md`](agent-settings-search-omits-agent-name.md) — Settings search can give
  different agents' pages or hook controls the same accessible name.

- [`hook-hint-settings-focus-return-mismatch.md`](hook-hint-settings-focus-return-mismatch.md) — Settings opened from a
  hook hint returns focus to the hint even though the Settings contract names the gear.

- [`pi-settings-tests-miss-trust-route.md`](pi-settings-tests-miss-trust-route.md) — Pi's Settings tests no longer check
  a successful trust read or the absence of a hook hint control.

- [`agent-settings-tests-miss-late-trust-replies.md`](agent-settings-tests-miss-late-trust-replies.md) — The per-agent
  Settings tests can miss a trust reply being applied to the page opened after the request.

- [`agent-settings-search-test-omits-focus-target.md`](agent-settings-search-test-omits-focus-target.md) — The Settings
  search test can pass with a trust result that names the right agent and focuses the wrong control.

- [`hook-failure-notice-lost-on-create-recovery.md`](hook-failure-notice-lost-on-create-recovery.md) — A recovered
  launch can keep its hook setup failure state while losing the notification that explains it.

- [`hook-silent-warning-lost-on-create-recovery.md`](hook-silent-warning-lost-on-create-recovery.md) — An agent that
  survives an interrupted create can permanently lose the warning that its installed hooks have gone silent.

- [`hook-hint-dismissal-survives-next-launch.md`](hook-hint-dismissal-survives-next-launch.md) — Dismissing a hook hint
  for one run can hide the hint on a later run of the same session.

- [`hook-hint-dismissal-survives-ui-reset.md`](hook-hint-dismissal-survives-ui-reset.md) — A local hook hint dismissal
  can survive reconnect even though the UI reset is supposed to clear it.

- [`hook-hint-dismissal-reply-loss-hides-all-hints.md`](hook-hint-dismissal-reply-loss-hides-all-hints.md) — A lost
  dismissal reply can hide every hint for that agent until the window closes.

- [`hook-hint-can-repaint-old-dismissal-error.md`](hook-hint-can-repaint-old-dismissal-error.md) — An old dismissal
  error can reappear after Settings has successfully restored the hint.

- [`hook-recovery-text-omits-direction-isolation.md`](hook-recovery-text-omits-direction-isolation.md) — Restart and
  recovery text escape host-supplied hook errors without isolating their text direction.

- [`launcher-model-list-loses-highlight-on-load.md`](launcher-model-list-loses-highlight-on-load.md) — Loading the model
  catalog can clear Enter's selection while the same model is still listed.

- [`container-golden-stays-on-loading.md`](container-golden-stays-on-loading.md) — The new-container golden settles on a
  loading status because its fixture never supplies the build result.

- [`launcher-loses-hook-offer-on-tab-switch.md`](launcher-loses-hook-offer-on-tab-switch.md) — Switching tabs during the
  hook settings read can suppress the setup offer while retaining the chosen agent.

- [`restart-recovery-shows-other-hook-result.md`](restart-recovery-shows-other-hook-result.md) — An interrupted Restart
  with can recover the previous terminal and display the hook setup result for a replacement that never started.

- [`failed-launch-clears-installed-hook-result.md`](failed-launch-clears-installed-hook-result.md) — A failed create can
  later say hook setup was skipped even though Farhelm wrote the persistent files.

- [`resume-off-command-gets-hook-warning.md`](resume-off-command-gets-hook-warning.md) — A command launch with Resume
  off can get a misleading silent-hook warning that an arriving report does not clear.

- [`launch-setup-recording-failure-looks-skipped.md`](launch-setup-recording-failure-looks-skipped.md) — A launch that
  verifies the hook files can later say setup was skipped if only the result write failed.

- [`held-escape-discards-launcher-draft.md`](held-escape-discards-launcher-draft.md) — On Wayland, holding Escape can
  discard the launcher draft after dismissing a highlighted result or the hook setup question.

- [`sandbox-recent-source-mark-follows-host.md`](sandbox-recent-source-mark-follows-host.md) — Changing the host can
  remove the mark from a still-listed sandbox setup while the form still names it as the source.

- [`launcher-host-search-rebuilds-registry-per-row.md`](launcher-host-search-rebuilds-registry-per-row.md) — Wide host
  search rebuilds the full host list for each matched row, increasing work as the fleet grows.

- [`terminal-paste-test-misses-first-ack-premise.md`](terminal-paste-test-misses-first-ack-premise.md) — The interrupted
  paste check can pass without demonstrating that the runtime processed the first acknowledgement.

- [`custom-command-spec-misstates-hook-arguments.md`](custom-command-spec-misstates-hook-arguments.md) — The
  custom-command contract says the launch placeholder supplies reporters for agents whose persistent configuration
  supplies them.

- [`uninstall-omits-persistent-hook-cleanup.md`](uninstall-omits-persistent-hook-cleanup.md) — Uninstall can leave Grok
  and Antigravity callbacks pointing to the removed Farhelm executable, and the uninstall guidance omits cleanup.

- [`hook-diagnostic-path-can-render-doc-link.md`](hook-diagnostic-path-can-render-doc-link.md) — A path inside a hook
  setup error can be shown as a Farhelm documentation link even though Farhelm supplied only diagnostic text.

- [`antigravity-drift-check-misses-plugin-manifest.md`](antigravity-drift-check-misses-plugin-manifest.md) — The
  Antigravity drift check can accept a callback file whose plugin manifest is missing.

- [`launcher-edit-hides-hook-save-refusal.md`](launcher-edit-hides-hook-save-refusal.md) — Editing the launcher while a
  hook setting saves can hide a later refusal.

- [`hook-hint-opens-unshown-agent-settings.md`](hook-hint-opens-unshown-agent-settings.md) — After automatic session
  selection, the focused hook hint can open the next agent's Settings page before that selection is visible.

- [`hook-removal-help-missing-from-accessibility.md`](hook-removal-help-missing-from-accessibility.md) — The Grok and
  Antigravity Settings pages can expose their setup switches without the persistence and removal guidance.

- [`model-picker-skips-hook-setup-offer.md`](model-picker-skips-hook-setup-offer.md) — Choosing an Antigravity model in
  the full Model picker skips the hook setup offer shown for the equivalent command-field choice.

- [`launcher-keyboard-recents-hide-custom-flags.md`](launcher-keyboard-recents-hide-custom-flags.md) — Keyboard users
  can no longer reveal the custom permission flags that distinguish otherwise identical recent setups.

- [`launcher-result-pick-leaves-approval-focus.md`](launcher-result-pick-leaves-approval-focus.md) — Picking a recent
  setup or template in the wide launcher can leave keyboard input focused on a separate approval card.

- [`hook-setup-accepts-nonutf8-config-path.md`](hook-setup-accepts-nonutf8-config-path.md) — Persistent hook setup can
  write to a non-UTF-8 configuration path and report success even though the path contract calls for refusing it.

- [`checkout-input-arms-hook-warning-early.md`](checkout-input-arms-hook-warning-early.md) — Enter during managed
  checkout preparation can make Farhelm warn about a missing hook report before the agent starts.

- [`lost-grok-record-shows-hook-setup-hint.md`](lost-grok-record-shows-hook-setup-hint.md) — Losing a saved Grok
  conversation can bring back setup advice even though Farhelm already received a successful hook report.

- [`hook-hint-omits-live-accessibility-notification.md`](hook-hint-omits-live-accessibility-notification.md) — The hook
  setup hint exposes readable text but does not request a live announcement when it appears while focus stays in the
  terminal.

- [`launcher-recent-highlight-omits-installation.md`](launcher-recent-highlight-omits-installation.md) — The launcher
  keys an armed recent setup by registry entry and setup, leaving cross-installation rebinding possible if matching
  history arrives without resetting that launcher.

- [`launcher-recent-highlight-can-change-path-spelling.md`](launcher-recent-highlight-can-change-path-spelling.md) — A
  refreshed recent setup can keep the same highlight while changing the literal folder spelling Enter will use.

- [`launcher-yolo-reply-uses-current-question.md`](launcher-yolo-reply-uses-current-question.md) — A permanent
  permission reply checks whichever launcher question is in flight. Whether a later question can receive the older
  launch permission remains unverified.

- [`fixture-privacy-checks-miss-link-local-addresses.md`](fixture-privacy-checks-miss-link-local-addresses.md) — The
  capture scrubber and committed fixture checks can miss link-local IPv4 endpoints. No actual disclosure was
  established.

- [`capture-test-python-minimum-undocumented.md`](capture-test-python-minimum-undocumented.md) — The standalone capture
  test gained a Python 3.11 dependency while its documented command still only names python3.

- [`environment-delete-title-unreachable-by-keyboard.md`](environment-delete-title-unreachable-by-keyboard.md) — A long
  target title can clip in an environment Delete confirmation where keyboard paging reaches the warning but cannot
  reveal the full title.

- [`pi-page-describes-wrong-trust-question.md`](pi-page-describes-wrong-trust-question.md) — Pi’s Settings description
  promises folder-trust behavior even though the setting approves project resources.

- [`hook-setting-read-can-miss-older-save.md`](hook-setting-read-can-miss-older-save.md) — Settings reopened after
  reconnect can read the old hook switch before an accepted save from the previous connection finishes.

- [`late-rename-persists-old-hook-advice.md`](late-rename-persists-old-hook-advice.md) — A late Rename reply can save
  the previous run’s hook advice in the helm cache until an authoritative refresh corrects it.

- [`hook-setup-can-report-partial-antigravity-plugin.md`](hook-setup-can-report-partial-antigravity-plugin.md) —
  Removing the Antigravity plugin directory during an accepted launch can leave it partly installed while Farhelm
  reports setup succeeded.

- [`hook-consent-summary-has-limited-keyboard-scroll.md`](hook-consent-summary-has-limited-keyboard-scroll.md) —
  Keyboard focus reveals the hook setup buttons without providing a separate way to scroll the notices. Whether this can
  hide the hook’s lasting effects in a supported window remains unverified.
