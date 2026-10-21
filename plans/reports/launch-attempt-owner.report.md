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

### Landing

Landed as #2516, #2517 and #2518, merged bottom-up, with the same content the report describes. It was the first of two
plans in this landing round, which stacked both and checked them together before either merged; helm-launch-admission
followed it.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained only queue bookkeeping and plan files for future work. A fresh-context reviewer and
I read this plan against helm-launch-admission, the other plan of this round, and against the recently landed work it
was written on top of.

- helm-launch-admission changes only the helm's create and restart routes. This plan changes only the supervisor (and
  one end-to-end test). Neither changes the messages between helm and supervisor or the kinds of error the supervisor
  returns, and nothing in the helm or UI matches on the refusal wording this plan extends.
- A create refused after its terminal is confirmed gone now saves any cleanup failure in its refusal, before its retry
  key is settled, so a retry gets the identical refusal back. The helm passes that refusal through unchanged under both
  plans.
- The launch progress file that never-started-launch-cause added is now removed on every path that removes the other
  launch files. Prepared spare checkouts from spare-launch-checkout keep their files in separate directories, which this
  cleanup never touches. Deleting a session still removes its launch files before its record, so a create retry that
  races a Delete cannot revive or close out the retry slot the Delete freed (create-reservation-retention).

#### Fixes made during the landing

None to the plan's code. helm-launch-admission's TODO.md removal conflicted with this plan's; the landing kept both
removals.

#### Possible follow-ups

- Found at landing, not mentioned in the report:
  - The changelog fragment's second clause, that Farhelm "avoids repeating successful cleanup when you stop a session
    whose agent could not start", describes internal work a user cannot observe. Nothing to do now; release-time
    changelog editing can drop it.
  - SPEC_impl.md still says a create that never starts removes its launch spec; it now also removes the status and
    progress files. The sentence is narrower than the code rather than wrong; possible follow-up: mention all three.

#### Checks

Together the two plans add about 1,500 lines and remove about 500 in how the supervisor removes a failed launch's files
and how the helm's create and restart routes resolve workspace trust, and both executors ran selected tests only; that
is why this round ran the whole Rust workspace, plus the browser specs that drive workspace trust, Replace, Restart
with, terminal restart and create retries end to end. All of the following ran on both plans stacked together on this
round's main, including the TODO.md conflict resolution:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the isolated test-sleep check, `dprint check` on every changed
  Markdown, JSON and TOML file, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, the
  workspace debug build and the UI's release web build: all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `68a7a44f-3142-4013-aaef-7d0dc5350e0c` (kept): 4153 passed, 1 failed; 40 tests
  marked as ignored did not run, and the output has no runtime `SKIPPED` message. The failure is a UI test,
  `api::save_order::tests::a_hung_save_releases_waiters_at_their_deadline`, which checks that a read queued behind a
  save that never finishes waits at least 30 milliseconds before giving up; it gave up slightly sooner. Neither plan
  changes the UI crate. The same test has failed this way in three earlier landing rounds, twice in the full workspace
  run (runs `e5bf0834` and `d854fd00`) and once in the UI's desktop-feature run (`d8ff6545`), and passes on its own: 50
  repeats on this round's code (run `e1bfd2c4-f6cd-4627-8ab3-028d8dd8ca26`) all passed. I attribute it to that
  pre-existing timing flake. It is not recorded yet; I am opening a separate draft PR that records it in FLAKES.md, adds
  its Deflake entry to TODO.md and its line to `deflake/known-flakes.txt`.
- `npx playwright test` in `e2e/` for `workspace-trust`, `claude-trust`, `restart-with`, `replace`, `terminal-restart`
  and `terminal-create-idempotency` on Chromium and WebKit, three workers each with its own helm and supervisors, no
  retries, run `34c8e607-2f6d-44ab-b53a-7bd8f1593c35`: 170 passed, none failed.
- Skipped: the desktop feature check and tests, the JavaScript harness and the website build (no UI, asset JavaScript or
  docs changed), the rest of the browser suite (the browser-visible behavior both plans could affect is in the specs
  above), the doctests (no documentation example changed), the desktop smoke test, the native Mac app (not runnable
  here) and the installer (unchanged).

The report's checks ran selected supervisor and lifecycle tests on the plan's own base and skipped the full Rust
workspace and the browser suite; the workspace and the browser specs above ran this round. Nothing in the executor's
report became untrue at landing.
