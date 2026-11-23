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

- [`template-delete-can-target-unshown-editor.md`](template-delete-can-target-unshown-editor.md) — Delete can remove a
  different template from the one shown. The old button can also suppress Undo.

- [`stale-permanent-delete-can-remove-repaired-template.md`](stale-permanent-delete-can-remove-repaired-template.md) — A
  stale Delete permanently button can remove a repaired revision of the same template without offering Undo. The
  specification’s acceptance of that exact case is disputed.

- [`test-state-guards-accept-replaceable-parent.md`](test-state-guards-accept-replaceable-parent.md) — With some shared
  temporary directories, the ownership check can accept a test-state path another account can replace.

- [`deflake-run-consumers-skip-state-ownership-check.md`](deflake-run-consumers-skip-state-ownership-check.md) — Deflake
  Stop can trust run state from a location Start would refuse and affect a process outside that sweep.

- [`settings-close-can-save-unlisted-drafts.md`](settings-close-can-save-unlisted-drafts.md) — Save and Close can write
  an unlisted form and clear the saved alert token outside the user’s confirmation.

- [`settings-queued-save-can-commit-later-draft.md`](settings-queued-save-can-commit-later-draft.md) — A queued Settings
  save can commit later edits to an approved form, including changes that clear a saved alert token.

- [`settings-footer-can-save-the-wrong-page.md`](settings-footer-can-save-the-wrong-page.md) — A stale Settings footer
  can save the wrong page’s draft and overwrite the stored Tensorlake key.

- [`launcher-resize-changes-enter-action.md`](launcher-resize-changes-enter-action.md) — Resizing Replace with can make
  Enter delete the source session while the visible template choice still offers to fill the form.

- [`launcher-recent-enter-before-display.md`](launcher-recent-enter-before-display.md) — Moving the highlight to a
  recent setup can arm Replace before the new Enter action appears, allowing deletion under the earlier template cue.

- [`hook-save-uses-newer-replace-warning.md`](hook-save-uses-newer-replace-warning.md) — Set up and launch can stop and
  delete a source that became live while hook settings were saving, without a new answer from the user.

- [`hook-settings-toggle-uses-unshown-state.md`](hook-settings-toggle-uses-unshown-state.md) — Space can save the
  all-hosts hook opt-in while the switch still shows the on state the user intended to turn off.

- [`grok-manual-hook-replacement-omitted.md`](grok-manual-hook-replacement-omitted.md) — Enabling Grok hook setup can
  replace local edits in the file the previous Farhelm guide asked users to create. The setup prompt leaves that
  consequence unstated.

- [`antigravity-hook-removal-can-delete-extra-files.md`](antigravity-hook-removal-can-delete-extra-files.md) — Settings
  directs users to remove Antigravity’s entire plugin directory even though setup manages two files. Any other files in
  that directory would also be removed.

- [`failed-restart-clears-goose-resume-target.md`](failed-restart-clears-goose-resume-target.md) — A failed named Goose
  Restart can leave the saved resume target cleared if Farhelm also cannot restore the previous outcome. The client only
  sees the launch error.

- [`replace-can-submit-before-checkout-warning.md`](replace-can-submit-before-checkout-warning.md) — Replace with can
  submit a fresh checkout before its updated warning is shown, allowing the previous checkout to move to trash without
  the displayed disclosure.

- [`fresh-checkout-trust-can-bind-replacement-directory.md`](fresh-checkout-trust-can-bind-replacement-directory.md) —
  Farhelm can grant saved Claude trust to a replacement directory after a configured checkout step changes the path that
  was originally checked.

- [`trailing-printed-terminal-link-stale-frame.md`](trailing-printed-terminal-link-stale-frame.md) — A click on a
  printed link can open an address from a newer terminal screen than the one the user clicked.

- [`trailing-implicit-home-spare-ownership.md`](trailing-implicit-home-spare-ownership.md) — The first managed checkout
  in an existing environment can claim and remove files already stored under the reserved spare directory in its home.

- [`trailing-gone-terminal-ended-confirmation.md`](trailing-gone-terminal-ended-confirmation.md) — When a lost terminal
  is shown as ended, Stop or Delete can terminate a main program the confirmation did not say it would stop.

- [`tailscale-uninstall-does-not-bind-verified-host.md`](tailscale-uninstall-does-not-bind-verified-host.md) — A
  confirmed uninstall can affect a different Tailscale host if the saved name changes after Farhelm verifies the
  original host.

- [`trailing-replace-with-folder-enter-out-of-bounds-highlight.md`](trailing-replace-with-folder-enter-out-of-bounds-highlight.md)
  — In Replace with, pressing Enter after a template update shrinks the folder list can bypass the presentation guard
  and replace and delete the source session.

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

- [`trailing-unstamped-host-skips-legacy-claude-settings-cleanup.md`](trailing-unstamped-host-skips-legacy-claude-settings-cleanup.md)
  — If a pre-upgrade Farhelm wrote Claude endpoint settings remotely before crashing or losing the reply, upgrading
  skips cleaning ~/.claude/settings.json so a stale URL there can receive the new launch token.

- [`trailing-podman-verify-place-nonce-in-argv-forgeable.md`](trailing-podman-verify-place-nonce-in-argv-forgeable.md) —
  Because the place-identity guard passes its expected proof nonce in the SSH command line, a compromised host reached
  after an SSH alias move can echo the nonce to spoof container absence on Delete or accept a pending create.

- [`trailing-podman-setup-scripts-and-creation-proxy-skip-place-verification.md`](trailing-podman-setup-scripts-and-creation-proxy-skip-place-verification.md)
  — During a container build or setup, an external SSH alias or User change can open a fresh SSH connection and stream
  cached proxy credentials on stdin without checking the host identity, or read proxies from the moved host.

- [`trailing-set-tailscale-unparks-actor-on-sync-registry-failure.md`](trailing-set-tailscale-unparks-actor-on-sync-registry-failure.md)
  — If saving a host's Tailscale setting succeeds but refreshing connections immediately hits a database read error,
  Farhelm unparks the host and can dial under the previous SSH trust mode while a retry skips retiring those masters.

- [`trailing-setup-rerun-omits-tailscale-allowing-ssh-downgrade.md`](trailing-setup-rerun-omits-tailscale-allowing-ssh-downgrade.md)
  — Re-running failed setup on a Tailscale host omits the Tailscale flag from the probe request, so removing or toggling
  the host concurrently lets Farhelm fall back to unpinned ordinary SSH and auto-provision or re-register it.

## High priority: material UX degradation

- [`template-leave-prompts-can-stack.md`](template-leave-prompts-can-stack.md) — Two outstanding leave questions can
  cause the remaining answer to save or discard a different template draft.

- [`antigravity-trust-warning-omitted-from-accessibility.md`](antigravity-trust-warning-omitted-from-accessibility.md) —
  Screen-reader users can enable automatic trust without access to the visible permission and trust-list warnings.

- [`approval-omits-automatic-trust-warning-from-accessibility.md`](approval-omits-automatic-trust-warning-from-accessibility.md)
  — Screen-reader users can miss the trusted-workspace status and warning on a launch or Restart approval.

- [`stale-host-retry-can-continue-sandbox-delete.md`](stale-host-retry-can-continue-sandbox-delete.md) — A Retry still
  shown as reconnecting can continue a sealed sandbox deletion if the failed deletion state arrives before the menu
  updates.

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

- [`launcher-hidden-recent-remains-enter-target.md`](launcher-hidden-recent-remains-enter-target.md) — A recent setup
  can scroll out of view while Enter still uses it to submit the launcher.

- [`launcher-underlining-can-stall-search.md`](launcher-underlining-can-stall-search.md) — Underlining matches in a long
  saved wrapper can do enough work on the GUI thread to slow launcher search.

- [`hook-save-launches-into-later-folder.md`](hook-save-launches-into-later-folder.md) — Set up and launch can start a
  session in a default folder that resolves after the user answers.

- [`launch-without-hook-choice-lost.md`](launch-without-hook-choice-lost.md) — A launch accepted with Launch without can
  still install hooks after the user enables setup for future launches while provisioning continues.

- [`terminal-link-panel-abbreviates-destination.md`](terminal-link-panel-abbreviates-destination.md) — The terminal link
  panel can abbreviate the destination it will open, with no way to read the complete URL before opening it.

- [`suspend-mixes-list-and-refresh-observations.md`](suspend-mixes-list-and-refresh-observations.md) — Suspend can use
  an old idle session list alongside a newer successful refresh and skip asking before it freezes active work.

- [`hook-settings-close-hides-opt-out-failure.md`](hook-settings-close-hides-opt-out-failure.md) — Closing Settings
  while an opt-out saves can hide the refusal and leave automatic hook setup active.

- [`wayland-ssh-paste-blocks-window.md`](wayland-ssh-paste-blocks-window.md) — Pasting an SSH answer on Wayland can
  block the GUI while another application supplies clipboard data.

- [`trailing-automatic-update-fresh-dev-build.md`](trailing-automatic-update-fresh-dev-build.md) — Automatic Update can
  replace a development supervisor even after a fresh probe identifies it as a development build.

- [`trailing-credential-delivery-holds-disconnected-environment.md`](trailing-credential-delivery-holds-disconnected-environment.md)
  — When an environment disconnects during initial credential delivery, reconnect and launch can stall while Farhelm
  still treats the old connection as eligible for replacements.

- [`trailing-replace-drops-late-checkout-warning.md`](trailing-replace-drops-late-checkout-warning.md) — A replacement
  session can start while the warning explaining where the original checkout was left disappears after the reply
  deadline.

- [`trailing-container-inherits-removed-proxy-setting.md`](trailing-container-inherits-removed-proxy-setting.md) — A new
  container session can still use a proxy removed from its host if the running systemd user manager retains the earlier
  setting.

- [`trailing-sandbox-confirmation-does-not-bind-billed-size-it-displays.md`](trailing-sandbox-confirmation-does-not-bind-billed-size-it-displays.md)
  — A Tensorlake checkout launch can admit a newer recipe size than the one shown when the user confirms, if the same
  named recipe is edited before admission.

- [`trailing-lifecycle-loses-held-child-after-terminal-loss.md`](trailing-lifecycle-loses-held-child-after-terminal-loss.md)
  — Stop can report success or Restart can start another run while the prior program survives after its terminal host
  disappears.

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

- [`trailing-interrupted-uploads-invisible-on-card-or-retired-session.md`](trailing-interrupted-uploads-invisible-on-card-or-retired-session.md)
  — If an upload is cut off when the open session switches to a recovery, retired, or post-reset unavailable card,
  neither a terminal message nor a window notice appears until you leave the session (and never if you quit).

- [`trailing-duplicate-interrupted-upload-notices-silently-dropped.md`](trailing-duplicate-interrupted-upload-notices-silently-dropped.md)
  — While an interrupted-upload banner remains undismissed, a later interrupted upload of the same file name (including
  a different screenshot named pasted-1.png after returning to the session) is silently dropped.

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

- [`trailing-remove-ssh-host-blocks-on-ssh-route-lock.md`](trailing-remove-ssh-host-blocks-on-ssh-route-lock.md) —
  Removing an SSH host or finishing an uninstall awaits the SSH route lock and can hang past 20 seconds or stall for
  minutes behind an open SSH sign-in prompt or background Podman check.

## Other: correctness, diagnostics, cleanup, or convenience

- [`template-leave-spec-still-inline.md`](template-leave-spec-still-inline.md) — The implementation spec still describes
  the Templates leave question as inline, although the app centers it.

- [`launcher-model-field-can-reinterpret-show-all.md`](launcher-model-field-can-reinterpret-show-all.md) — An arriving
  model catalog can turn Enter on the highlighted Show all option into a model selection.

- [`legacy-template-can-ask-to-save-automatic-cleanup.md`](legacy-template-can-ask-to-save-automatic-cleanup.md) — A
  legacy template can ask to save when the only remaining change is automatic cleanup of a retired trust field.

- [`ensure-hosts-repair-omits-settings-loss.md`](ensure-hosts-repair-omits-settings-loss.md) — The repair message omits
  that removing and re-adding a staged host forgets its saved organization and settings.

- [`sealed-container-delete-spec-conflicts.md`](sealed-container-delete-spec-conflicts.md) — The product and
  implementation specs disagree about where a late sealed container deletion failure is reported.

- [`tensorlake-key-clear-spec-misstates-leftover-refresh.md`](tensorlake-key-clear-spec-misstates-leftover-refresh.md) —
  The implementation spec says clearing the Tensorlake API key refreshes the leftovers list. The app clears and
  invalidates that list instead.

- [`antigravity-recovery-republishes-old-trust-status.md`](antigravity-recovery-republishes-old-trust-status.md) — A
  recovered terminal can initially show Antigravity’s old trust status and date the first fresh screen as new activity.

- [`antigravity-restart-can-send-stale-trust-alert.md`](antigravity-restart-can-send-stale-trust-alert.md) — A refresh
  during Restart can send a pending trust alert after the Antigravity process that asked has stopped.

- [`antigravity-trust-falls-back-to-uncanonical-folder.md`](antigravity-trust-falls-back-to-uncanonical-folder.md) — If
  the folder can no longer be resolved or encoded during launch preparation, the Antigravity trust setting can use the
  submitted spelling.

- [`antigravity-dismissal-can-hide-next-trust-hint.md`](antigravity-dismissal-can-hide-next-trust-hint.md) — A delayed
  close from an earlier Antigravity trust hint can hide a later hint in the same session.

- [`antigravity-trust-hint-omits-live-announcement.md`](antigravity-trust-hint-omits-live-announcement.md) — The new
  Antigravity trust hint emits no live announcement request for screen readers.

- [`unavailable-antigravity-trust-switch-announced-as-off.md`](unavailable-antigravity-trust-switch-announced-as-off.md)
  — Accessibility metadata reports an unread trust setting as off without marking its switch unavailable.

- [`antigravity-trust-link-can-hide-read-failure.md`](antigravity-trust-link-can-hide-read-failure.md) — When the trust
  setting cannot be read, the hint’s Settings link can leave the unavailable switch and its error below the fold.

- [`antigravity-trust-dismissal-lost-during-recovery.md`](antigravity-trust-dismissal-lost-during-recovery.md) — A
  terminal recovery can bring back an Antigravity trust hint the user already dismissed for the same question.

- [`launcher-alias-enter-can-apply-an-undrawn-model.md`](launcher-alias-enter-can-apply-an-undrawn-model.md) — Enter can
  fill in an agent and model from a new family alias while the launcher still says models are loading.

- [`installed-agent-hook-preflight-accepts-incomplete-setup.md`](installed-agent-hook-preflight-accepts-incomplete-setup.md)
  — The installed-agent check can report agent drift when its manual conversation hook is configured for the wrong
  event.

- [`installed-agent-teardown-hides-test-failure.md`](installed-agent-teardown-hides-test-failure.md) — A teardown
  failure can hide the original launch or Restart failure in the installed-agent check.

- [`drift-suite-lacks-blocking-update-classification.md`](drift-suite-lacks-blocking-update-classification.md) — The
  drift guide calls for an unavailable verdict on a blocking CLI update, but the suite has no update-specific
  classification. The exact verdict for the documented Codex screen has not been established.

- [`installed-agent-readiness-oracle-is-too-weak.md`](installed-agent-readiness-oracle-is-too-weak.md) — Launch-only
  checks can pass on identifying text without establishing that the agent is interactive.

- [`installed-agent-turn-oracle-accepts-old-output.md`](installed-agent-turn-oracle-accepts-old-output.md) — The
  first-turn check can credit text that was already visible before its prompt was submitted.

- [`installed-agent-runner-can-leave-interrupted-build-running.md`](installed-agent-runner-can-leave-interrupted-build-running.md)
  — Stopping only the installed-agent runner during the preliminary build can leave Cargo running.

- [`installed-agent-preflight-can-misclassify-executable-access.md`](installed-agent-preflight-can-misclassify-executable-access.md)
  — Binary preflight can mark an inaccessible executable as available and misreport the resulting launch failure as
  drift.

- [`interrupted-drift-case-retains-failed-verdict.md`](interrupted-drift-case-retains-failed-verdict.md) — A cancelled
  case can leave a failed drift verdict in its retained evidence before the test binary starts.

- [`docs-shot-partial-failure-hides-success-paths.md`](docs-shot-partial-failure-hides-success-paths.md) — One failed
  page can hide the paths of sibling screenshots that were captured successfully and kept in the archive.

- [`invalid-boat-member-limit-clears-previews.md`](invalid-boat-member-limit-clears-previews.md) — An invalid Boat
  worker limit can erase selected previews and the publish record before any capture starts.

- [`docs-shots-share-host-details-state.md`](docs-shots-share-host-details-state.md) — A docs screenshot can show
  different host row details depending on which other shots run on the same worker.

- [`docs-shot-run-can-overwrite-earlier-failure-frame.md`](docs-shot-run-can-overwrite-earlier-failure-frame.md) — A
  later shot can overwrite the raw image retained for a failed capture, leaving its error linked to the wrong frame.

- [`manual-update-check-can-be-consumed-by-automatic-skip.md`](manual-update-check-can-be-consumed-by-automatic-skip.md)
  — A manual update request can disappear when it joins a background check whose policy decision already skipped
  installation.

- [`settings-shows-unreadable-updater-as-on.md`](settings-shows-unreadable-updater-as-on.md) — Settings can still show
  automatic app updates as on when the updater has paused them because preferences are unreadable.

- [`menu-shortcuts-omitted-from-native-accessibility.md`](menu-shortcuts-omitted-from-native-accessibility.md) — The new
  menu shortcut labels do not reach the native accessibility shortcut fields.

- [`feedback-external-link-cue-is-visual-only.md`](feedback-external-link-cue-is-visual-only.md) — The feedback dialog’s
  Discord link omits the browser-opening cue from its accessible description.

- [`upgrade-highlights-feed-can-outgrow-desktop-reader.md`](upgrade-highlights-feed-can-outgrow-desktop-reader.md) — The
  cumulative highlights feed can exceed the desktop reader’s limit while still passing the release promotion check.

- [`upgrade-highlights-can-open-after-sign-in.md`](upgrade-highlights-can-open-after-sign-in.md) — Upgrade highlights
  can take focus after a startup SSH question has already opened and closed.

- [`unshown-upgrade-highlights-can-be-recorded-seen.md`](unshown-upgrade-highlights-can-be-recorded-seen.md) — Escape or
  Enter can mark upgrade highlights seen before they appear on screen.

- [`upgrade-highlights-opt-out-refusal-is-hidden.md`](upgrade-highlights-opt-out-refusal-is-hidden.md) — A refused Don’t
  show again save can close the highlights dialog without telling the user.

- [`release-gate-regeneration-guidance-stale.md`](release-gate-regeneration-guidance-stale.md) — The release workflow’s
  maintainer instructions can leave the generated workflow stale for the next tag.

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

- [`golden-runner-bypasses-cargo-runtime-setup.md`](golden-runner-bypasses-cargo-runtime-setup.md) — Custom screenshot
  builds can succeed but fail to run when they depend on Cargo’s runner or shared-library setup.

- [`focused-golden-update-replaces-unpublished-full-captures.md`](focused-golden-update-replaces-unpublished-full-captures.md)
  — A valid focused golden update can overwrite a complete set of unpublished captures.

- [`golden-runner-misdecodes-cargo-json-paths.md`](golden-runner-misdecodes-cargo-json-paths.md) — A successful golden
  build can fail before rendering when Cargo reports a path the runner decodes or splits incorrectly.

- [`golden-output-backpressure-can-block-cancellation-cleanup.md`](golden-output-backpressure-can-block-cancellation-cleanup.md)
  — Blocked parallel render output can delay cancellation cleanup and retain the update lock.

- [`failed-golden-reference-setup-can-leave-partial-files.md`](failed-golden-reference-setup-can-leave-partial-files.md)
  — A failed reference fetch can strand partial files from a golden comparison that never started rendering.

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

- [`settings-radio-click-can-use-clipped-bounds.md`](settings-radio-click-can-use-clipped-bounds.md) — The new Settings
  card geometry changes where an accessibility activation is aimed when a radio row is partly offscreen. A failure on a
  supported native adapter remains unverified.

- [`container-leftover-confirmation-host-identity-uncertain.md`](container-leftover-confirmation-host-identity-uncertain.md)
  — A registered-container cleanup confirmation passes the displayed host number. Whether it can reach a different
  installation after that number’s identity changes remains unverified.

- [`launcher-escape-discards-template-edits.md`](launcher-escape-discards-template-edits.md) — Escape can discard Save
  as template edits while the launcher still shows the search results that received the key.

- [`launcher-incomplete-template-banner-misplaced.md`](launcher-incomplete-template-banner-misplaced.md) — The
  incomplete-template message appears below the form and asks the user to complete the form below it.

- [`quick-launch-placement-contract-omits-local-host.md`](quick-launch-placement-contract-omits-local-host.md) — The
  spec promises New's placement defaults, while quick launch beside a disposable session deliberately defaults to the
  helm's own host.

- [`launcher-tab-order-omits-command-message.md`](launcher-tab-order-omits-command-message.md) — The specified Tab order
  omits a command-message stop that can appear before the form.

- [`podman-launch-lifetime-notice-hidden.md`](podman-launch-lifetime-notice-hidden.md) — The launcher only explains in
  Host help that deleting the last session removes a new container and its files.

- [`launcher-held-host-name-clipped.md`](launcher-held-host-name-clipped.md) — Replace with and Restart with can clip
  the source host's name with no visual way to reveal its full spelling.

- [`launcher-shell-group-unlabelled.md`](launcher-shell-group-unlabelled.md) — The launcher separates Bash and Zsh from
  agent choices with a divider that exposes no accessible Shells label.

- [`stacked-template-shot-does-not-check-effort.md`](stacked-template-shot-does-not-check-effort.md) — The
  stacked-template capture can pass without verifying that the selected effort matches its caption.

- [`shell-release-shot-parks-on-removed-control.md`](shell-release-shot-parks-on-removed-control.md) — The shell
  release-note recapture tries to park on a launcher title that the form no longer draws.

- [`antigravity-release-shot-waits-on-folded-field.md`](antigravity-release-shot-waits-on-folded-field.md) — The
  Antigravity release-note recapture waits for a conversation field that stays folded under More options.

- [`container-comparison-targets-removed-control.md`](container-comparison-targets-removed-control.md) — The container
  comparison scene targets the removed Runs on control and cannot reach the new Host menu route.

- [`launcher-comparison-heading-selector-case.md`](launcher-comparison-heading-selector-case.md) — Launcher comparison
  scenes wait for mixed-case heading text while the launcher paints it in uppercase.

- [`launcher-host-picker-omits-expanded-state.md`](launcher-host-picker-omits-expanded-state.md) — The Host picker gives
  assistive technology no way to tell whether Enter selects a destination or tries to launch.

- [`vanished-recent-traps-launcher-enter.md`](vanished-recent-traps-launcher-enter.md) — When the highlighted recent
  setup disappears, Enter can stay bound to the missing choice and do nothing.

- [`launcher-recent-highlight-mutes-yolo-cue.md`](launcher-recent-highlight-mutes-yolo-cue.md) — Selecting a recent
  setup replaces its red YOLO styling with ordinary highlight styling, leaving only the text cue.

- [`launcher-recent-accepts-modified-return.md`](launcher-recent-accepts-modified-return.md) — The new launcher accepts
  modified Return on highlighted recent setups in addition to the displayed Return shortcut.

- [`launcher-reconnect-discards-search-selection.md`](launcher-reconnect-discards-search-selection.md) — A reconnect can
  clear the selected search result and make Enter apply a different result.

- [`launcher-highlights-escape-notation.md`](launcher-highlights-escape-notation.md) — The launcher can underline the
  escape notation displayed for a path even when that notation was absent from the matching path.

- [`launcher-host-reorder-test-accepts-old-host.md`](launcher-host-reorder-test-accepts-old-host.md) — The Host menu
  reorder test can pass without demonstrating that Enter selected the visible host.

- [`launcher-tab-test-omits-placement-premise.md`](launcher-tab-test-omits-placement-premise.md) — The launcher test for
  all placements can pass when its fixture omits an environment offer.

- [`launcher-undo-leaves-template-source-mark.md`](launcher-undo-leaves-template-source-mark.md) — Undo can restore the
  earlier launch settings and leave the reverted template marked as their source.

- [`launcher-template-rows-rebuild-draft.md`](launcher-template-rows-rebuild-draft.md) — The wide launcher rebuilds the
  same draft data for every template row while it checks held fields.

- [`quick-launch-loses-template-source-mark.md`](quick-launch-loses-template-source-mark.md) — A launcher opened for
  review or correction by quick launch can omit the source marker for the template that filled it.

- [`launcher-template-row-edits-behind-save-panel.md`](launcher-template-row-edits-behind-save-panel.md) — A template
  row can change the launcher draft while Save as template remains open with the older values.

- [`launcher-disabled-setup-enter-gives-no-reason.md`](launcher-disabled-setup-enter-gives-no-reason.md) — Enter on a
  disabled setup can do nothing without showing the refusal reason to a keyboard user.

- [`launcher-recent-fill-option-named-replace.md`](launcher-recent-fill-option-named-replace.md) — Recent setup options
  can announce Replace while activating the option only fills the form.

- [`launcher-typed-recent-mislabels-enter.md`](launcher-typed-recent-mislabels-enter.md) — Enter on a typed recent
  result only fills the form even though its key cue says Replace.

- [`launcher-recent-hover-can-follow-old-row.md`](launcher-recent-hover-can-follow-old-row.md) — A recent setup can
  briefly show hover help for the previous setup in its list position.

- [`launcher-announces-two-selected-setups.md`](launcher-announces-two-selected-setups.md) — The launcher can report
  both the setup that filled the form and the current keyboard target as selected.

- [`golden-worker-interrupt-bypasses-cancel.md`](golden-worker-interrupt-bypasses-cancel.md) — A screenshot worker
  interrupted on its own can leave the runner packaging a partial result or certifying an update after the review worker
  was interrupted.

- [`golden-reference-failure-hides-cached-images.md`](golden-reference-failure-hides-cached-images.md) — One unavailable
  screenshot reference can hide other originals that were already cached.

- [`golden-report-packages-partial-capture.md`](golden-report-packages-partial-capture.md) — A failed screenshot
  renderer can leave a partial PNG that the changes page labels as a new capture.

- [`golden-report-omits-failed-scenes.md`](golden-report-omits-failed-scenes.md) — A portable screenshot report can omit
  failed scenes without telling its reviewer that the run was incomplete.

- [`notifications-act-on-unshown-list.md`](notifications-act-on-unshown-list.md) — Closing Notifications or clearing
  resolved entries can act on notices the window has drawn but has yet to show. This also affects late Delete warnings.

- [`orphan-delete-promises-unadmitted-cleanup.md`](orphan-delete-promises-unadmitted-cleanup.md) — Sandbox Delete can
  report cleanup in progress before the helm has accepted it, so a later admission failure never appears when Settings
  refreshes.

- [`restart-cleanup-comment-misstates-record-order.md`](restart-cleanup-comment-misstates-record-order.md) — The Restart
  cleanup comment promises a host record survives a failed sweep even though removal happens first.

- [`restart-failure-hides-unconfirmed-cleanup.md`](restart-failure-hides-unconfirmed-cleanup.md) — After a failed
  replacement, Restart can restore the previous run and show only the launch error while cleanup of the replacement
  scope is unconfirmed.

- [`sidebar-retry-warning-missing-accessible-name.md`](sidebar-retry-warning-missing-accessible-name.md) — A retrying
  host's warning appears as an amber edge without an equivalent in the session row's accessible description.

- [`exited-shell-suspend-confirmation-wording.md`](exited-shell-suspend-confirmation-wording.md) — Stop and suspend can
  still tell users it will stop a shell that has already exited.

- [`terminal-steady-block-comment-stale.md`](terminal-steady-block-comment-stale.md) — The terminal sender's comment
  describes a blinking cursor even though the updated contract specifies steady block behavior.

- [`shift-enter-test-assumes-one-program-read.md`](shift-enter-test-assumes-one-program-read.md) — The Shift+Enter
  integration check assumes the program receives the key in one read, which the terminal host contract leaves to the PTY
  and program.

- [`terminal-primary-selection-hint-delayed.md`](terminal-primary-selection-hint-delayed.md) — A checkpoint can delay
  the terminal's selection notice and leave stale drag guidance visible.

- [`terminal-prehello-test-misses-output-loss.md`](terminal-prehello-test-misses-output-loss.md) — The terminal
  lifecycle test checks a substring and can pass when other expected output is missing at exit.

- [`capture-stop-retry-can-lose-starting-host.md`](capture-stop-retry-can-lose-starting-host.md) — Retrying cleanup
  after an interrupted capture start can remove the recovery directory before the host finishes starting.

- [`capture-close-setup-error-discards-frames.md`](capture-close-setup-error-discards-frames.md) — A shutdown socket
  setup failure can skip cleanup and discard already staged agent screens.

- [`capture-drive-error-discards-staged-screens.md`](capture-drive-error-discards-staged-screens.md) — A capture
  controller error can discard previously verified frames even after shutdown handling runs.

- [`capture-fixture-copy-can-break-screen-pair.md`](capture-fixture-copy-can-break-screen-pair.md) — A fixture copy
  failure can damage the previous screen and title pair and discard the staged replacement.

- [`capture-stop-retry-requires-original-binary.md`](capture-stop-retry-requires-original-binary.md) — A retained
  capture cannot use the advertised stop retry when the exact build path disappears, even if a compatible executable is
  available.

- [`worker-refusal-can-copy-old-evidence.md`](worker-refusal-can-copy-old-evidence.md) — A test worker refusal can
  return evidence from an earlier invocation under the current run's output directory.

- [`release-verifier-retry-overruns-deadline.md`](release-verifier-retry-overruns-deadline.md) — The release verifier
  can start a retry whose action allowance extends past the shared checkpoint deadline.

- [`release-verifier-does-not-bind-relaunch-samples.md`](release-verifier-does-not-bind-relaunch-samples.md) — The
  release verifier can accept continuity samples without establishing that they span the app relaunch.

- [`centos-gate-can-stage-sibling-binary.md`](centos-gate-can-stage-sibling-binary.md) — The CentOS gate can stage a
  sibling checkout's supervisor from a shared Cargo output directory.

- [`centos-gate-resolves-relative-target-from-caller.md`](centos-gate-resolves-relative-target-from-caller.md) — The
  CentOS gate can verify the wrong file when Cargo output is relative and the command starts outside the checkout.

- [`release-verifier-can-downgrade-product-failure.md`](release-verifier-can-downgrade-product-failure.md) — Missing
  retry diagnostics can downgrade a recorded product failure to Could not run.

- [`agent-settings-search-omits-agent-name.md`](agent-settings-search-omits-agent-name.md) — Settings search can give
  different agents' pages or hook controls the same accessible name.

- [`hook-hint-settings-focus-return-mismatch.md`](hook-hint-settings-focus-return-mismatch.md) — Settings opened from a
  hook hint returns focus to the hint even though the Settings contract names the gear.

- [`pi-settings-tests-miss-trust-route.md`](pi-settings-tests-miss-trust-route.md) — Pi's Settings tests no longer check
  a successful trust read or the absence of a hook hint control.

- [`agent-settings-tests-miss-late-trust-replies.md`](agent-settings-tests-miss-late-trust-replies.md) — The per-agent
  Settings tests can miss a trust reply being applied to the page opened after the request.

- [`agent-settings-search-test-omits-focus-target.md`](agent-settings-search-test-omits-focus-target.md) — The Settings
  search test can pass with a trust result that names the right agent and focuses the wrong control.

- [`hook-failure-notice-lost-on-create-recovery.md`](hook-failure-notice-lost-on-create-recovery.md) — A recovered
  launch can keep its hook setup failure state while losing the notification that explains it.

- [`hook-silent-warning-lost-on-create-recovery.md`](hook-silent-warning-lost-on-create-recovery.md) — An agent that
  survives an interrupted create can permanently lose the warning that its installed hooks have gone silent.

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

- [`launcher-model-list-loses-highlight-on-load.md`](launcher-model-list-loses-highlight-on-load.md) — Loading the model
  catalog can clear Enter's selection while the same model is still listed.

- [`container-golden-stays-on-loading.md`](container-golden-stays-on-loading.md) — The new-container golden settles on a
  loading status because its fixture never supplies the build result.

- [`launcher-loses-hook-offer-on-tab-switch.md`](launcher-loses-hook-offer-on-tab-switch.md) — Switching tabs during the
  hook settings read can suppress the setup offer while retaining the chosen agent.

- [`restart-recovery-shows-other-hook-result.md`](restart-recovery-shows-other-hook-result.md) — An interrupted Restart
  with can recover the previous terminal and display the hook setup result for a replacement that never started.

- [`failed-launch-clears-installed-hook-result.md`](failed-launch-clears-installed-hook-result.md) — A failed create can
  later say hook setup was skipped even though Farhelm wrote the persistent files.

- [`resume-off-command-gets-hook-warning.md`](resume-off-command-gets-hook-warning.md) — A command launch with Resume
  off can get a misleading silent-hook warning that an arriving report does not clear.

- [`launch-setup-recording-failure-looks-skipped.md`](launch-setup-recording-failure-looks-skipped.md) — A launch that
  verifies the hook files can later say setup was skipped if only the result write failed.

- [`held-escape-discards-launcher-draft.md`](held-escape-discards-launcher-draft.md) — On Wayland, holding Escape can
  discard the launcher draft after dismissing a highlighted result or the hook setup question.

- [`sandbox-recent-source-mark-follows-host.md`](sandbox-recent-source-mark-follows-host.md) — Changing the host can
  remove the mark from a still-listed sandbox setup while the form still names it as the source.

- [`launcher-host-search-rebuilds-registry-per-row.md`](launcher-host-search-rebuilds-registry-per-row.md) — Wide host
  search rebuilds the full host list for each matched row, increasing work as the fleet grows.

- [`terminal-paste-test-misses-first-ack-premise.md`](terminal-paste-test-misses-first-ack-premise.md) — The interrupted
  paste check can pass without demonstrating that the runtime processed the first acknowledgement.

- [`custom-command-spec-misstates-hook-arguments.md`](custom-command-spec-misstates-hook-arguments.md) — The
  custom-command contract says the launch placeholder supplies reporters for agents whose persistent configuration
  supplies them.

- [`uninstall-omits-persistent-hook-cleanup.md`](uninstall-omits-persistent-hook-cleanup.md) — Uninstall can leave Grok
  and Antigravity callbacks pointing to the removed Farhelm executable, and the uninstall guidance omits cleanup.

- [`hook-diagnostic-path-can-render-doc-link.md`](hook-diagnostic-path-can-render-doc-link.md) — A path inside a hook
  setup error can be shown as a Farhelm documentation link even though Farhelm supplied only diagnostic text.

- [`antigravity-drift-check-misses-plugin-manifest.md`](antigravity-drift-check-misses-plugin-manifest.md) — The
  Antigravity drift check can accept a callback file whose plugin manifest is missing.

- [`launcher-edit-hides-hook-save-refusal.md`](launcher-edit-hides-hook-save-refusal.md) — Editing the launcher while a
  hook setting saves can hide a later refusal.

- [`hook-hint-opens-unshown-agent-settings.md`](hook-hint-opens-unshown-agent-settings.md) — After automatic session
  selection, the focused hook hint can open the next agent's Settings page before that selection is visible.

- [`hook-removal-help-missing-from-accessibility.md`](hook-removal-help-missing-from-accessibility.md) — The Grok and
  Antigravity Settings pages can expose their setup switches without the persistence and removal guidance.

- [`model-picker-skips-hook-setup-offer.md`](model-picker-skips-hook-setup-offer.md) — Choosing an Antigravity model in
  the full Model picker skips the hook setup offer shown for the equivalent command-field choice.

- [`launcher-keyboard-recents-hide-custom-flags.md`](launcher-keyboard-recents-hide-custom-flags.md) — Keyboard users
  can no longer reveal the custom permission flags that distinguish otherwise identical recent setups.

- [`launcher-result-pick-leaves-approval-focus.md`](launcher-result-pick-leaves-approval-focus.md) — Picking a recent
  setup or template in the wide launcher can leave keyboard input focused on a separate approval card.

- [`hook-setup-accepts-nonutf8-config-path.md`](hook-setup-accepts-nonutf8-config-path.md) — Persistent hook setup can
  write to a non-UTF-8 configuration path and report success even though the path contract calls for refusing it.

- [`checkout-input-arms-hook-warning-early.md`](checkout-input-arms-hook-warning-early.md) — Enter during managed
  checkout preparation can make Farhelm warn about a missing hook report before the agent starts.

- [`lost-grok-record-shows-hook-setup-hint.md`](lost-grok-record-shows-hook-setup-hint.md) — Losing a saved Grok
  conversation can bring back setup advice even though Farhelm already received a successful hook report.

- [`hook-hint-omits-live-accessibility-notification.md`](hook-hint-omits-live-accessibility-notification.md) — The hook
  setup hint exposes readable text but does not request a live announcement when it appears while focus stays in the
  terminal.

- [`launcher-recent-highlight-omits-installation.md`](launcher-recent-highlight-omits-installation.md) — The launcher
  keys an armed recent setup by registry entry and setup, leaving cross-installation rebinding possible if matching
  history arrives without resetting that launcher.

- [`launcher-recent-highlight-can-change-path-spelling.md`](launcher-recent-highlight-can-change-path-spelling.md) — A
  refreshed recent setup can keep the same highlight while changing the literal folder spelling Enter will use.

- [`launcher-yolo-reply-uses-current-question.md`](launcher-yolo-reply-uses-current-question.md) — A permanent
  permission reply checks whichever launcher question is in flight. Whether a later question can receive the older
  launch permission remains unverified.

- [`fixture-privacy-checks-miss-link-local-addresses.md`](fixture-privacy-checks-miss-link-local-addresses.md) — The
  capture scrubber and committed fixture checks can miss link-local IPv4 endpoints. No actual disclosure was
  established.

- [`capture-test-python-minimum-undocumented.md`](capture-test-python-minimum-undocumented.md) — The standalone capture
  test gained a Python 3.11 dependency while its documented command still only names python3.

- [`environment-delete-title-unreachable-by-keyboard.md`](environment-delete-title-unreachable-by-keyboard.md) — A long
  target title can clip in an environment Delete confirmation where keyboard paging reaches the warning but cannot
  reveal the full title.

- [`pi-page-describes-wrong-trust-question.md`](pi-page-describes-wrong-trust-question.md) — Pi’s Settings description
  promises folder-trust behavior even though the setting approves project resources.

- [`hook-setting-read-can-miss-older-save.md`](hook-setting-read-can-miss-older-save.md) — Settings reopened after
  reconnect can read the old hook switch before an accepted save from the previous connection finishes.

- [`late-rename-persists-old-hook-advice.md`](late-rename-persists-old-hook-advice.md) — A late Rename reply can save
  the previous run’s hook advice in the helm cache until an authoritative refresh corrects it.

- [`hook-setup-can-report-partial-antigravity-plugin.md`](hook-setup-can-report-partial-antigravity-plugin.md) —
  Removing the Antigravity plugin directory during an accepted launch can leave it partly installed while Farhelm
  reports setup succeeded.

- [`hook-consent-summary-has-limited-keyboard-scroll.md`](hook-consent-summary-has-limited-keyboard-scroll.md) —
  Keyboard focus reveals the hook setup buttons without providing a separate way to scroll the notices. Whether this can
  hide the hook’s lasting effects in a supported window remains unverified.

- [`trailing-hosts-delete-just-registered-environment-from-approved-agent-launch.md`](trailing-hosts-delete-just-registered-environment-from-approved-agent-launch.md)
  — A user-approved agent launch can expose its new environment to the Hosts delete control before provisioning takes
  the host lock. A deletion that wins can leave a stale launch task and a misleading cleanup failure.

- [`trailing-mac-update-miss-app-installed-before-run-admission.md`](trailing-mac-update-miss-app-installed-before-run-admission.md)
  — Installing the desktop app after Update's reach check but before the run is accepted can leave a separately
  installed launchd service where Update should refuse.

- [`trailing-busy-claim-retry-test-bypasses-fleet-wake-it-depends.md`](trailing-busy-claim-retry-test-bypasses-fleet-wake-it-depends.md)
  — The new test proves the planner can try again when explicitly called, while regressions that leave a host idle
  without waking automatic Update can pass this test.

- [`trailing-setup-and-takeover-offer-binding-remains-unverified.md`](trailing-setup-and-takeover-offer-binding-remains-unverified.md)
  — It remains unclear whether a setup answer always consumes the offer shown when the user confirmed, especially if the
  draft changes while the offer is pending.

- [`trailing-linux-add-writes-frozen-install-before-rechecking-systemd-availability.md`](trailing-linux-add-writes-frozen-install-before-rechecking-systemd-availability.md)
  — An approved Linux setup can install the binary and unit, then fail on reload, when the user service manager became
  unavailable while the setup question was open.

- [`trailing-retired-lease-still-admit-local-provisioning-command-before-next.md`](trailing-retired-lease-still-admit-local-provisioning-command-before-next.md)
  — An outgoing credential replacement can locally start another provisioning operation during retirement. The remote
  credential outcome is unverified.

- [`trailing-reconnect-regression-test-does-not-exercise-push-waiting-for.md`](trailing-reconnect-regression-test-does-not-exercise-push-waiting-for.md)
  — Tests can keep passing if a replacement already queued for a host resumes under the wrong connection lifetime.

- [`trailing-retired-yolo-flag-refusal-overstates-when-farhelm-asks.md`](trailing-retired-yolo-flag-refusal-overstates-when-farhelm-asks.md)
  — The retired --confirm-yolo flag tells agents that every launch requires the user to answer even though host command
  permissions can waive ordinary cards.

- [`trailing-retained-environment-queue-checkout-removal-without-waking-trash.md`](trailing-retained-environment-queue-checkout-removal-without-waking-trash.md)
  — After deleting the only session using a checkout in a still-running Farhelm environment, an idle Trash view misses
  the automatic cleanup and can delay showing a background failure.

- [`trailing-archive-wake-regression-test-covers-delete-but-leaves-replace.md`](trailing-archive-wake-regression-test-covers-delete-but-leaves-replace.md)
  — The new test would still pass if a future edit broke automatic Trash refresh after Replace removes its source.

- [`trailing-environment-delete-implementation-spec-still-describes-desktop-port-as.md`](trailing-environment-delete-implementation-spec-still-describes-desktop-port-as.md)
  — The implementation spec says the desktop does not yet send the environment Delete request, but it is already the
  shipped GUI path.

- [`trailing-agent-create-host-help-omits-supported-hostless-launches.md`](trailing-agent-create-host-help-omits-supported-hostless-launches.md)
  — Agent create's Host help can send callers looking for an unnecessary flag when a template launches a recipe or their
  session is already inside a Farhelm environment.

- [`trailing-timed-out-agent-delete-lose-late-checkout-warning.md`](trailing-timed-out-agent-delete-lose-late-checkout-warning.md)
  — An approved agent Delete can leave a checkout behind after its relay has timed out, with no warning in the app's
  notice history.

- [`trailing-environment-delete-drops-checkout-warning-received-after-inner-reply.md`](trailing-environment-delete-drops-checkout-warning-received-after-inner-reply.md)
  — A non-final environment Delete can complete on its host and explain why it left a checkout behind, yet the warning
  never reaches the rail if the reply arrives after the inner supervisor deadline.

- [`trailing-proxy-probe-failure-handling-remains-partly-unverified.md`](trailing-proxy-probe-failure-handling-remains-partly-unverified.md)
  — The inspected paths refuse a failed proxy read. Whether every failed read stays distinct from an empty result,
  including redaction of the refusal, remains unverified.

- [`trailing-recipe-images-retain-proxy-configuration-used-by-their-helpers.md`](trailing-recipe-images-retain-proxy-configuration-used-by-their-helpers.md)
  — For ordinary image-backed login configuration, changing the place manager’s proxy or deleting one session does not
  remove earlier credential-bearing settings from the shared recipe images.

- [`trailing-fresh-proxy-regression-overlooks-installed-boot-file.md`](trailing-fresh-proxy-regression-overlooks-installed-boot-file.md)
  — A regression could send the right values in the first import while writing the wrong bytes for later container
  boots.

- [`trailing-podman-diagnostic-tests-bypass-connection-stage-fixed-by-patch.md`](trailing-podman-diagnostic-tests-bypass-connection-stage-fixed-by-patch.md)
  — A regression could again hide the cause of an early container connection failure while the route-specific diagnostic
  checks pass.

- [`trailing-unrelated-manager-settings-block-container-launch-and-cleanup.md`](trailing-unrelated-manager-settings-block-container-launch-and-cleanup.md)
  — A place or custom image can be refused even when its proxy and path settings are small and readable. A failed place
  refresh can also prevent Farhelm from removing an existing container.

- [`trailing-full-podman-inspection-strand-container-farhelm-needs-to-remove.md`](trailing-full-podman-inspection-strand-container-farhelm-needs-to-remove.md)
  — A container with enough otherwise valid metadata can fail Farhelm's ownership inspection and remain undeletable from
  Farhelm.

- [`trailing-cleanup-report-failure-when-verified-container-has-already-disappeared.md`](trailing-cleanup-report-failure-when-verified-container-has-already-disappeared.md)
  — Delete can report a failure and leave a host or helper record for retry even though the requested container is
  already absent.

- [`trailing-delete-anyway-forget-second-tag-whose-absence-was-never.md`](trailing-delete-anyway-forget-second-tag-whose-absence-was-never.md)
  — One Delete anyway warning can let Farhelm forget multiple unfinished saves, including a tag whose first absence
  check occurs during confirmed Delete.

- [`trailing-saved-absence-replaces-different-places-or-providers-cleanup-error.md`](trailing-saved-absence-replaces-different-places-or-providers-cleanup-error.md)
  — After a Delete has found a missing capture, ordinary retry shows Delete anyway while a separate cleanup blocker
  stays hidden.

- [`trailing-confirmed-delete-does-not-have-regression-assertion-for-capture.md`](trailing-confirmed-delete-does-not-have-regression-assertion-for-capture.md)
  — The regression assertions would miss a change that forgets a capture when the confirmed request finds its image.

- [`trailing-new-launch-reservation-make-confirmed-delete-refuse-after-changing.md`](trailing-new-launch-reservation-make-confirmed-delete-refuse-after-changing.md)
  — When an approved new container launch overlaps Delete anyway for the same recipe, Delete can report that a host uses
  the recipe after already removing its prepared images.

- [`trailing-delete-anyway-omits-host-needed-to-locate-late-image.md`](trailing-delete-anyway-omits-host-needed-to-locate-late-image.md)
  — With several container hosts, even a warning for exactly one capture gives the user no location for the cleanup it
  says might be needed.

- [`trailing-first-deletes-live-confirmation-reply-lacks-regression-assertion.md`](trailing-first-deletes-live-confirmation-reply-lacks-regression-assertion.md)
  — Existing assertions would miss a regression where prompt absence on a first Delete results in an ordinary error
  instead of opening Delete anyway.

- [`trailing-post-exit-alt-release-clears-short-selection-and-queues.md`](trailing-post-exit-alt-release-clears-short-selection-and-queues.md)
  — A quick Alt/Option release over the final screen can clear the highlight for a just-copied character and dispatch
  cursor keys.

- [`trailing-losing-terminal-host-hide-history-already-exposed-after-exit.md`](trailing-losing-terminal-host-hide-history-already-exposed-after-exit.md)
  — During recovery from loss of an already exited terminal host, the retained screen can lose its exit line and stop
  showing primary history even though recovery has kept the same grid.

- [`trailing-final-frame-boundary-test-leaves-osc-8-and-file.md`](trailing-final-frame-boundary-test-leaves-osc-8-and-file.md)
  — Existing exit tests would miss a regression that joins a file path or combines OSC 8 labels across the
  retained-history/final-frame boundary.

- [`trailing-osc-52-copy-rule-omits-primary-selection-distinction.md`](trailing-osc-52-copy-rule-omits-primary-selection-distinction.md)
  — The wording can lead readers to expect a primary-selection write to replace the system clipboard.

- [`trailing-multi-clicking-exit-message-copies-different-row.md`](trailing-multi-clicking-exit-message-copies-different-row.md)
  — A double or triple click on the exit message can replace the clipboard with terminal text from the row above it.

- [`trailing-page-keys-skip-rows-in-lifted-terminal.md`](trailing-page-keys-skip-rows-in-lifted-terminal.md) — Keyboard
  paging loses its shared edge row when the terminal is lifted by one row and skips retained output between pages when
  two or more rows are lifted.

- [`trailing-recovery-warning-is-cut-off-in-supported-narrow-terminal.md`](trailing-recovery-warning-is-cut-off-in-supported-narrow-terminal.md)
  — Scrolling to the recovery warning after a full-screen program exits can reveal only the beginning of its message.

- [`trailing-lifted-viewport-makes-scrollbar-overstate-visible-share.md`](trailing-lifted-viewport-makes-scrollbar-overstate-visible-share.md)
  — The scrollback thumb can appear larger than the portion of output actually visible when a status or exit line lifts
  grid rows.

- [`trailing-late-replay-history-displace-already-browsed-final-screen.md`](trailing-late-replay-history-displace-already-browsed-final-screen.md)
  — If a replay reveals the exit frame before its history is delivered, the viewer can browse that frame and then jump
  to old history without a scroll request. An already stored selection may be dropped or name different rows.

- [`trailing-registered-all-numeric-ssh-names-bypass-first-label-shortening.md`](trailing-registered-all-numeric-ssh-names-bypass-first-label-shortening.md)
  — A multi-label numeric SSH name with too many components to be IPv4 can be saved in a host's Destination field. The
  collapsed rail then uses the whole spelling in its short caption and can truncate it. Hover help still has the full
  name.

- [`trailing-all-phase-fixture-does-not-compare-managers-phase-strings.md`](trailing-all-phase-fixture-does-not-compare-managers-phase-strings.md)
  — The strengthened fixture catches client/wire phase drift. A manager-only rename for a phase can still pass this
  fixture.

- [`trailing-status-mark-tests-lost-their-per-target-geometry-check.md`](trailing-status-mark-tests-lost-their-per-target-geometry-check.md)
  — The row status tests verify that each mark's help appears with a notch and no ring. They can pass if placement uses
  the bounds of a different element in the same sidebar.

- [`trailing-shift-click-mixes-live-range-anchor-with-displayed-row.md`](trailing-shift-click-mixes-live-range-anchor-with-displayed-row.md)
  — A Shift-click on a surviving row can mark a different range when the open session changes before the list redraws.
  The click can add or omit local marks relative to the range displayed.

- [`trailing-muted-waiting-dots-bypass-ringless-status-tooltip-path.md`](trailing-muted-waiting-dots-bypass-ringless-status-tooltip-path.md)
  — On a muted session waiting on a connected host, hovering the waiting dot shows the row summary. If the summary fits
  beside the sidebar, its notch and ring identify the row's open control. The separate Unmute bell still has its own
  action help.

- [`trailing-maximized-relaunch-contract-has-no-first-frame-or-subsequent.md`](trailing-maximized-relaunch-contract-has-no-first-frame-or-subsequent.md)
  — The checked cases could pass if a maximized Mac relaunch still flashed the ordinary frame or if the next Zoom
  returned to a different rectangle.

- [`trailing-one-create-test-skip-already-readable-duplicate.md`](trailing-one-create-test-skip-already-readable-duplicate.md)
  — The test can pass with a duplicate request already waiting on the scripted supervisor stream when the successful
  launch returns.

- [`trailing-recipe-fixture-omits-create-time-collision-and-provisioning-cleanup.md`](trailing-recipe-fixture-omits-create-time-collision-and-provisioning-cleanup.md)
  — The no-default fixture checks numbering after a preview conflict and a successful create, leaving the specific
  create-time lost-race cleanup unasserted.

- [`trailing-completed-setup-save-steal-focus-from-newer-input.md`](trailing-completed-setup-save-steal-focus-from-newer-input.md)
  — When a default checkout-folder save finishes after the user changes to a new environment and starts editing another
  field, the launcher can move focus back to Repository.

- [`trailing-replace-with-button-does-not-carry-own-rendered-delete.md`](trailing-replace-with-button-does-not-carry-own-rendered-delete.md)
  — The main Replace with button reads the most recently presented source warning when its callback runs, so the button
  itself carries no guard from the frame that made it available.

- [`trailing-locally-invalid-checkout-names-start-recipe-environment-before-refusal.md`](trailing-locally-invalid-checkout-names-start-recipe-environment-before-refusal.md)
  — The desktop can offer a sandbox launch for a checkout name that cannot produce a valid directory. The helm can
  create the environment and attempt cleanup before telling the user to fix the name.

- [`trailing-indented-blank-lines-defeat-release-note-pr-coverage-checks.md`](trailing-indented-blank-lines-defeat-release-note-pr-coverage-checks.md)
  — A separator containing only spaces can make the gate overlook a highlighted PR missing from All changes, or reject a
  list that already cites it.

- [`trailing-empty-highlights-release-path-lacks-consumer-test.md`](trailing-empty-highlights-release-path-lacks-consumer-test.md)
  — The release checker accepts a version with every entry in All changes. The generator, desktop reader and dialog lack
  an assertion that this release remains usable.

- [`trailing-delete-scope-comment-describes-old-stop-behavior.md`](trailing-delete-scope-comment-describes-old-stop-behavior.md)
  — The cleanup source misstates what happens to terminal tabs when a session is stopped.

- [`trailing-exited-session-skips-portable-sweep.md`](trailing-exited-session-skips-portable-sweep.md) — After a program
  exits and its scope disappears, Stop, Restart and Delete can skip the fallback search for its remaining jobs.

- [`trailing-shell-teardown-coverage-stops-at-signal-and-delete-reply.md`](trailing-shell-teardown-coverage-stops-at-signal-and-delete-reply.md)
  — The tests can pass without showing that a command typed in Bash or Zsh is saved when Farhelm stops the session.

- [`trailing-scoped-shell-teardown-suppresses-failed-history-saving-hangup.md`](trailing-scoped-shell-teardown-suppresses-failed-history-saving-hangup.md)
  — If the scope's hangup request fails to start, a first-class shell can be forcibly stopped with no chance to save
  history through that signal and the operation can still report success.

- [`trailing-portable-teardown-first-find-pinned-shell-when-it-is.md`](trailing-portable-teardown-first-find-pinned-shell-when-it-is.md)
  — One partial process-table read can cause a running unscoped shell to be stopped forcibly without a chance to handle
  the history-saving hangup.

- [`trailing-delete-skips-hangup-for-live-tab-it-knows-only.md`](trailing-delete-skips-hangup-for-live-tab-it-knows-only.md)
  — Deleting a non-shell session can forcibly terminate a known terminal tab without giving its shell the tab's graceful
  hangup.

- [`trailing-recorded-tab-scope-test-omits-deletes-discovery-and-refusal.md`](trailing-recorded-tab-scope-test-omits-deletes-discovery-and-refusal.md)
  — The existing assertions can miss a regression where Delete skips an inaccessible scope belonging to a terminal tab.

- [`trailing-no-existing-assertion-covers-terminal-loss-between-publication-and.md`](trailing-no-existing-assertion-covers-terminal-loss-between-publication-and.md)
  — An assertion can pass after this patch's late recovery-status sample is removed, despite a Restart acknowledgement
  again being able to hide the recovery card.

- [`trailing-restart-with-samples-terminal-availability-before-final-database-work.md`](trailing-restart-with-samples-terminal-availability-before-final-database-work.md)
  — Restart with can return and cache a terminal-present acknowledgement after the replacement terminal is gone.

- [`trailing-agent-re-adoption-discard-cleanup-identities-without-confirming-host.md`](trailing-agent-re-adoption-discard-cleanup-identities-without-confirming-host.md)
  — Recovery can drop the main terminal's saved host and child identities while the recorded host may still run,
  weakening later Delete cleanup.

- [`trailing-tab-close-discards-retry-state-when-final-process-sweep.md`](trailing-tab-close-discards-retry-state-when-final-process-sweep.md)
  — A terminal tab can disappear after Close reports incomplete process cleanup, leaving no tab to retry, even while its
  session remains.

- [`trailing-delete-level-tests-miss-retained-gone-host-refusal.md`](trailing-delete-level-tests-miss-retained-gone-host-refusal.md)
  — The existing assertions do not show that Delete keeps a session and its launch record if a terminal classified Gone
  still has a live adopted process.

- [`trailing-create-and-create-replay-sample-terminal-availability-before-projecting.md`](trailing-create-and-create-replay-sample-terminal-availability-before-projecting.md)
  — A create or a keyed replay can answer that its session has a terminal after that host has disappeared.

- [`trailing-retry-delete-ambiguous-restart-record-before-scope-is-confirmed.md`](trailing-retry-delete-ambiguous-restart-record-before-scope-is-confirmed.md)
  — If a failed Restart leaves an unresolved scope, another refused Restart can erase the evidence that blocks startup
  from reopening the older terminal.

- [`trailing-re-adoption-publish-missing-terminal-without-invalidating-session-list.md`](trailing-re-adoption-publish-missing-terminal-without-invalidating-session-list.md)
  — If re-adoption finds that an unreadable main terminal has gone, the recovery card can wait for a later refresh.

- [`trailing-attach-admits-retained-gone-main-terminal-listings-report-missing.md`](trailing-attach-admits-retained-gone-main-terminal-listings-report-missing.md)
  — A request to attach can be admitted after the supervisor already knows the main terminal is gone.

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

- [`trailing-stop-classification-test-pass-without-stop-waiting-for-exit.md`](trailing-stop-classification-test-pass-without-stop-waiting-for-exit.md)
  — A regression that stops waiting for a dead agent's final error output could escape the Stop test.

- [`trailing-seal-delete-test-cannot-show-delete-reached-directory-admission.md`](trailing-seal-delete-test-cannot-show-delete-reached-directory-admission.md)
  — A regression could let Delete decide to destroy an environment during a concurrent launch without reliably failing
  the existing regression test.

- [`trailing-borrower-reload-lacks-completed-origin-runtime-handoff-witness.md`](trailing-borrower-reload-lacks-completed-origin-runtime-handoff-witness.md)
  — The borrower test compares status across reload, but can pass without showing the origin terminal was released and
  successfully recovered.

- [`trailing-delete-verifies-stale-records-host-identity-after-requesting-takeover.md`](trailing-delete-verifies-stale-records-host-identity-after-requesting-takeover.md)
  — If a retained launch record names a socket serving another session whose host already has a client, Delete can close
  that owning connection before refusing.

- [`trailing-live-route-conformance-pass-when-terminal-control-skips-target.md`](trailing-live-route-conformance-pass-when-terminal-control-skips-target.md)
  — Keeping the target terminal alive improves setup, but the reader-progress test can still pass without proving that
  the target control ran under contention.

- [`trailing-omp-owner-exit-test-overstates-evidence-behind-resume-offer.md`](trailing-omp-owner-exit-test-overstates-evidence-behind-resume-offer.md)
  — The strengthened test establishes that a saved Resume offer survives process exit and supervisor reload. Its
  docstring additionally claims the offer stands on a verified file, which the test does not check.

- [`trailing-borrower-classification-test-does-not-establish-checkout-membership.md`](trailing-borrower-classification-test-does-not-establish-checkout-membership.md)
  — This borrower classification case can pass if its second session never joins the managed checkout.

- [`trailing-ready-and-borrower-cases-do-not-assert-vanished-terminal.md`](trailing-ready-and-borrower-cases-do-not-assert-vanished-terminal.md)
  — The test documents ordinary exit for a stopped Ready or borrower session, but can pass if the observer proposes
  nothing and reload keeps the old Running outcome.

- [`trailing-running-seed-cancellation-checks-readability-without-observing-returned-screen.md`](trailing-running-seed-cancellation-checks-readability-without-observing-returned-screen.md)
  — The strengthened assertion detects a stranded or lost emulator after the seed caller cancels. It can still pass if
  an empty emulator takes the old one's place and loses the screen.

- [`trailing-product-spec-still-promises-no-decision-after-unanswered-environment.md`](trailing-product-spec-still-promises-no-decision-after-unanswered-environment.md)
  — The product spec groups a request that was never sent with one that went unanswered, even though an unanswered
  Delete may proceed.

- [`trailing-interrupted-card-guide-still-names-only-host-restart.md`](trailing-interrupted-card-guide-still-names-only-host-restart.md)
  — The implementation guide describes recovery-card copy that covers one of the two interruption causes.

- [`trailing-work-start-spec-still-says-rename-replaces-sampling-state.md`](trailing-work-start-spec-still-says-rename-replaces-sampling-state.md)
  — The implementation spec gives conflicting directions about whether a title-only change should preserve the current
  screen reading.

- [`trailing-interrupted-recovery-guide-still-assumes-no-prior-process-remains.md`](trailing-interrupted-recovery-guide-still-assumes-no-prior-process-remains.md)
  — The implementation guide justifies Restart without a confirmation by asserting the prior process is gone, although
  legacy retirement may leave it running.

- [`trailing-corrected-clone-doorway-test-still-omits-valid-absent-host.md`](trailing-corrected-clone-doorway-test-still-omits-valid-absent-host.md)
  — The test whose contract #4529 repairs would still pass if the supervisor reinstated a blanket host requirement for
  Clone.

- [`trailing-clone-cli-reference-still-omits-environment-exception.md`](trailing-clone-cli-reference-still-omits-environment-exception.md)
  — The CLI reference tells agents to supply a host for a Clone that the environment contract requires to omit it.

- [`trailing-held-start-fixture-coverage-of-launch-publication-remains-unverified.md`](trailing-held-start-fixture-coverage-of-launch-publication-remains-unverified.md)
  — The fixture checks publication after first-stage entry. Its coverage of the ordering relied on by startup recovery
  remains unverified.

- [`trailing-migration-comments-reboot-classification-remains-disputed.md`](trailing-migration-comments-reboot-classification-remains-disputed.md)
  — The migration wording may imply that a Launching row avoids the interruption assigned to Running rows. The ensuing
  reboot classification gap remains unverified.

- [`trailing-restart-republish-tab-exited-during-relaunch.md`](trailing-restart-republish-tab-exited-during-relaunch.md)
  — A tab that exits during a slow Restart can still appear in the Restart result and be published by the helm before a
  fresh listing corrects it.

- [`trailing-restart-disposition-documentation-still-promises-no-external-change.md`](trailing-restart-disposition-documentation-still-promises-no-external-change.md)
  — The implementation has contradictory descriptions of whether a failed Restart may have run a replacement.

- [`trailing-work-start-guide-omits-deferral-while-lifecycle-claim-is.md`](trailing-work-start-guide-omits-deferral-while-lifecycle-claim-is.md)
  — A newly sampled question or work burst can update the badge before the recency key changes, and an observation
  deferred during relaunch can lose its chance to update the key.

- [`trailing-recent-activity-product-spec-still-promises-retired-capture-recovery.md`](trailing-recent-activity-product-spec-still-promises-retired-capture-recovery.md)
  — The recent-activity contract tells readers that a recovered screen always starts a fresh ordering baseline after
  capture failure.

- [`trailing-sampler-date-retained-final-screen-after-agent-exited.md`](trailing-sampler-date-retained-final-screen-after-agent-exited.md)
  — If an agent exits during another session's sample processing, its retained work screen can refresh its age and
  unseen marker after the exit.

- [`trailing-interrupted-be-superseded-by-inferred-preparation-failure.md`](trailing-interrupted-be-superseded-by-inferred-preparation-failure.md)
  — The protocol docs promise a definitive launch error before Interrupted changes, while the supervisor can reclassify
  from missing or unreadable preparation evidence.

- [`trailing-legacy-interrupted-settles-launch-reads-without-proving-writer-stopped.md`](trailing-legacy-interrupted-settles-launch-reads-without-proving-writer-stopped.md)
  — After same-boot legacy retirement, the ticker can stop checking for a later launch failure without proving the
  writer ended.

- [`trailing-restricted-connection-refusal-of-whole-environment-delete-remains-unverified.md`](trailing-restricted-connection-refusal-of-whole-environment-delete-remains-unverified.md)
  — The product requires restricted sessions to be denied this operation. The independent inspection did not establish
  the exact denial path from the supplied source excerpts.

- [`trailing-trust-hint-save-errors-use-live-selection-instead-of.md`](trailing-trust-hint-save-errors-use-live-selection-instead-of.md)
  — A busy or refused Don't show again click can attach its error to a different session if selection moved after the
  hint was drawn.

- [`deflake-sweep-fixtures-inherit-git-hooks.md`](deflake-sweep-fixtures-inherit-git-hooks.md) — The deflake sweep-base
  tests can fail before evaluating the base when a developer's configured Git hook refuses a fixture commit.

- [`deflake-base-refusal-test-misses-run-state.md`](deflake-base-refusal-test-misses-run-state.md) — The sweep-base
  tests can pass even if an unsupported revision creates run state before deflake refuses it.

- [`launcher-preview-path-no-longer-copyable.md`](launcher-preview-path-no-longer-copyable.md) — The compact
  managed-checkout row shows the exact planned folder without the selection and copy control available before the layout
  change.

- [`launcher-new-environment-folder-omits-repository-prompt.md`](launcher-new-environment-folder-omits-repository-prompt.md)
  — When no repository has been selected for a new environment, the Folder row says cloning will happen and omits its
  invitation to select a repository.

- [`sqlite-planner-test-misses-stale-statistics.md`](sqlite-planner-test-misses-stale-statistics.md) — The startup
  maintenance test can pass if Farhelm still creates missing SQLite statistics but stops refreshing stale ones.

- [`terminal-selection-paints-wide-wrap-padding.md`](terminal-selection-paints-wide-wrap-padding.md) — Selecting text
  across a wrapped wide character can highlight the unused cell at the end of the earlier row. That cell is absent from
  the copied text.

- [`terminal-link-oracle-regeneration-loses-wide-padding.md`](terminal-link-oracle-regeneration-loses-wide-padding.md) —
  Regenerating the terminal link fixtures can drop the row flag that distinguishes printed spaces from wide-character
  wrap padding.

- [`remote-probe-absence-test-does-not-witness-proxy.md`](remote-probe-absence-test-does-not-witness-proxy.md) — The
  remote probe regression test can pass through the missing-program path both times, without checking how it handles the
  stand-in's exit.

- [`worker-staging-failure-leaves-in-progress-evidence.md`](worker-staging-failure-leaves-in-progress-evidence.md) — A
  failed worker upload can leave its local evidence directory marked as staging after the command has returned an error.

- [`launcher-first-podman-check-hides-known-host-refusal.md`](launcher-first-podman-check-hides-known-host-refusal.md) —
  While Farhelm runs the first Podman check, the launcher can show a generic check-needed message even when the helm
  already knows why the host is refused.

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

- [`trailing-drift-turn2-hook-check-passes-on-noop-stop-ack.md`](trailing-drift-turn2-hook-check-passes-on-noop-stop-ack.md)
  — The opt-in Codex and Antigravity drift tests can pass after Restart even when Turn-2 hook verification did not
  commit, because an ignored Stop callback still logs an ack carrying Turn 1's conversation ID.

- [`trailing-test-worker-stop-reports-ended-before-lock-release.md`](trailing-test-worker-stop-reports-ended-before-lock-release.md)
  — Stopping a test-worker run can report ended while the remote checkout flock is still held during stage cleanup,
  causing an immediate follow-up run to fail as busy with an unknown holder.

- [`trailing-test-worker-stop-during-prep-mutates-and-prunes.md`](trailing-test-worker-stop-during-prep-mutates-and-prunes.md)
  — A stop requested after worker admission during preparation still overwrites the remote checkout, evicts an older
  retained run, and publishes an empty finished directory.

- [`trailing-test-worker-relative-checkout-breaks-lock-isolation.md`](trailing-test-worker-relative-checkout-breaks-lock-isolation.md)
  — Passing a relative --checkout . or .. places remote state in the wrong parent directory, bypasses worker-wide
  bootstrap and active locks, and forces a full re-bootstrap every run.

- [`trailing-test-worker-collect-discards-recorded-exit-status.md`](trailing-test-worker-collect-discards-recorded-exit-status.md)
  — Collecting a test-worker run later discards the worker's authoritative exit status, so invocation.txt cannot
  distinguish a pass, failure, or forced kill (137).

- [`trailing-test-worker-status-hides-bootstrap-lock.md`](trailing-test-worker-status-hides-bootstrap-lock.md) — When a
  test-worker run times out waiting for worker bootstrap and tells the caller to check status, status never reports an
  active standalone or cross-checkout bootstrap.

- [`trailing-shortcuts-chord-opens-during-terminal-ime.md`](trailing-shortcuts-chord-opens-during-terminal-ime.md) —
  Pressing the Keyboard shortcuts chord while an input method is composing in a focused terminal still opens the
  overview dialog instead of ignoring composition.

- [`trailing-settings-containers-reload-omits-loading-keeps-error.md`](trailing-settings-containers-reload-omits-loading-keeps-error.md)
  — After a failed refresh in Settings -> Containers, deleting the last leftover or clicking Refresh hides the reading
  indicator and leaves only the old refresh error visible while the new read waits.

- [`trailing-template-summary-misstates-unsupported-permissions.md`](trailing-template-summary-misstates-unsupported-permissions.md)
  — When a template stores an approval mode its chosen non-shell agent does not offer, approval cards and template list
  summaries display that agent's default mode instead of the stored mode.

- [`trailing-masked-input-test-omits-a11y-value-checks.md`](trailing-masked-input-test-omits-a11y-value-checks.md) — The
  masked-input test checks painted text and accessible names but never inspects accessible values or runs with
  accessibility active, leaving screen-reader secret exposure unguarded.

- [`trailing-podman-suspend-flashes-connecting-and-runs-retry-ladder.md`](trailing-podman-suspend-flashes-connecting-and-runs-retry-ladder.md)
  — Suspending a container briefly flashes Connecting before Suspended, and a transient post-Suspend check failure on a
  sandbox or container runs the full active reconnect ladder instead of one quiet check.

- [`trailing-tensorlake-setup-rerun-omits-suspended-and-sealed-refusal.md`](trailing-tensorlake-setup-rerun-omits-suspended-and-sealed-refusal.md)
  — Rerunning setup on a suspended or sealed Tensorlake host still offers the setup confirmation instead of refusing up
  front, and confirming on a suspended host temporarily overwrites Suspended with a failed setup run.

- [`trailing-tensorlake-rust-tests-summary-head-60-evicts-count.md`](trailing-tensorlake-rust-tests-summary-head-60-evicts-count.md)
  — When a passing Tensorlake Rust shard emits 60 or more SKIPPED lines, head -60 evicts nextest's Summary line, causing
  the runner to fail the passing shard and leave its sandbox retained.

- [`trailing-ssh-setup-existing-tensorlake-mutates-before-lock.md`](trailing-ssh-setup-existing-tensorlake-mutates-before-lock.md)
  — Confirming an SSH-targeted setup plan on an existing Tensorlake host that became suspended can overwrite saved paths
  and replace the suspended actor before taking the host lock or checking suspended state.

- [`trailing-tensorlake-sandbox-tests-allocates-before-ledger.md`](trailing-tensorlake-sandbox-tests-allocates-before-ledger.md)
  — An interruption or ledger write failure right after sandbox creation in the Tensorlake test harness can leave an
  untracked worker sandbox running outside cleanup.

- [`trailing-provisioning-service-omits-sealed-host-refusal.md`](trailing-provisioning-service-omits-sealed-host-refusal.md)
  — While a sealed sandbox or container awaits deletion retry, direct Update or SSH-targeted setup probes are still
  admitted instead of being refused.

- [`trailing-plain-replace-and-restart-do-not-disable-suspend.md`](trailing-plain-replace-and-restart-do-not-disable-suspend.md)
  — Unlike Replace with and Restart with, an in-flight plain Replace or Restart leaves Suspend enabled on that sandbox
  or container host so suspending can interrupt the request.

- [`trailing-suspend-silently-dropped-when-launch-starts-mid-check.md`](trailing-suspend-silently-dropped-when-launch-starts-mid-check.md)
  — Clicking Suspend or Stop and suspend quietly does nothing without an error note if a launch on that host begins
  while the pre-flight session activity read is in flight.

- [`trailing-post-wake-disconnect-or-start-fail-reveals-silently.md`](trailing-post-wake-disconnect-or-start-fail-reveals-silently.md)
  — If a waking host disconnects right as a post-wake checkout preview settles or launch start fails, the launcher can
  reopen without any error alert explaining why nothing launched.

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

- [`trailing-pending-restart-scope-cleanups-sequential-starvation.md`](trailing-pending-restart-scope-cleanups-sequential-starvation.md)
  — If multiple unconfirmed restart scope cleanups accumulate on Linux, wedged earlier entries can consume the 60-second
  startup cleanup budget and prevent later cleanable scopes from being reaped.

- [`trailing-unwind-failed-host-relaunch-drops-pending-scope-cleanup.md`](trailing-unwind-failed-host-relaunch-drops-pending-scope-cleanup.md)
  — If a session row disappears during a failing restart whose replacement host started a scoped process, failing to
  confirm scope teardown drops the pending cleanup obligation on the next supervisor start.

- [`trailing-pending-scope-cleanups-logs-invalid-session-id-unescaped.md`](trailing-pending-scope-cleanups-logs-invalid-session-id-unescaped.md)
  — When discarding a malformed session ID from startup scope cleanup, the supervisor logs it with raw Display (%id)
  instead of escaped Debug (?id), violating the session-ID logging rule.

- [`trailing-interrupted-upload-empty-title-and-batch-title-drop.md`](trailing-interrupted-upload-empty-title-and-batch-title-drop.md)
  — An interrupted upload in a session with an empty title renders 'to was', and batched interrupted uploads discard a
  later upload's title if the first upload started before the session row loaded.

- [`trailing-terminal-set-restore-blocks-carried-upload-interruptions.md`](trailing-terminal-set-restore-blocks-carried-upload-interruptions.md)
  — After reconnecting with a remembered shell tab while the session row is omitted or delayed, the visible Agent
  terminal never receives carried upload-interruption lines even after the user starts typing.

- [`trailing-overflowing-banner-stack-resets-to-oldest-after-reconnect.md`](trailing-overflowing-banner-stack-resets-to-oldest-after-reconnect.md)
  — When an overflowing banner stack reconnects or takes back control and its newest notice was already drawn before
  going inert, the stack resets to the top and hides the newest banner off the bottom.

- [`trailing-later-codex-github-failure-hides-claude-cleanup-warning.md`](trailing-later-codex-github-failure-hides-claude-cleanup-warning.md)
  — If legacy Claude settings cleanup fails on a host and a later Codex or GitHub credential step also fails, Farhelm
  hides the Claude cleanup warning and reports the unrelated error for Claude replacements.

- [`trailing-credential-delivery-test-stale-claude-settings-assertion.md`](trailing-credential-delivery-test-stale-claude-settings-assertion.md)
  — The backend-failure delivery test still asserts .claude/settings.json is absent (always true now), leaving failed
  agent-credentials and Codex config targets unverified before and after retry.

- [`trailing-replace-with-existing-env-checkout-promises-trash.md`](trailing-replace-with-existing-env-checkout-promises-trash.md)
  — In Replace with inside an existing sandbox or container, switching Destination to Managed checkout says the checkout
  moves to trash on Delete even though deleting in an environment discards or destroys it permanently.

- [`trailing-shifted-wide-wrap-spacer-drops-background.md`](trailing-shifted-wide-wrap-spacer-drops-background.md) —
  Deleting characters on a colored terminal row that ended in a wrapped wide character drops the shifted padding cell's
  background color and leaves a one-column gap.

- [`trailing-right-half-wide-erase-keeps-inverse-decoration-and-link.md`](trailing-right-half-wide-erase-keeps-inverse-decoration-and-link.md)
  — When a program erases only the right half of an inverted, underlined, struck-through, or hyperlinked wide character
  with the same raw background, Farhelm keeps painting those attributes and link hit area across the erased column.

- [`trailing-deleting-wide-spacer-swallows-shifted-space.md`](trailing-deleting-wide-spacer-swallows-shifted-space.md) —
  Deleting only the right half of a wide character shifts a following space into its column and absorbs it into the wide
  glyph, losing the space in copy and link detection.

- [`trailing-left-half-wide-erase-drops-orphaned-background.md`](trailing-left-half-wide-erase-drops-orphaned-background.md)
  — Erasing or deleting only the left half of a colored wide character leaves its surviving right cell painted in the
  default background instead of its colored background.

- [`trailing-host-scoped-trash-confirmation-stays-open-invisibly.md`](trailing-host-scoped-trash-confirmation-stays-open-invisibly.md)
  — If a host leaves or empties while its 'Delete all on' confirmation is open in Trash, the invisible confirmation
  keeps all other hosts' Delete/Retry buttons disabled and swallows the first Close or Escape.

- [`trailing-mac-reboot-shell-discards-command-status-tag.md`](trailing-mac-reboot-shell-discards-command-status-tag.md)
  — The manual macOS reboot test helper ignores the wrapped exit-status tag on SSH commands, so a lossy SSH server can
  make a failed reboot look successful and stall for 30 minutes.

- [`trailing-resolve-remote-status-clears-local-transport-signal.md`](trailing-resolve-remote-status-clears-local-transport-signal.md)
  — If the local SSH or Podman client process is killed by a signal after stderr delivers the status record, Farhelm
  discards the signal termination and can treat truncated stdout as exit 0.

- [`trailing-podman-place-commands-lack-ssh-exit-status-wrapping.md`](trailing-podman-place-commands-lack-ssh-exit-status-wrapping.md)
  — Podman commands over SSH trust raw SSH exit codes rather than status-tag wrapping, which would break container
  builds and deletion checks on any Linux SSH server that loses exit codes; whether a supported Linux SSH server does so
  remains unverified.

- [`trailing-container-host-unanswered-embeds-raw-stderr.md`](trailing-container-host-unanswered-embeds-raw-stderr.md) —
  When a verified container host command fails before the guard runs, Farhelm copies up to 64 KiB of uncropped,
  unsanitized remote stderr directly into host status and HTTP errors.

- [`trailing-podman-refresh-settles-claims-with-stale-pre-probe-row.md`](trailing-podman-refresh-settles-claims-with-stale-pre-probe-row.md)
  — If Setup or Update updates a host's remote install paths while a container host check is running, the check settles
  claims using the stale pre-probe row and can ignore an active installation identity refusal.

- [`trailing-podman-settle-claim-refuses-third-host-duplicate-transfer.md`](trailing-podman-settle-claim-refuses-third-host-duplicate-transfer.md)
  — When a stale unused container-host entry is refused as a duplicate of a third host, the verified owner of the
  container machine cannot automatically take over the unused claim.

- [`trailing-podman-probe-swallows-session-limit-on-identity-read.md`](trailing-podman-probe-swallows-session-limit-on-identity-read.md)
  — Hitting SSH MaxSessions on the final machine-identity step of a container host check reports an unreadable identity
  instead of naming the SSH session limit.

- [`trailing-podman-build-prepare-uses-runtime-seam-bypassing-admission.md`](trailing-podman-build-prepare-uses-runtime-seam-bypassing-admission.md)
  — Later recipe build and preparation stages look up cached runtime capabilities instead of rechecking fresh admission,
  allowing new helper containers to start on hosts with an ordinary identity refusal or stale probe.

- [`trailing-claim-podman-place-retains-unused-claim-after-twin.md`](trailing-claim-podman-place-retains-unused-claim-after-twin.md)
  — When an unused SSH host moves externally to a machine claimed by another host, Refresh refuses it as a twin but
  keeps its old claim on disk, blocking a third entry for the vacated machine.

- [`trailing-podman-leftovers-skip-place-identity-verification.md`](trailing-podman-leftovers-skip-place-identity-verification.md)
  — If a claimed container host's SSH alias moves externally, deleting an unregistered leftover in Settings ->
  Containers can falsely report success while leaving it running, and refreshing can hide all leftovers.

- [`trailing-session-pane-stale-doc-on-helm-pushes.md`](trailing-session-pane-stale-doc-on-helm-pushes.md) — The
  session_pane module comment still describes the pre-#4623 behavior where the helm withheld pushes while a command was
  in flight.

- [`trailing-trash-deleted-drops-removal-report-on-unordered-race.md`](trailing-trash-deleted-drops-removal-report-on-unordered-race.md)
  — If an unordered Trash refresh fails or runs ahead of a Trash delete, the Trash button can drop the freeing-space
  status or skip rechecking the listing after the delete settles.

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

- [`trailing-tailscale-retarget-skips-ssh-route-lock.md`](trailing-tailscale-retarget-skips-ssh-route-lock.md) — Editing
  a Tailscale host's destination races concurrent probes, sign-in prompts, and Podman checks—potentially deleting
  known-hosts pins under active SSH, leaving old masters alive, or re-inserting the old destination.

- [`trailing-ssh-route-lock-leaks-cache-locks-on-delete-race.md`](trailing-ssh-route-lock-leaks-cache-locks-on-delete-race.md)
  — Removing an SSH host concurrently with a probe, sign-in preflight, or Podman check can re-insert an orphaned entry
  into cache_locks for the lifetime of the helm.

- [`trailing-podman-leftovers-run-ssh-without-ssh-route-lock.md`](trailing-podman-leftovers-run-ssh-without-ssh-route-lock.md)
  — Listing or deleting leftover containers on an unused SSH host races turning off Tailscale mode or removing the host,
  risking failed SSH commands or orphaned Tailscale keys and ControlMasters.

- [`trailing-probe-tailscale-mismatch-runs-before-new-entry-validation.md`](trailing-probe-tailscale-mismatch-runs-before-new-entry-validation.md)
  — Probing an already-registered destination with the opposite Tailscale mode or probing a managed environment host
  with Tailscale enabled asks to change Host settings instead of reporting AlreadyAdded or an invalid target.

- [`trailing-add-host-tailscale-checkbox-evicts-focus.md`](trailing-add-host-tailscale-checkbox-evicts-focus.md) —
  Pressing Enter on the Tailscale checkbox in Add host or receiving a background approval card during an agent host-add
  request disables the checkbox and drops keyboard focus from it.

- [`trailing-host-settings-tailscale-switch-refusal-prefix-and-stuck-error.md`](trailing-host-settings-tailscale-switch-refusal-prefix-and-stuck-error.md)
  — Toggling Tailscale in Host settings during a launch shows an unprefixed sentence fragment, stacks any earlier
  failure above it, and leaves the launch refusal stuck even after an in-flight toggle succeeds.

- [`trailing-reply-and-outcome-debug-can-expose-ntfy-topic.md`](trailing-reply-and-outcome-debug-can-expose-ntfy-topic.md)
  — Although ntfy::Settings omits Debug so diagnostics never include the ntfy topic, Reply and operations::Outcome
  derive Debug over raw JSON response bodies that carry it (no production log call currently formats them).
