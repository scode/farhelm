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
- Execution: `complete`; `Launching` rows with a matching dead pane now remain `Unknown` and emit no observed-exit transition, while sentinel errors and established exits retain their behavior. Focused recorded nextest run `0d370584-9618-4571-bd07-80f8fe81b34a` passed both status regressions (858 skipped); formatting and isolated sleep checks passed. Draft PR [#917](https://github.com/scode/farhelm/pull/917/changes) is on bookmark `pr/ambiguous-restart-unknown`, jj change `8ad56228854a`.

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
  startup preservation of retained attachments, successful retry deletion, and existing quarantine cleanup paths.
  Draft PR [#914](https://github.com/scode/farhelm/pull/914/changes) is on bookmark `pr/failed-delete-attachments`,
  jj change `kmlmqnpm`.

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
- Execution: `complete`; captured and validated pane identities across delete/archive, stop, restart, and tab teardown; focused recorded nextest coverage passed matching, changed, gone, and end-to-end sweep-root cases. Draft PR [#915](https://github.com/scode/farhelm/pull/915/changes) is on bookmark `pr/pane-pid-identity`, jj change `ntkxxmtq`.

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
- Execution: `complete`; Delete now runs refusal-prone pane, tab, and scope preflights before cancelling uploads while retaining cancellation before the process sweep and destructive teardown. Focused recorded nextest run `398569c5-969b-4e8b-9b22-124b0fbfe979` passed both regressions (372 skipped); formatting, diff, and isolated sleep checks passed. Draft PR [#916](https://github.com/scode/farhelm/pull/916/changes) is on bookmark `pr/refused-delete-upload`, jj change `513d1a650bc9`.

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
- Execution: `complete`; refused keyed retries now remove the stranded generation-zero launch spec and sentinel after settling the durable row, while preserving the original refusal and surfacing cleanup failure. Focused recorded nextest run `fbd09538-c841-47db-99e1-cf7b698d7b9b` passed four retry lifecycle tests (856 skipped); formatting, changelog, and isolated sleep checks passed. Draft PR [#918](https://github.com/scode/farhelm/pull/918/changes) is on bookmark `pr/refused-retry-launch-cleanup`, jj change `2a176be0e560`.

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
- Execution: `complete`; fresh-terminal restart now preserves a surviving tmux session and replaces only the dead
  agent window, while marker-only ambiguity never authorizes destructive cleanup. Focused recorded nextest run
  `f9c9714d-833e-47cc-86ad-ac4f44e9db97` passed four restart lifecycle tests (860 skipped); the final Astra review
  found no remaining defects. Draft PR [#919](https://github.com/scode/farhelm/pull/919/changes) is on bookmark
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
- Execution: `complete`; the actor now discards any settled connection result when a retarget nudge is pending, while interrupted outcomes retain their own fresh-window decision. Focused pinned recorder run `a3b9b1b2-3db3-4296-9ae0-1baef377f1ba` passed four boundary tests (820 skipped); the isolated sleep checker passed 226 delays with zero missing rationales; formatting, diff, and changelog checks passed. The first Astra review's High finding was corrected and a fresh follow-up review reported `No entries. OK`. Draft PR [#920](https://github.com/scode/farhelm/pull/920/changes) is on bookmark `pr/stale-dial-publication`, implementation commit `930716226599450e375f016bf002bf4795c3b8c7`.

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
  Fresh Astra medium review reported `No entries. OK`. Draft PR publication is recorded in the follow-up metadata change.

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
- Execution: `pending`.
