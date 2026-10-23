# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

## Highest priority: security or data loss

- [`tunnel-can-take-standalone-helm-port.md`](tunnel-can-take-standalone-helm-port.md) — A localhost tunnel can serve a
  remote page at the standalone helm's address and read the browser's helm credential; how often one is stored is
  unverified.

- [`offline-uninstall-retry-skips-session-check.md`](offline-uninstall-retry-skips-session-check.md) — An uninstall
  retry can stop unchecked live sessions when a custom command disappears after a failed removal; the conditional
  still-active-supervisor route is source-established.

- [`tunnel-shadows-busy-wildcard-port.md`](tunnel-shadows-busy-wildcard-port.md) — A Mac tunnel may take a port a local
  service serves on all addresses and divert its clients to the remote host; macOS bind behavior is unverified.

- [`link-classifier-misses-embedded-ipv4.md`](link-classifier-misses-embedded-ipv4.md) — Some IPv6 forms of local IPv4
  addresses open as public links without a card; whether they reach the user's network is unverified.

- [`agent-stop-kills-shared-personal-tmux.md`](agent-stop-kills-shared-personal-tmux.md) — Stop or Restart kills the
  user's own tmux server or ssh master, with outside work in it, when the agent started it first; spec coverage is
  ambiguous.

- [`trash-entry-delete-hits-shifted-row.md`](trash-entry-delete-hits-shifted-row.md) — A one-click trash delete can
  permanently delete a different checkout when rows shift under the pointer; click timing is unmeasured.

- [`tab-owns-first-personal-tmux.md`](tab-owns-first-personal-tmux.md) — The accepted rule that a tab owns services its
  startup files start first rests on a premise tmux autostart contradicts; marker-only exclusion also leaves cgroup
  termination intact, and runtime inheritance is untested.

- [`shared-checkout-root-archives-other-host-work.md`](shared-checkout-root-archives-other-host-work.md) — A checkout
  root shared between hosts lets one host trash, then permanently delete, a checkout another host's session works in;
  whether a shared root is supported is unsettled.

- [`create-retry-takeover-replaces-restarted-session.md`](create-retry-takeover-replaces-restarted-session.md) — A stale
  create retry may take over a session after an ambiguous Restart failure; active Restart is locked, and the complete
  record-loss/survivor chain and fault-pair coverage remain unresolved.

- [`interrupted-delete-loses-attachments-at-startup.md`](interrupted-delete-loses-attachments-at-startup.md) — A session
  that survives an interrupted Delete loses its attachments at the next supervisor start; the partial-deletion allowance
  and a ledger decision conflict.

- [`named-restart-with-keeps-unreported-old-conversation.md`](named-restart-with-keeps-unreported-old-conversation.md) —
  After a named Restart with that cannot report, Resume reopens the previous conversation indefinitely; the unhooked
  spawn path is not traced end to end.

- [`no-birth-time-checkout-archives-reclone.md`](no-birth-time-checkout-archives-reclone.md) — On a filesystem without
  creation times, a folder re-cloned over a checkout is trashed as Farhelm's; which supported filesystems lack them is
  unverified.

- [`nested-goose-report-replaces-resume-target.md`](nested-goose-report-replaces-resume-target.md) — A nested or
  sub-agent Goose run can replace its Goose session's Resume target; a lore decision deferred this outside the coverage
  sources, and an inherited nonce alone cannot establish foreground ownership.

- [`uninstall-notice-copies-raw-remote-path.md`](uninstall-notice-copies-raw-remote-path.md) — The uninstall notice
  copies a remote path without warning; successful uninstall rejects controls, but concurrent removal can expose an owed
  notice after failure. Unsafe downstream paste remains unverified.

- [`templates-dialog-new-name-overwrites-outside-template.md`](templates-dialog-new-name-overwrites-outside-template.md)
  — A new or renamed template in an open Templates dialog replaces one an agent or the CLI created meanwhile; coverage
  by the last-write-wins decision is unsettled.

- [`grok-unrecognized-pane-admits-nested-grok.md`](grok-unrecognized-pane-admits-nested-grok.md) — A nested Grok's
  conversation can become the Resume target when the session's Grok has an unrecognized program name; such names are
  unverified.

- [`omp-direct-bun-pane-admits-nested-omp.md`](omp-direct-bun-pane-admits-nested-omp.md) — A direct Bun OMP launch with
  an unrecognized entry file can credit a nested OMP's conversation; the nested reporter premise is unverified.

- [`private-tmux-terminfo-path-under-tmp.md`](private-tmux-terminfo-path-under-tmp.md) — The shipped Linux tmux searches
  a deleted /tmp build path for terminal definitions another account could plant; only hand-run use reads it.

- [`plans-queue-counts-fork-prs-as-plan-work.md`](plans-queue-counts-fork-prs-as-plan-work.md) — The planning system
  treats an outsider's fork PR named like a plan branch as the plan's own work; an executor building on it is
  unverified, and the bucket is unsettled.

- [`centos-ssh-scrub-drops-config-after-unmatched-marker.md`](centos-ssh-scrub-drops-config-after-unmatched-marker.md) —
  The CentOS test deletes the user's ssh config after a leftover start marker with no end marker; an accidental trigger
  is unverified, and the bucket is unsettled.

- [`pi-child-report-replaces-resume-target.md`](pi-child-report-replaces-resume-target.md) — A child Pi carrying
  Farhelm's reporter can replace its Pi session's Resume target; a forwarding extension is unverified.

- [`uninstall-two-home-aliases-hide-kept-data.md`](uninstall-two-home-aliases-hide-kept-data.md) — Two alternate home
  paths with divergent nested mounts can hide retained data from uninstall; the configured source chain is established,
  but occurrence and support scope are unresolved.

- [`uninstall-home-bind-inside-installation.md`](uninstall-home-bind-inside-installation.md) — A home bind inside the
  installation can expose retained files to recursive uninstall; traversal is untested and configuration coverage is
  ambiguous.

- [`lost-shortcut-save-restores-misleading-assignment.md`](lost-shortcut-save-restores-misleading-assignment.md) — A
  lost committed-save reply restores an old shortcut indication while the next key resolves the new template; harmful
  reliance and coverage remain unproved.

- [`reseeded-launcher-retains-conversation-selector.md`](reseeded-launcher-retains-conversation-selector.md) — A
  superseded mounted launcher retains a conversation selector; an accessible shipped action completing the sequence is
  unestablished.

- [`recovery-retains-displaced-session-menu.md`](recovery-retains-displaced-session-menu.md) — Fast desktop recovery can
  leave a session menu displaced after header wrapping; consequential mistaken Stop/Delete use remains unverified.

- [`loading-cancel-may-discard-admitted-launch.md`](loading-cancel-may-discard-admitted-launch.md) — Loading Cancel may
  discard an admitted launch and recovery identity; its post-admission delivery and harmful duplication remain
  source-incomplete.

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

- [`duplicate-pane-facts-can-unprotect-spare-claim.md`](duplicate-pane-facts-can-unprotect-spare-claim.md) —
  Deliberately forged private-tmux facts can expose an active preparation to cleanup; explicit unsupported-use coverage
  remains ambiguous.

- [`cancelled-startup-sweep-outlives-directory-ownership.md`](cancelled-startup-sweep-outlives-directory-ownership.md) —
  Blocking cleanup can outlive supervisor ownership and delete a successor's claimed checkout or retry input; resulting
  failures are unmeasured.

- [`power-loss-version-folder-never-repaired.md`](power-loss-version-folder-never-repaired.md) — A Mac version folder
  damaged by power loss after an update is kept by every rerun and update; APFS exposing the damage is unverified.

- [`terminal-reveal-takes-focus-from-plain-button.md`](terminal-reveal-takes-focus-from-plain-button.md) — A terminal
  finishing its load takes focus from a keyboard-focused button, so Enter may answer an agent prompt; timing and bucket
  are unsettled.

- [`commands-setting-help-omits-delete.md`](commands-setting-help-omits-delete.md) — The help for running farhelm
  commands without asking omits that agents may delete sessions and templates; bucket is unsettled.

- [`replace-with-recent-row-says-launch.md`](replace-with-recent-row-says-launch.md) — In Replace with, Enter on a
  recent setup replaces while the row says only launch; whether it misleads in practice is unverified.

- [`stale-confirmation-optout-replayed-after-sign-in.md`](stale-confirmation-optout-replayed-after-sign-in.md) — A
  browser's failed confirmation opt-out is replayed over a later restore after sign-in; not reproduced.

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
  Removal/refusal destinations and manual startup paths collapse distinguishing spaces; live SSH examples, clipboard
  serialization and harmful startup reliance remain unverified.

- [`replacement-cleanup-test-never-proves-child-lived.md`](replacement-cleanup-test-never-proves-child-lived.md) — The
  replacement cleanup test never proves its stand-in was alive before checking absence; an actual false pass is
  unverified.

- [`secondary-press-suppresses-primary-selection-copy.md`](secondary-press-suppresses-primary-selection-copy.md) — A
  second mouse button may suppress primary-selection copying; physical browser behavior is unverified.

- [`selection-saves-omit-intermediate-visits.md`](selection-saves-omit-intermediate-visits.md) — Successful selection
  saves can omit intermediate visits from shared session history.

- [`session-view-fallback-omits-start-time.md`](session-view-fallback-omits-start-time.md) — The promised fallback for
  clipped hover details omits a session's separate start time.

- [`timing-history-can-retain-failed-recorder-success.md`](timing-history-can-retain-failed-recorder-success.md) —
  Permanent timing history can retain recorder success after required evidence publication fails; failure injection is
  unexecuted.

- [`welcome-recovery-counts-unfinished-selection-read.md`](welcome-recovery-counts-unfinished-selection-read.md) — The
  welcome-recovery test may reject an extra legitimate preference read; adverse scheduling is unverified.

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

- [`duplicate-uninstall-advice-points-to-removed-owner.md`](duplicate-uninstall-advice-points-to-removed-owner.md) —
  After owner removal, a duplicate's uninstall refusal points to the missing entry instead of explaining Retry.

- [`retention-plan-omits-live-retry-downgrade-limit.md`](retention-plan-omits-live-retry-downgrade-limit.md) — The
  plan's downgrade caveat omits older-code refusal of converted live checkout retries; intended compatibility is
  ambiguous.

- [`cancelled-upload-commit-retains-reply-entry.md`](cancelled-upload-commit-retains-reply-entry.md) — Cancelled final
  upload saves can retain reply bookkeeping on a surviving connection; practical memory impact is unmeasured.

- [`plan-pruning-can-interrupt-shared-cargo-build.md`](plan-pruning-can-interrupt-shared-cargo-build.md) — Plan cleanup
  may interrupt another workspace's build using its target directory; the concurrent failure schedule is unverified.

- [`spare-test-fifo-write-can-outlive-reader.md`](spare-test-fifo-write-can-outlive-reader.md) — A completion test's
  FIFO release can block after its reader exits; the standard runner supplies only an outer timeout.

- [`spare-owner-tests-register-child-cleanup-after-readiness.md`](spare-owner-tests-register-child-cleanup-after-readiness.md)
  — Owner-death tests can leave a disposable child outside cleanup after readiness failure.

- [`settings-test-restores-before-final-spare-save.md`](settings-test-restores-before-final-spare-save.md) — A final
  test save can overtake restoration of an initially disabled switch; the established starting state requires earlier
  cleanup failure.

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
