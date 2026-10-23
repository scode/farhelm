# UI: make Restart with a mode of the session launcher

Written against main at 71dd7eba on 2026-10-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `session-view-fixes-oct22`, `resume-target-fixes-oct22`, `launch-refusal-visibility` and
`per-host-last-folder` have landed. All four change the session launcher or Restart with; building on their settled code
avoids rebasing a large change across them. Read what they changed before starting.

## The goal

Launch-related features keep needing a separate decision for Restart with, and planners keep scoping it out. The TODO.md
entry "Stop treating Restart with as a separate launch surface by default" (Near term) records why: New, Clone, Replace
with and quick launch are one component, the session launcher (`CreateSessionForm` in
`crates/farhelm-ui/src/list/create_form.rs`, with its draft model in `crates/farhelm-ui/src/list/create_form/draft.rs`),
while Restart with is a separate dialog (`crates/farhelm-ui/src/restart_with.rs`, mounted by
`crates/farhelm-ui/src/session_view.rs`) with its own draft, validation, Enter handling, YOLO answer-taking and submit.
Fixes keep landing once per surface.

Since that entry was written, much of the groundwork for unifying them has landed (checked on main at 71dd7eba): both
dialogs render through the shared modal shell (`modal_dialog.rs`, which owns page isolation and the Tab trap); the
conversation field lives in the shared `LaunchControls`; both decide catalog compatibility through
`launch_composer::selection_fits_catalog`; the launcher's draft is one value with named operations
(`launcher-draft-model`, landed 2026-10-22, which left Restart with out as this entry's open question); and Replace's
YOLO question is one shared controller (`replace.rs`), leaving three YOLO answer-taking implementations (launcher,
Restart with, Replace).

Deliver option (a) of the entry: Restart with becomes a mode of the session launcher, and the separate dialog goes away.

### The held-field rule

Restart mode lets the user edit exactly what Restart with edits today: model, effort, permissions, wrapper prefix and
conversation for an agent launch; command, resume command and YOLO answer for a command launch. It adds templates. Every
launcher control that would set anything else (launch kind, agent type, host, folder or destination, session name, first
message) is not shown. When a template, composer search result, recent setup, quick-launch sequence or template shortcut
would set one of those, it is refused with the launcher's existing held-field wording, the way Replace with already
refuses a template that sets the host (`host_fixed`, `validated_template_batch`). Every other launcher feature applies
in restart mode by default; that default is the point of option (a).

### Acceptance criteria

- **Restart mode of the launcher.** Opening Restart with on a session opens the session launcher in restart mode,
  following the held-field rule above, and:
  - the held agent type, host and folder are shown as held, the way Replace with shows its held host;
  - the conversation field is shown, and is required when no resume target was captured, as today;
  - templates are offered as in Replace with: a template that sets a held field is refused with the existing wording;
  - the primary button reads "restart", or "stop and restart" when the agent is working, as today, with the launcher's
    context ("<verb> <agent> · <host> · <folder>"), and is active only when the draft differs from what the session runs
    now (today's Restart with rule);
  - submitting sends the restart (`POST /api/sessions/<id>/restart` with `with` or `with_command`), with today's
    stop-first consent, terminal reattachment (including when the restart is refused and the launcher stays open, as
    SPEC_impl.md requires) and no retry key;
  - command launches keep today's editing of the command and resume command, with byte preservation of unedited text;
  - initial focus is on Cancel (SPEC.md: the Restart with dialog starts on Cancel);
  - Enter follows the launcher's rules, which already cover held and composing Enter and Enter on a choice.
- `restart_with.rs` is deleted, and so are Restart with's own YOLO answer-taking (`RestartYoloQuestion`,
  `take_restart_yolo` and their neighbours in `session_view.rs`): restart mode uses the launcher's YOLO question and
  guarded answer.
- Plain Restart and plain Replace are unchanged: they relaunch the stored launch with no editor.
- **No behavior regressions.** Everything Restart with does today still works: what can be edited, what is sent,
  consent, focus, Enter, refusals and YOLO. Layout and wording follow the launcher. Port every browser test of
  restart-specific behavior to restart mode: held fields, the required conversation, the unchanged-draft rule,
  stop-first consent, reattachment including on a refusal, byte preservation of command fields, the guarded offer only
  while the draft still differs, and initial focus on Cancel. These live in `e2e/tests/restart-with.spec.ts` (31 tests
  on main at 71dd7eba) and in the Restart with cases of `header.spec.ts`, `terminal-restart.spec.ts` and
  `modal-layout.spec.ts`. A test of behavior that becomes generic launcher behavior (Tab trap, gap click, Enter on
  choices, held or composing Enter) may be dropped only when a named launcher test pins the same behavior on the same
  code path; log each such pair. Keep the Rust tests that cover restart
  (`crates/farhelm/tests/e2e/restart_with_resume.rs` and others) passing.
- List the observed differences from the old dialog (templates, the catalog's retry button, layout) in the working log
  and the report.
- SPEC.md: the Restart with bullet under "Lifecycle operations" keeps its restart-specific contract (availability
  without a captured conversation, the required conversation field, harnesses with no resume command, the working-agent
  consent, validation before stopping, how an edited start command applies), says Restart with opens the session
  launcher in restart mode, states the held-field rule once, and loses its own Enter paragraph (the launcher's rules
  apply). The templates sentence "Restart with does not offer templates" changes to the Replace with rule.
  SPEC_impl.md's rationale for the separate dialog ("Restart with uses a separate dialog because it relaunches the
  current session rather than creating one...") is replaced by how restart mode works.
- Restart mode inherits what `launch-refusal-visibility` and `per-host-last-folder` built, as launcher features: it does
  not read or write the per-host folder memory (its folder is held), and whether the "Not launched." wording and the
  pre-launch folder check fit a held-folder restart is yours to decide and log, without a restart-specific variant
  unless the wording is wrong for a restart.
- A `feat:` changelog fragment (Restart with gains templates and the launcher's controls).
- The last code PR removes the TODO.md entry "Stop treating Restart with as a separate launch surface by default." (Near
  term).

## Decisions already made

Settled with the maintainer on 2026-10-22, on the planning page:

- **Option (a)**: Restart with becomes a launcher mode, chosen over a shared draft model with two layouts (b) and over
  two dialogs plus a standing rule (c), because only (a) makes launcher features reach Restart with without anyone
  remembering to wire them.
- **Templates in restart mode, like Replace with**: a template that sets a held field is refused.
- **Order:** after `session-view-fixes-oct22`, `resume-target-fixes-oct22`, `launch-refusal-visibility` and
  `per-host-last-folder`.
- **Plain Restart and plain Replace stay outside**: they have no editor.
- **Reviewer:** opus at high effort (see Review gate). If it cannot be reached when a review is due and is still
  unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one plan" (step 10),
  after pushing what is consistent. No substitute reviewer.
- No-workhorse mode.

## Requirement sources

**The user's request:** the TODO.md entry quoted above (it carries the full history and the three options), and the
decisions above.

**Binding repository constraints:** SPEC.md's launcher passages under "Creation", the Restart with and Replace with
bullets under "Lifecycle operations", the templates passages, the YOLO confirmation and guarded-answer passages, and the
focus rules; SPEC_impl.md's launcher, retry-key, modal and reattachment passages; root `AGENTS.md`, "Tests and
validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md` for
tests.

**Planner proposals** (from a read of main at 71dd7eba, revised after a planning review; change them if the code says
otherwise, logged as a DECISION):

- Replace with is marked today by `CreatePrefill::replace_source`, from which the form derives `is_replace_with`, which
  drives `host_fixed` in template checks, the submit verb and the submit routing. A small launcher mode value (for
  example create, replace, restart) that says which fields are held and what submit does is the natural generalization;
  prefer it over a second boolean.
- Open restart mode from the session view through the existing `HeaderPrefillRequest`
  (`crates/farhelm-ui/src/list/view.rs`, already used by header Clone and Replace with and by the interrupted card),
  adding a restart variant, rather than a second channel. Every restart reply, success or refusal, must still reach the
  session view's reattachment.
- Restart with's "changed since opening" baseline maps onto `LaunchControls`' existing `baseline` prop.

## Outline

A suggested stack; reshape it if the code says otherwise:

1. `refactor:` a launcher mode value replacing the Replace with boolean, behavior unchanged.
2. `feat:` restart mode end to end (the held-field rule, open through `HeaderPrefillRequest`, edit, submit, YOLO,
   consent, reattach) behind the Restart with action, with the ported browser tests.
3. `refactor:` delete `restart_with.rs` and Restart with's YOLO copy; SPEC.md and SPEC_impl.md; changelog fragment;
   TODO.md entry removed.

Avoid churn: no PR adds paths nothing uses yet, and none builds helpers in the old dialog that the next PR deletes.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` (including the UI's web target), the UI unit tests through
`python3 scripts/record-test-run.py`, the ported Restart with browser specs, the launcher's own specs (New, Clone,
Replace with, quick launch, templates, YOLO), and the Rust end-to-end restart tests.

## Fallbacks

Examples of departures that need the scope reassessment: a generic form framework, keeping `restart_with.rs` alongside
restart mode, changing plain Restart or plain Replace, a new helm restart request (for example to carry a session name),
or showing a held control in restart mode.

Decisions to log: the mode representation, how restart replies reach reattachment, every ported or dropped test and why,
the observed differences from the old dialog, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-restart-with-as-launcher-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewers included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from these rules and run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/restart-with-as-launcher/<nn>-<short-name>` (for example `plan/restart-with-as-launcher/01-launcher-mode`).
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit for `feat` and `fix`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on opus at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch mechanics). If
opus cannot be reached, block as Decisions already made says; never substitute another model.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

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
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
