# Agent host-add requests wait as a card instead of opening a dialog

Written against main at 8bf05dd3 on 2026-11-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Stop an agent's host-add request from opening its dialog by itself.** Found by a code reading on 2026-11-10 while
> settling the keyboard focus rulebook (SPEC.md, "Keyboard focus"). In the focused window, an agent's
> `farhelm agent host add` request opens the Add host dialog by itself, even while the user types in a terminal, and
> moves focus into it. Once the 700 ms approval pause ends, a fresh Enter clicks Add, which approves the request; a
> dialog that opened after waiting behind another dialog gets no pause at all, and Enter in its destination field
> approves at once. An approved destination that already runs Farhelm is added with no further prompt. SPEC.md now says
> every window shows the request as a card whose action opens the dialog, and the request never opens the dialog by
> itself.

The maintainer's request was to check that route and queue the fix through the planning system. SPEC.md already states
the target behavior (it changed on 2026-11-10, #3316): the code is behind it.

### What the PR does

Today every window, focused or not, already shows the host-add approval card with its "Open add dialog" button
(`crates/farhelm-ui/src/approvals.rs`, around 302-310 and 580). On top of that, the focused window queues an automatic
opening when a new request appears (`approvals.rs`, around 145-180: `seen_adds`, `candidates`, the `document.hasFocus()`
sample, `AgentAddOpening { automatic: true, .. }`), and `crates/farhelm-ui/src/hosts.rs` (around 3063-3100) shows it
through `crate::modal_isolation::HostModal` with `require_active: opening.automatic` and `on_rejected`.

The fix removes the automatic opening. A new request only shows its card, in every window; the dialog opens only when
the user clicks the card's open action. Once the user has opened it, nothing about the dialog changes.

### Acceptance criteria

1. A newly arriving agent host-add request never opens the Add host dialog by itself, in any window, whether or not the
   window is focused and whether or not another dialog is open. It shows its card, and the card's open action is the
   only way the dialog opens.
2. With focus in a session's terminal when a request arrives, focus stays in the terminal, no dialog appears, and key
   presses (Enter included) answer nothing. A browser test shows this on Chromium and WebKit.
3. Opening the dialog from the card behaves as today: the arm pause, Allow and Deny semantics, edited destination, the
   setup prompt that follows (still starting on "Set up"), and the opening that waits behind another dialog (see
   Decisions).
4. SPEC_impl.md describes the new behavior. The automatic-open sentences in the host-add paragraph (search "Newly
   observed requests automatically open only in the live focused visible window") are rewritten, and the host-add clause
   of the "does not yet follow" note after "Other asynchronous GUI paths predate these rules" is replaced by an honest
   statement of the remaining deferred opening (see Decisions).
5. A changelog fragment says what changed for someone running Farhelm.
6. The PR removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-10 planning page:

- **Card only.** The focused window handles a new request exactly like any other window: card, explicit open. The
  rejected alternative was keeping the automatic opening and adding focus and fresh-key-press guards; it contradicts
  SPEC.md "Keyboard focus" (things the user did not start never take focus or open over what the user is doing) and adds
  a mechanism where the fix can delete one.
- **The setup prompt stays as it is.** After the user approves in a dialog they opened, the setup prompt keeps starting
  on "Set up": SPEC.md "Keyboard focus" treats it as a confirmation the user just triggered.
- **Keep the deferred explicit open.** Cards stay clickable over other dialogs. When the user clicks "Open add dialog"
  while another dialog, a manual add draft or a pending add submission is in the way, the dialog still opens once that
  clears, as today. The user asked for it, and refusing the click would need a click-time check for state that lives in
  the hosts panel. Say so plainly in SPEC_impl.md's "does not yet follow" note: this one case still opens later, after
  the user may have moved on.
- **No keyboard shortcut** to reach or answer the card. SPEC.md "Keyboard focus" leaves that unsettled.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending.**

## Requirement sources

**The user's request and decisions:** the TODO entry and Decisions above.

**Binding repository constraints:** SPEC.md "Keyboard focus" (rule on things the user did not start) and the agent
host-add paragraph (search "Agents can also add, change and remove hosts"); SPEC_impl.md's host-add paragraph and the
note above; root `AGENTS.md`, "Tests and validation", "Changelog fragments", "Conventional Commits";
`.agents/test-authoring.md`.

**Planner proposals** (from a fresh-context scope review of the code at 8bf05dd3; change them if the code says
otherwise, logged as a DECISION):

- Delete in `approvals.rs`: `seen_adds` and `candidates` (they exist only to spot new arrivals for the automatic path),
  the `hasFocus` sample, the `automatic` field and its initializer; rewrite the comment that describes the automatic
  path. Keep `agent_add.opening` (the card's button uses it), `answering`, `armed`, the expiry retain and the dequeue
  effect that serves the deferred explicit open.
- Delete `HostModal`'s `require_active` and `on_rejected` props, their JavaScript check and their doc in
  `modal_isolation.rs`: `hosts.rs` around 3063-3075 is their only caller. The wait for other dialogs stays; every other
  `HostModal` caller needs it. Confirm that nothing else fires `on_rejected` before deleting it.
- Browser tests that rely on the automatic opening are mostly in `e2e/tests/approval-layout.spec.ts`: the focused-add,
  inactive-add, manual-draft, simultaneous-adds, answer-status, winning-window and every-kind layout cases, plus the
  window-focus helpers that then lose their callers. Each gets the click, or is merged or deleted where the focused and
  inactive cases now coincide. `e2e/tests/provisioning.spec.ts` "agent host add asks before setup" needs the click too.
  Put the new regression test in `approval-layout.spec.ts`; asserting that the terminal keeps focus, no dialog appears
  and nothing is answered after a key press is enough.
- Changelog: host add shipped in a stable release, so a fragment is due. Choose `fixed` or `changed` by what a user
  sees: the dialog no longer pops up on its own.

## Outline

One PR: `plan/agent-host-add-card/01-card-only`, `fix:` with a changelog fragment.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-ui`, the affected UI unit tests through `python3 scripts/record-test-run.py`, every browser spec you change on
Chromium and WebKit, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, and
`python3 releasing/check-changelog.py format`.

## Fallbacks

If another plan has moved the code this file names, follow the current code and correct this file's names and line
numbers to the truth, logged as a DECISION. If main has already removed the automatic opening, the plan is done except
for whatever criteria remain; finish those.

Examples of departures that need the scope reassessment: changing the approval protocol or the host-add answer route,
changing the arm pause, or reworking other approval kinds.

Decisions to log: what was deleted and what was kept, how each changed test was handled, the changelog kind, and every
review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-agent-host-add-card-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
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
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

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

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/agent-host-add-card/<nn>-<short-name>`,
  as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- The plan's last code PR removes this plan's TODO.md entry.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

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

The plan is complete when a linear stack of draft PRs exists in the order the outline gives, together meeting the
acceptance criteria above (as refined by Decisions already made), each having passed its review gate, with the last one
removing this plan's TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its
report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
