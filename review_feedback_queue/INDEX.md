# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

## Highest priority: security or data loss

- [`same-destination-recovery-loses-trash-location.md`](same-destination-recovery-loses-trash-location.md) — Overlapping
  recovery with the same destination may hide retained checkout files from the trash; an affected platform is
  unverified.

## High priority: material UX degradation

- [`abandoned-planners-exhaust-global-planning-slots.md`](abandoned-planners-exhaust-global-planning-slots.md) —
  Repeated update or removal planning can leave adding, updating, and removing hosts stuck until the helm restarts, even
  after the pages close.

## Other: correctness, diagnostics, cleanup, or convenience

- [`agent-checkout-test-compares-lexical-tmp.md`](agent-checkout-test-compares-lexical-tmp.md) — New agent checkout
  tests compare canonical destinations with lexical /tmp paths and fail on macOS.

- [`introduction-promises-waiting-alerts-for-every-agent.md`](introduction-promises-waiting-alerts-for-every-agent.md) —
  Introductory artwork promises blocked-agent alerts for agents without waiting detection.

- [`introduction-omits-automatic-install-prerequisites.md`](introduction-omits-automatic-install-prerequisites.md) — The
  automatic-installation claim omits architecture and systemd user-manager prerequisites.

- [`template-discovery-fails-permanently-once-reply-exceeds.md`](template-discovery-fails-permanently-once-reply-exceeds.md)
  — A large valid template catalog permanently disables agent discovery.

- [`native-download-quit-leaves-partial-files.md`](native-download-quit-leaves-partial-files.md) — Desktop Quit leaves
  hidden partial downloads with no recovery owner.

- [`template-screenshot-implies-deletion-needs-save.md`](template-screenshot-implies-deletion-needs-save.md) — A reader
  could delete a saved template and close the dialog believing the deletion was never committed.

- [`introduction-promises-universal-reboot-resume.md`](introduction-promises-universal-reboot-resume.md) — The
  introduction promises conversation recovery after reboot for sessions without Resume reporting.

- [`repository-discovery-inherits-bare-git-config.md`](repository-discovery-inherits-bare-git-config.md) — An inherited
  Git configuration selector can prevent repositories from appearing as launcher suggestions.

- [`notification-test-cleanup-skips-preference-restoration.md`](notification-test-cleanup-skips-preference-restoration.md)
  — A failed notification fixture cleanup can skip later session releases and shared preference restoration.

- [`instruction-check-allows-claude-local-shadowing.md`](instruction-check-allows-claude-local-shadowing.md) — The
  instruction checker permits local Claude files that suppress default repository-rule loading.

- [`instruction-check-misses-mixed-case-spec-typo.md`](instruction-check-misses-mixed-case-spec-typo.md) — The
  instruction checker misses the mixed-case specification typo its own contract promises to reject.

- [`unselected-repository-keeps-stale-setup-eligibility.md`](unselected-repository-keeps-stale-setup-eligibility.md) —
  Checkout setup eligibility can remain stale while no repository is selected.

- [`archive-recovery-clears-reused-path-trust.md`](archive-recovery-clears-reused-path-trust.md) — Startup recovery or
  Delete retry of an interrupted archive can erase the trust choice for a different directory subsequently placed at the
  old path.

- [`stale-outcome-hides-successful-stop.md`](stale-outcome-hides-successful-stop.md) — A stale status publication can
  hide a successfully recorded user Stop until another lifecycle operation or reload repairs it.

- [`automatic-notification-dismissal-loses-focus.md`](automatic-notification-dismissal-loses-focus.md) — Automatic
  notification-list dismissal has no focus handoff to its session bell.

- [`notification-popup-drifts-after-local-delete.md`](notification-popup-drifts-after-local-delete.md) — A notification
  popup can remain at its former position after a session above it is deleted.

- [`empty-initial-prompt-skipped-by-tab.md`](empty-initial-prompt-skipped-by-tab.md) — Tab traversal can wrap before
  reaching the empty initial-prompt editor.

- [`template-name-editor-conceals-formatting.md`](template-name-editor-conceals-formatting.md) — The template name
  editor displays formatting characters raw although the list and approval card escape them.

- [`template-table-omits-custom-permission-flags.md`](template-table-omits-custom-permission-flags.md) — The default
  template table omits the words that its custom permission setting applies.

- [`stale-template-dialog-undoes-agent-shortcut.md`](stale-template-dialog-undoes-agent-shortcut.md) — An open Templates
  dialog can overwrite an approved agent shortcut move with its old bindings.

- [`noop-shortcut-approval-reports-missing-binding.md`](noop-shortcut-approval-reports-missing-binding.md) — A shortcut
  request can report success after another client moves its initially satisfied binding during approval.

- [`summary-card-clips-lower-facts.md`](summary-card-clips-lower-facts.md) — A tall hover summary can hide its directory
  and timestamps without scrolling.

- [`shortcut-tests-skip-owned-cleanup.md`](shortcut-tests-skip-owned-cleanup.md) — Early cleanup failures in shortcut
  and naming tests skip remaining owned resources or shared checkout configuration restoration.

- [`release-review-links-allow-page-script.md`](release-review-links-allow-page-script.md) — Draft Markdown links can
  inject release-review page script; attacker exposure and consequential harm remain unverified.

- [`draft-release-note-promises-update-confirmation.md`](draft-release-note-promises-update-confirmation.md) — Finalized
  v0.30.0 notes promise an Update confirmation although clicking Update starts it without asking.

- [`terminal-focus-tests-arm-cleanup-late.md`](terminal-focus-tests-arm-cleanup-late.md) — A focus-observer setup
  failure can leave a new regression test's live session outside its cleanup scope.

- [`removed-host-strands-tunnel-listeners.md`](removed-host-strands-tunnel-listeners.md) — Removing a host leaves its
  tunnel ports occupied while individual close requests fail.

- [`menu-close-dismisses-new-notifications.md`](menu-close-dismisses-new-notifications.md) — Closing an actions menu can
  immediately dismiss a newly opened notification list and mark its contents read.

- [`selection-history-plan-forgets-upgrade-selection.md`](selection-history-plan-forgets-upgrade-selection.md) — The
  visit-history plan's empty upgrade history conflicts with restoring the existing remembered session.
