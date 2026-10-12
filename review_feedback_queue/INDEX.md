# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

The collected correctness and security review findings refer to commit `bd8d5d76439f8f36bbc2637b44d7ba4026b65675`. That
review stopped with known coverage gaps; the collection is not a whole-repository pass. Later product changes have not
been used to reassess these findings. Each file retains its reviewed commit, confidence, caveats, and provenance for
triage.

## Highest priority: security or data loss

None open.

## High priority: material UX degradation

- `upload-cancellation-drops-final-reply.md` — Delete can discard an upload result during temporary connection
  backpressure, leaving the upload waiting forever even after the connection resumes normal traffic.

- `uninstall-forgotten-success.md` — A failed uninstall can be reported as successful after another client forgets the
  host.

- `uninstall-race-first-time-service-setup.md` — Concurrent setup and uninstall can leave services pointing at a deleted
  executable.
- `host-actions-disappear-supported-sidebar-widths.md` — A narrow sidebar could hide the actions needed for an
  unreachable host.
- `build-mismatch-turns-unattended-mounts-into-displacing.md` — Build mismatch can turn automatic recovery into an
  unsolicited takeover.
- `plain-replace-loses-retry-identity.md` — Retrying plain Replace can launch another replacement session.
- `newer-completed-run-leaves-older-update-permanently.md` — An old update can remain busy after a newer run has
  completed.
- `delayed-provisioning-confirmations-make-two-dialogs-mutually.md` — A delayed uninstall confirmation can disable both
  open dialogs.

## Other: correctness, diagnostics, cleanup, or convenience

- `session-detail-drains-full-list.md` — every fleet-revision bump makes each open session view trigger a full
  ListSessions on its host.
- `probe-reregister-drops-terminals.md` — probing an already-registered healthy host forces a reconnect that drops every
  open terminal on it.
- `header-actions-skip-listing-read.md` — header replace/restart request no listing read, so under build mismatch the
  sidebar keeps deleted/stale rows.
- `tab-reap-budget-starved-by-failures.md` — failed tab closes spend the per-tick reap budget, so a few persistent
  failures stop exited tabs from being reaped host-wide.

- `preview-ready-identity.md` — Preview startup can report an unrelated server as the docs preview.

- `failed-confirmation-discard-session-despite-failed-process.md` — Failed create recovery could discard the session
  needed to retry process cleanup.
- `template-discovery-fails-permanently-once-reply-exceeds.md` — A large valid template catalog permanently disables
  agent discovery.
- `ended-session-status-pushes-header-actions-beyond-promised.md` — An ended-session badge could clip actions at the
  promised pane width.
- `seen-state-queue-acknowledge-newer-choice-sending-it.md` — The read-state queue can report success without sending
  the user's latest choice.
- `rename-editor-conceals-title-characters-editable-title-editable-title-seed.md` — The rename editor hides meaningful
  characters in its editable title.
- `partial-templates-cannot-retain-non-yolo-approval-choices.md` — Approval-only templates cannot preserve supported
  partial settings.
- `session-refresh-failures-silently-retain-apparently-current.md` — Failed detail refreshes leave old session
  information looking current.
- `deflake-duplicates-bounded-evidence-into-unlimited-logs.md` — Test sweeps duplicate bounded evidence into unbounded
  logs.
- `busy-selection-test-loses-source-second-creation-fails.md` — Failed busy-selection setup leaves acquired sessions
  outside cleanup.
- `duplicate-scenario-titles-silently-conflate-distinct-sessions.md` — Duplicate scenario titles make capture checks
  refer to the wrong session.
- `constructor-font-regression-observes-mutable-value-construction.md` — The constructor-font test observes a value that
  can already have been corrected.
- `pending-mount-cancellation-test-cannot-distinguish-cancellation-surviving.md` — The pending-mount test cannot tell
  cancelled retries from surviving retries.
- `sibling-close-concurrency-test-executes-both-closes-sequentially.md` — The sibling-close test could pass with a
  global serialization guard.
- `classification-counts-runner-failures-deterministic-test-failures.md` — The failure classifier treats runner errors
  as proof of deterministic test failure.
- `history-fixture-enables-yolo-installing-restoration-guard.md` — History-test setup can leak its changed confirmation
  policy.
- `keyboard-launch-fixture-has-same-separate-setup-ownership.md` — Keyboard-launch setup independently leaves changed
  policy after failure.
- `history-fixtures-policy-restoration-skip-remaining-cleanup.md` — History-test restoration failure skips cleanup of
  its known session.
- `keyboard-launch-fixtures-policy-restoration-skip-session-deletion.md` — Keyboard-launch teardown skips session
  deletion if policy restoration fails.
- `window-restoration-uses-wrong-display-scale-macos.md` — macOS window restore applies the new display's scale to saved
  geometry.
- `unbounded-test-discovery-tmux-binaries.md` — Test discovery could hang on a tmux executable production would never
  select.
- `recently-modified-image-png-gets-generated-attachment.md` — A mixed paste could silently omit a distinct fresh image
  file.
- `feed-retained-browser-sign-in-interruption-falsely-represents.md` — Lost browser storage could leave an invisible
  feed counted as an available GUI.
- `template-supplied-session-names-rendered-raw.md` — A template's saved session name could be misleading in the
  launcher.
- `save-followed-immediately-delete-restores-outdated-local.md` — A queued Delete after Save could give Undo an outdated
  template.
- `retarget-test-failing-deliver-old-target-plan.md` — The retarget test could miss a stale plan completing after
  invalidation.
- `unreachable-operation-test-prove-absence-queued-work.md` — The unreachable-host test could accept queuing a refused
  Stop for later.
- `release-smoke-mistake-another-listener-child.md` — Release smoke could validate another listener instead of its
  launched helm.
- `process-death-falsely-inferred-proc-read-errors.md` — A later process-read error could be mistaken for confirmed
  death.
- `work-order-progress-reads-observe-empty-file-during.md` — An empty progress baseline could make old output look newly
  produced.
- `held-synthetic-delete-handlers-delay-failure-teardown.md` — Failed assertions could leave synthetic DELETE handlers
  blocking teardown.
- `guest-addresses-account-names-inject-ssh-configuration.md` — An IPv6 scope suffix could inject text into guest SSH
  configuration.
- `replaced-evidence-repeated-continuity-receipts-retain-passing.md` — Evidence replaced during digest capture could
  inherit a passing validation.
- `aggregate-identity-skew-creates-false-host-match.md` — An aggregate identity mismatch remains an unresolved
  transition question.
- `deletes-timed-out-sink-shutdown-leaks-output-client.md` — Delete could leave a still-tracked output client retrying
  after timeout.
- `diagnostic-writers-survive-their-deadline-accumulate-across.md` — Diagnostic timeouts leave writers running across
  repeated attempts.
- `rejected-permission-toggles-depend-observing-intermediate-busy.md` — A refused permission revocation could leave a
  misleading unchecked checkbox.
