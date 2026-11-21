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

- [`folder-suggestion-enter-can-submit-launcher.md`](folder-suggestion-enter-can-submit-launcher.md) — Enter on a
  visible folder suggestion can submit the launcher. A successful Replace with can retire the source session.

- [`quick-switcher-enter-can-run-a-different-template.md`](quick-switcher-enter-can-run-a-different-template.md) — If
  the template list changes, Enter can start a different saved setup from the one still shown as selected.

- [`antigravity-historical-restart-can-target-child.md`](antigravity-historical-restart-can-target-child.md) — After an
  update, Restart can stop Antigravity’s main session and try to resume a child conversation recorded by the previous
  version.

- [`guarded-delete-replace-omits-surviving-shell-work.md`](guarded-delete-replace-omits-surviving-shell-work.md) —
  Delete or Replace can end surviving main-shell work after a confirmation that mentions only extra tabs or says nothing
  is running.

- [`shell-lifecycle-approval-names-wrong-program.md`](shell-lifecycle-approval-names-wrong-program.md) — Agent-requested
  Stop or Delete can ask approval to terminate an agent even though accepting the card terminates the shell and its
  work.

- [`shared-environment-delete-omits-live-shell.md`](shared-environment-delete-omits-live-shell.md) — Deleting a shell in
  a shared environment can stop its live processes even though the confirmation only mentions stopping processes if the
  whole environment is deleted.

- [`bulk-action-uses-live-shell-guard.md`](bulk-action-uses-live-shell-guard.md) — Marked Stop or Delete can act
  immediately using newer confirmation settings or liveness than the user saw.

- [`bash-stop-loses-unsaved-history.md`](bash-stop-loses-unsaved-history.md) — Stop and Stop and suspend can force Bash
  down before it saves command history configured to write on exit.

- [`tabs-only-stop-ends-surviving-shell-jobs.md`](tabs-only-stop-ends-surviving-shell-jobs.md) — A Stop confirmation
  that only names extra tabs can also end background work left by the main shell.

## High priority: material UX degradation

- [`late-bound-agent-host-misses-adoption.md`](late-bound-agent-host-misses-adoption.md) — If an interrupted terminal
  starts only after its replacement supervisor finishes both startup scans, the agent can keep running while Farhelm
  cannot open its terminal. Another supervisor restart is needed to find it.

- [`retired-runtime-retains-restart-seed-reply.md`](retired-runtime-retains-restart-seed-reply.md) — An input/paste
  reserved before retirement can remain retained with no receipt; the same late-send lifetime on the runtime's control
  channel can also leave an accepted Restart waiting for a history-seed reply after the runtime has ended.

- [`orphan-cleanup-overlay-forgets-failed-discovery.md`](orphan-cleanup-overlay-forgets-failed-discovery.md) — When
  discovery fails while cleanup remains unfinished, earlier orphan rows disappear and unrelated fleet updates keep
  retrying the failed read.

- [`credential-push-misses-retired-lifetime.md`](credential-push-misses-retired-lifetime.md) — A replacement connection
  can hide the last lease's zero transition, allowing obsolete credential transport work to delay reconnect. No
  disclosure, wrong-machine delivery or user-work loss is established.

- [`trailing-new-container-proxy-snapshot-capability-cache.md`](trailing-new-container-proxy-snapshot-capability-cache.md)
  — New containers can permanently inherit a cached host proxy environment. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-archive-trash-epoch-lacks-unconditional-fleet-wake.md`](trailing-archive-trash-epoch-lacks-unconditional-fleet-wake.md)
  — Archive epoch can change without waking other clients. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-podman-rebuild-retires-fallback-before-success.md`](trailing-podman-rebuild-retires-fallback-before-success.md)
  — Failed container Rebuild discards the previous usable images. Abbreviated collection; confidence, proposed
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

- [`trailing-environment-delete-docs-overstate-admission-timeout-as-decision-bound.md`](trailing-environment-delete-docs-overstate-admission-timeout-as-decision-bound.md)
  — Delete documentation overstates the admission timeout's decision guarantee. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`omitted-session-row-drops-live-tabs.md`](omitted-session-row-drops-live-tabs.md) — A list update can take away the
  shell tab you’re using and make the agent terminal receive your next typing. The shell on the host can keep running.

- [`interrupted-upload-notice-lost-on-session-removal.md`](interrupted-upload-notice-lost-on-session-removal.md) — When
  you switch away during an upload and the old session is replaced or deleted, Farhelm can cancel the upload without
  ever showing you that it stopped.

- [`upload-path-pastes-into-restarted-agent.md`](upload-path-pastes-into-restarted-agent.md) — If you restart a session
  during an upload, the file can finish normally while its path is pasted into the new agent, possibly in a different
  conversation.

- [`session-delete-shortcut-ignores-composition.md`](session-delete-shortcut-ignores-composition.md) — Pressing the
  session Delete shortcut while composing text can delete the open session, including its processes, when Skip session
  confirmations is enabled.

- [`wayland-terminal-paste-ignores-file-list.md`](wayland-terminal-paste-ignores-file-list.md) — Copying a file in a
  Wayland file manager and pasting into Farhelm can paste its text representation or do nothing instead of attaching the
  file.

- [`tensorlake-lifecycle-falls-back-to-unowned-name.md`](tensorlake-lifecycle-falls-back-to-unowned-name.md) — If
  Tensorlake setup refuses a sandbox name already in use, the failed Farhelm host can still offer Suspend or Resume and
  act on the existing sandbox.

- [`retargeted-ssh-alias-can-forget-live-container.md`](retargeted-ssh-alias-can-forget-live-container.md) — If an SSH
  alias starts reaching a different machine or login account, Farhelm can report the old container deleted and remove it
  from Hosts even though it is still on the original machine.

- [`podman-place-probe-bypasses-host-identity-refusal.md`](podman-place-probe-bypasses-host-identity-refusal.md) —
  Farhelm can treat two SSH entries for the same machine and account as usable container places even while its ordinary
  host connection refuses the duplicate.

- [`macos-terminal-paste-blocks-gui-thread.md`](macos-terminal-paste-blocks-gui-thread.md) — When macOS stalls while
  supplying clipboard contents, pasting into the terminal can freeze the whole Farhelm window.

- [`bulk-mute-can-reset-window.md`](bulk-mute-can-reset-window.md) — Muting or unmuting a large marked set can reset the
  desktop connection after applying only part of the batch.

- [`gone-terminal-retained-handle-hides-recovery.md`](gone-terminal-retained-handle-hides-recovery.md) — A terminal the
  supervisor knows is gone can stay in reconnecting state and hide the recovery card.

- [`linux-ctrl-f-leaves-settings-field-at-end.md`](linux-ctrl-f-leaves-settings-field-at-end.md) — Ctrl+F at the end of
  a Linux Settings field can focus Search and send the next keystroke there.

- [`automatic-update-misses-development-builds.md`](automatic-update-misses-development-builds.md) — A released Farhelm
  can overwrite a remote development supervisor if it speaks an older protocol. An update already in progress can also
  miss a development build installed before the final check.

- [`automatic-update-counts-busy-as-failure.md`](automatic-update-counts-busy-as-failure.md) — An outdated host can miss
  automatic updates until it reconnects or Farhelm reopens if a manual operation takes the host first and leaves it
  outdated.

- [`provisioning-waits-use-fleet-slots.md`](provisioning-waits-use-fleet-slots.md) — If four host operations are waiting
  for user input, another accepted operation can sit idle until one of those prompts is answered or cancelled.

- [`desktop-relaunch-sleep-depends-on-path.md`](desktop-relaunch-sleep-depends-on-path.md) — Restart to update can close
  the app without reopening it if the original PATH cannot find `sleep`.

- [`desktop-path-handoff-exceeds-linux-entry-limit.md`](desktop-path-handoff-exceeds-linux-entry-limit.md) — On Linux,
  an inherited PATH the app can start with can still prevent its supervisor from starting, including after the previous
  supervisor stops.

- [`deferred-template-changes-unpresented-launch.md`](deferred-template-changes-unpresented-launch.md) — A delayed
  template can change the program or wrapper before the launcher shows the change, and Launch can submit it in that
  interval.

- [`shell-switch-can-submit-hidden-conversation-id.md`](shell-switch-can-submit-hidden-conversation-id.md) — Entering an
  agent choice from a shell draft can submit a conversation ID typed earlier for a different agent.

- [`filtered-tab-reset-can-route-input-before-display.md`](filtered-tab-reset-can-route-input-before-display.md) — A
  session-list update can send a key to the main shell while the extra tab is still on screen.

- [`remembered-terminal-restore-keeps-unavailable-tab.md`](remembered-terminal-restore-keeps-unavailable-tab.md) —
  Reopening a session can leave it on a remembered shell tab that cannot attach, even while the main terminal is
  available.

## Other: correctness, diagnostics, cleanup, or convenience

- [`untagged-sandbox-leftover-labelled-another-helm.md`](untagged-sandbox-leftover-labelled-another-helm.md) — A
  leftover sandbox whose name has no helm tag is labelled as another helm's.

- [`canonical-folder-history-contract-conflict.md`](canonical-folder-history-contract-conflict.md) — Host canonical-path
  replies become remembered folder text, despite a product rule saying those replies must not choose remembered
  defaults. The implementation specification explicitly carries canonical paths into history; spelling changes are
  established, arbitrary path substitution is not.

- [`fresh-claude-trust-birthtime-contract-conflict.md`](fresh-claude-trust-birthtime-contract-conflict.md) — Initial
  fresh-checkout creation can edit Claude trust without the no-birth-time check stated elsewhere. Fresh-creation rules
  may intentionally be an exception; this is a source/specification conflict, not an established unauthorized
  replacement-folder grant.

- [`plan-deferral-survives-replacement-question.md`](plan-deferral-survives-replacement-question.md) — After the
  maintainer answers one large plan question, a later ordinary question from that plan can remain excluded from
  automatic handling. Whether the operational-artifact exclusion covers this workflow defect is ambiguous.

- [`stale-plan-answer-clears-new-question.md`](stale-plan-answer-clears-new-question.md) — A delayed answer can be
  attached to a plan's replacement question and make that plan eligible again, even when the new question was reserved
  for the maintainer. Actual unauthorized execution is unestablished.

- [`closed-conflicting-pr-does-not-wake-executor.md`](closed-conflicting-pr-does-not-wake-executor.md) — A waiting plan
  can remain unstarted after the last conflicting PR closes, until unrelated activity or a manual drain.

- [`old-executor-close-resets-new-holder-bookmarks.md`](old-executor-close-resets-new-holder-bookmarks.md) — An old
  executor's prescribed full-close cleanup can reset plan bookmarks already being used by the new holder.

- [`host-permission-copy-quotes-retired-labels.md`](host-permission-copy-quotes-retired-labels.md) — Specifications and
  helm refusal text quote old host-permission labels rather than the controls users now see.

- [`browser-shard-failure-sweeps-running-siblings.md`](browser-shard-failure-sweeps-running-siblings.md) — An early
  shard exception can clean up workers that sibling shards are still using. Additional reviewed scope: A Rust runner
  infrastructure or local evidence-save exception can sweep sibling sandboxes before their work and evidence collection
  settle.

- [`browser-shard-failures-exit-successfully.md`](browser-shard-failures-exit-successfully.md) — The browser harness can
  exit successfully even when its shards report validation failures. Additional reviewed scope: The Rust sandbox command
  can exit successfully even when its shard results say tests failed.

- [`worker-evidence-deleted-after-download-failure.md`](worker-evidence-deleted-after-download-failure.md) — The harness
  can delete a failed worker's evidence even when downloading that evidence failed.

- [`worker-allocation-precedes-ownership-ledger.md`](worker-allocation-precedes-ownership-ledger.md) — A created worker
  can fall outside cleanup when ledger writing fails or remote creation succeeds before the local request times out.

- [`anonymous-container-volumes-survive-removal.md`](anonymous-container-volumes-survive-removal.md) — Removing a
  container can leave its anonymous image volumes behind; whether Farhelm promises to remove them is unresolved.

- [`automatic-update-planning-serializes-hosts.md`](automatic-update-planning-serializes-hosts.md) — One host's planning
  or wait for its provisioning lock can delay automatic updates for later hosts.

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

- [`blocking-test-cleanup-prevents-async-listener-release.md`](blocking-test-cleanup-prevents-async-listener-release.md)
  — A normal successful supervisor unit test waits for the five-second host-cleanup deadline and retains its state
  directory because directory teardown blocks the same executor needed to release its aborted stand-in listener.

- [`input-test-comments-overstate-termination-coverage.md`](input-test-comments-overstate-termination-coverage.md) — The
  test comments claim stronger coverage and transport guarantees than the tests establish.

- [`steady-block-cursor-contract-conflict.md`](steady-block-cursor-contract-conflict.md) — Grid terminals deliberately
  blink a block cursor even when the program requests it steady, while the wire contract says blinking is the default
  unless the program says otherwise.

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

- [`restored-capped-title-loses-boundary-whitespace.md`](restored-capped-title-loses-boundary-whitespace.md) — Recovery
  can change a retained terminal title and its saved title-stack entries by removing trailing whitespace exposed by the
  title-size cap.

- [`host-lock-table-contradicts-startup-span.md`](host-lock-table-contradicts-startup-span.md) — Farhelm's maintenance
  documentation states a lifecycle-lock constraint that startup deliberately violates, obscuring which recovery
  operation may perform host I/O while holding the lock.

- [`refused-input-test-can-credit-earlier-bytes.md`](refused-input-test-can-credit-earlier-bytes.md) — The terminal
  input-backpressure regression's claim that rejected input never reaches the host is stronger than what it observes.

- [`shift-enter-test-lacks-reader-readiness.md`](shift-enter-test-lacks-reader-readiness.md) — This integration test can
  report success without establishing the already-waiting reader its documentation requires.

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

- [`restart-screen-captured-before-prompt-settles.md`](restart-screen-captured-before-prompt-settles.md) — Correct
  terminal recovery can fail the restart regression because its expected screen is captured before the fixture finishes
  writing the post-echo prompt.

- [`wide-wrap-placeholder-loses-final-cell-style.md`](wide-wrap-placeholder-loses-final-cell-style.md) — A wide glyph
  wrapping from the final column leaves a default-background hole where the terminal actually holds a styled blank; both
  screen and history use the conversion.

- [`partial-wide-erasure-loses-second-cell-background.md`](partial-wide-erasure-loses-second-cell-background.md) — A
  program erasing only the second column of a wide glyph with a different background can leave grid viewers painting the
  old first-cell background over that erased column.

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

- [`mixed-input-acknowledgements-misattribute-written-bytes.md`](mixed-input-acknowledgements-misattribute-written-bytes.md)
  — After a transient Linux terminal-write allocation failure, input discarded by the terminal host can be reported as
  written when a later input succeeds before their acknowledgements are sent. The conditional source route is
  established; production prompts are disabled, and no user-work loss is claimed.

- [`joined-link-spec-omits-cursor-tail-exception.md`](joined-link-spec-omits-cursor-tail-exception.md) — The broad
  joined-link comparison promise omits an accepted cursor-positioned tail exception.

- [`stable-recut-removes-retained-announcement-anchor.md`](stable-recut-removes-retained-announcement-anchor.md) — A
  different-version stable re-cut can remove the website anchor still targeted by carried announcements.

- [`recovery-gap-notice-lifetime-contract-conflict.md`](recovery-gap-notice-lifetime-contract-conflict.md) —
  Same-supervisor recovery exposes contradictory product and implementation promises for the missing-output notice's
  lifetime.

- [`successful-suspend-flashes-connecting.md`](successful-suspend-flashes-connecting.md) — A confirmed sandbox
  suspension can briefly change back to Connecting during its status recheck, even though the sandbox remains suspended.

- [`upload-failure-test-lacks-pipe-holder-witness.md`](upload-failure-test-lacks-pipe-holder-witness.md) — The
  failed-upload regression can pass without starting the pipe-holding fixture needed to test the original hang.

- [`brick-recovery-truncates-checkpoint-budget.md`](brick-recovery-truncates-checkpoint-budget.md) — A recoverable
  provider refusal can cut release checks below their required time budget or wrongly prevent their retry.

- [`sandbox-termination-poll-exceeds-phase-deadline.md`](sandbox-termination-poll-exceeds-phase-deadline.md) — Sandbox
  deletion can exceed its stated termination-polling allowance because each lookup receives a fresh timeout beyond the
  phase's remaining time.

- [`sandbox-status-diagnosis-bypasses-cleaning.md`](sandbox-status-diagnosis-bypasses-cleaning.md) — Accepted sandbox
  failure descriptions bypass normal cleaning and redaction before reaching clients. An actual provider description
  containing a credential remains unverified after investigation.

- [`retired-connection-leaves-suspend-progress-stuck.md`](retired-connection-leaves-suspend-progress-stuck.md) — A
  failed Suspend on a retired connection can leave the host showing Suspending with lifecycle controls disabled until a
  separate recovery action.

- [`unreadable-sandbox-ssh-key-reports-ready.md`](unreadable-sandbox-ssh-key-reports-ready.md) — A damaged or unreadable
  saved SSH key pair can still be presented as ready, postponing the failure until later sandbox operations.

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

- [`redirect-regression-omits-location-header.md`](redirect-regression-omits-location-header.md) — The redirect
  regression does not supply a redirect destination, so it cannot prove that Farhelm refuses to follow one.

- [`cancelled-ssh-capture-can-retain-local-readers.md`](cancelled-ssh-capture-can-retain-local-readers.md) — Cancelling
  an SSH capture may leave local helpers or pipe readers alive beyond the caller. Their actual surviving lifetime
  remains unverified after investigation.

- [`package-name-trailing-minus-requests-removal.md`](package-name-trailing-minus-requests-removal.md) — A recipe
  package ending in a minus sign is passed to apt with its removal meaning instead of being rejected or treated as a
  literal package name.

- [`recipe-destination-refuses-absolute-dns-name.md`](recipe-destination-refuses-absolute-dns-name.md) — A valid final
  DNS dot is refused in recipe tunnel destinations and agent endpoint hostnames.

- [`package-repair-does-not-wait-for-dpkg-lock.md`](package-repair-does-not-wait-for-dpkg-lock.md) — The repair step
  does not share the package installer's lock-wait policy and could fail under contention. A supported concurrent
  package-manager trigger remains unverified after investigation.

- [`apt-regex-fallback-can-accept-package-typo.md`](apt-regex-fallback-can-accept-package-typo.md) — A misspelled
  package may be accepted through apt's pattern fallback instead of failing plainly. The shipped distribution behavior
  remains unverified after investigation.

- [`snapshot-state-races-build-admission.md`](snapshot-state-races-build-admission.md) — Adding a host as a build
  finishes can select a just-deleted snapshot or start an unnecessary charged build.

- [`automatic-update-opt-out-can-roll-back.md`](automatic-update-opt-out-can-roll-back.md) — A power loss can roll back
  opting out of automatic updates and allow later unattended updates. Whether the general preference-loss acceptance
  covers this authorization remains unresolved.

- [`terminal-attachment-trace-retains-raw-session-id.md`](terminal-attachment-trace-retains-raw-session-id.md) —
  Terminal attachment diagnostics retain a raw peer-supplied session identifier instead of the promised cleaned
  presentation copy.

- [`version-agreement-lifetime-doc.md`](version-agreement-lifetime-doc.md) — The newly documented agreement between a
  program's terminal-version environment and a terminal query needs a lifetime qualification. A surviving program keeps
  its launch environment after a Farhelm update, while a new query after adoption receives the current emulator's
  version. The established consequence is documentation ambiguity only; no application failure is assumed.

- [`capture-failed-stop-erases-recovery.md`](capture-failed-stop-erases-recovery.md) — When the real-agent
  inspection/capture tool cannot confirm its terminal host stopped, it hides that failure and still removes the run
  directory needed to retry the supported stop command. Continued agent execution after the deletion has not been
  demonstrated.

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

- [`mac-host-start-same-build-contract.md`](mac-host-start-same-build-contract.md) — Remote Mac Update contradicts the
  declared same-build startup invariant; no incompatible startup data or runtime failure was demonstrated.

- [`centos-rustup-missing-pin.md`](centos-rustup-missing-pin.md) — Standalone CentOS validation cannot install its
  target with Rustup 1.28.0 when the pinned compiler is absent; current Rustup auto-install avoids this route.

- [`devbuild-rustup-active-name.md`](devbuild-rustup-active-name.md) — Mac devbuild misparses Rustup 1.28.0's multiline
  active-toolchain output and aborts preflight even with the compiler installed; current one-line output is unaffected.

- [`stopped-host-update-floor-unresolved.md`](stopped-host-update-floor-unresolved.md) — A stopped host running Farhelm
  below the minimum update version can still be updated without checking its installed version. Whether the
  minimum-version rule is intended to cover stopped supervisors remains unresolved.

- [`cleanup-retry-omits-recovery-step-times.md`](cleanup-retry-omits-recovery-step-times.md) — After a cleanup failure,
  Retry can report zero time for recovery steps that actually ran, weakening its diagnostic timing report.

- [`overdue-plan-protection-ages-out.md`](overdue-plan-protection-ages-out.md) — The plan picker can forget that a
  repeatedly skipped plan is overdue when older claims leave its bounded history.

- [`plan-pass-count-includes-precreation-claims.md`](plan-pass-count-includes-precreation-claims.md) — A plan with a
  long edit history can be treated as overdue using claims made before that plan existed.

- [`plan-pass-pagination-follows-moving-main.md`](plan-pass-pagination-follows-moving-main.md) — The plan picker can
  count an in-flight claim as a skipped pass when main changes while history pages are read.

- [`dependency-block-counts-held-plan-time.md`](dependency-block-counts-held-plan-time.md) — A plan temporarily blocked
  by an abandoned dependency can accumulate overdue passes from time it was explicitly on hold.

- [`orphan-cleanup-promises-unheld-provisioning-lock.md`](orphan-cleanup-promises-unheld-provisioning-lock.md) — The
  implementation specification promises that orphan cleanup checks ownership under the host provisioning lock, but the
  endpoint does not hold that lock. No supported destructive adoption race was established.

- [`trailing-testing-overview-falsely-generalizes-built-farhelm-host-program.md`](trailing-testing-overview-falsely-generalizes-built-farhelm-host-program.md)
  — Testing overview names the wrong executable for supervisor unit-test hosts. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-agent-launch-specs-disagree-receipt-relative-versus-post-allow-timeout.md`](trailing-environment-agent-launch-specs-disagree-receipt-relative-versus-post-allow-timeout.md)
  — environment launch contracts describe different effective waits. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-spec-terminal-switch-point-names-deleted-terminal-type.md`](trailing-spec-terminal-switch-point-names-deleted-terminal-type.md)
  — Rewritten terminal contract points to the deleted Terminal type. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-old-terminal-create-refusal-qualified-rollback-wording.md`](trailing-old-terminal-create-refusal-qualified-rollback-wording.md)
  — Rollback phrase references a policy that already retains allocated checkouts. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-spec-shim-container-execution-branch-retired.md`](trailing-spec-shim-container-execution-branch-retired.md)
  — Launch contract still describes the retired shim container branch. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-shift-enter-unconditional-pty-write-guarantee.md`](trailing-shift-enter-unconditional-pty-write-guarantee.md)
  — Qualify the grid Shift+Enter single-write guarantee. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-spec-report-discard-on-terminal-disappearance.md`](trailing-spec-report-discard-on-terminal-disappearance.md)
  — Report-discard contract does not match gone host terminals. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-delete-recorded-tab-scope-kept-derived-under-unusable-manager.md`](trailing-delete-recorded-tab-scope-kept-derived-under-unusable-manager.md)
  — Scoped-tab Delete policy gap predates the whole review range. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-http-golden-omits-populated-environment.md`](trailing-http-golden-omits-populated-environment.md) — The new
  environment host JSON is omitted from every golden contract row. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-agent-launch-placement-docs-obsolete-caller-contract.md`](trailing-agent-launch-placement-docs-obsolete-caller-contract.md)
  — Placement documentation misstates ordinary-create inputs and links a missing helper. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-test-responders-heartbeat-control-decode.md`](trailing-environment-test-responders-heartbeat-control-decode.md)
  — Environment test responders panic on valid heartbeat frames. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-recipe-template-management-refusal-denies-supported-agent-launch-use.md`](trailing-recipe-template-management-refusal-denies-supported-agent-launch-use.md)
  — Recipe-template refusals still claim agent use is GUI-only. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-agent-environment-missing-local-ssh-key-preflight.md`](trailing-agent-environment-missing-local-ssh-key-preflight.md)
  — Sandbox agent launches ask for approval before checking the local SSH key. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-history-admits-agent-origin.md`](trailing-environment-history-admits-agent-origin.md) — Agent
  environment launches enter GUI place-default and recent-history bookkeeping. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-environment-wait-comment-promises-a-relay-ordering-the-budgets-do-not-guarantee.md`](trailing-environment-wait-comment-promises-a-relay-ordering-the-budgets-do-not-guarantee.md)
  — Environment wait comment promises a relay ordering the budgets do not guarantee. Abbreviated collection; confidence,
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

- [`trailing-cancelled-trash-observation-test-misses-fleet-publication.md`](trailing-cancelled-trash-observation-test-misses-fleet-publication.md)
  — The cancellation test does not assert its committed fleet wake. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ui-deleting-environment-retains-running-reconnect-promise.md`](trailing-ui-deleting-environment-retains-running-reconnect-promise.md)
  — Deleting environment can promise its cached session is still running. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ui-stopped-container-waiting-place-offers-suspend.md`](trailing-ui-stopped-container-waiting-place-offers-suspend.md)
  — Stopped container offers Suspend when its container host is unavailable. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-sealed-setup-test-overlay-hides-hold-and-run-leak.md`](trailing-sealed-setup-test-overlay-hides-hold-and-run-leak.md)
  — Sealed-setup regression test cannot observe its claimed no-hold boundary. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-ordinary-tensorlake-add-outer-source-lock-across-ssh-readiness.md`](trailing-ordinary-tensorlake-add-outer-source-lock-across-ssh-readiness.md)
  — Ordinary Add holds recipe source exclusion while waiting for SSH. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-provisioning-shared-environment-preparation-progress.md`](trailing-provisioning-shared-environment-preparation-progress.md)
  — Environment preparation stays on Select the recipe source in launch progress. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-provisioning-cleanup-test-misstates-pre-0-18-tmux-ownership.md`](trailing-provisioning-cleanup-test-misstates-pre-0-18-tmux-ownership.md)
  — Cleanup test misstates legacy tmux ownership. Abbreviated collection; confidence, proposed dismissals and unresolved
  consequences are preserved in the entry.

- [`trailing-ssh-provisioning-test-private-server-coverage-docstring.md`](trailing-ssh-provisioning-test-private-server-coverage-docstring.md)
  — SSH integration test still claims private-tmux teardown coverage. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

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

- [`trailing-shared-recipe-container-base-summary-raw-presentation.md`](trailing-shared-recipe-container-base-summary-raw-presentation.md)
  — Recipe summaries render literal container-base formatting characters. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-account-observation-exec-guesses-root-home.md`](trailing-podman-account-observation-exec-guesses-root-home.md)
  — Custom-image login lookup unnecessarily requires /root. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

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

- [`trailing-missing-release-entry-test-count-does-not-prove-exact-variant-inventory.md`](trailing-missing-release-entry-test-count-does-not-prove-exact-variant-inventory.md)
  — Missing-entry fixture premise overstates its inventory check. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-detach-timeout-test-confuses-cooperative-yield-with-writer-capacity.md`](trailing-detach-timeout-test-confuses-cooperative-yield-with-writer-capacity.md)
  — The detach timeout test yields before establishing writer backpressure. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-place-typed-ssh-refusal-reasons-missing-final-ui-escaping.md`](trailing-podman-place-typed-ssh-refusal-reasons-missing-final-ui-escaping.md)
  — Typed SSH capacity reasons bypass final display escaping. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-ui-retained-whole-environment-question-stale-unreachable-rationale.md`](trailing-ui-retained-whole-environment-question-stale-unreachable-rationale.md)
  — Retained whole-environment warning can misstate current reachability. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-agent-clone-wire-docs-unconditional-host-requirement.md`](trailing-agent-clone-wire-docs-unconditional-host-requirement.md)
  — Clone wire documentation still forbids the new environment clone form. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-agent-create-cwd-help-omits-recipe-home-default.md`](trailing-agent-create-cwd-help-omits-recipe-home-default.md)
  — Create folder help omits the new recipe launch default. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-retired-ci-tmux-python-ignore-removed-without-residue-cleanup.md`](trailing-retired-ci-tmux-python-ignore-removed-without-residue-cleanup.md)
  — Unignoring an older non-self-ignored venv can add generated files to jj changes. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-podman-payload-cargo-artifact-identity-not-held-through-copy.md`](trailing-podman-payload-cargo-artifact-identity-not-held-through-copy.md)
  — Shared Cargo output can change before same-source payload packaging. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-deflake-current-driver-downgrades-historical-recorder-required-tmux-policy.md`](trailing-deflake-current-driver-downgrades-historical-recorder-required-tmux-policy.md)
  — Historical-base deflake runs lose their selected revision's required tmux check. Abbreviated collection; confidence,
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

- [`trailing-tab-settle-gate-bare-link-out-of-scope.md`](trailing-tab-settle-gate-bare-link-out-of-scope.md) —
  TabSettleGate retains an unresolved documentation link through the pinned tip. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-restart-definitive-comment-promises-no-external-effects.md`](trailing-restart-definitive-comment-promises-no-external-effects.md)
  — Definitive-Restart documentation overpromises absence of external effects. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-restart-reply-stale-tab-rediscovery-comment.md`](trailing-restart-reply-stale-tab-rediscovery-comment.md) —
  Restart reply comment promises a removed tab rediscovery path. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

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

- [`trailing-omp-capture-survives-owner-exit-test-loses-exit-barrier.md`](trailing-omp-capture-survives-owner-exit-test-loses-exit-barrier.md)
  — Host-backed teardown removes the owner-exit test's exit barrier. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-terminal-less-host-orphan-doc-asserts-no-survivors.md`](trailing-terminal-less-host-orphan-doc-asserts-no-survivors.md)
  — Terminal-less test documentation treats host loss as proof no program survives. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-supervisor-clone-doorway-testdoc-universal-host-requirement.md`](trailing-supervisor-clone-doorway-testdoc-universal-host-requirement.md)
  — Creating-verb test documentation still requires a host for every clone. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-delete-directory-admission-test-observes-only-earlier-agent-fence.md`](trailing-delete-directory-admission-test-observes-only-earlier-agent-fence.md)
  — Directory-admission test observes the earlier fence, not the blocked directory wait. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-stop-comment-falsely-bans-retaining-original-final-screen.md`](trailing-stop-comment-falsely-bans-retaining-original-final-screen.md)
  — Stop comment incorrectly says full-screen final frames are never retained. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-busy-host-list-limit-negative-oracle-never-reads-client-output.md`](trailing-busy-host-list-limit-negative-oracle-never-reads-client-output.md)
  — List-limit test checks an undrained reply cache. Abbreviated collection; confidence, proposed dismissals and
  unresolved consequences are preserved in the entry.

- [`trailing-request-conformance-target-testhost-owner-dropped-at-preparation-return.md`](trailing-request-conformance-target-testhost-owner-dropped-at-preparation-return.md)
  — Keep the target host alive through conformance observation. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-test-one-shot-sample-before-host-catch-up.md`](trailing-test-one-shot-sample-before-host-catch-up.md) —
  Persistence test can skip an uncaught-up host. Abbreviated collection; confidence, proposed dismissals and unresolved
  consequences are preserved in the entry.

- [`trailing-sampler-docs-promise-deleted-capture-failure-reset.md`](trailing-sampler-docs-promise-deleted-capture-failure-reset.md)
  — Sampler field documentation still promises the deleted capture-failure reset. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

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

- [`trailing-migrated-seal-constraint-test-accepts-duplicate-metadata-key.md`](trailing-migrated-seal-constraint-test-accepts-duplicate-metadata-key.md)
  — Migrated seal CHECK assertion accepts a duplicate-key failure. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-outcome-backfill-comment-falsely-promises-live-probe-after-legacy-retirement.md`](trailing-outcome-backfill-comment-falsely-promises-live-probe-after-legacy-retirement.md)
  — Outcome-backfill comment promises live reconciliation that legacy rows no longer receive. Abbreviated collection;
  confidence, proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-647f022-pre-reap-tab-live-only-attach-oracle-not-ported.md`](trailing-647f022-pre-reap-tab-live-only-attach-oracle-not-ported.md)
  — Dead-but-unreaped tab refusal gap remains applicable to host tabs at tip. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-70bc66a-host-size-wait-comment-denies-public-status-size.md`](trailing-70bc66a-host-size-wait-comment-denies-public-status-size.md)
  — Resize wait comment gives a false synchronization rationale. Abbreviated collection; confidence, proposed dismissals
  and unresolved consequences are preserved in the entry.

- [`trailing-terminal-tabs-shell-harness-docs-retain-deleted-conformance-caller.md`](trailing-terminal-tabs-shell-harness-docs-retain-deleted-conformance-caller.md)
  — shell harness docs still describe the deleted conformance battery. Abbreviated collection; confidence, proposed
  dismissals and unresolved consequences are preserved in the entry.

- [`trailing-terminal-tabs-shared-gate-docs-retain-removed-local-hook-installations.md`](trailing-terminal-tabs-shared-gate-docs-retain-removed-local-hook-installations.md)
  — shared gate docs claim hook installations that the module no longer has. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-delete-held-gone-host-skips-adopted-process-confirmation.md`](trailing-delete-held-gone-host-skips-adopted-process-confirmation.md)
  — Known Delete confirmation bypass is separate from guarded tab-close identity. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

- [`trailing-connection-writer-comment-points-to-deleted-outputstream-checkpoint.md`](trailing-connection-writer-comment-points-to-deleted-outputstream-checkpoint.md)
  — Connection writer diagnostics still point to the deleted stream checkpoint. Abbreviated collection; confidence,
  proposed dismissals and unresolved consequences are preserved in the entry.

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

- [`tunnel-listener-stops-on-single-accept-error.md`](tunnel-listener-stops-on-single-accept-error.md) — One listener
  error can leave a tunnel’s port visible in Farhelm after it stopped accepting connections. The next list refresh can
  remove the mapping without a reason.

- [`linux-docs-capture-follow-ups.md`](linux-docs-capture-follow-ups.md) — The README hero still uses the Mac workflow
  by default. A small Templates button crop would fail the publisher’s size check if a page uses it, and the template
  search crop catches a warning below the search box.

- [`late-delete-loses-checkout-archive-notice.md`](late-delete-loses-checkout-archive-notice.md) — If Delete takes long
  enough to time out while the host is still working, Farhelm can remove the session without telling you where its
  unarchived checkout was left.

- [`stop-and-suspend-hides-timeout-reason.md`](stop-and-suspend-hides-timeout-reason.md) — When Stop and suspend exceeds
  its reply deadline, Farhelm can leave you without the explanation that it is still working or that Stop already
  failed.

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

- [`checkout-use-default-presentation.md`](checkout-use-default-presentation.md) — Use default can clear a freshly
  loaded checkout-folder override before the editor shows it.

- [`failed-demo-rollback-can-remove-previous-preview.md`](failed-demo-rollback-can-remove-previous-preview.md) — A
  preview rollback that fails can leave the previous generated demo output in temporary files that are then deleted.

- [`group-divider-can-change-shift-sweep-range.md`](group-divider-can-change-shift-sweep-range.md) — Shift-dragging
  across group spacing can change the end of the marked session range.

- [`launcher-catalog-moves-model-highlight.md`](launcher-catalog-moves-model-highlight.md) — A model catalog update can
  move the highlight so Enter picks a different model for the launch draft.

- [`macos-shift-sweep-can-miss-release.md`](macos-shift-sweep-can-miss-release.md) — A macOS Shift sweep can keep
  scrolling and marking after a release with changed modifiers.

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

- [`add-host-close-reads-live-sign-in.md`](add-host-close-reads-live-sign-in.md) — Clicking Cancel SSH sign-in can
  cancel a later question or close Add Host and discard the destination and alias you typed.

- [`session-sign-in-suppresses-connection-diagnosis.md`](session-sign-in-suppresses-connection-diagnosis.md) — A session
  can say to answer a sign-in question to reconnect even when the host still needs an update or another Farhelm manages
  it.

- [`auto-sign-in-uses-session-anchor.md`](auto-sign-in-uses-session-anchor.md) — If the sidebar Answer button is below
  the fold, an automatic sign-in popover can open beside the selected session’s card instead.

- [`sidebar-sign-in-hides-launch-refusal.md`](sidebar-sign-in-hides-launch-refusal.md) — Sign in from the sidebar or the
  session card can appear to do nothing while a launch is pending. The reason shows only inside Hosts.

- [`new-host-attention-cue-can-be-lost.md`](new-host-attention-cue-can-be-lost.md) — A new reason to sign in can lose
  its visual cue when the host already has a cue pending or playing.

- [`attention-cue-loses-reduced-motion-flash.md`](attention-cue-loses-reduced-motion-flash.md) — On Linux, if the
  desktop reports reduced motion after Farhelm has enabled animation, an attention cue can show neither a flight nor a
  flash.

- [`devbuild-state-comparison-uses-wrong-path.md`](devbuild-state-comparison-uses-wrong-path.md) — Equivalent paths can
  apply different automatic-update defaults in a devbuild. This can save an opt-out in the installed app’s state, or
  leave updates on in separate development state.

- [`devbuild-startup-test-misses-first-scan.md`](devbuild-startup-test-misses-first-scan.md) — The startup test can pass
  if a devbuild starts its first automatic update scan before saving the opt-out. It also starts from a database that
  has already migrated.

- [`automatic-update-wake-test-accepts-host-refresh.md`](automatic-update-wake-test-accepts-host-refresh.md) — The test
  can miss that turning on automatic updates failed to wake the helm, because a delayed first host refresh can wake it
  instead.

- [`dialog-scroll-focus-loses-regression-check.md`](dialog-scroll-focus-loses-regression-check.md) — A test no longer
  checks that keyboard focus moves to a remaining control when a dialog’s focused scroll region stops being a tab stop.
  Trash still uses that handoff.

- [`attention-cue-tests-miss-witnesses.md`](attention-cue-tests-miss-witnesses.md) — The cue tests can pass without
  showing that an initially offscreen cue is refused, a visible cue plays when the window becomes active, or reduced
  motion paints a flash.

- [`attention-baseline-test-never-cues.md`](attention-baseline-test-never-cues.md) — The test can pass if a reconnect
  incorrectly animates existing warnings. Its first snapshot contains a host state that never produces a cue.

- [`sign-in-reveal-test-misses-first-frame.md`](sign-in-reveal-test-misses-first-frame.md) — The test can pass if the
  popover opens before its sidebar button scrolls into view.

- [`sign-in-focus-test-accepts-either-host.md`](sign-in-focus-test-accepts-either-host.md) — The explicit sign-in test
  can pass if the focused popover belongs to the other host in the fixture.

- [`sign-in-handoff-test-uses-connected-fixtures.md`](sign-in-handoff-test-uses-connected-fixtures.md) — The startup
  handoff test can pass without checking that focus moves to a host actually waiting for sign-in.

- [`inline-sign-in-tests-conflate-identifiers.md`](inline-sign-in-tests-conflate-identifiers.md) — The Add Host tests
  can pass if Send or Cancel uses the identifier for the whole sign-in instead of the individual question.

- [`docs-shot-cleanup-hides-primary-error.md`](docs-shot-cleanup-hides-primary-error.md) — If a capture and its cleanup
  both fail, the screenshot report can show only the cleanup error and carry the unrestored fixture into the next shot.

- [`docs-shot-guard-ends-before-cleanup.md`](docs-shot-guard-ends-before-cleanup.md) — A screenshot can be accepted
  without checking whether its cleanup changed the staged sessions that the next shot will use.

- [`docs-group-shot-lacks-drawn-membership-check.md`](docs-group-shot-lacks-drawn-membership-check.md) — The release
  screenshot can pass its checks without confirming that the moved session was drawn under its new group.

- [`docs-group-shot-wait-exceeds-budget.md`](docs-group-shot-wait-exceeds-budget.md) — If the staged helm stalls, the
  group screenshot’s ten-second membership wait can spend about a minute on one request.

- [`test-summary-ledger-boundary-crosses-files.md`](test-summary-ledger-boundary-crosses-files.md) — A malformed heading
  in one selected flake ledger can invalidate the final cause tag in a different ledger.

- [`test-summary-archive-test-bypasses-cli.md`](test-summary-archive-test-bypasses-cli.md) — The reporting test can pass
  if the command stops loading the closed-flake archive by default or ignores a repeated `--flakes` option.

- [`brick-test-deferral-reuses-continuity-files.md`](brick-test-deferral-reuses-continuity-files.md) — The release test
  can turn a failed or unavailable remote update into a harness failure when it carries forward receipt filenames from a
  different checkpoint.

- [`test-summary-asserts-total-instead-of-flake-classes.md`](test-summary-asserts-total-instead-of-flake-classes.md) —
  The test can pass if the summary counts both fixture flakes in the same class.

- [`ssh-chime-test-accepts-pending-draft.md`](ssh-chime-test-accepts-pending-draft.md) — The chime test can pass while
  the switch still shows its draft choice, even if it never starts following the saved setting.

- [`sign-in-notice-test-omits-unrelated-focus-case.md`](sign-in-notice-test-omits-unrelated-focus-case.md) — The tests
  can pass if a question from a different sign-in takes focus when an open notice ends.

- [`systemd-isolation-comments-point-to-active-ledger.md`](systemd-isolation-comments-point-to-active-ledger.md) — The
  new test comments send maintainers to the active flake ledger for evidence that has moved to the archive.

- [`test-summary-partial-test-ignores-usable-run-count.md`](test-summary-partial-test-ignores-usable-run-count.md) — The
  partial-report test can pass if an unavailable flake ledger causes the summary to lose valid test-run counts.

- [`working-attention-row-hides-warning-edge.md`](working-attention-row-hides-warning-edge.md) — Starting work on a host
  can remove its attention row’s amber warning edge before an earlier setup or update warning is acknowledged.

- [`antigravity-first-refused-pin-test.md`](antigravity-first-refused-pin-test.md) — The Antigravity tests can pass
  despite recording a process identity from a rejected first callback.

- [`desktop-ignores-empty-login-path.md`](desktop-ignores-empty-login-path.md) — An explicitly empty login PATH triggers
  fallback and makes tools available that the shell excluded.

- [`notice-panel-marks-offscreen-rows.md`](notice-panel-marks-offscreen-rows.md) — Opening and closing Notifications can
  mark an off-screen PATH warning as read. Clear resolved can also remove unseen history.

- [`desktop-path-notice-first-read-untested.md`](desktop-path-notice-first-read-untested.md) — The startup tests can
  pass if the first client misses the login PATH warning.

- [`shell-status-samples-visible-tail.md`](shell-status-samples-visible-tail.md) — A visibly active shell can show Idle
  when its output changes above the sampled area. This can also suppress the extra Suspend warning.

- [`shell-main-tab-spec-still-says-agent.md`](shell-main-tab-spec-still-says-agent.md) — The session-view spec still
  tells Bash and Zsh users to look for an Agent tab instead of Shell.

- [`shell-idle-spec-omits-sampling-cadence.md`](shell-idle-spec-omits-sampling-cadence.md) — The shell description can
  understate how long a quiet shell takes to show Idle on a host with many sessions.

- [`shell-recovery-spec-omits-replace.md`](shell-recovery-spec-omits-replace.md) — The Status spec omits Replace as a
  way to recover an interrupted Bash or Zsh session.

- [`shell-idle-plays-agent-turn-finished.md`](shell-idle-plays-agent-turn-finished.md) — The optional Turn finished
  sound can play when a Bash or Zsh screen goes quiet, even while its command is still running.

- [`shell-template-clears-dormant-prompt.md`](shell-template-clears-dormant-prompt.md) — Applying a Bash or Zsh template
  can clear a draft prompt that selecting the same shell by hand would keep.

- [`shell-wrapper-help-names-agent.md`](shell-wrapper-help-names-agent.md) — A shell’s wrapper field says it runs before
  an agent command.

- [`shell-picker-split-by-model.md`](shell-picker-split-by-model.md) — The model field can split the agent and shell
  choices, moving the Shells group as the selection changes.

- [`shell-template-hides-incompatible-permissions.md`](shell-template-hides-incompatible-permissions.md) — The template
  editor says a shell has no approvals while retaining incompatible approvals that block saving.

- [`host-suspend-warning-calls-shell-agent.md`](host-suspend-warning-calls-shell-agent.md) — Hosts calls an active Bash
  or Zsh session an agent when asking to suspend.

- [`shell-approval-command-facts-untested.md`](shell-approval-command-facts-untested.md) — Approval tests check that the
  shell lacks a Permissions row but could miss a missing shell name or command.

- [`shell-template-approval-misstates-permission.md`](shell-template-approval-misstates-permission.md) — Approval cards
  can say a shell template has no permissions while saving a concrete incompatible setting.

- [`suspended-shell-card-names-agent.md`](suspended-shell-card-names-agent.md) — A suspended shell’s card says an agent
  command is preserved.

- [`suspend-treats-failed-session-refresh-as-complete.md`](suspend-treats-failed-session-refresh-as-complete.md) —
  Suspend can skip its activity question when the host stays connected but Farhelm could only read an older idle or
  empty session list.

- [`shell-stop-and-suspend-wording-untested.md`](shell-stop-and-suspend-wording-untested.md) — Stop and suspend tests
  can pass if its confirmation goes back to calling Bash or Zsh an agent.

- [`shell-tab-label-omitted-row.md`](shell-tab-label-omitted-row.md) — An open shell can show an Agent tab when the
  session list omits its row.

- [`shell-stop-suspend-error-names-agent.md`](shell-stop-suspend-error-names-agent.md) — A partial Stop and suspend
  failure can say an agent stopped after stopping Bash or Zsh.

- [`shell-stop-tab-only-help.md`](shell-stop-tab-only-help.md) — When only extra tabs remain, the Stop dialog’s help
  still says it will stop the shell.

- [`shell-session-e2e.md`](shell-session-e2e.md) — No end-to-end test starts a first-class Bash or Zsh session through
  its normal launch path.

- [`late-tab-restore-hides-upload-notice.md`](late-tab-restore-hides-upload-notice.md) — An interrupted upload can go
  unreported while the reopened session is usable but still missing from the list.

- [`retired-terminal-comment-names-agent-recipient.md`](retired-terminal-comment-names-agent-recipient.md) — The
  terminal retirement comment still sends maintainers looking for notices on Agent even when they follow the selected
  shell.

- [`tensorlake-lifecycle-loses-running-evidence.md`](tensorlake-lifecycle-loses-running-evidence.md) — A later outside
  pause can be credited to an old Suspend after an explicit host operation already saw the sandbox running.

- [`tensorlake-timestamp-clear-delayed-by-dial.md`](tensorlake-timestamp-clear-delayed-by-dial.md) — Quitting during
  supervisor connection retries can preserve an old Suspend time that Farhelm already knows is obsolete.

- [`tensorlake-reprobe-moves-observed-time.md`](tensorlake-reprobe-moves-observed-time.md) — An interruption in
  Tensorlake status checks can make the same suspension appear to have started later.

- [`tensorlake-stale-registry-skips-timestamp-clear.md`](tensorlake-stale-registry-skips-timestamp-clear.md) — A failed
  registry refresh can prevent a successful reconnect from clearing an old Suspend timestamp.

- [`tensorlake-quit-rewrites-cleared-suspend-time.md`](tensorlake-quit-rewrites-cleared-suspend-time.md) — Quitting
  while Suspend finishes can put back an old timestamp that Farhelm had already cleared.
