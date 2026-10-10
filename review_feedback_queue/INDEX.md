# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

The collected correctness and security review findings refer to commit `bd8d5d76439f8f36bbc2637b44d7ba4026b65675`. That
review stopped with known coverage gaps; the collection is not a whole-repository pass. Later product changes have not
been used to reassess these findings. Each file retains its reviewed commit, confidence, caveats, and provenance for
triage.

## Highest priority: security or data loss

- `installer-startup-prune.md` — An interrupted update can prune a version whose supervisor is starting.

- `executable-read-back-collapses-dollars-select-another-installations.md` — Uninstall can mistake a service with
  literal dollars for another installation's service.
- `multiple-execstart-commands-incorrectly-treated-last-assignment-wins.md` — A service running commands from multiple
  installations could be assigned to only one.
- `plain-clicks-overwrite-clipboard-old-selection.md` — An ordinary terminal click can replace the clipboard with an old
  selection.
- `rejected-permission-toggles-depend-observing-intermediate-busy.md` — A refused permission revocation could leave a
  misleading unchecked checkbox.
- `embedded-paste-terminators-allow-command-execution-definite.md` — An embedded paste terminator could make pasted text
  submit commands.
- `ascii-spaces-remain-invisible-host-identity-approval-labels.md` — ASCII spaces could make distinct installation
  identities hard to distinguish during approval.
- `template-command-editors-hide-meaningful-characters-silently-saved-command-editor.md` — Editing a saved launch
  command silently removes stored newlines.
- `template-command-editors-hide-meaningful-characters-silently-saved-resume-command-editor.md` — Editing a saved resume
  command silently removes stored newlines.
- `template-summaries-interpolate-model-text-into-approval-summary-model-interpolation.md` — Template summary
  construction could let model text distort permission wording.
- `template-summaries-interpolate-model-text-into-approval-templates-list-summary-rendering.md` — The templates list
  could display misleading permission wording in summaries.
- `template-summaries-interpolate-model-text-into-approval-quick-switcher-summary-rendering.md` — The quick switcher
  could show model text as misleading permission wording.
- `replace-omits-conversation-loss-warning-failed-resume.md` — Replace omits a conversation-loss warning after a failed
  resume launch.
- `stale-deflake-pid-terminate-unrelated-work.md` — Stopping an old test sweep can signal an unrelated process.
- `real-agent-cleanup-forget-somebody-elses-workspace.md` — A failed real-agent test can unregister another workspace.
- `release-failure-advice-tells-operator-reuse-tag.md` — Release recovery advice could lead an operator to reuse an
  immutable tag.
- `ssh-stanza-removal-destroy-existing-configuration.md` — Removing a temporary SSH stanza can destroy unrelated
  configuration.
- `legacy-ssh-stanza-cleanup-has-own-destructive.md` — Legacy SSH cleanup has a separate configuration-loss window.
- `installing-temporary-ssh-stanza-destroy-original-configuration.md` — Installing a test SSH stanza can leave the
  user's configuration incomplete.
- `displayed-recovery-paths-shell-quoting.md` — Following a recovery command could act on a different path.
- `unquoted-probe-fixture-paths-escaping-descendant-fixture-escaping-descendant-fixture.md` — A space in the temporary
  root could redirect this fixture's write outside its directory.
- `unquoted-probe-fixture-paths-inherited-pipe-fixture-inherited-pipe-fixture.md` — The inherited-pipe fixture could
  truncate a file outside its temporary directory.
- `arrow-navigation-retains-obsolete-position-item-disappears.md` — An outdated menu position could move keyboard focus
  onto Delete.
- `qualified-hostname-scrubbing-leaves-private-domain.md` — Screen capture could leave the private domain of a qualified
  hostname.

## High priority: material UX degradation

- `tab-cleanup-blocks-status-sampling.md` — Automatic cleanup of an exited terminal tab can leave every session on that
  host showing stale status for seconds while background processes stop.
- `upload-cancellation-drops-final-reply.md` — Delete can discard an upload result during temporary connection
  backpressure, leaving the upload waiting forever even after the connection resumes normal traffic.

- `feedback-cancel.md` — Feedback forwarding can be cancelled with its HTTP request.
- `snapshot-root.md` — Writable snapshot may pass replacement checkout ownership.
- `grok-trust.md` — Choosing Grok preserves incompatible workspace trust.
- `font-focus.md` — Text-size buttons leave focus outside the terminal at the limits.
- `installer-directory-target.md` — An update can report false success or redirect file writes into an external
  directory.
- `uninstall-forgotten-success.md` — A failed uninstall can be reported as successful after another client forgets the
  host.

- `supervisor-cannot-start-farhelms-executable-path-contains.md` — A dollar in the executable path makes the supervisor
  service unusable.
- `helm-cannot-start-farhelms-executable-path-contains.md` — A dollar in the executable path prevents the helm service
  from starting.
- `dollar-escaping-changes-executable-path.md` — Service rendering changes the executable selected by a
  dollar-containing path.
- `uninstall-race-first-time-service-setup.md` — Concurrent setup and uninstall can leave services pointing at a deleted
  executable.
- `interrupted-paired-writes-still-split-service-state.md` — Interrupted service reconfiguration can persist
  incompatible state directories.
- `host-actions-disappear-supported-sidebar-widths.md` — A narrow sidebar could hide the actions needed for an
  unreachable host.
- `build-mismatch-turns-unattended-mounts-into-displacing.md` — Build mismatch can turn automatic recovery into an
  unsolicited takeover.
- `selecting-osc-hyperlink-open-it-unintentionally.md` — Selecting hyperlink text can also open its target.
- `plain-replace-loses-retry-identity.md` — Retrying plain Replace can launch another replacement session.
- `automatic-updates-overwrite-newer-manual-installation.md` — A delayed automatic update can undo a newer manual
  installation.
- `retrying-through-unchanged-permission-choice-launch-duplicate.md` — An unchanged permission choice can make a launch
  retry create a duplicate.
- `older-listing-replace-newly-opened-session-another.md` — An older listing can permanently switch away from a newly
  launched session.
- `newer-completed-run-leaves-older-update-permanently.md` — An old update can remain busy after a newer run has
  completed.
- `delayed-provisioning-confirmations-make-two-dialogs-mutually.md` — A delayed uninstall confirmation can disable both
  open dialogs.
- `composition-fallback-resend-previously-typed-input.md` — Composition fallback could resend retained terminal input.
- `split-utf-characters-disappear-terminal-output.md` — Valid Unicode output can disappear at chunk boundaries.
- `erasing-through-bottom-right-cell-throws-interrupts-output.md` — An erase command could wedge a connected terminal's
  output.
- `narrowing-terminal-corrupts-combining-character-text.md` — Narrowing the terminal changes combining-character text.
- `relative-cursor-movement-adds-scrolling-margin-twice.md` — Relative cursor movement draws on the wrong row with a
  nonzero scrolling margin.
- `column-positioning-unexpectedly-changes-row.md` — Setting the cursor column also changes its row.
- `alternate-horizontal-positioning-command-changes-row.md` — The alternate column-positioning command independently
  moves the cursor's row.
- `setup-writes-unusable-service-paths-executable-path.md` — Setup persistently writes the wrong service executable for
  dollar-containing paths.
- `incomplete-latest-response-select-wrong-release.md` — A truncated latest-version response could install an unintended
  older release.
- `centos-test-changes-meaning-existing-global-ssh.md` — The CentOS test changes the scope of existing SSH settings.
- `unrelated-probe-failure-disables-argument-expansion-protection.md` — A transient probe error could disable protection
  against argument expansion.
- `archive-rename-move-replacement-directory.md` — Checkout archiving could move a foreign replacement directory.
- `service-start-timing-could-evade-restart-detection.md` — Setup could miss a service that starts from its old
  definition.

## Other: correctness, diagnostics, cleanup, or convenience

- `hostnotfound-refresh-keeps-serving.md` — an identity-less actor for a deleted host keeps serving, because its
  refreshes never write the store and so never see the row is gone.
- `session-detail-drains-full-list.md` — every fleet-revision bump makes each open session view trigger a full
  ListSessions on its host.
- `probe-reregister-drops-terminals.md` — probing an already-registered healthy host forces a reconnect that drops every
  open terminal on it.
- `header-actions-skip-listing-read.md` — header replace/restart request no listing read, so under build mismatch the
  sidebar keeps deleted/stale rows.
- `tab-reap-budget-starved-by-failures.md` — failed tab closes spend the per-tick reap budget, so a few persistent
  failures stop exited tabs from being reaped host-wide.
- `tmux-build-script-bash32.md` — build-private-tmux.sh's macOS branch aborts under bash 3.2 because of empty arrays
  under `set -u`.
- `provisioning-update-replaces-binary-before-setup-guard.md` — remote UPDATE replaces the farhelm binary before
  checking whether `farhelm helm setup` took the host over.

- `codex-draft-mistaken-for-question.md` — Pasting question-shaped diagnostics into an unsent Codex draft can make the
  sidebar say the agent needs an answer when it is idle.

- `uninstall-cancel.md` — Cancelled uninstall and Update planning can leave SSH helpers running.
- `birth-oracle.md` — Birth-time oracle treats execution errors as absent capability.
- `token-recovery-path.md` — Token recovery command silently targets a different path.
- `template-catalog-error.md` — Template discovery failure looks like an empty catalog.
- `uninstall-dryrun-locks.md` — Uninstall preview omits currently held lock blockers.
- `plans-heading-splice.md` — Legal Markdown headings can strand part of a plan question.
- `publisher-copy-race.md` — Screenshot publication may certify pixels different from those it checked.
- `watcher-revision-cache.md` — Plans watcher can cache an ignore verdict against the wrong revision.
- `preview-ready-identity.md` — Preview startup can report an unrelated server as the docs preview.
- `template-dot-name.md` — Dot-only template names pass validation but cannot be saved.

- `fixed-process-marker-lets-concurrent-stop-tests.md` — Concurrent Stop tests can kill each other's fixture processes.
- `failed-confirmation-discard-session-despite-failed-process.md` — Failed create recovery could discard the session
  needed to retry process cleanup.
- `template-command-editor-displays-untrusted-command-bytes.md` — The template launch-command editor hides meaningful
  remote-derived characters.
- `template-resume-command-editor-has-same-unsafe-display.md` — The template resume-command editor can misrepresent its
  retained command.
- `template-discovery-fails-permanently-once-reply-exceeds.md` — A large valid template catalog permanently disables
  agent discovery.
- `uninstall-test-inherits-unrelated-state-directory.md` — The uninstall test fails under a valid inherited
  state-directory setting.
- `fixture-startup-leave-real-systemd-service-behind.md` — Failed fixture startup can leave a real user service running.
- `failed-upload-regression-fails-native-macos-tools.md` — The failed-upload test stops at incompatible macOS metadata
  tools.
- `orphan-cleanup-regression-requires-gnu-tools-platform-guard.md` — The orphan-cleanup test fails on macOS before its
  intended sequence.
- `tampering-regression-fails-reaching-tampering-boundary-macos.md` — The tampering test never reaches byte tampering on
  native macOS.
- `literal-helm-state-path-breaks-ssh-connections.md` — A literal environment-reference spelling in the state path
  breaks SSH routing.
- `setups-sign-in-command-ignores-selected-state-directory.md` — Setup's sign-in advice can target a different helm.
- `empty-chunk-test-accepts-upload-stalls-only-traffic.md` — The empty-chunk upload test can pass the wrong timeout
  behavior.
- `releasing-both-relay-gates-establish-reverse-reply.md` — The relay test can pass without replies arriving in reverse
  order.
- `concurrent-list-test-silently-accepts-failure-polling-participant.md` — The concurrent-list test ignores failure of
  its observer.
- `live-processes-reported-dead-macos.md` — macOS process checks can certify death without observing the process.
- `child-discovery-always-times-out-macos.md` — Child-discovery fixtures cannot reach lifecycle checks on macOS.
- `unavailable-process-scan-certifies-no-marked-processes.md` — A missing process scan can make cleanup assertions pass
  without inspection.
- `pid-cleanup-guard-inert-macos.md` — The fixture's PID cleanup guard does not work on macOS.
- `tilde-replay-test-equates-display-canonical-paths.md` — The tilde-replay test rejects correct canonical paths.
- `checkout-recovery-compares-canonical-roots-unresolved-fixture.md` — Checkout recovery tests reject correctly resolved
  directory roots.
- `generic-launch-assertion-uses-different-argv-encoding-fixture.md` — The generic-launch test mistakes quoting
  differences for changed arguments.
- `raw-restart-helper-mistakes-notifications-replies.md` — A restart test could mistake a valid notification for the
  refusal reply.
- `release-builds-fail-held-stdin-hook-test.md` — The held-input hook test falsely fails release binaries.
- `later-refresh-hide-stale-refresh-regression.md` — A later refresh can repair the regression before the stale-refresh
  test checks it.
- `foreign-origin-test-finish-observing-sending-request.md` — The foreign-origin test can stop watching before its Stop
  request occurs.
- `shared-silence-observers-have-same-premature-success-boundary.md` — Shared silence assertions can succeed without
  covering the operation.
- `browse-routing-test-discards-wrong-host-assertion.md` — The browse test ignores an observed wrong-host request.
- `malformed-message-test-pass-through-ordinary-eof.md` — The malformed-message test can pass because the peer closes
  normally.
- `shutdown-test-accepts-forced-cancellation-natural-completion.md` — The shutdown test accepts cancellation of a writer
  that never finishes.
- `failed-adoption-test-leaks-private-tmux-server.md` — A failed adoption test can leave an unreachable tmux daemon
  running.
- `idle-flush-test-permits-deadline-extension-it-claims.md` — The idle-flush test allows the renewed deadline it is
  meant to reject.
- `replay-test-consumes-outstanding-reply-testing-recovery.md` — The replay test settles its pending reply before replay
  begins.
- `upload-memory-test-leaks-mib-fixture-successful-runs.md` — Successful upload-memory tests leave 64 MiB files behind.
- `upload-memory-probe-assumes-linux-kib-pages-rss-rss-procfs-observation-macos-portability.md` — The upload-memory test
  cannot measure RSS on native macOS.
- `directory-browse-test-compares-canonical-output-uncanonicalized-fixture.md` — Directory browsing tests fail on
  symlinked temporary roots.
- `hup-resistant-process-fixture-leaks-early-failure.md` — Early failure leaves HUP-resistant fixture processes behind.
- `live-agent-restart-test-never-establishes-live-agent.md` — The live-agent restart test can run only the dead-agent
  branch.
- `stop-intent-test-exercises-different-failure-path.md` — The stop-intent test misses the live-agent transaction
  ordering.
- `omp-transition-assertions-cannot-detect-stale-conversation.md` — Conversation-switch tests cannot distinguish the
  final target from a stale one.
- `omp-child-report-fixtures-bypass-nested-runtime-check.md` — The child-report test rejects its fixture before checking
  nested-runtime ownership.
- `lock-test-establish-contender-reached-lock.md` — The lock test can pass without any contender waiting behind the
  lock.
- `ended-session-replay-test-accepts-relaunch-under-same.md` — Ended-session replay can relaunch work without failing
  its test.
- `skip-tmux-test-passes-tmux-queried.md` — The skip-tmux test also passes when tmux is unnecessarily queried.
- `invalid-key-test-overlooks-permanently-settled-reservations.md` — Invalid-key tests miss permanently stored failed
  reservations.
- `oversized-resume-test-overlooks-settled-storage.md` — The oversized-resume test can accept prohibited settled
  storage.
- `dead-tab-regression-cannot-pass-ordinary-macos.md` — The dead-tab test fails on ordinary macOS before checking
  cleanup.
- `shutdown-race-test-releases-sink-shutdown-starts.md` — The shutdown race test starts after the critical transition is
  over.
- `busy-pane-assertion-depends-another-panes-readiness.md` — A delayed producer could make the busy-pane test reject
  correct status.
- `host-identity-race-test-pass-exercising-competing-writes.md` — The identity-race test can pass after the write guard
  is removed.
- `resume-preservation-test-never-verifies-relaunch-was-accepted.md` — The resume-preservation test can pass when
  relaunch is refused.
- `prompt-classification-test-finish-waiting-prompt-appears.md` — A delayed prompt could make its classification test
  fail on correct behavior.
- `foreign-destination-fixture-reuse-identity-it-expects-reject.md` — The foreign-directory test could accidentally
  recreate the original identity.
- `cancellation-protection-starts-too-late.md` — Cancelling a connection supervisor before its first poll leaves its
  worker unmanaged.
- `shutdown-race-test-establish-intended-interleaving.md` — The shutdown-race test does not hold reconciliation at the
  database boundary.
- `logging-test-establish-refresh-completed.md` — The quiet-logging test could inspect logs before any refresh
  completes.
- `restart-test-requires-screen-content-restart-correctly.md` — The restart test waits for screen content that correct
  restart erases.
- `cgroup-cleanup-masks-marker-discovery-regression.md` — Scope cleanup hides a broken portable marker-discovery path.
- `cgroup-cleanup-masks-dead-pane-sweep-regression.md` — The dead-pane sweep test can pass because scope cleanup kills
  the survivor.
- `cgroup-cleanup-masks-closure-seeding-regression.md` — Scope teardown masks failure to discover an unmarked child.
- `cgroup-cleanup-masks-fork-quiescing-regression.md` — A scope can eliminate the fork storm before quiescing is tested.
- `readiness-failure-leave-systemd-probe-child-running.md` — Readiness failure could leave a systemd probe child without
  cleanup ownership.
- `pipe-scan-assertion-failures-leave-fixture-processes-running.md` — Failed pipe-scan tests leave children and
  descriptors open.
- `repeated-buffer-copying-amplifies-tiny-frame-traffic.md` — Tiny supervisor frames could amplify buffer-copying cost.
- `macos-environment-test-accepts-failed-observations-success.md` — The macOS environment test treats failed observation
  as successful withholding.
- `environment-value-parsed-output-complete.md` — A split output line could make the environment test parse an
  incomplete value.
- `persistence-assertion-rejects-legitimately-newer-activity-timestamp.md` — A newer persisted timestamp could make the
  persistence test time out.
- `directory-witness-read-pwd-writes-it.md` — The directory-witness test reads publication before it is complete.
- `client-task-scheduling-mistaken-server-side-overlap.md` — Client scheduling could make a serialized server pass the
  overlap test.
- `stop-escalation-fixtures-cleanup-own-whole-stubborn-tree.md` — The stubborn Stop fixture lacks cleanup ownership of
  its whole process tree.
- `dispatch-overlap-fixture-has-same-incomplete-cleanup-armed.md` — The dispatch-overlap fixture arms incomplete cleanup
  after fallible setup.
- `wrapper-stop-test-fails-sh-uses-bashs-final-command.md` — A valid shell optimization breaks the wrapper-stop fixture.
- `orphan-client-test-accepts-failed-inspection-proof-no.md` — The orphan-client test accepts unknown inspection as
  proof of no writer leak.
- `signal-cleanup-test-interrupts-cleanup-exists.md` — The signal-cleanup test interrupts before staging or traps exist.
- `orderly-shutdown-assertion-vacuous-macos.md` — The orderly-shutdown death assertion is always satisfied on macOS.
- `eof-replacement-test-cannot-detect-overlap-macos.md` — The replacement-entry test cannot detect old/new client
  overlap on macOS.
- `replacement-open-shutdown-test-assumes-procfs.md` — Returned replacement-client cleanup is unverified on macOS.
- `ended-session-status-pushes-header-actions-beyond-promised.md` — An ended-session badge could clip actions at the
  promised pane width.
- `local-desktop-build-recipe-fails-relative-target.md` — The desktop build recipe could split artifacts across relative
  target directories.
- `restored-terminal-snapshots-lose-select-to-copy.md` — Restored terminal snapshots no longer copy selected text.
- `seen-state-queue-acknowledge-newer-choice-sending-it.md` — The read-state queue can report success without sending
  the user's latest choice.
- `rss-measurement-undercounts-larger-page-linux-systems.md` — Large Linux pages make the memory test accept
  whole-upload buffering.
- `cancelled-lock-waiters-leave-permanent-registry-entries.md` — Cancelled lock waiters leave retained registry keys.
- `failed-spec-publication-test-observe-whether-tmux-started.md` — Failed-publication tests cannot prove that no
  external window started.
- `checkout-validation-test-cannot-construct-fixture-apfs.md` — APFS rejects the validation fixture before its intended
  checks run.
- `directory-report-polling-accept-unfinished-write.md` — The launch-directory report can be read while still empty.
- `stop-test-leave-fixture-permanently-stopped-failure.md` — Failure after stopping the Stop fixture could leave its
  child indefinitely suspended.
- `restart-test-has-same-stopped-child-cleanup-gap.md` — The Restart fixture separately risks leaving a suspended child
  behind.
- `no-directory-created-assertion-requires-directory-exist.md` — The no-directory-creation test requires the directory
  to have been created.
- `update-popup-stops-following-current-step-opening.md` — The update popup stops keeping the current step visible.
- `non-ascii-template-names-lose-exact-match-priority.md` — A complete non-ASCII template name selects a broader match
  first.
- `stored-template-commands-bypass-safe-display-seeding.md` — The launcher can display a template command differently
  from its retained bytes.
- `stored-template-resume-commands-have-same-independent.md` — The template resume seed conceals bytes kept for later
  execution.
- `scrollbar-dragging-reverses-very-short-terminal.md` — A very short terminal reverses scrollbar dragging.
- `ordinary-creation-never-refreshes-sidebar-under-build.md` — Creation under build mismatch could leave the new session
  absent from navigation.
- `failed-notification-read-mark-suppresses-subsequent-retries.md` — One failed notification read mark prevents ordinary
  retries.
- `successful-notification-reads-leave-bell-unread-under.md` — A successful notification read leaves the bell stale
  under build mismatch.
- `clearing-notifications-leaves-cleared-bell-entries-visible.md` — Cleared notifications reappear under build mismatch.
- `manual-read-unread-toggles-cannot-visibly-toggle.md` — A manual read toggle cannot visibly reverse under build
  mismatch.
- `ship-font-license-embedded-ui.md` — Release bundles could omit the license required with embedded fonts.
- `earlier-setup-uninstall-error-hides-later-update.md` — An earlier planning error hides the reason a later Update
  failed.
- `hyperlink-starting-final-content-cell-disappears.md` — A hyperlink beginning in the row's final content cell
  disappears.
- `resizing-resurrects-cleared-selection-highlight.md` — Resizing brings back a selection highlight that was cleared.
- `concealed-wide-characters-collapse-terminal-columns-concealed-concealed-character-in-merged-span.md` — A concealed
  wide character collapses columns inside a merged span.
- `concealed-wide-characters-collapse-terminal-columns-concealed-concealed-character-in-new-span.md` — A concealed wide
  character also collapses columns in a new span.
- `rename-editor-conceals-title-characters-editable-title-editable-title-seed.md` — The rename editor hides meaningful
  characters in its editable title.
- `rename-refusal-text-bypasses-peer-text-rendering.md` — Rename refusal text can visually conceal peer-controlled
  content.
- `restart-refusal-text-bypasses-peer-text-rendering.md` — Restart with displays refusals without the ordinary restart
  protection.
- `partial-templates-cannot-retain-non-yolo-approval-choices.md` — Approval-only templates cannot preserve supported
  partial settings.
- `ime-confirmation-save-template-prematurely.md` — An IME confirmation could save a template before editing is
  finished.
- `session-refresh-failures-silently-retain-apparently-current.md` — Failed detail refreshes leave old session
  information looking current.
- `rename-original-title-display-conceals-title-characters-original-title-original-title-display.md` — Rename's
  original-title comparison hides meaningful title characters.
- `deflake-duplicates-bounded-evidence-into-unlimited-logs.md` — Test sweeps duplicate bounded evidence into unbounded
  logs.
- `opt-in-real-agent-smoke-launches-fake-claude-executable.md` — The opted-in real-agent smoke still launches the fake
  executable.
- `busy-selection-test-loses-source-second-creation-fails.md` — Failed busy-selection setup leaves acquired sessions
  outside cleanup.
- `title-retry-test-changes-folder-too-masking.md` — The title-change retry test also changes the folder, hiding its
  target regression.
- `failed-rotation-assertion-invalidates-subsequent-tests-credentials.md` — Failure after credential rotation could
  break later tests' authentication.
- `duplicate-scenario-titles-silently-conflate-distinct-sessions.md` — Duplicate scenario titles make capture checks
  refer to the wrong session.
- `stop-recovery-test-fail-wrong-listing-request.md` — The Stop recovery test could inject failure into an earlier
  listing read.
- `notification-stub-accepts-writes-wrong-session.md` — The notification stub accepts read marks addressed to another
  session.
- `sticks-test-wait-additional-refresh-it-claims.md` — The unread-stability test can finish after only the first render.
- `failed-clone-setup-leaves-hosts-route-permanently.md` — Early clone-test failure leaves a route gate unreleased.
- `constructor-font-regression-observes-mutable-value-construction.md` — The constructor-font test observes a value that
  can already have been corrected.
- `link-drag-fixture-mistake-wrapped-url-single-row-url.md` — The link-drag test could drag outside the link while
  claiming a single-row fixture.
- `reconnect-deadline-test-permits-deadline-reset-regression.md` — The reconnect test could pass even when switching
  restarts its deadline.
- `empty-frame-test-stops-stimulus-checking-watchdog.md` — The empty-frame test permits a watchdog reset on every frame.
- `outstanding-heartbeat-test-accept-probe-answered-wedge.md` — The heartbeat test could count a probe answered before
  the intended wedge.
- `resize-teardown-accepts-refused-deletion-success.md` — Resize-test teardown reports success after a refused deletion.
- `multi-session-test-hold-delete-request.md` — The multi-session test's DELETE gate does not match the real request.
- `exited-session-test-bypasses-delete-gate.md` — The exited-session test separately bypasses its deletion hold.
- `remote-browse-witness-accept-earlier-tests-request.md` — A remote-routing witness can reuse a previous test's
  receipt.
- `failed-setup-overwrite-ownership-running-replacement-supervisor.md` — Failed supervisor setup can overwrite ownership
  of the still-serving child.
- `phantom-tab-regression-test-never-waits-claimed-later.md` — The phantom-tab test checks absence before witnessing
  later reconciliation.
- `pending-mount-cancellation-test-cannot-distinguish-cancellation-surviving.md` — The pending-mount test cannot tell
  cancelled retries from surviving retries.
- `sibling-close-concurrency-test-executes-both-closes-sequentially.md` — The sibling-close test could pass with a
  global serialization guard.
- `initial-result-publication-failure-erases-observed-child-result.md` — A failed first result write discards known
  test-run facts.
- `yolo-refusal-fixture-unintentionally-puts-page-into.md` — The YOLO-refusal fixture silently changes the page into
  build mismatch.
- `saving-restoring-cursor-attributes-loses-extended-formatting.md` — Cursor restore leaves extended formatting changed
  after the save.
- `scrolling-downward-inserts-rows-wrong-background.md` — Downward scrolling inserts blank rows with the default
  background.
- `classification-counts-runner-failures-deterministic-test-failures.md` — The failure classifier treats runner errors
  as proof of deterministic test failure.
- `mouse-fidelity-assertion-accepts-utf-corrupted-reports.md` — The mouse-fidelity test accepts the encoding corruption
  it claims to reject.
- `failed-add-host-assertions-leave-extra-registered-host.md` — A failed add-host test leaves a registration that later
  resets do not remove.
- `history-fixture-enables-yolo-installing-restoration-guard.md` — History-test setup can leak its changed confirmation
  policy.
- `keyboard-launch-fixture-has-same-separate-setup-ownership.md` — Keyboard-launch setup independently leaves changed
  policy after failure.
- `history-fixtures-policy-restoration-skip-remaining-cleanup.md` — History-test restoration failure skips cleanup of
  its known session.
- `keyboard-launch-fixtures-policy-restoration-skip-session-deletion.md` — Keyboard-launch teardown skips session
  deletion if policy restoration fails.
- `hook-test-startup-delete-newly-created-launch-specification.md` — Hook-test startup could delete a launch
  specification created during cleanup.
- `replay-geometry-test-rejects-valid-live-output.md` — The replay-geometry test could mistake live output for a bad
  replay.
- `short-writer-timeout-masks-shutdown-drain-regression.md` — An independent writer timeout hides a broken
  shutdown-drain bound.
- `window-restoration-uses-wrong-display-scale-macos.md` — macOS window restore applies the new display's scale to saved
  geometry.
- `ambiguous-claim-orphaned-next-pick.md` — An uncertain plan claim could be stranded when the executor claims another.
- `claiming-current-plan-refresh-plan-subsequently-executed.md` — Plan execution can use stale instructions after
  claiming the current queue entry.
- `retained-failed-tags-become-mandatory-upgrade-test-inputs.md` — Failed release tags could make standard upgrade
  validation impossible.
- `cutover-probe-ignores-output-asserted-boundary.md` — The cutover probe ignores early output between reply blocks.
- `readme-capture-use-stale-builds-successful-build.md` — README capture can photograph stale behavior despite a
  successful rebuild.
- `video-recording-use-stale-builds-successful-build.md` — Video capture independently launches fixed paths after
  building elsewhere.
- `optional-desktop-interaction-smoke-times-out-during.md` — The optional desktop smoke times out during normal
  deletion.
- `hero-publisher-self-test-requires-different-hashes-potentially.md` — The publisher self-test could reject two valid
  identical commits.
- `light-theme-documentation-headings-have-unreadably-low-contrast.md` — Light-theme section headings have insufficient
  contrast.
- `cancelled-attach-retain-abandoned-terminal-channel.md` — Cancellation before attach transmission could retain local
  terminal-routing metadata.
- `wedged-browser-teardown-test-pass-browser-cleanup-finishes.md` — The wedged-browser test observes a detach that does
  not prove handler completion.
- `shutdown-leaves-retained-supervisor-clients-active.md` — Manager shutdown could leave retained clients active in a
  surviving runtime.
- `wedged-browser-test-accepts-detach-handler-completion.md` — A matching detach frame can precede browser-handler
  cleanup.
- `wedged-browser-test-accepts-independent-cleanup-signal.md` — The cleanup signal in the wedged-browser test has
  independent producers.
- `commit-race-test-exercise-only-pre-commit-abort-path.md` — The upload-abort test could finish before commit is
  actually in flight.
- `distinct-payload-test-observe-payload-bytes.md` — The distinct-payload test could accept installing the wrong
  executable bytes.
- `concurrent-first-use-barrier-guarantees-disputed-interleaving.md` — The first-use test could pass without exercising
  concurrent absent observations.
- `replay-refusal-tests-do-directly-observe-absence-delete-plain-replace.md` — Plain Replace refusal tests could miss an
  unintended asynchronous Delete.
- `replay-refusal-tests-do-directly-observe-absence-delete-replace-with-overrides.md` — Replace with overrides has a
  separate unobserved-Delete test boundary.
- `ignored-transcript-feeder-write-errors-conceal-missing-input.md` — A failed transcript feeder could make
  query-stripping tests pass with no input.
- `stopped-fixture-could-survive-assertion-failure.md` — The stopped-process fixture has failures outside any active
  cleanup owner.
- `recreation-test-assumes-distinct-birth-timestamps.md` — Immediate directory recreation could preserve the identity
  the test expects to differ.
- `replacement-tests-overstate-immediate-list-no-delete-evidence.md` — Replacement tests refresh away the
  immediate-cache regression.
- `wedged-browser-test-may-encounter-queue-backpressure-blocked.md` — Queue overflow can satisfy the wedged-browser test
  before socket cleanup.
- `shutdown-tests-prove-session-existence-more-strongly.md` — Supervisor-shutdown tests could accept a dead agent in a
  retained pane.
- `partial-shell-result-parsing.md` — Prefix-only shell waits could parse incomplete directory or environment results.
- `post-stall-liveness-witness-include-replay.md` — The post-detach progress test could accept records from old replay.
- `sink-healing-witness-consume-queued-output.md` — The healed-sink test could consume output queued before the failure.
- `unbounded-test-discovery-tmux-binaries.md` — Test discovery could hang on a tmux executable production would never
  select.
- `client-log-test-establish-claimed-later-capture.md` — The client-log test could pass with later capture permanently
  disabled.
- `recently-modified-image-png-gets-generated-attachment.md` — A mixed paste could silently omit a distinct fresh image
  file.
- `legacy-launch-validation-could-prevent-startup.md` — A historical launch shape could make the whole supervisor refuse
  startup.
- `feed-retained-browser-sign-in-interruption-falsely-represents.md` — Lost browser storage could leave an invisible
  feed counted as an available GUI.
- `template-supplied-session-names-rendered-raw.md` — A template's saved session name could be misleading in the
  launcher.
- `recursive-selection-across-very-long-wrapped-word.md` — Selecting a very long wrapped word could exhaust the
  browser's stack.
- `watchdog-logs-rather-recovering-window.md` — A persistent desktop bridge outage could accumulate native query
  entries.
- `mouse-handlers-surviving-disposal-during-drag.md` — Disposal during a lost mouse release could leave document
  handlers attached.
- `save-followed-immediately-delete-restores-outdated-local.md` — A queued Delete after Save could give Undo an outdated
  template.
- `concurrent-starts-bypass-sweep-serialization.md` — Concurrent sweep starts could publish the losing run as current.
- `intentional-child-passes-because-it-failed-wrong.md` — The intentional teardown-failure child could also fail its
  body and still pass the parent.
- `long-cwd-test-never-supplies-long-path.md` — The long-folder test could pass after ellipsis styling is removed.
- `retarget-test-failing-deliver-old-target-plan.md` — The retarget test could miss a stale plan completing after
  invalidation.
- `unreachable-operation-test-prove-absence-queued-work.md` — The unreachable-host test could accept queuing a refused
  Stop for later.
- `cleanup-leaves-orphan-tmux-processes-kill-failure.md` — Failed or skipped tmux shutdown could leave an orphan without
  its socket path.
- `missing-workspaces-cleanup-diagnostic-recognized-despite-discarded.md` — Discarded workspace errors could prevent
  restoring test preferences.
- `release-smoke-mistake-another-listener-child.md` — Release smoke could validate another listener instead of its
  launched helm.
- `process-death-falsely-inferred-proc-read-errors.md` — A later process-read error could be mistaken for confirmed
  death.
- `work-order-progress-reads-observe-empty-file-during.md` — An empty progress baseline could make old output look newly
  produced.
- `browser-children-retain-detached-descendants-every-platform.md` — Non-Linux supervision could leave detached browser
  descendants alive.
- `screenshot-runs-collide-through-shared-fleet-auth.md` — Overlapping screenshot commands could interfere through
  shared files.
- `shared-authentication-files-causing-concurrent-run-interference.md` — Overlapping runs could remove each other's
  authentication state.
- `held-synthetic-delete-handlers-delay-failure-teardown.md` — Failed assertions could leave synthetic DELETE handlers
  blocking teardown.
- `descendant-surviving-markeddecoy-destruction.md` — A decoy's sleep child could survive cleanup on non-procfs
  platforms.
- `guest-addresses-account-names-inject-ssh-configuration.md` — An IPv6 scope suffix could inject text into guest SSH
  configuration.
- `replaced-evidence-repeated-continuity-receipts-retain-passing.md` — Evidence replaced during digest capture could
  inherit a passing validation.
- `decoy-paths-derived-shell-pid.md` — A predictable temporary directory could redirect a test write through a symlink.
- `aggregate-identity-skew-creates-false-host-match.md` — An aggregate identity mismatch remains an unresolved
  transition question.
- `dropping-upload-guard-outside-tokio-runtime-skips.md` — Dropping an upload owner outside runtime context could skip
  cleanup.
- `detached-terminal-cleanup-tasks-wait-writer-capacity.md` — Repeated detach cleanup could accumulate tasks waiting for
  writer capacity.
- `oversized-successful-agent-reply-becomes-internal.md` — An oversized success reply could be reported as an internal
  failure after mutation.
- `pi-getter-serialization-exceptions-escaping-silent-hook-boundary.md` — A failed Pi callback could leave Resume
  pointing at an earlier conversation.
- `unbounded-aggregate-capture-replies.md` — Aggregate terminal capture could exceed a safe memory bound.
- `startup-verification-skipped-no-stale-client-initially.md` — An initially empty startup roster could miss a late
  predecessor client.
- `live-panes-process-read-error-degrades-pid-only-identity.md` — A live-pane identity read failure could admit an old
  report by process number alone.
- `omp-context-changes-initial-eligibility-check.md` — A retained reporter context leaves an unresolved
  foreground-ownership question.
- `failed-sink-record-hides-older-reaper.md` — An untracked older reaper could make planned shutdown finish cleanup too
  early.
- `deletes-timed-out-sink-shutdown-leaks-output-client.md` — Delete could leave a still-tracked output client retrying
  after timeout.
- `checkout-discovery-strips-valid-trailing-path-characters.md` — The test recorder identifies the wrong checkout when
  its name ends in whitespace.
- `diagnostic-writers-survive-their-deadline-accumulate-across.md` — Diagnostic timeouts leave writers running across
  repeated attempts.
- `non-fragment-files-satisfy-changelog-coverage.md` — Changelog coverage accepts files that fragment discovery never
  loads.
- `uninstall-documentation-promises-protection-modified-mac-app.md` — The uninstall guide promises Mac content
  protection the product does not provide.
