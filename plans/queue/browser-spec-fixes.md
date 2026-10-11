# Execute three triage outcomes: credential recovery after a failed rotation test, the long-folder sidebar test, and real-Claude spawn cleanup

Written against main at 3f430cc4 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out three triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. Their
headings, which are also the feedback file names under `review_feedback_queue/`:

- `failed-rotation-assertion-invalidates-subsequent-tests-credentials.md`
- `long-cwd-test-never-supplies-long-path.md`
- `missing-workspaces-cleanup-diagnostic-recognized-despite-discarded.md`

All three are defects in Farhelm's browser end-to-end suite, the Playwright specs that drive the web UI against a real
helm, supervisor and private tmux. None changes what someone running Farhelm sees; each makes a test either hide a real
failure or fail to check what it claims to check:

- One browser test rotates the helm's sign-in token and checks that an open browser is signed out and can sign back in.
  Rotation revokes the credential every other browser test in the run signs in with, and the test writes the new one
  back to the suite's shared sign-in file only after several of its own checks have passed. If any of those checks
  fails, every later browser test in the run fails to sign in, so one real failure shows up as a cascade of unrelated
  ones. The test's own comment says its credential write exists to prevent exactly that cascade, but it only covers
  failures after the write. Found by reading the code, not reproduced.
- The sidebar shows each session's folder on one line and, when the folder is too long for the row, clips the front of
  the path behind a leading ellipsis so its last directories stay visible. The test for that behavior uses `/tmp`, which
  never overflows the row, so it would keep passing if the ellipsis or the front-first clipping were removed. Found by
  reading the code.
- An opt-in browser test runs a real Claude and asks it to create a jj workspace and spawn a session into it. Its
  cleanup tries to forget that workspace and means to tolerate the case where Claude never created it, but it discards
  the command's error output, so the tolerance can never match a jj that fails on a missing workspace. A throw from any
  cleanup step before the end (closing the driver page, stopping the sessions, or that forget) then skips deleting the
  test's scratch directory, switching "start YOLO sessions without asking" back off, and resetting "run commands without
  asking" to off. Whether a missing workspace throws at all depends on the jj version: a recent jj exits 0 with a
  warning. Found by reading the code; the test only runs with `FARHELM_REAL_AGENT=1`.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. Read the three entries and feedback files before starting. Where this file and a ledger entry seem to
disagree, the ledger entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- A failure injected after rotation, verified in a recorded browser run and then removed, leaves a later spec signed in
  and still reports the injected failure; a hang injected after rotation (the test timing out) does the same; a failing
  recovery exchange does not replace the injected failure.
- The long-folder test uses a path that really overflows its row and fails when the ellipsis or the front-first clipping
  is removed, on Chromium and WebKit, with its logical-order and direction checks kept.
- The real-Claude spawn test's cleanup tolerates a missing workspace quietly (whether the jj on PATH succeeds or fails
  with a matching message), still reports any other forget failure, and removes its scratch directory and restores both
  preferences on every path through the cleanup.
- The PR removes each carried-out outcome's feedback file and index entry, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, and passed the review gate.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decision (2026-10-11):** the user asked for the whole review queue to be screened for findings
meeting all of these criteria: "it is CLEARLY a bug to be fixed. it's not ambiguous and doesn't need judgement that it
should be fixed given the nature of the product and its spec", "the fix seems reasonbly straight froward in the sense
that it won't add significant new complexity, require a re factor, etc.", and "its still relevant given current code vs
being stale due to code having changed"; and to triage that set as `fix code` "with an explicit complexity gate - it
must bounce back to me if during fixing it turns out to be more complicated and not fit the criteria". The decisions are
recorded in root `TRIAGE_OUTCOMES.md` (landed in #1896), one entry per feedback file, each with its own complexity gate
in its Completion criteria.

**The user's plan-time decisions (2026-10-11):** "let's use the planning system to schedule execution of the fixes".
Asked how to group the 31 outcomes, the user chose seven plans with one PR each; this plan is the browser specs group,
so its outcomes share one PR, overriding root `AGENTS.md`'s one-PR-per-outcome rule for these outcomes only. Review
gate: gpt-6.1-sol at high effort. No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main by a fresh-context
planning review. Handling a complexity-gate trip by dropping that one outcome from the PR and shipping the rest is also
a planner proposal: it follows the precedent of earlier grouped triage plans and is how this plan honors the user's
"bounce back to me".

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work, including its browser
validation rules; Execute triage outcomes, as overridden above; Releases and the changelog; Reproducing failures: narrow
tests first; Sharing the machine with other agents; Agent scratch space; Claude Code shell commands: no globs, no
expansions; the rule against tests that modify the test process's own environment variables), `plans/AGENTS.md`
(Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle), `.agents/test-authoring.md` for test changes,
`docs/test-run-evidence.md` (Recorded browser runs) for every Playwright run, and `docs/test-sleep-check.md` for the
delay check.

## Outline

Line numbers drift; find the code by name. Verified on main at 3f430cc4. All three changes are confined to `e2e/tests/`;
no product code, CSS or Rust changes.

- Credential recovery after a failed rotation: `e2e/tests/auth.spec.ts`, test "rotation logs out an open client and
  drops its feed and terminal sockets" (the file's last test). It calls `token("rotate")`, then asserts socket closure,
  a 401, and the sign-in prompt, then exchanges the replacement through the page, and only then writes
  `AUTH_STORAGE_STATE_PATH` (from `e2e/tests/helpers/device-auth.ts`) with
  `{ cookies, origins: [{ origin, localStorage: [{ name: DEVICE_SECRET_KEY, value }] }] }`. Later projects read that
  file through `harnessAuthorizationHeaders()` and `storageState` in `e2e/playwright.config.ts`, so a failure between
  rotation and the write revokes every later project's credential. Fix in this file: keep module-level state that the
  test sets (rotation attempted just before `token("rotate")`, credential persisted right after the existing write and
  `chmod`), and recover in a `test.afterEach` in `auth.spec.ts`, not in the test body's own `finally`. When the test
  hits its timeout, Playwright abandons the body's promise and runs after-hooks with their own time budget, while a
  `finally` in the body runs only if its pending `await` happens to settle during teardown; a missing
  `.auth-token-input` waits for the full test timeout, so a timeout here is plausible. In the hook, when rotation was
  attempted and nothing was persisted, recover: read the current token with `token("show")` (correct whether or not
  `rotate` itself completed), exchange it with `POST /api/auth/token` on a fresh request context the way
  `e2e/global-setup.ts` does (that route is on the helm's unauthenticated router and ignores a stale bearer header), and
  write the file in the same shape and mode. Refreshing this worker's headers is optional: later projects run in other
  workers that read the file. Factor the write into one small local helper that both the forward path and the recovery
  call, so the stored shape cannot drift; the success path's behavior and the file's shape stay as they are. The
  recovery catches its own errors and reports them (for example `console.error` plus a test annotation) without
  throwing, so a failed recovery never replaces the test's original failure. Validation injects a failure after rotation
  (see Validation); do not add a permanent test-only failure seam. Gate: a trip is anything beyond this test, a hook in
  the same file, and an optional helper beside it, such as changing how `e2e/playwright.config.ts` or
  `e2e/global-setup.ts` distribute the credential, adding a setup or teardown project, or a product change. Moving the
  shared write helper into `device-auth.ts` is allowed but not required; also switching `global-setup.ts` to it is not
  part of this outcome.
- Long-folder sidebar test: `e2e/tests/sidebar.spec.ts`, test "a long cwd shows its tail with characters in logical
  order" (keep the title). It creates its session with `cwd: "/tmp"`. The sidebar line is `.session-cwd` (overflow
  hidden, `text-overflow: ellipsis`, nowrap, `direction: rtl` in `crates/farhelm-ui/assets/app.css`) around the
  `dir="ltr"` span `.session-cwd-text` rendered in `crates/farhelm-ui/src/list/row.rs`. The sidebar is at most 600px
  wide (`sidebar-width.js`'s bound). Create a real directory well over 200 ASCII characters long with a distinctive last
  segment, nested under `stackScratchDir(...)` from `e2e/tests/helpers/scratch.ts` (already imported in this spec), with
  `fs.mkdirSync(..., { recursive: true })`, since the session starts in it. Plain ASCII avoids the row's
  hidden-character escaping, and a path under the stack's state directory is not abbreviated with `~`. Then:
  - assert the premise first: the `.session-cwd` element overflows (`scrollWidth > clientWidth`);
  - keep the DOM-text check (now the long path) and both computed-direction checks;
  - check front-first clipping geometrically: the inner span's left edge lies left of the clipper's left edge and its
    right edge sits at the clipper's right edge (a range over the last segment lying wholly inside the clipper is an
    acceptable equivalent); this fails if `direction: rtl` is removed, because the overflow then moves to the tail;
  - check the ellipsis: the clipper's computed `text-overflow` is `ellipsis`, with `overflow-x: hidden` and
    `white-space: nowrap`, which together with the proven overflow is the condition under which the engine draws it. The
    ellipsis is paint only and changes no layout box, so a computed-style check is the deterministic observation; pixel
    or screenshot analysis is out of scope. Remove the scratch directory in the existing `finally` (the stack also
    reclaims it). Gate: a trip is needing a UI or CSS change to make the test pass in either engine (that would be a
    product bug, reported, not fixed here), or needing pixel analysis to observe the ellipsis.
- Real-Claude spawn cleanup: `e2e/tests/spawn.spec.ts`, test "a real Claude creates a jj workspace and spawns into it
  without refreshing the observer". Its `finally` runs `execFileSync("jj", ["workspace", "forget", ...])` with
  `stdio: "ignore"`, then matches `/No such workspace|workspace .*does not exist/i` against the error's message plus
  `error.stderr`, which is always empty, so the tolerance never matches. The `finally` runs `driver.close()`,
  `cleanupSession(child)` and `cleanupSession(parent)` (which throws on any non-404 response) before the forget, and any
  throw among them skips `fs.rmSync(scratch)` and both `setLocal...(request, false)` restores. The ledger asks for
  restoration and scratch removal "on every path". Fix: pipe stderr (for example `stdio: ["ignore", "ignore", "pipe"]`
  with `encoding: "utf8"`) so the matcher sees jj's message, and nest the cleanup: the existing driver close, session
  cleanups and forget, in their current order, in an inner `try`, with scratch removal and the two restores in its
  `finally`, the restores chained so a failing first restore still runs the second. A non-matching forget failure is
  thrown, which reports it. Not masking the body's failure with a cleanup failure (a held error and a body-completed
  flag) is allowed but not required. On a recent jj (0.46.0 was observed at planning time), forgetting a missing
  workspace exits 0 with a "No such workspace" warning, so nothing throws; older versions may have failed with that
  text. There is no repository-pinned jj, so the ledger's "pinned jj" means the jj this test invokes; keep the regular
  expression for older versions and adjust it only if a failing version's real wording differs. Leave the fake-agent
  legs' cleanup as it is. Gate: a trip is needing to restructure shared helpers (`helpers/fleet.ts`,
  `helpers/approvals.ts`) or the other spawn tests' cleanup.
- Interactions: the three edits touch different files. The rotation fix decides whether specs that run after
  `auth.spec.ts` stay signed in, which is why the browser validation below runs the rotation test ahead of the
  long-folder test in the same run.
- Commit type and changelog: `test:`, because nothing changes for someone running Farhelm. Root `AGENTS.md` requires no
  changelog fragment for a `test` PR, and this one adds none.

### Validation

Follow root `AGENTS.md` "Finishing work" and "Reproducing failures". No Rust, CSS or product asset changes, so no
formatting, Clippy, nextest or desktop checks apply.

- Prerequisites, per root `AGENTS.md` browser validation: `cargo build`,
  `cd crates/farhelm-ui && dx build --package farhelm-ui --platform web --release`, the one-time
  `cd e2e && npm install && npx playwright install chromium webkit` if not already done, and the pinned tmux from
  `scripts/build-pinned-tmux-ci.sh` on PATH. Reuse matching builds.
- The final recorded run, from `e2e/`, through
  `python3 ../scripts/record-test-run.py --runner playwright --kind development --tmux required` (shape in
  `docs/test-run-evidence.md`, Recorded browser runs), both engines, one worker, retries 0: the child
  `npx playwright test 'auth\.spec\.ts' 'sidebar\.spec\.ts' 'spawn\.spec\.ts' -g 'rotation logs out|a long cwd shows its tail|a real Claude creates'`.
  The rotation test running ahead of the long-folder test proves the forward path still leaves the suite signed in; the
  real-Claude leg reports its SKIPPED line, which proves the edited file still loads. Read the retained output, not only
  the exit status.
- Injected-failure experiments for the rotation fix, recorded the same way with the selection
  `'auth\.spec\.ts' 'sidebar\.spec\.ts' -g 'rotation logs out|a long cwd shows its tail'`: (1) temporarily throw right
  after `token("rotate")`; the rotation test must fail with the injected message in both engines and the long-folder
  test must pass in both; (1b) instead hang after rotation (await a promise that never resolves) and expect the same
  outcome as (1), with the test reported as timed out; (2) additionally make the recovery exchange fail (for example
  exchange a bogus token), run with only `'auth\.spec\.ts' -g 'rotation logs out'`: on Chromium the reported failure
  must still be the injected one, with the recovery failure reported beside it; on WebKit expect an early sign-in
  failure, because Chromium's rotation was deliberately left unrecovered. One engine proves the property, which is
  Node-side control flow. Optionally run (1) on the unfixed test first to show the cascade. These runs fail by design:
  retain them, log their run ids and the expected outcome, and remove every injection before committing.
- Mutation experiments for the long-folder test, recorded with `'sidebar\.spec\.ts' -g 'a long cwd shows its tail'`:
  temporarily override the clipper's style from within the test (for example a `page.addStyleTag` setting
  `text-overflow: clip`, then separately `.session-cwd-text { display: block }`, which keeps both computed directions
  but moves the overflow to the tail) and confirm the test fails each time in both engines, and from the retained output
  that the second mutation fails on the geometric assertion. Overriding `direction: ltr` would fail the existing
  direction check first and prove nothing about the new assertion. Editing `app.css` instead works but needs a dx
  rebuild for each mutation. Remove the overrides before committing.
- The forget matcher: in a throwaway jj repository under your scratch directory, run a small Node script that calls
  `execFileSync` exactly as the edited cleanup does for a workspace name that does not exist and applies the edited
  regular expression; the call must either succeed or throw an error the expression matches. Run it once more in a
  directory that is not a jj repository; that call must throw and the expression must not match. Log `jj --version` and
  both observed outputs. Do not run the real-Claude leg itself: it needs vendor credentials and spends real vendor
  turns, so it is unverifiable here; log that, not a block.
- `python -B scripts/check-test-sleeps.py` with the isolated interpreter from `docs/test-sleep-check.md`; it must report
  zero unannotated delays. Apply `.agents/test-authoring.md` to all three tests.
- `dprint check` on the changed Markdown (`TRIAGE_OUTCOMES.md`, `review_feedback_queue/INDEX.md`).

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-browser-spec-fixes-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/browser-spec-fixes/01-browser-specs`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect; this PR is
  `test:` (see the outline). A `feat`, `fix`, `perf`, `style` or `revert` PR would add a changelog fragment under
  `releasing/changelog.d/` per root `AGENTS.md` (Releases and the changelog); a `test` PR adds none. If the type ever
  changes, validate a fragment with `python3 releasing/check-changelog.py format`.
- For every outcome the PR carries out, it removes the feedback file under `review_feedback_queue/` and its entry in
  `review_feedback_queue/INDEX.md` (entries can wrap onto a second line), and updates that outcome's
  `TRIAGE_OUTCOMES.md` Execution field to `complete` with the jj change ID, bookmark and PR URL. Record the change ID
  and bookmark before creating the PR, then add the URL to the same change and push again; no separate bookkeeping PR.
  An outcome dropped by its complexity gate is handled as Complexity gate says instead.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome in this plan carries the user's complexity gate, stated in its ledger entry and in the outline above: the
fix should stay about as small as assessed at triage. If one turns out to need significantly more (a new mechanism,
cross-component plumbing, a change well outside the files the outline names, or a product or design decision), do not
grow it and do not block the whole plan for it. Leave that outcome out of the PR, keep its feedback file and index
entry, and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field to `pending`, waiting on the user, followed by
what you found and the realistic options with a recommendation (see `installer-startup-prune.md` in the ledger for the
shape). Name it among the open questions in the plan's report. The rest of the PR ships. Block per Unattended fallback
only when every outcome in this plan was dropped, since then there is no PR to deliver. Code that turned out to be
already fixed on main is not a gate trip: record the outcome as `discard` with "already fixed" and the fixing commit,
per root `AGENTS.md`, and remove its queue item.

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
the user's decisions. This PR changes tests, so include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Also ask the reviewer to check that the rotation test's success path and stored credential shape
are unchanged, that a failing recovery cannot replace the original failure, that the long-folder assertions would fail
without the ellipsis or the front-first clipping while keeping the logical-order and direction checks, and that the
forget cleanup tolerates only a missing workspace, still reports other failures without masking the body's, and always
reaches scratch removal and both preference restores. Where you disagree with a finding, decide on the merits and log
the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new helper module, a
change to the Playwright config or global setup, edits outside the three spec files beyond an optional helper in
`device-auth.ts` and bookkeeping), and whenever the same component has needed repeated corrective review rounds, run a
fresh-context review through galaxy-brain with this charter, supplying the ledger entries, the user's decisions above,
this outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler
alternative was ruled out):

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
alternatives considered: in particular how the rotation recovery is structured and where a recovery failure is reported,
where the shared credential-write helper lives, the long path chosen and the observations used for the ellipsis and the
clipping, the jj version and both outputs the forget probe observed, how the spawn cleanup is nested and whether a
cleanup failure can mask the body's, the recorded run ids of the final run and of every injection and mutation
experiment with their expected outcomes, the Conventional Commit type and the absence of a changelog fragment, every
outcome dropped by its complexity gate and why, and every review finding you decided not to follow. The user will ask
for these later.

### Unattended fallback

You never ask the maintainer anything in conversation: not a direct question, not an optional one, not an offer of more
work, and not a harness question tool's prompt, because nobody is watching and the plan would stall. Every fork is
either decided by you and logged as a DECISION, or it is a block per `plans/AGENTS.md` (Executing one plan, step 10).
Things the maintainer should know and possible follow-ups (for example, the cleanup steps in the spawn spec that this
plan leaves alone) go into the report as statements, never into a question.

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate, meets the acceptance criteria above, and passed the review gate, with every outcome's
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out outcomes removed. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
