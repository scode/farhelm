# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

## Highest priority: security or data loss

- [`tunnel-reuses-custom-helm-credential-origin.md`](tunnel-reuses-custom-helm-credential-origin.md) — A desktop tunnel
  can receive a previously used custom browser helm port and expose its retained device credential to remote HTML in the
  same browser profile. The default helm port and embedded desktop credentials are outside this conditional route.

- [`container-policy-missing-from-approvals.md`](container-policy-missing-from-approvals.md) — An agent's approved
  template change can alter container isolation without showing that change in its approval card.

- [`pending-create-retry-deletes-initialized-container-files.md`](pending-create-retry-deletes-initialized-container-files.md)
  — After a crash and reboot during initial creation, a failed retry can remove a container that already holds the
  agent's files.

- [`container-stop-approval-omits-tab-work.md`](container-stop-approval-omits-tab-work.md) — An agent Stop approval
  describes stopping the agent, although it can also terminate independent work in that container's terminal tabs.

- [`checkout-token-path-rereads-host-files.md`](checkout-token-path-rereads-host-files.md) — If the configured token
  path lies in the writable checkout, a later container tab can receive a readable host file selected through that path.

- [`detached-container-client-hides-live-agent-and-tab-work.md`](detached-container-client-hides-live-agent-and-tab-work.md)
  — Detaching the container client can leave the agent running while Farhelm treats it as gone; Delete can then stop it
  without the live-agent warning, and automatic tab cleanup can stop inner tab work.

- [`container-inherits-host-proxy-credentials.md`](container-inherits-host-proxy-credentials.md) — Container creation
  can expose host proxy credentials to the contained workload without an explicit credential-sharing choice.

- [`retained-container-startup-reaches-replacement-checkout.md`](retained-container-startup-reaches-replacement-checkout.md)
  — Opening a new tab in a retained container can give its old startup state access to a replacement checkout at the
  saved path.

- [`normal-durability-restores-revoked-grants.md`](normal-durability-restores-revoked-grants.md) — A power loss can
  restore a host command grant or browser network/tunnel permission after Farhelm acknowledged its revocation.

- [`critical-mac-commits-omit-media-flush.md`](critical-mac-commits-omit-media-flush.md) — On Mac storage with volatile
  drive caches, a power failure can undo acknowledged credential changes or lose a protected source identity despite the
  stronger SQL durability setting.

- [`power-loss-removes-valid-resume-binding.md`](power-loss-removes-valid-resume-binding.md) — A power loss can remove
  or replace an already accepted Resume offer after its report has been acknowledged and deleted.

- [`rolled-back-host-number-transfers-retained-approval.md`](rolled-back-host-number-transfers-retained-approval.md) —
  After a particular registry rollback, a different connected host can finish an earlier host's retained, approved
  launch using the same request.

- [`failed-restart-restoration-loses-valid-resume.md`](failed-restart-restoration-loses-valid-resume.md) — A named
  Restart that never starts its replacement can permanently remove the previous Resume offer if the database restoration
  write also fails.

- [`stored-wrapper-breaks-new-prompt-literalness.md`](stored-wrapper-breaks-new-prompt-literalness.md) — A previously
  stored wrapper can make a new initial prompt acquire shell meaning instead of remaining literal data.

- [`checkpoint-image-inspection-restores-unowned-workload.md`](checkpoint-image-inspection-restores-unowned-workload.md)
  — Choosing a compatible local checkpoint image can restore its old workload and host mounts during account inspection,
  then report preparation failed without owning the restored container.

- [`wrapper-guide-stop-promises-tab-preservation.md`](wrapper-guide-stop-promises-tab-preservation.md) — The wrapper
  guide promises Stop leaves independent tabs running, but built-in container Stop terminates their work too.

## High priority: material UX degradation

- [`older-development-archive-moves-replacement-folder.md`](older-development-archive-moves-replacement-folder.md) — An
  interrupted archive written by older development code can move a replacement folder after device/inode reuse when
  birth time is unavailable. Current permanent deletion refuses this birthless case; a proven older-release variant may
  be filtered.

- [`devbuild-run-only-skips-old-payload-authentication.md`](devbuild-run-only-skips-old-payload-authentication.md) —
  Launching an earlier staged development build bypasses the new tmux signature check. Whether the repair must cover
  operator-staged older builds is unresolved; the claim that verification runs every time is too broad.

- [`concurrent-devbuild-copy-outlives-hash-check.md`](concurrent-devbuild-copy-outlives-hash-check.md) — Overlapping
  direct builds in one checkout can copy tmux bytes replaced after their digest was checked. This maintainer-tooling
  authenticity concern is conditional on an ambiguous concurrency contract; practical race control and catastrophic harm
  are unproved.

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

- [`container-attachments-use-unreachable-host-paths.md`](container-attachments-use-unreachable-host-paths.md) — Normal
  image/file paste can report success but insert a host path that the container's agent or tab cannot read.

- [`podman-storage-selection-escapes-lifecycle-lookup.md`](podman-storage-selection-escapes-lifecycle-lookup.md) — A
  container created under a login shell's Podman configuration can survive while Stop reports absence from a different
  store.

- [`image-inspection-inherits-keep-id-account.md`](image-inspection-inherits-keep-id-account.md) — An ambient Podman
  keep-id setting can make account inspection reject an image whose ordinary keeper account is valid.

- [`dead-pane-pid-check-drops-first-conversation-report.md`](dead-pane-pid-check-drops-first-conversation-report.md) —
  If the first conversation report arrives after the agent exits on tmux 3.8, Farhelm can permanently lose ordinary
  Restart for that session.

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
  selection save may overwrite history-fixture or capture restoration; adverse ordering is unverified.

- [`instruction-file-case-alias-evades-check.md`](instruction-file-case-alias-evades-check.md) — Case aliases evade the
  local instruction-file check; actual vendor lookup on case-insensitive filesystems is unverified.

- [`menu-blur-can-leave-delayed-arrow-current.md`](menu-blur-can-leave-delayed-arrow-current.md) — A blur without
  focus-in may leave delayed menu-arrow work current; browser event behavior is unverified.

- [`normal-quit-misses-empty-download-directory.md`](normal-quit-misses-empty-download-directory.md) — A narrow
  normal-Quit race may leave an empty download staging folder; saved files are unaffected by this mechanism.

- [`notification-bell-removal-skips-focus-return.md`](notification-bell-removal-skips-focus-return.md) — Removing the
  notification bell may skip keyboard-focus return; browser fallback is unverified.

- [`pinned-host-dialog-chrome-can-hide-content.md`](pinned-host-dialog-chrome-can-hide-content.md) — The deleted dialog
  record still has an unresolved Host Settings footer-clipping case; exact geometry is unverified.

- [`prompt-prefill-test-opens-before-source-selection.md`](prompt-prefill-test-opens-before-source-selection.md) — The
  prompt-prefill and clone-defaults tests may open New before source selection; startup ordering is unverified.

- [`release-heading-can-close-review-page-script.md`](release-heading-can-close-review-page-script.md) — A malformed
  curated release heading can inject review-page script; attacker influence and practical impact are unverified.

- [`removal-destination-collapses-distinguishing-spaces.md`](removal-destination-collapses-distinguishing-spaces.md) —
  Removal/refusal destinations and manual startup paths collapse distinguishing spaces; live SSH examples, clipboard
  serialization and harmful startup reliance remain unverified.

- [`secondary-press-suppresses-primary-selection-copy.md`](secondary-press-suppresses-primary-selection-copy.md) — A
  second mouse button may suppress primary-selection copying; physical browser behavior is unverified.

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
  recording recurring baseline flakes, including further template, host-action and sidebar Replace observations; private
  runtime evidence was not authenticated here.

- [`numeric-version-guard-hides-artifact-filenames.md`](numeric-version-guard-hides-artifact-filenames.md) — Some bare
  artifact filenames lose file actions under the numeric-version guard; acceptance is ambiguous.

- [`host-dialog-reservation-does-not-exclude-settings.md`](host-dialog-reservation-does-not-exclude-settings.md) —
  Settings or the shortcuts overview during host-dialog admission may leave both dialogs inert until Escape; native
  ordering is unmeasured.

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

- [`offline-uninstall-live-tmux-history-unresolved.md`](offline-uninstall-live-tmux-history-unresolved.md) — The offline
  retry can omit the session check, but a supported history leaving the unit file gone, unit stopped, supervisor absent
  and private tmux alive remains unestablished. Active-unit repairs do not explicitly accept losing surviving sessions.

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
  naming and bolt-test saves can remain in flight after private-stack restoration, then overwrite it. Adverse scheduling
  and required incoming values remain unmeasured; no production user-work loss is claimed.

- [`auth-skew-remount-retains-row-operation-count.md`](auth-skew-remount-retains-row-operation-count.md) — After another
  tab restores shared credentials, a build-skew remount can leave the old GUI usable but keep an abandoned mutation
  count that blocks Restart. The conditional route needs an outstanding mutation, authentication loss, build change and
  another GUI.

- [`shortcut-armed-state-survives-auth-remount.md`](shortcut-armed-state-survives-auth-remount.md) — A sequence started
  inside New can reappear saying Enter apply after New is gone. A complete credential and input sequence turning that
  into a harmful launch remains unestablished; upgrade-filter coverage is only partial and uncertain.

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

- [`terminal-focus-origin-overwrites-header-focus.md`](terminal-focus-origin-overwrites-header-focus.md) — Delayed
  terminal-script loading and automatic fallback after a tab disappears can move focus from a surviving header button
  into the agent terminal. The source paths are established; harmful commands or approvals are not.

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

- [`startup-surviving-tab-adoption-source-gap.md`](startup-surviving-tab-adoption-source-gap.md) — Startup could adopt a
  surviving shell tab as the agent if the required agent-gone/tab-alive state existed. Ordinary Delete and tab creation
  did not establish that history; the conditional concern is not a reproduced loss of Resume.

- [`legacy-page-server-rebinding-source-gap.md`](legacy-page-server-rebinding-source-gap.md) — The legacy local static
  server omits Host/Origin validation. Supported browser admission and an actual confidential served page remain
  missing, so rebinding impact is unestablished. The instructed decision-page workflows were separately retired.

- [`startup-alternative-live-clone-pane-source-gap.md`](startup-alternative-live-clone-pane-source-gap.md) — Startup
  inspects one eligible pane's liveness. No ordinary supported producer was found for a pane-less row with a selected
  dead pane and another eligible live clone; the possible destructive route remains an incomplete source lead.

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
  unmeasured.

- [`devbuild-buffers-unauthenticated-metadata.md`](devbuild-buffers-unauthenticated-metadata.md) — A compromised release
  origin can provide oversized checksum or signature files that are downloaded and fully read before rejection. Material
  resource exhaustion is unmeasured; this is maintainer-tooling resource scope, not a demonstrated catastrophic failure.

- [`reselecting-terminal-does-not-restore-focus.md`](reselecting-terminal-does-not-restore-focus.md) — Keyboard
  activation of an already-selected session row or tab leaves typing focus on that button rather than returning it to
  the revealed terminal. The exact reselection policy is unspecified; no harmful input route is alleged.

- [`quick-switcher-pending-read-keeps-old-shortcut-labels.md`](quick-switcher-pending-read-keeps-old-shortcut-labels.md)
  — A successful but still-pending opening read can leave old shortcut labels visible in the switcher. Existing
  decisions accept failed reads and later moves, but this interval's coverage is ambiguous; selection still captures the
  displayed template name.

- [`tab-helper-can-fail-after-open-succeeds.md`](tab-helper-can-fail-after-open-succeeds.md) — A tab can pass its short
  initial liveness check before its helper fails to enter the folder, then disappear without the promised directory
  error. No wrong-directory command execution or work loss is established.

- [`spare-ancestry-fallback-skips-destination-recheck.md`](spare-ancestry-fallback-skips-destination-recheck.md) — A
  prepared-checkout ancestry refusal returns before the post-fetch destination ownership check, and fallback cloning can
  use a replacement empty folder. No existing-file loss is shown; the scope of the accepted check/move window is
  ambiguous.

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
  Installer tests can report leftover debris when the installer correctly retains its permanent fallback lock file on a
  filesystem without directory locking.

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

- [`terminal-skin-tone-width-parity-source-gap.md`](terminal-skin-tone-width-parity-source-gap.md) — A newer hand emoji
  with a skin-tone modifier may use fewer browser cells than tmux, displacing later output or the replayed cursor. Its
  exact shipped add-on classifications remain unverified.

- [`terminal-narrow-base-zwj-width-source-gap.md`](terminal-narrow-base-zwj-width-source-gap.md) — A one-cell base
  followed by ZWJ and a two-cell pictograph may leave tmux and the browser disagreeing about following cells. The
  shipped trie classification of a concrete failing sequence remains unestablished.

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

- [`selected-template-state-contrast.md`](selected-template-state-contrast.md) — Dark appearance gives the selected
  template a weak persistent state indicator; the approved palette's coverage remains ambiguous.

- [`selected-template-shortcut-contrast.md`](selected-template-shortcut-contrast.md) — A selected quick-launch
  template's keyboard hint keeps muted ink, leaving little contrast against its filled background.

- [`light-appearance-darkens-terminal-catchup.md`](light-appearance-darkens-terminal-catchup.md) — Light appearance
  darkens catch-up prose while its terminal background stays dark; display duration is unmeasured.

- [`selected-session-inverse-ink-on-light-controls.md`](selected-session-inverse-ink-on-light-controls.md) — A selected
  session's menu errors and disabled-command focus outline become white on white, and its actions toggle keeps white
  dots on pale hover/focus backgrounds.

- [`header-confirmation-lowers-link-refusal.md`](header-confirmation-lowers-link-refusal.md) — A header confirmation
  lowers a safely refused link's explanation beneath unrelated covers; exact overlap is unmeasured.

- [`effort-catalog-replaces-focused-control.md`](effort-catalog-replaces-focused-control.md) — An arriving model catalog
  can replace the focused effort buttons with a dropdown; the next key is consumed without restoring effort focus.

- [`delayed-appearance-event-overrides-local-choice.md`](delayed-appearance-event-overrides-local-choice.md) — An older
  cross-page notification can undo a newer local theme choice; browser frequency and filter coverage remain uncertain.

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

- [`status-sound-test-misses-reseed-transition.md`](status-sound-test-misses-reseed-transition.md) — The status-filter
  sound regression never supplies the audible edge needed to detect missing silent reseeding.

- [`completed-only-retry-keeps-host-warning.md`](completed-only-retry-keeps-host-warning.md) — Another client's success
  may leave an old host warning if every Running observation is missed; the end-to-end schedule remains unverified.

- [`reopened-rail-menu-loses-keyboard-handles.md`](reopened-rail-menu-loses-keyboard-handles.md) — Reopening the same
  host's still-mounted rail menu clears its keyboard-navigation handles until it is closed and reopened.

- [`hosts-panel-test-misses-filtered-row-premise.md`](hosts-panel-test-misses-filtered-row-premise.md) — The terminal
  preservation test never establishes that its selected session row was filtered out.

- [`host-removal-test-leaks-confirmation-preference.md`](host-removal-test-leaks-confirmation-preference.md) — A host
  removal test omits preference restoration; changed later behavior needs an unestablished incoming true value.

- [`hidden-progress-test-observes-unaffected-host.md`](hidden-progress-test-observes-unaffected-host.md) — The hidden
  management regression observes a menu already excluded by a separate row-order rule.

- [`narrow-sidebar-tests-measure-outer-container.md`](narrow-sidebar-tests-measure-outer-container.md) — Two narrow row
  tests retain containment bounds from the wider outer sidebar after shrinking only its content area.

- [`remove-retirement-may-release-later-dialog.md`](remove-retirement-may-release-later-dialog.md) — Old desktop Remove
  cleanup may release a replacement dialog's isolation; native ordering remains unverified, with no harmful action
  established.

- [`native-confirmation-reopen-paint-order-source-gap.md`](native-confirmation-reopen-paint-order-source-gap.md) — A
  pre-existing retained Delete flyout is focusable while unmeasured; an ordinary native sequence accepting an unseen
  answer remains unestablished.

- [`tunnel-spacing-shrinks-confirmation-buttons.md`](tunnel-spacing-shrinks-confirmation-buttons.md) — When the tunnel
  pill appears, Restart, Replace and Delete confirmation buttons also become narrower.

- [`session-navigation-interrupts-close-all.md`](session-navigation-interrupts-close-all.md) — Switching sessions while
  Close all waits for a response leaves later tunnel listeners open without attempting their closure or reporting that
  the batch was interrupted.

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

- [`short-launcher-notice-reachability.md`](short-launcher-notice-reachability.md) — An extremely short window can
  exhaust the launcher's notice-height budget; supported native sizing, actual reachability and recovery remain
  unverified after investigation.

- [`launcher-live-draft-submission-source-gap.md`](launcher-live-draft-submission-source-gap.md) — Submission reads
  current launcher choices, but a shipped sequence accepting a harmful action different from the one shown remains
  unverified after investigation. Fresh choice-and-Enter behavior is explicitly supported.

- [`folder-refusal-fixture-enters-version-skew.md`](folder-refusal-fixture-enters-version-skew.md) — A folder-dropdown
  test's refusal mock omits the build stamp, so later recovery checks run with a version-mismatch notice and live
  updates withdrawn. No actual test failure or false pass is claimed.

- [`launcher-target-hover-merges-direction-runs.md`](launcher-target-hover-merges-direction-runs.md) — The launcher
  target hover merges separately isolated host and folder text. Misleading grouping of ordinary right-to-left names
  remains unverified after investigation.

- [`tunnel-allow-keyboard-guard-source-gap.md`](tunnel-allow-keyboard-guard-source-gap.md) — The focused Allow tunnels
  button lacks the repeated/composing-Enter guard. A complete input sequence causing unintended consent remains
  unverified after investigation; Cancel-first focus and container Enter retain their separate protections.

- [`closed-conflicting-pr-does-not-wake-executor.md`](closed-conflicting-pr-does-not-wake-executor.md) — A waiting plan
  can remain unstarted after the last conflicting PR closes, until unrelated activity or a manual drain.

- [`old-executor-close-resets-new-holder-bookmarks.md`](old-executor-close-resets-new-holder-bookmarks.md) — An old
  executor's prescribed full-close cleanup can reset plan bookmarks already being used by the new holder.

- [`network-state-wal-failure-topology-unresolved.md`](network-state-wal-failure-topology-unresolved.md) — The WAL
  conversion has a conditional network-filesystem concern, but no supported topology producing the remaining persistent
  failure was established.

- [`muse-current-hook-override-semantics-unresolved.md`](muse-current-hook-override-semantics-unresolved.md) — Current
  Muse hook overrides may conflict with Farhelm's generated mapping; the actual vendor merge/duplicate behavior remains
  unverified.

- [`legacy-muse-hook-preservation-source-gap.md`](legacy-muse-hook-preservation-source-gap.md) — A broad legacy
  hook-preservation claim remains uncertain for an unrecognized override spelling and destructive vendor replacement
  behavior.

- [`muse-unmarked-same-process-child-attribution.md`](muse-unmarked-same-process-child-attribution.md) — An unmarked
  Muse child reporting from the same process might be mistaken for the session conversation; an ordinary vendor producer
  remains unestablished.

- [`short-trash-frame-reachability-unresolved.md`](short-trash-frame-reachability-unresolved.md) — A very short Trash
  window may leave its content or controls unreachable; actual supported geometry and recovery remain unverified.

- [`sentence-case-sweep-leaves-authored-lowercase-text.md`](sentence-case-sweep-leaves-authored-lowercase-text.md) —
  Several ordinary refusal messages and the no-alias label retain lowercase wording after the sentence-case change.

- [`composer-assertions-precede-session-cleanup.md`](composer-assertions-precede-session-cleanup.md) — A failed composer
  assertion can leave its already-created fake session running on the shared test stack.

- [`uuid-hold-intercepts-terminal-lease.md`](uuid-hold-intercepts-terminal-lease.md) — The destination-authority test's
  global UUID hold can catch late terminal startup instead of the intended launch key.

- [`catalog-retry-test-does-not-witness-inventory.md`](catalog-retry-test-does-not-witness-inventory.md) — The catalog
  retry regression can pass without proving that the retried catalog response was consumed.

- [`checkout-refusal-conflicts-with-failed-save-rule.md`](checkout-refusal-conflicts-with-failed-save-rule.md) — The
  general failed-Save discard rule does not explain the explicit checkout-refusal exception that retains the draft.

- [`omp-permission-docs-retain-yolo-default.md`](omp-permission-docs-retain-yolo-default.md) — An OMP comment and
  implementation specification still describe the old default permission mode after new launches changed to Guarded.

- [`name-regression-registers-cleanup-after-assertions.md`](name-regression-registers-cleanup-after-assertions.md) — A
  name-field test can fail after creating a session but before registering that session for cleanup.

- [`recent-setup-description-omits-wrapper-prefix.md`](recent-setup-description-omits-wrapper-prefix.md) — Different
  saved setups can look identical even though Enter restores and launches different wrapper prefixes.

- [`template-cleanup-skipped-after-preference-reset-fails.md`](template-cleanup-skipped-after-preference-reset-fails.md)
  — A failed test preference reset can skip deletion of that test's owned templates.

- [`malformed-add-acceptance-test-misses-retries.md`](malformed-add-acceptance-test-misses-retries.md) — The
  malformed-acceptance regression does not count the retry requests it claims to rule out.

- [`update-preplanning-retarget-consent-ambiguity.md`](update-preplanning-retarget-consent-ambiguity.md) — A delayed
  Update plan can freeze a host entry's newer destination while the requesting GUI still shows its previous destination;
  whether that consent is sufficient is unresolved.

- [`uninstall-test-restores-before-run-settles.md`](uninstall-test-restores-before-run-settles.md) — The uninstall
  progress test can restore its backend and registry while its accepted uninstall is still active.

- [`tooltip-focus-test-lacks-installation-witness.md`](tooltip-focus-test-lacks-installation-witness.md) — Tooltip focus
  assertions can run before the tooltip listener is installed.

- [`outage-recovery-test-can-credit-ordinary-probe.md`](outage-recovery-test-can-credit-ordinary-probe.md) — The outage
  regression's observed probe may come from normal polling rather than the recovery nudge it claims to test.

- [`welcome-help-location-is-stale.md`](welcome-help-location-is-stale.md) — Welcome directs users to the top of the
  sidebar for Help after Help moved to the footer.

- [`sort-test-cleanup-stops-after-first-failure.md`](sort-test-cleanup-stops-after-first-failure.md) — One cleanup
  failure can leave the sort test's remaining owned sessions running until wider teardown.

- [`older-muse-restart-refusal-blames-command-mode.md`](older-muse-restart-refusal-blames-command-mode.md) — Plain
  Restart can describe an older Muse agent launch as a command launch with reporting disabled.

- [`header-copy-warning-survives-different-copy.md`](header-copy-warning-survives-different-copy.md) — Copying a
  different header field can leave the earlier field's hidden-character warning visible briefly.

- [`switcher-second-create-failure-leaks-first-session.md`](switcher-second-create-failure-leaks-first-session.md) — If
  a second setup create fails, the switcher test can skip cleanup of the first session it already acquired.

- [`transfer-result-height-can-hide-destination.md`](transfer-result-height-can-hide-destination.md) — A long transfer
  result may hide the saved destination or refusal without an expansion or scroll control.

- [`attachment-failure-announcement-unresolved.md`](attachment-failure-announcement-unresolved.md) — Attachment errors
  have no explicit live announcement, so screen-reader users may miss a failed paste without navigating to its message.

- [`early-empty-status-filter-stays-loading.md`](early-empty-status-filter-stays-loading.md) — An early empty
  status-filter reply can leave the main pane saying Loading sessions until the user changes views.

- [`container-tab-close-overpromises-process-termination.md`](container-tab-close-overpromises-process-termination.md) —
  The tab Close warning and internal contract say all started processes end, although detached container work can
  survive.

- [`fake-spawn-cleanup-can-leave-launch-policy-enabled.md`](fake-spawn-cleanup-can-leave-launch-policy-enabled.md) — A
  failed fake-spawn cleanup can skip restoration of shared launch policies.

- [`spawn-test-publishes-child-after-observations.md`](spawn-test-publishes-child-after-observations.md) — A spawned
  test child can escape teardown registration when an intervening assertion fails.

- [`host-permission-copy-quotes-retired-labels.md`](host-permission-copy-quotes-retired-labels.md) — Specifications and
  helm refusal text quote old host-permission labels rather than the controls users now see.

- [`backup-warning-can-imply-inconsistent-live-copy.md`](backup-warning-can-imply-inconsistent-live-copy.md) — The
  live-WAL backup warning may read as a complete copying recipe even though uncoordinated copies can be inconsistent.

- [`omp-npx-guide-omits-tracking-qualification.md`](omp-npx-guide-omits-tracking-qualification.md) — The OMP guide
  overstates tracking support for npx launches without the supported launch-chain qualification.

- [`feedback-guide-overpromises-window-close-survival.md`](feedback-guide-overpromises-window-close-survival.md) — The
  guide says feedback sending survives closing the window, although closing the last desktop window ends its embedded
  helm.

- [`settings-guide-misstates-device-local-preferences.md`](settings-guide-misstates-device-local-preferences.md) — The
  guide singles out appearance as device-local even though Sounds is device-local too.

- [`trash-question-reopen-skips-answer-focus.md`](trash-question-reopen-skips-answer-focus.md) — Reopening the same
  visible Trash question can leave keyboard focus on its opener, so Enter reopens it again instead of reaching an
  answer.

- [`host-menu-requested-index-survives-command-removal.md`](host-menu-requested-index-survives-command-removal.md) —
  Keyboard navigation can stay on the wrong command position after Adopt or Update disappears from an open host menu.

- [`light-native-select-keeps-dark-fill.md`](light-native-select-keeps-dark-fill.md) — On the inspected Linux
  WebKit/Adwaita path, Light template dropdowns combine dark text with a dark native background.

- [`alias-propagation-test-reloads-before-live-row-check.md`](alias-propagation-test-reloads-before-live-row-check.md) —
  The alias regression reloads the page before checking the row name, so it misses the promised live update behavior.

- [`zoom-cleanup-failure-skips-second-session.md`](zoom-cleanup-failure-skips-second-session.md) — A cleanup refusal can
  leave the zoom/remount test's remaining session running until wider teardown.

- [`terminal-size-readout-points-css-pixels-conflict.md`](terminal-size-readout-points-css-pixels-conflict.md) — The new
  terminal size readout is documented as points while its value reports CSS pixels.

- [`browser-shard-failure-sweeps-running-siblings.md`](browser-shard-failure-sweeps-running-siblings.md) — An early
  shard exception can clean up workers that sibling shards are still using.

- [`browser-shard-failures-exit-successfully.md`](browser-shard-failures-exit-successfully.md) — The browser harness can
  exit successfully even when its shards report validation failures.

- [`worker-evidence-deleted-after-download-failure.md`](worker-evidence-deleted-after-download-failure.md) — The harness
  can delete a failed worker's evidence even when downloading that evidence failed.

- [`worker-archive-omits-playwright-artifact-directory.md`](worker-archive-omits-playwright-artifact-directory.md) — The
  worker archive can omit the screenshots, traces and timelines Playwright writes outside its archived directory.

- [`worker-template-acceptance-ignores-prerequisite-failures.md`](worker-template-acceptance-ignores-prerequisite-failures.md)
  — Automated template acceptance can welcome a worker despite failed manager or SSH prerequisites.

- [`worker-checkpoint-polling-has-no-failure-exit.md`](worker-checkpoint-polling-has-no-failure-exit.md) — Worker
  checkpoint polling can wait indefinitely for completion; the exact service transition needed for the suspected
  permanent wait remains unresolved.

- [`worker-allocation-precedes-ownership-ledger.md`](worker-allocation-precedes-ownership-ledger.md) — A created worker
  can fall outside cleanup when ledger writing fails or remote creation succeeds before the local request times out.

- [`confirmation-rollback-omits-container-cleanup.md`](confirmation-rollback-omits-container-cleanup.md) — A failed
  final create confirmation can remove a borrowed-checkout session row without performing its container cleanup; an
  ordinary uncovered failure producer remains missing.

- [`replace-container-ready-wording-exceeds-create-boundary.md`](replace-container-ready-wording-exceeds-create-boundary.md)
  — Container Replace text promises a ready replacement container before deleting the source, while code waits only for
  the new session/terminal boundary.

- [`creator-fence-ends-before-podman-allocation.md`](creator-fence-ends-before-podman-allocation.md) — A terminal
  interrupt can release the creation fence while Podman still registers a container, leaving late storage after Stop.

- [`rootless-pause-inherits-session-marker.md`](rootless-pause-inherits-session-marker.md) — The rootless container
  infrastructure process can inherit a session marker and be selected for unnecessary termination or recreation.

- [`early-retry-refusal-orphans-container-storage.md`](early-retry-refusal-orphans-container-storage.md) — A refused
  pending-create retry can discard its session handle while an earlier container's storage remains.

- [`anonymous-container-volumes-survive-removal.md`](anonymous-container-volumes-survive-removal.md) — Removing a
  container can leave its anonymous image volumes behind; whether Farhelm promises to remove them is unresolved.

- [`negated-isolation-probes-cannot-fail-test.md`](negated-isolation-probes-cannot-fail-test.md) — The negative loopback
  probes can pass through a failed denial assertion and still report the test successful.

- [`pending-retry-generation-rationale-is-inaccurate.md`](pending-retry-generation-rationale-is-inaccurate.md) — The
  retry rationale and regression model a generation change that real pending-create retries do not make.

- [`checkout-root-reopen-lacks-identity-comparison.md`](checkout-root-reopen-lacks-identity-comparison.md) — A file
  download reopens the checkout pathname after verifying it; a complete uncancelled delivery through replacement remains
  unestablished.

- [`template-container-off-on-discards-editor-values.md`](template-container-off-on-discards-editor-values.md) — Turning
  a template's container option off and back on loses its image, port and token-path draft values before Save.

- [`image-tmpdir-stages-skeleton-inside-checkout.md`](image-tmpdir-stages-skeleton-inside-checkout.md) — An
  image-selected temporary directory can put account-preparation scratch files inside the checkout despite the stated
  exclusion.

- [`interrupted-image-inspection-leaves-unnamed-allocation.md`](interrupted-image-inspection-leaves-unnamed-allocation.md)
  — An interrupted account inspector can leave container storage allocated before its automatic-removal cleanup becomes
  effective.

- [`podman-remote-routing-host-contract-ambiguity.md`](podman-remote-routing-host-contract-ambiguity.md) — Ambient
  Podman routing can choose a remote engine despite Farhelm's host-local container description; the intended
  configuration boundary is unresolved.

- [`inspector-workdir-hides-missing-home.md`](inspector-workdir-hides-missing-home.md) — Image inspection can create the
  selected home path as its WORKDIR and then incorrectly decide that normal home preparation is unnecessary.

- [`container-test-panic-cleanup-precedes-late-creator.md`](container-test-panic-cleanup-precedes-late-creator.md) — A
  failed test's cleanup can run before its pending creator finishes, but the complete directory/allocation schedule
  remains unverified.

- [`account-assertion-chain-can-reach-false-readiness.md`](account-assertion-chain-can-reach-false-readiness.md) — A
  shell helper can continue after a failed intermediate account/home assertion and still publish its ready marker.

- [`checkout-fixture-inherits-git-repository-selector.md`](checkout-fixture-inherits-git-repository-selector.md) — An
  inherited GIT_DIR can redirect a managed-checkout test's initialization into another fixture repository.

- [`update-reminder-before-initial-provisioning-read.md`](update-reminder-before-initial-provisioning-read.md) — An
  Update reminder can advertise availability before the first provisioning-state read has established whether that
  action is eligible.

- [`completed-followup-leaves-old-update-tracked-busy.md`](completed-followup-leaves-old-update-tracked-busy.md) — If
  the GUI misses a successor's Running state and first sees Completed, an earlier tracked Update can remain busy.

- [`development-peer-version-age-wording-is-reversed.md`](development-peer-version-age-wording-is-reversed.md) — The
  automatic-update specification says development peers have unknown age, while the implementation treats a development
  helm as the special case.

- [`automatic-update-planning-serializes-hosts.md`](automatic-update-planning-serializes-hosts.md) — One slow host's
  planning can delay automatic updates for later hosts even though each individual wait is bounded.

- [`finished-task-handle-published-after-host-purge.md`](finished-task-handle-published-after-host-purge.md) — A
  completed uninstall task can leave bookkeeping reinserted after the host was already removed.

- [`unknown-policy-write-overtaking-unresolved.md`](unknown-policy-write-overtaking-unresolved.md) — An uncertain opt-in
  might outlive a later opt-out, but a concrete older request delayed before policy-lock admission remains missing.

- [`queued-preference-survives-signin-expiry.md`](queued-preference-survives-signin-expiry.md) — A queued preference can
  survive one tab's sign-in expiry and later commit using another tab's refreshed credential.

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

- [`rail-tooltip-merges-direction-boundaries.md`](rail-tooltip-merges-direction-boundaries.md) — A composite rail
  tooltip can group host and destination text differently from their separately isolated visible values; actual
  right-to-left rendering remains unverified.

- [`keyboard-overview-omits-in-sequence-number-action.md`](keyboard-overview-omits-in-sequence-number-action.md) — The
  keyboard overview calls number chords Launch without explaining that, in a typed sequence, they commit the unfinished
  word and queue the binding.

- [`physical-tab-crosses-catalog-readiness-unresolved.md`](physical-tab-crosses-catalog-readiness-unresolved.md) — A
  physical Tab press might be handled after catalog readiness changes, rather than under the advisory state present when
  it was pressed.

- [`digit-leading-template-limit-overstated.md`](digit-leading-template-limit-overstated.md) — The stated inability to
  type digit-leading template names is false on shifted-digit keyboard layouts.

- [`initial-shortcut-replay-lacks-busy-release-wake.md`](initial-shortcut-replay-lacks-busy-release-wake.md) — An
  initial replay that observes busy state has no explicit wake when that state clears; an ordinary overlapping input
  producer remains missing.

- [`shortcut-overview-pending-read-presentation-ambiguity.md`](shortcut-overview-pending-read-presentation-ambiguity.md)
  — The shortcut overview can show cached bindings while its fresh opening read is pending; the required first-visible
  policy is unresolved.

- [`back-to-back-shortcut-test-misses-first-batch.md`](back-to-back-shortcut-test-misses-first-batch.md) — The
  back-to-back shortcut regression can pass even if the first batch is dropped.

- [`expanded-sidebar-minimum-width-conflicts-with-controls.md`](expanded-sidebar-minimum-width-conflicts-with-controls.md)
  — The narrowest expanded sidebar leaves less room than its header and Hosts-row controls require; actual clipping and
  keyboard recovery remain unverified.

- [`newline-initial-login-directory-transformation.md`](newline-initial-login-directory-transformation.md) — A
  newline-bearing folder can be transformed before the initial login shell starts; a harmful supported startup operation
  remains unestablished.

- [`late-rail-restoration-retains-menu-coordinates.md`](late-rail-restoration-retains-menu-coordinates.md) — A host menu
  opened before saved expansion finishes restoring may remain at the host's former position.

- [`full-host-tooltip-conflicts-with-line-cap.md`](full-host-tooltip-conflicts-with-line-cap.md) — The promise to reveal
  full host names conflicts with the shared tooltip's eight-line limit; actual clipping remains unmeasured.

- [`selected-expanded-host-hover-white-on-white.md`](selected-expanded-host-hover-white-on-white.md) — In Light
  appearance, hovering the selected expanded host can make its name and destination unreadable.

- [`signal-test-child-lacks-unwind-cleanup.md`](signal-test-child-lacks-unwind-cleanup.md) — A failed signal-log
  regression can leave its owned sleeper running; the runner's leak report does not prove the child was cleaned up.

- [`replacement-cleanup-missing-pane-root-source-gap.md`](replacement-cleanup-missing-pane-root-source-gap.md) — Failed
  replacement cleanup without a pane root has a conditional orphaning concern, but a supported uncovered producer
  remains missing.

- [`bash-source-admission-retains-literal-splitter.md`](bash-source-admission-retains-literal-splitter.md) — Some valid
  Bash command source is refused despite the product's Bash-source promise.

- [`template-save-placeholder-validation-contract-conflict.md`](template-save-placeholder-validation-contract-conflict.md)
  — Template Save can retain a prefix that Launch later refuses, despite wording that the new placement check runs at
  Save time.

- [`brace-literal-command-display-is-bash-inexact.md`](brace-literal-command-display-is-bash-inexact.md) — The displayed
  command can give a brace-bearing literal suffix a different apparent Bash meaning from the actual argument.

- [`wrapper-test-setup-bypasses-container-cleanup-guard.md`](wrapper-test-setup-bypasses-container-cleanup-guard.md) — A
  failed wrapper-container setup can leave its real keeper outside the test's cleanup guard.

- [`unscoped-tab-log-says-scoped.md`](unscoped-tab-log-says-scoped.md) — A tab running without a system scope can still
  be logged as scoped merely because a candidate unit name exists.

- [`refused-add-rewrites-new-destination-owner.md`](refused-add-rewrites-new-destination-owner.md) — A racing ADD can
  change another busy host entry's install paths before returning Busy.

- [`cleanup-stop-transport-error-skips-delete.md`](cleanup-stop-transport-error-skips-delete.md) — A transport error
  from a cleanup Stop can prevent Delete even when the helper already knows the fake session's ID.

- [`typed-name-matching-repeats-quadratic-prefix-work.md`](typed-name-matching-repeats-quadratic-prefix-work.md) — Typed
  template matching performs quadratic duplicate checks before applying the five-row display cap; practical
  supported-scale latency remains unmeasured.

- [`podman-test-owner-death-leaves-keeper.md`](podman-test-owner-death-leaves-keeper.md) — Abrupt death of a real-Podman
  test owner can leave its detached keeper outside cleanup.

- [`unapproved-ssh-help-copy-receiver-contract-unresolved.md`](unapproved-ssh-help-copy-receiver-contract-unresolved.md)
  — An unapproved agent Add-host draft can copy raw SSH help text; the required warning and harmful downstream paste
  remain unestablished.

- [`creator-revocation-test-lacks-wait-boundary-witness.md`](creator-revocation-test-lacks-wait-boundary-witness.md) —
  The delayed-creator revocation regression can release its gate without proving the creator actually reached the queued
  lock boundary.

- [`discovery-registration-bypasses-host-row-lock.md`](discovery-registration-bypasses-host-row-lock.md) — Discovery
  registration can bypass a host row's execution lock, but a supported differing-path transition with lasting
  consequences remains missing.

- [`ordinary-menu-opening-focus-before-visible-placement.md`](ordinary-menu-opening-focus-before-visible-placement.md) —
  An ordinary menu opening may accept keyboard input before its measured placement becomes visible; the native
  input/paint sequence remains unverified.

- [`muse-capability-docstring-denies-reporting-and-resume.md`](muse-capability-docstring-denies-reporting-and-resume.md)
  — A capability docstring still says Muse lacks reporting and Resume command support although the current integration
  provides both.

- [`named-refusal-test-misses-selector-editability.md`](named-refusal-test-misses-selector-editability.md) — The
  named-create refusal regression checks a retained value without proving that users can still edit and retry it.

- [`failed-template-save-retains-attempted-draft.md`](failed-template-save-retains-attempted-draft.md) — A failed
  template Save leaves the attempted draft reusable even though the general GUI rule says failed saves discard it.

- [`take-control-skips-recovery-terminals.md`](take-control-skips-recovery-terminals.md) — Taking control in a new tab
  during a build mismatch can leave existing terminals in Recovery until Reconnect now is used.

- [`omp-readiness-comment-excludes-early-save.md`](omp-readiness-comment-excludes-early-save.md) — An OMP readiness
  comment says saving requires the first reply, although the current integration can persist a newly created
  conversation earlier.

- [`failed-screen-capture-can-forget-working-state.md`](failed-screen-capture-can-forget-working-state.md) — Capture
  failure can fall back to Idle after repeated unchanged screens; an ordinary busy Muse sequence causing an unaccepted
  Restart remains unestablished.

- [`automatic-update-mutates-unadopted-skewed-installation.md`](automatic-update-mutates-unadopted-skewed-installation.md)
  — Automatic Update can change a replacement installation at the saved SSH destination before discovering its new
  identity and refusing attachment.

- [`container-template-replacement-test-reuses-old-config.md`](container-template-replacement-test-reuses-old-config.md)
  — The template regression cannot detect keeping the old container settings because its replacement uses identical
  values.

- [`manual-unroute-leaves-listing-callback-running.md`](manual-unroute-leaves-listing-callback-running.md) — A test can
  remove a route without draining its active listing callback, letting that callback outlive assertions and fixture
  deletion.
