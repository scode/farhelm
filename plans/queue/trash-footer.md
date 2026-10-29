# UI: Trash dialog gets the shared footer

Written against main at 97d5a6cd on 2026-10-28. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> - **Revisit Trash's pinned header actions.** Decided on the maintainer's behalf on 2026-10-28 for the Steel dialogs
>   plan: Trash keeps its totals, progress, Delete all and Close in its pinned header instead of moving them to the
>   shared dialog footer. Revisit whether Trash should get the footer like the other dialogs. Plan:
>   `plans/queue/trash-footer.md`.

The Steel redesign (`plans/queue/steel-dialogs.md`, landed) brought every dialog to one anatomy: a header with the
title, an optional summary and a close control; a scrolling body; a pinned footer with an optional hint on the left and
the actions on the right, Cancel (or Close) first and the main action last. While landing it, the landing monitor
decided on the maintainer's behalf that Trash keeps its old pinned-header layout. The maintainer has now revisited that
call and wants Trash to follow the shared anatomy like the other dialogs.

### What changes

- **Header**: the shared dialog header. It keeps the title (with the trash can, upside down and bobbing while a deletion
  runs, as today) and the total ("N checkouts · size on reachable hosts") as its summary, and gains the shared header
  close control. That close control follows exactly the same rules as Close: unavailable while a deletion runs, with the
  same explanation on hover.
- **Footer**: a pinned footer. Its left side holds the deletion progress status (today's always-mounted status region,
  which screen readers announce when a run starts). Its right side holds Close, then Delete all (the danger action,
  last).
- **Delete all's confirmation** moves from the top of the archive list to a place directly above the footer, next to the
  button that opened it. As today, its answers are brought into view and focused when it opens, and cancelling returns
  focus to Delete all. Per-host "delete all on <host>" confirmations stay inside their host sections.
- **The archive list** remains the one scrolling region between header and footer.

### What does not change

Every deletion behavior SPEC.md describes for Trash: what Delete all, per-host delete and per-checkout delete do and
when they are available, the in-place confirmation's wording and guards, one checkout at a time per host, the progress
counts, spinners and "N of M deleted", Close and Escape staying unavailable until a running deletion finishes, the
listing recheck afterwards, and the Trash button in the session list. Initial focus stays on Close. Escape, focus return
and modal isolation keep their current owners.

### Acceptance criteria

- The Trash dialog uses the shared header, scrolling body and pinned footer, with the layout above, in Steel and Steel
  Light, at a wide window and at phone width (where the footer's actions stack like other dialogs' footers).
- Close and the header close control are both unavailable, with the explanation, while a deletion runs; Escape too.
- Delete all's confirmation appears directly above the footer, is focused and in view when it opens, and cancelling it
  returns focus to Delete all.
- Sequential Tab follows the visual order: header close, the archive body, then the footer's Close and Delete all. This
  is the dialog Tab-order rule the maintainer kept on 2026-10-28 (the session launcher, Rename, Templates and tunnel
  consent already follow it).
- No deletion behavior changes; the existing Trash browser tests keep testing the same behavior, updated only for where
  controls moved, and no assertion is weakened.
- SPEC.md's Trash paragraph and SPEC_impl.md's dialog paragraphs describe what was built; the code comment in
  `crates/farhelm-ui/src/list/trash.rs` that says Trash deliberately keeps its controls in the header goes.
- The docs website's session list page no longer says Delete all is "at the top".
- A changelog fragment.
- The TODO.md entry quoted above is removed.

## Decisions already made

Settled with the maintainer on 2026-10-28, on a planning page with before/after mockups:

- **Move Close and Delete all to a pinned footer, like the other dialogs.** The maintainer chose this over keeping the
  pinned-header exception. The reason given on the page, which the maintainer accepted: consistency across dialogs was
  the Steel redesign's point, and the cost is about one row of archive-list height. The mockup showed the header with
  title, total and a close control, and the footer with the progress text on its left and Close then Delete all on its
  right; Delete all's confirmation "would move from the top of the list to just above the footer, next to the button
  that opened it".
- **A correction to that rationale, found after the page by the planning review:** "about one row" holds at desktop
  widths. At phone width the shared footer stacks its actions as full-width rows, so Trash's list loses about two button
  rows there, plus the progress row while deleting. The maintainer was told this when approving the planning PR. The
  plan follows the shared phone behavior; the report states the resulting phone list height.
- **Deletion, its confirmation, the progress counts and the rule that Close waits for a running deletion stay the same**
  (stated on the page as part of the chosen option).
- **Runs after `steel-sentence-case.md`** has landed, because that plan rewrites the labels in this same dialog ("delete
  all", "close") to sentence case. Use whatever labels main has when you start.
- **Code reviewer:** a fresh-context gpt-6.1-sol at high effort. **Visual reviewer:** Opus 5.5 at high effort, as the
  Steel plans had. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.
- In the same planning round the maintainer kept three other calls from the Steel dialogs landing unchanged: the dialog
  Tab order above, the launcher's folder suggestions closing when Enter launches, and the launcher's folder suggestions
  covering the controls below them while open. Do not revisit those here.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md's Trash paragraph (in "Session list", starting "The session list's header
places Trash, Templates and New") for every behavior that stays; SPEC_impl.md's dialog-frame paragraphs (the one
beginning "Templates keeps its existing root scroll owner", which states today's Trash exception, and the confirmation
focus paragraph that says "trash focuses and scrolls its confirming answer into view"); root `AGENTS.md`, "Code rules",
"Tests and validation", "Conventional Commits", "Changelog fragments"; `.agents/test-authoring.md`;
`.agents/validation.md`; `website/AGENTS.md` for the docs page.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): one PR; no icon tile in the header
(today's Trash has none, and a tile would cost list height for no information); the confirmation's exact container.

## Outline

The existing pieces to use:

- `crates/farhelm-ui/src/modal_dialog.rs`: `DialogHeader` (title, optional summary and tile, a caller-rendered close
  element that keeps its own disabled state and handler), `DialogFooter` and `DialogActions` (action-row spacing, caller
  supplies the order). Rename (`crates/farhelm-ui/src/rename.rs`) and tunnel consent
  (`crates/farhelm-ui/src/tunnels.rs`) are the precedent to copy: each passes its own `button.dialog-close` that is
  disabled while an operation runs, and tunnel consent keeps its dialog's accessible name by putting an id-bearing span
  inside the title. Settings is not a direct precedent: it goes through the host dialogs' header adapter, whose close is
  never disabled. Settings does show the footer hint (`.dialog-footer-hint`).
- The launcher's notices region (`crates/farhelm-ui/src/list/create_form.rs`, the conditional `ScrollRegion` with class
  `launch-composer-notices` between the body and the footer; SPEC_impl.md, the session launcher paragraph) is the
  precedent for a bounded, keyboard-scrollable region above a footer that exists only while needed.
- `crates/farhelm-ui/src/list/trash.rs`, in `TrashControl`'s dialog render: today a hand-built
  `div.dialog-header.checkout-trash-header` holding the title, the total, `.checkout-trash-header-actions` (Delete all,
  and Close inside a hover-explanation wrapper because a disabled native button suppresses hover in some engines) and
  the always-mounted `role="status"` progress region; then a `ScrollRegion` (`.checkout-trash-body`) whose first child
  is the global `ConfirmBand`. Focus is set with `DialogFocus::remembered(".trash-close", ...)`; the close handler and
  Escape share one `close` closure, which already refuses while busy.
- `crates/farhelm-ui/assets/app.css`: the `.checkout-trash-*` rules, including the comment explaining why the header
  wraps and only the body scrolls.

Steps, in one PR `plan/trash-footer/01-footer`:

1. Header: render through `DialogHeader` with `class: "checkout-trash-header"` (an e2e selector relies on it), the title
   (icon and text) and the total as the summary. `DialogHeader` renders its own `h2` without an id, so put the
   `checkout-trash-title` id on a span inside the title, as tunnel consent does, or the dialog's `aria_labelledby`
   points at nothing. Reconcile `DialogHeader`'s own classes with the existing `.checkout-trash-header` CSS. The header
   close uses the same `close` closure, disabled state and hover explanation as Close (the `.trash-close-help` wrapper
   pattern; widen the `pointer-events: none` rule for disabled Close to cover it).
2. The header close must NOT carry the `.trash-close` class. Initial focus, the Escape fallback, the scroll body's focus
   recovery, `cancel_confirmation`'s fallback and many e2e tests resolve `.trash-close` with a first-match query, and
   the header now comes first in the page: sharing the class would quietly move initial focus and every fallback to the
   header close. Give it a class of its own.
3. Footer: `DialogFooter`, always mounted (the status test holds the progress region's element across a run and checks
   it stays the same node), with the progress status region as its hint (`.dialog-footer-hint`), then `DialogActions`
   with Close and Delete all. Keep the progress region mounted while idle (the existing comment explains why), and keep
   Close's hover wrapper; check that the wrapper does not stop Close from taking the full width where the actions stack.
4. Move the global `ConfirmBand` out of the top of the scroll body into its own `ScrollRegion` between the body and the
   footer, mounted only while that band is shown, with its own selector, name and focus recovery, modeled on the
   launcher's notices region. The band can be taller than the space it gets (long host names), which is why it needs the
   bound and keyboard scroll entry. Its existing on-mount focus and reveal, and cancel returning focus to Delete all,
   stay.
5. Phone width: the shared `.dialog-actions` stack as full-width rows below 560px, so on a phone the footer takes two
   button rows, plus the progress row while deleting; that is the shared behavior "like the other dialogs" means, so use
   it. Two tests open Trash in a 360×300 window (`e2e/tests/trash-dialog.spec.ts`, the global confirmation at that size,
   and `e2e/tests/modal-layout.spec.ts`, margins and keyboard reach to scrollports); they are the main layout risk.
   Header, the bounded confirmation region and the footer must all fit and stay usable there, with the archive list
   still reachable. If they cannot without changing the shared footer or dropping a promise, that is a scope
   reassessment, not a silent tweak; report the phone list height in the report either way.
6. CSS: drop the header-action rules that no longer apply.
7. Comments that become false: the `trash.rs` comment saying Trash deliberately keeps its global controls with its
   totals; `cancel_confirmation`'s doc ("the band lives in the scrolling body"); the `ConfirmBand` on-mount comment
   about not moving the pinned header; the `app.css` comment above `.checkout-trash-dialog .checkout-trash-header`
   ("Only the body scrolls; wrapping the fixed header keeps both buttons usable on phones").
8. SPEC.md's Trash paragraph: replace "The header keeps the title, total, global delete action and close control visible
   while the host list scrolls. Global confirmation appears first in that scroll region ... There is no footer." with
   the new layout, and change "the header announces how many checkouts remain to process" (the status region moves to
   the footer). The header's can and size total changing during a run stay true. SPEC_impl.md: remove the Trash
   exception sentence and say what Trash now shares; keep the focus paragraph accurate.
9. Tests: update `e2e/tests/trash-dialog.spec.ts` (it asserts Delete all in the header and no footer, and locates the
   global band inside the scroll body and checks its exposure against the body; all of that moves to the new region),
   `e2e/tests/modal-layout.spec.ts`'s Trash cases, and any other spec that locates Trash's controls by their old place
   (`e2e/tests/github-checkouts.spec.ts` and `e2e/tests/popover-width-movement.spec.ts` reference them). Add browser
   assertions for the new promises: the footer holds Close and Delete all, the confirmation sits above the footer and
   takes focus, the header close is unavailable during a run and is not where focus starts. The visual-review spec
   (`e2e/docs-shots/visual-review.spec.ts`) asserts today that Trash has no `.dialog-footer`; change it to the new
   layout and add the captures the visual gate needs (it already captures idle, confirmation and phone; mid-deletion is
   new, and `trash-dialog.spec.ts` already holds a delete response mid-run, which is the pattern). If captures must be
   trimmed, drop confirming at phone width, never mid-deletion.
10. Docs: `website/src/content/docs/docs/using/session-list.mdx` says Delete all is "at the top"; fix the wording. The
    docs screenshot of the Trash dialog will show the old layout until the next docs screenshot refresh; do not
    recapture or publish it (those refreshes are separate requests, `.agents/docs-media.md`), and name it in the report
    as a follow-up.
11. Changelog fragment (`kind: changed`), and remove the TODO.md entry in the same PR.

Before touching these paths, read the instruction files root `AGENTS.md`, "Instructions by path", names for them:
`website/AGENTS.md` for the docs page, `.agents/docs-media.md` and `docs/docs-shots/SPEC.md` for the visual-review spec,
and `.agents/validation.md` and `.agents/test-authoring.md` for the tests.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`,
`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, the UI crate's tests, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py format`, and, through `python3 scripts/record-test-run.py`, the changed browser
specs (`trash-dialog`, `modal-layout` narrowed to its Trash cases, `github-checkouts`, `popover-width-movement`) on
Chromium and WebKit. Run `python -B scripts/check-test-sleeps.py` when browser tests change.

## Fallbacks

Departures that need the scope reassessment: changing any deletion behavior or guard; adding a confirmation step;
changing the Trash button in the session list; restyling other dialogs.

Decisions to log: the confirmation's container, anything about the header close control's disabled explanation that
differs from Close's, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-trash-footer-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the code reviewer below, for instance), `$scode-harness-shellout` supplies the
launch mechanics.

The two review gates keep the models and efforts this file demands; the scope reassessment runs at your own model and
effort. The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and
the cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run, capture, sub-agent or review launch. A dead
monitor, or a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead
watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/trash-footer/<nn>-<short-name>`, as the
  outline names them. The outline expects one PR; split only if review shows a real seam.
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments").
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, what changes and what does not,
acceptance criteria, decisions and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Visual review gate

Every PR that changes anything a person can see also gets a visual review before it is finished, in addition to the code
review gate. The maintainer demands exactly this reviewer: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`)
at high effort, able to read images. If it cannot be reached when a review is due and is still unreachable after a
reasonable retry, block the plan as for the code review gate; never substitute another reviewer.

**Captures.** Take them from the PR's head with the visual-review shot spec, `e2e/docs-shots/visual-review.spec.ts`, run
through `scripts/docs-screenshots.sh --only visual-review`: the docs screenshots' staged fleet with a real helm, real
supervisors and real terminals (`docs/docs-shots/SPEC.md`). The PNGs land in the gitignored
`website/public/docs-shots-local/` and are never published or committed. Capture the Trash dialog idle with entries on
more than one host, with Delete all's confirmation open, and mid-deletion (progress showing, Close unavailable), each in
Steel and Steel Light, at a wide window and at phone width; take the same views from `main@origin` as the "before" set.
Extending the spec is part of the PR.

When the capture prerequisites (`docs/docs-shots/SPEC.md`, "Prerequisites") are missing and cannot be set up within the
checkout and the scratch directory, block the plan with that as the question; never skip the gate and never change ssh
configuration or anything else outside the checkout and scratch directory to make it run.

**The prompt** names the PNG files (with what each one shows), another Steel dialog's capture for comparison (Settings
or Rename, so the reviewer can judge consistency), this file's goal and acceptance criteria, and a findings file. It
carries this charter:

> Inspect every image as a person using Farhelm would see it. Report, most severe first: anything broken (overlapping or
> clipped text, misaligned or wrapped controls, content cut off at the shown width, focus or selection that is hard to
> see), anything hard to read (low contrast, text that strains the eye, a status that is hard to tell apart), anything
> inconsistent (a control, dialog or button that does not follow the shared design the comparison dialog shows, the
> wrong font for its role, a color used for the wrong role, a light-mode screen that differs from its dark counterpart
> other than in color), and every departure from the written goal that a person would notice. For each finding give the
> image, where in it, what is wrong, and what it should look like. Say plainly when everything looks right. Edit nothing
> and touch no VCS state.

Fix what it finds, capture again, and repeat the review on the new images until it reports nothing you have not decided
against; log each finding you decide not to follow as a DECISION with the reason.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name, a class name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its code review gate and, where it changes anything visible, its visual review gate, and the last PR removes the
TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
