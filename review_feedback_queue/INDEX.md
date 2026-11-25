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

- [`replace-can-submit-before-checkout-warning.md`](replace-can-submit-before-checkout-warning.md) — Replace with can
  submit a fresh checkout before its updated warning is shown, allowing the previous checkout to move to trash without
  the displayed disclosure.

- [`tailscale-uninstall-does-not-bind-verified-host.md`](tailscale-uninstall-does-not-bind-verified-host.md) — A
  confirmed uninstall can affect a different Tailscale host if the saved name changes after Farhelm verifies the
  original host.

- [`trailing-replace-with-suspended-checkout-recent-skips-warning.md`](trailing-replace-with-suspended-checkout-recent-skips-warning.md)
  — In Replace with on a suspended managed-checkout host, pressing Down then Enter on a retained recent checkout setup
  immediately wakes, clones fresh, and deletes the source without showing the fresh-clone warning.

- [`trailing-antigravity-omitted-from-approval-cwd-pinning.md`](trailing-antigravity-omitted-from-approval-cwd-pinning.md)
  — When Antigravity workspace trust is on, agent Create, Spawn, and Clone show an unpinned symlink on the approval card
  and resolve it only at start, allowing symlink masking or redirection during the approval wait.

- [`trailing-agent-create-drops-target-incarnation-after-approval.md`](trailing-agent-create-drops-target-incarnation-after-approval.md)
  — For agent-requested launches that do not pin an approval folder, a target host reconnect or retarget during the
  final post-approval check can let the launch dispatch to the replacement connection.

- [`trailing-agent-fresh-checkout-rereads-claude-trust-after-approval.md`](trailing-agent-fresh-checkout-rereads-claude-trust-after-approval.md)
  — Toggling Claude workspace trust while an agent checkout approval card waits or an environment starts can grant trust
  to a Not trusted approval or leave permanent trust with no archive cleanup record.

- [`trailing-podman-verify-place-nonce-in-argv-forgeable.md`](trailing-podman-verify-place-nonce-in-argv-forgeable.md) —
  Because the place-identity guard passes its expected proof nonce in the SSH command line, a compromised host reached
  after an SSH alias move can echo the nonce to spoof container absence on Delete or accept a pending create.

- [`trailing-podman-setup-scripts-and-creation-proxy-skip-place-verification.md`](trailing-podman-setup-scripts-and-creation-proxy-skip-place-verification.md)
  — During a container build or setup, an external SSH alias or User change can open a fresh SSH connection and stream
  cached proxy credentials on stdin without checking the host identity, or read proxies from the moved host.

- [`trailing-agent-stop-does-not-pin-target-connection-across-approval.md`](trailing-agent-stop-does-not-pin-target-connection-across-approval.md)
  — Approving a cross-host agent Stop does not pin the target host's connection incarnation across the approval wait, so
  a retarget or reconnect during approval can stop a matching session ID on the replacement connection (requires a rare
  cross-host retarget with a colliding session ID).

- [`trailing-claude-trust-relative-config-dir-persists-relative-lock.md`](trailing-claude-trust-relative-config-dir-persists-relative-lock.md)
  — If the login shell exports a relative CLAUDE_CONFIG_DIR, Farhelm accepts it, letting a symlink in a cloned
  repository redirect pre-prompt ~/.claude.json edits and persisting a relative lock path into supervisor cleanup
  (depends on user-exported relative CLAUDE_CONFIG_DIR).

- [`trailing-custom-agent-tilde-command-kills-live-agent-before-failing.md`](trailing-custom-agent-tilde-command-kills-live-agent-before-failing.md)
  — Expanding a custom agent's ~/ command only during launch preparation kills a live agent on Restart (and allocates
  checkouts on Create) before failing if HOME is missing or the expanded path cannot be resolved (borderline
  highest/high; durable conversation ID stays preserved).

- [`trailing-launcher-enter-deleted-custom-agent-chip-submits-previous-agent.md`](trailing-launcher-enter-deleted-custom-agent-chip-submits-previous-agent.md)
  — Pressing Enter on a deleted custom-agent chip in the launcher submits the previously selected agent instead of
  refusing, which in Replace with mode can retire the running session under the wrong agent (borderline highest/high).

## High priority: material UX degradation

- [`antigravity-trust-warning-omitted-from-accessibility.md`](antigravity-trust-warning-omitted-from-accessibility.md) —
  Screen-reader users can enable automatic trust without access to the visible permission and trust-list warnings.

- [`approval-omits-automatic-trust-warning-from-accessibility.md`](approval-omits-automatic-trust-warning-from-accessibility.md)
  — Screen-reader users can miss the trusted-workspace status and warning on a launch or Restart approval.

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

- [`wayland-ssh-paste-blocks-window.md`](wayland-ssh-paste-blocks-window.md) — Pasting an SSH answer on Wayland can
  block the GUI while another application supplies clipboard data.

- [`trailing-replace-drops-late-checkout-warning.md`](trailing-replace-drops-late-checkout-warning.md) — A replacement
  session can start while the warning explaining where the original checkout was left disappears after the reply
  deadline.

- [`trailing-container-inherits-removed-proxy-setting.md`](trailing-container-inherits-removed-proxy-setting.md) — A new
  container session can still use a proxy removed from its host if the running systemd user manager retains the earlier
  setting.

- [`trailing-sandbox-confirmation-does-not-bind-billed-size-it-displays.md`](trailing-sandbox-confirmation-does-not-bind-billed-size-it-displays.md)
  — A Tensorlake checkout launch can admit a newer recipe size than the one shown when the user confirms, if the same
  named recipe is edited before admission.

- [`trailing-authorized-stop-report-completion-without-settling-recorded-launch.md`](trailing-authorized-stop-report-completion-without-settling-recorded-launch.md)
  — Stop can acknowledge completion of a retained unconfirmed launch without establishing what its recorded
  terminal-host start left running.

- [`trailing-hosts-menu-rerun-reads-live-host-state.md`](trailing-hosts-menu-rerun-reads-live-host-state.md) — Clicking
  Re-run in a host menu re-reads live host state at click time, so a state update before the click can turn Re-run
  failed setup into an unconfirmed remote host Update.

- [`trailing-wake-state-misclassifies-failed-stopped-and-stalls-60s.md`](trailing-wake-state-misclassifies-failed-stopped-and-stalls-60s.md)
  — When waking a stopped container or previously failed sandbox fails past 20 seconds or repeats the same error, the
  launcher either hangs for 60 seconds or reports that the host was suspended again.

- [`trailing-hidden-post-wake-folder-wait-wedges-launcher.md`](trailing-hidden-post-wake-folder-wait-wedges-launcher.md)
  — If a waking or quick-launch host changes connection incarnation while the hidden launcher reads its recent folders,
  the stale reply is ignored and the launcher stays permanently stuck until disconnect or reset.

- [`trailing-visible-new-wake-replaces-tilde-and-quick-launch-error.md`](trailing-visible-new-wake-replaces-tilde-and-quick-launch-error.md)
  — Launching from a visible New dialog on a suspended or stopped host showing ~ wakes the host and silently starts in
  an unshown remembered folder (which can also enable workspace trust or YOLO there), or fails with quick-launch
  wording.

- [`trailing-settings-trust-help-omits-agent-requested-launches.md`](trailing-settings-trust-help-omits-agent-requested-launches.md)
  — The Settings pages for Codex, Muse, Pi, and Antigravity omit that their workspace-trust switch also applies to
  agent-requested launches, which run without an approval card on Always-allow hosts.

- [`trailing-new-container-managed-checkout-stretches-host-block.md`](trailing-new-container-managed-checkout-stretches-host-block.md)
  — Choosing New container and Managed checkout inserts roughly 300 px of empty vertical space below the Container host
  picker because a horizontal flex basis is applied inside a vertical flex column.

- [`trailing-hosts-menu-remove-reads-live-skip-confirmation.md`](trailing-hosts-menu-remove-reads-live-skip-confirmation.md)
  — Choosing Remove from a host menu checks the live skip-confirmation setting at click time rather than binding to the
  drawn frame, so an SSH host and its per-host and per-session UI state can be forgotten without confirmation.

- [`trailing-ssh-host-setup-reads-live-skip-confirmation-at-settle.md`](trailing-ssh-host-setup-reads-live-skip-confirmation-at-settle.md)
  — Adding an SSH host or re-running failed setup checks the skip-confirmation preference after the probe finishes
  instead of at click time, so enabling that setting mid-probe starts installing without showing the plan.

- [`trailing-failed-rebuild-on-removed-host-blocks-recipe-delete.md`](trailing-failed-rebuild-on-removed-host-blocks-recipe-delete.md)
  — If a recipe rebuild or preparation fails on an empty container host and that host is later removed, deleting the
  recipe always fails because cleanup still requires the removed host row.

- [`trailing-podman-claim-and-recipe-guards-omit-tailscale.md`](trailing-podman-claim-and-recipe-guards-omit-tailscale.md)
  — Toggling a container host's Tailscale SSH mode while a recipe build is starting can pass the SQL ownership guards
  after a fresh probe reclaims the machine, carrying pre-toggle proxy settings onto the updated route.

- [`trailing-adopt-identity-leaves-podman-cache-and-claim-stale.md`](trailing-adopt-identity-leaves-podman-cache-and-claim-stale.md)
  — After adopting a reinstalled or recycled container host, Farhelm can fail new container launches for up to 5 minutes
  with a misleading moved-destination error and lock out another entry for the old machine across restarts.

- [`trailing-podman-refresh-caches-wrong-machine-on-refusal.md`](trailing-podman-refresh-caches-wrong-machine-on-refusal.md)
  — After an SSH alias temporarily points to another Podman machine and is restored, cached capabilities from the wrong
  machine can break existing container operations or inject that machine's proxy settings into a recovering container.

- [`trailing-tailscale-predictable-tmp-socket-dir-local-dos.md`](trailing-tailscale-predictable-tmp-socket-dir-local-dos.md)
  — On a multi-user machine, another local account can pre-create the predictable /tmp/fhs-<digest> directory and
  permanently block Farhelm from connecting to or setting up that Tailscale host.

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

- [`trailing-launcher-recents-display-literal-custom-instead-of-agent-name.md`](trailing-launcher-recents-display-literal-custom-instead-of-agent-name.md)
  — In both the launcher's Recent list and typed search results (wide pane and narrow popover alike), every recent setup
  for a registered custom agent shows the generic label "Custom" instead of that agent's display name or ID, so users
  cannot visually tell different custom agents apart in recent setups without hovering.

- [`trailing-refused-setup-confirmation-persists-planned-paths-on-retargeted-host.md`](trailing-refused-setup-confirmation-persists-planned-paths-on-retargeted-host.md)
  — Confirming Setup for an already-registered SSH host writes the plan's remote Farhelm binary and state-directory
  paths into the host's database row before taking the host's provisioning lock.

- [`trailing-restart-with-custom-reporting-and-offer-use-stale-row-launch.md`](trailing-restart-with-custom-reporting-and-offer-use-stale-row-launch.md)
  — When a user edits a registered custom agent's reporting or resumption setting and uses Restart with to apply it, a
  conversation ID reported right after the restarted agent starts is judged against the previous launch's reporting
  setting, and the Restart reply (and listings before launch update completes) report a stale restart offer.

- [`trailing-shared-signin-triggers-auto-update-concurrently-with-manual-action.md`](trailing-shared-signin-triggers-auto-update-concurrently-with-manual-action.md)
  — When automatic host updates are on (the default) and a remote SSH host needs SSH sign-in, choosing Uninstall or
  Update on that host shares the waiting Setup sign-in question.

- [`trailing-templates-editor-hides-custom-effort-and-refuses-agentless-modifier.md`](trailing-templates-editor-hides-custom-effort-and-refuses-agentless-modifier.md)
  — When a launch template has a custom effort value without an agent type (a valid modifier template) or after
  switching its agent type from a custom agent to a built-in agent, the Templates editor displays Effort as 'Default'
  instead of showing the stored custom value, refuses to save, and (when Effort renders as a dropdown) ignores clicking
  'Default' so the user cannot reset it directly.

- [`trailing-templates-editor-hides-custom-yolo-control-and-saves-false.md`](trailing-templates-editor-hides-custom-yolo-control-and-saves-false.md)
  — In the Templates editor, adding Approvals to a Command template that declares a custom agent (or opening a stored
  one whose Approvals was set to 'Keeps safety checks') displays only the static message 'Custom agents always run in
  YOLO mode.' with no toggle and lets the user save, yet actually saves YOLO as off ('Not YOLO').

## Other: correctness, diagnostics, cleanup, or convenience

- [`antigravity-trust-hint-omits-live-announcement.md`](antigravity-trust-hint-omits-live-announcement.md) — The new
  Antigravity trust hint emits no live announcement request for screen readers.

- [`unavailable-antigravity-trust-switch-announced-as-off.md`](unavailable-antigravity-trust-switch-announced-as-off.md)
  — Accessibility metadata reports an unread trust setting as off without marking its switch unavailable.

- [`antigravity-trust-dismissal-lost-during-recovery.md`](antigravity-trust-dismissal-lost-during-recovery.md) — A
  terminal recovery can bring back an Antigravity trust hint the user already dismissed for the same question.

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

- [`podman-launch-lifetime-notice-hidden.md`](podman-launch-lifetime-notice-hidden.md) — The launcher only explains in
  Host help that deleting the last session removes a new container and its files.

- [`exited-shell-suspend-confirmation-wording.md`](exited-shell-suspend-confirmation-wording.md) — Stop and suspend can
  still tell users it will stop a shell that has already exited.

- [`shift-enter-test-assumes-one-program-read.md`](shift-enter-test-assumes-one-program-read.md) — The Shift+Enter
  integration check assumes the program receives the key in one read, which the terminal host contract leaves to the PTY
  and program.

- [`terminal-prehello-test-misses-output-loss.md`](terminal-prehello-test-misses-output-loss.md) — The terminal
  lifecycle test checks a substring and can pass when other expected output is missing at exit.

- [`agent-settings-search-omits-agent-name.md`](agent-settings-search-omits-agent-name.md) — Settings search can give
  different agents' pages or hook controls the same accessible name.

- [`pi-settings-tests-miss-trust-route.md`](pi-settings-tests-miss-trust-route.md) — Pi's Settings tests no longer check
  a successful trust read or the absence of a hook hint control.

- [`agent-settings-tests-miss-late-trust-replies.md`](agent-settings-tests-miss-late-trust-replies.md) — The per-agent
  Settings tests can miss a trust reply being applied to the page opened after the request.

- [`agent-settings-search-test-omits-focus-target.md`](agent-settings-search-test-omits-focus-target.md) — The Settings
  search test can pass with a trust result that names the right agent and focuses the wrong control.

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

- [`launcher-loses-hook-offer-on-tab-switch.md`](launcher-loses-hook-offer-on-tab-switch.md) — Switching tabs during the
  hook settings read can suppress the setup offer while retaining the chosen agent.

- [`resume-off-command-gets-hook-warning.md`](resume-off-command-gets-hook-warning.md) — A command launch with Resume
  off can get a misleading silent-hook warning that an arriving report does not clear.

- [`uninstall-omits-persistent-hook-cleanup.md`](uninstall-omits-persistent-hook-cleanup.md) — Uninstall can leave Grok
  and Antigravity callbacks pointing to the removed Farhelm executable, and the uninstall guidance omits cleanup.

- [`hook-diagnostic-path-can-render-doc-link.md`](hook-diagnostic-path-can-render-doc-link.md) — A path inside a hook
  setup error can be shown as a Farhelm documentation link even though Farhelm supplied only diagnostic text.

- [`hook-removal-help-missing-from-accessibility.md`](hook-removal-help-missing-from-accessibility.md) — The Grok and
  Antigravity Settings pages can expose their setup switches without the persistence and removal guidance.

- [`model-picker-skips-hook-setup-offer.md`](model-picker-skips-hook-setup-offer.md) — Choosing an Antigravity model in
  the full Model picker skips the hook setup offer shown for the equivalent command-field choice.

- [`launcher-keyboard-recents-hide-custom-flags.md`](launcher-keyboard-recents-hide-custom-flags.md) — Keyboard users
  can no longer reveal the custom permission flags that distinguish otherwise identical recent setups.

- [`hook-setup-accepts-nonutf8-config-path.md`](hook-setup-accepts-nonutf8-config-path.md) — Persistent hook setup can
  write to a non-UTF-8 configuration path and report success even though the path contract calls for refusing it.

- [`checkout-input-arms-hook-warning-early.md`](checkout-input-arms-hook-warning-early.md) — Enter during managed
  checkout preparation can make Farhelm warn about a missing hook report before the agent starts.

- [`hook-hint-omits-live-accessibility-notification.md`](hook-hint-omits-live-accessibility-notification.md) — The hook
  setup hint exposes readable text but does not request a live announcement when it appears while focus stays in the
  terminal.

- [`environment-delete-title-unreachable-by-keyboard.md`](environment-delete-title-unreachable-by-keyboard.md) — A long
  target title can clip in an environment Delete confirmation where keyboard paging reaches the warning but cannot
  reveal the full title.

- [`pi-page-describes-wrong-trust-question.md`](pi-page-describes-wrong-trust-question.md) — Pi’s Settings description
  promises folder-trust behavior even though the setting approves project resources.

- [`trailing-mac-update-miss-app-installed-before-run-admission.md`](trailing-mac-update-miss-app-installed-before-run-admission.md)
  — Installing the desktop app after Update's reach check but before the run is accepted can leave a separately
  installed launchd service where Update should refuse.

- [`trailing-busy-claim-retry-test-bypasses-fleet-wake-it-depends.md`](trailing-busy-claim-retry-test-bypasses-fleet-wake-it-depends.md)
  — The new test proves the planner can try again when explicitly called, while regressions that leave a host idle
  without waking automatic Update can pass this test.

- [`trailing-reconnect-regression-test-does-not-exercise-push-waiting-for.md`](trailing-reconnect-regression-test-does-not-exercise-push-waiting-for.md)
  — Tests can keep passing if a replacement already queued for a host resumes under the wrong connection lifetime.

- [`trailing-archive-wake-regression-test-covers-delete-but-leaves-replace.md`](trailing-archive-wake-regression-test-covers-delete-but-leaves-replace.md)
  — The new test would still pass if a future edit broke automatic Trash refresh after Replace removes its source.

- [`trailing-agent-create-host-help-omits-supported-hostless-launches.md`](trailing-agent-create-host-help-omits-supported-hostless-launches.md)
  — Agent create's Host help can send callers looking for an unnecessary flag when a template launches a recipe or their
  session is already inside a Farhelm environment.

- [`trailing-timed-out-agent-delete-lose-late-checkout-warning.md`](trailing-timed-out-agent-delete-lose-late-checkout-warning.md)
  — An approved agent Delete can leave a checkout behind after its relay has timed out, with no warning in the app's
  notice history.

- [`trailing-environment-delete-drops-checkout-warning-received-after-inner-reply.md`](trailing-environment-delete-drops-checkout-warning-received-after-inner-reply.md)
  — A non-final environment Delete can complete on its host and explain why it left a checkout behind, yet the warning
  never reaches the rail if the reply arrives after the inner supervisor deadline.

- [`trailing-recipe-images-retain-proxy-configuration-used-by-their-helpers.md`](trailing-recipe-images-retain-proxy-configuration-used-by-their-helpers.md)
  — For ordinary image-backed login configuration, changing the place manager’s proxy or deleting one session does not
  remove earlier credential-bearing settings from the shared recipe images.

- [`trailing-fresh-proxy-regression-overlooks-installed-boot-file.md`](trailing-fresh-proxy-regression-overlooks-installed-boot-file.md)
  — A regression could send the right values in the first import while writing the wrong bytes for later container
  boots.

- [`trailing-podman-diagnostic-tests-bypass-connection-stage-fixed-by-patch.md`](trailing-podman-diagnostic-tests-bypass-connection-stage-fixed-by-patch.md)
  — A regression could again hide the cause of an early container connection failure while the route-specific diagnostic
  checks pass.

- [`trailing-full-podman-inspection-strand-container-farhelm-needs-to-remove.md`](trailing-full-podman-inspection-strand-container-farhelm-needs-to-remove.md)
  — A container with enough otherwise valid metadata can fail Farhelm's ownership inspection and remain undeletable from
  Farhelm.

- [`trailing-confirmed-delete-does-not-have-regression-assertion-for-capture.md`](trailing-confirmed-delete-does-not-have-regression-assertion-for-capture.md)
  — The regression assertions would miss a change that forgets a capture when the confirmed request finds its image.

- [`trailing-first-deletes-live-confirmation-reply-lacks-regression-assertion.md`](trailing-first-deletes-live-confirmation-reply-lacks-regression-assertion.md)
  — Existing assertions would miss a regression where prompt absence on a first Delete results in an ordinary error
  instead of opening Delete anyway.

- [`trailing-late-replay-history-displace-already-browsed-final-screen.md`](trailing-late-replay-history-displace-already-browsed-final-screen.md)
  — If a replay reveals the exit frame before its history is delivered, the viewer can browse that frame and then jump
  to old history without a scroll request. An already stored selection may be dropped or name different rows.

- [`trailing-one-create-test-skip-already-readable-duplicate.md`](trailing-one-create-test-skip-already-readable-duplicate.md)
  — The test can pass with a duplicate request already waiting on the scripted supervisor stream when the successful
  launch returns.

- [`trailing-recipe-fixture-omits-create-time-collision-and-provisioning-cleanup.md`](trailing-recipe-fixture-omits-create-time-collision-and-provisioning-cleanup.md)
  — The no-default fixture checks numbering after a preview conflict and a successful create, leaving the specific
  create-time lost-race cleanup unasserted.

- [`trailing-completed-setup-save-steal-focus-from-newer-input.md`](trailing-completed-setup-save-steal-focus-from-newer-input.md)
  — When a default checkout-folder save finishes after the user changes to a new environment and starts editing another
  field, the launcher can move focus back to Repository.

- [`trailing-locally-invalid-checkout-names-start-recipe-environment-before-refusal.md`](trailing-locally-invalid-checkout-names-start-recipe-environment-before-refusal.md)
  — The desktop can offer a sandbox launch for a checkout name that cannot produce a valid directory. The helm can
  create the environment and attempt cleanup before telling the user to fix the name.

- [`trailing-shell-teardown-coverage-stops-at-signal-and-delete-reply.md`](trailing-shell-teardown-coverage-stops-at-signal-and-delete-reply.md)
  — The tests can pass without showing that a command typed in Bash or Zsh is saved when Farhelm stops the session.

- [`trailing-delete-skips-hangup-for-live-tab-it-knows-only.md`](trailing-delete-skips-hangup-for-live-tab-it-knows-only.md)
  — Deleting a non-shell session can forcibly terminate a known terminal tab without giving its shell the tab's graceful
  hangup.

- [`trailing-recorded-tab-scope-test-omits-deletes-discovery-and-refusal.md`](trailing-recorded-tab-scope-test-omits-deletes-discovery-and-refusal.md)
  — The existing assertions can miss a regression where Delete skips an inaccessible scope belonging to a terminal tab.

- [`trailing-agent-re-adoption-discard-cleanup-identities-without-confirming-host.md`](trailing-agent-re-adoption-discard-cleanup-identities-without-confirming-host.md)
  — Recovery can drop the main terminal's saved host and child identities while the recorded host may still run,
  weakening later Delete cleanup.

- [`trailing-tab-close-discards-retry-state-when-final-process-sweep.md`](trailing-tab-close-discards-retry-state-when-final-process-sweep.md)
  — A terminal tab can disappear after Close reports incomplete process cleanup, leaving no tab to retry, even while its
  session remains.

- [`trailing-delete-level-tests-miss-retained-gone-host-refusal.md`](trailing-delete-level-tests-miss-retained-gone-host-refusal.md)
  — The existing assertions do not show that Delete keeps a session and its launch record if a terminal classified Gone
  still has a live adopted process.

- [`trailing-retry-delete-ambiguous-restart-record-before-scope-is-confirmed.md`](trailing-retry-delete-ambiguous-restart-record-before-scope-is-confirmed.md)
  — If a failed Restart leaves an unresolved scope, another refused Restart can erase the evidence that blocks startup
  from reopening the older terminal.

- [`trailing-tab-resume-pass-without-receiving-flood-output.md`](trailing-tab-resume-pass-without-receiving-flood-output.md)
  — The terminal-tab regression can pass while the paused tab fails to deliver the output produced during its pause.

- [`trailing-paused-tab-test-pass-without-exercising-pause.md`](trailing-paused-tab-test-pass-without-exercising-pause.md)
  — The tab-isolation test can pass without showing that the tab ever stopped receiving output.

- [`trailing-landing-removed-ambiguous-create-refusal-marker-assertion.md`](trailing-landing-removed-ambiguous-create-refusal-marker-assertion.md)
  — The regression still checks that the session and its retry key are retained, but it stopped checking whether the
  client is wrongly told to discard that key.

- [`trailing-alternate-screen-stop-diagnostics-overstate-replay-oracle.md`](trailing-alternate-screen-stop-diagnostics-overstate-replay-oracle.md)
  — If the Stop replay test fails, it can point a maintainer at a screen capture even when replay selected the wrong
  emulator buffer.

- [`trailing-tab-cleanup-test-leave-first-two-fixture-jobs-running.md`](trailing-tab-cleanup-test-leave-first-two-fixture-jobs-running.md)
  — A failure in the regression can leave the test's own background work running for the fixture's remaining sleep
  interval.

- [`trailing-alternate-screen-stop-waits-on-output-written-before-term.md`](trailing-alternate-screen-stop-waits-on-output-written-before-term.md)
  — The test can fail even when Farhelm correctly keeps the frame of a program killed before it restored the primary
  screen.

- [`trailing-failed-restart-annotation-test-accepts-errors-before-restoration-is.md`](trailing-failed-restart-annotation-test-accepts-errors-before-restoration-is.md)
  — The regression can pass without proving a failed relaunch restores the user's stopped status.

- [`trailing-real-supervisor-restart-assertions-leave-recovered-socket-identity-unchecked.md`](trailing-real-supervisor-restart-assertions-leave-recovered-socket-identity-unchecked.md)
  — The executable-path restart tests show that the original host stays alive and the reattached session works. They can
  miss a changed durable socket address because the two observations are never tied together after reattach.

- [`trailing-strengthened-stop-tree-test-permits-cleanup-after-success-reply.md`](trailing-strengthened-stop-tree-test-permits-cleanup-after-success-reply.md)
  — The test can pass even if Stop tells the user it finished while the host's child or descendant is still running.

- [`trailing-restart-gap-open-tab-checks-survivors-but-never-witnesses.md`](trailing-restart-gap-open-tab-checks-survivors-but-never-witnesses.md)
  — The test can pass if a refused Open Tab launches a shell and cleans it up before replying.

- [`trailing-restart-ordering-witness-record-gone-without-reading-old-pid.md`](trailing-restart-ordering-witness-record-gone-without-reading-old-pid.md)
  — The test can accept a failed fixture PID read as evidence that the previous agent tree had already stopped before
  the replacement host ran.

- [`trailing-list-admission-test-observes-reply-timing-instead-of-list.md`](trailing-list-admission-test-observes-reply-timing-instead-of-list.md)
  — A regression could let a busy supervisor build lists outside its concurrency limit while this test still passes.

- [`trailing-seal-delete-test-cannot-show-delete-reached-directory-admission.md`](trailing-seal-delete-test-cannot-show-delete-reached-directory-admission.md)
  — A regression could let Delete decide to destroy an environment during a concurrent launch without reliably failing
  the existing regression test.

- [`trailing-borrower-reload-lacks-completed-origin-runtime-handoff-witness.md`](trailing-borrower-reload-lacks-completed-origin-runtime-handoff-witness.md)
  — The borrower test compares status across reload, but can pass without showing the origin terminal was released and
  successfully recovered.

- [`trailing-borrower-classification-test-does-not-establish-checkout-membership.md`](trailing-borrower-classification-test-does-not-establish-checkout-membership.md)
  — This borrower classification case can pass if its second session never joins the managed checkout.

- [`trailing-running-seed-cancellation-checks-readability-without-observing-returned-screen.md`](trailing-running-seed-cancellation-checks-readability-without-observing-returned-screen.md)
  — The strengthened assertion detects a stranded or lost emulator after the seed caller cancels. It can still pass if
  an empty emulator takes the old one's place and loses the screen.

- [`trailing-held-start-fixture-coverage-of-launch-publication-remains-unverified.md`](trailing-held-start-fixture-coverage-of-launch-publication-remains-unverified.md)
  — The fixture checks publication after first-stage entry. Its coverage of the ordering relied on by startup recovery
  remains unverified.

- [`trailing-restricted-connection-refusal-of-whole-environment-delete-remains-unverified.md`](trailing-restricted-connection-refusal-of-whole-environment-delete-remains-unverified.md)
  — The product requires restricted sessions to be denied this operation. The independent inspection did not establish
  the exact denial path from the supplied source excerpts.

- [`trailing-trust-hint-save-errors-use-live-selection-instead-of.md`](trailing-trust-hint-save-errors-use-live-selection-instead-of.md)
  — A busy or refused Don't show again click can attach its error to a different session if selection moved after the
  hint was drawn.

- [`launcher-new-environment-folder-omits-repository-prompt.md`](launcher-new-environment-folder-omits-repository-prompt.md)
  — When no repository has been selected for a new environment, the Folder row says cloning will happen and omits its
  invitation to select a repository.

- [`sqlite-planner-test-misses-stale-statistics.md`](sqlite-planner-test-misses-stale-statistics.md) — The startup
  maintenance test can pass if Farhelm still creates missing SQLite statistics but stops refreshing stale ones.

- [`remote-probe-absence-test-does-not-witness-proxy.md`](remote-probe-absence-test-does-not-witness-proxy.md) — The
  remote probe regression test can pass through the missing-program path both times, without checking how it handles the
  stand-in's exit.

- [`host-lifecycle-test-misses-other-host-control.md`](host-lifecycle-test-misses-other-host-control.md) — The test
  checks that a launch leaves another host available through a helper. It can pass even if the other host’s actual menu
  is disabled.

- [`inline-resume-test-misses-disabled-control-state.md`](inline-resume-test-misses-disabled-control-state.md) — The
  inline Resume test can pass when the button looks and announces itself as enabled while its click handler refuses the
  request.

- [`environment-launch-history-test-bypasses-origin-handoff.md`](environment-launch-history-test-bypasses-origin-handoff.md)
  — The launch-history regression test can pass if an agent-requested environment starts becoming the desktop’s default
  host again.

- [`tensorlake-prepared-add-test-misses-pending-snapshot-guard.md`](tensorlake-prepared-add-test-misses-pending-snapshot-guard.md)
  — The prepared Tensorlake Add test can pass if Farhelm releases the chosen snapshot for retirement while the new
  sandbox is still starting.

- [`shortcuts-chord-during-composition.md`](shortcuts-chord-during-composition.md) — The Keyboard shortcuts chord can
  interrupt an input method if native handling clears its mark first or the action cannot query the focused field.

- [`trailing-settings-containers-reload-omits-loading-keeps-error.md`](trailing-settings-containers-reload-omits-loading-keeps-error.md)
  — After a failed refresh in Settings -> Containers, deleting the last leftover or clicking Refresh hides the reading
  indicator and leaves only the old refresh error visible while the new read waits.

- [`trailing-masked-input-test-omits-a11y-value-checks.md`](trailing-masked-input-test-omits-a11y-value-checks.md) — The
  masked-input test checks painted text and accessible names but never inspects accessible values or runs with
  accessibility active, leaving screen-reader secret exposure unguarded.

- [`trailing-podman-suspend-flashes-connecting-and-runs-retry-ladder.md`](trailing-podman-suspend-flashes-connecting-and-runs-retry-ladder.md)
  — Suspending a container briefly flashes Connecting before Suspended, and a transient post-Suspend check failure on a
  sandbox or container runs the full active reconnect ladder instead of one quiet check.

- [`trailing-tensorlake-setup-rerun-omits-suspended-and-sealed-refusal.md`](trailing-tensorlake-setup-rerun-omits-suspended-and-sealed-refusal.md)
  — Rerunning setup on a suspended or sealed Tensorlake host still offers the setup confirmation instead of refusing up
  front, and confirming on a suspended host temporarily overwrites Suspended with a failed setup run.

- [`trailing-ssh-setup-existing-tensorlake-mutates-before-lock.md`](trailing-ssh-setup-existing-tensorlake-mutates-before-lock.md)
  — Confirming an SSH-targeted setup plan on an existing Tensorlake host that became suspended can overwrite saved paths
  and replace the suspended actor before taking the host lock or checking suspended state.

- [`trailing-provisioning-service-omits-sealed-host-refusal.md`](trailing-provisioning-service-omits-sealed-host-refusal.md)
  — While a sealed sandbox or container awaits deletion retry, direct Update or SSH-targeted setup probes are still
  admitted instead of being refused.

- [`trailing-wake-drops-one-time-yolo-confirmation.md`](trailing-wake-drops-one-time-yolo-confirmation.md) — If a host
  suspends while the launcher's YOLO confirmation is open, clicking Start YOLO session anyway wakes the host but forgets
  the confirmation and re-asks YOLO once connected.

- [`trailing-restart-with-wake-refusal-hardcodes-not-launched.md`](trailing-restart-with-wake-refusal-hardcodes-not-launched.md)
  — When Restart with fails while waking a suspended or stopped host, the alert says Not launched instead of Restart
  error, and Restart with wake is untested across launcher tests.

- [`trailing-unverified-resumable-host-shows-checkout-allocation-notice.md`](trailing-unverified-resumable-host-shows-checkout-allocation-notice.md)
  — On a suspended or stopped host without a verified identity, the managed-checkout Folder line promises automatic
  folder allocation on wake even though Launch is refused as unverified.

- [`trailing-failed-post-wake-checkout-preview-omits-refusal-alert.md`](trailing-failed-post-wake-checkout-preview-omits-refusal-alert.md)
  — When a post-wake checkout preview fails or is superseded, the reopened launcher omits the bordered Not launched
  Role::Alert beside Launch (and shows no error at all when superseded).

- [`trailing-wake-folder-tests-omit-timeouts-removal-and-replace.md`](trailing-wake-folder-tests-omit-timeouts-removal-and-replace.md)
  — The folder-wake test suite never asserts that the 60-second connection or busy-retry timeouts fire, that host
  removal or retarget cancels wake, or that Replace with submits with its frozen delete guard.

- [`trailing-wake-checkout-tests-omit-replace-with.md`](trailing-wake-checkout-tests-omit-replace-with.md) —
  Managed-checkout wake tests cover only New mode and never test Replace with across wake, post-wake preview, Quick
  Switcher failure release, or retained-recent Enter.

- [`trailing-wake-checkout-tests-omit-unverified-and-stopped-hosts.md`](trailing-wake-checkout-tests-omit-unverified-and-stopped-hosts.md)
  — Every wake-checkout test uses a verified suspended sandbox, leaving unverified resumable hosts and stopped Podman
  containers uncovered.

- [`trailing-claude-trust-and-antigravity-missing-regression-tests.md`](trailing-claude-trust-and-antigravity-missing-regression-tests.md)
  — No child-process test verifies the launch shim skips .claude.json in HOME, pins_approval_cwd has no direct unit
  test, and trust-independence tests never turn Antigravity trust on.

- [`trailing-claude-trust-hint-shows-on-command-and-legacy-ordinary-dirs.md`](trailing-claude-trust-hint-shows-on-command-and-legacy-ordinary-dirs.md)
  — When Claude asks about folder trust in a Command or legacy launch inside an ordinary folder, Farhelm shows the
  Settings trust hint banner even though the Settings switch does not apply to those launches.

- [`trailing-trust-band-keeps-undismissable-error-after-dismissal.md`](trailing-trust-band-keeps-undismissable-error-after-dismissal.md)
  — After a failed or busy Don't show again click, dismissing the trust hint in Settings (or having the first click
  succeed after a rapid second click) leaves a red Not saved alert stuck with no Close button.

- [`trailing-unwind-failed-host-relaunch-drops-pending-scope-cleanup.md`](trailing-unwind-failed-host-relaunch-drops-pending-scope-cleanup.md)
  — If a session row disappears during a failing restart whose replacement host started a scoped process, failing to
  confirm scope teardown drops the pending cleanup obligation on the next supervisor start.

- [`trailing-pending-scope-cleanups-logs-invalid-session-id-unescaped.md`](trailing-pending-scope-cleanups-logs-invalid-session-id-unescaped.md)
  — When discarding a malformed session ID from startup scope cleanup, the supervisor logs it with raw Display (%id)
  instead of escaped Debug (?id), violating the session-ID logging rule.

- [`trailing-later-codex-github-failure-hides-claude-cleanup-warning.md`](trailing-later-codex-github-failure-hides-claude-cleanup-warning.md)
  — If legacy Claude (or, since #4751, Codex) settings cleanup fails on a host and a later Codex or GitHub credential
  step also fails, Farhelm hides the cleanup warning and reports the unrelated error for that agent's replacements.

- [`trailing-credential-delivery-test-stale-claude-settings-assertion.md`](trailing-credential-delivery-test-stale-claude-settings-assertion.md)
  — The backend-failure delivery test still asserts .claude/settings.json is absent (always true now), leaving failed
  agent-credentials and Codex config targets unverified before and after retry.

- [`trailing-replace-with-existing-env-checkout-promises-trash.md`](trailing-replace-with-existing-env-checkout-promises-trash.md)
  — In Replace with inside an existing sandbox or container, switching Destination to Managed checkout says the checkout
  moves to trash on Delete even though deleting in an environment discards or destroys it permanently.

- [`trailing-host-scoped-trash-confirmation-stays-open-invisibly.md`](trailing-host-scoped-trash-confirmation-stays-open-invisibly.md)
  — If a host leaves or empties while its 'Delete all on' confirmation is open in Trash, the invisible confirmation
  keeps all other hosts' Delete/Retry buttons disabled and swallows the first Close or Escape.

- [`trailing-resolve-remote-status-clears-local-transport-signal.md`](trailing-resolve-remote-status-clears-local-transport-signal.md)
  — If the local SSH or Podman client process is killed by a signal after stderr delivers the status record, Farhelm
  discards the signal termination and can treat truncated stdout as exit 0.

- [`trailing-container-host-unanswered-embeds-raw-stderr.md`](trailing-container-host-unanswered-embeds-raw-stderr.md) —
  When a verified container host command fails before the guard runs, Farhelm copies up to 64 KiB of uncropped,
  unsanitized remote stderr directly into host status and HTTP errors.

- [`trailing-podman-refresh-settles-claims-with-stale-pre-probe-row.md`](trailing-podman-refresh-settles-claims-with-stale-pre-probe-row.md)
  — If Setup or Update updates a host's remote install paths while a container host check is running, the check settles
  claims using the stale pre-probe row and can ignore an active installation identity refusal.

- [`trailing-podman-settle-claim-refuses-third-host-duplicate-transfer.md`](trailing-podman-settle-claim-refuses-third-host-duplicate-transfer.md)
  — When a stale unused container-host entry is refused as a duplicate of a third host, the verified owner of the
  container machine cannot automatically take over the unused claim.

- [`trailing-podman-build-prepare-uses-runtime-seam-bypassing-admission.md`](trailing-podman-build-prepare-uses-runtime-seam-bypassing-admission.md)
  — Later recipe build and preparation stages look up cached runtime capabilities instead of rechecking fresh admission,
  allowing new helper containers to start on hosts with an ordinary identity refusal or stale probe.

- [`trailing-claim-podman-place-retains-unused-claim-after-twin.md`](trailing-claim-podman-place-retains-unused-claim-after-twin.md)
  — When an unused SSH host moves externally to a machine claimed by another host, Refresh refuses it as a twin but
  keeps its old claim on disk, blocking a third entry for the vacated machine.

- [`trailing-podman-leftovers-skip-place-identity-verification.md`](trailing-podman-leftovers-skip-place-identity-verification.md)
  — If a claimed container host's SSH alias moves externally, deleting an unregistered leftover in Settings ->
  Containers can falsely report success while leaving it running, and refreshing can hide all leftovers.

- [`trailing-suspend-read-activity-still-uses-ordered-lane.md`](trailing-suspend-read-activity-still-uses-ordered-lane.md)
  — Primary Suspend and Stop and suspend actions still wait up to 20 seconds behind any slow host on the ordered channel
  lane because their pre-flight session activity check was left on Lane::Ordered.

- [`trailing-tailscale-setup-probe-exit-255-blames-socket-length.md`](trailing-tailscale-setup-probe-exit-255-blames-socket-length.md)
  — On machines with a long username or state-directory path, a failed Tailscale SSH probe falsely reports that
  state-directory socket length disables interactive SSH sign-in even though Tailscale puts its sockets under /tmp.

- [`trailing-ensure-hosts-skips-tailscale-validation-for-registered.md`](trailing-ensure-hosts-skips-tailscale-validation-for-registered.md)
  — An invalid Tailscale destination in --ensure-hosts is silently accepted when already registered and fails without
  the entry index or Tailscale format guidance when new.

- [`trailing-tailscale-destination-format-guidance-discarded.md`](trailing-tailscale-destination-format-guidance-discarded.md)
  — Enabling Tailscale mode on an existing SSH host with a URI, port, or underscore alias reports it as 'not a usable
  ssh destination' instead of showing the Tailscale format guidance.

- [`trailing-add-host-tailscale-checkbox-evicts-focus.md`](trailing-add-host-tailscale-checkbox-evicts-focus.md) —
  Pressing Enter on the Tailscale checkbox in Add host or receiving a background approval card during an agent host-add
  request disables the checkbox and drops keyboard focus from it.

- [`trailing-host-settings-tailscale-switch-refusal-prefix-and-stuck-error.md`](trailing-host-settings-tailscale-switch-refusal-prefix-and-stuck-error.md)
  — Toggling Tailscale in Host settings during a launch shows an unprefixed sentence fragment, stacks any earlier
  failure above it, and leaves the launch refusal stuck even after an in-flight toggle succeeds.

- [`trailing-apply-template-custom-command-yolo-diverges-from-gui.md`](trailing-apply-template-custom-command-yolo-diverges-from-gui.md)
  — Applying launch templates that declare a custom agent on the Command tab behaves differently in the desktop app and
  the CLI.

- [`trailing-automatic-signin-tests-omit-shared-prompt-with-auto-updates-on.md`](trailing-automatic-signin-tests-omit-shared-prompt-with-auto-updates-on.md)
  — The automatic host-update sign-in test suite never tests what happens when a user clicks Update or Uninstall on a
  host that already has an automatic-update sign-in waiting and answers that shared prompt with automatic updates still
  turned on (the default setting), allowing a race between the automatic update resuming and the user's manual Update or
  Uninstall to go undetected.

- [`trailing-check-template-shape-allows-custom-agent-yolo-false.md`](trailing-check-template-shape-allows-custom-agent-yolo-false.md)
  — Both the CLI and the desktop Templates editor allow saving a Command launch template that declares a custom agent
  with YOLO turned off, creating a saved template that Farhelm always refuses when you try to apply it (and in the
  desktop editor the Approvals toggle is hidden behind a static YOLO message, so the user cannot even see or fix the
  saved non-YOLO flag without removing the field).

- [`trailing-check-template-shape-skips-validate-model-on-custom-agent.md`](trailing-check-template-shape-skips-validate-model-on-custom-agent.md)
  — Saving a custom-agent launch template from the CLI or API accepts invalid model values (such as uppercase letters,
  spaces, control characters, or empty strings) even though Farhelm refuses those same values in the desktop Templates
  editor and whenever the template is applied.

- [`trailing-codex-dual-agent-delivery-tests-raw-substring.md`](trailing-codex-dual-agent-delivery-tests-raw-substring.md)
  — Several credential-delivery tests that share one endpoint token between Claude and Codex either check only whether a
  token appears anywhere in the raw combined credentials file after recovery, or check only that `codex_endpoint` is
  absent without verifying `FARHELM_CODEX_ENDPOINT_TOKEN` was also stripped.

- [`trailing-codex-renamed-executable-missing-admission-and-e2e-test.md`](trailing-codex-renamed-executable-missing-admission-and-e2e-test.md)
  — The change that enables Restart to resume Codex sessions launched from renamed or versioned executables adds no
  admission or end-to-end test using a renamed Codex executable.

- [`trailing-custom-agent-ancestor-correction-test-misses-pin-correction.md`](trailing-custom-agent-ancestor-correction-test-misses-pin-correction.md)
  — The end-to-end test that claims to verify a custom agent parent can correct a child-first conversation report would
  still pass even if custom-agent ancestor correction and its user-facing correction notification were completely
  broken.

- [`trailing-custom-agent-draft-retains-stale-builtin-effort.md`](trailing-custom-agent-draft-retains-stale-builtin-effort.md)
  — When cloning, replacing, or loading a recent setup for a custom agent whose effort matches a built-in effort word
  (such as "high" or "low"), or switching from a built-in agent to a custom agent before the model catalog loads,
  editing or clearing Effort in the launcher leaves a hidden stale effort behind.

- [`trailing-custom-agent-model-effort-error-duplicate-element-id.md`](trailing-custom-agent-model-effort-error-duplicate-element-id.md)
  — When a user enters invalid values in both the Model and Effort fields of a custom agent in the session launcher at
  the same time, both validation status lines mount under the same UI element ID, which collides in the accessibility
  tree and triggers a debug panic when assistive technology is active.

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

- [`trailing-hidden-trash-button-retains-animation-handles.md`](trailing-hidden-trash-button-retains-animation-handles.md)
  — If the Trash button stops being shown (because the user opens the Hosts panel or marks sessions) while its archive
  wiggle or emptying bob is running, Farhelm does not release that animation handle while the button is hidden.

- [`trailing-launcher-agent-tab-omits-deleted-selected-custom-agent-chip.md`](trailing-launcher-agent-tab-omits-deleted-selected-custom-agent-chip.md)
  — When the Agent tab holds a deleted custom agent (for example from Clone, Replace with, a recent setup, a template,
  or a deletion while the launcher is open), no harness chip appears selected and the deleted agent's ID is not shown
  anywhere in the form, even though the form still shows custom-agent fields and refuses launch with "The selected
  custom agent is no longer registered".

- [`trailing-launcher-choose-effort-omits-sync-fields.md`](trailing-launcher-choose-effort-omits-sync-fields.md) — When
  resetting a custom agent's effort to default via launcher search (`effort:default`), the launcher's custom effort text
  field is never resynchronized with the draft, so the visible text input can retain stale custom effort text.

- [`trailing-launcher-custom-agent-yolo-pill-omits-escalation-outline.md`](trailing-launcher-custom-agent-yolo-pill-omits-escalation-outline.md)
  — When switching from a safer agent (such as Guarded Claude or Codex) to a registered custom agent (which always
  counts as YOLO), the red permission-escalation banner appears at the top of the launcher, but the custom agent's YOLO
  permission control does not show the required thick red escalation outline.

- [`trailing-launcher-search-custom-agent-rename-refused-as-stale.md`](trailing-launcher-search-custom-agent-rename-refused-as-stale.md)
  — If a registered custom agent is renamed while it is highlighted in the launcher's search results, or between drawing
  a search result and clicking it, the launcher drops the selection and warns that the choice is no longer available
  even though that agent is still present and matches the search.

- [`trailing-launcher-search-effort-default-misses-custom-effort.md`](trailing-launcher-search-effort-default-misses-custom-effort.md)
  — When a registered custom agent is selected in the session launcher and has a typed effort, choosing "Effort:
  Default" via composer search (e.g. `effort:default`) appears to accept the choice and clears the search box, but
  leaves the custom agent's effort unchanged and still sends it at launch.

- [`trailing-launcher-search-recents-omits-custom-agent-id-and-name.md`](trailing-launcher-search-recents-omits-custom-agent-id-and-name.md)
  — Typing a registered custom agent's ID or display name in launcher search (including `recent:<name>`) does not find
  recent setups for that custom agent; recent setups only match the literal word `custom`, their folder/repository, or
  their model.

- [`trailing-launcher-target-names-command-custom-agent-outside-restart-with.md`](trailing-launcher-target-names-command-custom-agent-outside-restart-with.md)
  — When creating, cloning, or replacing a session on the Command tab with a registered custom agent declared, the main
  action's accessible name includes the custom agent's name ("Launch Command · <name> · <host> · <folder>"), whereas
  declaring any built-in agent or generic Custom omits the agent name ("Launch Command · <host> · <folder>").

- [`trailing-mac-reach-omits-date-for-bootout.md`](trailing-mac-reach-omits-date-for-bootout.md) — Remote Mac setup,
  Update, and uninstall are specified to refuse during inspection if any command their shell path requires is missing
  from the host's PATH.

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

- [`trailing-repo-cache-sweep-single-error-aborts-all-remaining-repos.md`](trailing-repo-cache-sweep-single-error-aborts-all-remaining-repos.md)
  — When a host's supervisor starts up, if cleaning up an old repository cache directory for one repository fails (for
  example, because a leftover Git process is still writing inside it), startup cleanup stops immediately and skips
  cleaning up caches and prepared checkouts for every other repository on that host until the supervisor restarts again.

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

- [`trailing-spec-md-contradicts-send-handoff-auto-opening-auto-update-popover.md`](trailing-spec-md-contradicts-send-handoff-auto-opening-auto-update-popover.md)
  — The product specification contradicts itself on when an automatic host update's SSH sign-in popover can open on its
  own: one section says it opens only when you press **Answer** (or run a manual action on that host), while the
  keyboard-focus section and the desktop app allow sending an answer to any other host's SSH question to immediately
  open and focus an already-waiting automatic Update Setup question.

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

- [`trailing-template-replay-drops-custom-agent-effort-switching-to-builtin.md`](trailing-template-replay-drops-custom-agent-effort-switching-to-builtin.md)
  — Applying a launch template (or quick-launch sequence) that switches from a custom agent with a standard effort word
  such as `high` to a built-in agent like Claude drops the effort to Default in the desktop launcher, whereas the CLI
  keeps `high`.

- [`trailing-templates-editor-allows-generic-custom-on-agent-template.md`](trailing-templates-editor-allows-generic-custom-on-agent-template.md)
  — Switching a Command template with generic 'Custom' to an Agent template shows 'Custom (not offered)' in the Agent
  type dropdown, yet the editor still allows saving it without an error.

- [`trailing-tensorlake-key-replacement-keeps-old-discovered-sandboxes.md`](trailing-tensorlake-key-replacement-keeps-old-discovered-sandboxes.md)
  — When you replace your saved Tensorlake API key with a different key in Settings without clicking Clear first, the
  list of sandboxes not on this helm keeps showing leftover sandboxes from the old key while the new check runs — and
  keeps showing them indefinitely with active Delete buttons if the new key fails to list sandboxes (for example, if the
  new key is invalid or Tensorlake is unreachable).

- [`trailing-tensorlake-orphan-cleanup-retry-falsely-succeeds-after-key-change.md`](trailing-tensorlake-orphan-cleanup-retry-falsely-succeeds-after-key-change.md)
  — If a leftover sandbox cleanup failed (for example, snapshot deletion failed) and you then switch Settings to a
  different Tensorlake account's API key, the old account's unfinished cleanup stays in Settings; clicking Retry cleanup
  appears to succeed and removes the row, even though the old account's snapshots were never deleted and may keep
  billing.

- [`trailing-tensorlake-sandboxes-inline-delete-prompt-survives-key-change.md`](trailing-tensorlake-sandboxes-inline-delete-prompt-survives-key-change.md)
  — If you open the inline Delete confirmation for a leftover sandbox in Settings → Tensorlake and then clear or replace
  your Tensorlake API key (either on that page or in Settings → Credentials), the confirmation prompt for the old key's
  sandbox stays open — keeping the Sandboxes section visible even after Clear, or letting you confirm deleting an
  old-key sandbox under the new key.

- [`trailing-ticker-live-motion-resume-test-misses-blink-resume.md`](trailing-ticker-live-motion-resume-test-misses-blink-resume.md)
  — The test checking that live Reduce Motion changes pause and then resume both the terminal cursor blink and the
  running-session pulse does not actually verify that the cursor blink resumes blinking after motion is turned back on.

- [`trailing-ui-channel-tests-omit-custom-agents-mutation-round-trip.md`](trailing-ui-channel-tests-omit-custom-agents-mutation-round-trip.md)
  — Changes to registered custom agents are wired to push over the UI channel, but the helm test suite never exercises a
  live create, edit, or delete of a custom agent end-to-end over the UI channel to verify that the served payload
  projects and decodes cleanly.

- [`trailing-unanswered-auto-update-signin-blocks-set-tailscale-hostbusy.md`](trailing-unanswered-auto-update-signin-blocks-set-tailscale-hostbusy.md)
  — While an unanswered automatic-update SSH sign-in question is waiting in the attention row (up to two minutes per
  prompt), trying to turn **Tailscale host** on or off in Host settings fails with "This host is busy; try again after
  its current operation finishes" even though no setup or update is running and you never started an operation on that
  host.

- [`trailing-unknown-agent-typo-fails-with-misleading-custom-flag-error.md`](trailing-unknown-agent-typo-fails-with-misleading-custom-flag-error.md)
  — When an agent mistypes a built-in agent name (such as `--agent cluade` or `--agent opencode`) together with
  `--prompt`, `--permissions guarded`, or a built-in model name containing `.` or `/` (such as `gpt-6.2-sol`), Farhelm
  refuses the command with a misleading Custom-agent flag error (e.g., "--prompt/--prompt-file is not offered by this
  agent") instead of telling the agent that the agent name is unknown and listing the built-in types.

- [`trailing-unseen-quick-launch-deleted-custom-agent-wakes-suspended-host.md`](trailing-unseen-quick-launch-deleted-custom-agent-wakes-suspended-host.md)
  — Triggering a quick launch (⚡ shortcut, number shortcut, sequence, or Cmd+J) whose template names a deleted custom
  agent on a suspended host (such as a suspended Tensorlake sandbox or Podman container) wakes the suspended host
  anyway, only failing with "no longer registered" / "was deleted" after the host finishes waking.

- [`trailing-yolo-confirmation-custom-agent-reads-every-custom-permission.md`](trailing-yolo-confirmation-custom-agent-reads-every-custom-permission.md)
  — When you launch or plain-replace a registered custom agent on a host that asks before YOLO launches, the
  confirmation dialog explains the YOLO classification as "Farhelm treats every Custom permission choice as YOLO."
  Because "Custom" is also the name of the Custom permission flags mode, this reads as if the launch used custom
  permission flags rather than a registered custom agent.

- [`trailing-credential-test-name-says-setup-refuses.md`](trailing-credential-test-name-says-setup-refuses.md) — A test
  name says a failed Claude credential lookup makes host setup fail, while setup actually completes with a notice.

- [`trailing-codex-login-save-reports-later-github-failure.md`](trailing-codex-login-save-reports-later-github-failure.md)
  — Saving a new Codex login can install it on the host and still report "not delivered", naming a later GitHub token
  failure as the reason.

- [`trailing-link-scanner-comparison-leeways-untested.md`](trailing-link-scanner-comparison-leeways-untested.md) — No
  test covers the comparison leeways (trailing slash, escape case, empty `?`) since the oracle's quiet side was dropped,
  so a change could make honest links warn unnoticed.
