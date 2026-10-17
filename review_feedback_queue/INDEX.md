# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

## Highest priority: security or data loss

- [`pending-menu-arrows-can-ignore-independent-focus.md`](pending-menu-arrows-can-ignore-independent-focus.md) — Pending
  arrow moves can still override independent focus before activation; historical acceptance is absent from the
  specification and triage ledger.

## High priority: material UX degradation

- [`abandoned-planners-exhaust-global-planning-slots.md`](abandoned-planners-exhaust-global-planning-slots.md) —
  Repeated update or removal planning can leave adding, updating, and removing hosts stuck until the helm restarts, even
  after the pages close.

- [`uninstall-forgotten-success.md`](uninstall-forgotten-success.md) — A failed uninstall can be reported as successful
  after another client forgets the host.

- [`uninstall-race-first-time-service-setup.md`](uninstall-race-first-time-service-setup.md) — Concurrent setup and
  uninstall can leave services pointing at a deleted executable.

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
  headings and answers can exceed the available height, hiding controls or plan content.

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
