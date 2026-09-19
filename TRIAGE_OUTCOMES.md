# Triage outcomes

## abandon-upload-waits-unboundedly.md

- Outcome: `fix spec`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. Supervisor upload cleanup awaits blocking
  filesystem removal without a deadline (`crates/farhelm-supervisor/src/service/uploads.rs`, `abandon_upload`). A
  refused commit can hold the session lifecycle claim during that wait, and teardown waits for upload completion. The
  helm's affected upload request can remain pending, but its reply wait releases the shared request-map lock and does
  not block a runtime thread. No existing spec provision was found that explicitly accepts filesystem hangs. This
  inspection establishes the request's asynchronous wait, not an exhaustive proof of host isolation under filesystem
  failures.
- Decision: assume each host's local filesystem is healthy. Local filesystem I/O errors or hangs are allowed to halt
  progress or cause failures, and do not justify added recovery complexity as long as the impact is localized to the
  host whose filesystem is broken. The helm must continue to function: one remote host must not freeze or break the
  helm. This applies to the supervisor host's filesystem, not only the machine running the helm.
- Completion criteria: update the authoritative specs to state this general operating assumption and host-isolation
  boundary, rather than suppressing only this finding. No code or implementation-comment changes are part of this
  spec-only outcome. Do not treat permission for host-local failure as permission for a remote host to freeze or break
  the helm; if execution finds such a cross-host effect, surface it to the user rather than silently broadening this
  outcome. Delete the feedback file and its `review_feedback_queue/INDEX.md` entry in this item's execution PR.
- Execution: `complete`; applied immediately under the spec-only triage rule. jj change:
  `pyqsmmqtwqzyolsqwoyqxzlyytxkknvo`; bookmark: `triage-healthy-filesystems`; draft PR:
  https://github.com/scode/farhelm/pull/741. Verified with targeted `dprint check`; no runtime changes or tests.

## abandoned-publish-reports-failure.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `commit_upload` can stop awaiting its
  blocking publication task on cancellation or timeout while `StagedStream::publish_no_clobber` continues and leaves a
  complete published attachment. The ordinary cancellation response says cancelled; the timeout path can falsely say
  nothing was stored. The filesystem-stall case falls under the agreed healthy-filesystem assumption, but cancellation
  or disconnection can race publication on a healthy filesystem too.
- Decision: the user accepts the proposed spec clarification and code change, without rollback machinery. Cancellation
  or disconnection during final publication may leave a complete attachment. It has ordinary attachment retention:
  deletion with the session, not cleanup on startup, Stop, or Archive. One copy costs no more space than an acknowledged
  upload; a retry may leave an additional copy because the client did not receive the first copy's path.
- Completion criteria: clarify the accepted final-publication race in the authoritative specs and correct code responses
  and comments that falsely guarantee nothing was stored or that abandoned publication is undone. Preserve ordinary
  cleanup before publication and existing no-clobber behavior. Do not add rollback, deduplication, or background undo
  machinery. Delete the feedback file and its `review_feedback_queue/INDEX.md` entry in the same execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/748/changes. jj change:
  `oluptvyouxtmyswnzxrkzutvonnpokyy`; bookmark: `triage-upload-publication`. The cancellation regression failed with the
  old response and passed with the fix. The attachment-upload e2e suite passed, covering definite refusal, late
  publication, retention through Archive, retry copies, and session deletion. Specs and code comments now distinguish
  failed publication from unknown completion; no rollback machinery was added. The feedback file and index entry were
  removed.

## adopt-publishes-unconditionally-after-commit.md

- Outcome: `fix code`.
- Assessment: the unconditional post-commit status overwrite is confirmed by current-code inspection, not runtime
  reproduction. Adoption can replace a concurrently published `Retired` status with `Connecting`. Its subsequent retry
  also checks task completion and channel closure, so it normally revives the dead actor despite the overwritten status.
  A persistent misleading state additionally requires revival to fail. No concrete trigger for the initial actor panic
  was established. `SPEC_impl.md` requires stopped actors to be represented as retired; the healthy-filesystem
  assumption does not exempt arbitrary actor failures.
- Decision: the user chose the small, isolated code fix after discussing the existing recovery guard. Preserve a
  concurrently published `Retired` status inside `HostManager::adopt`'s status-update closure rather than replacing it
  with `Connecting`. Keep the subsequent retry/revival path unchanged. This addresses the retained finding, not a
  broader redesign of adoption concurrency.
- Completion criteria: normal adoption still reconnects; concurrent retirement remains visible if revival fails, while
  successful revival publishes the replacement actor's state. Verify this boundary with a focused regression test.
  Delete the feedback file and its `review_feedback_queue/INDEX.md` entry in the same execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/749/changes. jj change:
  `lqpvyzvsnxnkzrlwywwyznzrrxkqlpmt`; bookmark: `triage-adoption-retirement`. The regression failed without the guard
  and passed with it. It gates the adoption commit, observes real actor retirement after an injected panic, forces
  revival to fail, and verifies the durable adoption and preserved retired state before a later retry reconnects. This
  is a controlled reproduction of the ordering, not evidence of a natural actor-panic trigger. The full manager test
  module passed, including ordinary adoption. The feedback file and index entry were removed.
