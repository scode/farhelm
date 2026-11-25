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

- [`settings-close-can-save-unlisted-drafts.md`](settings-close-can-save-unlisted-drafts.md) — Save and Close can write
  an unlisted form and clear the saved alert token outside the user’s confirmation.

- [`settings-queued-save-can-commit-later-draft.md`](settings-queued-save-can-commit-later-draft.md) — A queued Settings
  save can commit later edits to an approved form, including changes that clear a saved alert token.

- [`settings-footer-can-save-the-wrong-page.md`](settings-footer-can-save-the-wrong-page.md) — A stale Settings footer
  can save the wrong page’s draft and overwrite the stored Tensorlake key.

- [`hook-settings-toggle-uses-unshown-state.md`](hook-settings-toggle-uses-unshown-state.md) — Space can save the
  all-hosts hook opt-in while the switch still shows the on state the user intended to turn off.

- [`trailing-custom-agent-tilde-command-kills-live-agent-before-failing.md`](trailing-custom-agent-tilde-command-kills-live-agent-before-failing.md)
  — Expanding a custom agent's ~/ command only during launch preparation kills a live agent on Restart (and allocates
  checkouts on Create) before failing if HOME is missing or the expanded path cannot be resolved (borderline
  highest/high; durable conversation ID stays preserved).

- [`agent-environment-launch-skips-place-host-incarnation-pin.md`](agent-environment-launch-skips-place-host-incarnation-pin.md)
  — An approved agent container-environment launch can create the container on a different machine than the card named
  if the place host was retargeted or re-identified while the card waited; the environment-launch path never pins the
  place host's connection (the ordinary-create path does).

- [`uninstall-newline-stripping-defeats-state-inside-lib-guard.md`](uninstall-newline-stripping-defeats-state-inside-lib-guard.md)
  — Uninstall's `readlink -f | tr -d '\n'` strips embedded newlines, corrupting the spellings every containment/identity
  guard consumes; a constructed dual-newline layout can make uninstall delete the user's data directory (traced, not
  run; deliberate-setup drop rejected by independent check).

- [`installer-leftover-cleanup-deletes-unverified-previous-app.md`](installer-leftover-cleanup-deletes-unverified-previous-app.md)
  — After an interrupted install, leftover cleanup deletes the displaced "previous" app without verifying ownership
  whenever the place is occupied — the exact case the restoration promise was written for; can destroy a foreign app
  (double-swap trigger rare; single-swap variant deletes the user's reinstallable prior install).

- [`trailing-legacy-claude-settings-v036-hosts-never-cleaned.md`](trailing-legacy-claude-settings-v036-hosts-never-cleaned.md)
  — A Tensorlake host set up by 0.36.0 and not connected under 0.37-0.43 keeps Farhelm's old Claude endpoint URL and
  token in Claude's settings for good; per SPEC_impl.md they can take precedence and receive a new token (Claude Code
  precedence unverified).

- [`trailing-legacy-codex-config-v036-hosts-never-cleaned.md`](trailing-legacy-codex-config-v036-hosts-never-cleaned.md)
  — The same 0.36.0-era Tensorlake hosts keep Farhelm's old Codex endpoint and token in Codex's config, so a newly
  selected login or endpoint may not take effect, and per SPEC_impl.md the old token can reach a new tokenless endpoint
  (Codex merge behavior unverified).

## High priority: material UX degradation

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

- [`hook-settings-close-hides-opt-out-failure.md`](hook-settings-close-hides-opt-out-failure.md) — Closing Settings
  while an opt-out saves can hide the refusal and leave automatic hook setup active.

- [`trailing-settings-trust-help-omits-agent-requested-launches.md`](trailing-settings-trust-help-omits-agent-requested-launches.md)
  — The Settings pages for Codex, Muse, Pi, and Antigravity omit that their workspace-trust switch also applies to
  agent-requested launches, which run without an approval card on Always-allow hosts.

- [`trailing-custom-agent-settings-close-save-silent-fail-during-delete-confirm.md`](trailing-custom-agent-settings-close-save-silent-fail-during-delete-confirm.md)
  — If a user edits a custom agent, clicks Delete to show the confirmation buttons, and then closes Settings and chooses
  Save in the 'Save changes before closing?' dialog, saving silently fails without displaying any error and leaves
  Settings stuck open on the delete confirmation (and leaves any remaining dirty Settings forms unsaved).

- [`trailing-custom-agent-settings-delete-confirmation-bypasses-ask-inline.md`](trailing-custom-agent-settings-delete-confirmation-bypasses-ask-inline.md)
  — The custom-agent Delete confirmation in Settings bypasses the shared inline prompt, so Escape closes Settings
  instead of cancelling, Cancel loses default focus, and external edits can change the agent while the unnamed
  confirmation stays open (borderline high/highest; registration ID stays fixed).

- [`trailing-custom-agent-settings-delete-dirty-leaves-ghost-draft.md`](trailing-custom-agent-settings-delete-dirty-leaves-ghost-draft.md)
  — If a user makes any unsaved edit to a registered custom agent and then deletes it, the deleted agent remains in the
  Settings sidebar as an unsaved draft instead of being removed; choosing Save when closing Settings then silently
  re-registers the agent they just deleted.

- [`trailing-custom-agents-share-one-remembered-wrapper-prefix.md`](trailing-custom-agents-share-one-remembered-wrapper-prefix.md)
  — Remembered wrapper prefixes are stored per agent type, but every registered custom agent shares a single bucket.

- [`trailing-custom-to-custom-switch-retains-previous-wrapper-prefix.md`](trailing-custom-to-custom-switch-retains-previous-wrapper-prefix.md)
  — Switching between two custom agents in the launcher or via templates retains the first agent's wrapper prefix and
  explicit flag under collapsed More options instead of resetting it (borderline high/highest if a wrapper injects
  agent-specific context).

- [`trailing-delete-custom-agent-leaks-remembered-wrapper-prefix.md`](trailing-delete-custom-agent-leaks-remembered-wrapper-prefix.md)
  — When a user launches a custom agent with a wrapper prefix and later deletes that custom agent, Farhelm keeps the
  wrapper prefix remembered and silently pre-fills it when selecting another or newly registered custom agent.

- [`trailing-restart-with-custom-reporting-and-offer-use-stale-row-launch.md`](trailing-restart-with-custom-reporting-and-offer-use-stale-row-launch.md)
  — When a user edits a registered custom agent's reporting or resumption setting and uses Restart with to apply it, a
  conversation ID reported right after the restarted agent starts is judged against the previous launch's reporting
  setting, and the Restart reply (and listings before launch update completes) report a stale restart offer.

- [`trailing-templates-editor-hides-custom-effort-and-refuses-agentless-modifier.md`](trailing-templates-editor-hides-custom-effort-and-refuses-agentless-modifier.md)
  — When a launch template has a custom effort value without an agent type (a valid modifier template) or after
  switching its agent type from a custom agent to a built-in agent, the Templates editor displays Effort as 'Default'
  instead of showing the stored custom value, refuses to save, and (when Effort renders as a dropdown) ignores clicking
  'Default' so the user cannot reset it directly.

- [`trailing-templates-editor-hides-custom-yolo-control-and-saves-false.md`](trailing-templates-editor-hides-custom-yolo-control-and-saves-false.md)
  — In the Templates editor, adding Approvals to a Command template that declares a custom agent (or opening a stored
  one whose Approvals was set to 'Keeps safety checks') displays only the static message 'Custom agents always run in
  YOLO mode.' with no toggle and lets the user save, yet actually saves YOLO as off ('Not YOLO').

- [`trailing-replace-with-recent-named-fill-enter-replaces.md`](trailing-replace-with-recent-named-fill-enter-replaces.md)
  — In Replace with, a screen reader announces a highlighted recent setup as "Fill from recent setup" while Enter on it
  replaces and deletes the source session; highest if the "⏎ Replace" key cap is not announced with it.

## Other: correctness, diagnostics, cleanup, or convenience

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

- [`sound-preview-regression-misses-separate-mappings.md`](sound-preview-regression-misses-separate-mappings.md) — The
  preview test exercises one sound and can miss a wrong mapping on the three other Play controls.

- [`app-settings-shared-upgrade-option-has-local-heading.md`](app-settings-shared-upgrade-option-has-local-heading.md) —
  The App heading says This Mac even though it contains a helm-shared highlights setting.

- [`agent-settings-search-omits-agent-name.md`](agent-settings-search-omits-agent-name.md) — Settings search can give
  different agents' pages or hook controls the same accessible name.

- [`pi-settings-tests-miss-trust-route.md`](pi-settings-tests-miss-trust-route.md) — Pi's Settings tests no longer check
  a successful trust read or the absence of a hook hint control.

- [`agent-settings-tests-miss-late-trust-replies.md`](agent-settings-tests-miss-late-trust-replies.md) — The per-agent
  Settings tests can miss a trust reply being applied to the page opened after the request.

- [`agent-settings-search-test-omits-focus-target.md`](agent-settings-search-test-omits-focus-target.md) — The Settings
  search test can pass with a trust result that names the right agent and focuses the wrong control.

- [`shortcuts-chord-during-composition.md`](shortcuts-chord-during-composition.md) — The Keyboard shortcuts chord can
  interrupt an input method if native handling clears its mark first or the action cannot query the focused field.

- [`trailing-settings-containers-reload-omits-loading-keeps-error.md`](trailing-settings-containers-reload-omits-loading-keeps-error.md)
  — After a failed refresh in Settings -> Containers, deleting the last leftover or clicking Refresh hides the reading
  indicator and leaves only the old refresh error visible while the new read waits.

- [`trailing-masked-input-test-omits-a11y-value-checks.md`](trailing-masked-input-test-omits-a11y-value-checks.md) — The
  masked-input test checks painted text and accessible names but never inspects accessible values or runs with
  accessibility active, leaving screen-reader secret exposure unguarded.

- [`trailing-credential-delivery-test-stale-claude-settings-assertion.md`](trailing-credential-delivery-test-stale-claude-settings-assertion.md)
  — The backend-failure delivery test still asserts .claude/settings.json is absent (always true now), leaving failed
  agent-credentials and Codex config targets unverified before and after retry.

- [`trailing-host-settings-tailscale-switch-refusal-prefix-and-stuck-error.md`](trailing-host-settings-tailscale-switch-refusal-prefix-and-stuck-error.md)
  — Toggling Tailscale in Host settings during a launch shows an unprefixed sentence fragment, stacks any earlier
  failure above it, and leaves the launch refusal stuck even after an in-flight toggle succeeds.

- [`trailing-apply-template-custom-command-yolo-diverges-from-gui.md`](trailing-apply-template-custom-command-yolo-diverges-from-gui.md)
  — Applying launch templates that declare a custom agent on the Command tab behaves differently in the desktop app and
  the CLI.

- [`trailing-check-template-shape-allows-custom-agent-yolo-false.md`](trailing-check-template-shape-allows-custom-agent-yolo-false.md)
  — Both the CLI and the desktop Templates editor allow saving a Command launch template that declares a custom agent
  with YOLO turned off, creating a saved template that Farhelm always refuses when you try to apply it (and in the
  desktop editor the Approvals toggle is hidden behind a static YOLO message, so the user cannot even see or fix the
  saved non-YOLO flag without removing the field).

- [`trailing-check-template-shape-skips-validate-model-on-custom-agent.md`](trailing-check-template-shape-skips-validate-model-on-custom-agent.md)
  — Saving a custom-agent launch template from the CLI or API accepts invalid model values (such as uppercase letters,
  spaces, control characters, or empty strings) even though Farhelm refuses those same values in the desktop Templates
  editor and whenever the template is applied.

- [`trailing-custom-agent-ancestor-correction-test-misses-pin-correction.md`](trailing-custom-agent-ancestor-correction-test-misses-pin-correction.md)
  — The end-to-end test that claims to verify a custom agent parent can correct a child-first conversation report would
  still pass even if custom-agent ancestor correction and its user-facing correction notification were completely
  broken.

- [`trailing-custom-agent-restart-readiness-none-misreports-no-resume-command.md`](trailing-custom-agent-restart-readiness-none-misreports-no-resume-command.md)
  — For a registered custom agent session created without session resumption enabled, the disabled Restart with menu
  item tells the user "Custom sessions can't be resumed" instead of explaining that this launch has no stored resume
  command and pointing them to Clone or Replace with.

- [`trailing-custom-agent-settings-busy-save-button-and-saw-saved-restore.md`](trailing-custom-agent-settings-busy-save-button-and-saw-saved-restore.md)
  — While a custom-agent save is in flight, a pushed catalog update can disable the busy Save button (causing it to drop
  keyboard focus) or overwrite the fields being saved with a subsequent external update.

- [`trailing-custom-agent-settings-dirty-close-prompt-collapses-multiple-agents.md`](trailing-custom-agent-settings-dirty-close-prompt-collapses-multiple-agents.md)
  — When two or more custom-agent pages have unsaved changes, the 'Save changes before closing?' prompt lists only a
  single 'Custom agent' and jumps Settings to whichever custom-agent draft sorts first rather than staying on the page
  the user was viewing.

- [`trailing-custom-agent-settings-launch-preview-breaks-bidi-isolation.md`](trailing-custom-agent-settings-launch-preview-breaks-bidi-isolation.md)
  — If a custom agent's command contains right-to-left characters, the Start and Resume command previews in Settings can
  visually reorder Farhelm's --model, --effort, and --resume flags with the command text instead of keeping them
  isolated.

- [`trailing-custom-agent-settings-omits-per-field-unsaved-dots.md`](trailing-custom-agent-settings-omits-per-field-unsaved-dots.md)
  — Editing Display name, ID, Command, or capability checkboxes on a custom-agent page marks the sidebar row and footer
  as unsaved, but never shows the unsaved dot next to any edited field.

- [`trailing-custom-agent-settings-save-delete-collapses-retired-and-missing-store.md`](trailing-custom-agent-settings-save-delete-collapses-retired-and-missing-store.md)
  — When saving or deleting a custom agent cannot start because the helm session is resetting or unavailable, Settings
  misleadingly reports 'Another change to this custom agent is in progress' (or fails silently with no error when no
  operation store exists), and when the key is busy it ignores Farhelm's shared busy-reason wording.

- [`trailing-custom-agent-settings-test-locks-in-kick-and-misses-dirty-delete.md`](trailing-custom-agent-settings-test-locks-in-kick-and-misses-dirty-delete.md)
  — The Settings test for custom-agent lifecycle locks in an unhelpful navigation jump that kicks the user off their
  draft page when an external delete arrives mid-save, and only exercises Delete on a clean draft, missing the bug where
  confirming Delete on a modified custom agent leaves a ghost entry in the Settings sidebar.

- [`trailing-custom-report-forwarder-lock-test-lacks-witness.md`](trailing-custom-report-forwarder-lock-test-lacks-witness.md)
  — The integration test for running a custom agent's conversation reporter through the app forwarder during an update
  or uninstall does not actually prove that the forwarder prevented the CLI from starting, so a regression that bypassed
  the lock or failed before locking would still pass.

- [`trailing-launcher-custom-agent-yolo-pill-omits-escalation-outline.md`](trailing-launcher-custom-agent-yolo-pill-omits-escalation-outline.md)
  — When switching from a safer agent (such as Guarded Claude or Codex) to a registered custom agent (which always
  counts as YOLO), the red permission-escalation banner appears at the top of the launcher, but the custom agent's YOLO
  permission control does not show the required thick red escalation outline.

- [`trailing-launcher-target-names-command-custom-agent-outside-restart-with.md`](trailing-launcher-target-names-command-custom-agent-outside-restart-with.md)
  — When creating, cloning, or replacing a session on the Command tab with a registered custom agent declared, the main
  action's accessible name includes the custom agent's name ("Launch Command · <name> · <host> · <folder>"), whereas
  declaring any built-in agent or generic Custom omits the agent name ("Launch Command · <host> · <folder>").

- [`trailing-more-options-locked-open-without-field-on-custom-restart-no-resumption.md`](trailing-more-options-locked-open-without-field-on-custom-restart-no-resumption.md)
  — In Restart with for a custom-agent session that has no captured conversation, if the custom agent's registration has
  resumption disabled (or is deleted), the More options disclosure is locked open with "A setting here is required or
  needs attention", yet shows neither the conversation field nor any error inside it.

- [`trailing-radio-group-chip-flex-none-overflows-long-custom-agent-name.md`](trailing-radio-group-chip-flex-none-overflows-long-custom-agent-name.md)
  — A registered custom agent with a long display name produces a chip in the session launcher's Custom group that
  cannot shrink or ellipsize to the dialog width, overflowing and getting clipped at the right edge of the launcher.

- [`trailing-refresh-custom-capabilities-treats-unloaded-collection-as-deleted.md`](trailing-refresh-custom-capabilities-treats-unloaded-collection-as-deleted.md)
  — If the helm's custom-agents catalog is unavailable or has not loaded yet, the launcher treats any selected custom
  agent (on the Agent or Command tab) as deleted—blocking launch with "no longer registered" / "was deleted" messages,
  hiding or erroring its conversation-ID field, and prompting the user to replace a registration that was not actually
  deleted.

- [`trailing-restart-with-mishandles-custom-agent-with-resumption-disabled.md`](trailing-restart-with-mishandles-custom-agent-with-resumption-disabled.md)
  — When you open Restart with on an existing custom-agent session after turning off "Supports resumption" for that
  custom agent in Settings, the launcher gives contradictory or broken behavior: if the session has no captured
  conversation, it tells you "Enter a conversation to restart" even though it hides the Conversation ID input and cannot
  resume; if the session has a captured conversation, Restart stays enabled and submits a request that the
  helm/supervisor then rejects.

- [`trailing-session-menu-and-quick-switcher-show-generic-custom-agent.md`](trailing-session-menu-and-quick-switcher-show-generic-custom-agent.md)
  — Sessions launched with a registered custom agent show the generic label "Custom agent" in both the session row's
  actions menu header and the quick switcher's agent column, instead of the custom agent's current display name (or its
  saved ID after the registration is deleted).

- [`trailing-settings-custom-agent-deleted-banner-omits-a11y-role-and-label.md`](trailing-settings-custom-agent-deleted-banner-omits-a11y-role-and-label.md)
  — When a custom agent is deleted externally while the user has unsaved edits, the 'This registration was deleted.
  Register it again to keep these edits.' banner is omitted from the accessibility tree so assistive technology never
  exposes or announces it.

- [`trailing-settings-custom-agent-duplicate-host-sentence-id.md`](trailing-settings-custom-agent-duplicate-host-sentence-id.md)
  — Whenever a registered custom agent's Settings page is open, both its sidebar navigation row and its page heading
  emit a label with the same flat element ID in GPUI's driver node list, violating Farhelm's unique-ID rule for host
  sentences and making element-ID lookups in tests and the gpui-agent automation bridge ambiguous.

- [`trailing-settings-custom-agent-nav-tooltip-omits-bidi-isolation.md`](trailing-settings-custom-agent-nav-tooltip-omits-bidi-isolation.md)
  — Hovering a custom agent's row in the Settings sidebar shows a tooltip that embeds the agent's display name directly
  in a plain string instead of isolating its text direction.

- [`trailing-settings-custom-agents-unavailable-collection-silently-appears-empty.md`](trailing-settings-custom-agents-unavailable-collection-silently-appears-empty.md)
  — If the helm fails to read the custom-agents catalog (for example due to a database error or a corrupted catalog
  row), Settings hides the failure and shows an empty Custom agents section with only '+ Add custom agent', leaving the
  user with no explanation for why their agents disappeared or why re-adding an existing ID fails as already taken.

- [`trailing-settings-heading-lacks-min-w-0-and-truncate-for-long-custom-agent-name.md`](trailing-settings-heading-lacks-min-w-0-and-truncate-for-long-custom-agent-name.md)
  — Opening a custom agent with a long display name in Settings can push the page's scope label ("Every host on this
  helm") past the right edge of the dialog or expand the pinned heading vertically over multiple lines, reducing space
  for the scrollable form.

- [`trailing-settings-search-omits-registered-custom-agent-sections.md`](trailing-settings-search-omits-registered-custom-agent-sections.md)
  — Searching in Settings for a registered custom agent's display name or ID always returns no matches and hides every
  registered custom agent from the Settings sidebar, even though each registered custom agent is its own Settings
  section.

- [`trailing-stacked-quick-launch-eligible-accepts-custom-agent-without-id.md`](trailing-stacked-quick-launch-eligible-accepts-custom-agent-without-id.md)
  — A launch template on the Agent tab that selects the generic Custom agent type without naming a registered custom
  agent ID can be marked as the quick-launch template (⚡) or treated as ready to quick-launch, even though Farhelm
  cannot launch an Agent-tab custom session without a registered agent ID.

- [`trailing-supervisor-positional-pin-tests-omit-custom-agent.md`](trailing-supervisor-positional-pin-tests-omit-custom-agent.md)
  — Custom agents share the supervisor's positional process-pin admission, ancestor-correction notification, and
  named-restart capture-clearing code, but none of those supervisor behaviors are tested for custom agents.

- [`trailing-template-apply-custom-to-custom-misses-switched-agent.md`](trailing-template-apply-custom-to-custom-misses-switched-agent.md)
  — If you switch in the launcher from a guarded agent (like Codex) to a custom agent — raising the red
  permission-escalation banner with Undo — and then apply a launch template that selects a different custom agent, the
  old escalation banner stays open instead of closing.

- [`trailing-templates-editor-allows-generic-custom-on-agent-template.md`](trailing-templates-editor-allows-generic-custom-on-agent-template.md)
  — Switching a Command template with generic 'Custom' to an Agent template shows 'Custom (not offered)' in the Agent
  type dropdown, yet the editor still allows saving it without an error.

- [`trailing-tensorlake-key-replacement-keeps-old-discovered-sandboxes.md`](trailing-tensorlake-key-replacement-keeps-old-discovered-sandboxes.md)
  — When you replace your saved Tensorlake API key with a different key in Settings without clicking Clear first, the
  list of sandboxes not on this helm keeps showing leftover sandboxes from the old key while the new check runs — and
  keeps showing them indefinitely with active Delete buttons if the new key fails to list sandboxes (for example, if the
  new key is invalid or Tensorlake is unreachable).

- [`trailing-ui-channel-tests-omit-custom-agents-mutation-round-trip.md`](trailing-ui-channel-tests-omit-custom-agents-mutation-round-trip.md)
  — Changes to registered custom agents are wired to push over the UI channel, but the helm test suite never exercises a
  live create, edit, or delete of a custom agent end-to-end over the UI channel to verify that the served payload
  projects and decodes cleanly.

- [`trailing-unknown-agent-typo-fails-with-misleading-custom-flag-error.md`](trailing-unknown-agent-typo-fails-with-misleading-custom-flag-error.md)
  — When an agent mistypes a built-in agent name (such as `--agent cluade` or `--agent opencode`) together with
  `--prompt`, `--permissions guarded`, or a built-in model name containing `.` or `/` (such as `gpt-6.2-sol`), Farhelm
  refuses the command with a misleading Custom-agent flag error (e.g., "--prompt/--prompt-file is not offered by this
  agent") instead of telling the agent that the agent name is unknown and listing the built-in types.

- [`trailing-yolo-confirmation-custom-agent-reads-every-custom-permission.md`](trailing-yolo-confirmation-custom-agent-reads-every-custom-permission.md)
  — When you launch or plain-replace a registered custom agent on a host that asks before YOLO launches, the
  confirmation dialog explains the YOLO classification as "Farhelm treats every Custom permission choice as YOLO."
  Because "Custom" is also the name of the Custom permission flags mode, this reads as if the launch used custom
  permission flags rather than a registered custom agent.

- [`ui-channel-displaced-notice-lost-to-reader-race.md`](ui-channel-displaced-notice-lost-to-reader-race.md) — A
  displaced UI channel's Displaced notice can be dropped by a reader/writer drain race, so the displaced window
  reconnects and steals the seat back (latent: a second UI cannot arise in the shipped embedded app).

- [`ui-channel-command-backlog-unbounded-bytes.md`](ui-channel-command-backlog-unbounded-bytes.md) — The UI channel's
  1000-command backlog allowance bounds count but not bytes, letting a credential-holding client buffer ~4 GiB of
  command bodies and OOM the helm (hardening).

- [`upload-cancel-mid-publication-reported-not-published.md`](upload-cancel-mid-publication-reported-not-published.md) —
  An upload cancelled while the host is publishing it can be reported to the user as "definitely not published" although
  the host's answer says the outcome is unknown (reachability premise unverified; recorded higher).

- [`helm-agent-create-dispatch-logs-asking-session-unescaped.md`](helm-agent-create-dispatch-logs-asking-session-unescaped.md)
  — The helm's agent-create audit log writes the supervisor-reported asking session id unescaped (every sibling site
  escapes it), allowing forged log lines or a disguised asker.

- [`preapproved-agent-requests-skip-deleting-session-refusal.md`](preapproved-agent-requests-skip-deleting-session-refusal.md)
  — While a session is being deleted, a new request from it that the host already allows without asking is carried out
  instead of refused, deviating from the deny-until-delete-finishes contract (no new authority granted).

- [`provisioning-orphan-sweep-deletes-concurrent-helm-upload.md`](provisioning-orphan-sweep-deletes-concurrent-helm-upload.md)
  — Two helms provisioning the same host concurrently can delete each other's active staging temporary via the orphan
  sweep (no liveness check); the victim run fails visibly and retryably (cross-helm premise unverified).

- [`provisioning-run-panic-strands-host-busy.md`](provisioning-run-panic-strands-host-busy.md) — A provisioning run task
  has no panic guard; a mid-run panic leaves the host marked busy with a Running run forever, blocking further
  Setup/Update/Uninstall until Remove or restart (reachable-panic premise unverified).

- [`probe-group-kill-after-reap-pid-reuse.md`](probe-group-kill-after-reap-pid-reuse.md) — A failing supervisor probe
  SIGKILLs its saved process-group id after reaping the child, so a reused pid could make the kill hit an unrelated
  group (microsecond window, route not established; sibling helper kills before reap).

- [`update-identity-unverified-misleading-plan-again-remedy.md`](update-identity-unverified-misleading-plan-again-remedy.md)
  — Update on a host whose supervisor reports no identity (record has one) is accepted then fails with "identity changed
  / plan again", a futile remedy; the real remedies are not named (rare state, safe refusal).

- [`installer-fallback-lock-world-openable.md`](installer-fallback-lock-world-openable.md) — On a lock-unsupported
  volume the installer's fallback lock file is created 0644, so another local account can hold it and block the owner's
  installs (DoS); umask 0 also allows substituting it (premises named).

- [`release-payload-cache-misses-private-dir-hardening.md`](release-payload-cache-misses-private-dir-hardening.md) — The
  release payload cache is created umask-dependent without symlink refusal, unlike the deliberately hardened sibling
  directory cache; protected today only by the 0700 state dir (hardening consistency).

- [`podman-settle-claim-incumbent-state-non-atomic.md`](podman-settle-claim-incumbent-state-non-atomic.md) — A place
  claim transfer reads the incumbent's live identity refusal non-atomically; if it resolves to Connected in the window,
  the claim still moves (benign routing shift between twins on the same machine; resource guards hold).

- [`supervisor-error-kind-sealed-not-decodable.md`](supervisor-error-kind-sealed-not-decodable.md) — The supervisor's
  error-kind encoder emits "sealed" but the decoder has no arm for it, so a failed reservation carrying Sealed could
  never be read back (latent: no current writer emits it; trivial round-trip fix).

- [`reconcile-archive-moves-creation-time-unconfirmed-checkout.md`](reconcile-archive-moves-creation-time-unconfirmed-checkout.md)
  — reconcile_archive lacks the creation-time-unconfirmed refusal archive_move has, so recovering a legacy interrupted
  archive can move an unconfirmed checkout to trash (displaced/recoverable, not destroyed; legacy-journal premise).

- [`trailing-drift-copyback-runs-while-agent-may-live.md`](trailing-drift-copyback-runs-while-agent-may-live.md) — A
  Grok or Antigravity drift case copies refreshed vendor files back once its terminal hosts end, even when the agent may
  still be running (failed Delete sweep, failed create, detached process), so a later vendor save such as a refreshed
  login can be lost without a note.

- [`trailing-drift-runner-drops-requested-harness.md`](trailing-drift-runner-drops-requested-harness.md) — The drift
  runner silently skips an explicitly requested agent that has no case in the chosen category and can exit successfully
  without checking it.

- [`trailing-drift-panic-skips-copyback.md`](trailing-drift-panic-skips-copyback.md) — A panic inside a Grok or
  Antigravity drift case skips copying a refreshed vendor login back to the maintainer's real config, with no note
  (pre-existing).

- [`trailing-drift-failed-copyback-passes-case.md`](trailing-drift-failed-copyback-passes-case.md) — A failed copy-back
  of a refreshed vendor login is only a console note and the drift case still reports passed, so the maintainer may be
  signed out unnoticed.

- [`trailing-antigravity-restart-selector-after-custom-dashdash.md`](trailing-antigravity-restart-selector-after-custom-dashdash.md)
  — With Custom permission flags ending in `--`, Antigravity Restart places the conversation choice after `--`, so
  Restart cannot resume and the new drift case always fails (pre-existing product defect).

- [`trailing-drift-cursor-initial-prompt-never-passes.md`](trailing-drift-cursor-initial-prompt-never-passes.md) —
  Cursor's launch-prompt drift case always ends "could not run" even when Cursor accepts the prompt, so the check can
  never pass.

- [`trailing-goose-named-start-reports-contrary-to-spec.md`](trailing-goose-named-start-reports-contrary-to-spec.md) — A
  named Goose start into a conversation Farhelm created appears to capture it and regain plain Restart, contrary to the
  launcher text and SPEC.md (pre-existing; admission premise unverified).

- [`trailing-drift-capture-timing-check-too-coarse.md`](trailing-drift-capture-timing-check-too-coarse.md) — The
  capture-timing drift check treats prompt-timed and reply-timed agents alike, so a vendor drift between "at the first
  prompt" and "after the first reply" would go unnoticed.

- [`trailing-release-notes-recapture-parks-on-tooltip-field.md`](trailing-release-notes-recapture-parks-on-tooltip-field.md)
  — Recapturing the v0.42.0 Antigravity release-notes picture parks the pointer on the Conversation field, so its
  tooltip would be drawn into the picture (from source; not seen in a capture).

- [`trailing-brick-cleanup-refuses-untested-remote-mac-pair.md`](trailing-brick-cleanup-refuses-untested-remote-mac-pair.md)
  — After a stop-after-P0 request, brick-test cleanup refuses a prepared but untested remote Mac pair and demands a
  retention decision for machines with no evidence.

- [`trailing-file-disclosure-holdback-silent-click.md`](trailing-file-disclosure-holdback-silent-click.md) — When a
  terminal file's details card would cover the pointer, it flashes and vanishes and a click on the path does nothing,
  with no "not visible" explanation (pre-existing).

- [`trailing-file-prepaint-click-test-gap.md`](trailing-file-prepaint-click-test-gap.md) — The test for clicking a
  terminal file before its card paints misses the placed-but-unpainted window, so dropping that guard would go unnoticed
  (likely; premise on GPUI event timing).

- [`trailing-spec-file-click-release-wording.md`](trailing-spec-file-click-release-wording.md) — SPEC.md says a file
  click must be pressed and released over the painted card, but the press closes the card and the release is accepted
  without it; SPEC_impl.md has it right.

- [`trailing-grid-wide-glyph-absorbs-foreign-spacer.md`](trailing-grid-wide-glyph-absorbs-foreign-spacer.md) — After a
  program deletes characters from the middle of a wide character, one cell can show the wrong background color
  (pre-existing; cosmetic).

- [`trailing-tab-shell-hangup-delete-wiring-untested.md`](trailing-tab-shell-hangup-delete-wiring-untested.md) — No test
  would notice if Delete stopped giving macOS tab shells their hangup, which would again lose their shell history.

- [`trailing-agent-create-connection-binding-untested.md`](trailing-agent-create-connection-binding-untested.md) — No
  test covers binding an agent-requested session to the host connection the user approved, so the fix could regress
  silently.

- [`trailing-reposted-notice-not-revealed.md`](trailing-reposted-notice-not-revealed.md) — In an overflowing notice
  stack, a repeated connection-lost or opt-out-refused notice stays scrolled out of view instead of being revealed
  (pre-existing).

- [`trailing-drift-claude-trust-real-config-byte-check.md`](trailing-drift-claude-trust-real-config-byte-check.md) — The
  Claude trust drift case fails whenever another Claude session rewrites the real ~/.claude.json during the case,
  reading as if Farhelm wrote it (frequency unmeasured).

- [`trailing-drift-claude-trust-ancestor-vacuous-pass.md`](trailing-drift-claude-trust-ancestor-vacuous-pass.md) — The
  Claude trust drift case can pass without proving Claude honours Farhelm's entry if the maintainer's config already
  trusts /tmp or /.

- [`trailing-launcher-new-clone-enter-after-vanished-pick.md`](trailing-launcher-new-clone-enter-after-vanished-pick.md)
  — In New and Clone, Return on a highlighted template or folder row can launch instead of picking when the highlight
  vanished before the redraw; only Replace with is guarded.

- [`trailing-spec-impl-hook-answer-preview-wait.md`](trailing-spec-impl-hook-answer-preview-wait.md) — SPEC_impl.md says
  a hook-setup answer waits for a checkout preview, but the launch is refused instead and the user has to press Launch
  again.

- [`trailing-launcher-held-escape-discards-draft.md`](trailing-launcher-held-escape-discards-draft.md) — Holding Escape
  with typed results or the Save as template panel open can close the whole launcher and discard its draft; the fix
  covered only the highlight case.

- [`trailing-launcher-stale-permanent-yolo-answer-silent.md`](trailing-launcher-stale-permanent-yolo-answer-silent.md) —
  A stale "Start, and don't ask again on this host" answer closes the YOLO question with nothing started and no reason,
  sometimes after the host setting was already saved.

- [`trailing-templates-stale-save-duplicate-footer.md`](trailing-templates-stale-save-duplicate-footer.md) — Save or
  Duplicate clicked just after switching templates can act on the newly opened template, writing automatic updates into
  an older template the user never saw (pre-existing).

- [`trailing-host-setup-frozen-skip-ignores-later-restore.md`](trailing-host-setup-frozen-skip-ignores-later-restore.md)
  — Turning "Set up new hosts without asking" off while a host check is running no longer brings back the setup
  confirmation for that attempt.

- [`trailing-tensorlake-rerun-reads-skip-setting-late.md`](trailing-tensorlake-rerun-reads-skip-setting-late.md) —
  Tensorlake Re-run reads "Create Tensorlake sandboxes without asking" after the plan arrives, so turning it on during
  the check skips the billed-creation confirmation.

- [`trailing-host-refusal-remove-reads-live-skip.md`](trailing-host-refusal-remove-reads-live-skip.md) — Remove in the
  host refusal dialog reads "Remove hosts without asking" at the press, so a just-changed setting can forget the host
  without the confirmation the dialog implied.

- [`trailing-spec-impl-custom-agent-id-letter.md`](trailing-spec-impl-custom-agent-id-letter.md) — SPEC_impl.md says
  registered custom agent IDs start with a letter, but they may start with a digit.
