# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

## Highest priority: security or data loss

- [`template-delete-reads-new-selection.md`](template-delete-reads-new-selection.md) — On desktop, Delete still shown
  for one template can delete a newly selected template. Its temporary Undo does not restore shortcut bindings; event
  frequency is unmeasured.

- [`template-retry-installs-unseen-permanent-delete.md`](template-retry-installs-unseen-permanent-delete.md) — After
  partial failures involving an invalid older template, desktop Retry can leave ordinary Delete visible while its click
  permanently deletes the template without the promised warning or Undo. Release-upgrade filtering covers at most part
  of this conditional route.

- [`template-delete-lost-reply-loses-undo.md`](template-delete-lost-reply-loses-undo.md) — An ordinary template deletion
  can commit but offer no Undo when its success reply is lost. Manual recovery from the mounted editor remains possible
  until it closes; the unknown-outcome exception's scope is ambiguous.

- [`link-refusal-conceals-delete-warning.md`](link-refusal-conceals-delete-warning.md) — A raised link refusal can hide
  a session's destructive Delete warning while keyboard opening and a fresh affirmative Enter still work. The complete
  source path is established; actual overlap pixels and loss were not measured.

- [`tunnel-reuses-custom-helm-credential-origin.md`](tunnel-reuses-custom-helm-credential-origin.md) — A desktop tunnel
  can receive a previously used custom browser helm port and expose its retained device credential to remote HTML in the
  same browser profile. The default helm port and embedded desktop credentials are outside this conditional route.

- [`restart-trust-follows-stale-checkout-membership.md`](restart-trust-follows-stale-checkout-membership.md) — With
  automatic workspace trust enabled, Restart with can trust a replacement folder because a resumable Codex or Pi session
  still belongs to the old managed checkout. Borrowing sessions skip the origin's folder-identity checks; no vendor
  execution was attempted.

- [`named-restart-reporter-failure-keeps-old-resume.md`](named-restart-reporter-failure-keeps-old-resume.md) — A
  successful named Pi or OMP restart can keep the previous conversation when preparing its reporter fails. A later
  ordinary Restart can then resume that previous conversation. The conditional source route is established; occurrence
  is unmeasured.

- [`older-installer-rebuild-deletes-late-foreign-app.md`](older-installer-rebuild-deletes-late-foreign-app.md) —
  Installing an older release can delete an unrelated app moved into Farhelm's usual location after the installer's
  initial ownership check. This conditional path needs neither an interruption nor failed restoration.

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
  selection save may overwrite the history fixture's reset; adverse ordering is unverified.

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

- [`host-dialog-reservation-does-not-exclude-settings.md`](host-dialog-reservation-does-not-exclude-settings.md) — A
  Settings click during host-dialog admission may leave both dialogs inert until Escape; native ordering is unmeasured.

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
  earlier caller-cleanup failure can skip restoring the new prepared-checkout switch; historical coverage remains
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

- [`trash-alias-refresh-skips-movement-pause.md`](trash-alias-refresh-skips-movement-pause.md) — A same-GUI host alias
  refresh can change trash layout without triggering its movement pause. A supported layout placing another enabled
  Delete under the old pointer remains unestablished, so this is a conditional fingerprint gap.

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

- [`post-clone-test-save-overtakes-restoration.md`](post-clone-test-save-overtakes-restoration.md) — The new post-clone
  Save can remain in flight after the test restores its private stack's settings, then overwrite that restoration.
  Adverse scheduling is unmeasured; no production user-work loss is claimed.

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
  Claude Restart clears the report pin before restoring the old pane. A delayed child could replace its Resume target,
  but an ordinary hooked, unmarked nested producer remains unestablished; shipped implementation now extends the plan
  evidence.

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
