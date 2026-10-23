# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

## Highest priority: security or data loss

- [`offline-uninstall-retry-skips-session-check.md`](offline-uninstall-retry-skips-session-check.md) — An uninstall
  retry can stop unchecked live sessions when a custom command disappears after a failed removal; the conditional
  still-active-supervisor route is source-established.

- [`trash-entry-delete-hits-shifted-row.md`](trash-entry-delete-hits-shifted-row.md) — A one-click trash delete can
  permanently delete a different checkout when rows shift under the pointer; click timing is unmeasured.

- [`uninstall-notice-copies-raw-remote-path.md`](uninstall-notice-copies-raw-remote-path.md) — The uninstall notice
  copies a remote path without warning; successful uninstall rejects controls, but concurrent removal can expose an owed
  notice after failure. Unsafe downstream paste remains unverified.

- [`templates-dialog-new-name-overwrites-outside-template.md`](templates-dialog-new-name-overwrites-outside-template.md)
  — A new or renamed template in an open Templates dialog replaces one an agent or the CLI created meanwhile; coverage
  by the last-write-wins decision is unsettled.

- [`private-tmux-terminfo-path-under-tmp.md`](private-tmux-terminfo-path-under-tmp.md) — The shipped Linux tmux searches
  a deleted /tmp build path for terminal definitions another account could plant; only hand-run use reads it.

- [`plans-queue-counts-fork-prs-as-plan-work.md`](plans-queue-counts-fork-prs-as-plan-work.md) — The planning system
  treats an outsider's fork PR named like a plan branch as the plan's own work; an executor building on it is
  unverified, and the bucket is unsettled.

- [`centos-ssh-scrub-drops-config-after-unmatched-marker.md`](centos-ssh-scrub-drops-config-after-unmatched-marker.md) —
  The CentOS test deletes the user's ssh config after a leftover start marker with no end marker; an accidental trigger
  is unverified, and the bucket is unsettled.

- [`uninstall-two-home-aliases-hide-kept-data.md`](uninstall-two-home-aliases-hide-kept-data.md) — Two alternate home
  paths with divergent nested mounts can hide retained data from uninstall; the configured source chain is established,
  but occurrence and support scope are unresolved.

- [`lost-shortcut-save-restores-misleading-assignment.md`](lost-shortcut-save-restores-misleading-assignment.md) — A
  lost committed-save reply restores an old shortcut indication while the next key resolves the new template; harmful
  reliance and coverage remain unproved.

## High priority: material UX degradation

- [`shell-inspection-drains-exhaust-planning-pool.md`](shell-inspection-drains-exhaust-planning-pool.md) — Pipe-draining
  shell inspections can occupy every host-planning slot; an actual helper trigger is unverified.

- [`untouched-checkout-folder-save-undoes-cli-edit.md`](untouched-checkout-folder-save-undoes-cli-edit.md) — Saving an
  untouched checkout-folder field can overwrite a newer CLI setting.

- [`power-loss-version-folder-never-repaired.md`](power-loss-version-folder-never-repaired.md) — A Mac version folder
  damaged by power loss after an update is kept by every rerun and update; APFS exposing the damage is unverified.

- [`commands-setting-help-omits-delete.md`](commands-setting-help-omits-delete.md) — The help for running farhelm
  commands without asking omits that agents may delete sessions and templates; bucket is unsettled.

- [`devbuild-update-strands-host-on-newer-schema.md`](devbuild-update-strands-host-on-newer-schema.md) — Updating back
  from a devbuild that migrated a shared remote host's database leaves the host unreachable; not reproduced, and the
  bucket is unsettled.

- [`devbuild-tmux-skips-release-signature.md`](devbuild-tmux-skips-release-signature.md) — A devbuild pushes a tmux to
  remote hosts checked only against an unsigned checksum list from the release site; spec coverage is ambiguous.

## Other: correctness, diagnostics, cleanup, or convenience

- [`removed-host-strands-tunnel-listeners.md`](removed-host-strands-tunnel-listeners.md) — Removing a host leaves its
  tunnel ports occupied while individual close requests fail.

- [`menu-close-dismisses-new-notifications.md`](menu-close-dismisses-new-notifications.md) — Closing one popup may
  dismiss a newly opened notification list and mark it read; event ordering remains unverified.

- [`download-interruption-promise-omits-crash-exception.md`](download-interruption-promise-omits-crash-exception.md) —
  The product's partial-download cleanup promise omits the newly accepted crash and force-quit exception.

- [`add-host-refusal-bypasses-dialog-admission.md`](add-host-refusal-bypasses-dialog-admission.md) — A delayed Add-host
  refusal may reopen a mutually inert dialog beside Settings; Escape remains available.

- [`browser-menu-focus-retains-evaluator-channels.md`](browser-menu-focus-retains-evaluator-channels.md) — Browser menu
  focus retains evaluator channels per request; practical memory impact is unmeasured.

- [`browser-suite-ignores-explicit-port.md`](browser-suite-ignores-explicit-port.md) — The browser suite silently
  ignores its documented manual port override.

- [`catalog-cleanup-can-start-untracked-test-session.md`](catalog-cleanup-can-start-untracked-test-session.md) —
  Releasing a held catalog or finishing a pending key mint during cleanup may start an untracked fake test session;
  adverse scheduling is unmeasured.

- [`classic-mouse-snapshot-copy-blocks-later-links.md`](classic-mouse-snapshot-copy-blocks-later-links.md) — Copying in
  a classic mouse-report snapshot may leave web clicks inert; native behavior is unverified.

- [`delayed-refusal-reopens-cancelled-replace-question.md`](delayed-refusal-reopens-cancelled-replace-question.md) — A
  delayed refusal may reopen a cancelled Replace question; a further affirmative answer is still required.

- [`history-fixture-reset-races-selection-save.md`](history-fixture-reset-races-selection-save.md) — An automatic
  selection save may overwrite the history fixture's reset; adverse ordering is unverified.

- [`instruction-file-case-alias-evades-check.md`](instruction-file-case-alias-evades-check.md) — Case aliases evade the
  local instruction-file check; actual vendor lookup on case-insensitive filesystems is unverified.

- [`menu-blur-can-leave-delayed-arrow-current.md`](menu-blur-can-leave-delayed-arrow-current.md) — A blur without
  focus-in may leave delayed menu-arrow work current; browser event behavior is unverified.

- [`normal-quit-misses-empty-download-directory.md`](normal-quit-misses-empty-download-directory.md) — A narrow
  normal-Quit race may leave an empty download staging folder; saved files are unaffected by this mechanism.

- [`notification-bell-removal-skips-focus-return.md`](notification-bell-removal-skips-focus-return.md) — Removing the
  notification bell may skip keyboard-focus return; browser fallback is unverified.

- [`pinned-host-dialog-chrome-can-hide-content.md`](pinned-host-dialog-chrome-can-hide-content.md) — The deleted dialog
  record still has an unresolved Host Settings footer-clipping case; exact geometry is unverified.

- [`prompt-prefill-test-opens-before-source-selection.md`](prompt-prefill-test-opens-before-source-selection.md) — The
  prompt-prefill and clone-defaults tests may open New before source selection; startup ordering is unverified.

- [`release-heading-can-close-review-page-script.md`](release-heading-can-close-review-page-script.md) — A malformed
  curated release heading can inject review-page script; attacker influence and practical impact are unverified.

- [`removal-destination-collapses-distinguishing-spaces.md`](removal-destination-collapses-distinguishing-spaces.md) —
  Removal/refusal destinations and manual startup paths collapse distinguishing spaces; live SSH examples, clipboard
  serialization and harmful startup reliance remain unverified.

- [`secondary-press-suppresses-primary-selection-copy.md`](secondary-press-suppresses-primary-selection-copy.md) — A
  second mouse button may suppress primary-selection copying; physical browser behavior is unverified.

- [`selection-saves-omit-intermediate-visits.md`](selection-saves-omit-intermediate-visits.md) — Successful selection
  saves can omit intermediate visits from shared session history.

- [`session-view-fallback-omits-start-time.md`](session-view-fallback-omits-start-time.md) — The promised fallback for
  clipped hover details omits a session's separate start time.

- [`timing-history-can-retain-failed-recorder-success.md`](timing-history-can-retain-failed-recorder-success.md) —
  Permanent timing history can retain recorder success after required evidence publication fails; failure injection is
  unexecuted.

- [`removed-deflake-items-leave-exclusions.md`](removed-deflake-items-leave-exclusions.md) — Removing two tracked flake
  items leaves their machine-readable sweep exclusions orphaned.

- [`landing-leaves-baseline-flake-unrecorded.md`](landing-leaves-baseline-flake-unrecorded.md) — Landing reports defer
  recording recurring baseline flakes, including the initial-prompt regression; their private runtime evidence was not
  authenticated here.

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

- [`retention-plan-omits-live-retry-downgrade-limit.md`](retention-plan-omits-live-retry-downgrade-limit.md) — The
  plan's downgrade caveat omits older-code refusal of converted live checkout retries; intended compatibility is
  ambiguous.

- [`cancelled-upload-commit-retains-reply-entry.md`](cancelled-upload-commit-retains-reply-entry.md) — Cancelled final
  upload saves can retain reply bookkeeping on a surviving connection; practical memory impact is unmeasured.

- [`plan-pruning-can-interrupt-shared-cargo-build.md`](plan-pruning-can-interrupt-shared-cargo-build.md) — Plan cleanup
  may interrupt another workspace's build using its target directory; the concurrent failure schedule is unverified.

- [`spare-test-fifo-write-can-outlive-reader.md`](spare-test-fifo-write-can-outlive-reader.md) — A completion test's
  FIFO release can block after its reader exits; the standard runner supplies only an outer timeout.

- [`checkout-test-skips-new-spare-setting-restoration.md`](checkout-test-skips-new-spare-setting-restoration.md) — An
  earlier caller-cleanup failure can skip restoring the new prepared-checkout switch; historical coverage remains
  ambiguous.

- [`tunnel-consent-conflicts-with-restart-dialog.md`](tunnel-consent-conflicts-with-restart-dialog.md) — Tunnel consent
  and Restart with can become mutually inert; native behavior and Escape recovery remain untested.

- [`claude-dialog-text-mimics-input-box.md`](claude-dialog-text-mimics-input-box.md) — Agent-written text in a Claude
  question dialog can read as the input box; dormant while agent prompts are off, multi-row rendering unverified.

- [`codex-output-mimics-queued-question.md`](codex-output-mimics-queued-question.md) — Codex output shaped like a
  queued-question hint reads waiting mid-turn, so Restart skips its question; the rendering premise is unverified.

- [`todo-picker-port-squatter-steers-planner.md`](todo-picker-port-squatter-steers-planner.md) — Another local account
  holding the picker or triage page's fixed address can forge pasted input; token-path routing alone does not
  authenticate that server. Tab reuse is unverified and the bucket is unsettled.

- [`sticky-parent-guard-contradicts-own-docs.md`](sticky-parent-guard-contradicts-own-docs.md) — The test recorder's and
  deflake's docs claim to refuse another account's /tmp/farhelm-tmp, which a sticky one passes.

- [`uninstall-completion-bypasses-size-decision.md`](uninstall-completion-bypasses-size-decision.md) — Uninstall work
  entered unattended landing eligibility after exceeding a required size gate, without a maintainer waiver in the pinned
  record; no product harm is established.

- [`launcher-subscription-test-masks-missing-observers.md`](launcher-subscription-test-masks-missing-observers.md) — A
  launcher regression shares replay subscriptions and changes several positive-control fields, masking the omission it
  claims to catch; no production defect is established.

- [`closed-popovers-repeat-prefix-scans.md`](closed-popovers-repeat-prefix-scans.md) — Refreshes repeat scans and sorted
  prefix work even with popups closed; supported-scale latency is unmeasured and browser availability coverage is
  ambiguous.

- [`held-preference-reply-blocks-fixture-restoration.md`](held-preference-reply-blocks-fixture-restoration.md) — A
  failed held reply can block four tests' preference/template restoration; the conditional contamination schedule is
  untested.
