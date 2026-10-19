# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

## Highest priority: security or data loss

- [`tunnel-error-notice-obscures-link-disclosure.md`](tunnel-error-notice-obscures-link-disclosure.md) — A terminal file
  or hovered web link can remain clickable while a tunnel-error notice hides its destination.

- [`pending-approval-can-cross-host-entries.md`](pending-approval-can-cross-host-entries.md) — Waiting approvals and GUI
  Delete/Replace consent can act on another host entry after session ownership changes.

- [`create-reply-foreign-id-bypasses-cache-refusal.md`](create-reply-foreign-id-bypasses-cache-refusal.md) — A hostile
  Create reply can select another host's existing session through paths that bypass the ownership refusal.

- [`installation-colon-splits-session-path.md`](installation-colon-splits-session-path.md) — A colon-bearing Mac
  installation path introduces unintended command-search directories; practical exploitability is unverified.

- [`payload-workers-outlive-state-lock.md`](payload-workers-outlive-state-lock.md) — Old payload work can overlap a
  successor helm after releasing state ownership; the integrity consequence remains unresolved.

- [`tunnel-never-pins-learned-identity.md`](tunnel-never-pins-learned-identity.md) — A tunnel opened before identity is
  learned can keep following a host entry without binding the subsequently learned installation.

- [`initial-launch-retains-uncanonicalized-path.md`](initial-launch-retains-uncanonicalized-path.md) — Initial launch
  retains a path spelling that can change meaning after validation; accidental wrong-directory impact remains
  unverified.

- [`unusable-directory-launches-in-home.md`](unusable-directory-launches-in-home.md) — An existing but unsearchable
  folder can make a session launch in the home directory instead.

- [`restart-abort-admits-old-child-report.md`](restart-abort-admits-old-child-report.md) — An aborted Restart can admit
  an old child conversation report into the restored run's Resume state.

- [`orphan-git-outlives-cache-lock.md`](orphan-git-outlives-cache-lock.md) — Interrupted checkout preparation can leave
  Git working after its repository-cache lock is released; completed-checkout damage is unverified.

- [`cancelled-startup-loses-archive-location.md`](cancelled-startup-loses-archive-location.md) — A cancelled startup
  worker can overwrite the recorded archive location after a successor moves the checkout.

- [`capture-marker-collision-misframes-replay.md`](capture-marker-collision-misframes-replay.md) — Terminal text can
  imitate capture delimiters, truncating replay or entering diagnostics; confidentiality impact is unverified.

- [`terminal-recovery-steals-confirmation-focus.md`](terminal-recovery-steals-confirmation-focus.md) — Automatic
  terminal recovery or tab fallback can take Enter intended for a header confirmation.

- [`header-copy-warning-follows-refreshed-value.md`](header-copy-warning-follows-refreshed-value.md) — Refreshed session
  metadata can remove the warning for hidden characters still on the clipboard; harmful subsequent use is unverified.

- [`session-accessible-names-use-raw-titles.md`](session-accessible-names-use-raw-titles.md) — Session accessible names
  retain raw title formatting; destructive-target confusion remains unverified and prior scope coverage is ambiguous.

- [`approval-session-id-display-collision.md`](approval-session-id-display-collision.md) — Approval cards can display
  distinct session IDs identically; mistaken authorization remains unverified.

- [`queued-launch-can-change-template.md`](queued-launch-can-change-template.md) — A queued sequence can show one
  template and directly launch another after its shortcut changes elsewhere.

- [`tunnel-port-can-divert-local-traffic.md`](tunnel-port-can-divert-local-traffic.md) — A consented tunnel can take a
  Mac port a later local service expects, without that consequence in the warning.

- [`refusal-banner-hides-approval-context.md`](refusal-banner-hides-approval-context.md) — A link-refusal banner can
  hide the requesting host or session while keyboard approval remains possible.

- [`inherited-spawn-carries-workspace-trust.md`](inherited-spawn-carries-workspace-trust.md) — Inherited spawn can carry
  managed-checkout trust into an ordinary folder.

- [`reused-checkout-path-grants-trust.md`](reused-checkout-path-grants-trust.md) — Replacing a managed checkout at its
  recorded path can make ordinary content appear eligible for automatic trust.

- [`empty-agent-prompt-submits-existing-draft.md`](empty-agent-prompt-submits-existing-draft.md) — An empty approved
  prompt can submit text already waiting in an agent's composer.

- [`prompt-approval-can-cross-agent-runs.md`](prompt-approval-can-cross-agent-runs.md) — A prompt approval can reach a
  replacement agent run whose permissions differ from those shown on the card.

- [`agent-approval-omits-workspace-trust.md`](agent-approval-omits-workspace-trust.md) — Agent launch approval cards no
  longer explain the effective managed-checkout trust choice in plain language.

- [`download-opens-without-hover-hint.md`](download-opens-without-hover-hint.md) — A policy change can make a Cmd-click
  open a file whose hover offered only a download.

- [`summary-card-omits-observed-yolo.md`](summary-card-omits-observed-yolo.md) — The main session summary can still say
  guarded after YOLO is observed; consequential reliance remains unverified.

- [`template-list-can-launch-recreated-name.md`](template-list-can-launch-recreated-name.md) — A stale template-list row
  can launch a replacement recreated under the same name; harmful execution remains unverified.

- [`template-list-advertises-rebound-shortcut.md`](template-list-advertises-rebound-shortcut.md) — Cached shortcut
  badges can guide a later launch to a different template; the gesture and harm remain unverified.

- [`expanded-setup-failure-displaces-open-menu.md`](expanded-setup-failure-displaces-open-menu.md) — A setup failure can
  leave a menu beside another row while it still acts on the original target; harmful reliance is unverified.

## High priority: material UX degradation

- [`abandoned-planners-exhaust-global-planning-slots.md`](abandoned-planners-exhaust-global-planning-slots.md) —
  Repeated update or removal planning can leave adding, updating, and removing hosts stuck until the helm restarts, even
  after the pages close.

- [`uninstall-forgotten-success.md`](uninstall-forgotten-success.md) — A failed uninstall can be reported as successful
  after another client forgets the host.

- [`uninstall-race-first-time-service-setup.md`](uninstall-race-first-time-service-setup.md) — Concurrent setup and
  uninstall can leave services pointing at a deleted executable; a conditional legacy path can miss existing services
  too.

- [`launcher-retries-ignore-settled-refusal-proof.md`](launcher-retries-ignore-settled-refusal-proof.md) — An unchanged
  New, Clone or Replace-with retry can keep replaying a missing-folder failure after the folder is restored.

- [`auth-refusal-forgets-partial-replace.md`](auth-refusal-forgets-partial-replace.md) — An authentication refusal can
  forget a partially successful Replace attempt, allowing its retry to create another replacement.

- [`unchanged-template-drops-launch-retry-key.md`](unchanged-template-drops-launch-retry-key.md) — Reapplying an
  unchanged template after a lost reply discards retry protection and can start another session.

## Other: correctness, diagnostics, cleanup, or convenience

- [`file-recognition-rejects-digit-leading-extensions.md`](file-recognition-rejects-digit-leading-extensions.md) —
  Printed filenames such as backup.7z cannot be hovered or downloaded unless the program includes a directory prefix.

- [`checkout-folder-save-trims-literal-path.md`](checkout-folder-save-trims-literal-path.md) — Saving an existing
  checkout-folder setting can change which directory it names, even without editing it.

- [`short-main-pane-hides-approval-requests.md`](short-main-pane-hides-approval-requests.md) — In a sufficiently short
  window, waiting approval requests disappear until the user enlarges the window or changes the selected session.

- [`agent-checkout-test-compares-lexical-tmp.md`](agent-checkout-test-compares-lexical-tmp.md) — New agent checkout
  tests compare canonical destinations with lexical /tmp paths and fail on macOS.

- [`introduction-promises-waiting-alerts-for-every-agent.md`](introduction-promises-waiting-alerts-for-every-agent.md) —
  Introductory artwork promises blocked-agent alerts for agents without waiting detection.

- [`introduction-omits-automatic-install-prerequisites.md`](introduction-omits-automatic-install-prerequisites.md) — The
  automatic-installation claim omits architecture and systemd user-manager prerequisites.

- [`pinned-host-dialog-chrome-can-hide-content.md`](pinned-host-dialog-chrome-can-hide-content.md) — Fixed host-dialog
  headings and answers can exceed the available height, hiding controls, plan content or a refusal's explanation.

- [`update-check-replacement-loses-focus.md`](update-check-replacement-loses-focus.md) — Installing an update replaces
  the keyboard-focused check button without transferring focus to the restart control.

- [`template-discovery-fails-permanently-once-reply-exceeds.md`](template-discovery-fails-permanently-once-reply-exceeds.md)
  — A large valid template catalog permanently disables agent discovery.

- [`native-download-quit-leaves-partial-files.md`](native-download-quit-leaves-partial-files.md) — Desktop Quit leaves
  hidden partial downloads with no recovery owner.

- [`terminal-right-click-recopies-stale-selection.md`](terminal-right-click-recopies-stale-selection.md) — A right-click
  in a terminal without mouse reporting can replace newer clipboard contents with an old terminal selection.

- [`template-screenshot-implies-deletion-needs-save.md`](template-screenshot-implies-deletion-needs-save.md) — A reader
  could delete a saved template and close the dialog believing the deletion was never committed.

- [`introduction-promises-universal-reboot-resume.md`](introduction-promises-universal-reboot-resume.md) — The
  introduction promises conversation recovery after reboot for sessions without Resume reporting.

- [`restored-terminal-snapshot-rejects-link-clicks.md`](restored-terminal-snapshot-rejects-link-clicks.md) — Restored
  terminal snapshots can retain hover effects while rejecting every web or file link click.

- [`repeated-terminal-links-acquire-mismatch-warning.md`](repeated-terminal-links-acquire-mismatch-warning.md) —
  Repeating one honest hyperlink on adjacent rows can produce a false destination-mismatch warning.

- [`repository-discovery-inherits-bare-git-config.md`](repository-discovery-inherits-bare-git-config.md) — An inherited
  Git configuration selector can prevent repositories from appearing as launcher suggestions.

- [`notification-test-cleanup-skips-preference-restoration.md`](notification-test-cleanup-skips-preference-restoration.md)
  — A failed notification fixture cleanup can skip later session releases and shared preference restoration.

- [`trash-sidebar-calls-unchecked-archives-empty.md`](trash-sidebar-calls-unchecked-archives-empty.md) — The trash
  button can advertise an empty collection while unchecked archives remain.

- [`over-limit-trash-total-loses-partial-size.md`](over-limit-trash-total-loses-partial-size.md) — A successful listing
  above the deletion limit can lose the global partial-size warning.

- [`instruction-check-allows-claude-local-shadowing.md`](instruction-check-allows-claude-local-shadowing.md) — The
  instruction checker permits local Claude files that suppress default repository-rule loading.

- [`instruction-check-misses-mixed-case-spec-typo.md`](instruction-check-misses-mixed-case-spec-typo.md) — The
  instruction checker misses the mixed-case specification typo its own contract promises to reject.

- [`unselected-repository-keeps-stale-setup-eligibility.md`](unselected-repository-keeps-stale-setup-eligibility.md) —
  Checkout setup eligibility can remain stale while no repository is selected.

- [`checkout-config-refresh-erases-setup-draft.md`](checkout-config-refresh-erases-setup-draft.md) — An unrelated
  configuration refresh can unmount the launcher's checkout-folder editor and erase its unfinished entry.

- [`outage-card-content-can-overflow-above-view.md`](outage-card-content-can-overflow-above-view.md) — A short terminal
  area or long host error can put the outage explanation and recovery instructions above the reachable view.

- [`replace-yolo-questions-remain-separate.md`](replace-yolo-questions-remain-separate.md) — Equivalent Replace controls
  can leave separate YOLO questions, with cancellation reflected in only one surface.

- [`archive-recovery-clears-reused-path-trust.md`](archive-recovery-clears-reused-path-trust.md) — Startup recovery or
  Delete retry of an interrupted archive can erase the trust choice for a different directory subsequently placed at the
  old path.

- [`cancelled-upload-retains-admission-entry.md`](cancelled-upload-retains-admission-entry.md) — Abandoned upload
  admission can accumulate pending requests until the host connection ends.

- [`download-eof-cancellation-retains-route.md`](download-eof-cancellation-retains-route.md) — Cancellation during
  successful download cleanup can retain completed-transfer bookkeeping.

- [`tab-attach-blocks-host-reader-during-stop.md`](tab-attach-blocks-host-reader-during-stop.md) — Attaching a tab
  during Stop can temporarily block unrelated requests on the host connection.

- [`stale-outcome-hides-successful-stop.md`](stale-outcome-hides-successful-stop.md) — A stale status publication can
  hide a successfully recorded user Stop until another lifecycle operation or reload repairs it.

- [`terminal-peer-failures-hide-format-controls.md`](terminal-peer-failures-hide-format-controls.md) — Terminal
  connection and upload errors display peer formatting characters outside the terminal without escaping them.

- [`automatic-notification-dismissal-loses-focus.md`](automatic-notification-dismissal-loses-focus.md) — Automatic
  notification-list dismissal has no focus handoff to its session bell.

- [`ssh-help-close-loses-replaced-opener-focus.md`](ssh-help-close-loses-replaced-opener-focus.md) — Closing SSH help
  after a setup offer replaces its opener can lose immediate keyboard focus.

- [`notification-popup-drifts-after-local-delete.md`](notification-popup-drifts-after-local-delete.md) — A notification
  popup can remain at its former position after a session above it is deleted.

- [`empty-initial-prompt-skipped-by-tab.md`](empty-initial-prompt-skipped-by-tab.md) — Tab traversal can wrap before
  reaching the empty initial-prompt editor.

- [`current-agent-chip-erases-custom-flags.md`](current-agent-chip-erases-custom-flags.md) — Reselecting the current
  agent can erase permission flags with an unfinished quote.

- [`incomplete-custom-flags-reset-compatible-effort.md`](incomplete-custom-flags-reset-compatible-effort.md) — Accepting
  a custom model can reset compatible effort because permission flags are incomplete.

- [`template-name-editor-conceals-formatting.md`](template-name-editor-conceals-formatting.md) — The template name
  editor displays formatting characters raw although the list and approval card escape them.

- [`template-table-omits-custom-permission-flags.md`](template-table-omits-custom-permission-flags.md) — The default
  template table omits the words that its custom permission setting applies.

- [`delayed-host-confirmation-conflicts-with-other-dialogs.md`](delayed-host-confirmation-conflicts-with-other-dialogs.md)
  — A delayed host confirmation or action refusal can disable another open dialog's clicks and normal keyboard
  interaction; Escape recovers.

- [`stale-template-dialog-undoes-agent-shortcut.md`](stale-template-dialog-undoes-agent-shortcut.md) — An open Templates
  dialog can overwrite an approved agent shortcut move with its old bindings.

- [`noop-shortcut-approval-reports-missing-binding.md`](noop-shortcut-approval-reports-missing-binding.md) — A shortcut
  request can report success after another client moves its initially satisfied binding during approval.

- [`summary-card-clips-lower-facts.md`](summary-card-clips-lower-facts.md) — A tall hover summary can hide its directory
  and timestamps without scrolling.

- [`shortcut-tests-skip-owned-cleanup.md`](shortcut-tests-skip-owned-cleanup.md) — Early cleanup failures in new
  shortcut tests skip remaining owned sessions or templates.

- [`release-review-links-allow-page-script.md`](release-review-links-allow-page-script.md) — Draft Markdown links can
  inject release-review page script; attacker exposure and consequential harm remain unverified.

- [`draft-release-note-promises-update-confirmation.md`](draft-release-note-promises-update-confirmation.md) — The draft
  release note promises a numbered Update confirmation although clicking Update starts it without asking.
