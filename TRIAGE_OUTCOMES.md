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
- Execution: `pending`.

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
- Execution: `pending`.

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
- Execution: `pending`.

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
- Execution: `pending`.

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
- Execution: `pending`.

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
- Execution: `pending`.

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
- Execution: `pending`.

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
- Execution: `pending`.
