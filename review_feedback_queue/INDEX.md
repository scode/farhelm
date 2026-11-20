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

- [`tunnel-reuses-custom-helm-credential-origin.md`](tunnel-reuses-custom-helm-credential-origin.md) — A desktop tunnel
  can receive a previously used custom browser helm port and expose its retained device credential to remote HTML in the
  same browser profile. The default helm port and embedded desktop credentials are outside this conditional route.

- [`folder-suggestion-enter-can-submit-launcher.md`](folder-suggestion-enter-can-submit-launcher.md) — Enter on a
  visible folder suggestion can submit the launcher. A successful Replace with can retire the source session.

- [`quick-switcher-enter-can-run-a-different-template.md`](quick-switcher-enter-can-run-a-different-template.md) — If
  the template list changes, Enter can start a different saved setup from the one still shown as selected.

## High priority: material UX degradation

- [`older-development-archive-moves-replacement-folder.md`](older-development-archive-moves-replacement-folder.md) — An
  interrupted archive written by older development code can move a replacement folder after device/inode reuse when
  birth time is unavailable. Current permanent deletion refuses this birthless case; a proven older-release variant may
  be filtered.

- [`older-handoff-reports-success-for-directory-program.md`](older-handoff-reports-success-for-directory-program.md) —
  Installing an older release can report success while leaving an already damaged app unlaunchable because its
  main-program path is a directory.

- [`older-installer-prunes-live-custom-state-version.md`](older-installer-prunes-live-custom-state-version.md) —
  Repeated installation of v0.27.0 can remove the executable a newer running supervisor still needs for later session
  launches, when that supervisor uses another state directory.

- [`automatic-legacy-installer-overwrites-later-terminal-choice.md`](automatic-legacy-installer-overwrites-later-terminal-choice.md)
  — An app kept open across a Terminal downgrade can choose an older installer that ignores its expected installation
  state, then replace a newer Terminal installation completed while it downloads.

- [`older-handoff-keeps-damaged-version-program.md`](older-handoff-keeps-damaged-version-program.md) — Reinstalling
  v0.27.0 can report that Farhelm is ready while retaining its already damaged command-line executable, even though the
  installer downloaded verified replacement bytes.

- [`container-token-read-blocks-host-admission.md`](container-token-read-blocks-host-admission.md) — A blocking
  configured token file can hold up other session creates and deletes across the host.

- [`podman-storage-selection-escapes-lifecycle-lookup.md`](podman-storage-selection-escapes-lifecycle-lookup.md) — A
  container created under a login shell's Podman configuration can survive while Stop reports absence from a different
  store.

- [`late-bound-agent-host-misses-adoption.md`](late-bound-agent-host-misses-adoption.md) — If an interrupted terminal
  starts only after its replacement supervisor finishes both startup scans, the agent can keep running while Farhelm
  cannot open its terminal. Another supervisor restart is needed to find it.

- [`retired-runtime-retains-restart-seed-reply.md`](retired-runtime-retains-restart-seed-reply.md) — An input/paste
  reserved before retirement can remain retained with no receipt; the same late-send lifetime on the runtime's control
  channel can also leave an accepted Restart waiting for a history-seed reply after the runtime has ended.

- [`extreme-history-numbering-hangs-selection.md`](extreme-history-numbering-hangs-selection.md) — Extreme peer-supplied
  history numbering can hang the page when the user selects text in a later screen row. The record now supplies its
  missing hostile-host acceptance context and preserves the unresolved browser scope.

- [`orphan-cleanup-overlay-forgets-failed-discovery.md`](orphan-cleanup-overlay-forgets-failed-discovery.md) — When
  discovery fails while cleanup remains unfinished, earlier orphan rows disappear and unrelated fleet updates keep
  retrying the failed read.

- [`credential-push-misses-retired-lifetime.md`](credential-push-misses-retired-lifetime.md) — A replacement connection
  can hide the last lease's zero transition, allowing obsolete credential transport work to delay reconnect. No
  disclosure, wrong-machine delivery or user-work loss is established.

- [`settings-save-races-final-replacement.md`](settings-save-races-final-replacement.md) — Credential delivery can
  silently overwrite an agent settings save made after its final digest check and before replacement. The new comparison
  protects earlier edits but leaves this narrower window.

- [`moved-on-launch-return.md`](moved-on-launch-return.md) — A failed launch or follow-up question can reopen the
  launcher and take the keyboard after the user has switched sessions or started typing elsewhere. It should wait behind
  a notice the user opens.

- [`late-suspend-question-interrupts-work.md`](late-suspend-question-interrupts-work.md) — After choosing Suspend in
  Hosts, the user can resume typing while Farhelm checks activity. A late reply can then open a modal question over that
  work and take keyboard focus instead of waiting to be reached.

- [`trailing-agent-host-add-explicit-queued-opening-ignores-moved-on.md`](trailing-agent-host-add-explicit-queued-opening-ignores-moved-on.md)
  — Queued host-add dialogs still take focus after the user moves on. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-new-container-proxy-snapshot-capability-cache.md`](trailing-new-container-proxy-snapshot-capability-cache.md)
  — New containers can permanently inherit a cached host proxy environment. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-experimental-supervisor-outside-container-sessions-retired-delete-only.md`](trailing-experimental-supervisor-outside-container-sessions-retired-delete-only.md)
  — Retired container sessions deliberately lose Stop and terminal access. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-offline-uninstall-old-private-tmux-survivor.md`](trailing-offline-uninstall-old-private-tmux-survivor.md) —
  Accepted survivor residual does not establish destructive overreach. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-archive-trash-epoch-lacks-unconditional-fleet-wake.md`](trailing-archive-trash-epoch-lacks-unconditional-fleet-wake.md)
  — Archive epoch can change without waking other clients. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-plain-environment-lifecycle-direction-before-screen-update.md`](trailing-plain-environment-lifecycle-direction-before-screen-update.md)
  — Opposite lifecycle direction is limited to an accepted input-delivery race. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-seal-listing-consumption-phase-integration.md`](trailing-environment-seal-listing-consumption-phase-integration.md)
  — The intermediate sealed-listing change has no helm seal sender yet. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-container-leftover-delete-registers-global-configuration-save.md`](trailing-container-leftover-delete-registers-global-configuration-save.md)
  — Container leftover deletion stalls unrelated GUI requests. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-restart-success-retired-by-precompletion-stale-listing.md`](trailing-restart-success-retired-by-precompletion-stale-listing.md)
  — A stale pre-Restart listing can retire success before terminal admission. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-custom-farhelm-binary-at-private-tmux-path-removed-by-update.md`](trailing-custom-farhelm-binary-at-private-tmux-path-removed-by-update.md)
  — A custom Farhelm executable at the reserved tmux path collides with cleanup. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-unconditional-stdin-option-compatibility.md`](trailing-environment-unconditional-stdin-option-compatibility.md)
  — Unconditional environment idle limit is specified. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-podman-rebuild-retires-fallback-before-success.md`](trailing-podman-rebuild-retires-fallback-before-success.md)
  — Failed container Rebuild discards the previous usable images. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-recipe-place-status-error-discards-success-and-has-no-retry.md`](trailing-recipe-place-status-error-discards-success-and-has-no-retry.md)
  — Failed per-place status reads discard controls and lack a direct retry. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-pending-commit-tag-no-dispatch-crash-never-settles.md`](trailing-podman-pending-commit-tag-no-dispatch-crash-never-settles.md)
  — Interrupted image capture can permanently block recipe cleanup. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-environment-d-drops-explicit-empty-proxy.md`](trailing-podman-environment-d-drops-explicit-empty-proxy.md)
  — Empty proxy variables disappear after the container user manager restarts. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-leftover-discovery-all-or-nothing-and-row-loss.md`](trailing-podman-leftover-discovery-all-or-nothing-and-row-loss.md)
  — Failure at an enumerated place suppresses healthy container leftovers. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-ownership-wal-normal-tail-loss.md`](trailing-podman-ownership-wal-normal-tail-loss.md) — Power loss
  can roll back container ownership before external work. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-podman-ordinary-wal-tail-loss-of-create-ownership.md`](trailing-podman-ordinary-wal-tail-loss-of-create-ownership.md)
  — Integrated container ownership keeps the accepted WAL power-loss limit. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-replace-with-accepts-incompatible-environment-recent.md`](trailing-replace-with-accepts-incompatible-environment-recent.md)
  — Environment Recent leaves Replace with unable to launch. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-replace-with-environment-recent-bypasses-held-placement-admission.md`](trailing-replace-with-environment-recent-bypasses-held-placement-admission.md)
  — Environment recents can put Replace with into an unlaunchable placement. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-launcher-feed-restarts-cancel-first-environment-catalog.md`](trailing-launcher-feed-restarts-cancel-first-environment-catalog.md)
  — Fleet notices can repeatedly cancel the launcher's first recipe read. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-restart-retirement-two-filtered-trigger-subsets.md`](trailing-restart-retirement-two-filtered-trigger-subsets.md)
  — Sidebar retirement can suppress the terminal after a successful Restart. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-retired-container-cli-helper-removal.md`](trailing-retired-container-cli-helper-removal.md) — Retired
  helpers refuse both old CLI entry and old queued isolation intent. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-startup-treats-old-terminals-as-ended-even-if-ending-the-server-fails.md`](trailing-startup-treats-old-terminals-as-ended-even-if-ending-the-server-fails.md)
  — Startup treats old terminals as ended even if ending the server fails. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-already-polled-displaced-input-handover.md`](trailing-already-polled-displaced-input-handover.md) — Keep
  unchanged already-polled handover separate from newly delayed natural-detach cancellation. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-delete-docs-overstate-admission-timeout-as-decision-bound.md`](trailing-environment-delete-docs-overstate-admission-timeout-as-decision-bound.md)
  — Delete documentation overstates the admission timeout's decision guarantee. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-old-terminal-retirement-startup-continues-after-failed-kill.md`](trailing-old-terminal-retirement-startup-continues-after-failed-kill.md)
  — Failed old-terminal termination still permits Restart. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-old-terminal-startup-cannot-discover-all-legacy-tab-identities.md`](trailing-old-terminal-startup-cannot-discover-all-legacy-tab-identities.md)
  — Old tab daemons can survive when their tab id is undiscoverable. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-delete-gone-adopted-tab-host-without-process-confirmation.md`](trailing-delete-gone-adopted-tab-host-without-process-confirmation.md)
  — Gone-tab Delete bypass is unverified after investigating the shipped producer. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-grid-only-old-page-display-and-malformed-input-detachment.md`](trailing-grid-only-old-page-display-and-malformed-input-detachment.md)
  — An old open page requires reload after the grid-only update. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`omitted-session-row-drops-live-tabs.md`](omitted-session-row-drops-live-tabs.md) — A list update can take away the
  shell tab you’re using and make the agent terminal receive your next typing. The shell on the host can keep running.

- [`interrupted-upload-notice-lost-on-session-removal.md`](interrupted-upload-notice-lost-on-session-removal.md) — When
  you switch away during an upload and the old session is replaced or deleted, Farhelm can cancel the upload without
  ever showing you that it stopped.

- [`late-composition-commit-reaches-previous-terminal.md`](late-composition-commit-reaches-previous-terminal.md) — Text
  from an input method or dead key can go to the shell tab you just left if the platform is still using that tab's input
  handler.

- [`upload-path-pastes-into-restarted-agent.md`](upload-path-pastes-into-restarted-agent.md) — If you restart a session
  during an upload, the file can finish normally while its path is pasted into the new agent, possibly in a different
  conversation.

- [`session-delete-shortcut-ignores-composition.md`](session-delete-shortcut-ignores-composition.md) — Pressing the
  session Delete shortcut while composing text can delete the open session, including its processes, when Skip session
  confirmations is enabled.

- [`wayland-terminal-paste-ignores-file-list.md`](wayland-terminal-paste-ignores-file-list.md) — Copying a file in a
  Wayland file manager and pasting into Farhelm can paste its text representation or do nothing instead of attaching the
  file.

- [`terminal-file-requests-can-wait-indefinitely.md`](terminal-file-requests-can-wait-indefinitely.md) — A terminal file
  link can stay at “Checking…” or “Downloading…” indefinitely while the host still appears connected and the rest of
  Farhelm works.

- [`tensorlake-lifecycle-falls-back-to-unowned-name.md`](tensorlake-lifecycle-falls-back-to-unowned-name.md) — If
  Tensorlake setup refuses a sandbox name already in use, the failed Farhelm host can still offer Suspend or Resume and
  act on the existing sandbox.

- [`retargeted-ssh-alias-can-forget-live-container.md`](retargeted-ssh-alias-can-forget-live-container.md) — If an SSH
  alias starts reaching a different machine or login account, Farhelm can report the old container deleted and remove it
  from Hosts even though it is still on the original machine.

- [`managed-ubuntu-bootstrap-pid1-stop-can-time-out.md`](managed-ubuntu-bootstrap-pid1-stop-can-time-out.md) — On some
  Linux Podman setups, a first Managed Ubuntu build can finish installing packages and still fail each time before it
  can save an image.

- [`podman-recipe-cleanup-tries-parent-before-child.md`](podman-recipe-cleanup-tries-parent-before-child.md) — Deleting
  an unused container recipe can keep failing because Farhelm tries to remove an image that its own second image still
  needs. Retrying can make the same attempt every time.

- [`podman-place-probe-bypasses-host-identity-refusal.md`](podman-place-probe-bypasses-host-identity-refusal.md) —
  Farhelm can treat two SSH entries for the same machine and account as usable container places even while its ordinary
  host connection refuses the duplicate.

- [`desktop-updater-waits-indefinitely-for-installer.md`](desktop-updater-waits-indefinitely-for-installer.md) — A
  stalled installer can leave the desktop updater waiting indefinitely. Later checks cannot run, and a user-requested
  check can stay on Installing.

- [`macos-terminal-paste-blocks-gui-thread.md`](macos-terminal-paste-blocks-gui-thread.md) — When macOS stalls while
  supplying clipboard contents, pasting into the terminal can freeze the whole Farhelm window.

- [`antigravity-restart-may-stop-without-relaunching.md`](antigravity-restart-may-stop-without-relaunching.md) —
  Antigravity can stop during Restart and require another Restart to reopen the session.

- [`bulk-mute-can-reset-window.md`](bulk-mute-can-reset-window.md) — Muting or unmuting a large marked set can reset the
  desktop connection after applying only part of the batch.

- [`gone-terminal-retained-handle-hides-recovery.md`](gone-terminal-retained-handle-hides-recovery.md) — A terminal the
  supervisor knows is gone can stay in reconnecting state and hide the recovery card.

- [`linux-ctrl-f-leaves-settings-field-at-end.md`](linux-ctrl-f-leaves-settings-field-at-end.md) — Ctrl+F at the end of
  a Linux Settings field can focus Search and send the next keystroke there.

## Other: correctness, diagnostics, cleanup, or convenience

- [`untagged-sandbox-leftover-labelled-another-helm.md`](untagged-sandbox-leftover-labelled-another-helm.md) — A
  leftover sandbox whose name has no helm tag is labelled as another helm's.

- [`open-url-card-without-host-shows-no-site.md`](open-url-card-without-host-shows-no-site.md) — An open-link card for a
  URL without a parsed host shows no site row; whether such a URL reaches a card is unverified.

- [`removed-host-strands-tunnel-listeners.md`](removed-host-strands-tunnel-listeners.md) — Removing a host leaves its
  tunnel ports occupied while individual close requests fail.

- [`menu-close-dismisses-new-notifications.md`](menu-close-dismisses-new-notifications.md) — Closing one popup may
  dismiss a newly opened notification list and mark it read; event ordering remains unverified.

- [`download-interruption-promise-omits-crash-exception.md`](download-interruption-promise-omits-crash-exception.md) —
  The product's partial-download cleanup promise omits the newly accepted crash and force-quit exception.

- [`add-host-refusal-bypasses-dialog-admission.md`](add-host-refusal-bypasses-dialog-admission.md) — A delayed Add-host
  refusal may reopen a mutually inert dialog beside Settings; Escape remains available.

- [`browser-suite-ignores-explicit-port.md`](browser-suite-ignores-explicit-port.md) — The browser suite silently
  ignores its documented manual port override.

- [`catalog-cleanup-can-start-untracked-test-session.md`](catalog-cleanup-can-start-untracked-test-session.md) —
  Releasing a held catalog or finishing a pending key mint during cleanup may start an untracked fake test session;
  adverse scheduling is unmeasured.

- [`delayed-refusal-reopens-cancelled-replace-question.md`](delayed-refusal-reopens-cancelled-replace-question.md) — A
  delayed refusal may reopen a cancelled Replace question; a further affirmative answer is still required.

- [`history-fixture-reset-races-selection-save.md`](history-fixture-reset-races-selection-save.md) — An automatic
  selection save may overwrite history-fixture or capture restoration; adverse ordering is unverified.

- [`instruction-file-case-alias-evades-check.md`](instruction-file-case-alias-evades-check.md) — Case aliases evade the
  local instruction-file check; actual vendor lookup on case-insensitive filesystems is unverified.

- [`pinned-host-dialog-chrome-can-hide-content.md`](pinned-host-dialog-chrome-can-hide-content.md) — The deleted dialog
  record still has an unresolved Host Settings footer-clipping case; exact geometry is unverified.

- [`prompt-prefill-test-opens-before-source-selection.md`](prompt-prefill-test-opens-before-source-selection.md) — The
  prompt-prefill and clone-defaults tests may open New before source selection; startup ordering is unverified.

- [`release-heading-can-close-review-page-script.md`](release-heading-can-close-review-page-script.md) — A malformed
  curated release heading can inject review-page script; attacker influence and practical impact are unverified.

- [`removal-destination-collapses-distinguishing-spaces.md`](removal-destination-collapses-distinguishing-spaces.md) —
  Removal/refusal destinations and manual startup paths collapse distinguishing spaces; live SSH examples, clipboard
  serialization and harmful startup reliance remain unverified.

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
  recording recurring baseline flakes, including quick-launch and further browser observations. Private browser evidence
  remains unauthenticated; inspected Rust failures do not establish latent flakiness.

- [`host-dialog-reservation-does-not-exclude-settings.md`](host-dialog-reservation-does-not-exclude-settings.md) —
  Settings or the shortcuts overview during host-dialog admission may leave both dialogs inert until Escape; native
  ordering is unmeasured.

- [`launcher-submit-callback-retains-render-snapshots.md`](launcher-submit-callback-retains-render-snapshots.md) —
  Launcher renders retain additional submit snapshots until closure; practical memory impact is unmeasured.

- [`conformance-test-never-admits-management-candidates.md`](conformance-test-never-admits-management-candidates.md) —
  The generic responsiveness regression busy-refuses its candidates before their admitted waits.

- [`probe-timeout-discards-parsed-remote-path.md`](probe-timeout-discards-parsed-remote-path.md) — Probe cleanup can
  discard an already parsed executable path; a real escaped-helper trigger is unverified.

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
  earlier caller-cleanup failure can skip restoring the prepared-checkout switch, chrome confirmation preference or
  Hosts-test sort; effective confirmation contamination needs an incoming true value, and decision coverage remains
  ambiguous.

- [`tunnel-consent-conflicts-with-restart-dialog.md`](tunnel-consent-conflicts-with-restart-dialog.md) — Tunnel consent
  and Restart with can become mutually inert; native behavior and Escape recovery remain untested.

- [`claude-dialog-text-mimics-input-box.md`](claude-dialog-text-mimics-input-box.md) — Agent-written text in a Claude
  question dialog can read as the input box; dormant while agent prompts are off, multi-row rendering unverified.

- [`codex-output-mimics-queued-question.md`](codex-output-mimics-queued-question.md) — Codex output shaped like a
  queued-question hint reads waiting mid-turn, so Restart skips its question; the rendering premise is unverified.

- [`sticky-parent-guard-contradicts-own-docs.md`](sticky-parent-guard-contradicts-own-docs.md) — The test recorder's and
  deflake's docs claim to refuse another account's /tmp/farhelm-tmp, which a sticky one passes.

- [`uninstall-completion-bypasses-size-decision.md`](uninstall-completion-bypasses-size-decision.md) — Uninstall work
  entered unattended landing eligibility after exceeding a required size gate, without a maintainer waiver in the pinned
  record; additional approximate-size cases retain their authorization ambiguity and later approvals.

- [`launcher-subscription-test-masks-missing-observers.md`](launcher-subscription-test-masks-missing-observers.md) — A
  launcher regression shares replay subscriptions and changes several positive-control fields, masking the omission it
  claims to catch; no production defect is established.

- [`closed-popovers-repeat-prefix-scans.md`](closed-popovers-repeat-prefix-scans.md) — Refreshes repeat scans and sorted
  prefix work even with popups closed; supported-scale latency is unmeasured and browser availability coverage is
  ambiguous.

- [`held-preference-reply-blocks-fixture-restoration.md`](held-preference-reply-blocks-fixture-restoration.md) — A
  failed held reply can block four tests' preference/template restoration; the conditional contamination schedule is
  untested.

- [`canonical-folder-history-contract-conflict.md`](canonical-folder-history-contract-conflict.md) — Host canonical-path
  replies become remembered folder text, despite a product rule saying those replies must not choose remembered
  defaults. The implementation specification explicitly carries canonical paths into history; spelling changes are
  established, arbitrary path substitution is not.

- [`history-reply-lacks-installation-provenance.md`](history-reply-lacks-installation-provenance.md) — If a host changes
  installations and changes back while the GUI misses the intermediate snapshot, history from the intermediate
  installation can become the current host's frozen folder default. Final launch still targets the current machine.

- [`launcher-source-supersession-route-unresolved.md`](launcher-source-supersession-route-unresolved.md) — A mounted
  launcher retains its earlier source, but no complete shipped input sequence establishes the suspected harmful action
  after that source is superseded. Narrow token and permission repairs do not settle the whole conditional lead.

- [`uncertain-launch-marker-overwrites-older-attempt.md`](uncertain-launch-marker-overwrites-older-attempt.md) — A later
  uncertain launch overwrites the single visible marker while an earlier managed attempt remains retained. The original
  duplicate-launch claim was refuted; a compatible pre-admission sequence producing the stronger harm remains missing.

- [`restart-selector-retains-source-conversation.md`](restart-selector-retains-source-conversation.md) — Restart with
  retains an entered conversation when its source is reseeded. A shipped source-switch action after editing, and a
  harmful resulting restart, remain unestablished; the older Clone/Replace discard does not explicitly settle this new
  surface.

- [`trash-alias-refresh-skips-movement-pause.md`](trash-alias-refresh-skips-movement-pause.md) — A host alias refresh or
  viewport reflow can change Trash layout without triggering its movement pause. A supported layout placing another
  enabled Delete under the old pointer remains unestablished, so this is a conditional fingerprint gap.

- [`uninstall-final-scan-concurrent-mount-gap.md`](uninstall-final-scan-concurrent-mount-gap.md) — A mount introduced
  after uninstall's last containment scan could be traversed by removal. No complete supported topology-change sequence
  establishing loss of unrelated work was found; the stable-layout repairs remain valid.

- [`uninstall-inspection-newline-identity-gap.md`](uninstall-inspection-newline-identity-gap.md) — Inspection can
  transform newline-bearing path text before comparing identities. A supported incidental path and complete layout
  defeating all removal guards remain missing; this is separate from the repaired ordinary path cases.

- [`older-direct-bun-omp-reporter-gap.md`](older-direct-bun-omp-reporter-gap.md) — Older direct-Bun launches keep a
  permissive reporter corridor. A nested interactive OMP receiving the required reporter and ancestry remains
  unestablished; only the proven older-release subset has filter coverage.

- [`bun-option-value-retains-launcher-exemption.md`](bun-option-value-retains-launcher-exemption.md) — The OMP
  classifier may treat a Bun option value of x as its launcher marker. Actual supported Bun parsing and a nested
  reporter route remain missing, so no wrong-conversation execution is claimed.

- [`renamed-grok-reporter-ancestry-gap.md`](renamed-grok-reporter-ancestry-gap.md) — A wholly renamed Grok may leave a
  nested child looking like the sole recognized emitter. A runnable supported vendor configuration remains
  unestablished; the concrete ordinary Grok chain was repaired.

- [`repeated-ssh-markers-collapse-incidental-config.md`](repeated-ssh-markers-collapse-incidental-config.md) — The
  provisioning test's begin/begin/end parser removes the intervening span. A supported unrelated stanza layout and stale
  per-run alias selection producing actual config loss remain unestablished; balanced writers do not prove that route.

- [`owner-death-child-precedes-pid-publication.md`](owner-death-child-precedes-pid-publication.md) — Killing the test
  owner before it publishes the child's PID can leave its owned five-minute sleep outside cleanup. This is separate from
  the explicitly accepted same-run PID-reuse limit.

- [`quick-launch-mark-inverts-live-owner.md`](quick-launch-mark-inverts-live-owner.md) — A delayed mark/clear click
  chooses its direction from the current owner rather than the direction its render offered. The conditional mismatch is
  established; no harmful automatic launch is shown, and delivery-window coverage is ambiguous.

- [`quick-launch-clear-erases-new-agent-mark.md`](quick-launch-clear-erases-new-agent-mark.md) — An agent can move the
  quick-launch mark after the GUI's fresh read but before its unconditional clear, losing the newer choice. Broad
  last-writer-wins wording and the narrower preservation rule conflict.

- [`rename-lost-reply-restores-old-shortcut-indication.md`](rename-lost-reply-restores-old-shortcut-indication.md) — A
  rename's shortcut change can commit while its reply is lost, leaving the GUI showing the old mark while the next
  shortcut reads the new one. The implementation follows an explicit historical rollback criterion; its full
  indication/dispatch consequences remain ambiguously covered.

- [`post-clone-test-save-overtakes-restoration.md`](post-clone-test-save-overtakes-restoration.md) — Post-clone, Hosts,
  naming, bolt-test and bulk-Delete saves can remain in flight after private-stack restoration, then overwrite it.
  Adverse scheduling and required incoming values remain unmeasured; no production user-work loss is claimed.

- [`held-font-test-lacks-mount-attempt-witness.md`](held-font-test-lacks-mount-attempt-witness.md) — The font test can
  finish its no-mount observation window before a terminal first tries mounting. Ordinary page text can request the font
  first, so later terminal appearance does not prove the test would catch premature mounting.

- [`restart-template-plan-overstates-refusal.md`](restart-template-plan-overstates-refusal.md) — The plan says no saved
  template can apply in Restart with, although kindless older templates can already apply editable fields. This is a
  background explanation error; its implementation criteria remain useful.

- [`implementation-version-doc-omits-schema-update-guard.md`](implementation-version-doc-omits-schema-update-guard.md) —
  The implementation specification's version/skew description omits the schema-based guard that can forbid Update. The
  product specification and actual guard remain intact.

- [`quick-launch-preparation-refusal-uses-plain-alert.md`](quick-launch-preparation-refusal-uses-plain-alert.md) — A
  quick-launch preparation read failure bypasses the new bordered refusal presentation. Whether the presentation
  requirement or diagnostic filter covers this stage is unresolved; request admission stays safe.

- [`folder-refusal-classifier-matches-path-text.md`](folder-refusal-classifier-matches-path-text.md) — A path containing
  familiar error phrases can be misclassified as missing rather than refused or non-directory. This changes an advisory
  annotation, not launch authority; the rare-diagnostic filter's scope is ambiguous.

- [`shortcut-batch-revalidates-with-new-host-snapshot.md`](shortcut-batch-revalidates-with-new-host-snapshot.md) — A
  host refresh during the shortcut batch's context wait can make projection and per-template validation use different
  snapshots. Earlier draft edits can remain after a later template refuses; no automatic launch or user-work loss is
  established.

- [`fresh-claude-trust-birthtime-contract-conflict.md`](fresh-claude-trust-birthtime-contract-conflict.md) — Initial
  fresh-checkout creation can edit Claude trust without the no-birth-time check stated elsewhere. Fresh-creation rules
  may intentionally be an exception; this is a source/specification conflict, not an established unauthorized
  replacement-folder grant.

- [`host-remove-doc-overstates-second-answer-guard.md`](host-remove-doc-overstates-second-answer-guard.md) — The GUI
  contract says Host Remove refuses every second answer during its request, but another permanent answer can still
  change the future confirmation preference. The helper explicitly preserves that same-burst behavior; the operation
  itself remains guarded.

- [`narrow-outage-card-left-edge-unreachable.md`](narrow-outage-card-left-edge-unreachable.md) — A very narrow, short
  viewport may put part of an outage explanation beyond the left reachable edge. Actual supported sizing and recovery
  geometry remain unverified; this differs from accepted overlap that remains scrollable.

- [`ordinary-setup-discards-deferred-agent-add.md`](ordinary-setup-discards-deferred-agent-add.md) — Submitting ordinary
  host setup can clear a deferred agent Add-host opening that was already seen. Its approval card remains available for
  manual reopening; automatic presentation is lost.

- [`queued-agent-dialog-inherits-elapsed-arm-timer.md`](queued-agent-dialog-inherits-elapsed-arm-timer.md) — The next
  queued agent Add-host dialog can become interactive without a fresh pause. Whether every new dialog needs that pause
  is ambiguous; no harmful carried-over input sequence was established.

- [`installer-rerun-advice-omits-stale-lock-prerequisite.md`](installer-rerun-advice-omits-stale-lock-prerequisite.md) —
  Installer rerun advice omits manual stale-lock removal after an older unmarked lock or a new marking failure followed
  by interruption. Executable refusal is accepted; documentation-filter coverage remains ambiguous.

- [`window-check-export-budget-omits-standalone-captures.md`](window-check-export-budget-omits-standalone-captures.md) —
  Ten raw screenshots can exceed the standalone reference budget, leaving fewer exported captures even when the check
  passes. Original image bytes remain in digest-bound raw events; total evidence loss is not claimed.

- [`native-app-binding-may-relaunch-crashed-candidate.md`](native-app-binding-may-relaunch-crashed-candidate.md) — The
  test provider's app-binding operation might relaunch a candidate that crashed after the PID check. The exact
  provider-version semantics are unavailable, so a hidden false pass remains a conditional source lead.

- [`old-process-regression-misses-cli-premise.md`](old-process-regression-misses-cli-premise.md) — The
  old-running-version regression can fail because its fixture omits a CLI observation, even if the intended old-process
  guard regresses. The production guard remains correct.

- [`installer-prerequisite-test-conflates-two-guards.md`](installer-prerequisite-test-conflates-two-guards.md) —
  Supplying neither prerequisite cannot show that each prerequisite is independently required. The test can miss removal
  of either production guard; no shipped installer defect is established.

- [`removal-copy-hidden-unicode-warning-unresolved.md`](removal-copy-hidden-unicode-warning-unresolved.md) — A retained
  removal path can copy hidden Unicode without a warning. Control bytes and line breaks are rejected, and
  validated/copied fields match; the warning requirement and harmful downstream paste remain unestablished.

- [`schema-disposition-removes-excluded-variant-tracking.md`](schema-disposition-removes-excluded-variant-tracking.md) —
  An explicit queue-removal instruction also removes an equal-number schema concern excluded from the planned fix.
  Whether that means deliberate whole-finding discard is ambiguous; no concrete incompatible migration pair or
  corruption route was established.

- [`legacy-page-server-rebinding-source-gap.md`](legacy-page-server-rebinding-source-gap.md) — The legacy local static
  server omits Host/Origin validation. Supported browser admission and an actual confidential served page remain
  missing, so rebinding impact is unestablished. The instructed decision-page workflows were separately retired.

- [`retained-refusal-can-clear-restart-delete-protection.md`](retained-refusal-can-clear-restart-delete-protection.md) —
  Retained-refusal settlement lacks a generation check and could replace terminal-less Launching with Error, weakening
  Delete protection. A compatible supported validation-failure trigger across concurrent Restart remains missing; no
  unconfirmed process kill is claimed.

- [`stale-report-approval-retires-unseen-followup.md`](stale-report-approval-retires-unseen-followup.md) — If another
  review's follow-up lands while an older report page remains open, approving that old page can retire the newer
  report's tracking. This pre-existing slug-only approval gap is preserved by the rewrite; no product-data loss is
  claimed.

- [`invalid-folder-style-overrides-focus-ring.md`](invalid-folder-style-overrides-focus-ring.md) — After a folder
  refusal, the focused editor uses the same outline as the blurred invalid field. CSS specificity establishes the
  missing distinct ring; practical accessibility impact was not measured.

- [`ensure-host-warning-names-unavailable-setup-action.md`](ensure-host-warning-names-unavailable-setup-action.md) — The
  startup warning tells an ordinary remote host with an invalid install path to use a setup action its row does not
  offer. Manual/API recovery remains possible; this is diagnostic correctness with ambiguous filter coverage.

- [`host-edit-doc-overstates-preapproval-refusal.md`](host-edit-doc-overstates-preapproval-refusal.md) — A rename
  colliding with another host's name can ask for approval and then fail, although its docstring says refused aliases get
  no card. The source check is syntax-only before approval; no authorization defect is claimed.

- [`window-check-inherits-preflight-image-history.md`](window-check-inherits-preflight-image-history.md) — The
  window-drag check validates the whole pass's frame history against only its own event stream, so prior preflight
  images can reject otherwise valid evidence. Actual captures and frequency are unmeasured; release-test availability is
  the consequence.

- [`template-cached-collision-blocks-freed-name.md`](template-cached-collision-blocks-freed-name.md) — After the CLI or
  an agent deletes a template, repeated GUI saves under its now-free name can still report a collision without doing the
  fresh check. Reopening refreshes the list; the gate predates the patch.

- [`failed-test-response-does-not-fence-settings-write.md`](failed-test-response-does-not-fence-settings-write.md) —
  Transport failure, forwarded saves and unfenced retries can let a test restore settings before its earlier owned write
  commits. Version-matched Playwright source establishes the response and cleanup boundaries; adverse scheduling is
  unmeasured. Additional recipe scope: independent cleanup can accept a missing row before an already accepted POST
  inserts it, leaving fictional recipe and credential rows behind.

- [`quick-switcher-pending-read-keeps-old-shortcut-labels.md`](quick-switcher-pending-read-keeps-old-shortcut-labels.md)
  — A successful but still-pending opening read can leave old shortcut labels visible in the switcher. Existing
  decisions accept failed reads and later moves, but this interval's coverage is ambiguous; selection still captures the
  displayed template name.

- [`closed-model-enter-test-can-misattribute-restart.md`](closed-model-enter-test-can-misattribute-restart.md) — An
  incorrectly admitted early Enter can start an asynchronous restart after the immediate request-count assertion; the
  later primary press can then be refused while the earlier request satisfies the test. Production behavior remains
  correct.

- [`claude-plan-abort-removes-established-report-pin.md`](claude-plan-abort-removes-established-report-pin.md) — Failed
  Claude or Muse Restart clears the report pin before restoring the old pane. A delayed child could replace its Resume
  target, but an ordinary hooked, unmarked nested producer remains unestablished.

- [`new-opening-folder-inherits-fallback-host.md`](new-opening-folder-inherits-fallback-host.md) — A host refresh before
  New mounts can pair the selected remote session's raw folder with the fallback local host. This contextual mechanism
  predates the reviewed patch; final launch still shows and fences its target, and delivery-window coverage is
  ambiguous.

- [`omitted-ipv6-local-routing-source-gap.md`](omitted-ipv6-local-routing-source-gap.md) — Some IPv6 forms receive
  public-link permission. A supported route reaching a local resource remains unestablished, so a local authorization
  bypass is conditional.

- [`browse-result-survives-automatic-host-fallback.md`](browse-result-survives-automatic-host-fallback.md) — On desktop,
  an old host's directory result can become an explicit folder after New falls back to the local host. Frequency and the
  web route are unverified; final Launch still displays and checks its target.

- [`installer-fallback-lock-parent-replacement-source-gap.md`](installer-fallback-lock-parent-replacement-source-gap.md)
  — Replacing the installation's parent directory could separate the held directory from its fallback lock. A supported
  sequence causing conflicting work on the same app remains unestablished.

- [`uninstall-refusal-missing-identity-wording-source-gap.md`](uninstall-refusal-missing-identity-wording-source-gap.md)
  — A refusal could describe an unidentified installation as a different installation. A compatible shipped sequence
  reaching that missing-identity comparison remains unestablished.

- [`hidden-trash-overlap-migration-result-source-gap.md`](hidden-trash-overlap-migration-result-source-gap.md) — The
  planned trash migration must skip active checkouts without making existing archives disappear from tracking. No
  prescribed or implemented skip result establishing the suspected loss was found.

- [`shutdown-wait-test-does-not-witness-waiting.md`](shutdown-wait-test-does-not-witness-waiting.md) — The shutdown
  regression can pass even if shutdown stops waiting for a launch whose registration is still in flight.

- [`installer-file-symlink-test-masks-nofollow-removal.md`](installer-file-symlink-test-masks-nofollow-removal.md) — The
  installer's symlink test would still pass if its no-follow protection disappeared, because the fixture points to a
  directory that another check rejects.

- [`installer-stage-real-folder-substitution-source-gap.md`](installer-stage-real-folder-substitution-source-gap.md) —
  Cleanup removes the current private staging directory without rechecking its acquired identity. An ordinary supported
  replacement placing unrelated work there remains unestablished.

- [`installer-legacy-inode-approval-alias-source-gap.md`](installer-legacy-inode-approval-alias-source-gap.md) — A
  different directory with the same inode number could inherit legacy-app replacement approval. A supported incidental
  Mac history reaching harmful replacement remains unestablished.

- [`installer-mounted-descendant-admission-source-gap.md`](installer-mounted-descendant-admission-source-gap.md) —
  Recursive installer cleanup could reach external mounted contents if such a tree passes all preceding moves. No
  supported incidental Mac topology establishing that admission was found.

- [`installer-compatibility-folder-replacement-source-gap.md`](installer-compatibility-folder-replacement-source-gap.md)
  — Cleanup renames the current compatibility-lock folder before removing it. A supported replacement causing harmful
  movement of unrelated work remains unestablished.

- [`installer-terminal-backup-crash-persistence-source-gap.md`](installer-terminal-backup-crash-persistence-source-gap.md)
  — Whether power loss can preserve a replacement Terminal link while losing the preceding foreign-file backup remains
  unresolved. The source's live-file preservation order alone establishes neither loss nor crash safety.

- [`installer-moved-app-cleanup-durability-source-gap.md`](installer-moved-app-cleanup-durability-source-gap.md) —
  Interrupted cleanup can flush the app currently at the usual location rather than the app moved during installation. A
  harmful second-crash history remains unestablished.

- [`installer-interruption-test-rejects-no-swap-fallback.md`](installer-interruption-test-rejects-no-swap-fallback.md) —
  The same-version interruption regression can reject the installer's documented fallback on a filesystem without atomic
  directory exchange.

- [`installer-leftover-test-rejects-fallback-lock-file.md`](installer-leftover-test-rejects-fallback-lock-file.md) —
  Installer leftovers checks and the native legacy-refusal snapshot reject the supported permanent fallback lock. A Mac
  test volume requiring that fallback remains unverified after investigation.

- [`automatic-reopen-can-launch-older-replacement-app.md`](automatic-reopen-can-launch-older-replacement-app.md) — An
  older bundle substituted during automatic reopening can launch instead of being refused. Native dispatch and timing
  remain unmeasured; no work-loss consequence is established.

- [`legacy-expectation-is-parsed-as-cli-option.md`](legacy-expectation-is-parsed-as-cli-option.md) — The installer CLI
  rejects its documented ID-less expectation when the shell supplies it as a separate argument. An ordinary published
  updater history reaching that boundary remains unestablished.

- [`uninstall-skipped-stage-resurrection-source-gap.md`](uninstall-skipped-stage-resurrection-source-gap.md) — Normal
  readable staging leftovers are now consumed before uninstall removes the app. A supported history leaving a skipped
  stage that later restores the uninstalled app remains unestablished.

- [`legacy-update-expectation-identity-collision-source-gap.md`](legacy-update-expectation-identity-collision-source-gap.md)
  — Two legacy ownership records can have indistinguishable expected identities. A complete supported
  updater/replacement history producing harm through that collision remains unestablished.

- [`steel-split-restart-availability-wording-conflict.md`](steel-split-restart-availability-wording-conflict.md) — One
  redesign instruction could disable Restart with when users need to enter a conversation manually, while other clauses
  explicitly preserve its independent availability. No current GUI regression is established.

- [`delivery-reports-reuse-cases-from-failed-commands.md`](delivery-reports-reuse-cases-from-failed-commands.md) —
  Several delivery reports combine passing cases from a failed command with narrower reruns, despite the repository's
  rule that evidence reuse is per command. They disclose the failures; this is a validation-accounting conflict.

- [`steel-visual-delivery-omits-required-states.md`](steel-visual-delivery-omits-required-states.md) — The redesign
  report reaches delivery with some changed states reviewed only from code, although its plan requires visual captures
  of every changed screen and state. An external waiver remains unverified.

- [`claude-unread-parent-report-displacement-source-gap.md`](claude-unread-parent-report-displacement-source-gap.md) — A
  hooked, unmarked nested Claude could overwrite an unread parent conversation-switch report before admission refuses
  the child. An ordinary supported producer and complete consequential sequence remain unestablished.

- [`claude-oversized-stop-payload-source-gap.md`](claude-oversized-stop-payload-source-gap.md) — A Claude Stop callback
  above the hook's input limit is rejected. Whether supported vendor payloads can reach that limit and lose needed
  tracking information remains unestablished.

- [`trash-failed-host-refresh-test-lacks-application-witness.md`](trash-failed-host-refresh-test-lacks-application-witness.md)
  — The multi-host trash-failure test can miss a regression that loses the failed host's remaining entries or warning
  when its final refresh arrives.

- [`launch-input-database-duplication-rationale-overstated.md`](launch-input-database-duplication-rationale-overstated.md)
  — The implementation specification says launch input contains nothing absent from the session database, but
  create-time agent-request prompts are an exception. Retaining the private current-generation file is explicitly
  allowed.

- [`older-handoff-power-loss-durability-source-gap.md`](older-handoff-power-loss-durability-source-gap.md) — The
  historical installer uses copy and rename without explicit flushes. Supported Mac utility and filesystem guarantees
  establishing an unusable post-power-loss installation remain unverified.

- [`matching-restart-template-model-normalizes-historical-permissions.md`](matching-restart-template-model-normalizes-historical-permissions.md)
  — A template repeating the current agent and model can change a session's historical agent-owned permission setting to
  Guarded without containing a permission edit, making an otherwise unchanged Restart draft count as edited.

- [`light-appearance-darkens-terminal-catchup.md`](light-appearance-darkens-terminal-catchup.md) — Light appearance
  darkens catch-up prose while its terminal background stays dark; display duration is unmeasured.

- [`header-confirmation-lowers-link-refusal.md`](header-confirmation-lowers-link-refusal.md) — A header confirmation
  lowers a safely refused link's explanation beneath unrelated covers; exact overlap is unmeasured.

- [`effort-catalog-replaces-focused-control.md`](effort-catalog-replaces-focused-control.md) — An arriving model catalog
  can replace the focused effort buttons with a dropdown; the next key is consumed without restoring effort focus.

- [`restart-trust-discards-observed-canonical-folder.md`](restart-trust-discards-observed-canonical-folder.md) — Restart
  trust discards its lookup's canonical folder. An alias round trip exposes a conditional mismatch, but an ordinary
  accidental producer and in-scope trust-boundary harm remain unestablished.

- [`late-reporter-preparation-capture-conflict.md`](late-reporter-preparation-capture-conflict.md) — A reporter failure
  arising only during named restart can retain the previous conversation. Broad tracking-failure acceptance and the
  specific preparation-clearing promise conflict; no healthy concurrent-publication trigger is established.

- [`unsupported-filter-warning-cannot-wrap.md`](unsupported-filter-warning-cannot-wrap.md) — A CSS conflict prevents the
  older-helm filter warning from wrapping; its full text remains in the tooltip and accessible name.

- [`short-window-host-rail-reachability.md`](short-window-host-rail-reachability.md) — A very short window may put the
  host choices behind the footer; engine hit testing and keyboard recovery remain unverified.

- [`visual-capture-clicks-hidden-templates.md`](visual-capture-clicks-hidden-templates.md) — The unscoped visual capture
  opens Hosts and then waits on the hidden Templates button.

- [`completed-only-retry-keeps-host-warning.md`](completed-only-retry-keeps-host-warning.md) — Another client's success
  may leave an old host warning if every Running observation is missed; the end-to-end schedule remains unverified.

- [`hosts-panel-test-misses-filtered-row-premise.md`](hosts-panel-test-misses-filtered-row-premise.md) — The terminal
  preservation test never establishes that its selected session row was filtered out.

- [`host-removal-test-leaks-confirmation-preference.md`](host-removal-test-leaks-confirmation-preference.md) — A host
  removal test omits preference restoration; changed later behavior needs an unestablished incoming true value.

- [`hidden-progress-test-observes-unaffected-host.md`](hidden-progress-test-observes-unaffected-host.md) — The hidden
  management regression observes a menu already excluded by a separate row-order rule.

- [`narrow-sidebar-tests-measure-outer-container.md`](narrow-sidebar-tests-measure-outer-container.md) — Two narrow row
  tests retain containment bounds from the wider outer sidebar after shrinking only its content area.

- [`request-cards-cover-tunnel-controls.md`](request-cards-cover-tunnel-controls.md) — A waiting agent request can cover
  the opened tunnel list and prevent pointer access to overlapping close controls. Exact native overlap geometry remains
  unmeasured, and specification coverage is ambiguous.

- [`tunnel-error-covers-approval-disclosure.md`](tunnel-error-covers-approval-disclosure.md) — A floating tunnel error
  can paint over an agent request's approval facts while its answer handlers remain enabled. A concrete harmful approval
  layout is **unverified after investigation**.

- [`plan-deferral-survives-replacement-question.md`](plan-deferral-survives-replacement-question.md) — After the
  maintainer answers one large plan question, a later ordinary question from that plan can remain excluded from
  automatic handling. Whether the operational-artifact exclusion covers this workflow defect is ambiguous.

- [`stale-plan-answer-clears-new-question.md`](stale-plan-answer-clears-new-question.md) — A delayed answer can be
  attached to a plan's replacement question and make that plan eligible again, even when the new question was reserved
  for the maintainer. Actual unauthorized execution is unestablished.

- [`landing-baseline-delays-new-block.md`](landing-baseline-delays-new-block.md) — A plan question raised during a
  landing round can wait another 110 minutes before automatic handling. Whether the operational-artifact exclusion
  covers this scheduling defect is ambiguous.

- [`tunnel-allow-keyboard-guard-source-gap.md`](tunnel-allow-keyboard-guard-source-gap.md) — The focused Allow tunnels
  button lacks the repeated/composing-Enter guard. A complete input sequence causing unintended consent remains
  unverified after investigation; Cancel-first focus and container Enter retain their separate protections.

- [`closed-conflicting-pr-does-not-wake-executor.md`](closed-conflicting-pr-does-not-wake-executor.md) — A waiting plan
  can remain unstarted after the last conflicting PR closes, until unrelated activity or a manual drain.

- [`old-executor-close-resets-new-holder-bookmarks.md`](old-executor-close-resets-new-holder-bookmarks.md) — An old
  executor's prescribed full-close cleanup can reset plan bookmarks already being used by the new holder.

- [`legacy-muse-hook-preservation-source-gap.md`](legacy-muse-hook-preservation-source-gap.md) — A broad legacy
  hook-preservation claim remains uncertain for an unrecognized override spelling and destructive vendor replacement
  behavior.

- [`short-trash-frame-reachability-unresolved.md`](short-trash-frame-reachability-unresolved.md) — A very short Trash
  window may leave its content or controls unreachable; actual supported geometry and recovery remain unverified.

- [`uuid-hold-intercepts-terminal-lease.md`](uuid-hold-intercepts-terminal-lease.md) — The destination-authority test's
  global UUID hold can catch late terminal startup instead of the intended launch key.

- [`recent-setup-description-omits-wrapper-prefix.md`](recent-setup-description-omits-wrapper-prefix.md) — Different
  saved setups can look identical even though Enter restores and launches different wrapper prefixes.

- [`update-preplanning-retarget-consent-ambiguity.md`](update-preplanning-retarget-consent-ambiguity.md) — A delayed
  Update plan can freeze a host entry's newer destination while the requesting GUI still shows its previous destination;
  whether that consent is sufficient is unresolved.

- [`uninstall-test-restores-before-run-settles.md`](uninstall-test-restores-before-run-settles.md) — The uninstall
  progress test can restore its backend and registry while its accepted uninstall is still active.

- [`sort-test-cleanup-stops-after-first-failure.md`](sort-test-cleanup-stops-after-first-failure.md) — One cleanup
  failure can leave the sort test's remaining owned sessions running until wider teardown.

- [`older-muse-restart-refusal-blames-command-mode.md`](older-muse-restart-refusal-blames-command-mode.md) — Plain
  Restart can describe an older Muse agent launch as a command launch with reporting disabled.

- [`attachment-failure-announcement-unresolved.md`](attachment-failure-announcement-unresolved.md) — Attachment errors
  have no explicit live announcement, so screen-reader users may miss a failed paste without navigating to its message.

- [`container-tab-close-overpromises-process-termination.md`](container-tab-close-overpromises-process-termination.md) —
  The tab Close warning and internal contract say all started processes end, although detached container work can
  survive.

- [`host-permission-copy-quotes-retired-labels.md`](host-permission-copy-quotes-retired-labels.md) — Specifications and
  helm refusal text quote old host-permission labels rather than the controls users now see.

- [`trash-question-reopen-skips-answer-focus.md`](trash-question-reopen-skips-answer-focus.md) — Reopening the same
  visible Trash question can leave keyboard focus on its opener, so Enter reopens it again instead of reaching an
  answer.

- [`host-menu-requested-index-survives-command-removal.md`](host-menu-requested-index-survives-command-removal.md) —
  Keyboard navigation can stay on the wrong command position after Adopt or Update disappears from an open host menu.

- [`alias-propagation-test-reloads-before-live-row-check.md`](alias-propagation-test-reloads-before-live-row-check.md) —
  The alias regression reloads the page before checking the row name, so it misses the promised live update behavior.

- [`terminal-size-readout-points-css-pixels-conflict.md`](terminal-size-readout-points-css-pixels-conflict.md) — The new
  terminal size readout is documented as points while its value reports CSS pixels.

- [`browser-shard-failure-sweeps-running-siblings.md`](browser-shard-failure-sweeps-running-siblings.md) — An early
  shard exception can clean up workers that sibling shards are still using. Additional reviewed scope: A Rust runner
  infrastructure or local evidence-save exception can sweep sibling sandboxes before their work and evidence collection
  settle.

- [`browser-shard-failures-exit-successfully.md`](browser-shard-failures-exit-successfully.md) — The browser harness can
  exit successfully even when its shards report validation failures. Additional reviewed scope: The Rust sandbox command
  can exit successfully even when its shard results say tests failed.

- [`worker-evidence-deleted-after-download-failure.md`](worker-evidence-deleted-after-download-failure.md) — The harness
  can delete a failed worker's evidence even when downloading that evidence failed.

- [`worker-archive-omits-playwright-artifact-directory.md`](worker-archive-omits-playwright-artifact-directory.md) — The
  worker archive can omit the screenshots, traces and timelines Playwright writes outside its archived directory.

- [`worker-template-acceptance-ignores-prerequisite-failures.md`](worker-template-acceptance-ignores-prerequisite-failures.md)
  — Automated template acceptance can welcome a worker despite failed SSH readiness; manager readiness now gates it.

- [`worker-checkpoint-polling-has-no-failure-exit.md`](worker-checkpoint-polling-has-no-failure-exit.md) — Worker
  checkpoint polling can wait indefinitely for completion; the exact service transition needed for the suspected
  permanent wait remains unresolved. Additional reviewed scope: Unverified after investigation: Rust staging also polls
  a snapshot indefinitely until completed, with no deadline. The actual service transition needed to leave that wait
  permanently stuck remains unestablished.

- [`worker-allocation-precedes-ownership-ledger.md`](worker-allocation-precedes-ownership-ledger.md) — A created worker
  can fall outside cleanup when ledger writing fails or remote creation succeeds before the local request times out.

- [`confirmation-rollback-omits-container-cleanup.md`](confirmation-rollback-omits-container-cleanup.md) — A failed
  final create confirmation can remove a borrowed-checkout session row without performing its container cleanup; an
  ordinary uncovered failure producer remains missing.

- [`replace-container-ready-wording-exceeds-create-boundary.md`](replace-container-ready-wording-exceeds-create-boundary.md)
  — Container Replace text promises a ready replacement container before deleting the source, while code waits only for
  the new session/terminal boundary.

- [`rootless-pause-inherits-session-marker.md`](rootless-pause-inherits-session-marker.md) — The rootless container
  infrastructure process can inherit a session marker and be selected for unnecessary termination or recreation.

- [`anonymous-container-volumes-survive-removal.md`](anonymous-container-volumes-survive-removal.md) — Removing a
  container can leave its anonymous image volumes behind; whether Farhelm promises to remove them is unresolved.

- [`podman-remote-routing-host-contract-ambiguity.md`](podman-remote-routing-host-contract-ambiguity.md) — Ambient
  Podman routing can choose a remote engine despite Farhelm's host-local container description; the intended
  configuration boundary is unresolved.

- [`update-reminder-before-initial-provisioning-read.md`](update-reminder-before-initial-provisioning-read.md) — An
  Update reminder can advertise availability before the first provisioning-state read has established whether that
  action is eligible.

- [`completed-followup-leaves-old-update-tracked-busy.md`](completed-followup-leaves-old-update-tracked-busy.md) — If
  the GUI misses a successor's Running state and first sees Completed, an earlier tracked Update can remain busy.

- [`automatic-update-planning-serializes-hosts.md`](automatic-update-planning-serializes-hosts.md) — One slow host's
  planning can delay automatic updates for later hosts even though each individual wait is bounded.

- [`finished-task-handle-published-after-host-purge.md`](finished-task-handle-published-after-host-purge.md) — A
  completed uninstall or failed initial sandbox launch can leave task bookkeeping reinserted after its host was removed.

- [`codex-wrapper-guides-omit-isolation-purpose.md`](codex-wrapper-guides-omit-isolation-purpose.md) — The wrapper
  guides explain generated Codex arguments without explaining their role in keeping one session from sharing another's
  daemon.

- [`abbreviated-ipv4-short-label-scope-ambiguity.md`](abbreviated-ipv4-short-label-scope-ambiguity.md) — A valid
  abbreviated IPv4 destination can lose address components in its short label; whether such forms count as IP literals
  under the display contract is unresolved.

- [`valid-ssh-uri-short-label-parsing.md`](valid-ssh-uri-short-label-parsing.md) — Valid SSH URIs with a trailing slash,
  service-name port or empty port can show an incorrectly shortened host label.

- [`literal-this-machine-alias-rewritten-in-full-labels.md`](literal-this-machine-alias-rewritten-in-full-labels.md) —
  An explicitly chosen local alias equal to this machine is rewritten on full-name surfaces even though its short label
  preserves it.

- [`initial-shortcut-replay-lacks-busy-release-wake.md`](initial-shortcut-replay-lacks-busy-release-wake.md) — An
  initial replay that observes busy state has no explicit wake when that state clears; an ordinary overlapping input
  producer remains missing.

- [`full-host-tooltip-conflicts-with-line-cap.md`](full-host-tooltip-conflicts-with-line-cap.md) — The shared tooltip
  cap conflicts with full-value disclosure for host names and approval requester titles. The new admitted long-title
  route is source-confirmed; original host-alias wrapping remains unmeasured.

- [`selected-expanded-host-hover-white-on-white.md`](selected-expanded-host-hover-white-on-white.md) — In Light
  appearance, hovering the selected expanded host can make its name and destination unreadable.

- [`template-save-placeholder-validation-contract-conflict.md`](template-save-placeholder-validation-contract-conflict.md)
  — Template Save can retain a prefix that Launch later refuses, despite wording that the new placement check runs at
  Save time.

- [`cleanup-stop-transport-error-skips-delete.md`](cleanup-stop-transport-error-skips-delete.md) — A transport error
  from a cleanup Stop can prevent Delete even when the helper already knows the fake session's ID.

- [`typed-name-matching-repeats-quadratic-prefix-work.md`](typed-name-matching-repeats-quadratic-prefix-work.md) — Typed
  template matching performs quadratic duplicate checks before applying the five-row display cap; practical
  supported-scale latency remains unmeasured.

- [`unapproved-ssh-help-copy-receiver-contract-unresolved.md`](unapproved-ssh-help-copy-receiver-contract-unresolved.md)
  — An unapproved agent Add-host draft can copy raw SSH help text; the required warning and harmful downstream paste
  remain unestablished.

- [`discovery-registration-bypasses-host-row-lock.md`](discovery-registration-bypasses-host-row-lock.md) — Discovery
  registration can bypass a host row's execution lock, but a supported differing-path transition with lasting
  consequences remains missing.

- [`ordinary-menu-opening-focus-before-visible-placement.md`](ordinary-menu-opening-focus-before-visible-placement.md) —
  An ordinary menu opening may accept keyboard input before its measured placement becomes visible; the native
  input/paint sequence remains unverified.

- [`muse-capability-docstring-denies-reporting-and-resume.md`](muse-capability-docstring-denies-reporting-and-resume.md)
  — A capability docstring still says Muse lacks reporting and Resume command support although the current integration
  provides both.

- [`automatic-update-mutates-unadopted-skewed-installation.md`](automatic-update-mutates-unadopted-skewed-installation.md)
  — Automatic Update can change a replacement installation at the saved SSH destination before discovering its new
  identity and refusing attachment.

- [`container-template-replacement-test-reuses-old-config.md`](container-template-replacement-test-reuses-old-config.md)
  — The template regression cannot detect keeping the old container settings because its replacement uses identical
  values.

- [`legacy-quickstart-stops-surviving-supervisor.md`](legacy-quickstart-stops-surviving-supervisor.md) — The parked
  quickstart contradicts itself about whether the supervisor survives quitting the Mac app.

- [`literal-deleted-suffix-breaks-desktop-owner.md`](literal-deleted-suffix-breaks-desktop-owner.md) — A development
  executable literally ending in ` (deleted)` can lose ownership recognition and leave an older answering build reused.

- [`desktop-smoke-supervisor-precedes-cleanup-record.md`](desktop-smoke-supervisor-precedes-cleanup-record.md) —
  Interruption before desktop-test ownership publication can leave a detached supervisor outside cleanup; timing and
  survivor duration are unmeasured, and an older browser-helper discard's broader intent is ambiguous.

- [`stalled-input-test-accepts-discarded-bytes.md`](stalled-input-test-accepts-discarded-bytes.md) — The stalled-input
  regression accepts discarded typing rather than proving that controls remain responsive beside pending input.

- [`preclient-output-test-connects-during-wait.md`](preclient-output-test-connects-during-wait.md) — The pre-client
  output regression connects during its premise wait, so it can pass without output preceding the first negotiated
  client.

- [`late-output-test-does-not-distinguish-flush-order.md`](late-output-test-does-not-distinguish-flush-order.md) — The
  overdue-frame regression cannot distinguish flushing before new output from flushing afterward; production order is
  currently correct.

- [`reflow-property-timeout-retains-workers.md`](reflow-property-timeout-retains-workers.md) — If an allocating
  regression returns, property-test shrinking can retain timed-out workers and start more; no current emulator defect or
  measured memory exhaustion is claimed.

- [`runtime-recorder-guidance-conflict.md`](runtime-recorder-guidance-conflict.md) — Run-all-tests gives conflicting
  directions about whether emulator stress and memory tests require the runtime-test recorder; taking the optional
  direction can leave future failures without the normally required retained run record.

- [`restart-unwind-omits-tree-sweep-failure.md`](restart-unwind-omits-tree-sweep-failure.md) — The authoritative
  implementation specification incorrectly says a failed Restart restores the stopped old terminal whenever the
  replacement host can be ended; failed cleanup of processes the replacement started is another ambiguous path, leaving
  no displayed terminal.

- [`mac-process-lookup-truncates-live-host-command.md`](mac-process-lookup-truncates-live-host-command.md) — On a Mac
  with an oversized process listing, test cleanup can erase a stopped host's live socket and state directory, leaving
  that host unreachable for future cleanup.

- [`mac-process-lookup-transforms-socket-path.md`](mac-process-lookup-transforms-socket-path.md) — Mac test cleanup can
  remove a stopped terminal host's live socket and directory under certain valid injected paths, leaving the host
  unreachable for future cleanup.

- [`blocking-test-cleanup-prevents-async-listener-release.md`](blocking-test-cleanup-prevents-async-listener-release.md)
  — A normal successful supervisor unit test waits for the five-second host-cleanup deadline and retains its state
  directory because directory teardown blocks the same executor needed to release its aborted stand-in listener.

- [`input-test-comments-overstate-termination-coverage.md`](input-test-comments-overstate-termination-coverage.md) — The
  test comments claim stronger coverage and transport guarantees than the tests establish.

- [`steady-block-cursor-contract-conflict.md`](steady-block-cursor-contract-conflict.md) — Grid terminals deliberately
  blink a block cursor even when the program requests it steady, while the wire contract says blinking is the default
  unless the program says otherwise.

- [`unavailable-host-recipe-misses-fourth-disconnect.md`](unavailable-host-recipe-misses-fourth-disconnect.md) — The
  maintained follow-up asks for three disconnects, but a healthy test host reconnects after all three. A test following
  that recipe can fail before reaching the intended unavailable state.

- [`repeated-resizes-clear-missing-history-notice.md`](repeated-resizes-clear-missing-history-notice.md) — After
  restoring a checkpoint that omitted old scrollback, repeated taller/shorter resizes can remove the missing-output
  notice without replacing the old history.

- [`empty-history-clear-keeps-missing-history-notice.md`](empty-history-clear-keeps-missing-history-notice.md) — When a
  recovered checkpoint contains no scrollback because it omitted all the old history, an explicit clear-scrollback
  command does not dismiss the missing-output notice; the warning remains until enough new lines scroll or a full
  terminal reset occurs.

- [`uri-spare-capacity-missing-from-memory-budget.md`](uri-spare-capacity-missing-from-memory-budget.md) — Hyperlink
  allocations can be larger than the terminal memory estimate counts, although the logical storage counter still obeys
  its cap.

- [`host-restart-history-painter-omits-palette.md`](host-restart-history-painter-omits-palette.md) — Restart into a
  fresh terminal host changes retained scrollback colors when the previous program redefined terminal colors.

- [`width-table-change-removes-restored-history.md`](width-table-change-removes-restored-history.md) — A changed
  character-width table could displace restored history or fixups, but no supported writing-and-reading build pair with
  the required width change was found. Permanent history loss is not clearly covered by the accepted restore-and-redraw
  limit.

- [`restored-capped-title-loses-boundary-whitespace.md`](restored-capped-title-loses-boundary-whitespace.md) — Recovery
  can change a retained terminal title and its saved title-stack entries by removing trailing whitespace exposed by the
  title-size cap.

- [`snapshot-measurement-buffers-past-claimed-bound.md`](snapshot-measurement-buffers-past-claimed-bound.md) — Sizing a
  checkpoint historically retained a temporary stream beyond its claimed limit. Later bounded-retention repair and
  corrected MiB arithmetic are recorded for explicit cleanup.

- [`replay-work-model-omits-history-rotations.md`](replay-work-model-omits-history-rotations.md) — The checkpoint reader
  undercounts repeated history-buffer rearrangement for unusually large library-requested scrollback. No excessive
  latency at shipped settings or shipped producer for the extreme history count is established.

- [`lossy-storage-precedes-cursor-template-validation.md`](lossy-storage-precedes-cursor-template-validation.md) — A
  malformed terminal checkpoint can be accepted during recovery when its cursor template contains forbidden combining
  characters and the attribute-storage budget is exhausted.

- [`snapshot-comparison-ignores-link-grouping.md`](snapshot-comparison-ignores-link-grouping.md) — Snapshot regression
  checks can pass after terminal hyperlink identities or boundaries change.

- [`host-lock-table-contradicts-startup-span.md`](host-lock-table-contradicts-startup-span.md) — Farhelm's maintenance
  documentation states a lifecycle-lock constraint that startup deliberately violates, obscuring which recovery
  operation may perform host I/O while holding the lock.

- [`grid-input-omits-x10-only-mouse-reporting.md`](grid-input-omits-x10-only-mouse-reporting.md) — Programs that request
  only classic X10 mouse tracking lose clicks in terminal-host terminals, unlike the live raw-stream browser terminal.

- [`refused-input-test-can-credit-earlier-bytes.md`](refused-input-test-can-credit-earlier-bytes.md) — The terminal
  input-backpressure regression's claim that rejected input never reaches the host is stronger than what it observes.

- [`shift-enter-test-lacks-reader-readiness.md`](shift-enter-test-lacks-reader-readiness.md) — This integration test can
  report success without establishing the already-waiting reader its documentation requires.

- [`hosts-directory-replacement-stop-uncertainty.md`](hosts-directory-replacement-stop-uncertainty.md) — A changed
  directory could redirect terminal shutdown, but no supported history replacing it with another installation's matching
  live socket was established. Independent review does not agree to drop that remaining uncertainty; deliberate
  private-directory interference is separately accepted.

- [`desktop-smoke-misparses-space-bearing-run-path.md`](desktop-smoke-misparses-space-bearing-run-path.md) — Choosing a
  short fresh DESKTOP_SMOKE_DIR containing a space makes the gate report that a healthy created session has no live
  terminal.

- [`reconnect-replay-keeps-obsolete-event-count-fence.md`](reconnect-replay-keeps-obsolete-event-count-fence.md) — After
  a resize write interrupts replay and reconnects, Farhelm can keep refusing prompts as still restoring even after the
  terminal has caught up. More output or replacement of that runtime can clear it.

- [`paste-receipt-test-checks-before-ack-processing.md`](paste-receipt-test-checks-before-ack-processing.md) — The
  regression does not prove that a long paste stays unconfirmed until all its pieces are acknowledged. It can miss a
  receipt completing after the first piece. No current production receipt defect is established.

- [`stop-wait-test-releases-exit-before-wait-witness.md`](stop-wait-test-releases-exit-before-wait-witness.md) — The
  regression can miss Stop ceasing to wait for final terminal output, leaving a lost opportunity to return the complete
  launch-failure explanation undetected.

- [`foreign-host-refusal-test-cannot-observe-shutdown.md`](foreign-host-refusal-test-cannot-observe-shutdown.md) — The
  regression meant to protect another session's terminal cannot detect a Shutdown sent before the expected refusal.

- [`placeholder-grid-fixture-readoption-uncertainty.md`](placeholder-grid-fixture-readoption-uncertainty.md) — A
  placeholder-attachment test lacks a handshake with the supervisor's initial retry scan. Source checks found no
  barrier, but the complete execution causing a false failure remains unverified after investigation.

- [`seed-cancellation-test-accepts-lost-emulator.md`](seed-cancellation-test-accepts-lost-emulator.md) — The regression
  test for keeping an ended runtime's emulator readable also passes when read reports that the emulator was lost.

- [`cancellation-test-releases-encoder-before-abort-witness.md`](cancellation-test-releases-encoder-before-abort-witness.md)
  — The regression that promises to cancel a Restart caller during history encoding can instead let that caller finish
  normally, without rejecting the failed cancellation premise.

- [`child-hangup-masks-missing-stop-tree-sweep.md`](child-hangup-masks-missing-stop-tree-sweep.md) — The Stop regression
  can credit the death of an unmarked descendant without distinguishing whether Stop swept that descendant or merely
  killed its parent shell.

- [`restart-test-counts-zombie-as-running-agent.md`](restart-test-counts-zombie-as-running-agent.md) — The Restart
  regression can fail after correctly stopping the old agent because its observation mistakes an exited, unreaped
  process for a running one.

- [`idle-readoption-task-outlives-supervisor-drop.md`](idle-readoption-task-outlives-supervisor-drop.md) — Dropping an
  idle in-process supervisor without running its orderly-stop method leaves its background retry task and notification
  state allocated until the caller's Tokio runtime ends.

- [`restart-screen-captured-before-prompt-settles.md`](restart-screen-captured-before-prompt-settles.md) — Correct
  terminal recovery can fail the restart regression because its expected screen is captured before the fixture finishes
  writing the post-echo prompt.

- [`grid-shift-enter-loses-independent-altgraph.md`](grid-shift-enter-loses-independent-altgraph.md) — On a
  keyboard/browser that reports AltGraph independently of Ctrl and Alt, holding AltGraph while pressing Shift+Enter can
  insert Farhelm newline ESC CR instead of the ordinary Enter CR the existing terminal sends.

- [`bold-brightening-ignores-redefined-palette.md`](bold-brightening-ignores-redefined-palette.md) — A program
  redefining bright terminal colors still gets the original theme colors for bold text.

- [`invalidated-selection-drag-keeps-old-anchors.md`](invalidated-selection-drag-keeps-old-anchors.md) — If history
  resets, the terminal resizes, or the alternate screen switches while a local text drag is held, the next drag movement
  can recreate a selection using the invalidated old line positions.

- [`alternate-screen-wheel-keeps-stale-selection.md`](alternate-screen-wheel-keeps-stale-selection.md) — After selecting
  text in a full-screen terminal program with mouse reporting off, wheel navigation leaves that local selection
  highlighted.

- [`exit-notice-hides-oldest-scrollback-row.md`](exit-notice-hides-oldest-scrollback-row.md) — Once an agent's program
  exits, scrolling to the beginning of its retained output can still leave the oldest row clipped out of view.

- [`grid-drop-suppresses-refusal-explanation.md`](grid-drop-suppresses-refusal-explanation.md) — Dropping content onto a
  grid terminal historically suppressed its refusal explanation. The later native-drop repair is recorded for explicit
  cleanup; physical delivery on every engine is not certified.

- [`changed-file-hover-test-clicks-before-stat-readiness.md`](changed-file-hover-test-clicks-before-stat-readiness.md) —
  The file-link redraw regression can issue its only download click while the replacement file is still being checked,
  then treat the correctly ignored click as a download regression.

- [`grid-fixture-measures-before-bold-font-ready.md`](grid-fixture-measures-before-bold-font-ready.md) — The browser
  layout check does not establish that the bundled bold font is ready before measuring bold output, so it can assess
  fallback geometry instead of the intended terminal face.

- [`triple-click-truncates-long-logical-line.md`](triple-click-truncates-long-logical-line.md) — Triple-clicking a
  retained terminal logical line that extends more than 1000 wrapped rows from the click copies only part of that line.

- [`nul-delimited-osc-identity-ambiguity.md`](nul-delimited-osc-identity-ambiguity.md) — Different hyperlink identities
  historically collided in a delimiter key without an established harmful opening. The later JSON-pair repair is
  recorded alongside that investigated uncertainty for explicit cleanup.

- [`hovered-link-double-click-selects-one-word.md`](hovered-link-double-click-selects-one-word.md) — A stable hovered
  hyperlink with multiword text copies only the clicked word on double-click in a grid terminal, whereas xterm selects
  the entire hovered link range.

- [`block-copy-omits-wide-continuation-column.md`](block-copy-omits-wide-continuation-column.md) — A rectangular
  terminal copy omits the leading blank corresponding to a wide-character continuation cell on a row whose left
  selection boundary cuts through that character, shifting that row's copied content left relative to other selected
  rows.

- [`wide-wrap-inference-drops-real-boundary-space.md`](wide-wrap-inference-drops-real-boundary-space.md) — A real space
  printed at the right edge before a wide character can disappear when copied, and printed-URL detection can incorrectly
  append the following wide text to the URL instead of stopping at that space.

- [`combined-punctuation-word-selection-differs.md`](combined-punctuation-word-selection-differs.md) — Double-clicking
  unlinked text beside punctuation carrying a combining mark can select a shorter word in a grid terminal than in an old
  terminal.

- [`wide-wrap-placeholder-loses-final-cell-style.md`](wide-wrap-placeholder-loses-final-cell-style.md) — A wide glyph
  wrapping from the final column leaves a default-background hole where the terminal actually holds a styled blank; both
  screen and history use the conversion.

- [`initial-oversized-grid-cannot-attach.md`](initial-oversized-grid-cannot-attach.md) — The supervisor accepts an
  attach size above the grid stream's 1000-cell bound and then ends the attachment at its first `hello`; the page no
  longer sends one, but any client that does gets a terminal that fails to open.

- [`program-selected-cursor-color-never-reaches-viewer.md`](program-selected-cursor-color-never-reaches-viewer.md) — A
  program that changes its text cursor color continues to show the theme's fixed cursor color, even though Farhelm's
  emulator accepts the new color.

- [`partial-wide-erasure-loses-second-cell-background.md`](partial-wide-erasure-loses-second-cell-background.md) — A
  program erasing only the second column of a wide glyph with a different background can leave grid viewers painting the
  old first-cell background over that erased column.

- [`clipboard-notice-drain-lacks-per-turn-work-bound.md`](clipboard-notice-drain-lacks-per-turn-work-bound.md) — The
  viewer drains concurrently arriving clipboard writes without a per-turn work or cancellation bound. The missing bound
  is confirmed; material starvation or wider supervisor disruption remains unverified after investigation.

- [`resize-measurement-discards-arrival-timestamps.md`](resize-measurement-discards-arrival-timestamps.md) — The manual
  resize benchmark can report zero last-frame delivery time after it has transmitted its resize output.

- [`oversized-paste-test-cancels-before-pump-can-run.md`](oversized-paste-test-cancels-before-pump-can-run.md) — The
  test meant to prove that an oversized paste writes nothing can pass without ever allowing its input pump to attempt
  delivery of earlier paste parts.

- [`emulator-lock-test-accepts-expired-holder.md`](emulator-lock-test-accepts-expired-holder.md) — The test for
  nonblocking terminal input can pass after its emulator lock has already expired, so it can miss a regression that
  would wait for that lock.

- [`tab-retry-test-measures-observation-time.md`](tab-retry-test-measures-observation-time.md) — The retry regression
  can accept cleanup observed after 900 ms without proving that the retry itself waited for the required delay.

- [`last-output-fixture-readiness-precedes-trap.md`](last-output-fixture-readiness-precedes-trap.md) — The final-output
  regression can reject correct output preservation because its agent has published readiness before installing the
  exit-output trap.

- [`host-preservation-test-can-select-starter-pid.md`](host-preservation-test-can-select-starter-pid.md) — The
  reserved-session preservation regression can blame correct startup cleanup for a fixture launcher exiting normally,
  because its readiness check can save that temporary launcher as the terminal-host process.

- [`restart-test-types-before-safe-prompt-barrier.md`](restart-test-types-before-safe-prompt-barrier.md) — The restart
  tests can overwrite their startup marker with early test input and then report a healthy supervisor restart as having
  lost terminal contents.

- [`replacement-history-absence-asserted-before-delivery.md`](replacement-history-absence-asserted-before-delivery.md) —
  The replacement regression checks that old history is absent before proving history delivery has finished. The
  boundary gap is confirmed, but a concrete false-pass counterpart for this small fixture remains unverified after
  investigation.

- [`agent-text-helper-removes-wrap-boundary-spaces.md`](agent-text-helper-removes-wrap-boundary-spaces.md) — Browser
  agent checks can fail to recognize text that is present when a space lands at a soft-wrap boundary.

- [`mixed-input-acknowledgements-misattribute-written-bytes.md`](mixed-input-acknowledgements-misattribute-written-bytes.md)
  — After a transient Linux terminal-write allocation failure, input discarded by the terminal host can be reported as
  written when a later input succeeds before their acknowledgements are sent. The conditional source route is
  established; production prompts are disabled, and no user-work loss is claimed.

- [`emoji-report-names-wrong-vendored-patch.md`](emoji-report-names-wrong-vendored-patch.md) — An emoji landing-report
  follow-up names the parser patch instead of the terminal emulator patch containing its cosmetic edits.

- [`announcement-report-overstates-startup-validation.md`](announcement-report-overstates-startup-validation.md) —
  Announcement table validation cannot exclude the database and creation-version failures omitted by the landing report.

- [`self-ssh-guidance-misses-skipped-tests.md`](self-ssh-guidance-misses-skipped-tests.md) — Sandbox instructions
  promise dependent test failures after missing self-SSH, although non-CI multi-host cases can skip instead.

- [`feedback-discord-link-omits-external-indicator.md`](feedback-discord-link-omits-external-indicator.md) — The
  feedback dialog's Discord link omits the required external-link arrow.

- [`drag-copy-guidance-gap-stays-stale-after-text-size-change.md`](drag-copy-guidance-gap-stays-stale-after-text-size-change.md)
  — Changing terminal text size leaves visible drag-copy guidance at its old pixel gap instead of three current rows.

- [`detached-grid-retains-program-drag-guidance.md`](detached-grid-retains-program-drag-guidance.md) — A retained
  takeover screen can still raise guidance claiming its program handled a drag that the detached client cannot send.

- [`pending-replace-recreates-announcement.md`](pending-replace-recreates-announcement.md) — Moving pending Replace
  progress recreates its live region; repeated supported screen-reader speech remains unverified after investigation.

- [`replace-source-removal-hides-progress-and-timeout.md`](replace-source-removal-hides-progress-and-timeout.md) —
  Source removal can hide pending Replace progress and its eventual deadline explanation while navigation remains
  locked.

- [`joined-link-spec-omits-cursor-tail-exception.md`](joined-link-spec-omits-cursor-tail-exception.md) — The broad
  joined-link comparison promise omits an accepted cursor-positioned tail exception.

- [`announcement-close-missing-rendered-opening-token.md`](announcement-close-missing-rendered-opening-token.md) —
  Announcement close lacks its rendered opening token; an old close reaching reopened history remains unverified after
  investigation.

- [`announcement-history-overwrites-waiting-pending-batch.md`](announcement-history-overwrites-waiting-pending-batch.md)
  — Opening shown announcement history before modal admission can suppress this page's pending batch while durable rows
  remain queued.

- [`stable-recut-removes-retained-announcement-anchor.md`](stable-recut-removes-retained-announcement-anchor.md) — A
  different-version stable re-cut can remove the website anchor still targeted by carried announcements.

- [`recovery-gap-notice-lifetime-contract-conflict.md`](recovery-gap-notice-lifetime-contract-conflict.md) —
  Same-supervisor recovery exposes contradictory product and implementation promises for the missing-output notice's
  lifetime.

- [`sandbox-create-cancel-lacks-opening-guard.md`](sandbox-create-cancel-lacks-opening-guard.md) — Cancel lacks the
  creation question's opening check. A normal input sequence dismissing a later question remains unverified after
  investigation.

- [`successful-suspend-flashes-connecting.md`](successful-suspend-flashes-connecting.md) — A confirmed sandbox
  suspension can briefly change back to Connecting during its status recheck, even though the sandbox remains suspended.

- [`upload-failure-test-lacks-pipe-holder-witness.md`](upload-failure-test-lacks-pipe-holder-witness.md) — The
  failed-upload regression can pass without starting the pipe-holding fixture needed to test the original hang.

- [`brick-recovery-truncates-checkpoint-budget.md`](brick-recovery-truncates-checkpoint-budget.md) — A recoverable
  provider refusal can cut release checks below their required time budget or wrongly prevent their retry.

- [`checkpoint-image-triage-rootless-path-unresolved.md`](checkpoint-image-triage-rootless-path-unresolved.md) — The
  prior checkpoint-image assessment's root-only exclusion does not establish what happens through the plain-run
  inspection path. Actual rootless workload restoration remains unverified after investigation.

- [`sandbox-termination-poll-exceeds-phase-deadline.md`](sandbox-termination-poll-exceeds-phase-deadline.md) — Sandbox
  deletion can exceed its stated termination-polling allowance because each lookup receives a fresh timeout beyond the
  phase's remaining time.

- [`ambiguous-resume-retains-old-connection-token.md`](ambiguous-resume-retains-old-connection-token.md) — A successful
  retry after an uncertain Resume can retain the old connection token. Resulting gateway or operation failures remain
  unverified after investigation.

- [`sandbox-status-diagnosis-bypasses-cleaning.md`](sandbox-status-diagnosis-bypasses-cleaning.md) — Accepted sandbox
  failure descriptions bypass normal cleaning and redaction before reaching clients. An actual provider description
  containing a credential remains unverified after investigation.

- [`retired-connection-leaves-suspend-progress-stuck.md`](retired-connection-leaves-suspend-progress-stuck.md) — A
  failed Suspend on a retired connection can leave the host showing Suspending with lifecycle controls disabled until a
  separate recovery action.

- [`resume-launch-test-records-session-after-assertion.md`](resume-launch-test-records-session-after-assertion.md) — The
  resume-launch test can create a session and then fail before saving its ID, causing per-test cleanup to skip that
  session.

- [`resume-test-retires-create-stub-before-client-settles.md`](resume-test-retires-create-stub-before-client-settles.md)
  — Failure cleanup can retire a create trap before the pending launcher settles, including the newer existing-host
  Resume test; actual extra allocation remains unmeasured.

- [`lifecycle-fixture-cleanup-stops-after-release-failure.md`](lifecycle-fixture-cleanup-stops-after-release-failure.md)
  — One fixture-release failure can skip the remaining browser-test cleanup and leave shared settings or resources
  behind.

- [`route-retirement-failure-skips-owned-session-cleanup.md`](route-retirement-failure-skips-owned-session-cleanup.md) —
  Fallible browser-handle cleanup, route retirement or optional session discovery can skip independent cleanup of
  already-owned test resources and preferences.

- [`snapshot-cleanup-spec-omits-unrecorded-builder-exception.md`](snapshot-cleanup-spec-omits-unrecorded-builder-exception.md)
  — The product says reusing any Farhelm-built snapshot is refused, but an unrecorded build snapshot can be accepted as
  a recipe source and later deleted by builder cleanup.

- [`recipe-leave-save-live-draft-mismatch-unresolved.md`](recipe-leave-save-live-draft-mismatch-unresolved.md) — Save
  before leaving a recipe reads the live draft. A supported input sequence saving data different from the visible draft
  remains unverified after investigation.

- [`unreadable-sandbox-ssh-key-reports-ready.md`](unreadable-sandbox-ssh-key-reports-ready.md) — A damaged or unreadable
  saved SSH key pair can still be presented as ready, postponing the failure until later sandbox operations.

- [`tunnel-helper-exit-may-block-flushing-stdout.md`](tunnel-helper-exit-may-block-flushing-stdout.md) — A tunnel helper
  may remain alive after its silence deadline if exit cleanup flushes buffered output into a full pipe. The exact
  ordering and shipped-library premise remain unverified after investigation.

- [`identity-adoption-overwrites-held-lifecycle-display.md`](identity-adoption-overwrites-held-lifecycle-display.md) —
  Concurrent installation adoption can overwrite the displayed Suspend or Resume progress while that operation still
  holds the connection.

- [`sandbox-diagnostic-json-shape-compatibility-unresolved.md`](sandbox-diagnostic-json-shape-compatibility-unresolved.md)
  — Structured sandbox failure descriptions would be rejected instead of displayed. Whether the configured production
  origin sends those shapes remains unverified after investigation.

- [`suspended-plan-test-erases-its-suspension-premise.md`](suspended-plan-test-erases-its-suspension-premise.md) — The
  suspended-host setup test changes installation paths and erases its own Suspended state before checking the guard. The
  resulting failing scheduler ordering remains unverified after investigation.

- [`tunnel-control-exit-can-open-new-ssh-connection.md`](tunnel-control-exit-can-open-new-ssh-connection.md) — Closing
  an old tunnel can start a new SSH connection if the control handshake fails after connecting. The deployed ordering
  and resulting delay remain unverified after investigation.

- [`snapshot-cleanup-races-new-host-registration.md`](snapshot-cleanup-races-new-host-registration.md) — Snapshot
  cleanup can remove a generated snapshot before a newly admitted host records its use, causing setup failure and
  rebuild cost.

- [`tunnel-fixture-assumes-simple-short-temporary-root.md`](tunnel-fixture-assumes-simple-short-temporary-root.md) — The
  tunnel transport tests can reject correct behavior when the temporary root needs path escaping or is too long for SSH
  sharing.

- [`mixed-setup-plans-escape-cache-bound.md`](mixed-setup-plans-escape-cache-bound.md) — A mixture of setup-plan kinds
  can accumulate retained plan entries beyond the intended cache bound.

- [`provider-error-cleaning-can-miss-transformed-secret.md`](provider-error-cleaning-can-miss-transformed-secret.md) —
  Error cleaning can miss a key after truncation or normalization changes its spelling. An actual provider response
  echoing that key remains unverified after investigation.

- [`sandbox-timestamp-wire-contract-unresolved.md`](sandbox-timestamp-wire-contract-unresolved.md) — Sandbox timestamp
  decoding accepts only a narrow numeric form and unit. Compatibility with the configured production origin remains
  unverified after investigation.

- [`sandbox-list-pagination-contract-unresolved.md`](sandbox-list-pagination-contract-unresolved.md) — Sandbox and
  snapshot listings use one response and ignore possible continuation cursors. Whether the configured production origin
  paginates these results remains unverified after investigation.

- [`redirect-regression-omits-location-header.md`](redirect-regression-omits-location-header.md) — The redirect
  regression does not supply a redirect destination, so it cannot prove that Farhelm refuses to follow one.

- [`cancelled-ssh-capture-can-retain-local-readers.md`](cancelled-ssh-capture-can-retain-local-readers.md) — Cancelling
  an SSH capture may leave local helpers or pipe readers alive beyond the caller. Their actual surviving lifetime
  remains unverified after investigation.

- [`restart-with-does-not-resume-suspended-host.md`](restart-with-does-not-resume-suspended-host.md) — Restart with on a
  suspended sandbox does not perform the resume step, so the requested restart fails instead of continuing.

- [`long-host-name-progress-reachability-unresolved.md`](long-host-name-progress-reachability-unresolved.md) — A long
  host name may push progress content beyond its available layout. Actual supported native geometry remains unverified
  after investigation.

- [`ssh-key-tooltip-test-lacks-loading-witness.md`](ssh-key-tooltip-test-lacks-loading-witness.md) — The SSH-key tooltip
  regression can make its assertion without establishing the key-loading state it intends to test.

- [`package-name-trailing-minus-requests-removal.md`](package-name-trailing-minus-requests-removal.md) — A recipe
  package ending in a minus sign is passed to apt with its removal meaning instead of being rejected or treated as a
  literal package name.

- [`recipe-destination-refuses-absolute-dns-name.md`](recipe-destination-refuses-absolute-dns-name.md) — A valid final
  DNS dot is refused in recipe tunnel destinations and agent endpoint hostnames.

- [`privileged-sandbox-tunnel-port-compatibility-unresolved.md`](privileged-sandbox-tunnel-port-compatibility-unresolved.md)
  — Recipe tunnels accept privileged sandbox listener ports that the gateway may refuse. Actual gateway enforcement
  remains unverified after investigation.

- [`tunnel-remove-uses-shifting-row-index.md`](tunnel-remove-uses-shifting-row-index.md) — Remove clicks from one
  rendered tunnel list can use shifted indices and remove a different unsaved row.

- [`queued-recipe-edits-after-save-unresolved.md`](queued-recipe-edits-after-save-unresolved.md) — Form events arriving
  after Save captures its draft may disappear when the editor closes. A complete native producer remains unverified
  after investigation.

- [`removed-tunnel-retains-validation-error.md`](removed-tunnel-retains-validation-error.md) — Removing an invalid
  tunnel entry can leave its obsolete validation error displayed against the remaining draft.

- [`old-recipe-admission-builds-same-name-replacement.md`](old-recipe-admission-builds-same-name-replacement.md) — A
  previously admitted build can act on a later recipe recreated under the same name instead of the definition that was
  originally admitted.

- [`failed-recipe-refresh-test-lacks-baseline-row.md`](failed-recipe-refresh-test-lacks-baseline-row.md) — The
  failed-refresh regression does not establish the prior recipe row needed to prove that a failed refresh preserves it.

- [`late-snapshot-visibility-escapes-builder-cleanup.md`](late-snapshot-visibility-escapes-builder-cleanup.md) — A
  snapshot becoming visible only after cleanup's last listing could remain after builder removal. The provider's
  relevant visibility guarantees remain unverified after investigation.

- [`detached-recipe-write-crosses-recreated-generation.md`](detached-recipe-write-crosses-recreated-generation.md) — A
  detached database write can outlive its recipe generation and update a later recipe recreated with the same name.

- [`package-repair-does-not-wait-for-dpkg-lock.md`](package-repair-does-not-wait-for-dpkg-lock.md) — The repair step
  does not share the package installer's lock-wait policy and could fail under contention. A supported concurrent
  package-manager trigger remains unverified after investigation.

- [`apt-regex-fallback-can-accept-package-typo.md`](apt-regex-fallback-can-accept-package-typo.md) — A misspelled
  package may be accepted through apt's pattern fallback instead of failing plainly. The shipped distribution behavior
  remains unverified after investigation.

- [`snapshot-state-races-build-admission.md`](snapshot-state-races-build-admission.md) — Adding a host as a build
  finishes can select a just-deleted snapshot or start an unnecessary charged build.

- [`packages-build-result-retired-under-changed-image-source.md`](packages-build-result-retired-under-changed-image-source.md)
  — A build that captured a Packages source can have its result retired after the recipe's current source changes to
  Image.

- [`orphan-cleanup-start-does-not-invalidate-feed.md`](orphan-cleanup-start-does-not-invalidate-feed.md) — Starting
  orphan cleanup does not notify the live feed, so other views may miss its new in-progress state.

- [`orphan-test-setup-precedes-cleanup-guard.md`](orphan-test-setup-precedes-cleanup-guard.md) — Failure while preparing
  orphan-test recipes can leave earlier acquired fixture state outside the test's cleanup guard.

- [`orphan-test-cleanup-stops-after-first-error.md`](orphan-test-cleanup-stops-after-first-error.md) — One
  orphan-fixture cleanup failure can skip cleanup of its remaining owned resources.

- [`orphan-fixture-edits-shared-model-without-lock.md`](orphan-fixture-edits-shared-model-without-lock.md) — The orphan
  browser fixture changes its shared fake model outside the synchronization used by its other readers and writers.

- [`orphan-fixture-cleanup-ignores-http-refusals.md`](orphan-fixture-cleanup-ignores-http-refusals.md) — Fixture cleanup
  can treat a refused HTTP operation as completed because it never checks the response status.

- [`orphan-confirmation-opens-outside-visible-scroll.md`](orphan-confirmation-opens-outside-visible-scroll.md) — With a
  long orphan list, the new confirmation can open outside the visible area without scrolling or moving focus to it; its
  controls remain reachable by scrolling.

- [`account-test-does-not-refresh-healthy-orphan-list.md`](account-test-does-not-refresh-healthy-orphan-list.md) — After
  another window changes the account key, Test alone does not refresh a healthy orphan list. Whether the specification
  requires that refresh remains unverified after investigation.

- [`neighbor-status-redraw-cancels-stable-url-click.md`](neighbor-status-redraw-cancels-stable-url-click.md) — A status
  change elsewhere can cancel a click on an unchanged terminal URL.

- [`automatic-update-opt-out-can-roll-back.md`](automatic-update-opt-out-can-roll-back.md) — A power loss can roll back
  opting out of automatic updates and allow later unattended updates. Whether the general preference-loss acceptance
  covers this authorization remains unresolved.

- [`terminal-attachment-trace-retains-raw-session-id.md`](terminal-attachment-trace-retains-raw-session-id.md) —
  Terminal attachment diagnostics retain a raw peer-supplied session identifier instead of the promised cleaned
  presentation copy.

- [`escaped-token-path-display-collision-unresolved.md`](escaped-token-path-display-collision-unresolved.md) — Escaped
  and literal token-file paths can look identical in approval text. A supported harmful pair of real files remains
  unverified after investigation.

- [`attention-update-accessible-name-omits-target.md`](attention-update-accessible-name-omits-target.md) —
  Attention-list Update buttons omit their host and version from the accessible name, leaving identical actions
  difficult to distinguish.

- [`attention-update-removes-focused-control.md`](attention-update-removes-focused-control.md) — Starting Update from
  the attention list removes the focused action without handing keyboard focus to a surviving control.

- [`pinned-session-menu-placement-uses-outdated-item-count.md`](pinned-session-menu-placement-uses-outdated-item-count.md)
  — Pinned-session menu placement still budgets for eight actions after the menu grew to ten, causing unnecessary
  scrolling even when the full menu could fit above.

- [`drag-preview-neighbor-diverges-from-drop-unresolved.md`](drag-preview-neighbor-diverges-from-drop-unresolved.md) —
  The drag preview and final drop can use different neighboring rows. A complete supported native ordering producing the
  mismatch remains unverified after investigation.

- [`pointer-capture-drop-crosses-new-modal-unresolved.md`](pointer-capture-drop-crosses-new-modal-unresolved.md) — A
  captured drag may finish after a new modal appears and still reorder rows beneath it. The native event sequence
  remains unverified after investigation.

- [`primary-row-drag-continues-through-middle-button.md`](primary-row-drag-continues-through-middle-button.md) —
  Releasing the primary button during a middle-button chord leaves a row drag active; releasing the other button can
  then commit the move.

- [`macos-control-click-suppresses-row-marking.md`](macos-control-click-suppresses-row-marking.md) — On the inspected
  macOS Chromium route, Control-click is consumed as a context-menu gesture instead of marking the session.

- [`marked-count-reflow-leaves-popup-at-old-anchor.md`](marked-count-reflow-leaves-popup-at-old-anchor.md) — A changed
  marked count may move the menu button onto another toolbar line while its popup stays put. The necessary rendered
  geometry remains unverified after investigation.

- [`bulk-screenshot-test-expects-wrong-local-alias.md`](bulk-screenshot-test-expects-wrong-local-alias.md) — The
  bulk-action screenshot regression expects a local-host alias that its own fixture does not provide.

- [`resume-preview-readiness-witness.md`](resume-preview-readiness-witness.md) — The regression test never forces a
  ready checkout preview while the form still displays Connecting. A regression at that boundary can consume the single
  accepted launch press without eventually starting a session.

- [`suspended-launch-missing-identity.md`](suspended-launch-missing-identity.md) — After interrupted initial sandbox
  setup, a suspended host can appear as an available fresh-checkout target, yet Launch does nothing and shows no
  refusal. The new pre-resume identity requirement returns before even resuming the sandbox.

- [`version-agreement-lifetime-doc.md`](version-agreement-lifetime-doc.md) — The newly documented agreement between a
  program's terminal-version environment and a terminal query needs a lifetime qualification. A surviving program keeps
  its launch environment after a Farhelm update, while a new query after adoption receives the current emulator's
  version. The established consequence is documentation ambiguity only; no application failure is assumed.

- [`capture-failed-stop-erases-recovery.md`](capture-failed-stop-erases-recovery.md) — When the real-agent
  inspection/capture tool cannot confirm its terminal host stopped, it hides that failure and still removes the run
  directory needed to retry the supported stop command. Continued agent execution after the deletion has not been
  demonstrated.

- [`stopped-reader-output-witness.md`](stopped-reader-output-witness.md) — The browser flood test checks retained output
  precisely, but a pass does not establish that the flood ran while the page stopped consuming frames. It can therefore
  leave the intended slow-viewer case unproved while appearing to cover it.

- [`container-redraw-discards-hover.md`](container-redraw-discards-hover.md) — A container terminal can lose a completed
  file hover when other cells on that row redraw. One redraw can cause temporary rechecking. Another relevant row change
  during replacement admission can leave the link absent while the pointer stays at rest; movement or completion of a
  later gesture can retry. A first click started without a hovered link does not activate the file.

- [`delete-reader-notice-bypass.md`](delete-reader-notice-bypass.md) — Input arriving while a session is being deleted
  can make its terminal detach with an input-failure notice instead of the Delete's notice or its cleanup-failure
  explanation. The landed notice fix misses the reader's direct failure path.

- [`grid-row-is-not-program-line.md`](grid-row-is-not-program-line.md) — The new terminal transcript can make a
  partially rendered program line appear complete to a test. Agent checks can then reject correct behavior; one
  direct-signal caller could instead use a process-number prefix and terminate unrelated work. The exact short-line
  split, unrelated signal target and regression attribution remain unverified after investigation.

- [`grid-text-omits-blank-columns.md`](grid-text-omits-blank-columns.md) — The new test helper can turn visibly
  separated terminal text into a different string. A test searching that transcript for the displayed text may then time
  out or match text that was never contiguous on screen. No currently failing assertion was identified.

- [`reboot-concurrency-early-release.md`](reboot-concurrency-early-release.md) — The reboot tests do not keep their
  integration permit through successor retirement and terminal cleanup. Under the explicit in-process/libtest use of
  that permit, another fixture can be admitted during the preceding fixture's teardown. No permanent process leakage is
  established.

- [`stop-annotation-exit-witness.md`](stop-annotation-exit-witness.md) — The test can reject a successful Stop if its
  immediate listing runs before the host runtime processes the exit record. This is a conditional source-confirmed race,
  not an observed flake or frequency claim.

- [`kicked-input-test-local-filter.md`](kicked-input-test-local-filter.md) — A takeover regression assertion lets the
  client discard the displaced viewer's attempted keystrokes before the supervisor sees them. Passing therefore does not
  establish the supervisor's input ownership enforcement.

- [`argv-grid-wrap-witness.md`](argv-grid-wrap-witness.md) — The structured-launch test can decode arguments the fake
  process never received if attachment catches a wrapped hex witness partway through a continuation row. That can reject
  a correct launch. The event-shape defect is supported by source; the exact scheduling cut remains unverified after
  investigation.

- [`final-stack-drop-claim-window.md`](final-stack-drop-claim-window.md) — A very narrow setup race can make the
  converted restart-gap tab test fail even when tab opening behaves correctly. Ordinary overlap between the test's
  supervisors is not this race: both share the same process's live ownership claim. The possible consequence is a
  maintainer-test false failure only.

- [`host-flood-stall-window-claim.md`](host-flood-stall-window-claim.md) — A migrated test now claims to cross the new
  terminal's stall timeout, but observes only eight of its configured sixty seconds. Its inherited flood witness also
  does not establish output pressure. This is a new coverage-description error, with no observed product stall.

- [`fixture-private-ipv6-url-guard.md`](fixture-private-ipv6-url-guard.md) — The fixture checks can approve a screen
  containing a private IPv6 proxy URL. This is a guard omission for valid URL/configuration text, not evidence that a
  private address or credential was published.

- [`capture-startup-metadata-cleanup.md`](capture-startup-metadata-cleanup.md) — A screen-capture run can fail after
  launching its agent but before returning the run handle, leaving the detached capture process and its terminal host
  running without the caller cleaning them up.

- [`capture-local-program-path.md`](capture-local-program-path.md) — Naming a Farhelm binary in the current directory
  with --farhelm ./farhelm can fail to launch it or silently run another Farhelm found on PATH, so screen inspection can
  use a different build than requested.

- [`codex-auth-object-depth-contract.md`](codex-auth-object-depth-contract.md) — The declared size/object-only Codex
  login validation contract omits a parser nesting bound. No affected vendor-produced login file or outage is
  established.

- [`credential-cancel-erases-delivery-report.md`](credential-cancel-erases-delivery-report.md) — Cancelling an unrelated
  credential draft erases the previous save's inline failed-delivery report; the host's separate warning remains.

- [`credential-catalog-success-starvation.md`](credential-catalog-success-starvation.md) — Unrelated fleet notices
  arriving faster than successful catalog replies can keep Credentials loading or stale. A quiet interval permits
  recovery; frequency is unmeasured.

- [`release-shot-account-readiness.md`](release-shot-account-readiness.md) — A release capture's enabled-input check
  accepts an unloaded or failed account form. No existing published image is established to be wrong.

- [`recipe-cleanup-navigation.md`](recipe-cleanup-navigation.md) — Navigation rejection skips a browser test's owned
  recipe and credential deletions, potentially contaminating later tests on its private stack.

- [`tunnel-dns-outlives-dial-admission.md`](tunnel-dns-outlives-dial-admission.md) — Unverified after investigation:
  stalled hostname resolution can outlive recipe tunnel timeouts and admission limits, potentially accumulating work or
  delaying helm shutdown. An affected native resolver execution has not been demonstrated.

- [`container-path-publication-keeps-stale-cut.md`](container-path-publication-keeps-stale-cut.md) — Unverified after
  investigation: a container terminal can offer an obsolete joined filename if neighboring output changes while a
  separate hyperlink lookup is pending. The download path independently checks recognition and refuses the changed
  target.

- [`capture-scrubber-misses-multiple-at-signs.md`](capture-scrubber-misses-multiple-at-signs.md) — The capture scrubber
  and its immediate check both miss a local proxy URL with another raw at sign in its password. The separate Rust check
  catches it when run; actual vendor display and publication of such a URL are unestablished.

- [`fixture-url-guard-consumes-adjacent-at-sign.md`](fixture-url-guard-consumes-adjacent-at-sign.md) — The independent
  fixture privacy check can miss a local proxy URL followed by comma-separated text containing an at sign. Automatic
  capture scrubs this exact input, but manually authored fixtures can pass the weakened guard.

- [`landing-tests-import-unregistered-module.md`](landing-tests-import-unregistered-module.md) — The documented
  landing-tool validation command can crash during import before testing any safeguards, because its dynamically loaded
  module is absent from Python's module registry.

- [`outer-recorder-deletes-nested-failure-evidence.md`](outer-recorder-deletes-nested-failure-evidence.md) — Recording a
  script that starts another recorder can delete the inner failed run's evidence during outer temporary-directory
  cleanup. The release invocation that explicitly forwards both output roots is outside this route.

- [`rust-sandbox-runner-passes-empty-selection.md`](rust-sandbox-runner-passes-empty-selection.md) — The Rust sandbox
  runner can announce all shards passed while running no tests, including with zero or nonnumeric shard counts and an
  empty overall filter selection.

- [`rust-sandbox-pack-ignores-cargo-target-dir.md`](rust-sandbox-pack-ignores-cargo-target-dir.md) — A checkout built
  only into a shared or external Cargo target directory cannot package its Rust sandbox tests: the runner still requires
  its fixture binary under the default target path.

- [`rust-sandbox-staging-misquotes-checkout-path.md`](rust-sandbox-staging-misquotes-checkout-path.md) — The Rust
  sandbox runner cannot stage an ordinary checkout whose absolute path contains spaces, because nested remote shell
  commands interpolate that path without quoting it.

- [`mac-uninstall-test-lacks-live-host-premise.md`](mac-uninstall-test-lacks-live-host-premise.md) — The native Mac
  uninstall test can pass without establishing that a retained agent terminal host was still alive before uninstall.

- [`mac-fixture-cleanup-stop-budget.md`](mac-fixture-cleanup-stop-budget.md) — Mac test cleanup uses a shorter deadline
  than production permits for the same stop script; no actual timeout was observed.

- [`mac-setup-confirmation-misses-installed-app.md`](mac-setup-confirmation-misses-installed-app.md) — An old Mac setup
  confirmation can install the remote supervisor after Farhelm.app is installed; the pre-confirmation exception's scope
  remains uncertain.

- [`mac-uninstall-unresolved-loaded-plist.md`](mac-uninstall-unresolved-loaded-plist.md) — A removed loaded plist can
  bypass uninstall's service-path comparison; the nonstandard-layout exception's applicability remains uncertain.

- [`mac-rosetta-ssh-classification.md`](mac-rosetta-ssh-classification.md) — Unverified after investigation: translated
  SSH inspection may refuse Apple-silicon hardware; the exact supported command chain remains unestablished.

- [`mac-host-start-same-build-contract.md`](mac-host-start-same-build-contract.md) — Remote Mac Update contradicts the
  declared same-build startup invariant; no incompatible startup data or runtime failure was demonstrated.

- [`devbuild-rust-llvm-layout-unverified.md`](devbuild-rust-llvm-layout-unverified.md) — Unverified after investigation:
  local Mac devbuild may need the release LLVM repair; the exact pinned Rust artifact's loader layout remains missing.

- [`centos-rustup-missing-pin.md`](centos-rustup-missing-pin.md) — Standalone CentOS validation cannot install its
  target with Rustup 1.28.0 when the pinned compiler is absent; current Rustup auto-install avoids this route.

- [`devbuild-rustup-active-name.md`](devbuild-rustup-active-name.md) — Mac devbuild misparses Rustup 1.28.0's multiline
  active-toolchain output and aborts preflight even with the compiler installed; current one-line output is unaffected.

- [`stopped-host-update-floor-unresolved.md`](stopped-host-update-floor-unresolved.md) — A stopped host running Farhelm
  below the minimum update version can still be updated without checking its installed version. Whether the
  minimum-version rule is intended to cover stopped supervisors remains unresolved.

- [`repeated-cleanup-failure-test-misses-recurrence.md`](repeated-cleanup-failure-test-misses-recurrence.md) — The
  regression can pass without observing the cleanup failure it repeats, leaving recurrence handling unprotected.

- [`cleanup-retry-omits-recovery-step-times.md`](cleanup-retry-omits-recovery-step-times.md) — After a cleanup failure,
  Retry can report zero time for recovery steps that actually ran, weakening its diagnostic timing report.

- [`large-notice-history-exceeds-mark-budget.md`](large-notice-history-exceeds-mark-budget.md) — Extremely large
  notification histories may become impossible to acknowledge or clear in the app. Supported lifetime size and
  successful rendering at that scale remain unverified after investigation.

- [`trash-read-eligibility-can-remain-cached.md`](trash-read-eligibility-can-remain-cached.md) — A queued Trash reader
  may start an extra listing while cleanup is pending. The required native event ordering remains unverified after
  investigation; no destructive authority or receipt overwrite is established.

- [`overdue-plan-protection-ages-out.md`](overdue-plan-protection-ages-out.md) — The plan picker can forget that a
  repeatedly skipped plan is overdue when older claims leave its bounded history.

- [`plan-pass-count-includes-precreation-claims.md`](plan-pass-count-includes-precreation-claims.md) — A plan with a
  long edit history can be treated as overdue using claims made before that plan existed.

- [`plan-pass-pagination-follows-moving-main.md`](plan-pass-pagination-follows-moving-main.md) — The plan picker can
  count an in-flight claim as a skipped pass when main changes while history pages are read.

- [`dependency-block-counts-held-plan-time.md`](dependency-block-counts-held-plan-time.md) — A plan temporarily blocked
  by an abandoned dependency can accumulate overdue passes from time it was explicitly on hold.

- [`plain-url-scan-repeats-history-decoding.md`](plain-url-scan-repeats-history-decoding.md) — Hovering a URL in
  retained terminal history repeats full-row decoding for its characters. Added work is confirmed; material pointer or
  UI latency remains unverified after investigation.

- [`approval-copy-test-assumes-clipboard-object.md`](approval-copy-test-assumes-clipboard-object.md) — The Copy ID test
  can fail before its assertion if the engine provides no clipboard object. Whether the supported browser configuration
  omits it remains unverified after investigation.

- [`progress-hover-keeps-short-host-name.md`](progress-hover-keeps-short-host-name.md) — Update progress repeats a
  shortened host name instead of revealing its full display name, so similarly named hosts remain indistinguishable
  there.

- [`orphan-cleanup-promises-unheld-provisioning-lock.md`](orphan-cleanup-promises-unheld-provisioning-lock.md) — The
  implementation specification promises that orphan cleanup checks ownership under the host provisioning lock, but the
  endpoint does not hold that lock. No supported destructive adoption race was established.

- [`sandbox-lifecycle-questions-lack-opening-guards.md`](sandbox-lifecycle-questions-lack-opening-guards.md) — Sandbox
  lifecycle questions lack the required opening-token and operation-claim ownership. Same-dialog Cancel and Submit
  before redraw is product-accepted when its destructive guards hold; a stronger obsolete-opening loss route remains
  unverified.

- [`sandbox-create-reply-loses-new-recipe-default.md`](sandbox-create-reply-loses-new-recipe-default.md) — Immediately
  after opening a newly launched sandbox session, New and quick-switcher creation visibly default to Local instead of
  that sandbox's recipe. Reselecting the session repairs the default.

- [`sandbox-launch-question-lacks-opening-token.md`](sandbox-launch-question-lacks-opening-token.md) — The
  sandbox-launch question uses unguarded answers instead of the required opening token. Same-burst Cancel and Submit is
  product-accepted; a stronger obsolete-question consequence remains unverified after investigation.

- [`sandbox-checkout-hides-required-folder-setting.md`](sandbox-checkout-hides-required-folder-setting.md) — A new
  sandbox checkout can be offered without exposing its missing global checkout-folder setting, then fail before billing
  with a Settings remedy.

- [`clone-read-overwrites-manual-recipe-choice.md`](clone-read-overwrites-manual-recipe-choice.md) — While Clone waits
  for its source-host read, choosing another enabled recipe does not take ownership of the draft; the late read can
  replace that choice.

- [`sandbox-launch-sends-hidden-agent-prompt.md`](sandbox-launch-sends-hidden-agent-prompt.md) — Switching from an agent
  with Initial prompt to Goose or Pi can hide a retained prompt that still blocks sandbox launch. The backend refuses
  before billing.

- [`sandbox-launch-bypasses-repository-search-refusal.md`](sandbox-launch-bypasses-repository-search-refusal.md) —
  Sandbox Launch can proceed with the previous repository or home destination while the form says Select a repository
  for the unfinished search. With creation questions skipped, it can allocate a billed sandbox.

- [`sandbox-row-menu-owns-transient-lifecycle-state.md`](sandbox-row-menu-owns-transient-lifecycle-state.md) — Closing
  the row menu can discard a sandbox lifecycle question, its local waiter or its error. The backend operation continues
  independently, so this does not establish abandoned Stop-and-suspend work.

- [`bulk-sandbox-delete-drops-eligible-peer.md`](bulk-sandbox-delete-drops-eligible-peer.md) — If the first marked
  sandbox session retains a hidden question, bulk Delete can skip an eligible peer, leave the sandbox untouched, clear
  both marks and explain only the first refusal.

- [`frozen-sandbox-retains-header-lifecycle-answers.md`](frozen-sandbox-retains-header-lifecycle-answers.md) — Retained
  header Restart and Replace questions, and an interrupted-session card during refresh, can keep enabled answers for a
  frozen sandbox. The backend refuses those operations.

- [`sandbox-launch-skips-remembered-agent-choices.md`](sandbox-launch-skips-remembered-agent-choices.md) — A successful
  sandbox launch does not persist the user's permission or wrapper choice. The initiating page's local mirror hides the
  omission until a reload.

- [`suspended-sandbox-keeps-row-confirmations-enabled.md`](suspended-sandbox-keeps-row-confirmations-enabled.md) — A
  Stop or Replace question opened before suspension can remain answerable after the normal menu actions disable.
  Accepting it dismisses the question and sends a request the backend refuses.

- [`sandbox-outage-card-ignores-host-refresh-failure.md`](sandbox-outage-card-ignores-host-refresh-failure.md) — After
  Resume, a failed host-list refresh can leave a recovered terminal hidden behind an unqualified Session suspended card,
  with lifecycle controls disabled until a successful refresh.

- [`retained-restart-hides-reboot-card.md`](retained-restart-hides-reboot-card.md) — A reboot before any post-Restart
  listing sees the session running or stale can leave its recovery card hidden. Header recovery remains available; a
  landed-plan acceptance conflicts with the product specification.

- [`moved-on-body-focus.md`](moved-on-body-focus.md) — A delayed Replace YOLO question can focus and scroll Cancel after
  the user has clicked elsewhere, if that click left focus on empty page space. Farhelm already knows the user pressed
  something and still moves focus.

- [`replace-selection-focus-contract.md`](replace-selection-focus-contract.md) — The specifications give conflicting
  answers about opening a replacement after the user types while its source remains selected. Current code follows the
  explicit Replace selection rule. Whether that violates the new focus policy remains unverified after investigation and
  needs a product-contract clarification.

- [`failed-launch-pins-template-host.md`](failed-launch-pins-template-host.md) — After an ordinary launch fails, Save as
  template prechecks its inherited host even when the user never picked that host. Saving without clearing the visible
  check can make a template installation-specific unexpectedly.

- [`cache-lock-repair-child-lifetime.md`](cache-lock-repair-child-lifetime.md) — Repository-cache repair assumes Git
  cannot outlive its outer-lock owner; a real collision or damage remains unverified after investigation.

- [`pending-stop-header-width.md`](pending-stop-header-width.md) — Pending Stopping feedback may exceed the compact
  header's 650px budget with a tunnel pill; actual geometry remains unverified after investigation.

- [`replace-cleanup-snapshots-owned-ids-before-drain.md`](replace-cleanup-snapshots-owned-ids-before-drain.md) —
  Replace-test cleanup constructs owned-session deletion steps before draining a callback that can add the replacement
  ID.

- [`replace-question-test-starts-with-restart-unavailable.md`](replace-question-test-starts-with-restart-unavailable.md)
  — The Replace-question exclusion test asserts Restart disabled on a source that was already not resumable.

- [`stop-progress-omits-row-movement.md`](stop-progress-omits-row-movement.md) — Stop progress can wrap an earlier
  noncompact row without updating a later row's open menu or notification position; exact native geometry is unmeasured.

- [`replace-question-omits-prefix-movement.md`](replace-question-omits-prefix-movement.md) — A background Replace
  question can move session rows without updating another row's open menu position; runtime geometry was not measured.

- [`deleted-source-retains-replace-question.md`](deleted-source-retains-replace-question.md) — A deleted session's
  retained Replace permission question survives authoritative absence, contrary to the final subject-gone rule; an empty
  fleet can hide it until another session appears.

- [`older-pin-success-clears-newer-stop-error.md`](older-pin-success-clears-newer-stop-error.md) — A delayed bulk Pin
  success can clear a newer Stop failure or partial-cleanup notice from both the session row and open view.

- [`trailing-retired-container-delete-treats-podman-lookup-none-as-absence.md`](trailing-retired-container-delete-treats-podman-lookup-none-as-absence.md)
  — Unverified after investigation: lookup failure exceeds the literal missing-Podman exception. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-wake-budget-inflight-busy-resume.md`](trailing-wake-budget-inflight-busy-resume.md) — Wake contract omits
  the longer in-flight Resume wait. Abbreviated collection; confidence, proposed dismissals and unresolved consequences
  are preserved in the entry.

- [`trailing-testing-overview-falsely-generalizes-built-farhelm-host-program.md`](trailing-testing-overview-falsely-generalizes-built-farhelm-host-program.md)
  — Testing overview names the wrong executable for supervisor unit-test hosts. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-agent-launch-specs-disagree-receipt-relative-versus-post-allow-timeout.md`](trailing-environment-agent-launch-specs-disagree-receipt-relative-versus-post-allow-timeout.md)
  — environment launch contracts describe different effective waits. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-suspended-source-clone-rules-distinguish-agent-authority-from-gui-prefill.md`](trailing-suspended-source-clone-rules-distinguish-agent-authority-from-gui-prefill.md)
  — suspended-source Clone rules distinguish agent authority from GUI prefill. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-removed-terminal-selector-stale-migration-spec-condition.md`](trailing-removed-terminal-selector-stale-migration-spec-condition.md)
  — Remove the obsolete condition from the migration contract. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-old-terminal-startup-best-effort-duplicate-agent-residual.md`](trailing-old-terminal-startup-best-effort-duplicate-agent-residual.md)
  — Fresh Restart refutes ordinary duplicate-agent inference, not misleading stop consent. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-spec-terminal-switch-point-names-deleted-terminal-type.md`](trailing-spec-terminal-switch-point-names-deleted-terminal-type.md)
  — Rewritten terminal contract points to the deleted Terminal type. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-old-terminal-create-refusal-qualified-rollback-wording.md`](trailing-old-terminal-create-refusal-qualified-rollback-wording.md)
  — Rollback phrase references a policy that already retains allocated checkouts. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-launch-excerpt-inherited-history-provenance.md`](trailing-launch-excerpt-inherited-history-provenance.md) —
  Pre-existing excerpt contamination; in-range implementation spec repeats a false blank-screen proof. Abbreviated
  collection; confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-spec-shim-container-execution-branch-retired.md`](trailing-spec-shim-container-execution-branch-retired.md)
  — Launch contract still describes the retired shim container branch. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-shift-enter-unconditional-pty-write-guarantee.md`](trailing-shift-enter-unconditional-pty-write-guarantee.md)
  — Qualify the grid Shift+Enter single-write guarantee. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-grid-input-font-inline-owner-rationale.md`](trailing-grid-input-font-inline-owner-rationale.md) — The new
  font-rule explanation assigns the inline font to the wrong element. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-uninstall-accepted-outlived-private-tmux-residual.md`](trailing-uninstall-accepted-outlived-private-tmux-residual.md)
  — Uninstall can leave an earlier private tmux server running. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-spec-report-discard-on-terminal-disappearance.md`](trailing-spec-report-discard-on-terminal-disappearance.md)
  — Report-discard contract does not match gone host terminals. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-delete-recorded-tab-scope-kept-derived-under-unusable-manager.md`](trailing-delete-recorded-tab-scope-kept-derived-under-unusable-manager.md)
  — Scoped-tab Delete policy gap predates the whole review range. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-http-golden-defaulted-trash-epoch.md`](trailing-http-golden-defaulted-trash-epoch.md) — The new Trash epoch
  golden fixture cannot detect a defaulted decoder regression. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-http-golden-omits-populated-environment.md`](trailing-http-golden-omits-populated-environment.md) — The new
  environment host JSON is omitted from every golden contract row. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-agent-launch-placement-docs-obsolete-caller-contract.md`](trailing-agent-launch-placement-docs-obsolete-caller-contract.md)
  — Placement documentation misstates ordinary-create inputs and links a missing helper. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-agent-environment-guide-key-and-wait-exceptions.md`](trailing-agent-environment-guide-key-and-wait-exceptions.md)
  — Agent guide explicitly qualifies key and wait advice for environment launches. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-test-responders-heartbeat-control-decode.md`](trailing-environment-test-responders-heartbeat-control-decode.md)
  — Environment test responders panic on valid heartbeat frames. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-intermediate-agent-delete-calls-podman-container-sandbox.md`](trailing-intermediate-agent-delete-calls-podman-container-sandbox.md)
  — Intermediate agent refusal calls a Podman container a sandbox. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-recipe-template-management-refusal-denies-supported-agent-launch-use.md`](trailing-recipe-template-management-refusal-denies-supported-agent-launch-use.md)
  — Recipe-template refusals still claim agent use is GUI-only. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-generic-lost-reply-remedy-does-not-prescribe-an-unsupported-environment-retry-key.md`](trailing-generic-lost-reply-remedy-does-not-prescribe-an-unsupported-environment-retry-key.md)
  — Generic lost-reply remedy does not prescribe an unsupported environment retry key. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-agent-environment-missing-local-ssh-key-preflight.md`](trailing-agent-environment-missing-local-ssh-key-preflight.md)
  — Sandbox agent launches ask for approval before checking the local SSH key. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-history-admits-agent-origin.md`](trailing-environment-history-admits-agent-origin.md) — Agent
  environment launches enter GUI place-default and recent-history bookkeeping. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-wait-comment-promises-a-relay-ordering-the-budgets-do-not-guarantee.md`](trailing-environment-wait-comment-promises-a-relay-ordering-the-budgets-do-not-guarantee.md)
  — Environment wait comment promises a relay ordering the budgets do not guarantee. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-create-result-drops-operation-claim-remints-current.md`](trailing-environment-create-result-drops-operation-claim-remints-current.md)
  — Environment create replies can mark an earlier connection result current. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-module-overview-does-not-independently-require-every-clone-to-name-a-host.md`](trailing-module-overview-does-not-independently-require-every-clone-to-name-a-host.md)
  — Module overview does not independently require every clone to name a host. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-nonfinal-environment-delete-cache-survivor-oracle.md`](trailing-nonfinal-environment-delete-cache-survivor-oracle.md)
  — Non-final Delete test accepts clearing every cached session. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-sealed-delete-comment-hold-versus-seal-record.md`](trailing-sealed-delete-comment-hold-versus-seal-record.md)
  — Deletion test comment wrongly forbids releasing the failed host's connection hold. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-stopped-idle-local-arc-blocks-direct-manager-drop.md`](trailing-stopped-idle-local-arc-blocks-direct-manager-drop.md)
  — Stopped actors retain strong manager references across idle waits. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-shared-suspended-refusal-retains-tensorlake-noun.md`](trailing-shared-suspended-refusal-retains-tensorlake-noun.md)
  — Paused-container refusals incorrectly identify a Tensorlake sandbox. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-create-history-current-claim-comment-contradiction.md`](trailing-create-history-current-claim-comment-contradiction.md)
  — Create routing comment promises history can bypass connection-claim checks. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-stopped-episode-test-does-not-witness-settled-unavailable-cycle.md`](trailing-stopped-episode-test-does-not-witness-settled-unavailable-cycle.md)
  — Unanswered-status regression test can skip the settled failure boundary. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-host-phase-vocabulary-test-omits-stopped.md`](trailing-host-phase-vocabulary-test-omits-stopped.md) —
  Phase-consistency test omits new Stopped and Deleting states despite its every-variant promise. Abbreviated
  collection; confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-shared-suspended-view-doc-retains-tensorlake-only-contract.md`](trailing-shared-suspended-view-doc-retains-tensorlake-only-contract.md)
  — Suspended view documentation retains the known Tensorlake-only vocabulary. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-dial-documentation-incorrectly-exempts-podman-from-the-provider-status-gate.md`](trailing-dial-documentation-incorrectly-exempts-podman-from-the-provider-status-gate.md)
  — Dial documentation incorrectly exempts Podman from the provider status gate. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ordinary-tensorlake-add-keeps-source-mutex-through-ssh-readiness.md`](trailing-ordinary-tensorlake-add-keeps-source-mutex-through-ssh-readiness.md)
  — Ordinary Add retains source exclusion during post-Running SSH readiness. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-wake-browser-stale-suspension-oracle-unattributed-host-read-count.md`](trailing-wake-browser-stale-suspension-oracle-unattributed-host-read-count.md)
  — The stale-suspension browser test counts unrelated host reads as its readiness oracle. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ordinary-host-remove-bypasses-sealed-attempt-outcome.md`](trailing-ordinary-host-remove-bypasses-sealed-attempt-outcome.md)
  — Whole-host Delete does not record sealed deletion failures. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-normal-removal-parks-or-cancels-the-listing-before-clearing-its-seal.md`](trailing-normal-removal-parks-or-cancels-the-listing-before-clearing-its-seal.md)
  — Normal removal parks or cancels the listing before clearing its seal. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-seal-finisher-stale-pending-snapshot-admission.md`](trailing-seal-finisher-stale-pending-snapshot-admission.md)
  — Finisher can automatically retry a destruction that already failed. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-delete-freshness-test-docstring-connected-versus-suspended.md`](trailing-delete-freshness-test-docstring-connected-versus-suspended.md)
  — Delete freshness test describes the opposite retained host state. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-bulk-delete-test-row-disappearance-comment-false-oracle.md`](trailing-bulk-delete-test-row-disappearance-comment-false-oracle.md)
  — Bulk Delete test comment promises a reply-completion oracle it does not have. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-browser-cleanup-drains-real-api-route-before-request-client-lifetime.md`](trailing-browser-cleanup-drains-real-api-route-before-request-client-lifetime.md)
  — Draining mocked Deletes can outlive the client needed to clean independent sessions. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-cancelled-trash-observation-test-misses-fleet-publication.md`](trailing-cancelled-trash-observation-test-misses-fleet-publication.md)
  — The cancellation test does not assert its committed fleet wake. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ui-host-lifecycle-provider-and-start-wording.md`](trailing-ui-host-lifecycle-provider-and-start-wording.md)
  — Container lifecycle wording bypasses provider-aware UI text. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-ui-deleting-environment-retains-running-reconnect-promise.md`](trailing-ui-deleting-environment-retains-running-reconnect-promise.md)
  — Deleting environment can promise its cached session is still running. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ui-stopped-container-waiting-place-offers-suspend.md`](trailing-ui-stopped-container-waiting-place-offers-suspend.md)
  — Stopped container offers Suspend when its container host is unavailable. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ui-host-phase-fixture-omits-stopped-deleting.md`](trailing-ui-host-phase-fixture-omits-stopped-deleting.md)
  — UI phase-wide regression guards omit the new host phases. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-ui-host-flow-omits-shared-environment-details.md`](trailing-ui-host-flow-omits-shared-environment-details.md)
  — Unverified after investigation: shared environment detail changes bypass causal anchor projection. Abbreviated
  collection; confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ui-remove-label-docstring-still-sandbox-only.md`](trailing-ui-remove-label-docstring-still-sandbox-only.md)
  — Removal-label documentation still promises sandbox wording for containers. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-stopped-waiting-row-menu-order-disagrees-with-render.md`](trailing-stopped-waiting-row-menu-order-disagrees-with-render.md)
  — A stopped-waiting container row renders an item omitted from keyboard navigation. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-retired-only-bulk-delete-cancel-has-no-focus-target.md`](trailing-retired-only-bulk-delete-cancel-has-no-focus-target.md)
  — Retired-only bulk Delete cancellation has no rendered focus return. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-retired-delete-only-menu-invalidates-unconditional-rename-rationale.md`](trailing-retired-delete-only-menu-invalidates-unconditional-rename-rationale.md)
  — Delete-only retirement leaves the separator explanation claiming every row has Rename. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-sidebar-retires-restart-success-before-mounted-observer.md`](trailing-sidebar-retires-restart-success-before-mounted-observer.md)
  — Sidebar retirement can consume a restart result before the open view observes it. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-warned-delete-handoff-clears-error-before-view-admission.md`](trailing-warned-delete-handoff-clears-error-before-view-admission.md)
  — A deferred whole-environment Delete offer can lose its failure explanation. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-bulk-cleanup-captures-targets-after-deleting-exclusion.md`](trailing-bulk-cleanup-captures-targets-after-deleting-exclusion.md)
  — A confirmed bulk target excluded as Deleting keeps its old mark. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-deleting-environment-row-rename-omits-phase-guard.md`](trailing-deleting-environment-row-rename-omits-phase-guard.md)
  — Rename stays enabled for retained sessions while their environment is being deleted. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-late-environment-delete-count-republished-after-host-absence.md`](trailing-late-environment-delete-count-republished-after-host-absence.md)
  — A delayed Delete result can leave a destroyed environment described as staying. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-delete-expanded-error-false-no-change-comment.md`](trailing-environment-delete-expanded-error-false-no-change-comment.md)
  — Expanded Delete error handling retains a false no-change explanation. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-settings-container-tensorlake-order-contract.md`](trailing-settings-container-tensorlake-order-contract.md)
  — Settings regression test enforces the wrong section order. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-retired-container-positive-ui-test-removal.md`](trailing-retired-container-positive-ui-test-removal.md) —
  Legacy-container replacement and owned-template cleanup preserve their contracts. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-live-recipe-settings-need-an-independent-contract-based-drop-check.md`](trailing-live-recipe-settings-need-an-independent-contract-based-drop-check.md)
  — Live recipe settings need an independent contract-based drop check. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-wrapper-direct-allow-without-separate-prefix-gate.md`](trailing-environment-wrapper-direct-allow-without-separate-prefix-gate.md)
  — Environment wrapper bytes are shown and the separate prefix gate does not apply. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-retired-supervisor-tmux-pin.md`](trailing-retired-supervisor-tmux-pin.md) — Removing the unit's tmux pin
  matches the retired runtime contract. Abbreviated collection; confidence, proposed dismissals and unresolved
  consequences are preserved in the entry.

- [`trailing-stopped-provider-state-zero-refusal-ticket.md`](trailing-stopped-provider-state-zero-refusal-ticket.md) —
  Stopped-environment attach diagnosis is excluded by its own freshness gate. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-sealed-setup-test-overlay-hides-hold-and-run-leak.md`](trailing-sealed-setup-test-overlay-hides-hold-and-run-leak.md)
  — Sealed-setup regression test cannot observe its claimed no-hold boundary. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ordinary-tensorlake-add-outer-source-lock-across-ssh-readiness.md`](trailing-ordinary-tensorlake-add-outer-source-lock-across-ssh-readiness.md)
  — Ordinary Add holds recipe source exclusion while waiting for SSH. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-prepared-image-inherits-private-tmux-cleanup-exception.md`](trailing-prepared-image-inherits-private-tmux-cleanup-exception.md)
  — Prepared-snapshot cleanup explicitly excludes bundled-tmux removal. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-provisioning-shared-environment-preparation-progress.md`](trailing-provisioning-shared-environment-preparation-progress.md)
  — Environment preparation stays on Select the recipe source in launch progress. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-intermediate-reach-test-comment-accurately-names-the-remaining-uninstall-consumer.md`](trailing-intermediate-reach-test-comment-accurately-names-the-remaining-uninstall-consumer.md)
  — Intermediate reach-test comment accurately names the remaining uninstall consumer. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-provisioning-cleanup-test-misstates-pre-0-18-tmux-ownership.md`](trailing-provisioning-cleanup-test-misstates-pre-0-18-tmux-ownership.md)
  — Cleanup test misstates legacy tmux ownership. Abbreviated collection; confidence, proposed dismissals and unresolved
  consequences are preserved in the entry.

- [`trailing-uninstall-previously-orphaned-legacy-private-tmux-survives.md`](trailing-uninstall-previously-orphaned-legacy-private-tmux-survives.md)
  — Deleted retry test concerns only the accepted previously orphaned tmux server. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ssh-provisioning-test-private-server-coverage-docstring.md`](trailing-ssh-provisioning-test-private-server-coverage-docstring.md)
  — SSH integration test still claims private-tmux teardown coverage. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-history-obsolete-claim-after-adoption.md`](trailing-environment-history-obsolete-claim-after-adoption.md)
  — Origin-provenance input confirmed separately from replay authority. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-history-deleted-place-id-reuse.md`](trailing-environment-history-deleted-place-id-reuse.md) —
  Reusable place intent does not freeze a pending action's endpoint authority. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-clone-protocol-docs-omit-environment-hostless-contract.md`](trailing-clone-protocol-docs-omit-environment-hostless-contract.md)
  — Clone protocol docs reject the omission required by environment clones. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-interrupted-doc-defines-only-reboot-attribution-after-retirement.md`](trailing-interrupted-doc-defines-only-reboot-attribution-after-retirement.md)
  — Interruption documentation still claims boot-id change is its only detector. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-provisioning-probe-omits-stdin-watchdog.md`](trailing-environment-provisioning-probe-omits-stdin-watchdog.md)
  — Environment provisioning probes omit the required stdin deadline. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-dial-kind-advances-before-process-marker.md`](trailing-podman-dial-kind-advances-before-process-marker.md)
  — Container dial staging disables its process-exit diagnosis. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-uninstall-retains-already-orphaned-legacy-private-tmux-server.md`](trailing-uninstall-retains-already-orphaned-legacy-private-tmux-server.md)
  — Legacy tmux server may survive uninstall; specification explicitly accepts it. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-literal-image-user-argument-injection.md`](trailing-podman-literal-image-user-argument-injection.md)
  — Literal input boundary refutation persists through the review tip. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-shared-recipe-container-base-summary-raw-presentation.md`](trailing-shared-recipe-container-base-summary-raw-presentation.md)
  — Recipe summaries render literal container-base formatting characters. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-tensorlake-add-host-inline-container-recipe-compatibility.md`](trailing-tensorlake-add-host-inline-container-recipe-compatibility.md)
  — Inline container-recipe Save leaves Tensorlake Add host on an unusable choice. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-recipe-row-deep-clones-global-place-status-matrix.md`](trailing-recipe-row-deep-clones-global-place-status-matrix.md)
  — Recipe list clones the complete status matrix for every row. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-podman-job-removal-after-last-fleet-invalidation.md`](trailing-podman-job-removal-after-last-fleet-invalidation.md)
  — Container job completion lacks a final fleet invalidation. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-podman-account-observation-exec-guesses-root-home.md`](trailing-podman-account-observation-exec-guesses-root-home.md)
  — Custom-image login lookup unnecessarily requires /root. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-podman-ordinary-wal-tail-loss-after-external-create.md`](trailing-podman-ordinary-wal-tail-loss-after-external-create.md)
  — Power loss can lose a binding after external creation under accepted WAL durability. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-probe-dedup-test-asserts-before-spawned-task.md`](trailing-podman-probe-dedup-test-asserts-before-spawned-task.md)
  — Probe deduplication test checks its counter before a duplicate task can run. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-proxy-exact-inheritance-test-shared-serializer-oracle.md`](trailing-podman-proxy-exact-inheritance-test-shared-serializer-oracle.md)
  — Proxy test's shared serializer cannot prove its exact-inheritance claim. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-delete-lifecycle-test-equal-membership-oracle.md`](trailing-podman-delete-lifecycle-test-equal-membership-oracle.md)
  — Deletion test does not distinguish the membership authority it claims. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-tests-nested-build-module-registration.md`](trailing-podman-tests-nested-build-module-registration.md)
  — Nested build module registers the Podman tests and validates their namespace. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-test-payload-producer-consumer-tmux-mismatch.md`](trailing-podman-test-payload-producer-consumer-tmux-mismatch.md)
  — Real-container prerequisites require a payload the producer no longer stages. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-lost-session-create-test-misses-presend-store-witness.md`](trailing-podman-lost-session-create-test-misses-presend-store-witness.md)
  — Lost-create test does not observe its promised pre-send durability boundary. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-acceptance-fallback-misses-inflight-ssh-bootstrap-storage.md`](trailing-podman-acceptance-fallback-misses-inflight-ssh-bootstrap-storage.md)
  — Source-verified conditional SSH bootstrap cleanup gap. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-real-container-agent-host-list-oracle-omits-extra-ssh-hosts.md`](trailing-real-container-agent-host-list-oracle-omits-extra-ssh-hosts.md)
  — Real-container hosts-list test overstates its exact-membership oracle. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-selected-above-handoff-floor-legacy-target-missing-tmux-launch-advice.md`](trailing-selected-above-handoff-floor-legacy-target-missing-tmux-launch-advice.md)
  — Older-target prerequisite omission has explicit competing installer policy. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-snapshot-preparation-skips-inherited-private-tmux-removal.md`](trailing-snapshot-preparation-skips-inherited-private-tmux-removal.md)
  — Snapshot tmux retention follows an explicit preparation exception. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-missing-release-entry-test-count-does-not-prove-exact-variant-inventory.md`](trailing-missing-release-entry-test-count-does-not-prove-exact-variant-inventory.md)
  — Missing-entry fixture premise overstates its inventory check. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-detach-timeout-test-confuses-cooperative-yield-with-writer-capacity.md`](trailing-detach-timeout-test-confuses-cooperative-yield-with-writer-capacity.md)
  — The detach timeout test yields before establishing writer backpressure. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ui-agent-environment-launch-last-session-consequence.md`](trailing-ui-agent-environment-launch-last-session-consequence.md)
  — Last-session wording matches the later revision's changed deletion contract. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-place-typed-ssh-refusal-reasons-missing-final-ui-escaping.md`](trailing-podman-place-typed-ssh-refusal-reasons-missing-final-ui-escaping.md)
  — Typed SSH capacity reasons bypass final display escaping. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-ui-shared-sandbox-recipe-choices-omit-resource-sizes.md`](trailing-ui-shared-sandbox-recipe-choices-omit-resource-sizes.md)
  — Shared sandbox recipe picker omits CPU and memory sizes. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-ui-podman-start-help-overpromises-interrupted-and-restart.md`](trailing-ui-podman-start-help-overpromises-interrupted-and-restart.md)
  — Container-start explanation promises interrupted status for every session. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ui-container-leftover-question-missing-answer-focus-handoff.md`](trailing-ui-container-leftover-question-missing-answer-focus-handoff.md)
  — Container-leftover Delete opens its question without focusing an answer. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ui-recipe-panel-move-leaves-old-key-location-and-module-map.md`](trailing-ui-recipe-panel-move-leaves-old-key-location-and-module-map.md)
  — Moved Recipes panel retains guidance for the old Tensorlake layout. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-unregistered-explicit-cleanup-lacks-retained-operation-reason.md`](trailing-podman-unregistered-explicit-cleanup-lacks-retained-operation-reason.md)
  — Closing Containers loses an unregistered cleanup failure's reason. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ui-legacy-container-editor-specified-retirement.md`](trailing-ui-legacy-container-editor-specified-retirement.md)
  — Removing the legacy container editor is specified retirement. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ui-retained-whole-environment-question-stale-unreachable-rationale.md`](trailing-ui-retained-whole-environment-question-stale-unreachable-rationale.md)
  — Retained whole-environment warning can misstate current reachability. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-grid-only-removal-leaves-hidden-pane-fitaddon-rationale-stale.md`](trailing-grid-only-removal-leaves-hidden-pane-fitaddon-rationale-stale.md)
  — Renderer removal leaves hidden-pane rationale pointing at removed FitAddon. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-file-links-failed-grid-mount-fixture-getter-mismatch.md`](trailing-file-links-failed-grid-mount-fixture-getter-mismatch.md)
  — Rewritten failed-mount test misses the grid hosting failure. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-environment-pending-launch-label-resolves-with-empty-places.md`](trailing-environment-pending-launch-label-resolves-with-empty-places.md)
  — Container launch progress loses the selected host name. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-agent-clone-wire-docs-unconditional-host-requirement.md`](trailing-agent-clone-wire-docs-unconditional-host-requirement.md)
  — Clone wire documentation still forbids the new environment clone form. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-agent-create-cwd-help-omits-recipe-home-default.md`](trailing-agent-create-cwd-help-omits-recipe-home-default.md)
  — Create folder help omits the new recipe launch default. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-replay-reveal-helper-retired-async-write-rationale.md`](trailing-replay-reveal-helper-retired-async-write-rationale.md)
  — Reveal-wait documentation describes a retired asynchronous write. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-retired-ci-tmux-python-ignore-removed-without-residue-cleanup.md`](trailing-retired-ci-tmux-python-ignore-removed-without-residue-cleanup.md)
  — Unignoring an older non-self-ignored venv can add generated files to jj changes. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-mac-devbuild-header-retains-removed-local-tmux-requirement.md`](trailing-mac-devbuild-header-retains-removed-local-tmux-requirement.md)
  — Intermediate Mac devbuild requirements still demand the removed tmux prerequisite. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-desktop-smoke-direct-websocket-comment-overclaims-page-delivery.md`](trailing-desktop-smoke-direct-websocket-comment-overclaims-page-delivery.md)
  — Direct WebSocket smoke helper overstates its evidence of page delivery. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-payload-cargo-artifact-identity-not-held-through-copy.md`](trailing-podman-payload-cargo-artifact-identity-not-held-through-copy.md)
  — Shared Cargo output can change before same-source payload packaging. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-deflake-current-driver-downgrades-historical-recorder-required-tmux-policy.md`](trailing-deflake-current-driver-downgrades-historical-recorder-required-tmux-policy.md)
  — Historical-base deflake runs lose their selected revision's required tmux check. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-legacy-container-delete-conflates-podman-resolution-failure-with-absence.md`](trailing-legacy-container-delete-conflates-podman-resolution-failure-with-absence.md)
  — unverified after investigation: PATH lookup failure versus Podman absence. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-launch-comment-terminal-host-inherits-tab-marker.md`](trailing-launch-comment-terminal-host-inherits-tab-marker.md)
  — marker-inheritance documentation wrongly puts tab markers on terminal hosts. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-failed-host-relaunch-warn-scope-error-definitive-rollback.md`](trailing-failed-host-relaunch-warn-scope-error-definitive-rollback.md)
  — failed-relaunch warning policy and restoration route predate the full range. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-deleted-integrated-unmarked-tab-shell-delete-oracle.md`](trailing-deleted-integrated-unmarked-tab-shell-delete-oracle.md)
  — Unmarked tab-shell cleanup loses its integrated regression oracle. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-checkout-status-borrower-reload-keeps-origin-runtime.md`](trailing-checkout-status-borrower-reload-keeps-origin-runtime.md)
  — borrower reload fixture retains a competing origin runtime. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-legacy-tmux-retry-refusal-private-launch-artifact-retention.md`](trailing-legacy-tmux-retry-refusal-private-launch-artifact-retention.md)
  — Tmux-retry refusal defers cleanup of credential-bearing launch input. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-tab-settle-gate-bare-link-out-of-scope.md`](trailing-tab-settle-gate-bare-link-out-of-scope.md) —
  TabSettleGate retains an unresolved documentation link through the pinned tip. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-restart-definitive-comment-promises-no-external-effects.md`](trailing-restart-definitive-comment-promises-no-external-effects.md)
  — Definitive-Restart documentation overpromises absence of external effects. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-restart-reply-stale-tab-rediscovery-comment.md`](trailing-restart-reply-stale-tab-rediscovery-comment.md) —
  Restart reply comment promises a removed tab rediscovery path. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-legacy-retry-refusal-skips-launch-artifact-cleanup.md`](trailing-legacy-retry-refusal-skips-launch-artifact-cleanup.md)
  — Legacy retry refusal leaves old launch artifacts until later cleanup; proposed specification-based drop. Abbreviated
  collection; confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-lone-surviving-old-tab-loses-its-guard-and-stop-regression-oracles.md`](trailing-lone-surviving-old-tab-loses-its-guard-and-stop-regression-oracles.md)
  — lone surviving old tab loses its guard and Stop regression oracles. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-failed-spec-test-accepts-an-attempted-host-start-as-never-started.md`](trailing-failed-spec-test-accepts-an-attempted-host-start-as-never-started.md)
  — failed-spec test accepts an attempted host start as never started. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-failed-restart-loses-the-live-tab-preservation-regression-oracle.md`](trailing-failed-restart-loses-the-live-tab-preservation-regression-oracle.md)
  — failed Restart loses the live-tab preservation regression oracle. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-deleted-ambiguous-create-test-loses-published-handle-and-pending-intent-oracles.md`](trailing-deleted-ambiguous-create-test-loses-published-handle-and-pending-intent-oracles.md)
  — deleted ambiguous-create test loses published-handle and pending-intent oracles. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-held-start-documentation-retains-the-retired-uncancelled-tmux-child-lifetime.md`](trailing-held-start-documentation-retains-the-retired-uncancelled-tmux-child-lifetime.md)
  — held-start documentation retains the retired uncancelled-tmux child lifetime. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-deleted-startup-error-artifact-readonly-oracle.md`](trailing-deleted-startup-error-artifact-readonly-oracle.md)
  — Deleted reload test loses read-only Error-artifact ownership oracle. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-host-scan-lock-map-forbids-required-host-io.md`](trailing-host-scan-lock-map-forbids-required-host-io.md) —
  Host-scan lock map forbids the terminal calls its fence must span. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-sentinel-reload-comment-denies-recorded-host-delete-fallback.md`](trailing-sentinel-reload-comment-denies-recorded-host-delete-fallback.md)
  — Reload comment incorrectly makes the runtime handle Delete's only host ownership record. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-tab-scope-prefix-stale-two-terminal-kinds-docstring.md`](trailing-tab-scope-prefix-stale-two-terminal-kinds-docstring.md)
  — Tab scope-helper documentation retains a removed two-kind responsibility. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-omp-capture-survives-owner-exit-test-loses-exit-barrier.md`](trailing-omp-capture-survives-owner-exit-test-loses-exit-barrier.md)
  — Host-backed teardown removes the owner-exit test's exit barrier. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-prompt-recorder-default-off-without-catchup-witness.md`](trailing-prompt-recorder-default-off-without-catchup-witness.md)
  — Proposed source-based exclusion: examined raw paste-off fixture produces one initial output record. Abbreviated
  collection; confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-terminal-less-host-orphan-doc-asserts-no-survivors.md`](trailing-terminal-less-host-orphan-doc-asserts-no-survivors.md)
  — Terminal-less test documentation treats host loss as proof no program survives. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-hold-mark-negative-oracle-retained-in-host-terminal-tests.md`](trailing-hold-mark-negative-oracle-retained-in-host-terminal-tests.md)
  — Host-terminal test module preserves the missing/stale hold-mark oracle. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-migration-missing-terminal-lifecycle-docs.md`](trailing-migration-missing-terminal-lifecycle-docs.md) —
  Migration commit has stale lifecycle docs, corrected by later removal. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-supervisor-clone-doorway-testdoc-universal-host-requirement.md`](trailing-supervisor-clone-doorway-testdoc-universal-host-requirement.md)
  — Creating-verb test documentation still requires a host for every clone. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-delete-directory-admission-test-observes-only-earlier-agent-fence.md`](trailing-delete-directory-admission-test-observes-only-earlier-agent-fence.md)
  — Directory-admission test observes the earlier fence, not the blocked directory wait. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-pre-range-grid-first-input-queue-acceptance-wording.md`](trailing-pre-range-grid-first-input-queue-acceptance-wording.md)
  — Direct grid input call preserves the prior delivery contract. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-deleted-replay-chunk-leaves-connection-doc-reference.md`](trailing-deleted-replay-chunk-leaves-connection-doc-reference.md)
  — Replay removal leaves a dangling writer-queue documentation link. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-stop-comment-falsely-bans-retaining-original-final-screen.md`](trailing-stop-comment-falsely-bans-retaining-original-final-screen.md)
  — Stop comment incorrectly says full-screen final frames are never retained. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-busy-host-list-limit-negative-oracle-never-reads-client-output.md`](trailing-busy-host-list-limit-negative-oracle-never-reads-client-output.md)
  — List-limit test checks an undrained reply cache. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-request-conformance-target-testhost-owner-dropped-at-preparation-return.md`](trailing-request-conformance-target-testhost-owner-dropped-at-preparation-return.md)
  — Keep the target host alive through conformance observation. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-retired-tmux-periodic-warning-contract.md`](trailing-retired-tmux-periodic-warning-contract.md) —
  Old-terminal warning tests are obsolete under the migration contract. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-test-one-shot-sample-before-host-catch-up.md`](trailing-test-one-shot-sample-before-host-catch-up.md) —
  Persistence test can skip an uncaught-up host. Abbreviated collection; confidence, proposed dismissals and unresolved
  consequences are preserved in the entry.

- [`trailing-sampler-docs-promise-deleted-capture-failure-reset.md`](trailing-sampler-docs-promise-deleted-capture-failure-reset.md)
  — Sampler field documentation still promises the deleted capture-failure reset. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-terminal-files-cursor-teardown-comment-misattributes-global-setter-to-grid.md`](trailing-terminal-files-cursor-teardown-comment-misattributes-global-setter-to-grid.md)
  — Cursor teardown comment attributes retired accessor behavior to the grid. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-retired-container-positive-path-test-removal.md`](trailing-retired-container-positive-path-test-removal.md)
  — Retired container isolation tests were deliberately removed. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-legacy-container-delete-existing-object-removal-oracle-lost.md`](trailing-legacy-container-delete-existing-object-removal-oracle-lost.md)
  — Legacy Delete loses its existing-container removal oracle. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-upload-ack-priority-regression-oracle-removed.md`](trailing-upload-ack-priority-regression-oracle-removed.md)
  — Upload credit priority loses its end-to-end regression check. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-refused-delete-live-upload-continuation-oracle-removed.md`](trailing-refused-delete-live-upload-continuation-oracle-removed.md)
  — Refused Delete loses its in-flight upload preservation oracle. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-deleted-legacy-deep-replay-tail-oracle.md`](trailing-deleted-legacy-deep-replay-tail-oracle.md) — Removing
  the replay suite loses the legacy replay-tail oracle. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-deleted-signal-legacy-two-client-shutdown-oracle.md`](trailing-deleted-signal-legacy-two-client-shutdown-oracle.md)
  — Stop-test migration removes the signal-to-legacy-client teardown oracle. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-deleted-raw-pty-cursor-control-sequence-oracle.md`](trailing-deleted-raw-pty-cursor-control-sequence-oracle.md)
  — The deleted raw-input test leaves cursor/control-sequence delivery without its end-to-end oracle. Abbreviated
  collection; confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-removed-serving-legacy-binary-final-output-oracle.md`](trailing-removed-serving-legacy-binary-final-output-oracle.md)
  — Deleting the immediate-exit test loses the real-ticker legacy final-output oracle. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-removed-completed-keyed-create-reboot-replay-oracle.md`](trailing-removed-completed-keyed-create-reboot-replay-oracle.md)
  — Deleting the kind test also removes the completed-create reboot retry oracle. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-stopped-altscreen-test-comment-claims-inactive-buffer-erasure.md`](trailing-stopped-altscreen-test-comment-claims-inactive-buffer-erasure.md)
  — Correct the stopped alternate-screen test's emulator-retention explanation. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-old-server-guard-loses-directory-in-consumed-restart.md`](trailing-old-server-guard-loses-directory-in-consumed-restart.md)
  — Restart failure deletes the tmux socket before the explicit server guard runs. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-flood-fixture-false-child-exit-terminal-lifetime-rationale.md`](trailing-flood-fixture-false-child-exit-terminal-lifetime-rationale.md)
  — Flood fixture docs incorrectly say program exit ends its terminal. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-rust-worker-trims-explicit-payload-directory.md`](trailing-rust-worker-trims-explicit-payload-directory.md)
  — Archive packer changes an accepted explicit payload directory. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-docs-stop-callout-overpromises-descendant-cleanup.md`](trailing-docs-stop-callout-overpromises-descendant-cleanup.md)
  — Exclusive range base already contains the broad Stop claim and conflicting cleanup caveat. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-capture-terminaltext-stale-no-row-dom-rationale.md`](trailing-capture-terminaltext-stale-no-row-dom-rationale.md)
  — Update the terminal target helper's obsolete no-row-DOM rationale. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-old-terminal-startup-filesystem-cleanup-outside-deadline.md`](trailing-old-terminal-startup-filesystem-cleanup-outside-deadline.md)
  — Old-terminal file cleanup runs outside its startup deadline. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-migrated-seal-constraint-test-accepts-duplicate-metadata-key.md`](trailing-migrated-seal-constraint-test-accepts-duplicate-metadata-key.md)
  — Migrated seal CHECK assertion accepts a duplicate-key failure. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-outcome-backfill-comment-falsely-promises-live-probe-after-legacy-retirement.md`](trailing-outcome-backfill-comment-falsely-promises-live-probe-after-legacy-retirement.md)
  — Outcome-backfill comment promises live reconciliation that legacy rows no longer receive. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-647f022-old-terminal-process-compatibility-oracle-deletion.md`](trailing-647f022-old-terminal-process-compatibility-oracle-deletion.md)
  — Old-terminal process wiring gap is historical after later source retirement. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-647f022-pre-reap-tab-live-only-attach-oracle-not-ported.md`](trailing-647f022-pre-reap-tab-live-only-attach-oracle-not-ported.md)
  — Dead-but-unreaped tab refusal gap remains applicable to host tabs at tip. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-70bc66a-host-size-wait-comment-denies-public-status-size.md`](trailing-70bc66a-host-size-wait-comment-denies-public-status-size.md)
  — Resize wait comment gives a false synchronization rationale. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-647f022-old-to-host-migration-integration-oracle-deletion.md`](trailing-647f022-old-to-host-migration-integration-oracle-deletion.md)
  — Migration regression coverage was removed before its behavior retired. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-647f022-stale-connection-pause-oracle-restored-f560058.md`](trailing-647f022-stale-connection-pause-oracle-restored-f560058.md)
  — Stale-owner pause coverage lost at 647f022 is restored at f560058. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-mixed-session-regression-coverage-disappears-before-legacy-serving-retires.md`](trailing-mixed-session-regression-coverage-disappears-before-legacy-serving-retires.md)
  — Mixed-session regression coverage disappears before legacy serving retires. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-legacy-forced-pause-mode-restoration-oracles-disappear-before-legacy-serving-retires.md`](trailing-legacy-forced-pause-mode-restoration-oracles-disappear-before-legacy-serving-retires.md)
  — Legacy forced-pause mode-restoration oracles disappear before legacy serving retires. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-legacy-stalled-viewer-rss-regression-check-is-removed-before-legacy-serving-retires.md`](trailing-legacy-stalled-viewer-rss-regression-check-is-removed-before-legacy-serving-retires.md)
  — Legacy stalled-viewer RSS regression check is removed before legacy serving retires. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-647f0227-removes-supported-tmux-pending-cleanup-request-oracles.md`](trailing-647f0227-removes-supported-tmux-pending-cleanup-request-oracles.md)
  — pending-cleanup integration oracles retire before tmux runtime support. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-terminal-tabs-shell-harness-docs-retain-deleted-conformance-caller.md`](trailing-terminal-tabs-shell-harness-docs-retain-deleted-conformance-caller.md)
  — shell harness docs still describe the deleted conformance battery. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-terminal-tabs-shared-gate-docs-retain-removed-local-hook-installations.md`](trailing-terminal-tabs-shared-gate-docs-retain-removed-local-hook-installations.md)
  — shared gate docs claim hook installations that the module no longer has. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-647f0227-removes-supported-tmux-transport-sink-crash-recovery-oracles.md`](trailing-647f0227-removes-supported-tmux-transport-sink-crash-recovery-oracles.md)
  — Still-supported old-terminal recovery loses integration regression protection. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-pre-range-tmux-probe-fixture-process-oracles.md`](trailing-pre-range-tmux-probe-fixture-process-oracles.md)
  — Retained probe fixtures and process oracles are unchanged from the range base. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-historical-desktop-tmux-abovepin-warning-after-retirement.md`](trailing-historical-desktop-tmux-abovepin-warning-after-retirement.md)
  — Historical installation policy keeps a newer-tmux warning promise after its desktop emitter retires. Abbreviated
  collection; confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-delete-held-gone-host-skips-adopted-process-confirmation.md`](trailing-delete-held-gone-host-skips-adopted-process-confirmation.md)
  — Known Delete confirmation bypass is separate from guarded tab-close identity. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-restart-seeded-history-promoted-into-launch-error-screen.md`](trailing-restart-seeded-history-promoted-into-launch-error-screen.md)
  — Earlier codec-removal refutation does not refute the known inherited-history excerpt route. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-connection-writer-comment-points-to-deleted-outputstream-checkpoint.md`](trailing-connection-writer-comment-points-to-deleted-outputstream-checkpoint.md)
  — Connection writer diagnostics still point to the deleted stream checkpoint. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-old-terminal-osc52-browser-refusal-test-removed-before-retirement.md`](trailing-old-terminal-osc52-browser-refusal-test-removed-before-retirement.md)
  — Old-terminal clipboard read refusal loses its browser check before retirement. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-real-app-terminal-disclosure-cover-and-refusal-layer-tests-removed.md`](trailing-real-app-terminal-disclosure-cover-and-refusal-layer-tests-removed.md)
  — Keep real-app checks for terminal disclosure admission and refusal layers. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-settings-to-mounted-agent-shell-link-ownership-integration-test-removed.md`](trailing-settings-to-mounted-agent-shell-link-ownership-integration-test-removed.md)
  — Keep program-observed link ownership checks through Settings and mounted tabs. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-old-terminal-whole-stream-flow-stall-paused-remount-oracles-removed-before-retirement.md`](trailing-old-terminal-whole-stream-flow-stall-paused-remount-oracles-removed-before-retirement.md)
  — Old-terminal flood and paused-remount checks disappear before retirement. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-old-terminal-positive-large-websocket-message-oracle-removed-before-retirement.md`](trailing-old-terminal-positive-large-websocket-message-oracle-removed-before-retirement.md)
  — Old-terminal large-message acceptance loses its regression check. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-retired-xterm-private-url-tag-test-gap.md`](trailing-retired-xterm-private-url-tag-test-gap.md) —
  Intermediate removal of the vendored-xterm tag contract test. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-retired-xterm-deferred-replay-flush-test-gap.md`](trailing-retired-xterm-deferred-replay-flush-test-gap.md)
  — Intermediate loss of late-xterm-flush recovery coverage. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-retired-xterm-per-tab-write-flow-isolation-test-gap.md`](trailing-retired-xterm-per-tab-write-flow-isolation-test-gap.md)
  — Intermediate loss of old-renderer per-tab flow isolation coverage. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-grid-catchup-progress-and-empty-frame-idle-tests-removed.md`](trailing-grid-catchup-progress-and-empty-frame-idle-tests-removed.md)
  — Deleting replay degradation tests also removes surviving grid idle-bound assertions. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-old-xterm-sustained-viewport-tests-removed-before-retirement.md`](trailing-old-xterm-sustained-viewport-tests-removed-before-retirement.md)
  — Intermediate loss of sustained viewport regression checks before old-renderer retirement. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-old-xterm-query-selection-unicode-tests-removed-before-retirement.md`](trailing-old-xterm-query-selection-unicode-tests-removed-before-retirement.md)
  — Old-renderer regressions lose coverage before the supported renderer is retired. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-mouse-reattach-test-claims-nonexistent-viewer-sgr-encoding-restoration.md`](trailing-mouse-reattach-test-claims-nonexistent-viewer-sgr-encoding-restoration.md)
  — Mouse test claims to check SGR viewer restoration that the grid protocol does not carry. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-retired-renderer-swap-kind-notice-test-deletion.md`](trailing-retired-renderer-swap-kind-notice-test-deletion.md)
  — Retired renderer compatibility tests removed with the compatibility path. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-held-replay-helper-misstates-history-drained-boundary.md`](trailing-held-replay-helper-misstates-history-drained-boundary.md)
  — held-replay helper claims history is applied before its oracle proves it. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-retired-old-terminal-migration-ui-test-deletion.md`](trailing-retired-old-terminal-migration-ui-test-deletion.md)
  — Removing the old-terminal migration UI tests matches the retired feature contract. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-hidden-grid-test-no-measurement-two-column-rationale.md`](trailing-hidden-grid-test-no-measurement-two-column-rationale.md)
  — Grid geometry test gives a false two-column failure rationale. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-tmux-only-sgr-adapter-deletion-frozen-snapshot-compatibility-concern.md`](trailing-tmux-only-sgr-adapter-deletion-frozen-snapshot-compatibility-concern.md)
  — Frozen snapshot compatibility concern remains source-refuted, pending independent check. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-primary-write-notice-filter-drag-guidance.md`](trailing-primary-write-notice-filter-drag-guidance.md) —
  Primary-selection writes cannot suppress drag-copy guidance (predates range). Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-terminal-grid-lacks-sgr53-overline-after-old-route-retirement.md`](trailing-terminal-grid-lacks-sgr53-overline-after-old-route-retirement.md)
  — Retiring the byte renderer removes overline support from served terminals. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`ime-commit-bypasses-quick-launch-capture.md`](ime-commit-bypasses-quick-launch-capture.md) — Some input-method text
  typed for a quick-launch sequence goes to the underlying terminal while the sequence stays armed.

- [`wayland-drop-lost-before-file-list-arrives.md`](wayland-drop-lost-before-file-list-arrives.md) — Releasing a file
  over a terminal before the file manager has returned its list can do nothing. Farhelm gives no indication that it
  missed the drop.

- [`checkout-status-bypasses-host-text.md`](checkout-status-bypasses-host-text.md) — A launcher status line can show
  repository-search errors without Farhelm’s usual treatment of invisible or right-to-left characters.

- [`gui-command-debug-includes-secret-body.md`](gui-command-debug-includes-secret-body.md) — A future diagnostic that
  prints a GUI command could expose a token or login file included in that request.

- [`linux-second-launch-lost-during-quit.md`](linux-second-launch-lost-during-quit.md) — If you reopen Farhelm while the
  old window is quitting, the new process can say it handed off to that window and exit. The old process then exits too,
  leaving no window.

- [`untouched-host-alias-save-restores-old-value.md`](untouched-host-alias-save-restores-old-value.md) — Clicking Save
  without typing can undo a host rename that just happened through an approved agent request.

- [`partial-host-edit-reports-whole-failure.md`](partial-host-edit-reports-whole-failure.md) — A single approved host
  edit can change the alias and then fail to change the icon or color. The failure message can leave out that the alias
  already changed.

- [`tunnel-listener-stops-on-single-accept-error.md`](tunnel-listener-stops-on-single-accept-error.md) — One listener
  error can leave a tunnel’s port visible in Farhelm after it stopped accepting connections. The next list refresh can
  remove the mapping without a reason.

- [`linux-helm-allows-mac-only-tunnel-open.md`](linux-helm-allows-mac-only-tunnel-open.md) — The Linux desktop shares a
  helm route that will open a localhost tunnel for an authenticated caller. Its own terminal UI only offers the action
  on macOS.

- [`linux-docs-capture-follow-ups.md`](linux-docs-capture-follow-ups.md) — The README hero still uses the Mac workflow
  by default. A small Templates button crop would fail the publisher’s size check if a page uses it, and the template
  search crop catches a warning below the search box.

- [`desktop-updater-read-failure-enables-auto-install.md`](desktop-updater-read-failure-enables-auto-install.md) —
  Farhelm can install an update in the background after you turned automatic updates off, if it later cannot read the
  setting.

- [`tensorlake-add-host-caches-skip-confirmation.md`](tensorlake-add-host-caches-skip-confirmation.md) — If the
  confirmation setting changes while Add Host stays open, the form can skip the question before starting a billed
  Tensorlake sandbox.

- [`environment-launch-opens-earliest-session.md`](environment-launch-opens-earliest-session.md) — If two sessions
  appear in a new environment before launch finishes, Farhelm can open the other session when their creation times tie
  or arrive out of order.

- [`late-delete-loses-checkout-archive-notice.md`](late-delete-loses-checkout-archive-notice.md) — If Delete takes long
  enough to time out while the host is still working, Farhelm can remove the session without telling you where its
  unarchived checkout was left.

- [`stop-and-suspend-hides-timeout-reason.md`](stop-and-suspend-hides-timeout-reason.md) — When Stop and suspend exceeds
  its reply deadline, Farhelm can leave you without the explanation that it is still working or that Stop already
  failed.

- [`credential-editor-retains-obsolete-secret-copies.md`](credential-editor-retains-obsolete-secret-copies.md) — The
  credential editor can keep an extra copy of a rejected or superseded token or login file in app memory after you have
  edited the draft.

- [`attachment-paste-keeps-directory-control-bytes.md`](attachment-paste-keeps-directory-control-bytes.md) — Uploading
  from a session can paste control characters from the host's attachment directory into the same terminal along with the
  path.

- [`secret-handling-tests-omit-output-assertions.md`](secret-handling-tests-omit-output-assertions.md) — Credential
  tests can pass even if a token, login file, or device credential appears outside the intended entry field or request,
  because they rarely look for the secret in the painted frame, refusal text, or logs.

- [`desktop-startup-update-duplicates.md`](desktop-startup-update-duplicates.md) — Desktop startup and updates maintain
  separate copies of the same checks, formatting, and reopen command construction.

- [`terminal-grid-row-traversal-duplicated.md`](terminal-grid-row-traversal-duplicated.md) — Terminal word selection and
  file-path recognition each interpret the same grid row separately, so their treatment of the same cells can drift.

- [`gui-controls-repeat-shared-rendering.md`](gui-controls-repeat-shared-rendering.md) — Controls draw their own focus
  rings and repeat small formatting rules that should stay consistent across forms.

- [`gui-focus-checks-repeat-drawn-predicate.md`](gui-focus-checks-repeat-drawn-predicate.md) — Several focus paths
  reimplement the check that a control is present in the frame.

- [`approval-views-repeat-status-lookups.md`](approval-views-repeat-status-lookups.md) — Host-add and tunnel approval
  views independently look up whether a request is waiting or being answered.

- [`gui-command-paths-repeat-reply-classification.md`](gui-command-paths-repeat-reply-classification.md) — Two GUI
  request paths repeat the classification of the same helm replies and refusal messages.

- [`hosts-menu-and-settings-repeat-operation-logic.md`](hosts-menu-and-settings-repeat-operation-logic.md) — Hosts
  repeats the code that closes menus, describes lifecycle operations, retains current progress, and saves permanent
  answers.

- [`launcher-repeats-choices-and-summary-formatting.md`](launcher-repeats-choices-and-summary-formatting.md) — Launcher
  controls, search, and recent setups have separate implementations of labels and display rules that are supposed to
  match.

- [`gui-screenshot-scenes-repeat-fixture-wiring.md`](gui-screenshot-scenes-repeat-fixture-wiring.md) — Screenshot scenes
  each build similar fake sessions and set up dialogs independently. One scene also recreates the update text it is
  meant to show.

- [`desktop-supervisor-ownership-save-is-silent.md`](desktop-supervisor-ownership-save-is-silent.md) — Farhelm can start
  its supervisor and then silently fail to record it. Later launches can treat that supervisor as externally started.

- [`launcher-stale-search-selection-fails-silently.md`](launcher-stale-search-selection-fails-silently.md) — Choosing a
  vanished host or stale recent setup can clear the search or do nothing without saying why.

- [`session-list-actions-repeat-admission-and-progress.md`](session-list-actions-repeat-admission-and-progress.md) —
  Session actions repeat their request setup, current-work lookup, and progress rendering. Compact and full rows also
  repeat part of the status display.

- [`session-pane-repeats-last-known-status.md`](session-pane-repeats-last-known-status.md) — The Details bar and the
  unavailable-session view separately calculate the same last known status and activity age.

- [`settings-repeat-preference-and-focus-wiring.md`](settings-repeat-preference-and-focus-wiring.md) — Several controls
  build the same preference command, and two Settings editors independently restore focus to the control that opened
  them.

- [`templates-repeat-lookup-and-control-wiring.md`](templates-repeat-lookup-and-control-wiring.md) — Template screens
  repeat lookups, refusal wording, control construction, and shortcut input mapping.

- [`terminal-repeats-modifier-mapping.md`](terminal-repeats-modifier-mapping.md) — The terminal and its help keep
  separate copies of platform modifier rules. The terminal also copies modifier conversion for mouse and keyboard input.

- [`trash-repeats-animation-and-focus-wiring.md`](trash-repeats-animation-and-focus-wiring.md) — Trash repeats the
  spinner's timer and independently looks up the same focus destination on confirmation and cancellation.

- [`moved-on-confirmation-test-accepts-wrong-focus.md`](moved-on-confirmation-test-accepts-wrong-focus.md) — The
  macOS-style Reset test can pass when focus lands on the wrong drawn control and stays there.

- [`delete-shortcut-test-misses-early-request.md`](delete-shortcut-test-misses-early-request.md) — The test can pass if
  Delete sends a request before the user answers the question, even though Enter is supposed to authorize that request.

- [`notice-docs-link-test-accepts-plain-text.md`](notice-docs-link-test-accepts-plain-text.md) — The test only checks
  escaping after the notice recognizes a link, so it can pass if the address is displayed as plain text.

- [`update-restart-failure-test-follows-status-id.md`](update-restart-failure-test-follows-status-id.md) — The updater
  test can pass if a restart failure moves inline under a new element ID instead of staying in the hover help.

- [`terminal-input-replays-accept-empty-fixtures.md`](terminal-input-replays-accept-empty-fixtures.md) — The terminal
  key and mouse compatibility replays can pass without running their assertions when their fixture tables are empty.

- [`wrapped-link-test-misses-second-open.md`](wrapped-link-test-misses-second-open.md) — The test checks that the second
  click gives no refusal but never verifies that it opens the link.

- [`control-tests-repeat-simulated-key-events.md`](control-tests-repeat-simulated-key-events.md) — Control and approval
  tests each construct and deliver the same simulated key presses and releases.

- [`session-list-tests-repeat-frame-and-request-helpers.md`](session-list-tests-repeat-frame-and-request-helpers.md) —
  Session-list tests maintain separate copies of their frame queries, mouse gestures, and fake-helm acknowledgements.

- [`connection-refusal-tests-release-port-too-early.md`](connection-refusal-tests-release-port-too-early.md) — Two tests
  can reach a different process if it starts listening after the test gives up the port it reserved.

- [`credential-chooser-test-assumes-missing-global-path.md`](credential-chooser-test-assumes-missing-global-path.md) —
  The missing-file test depends on a machine-wide file path staying absent instead of controlling that premise in its
  fixture.

- [`staged-payloads-and-automatic-host-updates.md`](staged-payloads-and-automatic-host-updates.md) — Whether automatic
  host updates can push an operator-staged, unverified payload directory without a per-host action has not been checked.

- [`checkout-use-default-presentation.md`](checkout-use-default-presentation.md) — Use default can clear a freshly
  loaded checkout-folder override before the editor shows it.

- [`failed-demo-rollback-can-remove-previous-preview.md`](failed-demo-rollback-can-remove-previous-preview.md) — A
  preview rollback that fails can leave the previous generated demo output in temporary files that are then deleted.

- [`group-divider-can-change-shift-sweep-range.md`](group-divider-can-change-shift-sweep-range.md) — Shift-dragging
  across group spacing can change the end of the marked session range.

- [`group-toggle-inverts-drawn-action.md`](group-toggle-inverts-drawn-action.md) — Collapse group can expand the group
  after its shared state changes before the click.

- [`launcher-catalog-moves-model-highlight.md`](launcher-catalog-moves-model-highlight.md) — A model catalog update can
  move the highlight so Enter picks a different model for the launch draft.

- [`macos-shift-sweep-can-miss-release.md`](macos-shift-sweep-can-miss-release.md) — A macOS Shift sweep can keep
  scrolling and marking after a release with changed modifiers.

- [`manual-update-check-can-join-already-settled-auto-hold.md`](manual-update-check-can-join-already-settled-auto-hold.md)
  — A manual desktop update check can join an automatic check that has already decided to keep an older pending update.

- [`mute-menu-expiry-preview-can-go-stale.md`](mute-menu-expiry-preview-can-go-stale.md) — A mute menu left open across
  the morning cutoff can save a mute that lasts a day longer than the menu shows.

- [`older-catalog-model-overrides-custom-owner.md`](older-catalog-model-overrides-custom-owner.md) — Switching agents in
  the launcher can keep a custom model because the destination has an older catalog entry with the same name.

- [`session-groups-lose-membership-in-accessibility-tree.md`](session-groups-lose-membership-in-accessibility-tree.md) —
  The sidebar can omit a session's group membership from screen-reader information.

- [`settings-search-key-hidden-by-open-editor.md`](settings-search-key-hidden-by-open-editor.md) — The Tensorlake key
  search result can take you back to an open credential editor and keep the key field hidden.

- [`shift-sweep-can-mark-through-sidebar-clip.md`](shift-sweep-can-mark-through-sidebar-clip.md) — In a narrow macOS
  window, a Shift sweep can keep changing the selection when the pointer is beside the clipped sidebar.

- [`sidebar-order-stays-held-after-shift-sweep.md`](sidebar-order-stays-held-after-shift-sweep.md) — Releasing a Shift
  sweep outside the sidebar can leave the previous list order visible for up to five seconds.

- [`test-worker-success-loses-evidence.md`](test-worker-success-loses-evidence.md) — An SSH test worker can report
  success when the results could not be copied back.

- [`test-worker-sync-precedes-lock.md`](test-worker-sync-precedes-lock.md) — A second SSH test request can change the
  checkout used by the run already in progress.

- [`tunnel-pill-can-show-previous-session-mapping.md`](tunnel-pill-can-show-previous-session-mapping.md) — Tunnel hover
  help can show the previous session's ports after the open session changes.
