# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

## Highest priority: security or data loss

- [`tunnel-reuses-custom-helm-credential-origin.md`](tunnel-reuses-custom-helm-credential-origin.md) — A desktop tunnel
  can receive a previously used custom browser helm port and expose its retained device credential to remote HTML in the
  same browser profile. The default helm port and embedded desktop credentials are outside this conditional route.

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

- [`late-bound-agent-host-misses-adoption.md`](late-bound-agent-host-misses-adoption.md) — If an interrupted terminal
  starts only after its replacement supervisor finishes both startup scans, the agent can keep running while Farhelm
  cannot open its terminal. Another supervisor restart is needed to find it.

- [`retired-runtime-retains-restart-seed-reply.md`](retired-runtime-retains-restart-seed-reply.md) — An input/paste
  reserved before retirement can remain retained with no receipt; the same late-send lifetime on the runtime's control
  channel can also leave an accepted Restart waiting for a history-seed reply after the runtime has ended.

- [`extreme-history-numbering-hangs-selection.md`](extreme-history-numbering-hangs-selection.md) — Extreme peer-supplied
  history numbering can hang the page when the user selects text in a later screen row. The record now supplies its
  missing hostile-host acceptance context and preserves the unresolved browser scope.

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
  a classic mouse-report snapshot may leave web clicks inert; native behavior is unverified. Additional reviewed scope:
  After copying a forced local selection from a frozen terminal snapshot whose program enabled mouse reporting, ordinary
  file-link clicks can keep doing nothing until the selection is explicitly cleared.

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
  recording recurring baseline flakes, including quick-launch and further browser observations. Private browser evidence
  remains unauthenticated; inspected Rust failures do not establish latent flakiness.

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
  a second setup create fails, the switcher test can skip cleanup of the first session it already acquired. Additional
  reviewed scope: A failed second setup create can leave the migration test's first fake-agent session running until a
  later shared-stack reset or teardown.

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
  leave the zoom/remount test's remaining session running until wider teardown. Additional reviewed scope: Failure
  cleaning one fake-agent test session prevents any cleanup attempt for the other.

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
  — Automated template acceptance can welcome a worker despite failed SSH readiness; manager readiness now gates it.

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
  inherited GIT_DIR can redirect a managed-checkout test's initialization into another fixture repository. Additional
  reviewed scope: Running the new unit-test fixture with an inherited absolute Git repository selector can initialize
  and mutate a repository outside its temporary fixture.

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

- [`signal-test-child-lacks-unwind-cleanup.md`](signal-test-child-lacks-unwind-cleanup.md) — Failed signal-log,
  supervisor and terminal-host fixtures can leave owned processes running before cleanup ownership is armed. Additional
  reviewed scope: A failed regression setup can leave its stand-in shell running after the test exits.

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

- [`legacy-quickstart-stops-surviving-supervisor.md`](legacy-quickstart-stops-surviving-supervisor.md) — The parked
  quickstart contradicts itself about whether the supervisor survives quitting the Mac app.

- [`inherited-lock-test-self-dup.md`](inherited-lock-test-self-dup.md) — A terminal-host regression can pass without
  inheriting its intended version lock if the opened file already has descriptor 8; that allocation was not observed.

- [`desktop-owner-record-lacks-boot-domain.md`](desktop-owner-record-lacks-boot-domain.md) — A Linux desktop development
  build can recognize another boot's coincident process identity as its own and stop it during replacement. A qualifying
  user-work-loss route remains unestablished; this is not a macOS claim.

- [`literal-deleted-suffix-breaks-desktop-owner.md`](literal-deleted-suffix-breaks-desktop-owner.md) — A development
  executable literally ending in ` (deleted)` can lose ownership recognition and leave an older answering build reused.

- [`desktop-smoke-supervisor-precedes-cleanup-record.md`](desktop-smoke-supervisor-precedes-cleanup-record.md) —
  Interruption before desktop-test ownership publication can leave a detached supervisor outside cleanup; timing and
  survivor duration are unmeasured, and an older browser-helper discard's broader intent is ambiguous.

- [`failed-client-close-test-lacks-writer-premise.md`](failed-client-close-test-lacks-writer-premise.md) — The
  failed-client close regression can pass before its continuously sending peer runs, missing the intended receive bound.

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

- [`migrated-session-tab-kind-contract-conflict.md`](migrated-session-tab-kind-contract-conflict.md) — The authoritative
  implementation specification contradicts itself about preserving an existing session's old terminal tabs when its
  agent moves into a terminal host.

- [`stderr-readiness-lacks-live-holder-premise.md`](stderr-readiness-lacks-live-holder-premise.md) — The regression
  intended to keep terminal opens from waiting on unrelated stderr holders can report success without its held-open-pipe
  premise.

- [`late-listener-blocks-start-test-cleanup.md`](late-listener-blocks-start-test-cleanup.md) — A delayed test listener
  can turn the cleanup regression into a five-minute runner timeout instead of a bounded fixture failure.

- [`staged-start-test-lacks-reader-drain-witness.md`](staged-start-test-lacks-reader-drain-witness.md) — The new
  regression test can pass without protecting the boundary that prevents a terminal from starting before its owner is
  recorded.

- [`container-host-test-lacks-execution-location-witness.md`](container-host-test-lacks-execution-location-witness.md) —
  The terminal-host container regression can pass when its fake agent runs outside the container, provided the separate
  keeper still starts and stops. No production isolation failure is established.

- [`mac-process-lookup-truncates-live-host-command.md`](mac-process-lookup-truncates-live-host-command.md) — On a Mac
  with an oversized process listing, test cleanup can erase a stopped host's live socket and state directory, leaving
  that host unreachable for future cleanup.

- [`mac-process-lookup-transforms-socket-path.md`](mac-process-lookup-transforms-socket-path.md) — Mac test cleanup can
  remove a stopped terminal host's live socket and directory under certain valid injected paths, leaving the host
  unreachable for future cleanup.

- [`blocking-test-cleanup-prevents-async-listener-release.md`](blocking-test-cleanup-prevents-async-listener-release.md)
  — A normal successful supervisor unit test waits for the five-second host-cleanup deadline and retains its state
  directory because directory teardown blocks the same executor needed to release its aborted stand-in listener.

- [`readiness-test-does-not-establish-first-stage-exit.md`](readiness-test-does-not-establish-first-stage-exit.md) — The
  regression can pass while a terminal-start helper remains alive, leaving its claimed lifecycle guarantee untested.

- [`input-test-comments-overstate-termination-coverage.md`](input-test-comments-overstate-termination-coverage.md) — The
  test comments claim stronger coverage and transport guarantees than the tests establish.

- [`replay-order-comments-confuse-caller-scheduling.md`](replay-order-comments-confuse-caller-scheduling.md) — The
  implementation notes misstate the acknowledgement/replay ordering that protects healthy terminal attachments from
  false stalls.

- [`stream-stall-test-lacks-blocked-send-witness.md`](stream-stall-test-lacks-blocked-send-witness.md) — The
  byte-terminal regression can pass without testing teardown during a blocked browser send.

- [`steady-block-cursor-contract-conflict.md`](steady-block-cursor-contract-conflict.md) — Grid terminals deliberately
  blink a block cursor even when the program requests it steady, while the wire contract says blinking is the default
  unless the program says otherwise.

- [`unavailable-host-recipe-misses-fourth-disconnect.md`](unavailable-host-recipe-misses-fourth-disconnect.md) — The
  maintained follow-up asks for three disconnects, but a healthy test host reconnects after all three. A test following
  that recipe can fail before reaching the intended unavailable state.

- [`explicit-hostname-title-normalized-as-unset.md`](explicit-hostname-title-normalized-as-unset.md) — Emulator parity
  validation can fail on machines named codex or workspace even when tmux and the emulator show the same explicitly set
  title.

- [`malformed-pane-fixtures-mask-intended-validators.md`](malformed-pane-fixtures-mask-intended-validators.md) — The
  malformed-pane regression can pass even if earlier validation regresses, reducing confidence in the pane-fact parser.

- [`tmux-history-seed-loses-redefined-palette.md`](tmux-history-seed-loses-redefined-palette.md) — Restarting an old
  tmux session into a terminal host changes retained scrollback colors when the old program redefined palette entries.

- [`tmux-capture-capacity-exceeds-memory-estimate.md`](tmux-capture-capacity-exceeds-memory-estimate.md) — Restart
  migration of dense styled history can retain a capture allocation near 128 MiB despite the documented roughly 64 MiB
  capture peak, adding avoidable supervisor memory pressure while history is encoded.

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

- [`style-fixture-overwrites-strikeout-sample.md`](style-fixture-overwrites-strikeout-sample.md) — The compatibility
  styles fixture overwrites its strikeout sample. A later independent frozen fixture repairs the missing coverage; that
  status is preserved for explicit cleanup.

- [`peak-memory-test-lacks-baseline-headroom.md`](peak-memory-test-lacks-baseline-headroom.md) — The memory tests may
  hide an excessive checkpoint allocation below an earlier process memory peak. After source investigation, the physical
  headroom needed for an actual false pass in these fixtures remains unestablished.

- [`malformed-snapshot-fixtures-mask-intended-validators.md`](malformed-snapshot-fixtures-mask-intended-validators.md) —
  Several terminal-snapshot corruption fixtures never exercise the validation they name, so they remain green after
  those checks regress.

- [`recording-loader-rejects-valid-json-field-spacing.md`](recording-loader-rejects-valid-json-field-spacing.md) — A
  developer can provide a private terminal recording in the documented JSON-lines format and have the snapshot test
  reject it before checking the recording.

- [`host-lock-table-contradicts-startup-span.md`](host-lock-table-contradicts-startup-span.md) — Farhelm's maintenance
  documentation states a lifecycle-lock constraint that startup deliberately violates, obscuring which recovery
  operation may perform host I/O while holding the lock.

- [`grid-input-omits-x10-only-mouse-reporting.md`](grid-input-omits-x10-only-mouse-reporting.md) — Programs that request
  only classic X10 mouse tracking lose clicks in terminal-host terminals, unlike the live raw-stream browser terminal.

- [`migration-note-dismissal-loses-terminal-focus.md`](migration-note-dismissal-loses-terminal-focus.md) — After closing
  the old-terminal note from the keyboard, ordinary typing no longer reaches the already-revealed terminal until the
  user refocuses it.

- [`tab-kind-projection-repeats-quadratic-scans.md`](tab-kind-projection-repeats-quadratic-scans.md) — Opening or
  refreshing a session with many shell tabs repeatedly scans the entire tab list just to obtain each tab's old/new
  terminal label.

- [`refused-input-test-can-credit-earlier-bytes.md`](refused-input-test-can-credit-earlier-bytes.md) — The terminal
  input-backpressure regression's claim that rejected input never reaches the host is stronger than what it observes.

- [`stale-reply-test-lacks-processing-witness.md`](stale-reply-test-lacks-processing-witness.md) — The regression for
  cancelling a terminal-host Info request does not prove that a late reply cannot satisfy the next request.

- [`shift-enter-test-lacks-reader-readiness.md`](shift-enter-test-lacks-reader-readiness.md) — This integration test can
  report success without establishing the already-waiting reader its documentation requires.

- [`hosts-directory-replacement-stop-uncertainty.md`](hosts-directory-replacement-stop-uncertainty.md) — A changed
  directory could redirect terminal shutdown, but no supported history replacing it with another installation's matching
  live socket was established. Independent review does not agree to drop that remaining uncertainty; deliberate
  private-directory interference is separately accepted.

- [`process-presence-fixture-leaves-sleep-descendant.md`](process-presence-fixture-leaves-sleep-descendant.md) — Running
  the process-presence unit test can leave its sleeping helper behind even when the test passes.

- [`host-browser-setup-precedes-startup-session-create.md`](host-browser-setup-precedes-startup-session-create.md) — A
  healthy host-terminal browser-validation run can fail before its startup session exists.

- [`repeated-typing-test-can-credit-old-echo.md`](repeated-typing-test-can-credit-old-echo.md) — Repeating the dedicated
  host-terminal typing test can report success from an earlier test's screen without proving that this invocation
  delivered its typing.

- [`copied-sandbox-runner-relative-import-fails.md`](copied-sandbox-runner-relative-import-fails.md) — A maintainer
  using the runner's existing alternate-repository option from a copied script can no longer start it, including its
  cleanup-only sweep command, unless the copy also has a checkout-shaped e2e directory beside it.

- [`desktop-smoke-misparses-space-bearing-run-path.md`](desktop-smoke-misparses-space-bearing-run-path.md) — Choosing a
  short fresh DESKTOP_SMOKE_DIR containing a space makes the gate report that a healthy created session has no live
  terminal.

- [`reconnect-replay-keeps-obsolete-event-count-fence.md`](reconnect-replay-keeps-obsolete-event-count-fence.md) — After
  a resize write interrupts replay and reconnects, Farhelm can keep refusing prompts as still restoring even after the
  terminal has caught up. More output or replacement of that runtime can clear it.

- [`checkpoint-rejection-test-asserts-before-processing.md`](checkpoint-rejection-test-asserts-before-processing.md) —
  The checkpoint regression can pass if the supervisor wrongly records a rejected checkpoint as accepted.

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

- [`empty-history-queue-bypasses-bounds.md`](empty-history-queue-bypasses-bounds.md) — Empty deferred-history messages
  escape the row cap and an accumulated backlog would bypass yielding. The structural gap is confirmed; material backlog
  or resource exhaustion remains unverified after investigation, and ordinary senders were not found. The record
  supplies its missing acceptance context without settling browser scope.

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

- [`secondary-button-erases-intercepted-primary-ownership.md`](secondary-button-erases-intercepted-primary-ownership.md)
  — Holding a web-link click and pressing a second mouse button loses the primary click's interception, so its
  subsequent drag/release reaches the terminal program and its original link no longer opens.

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

- [`initial-oversized-grid-cannot-attach.md`](initial-oversized-grid-cannot-attach.md) — A terminal fitted to over1000
  cells can fail to open repeatedly because its initial dimensions exceed the grid stream's bound.

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

- [`tab-reap-deadline-excludes-listing-request.md`](tab-reap-deadline-excludes-listing-request.md) — The tab-exit
  regression's advertised 15-second failure boundary does not cover the listing it awaits.

- [`tab-retry-test-measures-observation-time.md`](tab-retry-test-measures-observation-time.md) — The retry regression
  can accept cleanup observed after 900 ms without proving that the retry itself waited for the required delay.

- [`last-output-fixture-readiness-precedes-trap.md`](last-output-fixture-readiness-precedes-trap.md) — The final-output
  regression can reject correct output preservation because its agent has published readiness before installing the
  exit-output trap.

- [`held-start-test-lacks-prelaunch-fencing-witness.md`](held-start-test-lacks-prelaunch-fencing-witness.md) — The
  regression can pass when the next agent launches before an earlier outstanding first stage is fenced, so it does not
  protect the before-launch ordering its assertion claims.

- [`host-preservation-test-can-select-starter-pid.md`](host-preservation-test-can-select-starter-pid.md) — The
  reserved-session preservation regression can blame correct startup cleanup for a fixture launcher exiting normally,
  because its readiness check can save that temporary launcher as the terminal-host process.

- [`keeper-can-win-one-shot-interrupted-start-gate.md`](keeper-can-win-one-shot-interrupted-start-gate.md) — Two
  startup-reclaim regression tests lack the ordering needed to hold their intended first launch; a keeper launch can
  take the gate instead and fail setup before reclamation is tested.

- [`restart-test-types-before-safe-prompt-barrier.md`](restart-test-types-before-safe-prompt-barrier.md) — The restart
  tests can overwrite their startup marker with early test input and then report a healthy supervisor restart as having
  lost terminal contents.

- [`migration-switch-test-accepts-dead-old-agent.md`](migration-switch-test-accepts-dead-old-agent.md) — The update
  regression can pass if the old running agent dies during supervisor stop/start, so it fails to test its stated
  keep-working guarantee.

- [`retry-capture-test-cannot-distinguish-smaller-read.md`](retry-capture-test-cannot-distinguish-smaller-read.md) — The
  migration regression can pass if a timed-out history capture is retried at its original size, leaving the promised
  smaller retry untested.

- [`oversized-pane-fixture-pulls-history-onto-screen.md`](oversized-pane-fixture-pulls-history-onto-screen.md) — The
  oversized-terminal regression claims to preserve existing scrollback but moves its entire short scrollback onto the
  visible screen before Restart, so it misses the positive-scrollback case it says it exercises.

- [`reaping-test-mistakes-missing-sink-for-complete-cleanup.md`](reaping-test-mistakes-missing-sink-for-complete-cleanup.md)
  — The migration regression cannot detect incomplete old-output-client reaping through the predicate it calls its reap
  check.

- [`replacement-history-absence-asserted-before-delivery.md`](replacement-history-absence-asserted-before-delivery.md) —
  The replacement regression checks that old history is absent before proving history delivery has finished. The
  boundary gap is confirmed, but a concrete false-pass counterpart for this small fixture remains unverified after
  investigation.

- [`agent-text-helper-removes-wrap-boundary-spaces.md`](agent-text-helper-removes-wrap-boundary-spaces.md) — Browser
  agent checks can fail to recognize text that is present when a space lands at a soft-wrap boundary.

- [`brick-test-requires-hidden-terminal-kind-details.md`](brick-test-requires-hidden-terminal-kind-details.md) — A
  healthy candidate that creates only tmux terminals cannot satisfy the brick test because the test requires a
  terminal-kind detail that the product intentionally hides.

- [`growing-final-output-omits-trimming-notice.md`](growing-final-output-omits-trimming-notice.md) — Final buffered
  output can grow the old terminal's history between Restart's count and capture, making the replacement omit older
  lines without the promised trimming notice. This does not allege loss beyond the accepted history capacity.

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

- [`agent-screen-tools-use-locale-decoder-for-utf8.md`](agent-screen-tools-use-locale-decoder-for-utf8.md) —
  Agent-screen tools request UTF-8 tmux output but can decode it with a legacy Python encoding; the decoder mismatch is
  verified conditionally.

- [`feedback-discord-header-hides-send.md`](feedback-discord-header-hides-send.md) — The Discord header hides the
  feedback Send pointer target in short narrow static layouts on both engines; native Tab restores it.

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

- [`recovered-agent-retains-stale-launch-hold-handle.md`](recovered-agent-retains-stale-launch-hold-handle.md) — After
  agent recovery, Mac supervisor replacement can wait unnecessarily and falsely warn about version protection because it
  checks the ended runtime.

- [`recovery-gap-notice-lifetime-contract-conflict.md`](recovery-gap-notice-lifetime-contract-conflict.md) —
  Same-supervisor recovery exposes contradictory product and implementation promises for the missing-output notice's
  lifetime.

- [`long-launch-status-crops-painted-edge.md`](long-launch-status-crops-painted-edge.md) — Long launch-status labels
  crop the pill's decorative edge in both static engines; elapsed text remains visible.
