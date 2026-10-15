# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

## Highest priority: security or data loss

- [`claude-trust-publish-after-lock-takeover.md`](claude-trust-publish-after-lock-takeover.md) — A paused trust writer
  may overwrite newer vendor configuration after stale-lock takeover; the vendor interleaving remains unverified.

- [`trash-confirmation-answer-consumes-new-selection.md`](trash-confirmation-answer-consumes-new-selection.md) — If an
  old confirmation event survives a change of pending selection, it could delete the newly selected host’s archives.

- [`menu-request-overrides-independent-focus.md`](menu-request-overrides-independent-focus.md) — Menu arrows can follow
  an older keyboard target after independent focus moves, potentially reaching Delete.

## High priority: material UX degradation

- [`claude-trust-cleanup-delays-desktop-start.md`](claude-trust-cleanup-delays-desktop-start.md) — Retrying trust
  cleanup behind a stale vendor lock can exceed the app's startup deadline.

- [`bare-cache-fetch-conflicts-with-explicit-git-policy.md`](bare-cache-fetch-conflicts-with-explicit-git-policy.md) —
  Hosts using Git’s safe.bareRepository=explicit setting cannot launch sessions in fresh GitHub checkouts.

- [`setup-advice-omits-environment-selected-state-directory.md`](setup-advice-omits-environment-selected-state-directory.md)
  — Following setup’s sign-in advice can return an unusable token or create credentials for a different helm.

- [`abandoned-planners-exhaust-global-planning-slots.md`](abandoned-planners-exhaust-global-planning-slots.md) —
  Repeated update or removal planning can leave adding, updating, and removing hosts stuck until the helm restarts, even
  after the pages close.

- [`host-plans-cannot-receive-keyboard-scroll-focus.md`](host-plans-cannot-receive-keyboard-scroll-focus.md) — Desktop
  keyboard users may be unable to scroll through setup and uninstall plans before approving them.

- [`missing-ca-bundle-blocks-helm-start.md`](missing-ca-bundle-blocks-helm-start.md) — A Linux helm with no usable CA
  bundle exits during startup, even when it would use local payloads.

- [`replace-retry-key-splits-across-controls.md`](replace-retry-key-splits-across-controls.md) — Retrying an unanswered
  Replace through the other control can start another replacement agent.

- [`replace-retries-replay-settled-create-failure.md`](replace-retries-replay-settled-create-failure.md) — Replace can
  replay a permanent creation refusal after its cause is repaired.

- [`uninstall-forgotten-success.md`](uninstall-forgotten-success.md) — A failed uninstall can be reported as successful
  after another client forgets the host.

- [`uninstall-race-first-time-service-setup.md`](uninstall-race-first-time-service-setup.md) — Concurrent setup and
  uninstall can leave services pointing at a deleted executable.

- [`trash-listing-exceeds-control-frame.md`](trash-listing-exceeds-control-frame.md) — An oversized trash listing
  disables both listing and individual cleanup, even below the batch's entry limit.

- [`default-approval-reset-renews-retry-key.md`](default-approval-reset-renews-retry-key.md) — Default-approval
  templates repeatedly renew launch keys, defeating confirmation and unanswered retries; pre-existing.

## Other: correctness, diagnostics, cleanup, or convenience

- [`pin-test-skips-owned-session-cleanup.md`](pin-test-skips-owned-session-cleanup.md) — A failed pin browser test can
  skip its owned session cleanup and leave agents running for later tests.

- [`resume-docs-promise-newest-conversation.md`](resume-docs-promise-newest-conversation.md) — Resume documentation
  promises the new conversation after clearing, beyond the accepted tracking guarantee.

- [`claude-trust-settings-misses-invalidation.md`](claude-trust-settings-misses-invalidation.md) — Already-open Settings
  can retain old trust choices after a shared change; the multi-GUI qualifier needs triage.

- [`claude-trust-tests-compare-lexical-temp-paths.md`](claude-trust-tests-compare-lexical-temp-paths.md) — New trust
  tests assume temporary-directory spellings are canonical and can fail on macOS.

- [`claude-trust-browser-fixtures-escape-cleanup.md`](claude-trust-browser-fixtures-escape-cleanup.md) — A failed
  trust-hint browser test can leave agents or shared preferences behind.

- [`custom-permission-test-skips-template-cleanup.md`](custom-permission-test-skips-template-cleanup.md) — One failed
  template deletion skips the remaining custom-permission or permission-switch browser-test fixtures.

- [`number-shortcut-test-skips-session-cleanup.md`](number-shortcut-test-skips-session-cleanup.md) — A failed expanded
  shortcut test can leave its agent sessions running for subsequent tests.

- [`cached-launch-error-overrides-completed-stop.md`](cached-launch-error-overrides-completed-stop.md) — A session
  stopped during checkout preparation can continue showing a launch error instead of its recorded stopped status, even
  after repeated refreshes.

- [`trash-host-totals-exclude-unchecked-archives.md`](trash-host-totals-exclude-unchecked-archives.md) — A host with
  unreadable archived work can display “0 checkouts · 0 B” instead of an unknown or partial total.

- [`host-picker-folded-letter-cycling-misses-dotted-i.md`](host-picker-folded-letter-cycling-misses-dotted-i.md) —
  Repeatedly typing İ cannot cycle through hosts whose names begin with that letter.

- [`repository-selection-keeps-global-search-launch-block.md`](repository-selection-keeps-global-search-launch-block.md)
  — Choosing a valid repository in the new destination field can leave Launch disabled despite a successful checkout
  preview.

- [`file-recognition-rejects-digit-leading-extensions.md`](file-recognition-rejects-digit-leading-extensions.md) —
  Printed filenames such as backup.7z cannot be hovered or downloaded unless the program includes a directory prefix.

- [`download-cancel-retains-pending-admission.md`](download-cancel-retains-pending-admission.md) — Repeatedly cancelling
  downloads before they start leaves a small amount of helm memory retained until the host reconnects.

- [`checkout-settings-test-restores-before-save-settles.md`](checkout-settings-test-restores-before-save-settles.md) — A
  failed browser test can leave its checkout-folder setting behind and affect later tests.

- [`checkout-override-error-points-to-global-settings.md`](checkout-override-error-points-to-global-settings.md) — A
  checkout failure can tell users to change the folder in Settings even though that host ignores the Settings value.

- [`checkout-folder-save-trims-literal-path.md`](checkout-folder-save-trims-literal-path.md) — Saving an existing
  checkout-folder setting can change which directory it names, even without editing it.

- [`inline-checkout-setup-retains-stale-config-eligibility.md`](inline-checkout-setup-retains-stale-config-eligibility.md)
  — If the checkout folder is cleared while a repository remains selected, the launcher shows a failed preview but does
  not offer its new setup field.

- [`trash-body-lacks-keyboard-scroll-focus.md`](trash-body-lacks-keyboard-scroll-focus.md) — Keyboard-only users can
  lose access to offscreen host information when all trash actions are disabled.

- [`live-sibling-test-returns-before-reap-completion.md`](live-sibling-test-returns-before-reap-completion.md) — The
  regression test can pass before cleanup finishes, weakening its assurance that another terminal survives.

- [`docs-preview-does-not-watch-external-changelog.md`](docs-preview-does-not-watch-external-changelog.md) — Editing
  release notes while the docs preview runs can leave reviewers seeing the previous text until the server restarts.
  Production builds regenerate correctly.

- [`link-click-tests-skip-preference-restoration.md`](link-click-tests-skip-preference-restoration.md) — A failed test
  can leave “program only” enabled for the shared test helm, causing subsequent link-opening tests to fail misleadingly.
  No shipped-product regression was confirmed.

- [`copy-guidance-measures-stale-resize-position.md`](copy-guidance-measures-stale-resize-position.md) — Narrowing the
  window while the notice is showing can collapse it into a thin column with most guidance clipped.

- [`short-main-pane-hides-approval-requests.md`](short-main-pane-hides-approval-requests.md) — In a sufficiently short
  window, waiting approval requests disappear until the user enlarges the window or changes the selected session.

- [`managed-checkout-row-hides-accessible-directory.md`](managed-checkout-row-hides-accessible-directory.md) —
  Screen-reader users lose the working directory when browsing managed-checkout sessions in the default, noncompact
  list.

- [`appearance-test-restoration-skips-session-cleanup.md`](appearance-test-restoration-skips-session-cleanup.md) — A
  failed browser test can leave its session and agent running in the shared test stack.

- [`repository-picker-selection-moves-offscreen.md`](repository-picker-selection-moves-offscreen.md) — When repository
  suggestions overflow, keyboard users cannot see which repository Enter will select.

- [`host-picker-refusal-fixture-enters-version-mismatch.md`](host-picker-refusal-fixture-enters-version-mismatch.md) —
  The new browser test checks recovery from a failed launch with ordinary background refresh disabled, weakening its
  evidence for normal launcher behavior.

- [`trash-fixture-cleanup-compares-lexical-root.md`](trash-fixture-cleanup-compares-lexical-root.md) — On macOS,
  checkout tests can remove their fixture folders while leaving their trash records behind, contaminating later trash
  listings.

- [`native-download-collision-exceeds-filename-limit.md`](native-download-collision-exceeds-filename-limit.md) — The
  desktop cannot download another copy of a file whose name already fills the destination filesystem’s filename limit.

- [`download-regression-fixtures-arm-cleanup-too-late.md`](download-regression-fixtures-arm-cleanup-too-late.md) — A
  failed test setup can leave its agent session running and listed for subsequent browser tests.

- [`sound-tests-skip-remaining-cleanup-and-preferences.md`](sound-tests-skip-remaining-cleanup-and-preferences.md) — A
  failed browser-test cleanup can leave sessions and shared preferences behind, contaminating later tests.

- [`paste-regression-loses-session-cleanup-ownership.md`](paste-regression-loses-session-cleanup-ownership.md) — A
  failed browser-test setup can leave its fixture agent running and then remove its working directory.

- [`checkout-composer-test-skips-configuration-restoration.md`](checkout-composer-test-skips-configuration-restoration.md)
  — One cleanup failure can leave later browser tests using this test’s checkout folder and configuration.

- [`listing-regression-fixtures-escape-cleanup.md`](listing-regression-fixtures-escape-cleanup.md) — A failed browser
  test can leave a fixture agent running and affect subsequent tests.

- [`agent-checkout-test-compares-lexical-tmp.md`](agent-checkout-test-compares-lexical-tmp.md) — New agent checkout
  tests compare canonical destinations with lexical /tmp paths and fail on macOS.

- [`introduction-promises-waiting-alerts-for-every-agent.md`](introduction-promises-waiting-alerts-for-every-agent.md) —
  Introductory artwork promises blocked-agent alerts for agents without waiting detection.

- [`introduction-omits-automatic-install-prerequisites.md`](introduction-omits-automatic-install-prerequisites.md) — The
  automatic-installation claim omits architecture and systemd user-manager prerequisites.

- [`pinned-host-dialog-chrome-can-hide-content.md`](pinned-host-dialog-chrome-can-hide-content.md) — Fixed host-dialog
  headings and answers can exceed the available height, hiding controls or plan content.

- [`single-trash-test-skips-configuration-restoration.md`](single-trash-test-skips-configuration-restoration.md) —
  Failed session cleanup can leave the single-checkout trash test's configuration and Git rewrite in place.

- [`template-regression-fixtures-arm-cleanup-too-late.md`](template-regression-fixtures-arm-cleanup-too-late.md) —
  Template regression tests can fail after storing fixtures but before acquiring cleanup ownership.

- [`header-regression-fixtures-escape-cleanup.md`](header-regression-fixtures-escape-cleanup.md) — A header regression
  test can lose session ownership during readiness or skip its replacement's cleanup after another cleanup failure.

- [`update-check-replacement-loses-focus.md`](update-check-replacement-loses-focus.md) — Installing an update replaces
  the keyboard-focused check button without transferring focus to the restart control.

- [`installation-guide-hides-update-failures.md`](installation-guide-hides-update-failures.md) — The installation guide
  still says automatic update failures are only logged.

- [`tab-retarget-fixture-escapes-cleanup.md`](tab-retarget-fixture-escapes-cleanup.md) — Failure during the new
  tab-retarget test's setup can leave its agent and tabs running.

- [`confirmation-test-optout-survives-cleanup-failure.md`](confirmation-test-optout-survives-cleanup-failure.md) —
  Failed cleanup can leave session confirmations disabled for later browser tests.

- [`template-discovery-fails-permanently-once-reply-exceeds.md`](template-discovery-fails-permanently-once-reply-exceeds.md)
  — A large valid template catalog permanently disables agent discovery.

- [`native-download-quit-leaves-partial-files.md`](native-download-quit-leaves-partial-files.md) — Desktop Quit leaves
  hidden partial downloads with no recovery owner.

- [`file-stat-test-compares-unresolved-temp-path.md`](file-stat-test-compares-unresolved-temp-path.md) — The new
  file-stat unit test compares a canonical result against a lexical `/tmp` path and fails on macOS.

- [`token-rotation-test-invalid-apfs-name.md`](token-rotation-test-invalid-apfs-name.md) — The new token-rotation unit
  test requires an invalid-byte directory name that APFS refuses.

- [`teststate-socket-probe-blocks-sweep.md`](teststate-socket-probe-blocks-sweep.md) — A full Unix-socket backlog can
  block the fixture cleanup probe before either cleanup deadline applies.

- [`custom-permission-model-choice-erases-draft.md`](custom-permission-model-choice-erases-draft.md) — Choosing a model
  or re-selecting the same agent can erase unfinished custom permission flags.

- [`failed-feedback-cleanup-loses-evidence.md`](failed-feedback-cleanup-loses-evidence.md) — Cleanup can delete the
  final test guest after failed feedback checks even when its exported evidence is missing; live evidence loss remains
  conditional.

- [`save-template-composing-escape-discards-draft.md`](save-template-composing-escape-discards-draft.md) — Cancelling an
  input-method candidate can close the save-template panel and lose its unsaved name and checkbox choices.

- [`terminal-right-click-recopies-stale-selection.md`](terminal-right-click-recopies-stale-selection.md) — A right-click
  in a terminal without mouse reporting can replace newer clipboard contents with an old terminal selection.

- [`template-screenshot-implies-deletion-needs-save.md`](template-screenshot-implies-deletion-needs-save.md) — A reader
  could delete a saved template and close the dialog believing the deletion was never committed.

- [`git-discovery-fixtures-inherit-bare-git-config.md`](git-discovery-fixtures-inherit-bare-git-config.md) — Running
  repository-discovery tests can still overwrite configuration belonging to another repository or the user.

- [`browser-git-fixture-can-change-caller-repository.md`](browser-git-fixture-can-change-caller-repository.md) — Running
  the new browser test with an inherited Git repository selector can change a developer’s repository instead of its
  temporary fixture.

- [`paste-guide-overstates-control-byte-protection.md`](paste-guide-overstates-control-byte-protection.md) — The paste
  guide promises unseen-keystroke protection even though control bytes such as Ctrl-C remain deliverable.

- [`introduction-promises-universal-reboot-resume.md`](introduction-promises-universal-reboot-resume.md) — The
  introduction promises conversation recovery after reboot for sessions without Resume reporting.
