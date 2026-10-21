# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

## Highest priority: security or data loss

- [`same-destination-recovery-loses-trash-location.md`](same-destination-recovery-loses-trash-location.md) — Overlapping
  recovery with the same destination may hide retained checkout files from the trash; an affected platform is
  unverified.

- [`launch-pill-can-cover-link-disclosure.md`](launch-pill-can-cover-link-disclosure.md) — Launch status may hide a
  link's destination at activation; overlap and event timing are unverified.

- [`menu-focus-marker-exposes-native-address.md`](menu-focus-marker-exposes-native-address.md) — Desktop menu IDs
  disclose a native allocation address; usefulness to a separate exploit is unverified.

- [`refused-sequence-displaces-later-binding-snapshots.md`](refused-sequence-displaces-later-binding-snapshots.md) — A
  refused sequence may displace later Enter bindings; native event reachability remains unresolved.

- [`reset-search-agent-restores-other-custom-flags.md`](reset-search-agent-restores-other-custom-flags.md) — Reset and
  agent search can restore another agent's custom permission flags; harmful interpretation is unverified.

- [`restored-link-press-misses-intervening-drag.md`](restored-link-press-misses-intervening-drag.md) — A held press may
  activate a link after recovery misses dragging; native delivery and security impact are unverified.

- [`shortcut-save-overwrites-intervening-agent-binding.md`](shortcut-save-overwrites-intervening-agent-binding.md) — A
  shortcut save can restore an older target, including a pre-read agent edit after save failure; harmful launch is
  unverified.

- [`template-preservation-promise-omits-last-write-wins.md`](template-preservation-promise-omits-last-write-wins.md) —
  The agent-edit preservation promise omits accepted template overwrites; loss from reliance is conditional.

- [`guarded-preview-widens-source-deletion-consent.md`](guarded-preview-widens-source-deletion-consent.md) — A delayed
  Guarded preview can widen consent to delete a source session that has started running.

- [`header-yolo-answer-may-cross-question-opening.md`](header-yolo-answer-may-cross-question-opening.md) — An old answer
  could authorize a reopened header question; the complete shipped input route remains unestablished.

- [`superseding-launcher-may-retain-old-replace-source.md`](superseding-launcher-may-retain-old-replace-source.md) — A
  superseded launcher could retain an earlier replacement source; normal event reachability remains unresolved.

- [`numeric-token-regex-performance-remains-unresolved.md`](numeric-token-regex-performance-remains-unresolved.md) — A
  bounded numeric-token regex slowdown remains conditional on unverified browser-engine behavior and impact.

- [`repository-label-ambiguity-has-unresolved-selection-risk.md`](repository-label-ambiguity-has-unresolved-selection-risk.md)
  — Ambiguous repository labels have an unresolved harmful-selection interpretation; launch bypasses were refuted.

- [`changed-host-warning-flattens-compared-identities.md`](changed-host-warning-flattens-compared-identities.md) — A
  changed-host warning loses distinct identities and isolation; consequential reliance remains unestablished.

- [`escaped-start-remedy-changes-program-and-state-path.md`](escaped-start-remedy-changes-program-and-state-path.md) —
  The suggested start command changes path bytes; collateral inactive-installation harm remains conditional.

- [`respawn-reflow-crosses-launch-excerpt-boundary.md`](respawn-reflow-crosses-launch-excerpt-boundary.md) — Restart can
  retain preceding-run terminal text as a startup error; consequential confidentiality exposure remains unverified.

## High priority: material UX degradation

- [`shell-inspection-drains-exhaust-planning-pool.md`](shell-inspection-drains-exhaust-planning-pool.md) — Pipe-draining
  shell inspections can occupy every host-planning slot; an actual helper trigger is unverified.

- [`short-window-outage-blocks-pointer-recovery.md`](short-window-outage-blocks-pointer-recovery.md) — A short-window
  outage explanation may block pointer recovery and healthy-session navigation; keyboard and resize remain available.

- [`guarded-instead-can-duplicate-uncertain-launch.md`](guarded-instead-can-duplicate-uncertain-launch.md) — Guarded
  “instead” can duplicate an uncertain authorized launch; whether it promises one intent remains ambiguous.

- [`untouched-checkout-folder-save-undoes-cli-edit.md`](untouched-checkout-folder-save-undoes-cli-edit.md) — Saving an
  untouched checkout-folder field can overwrite a newer CLI setting.

- [`spare-checkout-rewind-keeps-removed-history.md`](spare-checkout-rewind-keeps-removed-history.md) — After an upstream
  rewind, a prepared checkout can successfully retain removed commits instead of the current upstream worktree.

## Other: correctness, diagnostics, cleanup, or convenience

- [`removed-host-strands-tunnel-listeners.md`](removed-host-strands-tunnel-listeners.md) — Removing a host leaves its
  tunnel ports occupied while individual close requests fail.

- [`menu-close-dismisses-new-notifications.md`](menu-close-dismisses-new-notifications.md) — Closing one popup may
  dismiss a newly opened notification list and mark it read; event ordering remains unverified.

- [`selection-history-plan-forgets-upgrade-selection.md`](selection-history-plan-forgets-upgrade-selection.md) — The
  visit-history plan's empty upgrade history conflicts with restoring the existing remembered session.

- [`download-interruption-promise-omits-crash-exception.md`](download-interruption-promise-omits-crash-exception.md) —
  The product's partial-download cleanup promise omits the newly accepted crash and force-quit exception.

- [`add-host-refusal-bypasses-dialog-admission.md`](add-host-refusal-bypasses-dialog-admission.md) — A delayed Add-host
  refusal may reopen a mutually inert dialog beside Settings; Escape remains available.

- [`browser-menu-focus-retains-evaluator-channels.md`](browser-menu-focus-retains-evaluator-channels.md) — Browser menu
  focus retains evaluator channels per request; practical memory impact is unmeasured.

- [`browser-suite-ignores-explicit-port.md`](browser-suite-ignores-explicit-port.md) — The browser suite silently
  ignores its documented manual port override.

- [`catalog-cleanup-can-start-untracked-test-session.md`](catalog-cleanup-can-start-untracked-test-session.md) —
  Releasing a held catalog during cleanup may start a test session after observation ends; scheduling is unverified.

- [`changed-template-test-allows-missing-retry-key.md`](changed-template-test-allows-missing-retry-key.md) — The
  changed-template regression can pass with no fresh retry key; no production omission is established.

- [`classic-mouse-snapshot-copy-blocks-later-links.md`](classic-mouse-snapshot-copy-blocks-later-links.md) — Copying in
  a classic mouse-report snapshot may leave web clicks inert; native behavior is unverified.

- [`delayed-refusal-reopens-cancelled-replace-question.md`](delayed-refusal-reopens-cancelled-replace-question.md) — A
  delayed refusal may reopen a cancelled Replace question; a further affirmative answer is still required.

- [`guarded-template-leaves-stale-yolo-question.md`](guarded-template-leaves-stale-yolo-question.md) — Applying a
  Guarded template can leave the previous YOLO question visible while submission still enforces the changed permissions.

- [`held-reveal-fixture-pins-before-cleanup.md`](held-reveal-fixture-pins-before-cleanup.md) — A preference-pin failure
  can bypass cleanup for an owned test session; the failure has not been injected.

- [`history-fixture-reset-races-selection-save.md`](history-fixture-reset-races-selection-save.md) — An automatic
  selection save may overwrite the history fixture's reset; adverse ordering is unverified.

- [`instruction-file-case-alias-evades-check.md`](instruction-file-case-alias-evades-check.md) — Case aliases evade the
  local instruction-file check; actual vendor lookup on case-insensitive filesystems is unverified.

- [`menu-blur-can-leave-delayed-arrow-current.md`](menu-blur-can-leave-delayed-arrow-current.md) — A blur without
  focus-in may leave delayed menu-arrow work current; browser event behavior is unverified.

- [`model-effort-test-never-proves-model-accepted.md`](model-effort-test-never-proves-model-accepted.md) — The
  model-effort regression can pass without committing its custom model; no product failure is established.

- [`normal-quit-misses-empty-download-directory.md`](normal-quit-misses-empty-download-directory.md) — A narrow
  normal-Quit race may leave an empty download staging folder; saved files are unaffected by this mechanism.

- [`notification-bell-removal-skips-focus-return.md`](notification-bell-removal-skips-focus-return.md) — Removing the
  notification bell may skip keyboard-focus return; browser fallback is unverified.

- [`notification-fixtures-register-ownership-too-late.md`](notification-fixtures-register-ownership-too-late.md) — A
  failed second fixture creation can leave the first notification-test session outside cleanup.

- [`pinned-host-dialog-chrome-can-hide-content.md`](pinned-host-dialog-chrome-can-hide-content.md) — The deleted dialog
  record still has an unresolved Host Settings footer-clipping case; exact geometry is unverified.

- [`prompt-prefill-test-opens-before-source-selection.md`](prompt-prefill-test-opens-before-source-selection.md) — The
  prompt-prefill and clone-defaults tests may open New before source selection; startup ordering is unverified.

- [`release-heading-can-close-review-page-script.md`](release-heading-can-close-review-page-script.md) — A malformed
  curated release heading can inject review-page script; attacker influence and practical impact are unverified.

- [`removal-destination-collapses-distinguishing-spaces.md`](removal-destination-collapses-distinguishing-spaces.md) —
  Removal and refusal paragraphs collapse spaces distinguishing registered destinations; live SSH examples are
  unverified.

- [`replacement-cleanup-test-never-proves-child-lived.md`](replacement-cleanup-test-never-proves-child-lived.md) — The
  replacement cleanup test never proves its stand-in was alive before checking absence; an actual false pass is
  unverified.

- [`secondary-press-suppresses-primary-selection-copy.md`](secondary-press-suppresses-primary-selection-copy.md) — A
  second mouse button may suppress primary-selection copying; physical browser behavior is unverified.

- [`selection-saves-omit-intermediate-visits.md`](selection-saves-omit-intermediate-visits.md) — Successful selection
  saves can omit intermediate visits from shared session history.

- [`session-view-fallback-omits-start-time.md`](session-view-fallback-omits-start-time.md) — The promised fallback for
  clipped hover details omits a session's separate start time.

- [`ssh-help-response-steals-approval-focus.md`](ssh-help-response-steals-approval-focus.md) — SSH help responses can
  take focus from approval answers; runtime ordering is unverified.

- [`timing-history-can-retain-failed-recorder-success.md`](timing-history-can-retain-failed-recorder-success.md) —
  Permanent timing history can retain recorder success after required evidence publication fails; failure injection is
  unexecuted.

- [`welcome-recovery-counts-unfinished-selection-read.md`](welcome-recovery-counts-unfinished-selection-read.md) — The
  welcome-recovery test may reject an extra legitimate preference read; adverse scheduling is unverified.

- [`approval-poll-http-await-exceeds-deadline.md`](approval-poll-http-await-exceeds-deadline.md) — A stalled HTTP
  response can outlive the approval test's deadline until the outer runner stops it.

- [`removed-deflake-items-leave-exclusions.md`](removed-deflake-items-leave-exclusions.md) — Removing two tracked flake
  items leaves their machine-readable sweep exclusions orphaned.

- [`landing-leaves-baseline-flake-unrecorded.md`](landing-leaves-baseline-flake-unrecorded.md) — A landing report defers
  recording a reproduced baseline flake; its private runtime evidence was not authenticated here.

- [`launcher-report-confuses-apply-with-launch.md`](launcher-report-confuses-apply-with-launch.md) — The landing report
  says Enter launches where the launcher actually applies queued templates to the draft.

- [`numeric-version-guard-hides-artifact-filenames.md`](numeric-version-guard-hides-artifact-filenames.md) — Some bare
  artifact filenames lose file actions under the numeric-version guard; acceptance is ambiguous.

- [`host-dialog-reservation-does-not-exclude-settings.md`](host-dialog-reservation-does-not-exclude-settings.md) — A
  Settings click during host-dialog admission may leave both dialogs inert until Escape; native ordering is unmeasured.

- [`launcher-submit-callback-retains-render-snapshots.md`](launcher-submit-callback-retains-render-snapshots.md) —
  Launcher renders retain additional submit snapshots until closure; practical memory impact is unmeasured.

- [`conformance-test-never-admits-management-candidates.md`](conformance-test-never-admits-management-candidates.md) —
  The generic responsiveness regression busy-refuses its candidates before their admitted waits.

- [`probe-timeout-discards-parsed-remote-path.md`](probe-timeout-discards-parsed-remote-path.md) — Probe cleanup can
  discard an already parsed executable path; a real escaped-helper trigger is unverified.

- [`guarded-answer-loses-launcher-focus.md`](guarded-answer-loses-launcher-focus.md) — An ordinary Guarded answer
  removes its focused control during preview refresh without a handoff; browser fallback is untested.

- [`guarded-cancellation-test-lacks-effect-witness.md`](guarded-cancellation-test-lacks-effect-witness.md) — The
  edited-form cancellation test lacks a browser-effect witness before its manual launch clears pending work.

- [`repository-search-reports-empty-before-reply.md`](repository-search-reports-empty-before-reply.md) — Partial
  repository search reports no matches while discovery is still pending.

- [`startup-selection-read-outlives-intervening-visits.md`](startup-selection-read-outlives-intervening-visits.md) — A
  delayed startup read can select an older session after intervening visits and deletion; timing is unmeasured.

- [`replacement-test-supervisor-spawns-before-pid-publication.md`](replacement-test-supervisor-spawns-before-pid-publication.md)
  — Worker interruption can leave a detached test supervisor outside cleanup registration.

- [`duplicate-uninstall-advice-points-to-removed-owner.md`](duplicate-uninstall-advice-points-to-removed-owner.md) —
  After owner removal, a duplicate's uninstall refusal points to the missing entry instead of explaining Retry.

- [`retention-plan-omits-live-retry-downgrade-limit.md`](retention-plan-omits-live-retry-downgrade-limit.md) — The
  plan's downgrade caveat omits older-code refusal of converted live checkout retries; intended compatibility is
  ambiguous.

- [`cancelled-upload-commit-retains-reply-entry.md`](cancelled-upload-commit-retains-reply-entry.md) — Cancelled final
  upload saves can retain reply bookkeeping on a surviving connection; practical memory impact is unmeasured.

- [`plan-pruning-can-interrupt-shared-cargo-build.md`](plan-pruning-can-interrupt-shared-cargo-build.md) — Plan cleanup
  may interrupt another workspace's build using its target directory; the concurrent failure schedule is unverified.
