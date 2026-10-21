# One owner for a launch's leftover files

## What this was about

Starting an agent leaves a launch spec, a failure-status file and a progress waypoint. Create, restart, Stop and startup
cleanup each selected their own files and removal rules. A newly added waypoint had already missed one failed-create
path, and Stop repeated cleanup that the background observer already knew had succeeded.

The stack puts path construction and launch-specific removals behind one file owner. A create whose tmux session is
confirmed absent now attempts to remove all three files after its process sweep. Stop shares the observer's successful
cleanup state, while failed preservation or removal remains retryable. The final PR removes the plan's TODO entry.

## Things you should know

The inventory covered path construction, reads, removals, cleanup state, failed-launch classification and prior-run
cleanup throughout the supervisor, including the launch helper, startup, reload, create, restart, Stop, observers and
Delete. The retained contracts are:

| Operation                                   | Files and removal contract                                                                 | Cleanup state and ordering                                                                                                                                  |
| ------------------------------------------- | ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Launch helper                               | Read and strictly delete only its spec before parsing; leave failure evidence alone.       | This consumes one process's input, rather than using an observer's cleanup cache.                                                                           |
| Stop and background observation of Error    | Preserve accepted-create evidence first; remove status, spec and waypoint, attempting all. | Record success only after preservation and every removal succeed. Rename shares it; relaunch and reload establish it anew.                                  |
| Startup and reload Error reconciliation     | Same preservation and best-effort removal.                                                 | Startup's existing local success set seeds the returned sessions' cleanup state.                                                                            |
| Create refusal after confirmed tmux absence | Remove spec, status and waypoint, attempting all and retaining removal errors.             | Process sweep first; save cleanup errors before discarding the row or retaining an allocated checkout's Error. No published run exists to cache cleanup on. |
| Other create failure unwind                 | Remove all three best effort in the existing spec, status, waypoint order.                 | Keep existing process and tmux cleanup, row-retention and retry settlement order.                                                                           |
| Retry taking over an interrupted create     | Remove status, waypoint, spec; stop at the first non-absence error.                        | Clear reused paths before publishing again; an old success cache cannot authorize reuse.                                                                    |
| Restart losing its row to Delete            | Attempt all three and retain every removal failure.                                        | Keep process and terminal cleanup before file removal; no completed new run exists to cache cleanup on.                                                     |
| Confirmed-absent restart unwind             | Remove spec and waypoint, retain status; retain both removal errors.                       | Keep this partial cleanup before its process sweep.                                                                                                         |
| Delete                                      | Enumerate every matching generation and staged copy; fail on the first error.              | This covers the whole session, so a current-run cleanup cache cannot replace enumeration.                                                                   |
| Startup directory sweep                     | Keep the existing exact-entry, generation and orphan-evidence rules and warnings.          | Exclusive state ownership and reload precede the sweep; it spans sessions and generations.                                                                  |

Create and restart still classify failures and settle rows differently. The inventory found no duplicated settlement
rule worth extracting. Ending a live agent was already shared; the small repeated prior-run sweep call remains explicit
so its policy and placement stay visible. Database lifecycle, process scopes and systemd fallback are unchanged.

The only behavior changes are in the final fix PR: the complete failed-create file set, ordinary success for files
already absent, cleanup error context included in saved refusals, and Stop's successful-cleanup cache. A removal failure
can still leave the obstructed file; the refusal now includes that failure and an identical retry replays it. Stop and
the observer can still overlap before either succeeds; this adds no serialization.

Validation also exposed an existing Delete-test assumption: blocking its launch directory made both the spec and the
wrapper's waypoint unremovable, while the assertion demanded that the spec be encountered first. Delete correctly uses
the directory's own order. The fixture now removes the completed wrapper's waypoint after observing the running agent,
proves the spec is the sole remaining file, and retains its exact-path failure and retained-session assertions. Delete's
production behavior is unchanged.

## Open questions and possible follow-ups

No maintainer decision is needed. Restart's existing partial cleanup and its placement before the sweep were preserved;
the inventory did not establish why they differ, so this plan did not normalize them speculatively.

No open review-queue item described this launch-file cleanup class. The unrelated entries in the
[inspected queue snapshot](https://github.com/scode/farhelm/blob/3f32acbb1e42f1e7f57ebd51480a06adea2a12ca/review_feedback_queue/INDEX.md)
remain unaddressed by this stack. In particular, the replacement-cleanup test finding concerns a browser fixture's proof
that its child process was alive, rather than these launch files.

## The PRs

- [#2516](https://github.com/scode/farhelm/pull/2516/changes): file owner, strict spec consumption and shared removal
  policies.
- [#2517](https://github.com/scode/farhelm/pull/2517/changes): carry that owner through create and restart without
  changing their contracts.
- [#2518](https://github.com/scode/farhelm/pull/2518/changes): complete failed-create cleanup and share Stop's
  successful-cleanup state.

All three are open drafts in a linear stack, ending at `4d998afa`. None was marked ready or merged by this executor.

## Checks run, reused and skipped

Every runtime command used `scripts/record-test-run.py`, pinned nextest 0.9.148, required pinned tmux, four slots and
zero retries. Selection-skipped tests are excluded from the pass counts. Complete retained output and reports were
inspected.

- PR1:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(launch::tests::) | test(service::launch_artifacts::tests::)'`:
  92 passed; run `fa68490b-3abd-4cf8-85df-4974d512a4db`. Covers the shim, wrapper, file policies and directory
  selection.
- PR2: `cargo nextest run -p farhelm-supervisor --lib` with the recorded owner/carrier/create/restart selection: 20
  passed; run `f36fa3cf-2bbe-41cc-aa34-c589f84febd9`. Includes full artifact-policy tests, replacement unwind, ambiguous
  failure, unreadable evidence, retry refusal and generation-zero acceptance preservation.
- Final fix: `cargo nextest run -p farhelm-supervisor --lib` with the recorded cleanup/replay/Stop selection: 7 passed;
  run `7a3f25af-4f94-44fa-ba2e-4520d73255f4`. The new create test proves all files were planted before refusal, covers a
  blocked status removal and exact saved-error replay; the Stop test proves authority, failure retry and shared
  successful cleanup. Existing observer, acceptance-preservation and mid-create Delete regressions also passed.
- CLI: `cargo nextest run -p farhelm --test e2e` with the recorded four-case lifecycle selection; run
  `01cacacc-1acb-4d89-9938-a87eb6c3d793` had three passes and the Delete-fixture failure described above. The three
  passes cover Stop before observation, successful Delete cleanup and restart generation isolation. After correction the
  full same four-case selection passed in run `7ae3bc49-f4f3-4b8b-9840-9b98a92cb49d`, with no runtime skips.
- Exact unchanged reproduction:
  `cargo nextest run -p farhelm --test e2e -E 'test(=session_lifecycle::delete_fails_closed_when_a_launch_artifact_cannot_be_removed)'`
  failed the same assertion in run `b8886958-4819-42bc-9771-e0e80565bd5e`. After the fixture correction the same command
  passed in run `9feeeeca-c215-420a-aaa5-bf0d19d2df86`. Both failed UUID records are retained. The corrected case
  exercised the permission refusal; it did not take the root-user skip. The original four-case command is not claimed as
  a pass.
- `cargo fmt --all -- --check`, the source-only test-sleep checker (314 delays, zero missing rationales), changelog
  format validation and dprint on the changed Markdown passed.
- `cargo clippy --all-targets -- -D warnings` passed on `74eda071`; focused Clippy for the subsequent fixture correction
  (`cargo clippy -p farhelm --test e2e -- -D warnings`) also passed. Its first run identified an unnecessary clone in
  the new assertion; the equivalent borrowed comparison passed the rerun and both reviewers' follow-up checks.

Earlier successful commands cover their frozen reviewed trees, published as `13e8bd6c`, `25411d76` and the production
tree of `74eda071` after description-only rewrites. The final test correction changes no production code or other test,
so their coverage remains applicable. The successful final four-case run also covers that corrected fixture. The last
Clippy adjustment uses a borrowed slice for the same exact path comparison; it changes no fixture action or predicate,
so that runtime evidence is reused. The final fix does not change the earlier shim/path/enumeration contracts; its new
behavior has the separate focused coverage above. Runtime failures and their narrow reproduction are recorded above.

Browser, desktop, installer, provisioning and the full workspace runtime battery were skipped: this stack changes
supervisor launch-file ownership and cleanup, and the focused service and CLI checks cover that integration. No
instruction files or Markdown headings were changed. The live installation was not used or changed.

## The review gate's outcome

Exactly two independent fresh gpt-6-astra reviewers at high effort reviewed each PR: general correctness/design and
observable behavior. All six passed with no findings. Both refactor PRs had no observable behavior differences. The
final PR's dedicated reviewer confirmed that every difference qualifies for the plan's aligned cleanup-change exemption.
The general reviews received the full test-authoring checklist. No review finding was rejected. Commit and PR prose also
passed independent cold reads. Both existing final-PR reviewers also accepted the test-only fixture correction and
confirmed it does not hide a product behavior change.
