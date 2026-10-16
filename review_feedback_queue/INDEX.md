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

- [`abandoned-planners-exhaust-global-planning-slots.md`](abandoned-planners-exhaust-global-planning-slots.md) —
  Repeated update or removal planning can leave adding, updating, and removing hosts stuck until the helm restarts, even
  after the pages close.

- [`host-plans-cannot-receive-keyboard-scroll-focus.md`](host-plans-cannot-receive-keyboard-scroll-focus.md) — Desktop
  keyboard users may be unable to scroll through setup and uninstall plans before approving them.

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

- [`claude-trust-settings-misses-invalidation.md`](claude-trust-settings-misses-invalidation.md) — Already-open Settings
  can retain old trust choices after a shared change; the multi-GUI qualifier needs triage.

- [`claude-trust-browser-fixtures-escape-cleanup.md`](claude-trust-browser-fixtures-escape-cleanup.md) — A failed
  trust-hint browser test can leave agents or shared preferences behind.

- [`number-shortcut-test-skips-session-cleanup.md`](number-shortcut-test-skips-session-cleanup.md) — A failed expanded
  shortcut test can leave its agent sessions running for subsequent tests.

- [`repository-selection-keeps-global-search-launch-block.md`](repository-selection-keeps-global-search-launch-block.md)
  — Choosing a valid repository in the new destination field can leave Launch disabled despite a successful checkout
  preview.

- [`file-recognition-rejects-digit-leading-extensions.md`](file-recognition-rejects-digit-leading-extensions.md) —
  Printed filenames such as backup.7z cannot be hovered or downloaded unless the program includes a directory prefix.

- [`download-cancel-retains-pending-admission.md`](download-cancel-retains-pending-admission.md) — Repeatedly cancelling
  downloads before they start leaves a small amount of helm memory retained until the host reconnects.

- [`checkout-settings-test-restores-before-save-settles.md`](checkout-settings-test-restores-before-save-settles.md) — A
  failed browser test can leave its checkout-folder setting behind and affect later tests.

- [`checkout-folder-save-trims-literal-path.md`](checkout-folder-save-trims-literal-path.md) — Saving an existing
  checkout-folder setting can change which directory it names, even without editing it.

- [`inline-checkout-setup-retains-stale-config-eligibility.md`](inline-checkout-setup-retains-stale-config-eligibility.md)
  — If the checkout folder is cleared while a repository remains selected, the launcher shows a failed preview but does
  not offer its new setup field.

- [`trash-body-lacks-keyboard-scroll-focus.md`](trash-body-lacks-keyboard-scroll-focus.md) — Keyboard-only users can
  lose access to offscreen host information when all trash actions are disabled.

- [`link-click-tests-skip-preference-restoration.md`](link-click-tests-skip-preference-restoration.md) — A failed test
  can leave “program only” enabled for the shared test helm, causing subsequent link-opening tests to fail misleadingly.
  No shipped-product regression was confirmed.

- [`short-main-pane-hides-approval-requests.md`](short-main-pane-hides-approval-requests.md) — In a sufficiently short
  window, waiting approval requests disappear until the user enlarges the window or changes the selected session.

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

- [`sound-tests-skip-remaining-cleanup-and-preferences.md`](sound-tests-skip-remaining-cleanup-and-preferences.md) — A
  failed browser-test cleanup can leave sessions and shared preferences behind, contaminating later tests.

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

- [`update-check-replacement-loses-focus.md`](update-check-replacement-loses-focus.md) — Installing an update replaces
  the keyboard-focused check button without transferring focus to the restart control.

- [`tab-retarget-fixture-escapes-cleanup.md`](tab-retarget-fixture-escapes-cleanup.md) — Failure during the new
  tab-retarget test's setup can leave its agent and tabs running.

- [`template-discovery-fails-permanently-once-reply-exceeds.md`](template-discovery-fails-permanently-once-reply-exceeds.md)
  — A large valid template catalog permanently disables agent discovery.

- [`native-download-quit-leaves-partial-files.md`](native-download-quit-leaves-partial-files.md) — Desktop Quit leaves
  hidden partial downloads with no recovery owner.

- [`custom-permission-model-choice-erases-draft.md`](custom-permission-model-choice-erases-draft.md) — Choosing a model
  or re-selecting the same agent can erase unfinished custom permission flags.

- [`terminal-right-click-recopies-stale-selection.md`](terminal-right-click-recopies-stale-selection.md) — A right-click
  in a terminal without mouse reporting can replace newer clipboard contents with an old terminal selection.

- [`template-screenshot-implies-deletion-needs-save.md`](template-screenshot-implies-deletion-needs-save.md) — A reader
  could delete a saved template and close the dialog believing the deletion was never committed.

- [`browser-git-fixture-can-change-caller-repository.md`](browser-git-fixture-can-change-caller-repository.md) — Running
  the new browser test with an inherited Git repository selector can change a developer’s repository instead of its
  temporary fixture.

- [`introduction-promises-universal-reboot-resume.md`](introduction-promises-universal-reboot-resume.md) — The
  introduction promises conversation recovery after reboot for sessions without Resume reporting.
