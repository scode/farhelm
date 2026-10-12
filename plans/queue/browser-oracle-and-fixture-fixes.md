# Execute twelve triage outcomes: browser specs that prove what they claim and clean up after failure

Written against main at d3b28820 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out twelve triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. Their
headings, which are also the feedback file names under `review_feedback_queue/`:

- `pending-mount-cancellation-test-cannot-distinguish-cancellation-surviving.md`
- `sibling-close-concurrency-test-executes-both-closes-sequentially.md`
- `retarget-test-failing-deliver-old-target-plan.md`
- `unreachable-operation-test-prove-absence-queued-work.md`
- `work-order-progress-reads-observe-empty-file-during.md`
- `held-synthetic-delete-handlers-delay-failure-teardown.md`
- `process-death-falsely-inferred-proc-read-errors.md`
- `history-fixture-enables-yolo-installing-restoration-guard.md`
- `keyboard-launch-fixture-has-same-separate-setup-ownership.md`
- `history-fixtures-policy-restoration-skip-remaining-cleanup.md`
- `keyboard-launch-fixtures-policy-restoration-skip-session-deletion.md`
- `busy-selection-test-loses-source-second-creation-fails.md`

All twelve are defects in the Playwright browser specs under `e2e/tests/`, which drive the built web UI against a real
helm and supervisor. Nothing changes for someone running Farhelm. They fall into two kinds.

The first seven are tests that can pass while the behavior they guard is broken, or whose failure path strands teardown:

- When a terminal tab leaves the session before its terminal could mount, the pending mount must be cancelled. The test
  only counts mounted terminals, and a surviving retry can never mount once the tab's element is gone, so a retry loop
  that runs forever passes.
- Closing one tab must not block closing a sibling tab. The test closes the second tab only after the first is gone, so
  a global close lock would pass.
- When a host's destination changes while an Update is being planned, the old target's plan must never be offered. The
  test holds the planning request before the helm ever sees it, so no old-target reply exists to deliver late, and it
  asserts without waiting for the released reply to arrive.
- A Stop against a session on an unreachable host must be refused with nothing queued for later. The test checks the
  refusal text and the stale row, then deletes the session after the host returns without checking the agent survived
  the reconnect.
- The work-ordering test's producer truncates and rewrites its progress file; a read between the two sees an empty file,
  which `Number("")` turns into 0, so "the producer emitted 30 more lines" can be satisfied without them.
- Two synthetic-session delete tests hold their DELETE responses and release them only on the success path, so a failed
  assertion leaves a handler that `afterEach`'s route draining waits on until the hook times out.
- Closing a tab must kill its shell. The test's process check treats any error reading `/proc/<pid>/stat` as the process
  being gone, so an unreadable process that is still running would pass the kill assertion.

The other five are fixture cleanup that an earlier failure can skip:

- Two sidebar tests turn on "YOLO without asking" for the shared test helm before their `try`. The helper writes the
  setting and then reads it back as a separate, fallible step, so a failure after the write leaves the setting on for
  every later spec in the run.
- The same two tests restore the setting first in their `finally`, so a failed restore skips closing a second browser
  context and deleting the sessions they created.
- The busy-selection test creates two sessions and installs a route before its `try`, so a failure creating the second
  session or installing the route leaves the first (or both) running in the shared stack.

None of these was reproduced at runtime; all were confirmed by reading the code. Each ledger entry (Assessment,
Decision, Completion criteria) is the authoritative statement of what the PR must do for that outcome. Read the twelve
entries and feedback files before starting. Where this file and a ledger entry seem to disagree, the ledger entry wins
and the disagreement is a DECISION to log (or a gate trip, per Complexity gate). When this plan starts, each outcome's
Execution field reads "planned in `plans/queue/browser-oracle-and-fixture-fixes.md`", which the planning PR set; that is
the expected starting state, not `pending`.

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Every changed test keeps its existing assertions unless its ledger entry says otherwise, and its comment or docstring
  states what it now proves and why.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, and passed the review gate.
- Every changed test passed in a recorded browser run on both Chromium and WebKit, and the sleep check reports no
  unannotated delay.
- No product code changes. The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decisions (2026-10-11, recorded in `TRIAGE_OUTCOMES.md`, landed in #1919):** the review queue was
triaged by class. For tests that can pass without exercising what they claim, or fail on correct code, the user chose
"Fix, skip hard ones": fix them, and if a fix needs a new test-only hook in product code, record `discard` instead; any
other complication goes back to the user. That decision covers the first seven outcomes above. For test setup that is
not cleaned up when an earlier step fails, the user chose "Fix all, with gate": fix all of them, and stop and bring one
back if it needs more than a contained test change. That decision covers the last five. Each entry restates its gate in
its Completion criteria.

**The user's plan-time decisions (2026-10-11):** "use the planning system to start planning all of the already triaged
entries". Asked how to group the outcomes, the user chose "8 plans as proposed"; this plan is the browser-test group.
Asked about PR shape, the user chose "One PR per plan", so this plan's outcomes share one PR, overriding root
`AGENTS.md`'s one-PR-per-outcome rule for these outcomes only. Review gate: "gpt-6.1-sol high", a single fresh-context
agent with the general charter, no swarm. No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main at d3b28820.
Handling a complexity-gate trip by dropping that one outcome from the PR and shipping the rest is also a planner
proposal: it follows the earlier grouped triage plans (#1901) and is how this plan honors the user's gates.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work, including its browser
end-to-end paragraph; Execute triage outcomes, as overridden above; Releases and the changelog; Reproducing failures:
narrow tests first; Sharing the machine with other agents; Agent scratch space), `plans/AGENTS.md` (Executing one plan),
`review_feedback_queue/AGENTS.md` (Lifecycle), `.agents/test-authoring.md` for every test change,
`.agents/narrow-tests.md` (Playwright: one spec, one engine) and `docs/test-run-evidence.md` (Recorded browser runs) for
the runs, and `docs/test-sleep-check.md` for any delay a fix adds.

## Outline

Line numbers drift; find the code by test title. Verified on main at d3b28820. All twelve fixes are in `e2e/tests/` spec
files; no product code changes, and no shared helper in `e2e/tests/helpers/` needs to change (a gate trip if one does,
beyond a doc tweak). `sidebar.spec.ts` carries five outcomes and `terminal-tabs.spec.ts` three, in separate tests.

- Pending-mount cancellation (`pending-mount-cancellation-test-cannot-distinguish-cancellation-surviving.md`):
  `terminal-tabs.spec.ts`, "a pending tab mount is cancelled when the tab leaves the desired set". The test withholds
  `window.Terminal` with `delete`, opens and closes a tab, restores `Terminal`, and polls `mountedIslands`. In
  `crates/farhelm-ui/assets/terminal.js`, `mountWhenReady`'s `tryMount` re-arms a 50 ms timer while any readiness check
  fails, and `window.Terminal` is the first check it reads; the only reader of `window.Terminal` in that file is that
  check. So the test can observe the retry loop itself without a product hook: instead of deleting `window.Terminal`,
  replace it with an accessor (`Object.defineProperty` on `window`, configurable) that returns `undefined` and counts
  reads. Before closing the tab, require the count to rise (the positive witness that the tab's mount is genuinely
  pending, which the finding also notes is missing). After the tab slot is gone, read the count, hold a short
  observation window of several retry periods (about 500 ms; a deliberate observation window, so give it a `sleep-ok:`
  reason per `docs/test-sleep-check.md`), and require the count unchanged. Then restore the real `Terminal` (redefine
  the property as the stashed value) and keep the existing `mountedIslands` poll. Gate: if the counter also moves for
  unrelated reasons on a correct build (some other mount attempt reading the global), the witness is not usable without
  a product hook, so record `discard` per Complexity gate. The plan `ui-layout-and-escaping-fixes.md` also changes the
  terminal mount path in `terminal.js` and edits `terminal-tabs.spec.ts`; if it lands before this PR, re-verify during
  the careful rebase that `mountWhenReady`'s readiness check is still the only reader of `window.Terminal` while a mount
  is pending, since the witness depends on that premise.
- Sibling close (`sibling-close-concurrency-test-executes-both-closes-sequentially.md`): `terminal-tabs.spec.ts`, "a
  confirmed close sends one DELETE per tab, and a sibling can close alongside it". Make the route hold tab A's DELETE
  (record it, then await a gate before `route.continue()`), keep B's DELETE unheld, and release A's gate in the test's
  `finally` as well as on the success path. After the double confirm on A, wait until A's DELETE reached the hold and
  check A's slot is still present (its close is in flight). Then close B and require B's DELETE to be recorded and B's
  slot to disappear while A is still held. Only then release A and wait for its slot to go. Keep the one-DELETE-per-tab
  assertion. If B's confirm control cannot be addressed unambiguously while A's close is pending, scope the locator to
  B's slot. Gate: if the UI on a correct build will not issue B's close while A's is in flight, that contradicts the
  per-tab guard in `crates/farhelm-ui/src/session_view.rs` and is a product finding, so bring it back.
- Retarget with a late old-target plan (`retarget-test-failing-deliver-old-target-plan.md`): `provisioning.spec.ts`, "a
  retarget and a foreign run each invalidate an unsubmitted update". The first leg's route awaits its gate before
  `route.continue()`, so the helm only plans after the retarget. Change that route to execute the planning request first
  (`route.fetch()`), signal that the old target's reply is captured, await the gate, then deliver it with
  `route.fulfill({ response })`. The test waits for the capture signal before posting the destination change, keeps its
  existing proof that the pending intent disappeared, then registers a `page.waitForResponse` for that planning request,
  releases the gate, and waits for the response to arrive before asserting that no `.host-detail` appeared, the details
  toggle stayed unchecked, and `confirms` is still 0. Apply the same received-before-asserting wait to the second leg's
  release (that leg's reply is not stale; it only lacks the completion wait). The late-reply guard this exercises is the
  binding/epoch check in `crates/farhelm-ui/src/provisioning.rs`. Gate: if the UI on a correct build acts on the late
  old-target reply, that is a product finding; bring it back.
- Nothing queued on an unreachable host (`unreachable-operation-test-prove-absence-queued-work.md`):
  `terminal-multihost.spec.ts`, the serial group "with the remote supervisor killed", test "op-refused-on-unreachable:
  the helm's own 409 words, and nothing queued", whose `afterAll` restores the remote supervisor (only if it is not
  alive) and then deletes the session. The ledger requires that after the host reconnects and before cleanup, the test
  asserts the agent is still running. Because the group is `describe.serial`, add a last test in the group that restores
  the supervisor with `restoreRemoteSupervisor`, then requires the stale session to be listed non-stale with a live
  status (`LIVE_BADGE`, as the group's `beforeAll` uses), and still live after an observation window of about five
  seconds. A Stop queued for later would be delivered on reconnect, and the `sleep 600` agent dies on its first SIGTERM,
  so the window has to cover that delivery plus the helm's listing latency until the death would show; the kill grace
  does not matter here. Give the wait a `sleep-ok:` reason saying exactly that. The existing `afterAll` keeps its
  conditional restore as the safety net and still deletes the session. Putting the check at the end of `afterAll`
  instead is acceptable if a final test does not fit; log the DECISION. Gate: if the agent is gone after reconnect on a
  correct build, that is a product finding; bring it back.
- Work-order progress reads (`work-order-progress-reads-observe-empty-file-during.md`): `work-start-order.spec.ts`,
  `startProducer` writes `printf '%s' "$n" > progress`. Make the producer publish atomically: write to a sibling temp
  file and `mv` it over the progress file, so a reader sees either the old or the new number, never an empty file. That
  fixes every read site at once (the initial `beforeA`/`beforeB` baselines and `mixedProgress`). Update the producer's
  docstring. Optionally also have the baseline reads reject a non-numeric read, if that is clearer; the ledger accepts
  either. Gate: anything beyond the producer program and the read sites.
- Held synthetic DELETE handlers (`held-synthetic-delete-handlers-delay-failure-teardown.md`): `terminal.spec.ts`, "an
  interrupted session shows its badge and deletes without confirming" and "an error session shows its badge with detail
  and deletes without confirming". Both await `deleteHeld` in the route handler and call `releaseDelete()` only as their
  last line. Wrap each test body after the route is installed in `try`/`finally` that calls `releaseDelete()`, with the
  comment the file already uses elsewhere (route-draining teardown must not wait forever after an assertion fails). The
  ledger asks to check the other variants the finding names: on main the held handlers in "multi-session flow: create
  two, open and type in one, stop and delete the other, then delete the first with confirmation" (`releaseDeleteB`),
  "exited session deletes immediately with no confirming state" (`releaseDelete`) and "stop's in-flight guard disables
  this row's stop, delete, and open, while another row's stop and delete stay usable" (`releaseStop`) already release in
  `finally`; confirm that and change nothing there. Gate: none expected beyond these two tests.
- /proc read errors taken as death (`process-death-falsely-inferred-proc-read-errors.md`): `terminal-tabs.spec.ts`,
  `readProcStat`, used by `processIdentity` and `pidAlive` in "closing a tab confirms in-page, then kills its shell and
  drops it from the session" (its kill check is an `expect.poll` on `pidAlive(shell)`). `readProcStat` catches every
  `readFileSync` error and returns `undefined`, which `pidAlive` reads as gone. Per the ledger, only `ENOENT` or `ESRCH`
  (the process no longer exists) returns `undefined`; any other error is rethrown, so the poll keeps retrying and the
  test fails with that error rather than passing. Keep the zombie and start-time rules, and update the helper's
  docstring. Gate: anything beyond this helper.
- YOLO restore armed after the write (`history-fixture-enables-yolo-installing-restoration-guard.md`,
  `keyboard-launch-fixture-has-same-separate-setup-ownership.md`): `sidebar.spec.ts`, "composer holds offered history
  steady until destination and search promotion" and "composer Enter on a focused recent launches the filled setup".
  Each calls `await setLocalYoloWithoutAsking(request, true)` immediately before its `try`. Move that call to be the
  first statement inside the `try`, so the `finally`'s restore runs whenever the write may have committed (the helper in
  `e2e/tests/helpers/fleet.ts` posts the setting, then reads it back separately). Restoring an unchanged setting is
  harmless. Gate: anything beyond moving the call and the comment above it.
- Restore failure skips the rest of cleanup (`history-fixtures-policy-restoration-skip-remaining-cleanup.md`,
  `keyboard-launch-fixtures-policy-restoration-skip-session-deletion.md`): the same two tests' `finally` blocks await
  the restore first, then `second?.close()` and `cleanupSession` (history), or the `created` deletion loop (keyboard
  launch). Run every cleanup step even when an earlier one throws, and report the failures together: for example,
  collect each step's error into a list, run all steps, and if any failed throw one error naming all of them (an
  `AggregateError` is fine). Do not let a cleanup error replace an assertion error already thrown from the `try`; a
  cleanup error then is reported alongside it or logged, but the original failure stays the test's reported cause. A
  small local helper in `sidebar.spec.ts` shared by these two tests is fine. Gate: a change to `e2e/tests/helpers/`.
- Busy-selection setup ownership (`busy-selection-test-loses-source-second-creation-fails.md`): `sidebar.spec.ts`, "an
  unanswered view operation disables row selection until it completes". Sessions `a` and `b` and the restart route are
  created before the `try`. Declare `a` and `b` as possibly undefined before the `try`, create both and install the
  route inside it, and have the `finally` release the gate, unroute, and clean up whichever sessions exist, each
  attempted even if an earlier one throws (the same run-all-steps shape as above). Gate: anything beyond this test.
- Changelog: the PR is test-only, so its type is `test:` and, per root `AGENTS.md` (Releases and the changelog), it adds
  no changelog fragment. If a gate trip leaves only some outcomes, the type stays `test:`.

### Validation

Follow root `AGENTS.md` "Finishing work", its browser end-to-end paragraph, and "Reproducing failures: narrow tests
first"; no full battery.

- Build what the browser suite drives, reusing matching builds when they exist: `cargo build`, and
  `cd crates/farhelm-ui && dx build --package farhelm-ui --platform web --release`, plus the one-time
  `cd e2e && npm install && npx playwright install chromium webkit` if the checkout lacks them.
- During development, iterate on one spec and one engine per `.agents/narrow-tests.md` (Playwright: one spec, one
  engine), with the pinned tmux first on PATH.
- The evidence is one recorded browser run through `scripts/record-test-run.py --runner playwright`, from `e2e`, per
  `docs/test-run-evidence.md` (Recorded browser runs), with `--kind development`, `--tmux required`, the pinned tmux
  first on PATH, `--concurrency 'one browser worker; retries 0'`, and a selection naming exactly what runs: the six
  changed spec files (`terminal-tabs`, `provisioning`, `terminal-multihost`, `work-start-order`, `terminal`, `sidebar`)
  narrowed with `-g` to the changed tests' titles. Selecting a test of the serial "with the remote supervisor killed"
  group runs that group's hooks, so select the whole group. Both engines must execute cases. If the recorder refuses the
  combined selection, split it into per-file runs and log that. Do not pass a hard `--timeout` that would kill a run on
  a guess; watch the run and decide on evidence, as `plans/AGENTS.md` describes for landings.
- Read the retained output for skipped tests: the multihost group skips without a fleet (`requireFleet`), and a skip is
  not a pass; log it and say so in the report.
- A failure under another agent's load is an environment event (root `AGENTS.md`, Sharing the machine): keep the
  retained run, re-run narrowly, and do not log a flake from one observation.
- Optionally, a one-off local mutation check where it is cheap and decisive (for example, temporarily not clearing the
  pending timer in `terminal.js`'s `unmount` and confirming the pending-mount test now fails), reverted before
  committing; log whether it ran.
- `python -B scripts/check-test-sleeps.py` with the isolated interpreter from `docs/test-sleep-check.md`; it covers
  browser tests and must report zero unannotated delays. Apply `.agents/test-authoring.md` to every changed test.
- `dprint check` on the changed files (the specs, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/INDEX.md`).

No Rust tests, doctests, desktop, installer or JS-harness runs: nothing they cover changes (the JS harness only if a
change unexpectedly touches `crates/farhelm-ui/js-tests` inputs).

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-browser-oracle-and-fixture-fixes-log.md` in the parent directory of the checkout you run in, derived as
that section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution fields on the PR) rather than starting over. If it does not exist, this is a
fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/AGENTS.md` (Executing one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/browser-oracle-and-fixture-fixes/01-browser-specs`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's PR-shape decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. This PR is
  test-only, so its type is `test:` and it adds no changelog fragment; if the change unexpectedly gains a user-visible
  effect, the type and a fragment under `releasing/changelog.d/` follow root `AGENTS.md` (Releases and the changelog),
  validated with `python3 releasing/check-changelog.py format`.
- For every outcome the PR carries out, it removes the feedback file under `review_feedback_queue/` and its line in
  `review_feedback_queue/INDEX.md`, and updates that outcome's `TRIAGE_OUTCOMES.md` Execution field to `complete` with
  the jj change ID, bookmark and PR URL. Record the change ID and bookmark before creating the PR, then add the URL to
  the same change and push again; no separate bookkeeping PR. An outcome that trips its complexity gate is handled as
  Complexity gate says instead.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome carries the user's gate, stated in its ledger entry and in the outline above, and the two decisions treat
a trip differently:

- The first seven outcomes (pending mount, sibling close, retarget, nothing queued, work-order progress, held DELETE
  handlers, /proc read errors) fall under "Fix, skip hard ones". If the fix needs a new test-only hook in product code
  (anything under `crates/`, such as a debug global in `terminal.js`), do not add it. In the same PR, leave the recorded
  Outcome as it is and set the Execution field to "`complete`: gate tripped (<reason>); discarded per the recorded
  decision", followed by the jj change ID, bookmark and PR URL; remove its feedback file and index line, and name it in
  the report's "things you should know".
- Any other complication, for those seven or for the last five (a change well outside the test the outline names, a
  shared helper change, a product or design decision, or a strengthened test that exposes a real product failure), means
  do not grow the fix and do not block the whole plan for it. Leave that outcome out of the PR, keep its feedback file
  and index line, leave the recorded Outcome as it is, and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field
  to "`pending`, waiting on the user", followed by what you found and the realistic options with a recommendation (see
  `installer-startup-prune.md` in the ledger for the shape). Name it among the open questions in the plan's report.

The rest of the PR ships. Block per Unattended fallback only when no outcome is left to ship. Code that turned out to be
already fixed on main is not a gate trip: leave the Outcome as it is, set Execution to "`complete`: already fixed on
main by <commit>" with the change ID, bookmark and PR URL, and remove its queue item.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: each outcome's `TRIAGE_OUTCOMES.md` entry, the outline above, and
the user's decisions. The PR changes tests, so include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Also ask the reviewer to check, for each changed test, that it would now fail on the regression it
names and still passes on correct behavior: a pending-mount retry that survives the tab's removal; a global close lock
that serializes sibling closes; a UI that acts on a late old-target plan; a Stop queued and delivered on reconnect; a
progress read that sees an empty file; a held DELETE handler stranded by a failed assertion; a `/proc` read error taken
as the shell's death; a YOLO setting left on, or a session or browser context left behind, after a failed setup or
restore step. And ask it to check that a cleanup error never hides the test's original failure, and that every new delay
carries a sound `sleep-ok` reason. Where you disagree with a finding, decide on the merits and log the DECISION. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a change to a shared
helper under `e2e/tests/helpers/`, any change under `crates/`, edits outside the six named spec files beyond
bookkeeping), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the ledger entries, the user's decisions above, this outline, the
current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled
out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A departure that
the review does not find necessary and small is a complexity-gate trip, handled as above.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how the pending-mount witness counts retries and the length of its observation
window, how the sibling-close test holds and releases A's DELETE, how the retarget test captures and delivers the
old-target reply, where the reconnect liveness check lives (a final serial test or `afterAll`) and its observation
window, whether the producer publishes by rename or the reads reject empty values, the shape of the run-all-cleanup
steps and how the original failure is preserved, whether any one-off mutation check ran, every skipped test seen, the
Conventional Commit type, every outcome dropped or discarded by its complexity gate and why, and every review finding
you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

You never ask the maintainer anything in conversation while running this plan: not a direct question, not an optional
one, not an offer of more work, and not a turn that ends waiting for a confirmation. Every fork is either decided by you
and logged as a DECISION, or, when only the maintainer can decide it, a block per `plans/AGENTS.md` (Executing one plan,
step 10). Things the maintainer should know and possible follow-ups go into the plan's report, written as statements.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate, meets the acceptance criteria above, and passed the review gate, with every outcome's
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out or discarded outcomes removed. Open,
not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.
