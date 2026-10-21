# YOLO confirmation: one click to start the same launch guarded instead

Written against main at 2897c914 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change.

This plan runs after `ui-peer-text-type.md`: the queue holds it back until that plan has landed on main. That plan
converts host names and other text from remote machines in the UI to a type of its own, including the host name the YOLO
confirmation shows and much of `crates/farhelm-ui/src/list/create_form.rs`; building on it avoids rebasing a small
change across a large conversion. It deliberately does not run after `ui-session-actions.md` (see Outline, "Why not
after `ui-session-actions.md`").

## The goal

When a launch on a host that asks before YOLO launches is refused for confirmation, the red "Confirm YOLO launch" box
offers three answers: "start YOLO session anyway", "start, and don't ask again on this host", and "cancel". A user who
would rather keep the agent's own safety checks has to cancel, pick **guarded** in the permissions row, and launch
again. Add a fourth answer, **start guarded session instead**, that switches the same launch to guarded and starts it in
one click.

TODO.md entry this plan covers (Near term): "**Launch guarded from the YOLO confirmation.** When the session launcher
shows the confirmation for a YOLO launch on a host that asks first, offer a one-click "launch guarded instead" button
next to "start YOLO session anyway", so switching to a safer launch does not mean cancelling, changing the permission
and launching again. UI details TBD."

### Acceptance criteria

- **Where it appears.** In the YOLO confirmation drawn by the session launcher (New, Clone, Replace with and quick
  launch all run through it) and by the Restart with dialog. Plain Replace (sidebar and session header) reuses the
  session's stored launch and has no permission choice to change, so its confirmation is unchanged. Plain Restart never
  asks: the helm asks for YOLO confirmation only on a restart with new settings, since a plain restart relaunches a
  choice already made.
- **When it appears.** Only when the refused launch is a YOLO launch because the user chose YOLO explicitly as the
  permission (the confirmation's "chosen permission" reason), and the launch's agent offers a guarded mode. Whether an
  agent offers guarded comes from the one per-harness permission table (`offers_permission` in
  `crates/farhelm-proto/src/launch.rs`), never from a list of harnesses at the call site (root `AGENTS.md`, "Code
  rules"). Today that table gives guarded to Claude, Codex, Muse, Grok and OMP; Pi, OpenCode, Goose and Cursor, custom
  permission flags, and command launches get no button.
- **What it does.** It answers this opening of the question the way the existing answers do (an answer drawn for an
  earlier opening, or one that arrives after Cancel in the same burst, does nothing). It records no YOLO consent and
  does not mark the host as safe. It applies **guarded** to the draft exactly as clicking guarded in the permissions row
  would (the same draft-transition check, the same clearing of custom flags, the same "changed from the template" marker
  when a template or prefill had set YOLO), clears the question's error, and submits the draft again, with no YOLO
  confirmation flag. In the launcher the resubmitted request is a new launch request: changing the permission retires
  the refused request's retry key, as it does by hand today. If the draft transition is refused (a busy operation),
  nothing happens and the question stays.
- **Restart with, when guarded is what the session already had.** Restart with does not submit a draft equal to the
  session's stored launch (its submit returns early, as its own submit button is disabled then). If the session was
  launched guarded and the user changed only the permission to YOLO, switching back to guarded would make the draft
  equal the stored launch, and the button would do nothing. So in Restart with the button is offered only when applying
  guarded leaves the draft different from the session's stored launch; otherwise it is not shown, and the existing
  answers stay. Cover this case with a test.
- **Appearance.** The button is labelled **start guarded session instead**. It is the first button in the actions row,
  before "start YOLO session anyway", styled as the blue primary button (`btn btn-primary`), while the two YOLO answers
  stay red. **Cancel keeps the initial focus**, so Enter in the box still cancels (SPEC_impl's "YOLO and Restart with
  keep their Cancel default" is untouched). Tooltip, in the style of the existing three: "start this session in guarded
  mode instead, where the agent's own safety checks stay on" (final wording is yours; log it). The heading, consequence,
  reason and "asks before every YOLO launch" lines are unchanged.
- **Afterwards.** In the launcher, a successful guarded launch is remembered like any successful launch (the next
  launcher opens on guarded where it would remember the last choice); no special case. In Restart with, the dialog's own
  permission row shows guarded and the restart proceeds as a Restart with guarded would; it changes no launcher memory
  beyond what Restart with already does.
- SPEC.md's YOLO paragraph in "Creation" (around SPEC.md:857-879) gains one sentence: where the agent offers a guarded
  mode and the user chose YOLO, the confirmation also offers starting the same launch guarded instead, which needs no
  confirmation. The module docs of `crates/farhelm-ui/src/yolo_confirm.rs` describe the fourth answer. The docs page
  `website/src/content/docs/docs/using/start-a-session.mdx` ("Confirm a YOLO launch") mentions it, and its screenshot
  alt text, which describes the buttons, is updated in words.
- The docs screenshot `start-a-session/yolo-confirmation` becomes out of date. Do not re-capture it (re-capture is a
  separate maintainer-triggered flow in `.agents/docs-media.md`); name it as stale in the report.
- A changelog fragment, `kind: added`.
- The last code PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-20), from the planning page:**

- Label: "start guarded session instead", chosen over "launch guarded instead" and "use guarded instead".
- When: "Only for an explicit YOLO choice on an agent that has guarded", chosen over also offering it for custom
  permission flags (switching would drop the user's custom flags) and over a Goose-specific variant.
- Flows: "The launcher plus Restart with", chosen over the launcher only. Alongside that pick the maintainer asked
  separately for a TODO entry about why Restart keeps being treated as a special case; that is not this plan's work.
- Placement: "First in the row, blue, Cancel keeps focus", chosen over placing it after "start YOLO session anyway" in a
  neutral style and over making it the focused default.
- Memory: "Yes, no special case" (the launcher remembers guarded after a successful guarded launch).
- Review gate and mode are in the run instructions below.

**Binding repository constraints:**

- SPEC.md, "Creation", the YOLO confirmation paragraph (the three answers, "start, and don't ask again" setting the host
  first, the retry-with-override rule).
- SPEC_impl.md's GUI passages on the YOLO confirmation and the Cancel default.
- Root `AGENTS.md`, "Code rules" (per-harness behavior answered in the named places, no harness lists at call sites),
  "Changelog fragments", "Tests and validation"; `.agents/test-authoring.md`; `website/AGENTS.md` before editing the
  docs page; `.agents/docs-media.md` for why the screenshot is not re-captured here.

**Planner proposals (checked by a fresh-context planning review):** the outline below. A planner proposal is not a
requirement: a smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main at 2897c914 (verify them; `ui-peer-text-type.md` will have moved some
of this code):

- The shared component is `YoloConfirmation` in `crates/farhelm-ui/src/yolo_confirm.rs` (actions row around 268-292;
  `YoloAsk` around 110-119 carries `host`, `host_name` and `reason`, no harness; `YoloReason::of_launch` around 66-88
  derives the reason from the sent `LaunchSelection`). Callers: the launcher (`create_form.rs`, render site around
  6004-6056, question state around 2869-2905, `answer_launcher_yolo` and `LauncherYoloQuestion` around 1296-1324, the
  refusal-to-ask conversion around 5707-5730 where the sent selection is in hand), the sidebar's plain Replace
  (`list/view.rs` around 4327), the session header's plain Replace (`session_view.rs` around 2756), and Restart with
  (`restart_with.rs` around 717-745, fed by `session_view.rs` around 2647-2655, whose ask is built around 1448).
- The launcher's resubmit is `resubmit_composer()` (`create_form.rs` around 1270). The permissions row's handler (around
  7121-7133) holds the logic to reuse for applying guarded; factor it into one helper both call rather than copying it.
- Restart with keeps its own draft `selection` signal; its permissions handler is around `restart_with.rs:698-704`, and
  its `primary` callback (around 424-436) submits with `(confirm_yolo, stop_asking)`. The new answer sets guarded
  through the same permission logic, then calls `primary` with no confirmation and no stop-asking.
- `LaunchPermission` and `offers_permission` are in `crates/farhelm-proto/src/launch.rs` (`offers_permission` around
  418). The UI word is lowercase "guarded".

Proposed shape (planner proposal):

1. One gate, not two: `YoloConfirmation` gains an optional `on_launch_guarded` handler and renders the button exactly
   when it is supplied. A small helper beside `YoloReason` decides whether a caller may supply it: the ask's reason is
   the explicit-YOLO reason and the launch's harness offers guarded. Every caller already knows the harness (the
   launcher from the selection it sent, Restart with from its draft), so `YoloAsk` needs no new field. Plain Replace
   passes nothing and is unchanged.
2. The launcher supplies the handler, using the shared permission helper and the same stale-opening guard as the other
   answers; Restart with supplies its own, subject to the stored-launch rule in the acceptance criteria.

Slicing: one PR, `feat: offer a guarded launch from the YOLO question`, with tests, SPEC, docs and fragment. Split
Restart with into a second PR if the first grows large; log the choice.

### Why not after `ui-session-actions.md`

That queued plan (pending, behind several others) consolidates the sidebar's and header's copies of plain Replace and
its YOLO question into one controller, and lets Restart or Delete join only if its inventory finds the same flow
duplicated there. It does not plan to change Restart with's dialog or the launcher's question, which are the two places
this plan wires. A dependency would hold this plan back for a speculative overlap; the cost of not waiting is at most a
rebase in `session_view.rs` for whichever lands second. If, when you start, that plan has already landed and moved the
Restart with question into its controller, wire the handler there instead and log it.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the UI crate's unit tests, and the browser specs below through the recorder
on both engines; `python -B scripts/check-test-sleeps.py` for changed browser tests; the website checks
`website/AGENTS.md` names for the docs edit; `python3 releasing/check-changelog.py format`.

New tests:

- Unit: the availability helper, one case per branch (explicit YOLO on a harness that offers guarded; on one that does
  not; custom flags; a command launch). It does not re-test the `offers_permission` table harness by harness.
- Browser (`e2e/tests/yolo-guard.spec.ts`, helper `openYoloLaunch`): a Codex YOLO launch on an asking host shows the
  question; "start guarded session instead" sends exactly one more create, without the YOLO confirmation flag and with
  `guarded` permissions; one session results; the dialog closes; the host is not marked as starting YOLO without asking;
  Cancel still has initial focus.
- Browser: Cancel then the guarded answer in one burst starts nothing (same shape as the existing burst test).
- Browser: the button is absent for custom flags (the per-agent absences belong to the unit test).
- Browser (`e2e/tests/restart-with.spec.ts`): Restart with choosing YOLO on an asking host, answered with the guarded
  button, restarts with guarded; and for a session launched guarded whose only change is YOLO, the button is not shown.

Other specs click `.yolo-confirm` by class (`quick-launch`, `custom-permissions`, `replace`, `launcher-enter`,
`terminal-create-idempotency`, `terminal-restart`); a new button with its own class must not break them, but run them.

## Fallbacks

No complexity gate. Departures that need the scope reassessment: offering the button in plain Replace, a new kind of
question or dialog, or changing how the launcher's retry key is kept or retired.

Decisions to log: the tooltip wording, where the availability helper lives (if you end up putting the fact on `YoloAsk`
after all, say why), the slicing, and the dependency reasoning above if the situation on main differs.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-yolo-confirm-launch-guarded-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; the user chose this mode explicitly, so if galaxy-brain is already active when you
start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

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

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/yolo-confirm-launch-guarded/<nn>-<short-name>` (for example
  `plan/yolo-confirm-launch-guarded/01-guarded-answer`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

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
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
