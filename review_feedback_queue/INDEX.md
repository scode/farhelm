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

- [`gone-retry-forgets-pinned-foreground-child.md`](gone-retry-forgets-pinned-foreground-child.md) — Recovery can
  discard a previously identified foreground child after its terminal host disappears. A complete production sequence
  leaving that program running after removal remains unverified after investigation.

- [`missing-agent-terminal-blocks-surviving-tmux-tabs.md`](missing-agent-terminal-blocks-surviving-tmux-tabs.md) — After
  the agent terminal disappears, surviving older shell tabs can remain listed while opening or closing them is refused.

- [`orphan-cleanup-overlay-forgets-failed-discovery.md`](orphan-cleanup-overlay-forgets-failed-discovery.md) — When
  discovery fails while cleanup remains unfinished, earlier orphan rows disappear and unrelated fleet updates keep
  retrying the failed read.

- [`credential-push-misses-retired-lifetime.md`](credential-push-misses-retired-lifetime.md) — A replacement connection
  can hide the last lease's zero transition, allowing obsolete credential transport work to delay reconnect. No
  disclosure, wrong-machine delivery or user-work loss is established.

- [`settings-save-races-final-replacement.md`](settings-save-races-final-replacement.md) — Credential delivery can
  silently overwrite an agent settings save made after its final digest check and before replacement. The new comparison
  protects earlier edits but leaves this narrower window.

- [`delete-lost-tab-markers-skip-sweep.md`](delete-lost-tab-markers-skip-sweep.md) — If an old terminal server
  disappears during Delete's tab discovery, Delete can succeed and remove the session while background jobs from an old
  unscoped tab keep running.

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
  naming, bolt-test and bulk-Delete saves can remain in flight after private-stack restoration, then overwrite it.
  Adverse scheduling and required incoming values remain unmeasured; no production user-work loss is claimed.

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
  unmeasured. Additional recipe scope: independent cleanup can accept a missing row before an already accepted POST
  inserts it, leaving fictional recipe and credential rows behind.

- [`devbuild-buffers-unauthenticated-metadata.md`](devbuild-buffers-unauthenticated-metadata.md) — A compromised release
  origin can provide oversized checksum or signature files that are downloaded and fully read before rejection. Material
  resource exhaustion is unmeasured; this is maintainer-tooling resource scope, not a demonstrated catastrophic failure.

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
  completed uninstall or failed initial sandbox launch can leave task bookkeeping reinserted after its host was removed.

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

- [`full-host-tooltip-conflicts-with-line-cap.md`](full-host-tooltip-conflicts-with-line-cap.md) — The shared tooltip
  cap conflicts with full-value disclosure for host names and approval requester titles. The new admitted long-title
  route is source-confirmed; original host-alias wrapping remains unmeasured.

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
  cells can fail to open repeatedly because its initial dimensions exceed the grid stream's bound. The inherited-size
  handoff no longer sends a corrective clamp at unchanged geometry; a distinct incremental failure remains unproved.

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

- [`launcher-diagnosis-overstates-unscoped-name-rule.md`](launcher-diagnosis-overstates-unscoped-name-rule.md) — The
  recorded launcher diagnosis incorrectly says only unscoped text can offer a session-name action; explicit name
  searches can offer it too.

- [`attention-list-refresh-failure-scope-unresolved.md`](attention-list-refresh-failure-scope-unresolved.md) — A
  connected host's session list can stop refreshing without entering the attention list. Whether that omission violates
  the intended scope remains unverified after investigation.

- [`sandbox-create-cancel-lacks-opening-guard.md`](sandbox-create-cancel-lacks-opening-guard.md) — Cancel lacks the
  creation question's opening check. A normal input sequence dismissing a later question remains unverified after
  investigation.

- [`successful-suspend-flashes-connecting.md`](successful-suspend-flashes-connecting.md) — A confirmed sandbox
  suspension can briefly change back to Connecting during its status recheck, even though the sandbox remains suspended.

- [`templates-close-targets-disabled-bulk-button.md`](templates-close-targets-disabled-bulk-button.md) — Closing
  Templates during a bulk operation can lose keyboard return focus because it targets a disabled button.

- [`bulk-pacing-test-samples-before-quiet-regrouping.md`](bulk-pacing-test-samples-before-quiet-regrouping.md) — The
  bulk Stop pacing test can compare requests with an earlier row order that its own quiet fixtures may legitimately
  change before Stop is accepted.

- [`empty-filter-padding-overridden-by-status-rule.md`](empty-filter-padding-overridden-by-status-rule.md) — A generic
  style overrides the intended spacing between an empty-filter explanation and its clear-filter links.

- [`filter-escape-test-accepts-earlier-query.md`](filter-escape-test-accepts-earlier-query.md) — The removed-host filter
  test can credit an earlier unfiltered request instead of proving that clearing the filter sent a new one.

- [`modified-keyboard-row-activation-marks-instead-of-opens.md`](modified-keyboard-row-activation-marks-instead-of-opens.md)
  — Shift-modified keyboard activation can mark sessions instead of opening them in the inspected browser sources;
  native platform coverage remains unverified after investigation.

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

- [`failed-setup-refresh-overwrites-held-lifecycle-status.md`](failed-setup-refresh-overwrites-held-lifecycle-status.md)
  — After setup's registry synchronization fails, a later refresh can replace in-progress Suspend or Resume with
  Connecting while the operation is still running.

- [`retired-connection-leaves-suspend-progress-stuck.md`](retired-connection-leaves-suspend-progress-stuck.md) — A
  failed Suspend on a retired connection can leave the host showing Suspending with lifecycle controls disabled until a
  separate recovery action.

- [`resume-launch-test-records-session-after-assertion.md`](resume-launch-test-records-session-after-assertion.md) — The
  resume-launch test can create a session and then fail before saving its ID, causing per-test cleanup to skip that
  session.

- [`resume-test-retires-create-stub-before-client-settles.md`](resume-test-retires-create-stub-before-client-settles.md)
  — Failure cleanup may remove the create stub before a pending launcher settles, allowing a real test session to start
  afterward. The complete browser ordering remains unverified after investigation.

- [`lifecycle-fixture-cleanup-stops-after-release-failure.md`](lifecycle-fixture-cleanup-stops-after-release-failure.md)
  — One fixture-release failure can skip the remaining browser-test cleanup and leave shared settings or resources
  behind.

- [`route-retirement-failure-skips-owned-session-cleanup.md`](route-retirement-failure-skips-owned-session-cleanup.md) —
  A route-retirement exception can prevent cleanup of sessions the test already owns.

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

- [`agent-settings-write-exceeds-next-read-limit.md`](agent-settings-write-exceeds-next-read-limit.md) — Saving agent
  settings can produce a file larger than Farhelm's own next-read limit, making later settings operations refuse it.

- [`old-suspension-stamp-labels-later-external-pause.md`](old-suspension-stamp-labels-later-external-pause.md) — A
  suspension remembered from an earlier operation can be reused to describe a later independent pause, misattributing
  its timing.

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

- [`remote-version-display-retains-format-characters.md`](remote-version-display-retains-format-characters.md) — Remote
  version text bypasses the promised presentation cleaning and can retain invisible formatting characters; no execution
  sink or harmful ordinary producer was established.

- [`fake-filesystem-cancellation-outlives-test-transaction.md`](fake-filesystem-cancellation-outlives-test-transaction.md)
  — A cancelled asynchronous fake-filesystem operation can continue into a later test transaction and change its state.

- [`settings-close-tooltip-promises-immediate-discard.md`](settings-close-tooltip-promises-immediate-discard.md) —
  Settings' Close tooltip says edits will be discarded even though unsaved recipe changes open a guarded leave question.

- [`restart-with-does-not-resume-suspended-host.md`](restart-with-does-not-resume-suspended-host.md) — Restart with on a
  suspended sandbox does not perform the resume step, so the requested restart fails instead of continuing.

- [`old-keep-editing-clears-new-leave-continuation.md`](old-keep-editing-clears-new-leave-continuation.md) — An earlier
  Keep editing answer can erase a later pending close request, so Save or Discard returns to the form without closing
  its dialog.

- [`disabled-lifecycle-items-receive-roving-focus.md`](disabled-lifecycle-items-receive-roving-focus.md) — Keyboard
  navigation can target disabled lifecycle actions and lose the intended focus movement.

- [`long-host-name-progress-reachability-unresolved.md`](long-host-name-progress-reachability-unresolved.md) — A long
  host name may push progress content beyond its available layout. Actual supported native geometry remains unverified
  after investigation.

- [`settings-save-replies-arrive-out-of-order.md`](settings-save-replies-arrive-out-of-order.md) — An older
  settings-save reply can overwrite the displayed result of a newer save even though the persisted settings remain
  correct.

- [`account-save-visible-draft-mismatch-unresolved.md`](account-save-visible-draft-mismatch-unresolved.md) — Account
  Save reads the live API-key field. A native input sequence saving a key different from the visible field remains
  unverified after investigation.

- [`ssh-key-tooltip-test-lacks-loading-witness.md`](ssh-key-tooltip-test-lacks-loading-witness.md) — The SSH-key tooltip
  regression can make its assertion without establishing the key-loading state it intends to test.

- [`package-name-trailing-minus-requests-removal.md`](package-name-trailing-minus-requests-removal.md) — A recipe
  package ending in a minus sign is passed to apt with its removal meaning instead of being rejected or treated as a
  literal package name.

- [`recipe-destination-refuses-absolute-dns-name.md`](recipe-destination-refuses-absolute-dns-name.md) — A valid final
  DNS dot is refused in recipe tunnel destinations and agent endpoint hostnames.

- [`agent-endpoint-refuses-valid-at-sign-path.md`](agent-endpoint-refuses-valid-at-sign-path.md) — A valid at-sign in an
  agent endpoint's URL path is rejected as though it supplied credentials.

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

- [`changed-project-key-cleanup-absence-unresolved.md`](changed-project-key-cleanup-absence-unresolved.md) — After an
  account key changes projects, cleanup may confuse inaccessible resources with absent ones. The provider's foreign-ID
  response behavior remains unverified after investigation.

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

- [`rebuild-order-test-misses-persistence-boundary.md`](rebuild-order-test-misses-persistence-boundary.md) — The
  rebuild-order regression observes calls without proving the database state was persisted at the boundary it claims to
  protect.

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

- [`first-state-directory-ancestor-durability-unresolved.md`](first-state-directory-ancestor-durability-unresolved.md) —
  A first-created state directory does not flush every required ancestor explicitly. Whether supported filesystem
  behavior can lose the admitted state remains unverified after investigation.

- [`automatic-update-opt-out-can-roll-back.md`](automatic-update-opt-out-can-roll-back.md) — A power loss can roll back
  opting out of automatic updates and allow later unattended updates. Whether the general preference-loss acceptance
  covers this authorization remains unresolved.

- [`terminal-attachment-trace-retains-raw-session-id.md`](terminal-attachment-trace-retains-raw-session-id.md) —
  Terminal attachment diagnostics retain a raw peer-supplied session identifier instead of the promised cleaned
  presentation copy.

- [`peer-text-normalization-contract-overstates-retained-scope.md`](peer-text-normalization-contract-overstates-retained-scope.md)
  — The specification's universal normalization rule for retained peer text contradicts its explicit raw-byte identity
  and diagnostic exceptions.

- [`escaped-token-path-display-collision-unresolved.md`](escaped-token-path-display-collision-unresolved.md) — Escaped
  and literal token-file paths can look identical in approval text. A supported harmful pair of real files remains
  unverified after investigation.

- [`attention-dismissal-membership-scope-unresolved.md`](attention-dismissal-membership-scope-unresolved.md) —
  Dismissing optional updates also suppresses later reminders for hosts currently shown as problems. Whether dismissal
  should include those hidden offers remains unverified after investigation.

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

- [`shift-range-uses-refreshed-row-order-unresolved.md`](shift-range-uses-refreshed-row-order-unresolved.md) — A Shift
  range can use a newer row order than the earlier anchor interaction. A consequential sequence beyond the accepted
  short delivery interval remains unverified after investigation.

- [`macos-control-click-suppresses-row-marking.md`](macos-control-click-suppresses-row-marking.md) — On the inspected
  macOS Chromium route, Control-click is consumed as a context-menu gesture instead of marking the session.

- [`marked-count-reflow-leaves-popup-at-old-anchor.md`](marked-count-reflow-leaves-popup-at-old-anchor.md) — A changed
  marked count may move the menu button onto another toolbar line while its popup stays put. The necessary rendered
  geometry remains unverified after investigation.

- [`bulk-test-preference-latch-bypasses-finally.md`](bulk-test-preference-latch-bypasses-finally.md) — An unresolved
  preference latch can outlive the test timeout without unwinding its finally block, skipping owned session and
  preference cleanup.

- [`empty-filter-actions-escape-keeps-session-marks.md`](empty-filter-actions-escape-keeps-session-marks.md) — Escape
  from empty-filter actions leaves session marks active instead of clearing them under the list's Escape policy.

- [`truncated-empty-filter-result-omits-clear-actions.md`](truncated-empty-filter-result-omits-clear-actions.md) — A
  truncated empty-filter result can omit the clear-filter links needed to recover through that view.

- [`bulk-screenshot-test-expects-wrong-local-alias.md`](bulk-screenshot-test-expects-wrong-local-alias.md) — The
  bulk-action screenshot regression expects a local-host alias that its own fixture does not provide.

- [`no-tmux-lifecycle-witness.md`](no-tmux-lifecycle-witness.md) — The desktop smoke can pass if creating or restarting
  a new terminal unnecessarily starts tmux. Its new no-tmux check has already finished before it exercises any sessions.
  This weakens evidence for the requirement that a host with only new terminals never runs tmux.

- [`pending-test-host-start-cleanup.md`](pending-test-host-start-cleanup.md) — After a supervisor crashes during a
  terminal launch, browser-test cleanup can report success and delete its state while a terminal host finishes starting.
  That leaves an owned test process running without its socket or retained run directory. This concerns maintainer
  tooling, not an established loss of user sessions.

- [`stty-tool-format-mismatch.md`](stty-tool-format-mismatch.md) — The terminal-settings test can fail on a Mac with GNU
  command-line tools first on PATH even when the host's terminal settings are correct. Its reference and host samples
  use different stty implementations, whose saved-settings strings cannot match.

- [`closed-stdin-epipe-witness.md`](closed-stdin-epipe-witness.md) — The test for keeping SSH input open after the
  remote stops accepting it can pass without ever reaching the failure-handling boundary it claims to check. A refactor
  that buffers input without forwarding it can leave that boundary untested while the test stays green. This is a
  regression-test sensitivity defect, not evidence that current forwarding is wrong.

- [`catalog-unencoded-file-url.md`](catalog-unencoded-file-url.md) — The Tensorlake test runner cannot load its terminal
  catalog from a tested checkout whose directory name contains # or certain literal percent escapes. It builds a URL
  from an unencoded filesystem path.

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

- [`host-tab-management-tmux-dependency.md`](host-tab-management-tmux-dependency.md) — Keeping one old session on a host
  can make Restart or Delete of a completely new session fail when the old terminal server cannot answer, even though
  the new session has no old tabs.

- [`capture-failed-stop-erases-recovery.md`](capture-failed-stop-erases-recovery.md) — When the real-agent
  inspection/capture tool cannot confirm its terminal host stopped, it hides that failure and still removes the run
  directory needed to retry the supported stop command. Continued agent execution after the deletion has not been
  demonstrated.

- [`selection-cleanup-after-grid-adoption.md`](selection-cleanup-after-grid-adoption.md) — A queued selection-cleanup
  callback can throw if the compatibility terminal switches to the grid before that callback runs. Actual occurrence in
  a supported browser has not been observed. No lasting input failure, lost work or wider disruption is established.

- [`asset-wait-bypasses-pending-kind.md`](asset-wait-bypasses-pending-kind.md) — A terminal still waiting for
  fonts/helpers can construct the old component and open a socket while Restart tells it to wait. The established l1
  consequence remains unnecessary component/socket work; successful grid adoption leaves no established lasting defect.
  A separate known pre-notice input-format hazard must not be treated as safe by this limited conclusion.

- [`pending-kind-font-premise.md`](pending-kind-font-premise.md) — The test intended to prove that an unresolved
  terminal kind blocks mounting can pass because its fonts are still loading instead. A pass does not establish the
  stated reason that no socket opened during the measured interval.

- [`direct-grid-test-bootstrap-order.md`](direct-grid-test-bootstrap-order.md) — A healthy grid terminal can fail its
  integration test when the unrelated xterm asset loads slowly, because the test asserts the asset is loaded before
  establishing that premise.

- [`stopped-reader-output-witness.md`](stopped-reader-output-witness.md) — The browser flood test checks retained output
  precisely, but a pass does not establish that the flood ran while the page stopped consuming frames. It can therefore
  leave the intended slow-viewer case unproved while appearing to cover it.

- [`container-redraw-discards-hover.md`](container-redraw-discards-hover.md) — A container terminal can lose a completed
  file hover when other cells on that row redraw. One redraw can cause temporary rechecking. Another relevant row change
  during replacement admission can leave the link absent while the pointer stays at rest; movement or completion of a
  later gesture can retry. A first click started without a hovered link does not activate the file.

- [`osc-admission-repeats-row-work.md`](osc-admission-repeats-row-work.md) — Unverified after investigation: admitting a
  dense row of program hyperlinks in a container terminal repeats whole-row construction for each candidate, potentially
  delaying pointer or UI work. The repeated work is source-confirmed; material latency has not been measured. This is
  separate from asynchronous redraw losing a hover.

- [`delete-reader-notice-bypass.md`](delete-reader-notice-bypass.md) — Input arriving while a session is being deleted
  can make its terminal detach with an input-failure notice instead of the Delete's notice or its cleanup-failure
  explanation. The landed notice fix misses the reader's direct failure path.

- [`version-probe-latches-gone-unusable.md`](version-probe-latches-gone-unusable.md) — If the old terminal server
  disappears after the supervisor's initial socket check but before its server-version query, Farhelm can refuse Restart
  and Delete of its old sessions for the rest of that supervisor run. The error says a running unusable server holds
  them even though tmux already reported that no server exists. Restarting the supervisor is needed to clear this false
  refusal.

- [`tmux-negative-result-observation.md`](tmux-negative-result-observation.md) — After the old terminal server
  disappears, some capture and cleanup paths continue issuing tmux commands despite receiving a definitive absence
  reply. The extra work is finite; no indefinite churn or loss of user work is established. The sixteen-sample bound
  applies to the sampler alone, not every cleanup roster.

- [`tmux-refusal-broken-rustdoc-link.md`](tmux-refusal-broken-rustdoc-link.md) — An internal rustdoc link names a method
  the driver does not have. The refusal behavior and surrounding explanation remain accurate; no product consequence is
  established.

- [`tmux-exit-empty-spec-citation.md`](tmux-exit-empty-spec-citation.md) — The new configuration explanation directs
  maintainers to a specification section in the wrong document. Its description of when the private server ends is
  accurate; the error is its source citation.

- [`agent-screen-refusal-missing-path.md`](agent-screen-refusal-missing-path.md) — If a capture working directory
  contains bytes that are not UTF-8, the new backend refuses it without displaying the path it refused.

- [`old-terminal-query-answer-doc.md`](old-terminal-query-answer-doc.md) — A new maintenance comment attributes all
  automatic old-terminal replies to the browser, although ordinary device and cursor queries are answered inside tmux
  and removed before reaching it. This misstates the warning's input path; no changed execution or user-visible
  malfunction is claimed.

- [`attach-autostarts-empty-tmux.md`](attach-autostarts-empty-tmux.md) — After upgrading a host with old terminals, an
  attachment that races the old terminal server going away can start a new empty tmux server. Farhelm then stops
  querying it but leaves that unnecessary process running, contrary to the promise that tmux ends with the last old
  terminal. No agent or user-work loss is claimed.

- [`tmux-empty-error-doc.md`](tmux-empty-error-doc.md) — No current user-visible failure is established. The code's
  explanation of how stopped old terminals are detected contradicts its new behavior, making future lifecycle changes
  easier to get wrong.

- [`tmux-no-spawn-test-retired-query-doc.md`](tmux-no-spawn-test-retired-query-doc.md) — The no-spawn test says it
  checks a version-query refusal, but that operation was already removed when the test was added. Correct the
  description so readers can tell what the test actually covers. No product behavior or safety defect is attributed to
  this wording.

- [`tmux-presence-false-doc.md`](tmux-presence-false-doc.md) — The new internal API documentation says a false
  query-gate answer proves an old session does not exist. It also returns false when a server is unusable and its agents
  may still run. Future maintainers are given an incorrect contract; no additional shipped process-loss route is
  attributed to the prose.

- [`notification-cleanup-row-premise.md`](notification-cleanup-row-premise.md) — The new upgrade test's cleanup
  assertion does not prove that deleting a session removes its retained notification. Another test independently checks
  notification preservation through all later migrations, so this is a narrow missing premise in the migrated-trigger
  check, not absence of all notification-upgrade coverage.

- [`transition-accessor-comment.md`](transition-accessor-comment.md) — A maintainer following the test's explanation
  encounters an accessor name that no longer exists. No product behavior or lifecycle guarantee is shown to be wrong.

- [`successor-exit-processing-witness.md`](successor-exit-processing-witness.md) — The retained-exit-code regression
  test relies on recovery replay finishing in the background before it checks the code. A slow replay can make a correct
  restart fail this test; the failing schedule is unverified after investigation.

- [`grid-row-is-not-program-line.md`](grid-row-is-not-program-line.md) — The new terminal transcript can make a
  partially rendered program line appear complete to a test. Agent checks can then reject correct behavior; one
  direct-signal caller could instead use a process-number prefix and terminate unrelated work. The exact short-line
  split, unrelated signal target and regression attribution remain unverified after investigation.

- [`grid-text-omits-blank-columns.md`](grid-text-omits-blank-columns.md) — The new test helper can turn visibly
  separated terminal text into a different string. A test searching that transcript for the displayed text may then time
  out or match text that was never contiguous on screen. No currently failing assertion was identified.

- [`hook-waiter-missing-invocation-binding.md`](hook-waiter-missing-invocation-binding.md) — A repeated hook callback
  can be credited with an earlier callback's terminal result. Report-write counts prove that reports ran and were
  accepted, but do not prove the second callback exited successfully and stayed silent. The exact false-pass scheduling
  remains unverified after investigation.

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

- [`channel-zero-missing-capability.md`](channel-zero-missing-capability.md) — The reserved-channel regression assertion
  can pass for a separate missing-capability refusal, even with a valid real session.

- [`argv-grid-wrap-witness.md`](argv-grid-wrap-witness.md) — The structured-launch test can decode arguments the fake
  process never received if attachment catches a wrapped hex witness partway through a continuation row. That can reject
  a correct launch. The event-shape defect is supported by source; the exact scheduling cut remains unverified after
  investigation.

- [`final-stack-drop-claim-window.md`](final-stack-drop-claim-window.md) — A very narrow setup race can make the
  converted restart-gap tab test fail even when tab opening behaves correctly. Ordinary overlap between the test's
  supervisors is not this race: both share the same process's live ownership claim. The possible consequence is a
  maintainer-test false failure only.

- [`tab-survival-missing-live-exchange.md`](tab-survival-missing-live-exchange.md) — The converted tab-survival test can
  potentially pass after a handoff has incorrectly ended the shell. Its successor checks the host's remembered child
  number and old terminal text, rather than asking the shell to do anything. This is a test-coverage concern, not a
  claim that shipped handoff kills shells. Exact false-pass scheduling remains unverified after investigation.

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

- [`claude-recreation-test-clears-old-stamp.md`](claude-recreation-test-clears-old-stamp.md) — The
  recreated-Claude-login regression clears the old delivery stamp first, so it cannot protect the
  same-name/same-revision incarnation check. Production currently includes that check.

- [`release-shot-account-readiness.md`](release-shot-account-readiness.md) — A release capture's enabled-input check
  accepts an unloaded or failed account form. No existing published image is established to be wrong.

- [`recipe-cleanup-navigation.md`](recipe-cleanup-navigation.md) — Navigation rejection skips a browser test's owned
  recipe and credential deletions, potentially contaminating later tests on its private stack.

- [`ambiguous-codex-seed-renewal.md`](ambiguous-codex-seed-renewal.md) — Unverified after investigation: an uncertain
  Codex login upload can replay after renewal. Whether restoring the saved seed makes authentication or later refresh
  unusable remains unestablished; explicit exactly-once ambiguity coverage is partial.

- [`launcher-live-draft-submission-source-gap.md`](launcher-live-draft-submission-source-gap.md) — The launcher reads
  live choices at submission, but no complete supported sequence establishes a contradictory accepted launch for the
  remaining generic concern. Other priority; unverified after investigation.

- [`notification-read-failure-leaves-stale-cell.md`](notification-read-failure-leaves-stale-cell.md) — After a
  notification read fails just after an old-terminal warning changes, the bell can remain missing or urgently unresolved
  after storage recovers. Normal reconciliation does not retry that publication.

- [`tunnel-dns-outlives-dial-admission.md`](tunnel-dns-outlives-dial-admission.md) — Unverified after investigation:
  stalled hostname resolution can outlive recipe tunnel timeouts and admission limits, potentially accumulating work or
  delaying helm shutdown. An affected native resolver execution has not been demonstrated.

- [`container-path-publication-keeps-stale-cut.md`](container-path-publication-keeps-stale-cut.md) — Unverified after
  investigation: a container terminal can offer an obsolete joined filename if neighboring output changes while a
  separate hyperlink lookup is pending. The download path independently checks recognition and refuses the changed
  target.

- [`settings-close-test-forwards-cancelled-save.md`](settings-close-test-forwards-cancelled-save.md) — The regression
  meant to prove that a settings save survives closing its dialog can pass even if the original browser request is
  cancelled: the test itself sends an independent write. No current product save-loss bug is established.

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

- [`switch-geometry-discards-visible-frames.md`](switch-geometry-discards-visible-frames.md) — The session-switch
  regression discards visible frames when their expected edge text or rule is absent. Its final comparison can therefore
  miss that category of temporary visible failure. No current product rendering failure was demonstrated.

- [`switch-geometry-capped-readiness.md`](switch-geometry-capped-readiness.md) — The session-switch test can keep
  waiting on frozen historical geometry after a trace fills, even if the terminal later paints correctly. This is a
  conditional source-level test defect; no actual failing run was observed.

- [`tmux-image-role-recognition-unresolved.md`](tmux-image-role-recognition-unresolved.md) — Cleanup might miss a
  session-started tmux server under a nonstandard executable name. A supported installation producing the required name
  and truncated role has not been established, so this is an investigated uncertainty.

- [`stopped-host-update-floor-unresolved.md`](stopped-host-update-floor-unresolved.md) — A stopped host running Farhelm
  below the minimum update version can still be updated without checking its installed version. Whether the
  minimum-version rule is intended to cover stopped supervisors remains unresolved.

- [`switch-geometry-row-readiness-unresolved.md`](switch-geometry-row-readiness-unresolved.md) — The session-switch test
  might accept an old-height screen as ready and later compare a correct screen with that stale baseline. The fixture's
  required row change and timing have not been established; this is an investigated uncertainty.

- [`repeated-cleanup-failure-test-misses-recurrence.md`](repeated-cleanup-failure-test-misses-recurrence.md) — The
  regression can pass without observing the cleanup failure it repeats, leaving recurrence handling unprotected.

- [`unmeasured-notices-can-be-marked-read.md`](unmeasured-notices-can-be-marked-read.md) — Closing a notification panel
  before its measured placement appears may mark unseen notices read. The required native event ordering remains
  unverified after investigation; history is retained.

- [`cleanup-retry-omits-recovery-step-times.md`](cleanup-retry-omits-recovery-step-times.md) — After a cleanup failure,
  Retry can report zero time for recovery steps that actually ran, weakening its diagnostic timing report.

- [`cleanup-list-error-hides-accepted-progress.md`](cleanup-list-error-hides-accepted-progress.md) — Retry can accept
  cleanup while Trash keeps showing its earlier listing error without running progress, until a successful refresh
  arrives.

- [`notice-retry-error-hides-clear-failure.md`](notice-retry-error-hides-clear-failure.md) — After Retry fails, a later
  failed Clear resolved action can show only the earlier Retry error.

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

- [`attention-save-error-can-exhaust-list-height.md`](attention-save-error-can-exhaust-list-height.md) — A retained
  update-preference error and wrapped header may leave no reachable attention-list body. The required supported viewport
  and font geometry remain unverified after investigation.

- [`approval-scrollports-lack-keyboard-entry.md`](approval-scrollports-lack-keyboard-entry.md) — Keyboard users lack a
  named focus target for horizontally clipped approval facts and text-only scrolling approval bodies. The layout test
  currently requires that omission.

- [`requester-full-id-uses-unnamed-generic-span.md`](requester-full-id-uses-unnamed-generic-span.md) — The requesting
  session's shortened ID relies on an accessible label on a generic span, where that label is prohibited. The markup
  mismatch is confirmed; actual assistive-technology output was not tested.

- [`approval-copy-test-assumes-clipboard-object.md`](approval-copy-test-assumes-clipboard-object.md) — The Copy ID test
  can fail before its assertion if the engine provides no clipboard object. Whether the supported browser configuration
  omits it remains unverified after investigation.

- [`short-progress-card-can-hide-overflow-notice.md`](short-progress-card-can-hide-overflow-notice.md) — A very short
  update-progress hover may hide its Full details guidance even after diagnostics shrink. Reachable native geometry
  remains unverified after investigation.

- [`progress-hover-keeps-short-host-name.md`](progress-hover-keeps-short-host-name.md) — Update progress repeats a
  shortened host name instead of revealing its full display name, so similarly named hosts remain indistinguishable
  there.

- [`progress-hover-test-can-scroll-away-entry.md`](progress-hover-test-can-scroll-away-entry.md) — The browser test may
  scroll an ancestor after entering the hover target and dismiss the card it is waiting for. The fixture geometry and
  native ordering remain unverified after investigation.

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

- [`attention-retry-omits-failed-save-identity.md`](attention-retry-omits-failed-save-identity.md) — An old Try again
  activation could restore automatic updates after a newer opt-out, but no ordinary shipped ordering outside accepted
  input races was established. This remains unverified after investigation.
