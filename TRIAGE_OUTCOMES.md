# Triage outcomes

## ambiguous-restart-misattributes-exit.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. After an ambiguous restart, a live listing
  can treat the old dead pane as the new launch's exit and persist that exit, while supervisor reload correctly keeps
  the launch unknown. The same session can therefore report different outcomes across supervisor restarts and attribute
  the old run's exit code to the new generation.
- Decision: the user chose the bounded code fix. Treat `Launching` with a dead pane as `Unknown` and do not offer an
  observed-exit transition. Preserve the existing sentinel-first classification for genuine launch errors.
- Completion criteria: update both live status and observation paths, add focused regression coverage for the ambiguous
  dead-pane case, preserve genuine launch-error and ordinary exited-session behavior, and remove this feedback file and
  its index entry in the execution change.
- Execution: `complete`; `Launching` rows with a matching dead pane now remain `Unknown` and emit no observed-exit
  transition, while sentinel errors and established exits retain their behavior. Focused recorded nextest run
  `0d370584-9618-4571-bd07-80f8fe81b34a` passed both status regressions (858 skipped); formatting and isolated sleep
  checks passed. Draft PR [#917](https://github.com/scode/farhelm/pull/917/changes) is on bookmark
  `pr/ambiguous-restart-unknown`, jj change `8ad56228854a`.

## failure-suppressor-never-resets.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. A host actor keeps one text-keyed failure
  suppressor for its entire lifetime, shared by connection, protocol, refresh, collision, and cache-write diagnostics.
  After the limit is reached, identical warnings can disappear indefinitely, and suppression counts can be attributed to
  an unrelated failure after recovery or a change of failure kind.
- Decision: the user chose to state in the authoritative specification that diagnostic output on stderr may be noisy and
  may repeat indefinitely. Remove the suppressor and the state and call-site complexity it introduced. Keep bounded,
  escaped peer text and the existing one-line collision summary shape.
- Completion criteria: update the logging specification, remove the suppressor and its obsolete tests/comments, preserve
  safety bounds and meaningful diagnostic fields, and remove this feedback file and its index entry in the execution
  change.
- Execution: implemented in change `qmvpwnuqnyswyztnzmqmlzyyvrxtkpvt` on bookmark `pr/triage-failure-diagnostics`;
  [draft PR #887](https://github.com/scode/farhelm/pull/887/changes).

## folder-history-rename-unique-failure.md

- Outcome: `fix code`.
- Assessment: partly confirmed by current-code inspection and the reported SQLite constraint behavior. Folder-history
  refinement can encounter multiple unproven rows with the same display spelling; the bulk canonical-key update can then
  violate the unique canonical-path constraint and roll back the whole convenience-history refinement. The duplicate
  alias precondition is uncommon and was not observed in ordinary data. Browsing and session creation still work, but
  stale duplicate suggestions remain and later visits repeat the warning.
- Decision: the user chose the narrow code fix, conditional on it remaining simple. Keep one deterministic alias, remove
  the remaining same-display aliases in the same transaction, and do not redesign folder-history semantics. If the fix
  requires significant complexity or scope creep, defer it with the blocker documented in TODO.md instead.
- Completion criteria: duplicate-display aliases no longer make refinement fail; the retained row follows the existing
  ordering rules; focused regression coverage proves cleanup and subsequent successful refinement; remove this feedback
  file and its index entry in the execution change, or document the deferral and retain/narrow the item if the
  simplicity gate is reached.
- Execution: implemented in change `rsuwmxnppkqkpnooszsprtxykmoxywmy` on bookmark `pr/folder-history-duplicate-aliases`;
  [draft PR #889](https://github.com/scode/farhelm/pull/889/changes).

## folder-merge-drops-newer-alias-into-proven.md

- Outcome: `fix code`.
- Assessment: confirmed by code inspection, not runtime reproduction. When two saved folder suggestions resolve to the
  same directory, browsing can delete the newer suggestion without carrying its newer name and recency onto the row that
  remains. The folder operation still succeeds, but the recent-folder list can show stale suggestion text or ordering.
- Decision: the user chose a code fix only if it remains easy and local. Preserve the newer suggestion's visible name
  and recency while retaining the existing canonical-directory identity; do not redesign folder history. If that is not
  a simple change, defer it and document the blocker in TODO.md.
- Completion criteria: browsing cannot discard a newer folder suggestion in this case; focused regression coverage
  proves the visible name and ordering survive; remove this feedback file and its index entry in the execution change,
  or retain and narrow it if the simplicity gate requires deferral.
- Execution: implemented in change `vuszlpuxxxvqvtywrusyyrzttwqnvmkr` on bookmark
  `pr/folder-history-proven-presentation`; [draft PR #890](https://github.com/scode/farhelm/pull/890/changes).

## generic-session-accepts-placeholder-template.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by code inspection. A generic session can accept a configured restart command that needs a
  conversation ID, even though Farhelm has no way to obtain that ID for the session. The command is then never usable;
  restart silently falls back to a fresh conversation. The current specification has conflicting wording about generic
  fallback commands and needs the rule made explicit.
- Decision: the user agreed to reject such a command for generic sessions and clarify the specification. A generic
  restart command may be accepted only when it can run without Farhelm-supplied conversation identity.
- Completion criteria: update the authoritative specification, reject the invalid configuration at the existing
  validation boundary with an actionable error, add focused validation coverage, and remove this feedback file and its
  index entry in the execution change.
- Execution: implemented in change `xtpupovr` on bookmark `pr/generic-placeholder-validation`;
  [draft PR #895](https://github.com/scode/farhelm/pull/895/changes).

## getent-colonless-line-accepted-as-shell.md

- Outcome: `fix code`.
- Assessment: partly confirmed by code inspection. The login-shell parser accepts a separator-free account lookup line
  as the shell path, so malformed lookup output can bypass the existing direct-account fallback and make every launch
  fail with a nonexistent shell. The malformed response is hypothetical, but the parser behavior is verified.
- Decision: the user agreed to the narrow parser fix. Reject malformed lines that lack the expected separator, retain
  the warning, and let the existing fallback lookup run.
- Completion criteria: malformed separator-free output reaches the fallback path, valid and empty-shell records retain
  their current behavior, focused parser coverage is added, and this feedback file and its index entry are removed in
  the execution change.
- Execution: implemented in change `xtvwxywv` on bookmark `pr/getent-shell-fallback`;
  [draft PR #896](https://github.com/scode/farhelm/pull/896/changes).

## helm-upload-fast-path-spin.md

- Outcome: `fix code`.
- Assessment: unresolved as a production trigger, but the loop defect is confirmed by inspection. Repeated immediately
  ready empty upload chunks can bypass both waiting and deadline checks and keep the helm at full CPU indefinitely. The
  review did not establish that Hyper's production body stream can produce this shape.
- Decision: the user chose the small defensive code fix despite the reachability uncertainty. Empty fast-path items must
  go through the normal deadline-driven wait instead of immediately restarting the loop.
- Completion criteria: the relay cannot busy-loop on repeated empty chunks, the existing upload progress and stall
  behavior remains intact, focused regression coverage exercises the shape, and this feedback file and its index entry
  are removed in the execution change.
- Execution: implemented in change `ktzkpzzu` on bookmark `pr/empty-upload-stall`;
  [draft PR #898](https://github.com/scode/farhelm/pull/898/changes).

## normal-teardown-waits-unboundedly-on-detach.md

- Outcome: `fix code`.
- Assessment: confirmed by code inspection. Normal terminal teardown waits for the detach enqueue without the
  five-second bound already used by forced teardown paths, so a wedged connection retains the local teardown handler and
  attachment bookkeeping for the connection's roughly 60-second writer-stall window. The browser tab is already gone;
  the impact is delayed local cleanup and possible accumulation during repeated closures.
- Decision: the user chose to use the existing bounded detach helper in the normal path. Keep the distinct graceful and
  forced lifecycle paths, but remove the stale unbounded wait left behind after detach sends became independently owned.
- Completion criteria: normal close stops waiting after the existing teardown grace, the independent detach send remains
  active, forced teardown behavior is unchanged, focused teardown coverage passes, and this feedback file and its index
  entry are removed in the execution change.
- Execution: implemented in change `pnmnrvpw` on bookmark `pr/normal-teardown-detach`;
  [draft PR #900](https://github.com/scode/farhelm/pull/900/changes).

## orphaned-install-temps-on-managed-hosts.md

- Outcome: `fix code`.
- Assessment: confirmed by code inspection. An interrupted managed-host install can leave a payload-sized hidden
  temporary file behind, and each retry uses a new name that never reaches later cleanup. Repeated interruptions can
  consume disk space invisibly. The behavior conflicts with provisioning's rerun-and-recovery contract.
- Decision: the user chose a code fix with an explicit ownership boundary. Temporary artifacts must live in a directory
  Farhelm clearly owns, or use a narrowly unique Farhelm naming pattern that cannot match unrelated files. Cleanup may
  remove only artifacts proven to belong to Farhelm in the exact managed destinations.
- Completion criteria: interrupted-install leftovers converge away on a later run, unrelated files cannot be selected
  for deletion, cleanup remains best effort and logged, focused interruption/retry coverage proves the boundary, and
  this feedback file and its index entry are removed in the execution change.
- Execution: implemented in change `sovswwwl` on bookmark `pr/orphaned-install-temps`; draft PR
  [#905](https://github.com/scode/farhelm/pull/905/changes).

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

## adopt-request-silently-ignores-unknown-fields.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `AdoptReq` in
  `crates/farhelm-helm/src/hosts.rs` accepts unknown JSON fields, unlike neighboring host-add and alias requests.
  Missing `reported` still fails, and the manager still checks the approved identity and requires an identity mismatch.
  This is a low-impact request-validation inconsistency, not an identity-check bypass. Neither SPEC.md's explicit
  adoption requirement nor SPEC_impl.md's approved-identity check requires rejection of unknown fields.
- Decision: the user chose the proposed bounded code fix: reject unknown adoption-request fields, matching the
  neighboring request types. No spec change.
- Completion criteria: add `#[serde(deny_unknown_fields)]` to `AdoptReq`; verify that extra fields are rejected while
  valid requests and the existing identity checks retain their behavior. Remove the feedback file and its index entry in
  the same execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/761/changes. jj change:
  `okpsxlvtzzxmmkrtlpknskowwsuktvyl`; bookmark: `fix-adopt-unknown-fields`. The focused HTTP regression failed before
  the fix (200 instead of 422); seven adoption checks pass after the fix.

## agent-fence-claimed-before-validation.md

- Outcome: `fix code`.
- Assessment: partly correct, established by current-code inspection rather than runtime reproduction.
  `handle_restricted_control` claims the asking session's fence before pure verb validation and runs inline on that
  session-authenticated connection's read loop. Invalid requests therefore wait behind an existing holder unnecessarily.
  This does not establish a stall of other agents or ordinary helm/GUI controls: it is not the helm's shared connection.
  Ten minutes is the retained-mutation safeguard, not a general deadline on acquiring the fence. SPEC_impl.md requires
  the fence before credential validation, not before shape validation. SPEC.md excludes hostile same-account
  availability defense, but ordinary invalid requests can encounter this unnecessary wait too.
- Decision: the user chose the narrow code fix: validate request shape before acquiring the fence, while keeping
  credential validation under the fence for valid mutations. No scheduling redesign, new timeout, or spec change.
- Completion criteria: malformed verbs are refused without waiting for the asking session's fence; valid mutations
  retain the claim-before-credential-check ordering and existing mutation-lifetime protection. Verify the contention
  boundary with a focused regression. Remove the feedback file and its index entry in the same execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/762/changes. jj change:
  `woqnkkunnsuunnznmwqlsovrxyuymknl`; bookmark: `fix-agent-validation-before-fence`. The occupied-fence regression times
  out before the fix and passes after it. Four agent-request checks and the separate claim-before-credential-check
  regression pass.

## agentrequest-refusals-hold-fence-across-reply.md

- Outcome: `fix code`.
- Assessment: partly correct by current-code inspection, not runtime reproduction. `service/handlers.rs:3588-3646`
  retains the fence across refusal replies; the successful relay takes ownership correctly. The reply contract forbids
  waiting on the queue with a supervisor mutex held. The writer has a no-progress deadline, so indefinite freezing is
  overstated. SPEC_impl.md requires fencing credential validation and actual mutations, not refusal delivery.
- Decision: scheduled under the user's authorization for clear, small code fixes. Scope fence ownership around outcome
  construction and release it before refusal replies. Coordinate with the approved validation-order fix; preserve
  credential validation under the fence and successful relay ownership. No new timeout or task mechanism.
- Completion criteria: reply backpressure on a refusal no longer holds the deletion fence; successful mutations retain
  their existing lifetime protection. Remove this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/763/changes. jj change:
  `krslnlvxuqztxzzkkpzpzlvkqnrvqvvo`; bookmark: `fix-agent-refusal-fence-lifetime`. The reply-backpressure regression
  failed before the fix and passes after it, together with five related mutation and credential-ordering checks.

## archive-drops-permit-before-reply.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `service/handlers.rs:1540-1541` drops the
  successful archive's admission permit before metadata reconstruction and reply at 1567-1583. Error branches retain it.
  This contradicts the existing admission/reply lifetime contract; no hostile-client overload claim is needed.
- Decision: scheduled under the user's authorization for clear, small code fixes. Keep the existing permit in the reply
  task's scope through metadata reconstruction and response delivery. No new quota or scheduling mechanism.
- Completion criteria: archive success and metadata-read failure retain admission until the reply task finishes or is
  cancelled. Preserve mutation ownership. Remove this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/764/changes. jj change:
  `qsnstvqpwvqsyzvvkoyvtokkzmwulntx`; bookmark: `fix-archive-reply-admission`. The metadata-boundary regression
  reproduced early permit release; four focused checks pass after the fix, including reply cancellation and
  supervisor-owned mutation cancellation.

## attach-refuses-tombstoned-channel.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `service/handlers.rs:1614-1622` rejects
  any upload route, whereas BeginUpload and data routing check liveness. `service/uploads.rs:1323-1335` explicitly
  permits channel reuse after completion. No spec requires finished receipts to reserve channels.
- Decision: scheduled under the user's authorization for clear, small code fixes. Use the existing upload-route liveness
  predicate in both attach admission and diagnostic selection. Leave tombstone retention unchanged.
- Completion criteria: attach accepts a finished-upload channel but rejects live uploads, live input routes, channel
  zero and oversized leases. Remove this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/765/changes. jj change:
  `quoqtpvlslmpyonmrrrrvnzrxqozkpyq`; bookmark: `fix-attach-finished-upload-channel`. The real upload-to-terminal replay
  regression failed before the fix; it and two related upload-admission checks pass after the fix.

## clearing-local-alias-restores-colliding-name.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. Helm `store.rs:3961-3969` checks the
  restored local name only against explicit aliases, missing an unaliased SSH destination displaying that name. SPEC.md
  requires unique host display names; alias SET already compares effective names.
- Decision: scheduled under the user's authorization for clear, small code fixes. Check the restored local effective
  name against other effective names inside the alias-clear transaction, using existing derivation and AliasTaken.
  Coordinate with the destination-collision item without merging their outcomes or adding a reserved-name policy.
- Completion criteria: collision refuses the clear without changing the alias. A destination hidden by its own different
  alias is not falsely treated as the visible name. Remove this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/766/changes. jj change:
  `ymvortomnkutqlrzmoptpopunopukksk`; bookmark: `fix-local-alias-clear-collision`. The restored-name collision
  reproduced before the fix; all nine alias-update checks pass with the existing full-name scan shared by setting and
  clearing.

## create-mode-message-blames-wrong-caller.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `service/handlers.rs:359-362` gives a
  full-authority empty-selector create the restricted-spawn advice, although its resolver refuses those suggested
  selectors. Restricted dispatch already rejects missing selectors before this helper. SPEC.md requires actionable
  invalid-operation errors.
- Decision: scheduled under the user's authorization for clear, small code fixes. Correct this refusal to describe the
  invocation bundle the reachable full-authority path accepts. Keep admission and accepted request shapes unchanged.
- Completion criteria: the refusal no longer suggests a selector the next check rejects. No new authority abstraction or
  wording-only regression test. Remove this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/767/changes. jj change:
  `kwnuwumqrztromqskkrlqxtsopzlowom`; bookmark: `fix-full-authority-create-advice`. Before/after dispatcher smoke
  captured the misleading restricted-selector advice and the corrected invocation-bundle response. Both refusal-contract
  and normal restricted-create checks pass. Removed temporary output instrumentation and existing wording-only
  assertions.

## discarded-replacement-logs-spurious-row-gone-retire.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `manager.rs:1600-1609` returns normally
  when an unused replacement's start gate is dropped; 1626-1662 treats that as a missing registry row, logs retirement
  and bumps fleet events. This violates the existing side-effect-free discarded-start contract; no actor ran.
- Decision: scheduled under the user's authorization for clear, small code fixes. Distinguish never-started completion
  from a running actor ending with a small task-result value. Exit supervision silently for the former.
- Completion criteria: discarded gated replacements produce no false retirement or fleet invalidation; started actors
  ending or panicking still publish retirement. No new supervision framework. Remove this feedback file and its index
  entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/768/changes. jj change:
  `nukzlnkqkpqpmkpppyopkpnxzwtslmwo`; bookmark: `fix-discarded-actor-retirement`. The dropped-gate regression reproduced
  the false retirement and fleet revision. Four focused actor checks pass; the regression also passes with an isolated
  fixture that starts no unrelated actor.

## disconnect-publishes-keep-stale-contested-claims.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `manager.rs:1484-1502,1648-1656` withdraws
  clients/live lists but retains contested IDs that contested_claimants still reads. Ordinary disconnected publication
  clears them. SPEC_impl.md ties collision evidence to current reporting hosts.
- Decision: scheduled under the user's authorization for clear, small code fixes. Clear contested IDs in the existing
  atomic retarget and actor-retirement status mutations, beside withdrawing the client and live list.
- Completion criteria: obsolete claims from withdrawn connections no longer block routing. Failed refreshes on
  still-live connections retain their existing evidence policy. No collision-policy redesign. Remove this feedback file
  and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/769/changes. jj change:
  `mmvyvnxlntmnqyopouutywwkwzpsuwkk`; bookmark: `fix-withdrawn-host-collision-claims`. The retarget regression
  reproduced stale claims before the fix. Seven focused retarget, failed-refresh and actor-retirement checks pass after
  the fix.

## forget-splits-guarded-update-drops-fresh-contested.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `manager.rs:2024-2034` guards the
  in-memory list update by incarnation/client presence, but 2039-2049 clears contested evidence without that guard. The
  durable branch also awaits before this unguarded mutation. A stale delete can erase a replacement connection's
  collision evidence, contrary to the existing claim-bound update discipline.
- Decision: scheduled under the user's authorization for clear, small code fixes. Put the relevant in-memory list and
  contested changes under the existing incarnation/client guard at final publication for both storage branches. Preserve
  the cache-write lock and valid-delete epoch behavior; do not broaden this into actor-map publication redesign.
- Completion criteria: stale claims cannot remove replacement-connection collision evidence or report stale in-memory
  changes; valid deletes still clear their own row and claim. Remove this feedback file and its index entry in its
  execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/770/changes. jj change:
  `xznxtnmtlsmypqmpkoxtkszkrpzkqwlu`; bookmark: `fix-stale-delete-collision-publication`. The gated durable-delete
  regression reproduced loss of a newer collision claim. It and two related deletion/adoption checks pass with atomic
  guarded publication.

## in-memory-seed-skips-id-length-bound.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `manager.rs:1797-1934` lacks the seeded-ID
  bound enforced by drain validation and helm `store.rs:4514-4524`. SPEC_impl.md requires bounded IDs at every peer
  ingress and treats mutation seeding as best effort.
- Decision: scheduled under the user's authorization for clear, small code fixes. Apply MAX_SESSION_ID_BYTES before
  manager seed publication using established refusal behavior. Retain the store defense.
- Completion criteria: oversized IDs never enter the identity-less list; valid boundary-sized IDs remain accepted.
  Rejecting a seed must not turn a successful remote mutation into a reported failure. Remove this feedback file and its
  index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/771/changes. jj change:
  `uxkrpzpvvnwsytykkvvpnzrvovzuowop`; bookmark: `fix-identityless-seed-id-bound`. The regression accepted an oversized
  ID before the fix and now refuses it while retaining the boundary-sized ID. The existing best-effort mutation caller
  remains unchanged.

## local-identity-conflict-returns-500.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `provisioning/service.rs:473-483` erases
  non-Recorded FirstContactOutcome values into untyped errors. Existing HostStoreError mappings already express these
  conflicts as 409. SPEC.md treats identity changes as adopt-or-fix states, not internal malfunction.
- Decision: scheduled under the user's authorization for clear, small code fixes. Translate Mismatch, Collision and
  StaleAttempt exhaustively into their existing typed store errors; reuse HTTP mapping.
- Completion criteria: local discovery identity conflicts produce actionable conflict responses without changing
  identity or dial coordinates. Successful registration and internal errors retain their behavior. Remove this feedback
  file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/772/changes. jj change:
  `soyrnolxnmwzvkzkykypvpprkskyormm`; bookmark: `fix-local-discovery-identity-conflict`. The endpoint regression
  reproduced HTTP 500 before the fix and now returns HTTP 409 while retaining the recorded identity. Its fixture
  explicitly verifies that the original identity was stored.

## local-update-refusal-returns-500.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `provisioning/service.rs:594-599` refuses
  local UPDATE with untyped bail, becoming HTTP 500. SPEC.md deliberately hands local installation to setup; this
  refusal is policy, not server malfunction.
- Decision: scheduled under the user's authorization for clear, small code fixes. Add one message-carrying provisioning
  refusal variant mapped to 409 and use it for the existing local handoff. Share it with the manual-update sibling.
- Completion criteria: local update planning returns 409 with the existing handoff reason. Failure to obtain that reason
  remains a real failure; no local update plan becomes possible and no success-response schema changes. Remove this
  feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/773/changes. jj change:
  `mmwpzyrrlwwmytyrnzurmzlyzuymomkt`; bookmark: `fix-local-update-refusal-status`. The endpoint regression reproduced
  HTTP 500 before the fix and now returns 409 without retaining a plan. The ordinary unclassified-error mapping still
  returns 500.

## manual-update-refusal-returns-500.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `provisioning/service.rs:696-704` turns
  ReachOutcome::Manual into untyped bail and HTTP 500. SPEC.md treats unsupported automatic installation as a normal
  manual fallback.
- Decision: scheduled under the user's authorization for clear, small code fixes. Reuse the typed 409 refusal from
  `local-update-refusal-returns-500.md`; execute after that dependency. Preserve the original reason.
- Completion criteria: manual-needs update planning returns 409, backend failures remain errors and successful
  update-plan responses keep their schema. Remove this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/774/changes. jj change:
  `rpvntromzvvyxlkmuuqkoznvppkktnyu`; bookmark: `fix-manual-update-refusal-status`. The endpoint regression changed from
  HTTP 500 to 409 without retaining a plan. The combined run also preserved unclassified errors as HTTP 500.

## oversized-profile-relayed-before-size-check.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `service/handlers.rs:3337-3367` resolves a
  restricted profile before the shared aggregate cap at 590-610. The current issue concerns profile name/ID selectors,
  not accepted caller-supplied source_profile metadata. SPEC.md and SPEC_impl.md define the combined creation-field
  limit.
- Decision: scheduled under the user's authorization for clear, small code fixes. Apply the existing aggregate
  calculation before restricted profile resolution, reusing or extracting it rather than adding per-field limits.
- Completion criteria: oversized combined parent/cwd/profile-selector/title requests are refused before relay; valid
  requests resolve normally. Preserve the shared ingress check and account for fields actually accepted on each path. No
  queue-policy changes. Remove this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/775/changes. jj change:
  `rmroouutuuvvwyqpzpurqyzuwurmzomp`; bookmark: `fix-profile-size-before-relay`. The authenticated-dispatch regression
  now refuses the oversized aggregate before a profile upcall and still completes valid profile-based creation.

## refresh-arm-busy-drains-on-dropped-sender.md

- Outcome: `fix code`.
- Assessment: confirmed readiness defect by current-code inspection, not runtime reproduction. `manager.rs:3432-3444`
  ignores refresh.changed() errors, allowing a closed watch channel to wake refresh repeatedly. next_nudge already parks
  a closed sender. SPEC_impl.md specifies bounded refresh cadence; actual orphan duration and request volume were not
  measured.
- Decision: scheduled under the user's authorization for clear, small code fixes. Disable this wakeup on sender closure
  using the existing pending-on-closure pattern, inside the selected future. No new retry policy.
- Completion criteria: closure cannot drive repeated drains; timer, client-closure and nudge branches remain selectable.
  Normal notifications still refresh without reconnecting. Remove this feedback file and its index entry in its
  execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/776/changes. jj change:
  `loqoozyupyzxrqskquntnvtsnstmvuyu`; bookmark: `fix-closed-refresh-watch`. The closed-watch probe stayed at one
  completed request instead of 106; timer refresh and nudge-driven reconnect still completed. The temporary probe was
  removed after verification.

## resolve-owner-compares-first-claimant-only.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `sessions.rs:750-764` checks only the
  first sorted contested claimant. If that is the cached owner, a second different claimant is ignored. The existing
  resolve_owner contract and SPEC_impl.md collision rules require fail-closed routing.
- Decision: scheduled under the user's authorization for clear, small code fixes. Find any claimant different from the
  cached owner and return the existing SessionOwnerAmbiguous error, preserving its ordered pair and subsequent checks.
- Completion criteria: owner X with claimants [X, Y] is refused regardless of ordering; a sole self-claim is not falsely
  ambiguous. No routing-policy redesign. Remove this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/777/changes. jj change:
  `tmnzxtzlzzxwzusqxzstvsuvslxmtswn`; bookmark: `fix-owner-contested-claimants`. The cache-handoff regression now
  refuses the later competing claim and restores routing when that competitor withdraws, preserving a harmless sole
  self-claim.

## restart-failure-says-restarted.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `service/core.rs:10943-10955` computes
  cleanup failure/reaping state, sends session restarted, then returns failure. SPEC.md defines restart as a real
  relaunch; the removed attachment must still receive a detach notice on failure.
- Decision: scheduled under the user's authorization for clear, small code fixes. Construct the local cleanup result
  before choosing the detach reason; notify truthfully when cleanup prevents restart, then return that failure.
- Completion criteria: failed detach-for-restart says the attachment ended but restart failed, without claiming
  completion. Both paths notify; successful flow remains unchanged. No lifecycle or automatic-reattach redesign. Remove
  this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/778/changes. jj change:
  `mrxvyvxnotskvuyzronkroottyvxxnvp`; bookmark: `fix-restart-failure-notice`. The real attachment probe now receives a
  restart-failure notice at the cleanup barrier. The adjacent successful-restart case also passes. Temporary logging was
  removed; wording-only assertions were not retained.

## reverify-stamp-refresh-never-lands.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `service/capture.rs:312-328` rejects
  Captured-to-Captured advance, but 1597-1607 relies on it to refresh the stamp after a matching record read. The stale
  stamp causes repeated reads. SPEC_impl.md calls for cheap re-verification while retaining identity.
- Decision: scheduled under the user's authorization for clear, small code fixes. Under the existing mutex, update only
  the stamp if still Captured for the same conversation and record locator. Do not broaden the state ladder.
- Completion criteria: a verified append updates the stamp so unchanged later polls avoid content reads. Concurrent
  Reported state, another conversation or another record is never overwritten. No new cache/polling machinery. Remove
  this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/779/changes. jj change:
  `xosxqxwvkxkrlzmulkomlwmmryrytoqn`; bookmark: `fix-reverification-capture-stamp`. The append-and-reverify regression
  now refreshes the stamp while retaining the captured conversation and record. The existing capture-state ladder
  remains unchanged.

## seed-eviction-evicts-just-recorded-row.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `manager.rs:1918-1928` includes the new
  seed in victim selection; helm `store.rs:4604-4623` excludes it. This contradicts the existing admitted-seed invariant
  and SPEC_impl.md's mutation seeding contract. The feedback overstates truncation: any actual eviction correctly sets
  the truncated flag.
- Decision: scheduled under the user's authorization for clear, small code fixes. Exclude the new ID before selecting
  the in-memory victim, preserving original vector indices and ordering.
- Completion criteria: at capacity, an oldest/tied new row remains routable and the correct other row is evicted.
  Preserve the cap and truncated flag. Remove this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/780/changes. jj change:
  `xmwqnwmyprkoprlxmmtwwklttrqurpqt`; bookmark: `fix-new-session-seed-eviction`. The at-capacity regression now retains
  newly admitted old/tied rows, evicts the correct other row, and preserves the cap and truncation flag.

## ssh-destination-collides-with-local-display-name.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. Helm `store.rs:1230-1246` checks
  destination candidates against explicit aliases but omits the unaliased local effective name. Destination-write paths
  reuse this check. SPEC.md requires unique display names; alias SET already compares effective names.
- Decision: scheduled under the user's authorization for clear, small code fixes. Extend the existing transactional
  collision check to include the local effective name, using existing derivation/refusal types. Preserve self-exclusion
  and aliased-retarget semantics; coordinate with the separate local-alias-clear fix.
- Completion criteria: add, probed registration, ensure and retarget cannot introduce a visible local-name collision.
  Local alias changes naturally change the comparison. No literal-string blacklist or new SSH syntax policy. Remove this
  feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/781/changes. jj change:
  `wkpkvnsxnprqtpnxkmxrxmllwmqpxvlw`; bookmark: `fix-ssh-local-display-collision`. Add, probed registration, atomic
  ensure, and retarget regressions now reject the visible local-name collision using the existing transactional refusal.

## stop-actor-skips-client-retirement.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `manager.rs:1399-1415,2554-2570` aborts
  removed-row actors without explicitly retiring their published clients. Retained clones can keep transport serving;
  retire_withdrawn exists for this distinction. SPEC_impl.md requires withdrawn connections to stop serving.
- Decision: scheduled under the user's authorization for clear, small code fixes. At both removed-row paths, withdraw
  the client through the existing status mutation and call retire_withdrawn outside it, alongside actor cancellation.
- Completion criteria: removal retires transport despite retained clones and pending requests, without stopping remote
  sessions. No new graceful-shutdown protocol. Remove this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/782/changes. jj change:
  `vxssnyppuzlvkwnqqyptzklsowznzqrn`; bookmark: `fix-stopped-actor-client-retirement`. Both removal paths now close the
  observed peer transport despite retained client clones. The existing in-flight request-retirement regression also
  passes.

## sweep-deletes-live-staged-sentinel.md

- Outcome: `fix code`.
- Assessment: confirmed internal healthy-filesystem race by current-code inspection, not runtime reproduction.
  `service/launch_artifacts.rs:392-393` removes every staged artifact, including a surviving shim's unpublished
  sentinel. Existing staged-name parsing identifies its session. SPEC.md and SPEC_impl.md rely on launch-failure
  evidence surviving supervisor restart.
- Decision: scheduled under the user's authorization for clear, small code fixes. Reuse staged launch-name parsing to
  preserve staging owned by reloaded sessions and remove genuinely orphaned staging. Keep published sentinel retention.
- Completion criteria: a surviving session's staged sentinel is not startup-swept; orphaned staging remains eligible. No
  generation reconciliation, age policy or background sweep. Remove this feedback file and its index entry in its
  execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/783/changes. jj change:
  `nxvtytyoykynplrkylzrmptputpmklvp`; bookmark: `fix-live-staged-launcher-sweep`. The startup-sweep regression now
  preserves surviving-session staging and published sentinels while removing orphaned staging.

## terminal-less-delete-no-server-guards-never-match.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `service/teardown.rs:802-812` searches
  error.to_string() for missing-server diagnostics, but `tmux.rs:2479` wraps them in outer context. The guards cannot
  see the raw message. The existing teardown contract permits deletion for proven absence, not uncertain liveness.
- Decision: scheduled under the user's authorization for clear, small code fixes. Classify known absent-server
  diagnostics from existing typed raw tmux stderr at the driver boundary for this delete path, including ENOENT for the
  error-connecting form. Do not search rendered error chains.
- Completion criteria: terminal-less deletion succeeds for proven server absence. Permission errors, unknown diagnostics
  and diagnostic-like target paths remain failures. Preserve other has_session callers' semantics; no broad error-system
  rewrite. Remove this feedback file and its index entry in its execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/784/changes. jj change:
  `rlnoswqskmtlkqzxlowkvxunoswqrzxn`; bookmark: `fix-terminal-less-delete-diagnostics`. The deletion-specific probe
  accepts proven raw absence and rejects permission failures, unknown diagnostics, and diagnostic-like paths. The
  corrected fixture passed runtime and independent source review; legacy probe callers remain unchanged.

## tombstone-eviction-counts-live-transfers.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. `service/uploads.rs:1336-1351` collects
  finished tombstones but calculates excess from all routes. With 33 tombstones and eight live routes it removes nine
  instead of one. The existing contract bounds finished receipts independently of live work.
- Decision: scheduled under the user's authorization for clear, small code fixes. Calculate excess from tombstones.len()
  before consuming the vector and evict that many oldest tombstones. Keep live routes untouched.
- Completion criteria: mixed routes retain the newest MAX_UPLOAD_TOMBSTONES receipts, evict exactly the finished excess
  and preserve live transfers. No retention-policy change. Remove this feedback file and its index entry in its
  execution PR.
- Execution: `complete`; draft PR: https://github.com/scode/farhelm/pull/785/changes. jj change:
  `qynrmolprrxqxxvxvqoqmuwvuvuvzrzx`; bookmark: `fix-upload-tombstone-count`. The mixed-route regression now retains the
  newest 32 finished receipts and all eight live transfers, evicting only the oldest finished excess.

## agent-label-empty-cell-on-trailing-slash.md

- Outcome: `discard`.
- Assessment: partly confirmed by current-code inspection, not runtime reproduction. The agent-facing label helper
  returns an empty basename for a raw program token ending in `/`; the browser UI already preserves that token. Empty
  invocations are refused by current creation validation. This is a cosmetic edge case for an invalid executable path,
  not a failure to support unfamiliar agents. The spec requires argument secrecy, not an explicitly nonempty label.
- Decision: the user chose discard after discussing the label's purpose, arbitrary-command support, and the limited
  impact. No remedial action.
- Completion criteria: remove the feedback file and its index entry during execution, without code, spec, or TODO
  changes.
- Execution: `complete`; removed the feedback file and index entry without remedial changes. jj change:
  `vmltuxlwvzwuvzwlxotsllpxyuypzqxo`; bookmark: `triage-discard-empty-agent-label`; draft PR:
  https://github.com/scode/farhelm/pull/832/changes. Targeted Markdown formatting and independent review passed.

## ambiguous-restart-misattributes-exit.md

- Outcome: `fix code`.
- Assessment: partly confirmed by current-code inspection, not runtime reproduction. Ambiguous relaunch recovery
  republishes the previous terminal with the new generation and a Launching outcome. The status and observation paths
  can then attribute the old dead pane's exit to that generation. Startup reconciliation has a stale-pane guard, but the
  feedback's claim of repeated status flips is overstated: a wrongly committed terminal outcome need not be undone by a
  supervisor restart.
- Decision: the user chose fix code only while the change remains simple. Stop and return the item for discussion if
  implementation requires substantial complexity; do not expand into a lifecycle redesign or new tracking machinery.
- Completion criteria: prevent an unresolved restart from attributing the previous run's exit to the new launch, with
  consistent displayed and durable outcomes. Preserve genuine launch-error reporting and verified exits, including quick
  exits. Verify the boundary with a focused regression and inspect startup reconciliation. Remove the feedback file and
  its index entry only when the bounded fix is complete; retain them if the complexity caveat stops execution.
- Execution: `blocked`; deferred under the user's simplicity caveat after current-code inspection. Ambiguous recovery
  republishes the old terminal as `Launching`, but a verified new terminal whose database confirmation fails is also
  published as `Launching`. Suppressing every dead `Launching` pane would lose that verified quick exit; clearing the
  terminal would change immediate attachment, stop, and tab behavior. Preserving both contracts needs a provenance
  distinction beyond the agreed classifier fix. Startup's empty-pane guard does not supply that distinction to live
  observations. No runtime reproduction or code change was made; the feedback and index entry remain for discussion.
  Deferral record: jj change `uqyyskvosywqwumslvvyppspqrzmxuts`; bookmark: `triage-defer-ambiguous-restart`; draft PR
  [#833](https://github.com/scode/farhelm/pull/833/changes). The remaining triaged outcomes continue independently.

## archive-discards-stopped-agent-exit-code.md

- Outcome: `fix spec+code`.
- Assessment: partly confirmed by current-code inspection, not runtime reproduction. Session Archive can destroy the
  terminal without collecting its available final exit code, then record an annotated exit with no code. Already
  recorded terminal outcomes are preserved, so the finding's universal claim is overstated. A natural exit racing
  Archive can receive a stop annotation, but collecting an exit code alone does not resolve that attribution race. The
  GUI exposes Archive without exposing a control to include archived sessions in its list.
- Decision: remove the concept of an archived SESSION from the product, rather than repair this feature. The user has no
  established use case for it and prefers removing its complexity; a future feature can be designed if a clear use case
  emerges. This is a product-wide removal, not merely hiding the GUI action: remove session Archive operations,
  archived-session state and filtering, unarchive behavior, and associated special cases across GUI, CLI, agent tools,
  APIs, protocol, persistence, and implementation wherever they exist. Update the authoritative specifications and
  maintained documentation to describe the resulting product.
- Completion criteria: no supported operation archives or unarchives a session, and no active session model or listing
  depends on an archived-session flag. Remove obsolete feature-specific code and tests; validate remaining lifecycle
  operations and address existing persisted archived rows explicitly. The user's later decision permits restoring
  ordinary visibility if very simple, preserving metadata, attachments and recorded outcomes without launching agents;
  otherwise use ordinary session deletion with its cleanup and ownership safeguards. Assess schema and protocol
  compatibility during implementation, retaining only compatibility machinery actually required by repository policy.
  This decision does NOT remove or change OWNED GITHUB CHECKOUT DIRECTORY ARCHIVAL: deleting a session with an owned
  GitHub checkout must retain the existing behavior that moves the checkout into `farhelm-archived-working-copies`,
  including its ownership checks, retention, journaling, recovery, and safety rules. That filesystem operation is
  separate from session Archive and remains specified and tested. Do not use a blanket removal of symbols or prose
  containing "archive"; classify each reference by which feature it serves. Remove this feedback file and its index
  entry when the session-feature removal is complete. Other queued findings made obsolete by that removal must be
  explicitly accounted for, not silently fixed or discarded during triage.
- Execution: `complete`; jj change: `smnpywstvpqqmssztqquontnxprzumuv`; bookmark: `triage-remove-session-archive`; draft
  PR: https://github.com/scode/farhelm/pull/834/changes. Supervisor schema 19 drops the archive flag while retaining
  sessions and their data; helm schema 29 drops its mirror and removes the retired JSON member. This straightforward
  migration restores visibility without launching agents. Protocol 26 and agent JSON schema 2 remove the corresponding
  vocabulary. Owned-checkout directory archival remains supported. The dependent archived-retry finding is resolved
  separately; the other queued findings mentioning Archive retain independent deletion or restart concerns.

## archived-retry-resurrects-session.md

- Outcome: `other`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. An interrupted keyed create can retain a
  pending reservation and an unlaunched session. Archiving that session does not settle the reservation; a later retry
  can take it over, replace the archived row with an unarchived one, and launch the agent. The existing lifecycle claim
  protects takeover but does not check the archived flag. Ordinary completed-create retries are outside this finding.
- Decision: the user agreed to resolve this through the session Archive removal recorded under
  `archive-discards-stopped-agent-exit-code.md`, without a separate bug fix.
- Completion criteria: after that removal eliminates the affected archived-session state and operation, remove this
  feedback file and its index entry and record the dependency as satisfied. Do not change owned GitHub checkout
  directory archival on session deletion, which is unrelated and remains supported.
- Execution: `complete`; jj change: `nzvolvnmtszsplqntnunqqtxnlxqwrkl`; bookmark: `triage-close-archived-retry`; draft
  PR: https://github.com/scode/farhelm/pull/835/changes. The preceding Archive-removal change eliminates the operation,
  session flag and retry special cases that this finding depends on. Pending create reservations retain their ordinary
  retry behavior; no separate retry fix is introduced. Owned-checkout directory archival is unchanged.

## cached-session-skips-created-at-check.md

- Outcome: `fix spec`.
- Assessment: the read-path inconsistency is confirmed by code inspection, not runtime reproduction: the list compares
  the cached creation-time column with its JSON payload, while the individual stale-detail read does not. Both normal
  cache writers derive those representations from the same session record and write them together in one SQL statement
  within a transaction. No current writer defect or ordinary timing window producing disagreement was established.
- Decision: the user accepts multiple materialized representations within a database when maintained together and
  transactionally, relying on correct writes and good test coverage. Extremely trivial sanity checks without performance
  or complexity costs are encouraged but optional; missing checks are not bugs. Do not add recurring corruption checks
  and overhead as a substitute for trusting that invariant.
- Completion criteria: state the general principle in SPEC_impl.md, without adding a timestamp check or removing
  existing checks. Remove this feedback file and its index entry during execution. Preserve external-input validation
  and review of concrete writer or migration defects.
- Execution: `complete`; verified SPEC_impl.md's "Transactional database representations" principle, already committed
  at the triage anchor. Removed the feedback and index entry without adding or removing runtime checks. jj change:
  `ptwopqztykytvxmwnrmzktszktmuokwr`; bookmark: `triage-complete-cached-session-spec`; draft PR:
  https://github.com/scode/farhelm/pull/836/changes.

## clone-audit-log-skips-escape-for-log.md

- Outcome: `fix spec`.
- Assessment: the claimed log-forgery behavior is not supported by current-code inspection. Create and clone log session
  IDs as ordinary string fields; the configured tracing formatter uses Rust debug escaping for those fields, including
  the invisible and direction-changing characters described in the finding. Omitting the custom helper does not imply
  missing escaping. Clone also resolves the source against a current session listing before logging it. No runtime
  reproduction was performed.
- Decision: specify safe session-ID presentation in logs only, not a broad rule about untrusted input. The user accepts
  rejecting IDs outside plain normal ASCII and explicitly prefers rejection when it is simpler. There is no requirement
  to support arbitrary Unicode IDs or thread them through new escaping machinery. Existing logging-library escaping
  satisfies the behavior without additional call-site helpers.
- Completion criteria: add the narrow behavior and simplicity preference to SPEC_impl.md. No code change or general
  validation framework is requested. Remove this feedback file and its index entry during execution.
- Execution: `complete`; verified SPEC_impl.md's "Session IDs in logs" rule, already committed at the triage anchor.
  Removed the feedback and index entry without adding escaping helpers or a validation framework. jj change:
  `trxwxuywtovvskosvytpysluvwrxlyzn`; bookmark: `triage-complete-session-log-spec`; draft PR:
  https://github.com/scode/farhelm/pull/837/changes.

## cancelled-request-leaks-pending-entry.md

- Outcome: `fix spec`.
- Assessment: cancellation after enqueue retains a reply-routing entry until a reply arrives or the connection is
  closed. This is confirmed by code inspection, not runtime reproduction. Sustained growth requires unanswered requests
  on a connection that stays live; ordinary replies and connection retirement clear the entries. No leak surviving
  connection teardown or host removal was established.
- Decision: the user accepts indefinite retention of a fixed amount of supervisor metadata, even during unavailability,
  provided it does not accumulate without bound over time. Separately, defending against accumulation caused by a
  malicious or buggy supervisor selectively failing to answer requests is out of scope. Missing defenses against that
  behavior are not bugs and do not justify added complexity.
- Completion criteria: state both principles in SPEC_impl.md without adding cancellation cleanup or removing existing
  cleanup. Preserve ordinary-operation boundedness and specified connection-retirement and host-removal cleanup. Remove
  this feedback file and its index entry during execution.
- Execution: `complete`; verified SPEC_impl.md's "Supervisor metadata retention and nonresponse" principles, already
  committed at the triage anchor. Removed the feedback and index entry without changing cleanup or ordinary boundedness.
  jj change: `luwvxrmslsovvknmqnmyxpowwztzxpvt`; bookmark: `triage-complete-nonresponse-spec`; draft PR:
  https://github.com/scode/farhelm/pull/838/changes.

## commit-window-reads-unpublished-outcome.md

- Outcome: `discard`.
- Assessment: confirmed ordering gap by current-code inspection, not runtime reproduction. Upload cleanup closes its
  command channel before publishing the final ending reason. A commit arriving in between can receive the generic
  no-upload refusal instead of the deletion-specific refusal. The request still fails visibly, and a separate abort
  notification carries the reason. No data-loss or hung-request consequence was established.
- Decision: the user chose discard after discussing the limited diagnostic impact and the extra state needed to separate
  the ending reason from cleanup completion.
- Completion criteria: remove the feedback file and its index entry during execution without changing code or specs.
- Execution: `complete`; removed the feedback file and index entry without code or spec changes. jj change:
  `qtnkzurtorxvyunyvpswpltovustmllz`; bookmark: `triage-discard-upload-commit-diagnostic`; draft PR:
  https://github.com/scode/farhelm/pull/839/changes.

## host-views-transiently-pairs-new-identity-with-stale-mismatch.md

- Outcome: `discard`.
- Assessment: confirmed transient display inconsistency by current-code inspection, not runtime reproduction. Host-view
  assembly reads the registry separately from the actor snapshot; a read during adoption can pair the new identity with
  the old mismatch warning. Subsequent reads converge. Adoption still validates identity and routing uses manager state,
  not the assembled host view. No wrong action or durable state loss was established.
- Decision: discarded under the user's authorization to discard rare timing-dependent findings whose only impact is
  slightly misleading presentation, without data loss or a serious operational consequence.
- Completion criteria: remove the feedback file and its index entry during execution without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry without code or spec changes. jj change:
  `rqovkxtksxxokxluomrtsqktqwttuytt`; bookmark: `triage-discard-host-view-mismatch`; draft PR:
  https://github.com/scode/farhelm/pull/840/changes.

## supervisor-discards-actor-panic-cause.md

- Outcome: `discard`.
- Assessment: confirmed diagnostic limitation by current-code inspection, not runtime reproduction. The actor monitor
  discards the panic payload when constructing its structured warning and visible retirement reason. Retirement and
  client cleanup still run. The default panic hook retains the cause on stderr; silent hooks in separate agent-hook
  commands do not apply to the helm. No production panic trigger was established by this finding.
- Decision: discarded under the user's authorization to discard rare diagnostic-only edge cases. This finding concerns
  missing detail after an actor panic, not the cause of the panic or a failure to retire its connection.
- Completion criteria: remove the feedback file and its index entry during execution without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry without code or spec changes. jj change:
  `tkqzsoyyozmrstzsuwpnpwzkltmtvzpo`; bookmark: `triage-discard-actor-panic-diagnostic`; draft PR:
  https://github.com/scode/farhelm/pull/841/changes.

## create-runs-inline-on-read-loop.md

- Outcome: `other`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. Both create dispatch paths await creation
  inline on their connection's read loop, delaying later incoming frames, including terminal input and unrelated
  requests. Other hosts remain independent, and existing output tasks can continue. Creation has intent, directory, and
  parent lifecycle admission but does not use the shared slow-handler task admission. A correct background-handler fix
  requires preserving those guards and disconnect semantics; estimated medium effort.
- Decision: the user chose to create a `Planned` bucket in TODO.md and put this fix there. Also update triage to skip
  findings already covered by a planned item, as it skips behavior accepted by the current specifications. Planning the
  fix does not authorize implementing it now.
- Completion criteria: add the scoped planned item and triage rule, then remove this feedback file and its index entry
  during execution. Keep the planned TODO until the actual code fix is implemented; removing the feedback is not
  completion of that fix.
- Execution: `complete`; verified TODO.md's Planned item "Keep session creation off the connection read loop" and the
  already-planned cleanup rule in AGENTS.md and review_feedback_queue/AGENTS.md, all committed at the triage anchor.
  Removed the feedback and index entry; the planned TODO remains and no background-create implementation was made. jj
  change: `qlquzktqvlssswuknqluulkowzruntmw`; bookmark: `triage-complete-create-planning`; draft PR:
  https://github.com/scode/farhelm/pull/842/changes.

## delete-quarantine-waits-unboundedly.md

- Outcome: `other`.
- Assessment: the deletion path awaits filesystem operations while holding its session lifecycle claim, confirmed by
  code inspection. The finding requires that host's filesystem to hang. No cross-host or helm-wide blocking consequence
  was established.
- Decision: skipped under the triage rule for behavior already accepted by the specifications. SPEC.md's "Healthy local
  filesystems" section permits the affected host and its requests to stop making progress and rejects added complexity
  solely to bound those filesystem hangs. This is accepted behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately under the user's clarified triage rule,
  without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry locally during triage. The ledger retains the
  assessment and specification basis.

## detach-timeout-abandons-upstream-detach.md

- Outcome: `fix code`.
- Assessment: confirmed cleanup gap by current-code inspection, not runtime reproduction. Credential-revocation cleanup
  can time out while queueing Detach after removing the local attachment. The supervisor can retain the obsolete
  attachment when the connection recovers, interfering with cautious automatic reattachment. Explicit takeover remains
  available, and connection loss or other cleanup can release it; permanent inability to take over is overstated.
- Decision: the user agreed to the small code fix: reuse the existing background-detach mechanism so the cleanup
  notification survives the caller's timeout. Preserve prompt revocation of browser access.
- Completion criteria: queue backpressure and caller timeout cannot abandon the upstream detach notification while the
  connection remains usable. Verify with a focused cancellation/backpressure test, preserving explicit takeover and
  existing connection cleanup. Remove the feedback file and index entry in the execution change.
- Execution: `complete`; explicit detach now awaits an independently owned send task, preserving normal enqueue ordering
  while caller cancellation leaves upstream notification alive. Focused tests cover a full writer queue, cancellation
  after local removal, and ordering with later traffic. The old inline send fails the cancellation regression; the
  reviewed implementation passes both tests. Browser revocation and supervisor takeover checks are unchanged. Removed
  the feedback and index entry. jj change: `ltqmxvpqpnqvynkxrovoszwmyyvopmpr`; bookmark:
  `triage-preserve-upstream-detach`; draft PR: https://github.com/scode/farhelm/pull/843/changes.

## directory-source-staging-leak.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. The operator-supplied `--payload-dir`
  source creates a cache below that existing directory, stages archive members and copied binaries through random
  temporary files, and only prunes completed UUID-named snapshots. A helm crash can therefore leave a staging file in
  the cache indefinitely; repeated interrupted provisioning can grow the cache. The GitHub download source has first-use
  housekeeping for its fixed staging names, so this finding is specific to the directory source. No current
  specification accepts unbounded orphaned staging files.
- Decision: the user chose a code fix and added a naming requirement. Sweep stale directory-source staging files at a
  defined reuse point, with an age guard that cannot remove an active materialization. Rename the cache directory from
  the generic `.extracted` to a Farhelm-specific hidden name such as `.farhelm_extract_tmp`, reducing the chance that
  cleanup touches unrelated contents inside the operator-supplied parent. The implementation must manage only entries
  matching its own staging and snapshot patterns; an existing legacy `.extracted` directory must not be recursively
  deleted merely because the cache name changes.
- Completion criteria: clean crash-orphaned staging files on the next applicable directory-payload use (or earlier
  first-use housekeeping if that is the chosen implementation), preserve concurrent and active staging files, keep
  completed payload snapshots usable, use the Farhelm-specific cache name consistently, and verify the cleanup and
  collision boundary with focused tests. Remove the feedback file and its `review_feedback_queue/INDEX.md` entry in the
  execution change.
- Execution: implemented in change `tvomkzky` on bookmark `pr/directory-source-staging`;
  [draft PR #909](https://github.com/scode/farhelm/pull/909/changes).

## failed-restart-discards-capture.md

- Outcome: `fix spec`.
- Assessment: the implementation can clear capture fields during a non-`Resume` restart and fail before spawning the
  replacement, confirmed by code inspection rather than runtime reproduction. The finding's immediate user-facing
  consequence is limited: `FreshOnly` and `FallbackTemplate` already mean the requested restart cannot safely resume the
  captured conversation. Later re-verification of temporarily unavailable evidence could have less information, but some
  withdrawal of unsafe evidence is intentional and no concrete ordinary-user workflow requiring its preservation was
  established.
- Decision: the user chose to state the underlying simplicity principle in SPEC.md. Resumability remains a core feature
  while a safe Resume offer exists, but once the current offer is already non-resumable, Farhelm need not preserve every
  remaining capture field through a definitive failed restart or recovery transition when doing so adds complexity. The
  rule does not permit demoting a valid Resume offer or silently choosing another conversation.
- Completion criteria: add the general principle to SPEC.md, remove this feedback file and its
  `review_feedback_queue/INDEX.md` entry immediately under the accepted-spec triage rule, and make no code or TODO
  change.
- Execution: `complete`; the specification rule was added and queue cleanup performed.

## failed-forwarder-wedges-delete-until-restart.md

- Outcome: `fix code`.
- Assessment: partly confirmed by current-code inspection, not runtime reproduction. The session Delete and owned
  checkout cleanup feature records a failed terminal-output forwarder join as a permanent `Failed` barrier.
  Whole-session teardown then treats that marker like an unresolved reaper, so a session whose terminal is already gone
  cannot be deleted or finish checkout cleanup until the supervisor restarts. The fail-closed barrier remains
  appropriate for replacement attachment, but the permanent teardown refusal is not part of the user-facing lifecycle
  contract.
- Decision: the user chose a bounded code fix, provided it does not introduce significant complexity or scope creep.
  Preserve failed barriers for attach and replacement safety, while allowing whole-session teardown to distinguish a
  completed-but-failed forwarder cleanup from an active `Reaping` entry and proceed with terminal destruction. Keep the
  failure diagnostic visible.
- Completion criteria: Delete and owned-checkout cleanup can retry after a failed forwarder join without supervisor
  restart; replacement attachment still refuses while cleanup is unconfirmed; focused tests cover both boundaries. If
  implementation requires significant new lifecycle state, recovery machinery, or broader design changes, defer the item
  with the blocker documented in TODO.md instead of expanding scope. Remove the feedback file and its
  `review_feedback_queue/INDEX.md` entry only when the bounded fix is complete; retain or narrow it if deferred.
- Execution: `complete`; draft PR [#911](https://github.com/scode/farhelm/pull/911/changes) on bookmark
  `pr/failed-forwarder-delete`, jj change `puuqrxks`.

## failed-delete-strands-attachments-in-quarantine.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. Session Delete first moves the attachment
  directory into quarantine, then performs checkout safety checks and the final row-removal transaction. Any later
  refusal or failure can retain the session row while losing the attachment directory's reachable path; a later startup
  quarantine sweep can then delete those attachments even though the session survives. Triggers include deliberate
  owned-checkout safety refusals as well as database failures, so this is broader than an SQLite I/O failure. The
  behavior conflicts with the delete contract and the quarantine invariant that a retained session keeps its files.
- Decision: the user chose a code fix with good focused test coverage. On any failure after quarantine, restore the
  attachment directory to its session path when possible, preserving the row-and-files-together invariant. Keep the
  existing fail-closed behavior for the operation and log loudly if restoration itself fails. If implementing this
  requires a broader lifecycle redesign, new durable state, or other scope creep, defer the item instead: document the
  blocker and proposed follow-up in the relevant TODO item rather than expanding this change.
- Completion criteria: exercise the deliberate post-quarantine refusal and the recovery path, verify that a retained
  session's attachments remain reachable and are not removed by startup reconciliation, preserve successful deletion and
  quarantine-crash recovery, and stop for documented deferral if the simplicity gate is reached. Remove the feedback
  file and its `review_feedback_queue/INDEX.md` entry only when the bounded fix is complete; retain or narrow the item
  if it is deferred.
- Execution: `complete`; focused recorded nextest coverage passed for the post-quarantine row refusal and restoration,
  startup preservation of retained attachments, successful retry deletion, and existing quarantine cleanup paths. Draft
  PR [#914](https://github.com/scode/farhelm/pull/914/changes) is on bookmark `pr/failed-delete-attachments`, jj change
  `kmlmqnpm`.

## pane-pid-recycled-before-sweep-binds-identity.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. Session delete, archive, stop, and restart
  teardown paths pass a bare pane PID through asynchronous work before `reap_process_tree` captures its start time. If
  the original pane exits and the operating system reuses that PID during the gap, the sweep can bind and terminate an
  unrelated process tree. The downstream start-time checks then validate the replacement process's identity, so they do
  not prevent this initial misbinding. The race is extremely rare, but its consequence is loss of unrelated user work on
  the host.
- Decision: the user chose the bounded code fix. Capture `(PID, start time)` at the initial pane liveness check, carry
  that identity through delete, archive, stop, and restart teardown, and reject the pane root if the identity changes
  before sweeping. Preserve the existing marker discovery and per-signal validation. This remains a simple localized
  fix; defer it with a documented TODO blocker if implementation requires broader lifecycle redesign or scope creep.
- Completion criteria: all affected teardown paths carry and validate the original process identity; a PID-reuse race
  cannot make the sweep adopt an unrelated pane root; focused regression coverage proves both matching and changed
  identities; remove this feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change, or
  retain/narrow it with the documented deferral if the simplicity gate is reached.
- Execution: `complete`; captured and validated pane identities across delete/archive, stop, restart, and tab teardown;
  focused recorded nextest coverage passed matching, changed, gone, and end-to-end sweep-root cases. Draft PR
  [#915](https://github.com/scode/farhelm/pull/915/changes) is on bookmark `pr/pane-pid-identity`, jj change `ntkxxmtq`.

## refused-delete-discards-in-flight-upload.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. Session Delete cancels and joins in-flight
  uploads before its pane-ownership and checkout preflights. If one of those checks refuses the delete, the session row
  survives but the upload has already been destroyed, so a refused operation can still lose user data.
- Decision: the user chose the bounded code fix. Move upload cancellation below all refusal-prone read-only preflights
  but keep it before destructive teardown. Preserve the existing cancellation behavior for successful deletion.
- Completion criteria: a refused Delete preserves an in-flight upload; a successful Delete still cancels uploads before
  removing the session; focused regression coverage proves both paths; remove this feedback file and its
  `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; Delete now runs refusal-prone pane, tab, and scope preflights before cancelling uploads while
  retaining cancellation before the process sweep and destructive teardown. Focused recorded nextest run
  `398569c5-969b-4e8b-9b22-124b0fbfe979` passed both regressions (372 skipped); formatting, diff, and isolated sleep
  checks passed. Draft PR [#916](https://github.com/scode/farhelm/pull/916/changes) is on bookmark
  `pr/refused-delete-upload`, jj change `513d1a650bc9`.

## refused-retry-strands-credential-spec.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. A failed create can leave a 0600
  Supervisor-side launch spec containing the full agent command, the per-session Supervisor credential, and any
  user-supplied command-line secrets. A refused keyed retry removes the durable launch row and intent but leaves that
  generation's spec until startup cleanup, extending retention after the failed launch is gone.
- Decision: the user chose the bounded code fix. Reuse the existing per-generation launch-artifact cleanup after the
  retry row removal, preserving the private permissions and ordinary retry behavior. No Helm credential is involved in
  this finding, and no new lifecycle state is needed.
- Completion criteria: refused retries remove the corresponding launch spec and sentinel; valid retries and ordinary
  launch recovery retain their current behavior; focused regression coverage proves cleanup; remove this feedback file
  and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; refused keyed retries now remove the stranded generation-zero launch spec and sentinel after
  settling the durable row, while preserving the original refusal and surfacing cleanup failure. Focused recorded
  nextest run `fbd09538-c841-47db-99e1-cf7b698d7b9b` passed four retry lifecycle tests (856 skipped); formatting,
  changelog, and isolated sleep checks passed. Draft PR [#918](https://github.com/scode/farhelm/pull/918/changes) is on
  bookmark `pr/refused-retry-launch-cleanup`, jj change `2a176be0e560`.
- Execution (2026-09-25, supersedes the entry above): `in progress`. #918 merged carrying only this ledger update; the
  fix, its changelog fragment, and the queue removal never reached main, and `v0.16.0-rc.1` shipped without them. The
  unchanged fix is restored on bookmark `pr/restore-refused-retry-cleanup`, draft PR
  [#953](https://github.com/scode/farhelm/pull/953/changes).

## restart-kills-tabs-reports-present.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection. When the recorded agent pane is gone but its tmux session remains,
  fresh-terminal restart calls `kill-session`, destroying the session's extra tabs and their processes, then publishes
  the pre-kill tab list and leaves tab attachments watching dead panes. This violates the user-facing restart contract
  and can destroy unsaved tab work.
- Decision: the user chose the proper code fix: preserve the existing tmux session and replace only the agent window,
  leaving extra tabs and their processes intact. The rough implementation shape is a third launch mode alongside pane
  reuse and new-session creation: create and mark a replacement agent window, confirm it, then remove the old agent
  window; if creation or marking fails, clean up only the replacement and retain the existing session. The existing
  `new_window` and `kill_window` primitives should support this without a lifecycle redesign.
- Completion criteria: fresh-terminal restart replaces only the agent window; tabs and their attachments survive;
  failure after replacement creation cleans up without stranding an unowned window or losing tabs; focused lifecycle
  coverage proves success and failure paths; remove this feedback file and its `review_feedback_queue/INDEX.md` entry in
  the execution change. Gate execution on the solution remaining within the bounded, moderate-complexity shape above. If
  implementation requires materially more lifecycle state, recovery machinery, or design scope, defer it during
  execution with the blocker and proposed follow-up documented in TODO.md, retaining or narrowing the queue item.
- Execution: `complete`; fresh-terminal restart now preserves a surviving tmux session and replaces only the dead agent
  window, while marker-only ambiguity never authorizes destructive cleanup. Focused recorded nextest run
  `f9c9714d-833e-47cc-86ad-ac4f44e9db97` passed four restart lifecycle tests (860 skipped); the final Astra review found
  no remaining defects. Draft PR [#919](https://github.com/scode/farhelm/pull/919/changes) is on bookmark
  `pr/restart-preserves-tabs`, jj change `knrrtwzvvlyrvtvnvmmxnpzlkykwnwwl`.

## stale-dial-outcome-publishes-over-retarget-nudge.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. A host destination edit can race a
  connection attempt completing; settled outcomes are published without checking the pending retarget nudge, so the old
  client and its state can temporarily become active again and route operations to the old machine. The trigger is
  ordinary host editing during an in-flight dial, and the consequence is a brief integrity/security failure in routing.
- Decision: the user chose the bounded code fix, with especially strong regression coverage because the race is hard to
  trigger in production. After a settled connection outcome returns, consume and honor a pending retarget nudge before
  publishing; discard the old result, including its client, and continue with the fresh-window semantics already used by
  interrupted attempts. Preserve ordinary connected, mismatch, unverified, and failed handling when no nudge is pending.
- Completion criteria: no settled old-destination outcome can publish after a retarget nudge; focused deterministic
  tests cover the pending-nudge boundary for connected, mismatch, unverified, and failed outcomes, plus ordinary
  no-nudge behavior; remove this feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; the actor now discards any settled connection result when a retarget nudge is pending, while
  interrupted outcomes retain their own fresh-window decision. Focused pinned recorder run
  `a3b9b1b2-3db3-4296-9ae0-1baef377f1ba` passed four boundary tests (820 skipped); the isolated sleep checker passed 226
  delays with zero missing rationales; formatting, diff, and changelog checks passed. The first Astra review's High
  finding was corrected and a fresh follow-up review reported `No entries. OK`. Draft PR
  [#920](https://github.com/scode/farhelm/pull/920/changes) is on bookmark `pr/stale-dial-publication`, implementation
  commit `bef928a24cf9`.

## stripped-agent-marker-forges-killable-tab.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. A same-account process with access to the
  private tmux server can remove the mutable agent-window marker and add a forged tab marker, causing Farhelm to offer
  the agent window as a tab. Closing or automatically reaping that tab can then kill the agent process tree and destroy
  its window. The mechanism is real, but it requires local process access plus a user action or tab-exit cleanup.
- Decision: the user chose a simple code guard and a specification clarification. Tab discovery and close/reap paths
  must positively exclude the window containing the session's recorded agent pane, even when its marker is missing or
  malformed. The specification should state that processes the user runs on the target host are trusted and Farhelm does
  not provide strong same-account isolation against deliberate interference with agents or the private tmux session.
  Farhelm should still add simple protections against accidental interference when they are local and low-complexity, as
  this pane-based exclusion is; do not grow a significant security-hardening subsystem for this threat model.
- Completion criteria: update the authoritative security/threat-model wording; add the pane-based exclusion at listing,
  close, and automatic dead-tab-reap discovery; focused tests cover a removed agent marker, a forged tab marker, and
  each destructive path; remove this feedback file and its `review_feedback_queue/INDEX.md` entry in the execution
  change. If the code or spec work requires materially more complexity than this bounded guard and clarification, defer
  with the blocker documented in TODO.md instead of expanding scope.
- Execution: `complete`; the supervisor now excludes the entire window containing the recorded agent pane from tab
  discovery and destructive cleanup, even when mutable markers are missing or malformed, and rechecks that identity
  before explicit close. `SPEC.md` records the trusted-target-process boundary and the absence of strong same-account
  isolation. Focused pinned recorder run `6bdf9b00-01d6-49f4-ba54-5aa7232092bf` passed four tests (863 skipped); the
  isolated sleep checker passed 226 delays with zero missing rationales; formatting, diff, and changelog checks passed.
  Fresh Astra medium review reported `No entries. OK`. Draft PR
  [#921](https://github.com/scode/farhelm/pull/921/changes) is on bookmark `pr/stripped-agent-marker`; implementation
  commit `9cd45aea0836`.

## discard-quarantined-hangs-response.md

- Outcome: `other`.
- Assessment: confirmed by current-code inspection, not runtime reproduction. Session Delete commits the row removal,
  then awaits best-effort removal of the quarantined attachment directory. A wedged supervisor filesystem can therefore
  leave the delete response pending even though the session is already gone. No cross-host or helm-wide blocking effect
  was established.
- Decision: skipped because the behavior is explicitly accepted by SPEC.md's `Healthy local filesystems` section. A
  local filesystem hang may halt requests on the affected host; Farhelm does not add recovery or timeout machinery
  solely to bound that failure. The host-isolation boundary remains: this allowance does not permit the supervisor's
  filesystem to freeze the helm or other hosts.
- Completion criteria: remove this feedback file and its `review_feedback_queue/INDEX.md` entry immediately, without
  code, spec, or TODO changes.
- Execution: `complete`; queue cleanup performed under the accepted-spec triage rule.

## duplicate-freeze-clobbers-retarget-nudge.md

- Outcome: `fix code`.
- Assessment: partly confirmed by current-code inspection, not runtime reproduction. The host-management feature lets a
  user edit a registered host's destination while the entry is frozen as a duplicate. The duplicate recheck and
  post-attempt duplicate paths can republish the old duplicate state after a retarget nudge has already published the
  new row as reconnecting. The next loop then freezes again against the old identity, so the new destination is not
  dialed until another edit or twin change. The exact interleaving remains unverified, but the ordering gap is visible
  in both publication sites and conflicts with the destination-edit reconnect contract in SPEC_impl.md.
- Decision: the user chose the narrow code fix. Before either duplicate-state publication, preserve a pending retarget
  nudge using the existing `taken_nudge` seam, then let the loop reload the edited row and start its fresh connection
  window. Do not redesign duplicate lifecycle or change the specification.
- Completion criteria: a retarget that races duplicate rechecking cannot be overwritten by the old duplicate state; the
  edited destination is retried under the fresh-window rules, ordinary duplicate freezing still works, and a focused
  regression covers the race. Remove the feedback file and its `review_feedback_queue/INDEX.md` entry in the execution
  change.
- Execution: `complete`; both duplicate publication boundaries now consume a pending retarget nudge before restoring the
  old duplicate state, preserving the fresh retry window and reloading the edited destination. Focused pinned recorder
  run `54c13d4f-c4d1-4979-b765-9cd0ac597338` passed six duplicate and retarget tests; the isolated sleep checker passed
  226 delays with zero missing rationales; formatting, diff, and changelog checks passed. The initial assertion-only
  recorder failure is retained and documented. Fresh Astra medium review reported `No entries. OK`. Draft PR
  [#924](https://github.com/scode/farhelm/pull/924/changes) is on bookmark `pr/duplicate-freeze-retarget`; the
  implementation commit is `5f4bdac86887`.
- Execution (2026-09-25, supersedes the entry above): `in progress`. #924 merged carrying only this ledger update; the
  fix, its changelog fragment, and the queue removal never reached main, and `v0.16.0-rc.1` shipped without them. The
  cited `5f4bdac86887` is a ledger commit, not the implementation, which was `f4e9f8202ac3`. That fix is restored on
  bookmark `pr/restore-duplicate-freeze-retarget`, changed only by two test lint fixes the current toolchain requires;
  draft PR [#954](https://github.com/scode/farhelm/pull/954/changes).

## restart-kills-terminal-less-agent-without-consent.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `29ced1a`, not runtime reproduction. A session left without a
  recorded pane (chiefly an ambiguous create where tmux errored but the session may exist) takes `None => None` in
  `restart_session`'s liveness recheck, so `stop_if_running` is never demanded and the else-branch reaps every process
  carrying the session's agent marker. A live agent is killed without consent through the agent CLI or any client that
  does not confirm on Unknown; the web and desktop UI already confirm for Unknown rows. This contradicts SPEC.md's
  restart and Unknown-status rules and `restart_session`'s own docs, which promise terminal-less entries are treated as
  possibly alive. The finding's `tmux kill-session` half is stale: #919 replaced that path after the reviewed commit.
  How often tmux reports failure after creating the session is unverified.
- Decision: the user chose the minimal fix (option 1). A terminal-less entry is treated as possibly alive: restart
  refuses without `stop_if_running`, and with consent the existing marker-keyed reap performs the stop. No tmux probing
  or marker-based pane discovery inside restart; that more precise variant was rejected as added mechanism and scope.
  Accepted cost: a CLI restart of a genuinely dead terminal-less session needs `--stop-if-running`.
- Completion criteria: restart of a terminal-less entry without consent is refused with the existing consent error; with
  consent it proceeds as today; confirm the web/desktop Unknown-row confirmation sends consent so the UI path does not
  regress; add a focused regression test; update `restart_session` docs if needed; remove the feedback file and its
  `review_feedback_queue/INDEX.md` entry in the execution change.
- Decision (2026-09-27, supersedes the scope above): applied literally, "a terminal-less entry is treated as possibly
  alive" would also cover every Interrupted or Exited row that reloads without a terminal after a reboot or tmux-server
  death; the UI restarts those without consent and e2e tests pin that. The user narrowed the rule: restart refuses
  without `stop_if_running` only when the entry has no terminal and its recorded outcome is still `Launching` (reported
  as Unknown). With consent, the existing marker-keyed reap runs as before.
- Execution: `complete`; `restart_session` refuses a terminal-less `Launching` entry without consent through the new
  `terminal_less_launch_may_be_live` predicate, and every other terminal-less row keeps the no-consent restart. The UI
  already sends consent after its Unknown-row confirmation. Focused recorded runs passed the two new unit tests and the
  e2e restarts of an interrupted session and of a session whose pane was recycled; the sleep checker reported no missing
  rationales. Fresh gpt-6-astra high review reported no findings. Draft PR
  [#1030](https://github.com/scode/farhelm/pull/1030/changes) is on bookmark `pr/restart-unconfirmed-launch-consent`, jj
  change `tqpurvyw`.

## tab-close-kills-rc-started-services.md

- Outcome: `fix spec`.
- Assessment: confirmed by current-code inspection at `29ced1a`, not runtime reproduction. Tab markers are set through
  `new-window -e` and the tab's cgroup scope wraps the login shell itself, both before rc files run (`core.rs`
  `tab_environment`, `launch.rs` `tab_window_command`, pinned by `a_tab_scope_prefix_wraps_the_shell_itself`). A shared
  service an rc file starts from a tab therefore belongs to that tab and dies on close, exit-then-reap, and Delete. On
  systemd hosts the scope catches it even after daemonizing; elsewhere the marker sweep does, except for non-dumpable
  daemons such as `ssh-agent` (see `nondumpable-daemons-escape-sweep.md`). The realistic trigger is narrower than the
  finding implies: a tab can only open on a live agent, whose launch already ran the same rc files outside containment,
  so guarded rc logic normally reuses that instance and the tab never owns it.
- Decision: the user chose to specify the current behavior rather than change it. Principle: tab containment
  deliberately starts before the shell's startup files, because the tab shell is itself what close promises to kill;
  anything those files start from a tab is part of that tab. This contrasts with the agent launch, whose markers and
  scope apply only after startup files, which is why shared services normally already run outside any tab. The user also
  asked for a `TODO.md` entry under `Doc todo` to document this for users in user-facing documentation.
- Completion criteria: SPEC.md's tab lifecycle text and SPEC_impl.md's process-tree containment section state the
  principle and its contrast with the agent exemption; add a `Doc todo` entry in `TODO.md` for documenting the behavior
  in user-facing docs; remove the feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; SPEC.md's session view section now says a tab's processes include what its shell's startup
  files start, and contrasts that with the agent launch, whose startup services stay outside the cleanup guarantee
  (without promising cleanup cannot reach them); SPEC_impl.md's process-tree section records why tab containment starts
  before startup files. TODO.md's `Doc todo` has the requested user-documentation entry. Fresh gpt-6-astra high review
  found the first SPEC.md wording overstated the agent exemption; corrected. Draft PR
  [#1031](https://github.com/scode/farhelm/pull/1031/changes) is on bookmark `pr/tab-startup-services-spec`, jj change
  `zsrwswsr`.

## sweep-can-claim-supervisor-or-tmux-server.md

- Outcome: `fix spec+code`.
- Assessment: partly correct, by current-code inspection at `29ced1a` plus a `/tmp` experiment showing a tmux server
  keeps the marker environment it was started with. The sweep has no exclusion for the supervisor, its ancestors, or its
  private tmux server (`sweep.rs` `claims`, `snapshot_proc`, `enumerate_tree`), so a supervisor carrying one of its
  sessions' markers signals itself on Delete, tab close, or Stop; a tmux server started by such a supervisor would be
  claimed too, killing every pane on the host. A plausible contamination route is a tab rc file running bare
  `systemctl --user import-environment` or `dbus-update-activation-environment --systemd --all`, which the next
  supervisor restart inherits because the generated unit sets no `UnsetEnvironment=`. The reported permanent freeze is
  wrong: no SIGTERM handler exists, so the supervisor dies in the SIGTERM round (and, dying cleanly, is not restarted by
  `Restart=on-failure`). The hand-started and desktop variant is a supervisor inside its own session, not a
  missing-exclusion bug. How common the triggering rc lines are is unverified.
- Decision: the user chose the narrow hardening (option A): the generated supervisor unit sets
  `UnsetEnvironment=FARHELM_SESSION_ID FARHELM_AGENT_ID FARHELM_TAB_ID`, since a systemd-managed supervisor never
  legitimately belongs to a session. The user asked that SPEC_impl.md record this behavior. The sweep-side protected set
  (option B) was not chosen, so supervisors started outside systemd and other polluted user services stay uncovered.
- Completion criteria: the generated supervisor unit strips the three session markers, with a unit-generation test
  covering it; SPEC_impl.md states that the systemd supervisor unit strips session markers and why; remove the feedback
  file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; the shared supervisor unit template now strips the three session markers with
  `UnsetEnvironment=`, pinned by the exact-text unit test and a new test tying the stripped names to the supervisor's
  marker constants. SPEC_impl.md records the rule, its reason, and what it leaves uncovered. The focused recorded run
  passed the unit-rendering and provisioning unit tests. Fresh gpt-6-astra high review reported no findings. Draft PR
  [#1032](https://github.com/scode/farhelm/pull/1032/changes) is on bookmark `pr/supervisor-unit-strips-markers`, jj
  change `vznqxoro`.

## nondumpable-daemons-escape-sweep.md

- Outcome: `fix spec`.
- Assessment: confirmed by current-code inspection at `29ced1a` and a `/tmp` reproduction: a detached `ssh-agent` calls
  `setsid` and marks itself non-dumpable, so `/proc/<pid>/environ` is unreadable to its own user. The portable sweep
  treats an unreadable environment as unmarked (`procs.rs` `read_environ`, `sweep.rs` `environ_markers_of` and
  `snapshot_proc`), and the process is no longer a pane descendant, so on hosts without a usable systemd user manager
  Stop and Delete leave it running. Hosts with a user manager catch it through the cgroup scope. A comment in `sweep.rs`
  already calls the non-dumpable case an accepted residual, and SPEC_impl.md accepts the general class (marker-dropping
  daemons, the macOS platform-binary residual), but SPEC.md's "reap everything the agent started" promises carry no
  no-manager caveat. Homebrew `ssh-agent` readability on macOS is unverified. No trust boundary is crossed (0600 socket,
  same-user access only).
- Decision: the user chose a spec clarification rather than a code change. Principle: where no usable user manager
  exists, the reaping guarantee covers only processes the portable sweep can identify; processes whose environment is
  unreadable (non-dumpable, setuid exec) are a named residual that only cgroup containment closes.
- Completion criteria: SPEC_impl.md's process-tree section names non-dumpable and setuid-exec processes beside the
  existing residuals; SPEC.md's Stop/Delete reaping promises are qualified for hosts without a usable user manager;
  remove the feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; SPEC.md's lifecycle cleanup paragraph and its security-section teardown sentence now narrow the
  reaping guarantee on hosts without a usable systemd user manager to processes the sweep can identify, and SPEC_impl.md
  names unreadable-environment processes (non-dumpable `ssh-agent`, setuid programs) beside the existing residuals, with
  why only cgroup containment closes the gap. Fresh gpt-6-astra high review found the limit also had to apply to
  terminal-tab cleanup, whose text promised the opposite; both specs' tab text now cross-references it. Draft PR
  [#1033](https://github.com/scode/farhelm/pull/1033/changes) is on bookmark `pr/teardown-guarantee-without-manager`, jj
  change `poqprvkp`.

## offline-rotate-creates-fresh-database.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `29ced1a`, not runtime reproduction. `token rotate` against a
  state directory where no helm ever ran creates the directory (`ensure_private_dir`), takes the offline branch because
  no socket answers, opens `helm.db` through the creating, migrating open (`HelmStore::open` → `db::open_private` with
  `SQLITE_OPEN_CREATE`), writes a fresh token, prints it, and exits 0 without naming the directory. The real helm keeps
  the leaked token and every enrolled browser. Triggers are a mistyped `--state-dir` or a shell `XDG_STATE_HOME` that
  differs from the one `helm setup` pinned into the units. The store already documents this hazard on
  `open_existing_current_schema`. `token show` minting into a fresh directory is deliberate first-run bootstrap and out
  of scope.
- Decision: the user chose the code fix as recommended: offline rotation never creates a helm state directory, lock, or
  database.
- Completion criteria: offline `token rotate` refuses when the state directory or `helm.db` is absent, with an error
  naming the resolved path and pointing at `--state-dir` / `XDG_STATE_HOME`; the database is opened without the create
  flag; a regression test shows rotate against an empty temporary directory fails and leaves no directory, lock, or
  database behind; add a `fix` changelog fragment; remove the feedback file and its `review_feedback_queue/INDEX.md`
  entry in the execution change.
- Execution: `complete`; `rotate` checks for `helm.db` before `ensure_private_dir` and the ownership lock, refusing with
  the resolved path, and the offline path opens through the new `HelmStore::open_existing`, which keeps migration but
  omits SQLite's create flag via `farhelm_supervisor::db::open_private_existing`. Focused recorded runs passed the
  token-control and auth tests, including the new empty- and missing-directory regression, and the store-level
  never-create test. Fresh gpt-6-astra high review reported no findings. Draft PR
  [#1034](https://github.com/scode/farhelm/pull/1034/changes) is on bookmark `pr/token-rotate-needs-existing-helm`, jj
  change `rpkvlzol`.

## ipv4-only-bind-allows-localhost-squat.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `29ced1a` and `/tmp` experiments that never touched a helm. The
  helm binds only `127.0.0.1` (`farhelm-helm/src/lib.rs` listener setup) while the origin guard accepts `localhost` and
  `[::1]` (`middleware.rs` `origin_is_allowed`). Another local account can bind `[::1]:<port>` even while the helm runs;
  `localhost` resolved to `::1` first on the test host, and Playwright Chromium and WebKit both loaded a `[::1]` server
  at `http://localhost:<port>`. That same-origin page can read the stored device secret or show a fake token prompt.
  This removes the "helm not running" precondition SPEC.md and `docs/security.md` rely on. Firefox, macOS browsers, and
  a genuinely separate account were not tested. The desktop app uses `127.0.0.1` and is unaffected.
- Decision: the user rejected dual-binding `[::1]` because IPv6 can be enabled while the helm runs, reopening the squat.
  Instead the browser UI is served only under the literal IPv4 origin `127.0.0.1:<port>`: requests naming `localhost`,
  `[::1]`, or any other host are refused, so no device secret is ever stored under an origin another account could
  serve. A `localhost`/`[::1]` request that reaches the real helm may redirect to `127.0.0.1`. Accepted residual: a
  squatter answering `localhost` can still show a fake token prompt; that stays under the existing guidance against
  untrusted local users.
- Completion criteria: the Host/origin guard accepts only `127.0.0.1:<port>`; a request reaching the helm under
  `localhost` or `[::1]` redirects to `http://127.0.0.1:<port>/` or is refused clearly; tests cover acceptance of the
  IPv4 literal and refusal/redirect of the others; SPEC.md/SPEC_impl.md state the IPv4-literal-origin rule, its reason,
  and the fake-prompt residual; update `docs/browser-limitations.md`, `docs/security.md`, and any other user docs that
  present `localhost` or `[::1]`; add a changelog fragment; remove the feedback file and its
  `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; the loopback guard (`origin_is_allowed`) accepts only `127.0.0.1:<port>` (bare on port 80), and
  a new `legacy_loopback_redirect` sends a plain `GET`/`HEAD` whose Host is exactly `localhost` or `[::1]` (with this
  port) to the fixed `http://127.0.0.1:<port>/`; upgrades and other methods get the 403. SPEC.md, SPEC_impl.md,
  `docs/security.md`, and `docs/browser-limitations.md` state the rule, why dual-binding was rejected, the stale-secret
  residual with `token rotate` as the remedy, and the lookalike-prompt residual. The focused recorded run passed the
  nine middleware tests. Fresh gpt-6-astra high review reported no findings. Draft PR
  [#1035](https://github.com/scode/farhelm/pull/1035/changes) is on bookmark `pr/ui-only-at-ipv4-loopback`, jj change
  `nyvmnvmu`.

## supervisor-error-forges-reauth-401.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `29ced1a`, not live reproduction. The helm's REST error path
  (`farhelm-helm/src/lib.rs` `error_kind` → `http_error`) takes a supervisor reply's `ErrorKind` wholesale when no
  helm-side error type matches and maps it one-to-one to an HTTP status, with the supervisor's text as the body. A
  supervisor `Unauthorized` whose message is the `device_auth_required` JSON therefore yields a 401 byte-identical to
  the auth middleware's, and the browser UI replaces the whole app with the token prompt (desktop re-checks its native
  credential). No credential leaks; reload recovers; repeated re-pastes can evict other browsers' enrollments past the
  64-enrollment cap. The helm itself never raises `Unauthorized`, so no legitimate 401 depends on this path.
  Orchestrator follow-up found the same trust-by-name pattern in other client-control signals: `CheckoutConflict` sets
  the "definitely unaccepted" create-outcome header, `NotFound` on `fetch_session` makes the UI treat the session as
  gone, and the stale-create `INCARNATION_MARKER` is a text match on a body the supervisor partly writes. Only the 401
  reaches beyond the lying host's own objects. The agent relay and terminal WebSocket paths were not audited.
- Decision: the user widened the fix from the narrow 401 remap to the general principle, and required a SPEC_impl.md
  rule: an error code or response at one level of abstraction is NEVER by default equivalent to one at another level,
  even under the same name or number. Translating across levels is valid only through explicit, case-by-case reasoning
  recorded where the translation happens; the current blanket supervisor-kind → HTTP-status mapping (including
  `Unauthorized` → 401) is exactly the default equivalence the rule forbids. Client-control signals (re-authenticate,
  create-outcome certainty, stale-create re-seed) are produced only by the helm's own reasoning.
- Completion criteria: SPEC_impl.md states the cross-level non-equivalence rule and that helm client-control signals are
  helm-originated only; `error_kind`/`http_error` map supervisor-originated kinds through an explicit, per-kind reasoned
  translation that can never produce the device-auth 401 (supervisor `Unauthorized` becomes a non-401 status); each
  retained translation (`NotFound`, `CheckoutConflict` and its create-outcome header, the others) carries its
  case-specific justification; the incarnation marker no longer depends on body text a supervisor can write; fix the doc
  comments claiming only the middleware emits `device_auth_required`; tests show a supervisor error cannot produce the
  401 or the stale-create signal; add a changelog fragment; remove the feedback file and its
  `review_feedback_queue/INDEX.md` entry in the execution change. Auditing the agent relay and terminal WebSocket for
  the same pattern is out of this item's scope unless the user adds it.
- Execution: `complete`; `SupervisorError` gained an `origin` (`Helm` or `SupervisorReply`, set only where a
  supervisor's `ControlMsg::Error` becomes one), and `http_error` translates supervisor-decided kinds through
  `supervisor_reply_status`, one justified arm per kind, with `Unauthorized` becoming 502; `error_kind` and so the agent
  relay are unchanged. The stale-connection refusal is now the typed `precondition::IncarnationStale` (or a quoted
  `AlsoFailedValidation` on the keyed fresh path), signalled by the helm-only `x-farhelm-precondition` header instead of
  a body marker, and the UI's create calls return a `CreateRefusal` whose `stale` comes only from that header.
  SPEC_impl.md records the cross-level rule. Recorded runs passed the whole `farhelm-helm` and `farhelm-ui` packages
  after one in-session failure on the fresh path's quoted refusal, fixed by `AlsoFailedValidation`; the desktop and wasm
  `web` builds compile. Fresh gpt-6-astra high review reported no findings. Draft PR
  [#1036](https://github.com/scode/farhelm/pull/1036/changes) is on bookmark `pr/remote-errors-cannot-sign-out`, jj
  change `lpyuqsyw`.

## agent-label-leaks-env-prefix.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `5cf4b12`, not live reproduction. `agent_label`
  (`farhelm-helm/src/agent_requests.rs`) takes the first shell word of a raw invocation and strips it to its basename,
  so `ANTHROPIC_API_KEY=sk-… claude` yields the label `ANTHROPIC_API_KEY=sk-…`. Create accepts that invocation
  (`ensure_executable_argv` refuses only empty argv, an empty program, or NUL), launch then fails because the argv is
  exec'd without a shell, and the error row keeps showing the key to every process holding any attached session's
  credential until the row is deleted. This contradicts the `AgentSession::agent` doc and SPEC_impl.md's non-secret
  label rule. Composer-built Goose sessions (`env GOOSE_…=… goose …`, `farhelm-helm/src/launches.rs`) and raw
  `env NAME=value prog` sessions all read as `env`; `SessionInfo::launch` is never consulted. Not covered by SPEC or a
  Planned TODO item.
- Decision: the user chose a closed vocabulary over smarter parsing, to prevent this class of leak without complexity:
  the label is the source profile's snapshotted name for a profile session (a deliberate user label, kept), and
  otherwise the supervisor's recorded integrated agent kind (`claude`, `codex`, `goose`, `pi`, `omp`, `grok`), or
  `custom` when there is no supported agent. The invocation is never parsed for the label. No create-time refusal of a
  leading `NAME=value` word.
- Completion criteria: SPEC_impl.md states the rule — the `agent` label is a profile's snapshotted name or a value from
  the closed agent-kind vocabulary plus `custom`, and never text derived from the invocation. The supervisor reports
  each session's recorded agent kind to the helm (an additive `SessionInfo` field whose absence, from an older
  supervisor, yields `custom`); `agent_label` uses it; the `AgentSession::agent` doc matches. Tests cover a raw
  `KEY=secret claude` session, a composer-built Goose session, a recognized raw agent, and an unrecognized program.
  Changelog fragment. Remove the feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; `SessionInfo` gained an additive `agent_kind` (serde default `generic`) that the supervisor
  fills from its recorded integration snapshot on every build path, and the helm's `agent_label` returns the profile's
  snapshotted name, the kind's word, or `custom`, never reading the invocation. Judged additive within protocol 31: an
  old helm ignores the field and keeps its old label; a new helm reading an old supervisor shows `custom`. SPEC_impl.md
  and the `AgentSession::agent` doc state the closed-vocabulary rule. Recorded runs passed the proto, helm, UI
  HTTP-contract, and a new supervisor reload test. Fresh gpt-6-astra high review found one stale clone explanation, now
  fixed. Draft PR [#1038](https://github.com/scode/farhelm/pull/1038/changes), jj change `yorprtsp`, bookmark
  `pr/agent-label-closed-vocabulary`.

## helm-answers-resolveprofile-to-any-supervisor.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `5cf4b12`, not live reproduction; practical impact narrower than
  the TLDR. The helm answers `ResolveProfile` from any supervisor connection with the profile's full invocation, agent
  kind, resume template, and snapshot (`farhelm-helm/src/agent_requests.rs`), validates only the selector shape, and
  logs nothing; only the asking supervisor refuses the verb from sessions
  (`farhelm-supervisor/src/service/
  handlers.rs`). This contradicts SPEC.md's "Remote input" statement that profile
  discovery does not extend to raw command lines, and its rule that a remote host must not gain access to secrets.
  However, the same compromised host can already obtain the same bundle through SPEC.md's accepted temporary exception
  for agent-requested cross-host creation: `farhelm agent create --profile X` aimed at its own host makes the helm send
  the resolved invocation and resume template to that host's supervisor. The finding's unique delta is the absence of a
  visible session and an audit line. Closing only this verb (helm-side create instead of returning the bundle) would not
  keep profile contents from that host. The module docs' "the trust boundary is the connection" rationale is broader
  than the intended trust model.
- Decision: accept the exposure, but only as part of the existing temporary cross-host creation exception, not as a
  standing grant. The user stated the intended trust model: the helm trusts a supervisor about things that affect only
  that supervisor's own host, and never about things that affect other sessions on other hosts, other supervisors, the
  helm's machine, or the helm's own state beyond functionality inherent to the helm (a supervisor cannot delete a
  profile, for example). Reaching beyond its own host is allowed only where the spec grants it. Arbitrary hosts may
  currently create sessions on other hosts because that functionality is needed now; the user intends to restrict
  cross-host spawning and interrogation to explicitly trusted environments. Answering `ResolveProfile` to any attached
  host continues that same temporary exception forward and must end with it. Represent this so future agents do not
  mistake it for a permanent grant or treat profiles as secret from attached hosts today.
- Completion criteria: SPEC_impl.md states the effect-scoped trust rule next to the directional-trust text. SPEC.md's
  temporary cross-host creation exception explicitly covers attached hosts obtaining resolved profile bundles, with the
  reason (the exception already lets any host launch any profile on itself) and that it is revoked with that exception;
  the "Remote input" profile-discovery paragraph points at this exception instead of implying profile command lines are
  never exposed; until then profiles are not a place for secrets that must be hidden from attached hosts. TODO.md's
  Maybe later entry "Close the cross-host execution hole…" is widened, at the maintainer's request, to the intended end
  state: only explicitly trusted environments may spawn sessions on, or interrogate session and profile data of, other
  hosts; arbitrary attached supervisors lose both. It names `ResolveProfile` and the fleet-wide session and host
  listings as in scope next to create, clone, and their retry paths; cross-host stop and rename stay allowed bounded
  operations. The helm logs every `ResolveProfile` with origin host, supervisor-claimed asking session, and profile id.
  The `ResolveProfile` arm carries a comment naming the temporary exception it depends on. The `agent_requests.rs` and
  `client.rs` trust docs are restated as effect-scoped trust rather than connection trust. Remove the feedback file and
  its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; SPEC.md's temporary cross-host creation exception now names profile resolution as covered by it
  and ending with it, and the "Remote input" paragraph points there; SPEC_impl.md gained "What the helm believes from a
  supervisor" with the effect-scoped trust rule and `ResolveProfile`'s dependence on the exception. The helm logs every
  `ResolveProfile` answer (origin host, supervisor-claimed asking session, profile id), the arm carries a comment naming
  the exception, and the relay's trust docs in `agent_requests.rs` and `client.rs` are restated as effect-scoped trust.
  TODO.md's Maybe later cross-host entry is widened to the trusted-environments end state, naming `ResolveProfile` and
  the fleet listings. Draft PR [#1039](https://github.com/scode/farhelm/pull/1039/changes), jj change `uuslzvxl`,
  bookmark `pr/resolveprofile-temporary-exception`.

## tmux-cwd-format-expanded-on-create.md

- Outcome: `fix code`.
- Assessment: Session create passes the raw cwd to `new-session -c` (`farhelm-supervisor/src/tmux.rs`,
  `create_session`). confirmed at `5cf4b12` by inspection and by a reproduction on a private server with the pinned tmux
  3.7c: a pane started with `-c …/C#Samples` landed in `$HOME`; a `-c` path containing `#(touch …/MARK)` ran the `touch`
  (even though the directory did not exist); escaping `#` as `##` made the `C#Samples` pane land in the literal
  directory and kept an escaped `#(touch …/MARK2)` path from running. `ensure_cwd_usable`
  (`farhelm-supervisor/src/service/core.rs`) checks only the literal path, so it passes. No spec acceptance or Planned
  TODO item covers this.
- Decision: the user chose `fix code` for this and its two sibling call sites (create, relaunch, tab open), executed as
  three stacked PRs per the one-PR-per-item rule rather than merged into one.
- Completion criteria: Add one shared helper that escapes every `#` as `##` for a tmux `-c` value, documented with why
  (tmux format-expands `-c`; `-e` values and the command after `--` are not expanded), and use it in `create_session`.
  Real-tmux regression tests create sessions in directories named with `#(touch marker)`, `#S`, and `##`, asserting the
  pane's directory is the literal path and the marker never appears. First of the three stacked PRs; the helper lands
  here. Changelog fragment. Remove the feedback file and its `review_feedback_queue/INDEX.md` entry in the execution
  change.
- Execution: `complete`; `tmux_start_directory` in `farhelm-supervisor/src/tmux.rs` spells every `#` as `#{a:35}` for a
  tmux `-c` value, documented with why only `-c` needs it, and `create_session` uses it. The review found that the
  suggested `##` escape leaves `#[` style runs verbatim (verified: `p#[x]` landed in `$HOME`), which is why the modifier
  replaced it; tmux 3.7c is both the pin and the floor, so the modifier is always available. A real-tmux regression test
  creates sessions in directories named with a `#(...)` job, `#S`, `##`, `#[`, `##[`, and an unterminated `#{`, and
  asserts the pane's literal directory and that the job never ran; it was confirmed to fail with the escape disabled.
  Draft PR [#1040](https://github.com/scode/farhelm/pull/1040/changes), jj change `mvkzrulp`, bookmark
  `pr/tmux-cwd-escape-create`.

## tmux-cwd-format-expanded-on-relaunch.md

- Outcome: `fix code`.
- Assessment: Restart in place passes the resolved cwd raw to `respawn-pane -c` (`relaunch_in_pane`), defeating the
  `ensure_cwd_identity` check made just before. confirmed at `5cf4b12` by inspection and by a reproduction on a private
  server with the pinned tmux 3.7c: a pane started with `-c …/C#Samples` landed in `$HOME`; a `-c` path containing
  `#(touch …/MARK)` ran the `touch` (even though the directory did not exist); escaping `#` as `##` made the `C#Samples`
  pane land in the literal directory and kept an escaped `#(touch …/MARK2)` path from running. `ensure_cwd_usable`
  (`farhelm-supervisor/src/service/core.rs`) checks only the literal path, so it passes. No spec acceptance or Planned
  TODO item covers this.
- Decision: the user chose `fix code` for this and its two sibling call sites (create, relaunch, tab open), executed as
  three stacked PRs per the one-PR-per-item rule rather than merged into one.
- Completion criteria: `relaunch_in_pane` uses the shared helper; a real-tmux regression test restarts a session in such
  directories with the same assertions. Stacked on the create item's PR. Changelog fragment. Remove the feedback file
  and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; `relaunch_in_pane` passes its `respawn-pane -c` value through `tmux_start_directory`. A
  real-tmux regression test relaunches one pane into directories named with a `#(...)` job, `#S`, and `##`, and asserts
  the literal directory each time and that the job never ran. The create PR's changelog fragment now covers restart.
  Draft PR [#1041](https://github.com/scode/farhelm/pull/1041/changes), jj change `rppmsvws`, bookmark
  `pr/tmux-cwd-escape-relaunch`.

## tmux-cwd-format-expanded-on-tab-open.md

- Outcome: `fix code`.
- Assessment: Opening a tab passes the session cwd raw to `new-window -c` (`new_window`), contradicting SPEC.md's rule
  that unusable tab paths fail clearly. confirmed at `5cf4b12` by inspection and by a reproduction on a private server
  with the pinned tmux 3.7c: a pane started with `-c …/C#Samples` landed in `$HOME`; a `-c` path containing
  `#(touch …/MARK)` ran the `touch` (even though the directory did not exist); escaping `#` as `##` made the `C#Samples`
  pane land in the literal directory and kept an escaped `#(touch …/MARK2)` path from running. `ensure_cwd_usable`
  (`farhelm-supervisor/src/service/core.rs`) checks only the literal path, so it passes. No spec acceptance or Planned
  TODO item covers this.
- Decision: the user chose `fix code` for this and its two sibling call sites (create, relaunch, tab open), executed as
  three stacked PRs per the one-PR-per-item rule rather than merged into one.
- Completion criteria: `new_window` uses the shared helper; a real-tmux regression test opens a tab in such directories
  with the same assertions. Stacked on the relaunch item's PR. Changelog fragment. Remove the feedback file and its
  `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; `new_window` passes its `new-window -c` value through `tmux_start_directory`. A real-tmux
  regression test opens tabs in directories named with a `#(...)` job, `#S`, `##`, `#[`, `##[`, and an unterminated
  `#{`, and asserts each tab's literal directory and that the job never ran. The create PR's changelog fragment now
  covers tabs. Draft PR [#1042](https://github.com/scode/farhelm/pull/1042/changes), jj change `mpkrowty`, bookmark
  `pr/tmux-cwd-escape-tab`.

## supervisor-stop-closes-clients-output-on.md

- Outcome: `fix code`.
- Assessment: code shape confirmed at `5cf4b12` by inspection; the crash consequence on the pinned tmux is unverified.
  No crate installs a signal handler (no `tokio::signal`, `signal_hook`, or `sigaction`), the generated units use
  `KillMode=process` (`farhelm-helm/src/units.rs`), and `farhelm helm setup` restarts the units
  (`farhelm/src/setup.rs`), so a planned stop or upgrade SIGTERMs the supervisor into the same abrupt death as SIGKILL,
  closing every output-bearing control client without the acknowledged no-output step that `OutputStream::shutdown` and
  `shutdown_output_control_client` exist to perform. BUGS.md's evidence for the resulting server abort is 5 in roughly
  5,000 SIGKILLs under saturation on distro tmux 3.6, with the `not enough data` identification inferred; whether pinned
  tmux 3.7c aborts on this path is not established. BUGS.md's "a SIGKILLed supervisor runs nothing" rationale does not
  apply to catchable SIGTERM. No spec acceptance or Planned TODO item covers this.
- Decision: the user chose `fix code`, and required that the reason for the handler be clearly documented where it
  lives, so a future reader does not remove it as unnecessary shutdown ceremony.
- Completion criteria: `farhelm supervisor run` handles SIGTERM and SIGINT by running the existing orderly teardown
  (output disabled and acknowledged on every output client and sink before any is closed) within a bounded budget well
  under systemd's stop timeout, then exits. The handler's docs state why it exists: the tmux abort that takes down every
  session on the host, that `KillMode=process` makes every planned stop and upgrade a SIGTERM to the supervisor alone,
  that SIGKILL remains the accepted BUGS.md residual, and what the budget bounds. A test SIGTERMs a supervisor with
  output-bearing clients and shows output was disabled before their close. BUGS.md's entry is narrowed to abrupt deaths
  and points at the handler. Changelog fragment. Remove the feedback file and its `review_feedback_queue/INDEX.md` entry
  in the execution change.
- Decision note (superseding scope, from the goal-building session): the user asked that the desktop app's stdin tether
  exit run the same orderly output teardown as SIGTERM and SIGINT.
- Execution: `complete`; `farhelm_supervisor::service::run` now installs SIGTERM and SIGINT handlers before serving and
  takes the desktop tether as a stop future (`main.rs` passes it instead of racing it). Every stop runs
  `Supervisor::shutdown_output_clients` inside one 10-second budget that also covers waiting for the attachments lock:
  it first sets an irreversible stop boundary under that lock (a `stopping` flag in the sink registry), after which no
  new session sink is handed out and an attach reaching the lock is refused before opening any output client; then it
  runs whole-session delete's orderly sequence without the session filter (every forwarder's barrier published and
  signalled under the lock before any join, sink leases dropped after their forwarders) and waits until no sink is still
  owned and every reaper settled, then exits 0. The first review showed the planner's "an attach racing the stop is a
  residual" was below the user's "every output client and sink"; a scope reassessment upheld that and prescribed this
  boundary, built from the existing locks and registries. Docs on `run` and the method state why: the tmux abort,
  `KillMode=process`, the SIGKILL residual, what the budget bounds. BUGS.md's entry is narrowed to deaths that run no
  code. Tests: real-process e2e tests stop a supervisor holding a live, streaming attachment by SIGTERM and by closing
  the tether, and through a `--tmux` wrapper observe an acknowledged `no-output` on both output clients after the stop
  began, plus exit status 0 and the session surviving (confirmed to fail with the shutdown disabled); unit tests show a
  held sink blocks completion and new sinks are refused, the budget covers a held attachments lock, and an attach that
  got its sink before the stop is refused (confirmed to fail without the check). A second review round found that the
  cleanup wait read live sinks and reapers in two lock holds (now one snapshot, with a test holding a released sink's
  reaper) and that the tether was not watched during construction (now selected against it, with an e2e test that closes
  the tether while tmux startup hangs); both confirmed to fail before the fix. Whether pinned tmux 3.7c actually aborts
  on the old path remains unverified. Draft PR [#1043](https://github.com/scode/farhelm/pull/1043/changes), jj change
  `ztltrkzv`, bookmark `pr/supervisor-orderly-stop`.

## checkout-can-take-archive-dir-name.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `5cf4b12`. The fresh-checkout occupancy scan deliberately skips
  `ARCHIVE_DIR_NAME` (`farhelm-supervisor/src/working_copies.rs`), and `fresh_root_constraint_error`
  (`farhelm-supervisor/src/service/core.rs`) derives the reserved archive path only from non-retired registry rows that
  have a recorded path, so the first checkout in a root, or one whose checkouts are all retired, can be named
  `farhelm-archived-working-copies` by an ordinary title (repo `farhelm`, title "archived working copies"). Later
  archive moves then land inside that clone, and its own delete fails with `EINVAL` and stays `archive_pending`. The
  restater's correction holds: `git clean`/`git add -A` do not destroy or commit nested repositories; the realistic loss
  is the user removing the colliding clone by hand. Low likelihood, innocent trigger. No spec acceptance or Planned TODO
  item covers this.
- Decision: the user chose `fix code`; executed as its own PR, below the archive-side companion.
- Completion criteria: the preview and the create-time destination recheck refuse a planned basename equal to
  `ARCHIVE_DIR_NAME` regardless of what the registry holds (the occupancy scan reports the name as taken when it is the
  candidate, or the basename rule skips it). A test uses a root with no registry rows. Changelog fragment. Remove the
  feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; the occupancy scan both the preview and the create-time recheck use (`occupied_related_names`)
  now reports `ARCHIVE_DIR_NAME` as occupied whenever it is related to the repository's name, whether or not it exists,
  instead of skipping it, so a title that composes to the reserved name is refused as occupied. A test covers the three
  repository/title splits that produce the name, in an empty root and in one where the archive directory exists. Draft
  PR [#1044](https://github.com/scode/farhelm/pull/1044/changes), jj change `txkxuztw`, bookmark
  `pr/reserve-archive-dir-name`.

## archive-root-accepts-active-checkout.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `5cf4b12`. `ensure_archive_root`
  (`farhelm-supervisor/src/working_copies.rs`) accepts any real, non-symlink, same-filesystem directory at the archive
  path; `refuse_overlapping_archive` compares only the source with other active rows and never the destination; the row
  is journalled `archive_pending` before the rename, and `rename_noreplace` maps only `EEXIST`/`ENOTEMPTY` to a
  collision, so a source that is the archive directory fails with a generic `EINVAL` on every retry and startup
  reconciliation. This is the protection for databases that already hold a colliding checkout, and defense in depth for
  any other path by which an active checkout lands there.
- Decision: the user chose `fix code`; executed as its own PR, stacked on the creation-side item.
- Completion criteria: before the journal is written in `archive_move_with_effects`, and in
  `reconcile_archive_with_effects`, refuse the move when the archive directory equals, contains, or lies inside the path
  of any non-retired row, including the row being moved, so the row stays `allocated` rather than turning pending; the
  source-is-the-archive-directory case gets its own clear error instead of a raw `EINVAL`. Tests cover an active
  checkout at the archive path, both as another row and as the row being moved. Changelog fragment. Remove the feedback
  file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; a row that is, contains, or sits inside its root's archive directory gets `SourceIsArchiveRoot`
  from `refuse_overlapping_archive` (before archiving or recovery touch anything, since such a move can never have
  completed), and a new `refuse_archive_destination_checkout` refuses with `ArchiveRootIsCheckout` when another
  non-retired checkout occupies the archive directory: a fresh archive checks it before writing its journal, and
  recovery checks it only on paths that would still rename. The review found that checking it before recovery's
  matching-destination completion would strand an archive an older version had already finished; that case now
  completes. Tests cover both refusals through `archive_move` (rows stay `allocated`, nothing moves), an
  `archive_pending` squatter through `reconcile_archive`, and recovery completing a finished move into a squatter. Draft
  PR [#1045](https://github.com/scode/farhelm/pull/1045/changes), jj change `mztxqtvn`, bookmark
  `pr/archive-refuses-checkout-destination`.

## inode-reuse-defeats-ownership-check.md

- Outcome: `fix code`.
- Assessment: confirmed at `5cf4b12` by inspection and reproduction. `verify_identity`
  (`farhelm-supervisor/src/working_copies.rs`) and the other pre-destructive checks compare only `(st_dev, st_ino)`; on
  this host's ext4 a directory removed and recreated at the same path got the same inode every time, including when
  recreated by `git clone`. Deleting the old session then archives the new, foreign directory, contrary to SPEC.md's "A
  foreign object replacing the recorded path must remain untouched." The folder is moved within the same root, not
  destroyed. Trigger: the user removes a managed checkout by hand and re-clones or creates something at the same path.
  Bucket: the user agreed this is `high` (a surprising move of user work, recoverable) rather than `highest`; the index
  is left unchanged until execution removes the item.
- Decision: the user agreed with `fix code` using birth time as the reuse-proof discriminator, with the lenient rule for
  rows recorded before the change.
- Completion criteria: record the directory's birth time (Linux `statx` `STATX_BTIME`, macOS `st_birthtime`) alongside
  device and inode for new checkouts and roots; a mismatch, or an unreadable value where one was recorded, is
  `DifferentObject` in `verify_identity`, `archive_move_with_effects`, `reconcile_archive_with_effects`, and
  `verified_root`. Rows recorded before the change keep today's `(dev, ino)` comparison, documented as the accepted
  residual. Tests cover a recreated directory with a reused inode (a controlled birth-time mismatch where the filesystem
  cannot be made to reuse an inode) and a legacy row. Changelog fragment. Remove the feedback file and its
  `review_feedback_queue/INDEX.md` entry in the execution change.
- Decision note (from the goal-building session): the user accepted the schema migration for birth-time columns.
- Execution: `complete`; each checkout's and its root's birth time is recorded in two nullable columns added by the
  supervisor's 22→23 migration: the root's when the plan is recorded (if the root still has the identity admission saw),
  the checkout's at allocation. On Linux the birth time comes from a direct `statx` call asking for `STATX_BTIME`,
  because the standard library's `Metadata::created` never reports it on musl, which the released Linux binaries use
  (the first review caught this); macOS uses `st_birthtime`. Device, inode, and birth time come from one observation.
  One helper, `same_directory`, holds the rule every ownership check uses (`verify_identity`, `verified_root`, the
  pre-mkdir root check, the archive move, and recovery): same `(dev, ino)` and, when a birth time was recorded, the same
  birth time, with an unreadable one counting as different; rows without one keep the `(dev, ino)` comparison,
  documented as the residual. Tests plant a different birth time (identity reports a different object, archive refuses
  and moves nothing, root check fails, a planned row's retry refuses to allocate, a legacy row still matches) and
  recreate a checkout directory on the real filesystem, which on this host reused the inode. They fail rather than skip
  when coreutils `stat` shows the filesystem reports birth times; they pass built for both glibc and musl. Draft PR
  [#1046](https://github.com/scode/farhelm/pull/1046/changes), jj change `kxvrxyks`, bookmark
  `pr/directory-identity-birth-time`.

## archive-dir-owner-not-checked.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `5cf4b12`, conditional on a checkout root another local account
  can write. `ensure_archive_root` (`farhelm-supervisor/src/working_copies.rs`) checks neither owner nor mode of an
  existing archive directory and creates a missing one with plain `create_dir` (0777 minus umask). Nothing checks the
  checkout root's owner or mode either (`verified_root` has no uid or mode check), and no spec rule addresses shared
  roots. A co-user who can write the root can pre-create the archive directory, receive the victim's archived checkouts,
  and lock the victim out; the same co-user has other openings in such a root, so an archive-only check is incomplete.
- Decision: the user agreed to address the class one level up, and required the spec to state that Farhelm is NOT
  designed for checkout roots shared with, or writable by, other Unix accounts. The user also asked for a Doc todo entry
  to tell users this.
- Completion criteria: SPEC.md states that checkout roots shared with or writable by other local accounts are
  unsupported and outside the design. The supervisor refuses, at preview and at create, a checkout root not owned by its
  effective uid or that is group- or world-writable, with a message naming the reason. `ensure_archive_root` applies the
  same owner and mode check to an existing archive directory and creates a missing one 0700 (reusing
  `ensure_private_dir`'s approach). TODO.md's Doc todo bucket gains an entry (added at the maintainer's request) to tell
  users that checkout roots must be private to their account and shared directories are unsupported. Tests cover a
  group-writable root, a foreign-owned archive directory, and the created directory's mode. Changelog fragment. Remove
  the feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Decision note (supersedes the outcome above, from the goal-building session): the user changed this item to
  `fix
  spec` only: "lets' just not refuse at all, just have the spec update to clarify we aren't desining fro this.
  the user is responsible for keeping their checkouts in self-writable locations." No refusal of roots or of the archive
  directory, no owner or mode check, no change to how the archive directory is created. (The strict refusal would have
  refused ordinary roots under a 0002 umask with user-private groups, and broken archiving for existing installs.)
- Execution: `complete`; SPEC.md's fresh-checkout section states that Farhelm is not designed for working-copy roots
  other local accounts can write to, that keeping the root in a location only the user can write is the user's
  responsibility, that Farhelm does not check, and that the section's ownership and archive promises assume it.
  TODO.md's Doc todo bucket gained the entry to tell users. No code. Draft PR
  [#1047](https://github.com/scode/farhelm/pull/1047/changes), jj change `rtwnvrkk`, bookmark
  `pr/shared-checkout-roots-unsupported`.

## superseded-launch-specs-never-removed.md

- Outcome: `fix spec+code`.
- Assessment: mechanics confirmed by current-code inspection at `5cf4b12`; security impact essentially nil. The startup
  sweep keeps every generation's spec for a session that still exists, and its comment ("the restart that superseded it
  removes its own predecessor") describes code that does not exist, so an earlier generation's unread spec survives
  until Delete. But everything a spec holds (argv, session id, session token, paths; `farhelm-supervisor/src/launch.rs`)
  is already on disk in the same private state directory for the same lifetime: the supervisor database stores the full
  invocation and the plaintext session credential, deliberately recoverable for restart
  (`farhelm-supervisor/src/store.rs`). A leftover spec therefore exposes nothing new, and only to the trusted same
  account. By the bucket rules this is `other` (cleanup and an unkept spec promise), not `highest`.
- Decision: the user agreed with the recommendation and declined to discard: correct the spec's framing of launch-spec
  cleanup once, and make the small sweep fix.
- Completion criteria: SPEC_impl.md's runtime-state text says launch specs duplicate what the session's database row
  holds for the session's lifetime, so removing them is tidiness rather than a credential boundary; a current-generation
  spec left by a launch that never ran may remain until Delete, which removes it; the stale "one 0600 JSON spec per
  session" wording reflects per-generation naming. The startup sweep also removes specs (and their sentinels) whose
  generation is below the row's current generation, with its comment corrected; a test covers a superseded unread spec.
  Remove the feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; SPEC_impl.md's runtime-state text now says launch specs are per launch
  (`<session>.<generation>.json`), hold nothing the session's database row does not already hold for its lifetime, so
  removing them is tidiness rather than a credential boundary, and that a current-generation spec whose launch never
  reached the shim may stay until Delete whichever path observed the outcome (startup reconciliation, the runtime
  observers, or Stop), which is what items `reload-leaves-unread-launch-spec.md` and
  `observers-leave-unread-launch-spec.md` rely on. The startup sweep takes each session's current generation and also
  removes a superseded launch's spec and sentinel (sentinel reads address one exact generation, so an older one is
  unreachable; sentinels of sessions no longer on record stay, as before), with its docs corrected; a test pins the
  current launch's files kept and a superseded launch's removed. The first review caught that an earlier draft kept
  superseded sentinels. Draft PR [#1048](https://github.com/scode/farhelm/pull/1048/changes), jj change `kzokpnwp`,
  bookmark `pr/sweep-superseded-launch-specs`.

## reload-leaves-unread-launch-spec.md

- Outcome: `fix spec`.
- Assessment: mechanics confirmed by current-code inspection at `5cf4b12`: startup reconciliation calls
  `cleanup_launch_artifacts` only for Error outcomes, so the reboot conversion to Interrupted and unscoped early-exit
  shapes leave the current generation's unread spec until Delete. Same nil marginal exposure as
  `superseded-launch-specs-never-removed.md`: the database row holds the same data for the same lifetime.
- Decision: the user agreed: no reload cleanup; the spec principle recorded for
  `superseded-launch-specs-never-removed.md` covers this case explicitly.
- Completion criteria: the SPEC_impl.md sentence from `superseded-launch-specs-never-removed.md` explicitly covers a
  current-generation spec left after reload's Interrupted or exited outcomes (add wording in this item's change if that
  PR's text does not already). Remove the feedback file and its `review_feedback_queue/INDEX.md` entry in the execution
  change.
- Execution: `complete`; no wording was added: the SPEC_impl.md runtime-state sentence landed with
  `superseded-launch-specs-never-removed.md` already names startup reconciliation among the paths after which a
  current-generation spec whose launch never reached the shim may stay until Delete. This change only records that and
  removes the queue item. Draft PR [#1049](https://github.com/scode/farhelm/pull/1049/changes), jj change `rvlwskzq`,
  bookmark `pr/reload-unread-spec-accepted`.

## observers-leave-unread-launch-spec.md

- Outcome: `fix spec`.
- Assessment: mechanics confirmed by current-code inspection at `5cf4b12`: the ticker, listing, single-session reply
  observers, and Stop (`StopCompleted`) clean launch artifacts only on Error, so a pane that died before the shim ran
  leaves the current generation's spec until Delete; the restater's anchor correction for Stop holds. Same nil marginal
  exposure as `superseded-launch-specs-never-removed.md`.
- Decision: the user agreed: no observer or Stop cleanup; the same spec principle covers this case explicitly.
- Completion criteria: the same SPEC_impl.md sentence explicitly covers specs left by runtime observers and Stop (add
  wording in this item's change if needed). Remove the feedback file and its `review_feedback_queue/INDEX.md` entry in
  the execution change.
- Execution: `complete`; no wording was added: the same SPEC_impl.md sentence already names the runtime observers and
  Stop among the paths after which an unread current-generation spec may stay until Delete. This change only records
  that and removes the queue item. Draft PR [#1050](https://github.com/scode/farhelm/pull/1050/changes), jj change
  `ysuyklky`, bookmark `pr/observer-unread-spec-accepted`.

## codex-hook-trust-bypass-runs-repo-hooks.md

- Outcome: `fix spec`.
- Assessment: confirmed from code at `5cf4b12` plus Codex's published docs; not reproduced against a live Codex (0.156.1
  installed locally). Farhelm side: `--dangerously-bypass-hook-trust` is added to every injected Codex launch
  (`farhelm-supervisor/src/agent_kind/mod.rs`), the `{codex:trusted-cwd}` override trusts the folder (including fresh
  checkouts, per SPEC.md), and `with_hook_argv_using` never skips injection for trusted or hook-bearing folders. Codex
  side (docs at learn.chatgpt.com/docs/hooks): project `.codex/` hooks load only in a trusted project, then each hook
  needs hash-based review; the flag runs enabled hooks without persisted hook trust for that invocation. So in a trusted
  folder a repository's own hooks run unreviewed, outside Codex's sandbox, at the first prompt. SPEC_impl.md's accepted
  cost names only hooks in the user's own configuration home. Marginal exposure is smaller than the finding implies: per
  Codex's docs, trusting a project already loads its whole `.codex/` layer, including project `mcp_servers` commands;
  whether those start without a prompt on this Codex version was not verified. Skipping injection would lose
  conversation identity, and so resume, for trusted checkouts.
- Decision: the user accepts the exposure for now, explicitly as temporary: it holds until hook installation becomes an
  explicit step surfaced to the user, in which the user is told what is being installed and accepts specific hooks, so
  Farhelm no longer needs to pass the bypass arguments per launch.
- Completion criteria: SPEC_impl.md's accepted-cost text for Codex hook injection is widened to cover hooks from a
  trusted project's `.codex/` layer running without per-hook review, with the reason, and states that the acceptance
  lasts only until hook installation is an explicit, user-surfaced step with per-hook acceptance, after which the
  per-launch bypass is not passed. TODO.md's Maybe later bucket gains an entry (added at the maintainer's request) for
  that move: a separate "install hooks" process that tells the user what is being installed and lets them accept
  specific hooks, so launches need not pass `--dangerously-bypass-hook-trust` and its overrides. TODO.md's Doc todo
  bucket gains an entry (added at the maintainer's request) to document for users that on Farhelm-injected Codex
  launches, unapproved hooks in the user's config home and in a trusted workspace's `.codex/` run without Codex's
  per-hook review, and that trusting a workspace means trusting its Codex configuration to run commands. Remove the
  feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Decision note (from the goal-building session): no user-facing documentation is written now; the spec text and the two
  TODO.md entries are the whole change.
- Execution: `complete`; SPEC_impl.md's accepted cost for Codex hook injection now covers a trusted project's own
  `.codex/` hooks running without Codex's per-hook review, gives the reasons (trusting a workspace already lets its
  Codex configuration, MCP servers included, run commands; skipping injection would drop resume for trusted checkouts),
  and says the acceptance lasts only until hook installation is an explicit, user-surfaced step with per-hook
  acceptance. TODO.md gained that step in Maybe later and a Doc todo entry to tell users, both at the maintainer's
  request. No code. Draft PR [#1051](https://github.com/scode/farhelm/pull/1051/changes), jj change `lrktwomx`, bookmark
  `pr/codex-repo-hooks-accepted`.

## create-reply-foreign-id-misroutes.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `5cf4b12`. When a create on host B returns an id host A already
  caches, the write-back refusal (`SessionOwnerAmbiguous`) is only logged in `record_session`
  (`farhelm-helm/src/sessions.rs`), with a log line claiming the id "will not be routed while both keep claiming it";
  `resolve_owner` sees A as cached owner and no contest until B's next refresh, so operations on the "new" session route
  to A's session for that window. `resolve_owner`'s docstring cites a nonexistent `AppState::contested_sessions`.
  Requires a buggy or hostile B (honest supervisors mint UUIDs). Marginal harm is small: SPEC.md already lets any agent
  stop sessions on other hosts, and misrouted input goes to the user's own session on A, not to B. By the bucket rules
  this is `other` (correctness with a hostile-only trigger), not `highest`.
- Decision: the user chose `fix code`.
- Completion criteria: when the post-create write-back fails with `SessionOwnerAmbiguous`, the create fails with a
  conflict naming both hosts rather than reporting success, and an immediate refresh of the creating host is requested;
  the misleading log line and the `resolve_owner` docstring are corrected. A test covers a create reply naming an id
  another host caches. Changelog fragment. Remove the feedback file and its `review_feedback_queue/INDEX.md` entry in
  the execution change.
- Execution: `complete`; `record_session` now returns the `SessionOwnerAmbiguous` refusal (after requesting an immediate
  refresh of the reporting host so the id is marked contested) instead of only logging it, and `accept_created_session`,
  which first replies and reconciled replies both use, fails the create with a 409 naming the creating host and the
  colliding id and saying the session may exist there; no rollback is attempted. Rename and restart replies keep
  ignoring it, since their id was already routed. The misleading log line, `resolve_owner`'s docstring (the nonexistent
  `AppState::contested_sessions`), and `accept_created_session`'s best-effort wording are corrected. A helm test creates
  on one host with a reply naming another host's cached id. SPEC_impl.md's mutation write-back paragraph, which said no
  write-back can fail a mutation, now names this one exception (the review caught the contradiction). Draft PR
  [#1052](https://github.com/scode/farhelm/pull/1052/changes), jj change `trqunwmq`, bookmark
  `pr/create-reply-foreign-id-refused`.

## create-reply-sets-remembered-yolo.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `5cf4b12`. `accept_created_session`
  (`farhelm-helm/src/sessions.rs`) records create history with `LaunchChoiceMemory::Remember` for every user-origin
  create, and `record_launch_history` (`farhelm-helm/src/store.rs`) derives both the `launch_history` row's selection
  and the helm-wide `remembered_permissions`/`remembered_workspace_trust` from `entry.launch`, the supervisor's reply.
  Nothing compares it with what the helm sent; a raw or profile create sends no selection yet a reply carrying one is
  recorded. A hostile host can therefore set the remembered defaults to yolo with workspace trust for every host.
  Contradicts SPEC.md "Remote input, session defaults, and availability".
- Decision: the user stated the governing rule: only things the user explicitly selects in the GUI may affect future GUI
  defaults and suggestions. This covers the remembered permission and workspace-trust defaults, the remembered profile,
  and the recent-setups history the New dialog ranks its one-click recents from (per host, per folder). A choice is
  still recorded only after its create succeeds: the user's selection decides the value, the host's success only whether
  it is recorded. The rule goes into SPEC.md explicitly.
- Completion criteria: SPEC.md states the rule as the principle for remembered launch defaults and composer history,
  replacing or grounding the narrower "remote metadata must not override…" framing. The helm carries its own submitted
  selection (the compiled structured selection, or none) through create acceptance, and both the remembered defaults and
  the `launch_history` row's selection come from it, never from `entry.launch`; reply-only facts (session id, creation
  time, canonical cwd) still come from the reply. Tests cover a reply whose `launch` differs from the submitted
  selection, and a raw/profile create whose reply carries a selection. Changelog fragment. Remove the feedback file and
  its `review_feedback_queue/INDEX.md` entry in the execution change.
- Decision note (from the goal-building session): the user applied the rule literally to agent creates too; sessions an
  agent creates no longer add recent-setups rows. The New dialog's folder history is separate and unchanged.
- Execution: `complete`; SPEC.md states the rule (only explicit GUI selections shape remembered permission/trust, the
  remembered profile, and recent setups; recorded from the user's selection in the request that succeeded, never from a
  reply or a listed row) and SPEC_impl.md's schema-30 note matches. `CreateAcceptance` carries `explicit_selection` (a
  user-origin structured create's compiled selection, or the fresh-checkout request's own launch), and
  `record_create_history_with_destination` takes it in place of `LaunchChoiceMemory`, which is gone: no selection writes
  neither a history row nor a remembered default; a selection writes both from itself, whatever the reply says. Store
  tests cover no selection, a selection, and a reply that disagrees with the submission; a REST test submits the default
  permissions while the reply claims yolo with workspace trust, then a raw create with the same reply. The plain-Replace
  path still passes its copied selection; `replace-records-peer-launch-defaults.md` changes that. The review caught that
  the fresh-checkout path recorded the raw request selection while ordinary creates record the compiled one (they differ
  for Pi with omitted permissions); both now use `launches::normalize_selection`, with a test that it equals what
  `compile` records. Draft PR [#1053](https://github.com/scode/farhelm/pull/1053/changes), jj change `rstzutwr`,
  bookmark `pr/explicit-selection-drives-defaults`.

## create-reply-launch-mirrored-into-client-defaults.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `5cf4b12`. `ListView`'s `on_created` handler
  (`farhelm-ui/src/list/view.rs`) writes the create reply's `session.launch` permissions and workspace trust into the
  page's `remembered_permissions`/`remembered_workspace_trust` mirror, which seeds every later New dialog and "reset
  choices" until reload, independent of the helm's stored value.
- Decision: the same rule as `create-reply-sets-remembered-yolo.md`: only explicit GUI selections affect future GUI
  defaults. Executed as the PR adjacent to that item.
- Completion criteria: the client mirror derives from the selection the form submitted (none for command/profile
  creates), never from the reply; the SPEC.md rule from the adjacent item covers it. A UI test shows a reply carrying a
  different launch leaves the next New dialog's preselection unchanged. Remove the feedback file and its
  `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; the create form hands `on_created` a `CreatedSession` carrying the structured launch it
  submitted (none for command and profile creates) beside the returned session, and `ListView` mirrors remembered
  permissions and workspace trust through `mirror_submitted_launch`, which takes only that submitted selection and so
  cannot see the reply's `launch`. A UI unit test pins the helper, and (the review asked for the wiring to be tested
  through the page) a browser test in `e2e/tests/remembered-permissions.spec.ts` rewrites every create reply to claim
  yolo with workspace trust and checks that a structured default-permissions create and a command create both leave the
  next New dialog on "default"; it passed on Chromium and WebKit. The UI crate's tests pass and its desktop and wasm
  `web` builds compile. The SPEC.md rule came with `create-reply-sets-remembered-yolo.md`. Draft PR
  [#1055](https://github.com/scode/farhelm/pull/1055/changes), jj change `lqvkutlp`, bookmark
  `pr/ui-mirrors-submitted-launch`.

## replace-records-peer-launch-defaults.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `5cf4b12`. A plain Replace (no `with` body) builds its create from
  the source row the owning host lists (`mode_from_source`, `farhelm-helm/src/sessions.rs`) and runs it as
  `CreateOrigin::User`, so the peer's launch selection is recorded as the user's remembered permission and trust choice.
  The fix for `create-reply-sets-remembered-yolo.md` alone does not close this, because the submitted selection itself
  was copied from the peer.
- Decision: the same rule: a plain Replace involves no GUI selection, so it moves no remembered defaults or history
  suggestions; a "replace with" the user filled in does count.
- Completion criteria: a create whose mode is derived from a source row records no remembered launch choices (and no
  history suggestion derived from the peer row); composer creates and explicit "replace with" bodies still do. A test
  covers a plain Replace of a yolo row leaving the remembered defaults unchanged. Remove the feedback file and its
  `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; `CreateSpec` gained `settings_from_source`, set by a plain Replace (no `with` body) and false
  everywhere else, and a create with it set records no explicit selection, so no remembered permission or trust default
  and no recent setup. A "replace with" body the user filled in still records its selection. A REST test replaces a
  listed yolo-with-trust row and checks the preferences and recent setups are untouched; it fails with the flag
  disabled. The SPEC.md rule came with `create-reply-sets-remembered-yolo.md`. Draft PR
  [#1060](https://github.com/scode/farhelm/pull/1060/changes), jj change `uuzskvwk`, bookmark
  `pr/plain-replace-records-no-launch-choices`.

## replace-sets-peer-default-profile.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `5cf4b12`. For a plain Replace whose source row names a
  `source_profile`, `mode_from_source` resolves that catalog id, and `accept_created_session` calls
  `remember_default_profile` for the user-origin create, so a host listing a session under any catalog profile id sets
  the helm-wide default profile.
- Decision: the same rule: a plain Replace is not an explicit GUI profile choice and must not move the default profile.
- Completion criteria: `remember_default_profile` is skipped when the create mode was derived from a source row; it
  still runs for the composer and for a "replace with" body naming a profile the user chose. A test covers a plain
  Replace of a row claiming a different profile. Remove the feedback file and its `review_feedback_queue/INDEX.md` entry
  in the execution change.
- Execution: `complete`; the same `settings_from_source` flag now also clears the remembered profile for a create whose
  mode was derived from a listed row, so `remember_default_profile` never runs for a plain Replace; the composer and a
  "replace with" body naming a profile still set it. A REST test replaces a row that claims a catalog profile and checks
  the remembered profile stays unset; it fails with the new arm disabled. Draft PR
  [#1063](https://github.com/scode/farhelm/pull/1063/changes), jj change `oqwpvkuw`, bookmark
  `pr/plain-replace-keeps-default-profile`.

## remote-host-contests-foreign-session-ids.md

- Outcome: `fix spec`.
- Assessment: confirmed by current-code inspection at `03a3051`, not runtime reproduction. The list keeps the first
  claimant's row and drops the duplicate, but while another host keeps listing the id, `resolve_owner` refuses every
  operation on the real owner's session with `SessionOwnerAmbiguous` naming both hosts (terminal, stop, restart, rename,
  delete, Replace, uploads, mark-read, detail). A hostile host can learn every fleet id through the agent `sessions`
  listing, so it can make all other hosts' sessions unreachable through Farhelm until it is removed or stops listing
  them. SPEC_impl.md's collision rule chose this fail-closed routing deliberately; SPEC.md's availability rule ("must
  not disrupt unrelated hosts") does not reconcile with it. Impact is bounded: the agents keep running, the error names
  the claimant, and the contest clears on the next refresh after the host is removed. A plain first-claim-wins
  alternative was considered and rejected: after a cache purge (remove and re-add, or adoption), a hostile host can win
  the race to first claim and then silently receive terminal input, uploads, and stops meant for the real owner.
- Decision: the user chose to accept the fail-closed behaviour in the specification. Principle: when a remote host
  claims session ids another host owns, the helm refuses to route operations on those sessions rather than risk
  misrouting them, naming both hosts; that loss of access is the accepted response to a misbehaving host, and removing
  that host is the remedy. This is an explicit carve-out from the availability rule, chosen because a silent misroute
  (input or destructive operations reaching the wrong machine) is worse than a named refusal.
- Completion criteria: SPEC.md's availability rule states the carve-out and its reason; SPEC_impl.md's session-id
  collision rule points at it rather than leaving the conflict unreconciled. No code change. Remove this feedback file
  and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; SPEC.md's availability section now names session ownership as a deliberate exception: the helm
  refuses to route a session two hosts claim, names both, and removing the misbehaving host is the remedy. SPEC_impl.md
  cross-references it. No code change. PR [#1140](https://github.com/scode/farhelm/pull/1140/changes), jj change
  `utynmzqq`, bookmark `pr/session-id-contest-spec`.

## remote-unit-overwritten-without-ownership-check.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `03a3051`. Every provisioning plan ends in `WriteUnit` to
  `farhelm-supervisor.service` (`provisioning/plan.rs`), and `install_source` (`provisioning/backend.rs`) compares only
  content hashes before an atomic rename over the destination; it never reads the existing unit's managed-by marker or
  `ExecStart=`. The local row has the equivalent refusal (`local_handoff_reason`); remote rows do not. Every remote
  UPDATE rewrites the unit (following the running supervisor's dial path, so a setup-managed host keeps a working unit
  but loses the marker, after which `farhelm helm setup` and `farhelm uninstall` on that machine refuse to manage it);
  ADD does so only when the probe finds no answering supervisor. A hand-written unit under that name loses its content.
  Bucket revised from `highest` to `high` with the user's agreement: the realistic case is a setup-managed remote host
  updated from another helm's panel, which keeps working but confuses later setup/uninstall there.
- Decision: the user agreed to the bounded fix. Principle: on a host provisioned from the hosts panel, an unmarked
  `farhelm-supervisor.service` belongs to provisioning and ADD/UPDATE may replace it; a unit carrying
  `farhelm helm setup`'s marker belongs to setup on that machine, and provisioning refuses to touch it, handing off the
  way the local row does. A hand-written unit under that exact name on a host the user asks Farhelm to provision is the
  user's to move aside first. The reviewer's full classification (a new provisioning marker plus migration for existing
  unmarked units) is deliberately not adopted.
- Completion criteria: specification states the ownership principle above; remote inspection reads the existing unit and
  ADD and UPDATE refuse with a clear hand-off message when it carries setup's marker, before any action runs; tests
  cover a setup-marked remote unit being refused on both ADD and UPDATE and an unmarked unit still being replaced.
  Remove this feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; the reach check reports the remote unit's first-line owner and turns a setup-marked unit into
  the Manual hand-off that Add and Update refuse; the remote unit write re-checks the marker in the same command (exit
  77), and an unreadable unit fails closed (exit 78). SPEC.md and SPEC_impl.md state the rule. Tests cover the reach
  parsing, the reach script against fixture units, and a write refused for a marked destination. PR
  [#1141](https://github.com/scode/farhelm/pull/1141/changes), jj change `ozlzuymu`, bookmark
  `pr/setup-managed-remote-unit`.

## provisioning-chmods-shared-directories.md

- Outcome: `fix spec+code`.
- Assessment: confirmed at `03a3051` by code inspection and a local reproduction of the command's behavior. Every plan's
  `EnsureDirectories` (`provisioning/plan.rs`) includes the systemd user-unit directory and, on UPDATE, the registered
  binary's parent directory at `0755`; the remote executor runs `install -d -m 755 -- <dir>`
  (`provisioning/backend.rs`), and GNU `install -d -m` chmods an existing directory (`700` became `755` locally). A
  private home, bin, or unit directory therefore becomes readable by other accounts on the host. This contradicts
  SPEC.md "Ownership during cleanup and provisioning" and SPEC_impl.md's provisioning rule, which already name these
  shared directories and note that existing provisioning paths still needed assessment against the policy.
- Decision: the user chose the code fix and wants the rule covered explicitly in the specification. Principle:
  provisioning creates and enforces modes only on directories dedicated to Farhelm (its private lib directory and the
  supervisor state directory); shared directories that merely hold its executable or unit files (the systemd user-unit
  directory, the binary's parent when it is not the lib directory) are created only when missing and never chmodded, and
  permissions that block installation are reported as an obstacle. The reviewer's follow-on about swapping the temporary
  in a group- or world-writable binary directory is excluded: an account that can write that directory can already
  replace the installed binary. The user expects the specification to say install directories writable by other local
  accounts are not designed for; the existing statements cover only the working-copy root (SPEC.md) and the state
  directory (SPEC_impl.md), so execution adds the equivalent statement for provisioning's install directories. The
  "refuse `$HOME`, `/`, `/tmp` as binary directory" suggestion is unnecessary once shared directories are never
  chmodded.
- Completion criteria: specification states the dedicated-versus-shared rule explicitly (resolving the "still require
  assessment" note) and that install directories writable by other local accounts are outside the design; remote and
  local `EnsureDirectories` create shared directories only when missing without changing an existing mode, while
  dedicated directories keep mode enforcement; the ADD confirmation wording no longer implies an existing shared
  directory's mode is set; tests cover an existing restrictive shared directory keeping its mode and a dedicated
  directory still being repaired. Remove this feedback file and its `review_feedback_queue/INDEX.md` entry in the
  execution change.
- Execution: `complete`; provisioning's directory plan marks the systemd user-unit directory and a binary directory
  outside the lib directory as shared: created 0755 when missing, otherwise left alone (over ssh and locally); the lib
  and state directories are still forced to their modes. SPEC.md declares install directories other local accounts can
  write to unsupported. PR [#1142](https://github.com/scode/farhelm/pull/1142/changes), jj change `lrzynkyu`, bookmark
  `pr/provisioning-keeps-shared-dir-modes`.

## sftp-misparses-ipv6-and-uri-destinations.md

- Outcome: `fix code`.
- Assessment: confirmed at `03a3051` with an offline reproduction (OpenSSH 9.6, `-F /dev/null`, a ProxyCommand that only
  prints its target). The payload upload (`sftp_put`, `provisioning/backend.rs`) runs `sftp -b -` with the same argv
  every other step passes to `ssh`, but sftp ends the host name at the first colon: `fe80::1` dials `fe80`,
  `alice@2001:db8::5` dials `0.0.7.209`, and `ssh://alice@build.example:2222` dials host `ssh` on port 22 as the local
  user, while `ssh` dials the registered host in each case; `::1` and plain host names agree. The registry accepts these
  destination forms. With `BatchMode=yes`, an unknown host key fails the connection, so a wrong-machine upload needs the
  truncated name to reach a host already trusted in `known_hosts` or matched by ssh config; the payload is the public
  release binary. The common effect is that provisioning a host registered by IPv6 literal or `ssh://` URI always fails
  at the upload.
- Decision: the user chose to remove sftp from provisioning entirely rather than translate or refuse destination forms,
  so the class of ssh/sftp parsing mismatches disappears. sftp's only use is uploading payloads to a remote temporary;
  every neighbouring step (progress via remote size, digest check, rename, temporary cleanup) already runs over ssh.
  Streaming the payload on stdin to `cat > <temporary>` over the same ssh argv loses no capability, needs only the `sh`
  and `cat` every other step already requires, and drops the remote sftp-subsystem requirement.
- Completion criteria: the payload upload streams over the same ssh command as every other provisioning step, keeping
  the remote-growth idle timeout and the existing digest check before the rename; no provisioning path invokes `sftp`;
  tests that script an `sftp` process are converted; SPEC_impl.md's provisioning description, `ssh.rs`'s module doc, and
  the CentOS provisioning script's comment no longer name sftp. Remove this feedback file and its
  `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; every provisioning upload (binary, static tmux, units) now streams over the ordinary ssh
  command into `cat > <temporary>`; sftp and its path encoder are gone, and SPEC_impl.md and the CentOS script no longer
  need an sftp subsystem. `scripts/test-provision-centos.sh`, required for this item, passed. PR
  [#1143](https://github.com/scode/farhelm/pull/1143/changes), jj change `lrvklypr`, bookmark
  `pr/provisioning-upload-over-ssh`.

## installer-overwrites-user-farhelm-file.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `03a3051`. `scripts/install.sh` refuses a destination only when it
  is not a regular file; the replace loop moves any existing `farhelm` (and `farhelm-desktop` on macOS) to `.<name>.old`
  and removes that backup after commit, without comparing the existing file to the executable-directory ownership record
  (`.farhelm-installation`, which holds the SHA-256 of each executable the installer last wrote). A user's own file with
  that name in the install directory is destroyed while the run reports an update. SPEC.md's uninstall rule already
  forbids deleting a foreign file merely because its name matches; the installation section is silent, so install and
  uninstall disagree. The record exists only since #673 (2026-09-16), so older installs have none.
- Decision: the user chose to extend the "not destroyed by name match" principle to installation. An existing
  destination whose SHA-256 matches the record is the installer's own and is replaced as today. One with no record or a
  mismatching digest (a foreign file, or a pre-record Farhelm install) is not refused and not deleted: it is moved aside
  under a kept, non-reserved name and the closing message says where it went. Updates therefore never fail on this, and
  the cost is one leftover file on the first update of a pre-record install.
- Completion criteria: SPEC.md's installation section states the principle; `docs/install_uninstall.md` describes the
  kept file and message; `install.sh` implements the record-digest comparison and keep-and-report path for each
  installed executable, preserving the journaled rollback guarantees; `scripts/test-install-sh.sh` covers a recorded
  update (no leftover), a foreign file (kept and reported), and a pre-record install (kept and reported). Remove this
  feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; install.sh replaces an existing `farhelm`/`farhelm-desktop` only when its checksum matches the
  ownership record; anything else is hard-linked to `NAME.replaced-<UTC stamp>` before the transaction and named in the
  closing message, and a failed link refuses. SPEC.md and docs/install_uninstall.md describe it; test-install-sh.sh
  cases K1-K5 cover it. PR [#1144](https://github.com/scode/farhelm/pull/1144/changes), jj change `mwuvtsso`, bookmark
  `pr/installer-keeps-foreign-farhelm`.

## installer-deletes-farhelm-app-on-grep.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `03a3051`. The installer's only ownership test for an existing
  `~/Applications/Farhelm.app` is that `Contents/Info.plist` exists and case-insensitively contains "farhelm"
  (`scripts/install.sh`, deliberately loose to accept an early hand-built trial bundle); it then `rm -rf`s the bundle.
  It never reads the bundle's own ownership record (`Contents/.farhelm-installation`, written since #673), so a
  user-built, re-signed, or customised Farhelm.app and anything inside it is deleted without a prompt, while an
  installer-built bundle whose `Info.plist` an interrupted uninstall already removed is refused despite a valid record.
  This contradicts SPEC.md's rule that a foreign file or bundle is not deleted merely because its name matches, which
  the `installer-overwrites-user-farhelm-file.md` decision extends to installation.
- Decision: the user chose the code fix and agreed foreign bundles are refused rather than moved aside (a renamed `.app`
  in `~/Applications` still appears as an app, and "rename your bundle and re-run" is the clearer message; the
  executables are already installed at that point). Replace the bundle when it carries a valid installer record for this
  installation. Bundles built between #310 (2026-09-01) and #673 (2026-09-16) have no record; recognise them narrowly by
  the exact `CFBundleIdentifier` `org.scode.farhelm.desktop` plus the installer's fixed layout and replace them too.
  Refuse anything else.
- Completion criteria: `install.sh` decides bundle ownership from the record (with the narrow pre-record legacy check),
  no longer from a substring match; a half-uninstalled bundle with a valid record is replaced; a foreign bundle,
  including one whose `Info.plist` mentions farhelm, is refused with the existing actionable message;
  `scripts/test-install-sh.sh` covers the recorded, legacy, half-removed, and foreign-mentioning-farhelm shapes. Remove
  this feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; the bundle's `.farhelm-installation` record (its tag plus this canonical install directory)
  decides replacement, even for a half-uninstalled bundle; a record-less bundle from an earlier release is accepted only
  by its exact file set and a single uncommented `org.scode.farhelm.desktop` identifier; anything else, including the
  old hand-built trial bundle, is refused. test-install-sh.sh covers each shape; docs/install_uninstall.md updated. PR
  [#1145](https://github.com/scode/farhelm/pull/1145/changes), jj change `onnluwpt`, bookmark
  `pr/installer-bundle-ownership`.

## installer-mirror-var-drops-https-no-signature.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `03a3051`. When `FARHELM_RELEASE_BASE_URL` is set, `install.sh`'s
  `curl_get` drops `--proto '=https' --proto-redir '=https'`, `validate_release_base_url` accepts `http://` for any
  host, archives are checked only against the same server's `SHA256SUMS`, and no minisign signature is checked. The
  script's comment calls the variable an undocumented test-only hook, but the same name is the helm's documented
  `--release-base-url` environment form (`crates/farhelm-helm/src/lib.rs`, SPEC_impl.md CLI section), where signature
  verification makes a mirror safe. An operator who exported it for the helm gets an unauthenticated installer download
  on the helm's machine. Exploitation additionally needs an `http://` mirror, a redirect to HTTP, or a compromised
  mirror with an attacker positioned to tamper.
- Decision: the user agreed to separate the names rather than add signature verification to the installer. The
  installer's fixture hook gets its own test-only name (for example `FARHELM_INSTALL_TEST_BASE_URL`), so the helm's
  variable no longer affects installation; in that test mode `http://` is accepted only for loopback hosts; the
  misleading comment is corrected. The user also asked that SPEC.md itself (not SPEC_impl.md) state that the installer
  intentionally trusts GitHub over TLS and the upstream repository, rather than verifying a release signature.
- Completion criteria: SPEC.md's installation section states the installer's trust basis (GitHub over TLS and the
  upstream repository); `install.sh` ignores `FARHELM_RELEASE_BASE_URL`, reads its fixture base URL only from the new
  test-only variable, and refuses non-loopback `http://` there; `scripts/test-install-sh.sh` uses the new name and
  covers a refused non-loopback `http://` value and an ignored `FARHELM_RELEASE_BASE_URL`. Remove this feedback file and
  its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; the installer's fixture hook is now `FARHELM_INSTALL_TEST_BASE_URL`, plain http is accepted
  only for a loopback host with curl held there by `--connect-to`, and `FARHELM_RELEASE_BASE_URL` no longer affects the
  installer. SPEC.md states that the installer trusts GitHub over TLS and the upstream repository. test-install-sh.sh
  cases M1-M3 and a remote-redirect fixture cover it. PR [#1146](https://github.com/scode/farhelm/pull/1146/changes), jj
  change `sspxrktu`, bookmark `pr/installer-ignores-helm-mirror`.

## release-gate-runs-unpinned-privileged-container-before-signed-build.md

- Outcome: `other`.
- Assessment: partly correct, by inspection at `03a3051`. Confirmed: `scripts/test-provision-centos.sh` builds from the
  mutable tag `quay.io/centos/centos:stream9` with no digest and runs it `--privileged --cgroupns=host` with
  `/sys/fs/cgroup` bound read-write; in the generated `release.yml` that step precedes `dist build` in the same Linux
  x86_64 job; `dist-workspace.toml`'s residuals header does not mention it. Overstated: it is not the only unpinned
  third-party executable input in the release path. The same job runs `sudo apt-get install` of Ubuntu packages, whose
  maintainer scripts run as root on the runner under Ubuntu's signing keys alone, and every GitHub action is referenced
  by a mutable tag rather than a commit SHA. A compromised CentOS image or dnf key chain is the same trust class the
  pipeline already accepts for those. The precondition is a compromise of that upstream trust root, not an ordinary
  attacker path.
- Decision: the user chose to record the accepted trust chain rather than pin this one input. Add an entry to
  `dist-workspace.toml`'s "Residuals in the GENERATED workflow" header (or the residuals documentation it belongs with)
  stating that the release build trusts distribution package signing and registry/action tags as trust roots — Ubuntu
  apt packages, the CentOS Stream image and its dnf keys, and GitHub action tags — and that the privileged CentOS
  container runs before `dist build` on the same runner. No image digest pin; a broader pin-everything effort was not
  requested.
- Completion criteria: the residuals documentation names these trust roots and the ordering, with why they are accepted;
  no workflow or script behavior changes. Remove this feedback file and its `review_feedback_queue/INDEX.md` entry in
  the execution change.
- Execution: `complete`; dist-workspace.toml's header comments gain "Accepted trust roots in the release build"
  (root-installed apt packages, the privileged CentOS container on a mutable tag, GitHub actions by version tag), beside
  the generated workflow's residuals. PR [#1147](https://github.com/scode/farhelm/pull/1147/changes), jj change
  `zztrkkpw`, bookmark `pr/release-trust-roots`.

## app-info-plist-uses-caller-umask.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `03a3051`. `scripts/install.sh` writes the bundle's
  `Contents/Info.plist` with a heredoc outside any umask subshell and never chmods it, so it takes `0666` minus the
  caller's umask, while every sibling bundle file and directory gets an explicit mode. Under a permissive umask such as
  `002`, another local account in the shared `staff` group could edit it and add `LSEnvironment` (for example
  `DYLD_INSERT_LIBRARIES`) so their code runs as the user at the next launch. The default macOS umask `022` is
  unaffected. The installer creates the writable file itself, so this is not covered by the "shared-writable locations
  are outside the design" stance.
- Decision: the user chose the code fix, with an inline comment explaining why the explicit mode matters (the plist is
  launch configuration that `LSEnvironment` can turn into code execution, and the caller's umask must not decide who can
  write it).
- Completion criteria: `Info.plist` gets an explicit `0644` (or is written under `umask 022`) before it is hashed, with
  that inline comment; `scripts/test-install-sh.sh` runs the macOS-shaped install under `umask 002` and checks the
  plist's mode. Remove this feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; install.sh sets Info.plist to 0644 after writing it, with an inline comment on why
  (LSEnvironment can inject DYLD variables for any staff-group account); test-install-sh.sh case P1 installs under umask
  002 and checks the mode. PR [#1148](https://github.com/scode/farhelm/pull/1148/changes), jj change `wpurklxr`,
  bookmark `pr/app-plist-mode`.

## setup-pins-relative-path-tmux.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `03a3051`. `candidates_on_path`
  (`crates/farhelm-supervisor/src/tmux.rs`) skips only the empty PATH entry, with a comment that the directory setup
  happened to run in is never defensible to pin; relative entries such as `.`, `./bin`, or `node_modules/.bin` mean the
  same and still pass. `choose_tmux` in `crates/farhelm/src/setup.rs` resolves each hit against the working directory
  and pins the first acceptable one into the supervisor unit as `FARHELM_TMUX`, so a `tmux` in an untrusted checkout
  becomes every session's tmux at every boot, and moving the checkout breaks the unit. Requires a relative PATH entry
  (uncommon) and running setup inside such a directory.
- Decision: the user chose the code fix. Skip every non-absolute PATH entry in setup's tmux search, generalising the
  empty-entry rule and its comment; explicitly named `--tmux`/`FARHELM_TMUX` values may still be relative and resolve
  against the working directory; a no-usable-tmux refusal mentions a skipped relative candidate. If other callers of
  `candidates_on_path` need plain `execvp` semantics, put the filter in setup's search instead of the shared helper.
- Completion criteria: setup never pins a tmux found through a relative PATH entry; a test with `.` (and a nested
  relative entry) on PATH ahead of a valid absolute tmux pins the absolute one, and one with only a relative candidate
  refuses with the skipped-candidate mention; other `candidates_on_path` callers keep their intended behavior. Remove
  this feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; setup's PATH discovery considers absolute entries only, and a PATH-discovery refusal names a
  tmux a relative entry would have found and suggests `--tmux`. Tests cover the skip and the note; they fail with the
  filter removed. PR [#1149](https://github.com/scode/farhelm/pull/1149/changes), jj change `wwmqxwvn`, bookmark
  `pr/setup-skips-relative-path-tmux`.

## install-lock-owner-unchecked.md

- Outcome: `fix spec`.
- Assessment: confirmed as described at `03a3051`, under an unusual precondition. `is_our_lock` in `scripts/install.sh`
  checks entry names, regular files, and that the lock directory is not group- or world-writable, but never the owner of
  the lock directory, `pid`, or `journal`; stale-lock recovery replays the journal, so in a group-writable sticky
  install directory owned by the victim, a co-user can plant a dead-pid lock with a `PARK cli` journal and their own
  `.farhelm.old`, and the victim's next update renames it over `farhelm`. #1116 changed only the pid staleness check.
  Without the sticky bit a co-user could replace `farhelm` directly, so the finding exists only for an install directory
  other local accounts can write to, which the installer never creates (it masks write bits on directories it creates)
  and which requires the user to point `FARHELM_INSTALL_DIR` at one.
- Decision: the user agreed to settle this in the specification rather than harden the lock. Principle: the standalone
  installer's install directory (`~/.local/bin` or `FARHELM_INSTALL_DIR`), like provisioning's install directories and
  the working-copy root, must not be writable by other local accounts; Farhelm's installation, update, recovery, and
  uninstall guarantees assume no other account can create or replace entries there, and keeping it so is the user's
  responsibility. This widens the statement agreed for `provisioning-chmods-shared-directories.md`; execution should
  place one coherent statement rather than two drifting ones. No code change.
- Completion criteria: SPEC.md states the principle for the standalone install directory (and
  `docs/install_uninstall.md` mentions it where custom install directories are described); no installer behavior change.
  Remove this feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; SPEC.md's statement about install directories other accounts can write to now covers the
  standalone installer's directory (`~/.local/bin` or `FARHELM_INSTALL_DIR`); docs/install_uninstall.md warns the same.
  No code change. PR [#1150](https://github.com/scode/farhelm/pull/1150/changes), jj change `lwusvtpq`, bookmark
  `pr/install-dir-writers-unsupported`.

## delete-ended-session-kills-live-tabs.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `03a3051`. `ListView`'s `on_delete`
  (`crates/farhelm-ui/src/list/view.rs`) deletes immediately whenever `target.status.has_ended()`; `DeleteTarget`
  (`list/shared.rs`) carries only id and status, built in `list/row.rs` from a `Session` whose `tabs` are available but
  omitted. The branch's comment justifies skipping confirmation only by the agent's invisible leftover descendants, not
  by tabs, which the UI does know about. `status::confirm_consequence` never mentions tabs. A stopped or exited agent
  with a dev server or build still running in a tab loses it to one unconfirmed click, contradicting SPEC.md's Delete
  rule (confirmation "that says so when anything is still alive") and single-tab close's own confirmation.
- Decision: the user agreed to the fix as recommended: carry the session's tab count (or a has-tabs flag) into
  `DeleteTarget`; confirm whenever the agent has not ended or any tab is listed; make the delete and Replace
  confirmation wording say the running tabs will be killed.
- Completion criteria: an ended session with listed tabs opens the delete confirmation instead of deleting; an ended
  session with no tabs still deletes immediately; delete and Replace consequences mention tabs when there are any;
  row-level tests cover the exited-with-tabs and exited-without-tabs cases. Remove this feedback file and its
  `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; Delete asks first when the agent is live or any tab is open; the delete and Replace
  consequences name how many tabs will close, and the ended-agent wording no longer claims nothing is left to kill. UI
  tests cover the confirmation rule and the wording. PR [#1151](https://github.com/scode/farhelm/pull/1151/changes), jj
  change `qqkkplmz`, bookmark `pr/delete-confirms-open-tabs`.

## delete-lacks-liveness-precondition.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `03a3051`. Whether a delete is confirmed is decided only in the
  browser from the row's last-rendered status; `DELETE /api/sessions/{id}` is forwarded as
  `ControlMsg::DeleteSession { req_id, session_id }` (`crates/farhelm-proto/src/lib.rs`), which carries no precondition,
  and the supervisor tears down whatever is live. A row can show exited or interrupted while the agent runs again
  (restarted from another client or by an agent, the helm's cached interrupted status right after a relaunch, a stale
  listing), so an unconfirmed Delete, and Replace's source delete, can kill a live agent. Restart already closes the
  same race with `stop_if_running` rechecked by the supervisor at handling time. The feedback's header-restart listing
  trigger is its own queue item (`header-actions-skip-listing-read.md`) and is not part of this decision.
- Decision: the user asked whether the fix would sprawl; assessed as contained (low to medium) because it mirrors
  restart's consent pattern at each layer, and the user chose `fix code` with that bounded scope. Add a defaulted
  precondition field (for example `only_if_nothing_alive`) to `DeleteSession`, safe across mixed versions since the
  protocol ignores unknown fields and the default is today's unconditional delete. The supervisor checks it inside the
  existing lifecycle claim using restart's existing liveness notion (live pane, or an unconfirmed launch with no known
  terminal) plus any open tab window, and refuses with a Conflict. The helm's REST delete passes it through; the UI's
  unconfirmed delete path and Replace's unconfirmed source delete send it. A refusal is shown as the ordinary delete
  error and the row refreshes; the UI does not auto-open the confirmation. Implement together with, or after,
  `delete-ended-session-kills-live-tabs.md`, which edits the same `DeleteTarget`/`on_delete` code.
- Completion criteria: supervisor tests cover flag set with a live agent (refused), flag set with an ended agent and an
  open tab (refused), flag set with nothing alive (deleted), and flag unset with a live agent (deleted, as today); a
  helm REST test covers pass-through; a UI test covers the unconfirmed path sending the flag and a refusal surfacing as
  an error. Remove this feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; an unconfirmed delete sends `only_if_nothing_alive`; the supervisor checks the agent and every
  tab (reading window markers unconditionally) under the lifecycle claim and refuses with Conflict; the helm forwards
  the flag, and Replace sets it when its confirmation showed nothing alive. A failed sidebar delete refreshes the
  listing. Tests: supervisor e2e refusal while alive, the lone-tab-after-agent-window case, helm forwarding, and the
  UI's delete URL. PR [#1152](https://github.com/scode/farhelm/pull/1152/changes), jj change `vnrtroqw`, bookmark
  `pr/delete-rechecks-liveness`. Follow-up: the UI test the completion criteria require was missing when #1152 landed.
  `e2e/tests/delete-precondition.spec.ts` adds it: an unconfirmed Delete on an ended, tabless row sends
  `only_if_nothing_alive=true`, and an injected 409 shows as the row's delete error, keeps the row, opens no
  confirmation and re-reads the listing; it passed on Chromium and WebKit. PR
  [#1160](https://github.com/scode/farhelm/pull/1160/changes), jj change `qpszsumk`, bookmark
  `pr/delete-precondition-browser-test`.

## header-replace-confirm-ignores-cancel.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `03a3051`. The session header's Replace "replace" handler
  (`crates/farhelm-ui/src/session_view.rs`) clears its flag and calls `header_confirm_replace()` without checking the
  prompt is still open, so a cancel-then-confirm burst replaces (deletes) the session after the user cancelled, runs
  without the lifecycle lock cancel released, and releases whatever lock another operation claimed meanwhile. Every
  other inline confirmation hand-rolls that guard. An independent gpt-6-astra high review (read-only, 2026-09-28) also
  identified the mirror race on the lock-claiming prompts: their cancel handlers release unconditionally, so a queued
  confirm-then-cancel can unlock an in-flight restart or replace; the one-line guard alone does not close that.
- Decision: the user asked whether a shared solution could stop this bug class recurring, and made `fix code`
  conditional on an Astra high reviewer agreeing; the review returned "agree with changes", and its changes are adopted
  as the scope. Introduce a shared confirmation primitive beside `OpLock` in `ops.rs`, following its rationale (enforce
  the handler-time invariant once): a payload-bearing slot such as `ConfirmSlot<K, P = ()>` whose
  `take(&K) ->
  Option<P>` is the only way a confirm handler proceeds, with ordinary prompts carrying `()` and
  lifecycle prompts carrying an owned guard. Add `PaneGate::claim_guard()` (preserving its sidebar-operation check);
  lifecycle prompts store that guard while open and transfer it into the task on confirm, never release-and-reacquire;
  cancel drops only ownership still in the slot and is a no-op after `take`; remove the corresponding manual releases.
  Specify failed or mismatched `take` leaves the prompt untouched, explicit `open` replacement/refusal semantics,
  `cancel_for(&key)` for event handlers, and distinct slots or keys for header Replace and interrupted Replace
  (SPEC_impl.md's distinction). Use an opening generation if "that prompt" must reject clicks across cancel-and-reopen
  of the same target. Migrate header Replace, header restart (non-mechanical: its closure is shared with Restart With,
  which deliberately retains the claim after failure; preserve that), interrupted Replace, tab close (needs a tracked
  current-key accessor), and host remove (consumption only; it claims the op lock afterwards through `run`, do not lock
  on open). Defer profile delete (it keeps the prompt mounted during deletion and reconciles focus and notices). Leave
  sidebar delete and row Replace on their `RowPhase` machine.
- Completion criteria: the primitive exists with docs explaining the race it closes; the migrated prompts use it and no
  longer hand-roll the guard or release a claim they no longer own; header Replace ignores a confirm queued after
  cancel; lifecycle prompts ignore a cancel queued after confirm. Centralized tests cover cancel then confirm, confirm
  then cancel, duplicate confirm, wrong key, refused claim, and protection of a subsequent owner, plus a small headless
  `VirtualDom` test for prompt teardown and task cancellation and one header-wiring regression. Remove this feedback
  file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; a shared `ConfirmSlot` helper now backs the header Replace, the interrupted-session card's
  Replace, tab close and host removal: confirm acts only on a still-open prompt, and the Replace prompts' lock guard
  moves into the task. Fallback taken as agreed: the header Restart prompt stays off the helper (it shares state with
  Restart With) and its cancel releases the lock only while the prompt is open. Tests cover the confirm-after-cancel
  race, a later lock owner, unmount, and a confirmed task being cancelled. PR
  [#1153](https://github.com/scode/farhelm/pull/1153/changes), jj change `ypqunpzo`, bookmark
  `pr/confirm-prompts-cancel-race`.

## row-menu-drifts-after-own-delete.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `03a3051` (still present after #1123 touched the same path). The
  sidebar row menu's panel is fixed-positioned at coordinates measured when it opens, so the list closes it whenever its
  row may have moved; `commit_listing` does so via `rows::menu_row_reordered`. `do_delete`'s success path removes the
  deleted row directly from `listing` (`crates/farhelm-ui/src/list/view.rs`, `current.sessions.retain(...)`) without
  that check, and the next listing is compared against the already-pruned list. A menu opened on a row below the deleted
  one therefore stays open next to a different row; its actions still target the original row, and an ended session with
  no tabs deletes without a prompt naming it.
- Decision: the user chose the code fix with a test: run the same reorder check around the optimistic removal (compare
  the listing before and after `retain`) and close the menu when the open row's position changed.
- Completion criteria: the optimistic delete closes an open row menu whose row moved; a menu on a row above the deleted
  one stays open; a unit test covers deleting a row above the open menu (closed) and below it (still open). Remove this
  feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; the optimistic delete reports whether the open menu's row moved or was removed, and the list
  closes the menu then. A unit test covers a removal above, at and below the open row. PR
  [#1154](https://github.com/scode/farhelm/pull/1154/changes), jj change `kmoyypml`, bookmark
  `pr/row-menu-closes-after-delete`.

## session-header-raw-peer-text.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `03a3051`; #1126 escaped neighbouring session-view text but not
  these. The session header (`crates/farhelm-ui/src/session_view.rs`) interpolates the session title, working directory,
  and invocation, and their `title` tooltips, raw, without `display_peer` or direction isolation, and the two copy
  buttons copy the raw value. These values come from supervisors and agents; titles only lose control characters and
  cwd/invocation only get a length cap, so bidi overrides, isolates, zero-width characters, and (for cwd and invocation)
  newlines reach the header. The narrow command button leaves the tooltip as the only full view, so it can read
  differently from the bytes copied. Harm requires the user to paste and run the copied value.
- Decision: the user chose the recommended fix: render the title, directory, and invocation and their tooltips through
  `display_peer` inside direction-isolated `.peer-value` elements, following #1126; keep copying the raw bytes; when a
  copied value contains characters `display_peer` escapes, the copy feedback says it contains hidden characters (shown
  as `<U+…>`) rather than refusing the copy, since an invocation may legitimately contain a newline.
- Completion criteria: none of the three values or their tooltips renders raw; copy still yields the exact raw value;
  the warning appears only for values with escaped characters; a test covers a bidi override and a newline in the
  invocation (escaped display, raw copy, warning shown). Remove this feedback file and its
  `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; the header's title, folder and command render through `display_peer` in a direction-isolated
  `.peer-value`; copies stay raw, and copying a value with presentation-unsafe characters shows a warning anchored below
  the titlebar. The new Playwright case in e2e/tests/header.spec.ts passed on Chromium and WebKit. PR
  [#1155](https://github.com/scode/farhelm/pull/1155/changes), jj change `pporvtoz`, bookmark
  `pr/session-header-escapes-peer-text`.

## titles-raw-in-confirm-prompts.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at `03a3051`. In `crates/farhelm-ui/src/list/row.rs` the visible row
  title and the `confirm-title` quotes in the delete and replace confirmations render `session.title` raw without
  direction isolation; only their tooltips go through `display_peer`. Titles lose only control characters at the
  supervisor, so an agent rename can make two titles render identically or place an override inside the quoted title,
  spoofing the last confirmation before an irreversible delete or replace. Screen-reader labels (`clamp_title`) also use
  the raw title but are not a visual-spoofing surface.
- Decision: the user chose the code fix: render the row title and both confirmation quotes through `display_peer` inside
  `.peer-value` isolation; accessibility labels are out of scope.
- Completion criteria: no visible sidebar title or confirmation quote renders a raw title; a test with a zero-width
  character and a direction override in a title shows the escaped, isolated form in the row and both prompts. Remove
  this feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; the row title and both confirmation quotes render through a `PeerTitle` component
  (`display_peer` inside a `dir="ltr"` `.peer-value`). A VirtualDom test checks the emitted text and attributes and
  fails with the escaping or isolation removed. PR [#1156](https://github.com/scode/farhelm/pull/1156/changes), jj
  change `mtyztyos`, bookmark `pr/sidebar-titles-escaped`.

## display-peer-misses-invisible-chars.md

- Outcome: `fix code`.
- Assessment: partly addressed since the reviewed commit, remainder confirmed at `03a3051`. `display_peer` now uses the
  shared known-bad list `farhelm_proto::text::is_presentation_unsafe`, so the UI and the helm's log escaper agree and
  U+2061–U+2064 are covered. Still unescaped: U+034F, the Hangul fillers U+115F, U+1160, U+3164, U+FFA0, U+17B4–U+17B5,
  U+206A–U+206F, U+FFF9–U+FFFB, the tag block U+E0000–U+E007F, and variation selectors. `display_peer`'s visibility
  fallback counts U+3164 as visible, so a value of only that character renders blank. A hostile host can therefore
  report an identity differing from the recorded one only by such characters, and the adopt prompt shows both
  identically. Host identities are supervisor-generated UUIDs. Escaping variation selectors (and, less often, tags)
  everywhere would mangle ordinary emoji in session titles.
- Decision: the user chose the two-part code fix. Add the unambiguous invisibles (U+034F, the Hangul fillers,
  U+17B4–U+17B5, U+206A–U+206F, U+FFF9–U+FFFB, and the tag block) to the shared list, which also fixes the U+3164 blank
  fallback; leave variation selectors out for the emoji reason. Separately, render host identities in the adopt prompt
  with every non-ASCII character escaped, since they are UUIDs, which covers variation selectors and any future
  character the known-bad list misses exactly where the spoofing matters.
- Completion criteria: the shared list escapes each added range, pinned in its tests; a U+3164-only value no longer
  renders blank; the adopt prompt escapes all non-ASCII in recorded and reported identities, with a test where the two
  differ only by a variation selector and render distinguishably; session titles with emoji variation selectors still
  render unescaped. Remove this feedback file and its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; `is_presentation_unsafe` gains U+034F, the Hangul fillers, U+17B4-U+17B5, U+206A-U+206F,
  U+FFF9-U+FFFB and the tag block (variation selectors deliberately excluded), and host identities render through
  `display_identity`, which escapes all non-ASCII. Proto and UI tests cover both. PR
  [#1157](https://github.com/scode/farhelm/pull/1157/changes), jj change `snqktrlz`, bookmark
  `pr/more-invisible-characters-escaped`.

## osc8-link-target-never-shown.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at `03a3051`. The terminal's xterm `linkHandler`
  (`crates/farhelm-ui/assets/terminal.js`) defines only `activate`, which opens the OSC 8 target immediately through
  `farhelmTerminalLinks.openTerminalUrl` (a new tab on the web, the system browser on desktop); there are no
  `hover`/`leave` callbacks, so a program can underline trustworthy-looking text while linking elsewhere and the target
  is never shown. xterm restricts OSC 8 targets to http(s); plain-text WebLinks are unaffected. SPEC treats terminal
  output as untrusted and has no link-display contract. Harm needs a click and trust in the resulting page.
- Decision: the user agreed to the light fix and explicitly forbids any confirmation dialog or prompt on link
  activation, including a label-host versus target-host confirmation. Principle for SPEC.md's terminal section: terminal
  hyperlinks open their http(s) target on click without a prompt, and the exact target must be visible on hover before
  any click. Code: add `hover`/`leave` handlers that show the target in a small tooltip, set via `textContent`, with the
  host emphasised.
- Completion criteria: SPEC.md states the principle, including that activation is not gated by a confirmation; hovering
  an OSC 8 link shows its exact target and leaving hides it; clicking still opens directly with no `confirm()` or other
  prompt; a test covers the hover display for a link whose label differs from its target. Remove this feedback file and
  its `review_feedback_queue/INDEX.md` entry in the execution change.
- Execution: `complete`; SPEC.md states the rule; hovering an OSC 8 link shows its host and full target inside the
  terminal element (middle-shortened past 300 characters), and click still opens directly. JS unit tests and the
  terminal-links browser spec (Chromium and WebKit) cover it. PR
  [#1158](https://github.com/scode/farhelm/pull/1158/changes), jj change `ytxkpzkk`, bookmark
  `pr/terminal-link-target-on-hover`.

## token-prompt-invites-password-manager.md

- Outcome: `fix spec`.
- Assessment: code confirmed, browser behavior plausible but untested, at `03a3051`. The browser token prompt
  (`crates/farhelm-ui/src/auth.rs`) is `type="password"` with `autocomplete="off"` in a form removed after a successful
  exchange; the reviewer reports Chromium and Firefox ignore `autocomplete="off"` on password fields and offer to save
  such a form's value, which may sync the master web token off the machine. The lookalike-autofill half falls under the
  local port-squatter gap SPEC.md's security section and `docs/security.md` already accept for v1. The save-and-sync
  half requires the user to click Save; the token already travels by design (pasted into a browser, possibly on another
  machine over an SSH forward), and keeping tokens in a password manager is a common deliberate choice. The suggested
  code change relies on undocumented, per-browser autofill heuristics.
- Decision: the user chose to state the principle rather than change the prompt: whether the web token is stored in the
  user's password manager is the user's choice, and Farhelm does not try to prevent browsers from offering to save it;
  autofill into a lookalike prompt is part of the already-accepted local port-squatter gap. No code change.
- Completion criteria: SPEC.md's security section (and `docs/security.md` where it discusses the token prompt) states
  this principle; no UI change. Remove this feedback file and its `review_feedback_queue/INDEX.md` entry in the
  execution change.
- Execution: `complete`; SPEC.md's web UI trust section and docs/security.md say storing the web token in a password
  manager is the user's choice, and tie lookalike-prompt autofill to the already accepted port-squatter gap. No code
  change. PR [#1159](https://github.com/scode/farhelm/pull/1159/changes), jj change `xvnvrxrv`, bookmark
  `pr/web-token-password-manager`.

## refresh-publish-races-retarget-in-check-then-act-gap.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics". The trigger is rare and the whole
  consequence is a brief or view-local display glitch, a safe failure that succeeds on retry, or an imprecise or lost
  diagnostic.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## seed-write-validates-handle-outside-publish.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics". The trigger is rare and the whole
  consequence is a brief or view-local display glitch, a safe failure that succeeds on retry, or an imprecise or lost
  diagnostic.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## cancelled-refresh-overwrites-seed.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics". The trigger is rare and the whole
  consequence is a brief or view-local display glitch, a safe failure that succeeds on retry, or an imprecise or lost
  diagnostic.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## identityless-refresh-publishes-outside-lock.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics". The trigger is rare and the whole
  consequence is a brief or view-local display glitch, a safe failure that succeeds on retry, or an imprecise or lost
  diagnostic.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## tab-close-kills-before-detach.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics". The trigger is rare and the whole
  consequence is a brief or view-local display glitch, a safe failure that succeeds on retry, or an imprecise or lost
  diagnostic.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## tab-close-errors-linger.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics". The trigger is rare and the whole
  consequence is a brief or view-local display glitch, a safe failure that succeeds on retry, or an imprecise or lost
  diagnostic.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## token-show-busy-lock-misleading-failure.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics". The trigger is rare and the whole
  consequence is a brief or view-local display glitch, a safe failure that succeeds on retry, or an imprecise or lost
  diagnostic.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## credential-refusal-surfaces-as-broken-pipe.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics". The trigger is rare and the whole
  consequence is a brief or view-local display glitch, a safe failure that succeeds on retry, or an imprecise or lost
  diagnostic.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## spawn-lookup-timeout-message-misleads.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics". The trigger is rare and the whole
  consequence is a brief or view-local display glitch, a safe failure that succeeds on retry, or an imprecise or lost
  diagnostic.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## hook-log-truncation-erases-concurrent-line.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics". The trigger is rare and the whole
  consequence is a brief or view-local display glitch, a safe failure that succeeds on retry, or an imprecise or lost
  diagnostic.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## terminal-query-refused-before-upgrade.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics". The trigger is rare and the whole
  consequence is a brief or view-local display glitch, a safe failure that succeeds on retry, or an imprecise or lost
  diagnostic.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## sha256sums-part-repair-race.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics". The trigger is rare and the whole
  consequence is a brief or view-local display glitch, a safe failure that succeeds on retry, or an imprecise or lost
  diagnostic.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## tmux-kill-runs-unbounded-under-global-lock.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "A hung private tmux server or systemd user manager". The trigger is tmux or the systemd user manager
  not answering (or a tmux program that hangs), and the consequence stays on the affected host.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## untracked-mutations-leak-on-wedged-tmux.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "A hung private tmux server or systemd user manager". The trigger is tmux or the systemd user manager
  not answering (or a tmux program that hangs), and the consequence stays on the affected host.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## tmux-run-bytes-unbounded-under-lock.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "A hung private tmux server or systemd user manager". The trigger is tmux or the systemd user manager
  not answering (or a tmux program that hangs), and the consequence stays on the affected host.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## superseded-reap-watchers-never-exit.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "A hung private tmux server or systemd user manager". The trigger is tmux or the systemd user manager
  not answering (or a tmux program that hangs), and the consequence stays on the affected host.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## startup-tmux-version-check-unbounded.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "A hung private tmux server or systemd user manager". The trigger is tmux or the systemd user manager
  not answering (or a tmux program that hangs), and the consequence stays on the affected host.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## delete-serial-scope-kills-under-global-lock.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "A hung private tmux server or systemd user manager". The trigger is tmux or the systemd user manager
  not answering (or a tmux program that hangs), and the consequence stays on the affected host.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## staging-holds-claim-across-unbounded-io.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Healthy local filesystems". The trigger is a hung or failing filesystem, and
  the consequence stays on the affected machine.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## refresh-timeout-misses-profile-and-commit-tail.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Healthy local filesystems". The trigger is a hung or failing filesystem, and
  the consequence stays on the affected machine.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## ticker-retries-unkillable-tab-reaps.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Healthy local filesystems". The trigger is a hung or failing filesystem, and
  the consequence stays on the affected machine.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## failed-stop-leaves-stale-stop-intent.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Session status and history after a crash or a partly failed operation". The consequence is an
  inaccurate or imprecise session status or history after a crash, degraded recording, or a partly failed operation,
  recoverable through ordinary Stop, Restart or Delete.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## stop-terminal-less-records-exit-before-kill.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Session status and history after a crash or a partly failed operation". The consequence is an
  inaccurate or imprecise session status or history after a crash, degraded recording, or a partly failed operation,
  recoverable through ordinary Stop, Restart or Delete.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## stop-outcomes-lost-when-degraded.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Session status and history after a crash or a partly failed operation". The consequence is an
  inaccurate or imprecise session status or history after a crash, degraded recording, or a partly failed operation,
  recoverable through ordinary Stop, Restart or Delete.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## reused-pane-dead-treated-definitive.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Session status and history after a crash or a partly failed operation". The consequence is an
  inaccurate or imprecise session status or history after a crash, degraded recording, or a partly failed operation,
  recoverable through ordinary Stop, Restart or Delete.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## restart-refused-after-stopping-agent.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Session status and history after a crash or a partly failed operation". The consequence is an
  inaccurate or imprecise session status or history after a crash, degraded recording, or a partly failed operation,
  recoverable through ordinary Stop, Restart or Delete.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## crash-mid-sweep-leaves-frozen-tree.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Session status and history after a crash or a partly failed operation". The consequence is an
  inaccurate or imprecise session status or history after a crash, degraded recording, or a partly failed operation,
  recoverable through ordinary Stop, Restart or Delete.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.
- Note (2026-09-29, PR0 review): the finding's consequence also includes the agent or tab processes staying
  SIGSTOP-frozen until the user stops or deletes again. The user decided to filter this item with the other crash-status
  findings; the FILTER.md filter's wording was widened in the same change to state that case explicitly.

## reload-false-never-started-error.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Session status and history after a crash or a partly failed operation". The consequence is an
  inaccurate or imprecise session status or history after a crash, degraded recording, or a partly failed operation,
  recoverable through ordinary Stop, Restart or Delete.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## grok-double-null-field-refused.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare edge cases in harnesses without first-class support". It concerns Grok only, depends on
  unconfirmed vendor behavior, and affects only Resume capture or hook diagnostics for those sessions.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## grok-prompt-hooks-exceed-payload-cap.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  FILTER.md filter "Rare edge cases in harnesses without first-class support". It concerns Grok only, depends on
  unconfirmed vendor behavior, and affects only Resume capture or hook diagnostics for those sessions.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## spawn-holds-global-mutex-waiting-parent.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Waiting between operations on one host". Only session-management operations or
  host registration edits wait on one another, which that decision accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## delete-holds-directory-lock-whole-teardown.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Waiting between operations on one host". Only session-management operations or
  host registration edits wait on one another, which that decision accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## alias-edit-waits-on-provisioning.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Waiting between operations on one host". Only session-management operations or
  host registration edits wait on one another, which that decision accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## agent-created-tmux-windows-never-reaped.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md "Ownership during cleanup and provisioning" (the private tmux server is an implementation detail; direct
  interaction with it is unsupported). The trigger is a user or a program in a session using the private tmux server
  directly.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## split-tab-close-reaps-one-pane.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md "Ownership during cleanup and provisioning" (the private tmux server is an implementation detail; direct
  interaction with it is unsupported). The trigger is a user or a program in a session using the private tmux server
  directly.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## startup-reap-misnames-terminal-clients.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md "Ownership during cleanup and provisioning" (the private tmux server is an implementation detail; direct
  interaction with it is unsupported). The trigger is a user or a program in a session using the private tmux server
  directly.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## pane-states-skips-markers-single-window.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md "Ownership during cleanup and provisioning" (the private tmux server is an implementation detail; direct
  interaction with it is unsupported). The trigger is a user or a program in a session using the private tmux server
  directly.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## title-rewrite-erases-sweep-marker.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md "Lifecycle operations" (accepted cleanup limits on hosts without a usable systemd user manager). The survivors
  described are detached processes whose environment marker cannot be read or has been overwritten, which that section
  names as accepted limits.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## create-retry-duplicates-agent-without-manager.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md "Lifecycle operations" (accepted cleanup limits on hosts without a usable systemd user manager). The survivors
  described are detached processes whose environment marker cannot be read or has been overwritten, which that section
  names as accepted limits.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## probe-misses-install-sh-supervisor.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported host setup". The trigger is a host set up some other way than the
  helm's own setup (custom paths, install.sh, drop-ins, symlinked destinations) or setup re-applying the settings it
  manages, which that decision makes best effort or explicitly accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## update-breaks-install-sh-receipt.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported host setup". The trigger is a host set up some other way than the
  helm's own setup (custom paths, install.sh, drop-ins, symlinked destinations) or setup re-applying the settings it
  manages, which that decision makes best effort or explicitly accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## provisioning-ignores-unit-drop-ins.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported host setup". The trigger is a host set up some other way than the
  helm's own setup (custom paths, install.sh, drop-ins, symlinked destinations) or setup re-applying the settings it
  manages, which that decision makes best effort or explicitly accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## update-reenables-unit-and-linger.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported host setup". The trigger is a host set up some other way than the
  helm's own setup (custom paths, install.sh, drop-ins, symlinked destinations) or setup re-applying the settings it
  manages, which that decision makes best effort or explicitly accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## remote-mode-check-reads-symlink-mode.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported host setup". The trigger is a host set up some other way than the
  helm's own setup (custom paths, install.sh, drop-ins, symlinked destinations) or setup re-applying the settings it
  manages, which that decision makes best effort or explicitly accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## update-installs-tmux-into-shared-bin.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported host setup". The trigger is a host set up some other way than the
  helm's own setup (custom paths, install.sh, drop-ins, symlinked destinations) or setup re-applying the settings it
  manages, which that decision makes best effort or explicitly accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## add-ignores-requested-and-recorded-paths.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported host setup". The trigger is a host set up some other way than the
  helm's own setup (custom paths, install.sh, drop-ins, symlinked destinations) or setup re-applying the settings it
  manages, which that decision makes best effort or explicitly accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## tilde-remote-paths-never-expand.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported host setup". The trigger is a host set up some other way than the
  helm's own setup (custom paths, install.sh, drop-ins, symlinked destinations) or setup re-applying the settings it
  manages, which that decision makes best effort or explicitly accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## csh-login-shell-breaks-agent-launch.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported user environments". The trigger is an unsupported login shell or tmux
  program, or the deliberate PATH ordering that SPEC.md's environment contract now names.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## csh-login-shell-breaks-tab-launch.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported user environments". The trigger is an unsupported login shell or tmux
  program, or the deliberate PATH ordering that SPEC.md's environment contract now names.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## output-client-name-wrong-behind-wrapper.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported user environments". The trigger is an unsupported login shell or tmux
  program, or the deliberate PATH ordering that SPEC.md's environment contract now names.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## sink-client-name-wrong-behind-wrapper.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported user environments". The trigger is an unsupported login shell or tmux
  program, or the deliberate PATH ordering that SPEC.md's environment contract now names.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## launch-path-prepends-binary-directory.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Supported user environments". The trigger is an unsupported login shell or tmux
  program, or the deliberate PATH ordering that SPEC.md's environment contract now names.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## spawn-reply-returns-raw-profile-invocation.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md "Local authority and trust between hosts" (command lines and profiles are not secret from agents or attached
  hosts under the temporary cross-host exception). The exposure is an agent or attached host obtaining a profile or
  session command line, which that exception accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## clone-discloses-source-invocation.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md "Local authority and trust between hosts" (command lines and profiles are not secret from agents or attached
  hosts under the temporary cross-host exception). The exposure is an agent or attached host obtaining a profile or
  session command line, which that exception accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## replace-leaks-claimed-profile-invocation.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md "Local authority and trust between hosts" (command lines and profiles are not secret from agents or attached
  hosts under the temporary cross-host exception). The exposure is an agent or attached host obtaining a profile or
  session command line, which that exception accepts.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## same-site-navigation-passes-origin-guard.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Client hardening" and "Local authority and trust between hosts". The finding is
  defense in depth for the browser UI against a hypothetical flaw, or against actors the threat model excludes
  (same-account processes, other local accounts), not a concrete practical attack.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## csp-limits-only-framing.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Client hardening" and "Local authority and trust between hosts". The finding is
  defense in depth for the browser UI against a hypothetical flaw, or against actors the threat model excludes
  (same-account processes, other local accounts), not a concrete practical attack.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## unauthenticated-bearer-contends-sqlite.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Client hardening" and "Local authority and trust between hosts". The finding is
  defense in depth for the browser UI against a hypothetical flaw, or against actors the threat model excludes
  (same-account processes, other local accounts), not a concrete practical attack.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## e2e-backend-gate-lexical-prefix.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC.md maintainer-confirmed decision "Client hardening" and "Local authority and trust between hosts". The finding is
  defense in depth for the browser UI against a hypothetical flaw, or against actors the threat model excludes
  (same-account processes, other local accounts), not a concrete practical attack.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## same-version-cache-generations-never-pruned.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC_impl.md "Leftover files". The leftover files grow only with a rare explicit action or are removed by the next
  attempt or the next process start.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## failed-download-leaves-part-file.md

- Outcome: `other`.
- Assessment: not re-verified for correctness; the finding's trigger and consequence, as described, fall entirely under
  SPEC_impl.md "Leftover files". The leftover files grow only with a rare explicit action or are removed by the next
  attempt or the next process start.
- Decision: skipped under the triage rule for findings covered by the specifications or by a review filter. The
  underlying principle was decided with the user in the 2026-09-28 triage of product questions; this is accepted or
  filtered behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage, in the same change that records the
  principle.

## clone-source-missing-from-truncated-list.md

- Outcome: `other`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction. `clone_for_agent` still ignores the listing's `truncated` flag, and a session
  past the cap usually fails earlier at owner resolution with "no such session". The premise is a host holding more
  sessions than the listing cap.
- Decision: skipped under the triage rule for findings covered by the specifications, a review filter, or a Planned TODO
  item. Basis: SPEC.md's Session list section places a fleet that outgrows the cap outside what the product is built
  for, with the listing's incomplete-read notice as the whole answer; `sessions.rs` records the same acceptance. This is
  accepted, filtered or planned behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage on 2026-09-28.

## creates-accepted-while-boot-id-unreadable.md

- Outcome: `other`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction. Creates are still accepted while the boot id is unreadable, and the next normal
  start marks those rows interrupted. The finding overstates the effect: a live pane still lists as running, and Restart
  still asks for consent before stopping it. The lasting effect is a wrong recorded ending after the agent exits.
- Decision: skipped under the triage rule for findings covered by the specifications, a review filter, or a Planned TODO
  item. Basis: FILTER.md filter "Session status and history after a crash or a partly failed operation": the consequence
  is an inaccurate recorded outcome after degraded recording, recoverable through ordinary operations. This is accepted,
  filtered or planned behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage on 2026-09-28.

## create-replay-codex-offer-conflict.md

- Outcome: `other`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction. A create replay racing a conversation report still returns a Codex-worded
  Conflict, Grok included. The web form keeps the intent key for this refusal, so resubmitting the unchanged form
  replays the same key; a duplicate needs the user to edit the form.
- Decision: skipped under the triage rule for findings covered by the specifications, a review filter, or a Planned TODO
  item. Basis: FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics": a rare race whose
  consequence is a misleading refusal that succeeds on retry. This is accepted, filtered or planned behavior, not a
  completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage on 2026-09-28.

## revocation-during-admission-orphans-attachment.md

- Outcome: `other`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction. The helm still drops an in-flight attach after the teardown grace without
  detaching. The finding's "pinned until something shakes it loose" is wrong: the supervisor always follows `Attached`
  with replay data and `ReplayComplete` or `Detached` on the same channel, whose arrival removes the entry and sends
  `Detach`. The orphan lasts about one replay round trip.
- Decision: skipped under the triage rule for findings covered by the specifications, a review filter, or a Planned TODO
  item. Basis: FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics": token rotation at the exact
  moment of an attach, self-correcting within a round trip. SPEC_impl.md's "Who owns an accepted action" still describes
  the target shape for this code. This is accepted, filtered or planned behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage on 2026-09-28.

## revoked-attach-timeout-leaves-attachment.md

- Outcome: `other`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction. Same root cause, code and self-correction as
  `revocation-during-admission-orphans-attachment.md`; effectively a duplicate.
- Decision: skipped under the triage rule for findings covered by the specifications, a review filter, or a Planned TODO
  item. Basis: FILTER.md filter "Rare, self-correcting glitches and imprecise diagnostics", as for its duplicate. This
  is accepted, filtered or planned behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage on 2026-09-28.

## second-restart-holds-directory-lock.md

- Outcome: `other`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction. A queued second restart still holds the host-wide directory lock while waiting
  on the first restart's lifecycle claim, so creates, deletes and restarts on the host wait. Terminal typing freezes
  only if a create arrives meanwhile, because creates run inline on the connection read loop.
- Decision: skipped under the triage rule for findings covered by the specifications, a review filter, or a Planned TODO
  item. Basis: SPEC.md "Waiting between operations on one host" accepts session-management operations waiting on each
  other; the typing stall is the inline-create path covered by TODO.md's Planned item "Keep session creation off the
  connection read loop". This is accepted, filtered or planned behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage on 2026-09-28.

## update-ignores-remote-xdg-state-home.md

- Outcome: `other`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction. With no recorded state directory, Update still plans `~/.local/state/farhelm`
  while a dial without `--state-dir` honours an absolute `XDG_STATE_HOME`. A host installed by the helm's own setup
  always records its absolute state directory, and a fresh helm re-registering it probes absent and re-plans the same
  path; the wrong-directory case needs a supervisor started some other way.
- Decision: skipped under the triage rule for findings covered by the specifications, a review filter, or a Planned TODO
  item. Basis: SPEC.md "Supported host setup": hosts whose supervisor was started other than by the helm's own setup are
  best effort. This is accepted, filtered or planned behavior, not a completed fix.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage on 2026-09-28.

## ticker-waits-on-lifecycle-claim.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. `persist_work_started` still waits on the session's lifecycle claim
  inside the sequential sample pass, so one Stop, Restart or Delete stalls status sampling for every session on the
  host.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Waiting between operations on one host": session status must not wait on session-management operations. The principle
  itself is recorded in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; the ticker's work-start write now takes the lifecycle claim with a non-blocking
  `KeyedLocks::try_claim` and parks a start it cannot write yet (`deferred_work_start`) for the next pass, and the
  automatic reap of exited tabs uses `Supervisor::try_close_tab`, skipping a busy session until a later tick (found by
  the PR review), so a Stop, Restart or Delete no longer stalls the serial sample pass. Regression tests
  `busy_lifecycle_claim_defers_the_work_start_instead_of_waiting` and
  `dead_tab_reap_skips_a_session_whose_lifecycle_claim_is_busy`. jj change `xvznsnykpluk`, bookmark
  `pr/ticker-never-waits-on-lifecycle`, draft PR [#1163](https://github.com/scode/farhelm/pull/1163/changes).

## attachment-discard-under-global-lock.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. Delete still removes quarantined attachment files, retired preparation
  state and the hook log while holding the supervisor-wide `attachments` lock.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Waiting between operations on one host": terminal attach, input and resize must not wait on Delete. The principle
  itself is recorded in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; Delete now sends its detach notices and releases the supervisor-wide `attachments` guard before
  removing the deleted session's preparation state, quarantined attachment files and hook trace. Regression test
  `delete_releases_the_attachments_guard_before_removing_files` holds a new test-only fault hook
  (`deleted_session_cleanup_gate`) at that boundary and checks the guard is free. jj change `yppzuyowtwpp`, bookmark
  `pr/delete-cleanup-outside-attachments-lock`, draft PR [#1164](https://github.com/scode/farhelm/pull/1164/changes).

## provisioning-holds-host-cache-lock.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. A provisioning run holds the actor's `cache_lock` for its whole
  duration, and session write-backs and refresh commits wait on it with no deadline.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Waiting between operations on one host": the session list and replies to operations that already took effect must not
  wait on an install or update. The principle itself is recorded in the change that carries these triage decisions; this
  item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; a per-host provisioning lock (`ConnectionManager::host_provision_lock`, kept in a manager-level
  map so it survives actor replacement, found by the PR review) is held by a confirmed run instead of the cache-write
  lock; retarget, alias and removal take it before the cache-write lock, so they still wait for a run while session
  write-backs and refresh commits no longer do. Regression tests `a_running_update_leaves_the_cache_write_lock_free` (a
  real blocked run) and `the_provisioning_lock_survives_actor_replacement`. jj change `mzkoqwonkwrz`, bookmark
  `pr/provisioning-own-host-lock`, draft PR [#1165](https://github.com/scode/farhelm/pull/1165/changes).

## clipboard-sink-blocks-async-worker.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. The desktop clipboard endpoint still calls the blocking native writer,
  under a `std::sync::Mutex`, directly on an async worker. The blocking cost is unmeasured.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Waiting between operations on one host": terminals and the session list must not wait on clipboard writes. The
  principle itself is recorded in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; the desktop clipboard endpoint now runs the native writer on tokio's blocking pool
  (`spawn_blocking`) instead of an async worker, and `ClipboardSink`'s contract says it may block. Regression test
  `a_blocked_native_write_does_not_stall_other_requests`. jj change `npzwnsvxouml`, bookmark
  `pr/clipboard-off-async-worker`, draft PR [#1166](https://github.com/scode/farhelm/pull/1166/changes).

## release-download-unbounded-under-host-lock.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. Release downloads still have no overall deadline and run inside the run
  task that holds the host lock, and Remove waits for that lock.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Waiting between operations on one host": removing a host must respond promptly, if only to refuse. The principle
  itself is recorded in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; host removal no longer waits for a running setup or update: it tries the host's provisioning
  lock without waiting (`ConnectionManager::try_host_provision_lock`) and answers 409 "busy" while a run holds it, per
  the user's decision to refuse rather than abort. No download or ssh deadlines were added, since the ledger decision
  asks only for a prompt Remove. This finding and its sibling (`release-download-unbounded-under-host-lock.md` /
  `sftp-upload-unbounded-before-temp-appears.md`) share one change because the same diff fixes both. Regression test
  `removal_refuses_during_a_run_and_purges_after_it`. jj change `xputysstlkyn`, bookmark
  `pr/remove-host-refuses-while-busy`, draft PR [#1167](https://github.com/scode/farhelm/pull/1167/changes).

## sftp-upload-unbounded-before-temp-appears.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. The sftp upload arms no deadline until the first successful size poll,
  ssh sets no ConnectTimeout or ServerAlive options, and Remove waits for the host lock.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Waiting between operations on one host": removing a host must respond promptly, if only to refuse. The principle
  itself is recorded in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; host removal no longer waits for a running setup or update: it tries the host's provisioning
  lock without waiting (`ConnectionManager::try_host_provision_lock`) and answers 409 "busy" while a run holds it, per
  the user's decision to refuse rather than abort. No download or ssh deadlines were added, since the ledger decision
  asks only for a prompt Remove. This finding and its sibling (`release-download-unbounded-under-host-lock.md` /
  `sftp-upload-unbounded-before-temp-appears.md`) share one change because the same diff fixes both. Regression test
  `removal_refuses_during_a_run_and_purges_after_it`. jj change `xputysstlkyn`, bookmark
  `pr/remove-host-refuses-while-busy`, draft PR [#1167](https://github.com/scode/farhelm/pull/1167/changes).

## restart-relaunches-over-unconfirmed-scope.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. Stop's `stop_live_agent` and Restart's leftover reap both still pass
  `ScopeKillFailure::Warn`; only Delete refuses on an unconfirmed scope.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Lifecycle operations" (confirmed 2026-09-28): an operation whose cleanup cannot be confirmed fails visibly; Restart
  must not relaunch and Stop must report the failure, as Delete already does. The principle itself is recorded in the
  change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; Stop (both its live-agent path, `stop_live_agent`, which Restart also uses, and its
  exited-agent path in the Stop handler, found by the PR review) and Restart's reap of an exited agent's leftovers now
  use `ScopeKillFailure::Refuse`, so an unconfirmed scope fails the operation as Delete already did. A recorded scope
  that cannot be checked because the user manager is not usable now also counts as unconfirmed (second PR review).
  Regression tests `stop_refuses_when_the_scope_cannot_be_confirmed`,
  `stop_of_an_exited_agent_refuses_an_unconfirmed_scope`, `stop_refuses_when_a_recorded_scope_cannot_be_checked` and
  `restart_refuses_while_the_prior_scope_is_unconfirmed`. jj change `vwpkzvzlslon`, bookmark
  `pr/stop-restart-refuse-unconfirmed-scope`, draft PR [#1168](https://github.com/scode/farhelm/pull/1168/changes).

## tab-close-skips-scope-on-stale-verdict.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. `reap_tab_tree` still passes derived scope names that
  `reap_process_tree` drops on a cached negative manager verdict, contrary to `reap_tab_tree`'s docs; `reprobe` is
  one-shot per supervisor.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Lifecycle operations": a belief that the host has no usable user manager does not excuse skipping a scope the tab may
  have. The principle itself is recorded in the change that carries these triage decisions; this item's execution is the
  code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; `reap_tab_tree` (Close Tab, the ticker's reap of exited tabs, and a failed open's unwind) now
  uses the scope manager's one allowed re-probe when the cached verdict is negative before its derived tab scope is
  skipped, and fails the close when the tab's scope cannot be confirmed gone (`ScopeKillFailure::Refuse`), per SPEC.md
  "Lifecycle operations". Whether a tab was opened scoped is now recorded on its tmux window (`@farhelm-tab-scoped`) and
  decides whether an uncheckable scope fails the close, because the session's own launch can have seen a different
  manager verdict. Windows from older builds carry no marker: they are treated as scoped when their session's launch
  was, and otherwise still get the one re-probe, their scope skipped only if the manager stays unusable (such a tab is
  then indistinguishable from one on a host that never had a manager, where refusing would leave it unclosable).
  Regression tests `tab_reap_reprobes_a_stale_verdict_and_refuses_an_unconfirmed_scope`,
  `tab_reap_follows_the_tabs_own_scope_marker`, `an_unmarked_tab_of_an_unscoped_session_still_gets_the_reprobe`, and
  `an_opened_tab_records_whether_it_was_scoped`. Not addressed by decision: a failure of the automatic reap of an exited
  tab is logged and retried on the next pass rather than shown in the UI, which would need new protocol and UI state. jj
  change `uvolosqwuukw`, bookmark `pr/tab-close-rechecks-scope`, draft PR
  [#1170](https://github.com/scode/farhelm/pull/1170/changes).

## sweep-drops-unreadable-root-silently.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present, slightly wider than reported: `validate_root_identity` treats an
  unreadable root as missing, and `capture_process_identity` maps read errors to `None` without logging.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Lifecycle operations": a process Farhelm tried to reap but could not examine counts as unconfirmed, never as gone.
  The principle itself is recorded in the change that carries these triage decisions; this item's execution is the code
  fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; `capture_process_identity` and the sweep's `validate_root_identity` now return an error for a
  pane process that exists but cannot be read (a vanished process stays `None`), and Stop, Restart, Delete and the tab
  reap fail on it instead of sweeping without the root. Regression test
  `an_unreadable_pane_process_is_unconfirmed_not_gone`. jj change `mwtmlovlpput`, bookmark
  `pr/unreadable-root-fails-sweep`, draft PR [#1171](https://github.com/scode/farhelm/pull/1171/changes).

## missing-checkout-root-blocks-delete.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. `verified_root` still gates retirement, teardown, pending-archive
  recovery and restart recovery, with no fallback that retires the row.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md "Fresh
  GitHub checkouts" (confirmed 2026-09-28): archiving never blocks Delete; Delete completes with a visible notice naming
  the folder left behind. The principle itself is recorded in the change that carries these triage decisions; this
  item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; the last-reference archive in Delete (`archive_last_reference` in teardown.rs) no longer fails
  the Delete: any reason the checkout cannot be archived safely (root or folder identity mismatch, overlapping records,
  a failed or unreconcilable move) leaves the folder where it is (or, after a failed move, possibly at its journaled
  archive destination, which the notice then names), releases its registry row (`working_copies::release_unarchived`) in
  Delete's final transaction, and returns a notice. A missing folder whose absence no longer holds at commit time is
  released the same way instead of rolling the Delete back. The notice travels in `SessionDeleted.notice`, through the
  helm's delete reply and, for a Replace, its reply's `delete_notice`, to a dismissible line in the session list. The
  unresolved-plan notice the review also asked for is left to its own queued item,
  `preserved-plan-diagnostic-log-only.md`. Regression tests
  `deleting_with_a_replaced_root_releases_the_checkout_untouched`,
  `archive_parent_sync_failure_completes_delete_with_a_notice`,
  `recovery_barrier_failure_names_the_rejournaled_destination`,
  `an_overlap_refusal_of_a_pending_archive_names_its_destination`,
  `a_missing_checkout_that_reappears_before_commit_is_released_not_fatal`, the updated stranger-at-source test,
  `delete_session_passes_the_supervisors_notice_to_the_caller` and `a_replace_reply_carries_the_source_deletes_notice`,
  `a_delete_reply_that_cannot_be_read_is_not_silent` (UI) (helm), and the `SessionDeleted` wire-shape test (proto). jj
  change `lvpkwsqvksym`, bookmark `pr/delete-completes-despite-archive`, draft PR
  [#1173](https://github.com/scode/farhelm/pull/1173/changes).

## noreplace-rename-unsupported-strands-archive.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. Only EEXIST/ENOTEMPTY are classified; any other rename error propagates
  after the row is already `archive_pending`, and recovery retries the same rename.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md "Fresh
  GitHub checkouts": archiving never blocks Delete. The principle itself is recorded in the change that carries these
  triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; with `missing-checkout-root-blocks-delete.md` already making any archive failure complete the
  Delete with a notice, this change handles what that left: `rename_exclusive_into` (working_copies.rs), shared by the
  archive move and crash recovery, classifies rename errors that report nothing moved (`EINVAL`, `ENOSYS`,
  `ENOTSUP`/`EOPNOTSUPP`, `EXDEV`, `EACCES`, `EPERM`, `EROFS`) as `WorkingCopyError::RenameRefused`, rolls the journal
  back to `allocated`, and Delete's notice then says the checkout stays where it is. Other rename errors keep the
  pending journal, since a network filesystem can complete a rename whose reply is lost. Documented in SPEC_impl.md
  beside the no-replace rename. Regression tests `a_refused_rename_rolls_the_journal_back_and_leaves_the_checkout`,
  `an_ambiguous_rename_error_keeps_the_pending_journal`, and
  `a_refused_archive_rename_leaves_the_checkout_with_a_notice` (a real `EACCES`). jj change `pspqplxuntvv`, bookmark
  `pr/archive-rename-refusal-rolls-back`, draft PR [#1176](https://github.com/scode/farhelm/pull/1176/changes).

## checkout-path-too-long-for-archive.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. Admission accepts paths up to 4096 bytes with no room for the archive
  suffix, and recovery retries with a longer name.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md "Fresh
  GitHub checkouts": archiving never blocks Delete. The principle itself is recorded in the change that carries these
  triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; with `missing-checkout-root-blocks-delete.md` already keeping any archive failure from blocking
  Delete, this change stops such checkouts being admitted and handles the ones that exist. The preview and the create
  recheck refuse a planned checkout path over `working_copies::MAX_ADMITTED_CHECKOUT_PATH` (the platform's `PATH_MAX`
  less its NUL and the 82 bytes archiving can add), with a message naming the limit, and `ENAMETOOLONG` from the archive
  rename now counts as a rename that moved nothing (`rename_refused_without_moving`), so an older over-long checkout is
  rolled back and left in place with a notice rather than stuck mid-archive. SPEC.md "Fresh GitHub checkouts" states the
  limit. Regression test `a_checkout_path_too_long_to_archive_is_refused_at_preview`. jj change `wxmpukrrnuxt`, bookmark
  `pr/checkout-path-archive-margin`, draft PR [#1177](https://github.com/scode/farhelm/pull/1177/changes).

## preserved-plan-diagnostic-log-only.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. The preserved-path diagnostic is still only a `warn!`, and the Delete
  reply has no field to carry it.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md "Fresh
  GitHub checkouts": Delete's result tells the user about a folder left behind and names it. The principle itself is
  recorded in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; Delete's unresolved-plan branch (teardown.rs) now adds a notice naming the preserved path to
  the Delete's result, using the notice channel from `missing-checkout-root-blocks-delete.md`, so it reaches the session
  list. A path whose check fails for a reason other than absence is reported as unchecked rather than read as absent.
  The log line stays. Regression test `deleting_an_unresolved_plan_names_and_preserves_the_unknown_path` now asserts the
  returned notice. jj change `ukplmklxmwru`, bookmark `pr/delete-names-unresolved-plan-folder`, draft PR
  [#1178](https://github.com/scode/farhelm/pull/1178/changes).

## checkout-name-scan-case-sensitive.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. The occupancy scan still compares names byte for byte while candidates
  are lowercase, so a case-variant folder on a case-insensitive filesystem makes every unnamed launch conflict.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: the user
  confirmed macOS is fully supported; this is a bad footgun on its default filesystem. The principle itself is recorded
  in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; the occupancy scan (`occupied_related_names_from_entries` in working_copies.rs) lowercases
  entry names before comparing them with the always-lowercase candidates, so a case-variant folder occupies its name on
  every filesystem. Regression test `occupied_scan_treats_case_variants_as_occupied`. jj change `wxpqrpwozwwm`, bookmark
  `pr/checkout-name-scan-ignores-case`, draft PR [#1179](https://github.com/scode/farhelm/pull/1179/changes).

## linger-failure-blocks-update-restart.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. `EnableLinger` precedes the restart and attach steps, and only a narrow
  permission-refusal shape degrades; other loginctl failures stop the run.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: affects the
  supported helm-driven setup path (SPEC.md "Supported host setup"); linger is documented as optional. The principle
  itself is recorded in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; `linger_failure_outcome` (provisioning/backend.rs) degrades the optional linger step for any
  failure of the remote command (a recognized refusal keeps its wording; any other non-zero status other than ssh's 255
  reports the status only, with the host's stderr logged rather than shown), so an Update goes on to restart the
  supervisor. A failure to reach the host (255 or no status) stays fatal. The step order is unchanged: with every
  command failure degraded, running linger after restart would change nothing for the user. Regression test
  `any_remote_linger_failure_degrades_instead_of_failing_the_run`. jj change `lrlkyvvqlkwq`, bookmark
  `pr/linger-failure-degrades`, draft PR [#1181](https://github.com/scode/farhelm/pull/1181/changes).

## add-confirm-rewrites-row-before-busy-check.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. `start_add` consumes the plan and re-registers the row (rewriting its
  paths and dropping the connection) before `start_run` checks `busy`.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: affects the
  supported helm-driven setup path. The principle itself is recorded in the change that carries these triage decisions;
  this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; `start_add` (provisioning/service.rs) now claims the run slot (the `busy` set) of an
  already-registered destination before `register` rewrites its row, refusing with Busy and keeping the plan when a run
  is in flight; `start_run` takes the claim over instead of claiming again. ADD confirmations are serialized from the
  lookup to the run's claim (`add_confirmations`), so two confirmations for one destination cannot both find it
  unclaimed. The claim-register-start sequence runs on a task the service owns, so a dropped request cannot leave the
  host claimed (SPEC_impl.md "Who owns an accepted action"). Regression test
  `a_refused_add_leaves_the_busy_host_row_and_its_plan_alone`. jj change `pprtsxvtlnnk`, bookmark
  `pr/add-claims-busy-host-first`, draft PR [#1182](https://github.com/scode/farhelm/pull/1182/changes).

## installer-stale-lock-recovery-not-exclusive.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. Stale-lock recovery still has no exclusive claim, so two installers can
  both replay the journal.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Concurrent and interrupted runs": overlapping runs must end correct, and refusing is acceptable. The principle itself
  is recorded in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; `acquire_lock` in scripts/install.sh claims stale-lock recovery with an atomic sidecar
  `mkdir "$LOCK_DIR.recovering"` before it rolls back a journal or replaces the lock, re-checks that the lock still
  records the same stale pid, and releases the claim on every path out. A run that cannot take the claim refuses and
  asks for a retry; a leftover claim from a killed recovery gets the same refusal with remove-by-hand advice, like the
  pid-less lock. The lock's re-creation got its own failure message. Regression scenario "stale-lock recovery refuses
  while another run holds the recovery claim" in scripts/test-install-sh.sh. jj change `kslnoqskoymq`, bookmark
  `pr/installer-recovery-claim`, draft PR [#1183](https://github.com/scode/farhelm/pull/1183/changes).

## installer-bundle-swap-unlocked.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. The bundle step still runs after the lock is released and swaps with
  `rm -rf` then `mv`.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Concurrent and interrupted runs". The principle itself is recorded in the change that carries these triage decisions;
  this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; scripts/install.sh keeps the install lock through the macOS bundle step, so the bundle copies
  this run's committed binaries, and the bundle step takes its own lock (`~/Applications/.farhelm-app.lock`, a `mkdir`),
  because every install directory shares the one bundle name. Contention, or a lock an interrupted run left, refuses the
  bundle step with remove-by-hand advice (the binaries are installed). Under that lock the ownership check runs, the new
  bundle is built in a private `mktemp -d` directory beside it (same filesystem, so the final `mv` is a rename), the old
  bundle is moved into that directory, the new one moved in, and only then the directory deleted; the exit handler puts
  the old bundle back if the swap did not finish. Regression checks "bundle lock: ..." in scripts/test-install-sh.sh (a
  `cp` double shows both bundle copies run under the install lock; a held bundle lock refuses and leaves bundle and lock
  alone; unrelated hidden entries survive; "bundle swap failure" and "bundle restore failure" checks use an `mv` double
  to show a failed swap puts the old bundle back, and a failed restore keeps it at the named private path).
  docs/install_uninstall.md describes the one-at-a-time bundle step and the lock's recovery. jj change `pomunpvqnpxs`,
  bookmark `pr/installer-bundle-under-lock`, draft PR [#1184](https://github.com/scode/farhelm/pull/1184/changes). The
  swap also resolves two TODO entries (one added to main while this stack was open) that describe the in-place deletion
  it replaces; both are removed here.

## leftover-uninstall-receipt-blocks-uninstall.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. The sibling receipt survives an interrupted uninstall, the installer
  never clears it, and uninstall then refuses with advice that does not help.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Concurrent and interrupted runs": no state that no documented command recovers from. The principle itself is recorded
  in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; after rebuilding the bundle, scripts/install.sh removes
  `~/Applications/.Farhelm.app.uninstall-receipt` when it is a regular file carrying this installation's own record
  (`record_file_is_ours`, the test `bundle_record_is_ours` already used). Another installation's copy is that
  installation's only way to finish its uninstall, so the bundle step refuses (under the bundle lock, before building)
  with advice to finish or clear that uninstall, rather than create a bundle that would leave both installations unable
  to uninstall. The installer-side option was chosen because it needs no SPEC_impl.md change ("If both receipts survive,
  they must agree" stays). Regression checks "leftover receipt: ..." and "foreign receipt: ..." (a real installation A's
  receipt, installation B refused) in scripts/test-install-sh.sh. docs/install_uninstall.md describes both. jj change
  `xnynxxolwpsw`, bookmark `pr/installer-clears-own-uninstall-receipt`, draft PR
  [#1185](https://github.com/scode/farhelm/pull/1185/changes).

## interrupt-before-install-record-publish.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. The ownership record is published after the journal is deleted, and the
  resulting digest refusal gives no repair advice.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Concurrent and interrupted runs": binaries and ownership records must not disagree. The principle itself is recorded
  in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; uninstall's digest-mismatch refusal (`verify_payload` in
  crates/farhelm/src/uninstall/ownership.rs) now says that an interrupted install or update is repaired by re-running
  the installer, which republishes the record unconditionally, and then running uninstall again. The installer's commit
  order was left alone: publishing the record inside the journaled phase would need the journal to undo a record too,
  and the window is two deletes and two hashes that the advised re-run already repairs. Regression test
  `mismatched_flat_digest_refuses` now requires the advice. jj change `xmsmttxlslru`, bookmark
  `pr/uninstall-digest-mismatch-advice`, draft PR [#1186](https://github.com/scode/farhelm/pull/1186/changes).

## setup-partial-unit-write-mismatch.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. Setup still interleaves fallible per-unit steps with unit writes,
  supervisor unit first, before `daemon-reload`.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Concurrent and interrupted runs": helm and supervisor services must not be left on different state directories. The
  principle itself is recorded in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; `install()` in crates/farhelm/src/setup.rs now runs every fallible pre-write step for both
  units (the `is-active` query and the restart marker) before writing either, then writes the changed units back to back
  (`write_units_together`), restoring any unit already written (or removing one that did not exist before) if a later
  write fails. Regression test `a_failure_on_the_second_unit_leaves_both_units_unwritten`. jj change `xqnwvptyqnls`,
  bookmark `pr/setup-writes-units-together`, draft PR [#1187](https://github.com/scode/farhelm/pull/1187/changes).

## ssh-controlpath-too-long.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. The ControlPath is the state directory plus 65 bytes; with the default
  state directory the longest working username is 15 characters on Linux and 10 on macOS.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Supported user environments" (confirmed 2026-09-28): usernames up to 20 characters must work with the default state
  directory locations. The principle itself is recorded in the change that carries these triage decisions; this item's
  execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; the ControlPath is now `<state dir>/%C` (`ssh::control_socket`), 58 bytes on top of the state
  directory with OpenSSH's temporary suffix instead of 65, so the default state directory keeps connection sharing for
  usernames up to 22 characters on Linux and 17 on macOS. Where the socket cannot fit (longer macOS usernames, deep
  custom state directories) ssh runs with `ControlMaster=no` and `ControlPath=none` instead of failing, so every
  supported username works, some without sharing. `%C` is kept rather than a shorter hash of the destination, because it
  identifies the resolved host, port and user and so never reuses a master for an alias that now points elsewhere.
  SPEC_impl.md's transport section says so. Regression test
  `sharing_is_used_where_the_socket_fits_and_turned_off_where_it_cannot`. jj change `xqtrmqrsorlo`, bookmark
  `pr/ssh-short-control-socket`, draft PR [#1188](https://github.com/scode/farhelm/pull/1188/changes).

## relative-install-dir-installs-under-cwd.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. `FARHELM_INSTALL_DIR` is used as given, with no absolute-path check,
  and messages echo the relative spelling.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: the user chose
  to fix it: a stray `~` directory invites a destructive `rm -rf ~`. The principle itself is recorded in the change that
  carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; scripts/install.sh refuses a `FARHELM_INSTALL_DIR` that does not start with `/` before creating
  anything, naming an unexpanded leading `~` explicitly and suggesting `$HOME`. This follows the finding's suggestion to
  refuse every relative value, so the harness's earlier check that a relative destination under an inherited CDPATH
  recorded the right directory was retired with that behavior. Regression checks "relative dir: ..." in
  scripts/test-install-sh.sh, run from a scratch directory. docs/install_uninstall.md says the path must be absolute. jj
  change `oomkzonynlnn`, bookmark `pr/installer-requires-absolute-dir`, draft PR
  [#1189](https://github.com/scode/farhelm/pull/1189/changes).

## idempotency-fingerprint-keeps-raw-cmdline.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. Permanent-scope reservations keep their fingerprint, including the raw
  invocation and resume template, after Delete.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: decided with
  the command-line question: command lines are not secret under the temporary exception, but deleting a session must
  still remove what is stored about it. The principle itself is recorded in the change that carries these triage
  decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; Delete's reservation settlement (`settle_create_reservations_for_delete` in store.rs, used by
  both delete transactions) replaces each permanent reservation's fingerprint with `sha256:<hex>` of it
  (`store::tombstone_fingerprint`), plus `;identity=<hex>` of the client identity for a fresh-checkout fingerprint.
  `resolve_reservation` accepts either the raw fingerprint or its digest, so a retry of the same request still gets the
  deleted-session answer and a different request under the key is still refused; fresh-checkout reconciliation checks
  the identity digest the same way and refuses an ordinary create's tombstone. Opening the store digests settled
  (created or failed) permanent reservations whose session is already gone, except the identity-only refusal record, so
  sessions deleted before the upgrade are covered too. Fingerprints of live sessions are untouched (recovery needs
  them). SPEC_impl.md records the digest. Regression tests `a_deleted_sessions_retry_record_keeps_only_a_digest`,
  `fresh_reconciliation_of_a_deleted_key_still_checks_identity`, and
  `opening_the_store_digests_deleted_sessions_retry_records`;
  `populated_v17_upgrade_preserves_sessions_and_literal_key_semantics` now expects the deleted key's digest. jj change
  `qtvstxnqtlyl`, bookmark `pr/deleted-session-retry-digest`, draft PR
  [#1190](https://github.com/scode/farhelm/pull/1190/changes).

## desktop-bootstrap-token-always-pushed.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. The desktop bootstrap always sends the web token. Verification notes
  the finding's primary fix is insufficient on its own, because a faked 401 would still trigger the existing token
  retry; capturing primitives at first load is the part that addresses the scenario.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Client hardening" (confirmed 2026-09-28): the native app is held to a higher bar. The principle itself is recorded in
  the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; the web token no longer enters the desktop webview's JavaScript at all. When the page's stored
  device secret is missing or refused it asks native (`need_secret`), and `DesktopBootstrapGate` mints the webview's
  device secret itself (`mint_webview_secret` in auth.rs over `api::mint_webview_device_secret`, re-reading a rotated
  token once) and sends only that secret. This replaces the first plan here (send the token on request and capture
  browser primitives at first load): review showed a script injected after launch could still intercept the token
  through other page intrinsics, and moving the exchange out of the page removes the token from reach instead. JS tests
  `a page without a usable secret asks native to mint one` and
  `a native mint failure is reported as the
  authentication error`, with the rest updated to the protocol; the native
  exchange keeps the page's former five-second bound over headers and body (`WEBVIEW_EXCHANGE_TIMEOUT`), with native
  tests `a_refused_token_is_none_and_a_granted_one_is_the_secret` and
  `a_stalled_exchange_body_is_cut_off_at_the_deadline`. jj change `tytynmmukovw`, bookmark `pr/desktop-token-on-demand`,
  draft PR [#1194](https://github.com/scode/farhelm/pull/1194/changes).

## any-dioxus-webview-passes-origin-guard.md

- Outcome: `fix spec` (revised 2026-09-29; originally `fix code`).
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. The guard is still a bare `dioxus://`/`wry://` prefix check.
  Verification notes an exact-origin match would not help, since other Dioxus apps share `dioxus://index.html`; only a
  Farhelm-specific scheme closes it.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Client hardening": hardening that keeps other software from passing for the native app is wanted. The principle
  itself is recorded in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Revised decision (2026-09-29, supersedes the decision above): execution found the code fix impossible as scoped.
  dioxus-desktop 0.7.10 hardcodes the page URL `dioxus://index.html/` on Linux and macOS, so every Dioxus desktop app
  sends the same Origin, and only a Farhelm-specific scheme (a patched dependency) could tell Farhelm's window apart.
  The user decided the Origin check is a browser-facing defense and not how the native app is identified: another Dioxus
  or wry app's content passing it is an accepted risk for that browser check, not for the desktop app, whose
  identification must never rely on Origin. Moving the desktop client off the loopback network path entirely is recorded
  separately as a "Maybe later" TODO.
- Revised completion criteria: SPEC.md states the principle, SPEC_impl.md and the guard's doc comment explain the
  residual, and this feedback file and its index entry are removed.
- Execution: `complete`; SPEC.md "Client to helm", SPEC_impl.md's loopback-guard bullet, `docs/security.md` and
  `is_desktop_webview_origin`'s doc comment. Change `lsukxzqknprlktvxxynssykrtmklqqxx`, bookmark
  `pr/spec-origin-browser-only`, draft PR [#1216](https://github.com/scode/farhelm/pull/1216/changes).

## restart-sweep-on-abortable-connection-task.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. `restart_session` runs inside the connection's abortable task set; only
  the final relaunch moves to a supervisor-owned task, unlike Stop.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC_impl.md
  "Who owns an accepted action" (confirmed 2026-09-28): fix by moving execution ownership, not by per-step cleanup. The
  principle itself is recorded in the change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; `handle_restart_session` (handlers.rs) now runs `restart_session` on a supervisor-owned
  `tokio::spawn` holding the admission permit, the way Stop already does, and the connection's task set holds only the
  reply waiter, so a client disconnect cannot abort the stop-and-sweep phase. Regression test
  `restart_survives_connection_task_cancellation` (the Stop test's marker-only fixture, driven through
  `RestartSession`). jj change `qmqxrsxypwmq`, bookmark `pr/restart-owned-by-supervisor`, draft PR
  [#1195](https://github.com/scode/farhelm/pull/1195/changes).

## host-edits-not-cancellation-safe.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: partly present. Add, retarget, adopt and remove still commit and then converge
  in later awaits inside the handler. #1104 makes a connected actor whose row is gone retire on its next refresh, which
  covers part of Remove for hosts with an identity.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC_impl.md
  "Who owns an accepted action". The principle itself is recorded in the change that carries these triage decisions;
  this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; the add, retarget (`set_destination`), remove and adopt handlers in hosts.rs run their
  commit-then-reconcile bodies through `run_owned` (lib.rs), which spawns the work on a helm-owned task and awaits it,
  so a dropped request loses only the reply (SPEC_impl.md "Who owns an accepted action"). `run_owned` is the one small
  shared helper the plan allows; `cancelled-start-run-leaves-host-busy.md` reuses it. Regression test
  `owned_work_completes_after_its_waiter_is_dropped`. The YOLO-safe host setting, added on main while this stack was
  open, saves and then announces the same way, so its handler goes through `run_owned` too. jj change `lnnlrvnylkpx`,
  bookmark `pr/host-edits-owned-by-helm`, draft PR [#1196](https://github.com/scode/farhelm/pull/1196/changes).

## cancelled-start-run-leaves-host-busy.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. `start_run` marks the host busy, then awaits inside the request handler
  before spawning; nothing but the explicit error branch clears `busy`.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC_impl.md
  "Who owns an accepted action". The principle itself is recorded in the change that carries these triage decisions;
  this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; `start_update` (provisioning/service.rs) runs its body through `run_owned`, so marking the host
  busy, waiting for its provisioning lock and spawning the run that clears the mark happen on a helm-owned task and a
  dropped request cannot strand the mark. `start_add` already runs its claim-register-start sequence on an owned task
  (`add-confirm-rewrites-row-before-busy-check.md`). Regression test
  `a_dropped_update_confirmation_still_runs_and_clears_busy`. jj change `uvrxrrruunyz`, bookmark
  `pr/update-start-owned-by-helm`, draft PR [#1197](https://github.com/scode/farhelm/pull/1197/changes).

## agent-create-replays-asker-as-child.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present. Helm-relayed agent creates reach the target with full authority,
  bypassing the self-replay guard, and the fingerprint has no asker.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Agent-spawned sessions" (confirmed 2026-09-28): idempotency keys are scoped to the asking session, for spawn, agent
  create and agent clone. The principle itself is recorded in the change that carries these triage decisions; this
  item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; the helm relay stores an agent create's or clone's intent key on the target as
  `agent-<asking session>-<SHA-256 of the key>` (`asker_scoped_intent_key` in agent_requests.rs), so a key belongs to
  the session that asked and another session's identical keyed request creates afresh instead of replaying the first
  result. The hash keeps the stored key within the target's 512-byte limit. Keys reserved before the change no longer
  match (accepted in the goal's outline). SPEC_impl.md's agent-verbs section says so. Regression test
  `agent_intent_keys_are_scoped_to_the_asking_session`; the relay tests now expect the scoped key. jj change
  `mymmsoqrlrwx`, bookmark `pr/agent-keys-scoped-to-asker`, draft PR
  [#1191](https://github.com/scode/farhelm/pull/1191/changes).

## spawn-replays-asker-as-child.md

- Outcome: `fix code`.
- Assessment: verified against current main (11c3f7c plus the uncommitted 2026-09-28 principle edits) by code
  inspection, not runtime reproduction: present as narrowed: the self-replay case is refused, sibling key reuse is not,
  and the fingerprint has no asker field.
- Decision: settled by a principle the user decided in the 2026-09-28 triage of product questions. Basis: SPEC.md
  "Agent-spawned sessions": idempotency keys are scoped to the asking session. The principle itself is recorded in the
  change that carries these triage decisions; this item's execution is the code fix.
- Completion criteria: fix the behavior so it meets the cited rule, with focused regression coverage, and remove this
  feedback file and its index entry in the execution change.
- Execution: `complete`; `handle_create_session` reserves a spawn's intent key as
  `spawn-<asking session>-<SHA-256 of
  the key>` (`spawn_scoped_intent_key` in handlers.rs) before admission, so the
  per-key lock and the reservation both use the scoped form and another session reusing the key spawns afresh. The
  earlier self-replay refusal stays as a backstop. SPEC_impl.md states the scoping for spawn beside the agent
  create/clone scoping. Regression tests `spawn_intent_keys_are_scoped_to_the_asking_session` and
  `a_childs_identical_keyed_spawn_creates_its_own_child` (replacing the self-replay-refusal test); two other spawn tests
  now address the scoped key. jj change `llosqmksnzlw`, bookmark `pr/spawn-keys-scoped-to-asker`, draft PR
  [#1192](https://github.com/scode/farhelm/pull/1192/changes).

## window-maximize-fence-never-clears.md

- Outcome: `discard`.
- Assessment: not re-verified; the mechanism depends on a Linux window manager that ignores a maximize request,
  affecting only the development desktop build.
- Decision: the user chose to ignore unusual Linux window managers.
- Completion criteria: remove the feedback file and its index entry without code or spec changes.
- Execution: `complete`; the feedback file and its index entry were removed with no code or spec change. Change
  `ozxpwnwvoowmzuwlxwmpqkouzrknqmmr`, bookmark `pr/discard-window-maximize-fence`, draft PR
  [#1208](https://github.com/scode/farhelm/pull/1208/changes).

## reload-adopts-stale-pane.md

- Outcome: `discard` (already fixed).
- Assessment: already fixed on main by #917 (5130f10), which makes a `Launching` row with a dead pane list as Unknown
  and offer no exit transition, on the path reload, the ticker and the listing all share. Verified by code inspection.
- Decision: skipped; the user's standing rule is that already-fixed findings are removed without settling an outcome.
- Completion criteria: remove the feedback file and its index entry.
- Execution: `complete`; removed during triage on 2026-09-28.

## reload-stale-pane-records-old-exit.md

- Outcome: `discard` (already fixed).
- Assessment: already fixed on main by #917 (5130f10); duplicate of `reload-adopts-stale-pane.md`. Verified by code
  inspection.
- Decision: skipped; the user's standing rule is that already-fixed findings are removed without settling an outcome.
- Completion criteria: remove the feedback file and its index entry.
- Execution: `complete`; removed during triage on 2026-09-28.

## failed-plan-does-not-reserve-checkout-name.md

- Outcome: `discard` (already fixed).
- Assessment: already fixed on main by #1130 (0f9268b), which treats every non-retired registry row's basename, Planned
  rows included, as taken in both preview and create. Verified by code inspection.
- Decision: skipped; the user's standing rule is that already-fixed findings are removed without settling an outcome.
- Completion criteria: remove the feedback file and its index entry.
- Execution: `complete`; removed during triage on 2026-09-28.

## pre-mkdir-rollback-leaves-phantom-membership.md

- Outcome: `discard`.
- Assessment: partly confirmed by code inspection. `rollback_pre_mkdir_create` still removes only the session's own
  membership, but the reviewer's sequence is blocked on current main: `validate_retry` refuses when the planned path is
  occupied, and since #1130 a second create cannot choose the first one's planned name. What remains needs an outside
  process to create that exact path during a retry window after a crash, and its effect is an unarchived folder, not
  data loss.
- Decision: the user chose discard.
- Completion criteria: remove the feedback file and its index entry without code or spec changes.
- Execution: `complete`; the feedback file and its index entry were removed with no code or spec change. Change
  `vynvxnsuyvktmpslyruvmolnytxvuvrp`, bookmark `pr/discard-pre-mkdir-phantom-membership`, draft PR
  [#1209](https://github.com/scode/farhelm/pull/1209/changes).

## ambiguous-restart-republishes-old-terminal.md

- Outcome: `fix code`.
- Assessment: confirmed on current main by code inspection, not runtime reproduction. The ambiguous-relaunch branch
  still republishes the pre-restart terminal (`core.rs` around 10897), and the restart path discards #1094's
  `SpawnFailure::Tmux { created }` flag. A later Restart reads the stale pane as gone, skips the consent guard, and its
  reap can kill the new agent; opening the session targets a pane that no longer exists. Since #1030,
  `terminal_less_launch_may_be_live` already guards terminal-less entries.
- Decision: the user chose the bounded code fix: in the ambiguous branch, publish no terminal (or the new pane when tmux
  reported creating it) instead of the stale one, so the existing terminal-less consent guard applies.
- Completion criteria: a second Restart after an ambiguous relaunch asks before stopping a possibly live agent, opening
  the session no longer targets the stale pane, focused regression coverage, and remove this feedback file and its index
  entry in the execution change.
- Execution: `complete`; the failed-relaunch recovery in `restart_session` (core.rs) re-publishes the entry with no
  terminal when the failure is ambiguous and the relaunch had to build a fresh tmux session (`republished_terminal`),
  instead of the old, gone pane, so the existing terminal-less guard applies; definitive failures and relaunches into
  the surviving pane keep the prior terminal. The optional recording of the new pane (carrying it in
  `SpawnFailure::Tmux`) was not needed for the agreed bounded fix. Regression test
  `an_ambiguous_fresh_relaunch_republishes_no_terminal` (the decision as a pure function; there is no seam to fail tmux
  after a fresh session in-process). jj change `msnmoyuwrqnq`, bookmark `pr/ambiguous-restart-no-stale-terminal`, draft
  PR [#1198](https://github.com/scode/farhelm/pull/1198/changes).

## delete-roots-only-agent-pane.md

- Outcome: `fix code`.
- Assessment: partly confirmed on current main by code inspection. Delete roots its process walk only at the agent pane
  (`teardown.rs` around 215-217), and the comment claiming the marker scan finds tab processes wherever they are is
  false on macOS, where platform binaries withhold their environment. Tab processes still in their tab pane's tree
  therefore survive Delete on hosts without a usable systemd user manager; with one, tab scopes cover them. Hand-split
  panes are out of scope under SPEC.md's private tmux rule. Sessions with no recorded terminal have no walk root at all.
- Decision: the user chose the recommended fix with a simplicity gate. Root Delete's process walk at every terminal pane
  the session still has (agent and tabs, reusing the tab discovery Delete already does and Close Tab's per-pane walk);
  for a session with no recorded terminal, walk from whatever panes are found under its tmux session name, and accept
  what that still misses on hosts without a usable user manager. If planning or execution shows this needs significant
  new machinery or scope, stop, record the blocker, and return it to the user rather than growing the fix.
- Completion criteria: on a host without a usable user manager, a process still descended from a tab's shell is reaped
  by Delete; the misleading comment is corrected; focused regression coverage; remove this feedback file and its index
  entry in the execution change, or narrow it and document the blocker if the simplicity gate is reached.
- Execution: `complete`; Delete now roots its process walk at every live pane under the session's tmux name
  (`other_pane_roots` in teardown.rs), including the agent pane of a session with no recorded terminal, and the sweep
  accepts several roots. Execution showed the gap is narrower than reported: on Linux the marker scan already expanded
  from the tab's marked shell, so it matters where the tab's shell itself shows no marker (macOS platform binaries, or a
  pane process that replaced itself with a scrubbed environment); the regression test simulates that. Simplicity gate
  not reached. Change `uwkqkuomrnqq`, bookmark `pr/delete-roots-every-pane`, draft PR
  [#1199](https://github.com/scode/farhelm/pull/1199/changes).

## unstable-device-number-blocks-delete.md

- Outcome: `fix code`.
- Assessment: confirmed on current main by code inspection, not reproduced; how often device numbers change on real
  machines is unknown. Checkout identity is the raw `(st_dev, st_ino)` pair compared exactly, and #1046's birth-time
  check is combined with it rather than replacing it. The Delete half is now covered by SPEC.md's rule that archiving
  never blocks Delete (implemented with `missing-checkout-root-blocks-delete.md`). The Restart half remains: the session
  that created the checkout is refused permanently when only the device number changed.
- Decision: the user chose the bounded fix and asked that the code say clearly why. When the device number differs but
  the inode number and creation time both match, treat it as the same folder (optionally re-recording the identity).
  Rationale to document at the comparison: btrfs subvolumes (Fedora's default `/home`), NFS, overlayfs and some
  device-mapper setups assign device numbers at mount time, so they can change across a reboot or remount while the
  folder is untouched; these are sensible, supported setups and must not break. Inode plus creation time still detects a
  folder replaced at the same path, which is what the check exists for. Simplicity gate: where a platform cannot supply
  a usable creation time, refuse with a clear message instead of building further machinery.
- Completion criteria: Restart (and archiving) accept a checkout whose device number changed while inode and creation
  time match, with the rationale above in the code; a replaced folder is still refused; focused regression coverage;
  remove this feedback file and its index entry in the execution change.
- Execution: `complete`; `working_copies::same_directory` requires the device number only when no birth time was
  recorded, with the remount rationale at the comparison, so Restart, archiving and every other ownership check accept a
  folder whose inode and birth time match after a device-number change. Where no birth time can confirm the change
  (recorded or observed), `verify_identity` and `verified_root` refuse with a distinct `DeviceChangedUnconfirmed` answer
  whose message names a remount as the likely cause; the identity is not re-recorded. Change `uukxsptlrovx`, bookmark
  `pr/device-number-remount`, draft PR [#1200](https://github.com/scode/farhelm/pull/1200/changes).

## checkout-membership-misses-bind-mounts.md

- Outcome: `fix spec`.
- Assessment: confirmed by code inspection. Checkout membership compares canonical path text, which resolves symlinks
  but not bind mounts, so a session reaching a checkout through a bind-mount alias is not counted and the checkout can
  be archived while it still works there. SPEC.md said only "canonical subdirectories".
- Decision: the user chose to state in SPEC.md that filesystem aliasing is unsupported: bind mounts and any similar
  mechanism that makes the same files or folders appear at more than one path, including hard links (a file hard-linked
  between checkouts is not treated as shared). Farhelm treats each path as the location it names and need not detect
  aliasing or add complexity to cope with it.
- Completion criteria: SPEC.md "Supported user environments" carries the rule; no code change; remove this feedback file
  and its index entry.
- Execution: `complete`; the SPEC.md rule and the queue removal are in the change that carries these triage decisions.

## update-reports-success-on-hand-started-supervisor.md

- Outcome: `fix code`.
- Assessment: confirmed on current main by code inspection. Update planning accepts any answering supervisor, then
  enables and restarts `farhelm-supervisor.service`; when the running supervisor is not that unit, the unit crash-loops
  on the state lock while the attach step (which checks only that a client reconnected, never the version) reports
  success against the old supervisor.
- Decision: the user chose a fix only if it is super simple: after reconnecting, compare the build version in the
  supervisor's hello with the release just installed and report failure on a mismatch. If that turns out not to be
  trivially simple during planning or execution, abandon it and discard the item instead: all manual management of
  supervisors and systemd units is best effort and developer-facing only (SPEC.md "Supported host setup").
- Completion criteria: Update fails visibly when the supervisor it reconnects to is not the build it installed, with
  focused coverage, and this feedback file and its index entry are removed in the execution change; or, if the gate is
  hit, the item is removed as discarded with the reason recorded.
- Execution: `complete` as a discard; the simplicity gate was hit. The version of the build just installed is known only
  for release payloads: `--payload-dir` payloads are operator-staged, unverified, and carry no version, so comparing the
  hello's build against the helm's own version would falsely fail an Update from a directory of another release, and a
  correct comparison needs a version threaded through every payload source. Per the decision, the item is removed
  without a code change. Change `rqtzywntwlwvzzwzurmtvqsumuopumqv`, bookmark `pr/discard-hand-started-update`, draft PR
  [#1201](https://github.com/scode/farhelm/pull/1201/changes).

## update-identity-none-plan-confirm-disagree.md

- Outcome: `discard`.
- Assessment: the rule mismatch is confirmed by code inspection: planning accepts a supervisor reporting no identity,
  confirmation refuses it whenever an identity was recorded, so such a host would loop on "plan again". It is
  unreachable in the current version: only a supervisor without the state-directory lock can report no identity, such a
  supervisor never serves, and the production hello always sends the supervisor's own identity.
- Decision: the user chose discard solely because the path is unreachable in the current version. If a future change
  lets a serving supervisor report no identity, this mismatch becomes a real bug and the two checks must be made to
  agree.
- Completion criteria: remove the feedback file and its index entry without code or spec changes.
- Execution: `complete`; the feedback file and its index entry were removed with no code or spec change. Change
  `uqozolntwxvzmvywkzpqyrpntklqqrsq`, bookmark `pr/discard-update-identity-none`, draft PR
  [#1210](https://github.com/scode/farhelm/pull/1210/changes).

## payload-dir-must-be-writable.md

- Outcome: `fix code`.
- Assessment: confirmed on current main by code inspection. On every lookup `DirectoryPayloads::path` creates (or chmods
  to 0700) a cache subdirectory inside the operator's `--payload-dir`, now named `.farhelm_extract_tmp`, and writes an
  extracted copy there, so a read-only, root-owned or shared staging directory fails every setup and update.
- Decision: the user chose the fix with a complexity gate: keep extracted copies under the helm's own state directory
  and only read from `--payload-dir`. `--payload-dir` is developer-facing, best-effort functionality, so if the change
  turns out to be big during planning or execution, abandon it and discard the item instead.
- Completion criteria: a read-only `--payload-dir` works for setup and update, with focused coverage, and this feedback
  file and its index entry are removed in the execution change; or, if the gate is hit, the item is removed as discarded
  with the reason recorded.
- Execution: `complete`; the complexity gate was not reached. `DirectoryPayloads` takes its extraction cache as a
  separate path, and production wiring places it at `.farhelm_extract_tmp` under the helm's state directory, so
  `--payload-dir` is only read. Leftover caches older helms made inside a payload directory are left alone. Change
  `kznyorrylmup`, bookmark `pr/payload-dir-read-only`, draft PR
  [#1202](https://github.com/scode/farhelm/pull/1202/changes).

## receiptless-app-bundle-blocks-uninstall.md

- Outcome: `fix code`.
- Assessment: confirmed on current main by code inspection. On macOS `inspect_bundle_at` always inspects
  `~/Applications/Farhelm.app` and any error fails the whole uninstall plan, including the independently verified flat
  binaries. A receipt-less bundle (installers from #310 to #673, or a self-built app) is refused with "rerun the
  installer", which does nothing under `FARHELM_NO_APP_BUNDLE` and otherwise replaces the user's own app. #1076 improved
  only the different-installation case.
- Decision: the user chose the simple fix: when the bundle has no receipt, leave it untouched, say so plainly, and carry
  on removing the rest. Deleting what Farhelm cannot prove it installed stays excluded. Basis: SPEC.md "Concurrent and
  interrupted runs" forbids leaving a state no documented command recovers from.
- Completion criteria: uninstall with a receipt-less bundle removes the verified binaries, leaves the bundle, and
  reports that it did; focused coverage; remove this feedback file and its index entry in the execution change.
- Execution: `complete`; `inspect_bundle_at` reads the app's receipts before its layout checks and returns
  `RetainedWithoutReceipt` when neither the internal nor the pending sibling receipt exists, so a signed or self-built
  app's extra entries do not refuse it either. The preview and the final report both say the app was kept, and
  `docs/install_uninstall.md` describes it. A bundle with a receipt keeps every existing check. Change `oxkmovwvvktv`,
  bookmark `pr/receiptless-bundle-retained`, draft PR [#1203](https://github.com/scode/farhelm/pull/1203/changes).

## no-supervisor-setup-splits-state-dir.md

- Outcome: `discard`.
- Assessment: confirmed on current main by code inspection. With `--no-supervisor`, setup plans only the helm unit and
  never reads an existing setup-written supervisor unit, so a rerun with a different state directory or binary moves the
  helm while that supervisor stays on the old directory, and setup reports success.
- Decision: the user chose discard. `--no-supervisor` exists for supervisor management setup does not do, which is best
  effort and developer-facing.
- Completion criteria: remove the feedback file and its index entry without code or spec changes.
- Execution: `complete`; the feedback file and its index entry were removed with no code or spec change. Change
  `vtkwosozrktxxwsuwwsrzpsyxyrkzvnk`, bookmark `pr/discard-no-supervisor-split`, draft PR
  [#1211](https://github.com/scode/farhelm/pull/1211/changes).

## uninstall-creates-setup-lock-file.md

- Outcome: `discard`.
- Assessment: confirmed on current main by code inspection. Uninstall takes setup's lock even when it has nothing to
  remove, creating `.farhelm-setup.lock` (and possibly the unit directory) on a machine never set up; the file is never
  deleted and not reported. Leaving the lock file in place is deliberate: deleting a path-addressed `flock` file lets a
  waiter hold a lock on the unlinked file while a newcomer creates and locks a fresh one, so two runs would both hold
  "the lock". The finding's "unneeded daemon-reload" is also deliberate, so a retry can finish an earlier failed reload.
- Decision: the user chose discard; the leftover is an empty hidden file and harmless.
- Completion criteria: remove the feedback file and its index entry without code or spec changes.
- Execution: `complete`; the feedback file and its index entry were removed with no code or spec change. Change
  `wytsppnwvwwqxuzpnurkkuuwxxoqmwtz`, bookmark `pr/discard-uninstall-lock-file`, draft PR
  [#1212](https://github.com/scode/farhelm/pull/1212/changes).

## embedded-payload-cleanup-blocks-helm-start.md

- Outcome: `fix code`.
- Assessment: confirmed on current main by code inspection. Failures in the startup removal of the retired
  `<state>/embedded-payloads` cache (the `remove_dir_all`, the metadata inspection and the `--payload-dir` alias check)
  return `Err`, which propagates through `production_payloads_with_key`, `ProvisioningService::production` and
  `AppState::new` and aborts helm startup. Only the "uncertainty never resolves toward deletion" intent is documented.
- Decision: the user chose the fix: keep the conservative never-delete-when-unsure behavior, but log a warning naming
  the path and continue startup whenever the cleanup fails or cannot decide.
- Completion criteria: a helm whose old cache cannot be removed or inspected still starts and warns; focused coverage;
  remove this feedback file and its index entry in the execution change.
- Execution: `complete`; `production_payloads_with_key` logs a warning naming the path and continues when
  `remove_leftover_embedded_payloads` fails, which it only does before deleting anything it could not judge, so the
  never-delete-when-unsure behavior is unchanged. Change `yvppqqtlwxly`, bookmark `pr/embedded-cleanup-nonfatal`, draft
  PR [#1204](https://github.com/scode/farhelm/pull/1204/changes).

## reach-misreads-escaped-xdg-config-home.md

- Outcome: `fix code`.
- Assessment: confirmed on current main by code inspection; the trigger is documented in systemctl(1) on systemd 255,
  which prints values with whitespace or shell-special characters as `VARIABLE=$'value'`. Two sites parse that output:
  the remote reach script (`provisioning/backend.rs` around 1834-1840), which then refuses with a false "relative
  XDG_CONFIG_HOME" reason, and `manager_unit_dir` in `crates/farhelm/src/setup.rs`, which strips only plain quotes, so
  local `farhelm helm setup` refuses with a wrong message and `farhelm uninstall` can look in the wrong unit directory
  and leave units behind.
- Decision: the user chose to replace parsing of human/shell-oriented output with a structured query at both sites:
  `busctl --user get-property org.freedesktop.systemd1 /org/freedesktop/systemd1 org.freedesktop.systemd1.Manager
  Environment --json=short`
  (raw strings, systemd 240 and later), parsed in Rust.
- Completion criteria: both sites read the manager's environment through the structured query, a config path containing
  a space resolves correctly for remote setup, local setup and uninstall, focused coverage, and this feedback file and
  its index entry are removed in the execution change.
- Execution: `complete`; both sites read the manager's environment through the agreed `busctl` JSON query, parsed by the
  shared `farhelm_helm::units::ManagerEnvironment`. Remote: the reach check fetches only the manager's `XDG_CONFIG_HOME`
  entry, filtered on the host from `busctl`'s pretty JSON (one element per line) so no other environment value or size
  reaches the helm, in a first command and passes the unit directory chosen in Rust to the existing reach script, whose
  record format is unchanged. Local: `manager_unit_dir` in setup.rs, used by setup and uninstall. Execution added one
  thing the decision did not name, recorded as a DECISION: `busctl --user` needs a D-Bus user bus, which some minimal
  hosts lack even though `systemctl --user` works, so when `busctl` cannot answer both sites fall back to the
  `show-environment` block, parsed in Rust; a value there in the escaped `$'…'` form is refused with a message naming
  it, and every plain value keeps working as before. Change `qoqxtqrksrlylqlqqzwkptpunnyyqwpk`, bookmark
  `pr/manager-environment-busctl`, draft PR [#1205](https://github.com/scode/farhelm/pull/1205/changes).

## setup-build-tree-heuristic-misfires.md

- Outcome: `fix code`.
- Assessment: confirmed on current main by code inspection. `looks_like_a_build_tree` refuses any executable path with a
  component named exactly `target`, and treats an empty `TMPDIR` as a prefix of every path; the refusal is a hard
  `bail!` with no override. The guard exists to stop units pointing at a `cargo build` output or a temp copy that later
  vanishes.
- Decision: the user chose to fix only the `target` half: match Cargo's actual layout (`target/debug/`,
  `target/release/`, `target/<triple>/{debug,release}/`) instead of any `target` component. An empty or otherwise broken
  `TMPDIR` is unsupported and stays as it is.
- Completion criteria: installs under paths such as `/home/target/.local/bin` or `/opt/target/bin` are accepted, Cargo
  build outputs are still refused, focused coverage, and this feedback file and its index entry are removed in the
  execution change.
- Execution: `complete`; `looks_like_a_build_tree` matches a `target` component only when followed by `debug` or
  `release`, directly or after one target-triple component; the temporary-directory half is unchanged. Change
  `tllxylyvlvsnmszxyuukulpvrttsztot`, bookmark `pr/setup-cargo-layout-only`, draft PR
  [#1206](https://github.com/scode/farhelm/pull/1206/changes).

## tab-session-token-in-tmux-argv.md

- Outcome: `discard`.
- Assessment: confirmed on current main by code inspection. `tab_environment` adds `FARHELM_SESSION_TOKEN`, and
  `new_window` passes it as a `-e NAME=value` argument to a short-lived tmux client, so it is briefly readable by other
  accounts through the process list. The token only unlocks the supervisor socket, which is 0600 in a 0700 directory, so
  no working exploit is known; it is a consistency and defense-in-depth gap against the agent launch's 0600 spec file.
- Decision: the user chose discard.
- Completion criteria: remove the feedback file and its index entry without code or spec changes.
- Execution: `complete`; the feedback file and its index entry were removed with no code or spec change. Change
  `mspynmstlpunoolzkllnlsutqvkolsuv`, bookmark `pr/discard-tab-token-argv`, draft PR
  [#1213](https://github.com/scode/farhelm/pull/1213/changes).

## send-upload-ignores-cancellation.md

- Outcome: `fix code`.
- Assessment: confirmed on current main by code inspection. `send_upload` is a bare `priority.send(...).await` used for
  the per-chunk ack and other control frames; with the 32-frame queue full (up to 8 transfers with about 16 acks in
  flight each, and a viewer that stopped reading) the transfer cannot observe a delete or archive signal, and Delete's
  wait on `finished` has no bound of its own, so it stalls until the 60 s writer-stall timeout. Every other wait in the
  transfer already observes the signal.
- Decision: the user chose the fix: race the send against the transfer's signal receiver, as the other waits do.
- Completion criteria: Delete of a session with a transfer blocked on a full queue proceeds promptly; focused regression
  coverage; remove this feedback file and its index entry in the execution change.
- Execution: `complete`; every message a transfer sends goes through one rule (`reply` and
  `send_upload_unless_cancelled` in uploads.rs): while the transfer can still be cancelled, a send waits for room but
  gives way to the cancellation signal; once a cancellation has been taken, its goodbye (abort notice, answers to queued
  commits, the commit's or the begin's cancellation reply) is queued only if there is room. That covers the per-chunk
  ack the finding named and the other sends review found with the same stall: `UploadStarted`, cancellation during a
  chunk write, before staging and at commit, and the success reply after publication. Change `syplqxsvvurm`, bookmark
  `pr/upload-send-cancellable`, draft PR [#1207](https://github.com/scode/farhelm/pull/1207/changes).

## pi-resume-downgrade-on-read-error.md

- Outcome: `other`.
- Assessment: confirmed by current-code inspection at 35076d8. `verify_report_only_resume` still folds a read error
  (`Err`) into the not-verified arm and durably replaces the stored locator with a fileless token, so a transient read
  error of the session file permanently withdraws the Resume offer for that Pi or OMP session. The match is FILTER.md
  filter "Rare edge cases in harnesses without first-class support": Pi and OMP are not first-class harnesses, the
  trigger (a transient read error or torn read at restart) is rare, and the whole consequence is a missing Resume offer
  for the affected session. Nothing is resumed into the wrong conversation, and the conversation file on disk is intact.
- Decision: skipped under the triage rule for findings covered by a review filter. On 2026-10-01 the user also decided
  that Claude Code and Codex are the first-class harnesses, that support for other harnesses is intentionally partial,
  and that gaps there are expected and not worth raising in review. This item is filtered, not fixed.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage.

## desktop-reauth-remount-loses-action.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at 35076d8. `retry_desktop_request` (`crates/farhelm-ui/src/api.rs`)
  calls `require_desktop_webview_reauth()` before sending the retried request. That bumps the generation the desktop
  bootstrap gate watches, and the gate stops rendering `AppBody` (`crates/farhelm-ui/src/auth.rs`), which drops the
  component-scoped task awaiting the retry. The first Delete, Stop, restart, rename or create after a token rotation in
  the desktop app therefore ends with an unknown outcome and no message. The cancellation of component-scoped tasks on
  unmount is the reviewer's account of Dioxus behavior and was not reproduced at runtime.
- Decision: the user chose (b) from the sign-in product question of 2026-10-01, stated as a spec principle. The desktop
  app is the primary supported surface. Signing in again after a token rotation may reset the page and lose open forms,
  dialogs and drafts, and browser sign-in friction is acceptable; no significant complexity is spent preserving UI state
  across it. What still holds: an action the user started is never lost silently (it completes and reports, or reports
  that its outcome is unknown), sign-in recovery never crashes the window or leaves it dead, and a failed desktop
  re-sign-in can be retried. The principle may be revisited later.
- Completion criteria: add the principle to SPEC.md (and SPEC_impl.md if the mechanism needs recording). Make the
  desktop credential refresh not cancel the request it retries, for example by deferring the webview re-authentication
  until the retried response is in hand, so the triggering action reports its outcome. Add regression coverage where the
  desktop seam allows it. Remove this feedback file and its index entry.
- Execution: complete: change `owzunvypkmpz`, bookmark `triage-1001/01-desktop-reauth-action`, PR
  https://github.com/scode/farhelm/pull/1357.

## desktop-reauth-failure-dead-end.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at 35076d8. The only caller of `require_desktop_webview_reauth()` is
  the native 401 retry path in `crates/farhelm-ui/src/api.rs`, and the bootstrap gate's failure branch renders a single
  error paragraph with no retry control or timer. A transient failure of the desktop webview's re-authentication
  therefore leaves the window on that error until the app is relaunched. The trigger (a transient failure coinciding
  with a token rotation) is plausible but not reproduced.
- Decision: under the sign-in principle recorded for `desktop-reauth-remount-loses-action.md`, a failed desktop
  re-sign-in must be retryable. Keep it simple.
- Completion criteria: give the failure state a way out (a Retry control that restarts the authentication, and/or an
  automatic retry with backoff for transient failures), keeping a terminal error only for causes that cannot be retried.
  Remove this feedback file and its index entry.
- Execution: complete: change `zlwstzouwypu`, bookmark `triage-1001/02-desktop-reauth-retry`, PR
  https://github.com/scode/farhelm/pull/1358.

## seen-toggle-report-panics-after-unmount.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at 35076d8. The manual read/unread toggle's completion report calls
  `errors.write()` on the session list's signal (`crates/farhelm-ui/src/list/view.rs`, around line 2649) from a task
  that outlives the list. If the list has unmounted (desktop re-sign-in, or the browser token prompt), the write panics.
  The panic follows from dioxus-signals' `write()` unwrapping `try_write()`, per the reviewer; not reproduced at
  runtime.
- Decision: under the sign-in principle, recovery must never crash the window. The fix is small.
- Completion criteria: make the report tolerate a dropped signal (`try_write()` and drop the update), and document on
  the report type that a report can run after its caller unmounted. Remove this feedback file and its index entry.
- Execution: complete: change `ymzupppnnrmx`, bookmark `triage-1001/03-seen-toggle-no-panic`, PR
  https://github.com/scode/farhelm/pull/1359.

## hosts-panel-leaks-page-lock.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at 35076d8. The hosts panel's shared request runner still takes the
  page-wide operation lock with a bare `ops.claim()` (`crates/farhelm-ui/src/hosts.rs:879`) and releases it by hand at
  the end of a component-scoped task. A browser token prompt raised by another request unmounts the panel and drops the
  task, so after re-login every lock-gated control silently refuses until reload. Browser only: in the desktop app,
  re-sign-in remounts `AppBody`, which owns the lock, so the lock is rebuilt.
- Decision: the user chose to fix it because the fix is easy. The trigger is rare and browser-only, so no significant
  complexity is to be spent on it.
- Completion criteria: switch the runner to the self-releasing `claim_guard()` form and move the guard into the spawned
  task. Remove this feedback file and its index entry.
- Execution: complete: change `vwklsuxsqrot`, bookmark `triage-1001/04-hosts-panel-lock`, PR
  https://github.com/scode/farhelm/pull/1360.

## profile-popup-leaks-page-lock.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at 35076d8. The profile save and delete handlers still take the page
  lock with a bare `ops.claim()` (`crates/farhelm-ui/src/profiles.rs:1437` and `:1544`), with the same browser re-login
  failure as `hosts-panel-leaks-page-lock.md`. Browser only.
- Decision: as for `hosts-panel-leaks-page-lock.md`: fix, because it is easy; no significant complexity.
- Completion criteria: switch both handlers to `claim_guard()` and move the guard into the spawned task, taking it after
  the save handler's local validation early returns (or letting those returns drop it). Remove this feedback file and
  its index entry.
- Execution: complete: change `tnvvxtkkpzrx`, bookmark `triage-1001/05-profile-popup-lock`, PR
  https://github.com/scode/farhelm/pull/1361.

## codex-last-option-reads-idle.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection against the real capture
  `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/waiting-trust.txt`. Its dialog ends with
  `› 1. Trust and continue`, `2. Back to Agent Command Center` and the `enter continue · esc back` footer.
  `codex_dialog_may_be_open` (`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs`) requires a numbered option
  below the `›` row, so with the highlight on option 2 it decides no dialog is open and never checks the footer. The
  highlighted-last-option screen itself is not captured, but it follows from the captured dialog.
- Decision: the user decided on 2026-10-01 that Claude Code and Codex are the first-class harnesses. A clear, definite
  gap in their activity detection is fixed. Activity tracking, session tracking and similar integration features for
  other harnesses are intentionally partial; gaps there are expected and not worth raising in code review. This may
  improve later. That principle goes into SPEC.md with this item.
- Completion criteria: state the first-class principle in SPEC.md (FILTER.md's harness filter is review-only and does
  not substitute for it). Accept a numbered option above the `›` row as evidence of a menu, or require every row from
  `›` down to be an option or a known footer. Add a fixture-derived test with the last option highlighted. Remove this
  feedback file and its index entry.
- Execution: complete: change `vwtnvmzuzoks`, bookmark `triage-1001/06-codex-last-option`, PR
  https://github.com/scode/farhelm/pull/1362.

## codex-working-backstop-never-matches.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection against the real captures
  `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/working-*.txt`. Each ends with two footer rows (the
  model/context line, then `← for agents · ? for shortcuts`). `codex_composer_bounds`
  (`crates/farhelm-supervisor/src/agent_kind/mod.rs`) requires the second-to-last line to be blank padding, so it
  returns `None` on every real working screen, and the screen-text "Working" backstop never fires.
- Decision: a clear, definite gap in a first-class harness's activity detection, so it is fixed under the principle
  recorded for `codex-last-option-reads-idle.md`.
- Completion criteria: locate the composer by its `›` prompt row followed only by blank or indented rows, allow the
  blank rows the real fixtures show between the status line and the prompt, and test against the real `working-*.txt`
  fixtures with an empty title. Remove this feedback file and its index entry.
- Execution: complete: change `orrsomsxwzzw`, bookmark `triage-1001/07-codex-working-widget`, PR
  https://github.com/scode/farhelm/pull/1363.

## claude-last-option-reads-idle.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection against the real capture
  `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/waiting-question.txt`. Option 4 (`Chat about this`)
  sits directly under a `─` rule, and the highlighted option renders as `❯ 1. Tea` at column 0. `claude_input_box_rule`
  (`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs`) takes the last `❯` row under a rule as the input box,
  and its comment assumes menus never draw `❯` under a rule, which this screen contradicts. The one inferred premise is
  that a highlighted option 4 renders like the captured highlighted option 1; no capture shows it.
- Decision: treated as a clear, definite gap in a first-class harness under the principle recorded for
  `codex-last-option-reads-idle.md`.
- Completion criteria: require the input box's full shape (rule, `❯` row, closing rule), or check for the dialog footer
  before accepting a box candidate. Add the highlighted-last-option variant as a fixture-derived test, and correct the
  comment's assumption. Remove this feedback file and its index entry.
- Execution: complete: change `vnqnvtnwvllq`, bookmark `triage-1001/08-claude-last-option`, PR
  https://github.com/scode/farhelm/pull/1364.

## claude-spinner-window-too-short.md

- Outcome: `discard`.
- Assessment: unverified. The claim depends on Claude Code drawing a task list of five or more rows between its spinner
  and its input box; no captured screen shows that layout.
- Decision: only clear, definite gaps in Claude Code or Codex activity detection are fixed. This one rests on an
  uncaptured layout, so it is discarded for now.
- Completion criteria: remove this feedback file and its index entry, with no code or spec change.
- Execution: complete: change `vkyuyuvypkoo`, bookmark `triage-1001/09-discard-spinner-window`, PR
  https://github.com/scode/farhelm/pull/1365.

## claude-spinner-rejects-multiword.md

- Outcome: `discard`.
- Assessment: the code does reject multi-word activity text (a unit test pins it), but the claim that Claude Code draws
  a multi-word spinner line such as `✻ Compacting conversation… (…)` is unverified; no captured screen contains it.
- Decision: not a clear, definite gap, so it is discarded for now, as for `claude-spinner-window-too-short.md`.
- Completion criteria: remove this feedback file and its index entry, with no code or spec change.
- Execution: complete: change `xzmwpqzsursz`, bookmark `triage-1001/10-discard-spinner-multiword`, PR
  https://github.com/scode/farhelm/pull/1366.

## pi-reporter-asset-not-renamed.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at 35076d8. The Pi asset is still published as
  `integrations/pi/farhelm-conversation-v1.ts` (`crates/farhelm-supervisor/src/pi_extension.rs`), its source changed in
  ce5ea14 (#811, first released in v0.13.0) without a rename, and `materialize_asset` refuses an existing file whose
  bytes differ. Every host that ran Pi before v0.13.0 launches Pi without the extension: no conversation capture, so no
  Resume, and no `farhelm agent` instructions pointer. FILTER.md's harness filter does not apply, because the trigger is
  deterministic rather than rare.
- Decision: fix the code and add the test. The user is the only existing user, so no special handling, migration or
  cleanup for hosts already carrying the stale file is wanted.
- Completion criteria: publish the current Pi asset under a new file name (for example `farhelm-conversation-v2.ts`),
  update any spec or doc text naming the Pi file, and add a test that ties each published asset's bytes to its file name
  (a pinned hash or a content-derived name), so a content change without a rename fails. Remove this feedback file and
  its index entry.
- Execution: complete: change `nxkzooykszmp`, bookmark `triage-1001/11-pi-asset-v2`, PR
  https://github.com/scode/farhelm/pull/1367.

## checkout-preview-blocks-read-loop.md

- Outcome: `fix spec`.
- Assessment: not re-verified in code. As described, the GitHub checkout preview scans the checkout folder inline on the
  supervisor connection's read loop. It stalls other sessions' terminals only when that scan is slow (a slow network
  share or a very large folder); on a healthy host it is a short scan.
- Decision: the user decided on 2026-10-01 that a slow host is slow. No complexity is spent compensating for a slow
  remote host, or a slow helm machine, whether the slowness is in the filesystem or elsewhere, as long as the helm
  itself does not freeze or become unusable. Within one host, separate sessions should still stay independent, because
  even a healthy host can take a while over some operations: terminal input being blocked behind a large operation such
  as a git clone is not acceptable. This item's whole consequence is slowness, so the principle covers it. The principle
  goes into SPEC.md with this item.
- Completion criteria: write the principle into SPEC.md next to "Healthy local filesystems" and "Waiting between
  operations on one host", keeping the existing requirement that one session's long operations never block another
  session's terminal I/O. Remove this feedback file and its index entry, with no code change.
- Execution: complete: change `rlwwkmonmqvk`, bookmark `triage-1001/12-slow-host-spec`, PR
  https://github.com/scode/farhelm/pull/1368.

## repo-search-blocking-scan.md

- Outcome: `fix spec`.
- Assessment: not re-verified in code. As described, GitHub repository search scans the checkout folder with blocking
  calls on async workers; it can stall the supervisor only when listing or stat-ing that folder is slow.
- Decision: covered by the slow-host principle recorded for `checkout-preview-blocks-read-loop.md`.
- Completion criteria: if that item's spec change has landed, confirm it covers this case. Otherwise land the principle
  here. Remove this feedback file and its index entry, with no code change.
- Execution: complete: change `yppsqvpowlqk`, bookmark `triage-1001/13-repo-search-covered`, PR
  https://github.com/scode/farhelm/pull/1369. No spec text needed and no review gate: the Slow hosts section added by PR
  1368 names repository search and covers the finding.

## delete-holds-attachments-lock-through-archive.md

- Outcome: `fix spec`.
- Assessment: confirmed by current-code inspection at 35076d8, with the materiality unmeasured. Delete takes the
  host-wide terminal attachments lock (`crates/farhelm-supervisor/src/service/teardown.rs`, around line 450) only after
  the process sweep and its grace periods. It holds the lock through stopping forwarders, the tmux kill, quarantine
  renames, checkout archive moves with parent-directory fsyncs, and the final database commit, then releases it around
  line 833. All of that is short local work on a healthy disk, not a long operation like a clone. SPEC.md's "Waiting
  between operations on one host" currently says terminal input must not wait on a delete of any session, which this
  contradicts as written.
- Decision: under the slow-host principle recorded for `checkout-preview-blocks-read-loop.md`, brief, bounded local work
  under the shared terminal lock is acceptable, and a slow disk is just slow. Long operations must still not block other
  sessions' terminals.
- Completion criteria: clarify "Waiting between operations on one host" so that terminal I/O may wait on brief, bounded
  local work (such as a delete's renames, fsyncs and database commit), but never on long operations or kill grace
  periods. Remove this feedback file and its index entry, with no code change.
- Execution: complete: change `wprvzxpmuyww`, bookmark `triage-1001/14-delete-lock-spec`, PR
  https://github.com/scode/farhelm/pull/1370.

## restart-cwd-lossy-non-utf8.md

- Outcome: `fix spec+code`.
- Assessment: not re-verified in code beyond the finding's trace. Create records the canonical working directory with
  `to_string_lossy`, and the restart/create-retry check re-resolves it the same way and launches into the lossy text.
  When the real target is not valid UTF-8, tmux falls back to `$HOME` and the restart reports success. The tmux fallback
  is tmux behavior, cited by the reviewer.
- Decision: the user decided on 2026-10-01 that paths that are not valid UTF-8 are not supported. They must be refused
  clearly at every surface, and a path must never be silently corrupted. That principle goes into SPEC.md with this
  item. A complexity gate applies; see `non-utf8-farhelm-path-breaks-launch.md` for the sweep.
- Completion criteria: state the principle in SPEC.md. Use strict conversion for the canonical working directory at
  create and in the restart/retry identity check, and refuse with a clear message. Add a test with a symlink to a
  non-UTF-8 directory. Remove this feedback file and its index entry.
- Execution: complete: change `uurollwwuosl`, bookmark `triage-1001/15-non-utf8-cwd`, PR
  https://github.com/scode/farhelm/pull/1371.

## non-utf8-farhelm-path-breaks-launch.md

- Outcome: `fix code`.
- Assessment: not re-verified in code beyond the finding's trace. `window_command` builds the launch shell command with
  lossy conversions of the farhelm binary path and the launch-spec path, so a non-UTF-8 binary path or state directory
  makes every launch exec a nonexistent file, while the field docs and the startup warning claim only degraded capture.
- Decision: under the non-UTF-8 principle recorded for `restart-cwd-lossy-non-utf8.md`. The user chose to include a
  sweep of the codebase's remaining lossy path conversions in this work, with a complexity gate. Change a site only when
  refusing or blocking is clearly right and the fix is simple. Leave a site alone when that is not clear (for example,
  lossy text in logs or display is usually fine). Anything non-trivial goes back to the user for judgment rather than
  being fixed under the literal "never" rule.
- Completion criteria: refuse a non-UTF-8 farhelm binary path or state directory at supervisor startup with a message
  naming the path, and correct the misleading docs and warning. Sweep the remaining lossy path conversions under the
  gate above. Report the sites changed, the sites left alone with the reason, and any non-trivial sites for the user's
  decision. Add a test covering the startup refusal. Remove this feedback file and its index entry.
- Execution: complete: change `yunmwrpmyoxz`, bookmark `triage-1001/16-non-utf8-startup`, PR
  https://github.com/scode/farhelm/pull/1372. Two sites were left for the maintainer, who decided them on 2026-10-01,
  and the same change carries both: a captured conversation record path that is not valid UTF-8 is not stored (the
  identity is kept, with no location hint; `store.rs`, `record_captured_conversation`), and the program lookup skips a
  `systemd-run` or `systemctl` under a non-UTF-8 directory and keeps searching, so a copy that only exists there counts
  as absent (`scope.rs`, `resolve_program_in`). The PR description lists every site.

## sftp-overall-deadline-fails-slow-links.md

- Outcome: `fix spec`.
- Assessment: already fixed in code on main. #888 (4cb7fd7) replaced the fixed overall deadline with a stall timeout
  that renews on verified growth of the remote temporary file (`TRANSFER_IDLE_TIMEOUT`), and #1143 (5d9ff14) replaced
  the sftp upload with `ssh … cat` for every payload upload. Add and update share the same upload step. The specs do not
  yet state the rule.
- Decision: the user wants SPEC.md to require that payload transfers during both host add/install and update have no
  fixed overall timeout and time out only on stalls, matching the download path.
- Completion criteria: add that requirement to SPEC.md. No code change is needed unless execution finds a transfer on
  either path that still has a fixed overall deadline. Remove this feedback file and its index entry.
- Execution: complete: change `luxkrnwlmlwp`, bookmark `triage-1001/17-transfer-stall-spec`, PR
  https://github.com/scode/farhelm/pull/1373. Both paths checked: no fixed overall deadline remains, so no code change.
  The window before the host's temporary file appears has no deadline of its own; the maintainer accepted that on
  2026-10-01 (ordinary ssh behavior bounds it), and SPEC.md's stall section records the acceptance.

## plain-retry-erases-pending-fresh-window.md

- Outcome: `discard`.
- Assessment: confirmed by current-code inspection at a1655f0. `nudge_now` (`crates/farhelm-helm/src/manager.rs`)
  overwrites `Nudge::fresh_window` on every send, so a plain retry (the Retry button, or the reconnect after an
  adoption) that lands before the actor consumes a pending fresh-window nudge (from a destination change or
  provisioning's attach) downgrades the fast reconnect ladder to a single probe plus the roughly 45-second re-probe
  wait. The trigger is a human click inside a window of milliseconds. The consequence is a slower reconnect that heals
  on the next attempt or another Retry. A nudge carries only a revision counter and that flag, and the destination
  dialed comes from the host's row, so the downgrade never changes which machine is dialed.
- Decision: the user chose to discard it, on the condition that it cannot cause a connection to the wrong host. That
  condition holds per the assessment. The separate wrong-machine race after a retarget is
  `retarget-race-republishes-old-client.md`, triaged on its own.
- Completion criteria: remove this feedback file and its index entry, with no code or spec change.
- Execution: complete: change `zznrvwypywxo`, bookmark `triage-1001b/01-discard-retry-fresh-window`, PR
  https://github.com/scode/farhelm/pull/1374.

## unvalidated-state-dir-on-probe.md

- Outcome: `discard`.
- Assessment: already fixed. 70f6786 (#1071, "fix: refuse unusable remote state directories when registering hosts")
  added the `remote_state_dir_is_usable` refusal to all three registration paths, including `register_probed_ssh_host`
  (`crates/farhelm-helm/src/store.rs`). The check runs before the transaction, so it covers both the insert and the
  converge branch the finding named.
- Decision: already fixed; recorded as `discard` without asking the user, per the triage rule for findings already fixed
  on main.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage.

## event-feed-liveness-postponed-by-revisions.md

- Outcome: `fix code`.
- Assessment: mechanism confirmed by current-code inspection at a1655f0; consequence overstated. In `serve_events`
  (`crates/farhelm-helm/src/events.rs`), every successful revision write resets the `idle` timer, including while
  `awaiting_liveness` is set, so on a fleet that changes more often than every 30 seconds an unanswered keepalive Ping
  is never judged. That contradicts the module docs' promise that a vanished subscriber costs one idle interval, at most
  two. The pile-up to the 64-subscriber cap is implausible in practice: a peer that stops acknowledging while the helm
  (or the forwarding sshd) keeps sending is dropped by the kernel's retransmission timeout after roughly 15 to 30
  minutes (standard TCP behavior, not reproduced here), so reaching the cap would need about 64 vanished clients inside
  that window. Even at the cap, a refused tab falls back to the 3-second poll (`crates/farhelm-ui/src/feed.rs`) and
  stays correct; the desktop app is affected only if its embedded helm's port is forwarded.
- Decision: the user chose to fix it because the fix is trivial.
- Completion criteria: stop a revision write from postponing a pending liveness check (skip the `idle` reset while
  `awaiting_liveness` is true, or keep a separate liveness deadline), so a vanished subscriber is dropped within the
  documented bound; extend the existing keepalive test to cover revisions arriving while a Ping is unanswered. Remove
  this feedback file and its index entry.
- Execution: complete: change `zqknrkwkyrtu`, bookmark `triage-1001b/02-event-feed-liveness`, PR
  https://github.com/scode/farhelm/pull/1375. A write no longer restarts the idle window at all, before or after the
  Ping (review finding: resetting only while no Ping was outstanding still kept a busy fleet from ever pinging).

## sink-shutdown-retries-forever-after-delete.md

- Outcome: `fix code`.
- Assessment: confirmed for Delete by current-code inspection at a1655f0; the reviewer's tmux 3.7c reproduction (2 of 10
  concurrent trials) was not rerun. Delete's teardown (`crates/farhelm-supervisor/src/service/teardown.rs`) drops each
  attachment's sink reference after joining its forwarder and goes on to `kill_session` without waiting for the sink's
  background shutdown. If tmux destroys the session first, `SessionSink::shutdown`
  (`crates/farhelm-supervisor/src/tmux/sink.rs`) fails its output-off handshake while the client process is still alive
  (tmux holds its exit until queued output is drained, which nobody does any more), and
  `shutdown_session_sink_until_safe` (`crates/farhelm-supervisor/src/service/terminals.rs`) retries forever, backing off
  to 5 seconds. The consequence is a leaked tmux control client and a `refresh-client` spawn every 5 seconds until the
  supervisor restarts, per unlucky delete of a session with more than about 64 KiB of unread pane output. Not confirmed:
  the finding's more serious claim that the same race on a restart path that kills and recreates a tmux session under
  the same name leaves the recreated session unattachable; no such restart path was found at a1655f0 (restart reuses the
  tmux session), so that part may be outdated.
- Decision: the user chose `fix code` with the recommended shape: Delete waits for the sink's orderly shutdown to finish
  before killing the tmux session, so the output-off handshake always runs while the session still exists. The
  alternative of closing and draining after tmux's "can't find client" answer was not chosen, because it changes the
  crash-avoidance handshake itself.
- Completion criteria: make Delete await the last sink's reap before `kill_session`, keeping the existing lock and
  ordering contracts (see SPEC.md "Waiting between operations on one host" and whatever the triage plan for
  `delete-holds-attachments-lock-through-archive.md` lands there). Add a regression test that a delete racing a busy
  sink leaves no sink in a perpetual retry. If a restart path that kills and recreates a session under the same name
  turns out to exist, give it the same ordering. Remove this feedback file and its index entry.
- Execution: complete: change `lluxuuzmvmut`, bookmark `triage-1001b/06-delete-waits-for-sink`, PR
  https://github.com/scode/farhelm/pull/1379. No restart path kills and recreates a session under the same name (restart
  reuses the tmux session), so only Delete changed. Remaining gap: Delete cannot wait for the shutdown of an output
  client whose record an earlier failure already replaced. The maintainer accepted that gap on 2026-10-01 (not to be
  fixed), and the code documents it.

## input-client-notifications-pile-up.md

- Outcome: `other`.
- Assessment: confirmed by current-code inspection at a1655f0; the memory measurements are the reviewer's. The input
  client (`crates/farhelm-supervisor/src/tmux/input.rs`) attaches with `-f no-output`, which suppresses pane output but
  not tmux's server-wide notifications, and its stdout is read only inside `InputClient::send`. A terminal that stays
  attached without being typed into never reads them, and once the 64 KiB pipe fills tmux queues the rest in its own
  memory without bound (reviewer: 1.7 MB to 8.2 MB under 80,000 rename notifications on tmux 3.7c). The growth is
  self-limiting in practice: any keystroke reads everything queued up to its reply, and a reattach replaces the client,
  so only a long-idle attached terminal accumulates anything, at roughly a few hundred bytes per notification. A fix
  would need a background reader feeding replies to `send` over a channel, redesigning a deliberately synchronous part
  of the keystroke path.
- Decision: the user chose not to fix it and to document it in `BUGS.md` as a known bug that is not planned to be fixed.
- Completion criteria: add a `BUGS.md` entry in that file's style: what the user can notice (tmux server memory growing
  while terminals stay open for long periods without typing, and the first keystroke reading the backlog), the mechanics
  in brief, how sure we are (the reviewer's measurements; not reproduced in triage), and why it is not being fixed
  (bounded in practice, reset by any keystroke or reattach, and a fix would complicate the keystroke path). Remove this
  feedback file and its index entry.
- Execution: complete: change `twknzzlmoozz`, bookmark `triage-1001b/03-bugs-input-clients`, PR
  https://github.com/scode/farhelm/pull/1376.

## adopt-checks-current-row-not-dialed.md

- Outcome: `other`.
- Assessment: the code gap is confirmed at a1655f0, but its realistic trigger was closed after the reviewed commit.
  `ConnectionManager::adopt` (`crates/farhelm-helm/src/manager.rs`) still builds its `DialedAs` guard from the manager's
  current row, and `HostState::IdentityMismatch` carries no record of the configuration the mismatch was observed under.
  The race the finding relies on, a mismatch from the old destination published after a retarget, was fixed by 411302e
  (#920, merged 2026-09-25, after reviewed commit 2b597e9): `take_settled_outcome` drops a settled dial result when a
  retarget nudge is pending. What remains is the gap between that check and the state publication, and a stale prompt
  published there is replaced within milliseconds when the actor sees the nudge and dials the new destination; the user
  would have to click Adopt inside that. The worst case is a wrong identity recorded on the entry and its cached
  sessions purged; the cache refills, the next contact with the real destination raises a fresh mismatch prompt, and
  nothing connects to the wrong machine. The reverse direction (a genuine adoption refused as `StaleAttempt` after a
  failed reconcile) fails safely until the next successful reconcile.
- Decision: the user chose to discard the finding and to record the class in `review_feedback_queue/FILTER.md`, so
  similar findings stay out of the queue. Superseded in part on 2026-10-01: asked during execution, the user chose to
  widen the filter so that clearing convenience history (remembered suggestions that only pre-fill the new-session
  dialog, such as recent setups and used folders) also counts as recoverable.
- Completion criteria: add a filter to `review_feedback_queue/FILTER.md` for findings whose trigger needs a person to
  act inside a window of about a second or less that opens and closes on its own, and whose whole consequence is
  recoverable through ordinary use (the wrong state is replaced or asked about again on the next connection, refresh or
  prompt, and anything cleared is a cache Farhelm refills). Keep FILTER.md's standard exclusions: loss of user data,
  credentials, processes or other user-owned work; connecting to, sending an operation to, or acting on the wrong
  machine or session; a wrong state that persists with no ordinary way back; and any security or trust-boundary
  consequence. Remove this feedback file and its index entry. Superseded in part on 2026-10-01: "anything cleared is a
  cache Farhelm refills" widens to "a cache Farhelm refills, or convenience history" as defined in the Decision.
- Execution: complete: change `uyvlqzqposnk`, bookmark `triage-1001b/04-filter-sub-second-races`, PR
  https://github.com/scode/farhelm/pull/1377. Adoption also deletes the replaced install's launch and folder history,
  which Farhelm does not refill, so the filter as first agreed did not cover this finding. Asked about it, the
  maintainer chose on 2026-10-01 to widen the filter to accept losing convenience history, so it now covers this finding
  too.

## identity-mismatch-never-becomes-duplicate.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at a1655f0. `record_first_contact`
  (`crates/farhelm-helm/src/store.rs`) returns `Mismatch` whenever the row's recorded identity differs from the reported
  one and consults `claimant_of` only when nothing is recorded, so a row that has ever recorded an identity can never be
  classified as a duplicate. It gets the adopt prompt instead, and `adopt_identity` always refuses with
  `IdentityClaimed`; nothing ever switches the host to the duplicate state, despite that error's docs saying the host
  should then re-render as one. Ordinary triggers: retargeting an entry onto a machine another entry manages, or
  re-adding a reinstalled host as a new entry while the old entry still points at it. Context gathered in triage:
  nothing is ever merged today; the duplicate state already connects nothing and asks the user to edit or remove an
  entry. Its main extra machinery is the 45-second automatic re-check of the registry (the duplicate branch at the top
  of the actor loop in `crates/farhelm-helm/src/manager.rs`, with its own retarget-race handling).
- Decision: the user wants the simplest safe handling of this rare case, not a polished feature, and agreed to this
  shape. Replace SPEC.md's "Two destinations reaching the same identity are the same host, shown once" with a plain
  rule: an entry that reaches a machine another entry already holds connects nothing, says which entry holds it (by
  name), and tells the user to remove that entry or change this one's destination and then press Retry; Farhelm never
  connects two entries to one machine and never resolves this on its own. Check "held by another entry" before comparing
  with the remembered identity, so the un-adoptable adopt prompt cannot occur. Drop the duplicate state's automatic
  re-check: like identity-mismatch, it stays frozen until Retry, an edit of the entry, or a helm restart. Keep the
  schema rule that one identity belongs to at most one entry, adoption for reinstalled hosts, and the existing refusal
  when a probe would register an already-held identity.
- Completion criteria: update SPEC.md (and SPEC_impl.md's host-state and cadence text, which describe the duplicate
  re-check) to the rule above. In `record_first_contact`, resolve a claimant before the recorded-identity comparison and
  return `Collision`. Remove the duplicate re-check timer and its race handling, making the duplicate state a frozen
  state resolved by Retry, an edit, or a restart. Make the duplicate message name the other entry by its display name
  and tell the user to remove it or change this entry's destination, then press Retry. Add tests for both triggers
  (retarget onto another entry's machine; re-added reinstalled host with the old entry still pointing at it) and for
  Retry clearing the freeze after the other entry is removed. Remove this feedback file and its index entry.
- Execution: complete: change `xqltwtssqnpw`, bookmark `triage-1001b/05-duplicate-hosts`, PR
  https://github.com/scode/farhelm/pull/1378.

## yolo-guard-misses-env-prefix.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at 961e0a0. `argv_is_yolo` (`crates/farhelm-proto/src/yolo.rs`) picks
  the vendor's flag table from the first word's basename only, so
  `env NAME=value claude
  --dangerously-skip-permissions` (or `env A=1 codex --yolo`, or `env A=b pi`) classifies as
  `env`, matches no vendor, and is not YOLO. The helm guard (`invocation_is_yolo` in
  `crates/farhelm-helm/src/yolo_guard.rs`) therefore lets it start on a sensitive host without confirmation, and the
  sidebar badge (`invocation_marker`, same first-word lookup) misses it too. Farhelm itself treats a leading
  `env NAME=value` prefix as an ordinary launch shape: the supervisor's `effective_program_index`
  (`crates/farhelm-supervisor/src/agent_kind/mod.rs`) skips it to find the real program. No test pins either behavior,
  and no spec text, `Planned` item, `BUGS.md` entry or filter covers it. Affects raw command lines and profile
  invocations (and clone/replace of such sources, `farhelm agent create`/`spawn`); structured launches are unaffected.
- Decision: the user chose spec+code with an explicit principle: for custom launches (raw command lines and profile
  invocations), YOLO detection is best effort. Farhelm cannot guarantee that every possible command line that turns off
  approvals is recognized; it should cover as many reasonable shapes as it can, and an `env NAME=value` prefix is one of
  them. The spec must not claim complete detection for custom launches with arbitrary command lines.
- Completion criteria: make the shared YOLO classifier (guard and badge) look past a leading simple `env NAME=value`
  prefix using one shared copy of the rule the supervisor's `effective_program_index` applies, so the two cannot drift;
  an `env` followed by an option (such as `env -i`) is treated as YOLO by the guard rather than as not-YOLO. Add tests
  for `env A=1 claude --dangerously-skip-permissions`, `/usr/bin/env A=1 codex --yolo`, and `env A=b pi`. Amend
  SPEC.md's YOLO-launch paragraph (and SPEC_impl.md where it describes the classifier) to state that recognition of
  custom command lines is best effort: common shapes are covered, but arbitrary wrappers (scripts, `sh -c`, and the
  like) are not guaranteed to be detected; structured launches remain exact. Remove this feedback file and its index
  entry.
- Execution: complete: change `zmlxvrokvtll`, bookmark `triage-1001c/01-yolo-env-prefix`, PR
  https://github.com/scode/farhelm/pull/1384.

## yolo-guard-fails-open-without-row.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at 961e0a0, not reproduced at runtime. `check`
  (`crates/farhelm-helm/src/yolo_guard.rs`) returns OK when the host's registry row is missing, on the premise that
  routing to a removed host refuses on its own; but create and restart-with (`crates/farhelm-helm/src/sessions.rs`) take
  the host's connection client before calling the guard, and host removal (`remove_host_owned` in
  `crates/farhelm-helm/src/hosts.rs`) deletes the row before it stops the connection actor. Nothing serializes create
  against removal. A YOLO launch without the override (most plausibly an agent's `farhelm agent create`/`clone` or
  `farhelm spawn`) that reaches the guard in that window of a few awaits is allowed and dispatched over the still-open
  connection, starting an approval-free agent on a sensitive host that then runs invisibly because the helm has
  forgotten the host. Nothing in the spec, `Planned`, `BUGS.md` or filters covers it. Triage also identified the wider
  race behind it: any create routed before a removal can land on the forgotten host and run invisibly, YOLO or not.
  Closing that needs a new per-host lock held by every create path across dispatch and taken exclusively by removal,
  with lock ordering against the provisioning and cache-write locks and slower removal.
- Decision: the user chose the narrow fix only and discarded the wider create-versus-removal race: its consequence is a
  session the user or their agent asked for that is not visible until the host is re-added, not a safety bypass, and
  closing it is not worth the added locking.
- Completion criteria: make the YOLO guard treat a missing host row as host-not-found, refusing with the same error the
  helm gives for an unknown host, and correct the comment that claims routing refuses on its own. Add a unit test that a
  YOLO check against a host id with no registry row is refused. Do not add serialization between create and host
  removal. Remove this feedback file and its index entry.
- Execution: complete: change `lswtswtnqnlv`, bookmark `triage-1001c/05-yolo-guard-missing-host`, PR
  https://github.com/scode/farhelm/pull/1390.

## sighup-skips-orderly-shutdown.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at 961e0a0, not reproduced at runtime. The supervisor's `run`
  (`crates/farhelm-supervisor/src/service/core.rs`) installs listeners only for SIGTERM, SIGINT and the desktop tether,
  and nothing in the supervisor or the `farhelm` binary handles or ignores SIGHUP, so a hangup kills the supervisor with
  its default action and skips the orderly tmux output shutdown; per `BUGS.md` that can abort the private tmux server
  and every session on the host. Triggers: closing the terminal or losing the ssh connection of a hand-started
  `farhelm
  supervisor run` (a remedy the hosts page suggests), or closing the terminal a Linux desktop app was
  launched from (the managed supervisor is spawned in the app's process group, `crates/farhelm-ui/src/desktop.rs`).
  systemd-unit supervisors and desktop apps launched without a terminal are unaffected. `BUGS.md`'s "Abrupt supervisor
  death" entry does not cover this and is inaccurate: it says every planned stop runs the orderly path and only deaths
  that run no code remain, but SIGHUP is catchable and unhandled. Triage added a premise the finding understates: the
  tmux output and sink clients (`crates/farhelm-supervisor/src/tmux/stream.rs`, `tmux/sink.rs`) share the supervisor's
  process group, so a terminal hangup also reaches them and tmux tears them down outside the orderly order; a supervisor
  SIGHUP handler alone may not restore safe ordering (inferred from tmux 3.7c source, not tested).
- Decision: the user chose the code fix as recommended.
- Completion criteria: route SIGHUP into the same orderly shutdown as SIGTERM and SIGINT, updating `run`'s docs; start
  the tmux output and sink clients in their own process group so a terminal hangup or Ctrl-C reaches only the supervisor
  (optionally also start the desktop app's managed supervisor in its own group); correct `BUGS.md`'s description of
  which deaths skip the orderly path; add a focused test that a SIGHUP to the supervisor's process group runs the
  orderly shutdown. Remove this feedback file and its index entry.
- Execution: complete: change `xvywxyuqsnuz`, bookmark `triage-1001c/07-sighup-orderly-shutdown`, PR
  https://github.com/scode/farhelm/pull/1392. The desktop app's managed-supervisor spawn was left alone (optional per
  this entry).

## header-replace-recomputes-alive.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at 961e0a0, not reproduced in a browser. The header's `replace`
  closure (`crates/farhelm-ui/src/session_view.rs`) recomputes `only_if_nothing_alive` from the session's current state
  each time it runs, and both YOLO-confirmation buttons call it again. If the session is restarted or gains a tab while
  the YOLO question is open (another window, the desktop app, an agent through fleet operations, or a tab the user opens
  in the same view, which operations do not block), the re-run sends `false` and the supervisor deletes the source
  unconditionally (`crates/farhelm-supervisor/src/service/handlers.rs`), killing the just-started agent or shell after a
  prompt that said nothing was alive. Header Delete already captures the value from the render that drew its prompt. The
  reverse drift is harmless. Not covered by the spec, `Planned`, `BUGS.md` or filters; it is a hole in the
  `delete-lacks-liveness-precondition.md` outcome (#1152, #1160), which said Replace sets the flag when its confirmation
  showed nothing alive.
- Decision: the user chose the code fix provided it is easy and adds little complexity (it follows the existing Delete
  pattern), plus a spec principle: a single GUI attached to the helm is the supported user surface, and several
  concurrent GUIs are best effort. The user also asked for a `Maybe later` TODO entry to consider refusing more than one
  UI outright for simplicity (added during triage).
- Completion criteria: capture the nothing-alive value (with the source fields) from the prompt the user confirmed,
  carry it in the pending YOLO question's state, and send that stored value from both YOLO buttons; add a browser
  regression on Chromium and WebKit. If the fix turns out not to be easy, stop and return the item to the user rather
  than adding machinery. Add the single-GUI principle to SPEC.md (concurrent GUIs are best effort), worded so it does
  not contradict or remove the existing multi-client behavior the session view section specifies (one attached client
  per session, takeover, displaced clients). The user confirmed that the `farhelm` command line and the agent skill
  (agents acting through fleet operations) are a fully supported primary surface alongside the UI, including
  concurrently with it; the spec must say so, and the best-effort qualifier applies only to several concurrent GUIs.
  Remove this feedback file and its index entry.
- Execution: complete: change `sxxkpwstxrtl`, bookmark `triage-1001c/08-header-replace-keeps-answer`, PR
  https://github.com/scode/farhelm/pull/1394.

## sidebar-replace-recomputes-alive.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at 961e0a0, not reproduced in a browser. Same root cause as
  `header-replace-recomputes-alive.md` in the sidebar: `do_replace` (`crates/farhelm-ui/src/list/view.rs`) looks the row
  up again and recomputes `only_if_nothing_alive` each time it runs, and both YOLO-confirmation buttons re-enter it; its
  comment claiming the prompt was worded from the same row holds only on the first run. If the row has meanwhile dropped
  out of the list, `is_some_and` yields `false`, an unguarded delete. Trigger and consequence as in the header item
  (another window, the desktop app, or an agent restarts the session or opens a tab while the YOLO question is open; the
  just-started agent or shell is killed). Not covered by the spec, `Planned`, `BUGS.md` or filters.
- Decision: as for `header-replace-recomputes-alive.md`: the code fix provided it is easy and adds little complexity,
  plus the single-GUI spec principle (a single GUI attached to the helm is supported; several concurrent GUIs are best
  effort). The `Maybe later` TODO entry was added once, for both items.
- Completion criteria: capture the nothing-alive value from the row the confirm prompt was drawn from, carry it in the
  sidebar YOLO question's state, and send the stored value from both YOLO buttons; a row missing from the list must not
  turn into an unguarded delete. Add a browser regression on Chromium and WebKit. If the fix turns out not to be easy,
  stop and return the item to the user. If the header item's execution has already added the single-GUI principle to
  SPEC.md, this item needs no further spec change; otherwise add it as described there. Remove this feedback file and
  its index entry.
- Execution: complete: change `ysntyqlspvxq`, bookmark `triage-1001c/09-sidebar-replace-keeps-answer`, PR
  https://github.com/scode/farhelm/pull/1396. No spec change here: the single-GUI principle landed with
  `header-replace-recomputes-alive.md`.

## yolo-guard-misses-codex-option-form.md

- Outcome: `fix code`.
- Assessment: confirmed by current-code inspection at 961e0a0. `YOLO_OPTION_VALUES` (`crates/farhelm-proto/src/yolo.rs`)
  has entries for OMP's `--approval-mode yolo` and Claude's `--permission-mode bypassPermissions` but none for Codex, so
  `codex -a never -s danger-full-access` (and the long and `=` spellings) classifies as not YOLO in both the helm guard
  and the sidebar badge. The vendor premise the reviewer could not check was verified in triage against the installed
  `codex-cli 0.159.3 --help`: `-a never` is "Never ask for user approval" and `-s danger-full-access` removes the
  sandbox, together the same as `--dangerously-bypass-approvals-and-sandbox`. Reach is that of
  `yolo-guard-misses-env-prefix.md` (raw command lines, profiles, clone/replace of such sources,
  `farhelm agent
  create`/`spawn`); structured launches are unaffected. Not covered by the spec, `Planned`, `BUGS.md`
  or filters; the best-effort principle recorded for `yolo-guard-misses-env-prefix.md` frames this as a common
  documented shape to cover.
- Decision: the user chose the code fix as recommended. `-a never` alone keeps Codex's sandbox and does not count, like
  `--full-auto`; `-s danger-full-access` alone keeps approval prompts and does not count.
- Completion criteria: classify a Codex command as YOLO when it carries both a never-ask approval policy and the
  `danger-full-access` sandbox, in any of the `-a`/`--ask-for-approval`, `-s`/`--sandbox` and `=` spellings and in
  either order, in the guard and the sidebar badge; include the `-c`/`--config` spellings (`approval_policy="never"`
  with `sandbox_mode="danger-full-access"`) if that comes cheaply. Add classifier tests for each spelling and a guard
  test refusing such a create on a sensitive host. Remove this feedback file and its index entry.
- Execution: complete: change `yluskrnrnmwt`, bookmark `triage-1001c/02-yolo-codex-option-form`, PR
  https://github.com/scode/farhelm/pull/1387.

## yolo-safe-survives-identity-adoption.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at 961e0a0. The "start YOLO sessions without asking" setting is a
  column on the registry row (`hosts.yolo_safe`), read only by the YOLO guard (`crates/farhelm-helm/src/yolo_guard.rs`).
  Adoption (`adopt_identity` in `crates/farhelm-helm/src/store.rs`) swaps the identity and deliberately purges the old
  install's session cache and install-scoped history, but leaves `yolo_safe` untouched, so after an adopt (recycled
  address, reinstall, or a retarget to another machine followed by Adopt) YOLO launches on the new install skip the
  confirmation and the hosts panel shows nothing unusual. The setter's own docs say sensitivity is a property of the
  machine, and SPEC.md's Topology section treats a new identity as a new host. Not covered by the spec, `Planned`,
  `BUGS.md` or filters. The finding also names two narrower variants with the same root: a settings dialog or the YOLO
  confirmation's "don't ask again on this host" left open across a concurrent retarget/adopt marks whatever machine the
  row now points at (no identity precondition on `set_yolo_safe`); and a row marked before first contact, then
  retargeted, records the new machine's identity at first contact and keeps the mark.
- Decision: the user chose to fix the main adopt case only and to discard both narrower variants (stale-dialog toggle,
  and the never-contacted row retargeted before first contact) for now.
- Completion criteria: clear `yolo_safe` inside `adopt_identity`'s existing transaction, so an adopted host asks before
  YOLO launches again; make the adopt prompt say that YOLO launches will ask again after adopting; add to SPEC.md's host
  settings paragraph that adopting a new identity resets the host to asking before YOLO launches. Add a store test (mark
  safe, adopt a different identity, the row is no longer safe). Do not add an identity precondition to the setter or
  change first-contact recording. Remove this feedback file and its index entry.
- Execution: complete: change `vqonszswtolv`, bookmark `triage-1001c/06-adopt-resets-yolo`, PR
  https://github.com/scode/farhelm/pull/1391.

## yolo-guard-misses-equivalent-spellings.md

- Outcome: `fix spec+code`.
- Assessment: both parts confirmed by current-code inspection at 961e0a0. (A) Cursor: the classifier's Cursor rows
  (`agent`, `cursor-agent` in `crates/farhelm-proto/src/yolo.rs`) list only `--force` and `--yolo`, so the documented
  short `-f` (verified in the installed `cursor-agent --help`:
  `-f, --force  Force allow commands unless explicitly
  denied`) is not YOLO in the guard or the badge. (B) Pi: a
  launch declared as kind Pi (a profile's `agent_kind`, or a raw create's kind override) whose program is not named `pi`
  is not YOLO; the declared kind is available to the guard (`create_is_yolo` in `crates/farhelm-helm/src/yolo_guard.rs`,
  `ResolveProfile` in `agent_requests.rs`) but ignored. Restart-with is already covered; raw clone/replace drop the kind
  entirely. Triage also found that the program name `agent` is ambiguous: on the development host `agent` is Grok's
  executable, not Cursor's, so the classifier's assumption that `agent` means Cursor is unsound (Grok's
  `--always-approve` under the name `agent` is missed). Also noticed in passing, outside this finding: OMP's documented
  `--auto-approve` and Grok's `--permission-mode
  bypassPermissions` are absent from the tables.
- Decision: fix Cursor's `-f`. Agents installed or launched under alternative program names are explicitly out of scope
  for YOLO detection, a declared Pi kind under another program name included; record that in the spec rather than
  checking the declared kind. Recognize `cursor-agent` as Cursor and `grok` as Grok, and assume nothing about the
  program name `agent`, which is too general; say so explicitly in the spec. Because Farhelm's own built-in `cursor` and
  `cursor-yolo` profiles and the structured Cursor harness launch `agent`, and dropping `agent` from detection would
  leave the built-in `cursor-yolo` profile (classified by its invocation `agent --force`) unguarded, the user chose to
  switch Farhelm's Cursor launches to always use the `cursor-agent` program name, as the simplest option.
- Completion criteria: change the built-in `cursor` and `cursor-yolo` profiles (`builtin_profiles` in
  `crates/farhelm-helm/src/store.rs`) and the structured Cursor harness compiler to launch `cursor-agent` instead of
  `agent`; remove `agent` from the YOLO classifier tables and add `-f` to `cursor-agent`'s YOLO flags, in the guard and
  the badge; update tests that assume `agent` (including UI code keyed on the built-in Cursor profiles). Already stored
  sessions keep their recorded launch. Amend SPEC.md: the Cursor section names `cursor-agent`; the YOLO-launch paragraph
  (alongside the best-effort wording from `yolo-guard-misses-env-prefix.md`) says custom command lines are recognized by
  each vendor's standard program name (`cursor-agent` for Cursor, `grok` for Grok, `pi` for Pi, and so on), the generic
  name `agent` is not interpreted, and agents installed or launched under other names are not detected. OMP's
  `--auto-approve` and Grok's `--permission-mode bypassPermissions` are not part of this decision. Remove this feedback
  file and its index entry.
- Execution: complete: change `tqwsvpvomkwm`, bookmark `triage-1001c/03-cursor-agent-name`, PR
  https://github.com/scode/farhelm/pull/1388.

## yolo-guard-skips-resume-template.md

- Outcome: `fix spec`.
- Assessment: confirmed by current-code inspection at 961e0a0. `create_is_yolo`
  (`crates/farhelm-helm/src/yolo_guard.rs`) and the `farhelm spawn` profile lookup (`agent_requests.rs`) classify only
  the start command, never a profile's or a raw create's separate resume command, so a custom launch whose start command
  is plain and whose hand-written resume command carries a vendor YOLO flag passes the sensitive-host check at creation,
  and every later Resume (or Restart of a generic profile) runs it unconfirmed, because a plain restart is not asked
  again. Affects only custom launches (user profiles with a hand-written resume command, raw API creates that send one):
  structured launches build their resume command from the checked selection, built-in plain profiles have none, and
  built-in YOLO profiles' start command is already YOLO. Not covered by the spec, `Planned`, `BUGS.md` or filters.
- Decision: the user chose a spec clarification and discarded the code fix as not worth the complexity, and asked for a
  `Near term` TODO entry (added during triage) to re-examine and simplify how launches are handled, with this issue as
  the example.
- Completion criteria: amend SPEC.md's YOLO-launch paragraph so that, for custom launches, only the start command is
  classified: a separate resume command is not checked, and a plain Resume or Restart that runs it is not asked. Keep it
  consistent with the best-effort wording from `yolo-guard-misses-env-prefix.md`. No code change. Remove this feedback
  file and its index entry.
- Execution: complete: change `qsspryqlvrlp`, bookmark `triage-1001c/04-yolo-resume-spec`, PR
  https://github.com/scode/farhelm/pull/1389.

## codex-resume-template-duplicates-selector.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by current-code inspection at 0db9621 and a local parse check. Codex's derived resume template
  (`default_resume_template` for Codex in `crates/farhelm-supervisor/src/agent_kind/mod.rs`) copies the original argv
  and appends `resume {conversation}`, and its `ambiguous_derived_resume` always returns `None`, so a session launched
  as `codex resume <old-id>` restarts as `codex resume <old-id> resume <captured-id>`. Codex 0.159.3 rejects that at
  argument parsing (`unexpected argument '<uuid>' found`), so Restart or Resume of such a session exits with an error
  instead of continuing the conversation; the conversation on disk is unharmed and Restart with an explicit command
  still works. SPEC.md's derived-resume paragraph states the assumption that the launch has no launch-only option and
  defers the general separation; TODO.md's `Maybe later` entry on separating common, launch, and resume arguments names
  the selector collision, but neither is a `Planned` item or an explicit acceptance, so the item was not auto-skipped.
  The sibling Claude finding `claude-resume-template-selector-collision.md` (highest bucket, untriaged at the time) has
  the same shape.
- Decision: fix it the way Grok already handles the same risk, and also add this case as a second example to TODO.md's
  `Near term` entry "Re-examine and simplify how launches are represented", which already uses
  `yolo-guard-skips-resume-template.md` as its example of the complexity.
- Completion criteria: Codex's `ambiguous_derived_resume` refuses a derived template when the retained argv already
  carries a session selector (the `resume` or `fork` subcommand), so the create fails with a clear message unless an
  explicit resume template is supplied, mirroring `GrokAmbiguousResumeBoundary`. Amend SPEC.md's derived-resume
  paragraph to list Codex beside Grok as refusing derivation in that case. Add a regression covering an original
  `codex resume <id>` launch with and without an explicit template. Add the Codex resume-selector collision as an
  example to the TODO.md `Near term` entry "Re-examine and simplify how launches are represented". Remove this feedback
  file and its index entry.
- Execution: complete: change `uzvukoxktxkq`, bookmark `triage-1001d/01-codex-resume-selector`, PR
  https://github.com/scode/farhelm/pull/1397.

## tmux-capture-tail-deadline.md

- Outcome: `other`.
- Assessment: confirmed in code at 0db9621. The supervisor's pane-capture helper in
  `crates/farhelm-supervisor/src/tmux.rs` bounds only the stdout read loop; once stdout closes, the waits for stderr and
  for the tmux process to exit have no deadline, and the timeout path's stderr wait is likewise unbounded. Because the
  status sampler captures panes one at a time, a capture that never finishes stops status sampling on that host until
  the supervisor restarts. A real tmux `capture-pane` client starts no children and closes stdout as it exits, so the
  trigger needs a configured tmux program that does not behave like tmux (for example a wrapper that backgrounds
  something holding stderr). The match is FILTER.md filter "A hung private tmux server or systemd user manager": the
  trigger is a tmux program that never exits instead of behaving like tmux, and the whole consequence stays on the
  affected host (its sampler stalls until the supervisor restarts), with no helm, cross-host, durable, data-loss or
  security consequence. Prior ledger entries `tmux-run-bytes-unbounded-under-lock.md`,
  `tmux-kill-runs-unbounded-under-global-lock.md` and `startup-tmux-version-check-unbounded.md` were closed under the
  same filter.
- Decision: skipped under the triage rule for findings covered by a review filter.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage.

## pi-resume-selector-option-boundaries.md

- Outcome: `discard`.
- Assessment: confirmed in code at 0db9621. The value-taking option table that Pi's resume selector stripper uses
  (`strip_pi_selectors` in `crates/farhelm-supervisor/src/agent_kind/mod.rs`) lacks a dozen options that Pi's launch
  parser (`crates/farhelm-supervisor/src/agent_kind/pi.rs`) treats as value-taking, among them `--name`/`-n`,
  `--api-key`, `-t`, `-xt`, `--models`, `--mode`, `--skill`, `--prompt-template`, `--theme`, `--use-theme` and
  `--tui-mode`. An original launch such as `pi --name --resume` therefore has its option value stripped as a selector,
  and the appended `--session <file>` would likely be read as the name, starting a fresh conversation with the file path
  as the prompt. Unverified: Pi's own parsing of a dash-prefixed option value (inferred from Farhelm's launch-side
  parser). Trigger is negligible in practice: an option value literally spelled like a Pi session selector. Borderline
  on FILTER.md "Rare edge cases in harnesses without first-class support" (a fresh-start Resume is more than a missing
  offer), so brought to the user. Side observation: SPEC.md's derived-resume paragraph says only OMP's selectors are
  stripped, while the code also strips Pi's.
- Decision: discard; the trigger is unrealistic and Pi support is intentionally partial.
- Completion criteria: remove the feedback file and its index entry, without code or spec changes.
- Execution: complete: change `puowvnppqvtk`, bookmark `triage-1001d/02-discard-pi-selector`, PR
  https://github.com/scode/farhelm/pull/1398.

## provisioning-child-output-drain-deadline.md

- Outcome: `other`.
- Assessment: confirmed in code at 0db9621; no realistic trigger found. A provisioning step's deadline covers only the
  wait for the direct child (the local `ssh` or `sh`) to exit (`crates/farhelm-helm/src/provisioning/backend.rs`, around
  the child wait); the following drain of its stdout and stderr readers has no deadline, and the payload-transfer path
  shares it. If a process on the helm machine inherited the pipe's write end and outlives the child, the run never
  finishes: the host stays installing or updating until the helm restarts, rename and retarget on it wait on its lock,
  remove is refused as busy, and one of the four run slots stays held. Remote-side background work cannot cause this: it
  keeps `ssh` from exiting, which the existing deadline already covers. Farhelm's own provisioning scripts start no
  background children. A local check with OpenSSH 9.6p1 showed a ControlPersist master staying alive after the command
  while the output pipe reached EOF in about 0.5 s, so the connection master does not hold the pipe on current OpenSSH
  (older releases were not checked). The remaining trigger is a user's ssh `ProxyCommand` (or similar local helper) that
  leaves a child holding stderr, which nobody has observed. The finding's secondary claim, that cleanup reads the
  process-group id after reaping, is not a current bug; it constrains a future fix only. Not covered by the spec,
  `Planned`, `BUGS.md`, filters or the ledger.
- Decision: do not fix; record it in root `BUGS.md` as a known issue we do not plan to fix.
- Completion criteria: add a `BUGS.md` entry in that file's style: a host install or update can hang until the helm
  restarts if a local process started by the ssh invocation (for example a `ProxyCommand` helper that forks a lingering
  child) keeps the step's output pipe open after `ssh` exits, with the symptoms above, how sure we are (code reading
  plus the OpenSSH 9.6 ControlPersist check; no observed trigger), and why it is not being fixed (no realistic trigger
  on current OpenSSH; the fix would extend one deadline over exit, both output drains and process-group cleanup). Remove
  the feedback file and its index entry. No code or spec change.
- Execution: complete: change `xnpnzvlpmoln`, bookmark `triage-1001d/03-bugs-provisioning-drain`, PR
  https://github.com/scode/farhelm/pull/1399.

## clipboard-writes-unbounded-blocking-admission.md

- Outcome: `fix spec+code`.
- Assessment: partly correct at 0db9621. The missing admission bound is real: `post_clipboard`
  (`crates/farhelm-helm/src/clipboard.rs`) starts one `spawn_blocking` task per request with no limit, and the desktop's
  native writer serializes writes behind one lock, so a hung native clipboard makes every further copy park another
  blocking-pool thread; the helm's database work shares that pool. The stated consequence is overstated: other work only
  suffers once the backlog passes tokio's default blocking-thread ceiling (512), and the webview keeps only a few
  requests to one origin in flight (unverified: whether WebKit times these fetches out and whether the server drops the
  handler on client disconnect). The real trigger is a hung or very slow OS clipboard, not a program writing often; a
  working clipboard never builds a backlog. Follow-on to `clipboard-sink-blocks-async-worker.md` (fixed in #1166), not a
  repeat. Not covered by the spec, `Planned`, `BUGS.md`, filters or the ledger.
- Decision: fix it, because the fix is very small, and state in the spec that Farhelm assumes the host system's
  clipboard works: a broken, hung or slow clipboard is not something Farhelm adds complexity to support well.
- Completion criteria: bound in-flight native clipboard writes in the helm with a small fixed cap (try-acquire on a
  semaphore held in the helm's shared state; the permit moves into the blocking task), and when the cap is full, drop
  the write silently under the existing best-effort contract (log at most once per episode rather than per request). Add
  a test with a sink that blocks showing that writes beyond the cap return without starting blocking work and that a
  write succeeds again once the sink unblocks. Amend SPEC.md's clipboard best-effort text (Terminal experience) to say
  Farhelm assumes the host's system clipboard is functioning, and that a broken, hung or slow clipboard is outside what
  Farhelm adds complexity to support: copies may be dropped then, but the rest of Farhelm must not stall. Remove this
  feedback file and its index entry.
- Execution: complete: change `wvyxxyxyowvw`, bookmark `triage-1001d/04-clipboard-write-cap`, PR
  https://github.com/scode/farhelm/pull/1400.

## session-view-leaks-page-lock.md

- Outcome: `fix code`.
- Assessment: partly correct at 0db9621. The two Replace sites were already fixed by #1153 (`620f6897`): header Replace
  and the interrupted card's Replace hold a self-releasing guard in a confirmation slot that is dropped when the view
  unmounts. Still present: header Restart (`lifecycle.claim()` in `crates/farhelm-ui/src/session_view.rs`, released only
  by Cancel or at the end of the restart task), "Restart with" (claimed when the dialog opens; released only by the
  dialog's Cancel or a successful restart, and deliberately kept after a failed attempt), and the interrupted card's
  Restart (held through the in-flight request). The session view's `use_drop` unmounts terminals but releases nothing,
  and the claim lives on the top-level UI component (`crates/farhelm-ui/src/lib.rs`), so it outlives the view. If the
  view unmounts while one of those claims is held (another client deletes the session, the browser build's token prompt
  replaces the panes on a 401, or likely the restart deselect race in `restart-can-still-deselect-session.md`), every
  write action in that browser tab or desktop window stays disabled, including opening another session from the sidebar
  (`guarded_open` in `crates/farhelm-ui/src/list/view.rs`), until that tab or window is reloaded. Additional residual
  found in triage: if another client restarts the session while the interrupted card's Replace prompt is open, the card
  stops rendering while the view stays mounted, nothing clears the prompt, and its claim stays held with the same stuck
  result. Not covered by the spec, `Planned`, `BUGS.md`, filters or the ledger; the related
  `header-replace-confirm-ignores-cancel.md` fix deliberately left the Restart sites on hand-written claims.
- Decision: fix the code.
- Completion criteria: no session-view claim of the UI's operation lock can outlive the session view or the UI element
  that owns it: header Restart, "Restart with" (including after a failed attempt) and the interrupted card's Restart use
  a self-releasing guard like #1153's Replace sites, or the view releases any claim it holds on unmount; and the
  interrupted card's Replace prompt (and its claim) is cleared when the card stops rendering. Add regressions covering
  unmount while a Restart confirmation, an open "Restart with" dialog, and an in-flight Restart hold the claim, and the
  hidden interrupted-card prompt. Remove this feedback file and its index entry.
- Execution: complete: change `lklyyznrrnzl`, bookmark `triage-1001d/05-session-view-lock-release`, PR
  https://github.com/scode/farhelm/pull/1401.

## row-menu-drifts-on-row-height-change.md

- Outcome: `fix code`.
- Assessment: confirmed at 0db9621; the consequence is mostly visual. The sidebar row menu's panel is placed at
  coordinates measured when it opens, and the list closes it on scroll or resize, the create form opening, the host
  list's shape changing, or an index reorder (`crates/farhelm-ui/src/list/view.rs`, `rows::menu_row_reordered`). A row
  above the open one that gains or loses its detail line (session ended or stale; compact mode is off by default) keeps
  its index, so none of those trip and the panel ends up beside the next row down. The order hold while a menu is open
  makes a same-index change above more likely. Actions still target the session the menu was opened on; the panel header
  names it, delete and replace confirmations quote its title, and the row tint stays on the correct row. The code
  comment above the close effect knowingly accepts this residual, contradicting SPEC_impl.md's row-menu rule that the
  menu closes on any layout change that could have moved its row. Not covered by `Planned`, `BUGS.md`, filters or the
  ledger; the sibling `row-menu-drifts-after-own-delete.md` was decided `fix code` (#1154).
- Decision: fix the code, consistent with the sibling decision.
- Completion criteria: an open session row menu closes when a row above it changes height in a way that moves the open
  row (gaining or losing its ended, stale or error detail line), and the code comment that accepts this residual is
  updated to match SPEC_impl.md. Decide during execution whether the host row's identical accepted residual (noted in
  the same comment) is covered by the same mechanism cheaply; if not, leave it and say so. Add a unit test alongside the
  existing `rows.rs` ones covering a height change above the open row (closes) and below it (stays open). Remove this
  feedback file and its index entry.
- Execution: complete: change `yopkmnvsxpvx`, bookmark `triage-1001d/06-row-menu-height-drift`, PR
  https://github.com/scode/farhelm/pull/1402.

## uploads-aborted-silently-on-remount.md

- Outcome: `fix code`.
- Assessment: confirmed (both parts) at 0db9621. A terminal reconnect (`runReconnect`) or session restart unmounts the
  terminal, and unmount disposes its attachment handler (`crates/farhelm-ui/assets/terminal.js`), which aborts every
  in-flight upload and blob read and deliberately blanks the pane's status line; a later `send()` returns silently once
  disposed, so no path is inserted and no error is shown. A "landed at <path>" or "attaching X failed" message produced
  by the same outage is erased when the first automatic reconnect (500 ms after the socket dies) remounts the terminal,
  and the new mount starts with an empty message list. If the abort lands during final publication, a complete
  attachment may exist on the host with no path inserted and no message. This contradicts SPEC.md's Attachments rule
  that upload failures must be visible and an attachment must never disappear silently. Not covered by `Planned`,
  `BUGS.md`, filters or the ledger; the rare-glitch filter does not apply because nothing tells the user to retry.
- Decision: fix the code.
- Completion criteria: a terminal pane's upload status and failure messages survive a reconnect or restart remount (the
  new mount repaints them), and an upload aborted by a remount leaves a visible message naming the file and saying it
  was interrupted, distinguishing "may have been published" when the abort could have landed during publication, per
  SPEC.md's failure-response rule. Clearing stale "attaching…" text on remount stays correct for an upload that is no
  longer running. Add a JS or browser test covering an upload interrupted by a remount and a failure message present
  across one. Remove this feedback file and its index entry.
- Execution: complete: change `zvvqpykpqkrw`, bookmark `triage-1001d/07-upload-remount-messages`, PR
  https://github.com/scode/farhelm/pull/1403.

## replace-drop-skips-source-delete.md

- Outcome: `fix code`.
- Assessment: confirmed at 0db9621. The helm's replace handler (`replace_session` in
  `crates/farhelm-helm/src/sessions.rs`) runs the whole create-then-delete sequence (`do_replace_session`,
  `finish_replacement`) on the HTTP request's own task rather than a helm-owned one, so a client that goes away after
  the create was sent and before the delete (reload or tab close, desktop Quit, the browser build's 401 token-prompt
  swap, or the client's 60 s request timeout on a slow fresh-checkout clone) leaves the new session created and the
  original never deleted, usually with no error. The helm already has a helper for running work on a helm-owned task
  (`lib.rs`), used by the host routes but not the session routes. This violates SPEC_impl.md "Who owns an accepted
  action". Two report details are wrong without changing the conclusion: switching sessions mid-Replace is refused by
  the UI's operation lock, and desktop re-sign-in does not remount the app. Unverified: that the web server drops the
  handler promptly on client abort (the spec and the helper's existing test assume it), and that a clone can exceed the
  60 s client deadline. Not covered by `Planned` (the create read-loop item is about the supervisor), `BUGS.md`, filters
  or the ledger; the recent Replace decisions concern the "nothing alive" recheck and do not overlap.
- Decision: fix the code by moving the replace body onto a helm-owned task.
- Completion criteria: the helm runs Replace and Replace with on a helm-owned task, so a client that disconnects or is
  cancelled loses only the reply; add a regression that drops the request after the create is sent and asserts the
  source delete still happens. During execution, check the helm's other session routes (create, delete, restart and
  similar) against the same SPEC_impl.md rule and report any that are also request-task-bound; fix them in this change
  only if they share the same small mechanism, otherwise bring them back to the user by listing them in the executing
  plan's final report, without blocking (user decision at planning time, 2026-10-01). Remove this feedback file and its
  index entry.
- Execution: complete: change `vyqppyozstpv`, bookmark `triage-1001d/08-replace-helm-owned`, PR
  https://github.com/scode/farhelm/pull/1404.

## restart-can-still-deselect-session.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading at 0db9621; frequency not measured. During a restart the supervisor removes the
  session from its in-memory session map for the whole relaunch (`crates/farhelm-supervisor/src/service/core.rs`, the
  "Off the map for the duration" block before `relaunch_into_terminal`), a window covering the old terminal's detach and
  the full relaunch's tmux calls and database work, longer than the "couple of tmux round trips" its comment assumes.
  #1310 (`eb319780`) stopped the restart from hinting at the window's start, but other listings still land inside it:
  the helm's 3 s background refresh, the stop that precedes restarting a running agent (its requested and completed
  records each hint, rate-limited to one per 200 ms, so one can land inside the window), and any other change on that
  host. A complete listing without the session makes the UI deselect it (`crates/farhelm-ui/src/list/view.rs`,
  `selected_vanished`), and nothing re-selects it when it reappears. Likely the everyday trigger for
  `session-view-leaks-page-lock.md`. Only a code comment accepts the omission; not covered by the spec, `Planned`,
  `BUGS.md`, filters or the ledger.
- Decision: fix the code at the source, option (a): the supervisor keeps a restarting session listed rather than
  omitting it, instead of making the UI's deselect rule more tolerant.
- Completion criteria: a `ListSessions` taken while a session is mid-restart still includes that session (for example
  with a relaunching marker or its pre-restart state), while stop, delete and attachment installs stay excluded from the
  window as they are today; the UI therefore keeps the session selected through a restart. Update the code comment that
  accepts the omission. Add a regression that forces a listing into the relaunch window and asserts the session is
  present. Remove this feedback file and its index entry.
- Execution: complete: change `nxroulqzvtkk`, bookmark `triage-1001d/09-restart-keeps-listed`, PR
  https://github.com/scode/farhelm/pull/1405.

## create-dialog-empty-catalog-refuses.md

- Outcome: `fix code`.
- Assessment: confirmed at 0db9621. The new-session dialog fetches the launch catalog once
  (`crates/farhelm-ui/src/list/create_form.rs`, `launch_catalog` resource) and turns a failed or still-pending fetch
  into an empty model list (`unwrap_or_default`); with an empty list `launch_composer::selection_is_compatible` rejects
  any selection that names an effort, so New, Clone, Replace with and recent setups show "this saved choice is no longer
  supported by the current catalog" and disable Launch. The pending case corrects itself when the fetch lands; the
  failed case persists until the dialog is reopened (losing its draft) or the effort is cleared. "Restart with"
  (`crates/farhelm-ui/src/restart_with.rs`) already treats a failed fetch as compatible because "an outage cannot
  establish that a stored selection became invalid", though it too treats a pending fetch as incompatible. The trigger
  is rare (a network or sign-in hiccup when the dialog opens). Borderline on FILTER.md "Rare, self-correcting glitches
  and imprecise diagnostics" because reopening loses the dialog's draft; brought to the user rather than filtered.
- Decision: fix the code.
- Completion criteria: in the new-session dialog, a failed or still-pending catalog fetch never marks a selection
  incompatible; a failed fetch shows its error with a way to retry, and the helm's own validation of the final request
  stays the authority. Apply the pending-fetch half to "Restart with" too, so both dialogs share one rule. Add a test
  covering a failed and a pending catalog with an effort-bearing prefilled selection. Remove this feedback file and its
  index entry.
- Execution: complete: change `zloxqttnwoul`, bookmark `triage-1001d/10-create-catalog-read`, PR
  https://github.com/scode/farhelm/pull/1406.

## stop-terminalless-records-plain-exit.md

- Outcome: `other`.
- Assessment: partly correct at 0db9621. Stop on a terminal-less (ambiguous-create) session takes the "already dead or
  absent" path in `crates/farhelm-supervisor/src/service/handlers.rs` and records a plain exit, without the stop note,
  before running the process cleanup, so a failed cleanup reports an error while the session already shows as exited.
  The finding's Restart half is obsolete: #1325 (`4a683aa4`) made Restart of an unknown-status agent proceed without a
  consent prompt per SPEC.md "Lifecycle operations", so losing the possibly-live marking no longer changes Restart. The
  remaining residual is that, after such a failed Stop, Delete no longer treats the session as possibly alive and so
  would not show SPEC.md's "anything still alive" confirmation before killing survivors. The rest matches FILTER.md
  filter "Session status and history after a crash or a partly failed operation", and the same finding from an earlier
  review was closed under it as `stop-terminal-less-records-exit-before-kill.md`.
- Decision: the user agreed the Delete residual does not take this out of the filter: the processes are ones the user
  just asked Stop to kill, so a Delete without the alive confirmation is not a meaningful loss of consent. Closed under
  the filter, consistent with the earlier duplicate.
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry during triage.

## probe-drops-add-busy-claim.md

- Outcome: `fix code`.
- Assessment: confirmed in code at 0db9621; practically unreachable. Confirming a rerun of a failed ADD marks the host
  busy (`crates/farhelm-helm/src/provisioning/service.rs`), re-registers it with database awaits, and only then installs
  the new run's progress view. A probe that finds a live supervisor in that gap goes through
  `resolve_failed_add_discovery`, which clears the busy marker unconditionally, although every failure path already
  clears it itself, so that removal only matters when another operation holds the claim. A second install or update
  could then be accepted instead of refused as busy, and the two runs would overwrite each other's progress display.
  Milder than reported: the first install adopts the live supervisor and finishes in seconds, a second update is one a
  user explicitly confirmed, and nothing persists. The trigger needs a second window (the panel's own guard prevents
  overlap within one), a supervisor that starts answering right after the rerun's own probe, a probe landing within a
  few milliseconds, and a further confirmation. Fits FILTER.md "Races a person would have to win inside a sub-second
  window" except that the operation not being refused is slightly outside its "recoverable through ordinary use"
  wording, so brought to the user.
- Decision: fix the code because the fix is a one-line removal.
- Completion criteria: `resolve_failed_add_discovery` no longer clears a busy marker it does not own (remove the
  redundant unconditional removal, keeping every failure path's own release); add a test showing a probe during a held
  ADD claim leaves the host busy so a second install or update is refused. Remove this feedback file and its index
  entry.
- Execution: complete: change `souyzyuxtoqk`, bookmark `triage-1001d/11-probe-keeps-busy`, PR
  https://github.com/scode/farhelm/pull/1407.

## retarget-race-republishes-old-client.md

- Outcome: `fix code`.
- Assessment: confirmed for the republish at 0db9621; a wrong-machine consequence is possible but extremely unlikely.
  The host actor's refresh loop (`crates/farhelm-helm/src/manager.rs`) checks for a pending retarget and then publishes
  its refresh unconditionally (`publish_refresh`), which can republish the connection the retarget just withdrew (and
  mint a fresh connection generation for it). The loop's next iteration sees the retarget and publishes "connecting", so
  the bad state lasts microseconds, and the old connection is already retired: its writer stops sending once it observes
  the shutdown. A frame reaches the old machine only if the writer has not run since the retire, a request arrives in
  that window, and the writer's unbiased select picks the frame over the shutdown. The retarget's own comment states the
  invariant this breaks (the old client "must not remain routable for even the interval"). The same finding from an
  earlier review, `refresh-publish-races-retarget-in-check-then-act-gap.md`, was closed as `other` under the rare-glitch
  filter without re-verification; verified now, no filter strictly applies because each excludes acting on the wrong
  machine. Not covered by the spec, `Planned` or `BUGS.md`.
- Decision: fix the code.
- Completion criteria: a refresh publication cannot republish a connection that a retarget has withdrawn: the publish
  writes only if the published connection is still this actor's own, decided atomically with the retarget's withdrawal
  (inside the same `send_modify` or equivalent). Add a deterministic regression using a gate seam like the existing
  `DuplicatePublicationGate` that lands a retarget between the refresh's check and its publish. Remove this feedback
  file and its index entry.
- Execution: complete: change `quvqzsxprssm`, bookmark `triage-1001d/12-retarget-race`, PR
  https://github.com/scode/farhelm/pull/1408.

## probe-register-not-helm-owned.md

- Outcome: `fix code`.
- Assessment: confirmed at 0db9621. The hosts panel's probe (`probe_host` in
  `crates/farhelm-helm/src/provisioning/http.rs`) runs on the request's own task, and when it finds a live supervisor,
  `register` (`provisioning/service.rs`) saves the host and then reconciles and dials it, rolling back only on a
  reconcile error; nothing on the probe path uses the helm's owned-task helper (`run_owned`), which only Update uses
  there. A request dropped between the save and the reconcile (reload, tab close, desktop Quit) leaves a saved host with
  no running actor, so it is absent from the hosts list (built from running actors) and never dialed; for an
  already-registered host whose paths the probe updated, the running actor keeps the old paths. Either lasts until
  something re-reads the host list (helm restart, a host edit, or another probe). Milder than reported: the UI adds
  hosts only through the probe, and re-probing the same destination updates the existing row rather than refusing it as
  a duplicate, so re-adding the host repairs it. The route comment in `lib.rs` still calls the probe non-mutating.
  Violates SPEC_impl.md "Who owns an accepted action"; the ledger's `host-edits-not-cancellation-safe.md` (#1196) fixed
  add, retarget, remove and adopt but not the probe. Not covered by `Planned`, `BUGS.md` or filters.
- Decision: fix the code.
- Completion criteria: the probe's post-discovery registration (`register` together with `resolve_failed_add_discovery`)
  runs on a helm-owned task, as #1196 did for the other host edits, so a dropped probe request loses only the reply;
  correct the route comment that calls the probe non-mutating. Add a regression that drops the request after the save
  and asserts the host is registered and dialed. Remove this feedback file and its index entry.
- Execution: complete: change `mxmoyukwsztm`, bookmark `triage-1001d/13-probe-registration-owned`, PR
  https://github.com/scode/farhelm/pull/1409.

## desktop-start-fails-on-skewed-supervisor.md

- Outcome: `fix code`.
- Assessment: confirmed at 0db9621. On launch the desktop app treats any supervisor answering in its state directory as
  already running, including one on another protocol version (`crates/farhelm-helm/src/provisioning/backend.rs`), so it
  spawns nothing (`crates/farhelm-ui/src/desktop.rs`); its startup wait then accepts only a Connected local host and
  runs to its 30 s limit, failing with "managed local supervisor did not connect within 30 seconds". The message names
  neither the version mismatch nor the hand-started supervisor, says "managed" though the app spawned nothing, and the
  window that would show the mismatch never opens, so every launch fails the same way until the user stops that
  supervisor. Identity-mismatch and unverifiable-identity states behave the same. The app already receives those states
  from the helm. The app shares the default state directory with a hand-run `farhelm supervisor run`; its own supervisor
  exits with the app, so the trigger is a hand-started supervisor left running across a protocol-changing upgrade.
  SPEC.md "Supported host setup" makes hand-started supervisors best-effort but says refusing with a clear message is
  enough, and incompatible versions must refuse with a clear, actionable error; neither holds. Not covered by `Planned`,
  `BUGS.md`, filters or the ledger.
- Decision: fix the code.
- Completion criteria: the desktop startup wait stops as soon as the local host reports a version mismatch or an
  identity mismatch or unverifiable identity, and fails with that state's details and an instruction to stop the
  supervisor the user started; the message does not say "managed" when the app spawned no supervisor. Opening the window
  instead of failing is out of scope. Add a unit test alongside the existing timeout-text test. Remove this feedback
  file and its index entry.
- Execution: complete: change `ltsploputryn`, bookmark `triage-1001d/14-desktop-refused-supervisor`, PR
  https://github.com/scode/farhelm/pull/1410.

## terminal-tombstone-never-buried.md

- Outcome: `fix code`.
- Assessment: confirmed at 0db9621. `sync()`'s departure and identity-change loop in
  `crates/farhelm-ui/assets/terminal.js` iterates islands, pending mounts and reconnects but not tombstones (the frozen
  post-takeover or post-stall screens created by `cancelReconnect(…, "restore")`), so a tombstoned terminal that leaves
  the desired set (its tab closes, or a stale host empties the session view) is never buried, contrary to the
  `tombstones` map's own comment that "a departed terminal takes its tombstone with it". Only full teardown and take
  control clear them. When the host returns, the recreated agent terminal has the same identity and is skipped as
  tombstoned before the Detached notice is painted, leaving a blank pane with no take-control action until the user
  leaves and reopens the session; each stranded tombstone also retains a live xterm instance for the life of the view.
  Not covered by the spec, `Planned`, `BUGS.md`, filters or the ledger.
- Decision: fix the code.
- Completion criteria: a tombstoned terminal that departs the desired set is buried (its xterm disposed and its map
  entry removed), with the identity lookup falling back to the tombstone so a departed tombstone is handled without
  error; a terminal that returns after its tombstone was buried mounts or shows its Detached notice normally. Add a JS
  or browser test covering departure and return of a tombstoned terminal. Remove this feedback file and its index entry.
- Execution: complete: change `olxrkyourypv`, bookmark `triage-1001d/15-tombstone-departure`, PR
  https://github.com/scode/farhelm/pull/1411.

## checkout-retry-raw-device-check.md

- Outcome: `fix spec+code`.
- Assessment: confirmed at 0db9621. A fresh-checkout create retried with the same idempotency key accepts the allocated
  folder through the remount-tolerant identity rule #1200 added (`working_copies::same_directory` via `verify_identity`
  in `crates/farhelm-supervisor/src/working_copies.rs`: the device number is required only when no birth time was
  recorded; otherwise inode plus birth time decide), but then hands the in-terminal launcher the stored original
  `(device, inode)` pair (`path_identity` in `crates/farhelm-supervisor/src/service/core.rs`), and the launcher's
  preparation check (`crates/farhelm-supervisor/src/launch.rs`) compares that pair raw. After a reboot or remount that
  renumbered the device (btrfs, NFS, overlayfs), the retry is refused as "the directory … was replaced since it was
  allocated", records Failed (overwriting even an intact Ready record, before taking the lock), and can never succeed;
  the user must start a new session and an orphan checkout folder remains. Only create builds the launcher's preparation
  input, so Restart is unaffected. This is an ownership check #1200 (`unstable-device-number-blocks-delete.md`) missed.
  The trigger is very rare. Not covered by `Planned`, `BUGS.md` or filters; the #1200 decision that these setups must
  not break argues for the fix.
- Decision: fix the code the way #1200 did, and state in SPEC.md that this is how Farhelm identifies directories, so no
  future check relies on device numbers. The user chose `fix code`; recorded as `fix spec+code` because the agreed scope
  includes the spec change.
- Completion criteria: the launcher's preparation check uses the same identity rule as `same_directory` (inode plus
  birth time, device number only when no birth time was recorded), either by carrying the birth time to the launcher or
  by handing it the folder's currently observed identity once `verify_identity` has accepted it; a replaced folder is
  still refused. Add a focused regression covering a retried create after a device-number change. Amend SPEC.md's
  managed-checkout section (beside "durable identity capture") to say that Farhelm identifies a directory it created by
  inode number plus creation time, using the device number only where no creation time is available, and explain why: on
  supported setups such as btrfs subvolumes (Fedora's default `/home`), NFS, overlayfs and some device-mapper
  configurations, the device number is assigned at mount time and can change across an ordinary reboot or remount while
  the folder is untouched, so a check that compares device numbers refuses the user's own folder as "replaced" and
  permanently breaks the operation guarding it, while inode plus creation time still detects a folder actually replaced
  at the same path, which is what these checks exist for. New ownership or identity checks must follow that rule rather
  than compare device numbers. Remove this feedback file and its index entry.
- Execution: complete: change `pnrqrqqzsyks`, bookmark `triage-1001d/16-checkout-retry-device`, PR
  https://github.com/scode/farhelm/pull/1412.

## takeover-latch-misses-attaching-tabs.md

- Outcome: `fix code`.
- Assessment: confirmed by code inspection at 0db9621 (not reproduced in a browser). `sync()` in
  `crates/farhelm-ui/assets/terminal.js` mounts newly seen tabs on the displacing attach route (`spec.path`, not the
  refuse-if-owned variant), and `mountWhenReady` usually mounts synchronously, so a tab whose socket is still connecting
  is a live island, not a pending mount; `latchTakeover` unmounts only pending mounts and reconnecting terminals, so
  that tab's displacing attach still reaches the supervisor, which treats any attach with a different lease as a
  takeover (`crates/farhelm-supervisor/src/service/terminals.rs`, `service/handlers.rs`) and evicts the window that just
  took over. A cancelled pending mount is also left blank with no banner until the desired terminal set changes.
  Unmounting at latch time alone only narrows the window. Contradicts SPEC.md's one-attached-client rule, the displaced
  client's snapshot and take-control action, and the rule that a self-recovering terminal never takes the session.
  Trigger: a tab starting to attach within about one round trip of another window's takeover (a person opening a tab, or
  the 3 s poll finding a tab another actor created). Not covered by the spec, `Planned`, `BUGS.md` or filters; the
  planned "concurrent GUIs are best effort" principle (`header-replace-recomputes-alive.md`) keeps the takeover rules.
  Shares its root cause with `new-tab-mount-displaces-owner-during-recovery.md`.
- Decision: fix the code with one shared fix for this item and `new-tab-mount-displaces-owner-during-recovery.md`, in a
  single PR covering both.
- Completion criteria: after a session view's initial open, every newly seen tab attaches on the route that is refused
  when another client holds the session; only opening the session and an explicit take-control or reconnect action use
  the displacing route, and a refusal lands in the normal latched "Detached … take control" state, including for a
  pending mount cancelled by the latch. Add browser regressions on Chromium and WebKit using the existing two-client
  takeover setup for a tab attaching during another window's takeover. One PR together with
  `new-tab-mount-displaces-owner-during-recovery.md`, removing both feedback files and index entries.
- Execution: complete: change `pxrpnmmqvplq`, bookmark `triage-1001d/17-tab-attach-respects-takeover`, PR
  https://github.com/scode/farhelm/pull/1414.

## new-tab-mount-displaces-owner-during-recovery.md

- Outcome: `fix code`.
- Assessment: confirmed by code inspection at 0db9621 (not reproduced in a browser). The takeover latch in
  `crates/farhelm-ui/assets/terminal.js` is set only when a takeover notice arrives on a live socket, and `sync()` skips
  only reconnecting or latched elements, so a tab first seen while the view is recovering from a dropped connection
  (laptop sleep or network loss, while the user took over on another device and opened a tab there) mounts on the
  displacing route and silently evicts the device in use; the view's own later reconnect attempts then succeed under the
  same lease. After a long outage the 3 s fallback poll almost surely sees the tab first; after a wake it races the
  first 500 ms reconnect attempt. The extra triggers the reviewer mentioned (32-terminal cap, a remounted tombstone)
  were not traced. Contradicts the same SPEC.md takeover rules as `takeover-latch-misses-attaching-tabs.md`, with which
  it shares its root cause. Not covered by the spec, `Planned`, `BUGS.md` or filters.
- Decision: fix the code with the shared fix described under `takeover-latch-misses-attaching-tabs.md`, in the same PR.
- Completion criteria: as for `takeover-latch-misses-attaching-tabs.md`, plus a browser regression where a view
  recovering from a dropped connection sees a tab created by the client that took over and does not displace it. Same PR
  as that item.
- Execution: complete: change `pxrpnmmqvplq`, bookmark `triage-1001d/17-tab-attach-respects-takeover`, PR
  https://github.com/scode/farhelm/pull/1414.

## update-silently-downgrades-newer-hosts.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by code inspection at 0db9621 (not reproduced). Update's planning and revalidation
  (`plan_update_unguarded` and the revalidation in `crates/farhelm-helm/src/provisioning/service.rs`) ignore the host's
  probed build version; the UI offers Update on any provisionable remote row and "update all" takes every such row
  (`crates/farhelm-ui/src/provisioning.rs`, `crates/farhelm-ui/src/hosts.rs`); the binary is replaced in place (`mv -f`
  in `provisioning/backend.rs`), keeping no copy of the newer one. An older supervisor refuses a database with a newer
  schema (`crates/farhelm-supervisor/src/store.rs`) and its unit restarts on failure, so a downgrade across a schema
  change leaves the host unreachable and unmanaged (its agents keep running) until the newer build is reinstalled; the
  run fails after the 30 s attach timeout. The helm already computes "host older than me" for the `old version`
  advisory, so the reverse comparison is cheap. Inferred from SPEC_impl.md and not traced in code: a protocol skew shows
  "needs update" whichever side is newer, which invites the downgrade. Realistic triggers: a rolled-back helm, a host
  updated by its own installer, switching between RC/dev and stable helms, or two helms of different versions used in
  turn. SPEC.md's one-helm-at-a-time rule covers only concurrent helms; TODO.md's pre-upgrade backup (`Maybe later`) and
  multi-helm (`Unbucketized`) entries name the risk but are not `Planned`.
- Decision: fix the code and the spec. The host list must not merely stop showing "needs update" or "old version" for
  such a host: it must say the host runs a future (newer) version than the helm, so the user can tell something is off.
- Completion criteria: the helm refuses Update of a host whose probed build is newer than its own with a clear error
  naming both versions; the hosts panel hides Update on such rows and "update all" skips them; the host list labels such
  a host "too new" instead of "needs update" or "old version", including when the newer version also differs in
  protocol; the label stays that short for the host list's layout, and hovering it shows the full information: the
  helm's version and what the helm knows from the host's supervisor, such as its version and both protocol versions
  (user decision at planning time, 2026-10-01). Amend SPEC.md (the Update authorization text and the host version
  advisories) to say Update never downgrades a host, and that a host newer than the helm is shown as "too new". Add helm
  and UI tests for the refusal, the skipped "update all" row, and the displayed state. Remove this feedback file and its
  index entry.
- Execution: complete: change `kkruwynrypom`, bookmark `triage-1001d/18-update-never-downgrades`, PR
  https://github.com/scode/farhelm/pull/1418.

## confirmed-nothing-alive-prompt-kills-live-agent.md

- Outcome: `fix spec+code`.
- Assessment: confirmed for the sidebar Delete prompt, partly correct for the header Restart prompt, by code inspection
  at f087e0b6 (not reproduced). The sidebar's inline delete prompt rewords itself from the row's current status on every
  render and stays open while that status changes, but its confirm always sends a delete without the
  `only_if_nothing_alive` precondition (`confirm_delete` in `crates/farhelm-ui/src/list/view.rs`), so a prompt that
  drifted to "delete anyway" deletes an agent restarted meanwhile by the CLI, another window or an agent. A prompt that
  warned only about tabs has the same gap for a newly restarted agent. Header Delete already captures the value from the
  render that drew its prompt. The header Restart path needs the prompt to drift to interrupted or error and a restart
  by someone else before the click; its cost is a restart, not a deletion. The page's live feed normally narrows the
  window to moments; it widens when the feed is down. SPEC.md "Lifecycle operations" applies the binding reading to
  Restart only and is silent on Delete and on a prompt that changes while open. Not covered by `Planned`, `BUGS.md`,
  filters or plan items (the YOLO/Replace plan touches Replace only).
- Decision: a destructive confirmation authorizes only what the prompt the user answered said would happen; by default
  Farhelm does not accept races where a confirmation is applied to a state the user was not shown (user, 2026-10-01).
- Completion criteria: SPEC.md states the principle generally for destructive confirmations (Delete, Restart, Replace,
  Replace with, and any prompt that rewords while open): the request carries the precondition the answered prompt
  implied, and the server refuses when the current state exceeds it, so the next attempt asks again. The sidebar Delete
  confirm sends `only_if_nothing_alive` and `stop_if_running` derived from the prompt actually rendered, as header
  Delete does; header Restart's confirm sends `stop_if_running` only when the answered prompt offered to stop a working
  agent. Add UI tests for a prompt whose wording drifted. Remove this feedback file and its index entry.
- Decision (plan time, 2026-10-01, refines the above): Delete has no stop consent and only one precondition, so the user
  authorized a second, narrower precondition, "only if the agent has ended", on the Delete and Replace requests (proto,
  helm and supervisor), chosen by the UI from the prompt the user answered: nothing alive sends `only_if_nothing_alive`,
  a tabs-only warning sends the new one, a warning that the agent runs sends neither. It applies to header Delete as
  well. Accepted: a tab opened between the prompt and the click is still closed; naming the exact tabs shown is not
  required.
- Execution: complete: change zmnvlqmmpmus, bookmark `triage-1001e/01-confirm-binds-prompt`, PR
  https://github.com/scode/farhelm/pull/1441.

## replace-with-kills-running-source-unwarned.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by code inspection at f087e0b6 (not reproduced). "Replace with" opens the launcher straight from
  the row menu with no confirmation and no indication of the source's status; its request (`replace_session_with` and
  the fresh-checkout variant in `crates/farhelm-ui/src/api.rs`) omits `only_if_nothing_alive`, which the helm defaults
  to false, so the source's agent and tabs are killed unconditionally. Plain Replace sends the flag. Because the
  launcher stays open while the user edits settings, a source that was exited when it opened and was restarted meanwhile
  has its new run killed silently. SPEC.md does not say whether the launcher's button is a confirmation. Not covered by
  spec, `Planned`, `BUGS.md`, filters or plans.
- Decision: as for `confirmed-nothing-alive-prompt-kills-live-agent.md`: a confirmation authorizes only what it said
  (user, 2026-10-01).
- Completion criteria: SPEC.md's "Replace with" text says the launcher shows Replace's consequence text whenever the
  source has anything alive, and that launching carries the precondition matching what the launcher showed; the launch
  is refused, and asks again, when the source's state has since grown. The UI shows that text and sends
  `only_if_nothing_alive` (or the matching stop consent) from what the launcher displayed. Add UI tests for a running
  source and for a source restarted while the launcher was open. Remove this feedback file and its index entry.
- Decision (plan time, 2026-10-01, refines the above): the precondition levels from
  `confirmed-nothing-alive-prompt-kills-live-agent.md` apply. "Refused" keeps Replace's existing semantics, which the
  user accepted: the replacement is created first and a source that no longer matches what the launcher showed is kept,
  with the existing both-sessions-exist error; no liveness check before the create. SPEC.md is written to match. The
  fresh-checkout launch fixes the precondition into its retained, replayable payload at launch time.
- Execution: complete: change `xywmtprl`, bookmark `triage-1001e/02-replace-with-binds`, PR
  https://github.com/scode/farhelm/pull/1443. Deviation, decided during execution: the fresh-checkout launch does not
  fix the precondition at the first press; every press, retries included, sends the precondition matching the warning
  shown at that press, because the helm's idempotency for a fresh replacement covers only the source and the create, and
  a frozen precondition could contradict the warning on screen.

## claude-scan-claims-foreign-record.md

- Outcome: `other`.
- Assessment: confirmed by code inspection at f087e0b6 (not reproduced). Claude's record-scan fallback, used when a
  Claude session has no accepted hook report, claims a lone record inside its correlation window after filtering only by
  recorded working directory and by ids other Farhelm sessions reported; a `claude` run outside Farhelm in the same
  folder is never a rival, so Resume can open and append to another process's conversation. The comment in
  `crates/farhelm-supervisor/src/agent_kind/capture.rs` claiming a wrong claim is impossible is false. Likelihood is low
  (an unhooked session plus a foreign record at the right moment), higher if the terminal's startup query reply counts
  as first input (unverified). Highest bucket by SPEC.md "First-class harnesses" (resuming the wrong conversation is
  lost user work).
- Decision: Farhelm identifies an agent's conversation only from the harness's own explicit report, through a hook,
  plugin, extension or whatever reporting mechanism the harness needs; heuristic fallbacks that cannot be relied upon,
  such as correlating vendor files on disk, are not supported. Change SPEC.md accordingly, and record a near-term TODO
  to remove the remaining fallback code. Feedback that is only true because that code still exists is discarded (user,
  2026-10-01).
- Completion criteria: SPEC.md "Durability and resume" (and the matching SPEC_impl.md capture text) states the
  principle: conversation identity comes only from an explicit report; a launch whose harness cannot report, or whose
  report never arrived, has no captured identity and takes the existing uncaptured-identity fallback; the Claude record
  scan is named as pending removal under TODO.md's near-term entry rather than as supported behavior. Remove this
  feedback file and its index entry. The code removal itself is the TODO entry, not this item.
- Execution: complete: change `rosywsko`, bookmark `triage-1001e/03-identity-only-reported`, PR
  https://github.com/scode/farhelm/pull/1444.

## claude-scan-budget-never-settles.md

- Outcome: `discard`.
- Assessment: confirmed by code inspection at f087e0b6. The Claude record scan charges every directory entry against its
  budget before the age cutoff, and an incomplete scan neither commits nor gives up, so an unhooked Claude session in a
  project folder with more than about 4096 entries never offers Resume and is rescanned every 2 s. Rare.
- Decision: only true because the Claude record scan still exists; removed with it under the near-term TODO from
  `claude-scan-claims-foreign-record.md` (user, 2026-10-01).
- Completion criteria: remove the feedback file and its index entry, without code or spec changes.
- Execution: complete: change `xxxluwpp`, bookmark `triage-1001e/04-discard-scan-budget`, PR
  https://github.com/scode/farhelm/pull/1445.

## claude-capture-warns-forever.md

- Outcome: `discard`.
- Assessment: confirmed by code inspection at f087e0b6. Re-verification of a scan-captured Claude record logs a WARN and
  changes nothing when the transcript is missing, so an exited session whose transcript Claude cleaned up or the user
  deleted logs a warning every capture pass, across supervisor restarts. Only scan-captured Claude sessions are
  affected.
- Decision: only true because the Claude record scan still exists; removed with it under the near-term TODO from
  `claude-scan-claims-foreign-record.md`, which also covers re-verification of records the scan captured earlier (user,
  2026-10-01).
- Completion criteria: remove the feedback file and its index entry, without code or spec changes.
- Execution: complete: change `mppquzyn`, bookmark `triage-1001e/05-discard-transcript-warning`, PR
  https://github.com/scode/farhelm/pull/1446.

## ssh-forwarding-inherited.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by code inspection at f087e0b6; the live exposure was not reproduced. Every helm ssh invocation
  (the always-on supervisor connection and every provisioning step) builds its arguments from `ssh_base_args` in
  `crates/farhelm-helm/src/ssh.rs`, which sets batch mode, connection sharing and the destination and nothing about
  forwarding, so a `ForwardAgent yes` or `ForwardX11 yes` in the user's ssh config applies to Farhelm's permanent
  connection. `ssh -G` confirmed command-line `ForwardAgent=no` and `ClearAllForwardings=yes` override the config. Agent
  exposure needs the helm process to see an ssh agent (typical on macOS, deployment-dependent on Linux); X11 needs a
  display. Any agent on such a host could then use the user's keys around the clock. SPEC.md says a remote host must not
  gain access to secrets on the helm's machine or another host, while SPEC_impl.md lists agent forwarding among the ssh
  config features real ssh honors. Not covered.
- Decision: Farhelm's own ssh connections never forward the agent, X11 or ports, whatever the user's ssh config says;
  the config still governs reaching the host (user, 2026-10-01).
- Completion criteria: SPEC.md states that Farhelm's own connections honor the user's ssh config for reaching and
  authenticating to the host (keys, the agent used for authentication, ProxyJump, Match blocks) but never forward the
  agent, X11 or ports; SPEC_impl.md's list of honored features is corrected. Both branches of the shared argument prefix
  add `ForwardAgent=no`, `ForwardX11=no` and `ClearAllForwardings=yes`, with argument tests updated; confirm during
  execution that `ClearAllForwardings` leaves ProxyJump working. Remove this feedback file and its index entry.
- Execution: complete: change `xxwsvlnt` (shared with the other ssh outcome, by plan decision P2), bookmark
  `triage-1001e/06-ssh-overrides`, PR https://github.com/scode/farhelm/pull/1447.

## ssh-config-remotecommand-blocks-host.md

- Outcome: `fix spec+code`.
- Assessment: confirmed and reproduced with OpenSSH 9.6: a `RemoteCommand` in the user's ssh config makes ssh refuse
  Farhelm's own remote command ("Cannot execute command-line and remote command.", exit 255) before connecting, so such
  a host can never be added, set up, updated or connected to, and the hosts panel shows the generic handshake hint that
  suggests starting a supervisor. `-o RemoteCommand=none` gets past the check. Whether `RequestTTY force` would also
  corrupt the protocol stream is unverified. Rare.
- Decision: same principle as `ssh-forwarding-inherited.md`: Farhelm's own connections override ssh settings that
  conflict with running its own remote command (user, 2026-10-01).
- Completion criteria: the same SPEC.md statement names `RemoteCommand` (and a forced TTY, if execution confirms it
  matters) as overridden; the shared argument prefix adds `RemoteCommand=none` and, if needed, `-T`, with argument tests
  updated. May share a PR with `ssh-forwarding-inherited.md`. Remove this feedback file and its index entry.
- Execution: complete: change `xxwsvlnt` (shared with the other ssh outcome, by plan decision P2), bookmark
  `triage-1001e/06-ssh-overrides`, PR https://github.com/scode/farhelm/pull/1447.

## merged-list-crowded-by-one-host.md

- Outcome: `fix spec`.
- Assessment: confirmed by code inspection at f087e0b6. The helm merges, filters, sorts and then cuts the fleet list to
  500 rows, checking only the row cap, id length and duplicates at ingress and clamping no timestamps, so a host that
  reports about 500 sessions that sort first (future creation times, always running, or titles) pushes every other
  host's sessions out of the all-hosts list and agents' listings. Per-host views still show them, routing by id still
  works, and the rows carry the hostile host's name. Needs a hostile host; an honest one would need a fleet size SPEC.md
  puts out of scope. Filed as highest; not a security or data-loss consequence, so `other`.
- Decision: when a misbehaving remote host's effect is limited to things like spamming the UI so other sessions are hard
  to reach, removing the host is the remedy; only effects that break the helm or affect the security of other hosts must
  be prevented (user, 2026-10-01).
- Completion criteria: SPEC.md "Remote input, session defaults, and availability" generalizes the session-ownership
  carve-out: a misbehaving host may crowd or clutter what the helm and GUI show, including pushing other hosts' sessions
  out of the merged list, and removing it is the remedy; it must still not break the helm or affect other hosts'
  security. Remove this feedback file and its index entry.
- Execution: complete: change `nvrutkop`, bookmark `triage-1001e/07-crowding-accepted`, PR
  https://github.com/scode/farhelm/pull/1448.

## output-client-shutdown-can-retry-forever.md

- Outcome: `other`.
- Assessment: confirmed in code at f087e0b6, unreachable through supported use. The per-terminal output client retries
  shutdown forever if its tmux session disappears while output is paused, but every production session kill (Delete,
  create rollbacks, a relaunch racing a delete) first stops or never had attachments, and a `remain-on-exit` session
  does not vanish on its own; the remaining trigger is killing the session through Farhelm's private tmux server
  directly.
- Decision: skipped under the triage rule for behavior the specification already accepts. SPEC.md "Ownership during
  cleanup and provisioning" puts changes made through the private tmux server outside every guarantee and rules out
  added recovery for them; the user confirmed not spending non-trivial complexity on direct poking at the internal tmux,
  while trivial guards against mistakes remain fine (2026-10-01).
- Completion criteria: remove the feedback file and its index entry immediately, without code or spec changes.
- Execution: `complete`; removed the feedback file and index entry locally during triage.

## attach-reports-generic-timeout.md

- Outcome: `fix code`.
- Assessment: confirmed by code inspection at f087e0b6. After provisioning a host, the attach wait recognizes only
  "connected with a new connection number" or a vanished connection worker; version skew, identity mismatch, unverified
  identity and duplicate states all spin for the full 30 s and fail with "waiting for the provisioned supervisor: timed
  out", holding the host lock and a run slot meanwhile. The host row shows the real state, the run's error does not.
  Realistic trigger: re-adding a host whose machine was reinstalled or whose state was wiped. Contradicts SPEC.md
  "Errors and diagnostics"; same shape as the planned `desktop-start-fails-on-skewed-supervisor.md` fix.
- Decision: fix the code; the user approved fixing the clear-cut small items, with a gate: anything that turns into a
  significant complexity increase or refactor comes back to the user first (2026-10-01).
- Completion criteria: the attach wait stops early on skew, identity mismatch, unverified identity and duplicate, and
  reports the state and its remedy, reusing the existing update-trust wording; add tests. Remove this feedback file and
  its index entry. Stop and ask before implementing if this needs significant new complexity or a refactor.
- Execution: deferred: the complexity gate tripped. The fix needs the connection manager to tie each published refusal
  to the reconnect request current when its attempt started, with a deterministic test of an attempt in flight across
  the request; recorded as the TODO.md `Near term` entry "Name the real refusal when a provisioned host will not
  attach". Change `wmnrrwur`, bookmark `triage-1001e/08-attach-names-refusal`, PR
  https://github.com/scode/farhelm/pull/1449.

## folder-picker-skips-symlinks.md

- Outcome: `fix code`.
- Assessment: confirmed by code inspection at f087e0b6. The supervisor's directory browse keeps an entry only when its
  unfollowed file type is a directory, so symlinked folders (`~/src` pointing elsewhere) never appear in the create
  dialog's picker, and a `?` on a per-entry type lookup fails the whole listing (rare on Linux). Typing the path works.
  SPEC.md promises a directory picker; nothing addresses symlinks.
- Decision: fix the code under the same gate as `attach-reports-generic-timeout.md` (user, 2026-10-01).
- Completion criteria: the picker lists symlinks that resolve to directories, and an entry whose type cannot be read is
  skipped rather than failing the listing; add a symlink test. Following links can block on a wedged mount, which the
  existing browse worker and permit design tolerates. Remove this feedback file and its index entry. Stop and ask before
  implementing if this needs significant new complexity or a refactor.
- Execution: complete: change `xkpplqww`, bookmark `triage-1001e/09-picker-follows-symlinks`, PR
  https://github.com/scode/farhelm/pull/1450.

## tilde-in-remote-path-fields.md

- Outcome: `fix code`.
- Assessment: confirmed by code inspection at f087e0b6. The hosts panel's remote farhelm path and remote state dir are
  single-quoted into the probe script and the connection command, so `~` is never expanded; the helm only checks the
  value is non-empty. `~/.local/bin/farhelm` then probes as "not installed" and offers setup on a host where Farhelm is
  installed, and a state dir of `~/x` becomes `$HOME/~/x`. Update already refuses a non-absolute binary path.
- Decision: fix the code under the same gate as `attach-reports-generic-timeout.md` (user, 2026-10-01). The fix refuses
  rather than expands, matching Update's existing absolute-path rule.
- Completion criteria: the helm refuses a non-absolute (including `~`-prefixed) remote farhelm path or state dir at the
  API boundary with a message asking for an absolute path, and the UI shows that refusal; add tests. Remove this
  feedback file and its index entry. Stop and ask before implementing if this needs significant new complexity or a
  refactor.
- Decision (plan time, 2026-10-01, refines the above): a bare program name in the remote farhelm field (resolved through
  the remote PATH) is an existing, tested form and stays valid; refuse a leading `~` and a relative value containing `/`
  there, and anything non-absolute in the remote state dir field.
- Execution: complete: change `ltmkrwvy`, bookmark `triage-1001e/10-remote-paths-absolute`, PR
  https://github.com/scode/farhelm/pull/1452.

## drop-on-hidden-terminal-navigates-away.md

- Outcome: `fix code`.
- Assessment: confirmed by code inspection at f087e0b6 (browser navigation not observed). Drop and dragover handlers
  exist only on the terminal element, which is hidden while catching up or reconnecting, and no page-wide handler
  cancels the default, so a file dropped on a recovering terminal, or anywhere outside a live terminal, makes the web
  page navigate to the file: every terminal detaches and in-flight uploads abort. Back or reload recovers. SPEC.md
  Attachments classifies drops into any of a session's terminals.
- Decision: fix the code under the same gate as `attach-reports-generic-timeout.md` (user, 2026-10-01).
- Completion criteria: a page-wide dragover/drop handler cancels the browser default so a stray drop never navigates the
  app; a drop on a pane with no live terminal shows the "not connected" outcome SPEC.md implies rather than nothing. Add
  a browser regression. Remove this feedback file and its index entry. Stop and ask before implementing if this needs
  significant new complexity or a refactor.
- Execution: complete: change `qrutwxmx`, bookmark `triage-1001e/11-stray-drop-guard`, PR
  https://github.com/scode/farhelm/pull/1453.

## partial-release-download-left-behind.md

- Outcome: `fix code`.
- Assessment: confirmed by code inspection at f087e0b6. The helm's release download removes its `.part` file only on an
  oversize or checksum failure; a body-stream error or a write, flush or fsync error returns early and leaves it.
  Bounded to one partial file per asset, overwritten by the next attempt and swept after a helm restart, and never
  mistaken for a verified download. The filesystem-error paths are arguably covered by SPEC.md "Healthy local
  filesystems"; the network-error path is not.
- Decision: fix the code under the same gate as `attach-reports-generic-timeout.md` (user, 2026-10-01).
- Completion criteria: every failed download removes its partial file using the existing cleanup helper; add a test for
  a mid-stream failure. Remove this feedback file and its index entry. Stop and ask before implementing if this needs
  significant new complexity or a refactor.
- Execution: complete: change `vkzmokqy`, bookmark `triage-1001e/12-partial-download-cleanup`, PR
  https://github.com/scode/farhelm/pull/1454.

## incarnation-counter-restarts-per-process.md

- Outcome: `fix code`.
- Assessment: confirmed by code inspection at f087e0b6. A host connection's incarnation number starts at 1 in every helm
  process and the create precondition compares only the number, so a client that has not yet noticed a helm restart (for
  example a laptop woken with the create dialog open) can submit a stale number that happens to match a retargeted or
  adopted host's new connection, and the session launches on the replacement machine instead of being refused. Compound
  and unlikely, but the consequence is a launch on the wrong machine.
- Decision: fix the code under the same gate as `attach-reports-generic-timeout.md` (user, 2026-10-01).
- Completion criteria: incarnation numbers do not repeat across helm processes in practice (for example a per-process
  random or time-derived starting value kept below 2^53 for JSON safety); add a test. Remove this feedback file and its
  index entry. Stop and ask before implementing if this needs significant new complexity or a refactor.
- Execution: complete: change `xztkvzvk`, bookmark `triage-1001e/13-incarnation-random-start`, PR
  https://github.com/scode/farhelm/pull/1455.

## sessions-changed-hint-unthrottled.md

- Outcome: `fix spec+code`.
- Assessment: confirmed mechanism by code inspection at f087e0b6; impact unmeasured. Each "sessions changed" hint from a
  supervisor only bumps a counter on the helm, a refresh that answers a hint always raises a fleet-wide change event,
  and the helm refreshes again at once if another hint arrived meanwhile. The only pacing is the supervisor's own 200 ms
  minimum gap between hints, a sender-side rule. A hostile host can therefore drive back-to-back refreshes at network
  round-trip rate, each making every open client re-read the session list and every open session view re-read its host's
  full list; clients coalesce to one read in flight, so the cost is rate-bounded rather than queued. Cannot happen with
  an honest supervisor. Filed as highest; it is a resource and availability concern with a hostile-only trigger, so
  `other`.
- Decision: fix the code, and also state in SPEC.md that a misbehaving host degrading the helm's performance or
  availability (denial-of-service-style behavior) is accepted when it cannot easily be avoided: Farhelm avoids such
  effects where it reasonably can, but does not spend elaborate complexity to do so (user, 2026-10-01).
- Completion criteria: the helm enforces, per host connection, the same minimum gap between hint-driven refreshes that
  the supervisor promises (`HINT_MIN_GAP`), with at most one pending refresh, so a hostile host costs no more than a
  busy honest one; add a test with a flooding peer. SPEC.md "Remote input, session defaults, and availability" gains the
  principle above, alongside the misbehaving-host remedy recorded under `merged-list-crowded-by-one-host.md`. Remove
  this feedback file and its index entry.
- Execution: complete: change `qzvqsxoo`, bookmark `triage-1001e/14-hint-pacing`, PR
  https://github.com/scode/farhelm/pull/1457.

## claude-resume-template-selector-collision.md

- Outcome: `fix spec+code`.
- Assessment: confirmed in code at f087e0b6; Claude's handling of the conflicting flags was not checked. Claude's
  derived resume template (`default_resume_template` for Claude in `crates/farhelm-supervisor/src/agent_kind/mod.rs`) is
  the original argv plus `--resume {conversation}`, and its `ambiguous_derived_resume` always returns `None`, so
  `claude --continue` restarts as `claude --continue --resume <id>` and a command containing `--` restarts with the
  resume flag in prompt position. If Claude honors the original `--continue`, Resume opens the folder's most recent
  conversation, harmful when another, newer conversation exists in the same folder, and the hook then records whatever
  opened, replacing the valid offer; with `--`, Resume starts fresh or fails to launch. Built-in profiles never use
  these flags; raw commands and custom profiles can. Same shape as `codex-resume-template-duplicates-selector.md`.
- Decision: apply the Codex and Grok rule to Claude: refuse at creation when the original command already selects a
  conversation or contains a real `--`, unless an explicit resume command is supplied. The user chose refusal at
  creation over allowing the launch and withholding Resume, after being told it blocks a raw `claude --continue` launch.
  Also add this case as an example to TODO.md's `Near term` entry "Re-examine and simplify how launches are represented"
  (done during triage) (user, 2026-10-01).
- Completion criteria: Claude's `ambiguous_derived_resume` refuses a derived template when the retained argv carries
  `--continue`/`-c`, `--resume`/`-r`, `--session-id`, or a real end-of-options `--`, so the create fails with a clear
  message unless an explicit resume template is supplied, mirroring `GrokAmbiguousResumeBoundary`. Amend SPEC.md's
  derived-resume paragraph to list Claude beside Grok and Codex. Add regressions for an original `claude --continue` and
  a `--` launch, with and without an explicit template. Remove this feedback file and its index entry.
- Execution: complete: change `pmkorkrq`, bookmark `triage-1001e/15-claude-resume-selector`, PR
  https://github.com/scode/farhelm/pull/1458. The TODO.md example was added during triage.

## claude-clear-report-dropped-on-claim-timeout.md

- Outcome: `fix code`.
- Assessment: confirmed in code at f087e0b6; real-host lock hold times not measured. A conversation-identity report
  waits at most 1 s (`CAPTURE_CLAIM_WAIT` in `crates/farhelm-supervisor/src/service/core.rs`) for the session's capture
  claim and is otherwise refused with `Conflict` and nothing written. Claude's hook makes one round trip and never
  retries, Claude only reports on `SessionStart`, and the periodic refresh pass does nothing for Claude, so after a
  `/clear` whose report was refused, Resume reopens the cleared conversation until the next `SessionStart`. The code
  comments justifying the bound ("the reporter retries … the refresh pass converges the row") do not hold for Claude.
  Needs the claim held for over 1 s (slow disk, or two reports within a second); rare on a healthy host. Codex's report
  path uses the same wait and was not traced.
- Decision: a valid identity report from the correct, verified source must not be dropped because it arrived while the
  session's record was busy; it is applied once the record is free (user, 2026-10-01).
- Completion criteria: a report that passes admission is applied after contention clears rather than refused after a
  fixed wait, for Claude and for any other kind sharing the path (check Codex's); the hook's own bounded wait still
  keeps the agent from being held up. Correct the comments that claim a retry or refresh converges the row. Add a
  regression with the claim held past the old bound. Remove this feedback file and its index entry.
- Decision (plan time, 2026-10-01, supersedes the code fix above): planning found the fix needs a reorder of the
  documented report-admission steps (both simple approaches fail: a longer wait overruns the hook's 2 s budget, and a
  deferred write cannot redo Claude's live-process attribution). The user chose to promote it to a `Near term` TODO.md
  entry referencing this review item and explaining the complication, instead of fixing it now. The feedback file stays,
  referenced by that entry.
- Execution: `deferred`; promoted to TODO.md's `Near term` entry "Apply identity reports that arrive while the session's
  record is busy" during planning. The feedback file and its index line stay until that entry is done.

## omp-corridor-uncounted-pane-runtime.md

- Outcome: `fix code`.
- Assessment: confirmed in code at f087e0b6; the trigger depends on an unverified premise. OMP report attribution
  classifies the processes between the reporter and the session's pane, needs readable command-line arguments to
  recognize a Bun-run OMP runtime, drops a command line over 64 KiB, and never classifies the pane process itself
  (`crates/farhelm-supervisor/src/procs/omp.rs`). For a chain of reporter, nested OMP, then an unreadable pane runtime,
  the nested OMP is taken as the foreground and its report accepted, so Resume reopens the nested conversation. Needs an
  OMP launch command line over 64 KiB plus a nested interactive OMP that loads the gated reporter (unverified, since
  injection is a per-launch `-e`). SPEC.md and SPEC_impl.md already require refusing nested or unclassifiable runtimes
  and treating over-budget arguments as missing. Highest by consequence (SPEC.md "First-class harnesses" keeps resuming
  the wrong conversation in scope for non-first-class harnesses); likelihood negligible.
- Decision: fix the code with a complexity gate: if the fix turns out complicated, abandon it and record a `Near term`
  TODO.md entry describing the problem for the maintainer's triage instead (user, 2026-10-01).
- Completion criteria: for an `omp` launch, attribution refuses when the pane process is a Bun or Node runtime that is
  not the emitter or whose arguments cannot be read, with a unit test for the unreadable-pane chain; remove this
  feedback file and its index entry. If that turns out complicated, instead add a `Near term` TODO.md entry describing
  the problem and remove this feedback file and its index entry in the same change.
- Execution: complete: change `qmlmzzyp`, bookmark `triage-1001e/16-omp-pane-runtime`, PR
  https://github.com/scode/farhelm/pull/1459.

## process-snapshot-requires-supervisor-witness.md

- Outcome: `fix code`.
- Assessment: confirmed by code inspection at f087e0b6. The process-table snapshot passes an empty result straight
  through on Linux (an empty `/proc`) and macOS (a zero-size kernel answer), with no check that the supervisor's own pid
  is present, contradicting the module's fail-closed contract. Every process selection starts from a table row, so a
  missing table can only cause a missed kill, never a wrong one: on a host without a usable systemd user manager, Stop
  can record "stopped" while the agent keeps running and Delete can leave descendants behind. Hosts with a manager are
  unaffected (the scope kill does not use the table). The trigger is practically impossible on a working host. SPEC.md
  "Lifecycle operations" already requires that a process Farhelm could not examine counts as unconfirmed. Also tracked
  as TODO.md's "Snapshot self-witness" entry. A move from highest to `other` was suggested and not decided.
- Decision: fix the code, strictly scoped to the specific case of the snapshot not seeing the supervisor itself; this is
  not an opening to address processes that hide their environment or otherwise escape discovery (user, 2026-10-01).
- Completion criteria: a snapshot that does not contain the supervisor's own pid is an error ("could not look"), so
  cleanup relying on it is unconfirmed rather than successful; add a pure test. No broader discovery changes. Remove
  TODO.md's "Snapshot self-witness" entry, this feedback file and its index entry.
- Execution: complete: change `rvukuktu`, bookmark `triage-1001e/17-snapshot-self-witness`, PR
  https://github.com/scode/farhelm/pull/1467.

## probe-cancellation-leaves-helper-processes.md

- Outcome: `fix code`.
- Assessment: partly correct, by code inspection at f087e0b6. The host probe runs on the HTTP request's own task; the
  backend isolates the probe child in its own process group but kills the group only on its normal paths, so a dropped
  request (page closed or reloaded during a probe of up to 15 s) fires only `kill_on_drop` on the direct child and
  leaves the stderr reader task unowned. The claimed consequence is overstated: a local probe's child starts no helpers,
  plain ssh ends when killed, and ControlPersist masters are outside the group either way; what can survive is a
  user-configured `ProxyCommand` that does not exit with its ssh. Only helm-side helper processes, never user work on a
  host. Related planned item: `probe-register-not-helm-owned.md` (plan item 13 in
  `plans/triage-restart-takeover-update.md`) moves registration, not the backend probe, onto a helm-owned task. Filed as
  highest; recommended `other` (bucket move not explicitly decided).
- Decision: fix the code (user, 2026-10-01).
- Completion criteria: the whole probe runs on a helm-owned task (`run_owned`, as plan item 13 uses), so a dropped
  request still runs the probe's own process-group cleanup within its timeout; add a dropped-request regression. Land
  alongside or after plan item 13. Remove this feedback file and its index entry.
- Execution: complete: change `otwupulm`, bookmark `triage-1001e/18-probe-owned`, PR
  https://github.com/scode/farhelm/pull/1460.

## pi-pointer-overrides-user-prompt.md

- Outcome: `discard`.
- Assessment: incorrect for the Pi version Farhelm verified against. Farhelm's Pi integration always appends its pointer
  with `--append-system-prompt`, but Pi 0.85.1 accumulates repeated `--append-system-prompt` flags and joins them, so
  the user's own text survives. Older Pi versions were not checked.
- Decision: discard (user, 2026-10-01).
- Completion criteria: remove the feedback file and its index entry, without code or spec changes.
- Execution: complete: change `ykwnwmmm`, bookmark `triage-1001e/19-pi-pointer`, PR
  https://github.com/scode/farhelm/pull/1461.

## refresh-starved-by-seeds.md

- Outcome: `discard`.
- Assessment: mostly fixed by #1281 (9221a8e3): a host refresh discarded because one of the helm's own writes landed
  meanwhile now retries at once when a hint is pending instead of waiting 3 s. What remains needs a mutation landing
  inside every successive refresh, continuously, on one host; other sessions there show stale status until the burst
  stops, then correct themselves, while the host reads healthy.
- Decision: discard (user, 2026-10-01).
- Completion criteria: remove the feedback file and its index entry, without code or spec changes.
- Execution: complete: change `pypvuppx`, bookmark `triage-1001e/20-refresh-seeds`, PR
  https://github.com/scode/farhelm/pull/1462.

## env-wrapper-hides-command-not-found.md

- Outcome: `discard`.
- Assessment: confirmed by code inspection at f087e0b6. Goose, Pi and OMP launches run through Farhelm's own `env`
  wrapper, so a missing agent program surfaces as exited (127) rather than a launch error, unlike SPEC.md "Creation"'s
  promise for "command not found". Likely covered by SPEC.md "First-class harnesses" (session tracking for other
  harnesses is intentionally partial), though borderline because the error/exited split is a general session promise.
- Decision: discard (user, 2026-10-01).
- Completion criteria: remove the feedback file and its index entry, without code or spec changes.
- Execution: complete: change `zmxosmrx`, bookmark `triage-1001e/21-env-wrapper`, PR
  https://github.com/scode/farhelm/pull/1463.

## event-feed-cap-refusal-invisible.md

- Outcome: `discard`.
- Assessment: partly correct at f087e0b6. With all 64 live-update seats taken (one per open web UI page or desktop app
  window), the helm refuses the next subscriber with an HTTP 503 before the WebSocket upgrade, which browsers cannot
  observe, so the code's stated rationale is wrong; b60ae0a2 (#1375) removed the zombie-seat trigger the finding cites.
  The refused page silently falls back to its 3 s poll, by design, and stays correct. 64 pages is far beyond the handful
  of clients the product targets.
- Decision: discard (user, 2026-10-01).
- Completion criteria: remove the feedback file and its index entry, without code or spec changes.
- Execution: complete: change `wtxkpwnv`, bookmark `triage-1001e/22-feed-cap`, PR
  https://github.com/scode/farhelm/pull/1464.

## terminal-font-promise-leak.md

- Outcome: `discard`.
- Assessment: confirmed in code at f087e0b6. Each terminal mount attaches a callback to a page-wide font promise that
  never settles when the bundled font is not confirmed within the 3 s settle deadline or the Font Loading API is
  unusable, retaining every terminal instance; a terminal reconnecting for hours (about 1,000 mounts in 8 hours) grows
  page memory until reload. It exists only because of the font fallback, which `terminal.js` documents as a deliberate
  choice to treat the font as a best-effort enhancement rather than a required bundled asset.
- Decision: discard, and remove the font fallback as its own task: the bundled font is to be treated like Farhelm's
  other bundled assets, with the fallback complexity and behavior removed. Recorded as a TODO.md
  `Definite
  simplification` entry during triage (user, 2026-10-01).
- Completion criteria: remove the feedback file and its index entry, without code or spec changes.
- Execution: complete: change `lultloxq`, bookmark `triage-1001e/23-font-promise`, PR
  https://github.com/scode/farhelm/pull/1465.

## desktop-copy-fallback-never-runs.md

- Outcome: `discard`.
- Assessment: confirmed as dead code at f087e0b6, with a smaller consequence than reported. The header copy's
  `navigator.clipboard` fallback runs only if the native writer throws or rejects, which the desktop writer never does,
  and the helm's clipboard endpoint returns 204 whether or not the native write worked. What remains is a Linux desktop
  copy during a brief re-sign-in that silently fails while the button shows "copied", within SPEC.md's best-effort,
  silent-on-failure clipboard contract.
- Decision: discard (user, 2026-10-01).
- Completion criteria: remove the feedback file and its index entry, without code or spec changes.
- Execution: complete: change `ktroylmk`, bookmark `triage-1001e/24-copy-fallback`, PR
  https://github.com/scode/farhelm/pull/1466.

## desktop-clipboard-fetch-backlog.md

- Outcome: `fix code`.
- Assessment: confirmed at 73029557. Every OSC 52 write a remote program emits becomes one fire-and-forget `fetch` to
  the embedded helm's `POST /api/clipboard` (`arm_native_clipboard_script`, `crates/farhelm-ui/src/auth.rs`), called
  from terminal.js's clipboard provider, which returns immediately so the parser never waits. Nothing bounds outstanding
  submissions or replaces obsolete pending values; the webview caps concurrent connections, not queued requests. A
  working clipboard does not prevent the backlog: a loop of tiny OSC 52 sequences outruns a loopback round trip plus a
  serialized native write, and each pending request retains its text (up to the 256 KiB the endpoint accepts). Queued
  stale writes can also land after the user copied something newer. Actual memory growth or a webview crash was not
  measured. In scope through SPEC.md's "Local authority and trust between hosts": OSC 52 writes are an "explicitly
  allowed, bounded effect", and remote malicious behavior must not disrupt ordinary GUI controls. Not covered:
  `clipboard-writes-unbounded-blocking-admission.md` (planned in `plans/triage-restart-takeover-update.md`) caps native
  writes inside the helm for a hung clipboard, which speeds draining but does not bound the page-side queue; nothing in
  `Planned`, `BUGS.md` or the filters applies. Bucket `highest` (availability across the remote-host trust boundary, the
  same class as the hint-flood item); no data or credential loss.
- Decision: fix code (user, 2026-10-02).
- Completion criteria: bound the page-side clipboard bridge to at most one in-flight write plus one replaceable latest
  pending value, so the newest write wins and stale values never land after a newer one. Keep terminal parsing
  nonblocking and failures silent per the best-effort contract. Prefer placing the coalescing where it also covers the
  browser's `navigator.clipboard` path (terminal.js's provider) if that stays simple. Add a test that bursts writes
  against a write that never settles and asserts retained and submitted work stays bounded, and that the latest value is
  the one written once the write settles. Remove the feedback file and its index entry.
- Execution: complete: change `ttqlnmrk`, bookmark `plan/triage-clipboard-terminal-limit/01-clipboard-bound`, draft PR
  [#1473](https://github.com/scode/farhelm/pull/1473/changes).

## desktop-protocol-filesystem-fallback.md

- Outcome: `other` (supersedes an earlier `fix code` decision the same day; see Decision).
- Assessment: confirmed by source trace at 73029557; not reproduced at runtime. Farhelm's desktop asset handler claims
  only the `assets` path segment (`crates/farhelm-ui/src/desktop/assets.rs`), and the window config installs no
  catch-all, CSP or custom protocol. Every other `dioxus://` path goes to the pinned dioxus-desktop 0.7.10 default
  (`protocol.rs`), whose dioxus-asset-resolver percent-decodes it, uses an existing absolute path as is, reads it with
  `std::fs::read`, and answers 200 with `Access-Control-Allow-Origin: *`; the page is itself served from
  `dioxus://index.html/`, so such a fetch is same-origin. Page script could therefore read any file the desktop process
  can. No script-injection path was found (no `innerHTML`/`dangerous_inner_html`, terminal links limited to http(s),
  Rust-built page script interpolates only JSON-serialized values), and injected script would already hold the device
  secret and full helm API access, so the added reach is modest. Unverified side finding: the resolver `.expect()`s
  UTF-8 on the decoded path, so a request like `dioxus://index.html/%ff` likely panics in the protocol callback and may
  abort the desktop app. In scope per SPEC.md "Client hardening" (the native app's higher bar for proportionate
  hardening against hypothetical flaws). SPEC_impl.md's "no bundle-directory fallback at all" is accurate only under
  `/assets/`. Not covered by `Planned`, `BUGS.md`, filters, the ledger or plans/.
- Decision: initially fix code (user, 2026-10-02), with a very clear comment at the fix explaining why it exists and
  stating plainly that it is hardening only: no known exploitable bug motivated it. Superseded the same day once it was
  clear that every complete fix needs the repo's first patched Rust dependency (a `[patch.crates-io]` fork of
  dioxus-desktop or dioxus-asset-resolver) or an upstream Dioxus change, and that nothing known today can exercise the
  fallback (only Farhelm's embedded script runs in the window, navigation away is blocked, and untrusted data is
  rendered as text). Revised decision: record it as a TODO.md `Maybe later` entry that keeps clear that it is hardening
  only, the trusted versus untrusted story, and that a fix currently seems to need patching Dioxus or an upstream
  change.
- Completion criteria: the TODO.md `Maybe later` entry "Close the desktop window's filesystem read fallback (hardening
  only)" exists (recorded during triage). Remove the feedback file and its index entry, without code or spec changes.
- Execution: complete: change `uvznsnlv`, bookmark `plan/triage-clipboard-terminal-limit/02-fallback-queue-cleanup`,
  draft PR [#1474](https://github.com/scode/farhelm/pull/1474/changes).

## terminal-output-queue-missing-byte-budget.md

- Outcome: `fix code`.
- Assessment: confirmed by source trace at 73029557; actual memory pressure, swapping or OOM not reproduced. The only
  size cap on incoming terminal data is the protocol-wide `MAX_FRAME_LEN` of 8 MiB (`crates/farhelm-proto/src/lib.rs`);
  the helm queues each data frame unchanged (`crates/farhelm-helm/src/client.rs`, `dispatch` and `route_terminal_event`)
  into a per-attachment queue bounded at 256 events (`TERM_EVENT_QUEUE`), with no byte limit per connection, host or
  reader. The queue is registered before the attach request is sent but its receiver is handed out only after
  `Attached`, so a supervisor can send 256 × ~8 MiB before replying successfully; after attach, a supervisor that
  ignores the browser's pause can fill it too. That is about 2 GiB per opened terminal, held in the helm that serves
  every host and runs inside the desktop app. Only a hostile or compromised supervisor can trigger it: the honest one
  sends terminal data through a single path chunked at `REPLAY_CHUNK` (32 KiB,
  `crates/farhelm-supervisor/src/service/connection.rs`), a supervisor-private constant for progressive replay and pause
  granularity that is in neither the protocol nor the specs, and unchanged since #5. `TERM_EVENT_QUEUE`'s own comment
  admits it bounds events, not bytes, and skips a byte bound on the assumption that upstream flow control holds, which a
  hostile supervisor does not honor. In scope per SPEC.md "Local authority and trust between hosts" (supervisor messages
  are untrusted; a remote host must not disrupt unrelated hosts or ordinary helm/GUI controls; proportionate remedies).
  Not covered: SPEC_impl.md's "Supervisor metadata retention and nonresponse" concerns nonresponse and metadata, and the
  `sessions-changed-hint-unthrottled.md` decision accepts availability degradation only when not easily avoided; nothing
  in `Planned`, `BUGS.md` or the filters applies.
- Decision: fix code with a simple fix and a hard limit (user, 2026-10-02). The limit's relationship to `REPLAY_CHUNK`
  must be explained clearly, and a test must tie the two together so that growing `REPLAY_CHUNK` cannot silently outgrow
  the limit.
- Completion criteria: add a hard maximum terminal data chunk as a shared constant in `farhelm-proto` with headroom
  above today's 32 KiB (64 KiB was discussed), and have the helm refuse any incoming terminal data frame larger than it
  on receipt, including frames that arrive before `Attached`, by the simplest visible failure (detaching that terminal,
  or treating it as a protocol violation like other decode errors). Document on both the new constant and `REPLAY_CHUNK`
  that the supervisor's chunk must not exceed the limit, why, and that older helms enforce the limit, so raising it
  needs care. Add a test that fails if `REPLAY_CHUNK` exceeds the limit. Add a controlled-peer regression that sends one
  oversized data frame before `Attached` and shows it is refused, without a multi-gigabyte fixture. Rewrite
  `TERM_EVENT_QUEUE`'s comment so the memory bound it now implies (256 × the limit) is stated rather than disclaimed.
  Remove the feedback file and its index entry.
- Execution: complete: change `nztunrsk`, bookmark `plan/triage-clipboard-terminal-limit/03-terminal-data-limit`, draft
  PR [#1477](https://github.com/scode/farhelm/pull/1477/changes).

## delete-skips-scoped-tab-on-stale-verdict.md

- Outcome: `other`.
- Assessment: confirmed by source inspection on main at cc32cc53; not reproduced at runtime. Delete's teardown
  (`crates/farhelm-supervisor/src/service/teardown.rs`) records only the agent's own launch unit, adds every tab unit as
  derived regardless of the window's scoped marker, re-probes a stale verdict only when the agent's launch was scoped,
  and on a "no usable manager" verdict only logs the glob failure at debug level, so a scoped tab of an unscoped session
  is never checked and Delete reports success. The finding's preconditions (agent unscoped, tab scoped, negative verdict
  at delete time) all arise from per-launch and per-tab scope decisions against a cached verdict. Contradicts SPEC.md
  "Lifecycle operations" and SPEC_impl.md's rule that a scope-marked tab refuses until its scope check has passed.
- Decision: the user decided (2026-10-02) that on Linux a usable systemd user manager is simply required, and macOS
  keeps the portable sweep. A given host then no longer flip-flops between systemd and no systemd, and this edge case
  goes away with the fallback. The only action is to add this finding as example complexity to TODO.md's existing
  `Maybe later` entry about dropping Linux support without a systemd user manager (retitled "Require a systemd user
  manager on Linux, with no fallback" when the sibling entry on the silent fallback was folded into it), which takes
  care of it when executed.
- Completion criteria: the TODO.md entry "Require a systemd user manager on Linux, with no fallback" records the
  decision and names this case (done during triage). Remove the feedback file and its index entry, with no code or spec
  change.
- Execution: planned in `plans/queue/triage-crash-replies-and-locks.md`.

## create-rollback-orphans-unconfirmed-scope.md

- Outcome: `other`.
- Assessment: confirmed by source inspection on main at cc32cc53; not reproduced at runtime. The create rollback paths
  in `crates/farhelm-supervisor/src/service/core.rs` still pass `ScopeKillFailure::Warn`, under which a clean sweep plus
  an unconfirmed scope kill counts as success, and then remove the launching record, leaving nothing that can later find
  the scope. Needs four rare conditions together (create fails after the agent started, systemd does not confirm the
  scope kill, a process the sweep cannot see, and for one path a tmux session already gone). Unlike the Delete case, the
  trigger is a hung or overloaded manager at rollback time rather than the no-systemd fallback, so requiring systemd
  does not by itself change what this rollback does. What remains is not systemd-specific: the rollback drops the only
  record of a scope whose kill was not confirmed, where other unconfirmed cleanups keep the session so a later Delete
  can retry.
- Decision: same as `delete-skips-scoped-tab-on-stale-verdict.md`: systemd is required on Linux with no fallback (user,
  2026-10-02), a hung or broken user manager is a broken host like any other, and the action is to add this finding as
  example complexity to the same TODO.md entry. Asked whether the remaining non-systemd part needs anything, the user
  chose consistency with other places: a cleanup Farhelm cannot confirm fails visibly and keeps the session, so the
  TODO.md entry asks the rollback to keep the session when the scope kill is unconfirmed.
- Completion criteria: the TODO.md entry names this case and that the rollback must fail visibly and keep the session
  (done during triage). Remove the feedback file and its index entry, with no code or spec change.
- Execution: planned in `plans/queue/triage-crash-replies-and-locks.md`.

## desktop-auth-ready-with-stale-webview-credential.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by source inspection on main at cc32cc53; the reviewer's focused reproduction was not rerun.
  `crates/farhelm-ui/assets/desktop-auth.js` swallows a failed `localStorage` write of the new webview secret and still
  sends `ready: true`, while terminals, uploads and the event feed read their credential from `localStorage`, so the
  window can open with a missing or revoked secret for those consumers. The trigger is the desktop app's internal
  re-authentication, which runs after `farhelm helm token rotate` or when the helm's 64-credential cap evicts the app's
  stored secrets.
- Decision: the user (2026-10-02): the desktop app must never involve a user-visible credential; the split between the
  embedded helm and the window is an implementation detail. Agreed direction: token rotation and the client-credential
  cap no longer apply to the desktop app's own secrets, so this internal re-authentication essentially never runs,
  rather than hardening each step of it. This changes SPEC.md "Signing in again" (and the "Client to helm" and
  client-scale text), which today describe a desktop re-sign-in with a Retry surface.
- Completion criteria: SPEC.md and SPEC_impl.md say the desktop app's own credentials are not revoked by browser token
  rotation or evicted by the client cap, and drop the desktop re-sign-in requirement; the code matches, and no ordinary
  operation leaves the desktop window running with a credential the helm has revoked. Keep or remove the existing
  recovery machinery as the simplest correct design requires. Remove the feedback file and its index entry.
- Execution: complete — planned in `plans/queue/triage-signin-recovery.md`; jj change `suwwmmmvmrqk`, bookmark
  `plan/triage-signin-recovery/01-desktop-own-credentials`, https://github.com/scode/farhelm/pull/1487.

## desktop-reauth-failure-loses-action-outcomes.md

- Outcome: `fix spec+code`.
- Assessment: confirmed by source inspection on main at cc32cc53; not reproduced. A failed desktop authentication run
  replaces the whole app with the failure page and Retry button (`crates/farhelm-ui/src/auth.rs`), which unmounts the
  components awaiting pending actions such as Delete, so their outcome is never reported. SPEC.md "Signing in again"
  forbids that.
- Decision: same direction as `desktop-auth-ready-with-stale-webview-credential.md` (user, 2026-10-02): no user-visible
  credential in the desktop app, and its own credentials are not subject to rotation or eviction, so a desktop
  re-authentication, and with it this failure page, should not occur in ordinary operation. Execute together with that
  item.
- Completion criteria: as for `desktop-auth-ready-with-stale-webview-credential.md`; once desktop re-authentication no
  longer occurs in ordinary operation, a pending desktop action cannot be unmounted by it. Remove the feedback file and
  its index entry.
- Execution: complete — planned in `plans/queue/triage-signin-recovery.md`; jj change `wvtlsrzvvsut`, bookmark
  `plan/triage-signin-recovery/02-desktop-outcome-loss`, https://github.com/scode/farhelm/pull/1488. Checked against the
  code PR 1 produced: once the window is open, nothing replaces it. The failure page only comes from the window's
  authentication run, which runs before the app is first shown and again only from that page's own Retry button, and the
  browser's token prompt is never raised in the desktop build. SPEC.md's existing never-lost-silently sentence and PR
  1's "Signing in again" text cover it; no new spec sentence.

## dropped-create-skips-bookkeeping.md

- Outcome: `discard`.
- Assessment: already fixed on main by #1404 (`493c903e`), which postdates the reviewed commit: `create_session` and
  `replace_session` in `crates/farhelm-helm/src/sessions.rs` now run their bodies through `run_owned`, so a dropped
  request loses only the reply.
- Decision: already fixed; discarded without discussion under the triage rule for findings fixed on main.
- Completion criteria: remove the feedback file and its index entry.
- Execution: `complete`; removed during triage on 2026-10-02.

## agent-create-aborted-on-retire.md

- Outcome: `discard`.
- Assessment: already fixed on main by #1419 (`ad72d901`), which postdates the reviewed commit: `spawn_agent_answer` in
  `crates/farhelm-helm/src/client.rs` no longer registers an abort handle for state-changing agent requests, so retiring
  the asking host's connection cannot stop a started create or clone partway.
- Decision: already fixed; discarded without discussion under the triage rule for findings fixed on main.
- Completion criteria: remove the feedback file and its index entry.
- Execution: `complete`; removed during triage on 2026-10-02.

## seen-write-cancellation-skips-notification.md

- Outcome: `discard`.
- Assessment: already fixed on main by #1404 (`493c903e`): `mark_seen` in `crates/farhelm-helm/src/sessions.rs` runs the
  store write and the fleet-revision bump inside one `run_owned`. The dedicated cancellation test the finding proposed
  was not added; minor. Also covered by SPEC.md "One GUI at a time", since only a second GUI could see the stale dot.
- Decision: already fixed; discarded without discussion under the triage rule for findings fixed on main.
- Completion criteria: remove the feedback file and its index entry.
- Execution: `complete`; removed during triage on 2026-10-02.

## create-directory-wait-blocks-terminal-reader.md

- Outcome: `other`.
- Assessment: confirmed on main at cc32cc53: the supervisor's connection reader awaits `handle_create_session` inline
  (`crates/farhelm-supervisor/src/service/handlers.rs`), which waits on the per-key intent lock and the host-wide
  directory lock that Delete holds through its kill grace. Fully covered by TODO.md `Planned` "Keep session creation off
  the connection read loop" and the `create-runs-inline-on-read-loop.md` decision in this file; the feedback file itself
  says it was retained only as additional evidence.
- Decision: covered by planned work; removed under the triage rule for items covered by the `Planned` bucket.
- Completion criteria: remove the feedback file and its index entry.
- Execution: `complete`; removed during triage on 2026-10-02.

## stop-admission-blocks-terminal-reader.md

- Outcome: `fix code`.
- Assessment: confirmed on main at cc32cc53. The supervisor reads every frame of a helm connection in one loop and
  awaits control handling inline (`crates/farhelm-supervisor/src/service/connection.rs`); `handle_stop_session` awaits
  its own `acquire_owned()` on the eight-slot management semaphore (`HANDLER_ADMISSION_PERMITS`, `core.rs`) before
  spawning its task, so with all slots held, a Stop freezes terminal input, resize, detach and list requests for every
  session on the host. The waiting is deliberate today: the semaphore's comment and `spawn_admitted`'s comment say it
  backpressures the sending connection, and the test
  `spawn_admitted_acquires_the_permit_before_spawning_not_inside_the_task` enforces it. A long freeze can escalate: the
  helm drops the whole connection when a list request exceeds its 30 s `REFRESH_TIMEOUT`. Contradicts SPEC.md "Waiting
  between operations on one host".
- Decision: the user (2026-10-02): when management capacity is exhausted, refuse promptly instead of queueing, and make
  the code comments say clearly why. Delete keeps its current behavior: it already waits for its slot inside its own
  task, off the reader.
- Completion criteria: one shared non-waiting admission step (take a slot without waiting, otherwise reply with the
  existing `ErrorKind::Unavailable`, which the helm maps to 503 and the UI shows as the action's error text) replaces
  the waiting acquire in Stop, Restart, Rename and the shared `spawn_admitted` helper (which also covers directory
  browse, repository search, and tab open/close). Refusal happens before any change, so a retry is safe. Rewrite the
  semaphore's and helper's comments to explain the refusal and why the reader must never wait, and replace the test that
  enforces waiting with one where the slots are saturated, the request is refused, and terminal input to another session
  still reaches its pane. The four admission items share this change: whichever executes first introduces it, and the
  others apply it to their call site. Remove the feedback file and its index entry.
- Execution: complete: change `pmmvrrnw`, bookmark `plan/triage-busy-host-refusal/02-refuse-when-busy`, PR
  https://github.com/scode/farhelm/pull/1486.

## restart-admission-blocks-terminal-reader.md

- Outcome: `fix code`.
- Assessment: confirmed on main at cc32cc53: `handle_restart_session`
  (`crates/farhelm-supervisor/src/service/handlers.rs`) has its own inline wait for a management slot, independent of
  Stop's and of `spawn_admitted`, with the same reader-freezing consequence as
  `stop-admission-blocks-terminal-reader.md`.
- Decision: as for `stop-admission-blocks-terminal-reader.md` (user, 2026-10-02): refuse promptly when capacity is
  exhausted, with comments saying why.
- Completion criteria: Restart uses the shared non-waiting admission step described under
  `stop-admission-blocks-terminal-reader.md`, with a saturated-restart test showing the refusal and that input to
  another session progresses. Remove the feedback file and its index entry.
- Execution: complete: change `sswupomv`, bookmark `plan/triage-busy-host-refusal/03-restart-refuses`, PR
  https://github.com/scode/farhelm/pull/1490.

## rename-admission-blocks-terminal-reader.md

- Outcome: `fix code`.
- Assessment: confirmed on main at cc32cc53: Rename acquires its slot inline in the read loop (`handlers.rs`, commented
  "acquired here, in the read loop") and then hands that one permit through the database update and reply. The handoff
  still works if the slot is taken without waiting.
- Decision: as for `stop-admission-blocks-terminal-reader.md` (user, 2026-10-02).
- Completion criteria: Rename uses the shared non-waiting admission step, keeping its single-permit handoff, with a
  saturated-rename test showing the refusal and that input to another session progresses. Remove the feedback file and
  its index entry.
- Execution: complete: change `povunnsp`, bookmark `plan/triage-busy-host-refusal/04-rename-refuses`, PR
  https://github.com/scode/farhelm/pull/1493.

## list-admission-blocks-terminal-reader.md

- Outcome: `fix code`.
- Assessment: confirmed on main at cc32cc53: `handle_list_sessions` goes through `spawn_admitted`, which awaits a
  management slot inline in the reader, so eight in-flight lifecycle operations make a list request freeze the
  connection. The list needs no management slot for correctness: listing holds the session map only briefly and takes no
  lifecycle, intent or directory lock; the slot only bounds its tmux subprocesses and capture sweep. SPEC.md "Waiting
  between operations on one host" requires the session list not to wait on management operations, and refusing it while
  they hold the slots would show last-known rows with a refresh-failed note for as long as management is busy, which is
  the same dependence by another route.
- Decision: the user (2026-10-02) agreed that the session list is exempt from the management cap rather than refused; it
  gets its own small limit for its tmux and capture cost, like the status sampler's separate limit.
- Completion criteria: list requests no longer take a management slot and never wait in the reader; a separate small
  bound, documented with the reason, caps concurrent list work. Test: all management slots held by controlled lifecycle
  operations, then a list request and terminal input on the same connection both progress. Remove the feedback file and
  its index entry.
- Execution: complete: change `vnzttzst`, bookmark `plan/triage-busy-host-refusal/01-list-limit`, PR
  https://github.com/scode/farhelm/pull/1483.

## checkout-reconciliation-blocks-terminal-reader.md

- Outcome: `other`.
- Assessment: confirmed on main at cc32cc53: the reconciliation request the helm sends before a keyed fresh-checkout
  create is awaited inline in the reader (`handlers.rs`) and calls `admit_create` (`core.rs`), which waits on the
  per-key intent lock and the host-wide directory lock that Delete holds through its kill grace. This is a lock wait,
  not a capacity wait, and SPEC.md explicitly lets creates queue behind Delete, so refusing here would fail every fresh
  checkout while any Delete runs. Not named by the `Planned` create-dispatch item or any plan file.
- Decision: the user (2026-10-02) agreed: fold it into TODO.md `Planned` "Keep session creation off the connection read
  loop" by naming the reconciliation request there, and remove the queue item.
- Completion criteria: the `Planned` entry names the reconciliation request (done during triage). Remove the feedback
  file and its index entry.
- Execution: `complete`; TODO.md updated and the feedback file and index entry removed during triage on 2026-10-02.

## browser-signin-loses-action-outcomes.md

- Outcome: `fix spec`.
- Assessment: confirmed on main at cc32cc53: when a browser request gets 401, `crates/farhelm-ui/src/lib.rs` swaps the
  whole signed-in tree for `auth::TokenPrompt`, unmounting the components that await pending actions (Delete included).
  The helm still completes the work, since accepted actions are helm-owned; only the report is lost. The realistic
  trigger is `farhelm helm token rotate`, credential eviction past 64 enrollments, or cleared browser storage while an
  action is pending. A code fix is not trivial (hoisting action ownership above the sign-in branch). Contradicts SPEC.md
  "Signing in again": "An action the user started is never lost silently".
- Decision: the user (2026-10-02): the browser is best effort for rare UX issues that are not correctness issues; this
  is acceptable and not high priority. The desktop app keeps the guarantee. Moved from the `high` to the `other` bucket
  in the queue index, since nothing is lost on the server and the trigger is rare and user-initiated.
- Completion criteria: SPEC.md "Signing in again" carves the browser out of the never-lost-silently sentence, along the
  lines of: in the browser, an action still pending when the token prompt opens may lose its report; the helm still
  carries it out, and the list shows the result after sign-in. The desktop guarantee stays as specified (see the desktop
  credential decisions above). Remove the feedback file and its index entry.
- Execution: complete — planned in `plans/queue/triage-signin-recovery.md`; jj change `syytzxqntspm`, bookmark
  `plan/triage-signin-recovery/03-browser-carve-out`, https://github.com/scode/farhelm/pull/1489.

## list-ingress-id-validation-gap.md

- Outcome: `fix code`.
- Assessment: confirmed on main at cc32cc53: `drain_sessions` (`crates/farhelm-helm/src/manager.rs`) checks only the
  reply cap, id length and duplicates, while create's `created_session_from` (`client.rs`) also refuses empty and
  control-character ids, and its doc comment wrongly says both apply the same rule. The UI's `encode_path_segment`
  (`crates/farhelm-ui/src/api.rs`) claims `%2E` blocks dot-segment resolution; under the WHATWG URL Standard it does
  not. No current route is reachable this way. Supervisors mint UUIDs.
- Decision: the user (2026-10-02): reasonable, straightforward defensive checks are encouraged when cheap, without lots
  of complexity for defense in depth at every level. This one is cheap.
- Completion criteria: one shared session-id check (non-empty, within the length cap, no control characters, not `.` or
  `..`; a conservative character set is optional) used by both list and create ingress. A list containing a bad id is
  refused whole, keeping the previous cache, as oversized and duplicate ids already are. On the UI side, correct the
  encoder's doc and test comment only; the helm check is the boundary. Fix `created_session_from`'s doc. Remove the
  feedback file and its index entry.
- Execution: planned in `plans/queue/triage-boundary-checks.md`.

## profile-body-accepts-unknown-fields.md

- Outcome: `fix code`.
- Assessment: confirmed on main at cc32cc53: `ProfileSpec` (`crates/farhelm-helm/src/profiles.rs`) lacks
  `deny_unknown_fields` and its optional `resume_template` makes a misspelled key clear the stored template. The shipped
  UI, the e2e helper and the Rust e2e test send only known fields, though the latter two omit `resume_template`, so
  making that key required would break them.
- Decision: the user (2026-10-02): cheap defensive checks are encouraged.
- Completion criteria: `#[serde(deny_unknown_fields)]` on `ProfileSpec`, with `resume_template` still optional, and a
  REST test showing a misspelled key is refused and the stored profile is unchanged. Remove the feedback file and its
  index entry.
- Execution: planned in `plans/queue/triage-boundary-checks.md`.

## provision-lock-map-grows-per-requested-id.md

- Outcome: `fix code`.
- Assessment: confirmed on main at cc32cc53: `host_provision_lock` and `try_host_provision_lock`
  (`crates/farhelm-helm/src/manager.rs`) insert an entry for any requested id before any existence check, and retarget,
  alias and remove (`hosts.rs`) take the lock before the store refuses an unknown id, so the documented bound is false.
  Needs an authenticated client.
- Decision: the user (2026-10-02): cheap defensive checks are encouraged.
- Completion criteria: an entry is removed when its lock is released and nothing else holds or waits on it (checked
  under the map's mutex), bounding the map to locks in use without a store read and without changing callers. Correct
  the doc comment and add a test that requests many unregistered ids and checks the map's size. Remove the feedback file
  and its index entry.
- Execution: planned in `plans/queue/triage-boundary-checks.md`.

## restart-with-skips-create-validation.md

- Outcome: `fix code`.
- Assessment: confirmed on main at cc32cc53: the restart-with branch of `restart_session`
  (`crates/farhelm-supervisor/src/service/core.rs`) applies neither `ensure_no_cwd_program` nor `ensure_resume_template`
  that create applies, and the restart handler applies none of create's template caps, while loading refuses a row
  failing those checks, so one bad bundle stops the supervisor from loading its sessions. SPEC_impl.md "Restart-with
  backend wire and persistence" already says restart-with uses create's checks. The shipped helm never sends such a
  bundle. The checks would run before the old agent is stopped.
- Decision: the user (2026-10-02): cheap defensive checks are encouraged.
- Completion criteria: the restart-with branch applies `ensure_no_cwd_program` to the invocation and
  `ensure_resume_template` to the resolved template, mapping these and the existing argv and resolve errors to
  `InvalidRequest`, plus create's template element cap. Test that a bundle with `{conversation}` as the template's
  program is refused, the row is unchanged, and a fresh supervisor still starts. Remove the feedback file and its index
  entry.
- Execution: planned in `plans/queue/triage-boundary-checks.md`.

## escape-token-clamp-too-short.md

- Outcome: `fix code`.
- Assessment: confirmed on main at cc32cc53: `back_off_from_split_escape` (`crates/farhelm-ui/src/menu_panel.rs`)
  assumes 8-character tokens, but unsafe tag characters U+E0000–U+E007F render as 9-character `<U+E0041>` tokens, so one
  cut position leaves a broken marker in the host menu's accessible label. The doc comments claiming eight characters
  are wrong.
- Decision: the user (2026-10-02): cheap defensive checks are encouraged; this is trivial.
- Completion criteria: look back far enough for the longest token and back off to the nearest `<` (nearest matters: a
  complete token followed by a lone `<` must not match the earlier token), fix both doc comments, and test a cut inside
  a `<U+E00xx>` token. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-boundary-checks.md`.

## opencode-bare-model-rejected.md

- Outcome: `fix spec+code`.
- Assessment: confirmed on main at cc32cc53. Every model lookup compares the typed text with the catalog as typed. Bare
  `gpt-6-luna`, `gpt-5.6-terra`, `gpt-6.1-sol` and `gpt-6-astra` (and others such as `claude-fable-5`) are catalog
  entries owned by other harnesses, while OpenCode's entries carry the `opencode/` prefix. So `validate_selection`
  (`crates/farhelm-helm/src/launches.rs`) refuses an OpenCode launch of a bare name as belonging to another harness, the
  browser's `selection_is_compatible` and `compatible_efforts` (`crates/farhelm-ui/src/launch_composer.rs`) disagree the
  same way, and pressing Enter switches the harness (see `opencode-bare-model-switches-harness.md`). A server-only or
  UI-only fix leaves the bug visible.
- Decision: the user (2026-10-02): choosing a model name must never switch the harness unless it is a true special case
  where the model can only be available in one harness, and no such case is known; users must not have to type
  `opencode/`. Agreed details: bare names resolve inside the selected harness; pressing Enter on a typed model never
  switches an already selected harness, and a model only another harness offers keeps the harness and shows an inline
  error naming the owner; an explicit pick of a harness-labelled row in "Show all" still switches; with no harness
  selected, bare `gpt-6-luna` and similar pick Codex, the primary harness, with no extra question.
- Completion criteria: one per-harness method in the place the harness-specific-code rules name (`LaunchHarness` in
  `crates/farhelm-proto/src/launch.rs` was suggested) gives a typed model's catalog form (OpenCode qualifies a bare
  name; others return it unchanged), used by the helm's validation and effort lookup and by the UI's Enter handling,
  compatibility, effort and history filters, while the user's typed text stays the stored selection. Enter never
  switches a selected harness. SPEC.md's "A known model identifies its owning harness" is narrowed to filling an
  unselected harness, and the existing test asserting a typed Codex-to-Claude switch is reversed. Tests cover both
  spellings of the overlapping names under OpenCode on the server and on Enter. Remove the feedback file and its index
  entry.
- Execution: complete: change `rqqozxulsylw`, bookmark `plan/triage-opencode-model-names/01-bare-model-names`, PR
  https://github.com/scode/farhelm/pull/1484.

## opencode-bare-model-switches-harness.md

- Outcome: `other`.
- Assessment: confirmed on main at cc32cc53 and broader than reported: `model_enter_target`
  (`crates/farhelm-ui/src/launch_composer.rs`) keeps the selected harness only if it owns the exact typed id, and
  otherwise `apply_model_option` (`crates/farhelm-ui/src/list/create_form.rs`) switches to the owning harness. This
  happens for any known model of another harness (Codex to Claude too), not just OpenCode bare names. Same root cause as
  `opencode-bare-model-rejected.md`.
- Decision: the user (2026-10-02): fixed by the change recorded under `opencode-bare-model-rejected.md`, which includes
  the no-switch rule; the two halves cannot be fixed separately.
- Completion criteria: that item's change lands. Remove the feedback file and its index entry in the same PR as that
  change, or in its own bookkeeping PR immediately after it.
- Execution: complete: closed by PR https://github.com/scode/farhelm/pull/1484 (Enter keeps a selected harness; its
  browser test covers this report). Bookkeeping: change `psoyllpkxvvq`, bookmark
  `plan/triage-opencode-model-names/02-switch-harness-bookkeeping`, PR https://github.com/scode/farhelm/pull/1485.

## stop-restart-panic-no-reply.md

- Outcome: `fix code`.
- Assessment: confirmed on main at cc32cc53: the Stop and Restart waiter tasks in
  `crates/farhelm-supervisor/src/service/handlers.rs` only log a panicked work task and send no reply, while Delete's
  sends an `Internal` error. The helm puts no deadline on that reply. No concrete panic was found.
- Decision: the user (2026-10-02): crash the whole process when that is simpler than handling an internal crash, but
  where resilience is trivial and does not hurt the user experience, do that. Here it is trivial (about 15 lines copying
  Delete) and consistent with the helm's own rule that a panicking handler still answers.
- Completion criteria: the Stop and Restart waiters send an `Internal` error on a panicked join, Restart's saying the
  outcome is unknown, with a test for each. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/triage-crash-replies-and-locks.md`.

## host-write-lock-split-on-actor-respawn.md

- Outcome: `fix code`.
- Assessment: partly correct on main at cc32cc53. The headline consequence (a host edited or removed mid-install after
  its connection worker restarted) was fixed after the review by #1165 and #1167, which added the per-host provisioning
  lock in a manager-level map keyed by host id, with the test `the_provisioning_lock_survives_actor_replacement`. The
  cache-write lock is still created per actor in `spawn_actor` (`crates/farhelm-helm/src/manager.rs`), and a handle is
  replaced while its host still exists only after the actor panicked (via `sync_registry` or a Retry's `revive`), so the
  remaining split needs a panic plus a Retry during a retarget, remove or yolo-safe edit. Making an actor panic fatal is
  not simpler here: SPEC_impl.md deliberately retires a panicked actor and lets Retry revive it, with a test asserting
  that. The two locks must not be merged; #1165 split them so long runs do not stall session write-backs.
- Decision: the user (2026-10-02) agreed with the narrowed fix under the same crash policy as
  `stop-restart-panic-no-reply.md`: resilience is trivial here (about 25 lines copying the provisioning-lock pattern).
- Completion criteria: the cache-write lock lives in a manager-level map keyed by host id, as `provision_locks` does, so
  `spawn_actor` and `host_write_lock` share one lock per host across actor replacement; test it the way the provisioning
  lock is tested. Note in the PR that the finding's text predates #1165 and #1167. Remove the feedback file and its
  index entry.
- Execution: planned in `plans/queue/triage-crash-replies-and-locks.md`.
