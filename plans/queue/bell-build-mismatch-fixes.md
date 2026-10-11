# Execute three triage outcomes: the notification bell and the read/unread toggle under a build mismatch

Written against main at 3f430cc4 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out three triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. Their
headings, which are also the feedback file names under `review_feedback_queue/`:

- `successful-notification-reads-leave-bell-unread-under.md`
- `clearing-notifications-leaves-cleared-bell-entries-visible.md`
- `manual-read-unread-toggles-cannot-visibly-toggle.md`

All three happen while the sidebar shows the reload notice, which appears when the helm answering the window runs a
different Farhelm build than the window itself. In that state the window stops refreshing the session list on its own
(SPEC_impl.md: a mismatch withdraws every unattended behavior, while anything the user explicitly asks for keeps
working). Delete, Replace, rename and creating a session already re-read the list after they succeed, so they keep
working. Three sidebar actions do not:

- Closing a session's notification list marks its entries read on the helm, but the bell on the row stays loud until a
  reload, because nothing re-reads the list afterwards.
- Clear in the notification list clears the entries on the helm and closes the list, but the bell stays on the row, and
  reopening it shows the cleared entries again. SPEC.md (Status) says Clear makes the bell disappear.
- A session's mark read / mark unread toggle (the menu item and the clickable status dot) saves the new state, but the
  row keeps its old dot and label. A second click then sends the same value again instead of reversing it.

In each case the helm holds the right state; only the window's copy of the list is stale. All three were found by
reading the code, not reproduced in a browser.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. Read the three entries and feedback files before starting. Where this file and a ledger entry seem to
disagree, the ledger entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Under a forced build mismatch, browser tests show the bell turning grey after its list closes, the bell disappearing
  after Clear, and a row's read/unread toggle visibly changing and reversing on a second click. The read-mark and Clear
  tests are in `e2e/tests/notifications.spec.ts`. Each test fails on main.
- The read/unread toggle's report bumps the list's refresh counter on success, not on failure, while the list is
  mounted, and drops both outcomes without panicking once the list has unmounted (the existing unit test, extended); the
  browser test shows the bump becoming a listing refresh.
- Failure handling is unchanged: a failed read mark stays silent and retries on the next close, and a failed Clear or
  toggle still lands on the row's error line.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, adds a changelog fragment, and passed the review gate.
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
Asked how to group the 31 outcomes, the user chose seven plans with one PR each; this plan is the notification bell
under a build mismatch group, so its outcomes share one PR, overriding root `AGENTS.md`'s one-PR-per-outcome rule for
these outcomes only. Review gate: gpt-6.1-sol at high effort. No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main by a fresh-context
planning review. Handling a complexity-gate trip by dropping that one outcome from the PR and shipping the rest is also
a planner proposal: it follows the precedent of earlier grouped triage plans and is how this plan honors the user's
"bounce back to me".

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work, including its browser
validation rules; Execute triage outcomes, as overridden above; Releases and the changelog; Agent scratch space; the
rule against tests that modify the test process's own environment variables), `plans/AGENTS.md` (Executing one plan),
`review_feedback_queue/AGENTS.md` (Lifecycle), `.agents/test-authoring.md` for test changes, `docs/test-sleep-check.md`
for the sleep checker, and `docs/test-run-evidence.md` (Recorded browser runs) for the Playwright runs.

## Outline

Line numbers drift; find the code by name. Verified on main at 3f430cc4. Everything below is in
`crates/farhelm-ui/src/list/view.rs` (the `SessionList` component) unless named otherwise.

**The shared mechanism.** The list already has one door for "a person did something, re-read the list": the
`request_listing` closure called with `Trigger::Explicit` (`crate::reader::Trigger`). Explicit demand is never withdrawn
under a mismatch (`reader::request_read`, `Demand::Explicit`), and delete (`do_delete`), rename (`on_rename_submit`),
Replace and create all call it after success for exactly this reason. All three fixes are "call that door after the
write succeeds", with no optimistic local edit of the listing. Do not change the reader, `ops::ReadGate`, or
`commit_listing`.

**Why no fence or rename-style guard is needed.** The ledger asks that a late, older listing reply not bring the loud
bell back, and asks whether Clear needs the ordering guard rename uses. Rename keeps an optimistic overlay (`renamed`
with `observed_from` from `poll_sequence`), and delete edits the listing locally and then calls
`listing_reads.write().fence()`; both exist because the screen runs ahead of any read. With refresh-only fixes, nothing
on screen runs ahead: the bell stays loud (and Clear's entries stay) until the refresh's reply commits. The reader runs
one listing read at a time per surface and queues the explicit demand behind any read in flight (the comment in
`e2e/tests/listing-mutation-fence.spec.ts` states this), and `ReadGate::accept_success` refuses any success whose
generation is not newer than the last applied, which also covers the stop path's direct read outside the reader. So an
older reply can land only before the refresh's reply, never after it. Write this reasoning into a comment at the shared
call sites so a later reader does not add a fence. If you find a path where an older reply can still commit after the
refresh, that is a gate trip for the affected outcomes, not a reason to add a fence on your own.

- Read mark on close: the `use_effect` that watches `bell_open` and spawns `crate::api::mark_notifications(..., false)`
  only handles the error result. On success, call a clone of `request_listing` with `Trigger::Explicit`. The spawned
  task belongs to the list's scope, like `do_delete`'s, so a direct call is safe. Keep the failure branch exactly as is
  (the `bell_read_sent` release and the warning; #1832 fixed that path). Update the effect's comment: it currently
  explains only the failure behavior. Test: in `e2e/tests/notifications.spec.ts`, force a mismatch, open the bell, close
  it, and expect the bell to stop being loud with no feed notification. Gate: a fix that needs to patch the listing's
  read mark locally, a fence, or any change to the reader is a gate trip.
- Clear: `clear_bell` handles `Ok(())` by removing the row's error entry. Also call `request_listing` with
  `Trigger::Explicit` there, after the `errors` write guard is dropped (end the match or scope the guard), and skip it
  when the early `errors.try_write()` fails because the list is gone. Leave the `Err` branch (the "clear notifications:"
  error line) unchanged and add no refresh on failure, since the ledger says the failed-clear error stays as is. Clear
  also closes the list, which runs the read mark above; the two refreshes coalesce in the reader into at most one
  follow-up read, and either order of the two PUTs ends with the bell gone. Answer the ledger's ordering-guard question
  in the comment and in the log: no guard, for the reason above. Test: in `notifications.spec.ts`, under a forced
  mismatch, Clear makes the bell disappear with no feed notification, and the stub records the `cleared` write. Start
  this test with every entry already read (stub `readThrough` at the newest `seq`), so the bell is present but quiet and
  closing the list sends no read mark, and assert that no `read` write was recorded for the session. Otherwise the
  read-mark fix's refresh can remove the bell on its own and the test stops pinning the Clear fix. Gate: same as the
  read mark.
- Read/unread toggle: `on_mark_seen` queues the write through `api::queue_seen_write`, and its report is
  `seen_toggle_report(errors, id)`. That report runs from a `spawn_forever` writer in `api.rs` (`spawn_seen_writer`),
  outside the list's scope and possibly after the list unmounted. Calling `request_listing` from it would panic on a
  dropped reader signal, and the reader task it spawns would belong to the root scope instead of the list. So bridge it
  through list-owned state: give `SessionList` a small `Signal<u64>` that counts settled successful toggles, have
  `seen_toggle_report` take it and increment it through `try_write` on `Ok` (ignored when the list is gone, the same
  pattern the report already uses for `errors`), and add a `use_effect` in `SessionList` that reads the counter and,
  when it is non-zero, calls `request_listing(Trigger::Explicit)`. The effect runs in the list's scope, so the reader
  task belongs to the list. Skip the effect's first run (count zero) so mount does not read twice. Update the docs on
  `seen_toggle_report` and on the `on_mark_seen` comment block. Unit test: extend
  `seen_toggle_report_tolerates_an_unmounted_list` so that, while mounted, a success increments the counter and a
  failure does not, and after unmount both outcomes are still dropped quietly; the existing
  "seen-toggle-report-panics-after-unmount" guarantee must still hold. Browser test, beside the existing mark-seen tests
  in `e2e/tests/sidebar.spec.ts`: a real session that is not selected (pin auto-select elsewhere with `pinAutoSelect`,
  because the automatic mark-on-open would otherwise interfere), a forced mismatch, then the row's
  `.session-row-mark-seen` toggle used twice, with the label (`mark read` / `mark unread`) or the status badge's unseen
  state flipping after each use. The menu closes when an item is chosen, so the second toggle reopens it (`openRowMenu`)
  or uses the status dot. The item appears only on a live row whose seen state the helm has reported, and under a
  latched mismatch nothing re-reads the list, so a mount read that lands before the supervisor classifies the session
  leaves the item missing for good. Therefore, before forcing the mismatch, poll the helm through
  `request.get('/api/sessions')` until the session is live and carries `seen_activity_at`; then call `forceBuildSkew`,
  then `page.goto("/")`, then wait for `.build-skew`. If that still proves unreliable, stub the row's live status and
  seen fields on the listing route and update them from recorded `PUT .../seen` writes, the way `NotificationStub` does,
  and log the choice. Gate: a fix that changes `api::queue_seen_write`'s or `SeenWriteReport`'s contract, needs a
  mechanism beyond one list-owned signal and one effect, or cannot keep the unmount test passing is a gate trip.
- Forcing the mismatch in the browser: `forceBuildSkew(page, stamp)` in `e2e/tests/helpers/fleet.ts` routes `**/api/**`
  through `route.fetch()` and fulfills with a different `x-farhelm-build`; `feed.spec.ts` and `sidebar.spec.ts` use it,
  and the page shows `.build-skew` once latched. The verdict latches at the first disagreeing reply and is never cleared
  by agreeing ones, so stub replies carrying the real helm's stamp do no harm once any other API reply has latched it.
  What matters is registration order: Playwright tries the most recently registered handler first, and both helpers
  fulfill. In the mismatch tests in `notifications.spec.ts`, call `forceBuildSkew(page, stamp)` before
  `serveNotifications` (and before any per-test route that must win), with a call-site comment saying that the order is
  the mechanism, then wait for `.build-skew` to be visible before acting. No helper change is needed. Keep a `stubFeed`
  so no real feed notification can do the refresh's job, and do not call `feed.notify` in the mismatch tests. Each
  test's assertion must fail on main: without the fix nothing re-reads the list under a mismatch, so the loud bell, the
  bell, or the toggle label stays as it was until the assertion times out.
- Changelog: `fix:` with `kind: fixed`, for someone running Farhelm: while the sidebar shows the reload notice because
  the helm runs a different version, closing a session's notification list now turns its bell grey, Clear removes the
  bell, and mark read / mark unread updates the row right away instead of waiting for a reload.

The complexity gate for these three is two one-line refresh calls, one counter signal with one effect, their comments,
and tests. Any optimistic local edit of the listing, a fence, a change to the reader, the read gate or the seen-write
queue's contract, or a change outside `view.rs` and the two named e2e specs (beyond bookkeeping and the changelog) is a
gate trip for the outcome that needs it.

### Validation

Follow root `AGENTS.md` "Finishing work":

- `cargo fmt --all -- --check`.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`. The change is not cfg-gated, so the desktop feature check
  adds nothing here.
- The extended unit test through the recorder:
  `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'farhelm-ui seen toggle report' --concurrency '4 nextest slots; retries 0' --tmux none -- cargo nextest run -p farhelm-ui -E 'test(seen_toggle_report)'`,
  after the pinned nextest setup in `docs/test-run-evidence.md`.
- Browser specs, because all three fixes and their tests are browser behavior: build per root `AGENTS.md` (`cargo build`
  and the `dx build --package farhelm-ui --platform web --release` web bundle, plus the one-time e2e install), then run
  through the recorder's Playwright mode on Chromium and WebKit with pinned tmux, per `docs/test-run-evidence.md`
  (Recorded browser runs): the whole `notifications\.spec\.ts` file, and the new toggle test in `sidebar.spec.ts`
  selected by `-g` (with the existing mark-seen tests beside it if the grep catches them; not the whole sidebar spec).
  Confirm each new test fails against main's code, for example by running it before applying the `view.rs` change, and
  record that run's ID in the log.
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`; apply `.agents/test-authoring.md` to the new
  tests.
- `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown.

Nothing here is macOS-only. The feed, the reader and the desktop asset set are not changed, so no desktop smoke, asset
parity or full browser battery is warranted.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-bell-build-mismatch-fixes-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
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
  `plan/bell-build-mismatch-fixes/01-bell-build-mismatch`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- For every outcome the PR carries out, it removes the feedback file under `review_feedback_queue/` and its line in
  `review_feedback_queue/INDEX.md`, and updates that outcome's `TRIAGE_OUTCOMES.md` Execution field to `complete` with
  the jj change ID, bookmark and PR URL. Record the change ID and bookmark before creating the PR, then add the URL to
  the same change and push again; no separate bookkeeping PR. An outcome dropped by its complexity gate is handled as
  Complexity gate says instead.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome in this plan carries the user's complexity gate, stated in its ledger entry and in the outline above: the
fix should stay about as small as assessed at triage. If one turns out to need significantly more (a new mechanism,
cross-component plumbing, a change well outside the files the outline names, or a product or design decision), do not
grow it and do not block the whole plan for it. Leave that outcome out of the PR, keep its feedback file and index line,
and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field to `pending`, waiting on the user, followed by what you
found and the realistic options with a recommendation (see `installer-startup-prune.md` in the ledger for the shape).
Name it among the open questions in the plan's report. The rest of the PR ships. Block per Unattended fallback only when
every outcome in this plan was dropped, since then there is no PR to deliver. Code that turned out to be already fixed
on main is not a gate trip: record the outcome as `discard` with "already fixed" and the fixing commit, per root
`AGENTS.md`, and remove its queue item.

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
the user's decisions. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim, as root `AGENTS.md` requires. Also ask the reviewer to check that no listing reply older than a fix's refresh
can commit after it (the reader's serialization and `ReadGate`), that the toggle's report touches list state only
through `try_*` accessors and that its refresh runs in the list's scope, that failure paths are unchanged, and that each
new browser test fails without the fix. Where you disagree with a finding, decide on the merits and log the DECISION. Do
not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new helper module, a
change to a shared formatter or protocol, edits outside the files the outline names beyond tests and bookkeeping), and
whenever the same component has needed repeated corrective review rounds, run a fresh-context review through
galaxy-brain with this charter, supplying the ledger entries, the user's decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

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
alternatives considered: in particular refresh versus a local optimistic edit of the listing, the answer to the ledger's
question whether Clear needs rename's ordering guard (and whether any fence was added), how the toggle's report reaches
the list (the counter signal and effect, or whatever replaced it) and whether the bell paths share that bridge or call
the reader directly, whether the toggle's browser test drives a real session or a stubbed listing, how the mismatch is
forced alongside the notification stub, the run IDs showing each new test failing without the fix, the Conventional
Commit type and changelog kind, every outcome dropped by its complexity gate and why, and every review finding you
decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

You never ask the maintainer anything in conversation while running this plan: not a required question, not an optional
one, and not an offer of more work. Nobody is watching, so a question only stalls the plan. A fork is either decided and
logged, or it is a block per `plans/AGENTS.md` (Executing one plan, step 10). Anything the maintainer should know,
including possible follow-ups, goes into the report.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate, meets the acceptance criteria above, and passed the review gate, with every outcome's
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out outcomes removed. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
