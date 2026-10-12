# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

The collected correctness and security review findings refer to commit `bd8d5d76439f8f36bbc2637b44d7ba4026b65675`. That
review stopped with known coverage gaps; the collection is not a whole-repository pass. Later product changes have not
been used to reassess these findings. Each file retains its reviewed commit, confidence, caveats, and provenance for
triage.

## Highest priority: security or data loss

None open.

## High priority: material UX degradation

- `uninstall-forgotten-success.md` — A failed uninstall can be reported as successful after another client forgets the
  host.

- `uninstall-race-first-time-service-setup.md` — Concurrent setup and uninstall can leave services pointing at a deleted
  executable.
- `plain-replace-loses-retry-identity.md` — Retrying plain Replace can launch another replacement session.

## Other: correctness, diagnostics, cleanup, or convenience

- `template-discovery-fails-permanently-once-reply-exceeds.md` — A large valid template catalog permanently disables
  agent discovery.
- `busy-selection-test-loses-source-second-creation-fails.md` — Failed busy-selection setup leaves acquired sessions
  outside cleanup.
- `pending-mount-cancellation-test-cannot-distinguish-cancellation-surviving.md` — The pending-mount test cannot tell
  cancelled retries from surviving retries.
- `sibling-close-concurrency-test-executes-both-closes-sequentially.md` — The sibling-close test could pass with a
  global serialization guard.
- `history-fixture-enables-yolo-installing-restoration-guard.md` — History-test setup can leak its changed confirmation
  policy.
- `keyboard-launch-fixture-has-same-separate-setup-ownership.md` — Keyboard-launch setup independently leaves changed
  policy after failure.
- `history-fixtures-policy-restoration-skip-remaining-cleanup.md` — History-test restoration failure skips cleanup of
  its known session.
- `keyboard-launch-fixtures-policy-restoration-skip-session-deletion.md` — Keyboard-launch teardown skips session
  deletion if policy restoration fails.
- `recently-modified-image-png-gets-generated-attachment.md` — A mixed paste could silently omit a distinct fresh image
  file.
- `retarget-test-failing-deliver-old-target-plan.md` — The retarget test could miss a stale plan completing after
  invalidation.
- `unreachable-operation-test-prove-absence-queued-work.md` — The unreachable-host test could accept queuing a refused
  Stop for later.
- `process-death-falsely-inferred-proc-read-errors.md` — A later process-read error could be mistaken for confirmed
  death.
- `work-order-progress-reads-observe-empty-file-during.md` — An empty progress baseline could make old output look newly
  produced.
- `held-synthetic-delete-handlers-delay-failure-teardown.md` — Failed assertions could leave synthetic DELETE handlers
  blocking teardown.
